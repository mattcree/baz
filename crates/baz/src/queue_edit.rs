//! Editing the queue: turning a gesture on a row into the whole list
//! [`UpdateQueue`](baz_core::protocol::Command::UpdateQueue) wants.
//!
//! ADR-0006 layer 1 — pure, iced-free, unit-tested — and the only genuinely
//! new logic the information-architecture move needs (ADR-0016; the design
//! spec named this module in advance, `docs/design/01-ux-audit-and-ia.md` §5).
//!
//! # Why a whole list, and why that makes this module trivial
//!
//! ADR-0014 chose whole-queue commands over index deltas for a reason worth
//! restating where the deltas would have been built: *an index-based delta
//! applied against a stale picture removes a different track, and neither side
//! can tell.* A front end's list can go stale between the click and the send —
//! a track can fail and be skipped, a rate change can hand over — and
//! `RemoveAt { index }` has no way to notice.
//!
//! So the front end sends what it means: the queue as it should now be, in play
//! order. That makes the edit itself a list operation with no protocol in it,
//! which is exactly the kind of thing that belongs in a pure module with tests
//! rather than inline in a view.
//!
//! # Why it works on [`QueueVm`] rather than on paths
//!
//! Because the payload the engine is sent and the rows the popover lists must
//! not be able to describe different music. [`QueueVm`] is the one value that
//! carries both — the paths *are* [`QueueVm::paths`], and the rows are built
//! from the same items in the same pass — so an edit that produced a bare
//! `Vec<PathBuf>` would hand the engine a new queue while leaving the interface
//! showing the old one. Editing the record and taking the payload from it keeps
//! the two structurally identical, which is the property `vm::QueueVm`'s own
//! docs exist to protect.

use crate::vm::QueueVm;

/// The queue with entry `index` removed — the list a per-row ✕ means.
///
/// `None` when `index` is not in the queue at all, which is a click on a stale
/// picture: the honest answer is to ask for nothing rather than to remove a
/// different track. (The engine would accept whatever it was sent; this is the
/// front end declining to invent an intention.)
///
/// Removing the **last remaining** entry yields an empty queue rather than
/// `None`. Emptying the queue is a thing a listener can mean, the protocol
/// accepts it, and ADR-0014 says what the engine does with it: the run ends.
/// Refusing it here would make the twelfth ✕ work and the thirteenth silently
/// do nothing.
#[must_use]
pub fn without(queue: &QueueVm, index: usize) -> Option<QueueVm> {
    if index >= queue.items.len() {
        return None;
    }
    let mut edited = queue.clone();
    edited.items.remove(index);
    Some(edited)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;
    use crate::vm::QueueItemVm;

    fn item(name: &str) -> QueueItemVm {
        QueueItemVm {
            title: name.to_owned(),
            artist: None,
            album: Some("A Record".to_owned()),
            album_artist: None,
            duration: Some(Duration::from_secs(200)),
            path: PathBuf::from(format!("/m/{name}.flac")),
        }
    }

    fn queue(names: &[&str]) -> QueueVm {
        QueueVm {
            album: Some("Geogaddi".to_owned()),
            artist: "Boards of Canada".to_owned(),
            items: names.iter().copied().map(item).collect(),
            provenance: Some("Road Trip".to_owned()),
        }
    }

    fn titles(queue: &QueueVm) -> Vec<String> {
        queue.items.iter().map(|item| item.title.clone()).collect()
    }

    #[test]
    fn removing_an_entry_drops_it_and_keeps_the_order_of_the_rest() {
        let original = queue(&["a", "b", "c", "d"]);
        let edited = without(&original, 1).expect("row 1 is in the queue");
        assert_eq!(titles(&edited), ["a", "c", "d"]);
        assert_eq!(
            titles(&original),
            ["a", "b", "c", "d"],
            "the record the edit was computed from must not be mutated"
        );
    }

    #[test]
    fn either_end_is_an_ordinary_removal() {
        let original = queue(&["a", "b", "c"]);
        assert_eq!(
            titles(&without(&original, 0).expect("the head")),
            ["b", "c"]
        );
        assert_eq!(
            titles(&without(&original, 2).expect("the tail")),
            ["a", "b"]
        );
    }

    /// The payload and the rows come from one value, so they cannot describe
    /// different music — the property this module works on `QueueVm` for.
    #[test]
    fn the_paths_sent_are_exactly_the_rows_left() {
        let edited = without(&queue(&["a", "b", "c"]), 1).expect("row 1");
        assert_eq!(
            edited.paths(),
            vec![PathBuf::from("/m/a.flac"), PathBuf::from("/m/c.flac")]
        );
        assert_eq!(edited.paths().len(), edited.items.len());
        // And the queue's own identity travels with it: an edit is not a new
        // queue from a different album.
        assert_eq!(edited.album.as_deref(), Some("Geogaddi"));
        assert_eq!(edited.artist, "Boards of Canada");
        // Provenance stands through every edit (09 §6): a run that has been
        // edited is still "the run I started from Road Trip".
        assert_eq!(edited.provenance.as_deref(), Some("Road Trip"));
    }

    /// A click on a row this record does not have asks for nothing. It is an
    /// ordinary race — the queue shrank under the pointer — not a fault.
    #[test]
    fn a_row_that_is_not_there_asks_for_nothing() {
        let original = queue(&["a", "b"]);
        assert!(without(&original, 2).is_none());
        assert!(without(&original, usize::MAX).is_none());
        assert!(without(&queue(&[]), 0).is_none());
    }

    /// Emptying the queue is a thing a listener can mean, and the protocol
    /// accepts it. Refusing it would make the last ✕ the only one that does
    /// nothing.
    #[test]
    fn removing_the_last_entry_yields_an_empty_queue_rather_than_nothing() {
        let edited = without(&queue(&["only"]), 0).expect("the one entry");
        assert!(edited.is_empty());
        assert!(edited.paths().is_empty());
    }

    /// A queue that lists one file twice has two entries the engine cannot tell
    /// apart, and this must remove the one that was pointed at rather than the
    /// first match.
    #[test]
    fn a_repeated_file_is_removed_by_position_not_by_identity() {
        let mut original = queue(&["a", "b"]);
        original.items.push(item("a"));
        let edited = without(&original, 2).expect("the second copy of a");
        assert_eq!(titles(&edited), ["a", "b"]);
        assert_eq!(edited.paths()[0], PathBuf::from("/m/a.flac"));
    }

    /// Removing every entry one at a time, in every order, always ends in an
    /// empty queue and never loses or duplicates a track on the way.
    #[test]
    fn repeated_removals_never_lose_or_duplicate_a_track() {
        let names = ["a", "b", "c", "d"];
        for first in 0..names.len() {
            for second in 0..names.len() - 1 {
                let original = queue(&names);
                let once = without(&original, first).expect("in range");
                let twice = without(&once, second).expect("in range");
                assert_eq!(twice.len(), names.len() - 2);
                let mut left = titles(&twice);
                left.sort();
                left.dedup();
                assert_eq!(left.len(), names.len() - 2, "an entry was duplicated");
                for title in &left {
                    assert!(names.contains(&title.as_str()), "{title} was invented");
                }
            }
        }
    }
}
