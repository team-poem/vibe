use anyhow::{Context, Result};
use vibe_poc_double_clap::event::ClapEvent;
use vibe_poc_double_clap::matcher::{
    MatcherConfig, PairOutcome, RejectReason, TriggerEvent, analyze,
};

fn main() -> Result<()> {
    let config = MatcherConfig::default();
    print_config(&config);

    match std::env::args().nth(1) {
        Some(path) => run_file(&path, &config),
        None => {
            println!("[mode] no input path given, running built-in demo scenarios");
            run_demo(&config);
            Ok(())
        }
    }
}

fn run_file(path: &str, config: &MatcherConfig) -> Result<()> {
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read event json at {path}"))?;
    let events: Vec<ClapEvent> = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse event json at {path}"))?;
    println!("[mode] file  path={path}  events={}", events.len());
    report("input", &events, config);
    Ok(())
}

fn run_demo(config: &MatcherConfig) {
    let scenarios: [(&str, Vec<ClapEvent>); 5] = [
        (
            "happy: two similar claps 300ms apart",
            vec![clap(100, -10.0, 0.35, 0.85), clap(400, -11.0, 0.36, 0.82)],
        ),
        (
            "interval too short (120ms)",
            vec![clap(100, -10.0, 0.35, 0.85), clap(220, -10.5, 0.36, 0.83)],
        ),
        (
            "interval too long (800ms)",
            vec![clap(100, -10.0, 0.35, 0.85), clap(900, -10.5, 0.36, 0.83)],
        ),
        (
            "peak mismatch (one loud, one quiet)",
            vec![clap(100, -8.0, 0.35, 0.90), clap(400, -28.0, 0.36, 0.55)],
        ),
        (
            "three claps: first pair triggers, third stays unmatched",
            vec![
                clap(100, -10.0, 0.35, 0.85),
                clap(400, -11.0, 0.36, 0.82),
                clap(700, -10.5, 0.35, 0.84),
            ],
        ),
    ];

    for (label, events) in scenarios {
        println!();
        report(label, &events, config);
    }
}

fn report(label: &str, events: &[ClapEvent], config: &MatcherConfig) {
    println!("[scenario] {label}");
    print_events(events);
    let outcomes = analyze(events, config);
    print_outcomes(&outcomes);
}

fn clap(timestamp_ms: u64, peak_db: f32, flatness: f32, confidence: f32) -> ClapEvent {
    ClapEvent {
        timestamp_ms,
        peak_db,
        above_floor_db: peak_db + 60.0,
        flatness,
        confidence,
    }
}

fn print_config(config: &MatcherConfig) {
    println!(
        "[config] interval={}..{}ms  max_peak_diff={}dB  max_flatness_diff={:.2}",
        config.min_interval_ms,
        config.max_interval_ms,
        config.max_peak_db_diff,
        config.max_flatness_diff
    );
}

fn print_events(events: &[ClapEvent]) {
    if events.is_empty() {
        println!("  events: (none)");
        return;
    }
    println!(
        "  events {:>2} {:>8} {:>9} {:>9} {:>11}",
        "#", "time_ms", "peak_db", "flatness", "confidence"
    );
    for (i, event) in events.iter().enumerate() {
        println!(
            "         {:>2} {:>8} {:>9.1} {:>9.3} {:>11.2}",
            i + 1,
            event.timestamp_ms,
            event.peak_db,
            event.flatness,
            event.confidence
        );
    }
}

fn print_outcomes(outcomes: &[PairOutcome]) {
    if outcomes.is_empty() {
        println!("  result: no pair to evaluate");
        return;
    }
    for outcome in outcomes {
        match outcome {
            PairOutcome::Trigger(trigger) => print_trigger(trigger),
            PairOutcome::Rejected {
                first_idx,
                second_idx,
                interval_ms,
                reason,
            } => {
                println!(
                    "  reject  pair=({},{})  interval={}ms  reason={}",
                    first_idx + 1,
                    second_idx + 1,
                    interval_ms,
                    reject_label(*reason)
                );
            }
        }
    }
}

fn print_trigger(trigger: &TriggerEvent) {
    println!(
        "  TRIGGER first={}ms  second={}ms  interval={}ms  confidence={:.2}",
        trigger.first_at_ms, trigger.second_at_ms, trigger.interval_ms, trigger.confidence
    );
}

fn reject_label(reason: RejectReason) -> &'static str {
    match reason {
        RejectReason::IntervalTooShort => "interval too short",
        RejectReason::IntervalTooLong => "interval too long",
        RejectReason::PeakMismatch => "peak mismatch",
        RejectReason::FlatnessMismatch => "flatness mismatch",
    }
}
