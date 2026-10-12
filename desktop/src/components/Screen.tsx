import { useEffect, useRef, type ReactNode } from "react";

type ScreenKind = "prefs" | "analytics" | "help" | "stats";

type Props = {
  kind: ScreenKind;
  title: string;
  topbarRight?: ReactNode;
  onClose: () => void;
  children: ReactNode;
};

const CLASS_BY_KIND: Record<ScreenKind, string> = {
  prefs: "prefs-inline",
  analytics: "analytics-inline",
  help: "help-inline",
  stats: "stats-inline",
};

/**
 * Common chrome for inline views (Preferences / Analytics / Help / Stats).
 *
 * Owns three things every inline view needs and previously hand-rolled:
 *  - The .palette wrapper class so they share the rounded/clipped shell
 *    with the main palette
 *  - Window-level Escape → onClose (works regardless of which child has
 *    focus; the per-view handlers used to miss when focus landed in an
 *    input or button)
 *  - Auto-focus the root on mount so keyboard nav works without a click
 *
 * The palette itself doesn't use this — it has too much custom keyboard
 * + selection logic — but every other inline view goes through here.
 */
export function Screen({ kind, title, topbarRight, onClose, children }: Props) {
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    rootRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className={`palette ${CLASS_BY_KIND[kind]}`}
      ref={rootRef}
      tabIndex={-1}
    >
      <div className="topbar">
        <div className="prefs-title" role="heading" aria-level={1}>{title}</div>
        <div className="topbar-spacer" />
        {topbarRight ?? <span className="topbar-hint">esc to close</span>}
      </div>
      <div className="prefs-scroll">{children}</div>
    </div>
  );
}
