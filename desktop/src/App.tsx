import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, type Settings } from "./api";
import type { Update } from "@tauri-apps/plugin-updater";
import { r2, scheduleIdle } from "./utils";
import { Analytics } from "./components/Analytics";
import { Help } from "./components/Help";
import { ItemForm } from "./components/ItemForm";
import { Palette } from "./components/Palette";
import { Preferences } from "./components/Preferences";
import { Stats } from "./components/Stats";
import { UpdateBanner } from "./components/UpdateBanner";
import { stableEntryKey, type Item, type PaletteEntry, type Theme } from "./types";

const ENTRIES_CACHE_KEY = "davidcast.entries.cache.v1";

/// Identity-only fingerprint of the palette list. Two refreshes returning
/// the same set of items in the same order produce the same hash even if
/// volatile fields (agent elapsed, vite last_seen, etc.) differ. Lets us
/// skip setEntries on routine refreshes — Fuse keeps its index, the row
/// list keeps its references, no cascading re-render.
function entriesFingerprint(list: PaletteEntry[]): string {
  const parts: string[] = [String(list.length)];
  for (const e of list) parts.push(stableEntryKey(e));
  return parts.join("|");
}

function loadCachedEntries(): PaletteEntry[] {
  try {
    const v = localStorage.getItem(ENTRIES_CACHE_KEY);
    if (!v) return [];
    const parsed = JSON.parse(v);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

/// Set CSS variables on the document root from a theme. Mirrors the
/// `--<name>` shape used in palette.css.
function applyTheme(theme: Theme) {
  const root = document.documentElement;
  for (const [k, v] of Object.entries(theme.tokens)) {
    root.style.setProperty(`--${k}`, v);
  }
}

export type Session = {
  id: string;
  startedAt: number; // ms since epoch
};

function newSession(): Session {
  return {
    id:
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : `${Date.now()}-${Math.random().toString(36).slice(2)}`,
    startedAt: Date.now(),
  };
}

type View =
  | { kind: "palette" }
  | { kind: "create"; presetKind: "snippet" | "quicklink" }
  | { kind: "edit"; item: Item }
  | { kind: "preferences" }
  | { kind: "help" }
  | { kind: "analytics" }
  | { kind: "stats" };

type InitialFilter = PaletteEntry["kind"] | null;

export default function App() {
  // Hydrate entries from the on-disk cache so the palette has data on the
  // very first ⌃Space after launch — before list_palette has had a chance
  // to return. The next refresh will replace anything stale.
  const [entries, setEntries] = useState<PaletteEntry[]>(loadCachedEntries);
  const [settings, setSettings] = useState<Settings>({
    show_vite_inline: true,
    show_docker_inline: true,
    show_snippets_inline: true,
    show_quicklinks_inline: true,
    screenshot_dirs: [],
    check_updates_on_launch: true,
    github_repos: [],
    enable_recommendations: false,
  });
  const [update, setUpdate] = useState<Update | null>(null);
  const [view, setView] = useState<View>({ kind: "palette" });
  const [error, setError] = useState<string | null>(null);
  const [initialFilter, setInitialFilter] = useState<InitialFilter>(null);
  const [session, setSession] = useState<Session>(() => newSession());

  // Identity-only fingerprint of the last `entries` list. We re-fetch on
  // every palette open so dynamic kinds (agents, vite, docker, clipboard)
  // stay fresh — but if the resulting set of items is unchanged, we keep
  // the old array reference. That stops the Fuse index from rebuilding on
  // every back-to-back ⌃Space, which was a major source of typing latency.
  // Initialized from the hydrated cache so an unchanged refresh after
  // launch is also a no-op.
  const entriesFingerprintRef = useRef<string>(entriesFingerprint(loadCachedEntries()));

  // Tracks the most recent refresh outcome so the open-perf event can
  // include whether refresh changed anything (cache hit vs miss).
  const lastRefreshChangedRef = useRef<boolean>(false);
  const lastRefreshMsRef = useRef<number>(0);

  const refresh = useCallback(async () => {
    const t0 = performance.now();
    const [list, s] = await Promise.all([api.listPalette(), api.getSettings()]);
    const fp = entriesFingerprint(list);
    const changed = fp !== entriesFingerprintRef.current;
    if (changed) {
      entriesFingerprintRef.current = fp;
      setEntries(list);
      // Persist so next launch hydrates with the same data and the very
      // first open feels instant. Stringify is bounded (a few hundred KB
      // for ~300 entries) and runs off the keystroke critical path.
      try {
        localStorage.setItem(ENTRIES_CACHE_KEY, JSON.stringify(list));
      } catch {
        /* quota — ignore */
      }
    }
    setSettings(s);
    lastRefreshChangedRef.current = changed;
    lastRefreshMsRef.current = performance.now() - t0;
  }, []);

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
  }, [refresh]);

  // Pull and apply the theme on mount. Failures fall back to the CSS
  // defaults baked into palette.css.
  useEffect(() => {
    api.getActiveTheme().then(applyTheme).catch(() => {});
  }, []);

  // Check for updates on launch, gated by user setting. Silent when no
  // update or when the network is offline — manual "Check for Updates"
  // command surfaces errors.
  useEffect(() => {
    if (!settings.check_updates_on_launch) return;
    let cancelled = false;
    api
      .checkForUpdate()
      .then((u) => {
        if (cancelled || !u) return;
        setUpdate(u);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [settings.check_updates_on_launch]);

  const checkForUpdateNow = useCallback(async () => {
    try {
      const u = await api.checkForUpdate();
      if (u) {
        setUpdate(u);
      } else {
        setError(`You're up to date (v${import.meta.env.VITE_APP_VERSION ?? ""}).`);
      }
    } catch (e) {
      setError(`Update check failed: ${e}`);
    }
  }, []);

  // Stamped when the palette is dismissed (blur). Used to compute
  // idle_ms_since_last_use on the next open — that's the signal we
  // need to spot macOS App Nap waking the process up.
  const lastDismissAtRef = useRef<number | null>(null);

  const measureOpen = useCallback(
    (sessionId: string, via: string) => {
      const showStartedAt = performance.now();
      const idleMs =
        lastDismissAtRef.current != null
          ? showStartedAt - lastDismissAtRef.current
          : null;
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          const paintMs = performance.now() - showStartedAt;
          scheduleIdle(() => {
            api
              .analyticsRecord(sessionId, "perf_open", {
                via,
                paint_ms: r2(paintMs),
                idle_ms_since_last_use: idleMs == null ? null : r2(idleMs),
                refresh_changed: lastRefreshChangedRef.current,
                refresh_ms: r2(lastRefreshMsRef.current),
              })
              .catch(() => {});
          });
        });
      });
    },
    []
  );

  useEffect(() => {
    const offShow = listen("palette:show", () => {
      setView({ kind: "palette" });
      setError(null);
      setInitialFilter(null);
      const s = newSession();
      setSession(s);
      api.analyticsRecord(s.id, "open", { via: "hotkey" }).catch(() => {});
      measureOpen(s.id, "hotkey");
      refresh().catch((e) => setError(String(e)));
    });
    const offClipboard = listen("clipboard:show", () => {
      setView({ kind: "palette" });
      setError(null);
      setInitialFilter("clipboard");
      const s = newSession();
      setSession(s);
      api
        .analyticsRecord(s.id, "open", { via: "clipboard_hotkey" })
        .catch(() => {});
      measureOpen(s.id, "clipboard_hotkey");
      refresh().catch((e) => setError(String(e)));
    });
    // The tray menu's "Preferences…" item routes through the backend so
    // it can show the main window first; then it fires this event to
    // switch the view inside.
    const offPrefs = listen("preferences:show", () => {
      setView({ kind: "preferences" });
      setError(null);
      setInitialFilter(null);
      refresh().catch((e) => setError(String(e)));
    });
    // Stamp the moment the window loses focus so the *next* open can
    // report idle_ms_since_last_use. Cheap; just a timestamp.
    const onBlurStamp = () => {
      lastDismissAtRef.current = performance.now();
    };
    window.addEventListener("blur", onBlurStamp);
    return () => {
      offShow.then((fn) => fn());
      offClipboard.then((fn) => fn());
      offPrefs.then((fn) => fn());
      window.removeEventListener("blur", onBlurStamp);
    };
  }, [refresh, measureOpen]);

  // Auto-dismiss errors so a stale message doesn't sit forever.
  useEffect(() => {
    if (!error) return;
    const t = window.setTimeout(() => setError(null), 6000);
    return () => window.clearTimeout(t);
  }, [error]);

  const backToPalette = () => {
    setInitialFilter(null);
    setView({ kind: "palette" });
  };

  async function onCommand(id: string) {
    switch (id) {
      case "create.snippet":
        setView({ kind: "create", presetKind: "snippet" });
        break;
      case "create.quicklink":
        setView({ kind: "create", presetKind: "quicklink" });
        break;
      case "open.preferences":
        setView({ kind: "preferences" });
        break;
      case "help.show":
        setView({ kind: "help" });
        break;
      case "show.analytics":
        setView({ kind: "analytics" });
        break;
      case "show.stats":
        setView({ kind: "stats" });
        break;
      case "app.check_updates":
        checkForUpdateNow();
        break;
      // System quick actions. Reversible ones (lock, sleep) fire
      // immediately; destructive ones go through a native confirm so a
      // stray ↵ on a fuzzy match can't shut the machine down.
      case "system.lock":
        api.systemLockScreen().catch((e) => setError(String(e)));
        break;
      case "system.sleep":
        api.systemSleep().catch((e) => setError(String(e)));
        break;
      case "system.empty_trash":
        confirmAndRun(
          "Empty Trash?",
          "This permanently deletes everything in your Trash.",
          api.systemEmptyTrash,
        );
        break;
      case "system.restart":
        confirmAndRun(
          "Restart your Mac?",
          "macOS will close all apps. Unsaved work may be lost.",
          api.systemRestart,
        );
        break;
      case "system.shut_down":
        confirmAndRun(
          "Shut down your Mac?",
          "macOS will close all apps. Unsaved work may be lost.",
          api.systemShutDown,
        );
        break;
      case "system.log_out":
        confirmAndRun(
          "Log out of your account?",
          "macOS will close all apps in this session.",
          api.systemLogOut,
        );
        break;
    }
  }

  // Native confirm dialog. Hides the palette first so the dialog isn't
  // owned by a window that's about to auto-dismiss on blur.
  async function confirmAndRun(
    title: string,
    message: string,
    run: () => Promise<void>,
  ) {
    try {
      await api.hidePalette();
    } catch {
      /* hide failures shouldn't block the confirm */
    }
    try {
      const ok = await ask(message, { title, kind: "warning" });
      if (!ok) return;
      await run();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <>
      {view.kind === "create" && (
        <ItemForm
          presetKind={view.presetKind}
          onDone={async () => {
            await refresh();
            backToPalette();
          }}
          onCancel={backToPalette}
          onError={setError}
        />
      )}
      {view.kind === "edit" && (
        <ItemForm
          initial={view.item}
          onDone={async () => {
            await refresh();
            backToPalette();
          }}
          onCancel={backToPalette}
          onError={setError}
        />
      )}
      {view.kind === "palette" && (
        <Palette
          entries={entries}
          settings={settings}
          session={session}
          initialFilter={initialFilter}
          onEdit={(item) => setView({ kind: "edit", item })}
          onCommand={onCommand}
          refresh={refresh}
          onError={setError}
        />
      )}
      {view.kind === "preferences" && (
        <Preferences onClose={backToPalette} onError={setError} />
      )}
      {view.kind === "help" && <Help onClose={backToPalette} />}
      {view.kind === "analytics" && (
        <Analytics onClose={backToPalette} onError={setError} />
      )}
      {view.kind === "stats" && (
        <Stats onClose={backToPalette} onError={setError} />
      )}
      {error && (
        <div className="error-banner" role="alert">
          <span className="error-banner-text">{error}</span>
          <button
            type="button"
            aria-label="Dismiss"
            onClick={() => setError(null)}
          >
            ✕
          </button>
        </div>
      )}
      {update && (
        <UpdateBanner
          update={update}
          onDismiss={() => setUpdate(null)}
          onError={setError}
        />
      )}
    </>
  );
}
