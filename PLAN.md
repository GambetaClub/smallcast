# smallcast — Plan

A personal launcher with a global hotkey, focused on **snippets** and **quicklinks**. Local-first, syncable across devices via Cloudflare. Designed so an MCP server and Chrome extension can plug into the same store later.

## Scope

### MVP (phase 1 — build now)
1. macOS desktop app (Tauri) with global hotkey palette.
2. Two item types: **snippets** and **quicklinks**.
3. **Workspaces** from day 1 — items belong to a workspace (e.g. `personal`, `work`), user switches between them seamlessly.
4. Local JSON store in `~/Library/Application Support/smallcast/`.
5. Data model designed for sync — stable IDs, `updated_at`, tombstones — even though the sync engine isn't wired up yet.

### Phase 2
5. Chrome extension with the same palette UX, talking to a local HTTP API exposed by the desktop app.
6. Cloudflare Worker + KV (or D1) sync endpoint, so multiple devices share one store.

### Phase 3
7. MCP server (Node, stdio) exposing CRUD tools on the store.
8. iCloud Drive as an alternative Mac-to-Mac sync (cheap, no cloud infra).
9. Snippet auto-expansion (typing `;sig` → replaces with text) — needs a keystroke monitor.

Explicit non-goals: extensions/plugins, AI chat, clipboard history, window management, calendar/email integrations.

## Architecture

```
┌─────────────────────────────────────┐
│ ~/Library/Application Support/       │
│  smallcast/                          │
│    config.json                       │    { active, workspaces[] }
│    workspaces/                       │
│      personal/                       │
│        snippets.json                 │
│        quicklinks.json               │
│        sync_state.json               │
│      work/                           │
│        snippets.json                 │
│        quicklinks.json               │
│        sync_state.json               │
└─────────────┬───────────────────────┘
              │
     ┌────────┴─────────┐
     │  desktop app     │
     │  (Tauri)         │
     │  ─ global hotkey │
     │  ─ palette UI    │
     │  ─ HTTP API @    │
     │    :47123 [P2]   │◄──── Chrome extension [P2]
     │  ─ sync engine   │
     └────────┬─────────┘
              │
    ┌─────────┴──────────┐
    │ Cloudflare Worker  │ [P2]  cross-device sync
    │ + KV / D1          │
    └────────────────────┘
```

Desktop app is the only process that touches disk. Chrome extension and (eventually) MCP server go through the desktop app's local HTTP API — that keeps write ordering and file locking in one place, which matters once sync is live.

## Workspaces

**Model.** Each workspace is an isolated namespace of items with its own sync state. One is active at a time. First run creates `personal`. User can create/rename/delete workspaces and switch between them from the palette.

**On disk.** `config.json` at the app-support root lists workspaces and the active one. Each workspace is a folder under `workspaces/` containing its own `snippets.json`, `quicklinks.json`, and `sync_state.json`. No item carries a workspace field — membership is implicit in the path.

```json
// config.json
{
  "active_workspace": "personal",
  "workspaces": [
    { "id": "personal", "name": "Personal", "color": "#7bd" },
    { "id": "work",     "name": "Work",     "color": "#e97" }
  ]
}
```

**Switching.** Palette command "Switch workspace" + keyboard shortcuts inside the palette (`⌘1`…`⌘9`). Menu bar shows the active workspace name/color. Future: bind a distinct global hotkey per workspace (e.g. `⌥+Space` = personal, `⌥+⌘+Space` = work) so you land in the right context from the hotkey alone.

**Sync, later.** Each workspace syncs independently. That means you can point personal at Cloudflare and keep work local, or use different remotes per workspace. `sync_state.json` lives inside the workspace folder for exactly this reason.

**No cross-workspace sharing for now.** If demand emerges, add a `shared` pseudo-workspace whose items are merged into every search — not needed for MVP.

## Data model (sync-ready from day 1)

Every item has these fields so that phase 2 sync just bolts on:

```json
{
  "id": "uuid-v7",
  "kind": "snippet" | "quicklink",
  "name": "Email signature",
  "keyword": "sig",
  "created_at": "2026-04-24T08:00:00Z",
  "updated_at": "2026-04-24T08:00:00Z",
  "deleted": false,
  "rev": 1,
  ...type-specific fields...
}
```

Snippet-specific: `text`.
Quicklink-specific: `url`, `open_in` (`default_browser` / `chrome` / `safari`), optional `icon`.

**Sync design (for later, but shapes the data model now):**
- UUID v7 for IDs (time-ordered, sortable, no collisions across devices).
- `updated_at` drives last-write-wins conflict resolution — fine for 1 user × a few devices.
- `deleted: true` tombstones instead of hard deletes, so deletions propagate. GC'd after 30 days.
- `rev` increments on every change; cloud uses it to detect conflicts early.
- `sync_state.json` tracks `last_pulled_at` + `pending_changes` independently from the items themselves.

Storage format is plain JSON (not SQLite) — hand-editable, diffable, and makes the MCP server trivial later.

## Repo layout

```
smallcast/
  PLAN.md
  README.md
  .gitignore
  desktop/             # Tauri app (phase 1)
    src/               # React + TS frontend (palette UI)
    src-tauri/         # Rust backend (hotkey, store, future HTTP + sync)
  extension/           # Chrome extension (phase 2)
  worker/              # Cloudflare Worker (phase 2)
  mcp/                 # MCP server (phase 3)
  docs/
```

Keeping all surfaces in one repo while they're small — easier to change data model in lockstep. Split later if it grows.

## Build order (phase 1)

1. Scaffold Tauri app in `desktop/` with React + TypeScript.
2. Rust `core` module: item types, JSON-backed store with atomic writes, CRUD functions. Unit tests.
3. Tauri commands exposing `list`, `create`, `update`, `delete` to the frontend.
4. Palette UI: fuzzy search, keyboard navigation, enter-to-execute.
5. Global hotkey (`⌥+Space` default, rebindable) via `tauri-plugin-global-shortcut`.
6. Snippet action: copy to clipboard + simulate ⌘V into previously focused app (needs Accessibility permission).
7. Quicklink action: open URL, with a prompt for `{placeholder}` values.
8. Menu bar icon + launch-at-login.

## Decisions locked

- **Name:** smallcast.
- **Stack:** Tauri (Rust + React+TS).
- **Global hotkey default:** `⌥+Space`. Conflicts with Raycast if installed — user rebinds one of them.
- **Local storage:** JSON files in `~/Library/Application Support/smallcast/store/`.
- **Cloud target:** Cloudflare Worker + KV or D1. Chosen during phase 2 build — KV is simpler, D1 is better if the item count grows or we want server-side queries.
- **iCloud:** phase 3 additive option. Not sync-competitive with Cloudflare but zero-infra for Mac-to-Mac.
- **MCP:** phase 3. Will live in `mcp/` and go through the desktop app's HTTP API, not direct file access.

## Open questions (not blocking phase 1)

- Hotkey rebinding UX: dedicated settings window vs palette command?
- Quicklink `{placeholder}` prompt UI: inline in palette vs secondary modal?
- First-run: auto-migrate from a Raycast export if one exists at a known path?
