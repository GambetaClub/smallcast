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

const FONT_SYSTEM: &str = "Inter, -apple-system, BlinkMacSystemFont, \"SF Pro Text\", system-ui, sans-serif";
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
        name: "Parrot".into(),
        tokens: tokens! {
            "font-family" => FONT_SYSTEM,
            "font-family-mono" => FONT_MONO,
        },
    }
}
