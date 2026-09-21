//! cloudy-ui palette and shared style helpers.
//!
//! Two independent axes. [`Palette`] is the named color identity: Catppuccin
//! Mocha (the original, and the shipped default) or Dracula
//! (<https://draculatheme.com>). [`Tier`] is the capability/background axis,
//! unchanged by which palette is active: `full` uses 24-bit RGB; `compatible`
//! uses the nearest xterm-256 index; `dark` is the active palette's `full`
//! values with [`bg`] swapped to true black — every other color function reads
//! `dark` as `full` ([`pick`]), so it is a per-palette override, not a third
//! palette or a third color column to keep in sync.
//! Every color in the TUI comes from this module — raw `Color::Rgb` or raw index
//! values anywhere else are a bug.
//!
//! # Initialization
//!
//! Call [`init`] once before the TUI starts to seed the tier (CLI flag or
//! config file, else auto-detect) and the palette (CLI flag or config file,
//! else Catppuccin — nothing about a terminal implies a color IDENTITY the way
//! `$COLORTERM` implies a color DEPTH, so palette is never auto-detected). The
//! Config tab can later [`set_tier`] / [`set_palette`] live — both holders are
//! atomics, so a re-selection re-renders on the next frame without a process
//! restart. Renders read them via the accessor fns below.

use std::sync::atomic::{AtomicU8, Ordering};

use ratatui::style::{Color, Modifier, Style};

// ── Tier ──────────────────────────────────────────────────────────────────────

/// Color-depth capability tier. `full` = 24-bit RGB; `compatible` = xterm-256.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tier {
    /// 24-bit truecolor. Requires `$COLORTERM=truecolor|24bit` or an explicit
    /// CLI / config override.
    Full,
    /// Nearest xterm-256 palette index. Safe on any xterm-compatible terminal.
    Compatible,
    /// `Full`, background swapped to true black (see the module doc). An
    /// explicit CLI / config choice only — auto-detect never picks it, since
    /// `$COLORTERM` says nothing about a background preference.
    Dark,
}

impl Tier {
    /// Stable atomic encoding. `0` doubles as "uninitialized" so the accessor
    /// can fall back to auto-detect before [`init`] runs.
    fn as_code(self) -> u8 {
        match self {
            Tier::Full => 1,
            Tier::Compatible => 2,
            Tier::Dark => 3,
        }
    }

    fn from_code(code: u8) -> Option<Tier> {
        match code {
            1 => Some(Tier::Full),
            2 => Some(Tier::Compatible),
            3 => Some(Tier::Dark),
            _ => None,
        }
    }
}

/// Process-global tier as an atomic code (`0` = unset → auto-detect). Seeded by
/// [`init`] and swappable at runtime via [`set_tier`] for the live theme picker.
static TIER: AtomicU8 = AtomicU8::new(0);

/// Detect the tier from `$COLORTERM` per the cloudy-tui contract:
/// `truecolor` or `24bit` → [`Tier::Full`]; anything else → [`Tier::Compatible`].
pub(crate) fn detect() -> Tier {
    match std::env::var("COLORTERM")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "truecolor" | "24bit" => Tier::Full,
        _ => Tier::Compatible,
    }
}

/// Seed the process tier and palette at startup.
/// Tier precedence (highest first): explicit override → auto-detect.
/// Palette precedence: explicit override → Catppuccin (never auto-detected —
/// see the module doc).
pub(crate) fn init(override_tier: Option<Tier>, override_palette: Option<Palette>) {
    set_tier(override_tier.unwrap_or_else(detect));
    set_palette(override_palette.unwrap_or(Palette::Catppuccin));
}

/// Swap the active tier at runtime. The next render reads the new value, so the
/// Config tab's theme selector applies immediately.
pub(crate) fn set_tier(tier: Tier) {
    TIER.store(tier.as_code(), Ordering::Relaxed);
}

/// Return the active tier. Falls back to auto-detect if [`init`] was not called.
#[inline]
pub(crate) fn tier() -> Tier {
    Tier::from_code(TIER.load(Ordering::Relaxed)).unwrap_or_else(detect)
}

/// Serializes tests that pin the tier. Every `tests/inline/*.rs` module compiles
/// into the one bin target, so under `cargo test` they run as threads sharing
/// this `TIER`. `testutil::TierSandbox` acquires it as an RAII guard.
#[cfg(test)]
pub(crate) static TIER_TEST_LOCK: crate::lockorder::RankedMutex<
    (),
    crate::lockorder::rank::TierTest,
> = crate::lockorder::RankedMutex::new(());

/// Read the stored pin, `None` for unset. [`tier`] collapses unset into a
/// detected tier, which a restore would then write back as a real pin.
#[cfg(test)]
pub(crate) fn tier_override() -> Option<Tier> {
    Tier::from_code(TIER.load(Ordering::Relaxed))
}

/// Put back a [`tier_override`] reading.
#[cfg(test)]
pub(crate) fn restore_tier(snapshot: Option<Tier>) {
    TIER.store(snapshot.map_or(0, Tier::as_code), Ordering::Relaxed);
}

// ── Palette ───────────────────────────────────────────────────────────────────

/// Which named color identity is active. Orthogonal to [`Tier`]: every
/// palette carries its own Full/Compatible pair for each color below, and
/// `Tier::Dark` reads as a per-palette background override (see the module
/// doc), not a third palette to maintain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Palette {
    /// The original palette, and the shipped default.
    Catppuccin,
    /// <https://draculatheme.com>, verified against the upstream spec
    /// (`dracula/dracula-theme`'s README table and the `dracula/alacritty`
    /// port's ANSI colors) rather than assumed from memory.
    Dracula,
}

impl Palette {
    /// Stable atomic encoding. `0` doubles as "uninitialized" so the accessor
    /// can fall back to the shipped default before [`init`] runs.
    fn as_code(self) -> u8 {
        match self {
            Palette::Catppuccin => 1,
            Palette::Dracula => 2,
        }
    }

    fn from_code(code: u8) -> Option<Palette> {
        match code {
            1 => Some(Palette::Catppuccin),
            2 => Some(Palette::Dracula),
            _ => None,
        }
    }
}

/// Process-global palette as an atomic code (`0` = unset → Catppuccin).
/// Seeded by [`init`] and swappable at runtime via [`set_palette`] for the
/// live theme picker.
static PALETTE: AtomicU8 = AtomicU8::new(0);

/// Swap the active palette at runtime. The next render reads the new value, so
/// the Config tab's palette selector applies immediately.
pub(crate) fn set_palette(palette: Palette) {
    PALETTE.store(palette.as_code(), Ordering::Relaxed);
}

/// Return the active palette. Falls back to Catppuccin if [`init`] was not
/// called.
#[inline]
pub(crate) fn palette() -> Palette {
    Palette::from_code(PALETTE.load(Ordering::Relaxed)).unwrap_or(Palette::Catppuccin)
}

/// Serializes tests that pin the palette — [`TIER_TEST_LOCK`]'s sibling, a
/// SEPARATE rank rather than the same lock: a future test pinning both must
/// acquire them in one fixed order (`TierTest` then `PaletteTest`) instead of
/// risking a same-thread re-entrant deadlock on a shared lock, or an inverted
/// order across two tests racing each other.
#[cfg(test)]
pub(crate) static PALETTE_TEST_LOCK: crate::lockorder::RankedMutex<
    (),
    crate::lockorder::rank::PaletteTest,
> = crate::lockorder::RankedMutex::new(());

/// Read the stored pin, `None` for unset. [`palette`] collapses unset into
/// Catppuccin, which a restore would then write back as a real pin.
#[cfg(test)]
pub(crate) fn palette_override() -> Option<Palette> {
    Palette::from_code(PALETTE.load(Ordering::Relaxed))
}

/// Put back a [`palette_override`] reading.
#[cfg(test)]
pub(crate) fn restore_palette(snapshot: Option<Palette>) {
    PALETTE.store(snapshot.map_or(0, Palette::as_code), Ordering::Relaxed);
}

// ── Palette tables ────────────────────────────────────────────────────────────
//
// Each color function matches on `palette()` first, `pick()` (full vs
// compatible) second. Each arm: (full: Color::Rgb, compatible:
// Color::Indexed(xterm-256)). The xterm-256 index is the nearest xterm-256
// cube/grayscale match to the RGB value — except `bg_danger_color` /
// `bg_warning_color`, where the nearest match is a colorless gray that would
// render no banner tint at all; those keep the nearest match that still
// carries the hue (Catppuccin's own 52/58 are exactly that, verified by
// re-deriving them; Dracula's derived tints land close enough to Catppuccin's
// own red/yellow wash to reuse the same two indices rather than compute a
// worse, off-hue pair).

#[inline]
fn pick(full: Color, compatible: Color) -> Color {
    match tier() {
        Tier::Full | Tier::Dark => full,
        Tier::Compatible => compatible,
    }
}

// ── Surfaces ──────────────────────────────────────────────────────────────────
#[inline]
pub(crate) fn bg() -> Color {
    if tier() == Tier::Dark {
        return Color::Rgb(0, 0, 0);
    }
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(30, 30, 46), Color::Indexed(235)),
        Palette::Dracula => pick(Color::Rgb(40, 42, 54), Color::Indexed(236)),
    }
}
#[inline]
pub(crate) fn bg_sunken() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(17, 17, 27), Color::Indexed(233)),
        Palette::Dracula => pick(Color::Rgb(33, 34, 44), Color::Indexed(235)),
    }
}
#[inline]
pub(crate) fn bg_hover() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(40, 40, 56), Color::Indexed(236)),
        Palette::Dracula => pick(Color::Rgb(47, 49, 63), Color::Indexed(237)),
    }
}

// ── Lines ─────────────────────────────────────────────────────────────────────
#[inline]
pub(crate) fn line_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(49, 50, 68), Color::Indexed(238)),
        Palette::Dracula => pick(Color::Rgb(54, 56, 72), Color::Indexed(238)),
    }
}
#[inline]
pub(crate) fn line_strong_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(69, 71, 90), Color::Indexed(240)),
        // Dracula's own "Current Line / Selection" — the same RGB Catppuccin's
        // own line_strong_color coincidentally lands on almost exactly.
        Palette::Dracula => pick(Color::Rgb(68, 71, 90), Color::Indexed(239)),
    }
}

// ── Text ──────────────────────────────────────────────────────────────────────
#[inline]
pub(crate) fn text_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(205, 214, 244), Color::Indexed(189)),
        Palette::Dracula => pick(Color::Rgb(248, 248, 242), Color::Indexed(255)),
    }
}
#[inline]
pub(crate) fn text_dim_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(166, 173, 200), Color::Indexed(145)),
        Palette::Dracula => pick(Color::Rgb(173, 181, 203), Color::Indexed(146)),
    }
}
#[inline]
pub(crate) fn text_faint_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(127, 132, 156), Color::Indexed(102)),
        // Dracula's own "Comment" — same UX role: de-emphasized, secondary text.
        Palette::Dracula => pick(Color::Rgb(98, 114, 164), Color::Indexed(61)),
    }
}

// ── Accents ───────────────────────────────────────────────────────────────────
/// Sapphire (Catppuccin) / Purple (Dracula) primary — the cool accent that
/// carries the UI.
#[inline]
pub(crate) fn accent_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(67, 171, 229), Color::Indexed(75)),
        Palette::Dracula => pick(Color::Rgb(189, 147, 249), Color::Indexed(141)),
    }
}
/// Claude orange (Catppuccin) / Dracula's own Orange — the warm secondary;
/// cloudy-ui rule "once per screen max".
#[inline]
pub(crate) fn accent_2_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(217, 119, 87), Color::Indexed(173)),
        Palette::Dracula => pick(Color::Rgb(255, 184, 108), Color::Indexed(215)),
    }
}

// ── Semantic ──────────────────────────────────────────────────────────────────
#[inline]
pub(crate) fn success_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(166, 227, 161), Color::Indexed(151)),
        Palette::Dracula => pick(Color::Rgb(80, 250, 123), Color::Indexed(84)),
    }
}
#[inline]
pub(crate) fn warning_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(249, 226, 175), Color::Indexed(223)),
        Palette::Dracula => pick(Color::Rgb(241, 250, 140), Color::Indexed(228)),
    }
}
#[inline]
pub(crate) fn danger_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(243, 139, 168), Color::Indexed(211)),
        Palette::Dracula => pick(Color::Rgb(255, 85, 85), Color::Indexed(203)),
    }
}
#[inline]
pub(crate) fn info_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(116, 199, 236), Color::Indexed(117)),
        Palette::Dracula => pick(Color::Rgb(139, 233, 253), Color::Indexed(117)),
    }
}

// ── Banner background tints ───────────────────────────────────────────────────
/// DANGER wash blended into BG — banner background for critical conditions.
#[inline]
pub(crate) fn bg_danger_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(75, 35, 44), Color::Indexed(52)),
        Palette::Dracula => pick(Color::Rgb(87, 51, 61), Color::Indexed(52)),
    }
}
/// WARNING wash blended into BG — muted warm-amber background for warning rows.
#[inline]
pub(crate) fn bg_warning_color() -> Color {
    match palette() {
        Palette::Catppuccin => pick(Color::Rgb(74, 60, 33), Color::Indexed(58)),
        Palette::Dracula => pick(Color::Rgb(84, 88, 73), Color::Indexed(58)),
    }
}

/// Per-channel RGB blend of `over` onto `beneath`, weighted by `alpha`
/// (the weight of `over`, clamped to `0.0..=1.0`).
/// Blends only on the full truecolor tier with both colors RGB-resolvable;
/// otherwise returns `over` unchanged.
pub(crate) fn blend_over(beneath: Color, over: Color, alpha: f64) -> Color {
    let (Color::Rgb(br, bg, bb), Color::Rgb(or, og, ob)) = (beneath, over) else {
        return over;
    };
    if !matches!(tier(), Tier::Full | Tier::Dark) {
        return over;
    }
    let a = alpha.clamp(0.0, 1.0);
    let mix = |o: u8, b: u8| -> u8 { (a * f64::from(o) + (1.0 - a) * f64::from(b)).round() as u8 };
    Color::Rgb(mix(or, br), mix(og, bg), mix(ob, bb))
}

// ── Toggle glyphs (tier-sensitive) ────────────────────────────────────────────

/// Toggle switch in the **on** state.
/// `full`: `─●`  `compatible`: `[on]`
pub(crate) fn toggle_on() -> &'static str {
    match tier() {
        Tier::Full | Tier::Dark => "─●",
        Tier::Compatible => "[on]",
    }
}

/// Toggle switch in the **off** state.
/// `full`: `○─`  `compatible`: `[off]`
pub(crate) fn toggle_off() -> &'static str {
    match tier() {
        Tier::Full | Tier::Dark => "○─",
        Tier::Compatible => "[off]",
    }
}

/// Gutter glyph for a row in edit mode — replaces the `❯` selection caret while
/// a text/stepper field is being typed into. Same on both tiers (per cloudy-tui).
pub(crate) fn edit_glyph() -> &'static str {
    "✎"
}

// ── Style helpers ─────────────────────────────────────────────────────────────

pub(crate) fn base() -> Style {
    Style::default().fg(text_color()).bg(bg())
}

/// Plain body text — foreground only.
pub(crate) fn body() -> Style {
    Style::default().fg(text_color())
}

/// Hairline chrome — tooltip `└ ` leaders and borders at `line_color()`.
pub(crate) fn line() -> Style {
    Style::default().fg(line_color())
}

/// Stronger line color — empty-gauge track and structural fills above `line_color()`.
pub(crate) fn line_strong() -> Style {
    Style::default().fg(line_strong_color())
}

pub(crate) fn dim() -> Style {
    Style::default().fg(text_dim_color())
}

pub(crate) fn faint() -> Style {
    Style::default().fg(text_faint_color())
}

/// Eyebrow label — bold + dim per cloudy-ui's CLI mapping.
pub(crate) fn label() -> Style {
    Style::default()
        .fg(text_dim_color())
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn accent() -> Style {
    Style::default().fg(accent_color())
}

pub(crate) fn warning() -> Style {
    Style::default().fg(warning_color())
}

pub(crate) fn danger() -> Style {
    Style::default().fg(danger_color())
}

/// Background for the selected list row.
pub(crate) fn selected_row() -> Style {
    Style::default().bg(bg_hover())
}

/// Utilization color: dim <60%, warning 60–80%, danger >80%.
pub(crate) fn util_color(pct: f64) -> Color {
    let pct = pct.clamp(0.0, 100.0);
    if pct >= 80.0 {
        danger_color()
    } else if pct >= 60.0 {
        warning_color()
    } else {
        text_dim_color()
    }
}

/// `util_color` as a ready-to-use foreground style.
pub(crate) fn util(pct: f64) -> Style {
    Style::default().fg(util_color(pct))
}

/// Info accent (sapphire / cyan); spinner color for refresh ops.
pub(crate) fn info() -> Style {
    Style::default().fg(info_color())
}

/// Success tint (green in both palettes); spinner color for auto-start.
pub(crate) fn success() -> Style {
    Style::default().fg(success_color())
}

#[cfg(test)]
#[path = "../../tests/inline/tui_theme.rs"]
mod tests;
