//! Git-backed backup of the smallcast store directory.
//!
//! The working tree IS the data dir (`~/Library/Application Support/smallcast/`),
//! and the .git lives at `<data_dir>/.backup-git/` so it stays out of sight.
//! All operations shell out to the system `git` CLI, so whatever auth lets
//! the user `git push` from their terminal — SSH key, gh CLI, credential
//! helper — Just Works without smallcast managing credentials.
//!
//! Conflicts: the happy path is `pull --rebase` then push. If pull fails
//! (genuine conflict, or "remote has unrelated history"), we abort the
//! rebase and surface the error; the user resolves manually with their
//! own tooling, or hits **Force Push** to overwrite remote.
//!
//! Privacy: `.gitignore` is regenerated on every sync from the current
//! `include_analytics` flag, so toggling the setting takes effect on the
//! next push.

use crate::store::Store;
use serde::Serialize;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Duration;

const GIT_DIR_NAME: &str = ".backup-git";

/// Network-facing git calls (push/pull/ls-remote) can stall on an unreachable
/// remote. Bound them so a backup attempt can never wedge the caller. Local
/// ops (add/commit/status) are left on the plain `.output()` path — they run
/// against a tiny on-disk repo and don't block.
const GIT_NET_TIMEOUT: Duration = Duration::from_secs(30);

/// Run a network git command under a timeout, mapping failures to `what`.
fn run_net(cmd: Command, what: &str) -> Result<Output, String> {
    crate::proc::output_with_timeout(cmd, GIT_NET_TIMEOUT).map_err(|e| format!("{what}: {e}"))
}

#[derive(Serialize)]
pub struct BackupStatus {
    pub initialized: bool,
    pub remote: Option<String>,
    pub branch: Option<String>,
    /// Number of files in `git status --porcelain` (untracked + modified
    /// + staged), excluding the .gitignore patterns.
    pub dirty_count: usize,
    pub ahead: usize,
    pub behind: usize,
    pub last_synced_ms: Option<u64>,
    pub last_error: Option<String>,
}

fn root() -> Result<PathBuf, String> {
    Store::root_dir().map_err(|e| e.to_string())
}

fn git_dir() -> Result<PathBuf, String> {
    Ok(root()?.join(GIT_DIR_NAME))
}

/// Build a `git` command pre-configured with our git-dir + work-tree.
/// Anything calling this gets the same isolated repo, regardless of the
/// process's cwd.
fn git() -> Result<Command, String> {
    let mut cmd = Command::new("git");
    cmd.arg("--git-dir").arg(git_dir()?);
    cmd.arg("--work-tree").arg(root()?);
    // Never block waiting for a username/password on a terminal that doesn't
    // exist — fail fast instead. Combined with the stdin=/dev/null and the
    // 30s deadline in `run_net`, a push/pull can't hang the app forever.
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    Ok(cmd)
}

fn ok(out: Output, what: &str) -> Result<String, String> {
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("{what}: {stderr}"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn is_initialized() -> bool {
    git_dir().map(|p| p.exists()).unwrap_or(false)
}

/// Regenerate the .gitignore based on the current privacy setting. Called
/// on every sync so toggling the analytics flag is one-click effective.
pub fn write_gitignore(include_analytics: bool) -> Result<(), String> {
    let path = root()?.join(".gitignore");
    let mut content = String::from(
        "# Managed by smallcast — do not edit.\n\
         .backup-git/\n\
         apps_cache.json\n\
         icons/\n\
         *.tmp\n",
    );
    if !include_analytics {
        content.push_str("analytics.jsonl\n");
    }
    std::fs::write(&path, content).map_err(|e| format!("write .gitignore: {e}"))
}

/// Initialize the repo if needed, set the remote, write .gitignore, and
/// push the initial commit. Fails clearly if the remote already has
/// content on the target branch — the user picks Force Push or sets up
/// against an empty repo.
pub fn init(remote: &str, branch: &str, include_analytics: bool) -> Result<(), String> {
    if remote.trim().is_empty() {
        return Err("Remote URL is required".into());
    }

    if !is_initialized() {
        let out = Command::new("git")
            .args(["init", "--initial-branch", branch, "--separate-git-dir"])
            .arg(git_dir()?)
            .arg(root()?)
            // Match the `git()` helper — never block on a credential prompt.
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|e| format!("git init: {e}"))?;
        ok(out, "git init")?;
    }

    write_gitignore(include_analytics)?;

    // Replace any existing remote so re-running init with a different
    // URL just works.
    let _ = git()?.args(["remote", "remove", "origin"]).output();
    let out = git()?
        .args(["remote", "add", "origin", remote])
        .output()
        .map_err(|e| format!("git remote add: {e}"))?;
    ok(out, "git remote add")?;

    // Bail early if the remote already has something on this branch —
    // we'd rather fail loud than silently merge unrelated histories.
    let mut cmd = git()?;
    cmd.args(["ls-remote", "--heads", "origin", branch]);
    let out = run_net(cmd, "git ls-remote")?;
    let remote_has_branch = ok(out, "git ls-remote")?
        .lines()
        .any(|l| !l.trim().is_empty());
    if remote_has_branch {
        return Err(format!(
            "Remote already has commits on `{branch}`. Either start from \
             an empty repo, or click Force Push to overwrite the remote \
             with your local store."
        ));
    }

    // Stage + commit + push. -c flags so we don't depend on the user's
    // global git identity (the app may be running for someone who's
    // never `git config --global`'d).
    let out = git()?
        .args(["add", "."])
        .output()
        .map_err(|e| format!("git add: {e}"))?;
    ok(out, "git add")?;

    let out = git()?
        .args([
            "-c",
            "user.name=smallcast",
            "-c",
            "user.email=smallcast@local",
            "commit",
            "-m",
            "smallcast: initial backup",
            "--allow-empty",
        ])
        .output()
        .map_err(|e| format!("git commit: {e}"))?;
    ok(out, "git commit")?;

    let mut cmd = git()?;
    cmd.args(["push", "-u", "origin", branch]);
    let out = run_net(cmd, "git push")?;
    ok(out, "git push")?;

    Ok(())
}

/// Stage everything, commit if there are changes, pull --rebase, push.
/// Returns Ok on a clean sync (including the no-op case where nothing
/// changed and we're already up to date).
pub fn sync(branch: &str, include_analytics: bool) -> Result<(), String> {
    if !is_initialized() {
        return Err("Backup is not initialized — set up a remote in Preferences first.".into());
    }
    write_gitignore(include_analytics)?;

    let out = git()?
        .args(["add", "."])
        .output()
        .map_err(|e| format!("git add: {e}"))?;
    ok(out, "git add")?;

    // git diff --cached --quiet exits 1 if there are staged changes.
    let out = git()?
        .args(["diff", "--cached", "--quiet"])
        .output()
        .map_err(|e| format!("git diff: {e}"))?;
    if !out.status.success() {
        let msg = format!(
            "smallcast: backup {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M")
        );
        let out = git()?
            .args([
                "-c",
                "user.name=smallcast",
                "-c",
                "user.email=smallcast@local",
                "commit",
                "-m",
                &msg,
            ])
            .output()
            .map_err(|e| format!("git commit: {e}"))?;
        ok(out, "git commit")?;
    }

    let mut cmd = git()?;
    cmd.args(["pull", "--rebase", "origin", branch]);
    let out = run_net(cmd, "git pull")?;
    if !out.status.success() {
        // Don't leave the repo in a half-rebased state.
        let _ = git()?.args(["rebase", "--abort"]).output();
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "Pull/rebase failed — resolve manually with `git` in {}, or click \
             Force Push to overwrite remote: {}",
            git_dir()?.display(),
            stderr.trim()
        ));
    }

    let mut cmd = git()?;
    cmd.args(["push", "origin", branch]);
    let out = run_net(cmd, "git push")?;
    ok(out, "git push")?;

    Ok(())
}

pub fn pull(branch: &str) -> Result<(), String> {
    if !is_initialized() {
        return Err("Backup is not initialized.".into());
    }
    let mut cmd = git()?;
    cmd.args(["pull", "--rebase", "origin", branch]);
    let out = run_net(cmd, "git pull")?;
    if !out.status.success() {
        let _ = git()?.args(["rebase", "--abort"]).output();
        return Err(format!(
            "Pull failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// Push --force-with-lease — destructive enough that the UI confirms
/// first and surfaces "this overwrites remote".
pub fn force_push(branch: &str) -> Result<(), String> {
    if !is_initialized() {
        return Err("Backup is not initialized.".into());
    }
    let mut cmd = git()?;
    cmd.args(["push", "--force-with-lease", "origin", branch]);
    let out = run_net(cmd, "git push --force")?;
    ok(out, "git push --force-with-lease")?;
    Ok(())
}

pub fn status(
    branch: &str,
    last_synced_ms: Option<u64>,
    last_error: Option<String>,
) -> BackupStatus {
    if !is_initialized() {
        return BackupStatus {
            initialized: false,
            remote: None,
            branch: None,
            dirty_count: 0,
            ahead: 0,
            behind: 0,
            last_synced_ms: None,
            last_error,
        };
    }

    let remote = git().ok().and_then(|mut c| {
        let out = c.args(["remote", "get-url", "origin"]).output().ok()?;
        if out.status.success() {
            Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            None
        }
    });

    let dirty_count = git()
        .ok()
        .and_then(|mut c| {
            let out = c.args(["status", "--porcelain"]).output().ok()?;
            Some(
                String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .filter(|l| !l.is_empty())
                    .count(),
            )
        })
        .unwrap_or(0);

    // `git rev-list --left-right --count A...B` prints "<left> <right>".
    // With A=origin/<branch> and B=HEAD, left = behind, right = ahead.
    let (ahead, behind) = git()
        .ok()
        .and_then(|mut c| {
            let out = c
                .args([
                    "rev-list",
                    "--left-right",
                    "--count",
                    &format!("origin/{branch}...HEAD"),
                ])
                .output()
                .ok()?;
            if !out.status.success() {
                return None;
            }
            let s = String::from_utf8_lossy(&out.stdout);
            let mut parts = s.trim().split_whitespace();
            let behind: usize = parts.next()?.parse().ok()?;
            let ahead: usize = parts.next()?.parse().ok()?;
            Some((ahead, behind))
        })
        .unwrap_or((0, 0));

    BackupStatus {
        initialized: true,
        remote,
        branch: Some(branch.to_string()),
        dirty_count,
        ahead,
        behind,
        last_synced_ms,
        last_error,
    }
}

/// Path to the .git dir — surfaced so the UI can offer "Open in Finder"
/// for users who want to inspect the repo manually.
pub fn git_dir_path() -> Option<String> {
    git_dir().ok().map(|p| p.to_string_lossy().into_owned())
}
