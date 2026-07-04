//! Safe wrappers over the macOS Accessibility (AXUIElement) C API.
//! All `unsafe` in the layout feature is confined to this module.
//! Ported from the `poc/window-layout` branch.

use std::ffi::c_void;

use accessibility_sys::{
    kAXErrorSuccess, kAXPositionAttribute, kAXSizeAttribute, kAXTrustedCheckOptionPrompt,
    kAXValueTypeCGPoint, kAXValueTypeCGSize, kAXWindowsAttribute, AXIsProcessTrusted,
    AXIsProcessTrustedWithOptions, AXUIElementCopyAttributeValue, AXUIElementCreateApplication,
    AXUIElementRef, AXUIElementSetAttributeValue, AXValueCreate, AXValueGetValue, AXValueRef,
};
use core_foundation::array::CFArray;
use core_foundation::base::{CFEqual, CFRelease, CFRetain, CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};

#[derive(Debug, thiserror::Error)]
pub enum AxError {
    #[error("accessibility call {call} failed with AXError {code}")]
    Call { call: &'static str, code: i32 },
    #[error("attribute {0} has an unexpected type")]
    UnexpectedType(&'static str),
}

/// Owned AXUIElement handle, released on drop.
pub struct AxElement(AXUIElementRef);

impl Drop for AxElement {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as CFTypeRef) };
    }
}

/// Ask macOS whether this process may drive the Accessibility API. With
/// `prompt`, the system permission dialog is shown on first call and the
/// app is added to the Accessibility list in System Settings.
pub fn is_process_trusted(prompt: bool) -> bool {
    if !prompt {
        return unsafe { AXIsProcessTrusted() };
    }
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let options = CFDictionary::from_CFType_pairs(&[(
            key.as_CFType(),
            CFBoolean::true_value().as_CFType(),
        )]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
    }
}

/// CFEqual-based identity: true when both handles refer to the same
/// on-screen window, even across separate `windows()` snapshots.
pub fn same_element(a: &AxElement, b: &AxElement) -> bool {
    unsafe { CFEqual(a.0 as CFTypeRef, b.0 as CFTypeRef) != 0 }
}

pub fn application_element(pid: i32) -> AxElement {
    unsafe { AxElement(AXUIElementCreateApplication(pid)) }
}

/// The application's windows, front to back.
pub fn windows(app: &AxElement) -> Result<Vec<AxElement>, AxError> {
    let value = copy_attribute(app, kAXWindowsAttribute)?;
    let array = value
        .downcast::<CFArray>()
        .ok_or(AxError::UnexpectedType(kAXWindowsAttribute))?;

    let mut result = Vec::with_capacity(array.len() as usize);
    for item in array.iter() {
        let element_ref = *item as AXUIElementRef;
        unsafe { CFRetain(element_ref as CFTypeRef) };
        result.push(AxElement(element_ref));
    }
    Ok(result)
}

/// Current size of a window, for move-without-resize placement.
pub fn window_size(window: &AxElement) -> Option<CGSize> {
    let value = copy_attribute(window, kAXSizeAttribute).ok()?;
    let mut size = CGSize {
        width: 0.0,
        height: 0.0,
    };
    let ok = unsafe {
        AXValueGetValue(
            value.as_CFTypeRef() as AXValueRef,
            kAXValueTypeCGSize,
            &mut size as *mut _ as *mut c_void,
        )
    };
    ok.then_some(size)
}

/// Move a window without touching its size.
pub fn set_window_position(window: &AxElement, origin: CGPoint) -> Result<(), AxError> {
    set_point(window, kAXPositionAttribute, origin)
}

/// How much of the requested frame a window actually accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Full,
    /// Window moved but refused the resize (fixed-size windows such as
    /// Calculator). Position-only is still useful, so this is not an error.
    MovedOnly,
}

/// Move and resize a window to `frame`. Position is set before size and the
/// position is applied again afterwards because some apps re-clamp their
/// origin while resizing (the Rectangle app does the same dance).
pub fn set_window_frame(window: &AxElement, frame: CGRect) -> Result<Placement, AxError> {
    set_point(window, kAXPositionAttribute, frame.origin)?;
    let resize = set_size(window, kAXSizeAttribute, frame.size);
    set_point(window, kAXPositionAttribute, frame.origin)?;
    Ok(if resize.is_ok() {
        Placement::Full
    } else {
        Placement::MovedOnly
    })
}

fn copy_attribute(element: &AxElement, attribute: &'static str) -> Result<CFType, AxError> {
    let name = CFString::new(attribute);
    let mut value: CFTypeRef = std::ptr::null();
    let code =
        unsafe { AXUIElementCopyAttributeValue(element.0, name.as_concrete_TypeRef(), &mut value) };
    if code != kAXErrorSuccess {
        return Err(AxError::Call {
            call: attribute,
            code,
        });
    }
    Ok(unsafe { CFType::wrap_under_create_rule(value) })
}

fn set_point(element: &AxElement, attribute: &'static str, point: CGPoint) -> Result<(), AxError> {
    let value = unsafe { AXValueCreate(kAXValueTypeCGPoint, &point as *const _ as *const c_void) };
    set_value(element, attribute, value as CFTypeRef)
}

fn set_size(element: &AxElement, attribute: &'static str, size: CGSize) -> Result<(), AxError> {
    let value = unsafe { AXValueCreate(kAXValueTypeCGSize, &size as *const _ as *const c_void) };
    set_value(element, attribute, value as CFTypeRef)
}

fn set_value(
    element: &AxElement,
    attribute: &'static str,
    value: CFTypeRef,
) -> Result<(), AxError> {
    let name = CFString::new(attribute);
    let code =
        unsafe { AXUIElementSetAttributeValue(element.0, name.as_concrete_TypeRef(), value) };
    unsafe { CFRelease(value) };
    if code != kAXErrorSuccess {
        return Err(AxError::Call {
            call: attribute,
            code,
        });
    }
    Ok(())
}
