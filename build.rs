//! agentgear's version guard: `plugins/.claude-plugin/plugin.json` must carry
//! the crate version, or the build fails (CC keys its plugin cache on that
//! version, so a mismatch would ship a silent no-op). Also tracks the tree for
//! rebuilds and emits the marker the `#[derive(PluginHost)]` expansion checks
//! for, so a build without this file is a compile error. The tree lives in
//! `plugins/` rather than the default `plugin/`, so the derive's `tree` attr and
//! this call name it twice. The tree dir is read at runtime, never baked with
//! `env!`: build-script binaries are compiled once into the shared target dir
//! and reused across worktrees, so a baked path from a reaped worktree would
//! panic the next build of an unrelated tree.
//!
//! Also emits `CLAUTH_VERSION_SUFFIX` (see [`describe_suffix`]): this fork
//! tracks upstream by rebase rather than cutting its own numbered releases, so
//! `Cargo.toml`'s bare `CARGO_PKG_VERSION` alone would show the same
//! `0.15.2` for every one of this fork's own builds since that tag, upstream's
//! or ours. `--version` needs the two apart without guessing at whatever the
//! next real release will be numbered.

use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| panic!("cargo sets CARGO_MANIFEST_DIR"));
    agentgear::build::assert_plugin_version_at(Path::new(&manifest_dir).join("plugins"));
    emit_version_suffix(&manifest_dir);
}

/// Sets `CLAUTH_VERSION_SUFFIX` for `cli.rs` to append to `CARGO_PKG_VERSION`
/// (always — `env!`, not `option_env!`, reads it there, so it must exist even
/// when empty). Re-run triggers on `.git/HEAD` and whichever ref it points at,
/// so a new commit or checkout is picked up on the next build without
/// touching any tracked source file.
fn emit_version_suffix(manifest_dir: &str) {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/packed-refs");
    if let Ok(head) = std::fs::read_to_string(Path::new(manifest_dir).join(".git/HEAD"))
        && let Some(ref_path) = head.strip_prefix("ref: ")
    {
        println!("cargo:rerun-if-changed=.git/{}", ref_path.trim());
    }
    println!(
        "cargo:rustc-env=CLAUTH_VERSION_SUFFIX={}",
        describe_suffix(manifest_dir).unwrap_or_default()
    );
}

/// `None` (a clean `CARGO_PKG_VERSION` alone) when there is nothing more
/// honest to say: not a git checkout at all (a source tarball has no `.git`),
/// no `git` on `PATH`, or HEAD IS the matching upstream tag exactly. Neither
/// of the first two should ever fail the build over a detail this cosmetic.
///
/// `--match` pins the baseline to upstream's own `vX.Y.Z` release tags
/// specifically (never one of this fork's own, whatever shape those end up
/// taking), so `git describe`'s count is always "commits past the last real
/// release," upstream's and this fork's own summed together — which is
/// exactly what should distinguish one of this fork's builds from another
/// between two release bumps. Reformatted from `git describe`'s own
/// `TAG-N-gHASH[-dirty]` into semver BUILD metadata (leading `+`) rather than
/// a pre-release (`-`): this never claims to BE the next version, only how
/// far past the last tagged one it is.
fn describe_suffix(manifest_dir: &str) -> Option<String> {
    let output = Command::new("git")
        .args([
            "describe",
            "--tags",
            "--always",
            "--dirty",
            "--match",
            "v[0-9]*.[0-9]*.[0-9]*",
        ])
        .current_dir(manifest_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let described = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    if described == tag {
        return None;
    }
    Some(match described.strip_prefix(&format!("{tag}-")) {
        Some(rest) => format!("+{}", rest.replace('-', ".")),
        // Cargo.toml bumped ahead of any reachable matching tag, or none
        // exists at all (--always's bare-hash fallback): still informative,
        // never wrong, just not reformatted.
        None => format!("+{described}"),
    })
}
