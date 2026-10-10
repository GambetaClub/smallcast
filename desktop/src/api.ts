import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import type {
  ClipboardEntry,
  Item,
  OpenIn,
  PaletteEntry,
  Quicklink,
  Snippet,
} from "./types";

export interface Settings {
  show_vite_inline: boolean;
  show_docker_inline: boolean;
  show_snippets_inline: boolean;
  show_quicklinks_inline: boolean;
  screenshot_dirs: string[];
  check_updates_on_launch: boolean;
  github_repos: string[];
  enable_recommendations: boolean;
}

// On-device recommender — see desktop/src-tauri/src/recommend.rs.
export type RecommendScoreInput = { key: string; kind: string };
export type RecommendScoreOutput = {
  key: string;
  score: number;
  has_history: boolean;
};
export type RecommendTopItem = {
  key: string;
  kind: string;
  score: number;
  exec_count: number;
  last_used_ms: number;
};
export type RecommendStatus = {
  trained: boolean;
  trained_at_ms: number;
  last_event_ms: number;
  train_examples: number;
  item_count: number;
  weights: number[];
  feature_names: string[];
  top_items: RecommendTopItem[];
};

export const api = {
  // Settings
  getSettings: () => invoke<Settings>("get_settings"),
  setShowViteInline: (value: boolean) =>
    invoke<void>("set_show_vite_inline", { value }),
  setShowDockerInline: (value: boolean) =>
    invoke<void>("set_show_docker_inline", { value }),
  setShowSnippetsInline: (value: boolean) =>
    invoke<void>("set_show_snippets_inline", { value }),
  setShowQuicklinksInline: (value: boolean) =>
    invoke<void>("set_show_quicklinks_inline", { value }),
  setScreenshotDirs: (value: string[]) =>
    invoke<void>("set_screenshot_dirs", { value }),
  setCheckUpdatesOnLaunch: (value: boolean) =>
    invoke<void>("set_check_updates_on_launch", { value }),
  setGithubRepos: (value: string[]) =>
    invoke<void>("set_github_repos", { value }),
  setEnableRecommendations: (value: boolean) =>
    invoke<void>("set_enable_recommendations", { value }),

  // Recommender — local logistic regression trained from analytics.jsonl.
  // Inference is fast and synchronous; the model file lives at
  // ~/Library/Application Support/davidcast/recommender_state.json.
  recommendTrain: () => invoke<RecommendStatus>("recommend_train"),
  recommendStatus: () => invoke<RecommendStatus>("recommend_status"),
  recommendScore: (inputs: RecommendScoreInput[]) =>
    invoke<RecommendScoreOutput[]>("recommend_score", { inputs }),
  recommendClear: () => invoke<void>("recommend_clear"),

  // GitHub plugin — shells out to `gh`. Errors propagate as the rejected
  // promise's message; the palette surfaces them via the standard error
  // banner if `gh` isn't installed / authed.
  githubListPrs: () =>
    invoke<import("./types").GitHubPr[]>("github_list_prs"),
  githubListIssues: (query: string | null) =>
    invoke<import("./types").GitHubIssue[]>("github_list_issues", { query }),
  githubListAssigned: () =>
    invoke<import("./types").GitHubIssue[]>("github_list_assigned"),

  openUrl: (url: string) => invoke<void>("open_url", { url }),

  // Backup (git) — local commands all touch the system `git` CLI; auth
  // is whatever lets you `git push` from terminal. No credential storage.
  getBackupSettings: () =>
    invoke<import("./types").BackupSettings>("get_backup_settings"),
  setBackupRemote: (value: string) =>
    invoke<void>("set_backup_remote", { value }),
  setBackupBranch: (value: string) =>
    invoke<void>("set_backup_branch", { value }),
  setBackupEnabled: (value: boolean) =>
    invoke<void>("set_backup_enabled", { value }),
  setBackupIncludeAnalytics: (value: boolean) =>
    invoke<void>("set_backup_include_analytics", { value }),
  backupStatus: () => invoke<import("./types").BackupStatus>("backup_status"),
  backupInit: () => invoke<import("./types").BackupStatus>("backup_init"),
  backupSync: () => invoke<import("./types").BackupStatus>("backup_sync"),
  backupPull: () => invoke<import("./types").BackupStatus>("backup_pull"),
  backupForcePush: () =>
    invoke<import("./types").BackupStatus>("backup_force_push"),
  backupGitDir: () => invoke<string | null>("backup_git_dir"),

  // Updater — talks to the GitHub Releases endpoint pinned in
  // tauri.conf.json. `check()` returns null when there is no update.
  checkForUpdate: () => check(),
  installUpdateAndRelaunch: async (update: Update) => {
    await update.downloadAndInstall();
    await relaunch();
  },
  searchScreenshots: (limit?: number) =>
    invoke<import("./types").FileEntry[]>("search_screenshots", { limit }),

  // Analytics — local-only, fire-and-forget JSONL append.
  analyticsRecord: (sessionId: string, kind: string, data: unknown) =>
    invoke<void>("analytics_record", {
      sessionId,
      kind,
      data,
    }),
  analyticsTail: (limit: number) =>
    invoke<unknown[]>("analytics_tail", { limit }),
  analyticsSummary: () =>
    invoke<import("./types").AnalyticsSummary>("analytics_summary"),
  analyticsClear: () => invoke<void>("analytics_clear"),
  analyticsLogPath: () => invoke<string | null>("analytics_log_path"),

  // Items
  listItems: () => invoke<Item[]>("list_items"),
  listPalette: () => invoke<PaletteEntry[]>("list_palette"),
  listAgents: () => invoke<import("./types").AgentEntry[]>("list_agents"),
  listVitePorts: () =>
    invoke<import("./types").VitePortEntry[]>("list_vite_ports"),
  listDockerContainers: () =>
    invoke<import("./types").DockerEntry[]>("list_docker_containers"),
  executeApp: (path: string) => invoke<void>("execute_app", { path }),
  // Tauri converts Rust snake_case args to camelCase on the wire — `terminal_app`
  // becomes `terminalApp`. Matches `commands::execute_agent(.., terminal_app: String, ..)`.
  executeAgent: (args: { pid: number; tty: string; terminal_app: string }) =>
    invoke<void>("execute_agent", {
      pid: args.pid,
      tty: args.tty,
      terminalApp: args.terminal_app,
    }),
  executeVite: (url: string) => invoke<void>("execute_vite", { url }),
  executeDockerShell: (id: string) =>
    invoke<void>("execute_docker_shell", { id }),
  executeDockerLogs: (id: string) =>
    invoke<void>("execute_docker_logs", { id }),
  showPreferences: () => invoke<void>("show_preferences"),

  // Snippets
  createSnippet: (input: {
    name: string;
    text: string;
    keyword?: string;
    sensitive?: boolean;
  }) => invoke<Snippet>("create_snippet", { input }),
  updateSnippet: (input: {
    id: string;
    name?: string;
    text?: string;
    keyword?: string | null;
    sensitive?: boolean;
  }) => invoke<Snippet>("update_snippet", { input }),
  deleteSnippet: (id: string) => invoke<void>("delete_snippet", { id }),

  // Quicklinks
  createQuicklink: (input: {
    name: string;
    url: string;
    keyword?: string;
    open_in?: OpenIn;
  }) => invoke<Quicklink>("create_quicklink", { input }),
  updateQuicklink: (input: {
    id: string;
    name?: string;
    url?: string;
    keyword?: string | null;
    open_in?: OpenIn;
  }) => invoke<Quicklink>("update_quicklink", { input }),
  deleteQuicklink: (id: string) => invoke<void>("delete_quicklink", { id }),

  // Execute
  executeSnippet: (id: string) => invoke<void>("execute_snippet", { id }),
  executeQuicklink: (id: string, args?: Record<string, string>) =>
    invoke<void>("execute_quicklink", { id, args }),

  // Window
  hidePalette: () => invoke<void>("hide_palette"),
  hideAndPaste: () => invoke<void>("hide_and_paste"),

  // App icons (data URL, cached)
  getAppIcon: (path: string) =>
    invoke<string | null>("get_app_icon", { path }),

  // File search
  searchFiles: (opts: import("./types").FileSearchOpts) =>
    invoke<import("./types").FileEntry[]>("search_files", { opts }),
  openFile: (path: string) => invoke<void>("open_file", { path }),
  revealFile: (path: string) => invoke<void>("reveal_file", { path }),
  copyFilePath: (path: string) => invoke<void>("copy_file_path", { path }),
  copyFileImage: (path: string) => invoke<void>("copy_file_image", { path }),
  fileThumbnail: (path: string) =>
    invoke<string | null>("file_thumbnail", { path }),

  // System stats — one snapshot per call (no live sampler in the
  // backend). The Stats view re-invokes on its refresh button.
  systemStats: () => invoke<import("./types").SystemStats>("system_stats"),

  // System quick actions. Destructive ones (empty trash / restart /
  // shut down / log out) are gated by a confirm dialog in App.tsx —
  // these wrappers themselves don't prompt.
  systemLockScreen: () => invoke<void>("system_lock_screen"),
  systemSleep: () => invoke<void>("system_sleep"),
  systemEmptyTrash: () => invoke<void>("system_empty_trash"),
  systemRestart: () => invoke<void>("system_restart"),
  systemShutDown: () => invoke<void>("system_shut_down"),
  systemLogOut: () => invoke<void>("system_log_out"),

  // Skills
  listSkills: () => invoke<import("./types").SkillEntry[]>("list_skills"),
  readSkill: (path: string) => invoke<string>("read_skill", { path }),

  // Themes
  getActiveTheme: () => invoke<import("./types").Theme>("get_active_theme"),

  // Clipboard history
  listClipboard: () => invoke<ClipboardEntry[]>("list_clipboard"),
  executeClipboard: (id: string) => invoke<void>("execute_clipboard", { id }),
  deleteClipboardEntry: (id: string) =>
    invoke<void>("delete_clipboard", { id }),
  clearClipboard: () => invoke<void>("clear_clipboard"),

  // Import
  importFromFile: (path: string) =>
    invoke<{ snippets: number; quicklinks: number; skipped: number }>(
      "import_from_file",
      { path }
    ),
  detectRaycast: () =>
    invoke<{ installed: boolean; path: string | null }>("detect_raycast"),
};
