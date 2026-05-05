use vibe_poc_clap_detector::detector::detect;
use vibe_poc_clap_detector::wav::load_mono;

#[test]
fn detects_claps_in_short_clip() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/claps_short.wav");
    let data = load_mono(path).expect("test wav loads");
    assert_eq!(data.sample_rate, 48_000, "expected 48 kHz test wav");

    let events = detect(&data.samples, data.sample_rate);
    let count = events.len();

    assert!(
        (1..=3).contains(&count),
        "expected 1..=3 claps in a 6s clip, got {count}: {events:?}"
    );

    for event in &events {
        assert!(
            event.peak_db > -35.0,
            "clap peak too quiet to be confident: {event:?}"
        );
        assert!(
            event.above_floor_db >= 30.0,
            "clap not loud enough above floor: {event:?}"
        );
        assert!(
            event.flatness >= 0.15,
            "clap flatness suspiciously low: {event:?}"
        );
    }
}
