//! Process and window presence probes that work without Accessibility
//! access. `pgrep` was measured to miss Electron main processes entirely
//! (Cursor pid present in `ps` yet invisible to `pgrep -x` and `-f`), so
//! pids come from libproc path lookups; window presence comes from the
//! CoreGraphics window list, which sees windows on every Space.

use std::os::raw::{c_int, c_void};

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::TCFType;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::{CFNumber, CFNumberRef};
use core_foundation::string::CFString;

const PROC_ALL_PIDS: u32 = 1;
const PID_PATH_BUFFER: usize = 4096;

/// Real application windows are at layer 0 and at least this large;
/// everything smaller at layer 0 (menu-bar backing strips, tab previews,
/// 1x1 helpers) is a phantom, per live measurement.
const MIN_WINDOW_WIDTH: f64 = 200.0;
const MIN_WINDOW_HEIGHT: f64 = 150.0;

extern "C" {
    fn proc_listpids(kind: u32, typeinfo: u32, buffer: *mut c_void, buffersize: c_int) -> c_int;
    fn proc_pidpath(pid: c_int, buffer: *mut c_void, buffersize: u32) -> c_int;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;
}

/// `kCGWindowListOptionAll` — includes windows on other Spaces and
/// minimized windows, which AX cannot see.
const CG_WINDOW_LIST_OPTION_ALL: u32 = 0;

/// True when `path` is the main executable of the named app bundle.
/// Helpers live under `Contents/Frameworks/*.app/` and never match.
/// Chrome-family browsers run their main binary from a code-sign clone
/// (`…/com.google.Chrome.code_sign_clone/…/Google Chrome.app.bundle/…`),
/// so the `.app.bundle` form must match too — measured live: `ps` shows
/// the original path while `proc_pidpath` reports the clone.
fn path_matches_app(path: &str, app_name: &str) -> bool {
    for needle in [
        format!("/{app_name}.app/Contents/MacOS/"),
        format!("/{app_name}.app.bundle/Contents/MacOS/"),
    ] {
        if let Some(index) = path.find(&needle) {
            return !path[..index].contains("/Contents/Frameworks");
        }
    }
    false
}

/// Visit every process as `(pid, executable path)` in one libproc scan;
/// stop early when the visitor returns `false`.
fn scan_processes(mut visit: impl FnMut(i32, &str) -> bool) {
    let needed = unsafe { proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
    if needed <= 0 {
        return;
    }
    // Head-room for processes spawned between the two calls.
    let capacity = needed as usize / std::mem::size_of::<i32>() + 64;
    let mut pids = vec![0 as c_int; capacity];
    let written = unsafe {
        proc_listpids(
            PROC_ALL_PIDS,
            0,
            pids.as_mut_ptr() as *mut c_void,
            (capacity * std::mem::size_of::<i32>()) as c_int,
        )
    };
    if written <= 0 {
        return;
    }
    let count = written as usize / std::mem::size_of::<i32>();

    let mut buffer = [0u8; PID_PATH_BUFFER];
    for &pid in pids[..count].iter() {
        if pid <= 0 {
            continue;
        }
        let len = unsafe {
            proc_pidpath(
                pid,
                buffer.as_mut_ptr() as *mut c_void,
                PID_PATH_BUFFER as u32,
            )
        };
        if len <= 0 {
            continue;
        }
        let path = String::from_utf8_lossy(&buffer[..len as usize]);
        if !visit(pid, &path) {
            return;
        }
    }
}

/// Pid of the named app's main process, via executable path lookup.
/// Several processes can match one name at once (the real browser plus a
/// short-lived `--new-window` hand-off stub at the same path); the one
/// that owns a real window is the browser — prefer it.
pub fn find_pid(app_name: &str) -> Option<i32> {
    let mut matches: Vec<i32> = Vec::new();
    scan_processes(|pid, path| {
        if path_matches_app(path, app_name) {
            matches.push(pid);
        }
        true
    });
    match matches.len() {
        0 => None,
        1 => Some(matches[0]),
        _ => {
            let windowed = real_window_pids();
            matches
                .iter()
                .copied()
                .find(|pid| windowed.contains(&i64::from(*pid)))
                .or(Some(matches[0]))
        }
    }
}

/// Resolve several app names with a single process scan; every matching
/// pid per name is kept so callers can prefer the windowed one.
pub fn find_all_pids(app_names: &[String]) -> Vec<Vec<i32>> {
    let mut found: Vec<Vec<i32>> = vec![Vec::new(); app_names.len()];
    scan_processes(|pid, path| {
        for (slot, name) in found.iter_mut().zip(app_names.iter()) {
            if path_matches_app(path, name) {
                slot.push(pid);
            }
        }
        true
    });
    found
}

/// True when the process owns at least one real window on any Space.
/// Matching is by owner pid — window owner names are localized and
/// unreliable. Needs no Accessibility permission.
pub fn pid_has_real_window(pid: i32) -> bool {
    real_window_pids().contains(&i64::from(pid))
}

/// Owner pids that have at least one real window, from one window-list
/// enumeration — the batch counterpart of [`pid_has_real_window`], sharing
/// the same discriminant constants.
pub fn real_window_pids() -> std::collections::HashSet<i64> {
    let mut owners = std::collections::HashSet::new();
    let raw = unsafe { CGWindowListCopyWindowInfo(CG_WINDOW_LIST_OPTION_ALL, 0) };
    if raw.is_null() {
        return owners;
    }
    let windows: CFArray = unsafe { CFArray::wrap_under_create_rule(raw) };

    let pid_key = CFString::from_static_string("kCGWindowOwnerPID");
    let layer_key = CFString::from_static_string("kCGWindowLayer");
    let bounds_key = CFString::from_static_string("kCGWindowBounds");
    let width_key = CFString::from_static_string("Width");
    let height_key = CFString::from_static_string("Height");

    for item in windows.iter() {
        let dict: CFDictionary =
            unsafe { CFDictionary::wrap_under_get_rule(*item as CFDictionaryRef) };

        let Some(owner) = dict_i64(&dict, &pid_key) else {
            continue;
        };
        if owners.contains(&owner) {
            continue;
        }
        if dict_i64(&dict, &layer_key) != Some(0) {
            continue;
        }
        let Some(bounds_ptr) = dict.find(bounds_key.as_concrete_TypeRef() as *const c_void) else {
            continue;
        };
        let bounds: CFDictionary =
            unsafe { CFDictionary::wrap_under_get_rule(*bounds_ptr as CFDictionaryRef) };
        let width = dict_f64(&bounds, &width_key).unwrap_or(0.0);
        let height = dict_f64(&bounds, &height_key).unwrap_or(0.0);
        if width >= MIN_WINDOW_WIDTH && height >= MIN_WINDOW_HEIGHT {
            owners.insert(owner);
        }
    }
    owners
}

fn dict_i64(dict: &CFDictionary, key: &CFString) -> Option<i64> {
    let value = dict.find(key.as_concrete_TypeRef() as *const c_void)?;
    let number = unsafe { CFNumber::wrap_under_get_rule(*value as CFNumberRef) };
    number.to_i64()
}

fn dict_f64(dict: &CFDictionary, key: &CFString) -> Option<f64> {
    let value = dict.find(key.as_concrete_TypeRef() as *const c_void)?;
    let number = unsafe { CFNumber::wrap_under_get_rule(*value as CFNumberRef) };
    number.to_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_main_executable_path() {
        assert!(path_matches_app(
            "/Applications/Cursor.app/Contents/MacOS/Cursor",
            "Cursor"
        ));
        assert!(path_matches_app(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "Google Chrome"
        ));
    }

    /// Manual smoke check against the live system — run with
    /// `cargo test live_probe -- --ignored` while Cursor is open.
    #[test]
    #[ignore]
    fn live_probe_finds_running_apps() {
        let pid = find_pid("Cursor").expect("Cursor should be running");
        assert!(pid_has_real_window(pid), "Cursor should have a real window");
        assert!(find_pid("Finder").is_some());
    }

    #[test]
    fn matches_code_sign_clone_path() {
        assert!(path_matches_app(
            "/private/var/folders/nl/x/T/com.google.Chrome.code_sign_clone/code_sign_clone.abc/Google Chrome.app.bundle/Contents/MacOS/Google Chrome",
            "Google Chrome"
        ));
    }

    #[test]
    fn rejects_helpers_and_other_apps() {
        assert!(!path_matches_app(
            "/Applications/Cursor.app/Contents/Frameworks/Cursor Helper.app/Contents/MacOS/Cursor Helper",
            "Cursor"
        ));
        assert!(!path_matches_app(
            "/Applications/Figma.app/Contents/MacOS/Figma",
            "Cursor"
        ));
    }
}

#[cfg(test)]
mod chrome_live {
    use super::*;

    #[test]
    #[ignore]
    fn chrome_probe() {
        let pid = find_pid("Google Chrome").expect("chrome running");
        println!(
            "chrome pid={pid} has_real_window={}",
            pid_has_real_window(pid)
        );
        let app = crate::layout::ax::application_element(pid);
        match crate::layout::ax::windows(&app) {
            Ok(w) => println!("ax windows = {}", w.len()),
            Err(e) => println!("ax windows ERR: {e}"),
        }
    }
}
