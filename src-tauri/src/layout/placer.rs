//! Window placement flows built on the AX wrappers: find the right window
//! for an action, wait for it to exist, and snap it to its region.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use core_graphics::geometry::{CGPoint, CGRect, CGSize};

use crate::layout::ax::{self, AxElement, Placement};
use crate::layout::Region;

const WINDOW_WAIT_TIMEOUT: Duration = Duration::from_secs(8);
/// Chrome cold-starting under a launch storm can take well past 8 s to
/// show its first window — measured live at ~14 s with 7 tabs.
const CHROME_WINDOW_WAIT: Duration = Duration::from_secs(20);
/// Applied only when our spawn cold-starts Chrome (flags are ignored on
/// hand-off to a running instance): media pages opened without a user
/// gesture may start playback. Session-wide until Chrome quits — accepted
/// trade-off so the routine's music actually starts.
const CHROME_AUTOPLAY_FLAG: &str = "--autoplay-policy=no-user-gesture-required";
/// In the dedicated-document route, fall back to "the only fresh window"
/// when no title matches for this long.
const CHROME_TITLE_FALLBACK: Duration = Duration::from_secs(6);
const WINDOW_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("accessibility permission is not granted")]
    NotTrusted,
    #[error("no running process found for app {0:?}")]
    AppNotFound(String),
    #[error("timed out waiting for a window of {0}")]
    WindowTimeout(String),
    #[error("failed to run {command}: {source}")]
    Spawn {
        command: &'static str,
        source: std::io::Error,
    },
    #[error(transparent)]
    Ax(#[from] ax::AxError),
}

pub fn is_trusted(prompt: bool) -> bool {
    ax::is_process_trusted(prompt) && ax::control_probe_ok()
}

/// Print a placement diagnostic and append it to
/// `~/Library/Application Support/com.vibe.app/placement.log`, so failures
/// on end-user machines can be diagnosed regardless of how the app was
/// launched (Finder launches discard stdout).
pub fn log_place(line: &str) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let stamped = format!("[{secs}] {line}");
    println!("{stamped}");
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let path = format!("{home}/Library/Application Support/com.vibe.app/placement.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{stamped}");
    }
}

/// Snap the front window of a (just launched or already running) app to a
/// region. The front window is intentional here: for app actions the user
/// wants "that app's window" in the region, new or not.
pub fn place_app_window(
    app_name: &str,
    region: Region,
    display: CGRect,
) -> Result<Placement, LayoutError> {
    place_app_window_excluding(app_name, region, display, &[]).map(|(placement, _)| placement)
}

/// Placement that also returns the window it used and skips windows in
/// `exclude`, so repeated actions on one app each claim their own window.
pub fn place_app_window_excluding(
    app_name: &str,
    region: Region,
    display: CGRect,
    exclude: &[ax::AxElement],
) -> Result<(Placement, ax::AxElement), LayoutError> {
    if !is_trusted(false) {
        return Err(LayoutError::NotTrusted);
    }
    let pid = wait_for_pid(app_name)?;
    let app = ax::application_element(pid);
    let window = wait_for_main_window_excluding(&app, app_name, exclude)?;
    let placement = apply_placement(&window, region, display)?;
    Ok((placement, window))
}

/// Wait until the app owns at least `min_windows` real windows — the
/// stagger gate between two "open folder" actions on the same app.
pub fn wait_app_window_count(app_name: &str, min_windows: usize, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(pid) = find_pid(app_name) {
            if crate::layout::pid_real_window_count(pid) >= min_windows {
                return true;
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(WINDOW_POLL_INTERVAL);
    }
}

/// Wait until the app has windows, then return the largest one — the first
/// AX window can be a splash screen or utility panel, not the main window.
fn wait_for_main_window(app: &ax::AxElement, label: &str) -> Result<ax::AxElement, LayoutError> {
    wait_for_main_window_excluding(app, label, &[])
}

/// Like [`wait_for_main_window`], but never returns a window already
/// claimed by an earlier action — two "open this folder" actions on the
/// same app must land on two different windows.
fn wait_for_main_window_excluding(
    app: &ax::AxElement,
    label: &str,
    exclude: &[ax::AxElement],
) -> Result<ax::AxElement, LayoutError> {
    let deadline = Instant::now() + WINDOW_WAIT_TIMEOUT;
    loop {
        if let Ok(windows) = ax::windows(app) {
            let fresh: Vec<ax::AxElement> = windows
                .into_iter()
                .filter(|w| !exclude.iter().any(|used| ax::same_element(w, used)))
                .collect();
            if let Some(window) = pick_main_window(fresh) {
                return Ok(window);
            }
        }
        if Instant::now() >= deadline {
            return Err(LayoutError::WindowTimeout(label.to_owned()));
        }
        std::thread::sleep(WINDOW_POLL_INTERVAL);
    }
}

fn pick_main_window(windows: Vec<ax::AxElement>) -> Option<ax::AxElement> {
    windows
        .into_iter()
        .map(|window| {
            let area = ax::window_size(&window)
                .map(|size| size.width * size.height)
                .unwrap_or(0.0);
            (window, area)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(window, _)| window)
}

/// Single non-blocking check: does the app have a real window on any
/// Space? Backed by the CoreGraphics window list, so it needs no
/// Accessibility permission and sees minimized/other-Space windows AX
/// cannot.
pub fn app_window_ready(app_name: &str) -> bool {
    let Some(pid) = find_pid(app_name) else {
        return false;
    };
    crate::layout::pid_has_real_window(pid)
}

/// Batch variant for the assembled guard: one process scan and one
/// window-list enumeration decide every app at once (~15 ms for a whole
/// routine, vs. one scan + one enumeration per app).
pub fn apps_all_have_windows(app_names: &[String]) -> bool {
    if app_names.is_empty() {
        return false;
    }
    let candidates = crate::layout::probe_find_all_pids(app_names);
    let windowed = crate::layout::real_window_pids();
    candidates
        .iter()
        .all(|pids| pids.iter().any(|pid| windowed.contains(&i64::from(*pid))))
}

/// Snap or, for `Centered`, move-only: the window keeps its natural size
/// and is centered on the target display.
fn apply_placement(
    window: &ax::AxElement,
    region: Region,
    display: CGRect,
) -> Result<Placement, LayoutError> {
    if region == Region::Centered {
        let size = ax::window_size(window).unwrap_or(CGSize {
            width: 0.0,
            height: 0.0,
        });
        let origin = CGPoint {
            x: display.origin.x + ((display.size.width - size.width) / 2.0).max(0.0),
            y: display.origin.y + ((display.size.height - size.height) / 2.0).max(0.0),
        };
        ax::set_window_position(window, origin)?;
        return Ok(Placement::MovedOnly);
    }
    Ok(ax::set_window_frame(window, region.frame(display))?)
}

/// Open a URL in a fresh browser window and snap that specific window.
///
/// `open --args --new-window` is ignored when the browser is already
/// running, so the browser binary is invoked directly; the new window is
/// identified by diffing against a pre-open snapshot (both PoC findings).
/// Every URL in `urls` becomes a tab of that one window, so URLs sharing a
/// region never spawn separate windows. Falls back to plain `open <url>`
/// without placement when no supported browser is installed.
pub fn open_urls_in_placed_window(
    urls: &[&str],
    region: Region,
    display: CGRect,
) -> Result<Placement, LayoutError> {
    if urls.is_empty() {
        return Ok(Placement::Full);
    }
    if !is_trusted(false) {
        return Err(LayoutError::NotTrusted);
    }
    let Some(chrome) = find_chrome_binary() else {
        for url in urls {
            open_url_without_placement(url)?;
        }
        return Err(LayoutError::AppNotFound("Google Chrome".to_owned()));
    };

    quit_windowless_chrome();
    let snapshot = match find_pid("Google Chrome") {
        Some(pid) => ax::windows(&ax::application_element(pid)).unwrap_or_default(),
        None => Vec::new(),
    };
    log_place(&format!(
        "[url-window] snapshot={} window(s) before spawn",
        snapshot.len()
    ));

    Command::new(&chrome)
        .arg(CHROME_AUTOPLAY_FLAG)
        .arg("--new-window")
        .args(urls)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| LayoutError::Spawn {
            command: "chrome --new-window",
            source,
        })?;

    let window = match wait_for_fresh_chrome_window(&snapshot) {
        Ok(window) => window,
        Err(err) => {
            log_place(&format!("[url-window] fresh window NOT found: {err}"));
            return Err(err);
        }
    };
    let placement = apply_placement(&window, region, display)?;
    log_place(&format!(
        "[url-window] placed ({placement:?}) frame={:?}",
        ax::window_frame(&window)
    ));
    Ok(placement)
}

/// Open a document in its own Chrome window and place it, identifying the
/// right window by its title (the file name): the URL tab-group window can
/// appear late during a cold start and must never be mistaken for the
/// document window.
fn open_path_in_dedicated_chrome_window(
    path: &str,
    region: Region,
    display: CGRect,
) -> Result<Placement, LayoutError> {
    let Some(chrome) = find_chrome_binary() else {
        return Err(LayoutError::AppNotFound("Google Chrome".to_owned()));
    };
    let snapshot = match find_pid("Google Chrome") {
        Some(pid) => ax::windows(&ax::application_element(pid)).unwrap_or_default(),
        None => Vec::new(),
    };
    Command::new(&chrome)
        .arg(CHROME_AUTOPLAY_FLAG)
        .arg("--new-window")
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| LayoutError::Spawn {
            command: "chrome --new-window",
            source,
        })?;

    let stem = std::path::Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let started = Instant::now();
    let deadline = started + CHROME_WINDOW_WAIT;
    loop {
        let candidates = crate::layout::probe_find_all_pids(&["Google Chrome".to_owned()])
            .into_iter()
            .next()
            .unwrap_or_default();
        let mut fresh: Vec<ax::AxElement> = Vec::new();
        for pid in candidates {
            let app = ax::application_element(pid);
            let Ok(windows) = ax::windows(&app) else {
                continue;
            };
            fresh.extend(
                windows
                    .into_iter()
                    .filter(|w| !snapshot.iter().any(|old| ax::same_element(w, old))),
            );
        }
        let by_title = fresh.iter().position(|w| {
            ax::window_title(w)
                .map(|title| !stem.is_empty() && title.to_lowercase().contains(&stem))
                .unwrap_or(false)
        });
        let chosen = by_title.or_else(|| {
            // No title match yet: only fall back to "the single fresh
            // window" once the title has had a fair chance to appear.
            (started.elapsed() >= CHROME_TITLE_FALLBACK && fresh.len() == 1).then_some(0)
        });
        if let Some(index) = chosen {
            let window = fresh.swap_remove(index);
            log_place(&format!(
                "[place:file] chrome window chosen (title_match={})",
                by_title.is_some()
            ));
            return apply_placement(&window, region, display);
        }
        if Instant::now() >= deadline {
            return Err(LayoutError::WindowTimeout("Google Chrome".to_owned()));
        }
        std::thread::sleep(WINDOW_POLL_INTERVAL);
    }
}

/// A windowless resident Chrome (all windows closed, process alive —
/// macOS keeps it until a real Quit) swallows our spawn as a hand-off and
/// the autoplay flag with it. Nothing user-visible exists to lose, so quit
/// it and let the next spawn cold-start with our flags applied. Chrome
/// with real windows is in use and is never touched.
fn quit_windowless_chrome() {
    let pids = crate::layout::probe_find_all_pids(&["Google Chrome".to_owned()])
        .into_iter()
        .next()
        .unwrap_or_default();
    if pids.is_empty() {
        return;
    }
    let windowed = crate::layout::real_window_pids();
    if pids.iter().any(|pid| windowed.contains(&i64::from(*pid))) {
        return;
    }
    log_place("[url-window] quitting windowless resident chrome for a cold start");
    for pid in &pids {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let alive = crate::layout::probe_find_all_pids(&["Google Chrome".to_owned()])
            .into_iter()
            .next()
            .unwrap_or_default();
        if alive.is_empty() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    log_place("[url-window] windowless chrome did not exit in time");
}

/// Wait for a Chrome window that was not in `snapshot`, re-resolving the
/// pid on every poll. During a cold start the `--new-window` hand-off stub
/// (real `.app` path) and the browser main (code-sign-clone path) appear
/// at different moments — watching a single pid captured once misses the
/// window, which lands on whichever process wins.
fn wait_for_fresh_chrome_window(snapshot: &[AxElement]) -> Result<AxElement, LayoutError> {
    let deadline = Instant::now() + CHROME_WINDOW_WAIT;
    loop {
        let candidates = crate::layout::probe_find_all_pids(&["Google Chrome".to_owned()])
            .into_iter()
            .next()
            .unwrap_or_default();
        for pid in candidates {
            let app = ax::application_element(pid);
            let Ok(windows) = ax::windows(&app) else {
                continue;
            };
            let fresh = windows
                .into_iter()
                .find(|w| !snapshot.iter().any(|old| ax::same_element(w, old)));
            if let Some(window) = fresh {
                return Ok(window);
            }
        }
        if Instant::now() >= deadline {
            return Err(LayoutError::WindowTimeout("Google Chrome".to_owned()));
        }
        std::thread::sleep(WINDOW_POLL_INTERVAL);
    }
}

/// Open a document with its default app and snap that app's window.
///
/// LaunchServices resolves which app will handle the file, so the exact
/// viewer is targeted instead of guessing from focus (osascript frontmost
/// remains only as a fallback). Viewers re-fit their window after the
/// document loads at unpredictable times, so the frame is verified against
/// the window's actual frame and re-applied until it sticks.
pub fn open_file_in_placed_window(
    path: &str,
    region: Region,
    display: CGRect,
) -> Result<Placement, LayoutError> {
    if !is_trusted(false) {
        log_place("[place:file] accessibility not trusted — placement skipped");
        return Err(LayoutError::NotTrusted);
    }
    let handler = default_handler_app(path);

    // Files whose default app is Chrome must go through the same
    // dedicated-window path as URLs: a plain `open` joins them as a tab of
    // whichever Chrome window is frontmost, and placing "the document
    // window" would then drag that whole window — including another
    // display's tab group — onto this file's region.
    if handler.as_deref() == Some("Google Chrome") && find_chrome_binary().is_some() {
        log_place("[place:file] chrome is the handler → dedicated-window route");
        return open_path_in_dedicated_chrome_window(path, region, display);
    }

    let snapshot = handler
        .as_deref()
        .and_then(find_pid)
        .map(|pid| ax::windows(&ax::application_element(pid)).unwrap_or_default())
        .unwrap_or_default();

    Command::new("open")
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source| LayoutError::Spawn {
            command: "open <file>",
            source,
        })?;

    let pid = match &handler {
        Some(app_name) => wait_for_pid(app_name)?,
        None => {
            std::thread::sleep(Duration::from_millis(900));
            frontmost_pid().ok_or_else(|| LayoutError::AppNotFound("frontmost app".to_owned()))?
        }
    };
    let app = ax::application_element(pid);
    let label = handler.as_deref().unwrap_or("document viewer");
    log_place(&format!(
        "[place:file] handler={handler:?} pid={pid} snapshot_windows={}",
        snapshot.len()
    ));
    let placement = place_until_stable(&app, &snapshot, label, region, display)?;
    // Viewers can still re-fit the window to the document after placement
    // looked settled — typical on warm re-open, where macOS restores the
    // previous frame first and the viewer resizes once loading finishes.
    if region != Region::Centered {
        spawn_refit_guardian(app, snapshot, region, display);
    }
    Ok(placement)
}

const REFIT_GUARD_WINDOW: Duration = Duration::from_secs(6);
const REFIT_GUARD_POLL: Duration = Duration::from_millis(400);

/// Watch the placed document window for a few more seconds and re-assert
/// the target frame whenever it drifts. Detached on purpose: placement
/// already succeeded and was reported; this thread only defends it, so
/// nobody joins it and it exits by its own deadline.
fn spawn_refit_guardian(
    app: ax::AxElement,
    snapshot: Vec<ax::AxElement>,
    region: Region,
    display: CGRect,
) {
    std::thread::spawn(move || {
        let deadline = Instant::now() + REFIT_GUARD_WINDOW;
        let target = region.frame(display);
        while Instant::now() < deadline {
            std::thread::sleep(REFIT_GUARD_POLL);
            let Some(window) = current_target_window(&app, &snapshot) else {
                log_place("[place:guard] no window");
                continue;
            };
            let frame = ax::window_frame(&window);
            let drifted = frame
                .map(|frame| !frames_close(frame, target))
                .unwrap_or(false);
            if drifted {
                let result = ax::set_window_frame(&window, target);
                log_place(&format!(
                    "[place:guard] drift frame={frame:?} → re-assert {result:?}"
                ));
            }
        }
        log_place("[place:guard] done");
    });
}

/// Re-apply an app placement only if its window drifted off target —
/// called once after the settle delay, without making settled windows jump.
pub fn reassert_app_placement(app_name: &str, region: Region, display: CGRect) {
    // A centered window keeps its natural size; after the initial move any
    // further correction would fight the app's own geometry.
    if region == Region::Centered {
        return;
    }
    let Some(pid) = find_pid(app_name) else {
        return;
    };
    let app = ax::application_element(pid);
    let Ok(windows) = ax::windows(&app) else {
        return;
    };
    let Some(window) = pick_main_window(windows) else {
        return;
    };
    let target = region.frame(display);
    let drifted = ax::window_frame(&window)
        .map(|frame| !frames_close(frame, target))
        .unwrap_or(false);
    if drifted {
        let result = ax::set_window_frame(&window, target);
        log_place(&format!(
            "[place:app] drift re-assert {app_name} → {result:?}"
        ));
    }
}

/// App that LaunchServices would use to open `path`, by bundle name.
/// Public so the restack pass can order placed documents by their viewer.
pub fn file_handler_app(path: &str) -> Option<String> {
    default_handler_app(path)
}

fn default_handler_app(path: &str) -> Option<String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSString, NSURL};

    let file_url = NSURL::fileURLWithPath(&NSString::from_str(path));
    let workspace = NSWorkspace::sharedWorkspace();
    let app_url = workspace.URLForApplicationToOpenURL(&file_url)?;
    let app_path = app_url.path()?.to_string();
    let bundle_name = std::path::Path::new(&app_path)
        .file_name()?
        .to_string_lossy();
    bundle_name.strip_suffix(".app").map(str::to_owned)
}

const STABLE_DEADLINE: Duration = Duration::from_secs(4);
const STABLE_POLL: Duration = Duration::from_millis(350);
const FRAME_TOLERANCE: f64 = 2.0;

/// Apply the frame, then keep verifying against the window's actual frame
/// until it holds for two consecutive checks — fixed re-apply delays lose
/// races against the viewer's own re-fitting.
fn place_until_stable(
    app: &ax::AxElement,
    snapshot: &[ax::AxElement],
    label: &str,
    region: Region,
    display: CGRect,
) -> Result<Placement, LayoutError> {
    let deadline = Instant::now() + STABLE_DEADLINE;
    let target = region.frame(display);
    let mut last: Option<Placement> = None;
    let mut stable = 0;

    while Instant::now() < deadline {
        let Ok(window) = pick_target_window(app, snapshot, label) else {
            log_place("[place:file] no target window yet");
            std::thread::sleep(STABLE_POLL);
            continue;
        };

        // Centered keeps the window's natural size: one successful move
        // is final, and re-applying would just make the window twitch.
        if region == Region::Centered {
            let apply = apply_placement(&window, region, display);
            log_place(&format!("[place:file] centered apply={apply:?}"));
            if let Ok(placement) = apply {
                return Ok(placement);
            }
            std::thread::sleep(STABLE_POLL);
            continue;
        }

        // Only touch the window when its frame actually drifted — an
        // unconditional re-apply makes correctly placed windows jitter.
        let on_target = ax::window_frame(&window)
            .map(|frame| frames_close(frame, target))
            .unwrap_or(false);
        if on_target {
            if last.is_none() {
                last = Some(Placement::Full);
            }
            stable += 1;
            if stable >= 2 {
                break;
            }
        } else {
            stable = 0;
            let apply = apply_placement(&window, region, display);
            log_place(&format!(
                "[place:file] apply={apply:?} frame={:?} target={target:?}",
                ax::window_frame(&window)
            ));
            if let Ok(placement) = apply {
                last = Some(placement);
            }
        }
        std::thread::sleep(STABLE_POLL);
    }
    last.ok_or_else(|| LayoutError::WindowTimeout(label.to_owned()))
}

fn frames_close(a: CGRect, b: CGRect) -> bool {
    (a.origin.x - b.origin.x).abs() <= FRAME_TOLERANCE
        && (a.origin.y - b.origin.y).abs() <= FRAME_TOLERANCE
        && (a.size.width - b.size.width).abs() <= FRAME_TOLERANCE
        && (a.size.height - b.size.height).abs() <= FRAME_TOLERANCE
}

/// Prefer a window that did not exist before the document was opened
/// (largest of the fresh ones); fall back to the app's main window when
/// the viewer reused an existing window or tab.
fn pick_target_window(
    app: &ax::AxElement,
    snapshot: &[ax::AxElement],
    label: &str,
) -> Result<ax::AxElement, LayoutError> {
    match current_target_window(app, snapshot) {
        Some(window) => Ok(window),
        None => wait_for_main_window(app, label),
    }
}

/// Non-blocking variant of [`pick_target_window`]: `None` when the app has
/// no windows right now.
fn current_target_window(app: &ax::AxElement, snapshot: &[ax::AxElement]) -> Option<ax::AxElement> {
    let windows = ax::windows(app).ok()?;
    let (fresh, seen): (Vec<_>, Vec<_>) = windows
        .into_iter()
        .partition(|w| !snapshot.iter().any(|old| ax::same_element(w, old)));
    pick_main_window(fresh).or_else(|| pick_main_window(seen))
}

fn frontmost_pid() -> Option<i32> {
    let script = r#"tell application "System Events" to get unix id of first process whose frontmost is true"#;
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;
    first_pid(&output.stdout)
}

/// Open URLs in one fresh Chrome window without placing it — the
/// no-Accessibility fallback that still avoids tab pile-up. Falls back to
/// plain `open` per URL when Chrome is not installed.
pub fn open_urls_unplaced(urls: &[&str]) -> Result<(), LayoutError> {
    if urls.is_empty() {
        return Ok(());
    }
    let Some(chrome) = find_chrome_binary() else {
        for url in urls {
            open_url_without_placement(url)?;
        }
        return Ok(());
    };
    Command::new(&chrome)
        .arg(CHROME_AUTOPLAY_FLAG)
        .arg("--new-window")
        .args(urls)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| LayoutError::Spawn {
            command: "chrome --new-window",
            source,
        })?;
    Ok(())
}

/// Open a document without placement; Chrome-handled files go through the
/// dedicated-window route so they never join an existing tab group.
pub fn open_file_unplaced(path: &str) -> Result<(), LayoutError> {
    if file_handler_app(path).as_deref() == Some("Google Chrome") && find_chrome_binary().is_some()
    {
        return open_urls_unplaced(&[path]);
    }
    Command::new("open")
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source| LayoutError::Spawn {
            command: "open <file>",
            source,
        })?;
    Ok(())
}

fn open_url_without_placement(url: &str) -> Result<(), LayoutError> {
    Command::new("open")
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source| LayoutError::Spawn {
            command: "open <url>",
            source,
        })?;
    Ok(())
}

fn find_chrome_binary() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_owned(),
        format!("{home}/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.exists())
}

/// Resolve an app name to a pid via libproc executable-path lookup.
/// `pgrep` was measured to miss Electron main processes entirely.
fn find_pid(app_name: &str) -> Option<i32> {
    crate::layout::probe_find_pid(app_name)
}

fn first_pid(stdout: &[u8]) -> Option<i32> {
    String::from_utf8_lossy(stdout)
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
}

fn wait_for_pid(app_name: &str) -> Result<i32, LayoutError> {
    let deadline = Instant::now() + WINDOW_WAIT_TIMEOUT;
    loop {
        if let Some(pid) = find_pid(app_name) {
            return Ok(pid);
        }
        if Instant::now() >= deadline {
            return Err(LayoutError::AppNotFound(app_name.to_owned()));
        }
        std::thread::sleep(WINDOW_POLL_INTERVAL);
    }
}
