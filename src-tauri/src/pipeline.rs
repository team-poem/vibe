use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use crate::audio::{self, AudioChunk};
use crate::engine::detector::{DetectorConfig, StreamingDetector};
use crate::engine::matcher::{MatcherConfig, StreamingMatcher, TriggerEvent};

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
    stop: Arc<AtomicBool>,
}

impl Engine {
    pub fn is_detection_enabled(&self) -> bool {
        self.detection_enabled.load(Ordering::Relaxed)
    }

    pub fn set_detection_enabled(&self, enabled: bool) {
        self.detection_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn start<F>(on_event: F) -> Engine
where
    F: Fn(EngineEvent) + Send + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let detection_enabled = Arc::new(AtomicBool::new(true));

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
    thread::spawn(move || {
        run_detection(&audio_rx, &detection_flag, &event_tx);
    });

    thread::spawn(move || {
        for event in event_rx {
            on_event(event);
        }
    });

    Engine {
        detection_enabled,
        stop,
    }
}

fn run_detection(
    audio_rx: &mpsc::Receiver<AudioChunk>,
    detection_enabled: &AtomicBool,
    event_tx: &mpsc::Sender<EngineEvent>,
) {
    let mut detector: Option<StreamingDetector> = None;
    let mut matcher = StreamingMatcher::new(MatcherConfig::default());
    let mut was_enabled = true;

    // Ends when the audio thread drops its sender.
    for chunk in audio_rx {
        if !detection_enabled.load(Ordering::Relaxed) {
            was_enabled = false;
            continue;
        }
        if !was_enabled {
            // A clap from before the pause must not pair with one after it.
            matcher.reset();
            was_enabled = true;
        }

        let detector = detector.get_or_insert_with(|| {
            StreamingDetector::new(chunk.sample_rate, DetectorConfig::default())
        });
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
                if event_tx.send(EngineEvent::Trigger(trigger)).is_err() {
                    return;
                }
            }
        }
    }
}
