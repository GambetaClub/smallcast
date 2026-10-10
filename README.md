<p align="center">
  <img src="docs/assets/icon.png" alt="smallcast" width="128" height="128" />
</p>

<h1 align="center">smallcast</h1>

<p align="center">
  A smaller, keyboard-first launcher for macOS. Press <kbd>⌘ Space</kbd> anywhere, type, hit Enter.<br/>
  Snippets, quicklinks, apps, files, Claude CLI agents, Vite ports, Docker — all in one fuzzy pass.
</p>

> macOS-only (Ventura+). Personal fork for my own setup.

## Credits

smallcast is a fork of **[davidcast](https://github.com/davidbroza/davidcast)** by **[David Broza](https://github.com/davidbroza)**.
The palette, store, ranking, agents, Docker, Vite, clipboard, analytics, recommendations and backup are all his work.
This fork trims it down and changes file search. Go use the original if you want themes, workspaces or auto-updates.

## What's different from davidcast

**Removed**

- Themes, background overlays and the bundled fonts
- Workspaces and the workspace switcher
- Window management (`wm.*` commands)
- The `⌃⌥I` iTerm hotkey
- Confetti and laser effects
- Auto-updates and the Homebrew cask (build from source instead)

**Changed or added**

- Hotkey is `⌘ Space` instead of `⌥ Space`
- File search uses an `fsearch` whole-disk index instead of `fd`, hides noise (hidden folders, `~/Library`, build output, app bundles) and drops typo lookalikes
- The empty file list shows recent files from Desktop, Documents, Downloads, Pictures, Movies and iCloud Drive
- Drag a file row out of the palette into any app
- Finder shows up in the app list
- Only one copy can run at a time
- Optional stable code signing (`APPLE_SIGNING_IDENTITY`) so macOS permissions survive rebuilds

## Install

```bash
just install        # builds smallcast.app and puts it in /Applications
```

Then grant **System Settings → Privacy & Security → Accessibility → smallcast** to paste snippets at the cursor. Without it, snippets still land on the clipboard.

## What it does

| | |
|---|---|
| **Global palette** | `⌘ Space` anywhere. Single input, fuzzy search. |
| **Snippets** | Text stored on disk, pasted at cursor. `{placeholder}` args supported. Mark one `sensitive` to mask it. |
| **Quicklinks** | URL templates with `{arg}` substitution. Open in default browser, Chrome or Safari. |
| **App launcher** | Scans `/Applications`, `/System/Applications`, `~/Applications`, plus Finder. |
| **Claude CLI agents** | Finds each running agent's terminal tab; `↵` jumps back to it. Shows git branch and a dirty marker. |
| **Vite dev servers** | Lists listening Vite ports; `↵` opens the URL. |
| **Docker containers** | `↵` opens a shell, type `logs` for `docker logs -f`. |
| **File search** | Whole-disk search through `fsearch`. Query syntax: `:png`, `:img`, `:newest`. Image thumbnails, drag rows out. |
| **Find Screenshots** | Side preview. `↵` copies the path, `⌘⇧C` copies the bitmap, `⌘R` reveals in Finder. |
| **Clipboard history** | `⌘⇧V` opens it directly. |
| **GitHub** | Your assigned issues, issues and PRs. |
| **Skills browser** | Browse Claude Code `SKILL.md` files with a markdown preview. |
| **System actions and stats** | Lock, sleep, restart, empty trash; CPU, memory, disk, battery. |
| **Smart ranking** | Recents, kind priority, prefix bonus. Opt-in on-device recommendations. |
| **Local analytics** | `~/Library/Application Support/smallcast/analytics.jsonl`. Never leaves the machine. |
| **Git backup** | Push the store to a git remote you control (Preferences → Backup). |
| **Raycast import** | Import a Raycast JSON export in Preferences. |
| **Menu-bar only** | No dock icon. |

## Keybindings

| Key | Action |
|---|---|
| `↵` | Run the selected item |
| `↑` / `↓`, `⌃P` / `⌃N`, `⌃K` / `⌃J` | Navigate |
| `⌃U` | Clear the query |
| `⌘N` | New snippet / quicklink |
| `⌘E` | Edit the selected item |
| `⌘⌫` | Delete the selected item (press twice) |
| `⌘,` | Preferences |
| `⌘⇧V` | Clipboard history |
| `esc` | Close (or drop the active filter first) |

On file rows: `⌘↵` force-opens, `⌘C` copies the path, `⌘⇧C` copies the image, `⌘R` reveals in Finder.

## Storage

Everything is plain JSON under `~/Library/Application Support/smallcast/`:

```
config.json
workspaces/<id>/snippets.json
workspaces/<id>/quicklinks.json
analytics.jsonl
recommender_state.json     # only if Recommendations is on
.backup-git/               # only if git backup is set up
```

Writes are atomic and deletes are tombstones, so you can `cat`, diff and hand-edit any of it.

## Develop

Needs macOS Ventura+, Node 20+ with pnpm, Rust stable, `just`, and `fsearch` at `~/.local/bin/fsearch` for file search.

```bash
just dev
```

## License

The upstream repo has no license file. All original code is © David Broza.
