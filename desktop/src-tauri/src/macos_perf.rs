//! macOS-specific responsiveness tweaks. Two things live here:
//!
//! 1. Window-show animation suppression. NSWindow's default is a slide-in
//!    that can take ~250ms — measured as the dominant cost in our open-paint
//!    metric after we parallelized the backend. Setting animationBehavior to
//!    `.none` makes ⌃Space appear to draw instantly.
//!
//! 2. App Nap opt-out. macOS aggressively throttles "background" apps —
//!    smallcast as a menu-bar accessory with a hidden window qualifies.
//!    After a few minutes of idle, the next ⌃Space pays a wake-up tax
//!    (CPU clock-up, possibly page faults) that we've measured at 11s in
//!    pathological cases. Calling NSProcessInfo.beginActivity with the
//!    user-initiated option keeps us out of nap. We retain the returned
//!    Activity in a static so it isn't released.

#![cfg(target_os = "macos")]

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, NSObjectProtocol, ProtocolObject, Sel};
use objc2::sel;
use objc2_app_kit::{
    NSApplication, NSScreenSaverWindowLevel, NSWindow, NSWindowAnimationBehavior,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSActivityOptions, NSObject, NSProcessInfo, NSString};
use std::ffi::CStr;
use std::sync::OnceLock;

/// Disable the slide-in animation when the palette window is shown. Called
/// once at app setup with the main window's NSWindow*. Idempotent.
///
/// # Safety
/// `ns_window` must be a valid, retained NSWindow pointer for as long as
/// this function runs (Tauri owns the window so this holds during setup).
pub unsafe fn disable_window_animation(ns_window: *mut std::ffi::c_void) {
    if ns_window.is_null() {
        return;
    }
    let window: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };
    window.setAnimationBehavior(NSWindowAnimationBehavior::None);
}

/// Make the palette appear on every Space and float over fullscreen apps.
/// Without this, ⌥Space does nothing while another app is in macOS
/// fullscreen — the palette opens on the original Space and the user sees
/// no UI.
///
/// **Must be called after every `show()`**, not just at app setup.
/// Tauri's `alwaysOnTop: true` flag re-applies `NSFloatingWindowLevel`
/// (3) when the window is shown, which sits *under* fullscreen content
/// and overrides whatever level we set during setup. Re-applying the
/// higher `NSPopUpMenuWindowLevel` (101 — same level dropdown menus
/// use) on every show keeps the palette on top.
///
/// # Safety
/// Same as `disable_window_animation`.
pub unsafe fn make_visible_over_fullscreen(ns_window: *mut std::ffi::c_void) {
    if ns_window.is_null() {
        return;
    }
    let window: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };
    let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::Stationary
        | NSWindowCollectionBehavior::IgnoresCycle;
    window.setCollectionBehavior(behavior);
    // Top-of-the-stack window level. ScreenSaver (1000) is what
    // Spotlight uses; PopUpMenu (101) and MainMenu (24) both lost
    // the race against Tauri's `alwaysOnTop: true` re-application,
    // which now lives off in tauri.conf.json. Belt + suspenders.
    window.setLevel(NSScreenSaverWindowLevel as isize);
    // Don't auto-hide when the app deactivates — the palette is the
    // only window the user interacts with, and "deactivate" fires
    // every time their focus moves to the underlying app.
    window.setHidesOnDeactivate(false);
}

/// Bring smallcast (an LSUIElement / accessory app) to the front so the
/// palette can take key focus inside another app's fullscreen Space.
/// Without this, `NSWindow.makeKeyAndOrderFront` (what Tauri's
/// `set_focus` ends up calling) is allowed to *show* the window over
/// fullscreen content, but text input may still go to the underlying
/// fullscreen app — pressing keys does nothing in our search field.
///
/// `ignoringOtherApps: true` is the same flag Raycast/Spotlight use.
///
/// **Must be called BEFORE `show()`**, not after. `show()` internally
/// calls `orderFront:` which respects app-activation order; if our
/// LSUIElement app is still inactive at that point, the window is
/// ordered onto the user's home Space, not the fullscreen Space they're
/// currently in — they see the palette "open behind" the fullscreen app.
pub fn activate_app() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    // `activateIgnoringOtherApps:` is deprecated in macOS 14+ but is the
    // only path that reliably brings an LSUIElement app to the front
    // *across a fullscreen Space boundary*. The non-deprecated `activate`
    // is a no-op for accessory apps that aren't already frontmost.
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
}

/// Convert the regular NSWindow Tauri creates into an NSPanel-derived
/// class with the `NonactivatingPanel` style mask. This is the *actual*
/// Spotlight / Raycast / Alfred trick — without it, even with all the
/// right window level + collection behavior + activation calls, an
/// LSUIElement app's regular NSWindow does not appear over another
/// app's fullscreen Space. The system treats only NSPanels with this
/// style mask as "true" floating utility windows.
///
/// Plain NSPanel isn't quite enough either: `canBecomeKeyWindow` and
/// `canBecomeMainWindow` return NO for borderless panels, which is why
/// the first NSPanel attempt could float over fullscreen but couldn't
/// receive keyboard input (typing did nothing, Esc did nothing). We
/// register a runtime subclass `SmallcastPanel` that overrides both to
/// return YES, then swap the existing window's `isa` to it. NSPanel
/// adds no ivars to NSWindow, so a class swap is layout-safe.
///
/// Called once during setup, before `make_visible_over_fullscreen`.
///
/// # Safety
/// Same as `disable_window_animation`. Must run after Tauri finishes
/// constructing the window.
pub unsafe fn make_panel(ns_window: *mut std::ffi::c_void) {
    if ns_window.is_null() {
        return;
    }
    let cls = smallcast_panel_class();
    let obj: &AnyObject = unsafe { &*(ns_window as *const AnyObject) };
    unsafe { AnyObject::set_class(obj, cls) };
    let window: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };
    let mask = window.styleMask() | NSWindowStyleMask::NonactivatingPanel;
    window.setStyleMask(mask);
}

extern "C" fn yes_bool(_this: &NSObject, _cmd: Sel) -> Bool {
    Bool::YES
}

/// Register (once) a custom NSPanel subclass that always reports it can
/// become key + main. Required because borderless NSPanels return NO
/// from those defaults — the panel floats over fullscreen but can't
/// take keyboard input.
fn smallcast_panel_class() -> &'static AnyClass {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| {
        let panel = AnyClass::get(CStr::from_bytes_with_nul(b"NSPanel\0").unwrap())
            .expect("NSPanel class must exist on macOS");
        let mut builder = ClassBuilder::new(
            CStr::from_bytes_with_nul(b"SmallcastPanel\0").unwrap(),
            panel,
        )
        .expect("SmallcastPanel class registration failed");
        unsafe {
            builder.add_method(
                sel!(canBecomeKeyWindow),
                yes_bool as extern "C" fn(_, _) -> _,
            );
            builder.add_method(
                sel!(canBecomeMainWindow),
                yes_bool as extern "C" fn(_, _) -> _,
            );
        }
        builder.register()
    })
}

/// Force the window to the front regardless of app-activation state.
/// Tauri's `show()` calls `orderFront:`, which on an inactive app may
/// not cross the fullscreen-Space boundary even after we've activated.
/// `orderFrontRegardless` is the no-questions-asked variant Spotlight
/// uses — required as a final belt-and-suspenders for fullscreen Spaces.
///
/// # Safety
/// Same as `disable_window_animation`.
pub unsafe fn order_front_regardless(ns_window: *mut std::ffi::c_void) {
    if ns_window.is_null() {
        return;
    }
    let window: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };
    window.orderFrontRegardless();
}

/// Holds the activity returned by NSProcessInfo so it stays retained for
/// the lifetime of the process. Dropping it ends the activity and lets
/// App Nap kick back in.
static APP_NAP_ACTIVITY: OnceLock<ActivityHandle> = OnceLock::new();

struct ActivityHandle {
    _activity: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

unsafe impl Send for ActivityHandle {}
unsafe impl Sync for ActivityHandle {}

/// Tell macOS not to throttle this process. Called once at startup.
/// Returns true if the activity was started; false if it was already started.
pub fn opt_out_of_app_nap() -> bool {
    if APP_NAP_ACTIVITY.get().is_some() {
        return false;
    }
    let info = NSProcessInfo::processInfo();
    let reason = NSString::from_str("smallcast palette responsiveness");
    // UserInitiated = active user-driven work. Combined with
    // LatencyCritical so the OS keeps timers + I/O at full speed.
    // We allow idle system sleep — no reason to keep the laptop awake.
    let options = NSActivityOptions::UserInitiatedAllowingIdleSystemSleep
        | NSActivityOptions::LatencyCritical;
    let activity = info.beginActivityWithOptions_reason(options, &reason);
    let _ = APP_NAP_ACTIVITY.set(ActivityHandle { _activity: activity });
    true
}
