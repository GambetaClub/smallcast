use base64::{engine::general_purpose::STANDARD, Engine};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static MEM_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

pub fn get_app_icon(app_path: &str) -> Option<String> {
    let cache = MEM_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().get(app_path) {
        return hit.clone();
    }
    let result = generate(app_path);
    cache.lock().insert(app_path.to_string(), result.clone());
    result
}

fn generate(app_path: &str) -> Option<String> {
    let png = ensure_png(app_path)?;
    let bytes = std::fs::read(&png).ok()?;
    Some(format!("data:image/png;base64,{}", STANDARD.encode(&bytes)))
}

fn ensure_png(app_path: &str) -> Option<PathBuf> {
    let dir = cache_dir()?;
    std::fs::create_dir_all(&dir).ok();
    let cached = dir.join(format!("{}.png", hash_path(app_path)));
    if cached.exists() {
        return Some(cached);
    }
    let icns = find_icns(app_path)?;
    let mut cmd = Command::new("sips");
    cmd.args(["-s", "format", "png"])
        .arg(&icns)
        .arg("--out")
        .arg(&cached)
        .args(["-z", "64", "64"]);
    // sips on a malformed/huge .icns can stall — bound it so the apps scan
    // never wedges on one bad bundle.
    let out = crate::proc::output_with_timeout(cmd, std::time::Duration::from_secs(5)).ok()?;
    if !out.status.success() || !cached.exists() {
        return None;
    }
    Some(cached)
}

fn find_icns(app_path: &str) -> Option<PathBuf> {
    let info_plist = Path::new(app_path).join("Contents/Info.plist");
    let resources = Path::new(app_path).join("Contents/Resources");

    let mut pb = Command::new("/usr/libexec/PlistBuddy");
    pb.args(["-c", "Print :CFBundleIconFile"]).arg(&info_plist);
    if let Ok(out) = crate::proc::output_with_timeout(pb, std::time::Duration::from_secs(5)) {
        if out.status.success() {
            let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !raw.is_empty() {
                let with_ext = if raw.ends_with(".icns") {
                    raw.clone()
                } else {
                    format!("{raw}.icns")
                };
                let p = resources.join(&with_ext);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    // Fallback: pick the largest .icns in Resources/.
    let mut best: Option<(u64, PathBuf)> = None;
    if let Ok(read) = std::fs::read_dir(&resources) {
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("icns") {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if best.as_ref().map_or(true, |(s, _)| size > *s) {
                best = Some((size, path));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("smallcast/icons"))
}

// FNV-1a 64-bit — non-crypto, just a stable cache key per path.
fn hash_path(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}
