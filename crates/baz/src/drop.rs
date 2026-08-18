//! **What a file dropped on baz turns into.**
//!
//! The owner, 2026-08-18: *"drag and drop yes (depending on the
//! circumstances)"*. The circumstance this answers is the one every player
//! answers: something is dragged from a file manager onto a running baz, and
//! it should play.
//!
//! # What it deliberately is not
//!
//! **Not an import.** Nothing here scans, writes to the index, or adds a music
//! folder. A drop is a *listening* gesture — the run you had keeps playing and
//! the drop goes behind it — and a gesture that quietly enlarged the library
//! would be one drag away from a folder the listener never meant to keep. The
//! Settings place adds folders, deliberately, with a dialog.
//!
//! **Not a filter of what baz merely recognises.** The extension list is
//! [`baz_core::library::AUDIO_EXTENSIONS`], the same one the scanner uses, so
//! a drop can never queue a file the shelf would have refused to list. There
//! is exactly one list of what baz claims to play.
//!
//! # Bounded, because a filesystem is not
//!
//! A dropped folder is walked recursively, and a listener can drop `/`. So the
//! walk stops at [`MAX_DEPTH`] and [`MAX_FILES`], and says so in the health
//! log rather than appearing to work: an unbounded walk on a network share is
//! a frozen window, which is the one thing the wall's own rules never permit.

use std::path::{Path, PathBuf};

use baz_core::library::AUDIO_EXTENSIONS;

/// How deep a dropped folder is walked.
///
/// Eight is `Artist/Album/Disc` with five levels of slack. A library nested
/// deeper than that is not organised in a way a drop can guess at, and the
/// listener can drop the inner folder.
const MAX_DEPTH: usize = 8;

/// How many files one drop may queue.
///
/// A whole library dropped by accident should not become a fifty-thousand-row
/// queue that takes a minute to build. Two thousand is far past any album or
/// box set and well short of a collection.
const MAX_FILES: usize = 2_000;

/// **Every audio file under `path`**, in a stable order.
///
/// A file is itself, when baz plays that kind. A directory is its contents,
/// sorted by path so the album arrives in track order rather than in whatever
/// order the filesystem happened to hand back — the same reason the scanner
/// sorts.
#[must_use]
pub(crate) fn audio_under(path: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(path, 0, &mut found);
    found.sort();
    found.truncate(MAX_FILES);
    found
}

fn walk(path: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || found.len() >= MAX_FILES {
        return;
    }
    if path.is_file() {
        if is_audio(path) {
            found.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        // A folder baz cannot read is a folder with nothing in it, as far as
        // a drop is concerned. The scanner reports unreadable roots; a drop
        // is not a scan and has no business raising one.
        return;
    };
    for entry in entries.flatten() {
        walk(&entry.path(), depth + 1, found);
    }
}

/// Whether baz claims to play this file, by the scanner's own list.
fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

/// `n tracks`, singular where it should be — for the health line a drop
/// leaves behind.
#[must_use]
pub(crate) fn phrase(count: usize) -> String {
    if count == 1 {
        "1 track".to_owned()
    } else {
        format!("{count} tracks")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create");
        }
        std::fs::write(&path, b"not really audio").expect("write");
        path
    }

    /// **Only what baz plays**, and by the scanner's own list rather than a
    /// second one — a drop that queued a `.opus` would put a row on the queue
    /// that the shelf refuses to list.
    #[test]
    fn a_drop_takes_only_what_the_scanner_would_have_listed() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "a.flac");
        touch(dir.path(), "b.mp3");
        touch(dir.path(), "cover.jpg");
        touch(dir.path(), "notes.txt");
        touch(dir.path(), "c.opus");
        let found = audio_under(dir.path());
        let names: Vec<String> = found
            .iter()
            .filter_map(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        assert_eq!(names, vec!["a.flac".to_owned(), "b.mp3".to_owned()]);
    }

    /// **A single file is itself**, and one baz cannot play is nothing.
    #[test]
    fn a_dropped_file_is_taken_or_refused_on_its_own() {
        let dir = tempfile::tempdir().expect("tempdir");
        let flac = touch(dir.path(), "one.flac");
        let text = touch(dir.path(), "one.txt");
        assert_eq!(audio_under(&flac), vec![flac.clone()]);
        assert!(audio_under(&text).is_empty());
    }

    /// **Sorted, so an album arrives in its own order** rather than the
    /// filesystem's.
    #[test]
    fn a_folder_arrives_in_path_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in ["03 c.flac", "01 a.flac", "02 b.flac"] {
            touch(dir.path(), name);
        }
        let found = audio_under(dir.path());
        let names: Vec<String> = found
            .iter()
            .filter_map(|path| path.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        assert_eq!(
            names,
            vec![
                "01 a.flac".to_owned(),
                "02 b.flac".to_owned(),
                "03 c.flac".to_owned()
            ]
        );
    }

    /// **Nested folders are walked**, which is what makes dropping an artist
    /// folder queue their records.
    #[test]
    fn a_drop_reaches_into_folders() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "Album One/01.flac");
        touch(dir.path(), "Album Two/Disc 1/01.flac");
        assert_eq!(audio_under(dir.path()).len(), 2);
    }

    /// **And stops.** A listener can drop their root directory; a walk that
    /// did not bound itself would be a frozen window on a network share.
    #[test]
    fn the_walk_is_bounded_in_depth_and_in_count() {
        let dir = tempfile::tempdir().expect("tempdir");
        let deep = "a/".repeat(MAX_DEPTH + 4);
        touch(dir.path(), &format!("{deep}buried.flac"));
        assert!(
            audio_under(dir.path()).is_empty(),
            "the walk went past its own depth bound"
        );

        let wide = dir.path().join("wide");
        for n in 0..(MAX_FILES + 50) {
            touch(&wide, &format!("{n:05}.flac"));
        }
        assert_eq!(audio_under(&wide).len(), MAX_FILES);
    }
}
