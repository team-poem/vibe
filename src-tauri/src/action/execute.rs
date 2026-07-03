//! Routine execution: run actions in order, then snap windows to their
//! regions. Runs on the engine's event worker thread.

use crate::action::{self, Action};
use crate::layout;

/// Execute a routine in two phases. Phase 1 launches every action that can
/// open immediately, so slow window waits never delay later launches.
/// Phase 2 places windows into their regions. URL actions with a region are
/// deferred entirely to phase 2 because they must open their own browser
/// window (a plain `open <url>` would land in an existing tab).
pub fn run_routine(actions: &[Action]) {
    let mut deferred: Vec<&Action> = Vec::new();
    let mut placements: Vec<&Action> = Vec::new();

    for action in actions {
        let is_placed_url = matches!(
            action,
            Action::OpenUrl {
                region: Some(_),
                ..
            }
        );
        if is_placed_url {
            deferred.push(action);
            continue;
        }
        log_run(action::run(action));
        if action.region().is_some() {
            placements.push(action);
        }
    }

    if deferred.is_empty() && placements.is_empty() {
        return;
    }

    if !layout::is_trusted(false) {
        eprintln!("[layout] accessibility permission missing; skipping window placement");
        // Deferred URLs still have to open, just without placement.
        for action in &deferred {
            log_run(action::run(action));
        }
        return;
    }

    for action in deferred.into_iter().chain(placements) {
        place(action);
    }
}

fn place(action: &Action) {
    let Some(region) = action.region() else {
        return;
    };
    let outcome = match action {
        Action::OpenApp { name, .. } => layout::place_app_window(name, region),
        Action::OpenUrl { url, .. } => layout::open_url_in_placed_window(url, region),
    };
    match outcome {
        Ok(placement) => println!("[layout] {action} → {region:?} ({placement:?})"),
        Err(err) => eprintln!("[layout] {action} → {region:?} failed: {err}"),
    }
}

fn log_run(outcome: Result<action::ActionResult, action::RunError>) {
    match outcome {
        Ok(result) if result.exit_status.success() => println!(
            "[routine] {} done in {:.0} ms",
            result.action, result.dispatch_ms
        ),
        Ok(result) => eprintln!(
            "[routine] {} exited with {}",
            result.action, result.exit_status
        ),
        Err(err) => eprintln!("[routine] {err}"),
    }
}
