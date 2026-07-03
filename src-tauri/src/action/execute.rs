//! Routine execution: run actions in order, then snap windows to their
//! regions. Runs on the engine's event worker thread and returns one
//! structured outcome per action for the execution log.

use crate::action::{self, Action};
use crate::layout;
use crate::routine::ActionOutcome;

/// Execute a routine in two phases and report per-action outcomes in the
/// original action order. Phase 1 launches every action that can open
/// immediately, so slow window waits never delay later launches. Phase 2
/// places windows into their regions. URL actions with a region are
/// deferred entirely to phase 2 because they must open their own browser
/// window (a plain `open <url>` would land in an existing tab).
pub fn run_routine(actions: &[Action]) -> Vec<ActionOutcome> {
    let mut outcomes: Vec<ActionOutcome> = actions
        .iter()
        .map(|action| ActionOutcome {
            label: action.to_string(),
            success: false,
            detail: String::new(),
        })
        .collect();

    let mut deferred_urls: Vec<usize> = Vec::new();
    let mut app_placements: Vec<usize> = Vec::new();

    for (index, action) in actions.iter().enumerate() {
        let is_placed_url = matches!(
            action,
            Action::OpenUrl {
                region: Some(_),
                ..
            }
        );
        if is_placed_url {
            deferred_urls.push(index);
            continue;
        }
        outcomes[index] = run_outcome(action);
        if action.region().is_some() {
            app_placements.push(index);
        }
    }

    let wants_layout = !deferred_urls.is_empty() || !app_placements.is_empty();
    if wants_layout && !layout::is_trusted(false) {
        eprintln!("[layout] accessibility permission missing; skipping window placement");
        for index in deferred_urls {
            outcomes[index] = run_outcome(&actions[index]);
            append_detail(&mut outcomes[index], "placement skipped (no permission)");
        }
        for index in app_placements {
            append_detail(&mut outcomes[index], "placement skipped (no permission)");
        }
        return outcomes;
    }

    for index in deferred_urls {
        let action = &actions[index];
        let (Action::OpenUrl { url, .. }, Some(region)) = (action, action.region()) else {
            continue;
        };
        outcomes[index] = match layout::open_url_in_placed_window(url, region) {
            Ok(placement) => {
                println!("[layout] {action} → {region:?} ({placement:?})");
                ActionOutcome {
                    label: action.to_string(),
                    success: true,
                    detail: format!("new window → {region:?} ({placement:?})"),
                }
            }
            Err(err) => {
                eprintln!("[layout] {action} → {region:?} failed: {err}");
                ActionOutcome {
                    label: action.to_string(),
                    success: false,
                    detail: err.to_string(),
                }
            }
        };
    }

    for index in app_placements {
        let action = &actions[index];
        let (Action::OpenApp { name, .. }, Some(region)) = (action, action.region()) else {
            continue;
        };
        match layout::place_app_window(name, region) {
            Ok(placement) => {
                println!("[layout] {action} → {region:?} ({placement:?})");
                append_detail(
                    &mut outcomes[index],
                    &format!("→ {region:?} ({placement:?})"),
                );
            }
            Err(err) => {
                eprintln!("[layout] {action} → {region:?} failed: {err}");
                // The app itself opened; a failed snap is noted but does not
                // fail the action.
                append_detail(&mut outcomes[index], &format!("placement failed: {err}"));
            }
        }
    }

    outcomes
}

fn run_outcome(action: &Action) -> ActionOutcome {
    match action::run(action) {
        Ok(result) if result.exit_status.success() => {
            println!(
                "[routine] {} done in {:.0} ms",
                result.action, result.dispatch_ms
            );
            ActionOutcome {
                label: action.to_string(),
                success: true,
                detail: format!("done in {:.0} ms", result.dispatch_ms),
            }
        }
        Ok(result) => {
            eprintln!(
                "[routine] {} exited with {}",
                result.action, result.exit_status
            );
            ActionOutcome {
                label: action.to_string(),
                success: false,
                detail: format!("exited with {}", result.exit_status),
            }
        }
        Err(err) => {
            eprintln!("[routine] {err}");
            ActionOutcome {
                label: action.to_string(),
                success: false,
                detail: err.to_string(),
            }
        }
    }
}

fn append_detail(outcome: &mut ActionOutcome, extra: &str) {
    if outcome.detail.is_empty() {
        outcome.detail = extra.to_owned();
    } else {
        outcome.detail = format!("{}; {extra}", outcome.detail);
    }
}
