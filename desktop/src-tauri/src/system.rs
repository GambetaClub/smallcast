//! macOS quick actions — Lock Screen, Sleep, Empty Trash, Restart, Shut Down,
//! Log Out. All shell out (osascript / pmset). Destructive actions are gated
//! by a frontend confirm step before they ever land here, but each call is
//! still safe to fire on its own — we don't add extra "are you sure" prompts.

use std::time::Duration;

// osascript driving another app blocks on that app's Apple Event reply — an
// unresponsive Finder/System Events would hang the child forever. Cap it.
const OSASCRIPT_TIMEOUT: Duration = Duration::from_secs(8);

fn osascript(script: &str) -> Result<(), String> {
    let mut cmd = std::process::Command::new("osascript");
    cmd.arg("-e").arg(script);
    let out = crate::proc::output_with_timeout(cmd, OSASCRIPT_TIMEOUT)?;
    if !out.status.success() {
        return Err("osascript returned non-zero".into());
    }
    Ok(())
}

/// Lock the screen. Sends ⌃⌘Q via System Events — the system shortcut
/// macOS has bound to "Lock Screen" since Mojave. Inherits smallcast's
/// Accessibility permission (same one the window-management commands use).
pub fn lock_screen() -> Result<(), String> {
    osascript(
        r#"tell application "System Events" to keystroke "q" using {control down, command down}"#,
    )
}

/// Put the Mac to sleep immediately. `pmset sleepnow` is the canonical CLI
/// equivalent of the Apple-menu Sleep item; no permission required.
pub fn sleep_now() -> Result<(), String> {
    let mut cmd = std::process::Command::new("pmset");
    cmd.arg("sleepnow");
    let out = crate::proc::output_with_timeout(cmd, OSASCRIPT_TIMEOUT)?;
    if !out.status.success() {
        return Err("pmset returned non-zero".into());
    }
    Ok(())
}

pub fn empty_trash() -> Result<(), String> {
    osascript(r#"tell application "Finder" to empty trash"#)
}

pub fn restart() -> Result<(), String> {
    osascript(r#"tell application "System Events" to restart"#)
}

pub fn shut_down() -> Result<(), String> {
    osascript(r#"tell application "System Events" to shut down"#)
}

pub fn log_out() -> Result<(), String> {
    // `log out` triggers the standard macOS confirm sheet on its own; our
    // frontend confirm is redundant but harmless and keeps UX consistent
    // with restart/shutdown.
    osascript(r#"tell application "System Events" to log out"#)
}
