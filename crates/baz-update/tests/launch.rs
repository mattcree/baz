//! **The launcher becomes baz.**
//!
//! Almost every launch takes this path and nothing else: no stage, nothing to
//! ask, so `baz-boot` finds the application beside itself and replaces itself
//! with it. Three things have to be true for that to be invisible, and all
//! three are the kind that break silently — the sibling is found, the
//! arguments survive, and the exit status is the application's rather than the
//! launcher's.
//!
//! Unix only, because the test stands a shell script where baz goes.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;
use std::process::Command;

/// Put `baz-boot` in a directory of its own with a stand-in `baz` beside it,
/// and return that directory.
fn a_pretend_installation(script: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::copy(env!("CARGO_BIN_EXE_baz-boot"), dir.path().join("baz-boot"))
        .expect("the launcher is built before its own test runs");
    let player = dir.path().join("baz");
    std::fs::write(&player, script).expect("a stand-in baz");
    std::fs::set_permissions(&player, std::fs::Permissions::from_mode(0o755))
        .expect("a runnable stand-in");
    dir
}

/// **Every argument reaches baz, in order.**
///
/// A launcher that ate them would silently break `baz /path/to/album` and
/// every desktop entry that passes one — and it would break them for the
/// people least able to say what went wrong, because the launcher is
/// invisible by design.
#[test]
fn the_launcher_hands_every_argument_to_baz() {
    let dir = a_pretend_installation("#!/bin/sh\nprintf '%s\\n' \"$@\"\n");
    let out = Command::new(dir.path().join("baz-boot"))
        .args(["one", "two three", "--four"])
        .output()
        .expect("the launcher runs");
    // Both halves of the report, because the one time this failed the message
    // was `{out:?}` and said nothing a reader could act on.
    assert!(
        out.status.success(),
        "the launcher exited {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "one\ntwo three\n--four\n",
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// **The application's exit status is what the desktop sees**, because on Unix
/// the launcher does not survive to have one of its own. A wrapper that
/// swallowed a non-zero exit would make every crash look like a clean quit.
#[test]
fn the_launcher_leaves_no_process_of_its_own_between_the_desktop_and_baz() {
    let dir = a_pretend_installation("#!/bin/sh\nexit 3\n");
    let status = Command::new(dir.path().join("baz-boot"))
        .status()
        .expect("the launcher runs");
    assert_eq!(status.code(), Some(3));
}
