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
        // A placement bound to a disconnected display is that display's
        // setup — skip the whole action rather than opening it anywhere.
        if action.region().is_some()
            && matches!(
                layout::resolve_display(action.display()),
                layout::DisplayTarget::Missing
            )
        {
            {
                outcomes[index] = ActionOutcome {
                    label: action.to_string(),
                    success: true,
                    detail: "skipped — target display not connected".to_owned(),
                };
                continue;
            }
        }
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
        crate::layout::log_place(
            "[layout] accessibility permission missing; opening without placement",
        );
        // Placed URLs and Chrome-handled files still get their own fresh
        // browser window — a plain `open` would pile them as tabs onto
        // whichever window is frontmost.
        let urls: Vec<&str> = deferred_urls
            .iter()
            .filter_map(|&i| match &actions[i] {
                Action::OpenUrl { url, .. } => Some(url.as_str()),
                _ => None,
            })
            .collect();
        let url_outcome = layout::open_urls_unplaced(&urls);
        for &i in &deferred_urls {
            outcomes[i] = ActionOutcome {
                label: actions[i].to_string(),
                success: url_outcome.is_ok(),
                detail: "opened without placement (no permission)".to_owned(),
            };
        }
        for index in deferred_files {
            let Action::OpenFile { path, .. } = &actions[index] else {
                continue;
            };
            let outcome = layout::open_file_unplaced(path);
            outcomes[index] = ActionOutcome {
                label: actions[index].to_string(),
                success: outcome.is_ok(),
                detail: "opened without placement (no permission)".to_owned(),
            };
        }
        for index in app_placements {
            append_detail(&mut outcomes[index], "placement skipped (no permission)");
        }
        return outcomes;
    }

    // URLs sharing a display+region open together: one new browser window
    // per target, each URL a tab of it. Targets on a disconnected display
    // open plainly instead of being forced onto another screen.
    for ((display_id, region), indices) in group_by_target(actions, &deferred_urls) {
        if matches!(
            layout::resolve_display(display_id.as_deref()),
            layout::DisplayTarget::Missing
        ) {
            {
                for &i in &indices {
                    outcomes[i] = run_outcome(&actions[i]);
                    append_detail(&mut outcomes[i], "target display not connected");
                }
                continue;
            }
        }
        let display = layout::display_frame_for(display_id.as_deref());
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

    for &index in &app_placements {
        let action = &actions[index];
        let (Action::OpenApp { name, .. }, Some(region)) = (action, action.region()) else {
            continue;
        };
        if matches!(
            layout::resolve_display(action.display()),
            layout::DisplayTarget::Missing
        ) {
            {
                append_detail(&mut outcomes[index], "target display not connected");
                continue;
            }
        }
        let display = layout::display_frame_for(action.display());
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

    // Apps that restore their previous window position do so asynchronously
    // after launch, undoing the snap — settle, then correct only windows
    // that actually drifted, so settled ones never twitch.
    if !app_placements.is_empty() {
        std::thread::sleep(Duration::from_millis(700));
        for &index in &app_placements {
            let action = &actions[index];
            let (Action::OpenApp { name, .. }, Some(region)) = (action, action.region()) else {
                continue;
            };
            if matches!(
                layout::resolve_display(action.display()),
                layout::DisplayTarget::Missing
            ) {
                {
                    continue;
                }
            }
            let display = layout::display_frame_for(action.display());
            layout::reassert_app_placement(name, region, display);
        }
    }

    // Documents open with an unknown handler app, so each one is opened
    // and placed sequentially using the frontmost-app heuristic.
    for index in deferred_files {
        let action = &actions[index];
        let (Action::OpenFile { path, .. }, Some(region)) = (action, action.region()) else {
            continue;
        };
        if matches!(
            layout::resolve_display(action.display()),
            layout::DisplayTarget::Missing
        ) {
            {
                outcomes[index] = run_outcome(action);
                append_detail(&mut outcomes[index], "target display not connected");
                continue;
            }
        }
        let display = layout::display_frame_for(action.display());
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

/// True when every app this routine would launch already has a window on
/// screen — the workspace counts as assembled and a new trigger is a
/// deliberate no-op, since re-running would only relaunch, re-place and
/// re-stack windows that are already where the user put them.
///
/// Unplaced URLs are excluded from the verdict: they open as tabs of an
/// existing window, and tab presence cannot be observed from outside.
pub fn routine_already_assembled(actions: &[Action]) -> bool {
    let mut apps: Vec<String> = Vec::new();
    for action in actions {
        // Actions bound to a disconnected display are skipped by
        // run_routine, so they must not count here either.
        if action.region().is_some()
            && matches!(
                layout::resolve_display(action.display()),
                layout::DisplayTarget::Missing
            )
        {
            {
                continue;
            }
        }
        let app_name = match action {
            Action::OpenApp { name, .. } => Some(name.clone()),
            Action::OpenUrl {
                region: Some(_), ..
            } => Some("Google Chrome".to_owned()),
            Action::OpenUrl { region: None, .. } => None,
            Action::OpenFile { path, .. } => layout::file_handler_app(path),
        };
        if let Some(name) = app_name {
            if !apps.contains(&name) {
                apps.push(name);
            }
        }
    }
    // Window presence comes from the CG window list, so this verdict
    // works even without Accessibility access.
    !apps.is_empty() && apps.iter().all(|name| layout::app_window_ready(name))
}

/// Bring windows into list order: the app of action #1 ends frontmost.
/// Re-activating each owning app from the bottom of the list upwards gives
/// that stacking without any window-level z-order API.
///
/// Every activation steals focus and visibly raises windows, so the whole
/// pass runs at most twice: once after a short readiness wait (placements
/// already ran, so this usually exits immediately), and once more only if
/// an app finished launching after the first pass.
fn restack_frontmost_first(actions: &[Action]) {
    let mut entries: Vec<(String, usize)> = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        if action.region().is_some()
            && matches!(
                layout::resolve_display(action.display()),
                layout::DisplayTarget::Missing
            )
        {
            {
                continue;
            }
        }
        let app_name = match action {
            Action::OpenApp { name, .. } => Some(name.clone()),
            Action::OpenUrl {
                region: Some(_), ..
            } => Some("Google Chrome".to_owned()),
            // Placed documents take part through their viewer app, so a
            // late-activating viewer cannot end on top uninvited.
            Action::OpenFile {
                path,
                region: Some(_),
                ..
            } => layout::file_handler_app(path),
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
    // Without AX access there is no window readiness signal and no
    // placement happened either — blind activation passes would only
    // flash focus around, so skip restacking entirely.
    if !layout::is_trusted(false) {
        layout::log_place("[restack] skipped — accessibility not trusted");
        return;
    }
    entries.sort_by_key(|(_, index)| std::cmp::Reverse(*index));

    std::thread::spawn(move || {
        let ready_flags = |entries: &[(String, usize)]| -> Vec<bool> {
            entries
                .iter()
                .map(|(name, _)| layout::app_window_ready(name))
                .collect()
        };

        let deadline = std::time::Instant::now() + RESTACK_READY_WAIT;
        while std::time::Instant::now() < deadline && ready_flags(&entries).contains(&false) {
            std::thread::sleep(RESTACK_POLL);
        }
        activate_in_order(&entries);

        let after_first = ready_flags(&entries);
        if !after_first.contains(&false) {
            return;
        }
        // Stragglers past the wait: fire one corrective pass only if one
        // of them actually shows up — a blanket retry when nothing changed
        // is just another focus flash.
        let deadline = std::time::Instant::now() + RESTACK_STRAGGLER_WAIT;
        let mut became_ready = false;
        while std::time::Instant::now() < deadline {
            std::thread::sleep(RESTACK_POLL);
            let now = ready_flags(&entries);
            if now
                .iter()
                .zip(after_first.iter())
                .any(|(now, before)| *now && !*before)
            {
                became_ready = true;
                if !now.contains(&false) {
                    break;
                }
            }
        }
        if became_ready {
            activate_in_order(&entries);
        } else {
            layout::log_place("[restack] straggler pass skipped — nothing became ready");
        }
    });
}

const RESTACK_READY_WAIT: Duration = Duration::from_secs(4);
const RESTACK_STRAGGLER_WAIT: Duration = Duration::from_secs(6);
const RESTACK_POLL: Duration = Duration::from_millis(200);

fn activate_in_order(entries: &[(String, usize)]) {
    for (app_name, _) in entries {
        let _ = Command::new("open")
            .args(["-a", app_name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        std::thread::sleep(Duration::from_millis(120));
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
type PlacementTarget = (Option<String>, Region);

fn group_by_target(actions: &[Action], indices: &[usize]) -> Vec<(PlacementTarget, Vec<usize>)> {
    let mut groups: Vec<(PlacementTarget, Vec<usize>)> = Vec::new();
    for &index in indices {
        let Some(region) = actions[index].region() else {
            continue;
        };
        let target = (actions[index].display().map(str::to_owned), region);
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

    fn url_action(url: &str, region: Region, display: Option<&str>) -> Action {
        match Action::open_url(url) {
            Action::OpenUrl { url, .. } => Action::OpenUrl {
                url,
                region: Some(region),
                display: display.map(str::to_owned),
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
            url_action("https://b.com", Region::RightHalf, Some("uuid-b")),
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
