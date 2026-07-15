use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use crate::audio::{self, AudioChunk};
use crate::engine::detector::{DetectorConfig, StreamingDetector};
use crate::engine::features::rms_db;
use crate::engine::matcher::{MatcherConfig, StreamingMatcher, TriggerEvent};
use crate::engine::Sensitivity;

/// Post-trigger quiet gate. Claps are impulses: after the second clap the
/// room drops back to the noise floor almost immediately, while sustained
/// noise that slipped through the burst gates (hair dryers, vacuums,
/// running water) stays loud. A matched trigger is therefore held for a
/// short confirmation window and discarded if too much of that window is
/// loud — trading ~300 ms of latency for immunity against the whole class
/// of sustained-noise false positives.
const QUIET_GATE_WINDOW_MS: u64 = 300;
/// Loudness margin over the adaptive floor that counts as "still noisy".
/// Clap reverb decays within a few tens of ms; sustained sources sit far
/// above this for the entire window.
const QUIET_GATE_MARGIN_DB: f32 = 12.0;
/// Loud time tolerated inside the window (reverb tail allowance).
const QUIET_GATE_LOUD_LIMIT_MS: u64 = 90;

struct QuietGate {
    trigger: TriggerEvent,
    remaining_ms: u64,
    loud_ms: u64,
}

enum GateVerdict {
    Pending,
    Fire(TriggerEvent),
    Discard,
}

impl QuietGate {
    fn new(trigger: TriggerEvent) -> Self {
        Self {
            trigger,
            remaining_ms: QUIET_GATE_WINDOW_MS,
            loud_ms: 0,
        }
    }

    fn feed(&mut self, above_floor_db: f32, chunk_ms: u64) -> GateVerdict {
        if above_floor_db.is_finite() && above_floor_db >= QUIET_GATE_MARGIN_DB {
            self.loud_ms += chunk_ms;
        }
        if self.loud_ms > QUIET_GATE_LOUD_LIMIT_MS {
            return GateVerdict::Discard;
        }
        self.remaining_ms = self.remaining_ms.saturating_sub(chunk_ms);
        if self.remaining_ms == 0 {
            GateVerdict::Fire(self.trigger)
        } else {
            GateVerdict::Pending
        }
    }
}

/// Events surfaced by the detection pipeline to the application layer.
#[derive(Debug)]
pub enum EngineEvent {
    Trigger(TriggerEvent),
    CaptureFailed(String),
}

/// Handle to the running capture → detect → match pipeline.
///
/// Thread layout (one channel between each stage):
/// - audio thread: owns the cpal stream, forwards mono chunks
/// - detection thread: streaming detector + double-clap matcher
/// - event thread: runs the `on_event` callback so slow consumers
///   (action execution) never block detection
pub struct Engine {
    detection_enabled: Arc<AtomicBool>,
    sensitivity: Arc<AtomicU8>,
    stop: Arc<AtomicBool>,
}

impl Engine {
    pub fn is_detection_enabled(&self) -> bool {
        self.detection_enabled.load(Ordering::Relaxed)
    }

    pub fn set_detection_enabled(&self, enabled: bool) {
        self.detection_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Applied live: the detection thread rebuilds its detector and
    /// matcher on the next audio chunk.
    pub fn set_sensitivity(&self, sensitivity: Sensitivity) {
        self.sensitivity
            .store(sensitivity.as_u8(), Ordering::Relaxed);
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn start<F>(sensitivity: Sensitivity, on_event: F) -> Engine
where
    F: Fn(EngineEvent) + Send + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let detection_enabled = Arc::new(AtomicBool::new(true));
    let sensitivity = Arc::new(AtomicU8::new(sensitivity.as_u8()));

    // Bounded so a stalled detection thread sheds load instead of growing.
    let (audio_tx, audio_rx) = mpsc::sync_channel::<AudioChunk>(64);
    let (event_tx, event_rx) = mpsc::channel::<EngineEvent>();

    let capture_stop = stop.clone();
    let capture_event_tx = event_tx.clone();
    thread::spawn(move || {
        if let Err(err) = audio::run_capture(audio_tx, capture_stop) {
            let _ = capture_event_tx.send(EngineEvent::CaptureFailed(err.to_string()));
        }
    });

    let detection_flag = detection_enabled.clone();
    let sensitivity_flag = sensitivity.clone();
    thread::spawn(move || {
        run_detection(&audio_rx, &detection_flag, &sensitivity_flag, &event_tx);
    });

    thread::spawn(move || {
        for event in event_rx {
            on_event(event);
        }
    });

    Engine {
        detection_enabled,
        sensitivity,
        stop,
    }
}

fn run_detection(
    audio_rx: &mpsc::Receiver<AudioChunk>,
    detection_enabled: &AtomicBool,
    sensitivity: &AtomicU8,
    event_tx: &mpsc::Sender<EngineEvent>,
) {
    let mut level = Sensitivity::from_u8(sensitivity.load(Ordering::Relaxed));
    let mut detector: Option<StreamingDetector> = None;
    let mut matcher = StreamingMatcher::new(MatcherConfig::for_sensitivity(level));
    let mut quiet_gate: Option<QuietGate> = None;
    let mut was_enabled = true;

    // Ends when the audio thread drops its sender.
    for chunk in audio_rx {
        if !detection_enabled.load(Ordering::Relaxed) {
            was_enabled = false;
            continue;
        }
        if !was_enabled {
            // A clap from before the pause must not pair with one after it,
            // and a trigger held from before the pause is stale.
            matcher.reset();
            quiet_gate = None;
            was_enabled = true;
        }

        let next_level = Sensitivity::from_u8(sensitivity.load(Ordering::Relaxed));
        if next_level != level {
            // Rebuild both stages with the new tuning. The adaptive floor
            // restarts from its initial estimate and re-converges within
            // a second of ambient audio.
            level = next_level;
            detector = None;
            matcher = StreamingMatcher::new(MatcherConfig::for_sensitivity(level));
            quiet_gate = None;
        }

        let detector = detector.get_or_insert_with(|| {
            StreamingDetector::new(chunk.sample_rate, DetectorConfig::for_sensitivity(level))
        });

        if let Some(gate) = quiet_gate.as_mut() {
            let above = rms_db(&chunk.samples) - detector.floor_db();
            let chunk_ms = (chunk.samples.len() as u64 * 1000) / chunk.sample_rate.max(1) as u64;
            match gate.feed(above, chunk_ms) {
                GateVerdict::Pending => {}
                GateVerdict::Fire(trigger) => {
                    quiet_gate = None;
                    if event_tx.send(EngineEvent::Trigger(trigger)).is_err() {
                        return;
                    }
                }
                GateVerdict::Discard => {
                    println!("[trigger] discarded — sustained noise after the pair");
                    quiet_gate = None;
                }
            }
        }

        for clap in detector.push(&chunk.samples) {
            println!(
                "[clap] t={}ms peak={:.1}dB above_floor={:.1}dB flatness={:.2} confidence={:.2}",
                clap.timestamp_ms,
                clap.peak_db,
                clap.above_floor_db,
                clap.flatness,
                clap.confidence
            );
            if let Some(trigger) = matcher.push(clap) {
                // Held until the room stays quiet for the gate window.
                quiet_gate = Some(QuietGate::new(trigger));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trigger() -> TriggerEvent {
        TriggerEvent {
            first_at_ms: 0,
            second_at_ms: 300,
            interval_ms: 300,
            confidence: 0.8,
        }
    }

    #[test]
    fn quiet_window_fires_the_trigger() {
        let mut gate = QuietGate::new(trigger());
        for _ in 0..29 {
            assert!(matches!(gate.feed(2.0, 10), GateVerdict::Pending));
        }
        assert!(matches!(gate.feed(2.0, 10), GateVerdict::Fire(_)));
    }

    #[test]
    fn sustained_noise_discards_the_trigger() {
        let mut gate = QuietGate::new(trigger());
        let mut verdicts = Vec::new();
        for _ in 0..10 {
            verdicts.push(gate.feed(25.0, 10));
        }
        assert!(verdicts.iter().any(|v| matches!(v, GateVerdict::Discard)));
    }

    #[test]
    fn short_reverb_tail_is_tolerated() {
        let mut gate = QuietGate::new(trigger());
        // 80 ms of reverb, then silence: within the loud allowance.
        for _ in 0..8 {
            assert!(matches!(gate.feed(20.0, 10), GateVerdict::Pending));
        }
        for _ in 0..21 {
            assert!(matches!(gate.feed(1.0, 10), GateVerdict::Pending));
        }
        assert!(matches!(gate.feed(1.0, 10), GateVerdict::Fire(_)));
    }
}
