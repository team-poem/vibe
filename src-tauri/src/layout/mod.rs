mod ax;
mod placer;
mod probe;

pub use placer::{
    app_window_ready, apps_all_have_windows, file_handler_app, is_trusted, log_place,
    open_file_in_placed_window, open_file_unplaced, open_urls_in_placed_window, open_urls_unplaced,
    place_app_window, reassert_app_placement, LayoutError,
};
pub(crate) use probe::{
    find_all_pids as probe_find_all_pids, find_pid as probe_find_pid, pid_has_real_window,
    real_window_pids,
};

use core_graphics::display::CGDisplay;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use serde::{Deserialize, Serialize};

/// Screen regions an action's window can be snapped to: halves (2-split),
/// thirds (3-split), quadrants (4-split), or the full screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Region {
    Full,
    /// Move to the display without resizing (window keeps its own size).
    Centered,
    LeftHalf,
    RightHalf,
    LeftThird,
    CenterThird,
    RightThird,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Region {
    /// Target frame within a display frame, in the global top-left-origin
    /// coordinate space the AX API uses.
    pub fn frame(self, display: CGRect) -> CGRect {
        let x = display.origin.x;
        let y = display.origin.y;
        let w = display.size.width;
        let h = display.size.height;

        let rect = |rx: f64, ry: f64, rw: f64, rh: f64| CGRect {
            origin: CGPoint {
                x: x + w * rx,
                y: y + h * ry,
            },
            size: CGSize {
                width: w * rw,
                height: h * rh,
            },
        };

        match self {
            // Centered is handled by the placer (needs the window size);
            // the full frame is only a fallback.
            Self::Full | Self::Centered => rect(0.0, 0.0, 1.0, 1.0),
            Self::LeftHalf => rect(0.0, 0.0, 0.5, 1.0),
            Self::RightHalf => rect(0.5, 0.0, 0.5, 1.0),
            Self::LeftThird => rect(0.0, 0.0, 1.0 / 3.0, 1.0),
            Self::CenterThird => rect(1.0 / 3.0, 0.0, 1.0 / 3.0, 1.0),
            Self::RightThird => rect(2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0),
            Self::TopLeft => rect(0.0, 0.0, 0.5, 0.5),
            Self::TopRight => rect(0.5, 0.0, 0.5, 0.5),
            Self::BottomLeft => rect(0.0, 0.5, 0.5, 0.5),
            Self::BottomRight => rect(0.5, 0.5, 0.5, 0.5),
        }
    }
}

/// One connected display, in the global top-left coordinate space shared
/// with the AX API. Sent to the frontend for the arrangement picker.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub id: u32,
    /// Stable across reboots and re-plugs, unlike the numeric id.
    pub uuid: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub is_main: bool,
}

pub fn list_displays() -> Vec<DisplayInfo> {
    let main_id = CGDisplay::main().id;
    CGDisplay::active_displays()
        .unwrap_or_default()
        .into_iter()
        .map(|id| {
            let bounds = CGDisplay::new(id).bounds();
            DisplayInfo {
                id,
                uuid: display_uuid(id).unwrap_or_else(|| id.to_string()),
                x: bounds.origin.x,
                y: bounds.origin.y,
                width: bounds.size.width,
                height: bounds.size.height,
                is_main: id == main_id,
            }
        })
        .collect()
}

/// Stable UUID of a display via CoreGraphics. The numeric CGDisplay id
/// drifts across reboots/re-plugs; the UUID does not — actions persist it.
pub fn display_uuid(display_id: u32) -> Option<String> {
    use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
    use core_foundation::string::{CFString, CFStringRef};
    use std::os::raw::c_void;

    extern "C" {
        fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> *const c_void;
        fn CFUUIDCreateString(alloc: *const c_void, uuid: *const c_void) -> CFStringRef;
    }

    unsafe {
        let uuid = CGDisplayCreateUUIDFromDisplayID(display_id);
        if uuid.is_null() {
            return None;
        }
        let string = CFUUIDCreateString(std::ptr::null(), uuid);
        CFRelease(uuid as CFTypeRef);
        if string.is_null() {
            return None;
        }
        Some(CFString::wrap_under_create_rule(string).to_string())
    }
}

/// `(numeric id as string, uuid)` for every connected display — the
/// store's one-time migration input.
pub fn display_id_uuid_pairs() -> Vec<(String, String)> {
    CGDisplay::active_displays()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| display_uuid(id).map(|uuid| (id.to_string(), uuid)))
        .collect()
}

/// Where an action's display spec points right now.
pub enum DisplayTarget {
    Main,
    Id(u32),
    /// The referenced display is not connected (or the spec is a legacy
    /// id that no longer resolves) — the placement must be skipped, never
    /// silently retargeted.
    Missing,
}

pub fn resolve_display(spec: Option<&str>) -> DisplayTarget {
    let Some(spec) = spec else {
        return DisplayTarget::Main;
    };
    let active = CGDisplay::active_displays().unwrap_or_default();
    for &id in &active {
        if display_uuid(id).as_deref() == Some(spec) {
            return DisplayTarget::Id(id);
        }
    }
    // Legacy numeric spec mid-session (file not yet migrated).
    if let Ok(num) = spec.parse::<u32>() {
        if active.contains(&num) {
            return DisplayTarget::Id(num);
        }
    }
    DisplayTarget::Missing
}

/// Frame for an action's display spec; callers skip `Missing` beforehand,
/// so the main-display fallback here is only a safety net.
pub fn display_frame_for(spec: Option<&str>) -> CGRect {
    match resolve_display(spec) {
        DisplayTarget::Main | DisplayTarget::Missing => display_frame(None),
        DisplayTarget::Id(id) => display_frame(Some(id)),
    }
}

pub fn display_connected(display_id: u32) -> bool {
    CGDisplay::active_displays()
        .unwrap_or_default()
        .contains(&display_id)
}

/// Frame of the routine's target display; falls back to the main display
/// when the id is unset or no longer connected. Uses the display's visible
/// frame (menu bar and Dock excluded) so placed windows never hide under
/// either — full CG bounds is only the last-resort fallback.
pub fn display_frame(display_id: Option<u32>) -> CGRect {
    let id = match display_id {
        Some(id)
            if CGDisplay::active_displays()
                .unwrap_or_default()
                .contains(&id) =>
        {
            id
        }
        _ => CGDisplay::main().id,
    };
    visible_frame(id).unwrap_or_else(|| CGDisplay::new(id).bounds())
}

/// Usable frame of a display (menu bar and Dock excluded), converted from
/// Cocoa's bottom-left global coordinates to the top-left space shared by
/// CG and AX. The NSScreen is matched to the CG display by comparing full
/// frames, which avoids the device-description dictionary entirely.
fn visible_frame(display_id: u32) -> Option<CGRect> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;

    // SAFETY: only read-only geometry getters are touched. AppKit tolerates
    // reading NSScreen off the main thread, and the placement worker must
    // not block on a main-thread hop for every window it snaps.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let screens = NSScreen::screens(mtm);
    let primary_height = screens.iter().next()?.frame().size.height;

    let flip = |x: f64, y: f64, w: f64, h: f64| CGRect {
        origin: CGPoint {
            x,
            y: primary_height - (y + h),
        },
        size: CGSize {
            width: w,
            height: h,
        },
    };

    let target = CGDisplay::new(display_id).bounds();
    screens.iter().find_map(|screen| {
        let f = screen.frame();
        let frame = flip(f.origin.x, f.origin.y, f.size.width, f.size.height);
        let matches = (frame.origin.x - target.origin.x).abs() < 1.0
            && (frame.origin.y - target.origin.y).abs() < 1.0;
        matches.then(|| {
            let v = screen.visibleFrame();
            flip(v.origin.x, v.origin.y, v.size.width, v.size.height)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISPLAY: CGRect = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 1600.0,
            height: 1000.0,
        },
    };

    #[test]
    fn halves_split_the_width() {
        let left = Region::LeftHalf.frame(DISPLAY);
        let right = Region::RightHalf.frame(DISPLAY);
        assert_eq!(left.size.width, 800.0);
        assert_eq!(right.origin.x, 800.0);
        assert_eq!(left.size.height, 1000.0);
    }

    #[test]
    fn quadrants_split_both_axes() {
        let bottom_right = Region::BottomRight.frame(DISPLAY);
        assert_eq!(bottom_right.origin.x, 800.0);
        assert_eq!(bottom_right.origin.y, 500.0);
        assert_eq!(bottom_right.size.width, 800.0);
        assert_eq!(bottom_right.size.height, 500.0);
    }

    #[test]
    fn thirds_tile_the_width() {
        let center = Region::CenterThird.frame(DISPLAY);
        assert!((center.origin.x - 1600.0 / 3.0).abs() < 1e-6);
        assert!((center.size.width - 1600.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn region_serializes_kebab_case() {
        let json = serde_json::to_string(&Region::TopLeft).expect("serialize");
        assert_eq!(json, "\"top-left\"");
        let back: Region = serde_json::from_str("\"center-third\"").expect("deserialize");
        assert_eq!(back, Region::CenterThird);
    }
}
