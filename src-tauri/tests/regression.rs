//! Regression test against a real clap recording carried over from the
//! clap-detector PoC. Feeds the wav through the streaming detector in
//! microphone-callback-sized chunks and checks the detected claps and the
//! resulting double-clap trigger behavior.

use vibe_lib::engine::detector::{DetectorConfig, StreamingDetector};
use vibe_lib::engine::event::ClapEvent;
use vibe_lib::engine::matcher::{MatcherConfig, StreamingMatcher};

fn load_mono_wav(path: &str) -> (Vec<f32>, u32) {
    let mut reader = hound::WavReader::open(path).expect("failed to open test wav");
    let spec = reader.spec();
    let channels = spec.channels as usize;

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|s| s.expect("bad sample"))
            .collect(),
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.expect("bad sample") as f32 / max)
                .collect()
        }
    };

    let mono: Vec<f32> = interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    (mono, spec.sample_rate)
}

fn detect_in_chunks(samples: &[f32], sample_rate: u32, chunk_size: usize) -> Vec<ClapEvent> {
    let mut detector = StreamingDetector::new(sample_rate, DetectorConfig::default());
    samples
        .chunks(chunk_size)
        .flat_map(|chunk| detector.push(chunk))
        .collect()
}

#[test]
fn detects_real_claps_from_wav() {
    let (samples, sample_rate) = load_mono_wav("tests/data/claps_short.wav");
    let events = detect_in_chunks(&samples, sample_rate, 512);

    assert!(
        (1..=3).contains(&events.len()),
        "expected 1..=3 claps, got {}: {events:?}",
        events.len()
    );
    for event in &events {
        assert!(event.above_floor_db >= 32.0, "weak onset: {event:?}");
        assert!(event.flatness >= 0.20, "not broadband: {event:?}");
        assert!(event.confidence > 0.0, "zero confidence: {event:?}");
    }
}

#[test]
fn chunk_size_does_not_change_detections() {
    let (samples, sample_rate) = load_mono_wav("tests/data/claps_short.wav");

    let a = detect_in_chunks(&samples, sample_rate, 512);
    let b = detect_in_chunks(&samples, sample_rate, 479);
    let c = detect_in_chunks(&samples, sample_rate, 4096);

    let stamps = |events: &[ClapEvent]| events.iter().map(|e| e.timestamp_ms).collect::<Vec<_>>();
    assert_eq!(stamps(&a), stamps(&b));
    assert_eq!(stamps(&a), stamps(&c));
}

#[test]
fn wav_claps_drive_the_matcher_without_false_triggers() {
    let (samples, sample_rate) = load_mono_wav("tests/data/claps_short.wav");
    let events = detect_in_chunks(&samples, sample_rate, 512);

    let mut matcher = StreamingMatcher::new(MatcherConfig::default());
    let triggers: Vec<_> = events.iter().filter_map(|e| matcher.push(*e)).collect();

    // The recording contains isolated claps, not a deliberate double clap;
    // the matcher must not fire more often than clap pairs allow.
    assert!(
        triggers.len() <= events.len() / 2,
        "too many triggers: {triggers:?}"
    );
    for trigger in &triggers {
        assert!(trigger.interval_ms >= 150 && trigger.interval_ms <= 600);
    }
}
