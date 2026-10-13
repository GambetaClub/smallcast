//! File search plugin. Asks the fsearch daemon (whole-disk index) on each
//! debounced keystroke and filters out files that aren't the user's own.
//!
//! Query semantics are decided by the frontend (it parses `:png`, `:img`,
//! `:newest` etc. and hands us a structured `FileSearchOpts`). This module
//! just runs the search, decorates each hit with metadata, and answers the
//! per-row actions the palette needs (open, reveal, copy path, copy image,
//! thumbnail).

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    /// Parent directory, preserved as the user typed it (so home-relative
    /// paths stay short in the row subtitle).
    pub parent: String,
    pub size: u64,
    /// Unix milliseconds.
    pub modified_at: u64,
    pub is_image: bool,
    pub is_dir: bool,
    pub ext: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FileSearchOpts {
    pub query: Option<String>,
    pub extensions: Vec<String>,
    pub category: Option<String>, // "image"
    pub roots: Vec<String>,
    pub sort_by_mtime: bool,
    pub limit: Option<usize>,
    pub folders: bool,
}

const DEFAULT_LIMIT: usize = 100;
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "heic"];
/// macOS screencapture writes .mov for screen recordings; users searching
/// "screenshots" usually want those alongside still PNGs.
const SCREENSHOT_VIDEO_EXTS: &[&str] = &["mov", "mp4"];

pub fn default_roots() -> Vec<String> {
    dirs::home_dir().map(|h| vec![h.to_string_lossy().to_string()]).unwrap_or_default()
}

/// The empty "recent files" list leaves out code, which changes on every save.
fn recent_roots() -> Vec<String> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    ["Desktop", "Documents", "Downloads", "Pictures", "Movies", "Library/Mobile Documents/com~apple~CloudDocs"]
        .iter()
        .map(|s| home.join(s).to_string_lossy().to_string())
        .filter(|p| Path::new(p).exists())
        .collect()
}

const NOISE_DIRS: &[&str] = &[
    "node_modules", "target", "build", "dist", "out", "coverage", "DerivedData", "__pycache__",
    "venv", "site-packages", "Pods", "vendor", "worktrees",
];
const NOISE_BUNDLE_SUFFIXES: &[&str] = &[".app", ".bundle", ".framework", ".photoslibrary", ".noindex", ".xcassets"];
const LIBRARY_KEEP: &[&str] = &["Library/Mobile Documents/com~apple~CloudDocs", "Library/CloudStorage"];

/// Hidden folders, app internals, build output and `~/Library` (except iCloud Drive and cloud storage).
fn is_noise(path: &str, home: &str) -> bool {
    let rel = path.strip_prefix(home).map(|r| r.trim_start_matches('/')).unwrap_or(path);
    if rel.starts_with("Library/") && !LIBRARY_KEEP.iter().any(|k| rel.starts_with(k)) {
        return true;
    }
    let mut dirs = rel.split('/').collect::<Vec<_>>();
    let name = dirs.pop().unwrap_or_default();
    name.starts_with('.')
        || dirs.iter().any(|d| {
            d.starts_with('.') || NOISE_DIRS.contains(d) || NOISE_BUNDLE_SUFFIXES.iter().any(|s| d.ends_with(s))
        })
}

/// fsearch forgives typos, so "falko" also hits "Fallback". When some names contain every
/// typed word, drop the ones that don't; when none do, the user made the typo — keep them all.
fn drop_lookalikes(hits: &mut Vec<Hit>, pattern: &str) {
    let words: Vec<String> = pattern
        .split_whitespace()
        .filter(|w| !w.contains(':') && !w.starts_with('!'))
        .map(|w| w.trim_matches(|c| c == '^' || c == '$' || c == '\'').to_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return;
    }
    let contains_all = |h: &Hit| {
        let name = h.path.rsplit('/').next().unwrap_or(&h.path).to_lowercase();
        words.iter().all(|w| name.contains(w.as_str()))
    };
    if hits.iter().any(contains_all) {
        hits.retain(contains_all);
    }
}

/// Where macOS is currently saving screenshots. Reads
/// `defaults read com.apple.screencapture location`; if that's unset, falls
/// back to `~/Desktop` (the historical default) and `~/Screenshots` (Sonoma+
/// default for some installs). Only existing directories are returned.
pub fn default_screenshot_dirs() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    let mut cmd = std::process::Command::new("defaults");
    cmd.args(["read", "com.apple.screencapture", "location"]);
    if let Some(stdout) = crate::proc::capture_stdout(cmd, std::time::Duration::from_secs(3)) {
        let s = String::from_utf8_lossy(&stdout).trim().to_string();
        if !s.is_empty() {
            out.push(expand_tilde(&s));
        }
    }

    if let Some(home) = dirs::home_dir() {
        for sub in ["Screenshots", "Desktop"] {
            let p = home.join(sub).to_string_lossy().to_string();
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }

    out.into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect()
}

/// Expand a leading `~/` or bare `~` into the home directory. Idempotent
/// for already-absolute paths.
pub fn expand_tilde(p: &str) -> String {
    if p == "~" {
        return dirs::home_dir()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|| p.to_string());
    }
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    }
    p.to_string()
}

pub fn search(mut opts: FileSearchOpts) -> Vec<FileEntry> {
    let browsing_recent = opts.query.as_deref().map_or(true, |q| q.trim().is_empty()) && opts.sort_by_mtime;
    if opts.roots.is_empty() {
        opts.roots = if browsing_recent { recent_roots() } else { default_roots() };
    }
    let extensions = expand_extensions(&opts.extensions, opts.category.as_deref());
    let limit = opts.limit.unwrap_or(DEFAULT_LIMIT);

    let mut hits: Vec<Hit> = Vec::new();
    let pattern = opts
        .query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(".");

    std::thread::scope(|scope| {
        let workers: Vec<_> = opts
            .roots
            .iter()
            .filter(|root| Path::new(root).exists())
            .map(|root| {
                let extensions = &extensions;
                let kind = if opts.folders { "dir" } else { "file" };
                scope.spawn(move || {
                    if opts.sort_by_mtime {
                        newest_paths(kind, pattern, root, extensions, limit)
                    } else {
                        fsearch_paths(kind, pattern, root, extensions, None, FSEARCH_HITS)
                    }
                })
            })
            .collect();
        for w in workers {
            hits.extend(w.join().ok().flatten().unwrap_or_default());
        }
    });
    drop_lookalikes(&mut hits, pattern);
    if opts.sort_by_mtime {
        hits.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    } else {
        hits.sort_by(|a, b| b.score.total_cmp(&a.score).then(b.mtime.cmp(&a.mtime)));
    }

    hits.into_iter()
        .filter_map(|h| decorate(Path::new(&h.path), opts.folders))
        .take(limit)
        .collect()
}

struct Hit {
    path: String,
    score: f64,
    mtime: u64,
}

const FSEARCH_HITS: usize = 5000;
const NEWEST_WINDOWS: &[&str] = &["1h", "6h", "1d", "7d", "30d", "365d"];

fn fsearch_paths(kind: &str, pattern: &str, root: &str, extensions: &[String], window: Option<&str>, hits: usize) -> Option<Vec<Hit>> {
    if root.contains('"') {
        return None;
    }
    let mut query = format!("kind:{kind} in:\"{root}\" limit:{hits}");
    if !extensions.is_empty() {
        query.push_str(&format!(" ext:{}", extensions.join(",")));
    }
    if let Some(w) = window {
        query.push_str(&format!(" mtime:<{w}"));
    }
    if pattern != "." {
        query.push(' ');
        query.push_str(pattern);
    }
    let mut cmd = std::process::Command::new(dirs::home_dir()?.join(".local/bin/fsearch"));
    cmd.arg(query).arg("--json");
    let out = crate::proc::capture_stdout(cmd, std::time::Duration::from_secs(6))?;
    let json: serde_json::Value = serde_json::from_slice(&out).ok()?;
    let hits = json["hits"].as_array()?;
    let home = dirs::home_dir()?.to_string_lossy().to_string();
    Some(
        hits.iter()
            .filter_map(|h| {
                let path = h["path"].as_str()?;
                (!is_noise(path, &home)).then(|| Hit {
                    path: path.to_string(),
                    score: h["score"].as_f64().unwrap_or(0.0),
                    mtime: h["mtime"].as_u64().unwrap_or(0),
                })
            })
            .collect(),
    )
}

/// fsearch has no sort-by-mtime, so widen the time window until it holds `limit` files.
fn newest_paths(kind: &str, pattern: &str, root: &str, extensions: &[String], limit: usize) -> Option<Vec<Hit>> {
    for window in NEWEST_WINDOWS {
        let paths = fsearch_paths(kind, pattern, root, extensions, Some(window), FSEARCH_HITS)?;
        if paths.len() >= limit {
            return Some(paths);
        }
    }
    fsearch_paths(kind, pattern, root, extensions, None, FSEARCH_HITS)
}

fn decorate(p: &Path, folders: bool) -> Option<FileEntry> {
    let meta = p.metadata().ok()?;
    if !(if folders { meta.is_dir() } else { meta.is_file() }) {
        return None;
    }
    let modified_at = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let name = p.file_name().and_then(|s| s.to_str())?.to_string();
    let parent = p
        .parent()
        .map(|x| home_relative(&x.to_string_lossy()))
        .unwrap_or_default();
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    let is_image = IMAGE_EXTS.contains(&ext.as_str());
    Some(FileEntry {
        path: p.to_string_lossy().into_owned(),
        name,
        parent,
        size: meta.len(),
        modified_at,
        is_image,
        is_dir: meta.is_dir(),
        ext,
    })
}

fn home_relative(p: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let h = home.to_string_lossy().to_string();
        if let Some(rest) = p.strip_prefix(&format!("{h}/Library/Mobile Documents/com~apple~CloudDocs")) {
            return format!("iCloud Drive{rest}");
        }
        if let Some(rest) = p.strip_prefix(&h) {
            return format!("~{rest}");
        }
    }
    p.to_string()
}

fn expand_extensions(exts: &[String], category: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = exts.iter().map(|s| s.to_lowercase()).collect();
    let mut add = |list: &[&str]| {
        for &e in list {
            if !out.iter().any(|x| x == e) {
                out.push(e.to_string());
            }
        }
    };
    match category {
        Some("image") | Some("img") => add(IMAGE_EXTS),
        Some("screenshot") => {
            add(IMAGE_EXTS);
            add(SCREENSHOT_VIDEO_EXTS);
        }
        _ => {}
    }
    out
}

// ---------- Per-row actions ----------

pub fn open_in_default_app(path: &str) -> Result<(), String> {
    let mut cmd = std::process::Command::new("open");
    cmd.arg(path);
    crate::proc::output_with_timeout(cmd, std::time::Duration::from_secs(5))
        .map_err(|e| format!("open: {e}"))?;
    Ok(())
}

pub fn reveal_in_finder(path: &str) -> Result<(), String> {
    let mut cmd = std::process::Command::new("open");
    cmd.args(["-R", path]);
    crate::proc::output_with_timeout(cmd, std::time::Duration::from_secs(5))
        .map_err(|e| format!("open -R: {e}"))?;
    Ok(())
}

/// Copy the image content (not the path) to the macOS clipboard. The user
/// can then paste it directly into Slack, Notes, an email, etc.
///
/// Implemented via AppleScript's clipboard data classes — `«class PNGf»`
/// for PNG, `JPEG` for JPEG. Unsupported formats fall back to copying the
/// path so the action never silently no-ops.
pub fn copy_image_to_clipboard(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(format!("file not found: {path}"));
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    let class = match ext.as_str() {
        "png" => "PNGf",
        "jpg" | "jpeg" => "JPEG",
        _ => return copy_path_to_clipboard(path),
    };
    // AppleScript string escape — only " and \ matter inside a quoted run.
    let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "set the clipboard to (read (POSIX file \"{p}\") as «class {c}»)",
        p = escaped,
        c = class
    );
    let mut cmd = std::process::Command::new("osascript");
    cmd.arg("-e").arg(&script);
    let out = crate::proc::output_with_timeout(cmd, std::time::Duration::from_secs(8))?;
    if !out.status.success() {
        return Err("osascript returned non-zero".into());
    }
    Ok(())
}

pub fn copy_path_to_clipboard(path: &str) -> Result<(), String> {
    use std::io::Write;
    let mut child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("pbcopy spawn: {e}"))?;
    // Write then drop stdin so pbcopy sees EOF. Stash any write error but
    // still reap the child below — returning early here would leak a zombie.
    let write_err = match child.stdin.take() {
        Some(mut stdin) => stdin.write_all(path.as_bytes()).err(),
        None => None,
    };
    let status = child.wait().map_err(|e| format!("pbcopy wait: {e}"))?;
    if let Some(e) = write_err {
        return Err(format!("pbcopy write: {e}"));
    }
    if !status.success() {
        return Err("pbcopy failed".into());
    }
    Ok(())
}

/// Read an image and return a base64 data URL the webview can render
/// directly. Big images are skipped — over 5 MB we'd rather show no
/// thumbnail than freeze the palette.
pub fn thumbnail_data_url(path: &str) -> Option<String> {
    let p = Path::new(path);
    let meta = std::fs::metadata(p).ok()?;
    if !meta.is_file() || meta.len() > 5_000_000 {
        return None;
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())?;
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        _ => return None,
    };
    let bytes = std::fs::read(p).ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(format!("data:{};base64,{}", mime, b64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_image_category() {
        let out = expand_extensions(&[], Some("image"));
        assert!(out.contains(&"png".to_string()));
        assert!(out.contains(&"jpg".to_string()));
    }

    #[test]
    fn expand_image_category_dedupes() {
        let out = expand_extensions(&["png".into()], Some("img"));
        let count = out.iter().filter(|x| x.as_str() == "png").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn home_relative_strips_home() {
        if let Some(home) = dirs::home_dir() {
            let p = home.join("Desktop").to_string_lossy().to_string();
            assert!(home_relative(&p).starts_with("~/"));
        }
    }

    #[test]
    fn expand_screenshot_includes_video() {
        let out = expand_extensions(&[], Some("screenshot"));
        assert!(out.contains(&"png".to_string()));
        assert!(out.contains(&"mov".to_string()));
    }

    #[test]
    fn expand_tilde_resolves_home() {
        if let Some(home) = dirs::home_dir() {
            let h = home.to_string_lossy().to_string();
            assert_eq!(expand_tilde("~"), h);
            assert_eq!(expand_tilde("~/foo"), home.join("foo").to_string_lossy());
            assert_eq!(expand_tilde("/abs/path"), "/abs/path");
        }
    }
}

