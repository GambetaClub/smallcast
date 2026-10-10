import type { ReactNode } from "react";
import { Screen } from "./Screen";

type Props = {
  onClose: () => void;
};

// One source of truth for what davidcast can do. When a new feature
// lands, add it here AND update the matching section in CLAUDE.md so
// the agent reading the repo knows to keep this in sync.
type Feature = {
  name: string;
  command?: string;
  shortcut?: ReactNode;
  description: ReactNode;
};

type Group = {
  title: string;
  blurb?: string;
  features: Feature[];
};

const GROUPS: Group[] = [
  {
    title: "Palette basics",
    blurb: "Press ⌘ Space anywhere to open it. Type to fuzzy-search across everything.",
    features: [
      {
        name: "Toggle palette",
        shortcut: <kbd>⌘ Space</kbd>,
        description: "Global hotkey. Hides on blur so it always feels like a launcher.",
      },
      {
        name: "Run / open selected",
        shortcut: <kbd>↵</kbd>,
        description: "Default action depends on the row kind — paste a snippet, open an app, follow a quicklink, etc.",
      },
      {
        name: "Navigate",
        shortcut: <><kbd>↑</kbd>/<kbd>↓</kbd> · <kbd>⌃N</kbd>/<kbd>⌃P</kbd></>,
        description: "Arrow keys or readline-style ⌃N / ⌃P.",
      },
      {
        name: "Edit selected",
        shortcut: <kbd>⌘E</kbd>,
        description: "Snippets and quicklinks open in the inline editor.",
      },
      {
        name: "Delete selected",
        shortcut: <kbd>⌘⌫</kbd>,
        description: "Two-press confirmation — second ⌘⌫ within 4s commits.",
      },
      {
        name: "Clear filter / close",
        shortcut: <kbd>esc</kbd>,
        description: "Drops the active filter chip first, then closes the palette.",
      },
    ],
  },
  {
    title: "Snippets & Quicklinks",
    features: [
      {
        name: "Create Snippet",
        command: "create.snippet",
        shortcut: <kbd>⌘N</kbd>,
        description: "Opens the inline form. Pre-fills text from the clipboard if present.",
      },
      {
        name: "Create Quicklink",
        command: "create.quicklink",
        description: "URLs with {placeholder} arguments are prompted at run time.",
      },
      {
        name: "Search Snippets",
        command: "search.snippets",
        description: "Filter the palette down to just snippets. Useful when the inline toggle is off.",
      },
      {
        name: "Search Quicklinks",
        command: "search.quicklinks",
        description: "Same idea for quicklinks.",
      },
    ],
  },
  {
    title: "Plugins",
    blurb: "Built-in surfaces that pull from the system.",
    features: [
      {
        name: "Apps",
        description: "All /Applications and ~/Applications launchers, with their real icons.",
      },
      {
        name: "Show Running Agents",
        command: "show.agents",
        description: "Running Claude CLI sessions — Enter activates the terminal tab.",
      },
      {
        name: "Show Vite Ports",
        command: "show.vite",
        description: "Live Vite dev servers detected from `lsof`. Enter opens the URL.",
      },
      {
        name: "Show Docker Containers",
        command: "show.docker",
        description: "Running containers — two rows each (shell + logs).",
      },
      {
        name: "Find Files",
        command: "files.find",
        description: "Searches your home folder (fsearch). Filter tokens: :png :img :newest. ⌘C path · ⌘R reveal · right-click copies path · drag a row into any app.",
      },
      {
        name: "Find Screenshots",
        command: "files.screenshots",
        description: "Newest screenshots first, side preview, ↵ copies path.",
      },
      {
        name: "Search Skills",
        command: "skills.search",
        description: "Browse Claude Code SKILL.md files (personal + plugin). Side preview, ↵ copies path, ⌘⇧C copies the whole skill.",
      },
      {
        name: "Show Clipboard History",
        command: "show.clipboard",
        shortcut: <kbd>⌘⇧V</kbd>,
        description: "Recent clipboard entries — Enter pastes, ⌘⌫ removes from history.",
      },
    ],
  },
  {
    title: "System",
    blurb: "Raycast-style quick actions. Destructive ones confirm first.",
    features: [
      {
        name: "Lock Screen",
        command: "system.lock",
        description: "Sends ⌃⌘Q (the macOS Lock Screen shortcut). Needs Accessibility.",
      },
      {
        name: "Sleep",
        command: "system.sleep",
        description: "pmset sleepnow — same as the Apple-menu Sleep item.",
      },
      {
        name: "Empty Trash",
        command: "system.empty_trash",
        description: "Confirms first, then tells Finder to empty the Trash.",
      },
      {
        name: "Restart",
        command: "system.restart",
        description: "Confirms, then asks System Events to restart. macOS still gates with its own dialog.",
      },
      {
        name: "Shut Down",
        command: "system.shut_down",
        description: "Confirms, then asks System Events to shut down.",
      },
      {
        name: "Log Out",
        command: "system.log_out",
        description: "Confirms, then asks System Events to log out of the current macOS user.",
      },
    ],
  },
  {
    title: "Other",
    features: [
      {
        name: "Open Preferences",
        command: "open.preferences",
        shortcut: <kbd>⌘,</kbd>,
        description: "Inline preferences view — autostart, search filters, plugins, import.",
      },
      {
        name: "Show Help",
        command: "help.show",
        description: "This screen.",
      },
      {
        name: "Show Analytics",
        command: "show.analytics",
        description: "Top queries, top items, success rate, daily activity. Reads the local JSONL — never leaves the box.",
      },
      {
        name: "Show System Stats",
        command: "show.stats",
        description: "CPU load, memory pressure, disk usage, battery, thermal status. Pure shell-outs (sysctl / vm_stat / df / pmset). Auto-refreshes every 2s while open.",
      },
      {
        name: "Check for Updates",
        command: "app.check_updates",
        description: "Pings the GitHub Releases endpoint and downloads + installs a newer signed bundle. Auto-runs on launch unless disabled in Preferences.",
      },
      {
        name: "Backup to git",
        description: "Push the entire store to a git remote you control — Preferences → Backup. Auto-syncs every 10 min when there are changes; manual Sync now / Pull / Force push buttons too. analytics.jsonl is excluded by default for privacy.",
      },
      {
        name: "Smart palette ranking",
        description: "Optional on-device recommender that learns from your usage history (frequency, recency, time-of-day, day-of-week). When on, the empty palette is reordered by what's most likely useful right now — most-relevant items float to the top, no separate section. Off by default — toggle in Preferences. Trains and runs locally; the model file lives next to your data, never leaves the box.",
      },
      {
        name: "Import from Raycast",
        description: "Preferences → Import. Auto-detects Raycast snippet/quicklink JSON exports.",
      },
    ],
  },
];

export function Help({ onClose }: Props) {
  return (
    <Screen kind="help" title="Help · davidcast" onClose={onClose}>
      <div className="help">
          {GROUPS.map((group) => (
            <section key={group.title} className="help-group">
              <h2>{group.title}</h2>
              {group.blurb && <p className="help-blurb">{group.blurb}</p>}
              <div className="help-features">
                {group.features.map((f) => (
                  <div key={f.name} className="help-feature">
                    <div className="help-feature-head">
                      <span className="help-feature-name">{f.name}</span>
                      {f.shortcut && (
                        <span className="help-feature-shortcut">{f.shortcut}</span>
                      )}
                      {f.command && (
                        <code className="help-feature-cmd">{f.command}</code>
                      )}
                    </div>
                    <div className="help-feature-desc">{f.description}</div>
                  </div>
                ))}
              </div>
            </section>
          ))}
          <div className="help-footer">
            davidcast — local-first launcher · ⌘ Space toggles the palette
          </div>
        </div>
    </Screen>
  );
}
