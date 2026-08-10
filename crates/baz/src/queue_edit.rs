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

/// The queue with entry `index` swapped one place along — the list a row's
/// ▲ (`delta` −1) or ▼ (`delta` +1) stepper means (doc 09 §8.2: the playlist
/// page's reorder, grown onto the queue's own editor).
///
/// A swap with a neighbour rather than an arbitrary move, because that is
/// what the steppers say: one press, one place. `None` when `index` is not
/// in the queue (a click on a stale picture — [`without`]'s rule), when the
/// step would leave the list (▲ on the first row, ▼ on the last; the view
/// disables those steppers, and this is the same refusal made where the edit
/// is computed), or when `delta` is not a single step.
///
/// The **playing entry moves like any other**: the edit goes out as
/// [`UpdateQueue`](baz_core::protocol::Command::UpdateQueue), which ADR-0014
/// guarantees disturbs no delivered sample, and the cursor follows its track
/// — the engine re-derives the position by path and announces it, and until
/// it does the front end finds the row the same way
/// ([`QueueVm::playing`](crate::vm::QueueVm::playing)).
#[must_use]
pub fn shifted(queue: &QueueVm, index: usize, delta: i32) -> Option<QueueVm> {
    if !matches!(delta, -1 | 1) || index >= queue.items.len() {
        return None;
    }
    let neighbour = index.checked_add_signed(delta as isize)?;
    if neighbour >= queue.items.len() {
        return None;
    }
    let mut edited = queue.clone();
    edited.items.swap(index, neighbour);
    Some(edited)
}

/// The queue with entry `from` taken out and put back at `to` (its index in
/// the list *after* the removal) — the list a completed reorder **drag**
/// means (doc 09 §13 step 8; [`crate::drag`]'s commit, as [`shifted`] is the
/// steppers').
///
/// One arbitrary reposition rather than a chain of neighbour swaps, because
/// that is what the gesture says: the row was lifted once and put down once,
/// and the engine hears **one** whole-list
/// [`UpdateQueue`](baz_core::protocol::Command::UpdateQueue) for it —
/// ADR-0014's guarantee that an edit missing the playing track disturbs no
/// delivered sample, and the cursor follows its track by path exactly as it
/// does for a swap.
///
/// `None` when `from` is not in the queue, when `to` is past the shortened
/// list, or when the move would change nothing — a drop on the slot the row
/// came from is a click's worth of nothing, not an edit.
#[must_use]
pub fn moved(queue: &QueueVm, from: usize, to: usize) -> Option<QueueVm> {
    if from >= queue.items.len() || to >= queue.items.len() || from == to {
        return None;
    }
    let mut edited = queue.clone();
    let item = edited.items.remove(from);
    edited.items.insert(to, item);
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
            origin: Some(crate::origin::Origin::playlist("Road Trip")),
            source: crate::vm::RunSource::Playlist("Road Trip".to_owned()),
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
        assert_eq!(edited.provenance(), Some("Road Trip"));
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

    /// S9a / doc 09 §8.2 — **a stepper press swaps the entry with its
    /// neighbour and nothing else moves**: the playlist page's reorder
    /// semantics (`playlists::shift_entry`), on the queue's own record.
    #[test]
    fn shifting_swaps_with_the_neighbour_and_keeps_the_rest() {
        let original = queue(&["a", "b", "c", "d"]);
        let down = shifted(&original, 1, 1).expect("b has a row below");
        assert_eq!(titles(&down), ["a", "c", "b", "d"]);
        let up = shifted(&original, 1, -1).expect("b has a row above");
        assert_eq!(titles(&up), ["b", "a", "c", "d"]);
        assert_eq!(
            titles(&original),
            ["a", "b", "c", "d"],
            "the record the edit was computed from must not be mutated"
        );
    }

    /// A step off either end asks for nothing — the view disables those
    /// steppers, and the edit refuses the same press independently.
    #[test]
    fn a_step_off_either_end_asks_for_nothing() {
        let original = queue(&["a", "b", "c"]);
        assert!(shifted(&original, 0, -1).is_none(), "▲ on the first row");
        assert!(shifted(&original, 2, 1).is_none(), "▼ on the last row");
        assert!(shifted(&original, 3, -1).is_none(), "a stale row");
        assert!(shifted(&original, usize::MAX, 1).is_none());
        assert!(shifted(&queue(&[]), 0, 1).is_none());
        // The steppers speak in single steps; anything else is not a press
        // this module knows.
        assert!(shifted(&original, 1, 2).is_none());
        assert!(shifted(&original, 1, 0).is_none());
    }

    /// The paths sent are exactly the rows shown after a reorder, and the
    /// queue's identity — header and provenance — travels with the edit
    /// (09 §6: a run that has been reordered is still "the run I started
    /// from Road Trip").
    #[test]
    fn a_reorder_sends_the_rows_it_shows_and_keeps_provenance() {
        let edited = shifted(&queue(&["a", "b", "c"]), 0, 1).expect("row 0 down");
        assert_eq!(
            edited.paths(),
            vec![
                PathBuf::from("/m/b.flac"),
                PathBuf::from("/m/a.flac"),
                PathBuf::from("/m/c.flac"),
            ]
        );
        assert_eq!(edited.album.as_deref(), Some("Geogaddi"));
        assert_eq!(edited.artist, "Boards of Canada");
        assert_eq!(edited.provenance(), Some("Road Trip"));
    }

    /// ▲ then ▼ (and ▼ then ▲) is the queue it started as — a stepper pair
    /// that did not round-trip would be an edit the listener never made.
    #[test]
    fn a_step_up_undoes_a_step_down() {
        let original = queue(&["a", "b", "c", "d"]);
        for index in 0..3 {
            let down = shifted(&original, index, 1).expect("a row below");
            let back = shifted(&down, index + 1, -1).expect("the row it became");
            assert_eq!(titles(&back), titles(&original));
        }
    }

    /// A queue listing one file twice moves the occurrence that was pointed
    /// at — [`without`]'s position-not-identity rule, for the steppers.
    #[test]
    fn a_repeated_file_is_moved_by_position_not_by_identity() {
        let mut original = queue(&["a", "b"]);
        original.items.push(item("a"));
        let edited = shifted(&original, 2, -1).expect("the second copy of a");
        assert_eq!(titles(&edited), ["a", "a", "b"]);
    }

    /// Doc 09 §13 step 8 — **a completed drag repositions the entry and
    /// nothing else moves**: the drag's commit, beside the steppers' swap.
    #[test]
    fn a_move_lands_the_entry_where_the_line_said() {
        let original = queue(&["a", "b", "c", "d"]);
        let down = moved(&original, 0, 2).expect("a to position 2");
        assert_eq!(titles(&down), ["b", "c", "a", "d"]);
        let up = moved(&original, 3, 0).expect("d to the head");
        assert_eq!(titles(&up), ["d", "a", "b", "c"]);
        assert_eq!(
            titles(&original),
            ["a", "b", "c", "d"],
            "the record the edit was computed from must not be mutated"
        );
        // A one-step move is exactly the stepper's swap.
        assert_eq!(
            titles(&moved(&original, 1, 2).expect("one step down")),
            titles(&shifted(&original, 1, 1).expect("the same step"))
        );
    }

    /// A move to nowhere new, or from a row this record does not have, asks
    /// for nothing — the drag's no-op drop, refused where the edit is
    /// computed as well as in [`crate::drag::DragState::destination`].
    #[test]
    fn a_pointless_or_stale_move_asks_for_nothing() {
        let original = queue(&["a", "b", "c"]);
        assert!(moved(&original, 1, 1).is_none(), "dropped where it was");
        assert!(moved(&original, 3, 0).is_none(), "a stale row");
        assert!(moved(&original, 0, 3).is_none(), "past the shortened list");
        assert!(moved(&queue(&[]), 0, 0).is_none());
    }

    /// The paths sent are exactly the rows shown after a move, and the
    /// queue's identity — header and provenance — travels with the edit.
    #[test]
    fn a_move_sends_the_rows_it_shows_and_keeps_provenance() {
        let edited = moved(&queue(&["a", "b", "c"]), 2, 0).expect("c to the head");
        assert_eq!(
            edited.paths(),
            vec![
                PathBuf::from("/m/c.flac"),
                PathBuf::from("/m/a.flac"),
                PathBuf::from("/m/b.flac"),
            ]
        );
        assert_eq!(edited.album.as_deref(), Some("Geogaddi"));
        assert_eq!(edited.provenance(), Some("Road Trip"));
    }

    /// A queue listing one file twice moves the occurrence that was lifted
    /// — [`without`]'s position-not-identity rule, for the drag.
    #[test]
    fn a_repeated_file_is_dragged_by_position_not_by_identity() {
        let mut original = queue(&["a", "b"]);
        original.items.push(item("a"));
        let edited = moved(&original, 2, 0).expect("the second copy of a");
        assert_eq!(titles(&edited), ["a", "a", "b"]);
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
