import { describe, expect, it } from "vitest";
import {
  asItem,
  extractPlaceholders,
  isAgent,
  isApp,
  isClipboard,
  isCommand,
  isDocker,
  isFile,
  isQuicklink,
  isSkill,
  isSnippet,
  isVite,
  type PaletteEntry,
} from "./types";

describe("extractPlaceholders", () => {
  it("returns each unique placeholder name in order", () => {
    expect(
      extractPlaceholders("https://example.com/{q}/{lang}/{q}"),
    ).toEqual(["q", "lang"]);
  });
  it("returns empty array when there are no placeholders", () => {
    expect(extractPlaceholders("https://example.com")).toEqual([]);
  });
  it("ignores escaped or unbalanced braces", () => {
    expect(extractPlaceholders("https://example.com/{")).toEqual([]);
    expect(extractPlaceholders("https://example.com/}")).toEqual([]);
  });
  it("handles multi-character placeholder names", () => {
    expect(
      extractPlaceholders("https://github.com/search?q={query}&type={kind}"),
    ).toEqual(["query", "kind"]);
  });
});

const command: PaletteEntry = {
  kind: "command",
  id: "create.snippet",
  name: "Create Snippet",
  subtitle: "New snippet in this workspace",
};

const snippet: PaletteEntry = {
  kind: "snippet",
  id: "01941d-snippet",
  name: "Email signature",
  text: "Best,\nDavid",
  created_at: "2026-04-26T09:00:00Z",
  updated_at: "2026-04-26T09:00:00Z",
};

const quicklink: PaletteEntry = {
  kind: "quicklink",
  id: "01941d-quicklink",
  name: "GitHub search",
  url: "https://github.com/search?q={q}",
  open_in: "default_browser",
  created_at: "2026-04-26T09:00:00Z",
  updated_at: "2026-04-26T09:00:00Z",
};

const app: PaletteEntry = {
  kind: "app",
  name: "iTerm",
  path: "/Applications/iTerm.app",
};

describe("type guards", () => {
  it("only accept their own kind", () => {
    // Each guard must accept its kind and reject another kind.
    const cases: Array<[(e: PaletteEntry) => boolean, PaletteEntry, PaletteEntry]> = [
      [isCommand, command, snippet],
      [isSnippet, snippet, quicklink],
      [isQuicklink, quicklink, app],
      [isApp, app, snippet],
      [isAgent, { kind: "agent" } as PaletteEntry, app],
      [isVite, { kind: "vite" } as PaletteEntry, app],
      [isDocker, { kind: "docker" } as PaletteEntry, app],
      [isFile, { kind: "file" } as PaletteEntry, app],
      [isClipboard, { kind: "clipboard" } as PaletteEntry, app],
      [isSkill, { kind: "skill" } as PaletteEntry, app],
    ];
    for (const [guard, accept, reject] of cases) {
      expect(guard(accept)).toBe(true);
      expect(guard(reject)).toBe(false);
    }
  });
});

describe("asItem", () => {
  it("returns snippets and quicklinks as Items", () => {
    expect(asItem(snippet)).toMatchObject({ kind: "snippet", name: "Email signature" });
    expect(asItem(quicklink)).toMatchObject({ kind: "quicklink", name: "GitHub search" });
  });
  it("returns null for non-item entries", () => {
    expect(asItem(command)).toBeNull();
    expect(asItem(app)).toBeNull();
  });
});
