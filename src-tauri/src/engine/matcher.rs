use crate::engine::event::ClapEvent;

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

/// Stateful double-clap matcher for a live clap stream. Holds at most one
/// unconsumed clap; a rejected pair advances by one so the newer clap can
/// pair with the next one, mirroring the offline pairing semantics.
pub struct StreamingMatcher {
    config: MatcherConfig,
    pending: Option<ClapEvent>,
}

impl StreamingMatcher {
    pub fn new(config: MatcherConfig) -> Self {
        Self {
            config,
            pending: None,
        }
    }

    /// Drop any half-matched clap, e.g. when detection is paused and resumed.
    pub fn reset(&mut self) {
        self.pending = None;
    }

    pub fn push(&mut self, clap: ClapEvent) -> Option<TriggerEvent> {
        let Some(first) = self.pending.take() else {
            self.pending = Some(clap);
            return None;
        };

        let interval_ms = clap.timestamp_ms.saturating_sub(first.timestamp_ms);
        match evaluate_pair(&first, &clap, interval_ms, &self.config) {
            Ok(trigger) => Some(trigger),
            Err(_) => {
                self.pending = Some(clap);
                None
            }
        }
    }
}

pub fn evaluate_pair(
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

    fn push_all(matcher: &mut StreamingMatcher, claps: &[ClapEvent]) -> Vec<TriggerEvent> {
        claps.iter().filter_map(|c| matcher.push(*c)).collect()
    }

    #[test]
    fn single_clap_does_not_trigger() {
        let mut matcher = StreamingMatcher::new(MatcherConfig::default());
        assert!(matcher.push(clap(100, -10.0, 0.35)).is_none());
    }

    #[test]
    fn similar_pair_in_range_triggers() {
        let mut matcher = StreamingMatcher::new(MatcherConfig::default());
        let triggers = push_all(
            &mut matcher,
            &[clap(100, -10.0, 0.35), clap(400, -11.0, 0.33)],
        );
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].interval_ms, 300);
    }

    #[test]
    fn interval_too_short_is_rejected() {
        let mut matcher = StreamingMatcher::new(MatcherConfig::default());
        let triggers = push_all(
            &mut matcher,
            &[clap(100, -10.0, 0.35), clap(220, -10.0, 0.35)],
        );
        assert!(triggers.is_empty());
    }

    #[test]
    fn interval_too_long_is_rejected() {
        let mut matcher = StreamingMatcher::new(MatcherConfig::default());
        let triggers = push_all(
            &mut matcher,
            &[clap(100, -10.0, 0.35), clap(900, -10.0, 0.35)],
        );
        assert!(triggers.is_empty());
    }

    #[test]
    fn peak_mismatch_is_rejected() {
        let mut matcher = StreamingMatcher::new(MatcherConfig::default());
        let triggers = push_all(
            &mut matcher,
            &[clap(100, -5.0, 0.35), clap(400, -25.0, 0.35)],
        );
        assert!(triggers.is_empty());
    }

    #[test]
    fn flatness_mismatch_is_rejected() {
        let mut matcher = StreamingMatcher::new(MatcherConfig::default());
        let triggers = push_all(
            &mut matcher,
            &[clap(100, -10.0, 0.60), clap(400, -10.0, 0.22)],
        );
        assert!(triggers.is_empty());
    }

    #[test]
    fn rejected_pair_advances_by_one() {
        // Claps at 100/220/520: (100,220) too short, but (220,520) matches.
        let mut matcher = StreamingMatcher::new(MatcherConfig::default());
        let triggers = push_all(
            &mut matcher,
            &[
                clap(100, -10.0, 0.35),
                clap(220, -10.0, 0.35),
                clap(520, -10.0, 0.35),
            ],
        );
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].first_at_ms, 220);
    }

    #[test]
    fn triggered_pair_consumes_both_claps() {
        // After a trigger, the third clap has no partner left.
        let mut matcher = StreamingMatcher::new(MatcherConfig::default());
        let triggers = push_all(
            &mut matcher,
            &[
                clap(100, -10.0, 0.35),
                clap(400, -10.0, 0.35),
                clap(700, -10.0, 0.35),
            ],
        );
        assert_eq!(triggers.len(), 1);
    }

    #[test]
    fn reset_drops_pending_clap() {
        let mut matcher = StreamingMatcher::new(MatcherConfig::default());
        assert!(matcher.push(clap(100, -10.0, 0.35)).is_none());
        matcher.reset();
        assert!(matcher.push(clap(400, -10.0, 0.35)).is_none());
    }

    #[test]
    fn boundary_intervals_are_inclusive() {
        let config = MatcherConfig::default();
        let mut matcher = StreamingMatcher::new(config);
        let triggers = push_all(
            &mut matcher,
            &[
                clap(0, -10.0, 0.35),
                clap(config.min_interval_ms, -10.0, 0.35),
            ],
        );
        assert_eq!(triggers.len(), 1);

        let mut matcher = StreamingMatcher::new(config);
        let triggers = push_all(
            &mut matcher,
            &[
                clap(0, -10.0, 0.35),
                clap(config.max_interval_ms, -10.0, 0.35),
            ],
        );
        assert_eq!(triggers.len(), 1);
    }
}
