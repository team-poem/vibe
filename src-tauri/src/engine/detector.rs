use crate::engine::event::ClapEvent;
use crate::engine::features::{rms_db, FlatnessAnalyzer};
use crate::engine::floor::AdaptiveFloor;

#[derive(Debug, Clone, Copy)]
pub struct DetectorConfig {
    pub frame_ms: f32,
    pub fft_size: usize,
    pub initial_floor_db: f32,
    pub floor_alpha: f32,
    pub onset_threshold_db: f32,
    pub flatness_threshold: f32,
    pub decay_window_ms: f32,
    pub decay_drop_db: f32,
    pub refractory_ms: f32,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            frame_ms: 10.0,
            fft_size: 512,
            initial_floor_db: -60.0,
            floor_alpha: 0.05,
            onset_threshold_db: 32.0,
            flatness_threshold: 0.20,
            decay_window_ms: 60.0,
            decay_drop_db: 20.0,
            refractory_ms: 120.0,
        }
    }
}

/// A candidate onset waiting for its decay gate to confirm or expire.
struct PendingCandidate {
    frame_index: u64,
    peak_db: f32,
    above_floor_db: f32,
    flatness: f32,
    post_frames_seen: u64,
}

/// Stateful clap detector fed by arbitrary-size sample chunks from a live
/// microphone stream. Keeps the adaptive noise floor, refractory window,
/// and decay lookahead across calls.
///
/// A clap is confirmed as soon as a post-onset frame drops by
/// `decay_drop_db`, instead of waiting for the full decay window, so the
/// event fires at most one decay window (~60 ms) after the physical clap.
pub struct StreamingDetector {
    config: DetectorConfig,
    frame_size: usize,
    decay_frames: u64,
    refractory_frames: u64,
    leftover: Vec<f32>,
    frame_index: u64,
    floor: AdaptiveFloor,
    analyzer: FlatnessAnalyzer,
    next_allowed_frame: u64,
    pending: Option<PendingCandidate>,
}

impl StreamingDetector {
    pub fn new(sample_rate: u32, config: DetectorConfig) -> Self {
        let frame_size = ((sample_rate as f32) * (config.frame_ms / 1000.0)).round() as usize;
        let decay_frames = (config.decay_window_ms / config.frame_ms).ceil() as u64;
        let refractory_frames = (config.refractory_ms / config.frame_ms).ceil() as u64;
        Self {
            config,
            frame_size: frame_size.max(1),
            decay_frames,
            refractory_frames,
            leftover: Vec::new(),
            frame_index: 0,
            floor: AdaptiveFloor::new(config.initial_floor_db, config.floor_alpha),
            analyzer: FlatnessAnalyzer::new(config.fft_size),
            next_allowed_frame: 0,
            pending: None,
        }
    }

    /// Feed the next chunk of mono samples and return any claps confirmed
    /// within it. Chunk size does not need to align with frame boundaries.
    pub fn push(&mut self, samples: &[f32]) -> Vec<ClapEvent> {
        self.leftover.extend_from_slice(samples);
        let buffered = std::mem::take(&mut self.leftover);

        let mut events = Vec::new();
        let mut chunks = buffered.chunks_exact(self.frame_size);
        for frame in &mut chunks {
            if let Some(event) = self.process_frame(frame) {
                events.push(event);
            }
        }
        self.leftover = chunks.remainder().to_vec();
        events
    }

    fn process_frame(&mut self, frame: &[f32]) -> Option<ClapEvent> {
        let idx = self.frame_index;
        self.frame_index += 1;
        let db = rms_db(frame);

        if let Some(mut candidate) = self.pending.take() {
            candidate.post_frames_seen += 1;
            let drop_db = candidate.peak_db - db;

            if db.is_finite() && drop_db >= self.config.decay_drop_db {
                self.next_allowed_frame = candidate.frame_index + self.refractory_frames;
                self.floor.update(db);
                return Some(self.confirm(candidate, drop_db));
            }

            if candidate.post_frames_seen < self.decay_frames {
                self.pending = Some(candidate);
                let above = db - self.floor.current_db();
                if !db.is_finite() || above < self.config.onset_threshold_db {
                    self.floor.update(db);
                }
                return None;
            }
            // Decay gate failed: discard and let this frame open a new candidate.
        }

        let above = db - self.floor.current_db();
        let is_candidate = db.is_finite() && above >= self.config.onset_threshold_db;
        if !is_candidate {
            self.floor.update(db);
            return None;
        }
        if idx < self.next_allowed_frame {
            return None;
        }

        let flatness = self.analyzer.flatness(frame);
        if flatness < self.config.flatness_threshold {
            return None;
        }

        self.pending = Some(PendingCandidate {
            frame_index: idx,
            peak_db: db,
            above_floor_db: above,
            flatness,
            post_frames_seen: 0,
        });
        None
    }

    fn confirm(&self, candidate: PendingCandidate, drop_db: f32) -> ClapEvent {
        let timestamp_ms = (candidate.frame_index as f32 * self.config.frame_ms).round() as u64;
        let confidence = compute_confidence(
            candidate.above_floor_db,
            candidate.flatness,
            drop_db,
            &self.config,
        );
        ClapEvent {
            timestamp_ms,
            peak_db: candidate.peak_db,
            above_floor_db: candidate.above_floor_db,
            flatness: candidate.flatness,
            confidence,
        }
    }
}

fn compute_confidence(above: f32, flatness: f32, drop_db: f32, config: &DetectorConfig) -> f32 {
    let onset_score = ((above - config.onset_threshold_db) / 30.0).clamp(0.0, 1.0);
    let flat_score = ((flatness - config.flatness_threshold)
        / (1.0 - config.flatness_threshold).max(1e-3))
    .clamp(0.0, 1.0);
    let drop_score = ((drop_db - config.decay_drop_db) / 30.0).clamp(0.0, 1.0);
    (onset_score * 0.4 + flat_score * 0.3 + drop_score * 0.3).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 48_000;

    fn pseudo_noise(seed: u32) -> f32 {
        let x = seed
            .wrapping_mul(1_103_515_245)
            .wrapping_add(12_345)
            .wrapping_mul(2_654_435_761);
        ((x >> 8) as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Deterministic white noise with a stateful LCG. Unlike `pseudo_noise`
    /// (which is nearly DC and useful only for negative tests), this is
    /// spectrally flat and passes the broadband gate.
    fn white_noise(len: usize, seed: u32) -> Vec<f32> {
        let mut x = seed;
        (0..len)
            .map(|_| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (((x >> 16) & 0x7fff) as f32 / 32_768.0) * 2.0 - 1.0
            })
            .collect()
    }

    fn quiet_floor_then_burst() -> Vec<f32> {
        // One second of quiet noise to settle the floor, a 30 ms broadband
        // burst, then quiet again so the decay gate can confirm.
        let mut samples: Vec<f32> = white_noise(SAMPLE_RATE as usize, 1)
            .into_iter()
            .map(|s| s * 0.0005)
            .collect();
        let burst_len = (SAMPLE_RATE as usize) * 30 / 1000;
        samples.extend(white_noise(burst_len, 2).into_iter().map(|s| s * 0.8));
        samples.extend(
            white_noise(SAMPLE_RATE as usize / 2, 3)
                .into_iter()
                .map(|s| s * 0.0005),
        );
        samples
    }

    fn push_in_chunks(detector: &mut StreamingDetector, samples: &[f32]) -> Vec<ClapEvent> {
        // Deliberately misaligned chunk size to exercise leftover buffering.
        samples
            .chunks(479)
            .flat_map(|chunk| detector.push(chunk))
            .collect()
    }

    #[test]
    fn ignores_silence() {
        let mut detector = StreamingDetector::new(SAMPLE_RATE, DetectorConfig::default());
        let samples = vec![0.0f32; SAMPLE_RATE as usize];
        assert!(push_in_chunks(&mut detector, &samples).is_empty());
    }

    #[test]
    fn ignores_low_amplitude_steady_noise() {
        let mut detector = StreamingDetector::new(SAMPLE_RATE, DetectorConfig::default());
        let samples: Vec<f32> = (0..SAMPLE_RATE).map(|i| pseudo_noise(i) * 0.001).collect();
        let events = push_in_chunks(&mut detector, &samples);
        assert!(events.is_empty(), "got {events:?}");
    }

    #[test]
    fn ignores_sustained_loud_signal() {
        let mut detector = StreamingDetector::new(SAMPLE_RATE, DetectorConfig::default());
        let samples: Vec<f32> = (0..SAMPLE_RATE)
            .map(|i| (std::f32::consts::TAU * 440.0 * i as f32 / SAMPLE_RATE as f32).sin() * 0.5)
            .collect();
        let events = push_in_chunks(&mut detector, &samples);
        assert!(events.is_empty(), "got {events:?}");
    }

    #[test]
    fn detects_broadband_burst_over_quiet_floor() {
        let mut detector = StreamingDetector::new(SAMPLE_RATE, DetectorConfig::default());
        let samples = quiet_floor_then_burst();
        let events = push_in_chunks(&mut detector, &samples);
        assert_eq!(events.len(), 1, "got {events:?}");
        let event = &events[0];
        assert!(event.above_floor_db >= 32.0);
        assert!(event.flatness >= 0.20);
    }

    #[test]
    fn state_carries_across_pushes() {
        let config = DetectorConfig::default();
        let mut whole = StreamingDetector::new(SAMPLE_RATE, config);
        let mut split = StreamingDetector::new(SAMPLE_RATE, config);

        let samples = quiet_floor_then_burst();

        let whole_events = whole.push(&samples);
        let split_events = push_in_chunks(&mut split, &samples);

        assert_eq!(whole_events.len(), split_events.len());
        for (a, b) in whole_events.iter().zip(split_events.iter()) {
            assert_eq!(a.timestamp_ms, b.timestamp_ms);
        }
    }
}
