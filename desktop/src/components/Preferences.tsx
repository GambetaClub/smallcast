import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { Screen } from "./Screen";
import { api, type Settings } from "../api";
import { relativeTime } from "../utils";

type Props = {
  onClose: () => void;
  onError: (msg: string) => void;
};

export function Preferences({ onClose, onError }: Props) {
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  // Perf pill is a debug toggle, kept in localStorage rather than the
  // canonical Config so it stays per-machine.
  const [perfPillEnabled, setPerfPillEnabled] = useState<boolean>(() => {
    try {
      return localStorage.getItem("smallcast.perf_pill") !== "0";
    } catch {
      return true;
    }
  });

  const refresh = useCallback(async () => {
    try {
      const [as, s] = await Promise.all([
        isEnabled().catch(() => false),
        api.getSettings(),
      ]);
      setAutostart(as);
      setSettings(s);
    } catch (e) {
      onError(String(e));
    }
  }, [onError]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  async function toggleAutostart() {
    try {
      if (autostart) {
        await disable();
        setAutostart(false);
      } else {
        await enable();
        setAutostart(true);
      }
    } catch (e) {
      onError(`Autostart: ${e}`);
    }
  }

  // Generic toggle helper for the boolean inline-display flags. Optimistic
  // local update with revert-on-error so the switch never feels sticky.
  async function toggleSetting(
    key:
      | "show_vite_inline"
      | "show_docker_inline"
      | "show_snippets_inline"
      | "show_quicklinks_inline"
      | "check_updates_on_launch"
      | "enable_recommendations",
    setter: (v: boolean) => Promise<void>,
  ) {
    if (!settings) return;
    const next = !settings[key];
    setSettings({ ...settings, [key]: next });
    try {
      await setter(next);
    } catch (e) {
      onError(String(e));
      setSettings({ ...settings, [key]: !next });
    }
  }

  async function saveScreenshotDirs(value: string) {
    if (!settings) return;
    const dirs = value
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    setSettings({ ...settings, screenshot_dirs: dirs });
    try {
      await api.setScreenshotDirs(dirs);
    } catch (e) {
      onError(String(e));
    }
  }

  return (
    <Screen kind="prefs" title="Preferences" onClose={onClose}>
      <div className="prefs">
          <h2>General</h2>
          <section>
            <div className="prefs-row">
              <div className="label">
                <div className="label-title">Launch at login</div>
                <div className="label-sub">
                  Start smallcast automatically when you sign in.
                </div>
              </div>
              <div
                className={`switch ${autostart ? "on" : ""}`}
                onClick={toggleAutostart}
                role="switch"
                aria-checked={!!autostart}
              />
            </div>
            <div className="prefs-row">
              <div className="label">
                <div className="label-title">Global hotkey</div>
                <div className="label-sub">
                  Press <kbd>⌘ Space</kbd> anywhere to toggle the palette. Rebinding
                  lands in a later release.
                </div>
              </div>
            </div>
            <div className="prefs-row">
              <div className="label">
                <div className="label-title">Check for updates on launch</div>
                <div className="label-sub">
                  Pings the GitHub Releases endpoint each time smallcast
                  starts. You can also run{" "}
                  <code>app.check_updates</code> from the palette anytime.
                </div>
              </div>
              <div
                className={`switch ${settings?.check_updates_on_launch ? "on" : ""}`}
                onClick={() =>
                  toggleSetting(
                    "check_updates_on_launch",
                    api.setCheckUpdatesOnLaunch,
                  )
                }
                role="switch"
                aria-checked={!!settings?.check_updates_on_launch}
              />
            </div>
          </section>

          <h2>Search</h2>
          <section>
            <div className="prefs-row">
              <div className="label">
                <div className="label-title">Include snippets in search</div>
                <div className="label-sub">
                  When off, snippets only show under the <b>Search Snippets</b>{" "}
                  command. Useful if you have hundreds and they crowd the
                  unfiltered list.
                </div>
              </div>
              <div
                className={`switch ${settings?.show_snippets_inline ? "on" : ""}`}
                onClick={() =>
                  toggleSetting("show_snippets_inline", api.setShowSnippetsInline)
                }
                role="switch"
                aria-checked={!!settings?.show_snippets_inline}
              />
            </div>
            <div className="prefs-row">
              <div className="label">
                <div className="label-title">Include quicklinks in search</div>
                <div className="label-sub">
                  When off, quicklinks only show under the{" "}
                  <b>Search Quicklinks</b> command.
                </div>
              </div>
              <div
                className={`switch ${settings?.show_quicklinks_inline ? "on" : ""}`}
                onClick={() =>
                  toggleSetting("show_quicklinks_inline", api.setShowQuicklinksInline)
                }
                role="switch"
                aria-checked={!!settings?.show_quicklinks_inline}
              />
            </div>
          </section>

          <h2>Plugins</h2>
          <section>
            <div className="prefs-row">
              <div className="label">
                <div className="label-title">Show Vite ports inline</div>
                <div className="label-sub">
                  Running Vite dev servers appear in the main palette list. When
                  off, find them via the <b>Show Vite Ports</b> command.
                </div>
              </div>
              <div
                className={`switch ${settings?.show_vite_inline ? "on" : ""}`}
                onClick={() => toggleSetting("show_vite_inline", api.setShowViteInline)}
                role="switch"
                aria-checked={!!settings?.show_vite_inline}
              />
            </div>
            <div className="prefs-row">
              <div className="label">
                <div className="label-title">Show search performance pill</div>
                <div className="label-sub">
                  Tiny <code>in N · list N · count</code> overlay in the
                  top-right of the palette while typing. Useful for spotting
                  latency. Stored locally; ignored by sync.
                </div>
              </div>
              <div
                className={`switch ${perfPillEnabled ? "on" : ""}`}
                onClick={() => {
                  const next = !perfPillEnabled;
                  setPerfPillEnabled(next);
                  try {
                    localStorage.setItem(
                      "smallcast.perf_pill",
                      next ? "1" : "0"
                    );
                  } catch {
                    /* quota — ignore */
                  }
                }}
                role="switch"
                aria-checked={perfPillEnabled}
              />
            </div>
            <div className="prefs-row">
              <div className="label">
                <div className="label-title">Show Docker containers inline</div>
                <div className="label-sub">
                  Running containers appear in the main list with two rows each
                  (shell + logs). When off, reach them via the <b>Show Docker
                  Containers</b> command.
                </div>
              </div>
              <div
                className={`switch ${settings?.show_docker_inline ? "on" : ""}`}
                onClick={() => toggleSetting("show_docker_inline", api.setShowDockerInline)}
                role="switch"
                aria-checked={!!settings?.show_docker_inline}
              />
            </div>
            <div className="prefs-row">
              <div className="label">
                <div className="label-title">Screenshot folders</div>
                <div className="label-sub">
                  Where <b>Find Screenshots</b> looks. Comma-separated absolute
                  paths. macOS default is <code>~/Desktop</code>; some setups
                  redirect to <code>~/Pictures/Screenshots</code> via{" "}
                  <code>defaults write com.apple.screencapture location</code>.
                </div>
              </div>
              <input
                className="prefs-input"
                defaultValue={settings?.screenshot_dirs.join(", ") ?? ""}
                placeholder="/Users/you/Desktop, /Users/you/Pictures/Screenshots"
                onBlur={(e) => saveScreenshotDirs(e.target.value)}
              />
            </div>
          </section>

          <h2>Recommendations</h2>
          <section>
            <div className="prefs-row">
              <div className="label">
                <div className="label-title">Smart palette ranking</div>
                <div className="label-sub">
                  Trains a small on-device model from your usage history
                  ({" "}
                  <code>analytics.jsonl</code>
                  {" "}— never leaves the box) and reorders the empty
                  palette by what's most likely to be useful right now,
                  factoring in time of day and how often you pick each
                  thing.
                </div>
              </div>
              <div
                className={`switch ${settings?.enable_recommendations ? "on" : ""}`}
                onClick={() =>
                  toggleSetting(
                    "enable_recommendations",
                    api.setEnableRecommendations,
                  )
                }
                role="switch"
                aria-checked={!!settings?.enable_recommendations}
              />
            </div>
            {settings?.enable_recommendations && (
              <RecommenderSection onError={onError} />
            )}
          </section>

          <h2>GitHub</h2>
          <GitHubSection onError={onError} />

          <h2>Backup (Git)</h2>
          <BackupSection onError={onError} />

          <h2>Import</h2>
          <ImportSection onError={onError} />

          <div className="prefs-meta">
            Store: <code>~/Library/Application Support/smallcast/</code>
          </div>
      </div>
    </Screen>
  );
}

/// Status panel under the "Smart palette ranking" toggle. Loads the
/// recommender's current state on mount, exposes Retrain / Reset buttons,
/// and shows the model's current top-N items + per-feature weights so
/// the user can see what it's actually learned.
function RecommenderSection({ onError }: { onError: (s: string) => void }) {
  const [status, setStatus] = useState<import("../api").RecommendStatus | null>(
    null,
  );
  const [busy, setBusy] = useState<"train" | "clear" | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await api.recommendStatus();
      setStatus(s);
    } catch (e) {
      onError(String(e));
    }
  }, [onError]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  async function retrain() {
    setBusy("train");
    try {
      const s = await api.recommendTrain();
      setStatus(s);
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(null);
    }
  }

  async function reset() {
    setBusy("clear");
    try {
      await api.recommendClear();
      await refresh();
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(null);
    }
  }

  const trainedAt =
    status && status.trained_at_ms > 0
      ? relativeTime(status.trained_at_ms)
      : "never";

  const topByFreq = status
    ? [...status.top_items]
        .sort((a, b) => b.exec_count - a.exec_count)
        .slice(0, 5)
    : [];

  return (
    <>
      <div className="prefs-row" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
        <div className="label" style={{ display: "flex", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
          <div>
            <div className="label-title">Model status</div>
            <div className="label-sub">
              {status
                ? status.trained
                  ? `Trained ${trainedAt} on ${status.train_examples} events across ${status.item_count} items.`
                  : `Not enough data yet — ${status.train_examples} events recorded so far. Use the palette a bit more, then retrain.`
                : "Loading…"}
            </div>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button
              className="btn"
              onClick={retrain}
              disabled={busy !== null}
            >
              {busy === "train" ? "Training…" : "Retrain now"}
            </button>
            <button
              className="btn danger"
              onClick={reset}
              disabled={busy !== null}
              title="Delete the model file. Doesn't touch your analytics log."
            >
              {busy === "clear" ? "Clearing…" : "Reset model"}
            </button>
          </div>
        </div>
      </div>
      {status && status.trained && status.top_items.length > 0 && (
        <div className="prefs-row" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
          <div className="label">
            <div className="label-title">What the model thinks you'll want now</div>
            <div className="label-sub">
              Top picks at this exact moment of day, ranked by score (a
              probability between 0 and 1). When the toggle above is on,
              these float to the top of the empty palette.
            </div>
          </div>
          <ul style={{ listStyle: "none", padding: 0, margin: 0, display: "flex", flexDirection: "column", gap: 4 }}>
            {status.top_items.map((it) => (
              <li
                key={it.key}
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  fontFamily: "var(--font-family-mono)",
                  fontSize: 12,
                  color: "var(--fg-muted)",
                }}
              >
                <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {it.key}
                </span>
                <span style={{ marginLeft: 12 }}>
                  {(it.score * 100).toFixed(0)}% · {it.exec_count}×
                </span>
              </li>
            ))}
          </ul>
          <div className="label-sub" style={{ marginTop: 4 }}>
            These are the items most likely to appear at the top of your
            empty palette right now.
          </div>
        </div>
      )}
      {status && status.trained && (
        <div className="prefs-row" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
          <div className="label">
            <div className="label-title">Learned weights</div>
            <div className="label-sub">
              Each weight is what the model multiplies the named feature by.
              Positive weights help, negative hurt. Frequency, recency and
              time-of-day are the signals that usually matter most.
            </div>
          </div>
          <ul style={{ listStyle: "none", padding: 0, margin: 0, display: "flex", flexDirection: "column", gap: 4 }}>
            {status.feature_names.map((name, i) => (
              <li
                key={name}
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  fontFamily: "var(--font-family-mono)",
                  fontSize: 12,
                  color: "var(--fg-muted)",
                }}
              >
                <span>{name}</span>
                <span>{status.weights[i].toFixed(3)}</span>
              </li>
            ))}
          </ul>
          {topByFreq.length > 0 && (
            <div className="label-sub" style={{ marginTop: 4 }}>
              Most-executed items so far:{" "}
              {topByFreq.map((it, i) => (
                <span key={it.key}>
                  {i > 0 && " · "}
                  <code>{it.key}</code>{" "}
                  <span style={{ opacity: 0.6 }}>({it.exec_count}×)</span>
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </>
  );
}

function GitHubSection({ onError }: { onError: (s: string) => void }) {
  const [repos, setRepos] = useState<string[]>([]);
  const [newRepo, setNewRepo] = useState("");

  useEffect(() => {
    api
      .getSettings()
      .then((s) => setRepos(s.github_repos))
      .catch((e) => onError(String(e)));
  }, [onError]);

  async function commit(next: string[]) {
    try {
      await api.setGithubRepos(next);
      setRepos(next);
    } catch (e) {
      onError(String(e));
    }
  }

  function add() {
    const v = newRepo.trim().replace(/^https?:\/\/github\.com\//, "").replace(/\/$/, "");
    if (!v || !v.includes("/") || repos.includes(v)) {
      setNewRepo("");
      return;
    }
    commit([...repos, v]);
    setNewRepo("");
  }

  function remove(repo: string) {
    commit(repos.filter((r) => r !== repo));
  }

  return (
    <section>
      <p className="prefs-help">
        Repositories the GitHub plugin tracks. Format: <code>owner/repo</code>{" "}
        (or paste a github.com URL — we trim it). Auth piggybacks on{" "}
        <code>gh auth login</code>; nothing is stored here. Commands:{" "}
        <kbd>Show GitHub PRs</kbd>, <kbd>Search GitHub Issues</kbd>,{" "}
        <kbd>GitHub Issues Assigned to Me</kbd>.
      </p>
      {repos.length === 0 ? (
        <div className="prefs-meta" style={{ marginTop: 0, textAlign: "left" }}>
          No repos tracked yet. Add one below.
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {repos.map((r) => (
            <div key={r} className="prefs-ws">
              <code style={{ flex: 1 }}>{r}</code>
              <button className="btn danger" onClick={() => remove(r)}>
                Remove
              </button>
            </div>
          ))}
        </div>
      )}
      <div className="prefs-add">
        <input
          value={newRepo}
          onChange={(e) => setNewRepo(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder="owner/repo"
        />
        <button className="btn primary" onClick={add}>
          Add
        </button>
      </div>
    </section>
  );
}

function BackupSection({ onError }: { onError: (s: string) => void }) {
  const [settings, setSettings] = useState<
    import("../types").BackupSettings | null
  >(null);
  const [status, setStatus] = useState<
    import("../types").BackupStatus | null
  >(null);
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [s, st] = await Promise.all([
        api.getBackupSettings(),
        api.backupStatus(),
      ]);
      setSettings(s);
      setStatus(st);
    } catch (e) {
      onError(String(e));
    }
  }, [onError]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  async function run(label: string, fn: () => Promise<unknown>) {
    if (busy) return;
    setBusy(label);
    try {
      await fn();
      await refresh();
    } catch (e) {
      onError(String(e));
      await refresh();
    } finally {
      setBusy(null);
    }
  }

  async function toggleEnabled() {
    if (!settings) return;
    const next = !settings.enabled;
    setSettings({ ...settings, enabled: next });
    try {
      await api.setBackupEnabled(next);
    } catch (e) {
      onError(String(e));
      setSettings({ ...settings, enabled: !next });
    }
  }

  async function toggleIncludeAnalytics() {
    if (!settings) return;
    const next = !settings.include_analytics;
    setSettings({ ...settings, include_analytics: next });
    try {
      await api.setBackupIncludeAnalytics(next);
    } catch (e) {
      onError(String(e));
      setSettings({ ...settings, include_analytics: !next });
    }
  }

  async function saveRemote(value: string) {
    if (!settings) return;
    setSettings({ ...settings, remote: value });
    try {
      await api.setBackupRemote(value);
    } catch (e) {
      onError(String(e));
    }
  }

  async function saveBranch(value: string) {
    if (!settings) return;
    const v = value.trim() || "main";
    setSettings({ ...settings, branch: v });
    try {
      await api.setBackupBranch(v);
    } catch (e) {
      onError(String(e));
    }
  }

  if (!settings) return <section />;

  const initialized = !!status?.initialized;
  const diverged = (status?.behind ?? 0) > 0 && (status?.ahead ?? 0) > 0;

  return (
    <section>
      <div className="prefs-row">
        <div className="label">
          <div className="label-title">Enable git backup</div>
          <div className="label-sub">
            Pushes the contents of{" "}
            <code>~/Library/Application Support/smallcast/</code> to a git
            remote you control. Auth piggybacks on whatever lets you{" "}
            <code>git push</code> from terminal — SSH key, gh CLI, credential
            helper. smallcast never stores tokens.
          </div>
        </div>
        <div
          className={`switch ${settings.enabled ? "on" : ""}`}
          onClick={toggleEnabled}
          role="switch"
          aria-checked={settings.enabled}
        />
      </div>

      <div className="prefs-row">
        <div className="label">
          <div className="label-title">Remote URL</div>
          <div className="label-sub">
            e.g. <code>git@github.com:you/smallcast-backup.git</code>. Create
            an empty repo first; smallcast does the initial push for you.
          </div>
        </div>
        <input
          className="prefs-input"
          defaultValue={settings.remote}
          placeholder="git@github.com:you/smallcast-backup.git"
          onBlur={(e) => saveRemote(e.target.value)}
        />
      </div>

      <div className="prefs-row">
        <div className="label">
          <div className="label-title">Branch</div>
          <div className="label-sub">
            Defaults to <code>main</code>.
          </div>
        </div>
        <input
          className="prefs-input"
          style={{ minWidth: 140 }}
          defaultValue={settings.branch}
          onBlur={(e) => saveBranch(e.target.value)}
        />
      </div>

      <div className="prefs-row">
        <div className="label">
          <div className="label-title">Include analytics.jsonl</div>
          <div className="label-sub">
            <b>Off</b> by default — the log contains every query you've typed.
            Turn on if you want full-fidelity backups across machines.
          </div>
        </div>
        <div
          className={`switch ${settings.include_analytics ? "on" : ""}`}
          onClick={toggleIncludeAnalytics}
          role="switch"
          aria-checked={settings.include_analytics}
        />
      </div>

      <div className="prefs-row">
        <div className="label">
          <div className="label-title">Status</div>
          <div className="label-sub">
            {!initialized && (
              <>
                Not connected. Set the remote above, then <b>Connect</b>.
              </>
            )}
            {initialized && diverged && (
              <span style={{ color: "#f0b46b" }}>
                ⚠ Diverged: {status?.ahead} ahead / {status?.behind} behind.
                Resolve with `git` in the .backup-git dir, or use Force Push to
                overwrite remote.
              </span>
            )}
            {initialized && !diverged && (
              <>
                {status?.dirty_count
                  ? `${status.dirty_count} change${status.dirty_count === 1 ? "" : "s"} pending`
                  : "✓ in sync"}
                {status?.ahead ? ` · ${status.ahead} ahead` : ""}
                {status?.behind ? ` · ${status.behind} behind` : ""}
                {status?.last_synced_ms
                  ? ` · last pushed ${relativeTime(status.last_synced_ms)}`
                  : ""}
              </>
            )}
            {status?.last_error && (
              <div style={{ color: "#f08585", marginTop: 4, fontSize: 11 }}>
                Last error: {status.last_error}
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="prefs-row" style={{ flexWrap: "wrap", gap: 8 }}>
        {!initialized ? (
          <button
            className="btn primary"
            disabled={!settings.remote || !!busy}
            onClick={() => run("connect", api.backupInit)}
          >
            {busy === "connect" ? "Connecting…" : "Connect"}
          </button>
        ) : (
          <>
            <button
              className="btn primary"
              disabled={!!busy}
              onClick={() => run("sync", api.backupSync)}
            >
              {busy === "sync" ? "Syncing…" : "Sync now"}
            </button>
            <button
              className="btn"
              disabled={!!busy}
              onClick={() => run("pull", api.backupPull)}
            >
              {busy === "pull" ? "Pulling…" : "Pull"}
            </button>
            <button
              className="btn danger"
              disabled={!!busy}
              onClick={() => {
                if (
                  confirm(
                    "Force-push will OVERWRITE the remote with your local store. Other machines pushing to the same remote may lose data. Continue?",
                  )
                ) {
                  run("force", api.backupForcePush);
                }
              }}
            >
              {busy === "force" ? "Pushing…" : "Force push"}
            </button>
          </>
        )}
      </div>
    </section>
  );
}

function ImportSection({ onError }: { onError: (s: string) => void }) {
  const [raycastInstalled, setRaycastInstalled] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string | null>(null);

  useEffect(() => {
    api
      .detectRaycast()
      .then((r) => setRaycastInstalled(r.installed))
      .catch(() => setRaycastInstalled(false));
  }, []);

  async function pickAndImport() {
    if (busy) return;
    try {
      const selected = await openDialog({
        multiple: true,
        filters: [{ name: "JSON", extensions: ["json"] }],
        title: "Pick Raycast export file(s)",
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      if (paths.length === 0) return;
      setBusy(true);
      setResult(null);
      let totalS = 0,
        totalQ = 0,
        totalSkip = 0;
      for (const p of paths) {
        const r = await api.importFromFile(p);
        totalS += r.snippets;
        totalQ += r.quicklinks;
        totalSkip += r.skipped;
      }
      setResult(
        `Imported ${totalS} snippet(s), ${totalQ} quicklink(s)` +
          (totalSkip ? `, skipped ${totalSkip}` : "") +
          "."
      );
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function openRaycast() {
    try {
      await api.executeApp("/Applications/Raycast.app");
    } catch {
      /* ignore */
    }
  }

  return (
    <section>
      <div className="prefs-row">
        <div className="label">
          <div className="label-title">
            {raycastInstalled === null
              ? "Looking for Raycast…"
              : raycastInstalled
              ? "Raycast detected"
              : "Import from JSON"}
          </div>
          <div className="label-sub">
            {raycastInstalled ? (
              <>
                Raycast's data lives in an encrypted database, so we can't read
                it directly. Export its JSON once and we'll take it from there:
                <ol style={{ margin: "8px 0 0", paddingLeft: 18 }}>
                  <li>
                    Open Raycast, search <b>Export Quicklinks</b>, save the
                    JSON somewhere you can find it.
                  </li>
                  <li>
                    Same for <b>Export Snippets</b>.
                  </li>
                  <li>Click <b>Choose files…</b> below and pick both.</li>
                </ol>
              </>
            ) : (
              <>
                Drop in a JSON array with <code>{"{name, text}"}</code> snippets
                and/or <code>{"{name, link}"}</code> quicklinks. Raycast export
                shapes are auto-detected.
              </>
            )}
          </div>
        </div>
        {raycastInstalled && (
          <button className="btn" onClick={openRaycast}>
            Open Raycast
          </button>
        )}
      </div>

      <div className="prefs-row">
        <div className="label">
          <div className="label-title">Pick export file(s)</div>
          <div className="label-sub">
            You can pick more than one at once (⌘-click to multi-select).
          </div>
        </div>
        <button className="btn primary" onClick={pickAndImport} disabled={busy}>
          {busy ? "Importing…" : "Choose files…"}
        </button>
      </div>

      {result && (
        <div
          style={{
            color: "var(--accent)",
            fontSize: 12,
            padding: "6px 0",
          }}
        >
          ✓ {result}
        </div>
      )}
    </section>
  );
}
