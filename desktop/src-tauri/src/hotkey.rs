use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

/// Monotonic ms counter so the auto-hide-on-blur handler can ignore
/// blurs that fire as a side-effect of the hotkey press itself.
///
/// Without this: when the palette has focus and the user presses ⌥Space,
/// the keypress causes a focus event before our shortcut handler runs.
/// The blur handler hides the window, then the shortcut handler reads
/// is_visible() == false and shows it again — net effect, the palette
/// stays open and the user perceives the toggle as a "filter reset".
/// With it: blurs within HOTKEY_BLUR_GRACE_MS of a hotkey press are
/// ignored, so toggle_palette is the sole decider.
static APP_START: OnceLock<Instant> = OnceLock::new();
static LAST_HOTKEY_MS: AtomicU64 = AtomicU64::new(0);
const HOTKEY_BLUR_GRACE_MS: u64 = 250;

fn now_ms() -> u64 {
    APP_START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis() as u64
}

/// Returns true if the most recent hotkey press is recent enough that
/// the auto-hide-on-blur handler should *not* hide the window — the
/// toggle/show logic will handle it.
pub fn blur_within_hotkey_grace() -> bool {
    let last = LAST_HOTKEY_MS.load(Ordering::SeqCst);
    if last == 0 {
        return false;
    }
    now_ms().saturating_sub(last) < HOTKEY_BLUR_GRACE_MS
}

pub fn default_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::SUPER), Code::Space)
}

pub fn clipboard_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV)
}

pub fn on_palette_shortcut(app: &AppHandle, _shortcut: &Shortcut, state: ShortcutState) {
    if state != ShortcutState::Pressed {
        return;
    }
    // Stamp before toggling so the blur handler can recognize the
    // hotkey-induced blur and bow out.
    LAST_HOTKEY_MS.store(now_ms(), Ordering::SeqCst);
    toggle_palette(app);
}

pub fn on_clipboard_shortcut(app: &AppHandle, _shortcut: &Shortcut, state: ShortcutState) {
    if state != ShortcutState::Pressed {
        return;
    }
    LAST_HOTKEY_MS.store(now_ms(), Ordering::SeqCst);
    open_clipboard(app);
}

pub fn toggle_palette(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else {
        return;
    };
    match w.is_visible() {
        Ok(true) => {
            let _ = w.hide();
        }
        _ => {
            // Step order is load-bearing for "appear over a fullscreen
            // app". Don't reorder without testing in fullscreen Safari.
            //
            //   1. Activate this LSUIElement app FIRST. orderFront on an
            //      inactive accessory app does not cross the fullscreen-
            //      Space boundary — the window ends up on the user's
            //      home Space and they see the palette "open behind"
            //      the fullscreen app. v0.2.3 had show() before
            //      activate; this was the actual bug.
            //   2. Set NSWindow level + CanJoinAllSpaces|FullScreenAux
            //      *before* show, so the orderFront in show() lands at
            //      screen-saver level on the active Space.
            //   3. show() calls orderFront:, which respects app order;
            //      we follow it with orderFrontRegardless to punch
            //      through unconditionally (Spotlight pattern).
            //   4. Re-apply overlay — Tauri's alwaysOnTop handling can
            //      reset the level back to floating after show.
            activate_app();
            apply_fullscreen_overlay(&w);
            let _ = w.center();
            let _ = w.show();
            order_front_regardless(&w);
            let _ = w.set_focus();
            apply_fullscreen_overlay(&w);
            let _ = app.emit("palette:show", ());
        }
    }
}

pub fn open_clipboard(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else {
        return;
    };
    // Same step order as toggle_palette — see the comment there.
    activate_app();
    apply_fullscreen_overlay(&w);
    let _ = w.center();
    let _ = w.show();
    order_front_regardless(&w);
    let _ = w.set_focus();
    apply_fullscreen_overlay(&w);
    let _ = app.emit("clipboard:show", ());
}

/// Re-apply the NSWindow flags that make the palette float over
/// fullscreen apps. Must run *after* `show()` because Tauri's
/// `alwaysOnTop: true` resets the level back to `NSFloatingWindowLevel`
/// (3) on every show, which sits below fullscreen content.
#[cfg(target_os = "macos")]
fn apply_fullscreen_overlay(w: &tauri::WebviewWindow) {
    if let Ok(ns_window) = w.ns_window() {
        unsafe {
            crate::macos_perf::make_visible_over_fullscreen(ns_window);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn apply_fullscreen_overlay(_: &tauri::WebviewWindow) {}

#[cfg(target_os = "macos")]
fn activate_app() {
    crate::macos_perf::activate_app();
}

#[cfg(not(target_os = "macos"))]
fn activate_app() {}

#[cfg(target_os = "macos")]
fn order_front_regardless(w: &tauri::WebviewWindow) {
    if let Ok(ns_window) = w.ns_window() {
        unsafe {
            crate::macos_perf::order_front_regardless(ns_window);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn order_front_regardless(_: &tauri::WebviewWindow) {}
