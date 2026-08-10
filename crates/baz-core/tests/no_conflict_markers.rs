//! **No file tracked by git carries a merge conflict's markers.**
//!
//! This exists because they reached `main` on 2026-08-10, in `CHANGELOG.md`,
//! from a merge whose *other* two conflicted files were resolved by hand while
//! the third was missed. Nothing caught it: the gate compiles Rust and lints
//! Rust, and a marker in Markdown is neither. It was found hours later by an
//! agent reading the file for an unrelated reason, and it had been pushed,
//! CI'd green, and reported as landed in between.
//!
//! The documents this protects are not decoration. `docs/WORK.md` is the
//! ordered queue every agent starts from, and it has been mangled three times
//! — twice by keep-both resolution and once by a regex that matched its own
//! preamble. A conflicted `CHANGELOG.md` is a release note that ships with
//! `<<<<<<<` in it.
//!
//! Cheap enough to be unconditional: one `git ls-files` and a scan of what it
//! names.

use std::path::Path;
use std::process::Command;

/// The three markers `git` writes, split so this file cannot match itself —
/// a test that trips on its own source is a test nobody can keep.
const MARKERS: [&str; 3] = ["<<<<<<<", "=======", ">>>>>>>"];

#[test]
fn no_tracked_file_carries_a_conflict_marker() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let listed = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .expect("git ls-files");
    assert!(listed.status.success(), "git ls-files failed");

    let mut found = Vec::new();
    for name in String::from_utf8_lossy(&listed.stdout).split('\0') {
        if name.is_empty() {
            continue;
        }
        let path = root.join(name);
        // This file names the markers on purpose; binaries are not text.
        if path.ends_with("no_conflict_markers.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            // A marker is the whole start of a line, which is how git writes
            // them — a `=======` underline in Markdown is not one, and neither
            // is a `>>>` quoted in prose.
            if MARKERS
                .iter()
                .any(|m| line.starts_with(m) && line.trim_end() == *m)
                || MARKERS
                    .iter()
                    .any(|m| line.starts_with(m) && line.starts_with(&format!("{m} ")))
            {
                found.push(format!("{name}:{}: {line}", n + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "merge conflict markers are committed:\n{}",
        found.join("\n")
    );
}
