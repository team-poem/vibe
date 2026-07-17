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
/// running water) stays loud. A matched trigger is held only until a short
/// stretch of continuous quiet confirms the impulse — typically ~100 ms
/// after the reverb tail — so the latency cost is barely perceptible,
/// while sustained noise can never produce that quiet stretch.
const QUIET_GATE_WINDOW_MS: u64 = 300;
/// Continuous quiet that confirms the trigger immediately.
const QUIET_GATE_CONFIRM_MS: u64 = 100;
/// Loudness margin over the adaptive floor that counts as "still noisy".
/// Clap reverb decays within a few tens of ms; sustained sources sit far
/// above this for the entire window.
const QUIET_GATE_MARGIN_DB: f32 = 12.0;
/// Cumulative loud time tolerated inside the window (reverb allowance).
const QUIET_GATE_LOUD_LIMIT_MS: u64 = 90;

/// Fragmented-quiet borderline: this many interruptions of a running
/// quiet streak inside the window mean an impulse train, not clap reverb.
const QUIET_GATE_MAX_STREAK_RESETS: u32 = 3;

struct QuietGate {
    trigger: TriggerEvent,
    elapsed_ms: u64,
    loud_ms: u64,
    quiet_streak_ms: u64,
    streak_resets: u32,
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
            elapsed_ms: 0,
            loud_ms: 0,
            quiet_streak_ms: 0,
            streak_resets: 0,
        }
    }

    fn feed(&mut self, above_floor_db: f32, chunk_ms: u64) -> GateVerdict {
        let loud = above_floor_db.is_finite() && above_floor_db >= QUIET_GATE_MARGIN_DB;
        if loud {
            self.loud_ms += chunk_ms;
            if self.quiet_streak_ms > 0 {
                self.streak_resets += 1;
            }
            self.quiet_streak_ms = 0;
        } else {
            self.quiet_streak_ms += chunk_ms;
        }
        if self.loud_ms > QUIET_GATE_LOUD_LIMIT_MS {
            return GateVerdict::Discard;
        }
        if self.quiet_streak_ms >= QUIET_GATE_CONFIRM_MS {
            return GateVerdict::Fire(self.trigger);
        }
        self.elapsed_ms += chunk_ms;
        if self.elapsed_ms >= QUIET_GATE_WINDOW_MS {
            // Borderline: repeated interruptions of the quiet streak are
            // scattered impulses (bottle-crush crackles slip past the
            // detector's refractory window as loud chunks only), so the
            // benefit of the doubt flips to Discard.
            if self.streak_resets >= QUIET_GATE_MAX_STREAK_RESETS {
                GateVerdict::Discard
            } else {
                GateVerdict::Fire(self.trigger)
            }
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
    /// Detection diagnostics destined for placement.log — routed through
    /// the event thread so the detection loop never does file I/O.
    Diagnostic(String),
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

/// History horizon for the exactly-two rule bookkeeping.
const CLAP_HISTORY_MS: u64 = 3000;
/// Margin added to the pair interval when judging isolation.
const PAIR_ISOLATION_MARGIN_MS: u64 = 500;

/// More than two impulses inside the pair's lookback window means the
/// matched pair belongs to an impulse train (plastic-bottle crush), not a
/// deliberate double clap.
fn impulse_train(history: &[u64], second_at_ms: u64, window_ms: u64) -> bool {
    let from = second_at_ms.saturating_sub(window_ms);
    history
        .iter()
        .filter(|&&t| t >= from && t <= second_at_ms)
        .count()
        > 2
}

fn run_detection(
    audio_rx: &mpsc::Receiver<AudioChunk>,
    detection_enabled: &AtomicBool,
    sensitivity: &AtomicU8,
    event_tx: &mpsc::Sender<EngineEvent>,
) {
    let mut level = Sensitivity::from_u8(sensitivity.load(Ordering::Relaxed));
    let mut matcher_config = MatcherConfig::for_sensitivity(level);
    let mut detector: Option<StreamingDetector> = None;
    let mut matcher = StreamingMatcher::new(matcher_config);
    let mut quiet_gate: Option<QuietGate> = None;
    let mut clap_history: Vec<u64> = Vec::new();
    let mut last_fire_at_ms: Option<u64> = None;
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
            clap_history.clear();
            was_enabled = true;
        }

        let next_level = Sensitivity::from_u8(sensitivity.load(Ordering::Relaxed));
        if next_level != level {
            // Rebuild both stages with the new tuning. The adaptive floor
            // restarts from its initial estimate and re-converges within
            // a second of ambient audio.
            level = next_level;
            matcher_config = MatcherConfig::for_sensitivity(level);
            detector = None;
            matcher = StreamingMatcher::new(matcher_config);
            quiet_gate = None;
            clap_history.clear();
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
                    last_fire_at_ms = Some(trigger.second_at_ms);
                    if event_tx.send(EngineEvent::Trigger(trigger)).is_err() {
                        return;
                    }
                }
                GateVerdict::Discard => {
                    let _ = event_tx.send(EngineEvent::Diagnostic(
                        "[trigger] discarded — noise after the pair".to_owned(),
                    ));
                    quiet_gate = None;
                    matcher.reset();
                }
            }
        }

        for clap in detector.push(&chunk.samples) {
            clap_history.push(clap.timestamp_ms);
            let horizon = clap.timestamp_ms.saturating_sub(CLAP_HISTORY_MS);
            clap_history.retain(|&t| t >= horizon);
            let _ = event_tx.send(EngineEvent::Diagnostic(format!(
                "[clap] t={}ms peak={:.1}dB above_floor={:.1}dB flatness={:.2}",
                clap.timestamp_ms, clap.peak_db, clap.above_floor_db, clap.flatness
            )));

            // A late impulse right after a fire is evidence for the sparse
            // impulse-train residual — recorded for tuning, not acted on.
            if let Some(fired) = last_fire_at_ms {
                if clap.timestamp_ms.saturating_sub(fired) <= 1000 {
                    let _ = event_tx.send(EngineEvent::Diagnostic(
                        "[clap] late impulse within 1s of a fire".to_owned(),
                    ));
                }
            }

            // A third impulse while a pair is awaiting confirmation means
            // an impulse train, not a double clap.
            if quiet_gate.is_some() {
                quiet_gate = None;
                matcher.reset();
                let _ = event_tx.send(EngineEvent::Diagnostic(
                    "[trigger] discarded — extra impulse during confirmation".to_owned(),
                ));
                continue;
            }

            if let Some(trigger) = matcher.push(clap) {
                // Exactly-two rule: any further impulse in the pair's
                // recent past marks an impulse train (bottle crush etc.).
                let window = matcher_config.max_interval_ms + PAIR_ISOLATION_MARGIN_MS;
                if impulse_train(&clap_history, trigger.second_at_ms, window) {
                    matcher.reset();
                    let _ = event_tx.send(EngineEvent::Diagnostic(
                        "[trigger] discarded — impulse train, not a pair".to_owned(),
                    ));
                } else {
                    // Held until the room stays quiet for the gate window.
                    quiet_gate = Some(QuietGate::new(trigger));
                }
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
    fn continuous_quiet_fires_early() {
        let mut gate = QuietGate::new(trigger());
        // 100 ms of continuous quiet confirms without waiting the window.
        for _ in 0..9 {
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
        // 80 ms of reverb (within allowance), then 100 ms quiet → fire.
        for _ in 0..8 {
            assert!(matches!(gate.feed(20.0, 10), GateVerdict::Pending));
        }
        for _ in 0..9 {
            assert!(matches!(gate.feed(1.0, 10), GateVerdict::Pending));
        }
        assert!(matches!(gate.feed(1.0, 10), GateVerdict::Fire(_)));
    }

    #[test]
    fn fragmented_quiet_borderline_discards() {
        let mut gate = QuietGate::new(trigger());
        // Quiet runs interrupted 3+ times, cumulative loud stays under the
        // limit: the window end flips the borderline to Discard, and the
        // gate must never fire along the way.
        let mut discarded = false;
        let mut fired = false;
        for _ in 0..8 {
            for _ in 0..7 {
                match gate.feed(2.0, 10) {
                    GateVerdict::Discard => discarded = true,
                    GateVerdict::Fire(_) => fired = true,
                    GateVerdict::Pending => {}
                }
            }
            match gate.feed(20.0, 10) {
                GateVerdict::Discard => discarded = true,
                GateVerdict::Fire(_) => fired = true,
                GateVerdict::Pending => {}
            }
        }
        assert!(discarded);
        assert!(!fired);
    }

    #[test]
    fn exactly_two_impulses_are_a_pair() {
        assert!(!impulse_train(&[100, 400], 400, 1300));
    }

    #[test]
    fn extra_impulse_marks_a_train() {
        assert!(impulse_train(&[100, 250, 400], 400, 1300));
        // An impulse older than the window does not count.
        assert!(!impulse_train(&[100, 1600, 1900], 1900, 1300));
    }

    #[test]
    fn intermittent_noise_resets_the_quiet_streak() {
        let mut gate = QuietGate::new(trigger());
        // Quiet runs keep being interrupted before reaching 100 ms; the
        // trigger must not fire early on fragmented quiet.
        for _ in 0..3 {
            for _ in 0..8 {
                assert!(matches!(gate.feed(2.0, 10), GateVerdict::Pending));
            }
            assert!(matches!(gate.feed(20.0, 10), GateVerdict::Pending));
        }
    }
}
