//! The codex equivalent of claude's 5h auto-start ping: `auto_start` opts a
//! profile in, and once its primary window reads as dormant (never opened, or
//! lapsed back to that state after a real one closed), one `codex exec` turn
//! opens it.
//!
//! Unlike claude's kick, this fires ONCE per lapse rather than on every tick.
//! Claude's ping is a 1-token HTTP call cheap enough to retry through a
//! transient 429; codex has no such cheap path — the real inference call
//! carries the account's own model and a fixed system-prompt/tool-schema
//! overhead (thousands of tokens, several seconds), so retrying continuously
//! the way claude's kick survives a rejection would burn real quota for no
//! benefit. A failed or unconfirmed attempt just waits for the next lapse
//! rather than retrying within this one (owner ruling 2026-09-21).
//!
//! Verified empirically against live `wham/usage` reads (2026-09-21): a
//! dormant primary window reports `reset_after_seconds == limit_window_seconds`
//! exactly — the server's placeholder for "if you started now, it would run
//! the full window" — and both fields drift forward with every poll until a
//! real completion lands. Two different never-touched accounts read
//! IDENTICAL `reset_at`s, each exactly the poll instant plus the window
//! length, which is what rules out a real-but-recently-reset window: two
//! independent accounts cannot share one real anchor.
//!
//! The MODEL matters and the exit code does not:
//! - a session with no copied operator config defaults to a lighter model
//!   (`gpt-5.6-sol` observed) whose completions do not draw against this
//!   window at all — confirmed by kicking under it and finding the window
//!   still dormant afterward. Copying the operator's own `~/.codex/config.toml`
//!   into the kick's scratch home (exactly what `clauth start` already does
//!   for a real session, decision 3 — there is no per-profile model
//!   preference for codex) is what makes it use the account's real default.
//! - `codex exec` exits 0 even when the underlying request 400'd (observed
//!   live: an invalid `-c model_reasoning_effort` override was rejected by
//!   the server and printed as an error block, and the process still exited
//!   0). So [`spawn_kick`]'s return value is "the process ran to completion
//!   without an infrastructure failure", never "the window opened" — the
//!   caller confirms that by re-polling usage.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::logline::logline;
use crate::profile::{ProfileName, profile_subpath};

/// Profiles kicked for their CURRENT lapse, so a second tick within the same
/// lapse does not fire again. Cleared the moment a poll sees the window live
/// (real use or a landed kick, [`note_window_state`]), so the next lapse gets
/// exactly one more attempt. In-memory only: a restart re-arming one extra
/// attempt on an already-attempted lapse is a cheap mistake, unlike claude's
/// 429 backoff, which guards something that actually matters.
static KICKED_THIS_LAPSE: Mutex<Option<HashSet<String>>> = Mutex::new(None);

pub(crate) fn mark_kicked(name: &str) {
    if let Ok(mut guard) = KICKED_THIS_LAPSE.lock() {
        guard
            .get_or_insert_with(HashSet::new)
            .insert(name.to_string());
    }
}

pub(crate) fn already_kicked(name: &str) -> bool {
    KICKED_THIS_LAPSE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.contains(name)))
        .unwrap_or(false)
}

/// Fold a poll's lapsed reading into the mark: live clears it (arming the
/// next lapse), lapsed leaves it as it was. Called for every codex poll, not
/// only the ones that just kicked, so a window opened by ordinary use clears
/// the mark exactly the way a landed kick does.
pub(crate) fn note_window_state(name: &str, lapsed: bool) {
    if lapsed {
        return;
    }
    if let Ok(mut guard) = KICKED_THIS_LAPSE.lock()
        && let Some(set) = guard.as_mut()
    {
        set.remove(name);
    }
}

/// Whether the scheduler should fire a kick for a profile whose window just
/// read as lapsed. Pure — [`crate::usage::scheduler`] owns the `auto_start`
/// read, the state-tracking side effects, and the actual spawn around it.
pub(crate) fn should_kick(auto_start: bool, lapsed: bool, already_kicked: bool) -> bool {
    auto_start && lapsed && !already_kicked
}

/// Read a codex profile's `auto_start` straight off its `config.toml`.
/// Deliberately not `profile::ProfileConfig` (private, and shaped around
/// every claude-only field beside it): this reads exactly the one key codex
/// acts on, off the file every profile already has regardless of harness
/// (decision 3 — dirs are bare for both).
#[derive(Debug, Default, serde::Deserialize)]
struct CodexProfileConfig {
    #[serde(default, alias = "kick_timer")]
    auto_start: bool,
}

pub(crate) fn auto_start_enabled(name: &ProfileName) -> bool {
    let Ok(path) = profile_subpath(name, "config.toml") else {
        return false;
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return false;
    };
    toml::from_str::<CodexProfileConfig>(&raw)
        .map(|c| c.auto_start)
        .unwrap_or(false)
}

/// Generous on purpose: this is a background attempt with nobody waiting on
/// it, not an interactive session, so erring long costs nothing but the one
/// (rare, once-per-lapse) tick it runs on — and the once-per-lapse design
/// (module doc) makes a false timeout expensive: it spends the whole lapse's
/// one attempt for nothing. A real turn under `model_reasoning_effort = "max"`
/// (the operator's own config, copied verbatim) was observed at ~6.5s; this
/// leaves wide margin for a slower one without materially delaying the
/// scheduler tick it runs on.
const KICK_TIMEOUT: Duration = Duration::from_secs(120);

/// The prompt is content, not control: `-s read-only` means it cannot execute
/// anything regardless of what the model does with it, and the text is
/// honest about its own purpose rather than a disguised no-op.
const KICK_PROMPT: &str =
    "This is an automated ping to open the 5-hour usage window. Reply with a single word.";

/// Fire the once-per-lapse ping. Builds a throwaway `CODEX_HOME`: `auth.json`
/// symlinked onto the profile's own store (never copied — decision 8, one
/// physical carrier, so a mid-ping refresh from any other carrier is still
/// visible to it and vice versa), plus the operator's own `~/.codex/config.toml`
/// copied in (see the module doc for why that specific file is what makes
/// the ping spend against the right window). Then one minimal `codex exec`
/// turn, `-s read-only`/`--skip-git-repo-check`/`--ephemeral` so it cannot
/// touch this filesystem and leaves no session artifacts behind.
///
/// Returns whether the process ran to completion without an infrastructure
/// failure (spawn, timeout) — NOT whether the window opened; see the module
/// doc's note on `codex exec`'s exit code. The caller re-polls usage to find
/// out.
pub(crate) fn spawn_kick(name: &ProfileName) -> bool {
    let Ok(store_auth) = profile_subpath(name, "auth.json") else {
        return false;
    };
    let Ok(tmp) = tempfile::tempdir() else {
        logline!("{name}: 5h window kick could not build a scratch home");
        return false;
    };
    let home = tmp.path();

    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&store_auth, home.join("auth.json")).is_ok();
    #[cfg(not(unix))]
    let linked = false;
    if !linked {
        logline!(
            "{name}: 5h window kick skipped — no symlink support here, and a copied auth.json \
             would risk a second carrier of a single-use refresh token"
        );
        return false;
    }

    if let Ok(operator_home) = crate::actions::codex_operator_home() {
        let src = operator_home.join("config.toml");
        if src.exists() {
            let _ = crate::runtime::copy_codex_config(&src, &home.join("config.toml"));
        }
    }

    let mut command = crate::start::codex_spawn_command(
        home,
        &[
            "exec".to_string(),
            "-s".to_string(),
            "read-only".to_string(),
            "--skip-git-repo-check".to_string(),
            "--ephemeral".to_string(),
            KICK_PROMPT.to_string(),
        ],
        &[],
    );
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    let Ok(mut child) = command.spawn() else {
        logline!("{name}: 5h window kick failed to launch codex");
        return false;
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if started.elapsed() < KICK_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Ok(None) => {
                let _ = child.kill();
                logline!("{name}: 5h window kick timed out after {KICK_TIMEOUT:?}");
                return false;
            }
            Err(_) => return false,
        }
    }
}

#[cfg(test)]
#[path = "../tests/inline/codex_window_kick.rs"]
mod tests;
