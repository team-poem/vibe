use anyhow::{Context, Result};
use vibe_poc_clap_detector::detector::{ClapEvent, detect};
use vibe_poc_clap_detector::wav;

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: vibe-poc-clap-detector <wav-path>")?;

    let data = wav::load_mono(&path).with_context(|| format!("failed to load wav: {path}"))?;
    let duration_s = data.samples.len() as f32 / data.sample_rate as f32;

    println!(
        "[input] {path}  | {} Hz  | {} samples  | {:.2}s",
        data.sample_rate,
        data.samples.len(),
        duration_s
    );

    let events = detect(&data.samples, data.sample_rate);
    print_events(&events);
    print_summary(&events, duration_s);

    Ok(())
}

fn print_events(events: &[ClapEvent]) {
    if events.is_empty() {
        println!("[events] none");
        return;
    }
    println!(
        "[events] {:>4} {:>9} {:>10} {:>11} {:>9} {:>10}",
        "#", "time_ms", "peak_db", "above_floor", "flatness", "confidence"
    );
    for (i, e) in events.iter().enumerate() {
        println!(
            "         {:>4} {:>9} {:>10.1} {:>11.1} {:>9.3} {:>10.2}",
            i + 1,
            e.timestamp_ms,
            e.peak_db,
            e.above_floor_db,
            e.flatness,
            e.confidence
        );
    }
}

fn print_summary(events: &[ClapEvent], duration_s: f32) {
    let count = events.len();
    let per_minute = if duration_s > 0.0 {
        count as f32 * 60.0 / duration_s
    } else {
        0.0
    };
    println!("[summary] events {count}  duration {duration_s:.2}s  rate {per_minute:.1}/min");
}
