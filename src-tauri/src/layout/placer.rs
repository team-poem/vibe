//! Window placement flows built on the AX wrappers: find the right window
//! for an action, wait for it to exist, and snap it to its region.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use core_graphics::geometry::{CGPoint, CGRect, CGSize};

use crate::layout::ax::{self, AxElement, Placement};
use crate::layout::Region;

const WINDOW_WAIT_TIMEOUT: Duration = Duration::from_secs(8);
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

/// Snap the front window of a (just launched or already running) app to a
/// region. The front window is intentional here: for app actions the user
/// wants "that app's window" in the region, new or not.
pub fn place_app_window(
    app_name: &str,
    region: Region,
    display: CGRect,
) -> Result<Placement, LayoutError> {
    if !is_trusted(false) {
        return Err(LayoutError::NotTrusted);
    }
    let pid = wait_for_pid(app_name)?;
    let app = ax::application_element(pid);
    let window = wait_for_main_window(&app, app_name)?;
    apply_placement(&window, region, display)
}

/// Wait until the app has windows, then return the largest one — the first
/// AX window can be a splash screen or utility panel, not the main window.
fn wait_for_main_window(app: &ax::AxElement, label: &str) -> Result<ax::AxElement, LayoutError> {
    let deadline = Instant::now() + WINDOW_WAIT_TIMEOUT;
    loop {
        if let Ok(windows) = ax::windows(app) {
            if let Some(window) = pick_main_window(windows) {
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

/// Single non-blocking check: does the app have a window yet? Used by the
/// restack guardian to react to slow launches without waiting for them.
pub fn app_window_ready(app_name: &str) -> bool {
    let Some(pid) = find_pid(app_name) else {
        return false;
    };
    let app = ax::application_element(pid);
    ax::windows(&app).map(|w| !w.is_empty()).unwrap_or(false)
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

    let snapshot = match find_pid("Google Chrome") {
        Some(pid) => ax::windows(&ax::application_element(pid)).unwrap_or_default(),
        None => Vec::new(),
    };

    Command::new(&chrome)
        .arg("--new-window")
        .args(urls)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| LayoutError::Spawn {
            command: "chrome --new-window",
            source,
        })?;

    let pid = wait_for_pid("Google Chrome")?;
    let app = ax::application_element(pid);
    let window = wait_for_window(&app, &snapshot, "Google Chrome")?;
    apply_placement(&window, region, display)
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
        return Err(LayoutError::NotTrusted);
    }
    let handler = default_handler_app(path);
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
    place_until_stable(&app, &snapshot, label, region, display)
}

/// App that LaunchServices would use to open `path`, by bundle name.
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
            std::thread::sleep(STABLE_POLL);
            continue;
        };
        if let Ok(placement) = apply_placement(&window, region, display) {
            last = Some(placement);
        }
        std::thread::sleep(STABLE_POLL);

        let settled = match last {
            // Centered / fixed-size windows never match the target frame;
            // a successful apply counts as settled.
            Some(Placement::MovedOnly) => true,
            Some(Placement::Full) => ax::window_frame(&window)
                .map(|frame| frames_close(frame, target))
                .unwrap_or(false),
            None => false,
        };
        if settled {
            stable += 1;
            if stable >= 2 {
                break;
            }
        } else {
            stable = 0;
        }
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
    if let Ok(windows) = ax::windows(app) {
        let fresh: Vec<ax::AxElement> = windows
            .into_iter()
            .filter(|w| !snapshot.iter().any(|old| ax::same_element(w, old)))
            .collect();
        if let Some(window) = pick_main_window(fresh) {
            return Ok(window);
        }
    }
    wait_for_main_window(app, label)
}

fn frontmost_pid() -> Option<i32> {
    let script = r#"tell application "System Events" to get unix id of first process whose frontmost is true"#;
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;
    first_pid(&output.stdout)
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

/// First window not present in `snapshot` (front-most first); with an empty
/// snapshot this is simply the front window.
fn wait_for_window(
    app: &AxElement,
    snapshot: &[AxElement],
    label: &str,
) -> Result<AxElement, LayoutError> {
    let deadline = Instant::now() + WINDOW_WAIT_TIMEOUT;
    loop {
        if let Ok(windows) = ax::windows(app) {
            let fresh = windows
                .into_iter()
                .find(|w| !snapshot.iter().any(|old| ax::same_element(w, old)));
            if let Some(window) = fresh {
                return Ok(window);
            }
        }
        if Instant::now() >= deadline {
            return Err(LayoutError::WindowTimeout(label.to_owned()));
        }
        std::thread::sleep(WINDOW_POLL_INTERVAL);
    }
}

/// Resolve an app name to a pid. `pgrep -x` misses apps whose executable
/// name differs from the app name (e.g. VS Code runs as "Electron"), so
/// fall back to matching the bundle path.
fn find_pid(app_name: &str) -> Option<i32> {
    let exact = Command::new("pgrep").args(["-x", app_name]).output().ok()?;
    if let Some(pid) = first_pid(&exact.stdout) {
        return Some(pid);
    }
    let bundle_pattern = format!("{app_name}.app/Contents/MacOS/");
    let by_bundle = Command::new("pgrep")
        .args(["-f", &bundle_pattern])
        .output()
        .ok()?;
    first_pid(&by_bundle.stdout)
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
