//! Window placement flows built on the AX wrappers: find the right window
//! for an action, wait for it to exist, and snap it to its region.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::layout::ax::{self, AxElement, Placement};
use crate::layout::{main_display_frame, Region};

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
    ax::is_process_trusted(prompt)
}

/// Snap the front window of a (just launched or already running) app to a
/// region. The front window is intentional here: for app actions the user
/// wants "that app's window" in the region, new or not.
pub fn place_app_window(app_name: &str, region: Region) -> Result<Placement, LayoutError> {
    if !is_trusted(false) {
        return Err(LayoutError::NotTrusted);
    }
    let pid = wait_for_pid(app_name)?;
    let app = ax::application_element(pid);
    let window = wait_for_window(&app, &[], app_name)?;
    Ok(ax::set_window_frame(
        &window,
        region.frame(main_display_frame()),
    )?)
}

/// Open a URL in a fresh browser window and snap that specific window.
///
/// `open --args --new-window` is ignored when the browser is already
/// running, so the browser binary is invoked directly; the new window is
/// identified by diffing against a pre-open snapshot (both PoC findings).
/// Falls back to a plain `open <url>` without placement when no supported
/// browser is installed.
pub fn open_url_in_placed_window(url: &str, region: Region) -> Result<Placement, LayoutError> {
    if !is_trusted(false) {
        return Err(LayoutError::NotTrusted);
    }
    let Some(chrome) = find_chrome_binary() else {
        open_url_without_placement(url)?;
        return Err(LayoutError::AppNotFound("Google Chrome".to_owned()));
    };

    let snapshot = match find_pid("Google Chrome") {
        Some(pid) => ax::windows(&ax::application_element(pid)).unwrap_or_default(),
        None => Vec::new(),
    };

    Command::new(&chrome)
        .args(["--new-window", url])
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
    Ok(ax::set_window_frame(
        &window,
        region.frame(main_display_frame()),
    )?)
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
