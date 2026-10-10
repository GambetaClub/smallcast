export interface Snippet {
  id: string;
  name: string;
  keyword?: string;
  text: string;
  created_at: string;
  updated_at: string;
  deleted?: boolean;
  rev?: number;
  sensitive?: boolean;
}

export interface Quicklink {
  id: string;
  name: string;
  keyword?: string;
  url: string;
  open_in: OpenIn;
  created_at: string;
  updated_at: string;
  deleted?: boolean;
  rev?: number;
}

export type OpenIn = "default_browser" | "chrome" | "safari";

export interface AppEntry {
  name: string;
  path: string;
}

export interface CommandEntry {
  id: string;
  name: string;
  subtitle: string;
}

export interface GitInfo {
  is_repo: boolean;
  branch: string | null;
  dirty: boolean;
}

export interface AgentEntry {
  pid: number;
  cwd: string;
  project: string;
  tty: string;
  command: string;
  elapsed: string;
  terminal_app: string;
  git: GitInfo;
}

export interface VitePortEntry {
  pid: number;
  port: number;
  host: string;
  url: string;
  cwd: string;
  project: string;
  command: string;
  elapsed: string;
  git: GitInfo;
}

export interface DockerEntry {
  id: string;
  name: string;
  image: string;
  status: string;
  ports: string;
  mode: "shell" | "logs";
}

export interface FileEntry {
  path: string;
  name: string;
  parent: string;
  size: number;
  modified_at: number;
  is_image: boolean;
  ext: string;
}

export interface FileSearchOpts {
  query?: string;
  extensions?: string[];
  category?: string;
  roots?: string[];
  sort_by_mtime?: boolean;
  limit?: number;
}

export interface Theme {
  id: string;
  name: string;
  tokens: Record<string, string>;
}

export interface ClipboardEntry {
  id: string;
  text: string;
  copied_at: string;
  char_count: number;
}

export interface SkillEntry {
  id: string;
  name: string;
  description: string;
  path: string;
  source: string;
  size: number;
  modified_at: number;
}

export type Item =
  | ({ kind: "snippet" } & Snippet)
  | ({ kind: "quicklink" } & Quicklink);

export interface CalcEntry {
  id: string;
  expr: string;
  result: string;
}

export interface GitHubPr {
  repo: string;
  number: number;
  title: string;
  author: string;
  url: string;
  created_at: string;
  labels: string[];
  is_draft: boolean;
}

export interface GitHubIssue {
  repo: string;
  number: number;
  title: string;
  author: string;
  url: string;
  created_at: string;
  labels: string[];
  assignees: string[];
}

export type PaletteEntry =
  | ({ kind: "command" } & CommandEntry)
  | ({ kind: "snippet" } & Snippet)
  | ({ kind: "quicklink" } & Quicklink)
  | ({ kind: "app" } & AppEntry)
  | ({ kind: "agent" } & AgentEntry)
  | ({ kind: "vite" } & VitePortEntry)
  | ({ kind: "docker" } & DockerEntry)
  | ({ kind: "file" } & FileEntry)
  | ({ kind: "clipboard" } & ClipboardEntry)
  | ({ kind: "skill" } & SkillEntry)
  | ({ kind: "calc" } & CalcEntry)
  | ({ kind: "github_pr" } & GitHubPr)
  | ({ kind: "github_issue" } & GitHubIssue);

export function isSnippet(i: Item | PaletteEntry): i is { kind: "snippet" } & Snippet {
  return i.kind === "snippet";
}

export function isQuicklink(i: Item | PaletteEntry): i is { kind: "quicklink" } & Quicklink {
  return i.kind === "quicklink";
}

export function isApp(i: PaletteEntry): i is { kind: "app" } & AppEntry {
  return i.kind === "app";
}

export function isCommand(i: PaletteEntry): i is { kind: "command" } & CommandEntry {
  return i.kind === "command";
}

export function isAgent(i: PaletteEntry): i is { kind: "agent" } & AgentEntry {
  return i.kind === "agent";
}

export function isVite(i: PaletteEntry): i is { kind: "vite" } & VitePortEntry {
  return i.kind === "vite";
}

export function isDocker(
  i: PaletteEntry
): i is { kind: "docker" } & DockerEntry {
  return i.kind === "docker";
}

export function isFile(i: PaletteEntry): i is { kind: "file" } & FileEntry {
  return i.kind === "file";
}

export function isClipboard(
  i: PaletteEntry
): i is { kind: "clipboard" } & ClipboardEntry {
  return i.kind === "clipboard";
}

export function isSkill(i: PaletteEntry): i is { kind: "skill" } & SkillEntry {
  return i.kind === "skill";
}

export function isCalc(i: PaletteEntry): i is { kind: "calc" } & CalcEntry {
  return i.kind === "calc";
}

export function isGitHubPr(i: PaletteEntry): i is { kind: "github_pr" } & GitHubPr {
  return i.kind === "github_pr";
}

export function isGitHubIssue(i: PaletteEntry): i is { kind: "github_issue" } & GitHubIssue {
  return i.kind === "github_issue";
}

export function asItem(e: PaletteEntry): Item | null {
  if (e.kind === "snippet") return { ...e };
  if (e.kind === "quicklink") return { ...e };
  return null;
}

/// Stable identity key for a palette entry. Includes the user-visible
/// name where applicable, but excludes volatile fields (elapsed times,
/// last_seen, mtimes, status strings) so two refreshes of the same set
/// hash identically. Used both as a React key and for change detection
/// across refreshes — if every entry's stableEntryKey is unchanged, no
/// row content the user sees has changed and we can skip re-rendering.
export function stableEntryKey(e: PaletteEntry): string {
  switch (e.kind) {
    case "command":
      return `c:${e.id}`;
    case "snippet":
      return `s:${e.id}:${e.name}:${e.keyword ?? ""}:${e.sensitive ? 1 : 0}`;
    case "quicklink":
      return `q:${e.id}:${e.name}:${e.keyword ?? ""}:${e.url}`;
    case "app":
      return `a:${e.path}:${e.name}`;
    case "agent":
      return `g:${e.pid}:${e.project}:${e.cwd}`;
    case "vite":
      return `v:${e.pid}:${e.port}`;
    case "docker":
      return `d:${e.id}:${e.mode}`;
    case "clipboard":
      return `cp:${e.id}`;
    case "file":
      return `f:${e.path}`;
    case "skill":
      return `sk:${e.path}`;
    case "calc":
      return `calc:${e.expr}`;
    case "github_pr":
      return `ghpr:${e.repo}:${e.number}`;
    case "github_issue":
      return `ghi:${e.repo}:${e.number}`;
  }
}

export type BackupSettings = {
  enabled: boolean;
  remote: string;
  branch: string;
  include_analytics: boolean;
  auto_interval_min: number;
};

export type BackupStatus = {
  initialized: boolean;
  remote: string | null;
  branch: string | null;
  dirty_count: number;
  ahead: number;
  behind: number;
  last_synced_ms: number | null;
  last_error: string | null;
};

export type SystemStats = {
  load_1m: number;
  load_5m: number;
  load_15m: number;
  cpu_count: number;
  cpu_brand: string;
  mem_total: number;
  mem_used: number;
  mem_pressure: string | null;
  disk_total: number;
  disk_used: number;
  disk_path: string;
  battery_percent: number | null;
  battery_state: string | null;
  battery_time_remaining: string | null;
  thermal_pressure: string | null;
  uptime_secs: number;
  host_name: string;
  os_version: string;
  model: string;
};

export type AnalyticsSummary = {
  log_path: string | null;
  total_events: number;
  opens: number;
  executes: number;
  no_results: number;
  success_rate: number | null;
  top_queries: { q: string; count: number }[];
  top_items: { name: string; kind: string; count: number }[];
  kind_breakdown: { kind: string; count: number }[];
  daily_opens: { day: string; count: number }[];
  avg_dwell_ms: number | null;
  first_event_ts: number | null;
  last_event_ts: number | null;
};

export function extractPlaceholders(url: string): string[] {
  const out: string[] = [];
  const re = /\{([^}]+)\}/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(url)) !== null) {
    if (!out.includes(m[1])) out.push(m[1]);
  }
  return out;
}
