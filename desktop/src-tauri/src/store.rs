use crate::types::*;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Store {
    pub root: PathBuf,
    pub config: Config,
}

impl Store {
    pub fn load() -> Result<Self, StoreError> {
        let root = Self::root_dir()?;
        std::fs::create_dir_all(&root)?;
        let config_path = root.join("config.json");
        let config = if config_path.exists() {
            match serde_json::from_str(&std::fs::read_to_string(&config_path)?) {
                Ok(c) => c,
                Err(_) => {
                    // A corrupt config.json (partial write after a crash, a bad
                    // manual edit, or a conflicted backup merge) must NOT stop
                    // the menu-bar app from launching. Preserve the bad file for
                    // debugging and fall back to defaults so the app still opens.
                    let _ = std::fs::rename(&config_path, root.join("config.json.corrupt"));
                    let c = Config::default();
                    Self::ensure_workspace_dir(&root, &c.active_workspace)?;
                    write_json(&config_path, &c)?;
                    c
                }
            }
        } else {
            let c = Config::default();
            Self::ensure_workspace_dir(&root, &c.active_workspace)?;
            write_json(&config_path, &c)?;
            c
        };
        Self::ensure_workspace_dir(&root, &config.active_workspace)?;
        Ok(Self { root, config })
    }

    pub fn root_dir() -> Result<PathBuf, StoreError> {
        // macOS: ~/Library/Application Support/davidcast
        // Linux: ~/.local/share/davidcast
        // Windows: %APPDATA%\davidcast
        let base = dirs::data_dir().ok_or(StoreError::NoDataDir)?;
        Ok(base.join("davidcast"))
    }

    pub fn ensure_workspace_dir(root: &Path, id: &str) -> Result<(), StoreError> {
        std::fs::create_dir_all(root.join("workspaces").join(id))?;
        Ok(())
    }

    pub fn workspace_dir(&self) -> PathBuf {
        self.root.join("workspaces").join(&self.config.active_workspace)
    }

    pub fn save_config(&self) -> Result<(), StoreError> {
        write_json(&self.root.join("config.json"), &self.config)
    }

    pub fn load_snippets(&self) -> Result<Vec<Snippet>, StoreError> {
        read_json_or_default(&self.workspace_dir().join("snippets.json"))
    }

    pub fn save_snippets(&self, v: &[Snippet]) -> Result<(), StoreError> {
        write_json(&self.workspace_dir().join("snippets.json"), &v.to_vec())
    }

    pub fn load_quicklinks(&self) -> Result<Vec<Quicklink>, StoreError> {
        read_json_or_default(&self.workspace_dir().join("quicklinks.json"))
    }

    pub fn save_quicklinks(&self, v: &[Quicklink]) -> Result<(), StoreError> {
        write_json(&self.workspace_dir().join("quicklinks.json"), &v.to_vec())
    }
}

fn read_json_or_default<T>(path: &Path) -> Result<T, StoreError>
where
    T: serde::de::DeserializeOwned + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn write_json<T: Serialize>(path: &Path, v: &T) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(v)?;
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no data directory")]
    NoDataDir,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Snippet, Quicklink, OpenIn};

    fn stub_snippet(id: &str) -> Snippet {
        Snippet {
            id: id.into(),
            name: "Test".into(),
            keyword: None,
            text: "body".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            deleted: false,
            rev: 1,
            sensitive: false,
        }
    }

    fn stub_quicklink(id: &str) -> Quicklink {
        Quicklink {
            id: id.into(),
            name: "Link".into(),
            keyword: None,
            url: "https://example.com".into(),
            open_in: OpenIn::DefaultBrowser,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            deleted: false,
            rev: 1,
        }
    }

    // ---------- write_json / read_json_or_default ----------

    #[test]
    fn write_and_read_json_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("items.json");
        let items = vec![stub_snippet("a"), stub_snippet("b")];
        write_json(&path, &items).unwrap();
        let back: Vec<Snippet> = read_json_or_default(&path).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].id, "a");
        assert_eq!(back[1].id, "b");
    }

    #[test]
    fn read_json_or_default_returns_empty_vec_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let back: Vec<Snippet> = read_json_or_default(&path).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn write_json_is_atomic_temp_then_rename() {
        // After write_json the .tmp file must not exist alongside the real file.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        write_json(&path, &42u32).unwrap();
        assert!(path.exists());
        let tmp = path.with_extension("json.tmp");
        assert!(!tmp.exists(), "tmp file should have been renamed away");
    }

    // ---------- Store helper paths ----------

    #[test]
    fn ensure_workspace_dir_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        Store::ensure_workspace_dir(&root, "myws").unwrap();
        assert!(root.join("workspaces").join("myws").exists());
    }

    #[test]
    fn store_snippets_and_quicklinks_persist_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        // Build a minimal Config pointing at our temp root.
        let config = crate::types::Config {
            active_workspace: "ws1".into(),
            workspaces: vec![crate::types::Workspace {
                id: "ws1".into(),
                name: "WS1".into(),
                color: None,
            }],
            show_vite_inline: true,
            show_docker_inline: true,
            show_snippets_inline: true,
            show_quicklinks_inline: true,
            screenshot_dirs: vec![],
            github_repos: vec![],
            check_updates_on_launch: false,
            backup: crate::types::BackupConfig::default(),
            enable_recommendations: false,
        };
        Store::ensure_workspace_dir(&root, "ws1").unwrap();
        let store = Store { root, config };

        // Snippets
        let snips = vec![stub_snippet("s1")];
        store.save_snippets(&snips).unwrap();
        let loaded = store.load_snippets().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "s1");

        // Quicklinks
        let qls = vec![stub_quicklink("q1")];
        store.save_quicklinks(&qls).unwrap();
        let loaded = store.load_quicklinks().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "q1");
    }
}
