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
