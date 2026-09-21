#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The once-per-lapse decision and its state tracking, plus the
//! `config.toml` read `auto_start_enabled` does. `spawn_kick` itself needs a
//! real `codex` binary and a real account and is not unit-tested here, the
//! same way claude's `kick`/`kick_to` are not — see the module doc for how
//! it was verified instead (live probes against `wham/usage`).

use super::*;
use crate::profile::ProfileName;
use crate::testutil::HomeSandbox;

#[test]
fn should_kick_fires_only_on_a_fresh_lapse_with_auto_start_on() {
    assert!(
        should_kick(true, true, false),
        "opted in, lapsed, unattempted"
    );
    assert!(!should_kick(false, true, false), "opted out");
    assert!(
        !should_kick(true, false, false),
        "not lapsed — nothing to open"
    );
    assert!(
        !should_kick(true, true, true),
        "already spent this lapse's attempt"
    );
}

/// Marking, then a live reading, then a lapsed reading: the mark clears on
/// live and survives a lapsed reading unchanged, so the second lapse in a row
/// gets no second attempt.
#[test]
fn the_mark_clears_on_live_and_survives_a_repeated_lapse() {
    let name = "kick-test-cycle";
    assert!(!already_kicked(name));
    mark_kicked(name);
    assert!(already_kicked(name));

    note_window_state(name, true); // still lapsed — the attempt stands
    assert!(already_kicked(name));

    note_window_state(name, false); // opened, by the kick or by real use
    assert!(!already_kicked(name));

    note_window_state(name, true); // a fresh lapse
    assert!(
        !already_kicked(name),
        "a fresh lapse must get its own attempt"
    );
}

#[test]
fn auto_start_enabled_reads_the_key_and_its_old_alias() {
    let _home = HomeSandbox::new();
    let on = ProfileName::from("kick-test-on");
    let off = ProfileName::from("kick-test-off");
    let alias = ProfileName::from("kick-test-alias");
    let missing = ProfileName::from("kick-test-missing");
    write_config(&on, "auto_start = true\n");
    write_config(&off, "auto_start = false\n");
    write_config(&alias, "kick_timer = true\n");

    assert!(auto_start_enabled(&on));
    assert!(!auto_start_enabled(&off));
    assert!(
        auto_start_enabled(&alias),
        "the pre-rename spelling still reads"
    );
    assert!(
        !auto_start_enabled(&missing),
        "no config.toml at all reads as opted out, not an error"
    );
}

fn write_config(name: &ProfileName, body: &str) {
    let dir = crate::profile::profile_dir(name).expect("profile dir");
    std::fs::create_dir_all(&dir).expect("mkdir profile dir");
    std::fs::write(dir.join("config.toml"), body).expect("write config.toml");
}
