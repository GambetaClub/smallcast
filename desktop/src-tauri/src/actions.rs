use crate::clipboard as clip;
use crate::types::*;
use std::collections::HashMap;
use tauri::{AppHandle, Manager};

pub fn execute_snippet(app: &AppHandle, s: &Snippet) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    // Copying to the clipboard is the primary action. Frontend then shows a
    // brief toast and calls `hide_and_paste` to complete the flow.
    clip::suppress_next(&s.text);
    app.clipboard()
        .write_text(s.text.clone())
        .map_err(|e| format!("clipboard: {e}"))?;
    Ok(())
}

pub fn hide_and_paste(app: &AppHandle) {
    hide_palette(app);
    std::thread::sleep(std::time::Duration::from_millis(120));
    if let Err(e) = paste_at_cursor() {
        eprintln!("paste skipped: {e}");
    }
}

pub fn execute_quicklink(
    app: &AppHandle,
    q: &Quicklink,
    args: &HashMap<String, String>,
) -> Result<(), String> {
    let url = substitute_placeholders(&q.url, args);
    hide_palette(app);
    open_url(&url, q.open_in)
}

fn hide_palette(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

fn substitute_placeholders(template: &str, args: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('}') {
            let key = &after[..end];
            if let Some(v) = args.get(key) {
                out.push_str(v);
            } else {
                // unknown placeholder — leave literal so user sees it
                out.push('{');
                out.push_str(key);
                out.push('}');
            }
            rest = &after[end + 1..];
        } else {
            out.push_str(&rest[start..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(target_os = "macos")]
fn paste_at_cursor() -> Result<(), String> {
    let mut cmd = std::process::Command::new("osascript");
    cmd.args([
        "-e",
        r#"tell application "System Events" to keystroke "v" using command down"#,
    ]);
    // Snippet paste is the hottest action — a wedged System Events must not
    // freeze it. Bounded so the worst case is a failed paste, not a hang.
    let out = crate::proc::output_with_timeout(cmd, std::time::Duration::from_secs(5))?;
    if !out.status.success() {
        return Err(
            "paste failed — grant Accessibility permission to smallcast in System Settings"
                .into(),
        );
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn paste_at_cursor() -> Result<(), String> {
    // Non-macOS platforms are a later phase.
    Ok(())
}

fn open_url(url: &str, open_in: OpenIn) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        match open_in {
            OpenIn::DefaultBrowser => {
                cmd.arg(url);
            }
            OpenIn::Chrome => {
                cmd.args(["-a", "Google Chrome", url]);
            }
            OpenIn::Safari => {
                cmd.args(["-a", "Safari", url]);
            }
        }
        crate::proc::output_with_timeout(cmd, std::time::Duration::from_secs(5))
            .map_err(|e| format!("open failed: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = open_in;
        webbrowser::open(url).map_err(|e| format!("open failed: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_basic() {
        let mut args = HashMap::new();
        args.insert("query".to_string(), "tauri".to_string());
        assert_eq!(
            substitute_placeholders("https://x.com/search?q={query}", &args),
            "https://x.com/search?q=tauri"
        );
    }

    #[test]
    fn placeholders_missing_key_leaves_literal() {
        let args = HashMap::new();
        assert_eq!(
            substitute_placeholders("hi {name}", &args),
            "hi {name}"
        );
    }

    #[test]
    fn placeholders_none() {
        let args = HashMap::new();
        assert_eq!(substitute_placeholders("plain", &args), "plain");
    }
}
