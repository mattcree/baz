//! **Finding a missing playlist entry again.**
//!
//! ADR-0024 §3 states the posture and the limit in the same breath: an entry
//! whose path no longer resolves **stays in the file**, and repair is
//! **offered, never automatic** — *"candidate matches (same filename under a
//! current root) proposed per entry, confirmed by the user; the confirmation
//! is the only thing that writes the file."*
//!
//! Everything hard about that sentence is in the last clause. This module does
//! the easy half — proposing — and is careful to be nothing more: it reads the
//! index and returns paths. It cannot write a playlist, and the type it
//! returns is a list of suggestions rather than an answer.
//!
//! # Why the filename and not something cleverer
//!
//! The tempting move is to match on tags: same title, same artist, same
//! duration. It is the wrong move here for a reason worth writing down.
//!
//! A missing entry is a **path** and a `#EXTINF` line, and the `#EXTINF` is
//! whatever wrote the file — often another player, sometimes nothing at all.
//! So a tag match compares the index's confident reading against a string of
//! unknown provenance, and its failures are the expensive kind: two different
//! rips of the same song match each other perfectly, and the listener confirms
//! a swap they cannot see the consequences of.
//!
//! A filename match is dumber and its failures are cheap. `05 - Gasworks.flac`
//! either exists somewhere under a current root or it does not, and when
//! several do, they are shown with enough of their location to tell apart —
//! which is the point of proposing rather than repairing.
//!
//! # The index is already the root filter
//!
//! *"under a current root"* needs no separate check. The index holds what the
//! scanner walked, and the scanner walks the configured roots; a file outside
//! every root has no row. So iterating the index **is** the constraint,
//! rather than a scan that would then have to be filtered by it.
//!
//! # Ordering is a claim, so it is a small one
//!
//! Candidates are ordered by how much of the tail of the old path they still
//! share — the folder the file sat in, then its parent. A drive that was
//! remounted somewhere else keeps `Kesh/Signal Hill/05 - Gasworks.flac` intact
//! and changes only what is in front of it, so the true match usually sorts
//! first. Ties break on the path itself, so the list is stable between frames
//! and between runs; nothing here is a ranking of *likelihood*, only of shared
//! suffix, which is a fact rather than a guess.

use std::path::{Path, PathBuf};

use baz_core::index::Library;

/// **How many candidates are ever offered for one entry.**
///
/// A filename common enough to appear more times than this — `01.mp3`,
/// `track01.flac`, `Intro.m4a` — has stopped being evidence, and a card of
/// forty indistinguishable paths is not a proposal a person can act on. The
/// count is reported alongside so the surface can say the list was cut rather
/// than implying it was complete.
pub(crate) const MAX_CANDIDATES: usize = 8;

/// The candidates for one missing entry, in the order they should be offered,
/// and how many there were in total.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Candidates {
    /// At most [`MAX_CANDIDATES`], best-shared-suffix first.
    pub(crate) shown: Vec<PathBuf>,
    /// Every match found, including the ones past the cut.
    pub(crate) total: usize,
}

/// **Files in the index sharing this entry's filename.**
///
/// The entry's own path is excluded on the way past: a path the index still
/// holds is not missing, and offering a file its own location would be a
/// repair that changes nothing.
pub(crate) fn candidates(missing: &Path, library: &Library) -> Candidates {
    let Some(name) = missing.file_name() else {
        return Candidates::default();
    };
    let mut found: Vec<PathBuf> = library
        .tracks()
        .map(|meta| meta.path.clone())
        .filter(|path| path.file_name() == Some(name) && path != missing)
        .collect();
    let total = found.len();
    order(missing, &mut found);
    found.truncate(MAX_CANDIDATES);
    Candidates {
        shown: found,
        total,
    }
}

/// **Best-shared-suffix first**, then the path itself so the order never
/// depends on what the index happened to yield first.
///
/// Separate from [`candidates`] so it can be exercised without an index: the
/// ordering is the whole of the claim this module makes, and a test that had
/// to build a library to check it would end up re-stating the rule instead of
/// running it.
fn order(missing: &Path, found: &mut [PathBuf]) {
    found.sort_by(|a, b| {
        shared_tail(missing, b)
            .cmp(&shared_tail(missing, a))
            .then_with(|| a.cmp(b))
    });
}

/// How many trailing path components two paths have in common, counting from
/// the filename backwards. Always at least 1 for a candidate, since a shared
/// filename is what made it one.
fn shared_tail(a: &Path, b: &Path) -> usize {
    a.components()
        .rev()
        .zip(b.components().rev())
        .take_while(|(x, y)| x == y)
        .count()
}

/// **What to call a candidate on screen.**
///
/// The filename is the one part every candidate shares, so printing it would
/// be printing the same word several times. What distinguishes them is where
/// they sit, and the useful end of that is the *last* couple of folders —
/// `Kesh/Signal Hill` — rather than the front of a path that may be a mount
/// point nobody recognises. The full path is not hidden; it is simply not the
/// label, and the entry's own path is already on the row this was opened from.
pub(crate) fn location(path: &Path) -> String {
    // Named components only. The root and a Windows drive prefix are
    // components too, and `/ / Music` is not a place anybody recognises.
    let parts: Vec<_> = path
        .parent()
        .map(|parent| {
            parent
                .components()
                .rev()
                .filter_map(|part| match part {
                    std::path::Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
                    _ => None,
                })
                .take(2)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if parts.is_empty() {
        return path.to_string_lossy().into_owned();
    }
    parts.into_iter().rev().collect::<Vec<_>>().join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_location_reads_as_the_last_two_folders() {
        assert_eq!(
            location(Path::new(
                "/mnt/nas/Music/Kesh/Signal Hill/05 - Gasworks.flac"
            )),
            "Kesh / Signal Hill"
        );
        // A file at a root still has somewhere to be.
        assert_eq!(location(Path::new("/Music/a.flac")), "Music");
        // And one with no parent at all falls back to saying so rather than
        // rendering an empty label.
        assert_eq!(location(Path::new("a.flac")), "a.flac");
    }

    #[test]
    fn a_shared_tail_counts_from_the_filename_back() {
        let old = Path::new("/old/drive/Kesh/Signal Hill/05.flac");
        assert_eq!(
            shared_tail(old, Path::new("/new/Kesh/Signal Hill/05.flac")),
            3
        );
        assert_eq!(shared_tail(old, Path::new("/new/Elsewhere/05.flac")), 1);
        assert_eq!(shared_tail(old, Path::new("/new/other.flac")), 0);
    }

    /// **The ordering is the whole of the claim this module makes**, so it is
    /// pinned: a drive remounted under a different prefix keeps the album
    /// folder, and that candidate must lead.
    #[test]
    fn the_candidate_that_kept_its_folder_leads() {
        let missing = PathBuf::from("/old/Kesh/Signal Hill/05 - Gasworks.flac");
        let mut found = [
            PathBuf::from("/new/Compilations/05 - Gasworks.flac"),
            PathBuf::from("/new/Kesh/Signal Hill/05 - Gasworks.flac"),
            PathBuf::from("/new/Alt/Signal Hill/05 - Gasworks.flac"),
        ];
        order(&missing, &mut found);
        assert_eq!(
            found[0],
            PathBuf::from("/new/Kesh/Signal Hill/05 - Gasworks.flac")
        );
        assert_eq!(
            found[1],
            PathBuf::from("/new/Alt/Signal Hill/05 - Gasworks.flac")
        );
    }
}
