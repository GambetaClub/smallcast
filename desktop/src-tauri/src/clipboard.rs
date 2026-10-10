use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tauri::AppHandle;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: String,
    pub text: String,
    pub copied_at: String,
    #[serde(default)]
    pub char_count: usize,
}

const MAX_ENTRIES: usize = 200;
const MAX_TEXT_LEN: usize = 50_000; // skip absurdly large pastes
const POLL_MS: u64 = 800;

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

struct State {
    entries: Vec<ClipboardEntry>,
    last_seen: Option<String>,
    suppress: Option<String>,
}

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        let entries = load_from_disk().unwrap_or_default();
        let last_seen = entries.first().map(|e| e.text.clone());
        Mutex::new(State {
            entries,
            last_seen,
            suppress: None,
        })
    })
}

pub fn store_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("smallcast/clipboard.json"))
}

fn load_from_disk() -> Option<Vec<ClipboardEntry>> {
    let path = store_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_to_disk(entries: &[ClipboardEntry]) {
    let Some(path) = store_path() else { return; };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("json.tmp");
    let Ok(bytes) = serde_json::to_vec_pretty(entries) else { return; };
    if std::fs::write(&tmp, &bytes).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub fn list() -> Vec<ClipboardEntry> {
    state().lock().entries.clone()
}

pub fn clear() {
    let mut s = state().lock();
    s.entries.clear();
    s.last_seen = None;
    save_to_disk(&s.entries);
}

pub fn delete(id: &str) {
    let mut s = state().lock();
    s.entries.retain(|e| e.id != id);
    save_to_disk(&s.entries);
}

/// Tell the watcher to ignore the next clipboard write that matches `text`.
/// Used when the app itself writes (snippet paste, copy-back from history)
/// so we don't echo our own writes into history.
pub fn suppress_next(text: &str) {
    let mut s = state().lock();
    s.suppress = Some(text.to_string());
    s.last_seen = Some(text.to_string());
}

fn record(text: String) {
    if text.is_empty() || text.len() > MAX_TEXT_LEN {
        return;
    }
    let mut s = state().lock();
    if s.suppress.as_deref() == Some(text.as_str()) {
        s.suppress = None;
        s.last_seen = Some(text);
        return;
    }
    if s.last_seen.as_deref() == Some(text.as_str()) {
        return;
    }
    // Promote existing entry to top instead of duplicating.
    if let Some(pos) = s.entries.iter().position(|e| e.text == text) {
        let mut existing = s.entries.remove(pos);
        existing.copied_at = Utc::now().to_rfc3339();
        s.entries.insert(0, existing);
    } else {
        s.entries.insert(
            0,
            ClipboardEntry {
                id: Uuid::now_v7().to_string(),
                char_count: text.chars().count(),
                text: text.clone(),
                copied_at: Utc::now().to_rfc3339(),
            },
        );
    }
    s.last_seen = Some(text);
    if s.entries.len() > MAX_ENTRIES {
        s.entries.truncate(MAX_ENTRIES);
    }
    save_to_disk(&s.entries);
}

pub fn start_watcher(app: AppHandle) {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    // Touch state once so the OnceLock initializes from disk on the watcher
    // thread; afterwards lock-and-go is cheap.
    let _ = state().lock();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(POLL_MS));
        let Ok(text) = app.clipboard().read_text() else { continue; };
        record(text);
    });
}
