import React from "react";
import {
  AbsoluteFill,
  Sequence,
  useCurrentFrame,
  useVideoConfig,
  spring,
  interpolate,
} from "remotion";

const COLORS = {
  bg: "#0F0E2E",
  bgGrad: "linear-gradient(135deg, #1E1B4B 0%, #4F46E5 100%)",
  card: "rgba(30,27,75,0.92)",
  border: "rgba(255,255,255,0.08)",
  accent: "#818CF8",
  white: "#FFFFFF",
  muted: "rgba(255,255,255,0.55)",
};

const FONT = "-apple-system, BlinkMacSystemFont, 'SF Pro Display', sans-serif";
const FONT_TERMINAL = "'VT323', Monaco, Menlo, monospace";
const FONT_PIXEL = "'Press Start 2P', 'VT323', Monaco, monospace";
const FONT_NERD = "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace";
const FONT_SERIF = "'New York', 'Times New Roman', Georgia, serif";
const FONT_COMIC = "'Comic Sans MS', 'Chalkboard SE', cursive";

// Snapshot of the in-app theme tokens (themes.rs) for the marketing
// page's themes carousel. Subset of what ships in the binary — the
// loudest ones, since the goal is "look at how weird this gets".
type ThemeTokens = {
  id: string;
  name: string;
  bg: string;
  card: string;        // surface inside the bg gradient
  border: string;
  fg: string;
  muted: string;
  accent: string;
  rowActive: string;
  badgeApp: string;
  badgeSnip: string;
  badgeLink: string;
  badgeCmd: string;
  font: string;
  shadow?: string;
};

const THEMES: ThemeTokens[] = [
  {
    id: "synthwave",
    name: "Synthwave '84",
    bg: "linear-gradient(180deg, #1a0033 0%, #22123c 60%, #3a1a5a 100%)",
    card: "rgba(34, 18, 60, 0.94)",
    border: "rgba(254, 106, 217, 0.30)",
    fg: "#f8f8ff",
    muted: "rgba(185, 163, 255, 0.7)",
    accent: "#fe6ad9",
    rowActive: "rgba(254, 106, 217, 0.20)",
    badgeApp: "#01cdfe",
    badgeSnip: "#05ffa1",
    badgeLink: "#fe6ad9",
    badgeCmd: "#fff95b",
    font: FONT_NERD,
    shadow: "0 0 0 1px rgba(254, 106, 217, 0.35), 0 30px 100px rgba(20, 0, 40, 0.7), 0 0 80px rgba(1, 205, 254, 0.20)",
  },
  {
    id: "vaporwave",
    name: "Vaporwave",
    bg: "linear-gradient(180deg, #ffe6f6 0%, #ffd1ec 100%)",
    card: "rgba(255, 230, 246, 0.96)",
    border: "rgba(255, 113, 206, 0.30)",
    fg: "#5a3070",
    muted: "rgba(180, 134, 196, 0.85)",
    accent: "#ff71ce",
    rowActive: "rgba(255, 113, 206, 0.18)",
    badgeApp: "#01cdfe",
    badgeSnip: "#05ffa1",
    badgeLink: "#b967ff",
    badgeCmd: "#fff95b",
    font: FONT,
  },
  {
    id: "gameboy",
    name: "Gameboy DMG",
    bg: "linear-gradient(180deg, #9bbc0f 0%, #8bac0f 100%)",
    card: "rgba(155, 188, 15, 0.98)",
    border: "rgba(15, 56, 15, 0.40)",
    fg: "#0f380f",
    muted: "rgba(48, 98, 48, 0.80)",
    accent: "#0f380f",
    rowActive: "rgba(15, 56, 15, 0.22)",
    badgeApp: "#306230",
    badgeSnip: "#0f380f",
    badgeLink: "#578a34",
    badgeCmd: "#0f380f",
    font: FONT_TERMINAL,
    shadow: "0 0 0 2px #0f380f, 0 0 0 6px #8bac0f, 0 0 0 8px #0f380f",
  },
  {
    id: "pixel",
    name: "Pixel (8-bit)",
    bg: "linear-gradient(180deg, #0c0c18 0%, #1a1228 100%)",
    card: "rgba(12, 12, 24, 0.96)",
    border: "rgba(255, 220, 90, 0.20)",
    fg: "#ffe9a8",
    muted: "rgba(154, 142, 88, 0.85)",
    accent: "#7be07b",
    rowActive: "rgba(255, 220, 90, 0.18)",
    badgeApp: "#5cd1ff",
    badgeSnip: "#7be07b",
    badgeLink: "#ff7adb",
    badgeCmd: "#fff066",
    font: FONT_PIXEL,
  },
  {
    id: "cyberpunk",
    name: "Cyberpunk",
    bg: "linear-gradient(180deg, #080514 0%, #14092a 100%)",
    card: "rgba(8, 5, 20, 0.96)",
    border: "rgba(252, 238, 10, 0.30)",
    fg: "#fcee0a",
    muted: "rgba(154, 144, 0, 0.85)",
    accent: "#00f0ff",
    rowActive: "rgba(255, 240, 0, 0.18)",
    badgeApp: "#00f0ff",
    badgeSnip: "#fcee0a",
    badgeLink: "#ff00aa",
    badgeCmd: "#fcee0a",
    font: FONT_NERD,
    shadow: "0 0 0 1px rgba(252, 238, 10, 0.45), 0 30px 100px rgba(0, 240, 255, 0.20)",
  },
  {
    id: "matrix",
    name: "Matrix",
    bg: "linear-gradient(180deg, #000000 0%, #001a08 100%)",
    card: "rgba(0, 0, 0, 0.98)",
    border: "rgba(0, 255, 65, 0.25)",
    fg: "#00ff41",
    muted: "rgba(0, 143, 23, 0.95)",
    accent: "#a8ff60",
    rowActive: "rgba(0, 255, 65, 0.16)",
    badgeApp: "#00ff41",
    badgeSnip: "#aaff60",
    badgeLink: "#00ffaa",
    badgeCmd: "#aaff44",
    font: FONT_TERMINAL,
    shadow: "0 0 0 1px rgba(0, 255, 65, 0.35), 0 0 100px rgba(0, 255, 65, 0.30)",
  },
  {
    id: "hot-dog-stand",
    name: "Hot Dog Stand",
    bg: "linear-gradient(180deg, #ffff00 0%, #ffdd00 100%)",
    card: "rgba(255, 255, 0, 0.98)",
    border: "rgba(255, 0, 0, 0.50)",
    fg: "#000000",
    muted: "rgba(170, 0, 0, 0.95)",
    accent: "#ff0000",
    rowActive: "rgba(255, 0, 0, 0.30)",
    badgeApp: "#000000",
    badgeSnip: "#ff0000",
    badgeLink: "#ff6600",
    badgeCmd: "#aa0000",
    font: FONT,
    shadow: "0 0 0 3px #ff0000, 0 0 0 6px #000000",
  },
  {
    id: "brutalist",
    name: "Brutalist",
    bg: "linear-gradient(180deg, #f4f4f4 0%, #ffffff 100%)",
    card: "#ffffff",
    border: "#000000",
    fg: "#000000",
    muted: "rgba(0, 0, 0, 0.55)",
    accent: "#000000",
    rowActive: "#000000",
    badgeApp: "#000000",
    badgeSnip: "#000000",
    badgeLink: "#000000",
    badgeCmd: "#000000",
    font: FONT_NERD,
    shadow: "12px 12px 0 0 #000000",
  },
  {
    id: "newsprint",
    name: "Newsprint",
    bg: "linear-gradient(180deg, #f4ecda 0%, #efe3cd 100%)",
    card: "rgba(244, 236, 218, 0.98)",
    border: "rgba(40, 30, 20, 0.20)",
    fg: "#1a1410",
    muted: "rgba(107, 94, 80, 0.95)",
    accent: "#7a3a1a",
    rowActive: "rgba(40, 30, 20, 0.10)",
    badgeApp: "#1a4a6a",
    badgeSnip: "#3a4a6a",
    badgeLink: "#5a2a6a",
    badgeCmd: "#7a4a1a",
    font: FONT_SERIF,
  },
  {
    id: "comic-sans",
    name: "Comic Sans (please don't)",
    bg: "linear-gradient(180deg, #fff5e6 0%, #fff0d6 100%)",
    card: "rgba(255, 245, 230, 0.98)",
    border: "rgba(70, 200, 220, 0.30)",
    fg: "#22334d",
    muted: "rgba(106, 138, 170, 0.85)",
    accent: "#22c8a8",
    rowActive: "rgba(70, 200, 220, 0.18)",
    badgeApp: "#3aa6ff",
    badgeSnip: "#22c8a8",
    badgeLink: "#c63aff",
    badgeCmd: "#ff9a3a",
    font: FONT_COMIC,
  },
];

const Beacon: React.FC<{ size: number; color?: string }> = ({
  size,
  color = "#fff",
}) => (
  <svg viewBox="0 0 200 200" width={size} height={size}>
    <circle cx="60" cy="100" r="18" fill={color} />
    <path
      d="M 92 68 Q 125 100 92 132"
      fill="none"
      stroke={color}
      strokeWidth="14"
      strokeLinecap="round"
    />
    <path
      d="M 128 48 Q 175 100 128 152"
      fill="none"
      stroke={color}
      strokeWidth="14"
      strokeLinecap="round"
    />
  </svg>
);

type Row = { kind: string; title: string; sub?: string; tint?: string };

const KIND_TINT: Record<string, string> = {
  App: "#60A5FA",
  Snip: "#34D399",
  Link: "#F472B6",
  Cmd: "#FBBF24",
};

const Palette: React.FC<{
  query: string;
  rows: Row[];
  selected: number;
  cursorVisible: boolean;
}> = ({ query, rows, selected, cursorVisible }) => (
  <div
    style={{
      width: 760,
      background: COLORS.card,
      border: `1px solid ${COLORS.border}`,
      borderRadius: 18,
      boxShadow:
        "0 30px 100px rgba(0,0,0,0.6), 0 0 0 1px rgba(255,255,255,0.04)",
      overflow: "hidden",
      fontFamily: FONT,
    }}
  >
    <div
      style={{
        padding: "22px 26px",
        borderBottom: `1px solid ${COLORS.border}`,
        display: "flex",
        alignItems: "center",
        gap: 16,
      }}
    >
      <div style={{ opacity: 0.7 }}>
        <Beacon size={24} color={COLORS.accent} />
      </div>
      <span
        style={{
          color: COLORS.white,
          fontSize: 24,
          fontWeight: 400,
          letterSpacing: -0.2,
        }}
      >
        {query}
        <span
          style={{
            color: COLORS.accent,
            opacity: cursorVisible ? 1 : 0,
            marginLeft: 1,
          }}
        >
          ▍
        </span>
      </span>
    </div>
    {rows.map((r, i) => (
      <div
        key={i}
        style={{
          padding: "14px 26px",
          display: "flex",
          alignItems: "center",
          gap: 14,
          background:
            i === selected ? "rgba(129,140,248,0.18)" : "transparent",
          borderLeft:
            i === selected
              ? `3px solid ${COLORS.accent}`
              : "3px solid transparent",
        }}
      >
        <div
          style={{
            width: 36,
            height: 36,
            borderRadius: 8,
            background: (r.tint ?? KIND_TINT[r.kind] ?? "#888") + "33",
            color: r.tint ?? KIND_TINT[r.kind] ?? "#fff",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 11,
            fontWeight: 700,
            letterSpacing: 0.5,
          }}
        >
          {r.kind}
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ color: COLORS.white, fontSize: 17 }}>{r.title}</div>
          {r.sub && (
            <div style={{ color: COLORS.muted, fontSize: 12, marginTop: 2 }}>
              {r.sub}
            </div>
          )}
        </div>
      </div>
    ))}
  </div>
);

const Keycap: React.FC<{ label: string; pressed: boolean; wide?: boolean }> = ({
  label,
  pressed,
  wide,
}) => (
  <div
    style={{
      minWidth: wide ? 240 : 120,
      height: 120,
      borderRadius: 20,
      border: `2px solid ${COLORS.border}`,
      background: pressed ? COLORS.accent : "rgba(255,255,255,0.06)",
      color: COLORS.white,
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      fontSize: 46,
      fontWeight: 500,
      fontFamily: FONT,
      boxShadow: pressed
        ? "0 2px 0 rgba(0,0,0,0.5)"
        : "0 8px 0 rgba(0,0,0,0.5)",
      transform: pressed ? "translateY(6px)" : "translateY(0)",
    }}
  >
    {label}
  </div>
);

const Intro: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const fadeIn = interpolate(frame, [0, 15], [0, 1], {
    extrapolateRight: "clamp",
  });
  const scale = spring({ frame, fps, config: { damping: 12 } });
  const wordOpacity = interpolate(frame, [12, 30], [0, 1], {
    extrapolateRight: "clamp",
  });
  return (
    <AbsoluteFill
      style={{
        background: COLORS.bgGrad,
        alignItems: "center",
        justifyContent: "center",
        flexDirection: "column",
        gap: 28,
      }}
    >
      <div style={{ transform: `scale(${scale})`, opacity: fadeIn }}>
        <Beacon size={240} />
      </div>
      <div
        style={{
          opacity: wordOpacity,
          color: COLORS.white,
          fontSize: 80,
          fontWeight: 600,
          letterSpacing: -2.5,
          fontFamily: FONT,
        }}
      >
        smallcast
      </div>
    </AbsoluteFill>
  );
};

const Hotkey: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const fadeIn = interpolate(frame, [0, 12], [0, 1], {
    extrapolateRight: "clamp",
  });
  const press = spring({
    frame: frame - 22,
    fps,
    config: { damping: 10, stiffness: 200 },
  });
  const pressed = press > 0.3 && frame < 55;
  return (
    <AbsoluteFill
      style={{
        background: COLORS.bg,
        alignItems: "center",
        justifyContent: "center",
        flexDirection: "column",
        gap: 44,
        opacity: fadeIn,
      }}
    >
      <div style={{ display: "flex", gap: 18, alignItems: "center" }}>
        <Keycap label="⌥" pressed={pressed} />
        <span style={{ color: COLORS.muted, fontSize: 38 }}>+</span>
        <Keycap label="Space" pressed={pressed} wide />
      </div>
      <div
        style={{
          color: COLORS.white,
          fontSize: 30,
          fontFamily: FONT,
          opacity: interpolate(frame, [8, 22], [0, 1], {
            extrapolateRight: "clamp",
          }),
        }}
      >
        Press <span style={{ color: COLORS.accent }}>⌥ Space</span> anywhere
      </div>
    </AbsoluteFill>
  );
};

const PaletteDemo: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const slide = spring({ frame, fps, config: { damping: 14 } });
  const slideY = interpolate(slide, [0, 1], [60, 0]);
  const opacity = interpolate(frame, [0, 12], [0, 1], {
    extrapolateRight: "clamp",
  });

  const typeStart = 24;
  const word = "tailwind";
  const len = Math.max(
    0,
    Math.min(word.length, Math.floor((frame - typeStart) / 5))
  );
  const query = word.slice(0, len);

  const cursorVisible = Math.floor(frame / 15) % 2 === 0;

  const initialRows: Row[] = [
    { kind: "App", title: "Visual Studio Code", sub: "application" },
    { kind: "Snip", title: "Email signature", sub: "snippet" },
    {
      kind: "Link",
      title: "GitHub Issues",
      sub: "quicklink · github.com/search?q={q}",
    },
    { kind: "Cmd", title: "Show Clipboard History", sub: "built-in command" },
  ];

  const filteredRows: Row[] = [
    {
      kind: "Link",
      title: "Tailwind Docs",
      sub: "quicklink · tailwindcss.com/docs?q={q}",
    },
    { kind: "App", title: "Tailscale", sub: "application" },
  ];

  const showFiltered = query.length >= 3;
  const rows = showFiltered ? filteredRows : initialRows;
  const selected = 0;

  const enterFrame = 100;
  const showPasted = frame >= enterFrame;
  const flash = interpolate(
    frame,
    [enterFrame, enterFrame + 4, enterFrame + 12],
    [0, 0.4, 0],
    { extrapolateRight: "clamp" }
  );

  return (
    <AbsoluteFill
      style={{
        background: COLORS.bg,
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div style={{ transform: `translateY(${slideY}px)`, opacity }}>
        <Palette
          query={query}
          rows={rows}
          selected={selected}
          cursorVisible={cursorVisible}
        />
      </div>
      <AbsoluteFill
        style={{
          background: "white",
          opacity: flash,
          pointerEvents: "none",
        }}
      />
      {showPasted && (
        <div
          style={{
            position: "absolute",
            bottom: 90,
            background: "rgba(34,197,94,0.18)",
            border: "1px solid rgba(34,197,94,0.4)",
            color: "#86efac",
            padding: "14px 22px",
            borderRadius: 12,
            fontFamily: FONT,
            fontSize: 18,
            opacity: interpolate(
              frame,
              [enterFrame, enterFrame + 10],
              [0, 1],
              { extrapolateRight: "clamp" }
            ),
          }}
        >
          → Opening tailwindcss.com/docs
        </div>
      )}
    </AbsoluteFill>
  );
};

// Themed copy of the palette card. Doesn't share state with <Palette> —
// the marketing version paints from a flat ThemeTokens record so each
// theme can override colors AND font-family without touching the demo.
const ThemedPalette: React.FC<{ theme: ThemeTokens }> = ({ theme }) => {
  const rows: Row[] = [
    { kind: "App", title: "Visual Studio Code", sub: "application" },
    { kind: "Snip", title: "Email signature", sub: "snippet" },
    { kind: "Link", title: "GitHub Issues", sub: "quicklink · github.com/search?q={q}" },
    { kind: "Cmd", title: "Show Clipboard History", sub: "built-in command" },
  ];
  const tints: Record<string, string> = {
    App: theme.badgeApp,
    Snip: theme.badgeSnip,
    Link: theme.badgeLink,
    Cmd: theme.badgeCmd,
  };
  return (
    <div
      style={{
        width: 760,
        background: theme.card,
        border: `1px solid ${theme.border}`,
        borderRadius: 18,
        boxShadow:
          theme.shadow ??
          "0 30px 100px rgba(0,0,0,0.6), 0 0 0 1px rgba(255,255,255,0.04)",
        overflow: "hidden",
        fontFamily: theme.font,
      }}
    >
      <div
        style={{
          padding: "22px 26px",
          borderBottom: `1px solid ${theme.border}`,
          display: "flex",
          alignItems: "center",
          gap: 16,
        }}
      >
        <div style={{ opacity: 0.85 }}>
          <Beacon size={24} color={theme.accent} />
        </div>
        <span
          style={{
            color: theme.fg,
            fontSize: 22,
            fontWeight: 400,
            letterSpacing: -0.2,
          }}
        >
          Search smallcast…
        </span>
      </div>
      {rows.map((r, i) => {
        const isSel = i === 0;
        return (
          <div
            key={i}
            style={{
              padding: "14px 26px",
              display: "flex",
              alignItems: "center",
              gap: 14,
              background: isSel ? theme.rowActive : "transparent",
              borderLeft: isSel
                ? `3px solid ${theme.accent}`
                : "3px solid transparent",
            }}
          >
            <div
              style={{
                width: 36,
                height: 36,
                borderRadius: 8,
                background: tints[r.kind] + "33",
                color: tints[r.kind],
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                fontSize: 11,
                fontWeight: 700,
                letterSpacing: 0.5,
                fontFamily: FONT,
              }}
            >
              {r.kind}
            </div>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ color: theme.fg, fontSize: 17 }}>{r.title}</div>
              {r.sub && (
                <div style={{ color: theme.muted, fontSize: 12, marginTop: 2 }}>
                  {r.sub}
                </div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
};

// Cycle through THEMES, holding each one for `holdFrames` and
// crossfading between them. The header label reads the active theme
// name. Background also fades between theme bgs.
const ThemesShowcase: React.FC = () => {
  const frame = useCurrentFrame();
  const fadeIn = interpolate(frame, [0, 12], [0, 1], {
    extrapolateRight: "clamp",
  });
  const holdFrames = 36; // ~1.2s per theme at 30fps
  const fadeFrames = 8;
  const total = THEMES.length * holdFrames;
  const t = Math.min(frame, total - 1);
  const idx = Math.min(THEMES.length - 1, Math.floor(t / holdFrames));
  const localFrame = t - idx * holdFrames;
  const next = idx < THEMES.length - 1 ? idx + 1 : idx;
  const blend = interpolate(
    localFrame,
    [holdFrames - fadeFrames, holdFrames - 1],
    [0, 1],
    { extrapolateRight: "clamp", extrapolateLeft: "clamp" }
  );
  const current = THEMES[idx];
  const upcoming = THEMES[next];

  // Subtle bg crossfade. Stack two bgs and tween their opacities; the
  // ThemedPalette card on top stays sharp.
  return (
    <AbsoluteFill style={{ opacity: fadeIn }}>
      <AbsoluteFill style={{ background: current.bg, opacity: 1 - blend }} />
      <AbsoluteFill style={{ background: upcoming.bg, opacity: blend }} />
      <AbsoluteFill
        style={{
          alignItems: "center",
          justifyContent: "center",
          flexDirection: "column",
          gap: 38,
        }}
      >
        <div
          style={{
            color: current.fg,
            fontSize: 28,
            fontFamily: current.font,
            opacity: 1 - blend,
            position: "absolute",
            top: 80,
            letterSpacing: 0.5,
            textShadow: "0 2px 12px rgba(0,0,0,0.35)",
          }}
        >
          {current.name}
        </div>
        <div
          style={{
            color: upcoming.fg,
            fontSize: 28,
            fontFamily: upcoming.font,
            opacity: blend,
            position: "absolute",
            top: 80,
            letterSpacing: 0.5,
            textShadow: "0 2px 12px rgba(0,0,0,0.35)",
          }}
        >
          {upcoming.name}
        </div>
        <div style={{ position: "relative", opacity: 1 - blend }}>
          <ThemedPalette theme={current} />
        </div>
        <div style={{ position: "absolute", opacity: blend }}>
          <ThemedPalette theme={upcoming} />
        </div>
        <div
          style={{
            position: "absolute",
            bottom: 80,
            color: current.fg,
            fontFamily: FONT,
            fontSize: 18,
            opacity: 0.7,
            letterSpacing: 1,
          }}
        >
          {idx + 1} / {THEMES.length} · {THEMES.length} built-in themes ship in the binary
        </div>
      </AbsoluteFill>
    </AbsoluteFill>
  );
};

const Outro: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const fadeIn = interpolate(frame, [0, 15], [0, 1], {
    extrapolateRight: "clamp",
  });
  const scale = spring({ frame, fps, config: { damping: 12 } });
  return (
    <AbsoluteFill
      style={{
        background: COLORS.bgGrad,
        alignItems: "center",
        justifyContent: "center",
        flexDirection: "column",
        gap: 22,
      }}
    >
      <div style={{ transform: `scale(${scale})`, opacity: fadeIn }}>
        <Beacon size={150} />
      </div>
      <div
        style={{
          color: COLORS.white,
          fontSize: 60,
          fontWeight: 600,
          letterSpacing: -1.8,
          fontFamily: FONT,
          opacity: fadeIn,
        }}
      >
        smallcast
      </div>
      <div
        style={{
          color: "rgba(255,255,255,0.75)",
          fontSize: 24,
          fontFamily: FONT,
          opacity: interpolate(frame, [15, 35], [0, 1], {
            extrapolateRight: "clamp",
          }),
        }}
      >
        your launcher · your data · your control
      </div>
    </AbsoluteFill>
  );
};

// Themes carousel runs THEMES.length × 36 frames (~1.2s each at 30fps).
const THEMES_DURATION = 10 * 36; // 360 frames — keep in sync with Root.tsx

export const Promo: React.FC = () => (
  <AbsoluteFill style={{ background: COLORS.bg }}>
    <Sequence durationInFrames={50}>
      <Intro />
    </Sequence>
    <Sequence from={50} durationInFrames={70}>
      <Hotkey />
    </Sequence>
    <Sequence from={120} durationInFrames={150}>
      <PaletteDemo />
    </Sequence>
    <Sequence from={270} durationInFrames={THEMES_DURATION}>
      <ThemesShowcase />
    </Sequence>
    <Sequence from={270 + THEMES_DURATION} durationInFrames={90}>
      <Outro />
    </Sequence>
  </AbsoluteFill>
);
