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
use std::process::{Command, Output};
use std::sync::{Mutex, PoisonError};

/// **One installation is built and run at a time**, and the reason is a kernel
/// error rather than tidiness.
///
/// These two tests each copy `baz-boot` into a directory of their own and then
/// execute the copy, and they run **concurrently in one process**. `fork`
/// hands the child every open descriptor; `O_CLOEXEC` closes them at `exec`
/// and not before. So while one test is between its `fork` and its `exec`, it
/// is holding the other test's still-open *write* descriptor on the file that
/// other test is about to run — and Linux answers `exec` on a file open for
/// writing with `ETXTBSY`. One of the two dies and the other does not, which
/// is exactly the shape this failed in: `1 passed; 1 failed`.
///
/// `docs/BACKLOG.md` recorded this as one of two CI flakes that *"need a
/// recurrence to be worth chasing"*, and the assertion below already carries a
/// message somebody widened after the first one. It recurred twice under a
/// full `cargo test --workspace` on 2026-09-02 and passes every time in
/// isolation, which is the signature of a race that needs the load of other
/// suites to lose.
///
/// Serialising **copy through to exit** closes the window rather than
/// narrowing it: no `fork` in this process can happen while any copy's
/// descriptor is open. A retry on `ETXTBSY` would also work and would leave
/// the race in place, waiting for a third suite to be added beside these two.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Stand `baz-boot` in a directory of its own with `script` as the `baz`
/// beside it, run it with `args`, and give back what it did.
///
/// The temporary directory lives until this returns and no longer: nothing
/// after the process has exited has anything to read from it.
fn launcher_beside(script: &str, args: &[&str]) -> Output {
    let _serialised = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::copy(env!("CARGO_BIN_EXE_baz-boot"), dir.path().join("baz-boot"))
        .expect("the launcher is built before its own test runs");
    let player = dir.path().join("baz");
    std::fs::write(&player, script).expect("a stand-in baz");
    std::fs::set_permissions(&player, std::fs::Permissions::from_mode(0o755))
        .expect("a runnable stand-in");
    Command::new(dir.path().join("baz-boot"))
        .args(args)
        .output()
        .expect("the launcher runs")
}

/// **Every argument reaches baz, in order.**
///
/// A launcher that ate them would silently break `baz /path/to/album` and
/// every desktop entry that passes one — and it would break them for the
/// people least able to say what went wrong, because the launcher is
/// invisible by design.
#[test]
fn the_launcher_hands_every_argument_to_baz() {
    let out = launcher_beside(
        "#!/bin/sh\nprintf '%s\\n' \"$@\"\n",
        &["one", "two three", "--four"],
    );
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
    let out = launcher_beside("#!/bin/sh\nexit 3\n", &[]);
    assert_eq!(
        out.status.code(),
        Some(3),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
