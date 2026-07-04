//! Routine execution: run actions in order, then snap windows to their
//! regions. Runs on the engine's event worker thread and returns one
//! structured outcome per action for the execution log.

use std::process::{Command, Stdio};
use std::time::Duration;

use crate::action::{self, Action};
use crate::layout::{self, Region};
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
    let mut deferred_files: Vec<usize> = Vec::new();
    let mut app_placements: Vec<usize> = Vec::new();

    for (index, action) in actions.iter().enumerate() {
        match action {
            Action::OpenUrl {
                region: Some(_), ..
            } => {
                deferred_urls.push(index);
                continue;
            }
            Action::OpenFile {
                region: Some(_), ..
            } => {
                deferred_files.push(index);
                continue;
            }
            _ => {}
        }
        outcomes[index] = run_outcome(action);
        if action.region().is_some() {
            app_placements.push(index);
        }
    }

    let wants_layout =
        !deferred_urls.is_empty() || !deferred_files.is_empty() || !app_placements.is_empty();
    if wants_layout && !layout::is_trusted(false) {
        eprintln!("[layout] accessibility permission missing; skipping window placement");
        for index in deferred_urls.into_iter().chain(deferred_files) {
            outcomes[index] = run_outcome(&actions[index]);
            append_detail(&mut outcomes[index], "placement skipped (no permission)");
        }
        for index in app_placements {
            append_detail(&mut outcomes[index], "placement skipped (no permission)");
        }
        return outcomes;
    }

    // URLs sharing a display+region open together: one new browser window
    // per target, each URL a tab of it.
    for ((display_id, region), indices) in group_by_target(actions, &deferred_urls) {
        let display = layout::display_frame(display_id);
        let urls: Vec<&str> = indices
            .iter()
            .filter_map(|&i| match &actions[i] {
                Action::OpenUrl { url, .. } => Some(url.as_str()),
                _ => None,
            })
            .collect();
        match layout::open_urls_in_placed_window(&urls, region, display) {
            Ok(placement) => {
                println!(
                    "[layout] {} url(s) → {region:?} ({placement:?})",
                    urls.len()
                );
                for &i in &indices {
                    outcomes[i] = ActionOutcome {
                        label: actions[i].to_string(),
                        success: true,
                        detail: format!("tab in new window → {region:?} ({placement:?})"),
                    };
                }
            }
            Err(err) => {
                eprintln!("[layout] urls → {region:?} failed: {err}");
                for &i in &indices {
                    outcomes[i] = ActionOutcome {
                        label: actions[i].to_string(),
                        success: false,
                        detail: err.to_string(),
                    };
                }
            }
        }
    }

    for index in app_placements {
        let action = &actions[index];
        let (Action::OpenApp { name, .. }, Some(region)) = (action, action.region()) else {
            continue;
        };
        let display = layout::display_frame(action.display());
        match layout::place_app_window(name, region, display) {
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

    // Documents open with an unknown handler app, so each one is opened
    // and placed sequentially using the frontmost-app heuristic.
    for index in deferred_files {
        let action = &actions[index];
        let (Action::OpenFile { path, .. }, Some(region)) = (action, action.region()) else {
            continue;
        };
        let display = layout::display_frame(action.display());
        outcomes[index] = match layout::open_file_in_placed_window(path, region, display) {
            Ok(placement) => {
                println!("[layout] {action} → {region:?} ({placement:?})");
                ActionOutcome {
                    label: action.to_string(),
                    success: true,
                    detail: format!("→ {region:?} ({placement:?})"),
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

    restack_frontmost_first(actions);

    outcomes
}

/// Bring windows into list order: the app of action #1 ends frontmost.
/// Re-activating each owning app from the bottom of the list upwards gives
/// that stacking without any window-level z-order API.
fn restack_frontmost_first(actions: &[Action]) {
    let mut entries: Vec<(String, usize)> = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        let app_name = match action {
            Action::OpenApp { name, .. } => Some(name.clone()),
            Action::OpenUrl {
                region: Some(_), ..
            } => Some("Google Chrome".to_owned()),
            _ => None,
        };
        let Some(app_name) = app_name else { continue };
        match entries.iter_mut().find(|(name, _)| *name == app_name) {
            Some(entry) => entry.1 = entry.1.min(index),
            None => entries.push((app_name, index)),
        }
    }
    if entries.len() < 2 {
        return;
    }
    entries.sort_by_key(|(_, index)| std::cmp::Reverse(*index));
    for (app_name, _) in entries {
        let _ = Command::new("open")
            .args(["-a", &app_name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        std::thread::sleep(Duration::from_millis(150));
    }
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

/// Group action indices by their (display, region) target, preserving the
/// order in which targets first appear.
type PlacementTarget = (Option<u32>, Region);

fn group_by_target(actions: &[Action], indices: &[usize]) -> Vec<(PlacementTarget, Vec<usize>)> {
    let mut groups: Vec<(PlacementTarget, Vec<usize>)> = Vec::new();
    for &index in indices {
        let Some(region) = actions[index].region() else {
            continue;
        };
        let target = (actions[index].display(), region);
        match groups.iter_mut().find(|(t, _)| *t == target) {
            Some((_, group)) => group.push(index),
            None => groups.push((target, vec![index])),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url_action(url: &str, region: Region, display: Option<u32>) -> Action {
        match Action::open_url(url) {
            Action::OpenUrl { url, .. } => Action::OpenUrl {
                url,
                region: Some(region),
                display,
            },
            other => other,
        }
    }

    #[test]
    fn urls_sharing_a_target_group_together() {
        let actions = vec![
            url_action("https://a.com", Region::RightHalf, None),
            url_action("https://b.com", Region::LeftHalf, None),
            url_action("https://c.com", Region::RightHalf, None),
        ];
        let groups = group_by_target(&actions, &[0, 1, 2]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], ((None, Region::RightHalf), vec![0, 2]));
        assert_eq!(groups[1], ((None, Region::LeftHalf), vec![1]));
    }

    #[test]
    fn same_region_on_different_displays_stays_separate() {
        let actions = vec![
            url_action("https://a.com", Region::RightHalf, None),
            url_action("https://b.com", Region::RightHalf, Some(2)),
        ];
        let groups = group_by_target(&actions, &[0, 1]);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn actions_without_region_are_skipped() {
        let actions = vec![Action::open_url("https://a.com")];
        assert!(group_by_target(&actions, &[0]).is_empty());
    }
}
