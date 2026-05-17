use crate::action::Action;
use std::cmp::Ordering;
use std::process::{Command, ExitStatus};
use std::time::Instant;

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("failed to spawn {kind}: {source}")]
    Spawn {
        kind: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("failed waiting for {kind}: {source}")]
    Wait {
        kind: &'static str,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug)]
pub struct ActionResult {
    pub action: Action,
    pub spawn_ms: f64,
    pub dispatch_ms: f64,
    pub exit_status: ExitStatus,
}

#[derive(Debug, Clone, Copy)]
pub struct Distribution {
    pub min: f64,
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

#[derive(Debug)]
pub struct Stats {
    pub action: Action,
    pub spawn: Distribution,
    pub dispatch: Distribution,
    pub success_count: usize,
    pub total: usize,
}

pub fn run(action: &Action) -> Result<ActionResult, RunError> {
    let started = Instant::now();
    let mut child = Command::new(action.program())
        .args(action.args())
        .spawn()
        .map_err(|source| RunError::Spawn {
            kind: action.kind_label(),
            source,
        })?;
    let spawn_ms = elapsed_ms(started);

    let exit_status = child.wait().map_err(|source| RunError::Wait {
        kind: action.kind_label(),
        source,
    })?;
    let dispatch_ms = elapsed_ms(started);

    Ok(ActionResult {
        action: action.clone(),
        spawn_ms,
        dispatch_ms,
        exit_status,
    })
}

pub fn measure(action: &Action, repetitions: usize, warmup: usize) -> Result<Stats, RunError> {
    for _ in 0..warmup {
        let _ = run(action)?;
    }

    let mut spawn_samples = Vec::with_capacity(repetitions);
    let mut dispatch_samples = Vec::with_capacity(repetitions);
    let mut success_count = 0;
    for _ in 0..repetitions {
        let result = run(action)?;
        spawn_samples.push(result.spawn_ms);
        dispatch_samples.push(result.dispatch_ms);
        if result.exit_status.success() {
            success_count += 1;
        }
    }

    Ok(Stats {
        action: action.clone(),
        spawn: distribution(&mut spawn_samples),
        dispatch: distribution(&mut dispatch_samples),
        success_count,
        total: repetitions,
    })
}

fn distribution(values: &mut [f64]) -> Distribution {
    if values.is_empty() {
        return Distribution {
            min: f64::NAN,
            p50: f64::NAN,
            p95: f64::NAN,
            max: f64::NAN,
        };
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    Distribution {
        min: values[0],
        p50: quantile(values, 0.50),
        p95: quantile(values, 0.95),
        max: values[values.len() - 1],
    }
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    let n = sorted.len();
    let idx = ((q * (n as f64 - 1.0)).round() as usize).min(n - 1);
    sorted[idx]
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_handles_empty_input() {
        let mut values: Vec<f64> = Vec::new();
        let dist = distribution(&mut values);
        assert!(dist.min.is_nan() && dist.p50.is_nan() && dist.p95.is_nan() && dist.max.is_nan());
    }

    #[test]
    fn distribution_single_value() {
        let mut values = vec![42.0];
        let dist = distribution(&mut values);
        assert_eq!(dist.min, 42.0);
        assert_eq!(dist.p50, 42.0);
        assert_eq!(dist.p95, 42.0);
        assert_eq!(dist.max, 42.0);
    }

    #[test]
    fn distribution_sorted_quartiles() {
        let mut values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let dist = distribution(&mut values);
        assert_eq!(dist.min, 10.0);
        assert_eq!(dist.max, 50.0);
        assert_eq!(dist.p50, 30.0);
        assert_eq!(dist.p95, 50.0);
    }

    #[test]
    fn distribution_unsorted_input_is_sorted() {
        let mut values = vec![50.0, 10.0, 30.0, 20.0, 40.0];
        let dist = distribution(&mut values);
        assert_eq!(dist.min, 10.0);
        assert_eq!(dist.max, 50.0);
        assert_eq!(dist.p50, 30.0);
    }
}
