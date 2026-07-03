use std::process::{Command, ExitStatus, Stdio};
use std::time::Instant;

use crate::action::Action;

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

/// Execute one action and block until its subprocess exits. Callers run on
/// a worker thread, never on the detection path. Child stdio is discarded
/// so subprocess output cannot leak into the app's logs.
pub fn run(action: &Action) -> Result<ActionResult, RunError> {
    let started = Instant::now();
    let mut child = Command::new(action.program())
        .args(action.args())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
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

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
