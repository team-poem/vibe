use crate::features::{FlatnessAnalyzer, rms_db};
use crate::floor::AdaptiveFloor;

#[derive(Debug, Clone, Copy)]
pub struct ClapEvent {
    pub timestamp_ms: u64,
    pub peak_db: f32,
    pub above_floor_db: f32,
    pub flatness: f32,
    pub confidence: f32,
}

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

pub fn detect(samples: &[f32], sample_rate: u32) -> Vec<ClapEvent> {
    detect_with_config(samples, sample_rate, &DetectorConfig::default())
}

pub fn detect_with_config(
    samples: &[f32],
    sample_rate: u32,
    config: &DetectorConfig,
) -> Vec<ClapEvent> {
    let frame_size = ((sample_rate as f32) * (config.frame_ms / 1000.0)).round() as usize;
    if frame_size == 0 || samples.len() < frame_size {
        return Vec::new();
    }

    let decay_frames = (config.decay_window_ms / config.frame_ms).ceil() as usize;
    let refractory_frames = (config.refractory_ms / config.frame_ms).ceil() as usize;

    let frames: Vec<&[f32]> = samples.chunks_exact(frame_size).collect();
    let frame_dbs: Vec<f32> = frames.iter().map(|f| rms_db(f)).collect();

    let mut floor = AdaptiveFloor::new(config.initial_floor_db, config.floor_alpha);
    let mut analyzer = FlatnessAnalyzer::new(config.fft_size);
    let mut events: Vec<ClapEvent> = Vec::new();
    let mut next_allowed_frame: usize = 0;

    for (idx, frame) in frames.iter().enumerate() {
        let db = frame_dbs[idx];
        let above = db - floor.current_db();
        let is_candidate = db.is_finite() && above >= config.onset_threshold_db;

        if !is_candidate {
            floor.update(db);
            continue;
        }

        if idx < next_allowed_frame {
            continue;
        }

        let flatness = analyzer.flatness(frame);
        if flatness < config.flatness_threshold {
            continue;
        }

        let lookahead_end = (idx + decay_frames + 1).min(frames.len());
        let post_min_db = frame_dbs[idx + 1..lookahead_end]
            .iter()
            .copied()
            .filter(|d| d.is_finite())
            .fold(f32::INFINITY, f32::min);

        let drop = if post_min_db.is_finite() {
            db - post_min_db
        } else {
            0.0
        };
        if drop < config.decay_drop_db {
            continue;
        }

        let timestamp_ms = (idx as f32 * config.frame_ms).round() as u64;
        let confidence = compute_confidence(above, flatness, drop, config);
        events.push(ClapEvent {
            timestamp_ms,
            peak_db: db,
            above_floor_db: above,
            flatness,
            confidence,
        });

        next_allowed_frame = idx + refractory_frames;
    }

    events
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

    fn pseudo_noise(seed: u32) -> f32 {
        let x = seed
            .wrapping_mul(1_103_515_245)
            .wrapping_add(12_345)
            .wrapping_mul(2_654_435_761);
        ((x >> 8) as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    #[test]
    fn ignores_silence() {
        let sample_rate = 48_000u32;
        let samples = vec![0.0f32; sample_rate as usize];
        let events = detect(&samples, sample_rate);
        assert!(events.is_empty());
    }

    #[test]
    fn ignores_low_amplitude_steady_noise() {
        let sample_rate = 48_000u32;
        let samples: Vec<f32> = (0..sample_rate).map(|i| pseudo_noise(i) * 0.001).collect();
        let events = detect(&samples, sample_rate);
        assert!(events.is_empty(), "got {events:?}");
    }

    #[test]
    fn ignores_sustained_loud_signal() {
        let sample_rate = 48_000u32;
        let samples: Vec<f32> = (0..sample_rate)
            .map(|i| (std::f32::consts::TAU * 440.0 * i as f32 / sample_rate as f32).sin() * 0.5)
            .collect();
        let events = detect(&samples, sample_rate);
        assert!(events.is_empty(), "got {events:?}");
    }
}
