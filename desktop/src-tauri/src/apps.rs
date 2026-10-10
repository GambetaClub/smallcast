use parking_lot::RwLock;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct AppEntry {
    pub name: String,
    pub path: String,
}

/// Background-warmed cache. `list_apps()` returns from here without ever
/// walking the filesystem on the palette-open critical path. A worker
/// thread (started by `start_background_refresher`) rescans every
/// REFRESH_SECS so freshly installed apps surface within that window.
///
/// The /Applications walk was the dominant cost in `list_palette` and
/// scaled with installed-app count + filesystem cache state. Caching it
/// drops palette opens by hundreds of ms on cold-cache systems.
static APPS: OnceLock<RwLock<Vec<AppEntry>>> = OnceLock::new();

const REFRESH_SECS: u64 = 60;

fn cache() -> &'static RwLock<Vec<AppEntry>> {
    APPS.get_or_init(|| RwLock::new(Vec::new()))
}

pub fn list_apps() -> Vec<AppEntry> {
    {
        let c = cache().read();
        if !c.is_empty() {
            return c.clone();
        }
    }
    // Cold-start fallback: the refresher hasn't completed its first scan
    // yet (palette opened within milliseconds of launch). Pay the cost
    // once synchronously so the user doesn't see an apps-less palette.
    let fresh = scan_all();
    *cache().write() = fresh.clone();
    fresh
}

/// Spawn the cache-warmer thread. Does the first scan immediately, then
/// rescans every REFRESH_SECS forever. Call once at app setup.
pub fn start_background_refresher() {
    std::thread::spawn(|| loop {
        let fresh = scan_all();
        *cache().write() = fresh;
        std::thread::sleep(Duration::from_secs(REFRESH_SECS));
    });
}

#[cfg(target_os = "macos")]
fn scan_all() -> Vec<AppEntry> {
    let mut out: Vec<AppEntry> = Vec::new();
    let mut roots: Vec<PathBuf> = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
    ];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }
    for root in roots {
        scan(&root, &mut out, 0);
    }
    out.push(AppEntry {
        name: "Finder".to_string(),
        path: "/System/Library/CoreServices/Finder.app".to_string(),
    });
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out.dedup_by(|a, b| a.path == b.path);
    out
}

#[cfg(not(target_os = "macos"))]
fn scan_all() -> Vec<AppEntry> {
    Vec::new()
}

fn scan(dir: &Path, out: &mut Vec<AppEntry>, depth: usize) {
    if depth > 2 {
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let is_app = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("app"))
            .unwrap_or(false);
        if is_app {
            if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                out.push(AppEntry {
                    name: name.to_string(),
                    path: path.to_string_lossy().to_string(),
                });
            }
            continue;
        }
        if path.is_dir() {
            // Recurse into "Utilities", etc.
            scan(&path, out, depth + 1);
        }
    }
}
