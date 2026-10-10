use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
    pub text: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub rev: u64,
    /// When true the value is treated as a secret: never shown in the
    /// palette row subtitle, and the edit form starts with the
    /// textarea masked. Default false (existing snippets unaffected).
    #[serde(default)]
    pub sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quicklink {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
    pub url: String,
    #[serde(default)]
    pub open_in: OpenIn,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub rev: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenIn {
    #[default]
    DefaultBrowser,
    Chrome,
    Safari,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub active_workspace: String,
    pub workspaces: Vec<Workspace>,
    /// Whether running Vite dev servers appear inline in the unfiltered
    /// palette list. The "Show Vite Ports" command always finds them
    /// regardless of this flag.
    #[serde(default = "default_true")]
    pub show_vite_inline: bool,
    /// Same idea for Docker containers.
    #[serde(default = "default_true")]
    pub show_docker_inline: bool,
    /// Whether the user's snippets appear inline in the unfiltered
    /// palette list. When off, they're only reachable via the
    /// "Search Snippets" command. Defaults on.
    #[serde(default = "default_true")]
    pub show_snippets_inline: bool,
    /// Same idea for quicklinks.
    #[serde(default = "default_true")]
    pub show_quicklinks_inline: bool,
    /// Folders the "Find Screenshots" command scans, newest-mtime first.
    /// `~/Desktop` is the macOS default; `~/Pictures/Screenshots` is the
    /// most common redirect target. Stored as resolved absolute paths.
    #[serde(default = "default_screenshot_dirs")]
    pub screenshot_dirs: Vec<String>,
    /// Repositories the GitHub plugin tracks for PR / issue listings.
    /// Format: "owner/repo". Empty by default — the user adds repos in
    /// Preferences. Auth piggybacks on `gh auth login`.
    #[serde(default)]
    pub github_repos: Vec<String>,
    /// Whether to ping the GitHub Releases updater endpoint at startup.
    /// Manual "Check for Updates" still works either way.
    #[serde(default = "default_true")]
    pub check_updates_on_launch: bool,
    /// Git-based backup of the store directory. Disabled by default —
    /// the user wires up a remote in Preferences.
    #[serde(default)]
    pub backup: BackupConfig,
    /// On-device recommender that learns from `analytics.jsonl` and
    /// re-ranks the empty-query palette + surfaces a "Recommended for
    /// you" section. Off by default — the model only matters once the
    /// user has built up some usage history.
    #[serde(default)]
    pub enable_recommendations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Master switch. When false, no auto-sync timer fires; manual
    /// "Sync now" still works as long as the repo is initialized.
    #[serde(default)]
    pub enabled: bool,
    /// Git remote URL — anything `git` can push to. Empty until the
    /// user runs Backup → Connect.
    #[serde(default)]
    pub remote: String,
    /// Branch to push to. Defaults to `main`.
    #[serde(default = "default_branch")]
    pub branch: String,
    /// Whether `analytics.jsonl` is included in the backup. Off by
    /// default for privacy — the log contains every query you've typed.
    #[serde(default)]
    pub include_analytics: bool,
    /// Unix milliseconds of the last successful push, if any.
    #[serde(default)]
    pub last_synced_ms: Option<u64>,
    /// Stderr from the last failed sync, if any. Cleared on success.
    #[serde(default)]
    pub last_error: Option<String>,
    /// Minutes between background auto-sync attempts. The timer only
    /// pushes if the repo is dirty AND this much time has elapsed.
    #[serde(default = "default_auto_interval")]
    pub auto_interval_min: u32,
}

fn default_branch() -> String {
    "main".to_string()
}

fn default_auto_interval() -> u32 {
    10
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            remote: String::new(),
            branch: default_branch(),
            include_analytics: false,
            last_synced_ms: None,
            last_error: None,
            auto_interval_min: default_auto_interval(),
        }
    }
}

fn default_screenshot_dirs() -> Vec<String> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let mut out = vec![home.join("Desktop").to_string_lossy().into_owned()];
    let pics = home.join("Pictures").join("Screenshots");
    if pics.exists() {
        out.push(pics.to_string_lossy().into_owned());
    }
    out
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            active_workspace: "personal".to_string(),
            workspaces: vec![Workspace {
                id: "personal".to_string(),
                name: "Personal".to_string(),
                color: Some("#7bd88f".to_string()),
            }],
            show_vite_inline: true,
            show_docker_inline: true,
            show_snippets_inline: true,
            show_quicklinks_inline: true,
            screenshot_dirs: default_screenshot_dirs(),
            github_repos: Vec::new(),
            check_updates_on_launch: true,
            backup: BackupConfig::default(),
            enable_recommendations: false,
        }
    }
}

// Wire-format for the frontend: a flat list of items across snippets and quicklinks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Item {
    Snippet(Snippet),
    Quicklink(Quicklink),
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- OpenIn ----------

    #[test]
    fn open_in_default_is_default_browser() {
        let v: OpenIn = Default::default();
        assert_eq!(v, OpenIn::DefaultBrowser);
    }

    #[test]
    fn open_in_serde_roundtrip() {
        for variant in [OpenIn::DefaultBrowser, OpenIn::Chrome, OpenIn::Safari] {
            let json = serde_json::to_string(&variant).unwrap();
            let back: OpenIn = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    // ---------- Snippet ----------

    fn stub_snippet() -> Snippet {
        Snippet {
            id: "s1".into(),
            name: "My Snippet".into(),
            keyword: Some("ms".into()),
            text: "hello world".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            deleted: false,
            rev: 1,
            sensitive: false,
        }
    }

    #[test]
    fn snippet_serde_roundtrip() {
        let s = stub_snippet();
        let json = serde_json::to_string(&s).unwrap();
        let back: Snippet = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, s.id);
        assert_eq!(back.name, s.name);
        assert_eq!(back.keyword, s.keyword);
        assert_eq!(back.text, s.text);
        assert_eq!(back.deleted, s.deleted);
        assert_eq!(back.rev, s.rev);
        assert_eq!(back.sensitive, s.sensitive);
    }

    #[test]
    fn snippet_sensitive_defaults_false_on_deserialize() {
        // JSON without the `sensitive` field should deserialize with sensitive=false.
        let json = r#"{"id":"x","name":"n","text":"t","created_at":"","updated_at":"","rev":1,"deleted":false}"#;
        let s: Snippet = serde_json::from_str(json).unwrap();
        assert!(!s.sensitive);
    }

    #[test]
    fn snippet_keyword_skipped_when_none() {
        // keyword is None → should not appear in serialized JSON.
        let mut s = stub_snippet();
        s.keyword = None;
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("keyword"));
    }

    // ---------- Quicklink ----------

    fn stub_quicklink() -> Quicklink {
        Quicklink {
            id: "q1".into(),
            name: "GitHub".into(),
            keyword: Some("gh".into()),
            url: "https://github.com".into(),
            open_in: OpenIn::DefaultBrowser,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            deleted: false,
            rev: 1,
        }
    }

    #[test]
    fn quicklink_serde_roundtrip() {
        let q = stub_quicklink();
        let json = serde_json::to_string(&q).unwrap();
        let back: Quicklink = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, q.id);
        assert_eq!(back.url, q.url);
        assert_eq!(back.open_in, q.open_in);
        assert_eq!(back.rev, q.rev);
    }

    #[test]
    fn quicklink_open_in_defaults_on_missing_field() {
        let json = r#"{"id":"x","name":"n","url":"https://example.com","created_at":"","updated_at":"","rev":1,"deleted":false}"#;
        let q: Quicklink = serde_json::from_str(json).unwrap();
        assert_eq!(q.open_in, OpenIn::DefaultBrowser);
    }

    // ---------- Config defaults ----------

    #[test]
    fn config_default_has_personal_workspace() {
        let c = Config::default();
        assert_eq!(c.active_workspace, "personal");
        assert_eq!(c.workspaces.len(), 1);
        assert_eq!(c.workspaces[0].id, "personal");
    }

    #[test]
    fn config_default_inline_flags_all_true() {
        let c = Config::default();
        assert!(c.show_vite_inline);
        assert!(c.show_docker_inline);
        assert!(c.show_snippets_inline);
        assert!(c.show_quicklinks_inline);
    }

    #[test]
    fn config_default_check_updates_true() {
        let c = Config::default();
        assert!(c.check_updates_on_launch);
    }

    // ---------- BackupConfig defaults ----------

    #[test]
    fn backup_config_default_disabled() {
        let b = BackupConfig::default();
        assert!(!b.enabled);
        assert_eq!(b.branch, "main");
        assert_eq!(b.auto_interval_min, 10);
        assert!(!b.include_analytics);
        assert!(b.remote.is_empty());
        assert!(b.last_synced_ms.is_none());
        assert!(b.last_error.is_none());
    }
}

