//! `Tier::Dark`: full's exact palette, background swapped to true black. Pins
//! the claim the module doc makes — every OTHER color function reads `Dark`
//! identically to `Full` — so a future palette change to one but not the
//! other is caught here rather than read live in the TUI.

use super::*;
use crate::testutil::TierSandbox;

#[test]
fn dark_paints_a_true_black_background() {
    let _tier = TierSandbox::new(Tier::Dark);
    assert_eq!(bg(), Color::Rgb(0, 0, 0));
}

/// `full`'s background stays its own slate — the addition of `dark` must not
/// have accidentally changed what `full` itself renders.
#[test]
fn full_still_renders_its_own_background() {
    let _tier = TierSandbox::new(Tier::Full);
    assert_eq!(bg(), Color::Rgb(30, 30, 46));
}

#[test]
fn dark_matches_full_for_every_color_but_the_background() {
    let full = {
        let _tier = TierSandbox::new(Tier::Full);
        (
            text_color(),
            text_dim_color(),
            accent_color(),
            accent_2_color(),
            danger_color(),
            success_color(),
            warning_color(),
            bg_hover(),
            bg_sunken(),
            line_color(),
        )
    };
    let dark = {
        let _tier = TierSandbox::new(Tier::Dark);
        (
            text_color(),
            text_dim_color(),
            accent_color(),
            accent_2_color(),
            danger_color(),
            success_color(),
            warning_color(),
            bg_hover(),
            bg_sunken(),
            line_color(),
        )
    };
    assert_eq!(dark, full, "dark must equal full in everything but bg()");
}

/// `dark` has full's truecolor capability, so it gets full's glyphs, not
/// compatible's bracket fallback — the fallback is a capability concern
/// (does this terminal render Unicode/24-bit color at all), and dark's whole
/// premise is that it does.
#[test]
fn dark_uses_fulls_glyphs_not_compatibles() {
    let full = {
        let _tier = TierSandbox::new(Tier::Full);
        (toggle_on(), toggle_off())
    };
    let dark = {
        let _tier = TierSandbox::new(Tier::Dark);
        (toggle_on(), toggle_off())
    };
    assert_eq!(dark, full);

    let _tier = TierSandbox::new(Tier::Compatible);
    assert_eq!(
        toggle_on(),
        "[on]",
        "compatible is unaffected by dark's addition"
    );
}

#[test]
fn dark_gets_the_same_truecolor_blend_full_does() {
    let beneath = Color::Rgb(0, 0, 0);
    let over = Color::Rgb(200, 0, 0);
    // Each sandbox must drop before the next is acquired: shadowing `_tier`
    // hides the old binding but does not run its `Drop` early, so two block
    // scopes are load-bearing here, not just tidiness.
    let full_blend = {
        let _tier = TierSandbox::new(Tier::Full);
        blend_over(beneath, over, 0.5)
    };
    let dark_blend = {
        let _tier = TierSandbox::new(Tier::Dark);
        blend_over(beneath, over, 0.5)
    };
    assert_eq!(dark_blend, full_blend);
}

// ── Palette ───────────────────────────────────────────────────────────────────
//
// Nesting order matters: `lockorder::rank` pins `TierTest` (40) outermost of
// the two, `PaletteTest` (41) inner — every test below that needs both
// acquires `TierSandbox` first, `PaletteSandbox` second, or the ranked mutex
// panics on the inversion.

use crate::testutil::PaletteSandbox;

/// Unset (`init` never called) falls back to Catppuccin, the shipped default —
/// not a detected value, since nothing about a terminal implies a color
/// IDENTITY the way `$COLORTERM` implies a color DEPTH.
#[test]
fn unset_palette_defaults_to_catppuccin() {
    let guard = PALETTE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let prev = palette_override();
    restore_palette(None);
    assert_eq!(palette(), Palette::Catppuccin);
    restore_palette(prev);
    drop(guard);
}

/// Dracula is a genuinely different color identity, not a Catppuccin re-skin —
/// pinned per slot so a future edit that accidentally copies one Catppuccin
/// value into the Dracula arm is caught at that slot, not read as "some
/// difference somewhere".
#[test]
fn dracula_renders_different_colors_than_catppuccin() {
    let _tier = TierSandbox::new(Tier::Full);
    let catppuccin = {
        let _palette = PaletteSandbox::new(Palette::Catppuccin);
        (
            bg(),
            text_color(),
            accent_color(),
            accent_2_color(),
            success_color(),
            warning_color(),
            danger_color(),
            info_color(),
        )
    };
    let dracula = {
        let _palette = PaletteSandbox::new(Palette::Dracula);
        (
            bg(),
            text_color(),
            accent_color(),
            accent_2_color(),
            success_color(),
            warning_color(),
            danger_color(),
            info_color(),
        )
    };
    assert_ne!(catppuccin.0, dracula.0, "bg");
    assert_ne!(catppuccin.1, dracula.1, "text_color");
    assert_ne!(catppuccin.2, dracula.2, "accent_color");
    assert_ne!(catppuccin.3, dracula.3, "accent_2_color");
    assert_ne!(catppuccin.4, dracula.4, "success_color");
    assert_ne!(catppuccin.5, dracula.5, "warning_color");
    assert_ne!(catppuccin.6, dracula.6, "danger_color");
    assert_ne!(catppuccin.7, dracula.7, "info_color");
}

/// `Tier::Dark` reads as a per-palette override (module doc): true black
/// under Dracula exactly as under Catppuccin, every other Dracula color
/// unaffected — Dracula's own twin of
/// `dark_matches_full_for_every_color_but_the_background`.
#[test]
fn dark_matches_dracula_full_for_every_color_but_the_background() {
    let full = {
        let _tier = TierSandbox::new(Tier::Full);
        let _palette = PaletteSandbox::new(Palette::Dracula);
        (
            text_color(),
            accent_color(),
            danger_color(),
            success_color(),
        )
    };
    let dark = {
        let _tier = TierSandbox::new(Tier::Dark);
        let _palette = PaletteSandbox::new(Palette::Dracula);
        (
            text_color(),
            accent_color(),
            danger_color(),
            success_color(),
        )
    };
    assert_eq!(
        dark, full,
        "dark must equal Dracula's full in everything but bg()"
    );

    let _tier = TierSandbox::new(Tier::Dark);
    let _palette = PaletteSandbox::new(Palette::Dracula);
    assert_eq!(bg(), Color::Rgb(0, 0, 0));
}

/// `Compatible` dispatches to Dracula's OWN indexed table, not Catppuccin's —
/// `pick()` reads whichever palette is active before it reads the tier.
#[test]
fn dracula_uses_its_own_indexed_colors_on_compatible() {
    let _tier = TierSandbox::new(Tier::Compatible);
    let _palette = PaletteSandbox::new(Palette::Dracula);
    assert_eq!(bg(), Color::Indexed(236));
    assert_eq!(danger_color(), Color::Indexed(203));
    assert_eq!(accent_color(), Color::Indexed(141));
}

/// Two independent atomics: a tier pin (constructor or a live [`set_tier`])
/// must never move the palette, and vice versa.
#[test]
fn palette_and_tier_are_independent_axes() {
    let _tier = TierSandbox::new(Tier::Compatible);
    let _palette = PaletteSandbox::new(Palette::Dracula);
    assert_eq!(palette(), Palette::Dracula, "the tier pin must not move it");
    set_tier(Tier::Full);
    assert_eq!(
        palette(),
        Palette::Dracula,
        "a live tier swap must not move it either"
    );
    assert_eq!(tier(), Tier::Full);
}
