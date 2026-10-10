# Planned features — pick up on the other machine

These are feature asks recorded while shipping phase 1. They are **not yet implemented**. Everything above in `PLAN.md` is still authoritative for the phased roadmap; this file is a scratchpad for the next two things to build.

---

## Backlog (small, well-scoped)

- **Giphy search** — built-in command `giphy.search`. Type a query, hit ↵, copy the GIF URL or animated bitmap to clipboard, palette stays open. Needs a Giphy API key (their dev tier is free) — wire as `Config::giphy_api_key` with a Preferences input. Without a key, the command shows a one-line "set your Giphy API key in Preferences" hint and links out.
- **Move snippet / quicklink to another workspace** — keyboard shortcut from the edit form (or a side action), opens a workspace picker, performs the move (atomic file move between `workspaces/<src>/items.json` and `workspaces/<dst>/items.json` with the same `id`/`rev` so the sync engine sees a single op).

---

## 1. Agents — show running Claude Code sessions in the palette

### Goal

Type `agents` (or just start typing a project name) in the palette. See every terminal tab that's currently running `claude` — with the project folder and how long it's been running. Press Enter and the containing terminal window/tab comes to the front.

### Data shape

A new palette entry `kind: "agent"`:

```ts
type AgentEntry = {
  kind: "agent";
  pid: number;            // the `claude` process PID
  cwd: string;            // working directory (project root)
  project: string;        // basename of cwd for display
  tty: string;            // /dev/ttysNNN — used to find the terminal tab
  command: string;        // full command line, useful if one dir has two claudes
  started_at: string;     // ISO timestamp
  terminal_app: string;   // "iTerm2" | "Terminal" | "Warp" | "Unknown"
};
```

### Backend (Rust)

Module: `desktop/src-tauri/src/agents.rs`.

**Scan** — run `ps -axo pid,ppid,etime,tty,comm,args` and filter rows where the binary is `claude` (the Claude Code CLI). For each match:

1. **cwd:** shell out to `lsof -a -d cwd -p <pid> -F n` (or read `proc` on Linux; on macOS `lsof` is the reliable path).
2. **tty:** take from `ps` output.
3. **terminal_app:** climb the PPID chain until you find a process whose parent is `launchd` — that's the terminal app. Bucket its name into known terminals, else "Unknown".
4. **started_at:** derive from `etime` + now.

Cache the result for ~1.5 s — palette opens fire scans back-to-back and `ps`+`lsof` for ~5 processes is still cheap (~20 ms), but cache to be safe.

**Execute** — bring the terminal tab to front:

- **iTerm2:** AppleScript that enumerates windows → tabs → sessions, matches `tty`, then `select` the window + tab.
  ```applescript
  tell application "iTerm2"
    repeat with w in windows
      repeat with t in tabs of w
        repeat with s in sessions of t
          if tty of s is "TTY_HERE" then
            tell w to select
            select t
            activate
            return
          end if
        end repeat
      end repeat
    end repeat
  end tell
  ```
- **Terminal.app:** very similar, `tty` is a property of `tab`.
- **Warp:** AppleScript dict is thin — fallback to activating the app with `open -a Warp`.
- **Unknown:** fallback to activating whatever terminal_app is.

Wrap each in `osascript -e '...'` from Rust, return Ok on exit 0.

### Frontend

- New `kind: "agent"` in `PaletteEntry` union (both TS and Rust serde).
- Add a new row rendering: icon letter "A" (different color from the installed-app letter "A" — maybe teal); primary text = project name; subtitle = `cwd`; right side = `terminal_app` + uptime.
- `api.executeAgent(pid: number, tty: string)` → `execute_agent` Tauri command.
- The palette already merges arbitrary kinds, so `listPalette` just needs to call `agents::scan()` and concat.

### Edge cases

- Multiple `claude` in the same dir: keep both, differentiate by PID in the subtitle.
- `claude` running inside `tmux`: the tty we see is the tmux pane tty, not the terminal emulator tty. MVP: surface the entry anyway, but activation will only find the terminal app, not the pane. Phase 2: if we detect we're inside tmux, use `tmux select-window -t <target>`.
- SSH / remote: if `claude` is running on a remote host over SSH from one of your terminals, we won't see the remote PID. Not in scope.

### Build order

1. Scan + cwd + tty extraction in Rust (pure; add unit tests with mocked `ps` output).
2. Terminal-app detection (parent chain walk).
3. iTerm2 + Terminal.app activation.
4. Wire into `PaletteEntry` and the palette row renderer.
5. Warp + fallback.

---

## 2. Obsidian — search my vault from the palette

### Goal

Type a word and see matching Obsidian notes in the palette. Enter opens the note in Obsidian.

### Option A — local vault scan (MVP)

**User config:** in Preferences, a "Obsidian vault" path (default `~/Documents/Obsidian Vault`, otherwise blank → feature hidden).

**Backend** — `desktop/src-tauri/src/obsidian.rs`:

- On first use and on a timer (every 5 min or on palette open if older than 60 s), walk the vault, collect every `.md` file. For each:
  - `filename` (without extension)
  - `relative_path` (used in `obsidian://open?vault=...&file=...`)
  - `first_heading` (first `#` line, optional)
  - First ~400 chars of body (for search, not display)
  - `mtime` — for incremental re-indexing
- Cache on disk in `~/Library/Application Support/smallcast/cache/obsidian-<vault>.json`. Re-read on startup; refresh incrementally on mtime change.

**Palette entry:**

```ts
type NoteEntry = {
  kind: "note";
  vault: string;            // vault name (= folder basename)
  relative_path: string;    // "folder/subfolder/Note.md"
  title: string;            // filename without .md, or first heading if the filename is the date
  snippet: string;          // first non-empty body line, for the subtitle
};
```

Fuse.js keys add `title` (weight 3) and `snippet` (weight 0.5).

**Execute** — `open obsidian://open?vault=<url-encoded vault>&file=<url-encoded relative_path without .md>`.

### Option B — Obsidian MCP (later, when phase 3 MCP client lands)

When smallcast grows an MCP client (phase 3 work), wire in a community Obsidian MCP server (e.g. `mcp-obsidian`) as the search backend. Benefits:

- Live index (Obsidian already has one, no re-indexing from our side).
- Semantic search via Obsidian plugins if the server exposes it.
- Remote vaults (Obsidian Sync) without us touching iCloud.

Until then, stick with Option A.

### Edge cases

- Excluded folders: honor Obsidian's `.obsidian/app.json` `attachmentFolderPath` by skipping it; skip `.trash` and `.obsidian` by default.
- Huge vaults (>10k notes): Fuse gets slow. Pre-filter by substring on the query before Fuse; only Fuse-rank the top 200 hits.
- Symlinks: don't follow by default.

### Build order

1. Config field + Preferences UI.
2. Walk + index (ignore `.obsidian`, `.trash`).
3. Incremental re-index on mtime.
4. Palette entry kind + row.
5. `obsidian://` open action.

---

## Rough ordering vs existing roadmap

Existing `PLAN.md` has:

- Phase 2: Chrome extension + Cloudflare sync
- Phase 3: MCP + iCloud + auto-expansion + hotkey rebinding UI

Suggest inserting these two as the **new Phase 2 priority** (before Chrome extension), because:

- They extend the productivity surface on the machine I already use every day.
- Neither needs a network service or browser extension — both are local macOS work.
- They exercise the palette entry-kind system, which validates the same seams the Chrome extension will later consume.

Rechecks to do when picking up: confirm the `PaletteEntry` enum still lives in `commands.rs` (it does at `49382f4`), and that the `apps.rs` pattern is still a good template for `agents.rs` / `obsidian.rs`.
