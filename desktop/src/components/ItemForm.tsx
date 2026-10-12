import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { Item, OpenIn } from "../types";
import { isQuicklink, isSnippet } from "../types";

type Props = {
  initial?: Item;
  presetKind?: "snippet" | "quicklink";
  onDone: () => void;
  onCancel: () => void;
  onError: (msg: string) => void;
};

type Kind = "snippet" | "quicklink";

export function ItemForm({ initial, presetKind, onDone, onCancel, onError }: Props) {
  const editing = !!initial;
  const [kind, setKind] = useState<Kind>(
    initial ? (initial.kind as Kind) : (presetKind ?? "snippet")
  );
  const [name, setName] = useState(initial?.name ?? "");
  const [keyword, setKeyword] = useState(initial?.keyword ?? "");
  const [text, setText] = useState(
    initial && isSnippet(initial) ? initial.text : ""
  );
  const [sensitive, setSensitive] = useState(
    initial && isSnippet(initial) ? !!initial.sensitive : false
  );
  // For sensitive snippets the textarea is masked unless the user
  // opts to peek. New sensitive snippets start unmasked (you need to
  // see what you're typing); existing ones start masked.
  const [showSecret, setShowSecret] = useState(
    !(initial && isSnippet(initial) && initial.sensitive)
  );
  const [url, setUrl] = useState(
    initial && isQuicklink(initial) ? initial.url : ""
  );
  const [openIn, setOpenIn] = useState<OpenIn>(
    initial && isQuicklink(initial) ? initial.open_in : "default_browser"
  );
  const [saving, setSaving] = useState(false);
  const [pendingDelete, setPendingDelete] = useState(false);
  const nameRef = useRef<HTMLInputElement>(null);
  const deleteTimer = useRef<number | null>(null);

  useEffect(() => {
    nameRef.current?.focus();
  }, []);

  // Pre-fill the value field from the clipboard on create (never overwrite).
  useEffect(() => {
    if (editing) return;
    readText()
      .then((content) => {
        if (!content) return;
        if (kind === "snippet") {
          setText((t) => (t ? t : content));
        } else if (kind === "quicklink") {
          setUrl((u) => (u ? u : looksLikeUrl(content) ? content : u));
        }
      })
      .catch(() => {
        /* clipboard empty or unavailable — ignore */
      });
  }, [editing, kind]);

  async function save() {
    if (saving) return;
    if (!name.trim()) {
      onError("Name is required");
      return;
    }
    const kw = keyword.trim() || undefined;
    setSaving(true);
    try {
      if (kind === "snippet") {
        if (!text.trim()) {
          onError("Text is required for snippets");
          setSaving(false);
          return;
        }
        if (editing && initial && isSnippet(initial)) {
          await api.updateSnippet({
            id: initial.id,
            name,
            text,
            keyword: kw ?? null,
            sensitive,
          });
        } else {
          await api.createSnippet({ name, text, keyword: kw, sensitive });
        }
      } else {
        if (!url.trim()) {
          onError("URL is required for quicklinks");
          setSaving(false);
          return;
        }
        if (editing && initial && isQuicklink(initial)) {
          await api.updateQuicklink({
            id: initial.id,
            name,
            url,
            keyword: kw ?? null,
            open_in: openIn,
          });
        } else {
          await api.createQuicklink({
            name,
            url,
            keyword: kw,
            open_in: openIn,
          });
        }
      }
      onDone();
    } catch (e) {
      onError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function deleteItem() {
    if (!editing || !initial) return;
    if (!pendingDelete) {
      // Arm — wait for a second press.
      setPendingDelete(true);
      if (deleteTimer.current) window.clearTimeout(deleteTimer.current);
      deleteTimer.current = window.setTimeout(() => setPendingDelete(false), 4000);
      return;
    }
    // Confirmed.
    if (deleteTimer.current) window.clearTimeout(deleteTimer.current);
    setPendingDelete(false);
    try {
      if (isSnippet(initial)) await api.deleteSnippet(initial.id);
      else if (isQuicklink(initial)) await api.deleteQuicklink(initial.id);
      onDone();
    } catch (e) {
      onError(String(e));
    }
  }

  function handleKey(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      if (pendingDelete) {
        setPendingDelete(false);
        return;
      }
      onCancel();
      return;
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      save();
      return;
    }
    if (
      editing &&
      e.metaKey &&
      !e.ctrlKey &&
      !e.altKey &&
      (e.key === "Backspace" || e.key === "Delete")
    ) {
      e.preventDefault();
      deleteItem();
    }
  }

  return (
    <div className="palette" onKeyDown={handleKey}>
      <div className="form">
        <div className="form-header">
          <div className="form-title">{editing ? "Edit item" : "New item"}</div>
          {!editing && (
            <div className="tab-switch">
              <button
                className={kind === "snippet" ? "active" : ""}
                aria-pressed={kind === "snippet"}
                onClick={() => setKind("snippet")}
                type="button"
                tabIndex={-1}
              >
                Snippet
              </button>
              <button
                className={kind === "quicklink" ? "active" : ""}
                aria-pressed={kind === "quicklink"}
                onClick={() => setKind("quicklink")}
                type="button"
                tabIndex={-1}
              >
                Quicklink
              </button>
            </div>
          )}
        </div>

        <div className="form-body">
          <div className="form-field">
            <label htmlFor="form-name">Name</label>
            <input
              id="form-name"
              ref={nameRef}
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Email signature"
            />
          </div>

          {kind === "snippet" ? (
            <>
              <div className="form-field">
                <label htmlFor="form-text">
                  Text
                  {sensitive && (
                    <button
                      type="button"
                      className="link-btn"
                      onClick={() => setShowSecret((v) => !v)}
                      tabIndex={-1}
                    >
                      {showSecret ? "Hide" : "Peek"}
                    </button>
                  )}
                </label>
                <textarea
                  id="form-text"
                  value={text}
                  onChange={(e) => setText(e.target.value)}
                  placeholder="Paste content…"
                  spellCheck={!sensitive}
                  style={
                    sensitive && !showSecret
                      ? { WebkitTextSecurity: "disc", textSecurity: "disc" } as React.CSSProperties
                      : undefined
                  }
                />
              </div>
              <div className="form-field form-field-inline">
                <label htmlFor="form-sensitive">
                  Treat as sensitive
                  <span className="form-field-hint">
                    Hides the value in the palette list and masks it here.
                    Run / paste still works as normal.
                  </span>
                </label>
                <input
                  id="form-sensitive"
                  type="checkbox"
                  checked={sensitive}
                  onChange={(e) => {
                    setSensitive(e.target.checked);
                    // Switching to sensitive on an existing item should
                    // re-mask immediately; switching off reveals.
                    setShowSecret(!e.target.checked);
                  }}
                />
              </div>
            </>
          ) : (
            <>
              <div className="form-field">
                <label htmlFor="form-url">URL (use {"{placeholder}"} for arguments)</label>
                <input
                  id="form-url"
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  placeholder="https://github.com/search?q={query}"
                  spellCheck={false}
                />
              </div>
              <div className="form-field">
                <label htmlFor="form-open-in">Open in</label>
                <select
                  id="form-open-in"
                  value={openIn}
                  onChange={(e) => setOpenIn(e.target.value as OpenIn)}
                >
                  <option value="default_browser">Default browser</option>
                  <option value="chrome">Google Chrome</option>
                  <option value="safari">Safari</option>
                </select>
              </div>
            </>
          )}

          <div className="form-field">
            <label htmlFor="form-keyword">Keyword (optional)</label>
            <input
              id="form-keyword"
              value={keyword}
              onChange={(e) => setKeyword(e.target.value)}
              placeholder="e.g. sig"
              spellCheck={false}
            />
          </div>
        </div>

        <div className="footer">
          <span><kbd>⌘↵</kbd>Save</span>
          {editing && <span><kbd>⌘⌫</kbd>Delete</span>}
          <span><kbd>esc</kbd>Cancel</span>
          <div className="footer-spacer" />
          {saving && <span>Saving…</span>}
        </div>
      </div>
      {pendingDelete && initial && (
        <div className="confirm-banner">
          Delete <b>{initial.name}</b>? Press <kbd>⌘⌫</kbd> again to confirm,{" "}
          <kbd>esc</kbd> to cancel.
        </div>
      )}
    </div>
  );
}

function looksLikeUrl(s: string): boolean {
  return /^https?:\/\//i.test(s) || /^www\./i.test(s);
}
