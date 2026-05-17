use crate::event::ClapEvent;

#[derive(Debug, Clone, Copy)]
pub struct MatcherConfig {
    pub min_interval_ms: u64,
    pub max_interval_ms: u64,
    pub max_peak_db_diff: f32,
    pub max_flatness_diff: f32,
}

impl Default for MatcherConfig {
    fn default() -> Self {
        Self {
            min_interval_ms: 150,
            max_interval_ms: 600,
            max_peak_db_diff: 12.0,
            max_flatness_diff: 0.25,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriggerEvent {
    pub first_at_ms: u64,
    pub second_at_ms: u64,
    pub interval_ms: u64,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RejectReason {
    IntervalTooShort,
    IntervalTooLong,
    PeakMismatch,
    FlatnessMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PairOutcome {
    Trigger(TriggerEvent),
    Rejected {
        first_idx: usize,
        second_idx: usize,
        interval_ms: u64,
        reason: RejectReason,
    },
}

pub fn match_pattern(events: &[ClapEvent], config: &MatcherConfig) -> Vec<TriggerEvent> {
    analyze(events, config)
        .into_iter()
        .filter_map(|outcome| match outcome {
            PairOutcome::Trigger(trigger) => Some(trigger),
            PairOutcome::Rejected { .. } => None,
        })
        .collect()
}

pub fn analyze(events: &[ClapEvent], config: &MatcherConfig) -> Vec<PairOutcome> {
    let mut outcomes = Vec::new();
    let mut i = 0;
    while i + 1 < events.len() {
        let first = &events[i];
        let second = &events[i + 1];
        let interval_ms = second.timestamp_ms.saturating_sub(first.timestamp_ms);

        let outcome = evaluate_pair(first, second, interval_ms, config).map_or_else(
            |reason| PairOutcome::Rejected {
                first_idx: i,
                second_idx: i + 1,
                interval_ms,
                reason,
            },
            PairOutcome::Trigger,
        );

        let consumed_pair = matches!(outcome, PairOutcome::Trigger(_));
        outcomes.push(outcome);
        i += if consumed_pair { 2 } else { 1 };
    }
    outcomes
}

fn evaluate_pair(
    first: &ClapEvent,
    second: &ClapEvent,
    interval_ms: u64,
    config: &MatcherConfig,
) -> Result<TriggerEvent, RejectReason> {
    if interval_ms < config.min_interval_ms {
        return Err(RejectReason::IntervalTooShort);
    }
    if interval_ms > config.max_interval_ms {
        return Err(RejectReason::IntervalTooLong);
    }
    if (first.peak_db - second.peak_db).abs() > config.max_peak_db_diff {
        return Err(RejectReason::PeakMismatch);
    }
    if (first.flatness - second.flatness).abs() > config.max_flatness_diff {
        return Err(RejectReason::FlatnessMismatch);
    }

    let confidence = compute_confidence(first, second, interval_ms, config);
    Ok(TriggerEvent {
        first_at_ms: first.timestamp_ms,
        second_at_ms: second.timestamp_ms,
        interval_ms,
        confidence,
    })
}

fn compute_confidence(
    first: &ClapEvent,
    second: &ClapEvent,
    interval_ms: u64,
    config: &MatcherConfig,
) -> f32 {
    let base = (first.confidence + second.confidence) * 0.5;
    let peak_score =
        1.0 - (first.peak_db - second.peak_db).abs() / config.max_peak_db_diff.max(1e-3);
    let flat_score =
        1.0 - (first.flatness - second.flatness).abs() / config.max_flatness_diff.max(1e-3);
    let interval_center = (config.min_interval_ms + config.max_interval_ms) as f32 * 0.5;
    let interval_half = (config.max_interval_ms - config.min_interval_ms) as f32 * 0.5;
    let interval_score = if interval_half > 0.0 {
        1.0 - (interval_ms as f32 - interval_center).abs() / interval_half
    } else {
        0.0
    };

    (base * 0.4 + peak_score * 0.2 + flat_score * 0.2 + interval_score * 0.2).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clap(timestamp_ms: u64, peak_db: f32, flatness: f32) -> ClapEvent {
        ClapEvent {
            timestamp_ms,
            peak_db,
            above_floor_db: peak_db + 60.0,
            flatness,
            confidence: 0.8,
        }
    }

    #[test]
    fn empty_input_returns_no_triggers() {
        let events: Vec<ClapEvent> = Vec::new();
        assert!(match_pattern(&events, &MatcherConfig::default()).is_empty());
    }

    #[test]
    fn single_clap_returns_no_triggers() {
        let events = vec![clap(100, -10.0, 0.35)];
        assert!(match_pattern(&events, &MatcherConfig::default()).is_empty());
    }

    #[test]
    fn well_spaced_similar_claps_trigger_once() {
        let events = vec![clap(100, -10.0, 0.35), clap(400, -11.5, 0.36)];
        let triggers = match_pattern(&events, &MatcherConfig::default());
        assert_eq!(triggers.len(), 1);
        let trigger = triggers[0];
        assert_eq!(trigger.first_at_ms, 100);
        assert_eq!(trigger.second_at_ms, 400);
        assert_eq!(trigger.interval_ms, 300);
        assert!(
            trigger.confidence > 0.7,
            "confidence={}",
            trigger.confidence
        );
    }

    #[test]
    fn interval_too_short_is_rejected() {
        let events = vec![clap(100, -10.0, 0.35), clap(220, -10.5, 0.36)];
        let outcomes = analyze(&events, &MatcherConfig::default());
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(
            outcomes[0],
            PairOutcome::Rejected {
                reason: RejectReason::IntervalTooShort,
                ..
            }
        ));
    }

    #[test]
    fn interval_too_long_is_rejected() {
        let events = vec![clap(100, -10.0, 0.35), clap(900, -10.5, 0.36)];
        let outcomes = analyze(&events, &MatcherConfig::default());
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(
            outcomes[0],
            PairOutcome::Rejected {
                reason: RejectReason::IntervalTooLong,
                ..
            }
        ));
    }

    #[test]
    fn peak_mismatch_is_rejected() {
        let events = vec![clap(100, -8.0, 0.35), clap(380, -25.0, 0.36)];
        let outcomes = analyze(&events, &MatcherConfig::default());
        assert!(matches!(
            outcomes[0],
            PairOutcome::Rejected {
                reason: RejectReason::PeakMismatch,
                ..
            }
        ));
    }

    #[test]
    fn flatness_mismatch_is_rejected() {
        let events = vec![clap(100, -10.0, 0.35), clap(380, -11.0, 0.05)];
        let outcomes = analyze(&events, &MatcherConfig::default());
        assert!(matches!(
            outcomes[0],
            PairOutcome::Rejected {
                reason: RejectReason::FlatnessMismatch,
                ..
            }
        ));
    }

    #[test]
    fn three_consecutive_claps_consume_first_pair() {
        let events = vec![
            clap(100, -10.0, 0.35),
            clap(400, -11.0, 0.36),
            clap(700, -10.5, 0.35),
        ];
        let triggers = match_pattern(&events, &MatcherConfig::default());
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].first_at_ms, 100);
        assert_eq!(triggers[0].second_at_ms, 400);
    }

    #[test]
    fn four_consecutive_claps_produce_two_triggers() {
        let events = vec![
            clap(100, -10.0, 0.35),
            clap(400, -11.0, 0.36),
            clap(900, -10.5, 0.34),
            clap(1200, -10.8, 0.35),
        ];
        let triggers = match_pattern(&events, &MatcherConfig::default());
        assert_eq!(triggers.len(), 2);
        assert_eq!(triggers[0].first_at_ms, 100);
        assert_eq!(triggers[1].first_at_ms, 900);
    }

    #[test]
    fn rejected_pair_lets_second_clap_pair_with_next() {
        let events = vec![
            clap(100, -10.0, 0.35),
            clap(150, -10.5, 0.35),
            clap(450, -11.0, 0.36),
        ];
        let triggers = match_pattern(&events, &MatcherConfig::default());
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].first_at_ms, 150);
        assert_eq!(triggers[0].second_at_ms, 450);
    }

    #[test]
    fn boundary_interval_at_minimum_passes() {
        let events = vec![clap(100, -10.0, 0.35), clap(250, -10.0, 0.35)];
        let triggers = match_pattern(&events, &MatcherConfig::default());
        assert_eq!(triggers.len(), 1);
    }

    #[test]
    fn boundary_interval_at_maximum_passes() {
        let events = vec![clap(100, -10.0, 0.35), clap(700, -10.0, 0.35)];
        let triggers = match_pattern(&events, &MatcherConfig::default());
        assert_eq!(triggers.len(), 1);
    }

    #[test]
    fn boundary_peak_diff_at_limit_passes() {
        let config = MatcherConfig::default();
        let events = vec![
            clap(100, -10.0, 0.35),
            clap(400, -10.0 - config.max_peak_db_diff, 0.35),
        ];
        let triggers = match_pattern(&events, &config);
        assert_eq!(triggers.len(), 1);
    }
}
