//! The palette's colour tokens. Token names mirror the CSS variables on
//! `body[data-view="palette"]` (`bg`, `fg`, `accent`, …); the frontend sets
//! them on `document.documentElement` at load.

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    /// CSS-variable name (without the leading `--`) → value.
    pub tokens: BTreeMap<String, String>,
}

const FONT_SYSTEM: &str = "-apple-system, BlinkMacSystemFont, \"SF Pro Text\", system-ui, sans-serif";
const FONT_MONO: &str = "ui-monospace, SFMono-Regular, Menlo, monospace";

macro_rules! tokens {
    ($($k:literal => $v:expr),* $(,)?) => {{
        let mut m = BTreeMap::new();
        $(m.insert($k.to_string(), $v.to_string());)*
        m
    }};
}

pub fn default_theme() -> Theme {
    Theme {
        id: "default".into(),
        name: "Default (Dark)".into(),
        tokens: tokens! {
            "bg" => "rgba(24, 24, 28, 0.92)",
            "bg-solid" => "#18181c",
            "bg-row" => "transparent",
            "bg-row-active" => "rgba(255, 255, 255, 0.08)",
            "fg" => "#e8e8ea",
            "fg-dim" => "#8a8a92",
            "fg-faint" => "#5a5a60",
            "border" => "rgba(255, 255, 255, 0.06)",
            "accent" => "#7bd88f",
            "danger" => "#ef6a6a",
            "badge-snippet" => "#6aaaef",
            "badge-quicklink" => "#c56aef",
            "badge-clipboard" => "#e0a960",
            "badge-agent" => "#d08ae0",
            "badge-vite" => "#f5e961",
            "badge-docker" => "#5ab8e0",
            "badge-logs" => "#8aa2b8",
            "shadow" => "0 28px 80px rgba(0, 0, 0, 0.55), 0 6px 18px rgba(0, 0, 0, 0.35)",
            "font-family" => FONT_SYSTEM,
            "font-family-mono" => FONT_MONO,
        },
    }
}
