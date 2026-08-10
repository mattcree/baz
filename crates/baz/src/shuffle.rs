//! **Shuffle**: the player's standing property, and the wall's draw.
//!
//! Everything here is pure: a list of albums, the wall's own filter, a queue
//! record and a seed in; album ids and re-ordered queues out. No window, no
//! engine, no clock of its own. That is what makes the feature testable at all,
//! and it is the same separation [`crate::shelf`] and [`crate::rail`] already
//! keep.
//!
//! # Shuffle is a mode now, and this module changed shape because of it
//!
//! The owner, 2026-08-10: *"can you make shuffle a property of the player i.e.
//! toggle on/off."* Until then shuffle was an **act** — one press in the
//! Library strip drew eight records out of the wall's visible pool, queued them
//! and started; there was no mode, no flag, and nothing to turn off. It is now
//! **state**: on or off, held by [`crate::player::PlayerState`], persisted in
//! `config.toml`, and applying to *what plays next*. ADR-0023's amendment
//! records the decision.
//!
//! So the question this module answers changed. It was *what may a draw draw
//! from?* — answered by a `Pool` built from the wall's group key, query and
//! shelf, and made visible on the wall by dimming every non-member and ringing
//! the next two draws. It is now *what order does a run play in?* — answered by
//! [`arranged`] and [`restored`], over a queue somebody has already asked for.
//!
//! **The pool went with the draw, and what it was for is better served
//! without it.** The dimming and the rings existed to answer one question — *what
//! can this shuffle play?* — about a draw whose source was only *implied*. A
//! mode has no source of its own at all: what it re-orders is **the run**, the
//! queue, which is a place you can open, read row by row, edit and save. The
//! marks were a mitigation for an invisible pool; the pool is now literally the
//! list on screen, so there is nothing left to mitigate.
//!
//! The design line that still holds, and that this module is built to keep: **a
//! listener can see everything shuffle can play.** It holds more strongly than
//! before rather than less. What would break it — and what nothing here can do
//! — is reaching past the run into the library for "something similar", or
//! refilling an emptying queue: neither is expressible, because permuting a
//! list is the only operation in this module.
//!
//! # The pull is gone
//!
//! This module drew twice: shuffle, and **the pull** — one record, weighted
//! toward the long unplayed, offered rather than played. The owner removed it
//! on 2026-08-10 (*"please can we remove pull since it doesn't make sense
//! here"*), which also answers `docs/design/11-jobs-era-critique.md` **P9**
//! (*"Pull: explain it or rename it"*) a third way. What went with it:
//! `baz_core::history::pull_weight` and its two constants, `Ctrl+R`, the strip
//! word, and the record page's offer line. What stayed was the `Pool` the pull
//! borrowed from shuffle — until shuffle became a mode later the same day and
//! the pool went with the draw (above).
//!
//! # The unit
//!
//! A gesture decides what goes into the run — a record, a playlist, the whole
//! of All songs — and this module never widens it. Turning shuffle on cannot
//! add a track the listener did not ask for, because all it can do is permute
//! the list it is handed. That is the guarantee the old pool made about the
//! wall, made structural instead of made by construction.
//!
//! # It still ends
//!
//! *"The queue empties and there is silence"* is a refusal, and a mode does not
//! touch it: shuffle re-orders a finite list and stops when the list does. A
//! shuffle that refilled itself would be the radio the ledger rules out, and
//! there is nowhere in here to refill from.
//!
//! # Determinism
//!
//! Everything here takes a `seed` rather than reading a clock or a global
//! generator, so every arrangement this module can produce is reproducible in a
//! test. The generator is `SplitMix64` — sixteen lines, no dependency, and good
//! enough for choosing a record.

use std::path::PathBuf;

use crate::vm::QueueVm;

/// **The order a run would play in with shuffle off** — a queue's paths, in
/// the order the gesture that built it laid them out.
///
/// A `Vec<PathBuf>` and deliberately nothing richer. ADR-0023 §1 refuses *"a
/// live context object that keeps acting after the gesture"*, and the owner's
/// decision to make shuffle a mode needs the pre-shuffle order kept somewhere;
/// the amendment's answer is that what is retained is **inert data, not an
/// object**. This cannot re-read a playlist file, cannot notice a rescan,
/// cannot refill anything and has no methods. It is a list of paths that
/// [`restored`] reads once, when the listener turns the mode off.
pub(crate) type SourceOrder = Vec<PathBuf>;

/// The source order of `queue` — what to retain before [`arranged`] permutes
/// it.
pub(crate) fn source_order(queue: &QueueVm) -> SourceOrder {
    queue.paths()
}

/// **Bring the row the listener named to the front**, so a track click under
/// shuffle plays *that* track first.
///
/// Clicking track 4 with the mode on means two things at once — *this one* and
/// *then whatever* — and the only reading that honours both is: the clicked
/// track leads, and the rest of the record follows in a shuffled order. Pair it
/// with [`arranged`] at `keep = 1`.
///
/// A `row` the queue does not hold leaves the queue alone: a click on a stale
/// picture asks for nothing, which is [`crate::queue_edit`]'s rule for the same
/// situation.
pub(crate) fn leading(queue: &QueueVm, row: usize) -> QueueVm {
    let mut led = queue.clone();
    if row < led.items.len() {
        let item = led.items.remove(row);
        led.items.insert(0, item);
    }
    led
}

/// **Permute the run**, leaving the first `keep` entries where they are.
///
/// `keep` is what has already been settled and must not move:
///
/// | gesture | `keep` |
/// |---|---|
/// | a `Play` with the mode already on | `0` — nothing has sounded, so all of it is *next* |
/// | a track click with the mode on | `1` — after [`leading`] has hoisted the clicked row |
/// | turning the mode **on** mid-run | the playing row's index + 1 — what is behind the needle is history, and history does not re-order |
///
/// That last row is the whole reason this takes a `keep` at all. A mode turned
/// on halfway through an album must not re-order the four tracks you have
/// already heard, and must not interrupt the one that is sounding: the edit
/// goes out as `UpdateQueue`, which ADR-0014 guarantees disturbs no delivered
/// sample.
pub(crate) fn arranged(queue: &QueueVm, seed: u64, keep: usize) -> QueueVm {
    let mut shuffled = queue.clone();
    let keep = keep.min(shuffled.items.len());
    let tail = &mut shuffled.items[keep..];
    let mut order: Vec<usize> = (0..tail.len()).collect();
    shuffle_indices(&mut order, seed);
    let permuted: Vec<_> = order.iter().map(|&at| tail[at].clone()).collect();
    tail.clone_from_slice(&permuted);
    shuffled
}

/// **Put the run back into `order`** — what turning shuffle off means.
///
/// This is the hard half of the mode, and the promise is deliberately narrow
/// and deliberately total. Walk the retained order; for each path take the next
/// item of `queue` that carries it; then append everything left over, in the
/// order the run currently has. From which three properties fall out, and each
/// is a real thing a listener can do:
///
/// - **A row deleted while shuffled stays deleted.** Its path is in `order` and
///   no longer in the queue, so the walk finds nothing and moves on. Turning
///   shuffle off does not resurrect music you threw out.
/// - **A row appended while shuffled stays appended, at the end.** Its path is
///   not in `order` at all, so it falls into the leftovers — which is exactly
///   where the append put it. *Play next* keeps meaning *next*.
/// - **A file the run lists twice is put back twice**, because the walk
///   consumes one item per path rather than searching by identity. That is
///   [`crate::queue_edit`]'s position-not-identity rule, one layer up.
///
/// What is **not** promised: a run whose order the listener re-stated by hand —
/// a stepper press, a drag — has no source order left to return to. The hand
/// beats the machine's memory, so the reorder drops the retained order at the
/// call site (`crate::app`) and this is never reached. Nor does a run restored
/// from a snapshot have one: what was interrupted is put back as it was, and
/// baz does not remember an order for a run it did not itself arrange this
/// session.
pub(crate) fn restored(queue: &QueueVm, order: &[PathBuf]) -> QueueVm {
    let mut spare: Vec<Option<crate::vm::QueueItemVm>> =
        queue.items.iter().cloned().map(Some).collect();
    let mut items = Vec::with_capacity(queue.items.len());
    for path in order {
        if let Some(slot) = spare
            .iter_mut()
            .find(|item| item.as_ref().is_some_and(|item| item.path == *path))
            && let Some(item) = slot.take()
        {
            items.push(item);
        }
    }
    items.extend(spare.into_iter().flatten());
    let mut put_back = queue.clone();
    put_back.items = items;
    put_back
}

/// **Fisher–Yates over a list of positions.**
///
/// In place, unbiased, and deterministic: the same seed over the same list is
/// the same order every time, which is what lets a test pin an arrangement
/// instead of asserting that a shuffle is "random enough".
///
/// Over *positions* rather than over the items themselves because a queue item
/// is not `Copy`: the permutation is computed once and applied once, rather
/// than swapping clones about.
fn shuffle_indices(order: &mut [usize], seed: u64) {
    let mut state = seed_state(seed);
    for index in (1..order.len()).rev() {
        order.swap(index, pick(&mut state, index));
    }
}

/// A uniform position in `0..=upper`, from `state`.
///
/// The modulus is `upper + 1`, which is at most the list's length, so the
/// remainder is a valid index by construction and the conversion back cannot
/// lose anything. The modulo bias over a range this small against 2^64 is not
/// measurable; the alternative is rejection sampling for a record player.
fn pick(state: &mut u64, upper: usize) -> usize {
    let span = u64::try_from(upper).unwrap_or(u64::MAX).saturating_add(1);
    usize::try_from(next(state) % span).unwrap_or(0)
}

/// The generator's starting state, named rather than inlined so every
/// arrangement in this module seeds identically.
fn seed_state(seed: u64) -> u64 {
    seed
}

/// `SplitMix64` — the whole of baz's randomness.
///
/// Chosen because it is sixteen lines and no dependency (`docs/ENGINEERING.md`:
/// a new dependency is a reviewed decision), passes `BigCrush`, and is the
/// generator the Rust ecosystem itself uses to seed better ones. Nothing here is
/// cryptographic and nothing needs to be: the adversary is a listener who has
/// heard *Spirit of Eden* twice this week.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
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

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names
            .iter()
            .map(|name| PathBuf::from(format!("/m/{name}.flac")))
            .collect()
    }

    /// **An arrangement is a permutation**: every track, once, and nothing
    /// invented — for every seed, at every `keep`.
    ///
    /// The property that matters more than which order comes out. A shuffle
    /// that could lose a track or play one twice would be a different bug on
    /// every seed, so the sweep is over seeds rather than over one of them.
    #[test]
    fn an_arrangement_holds_exactly_the_tracks_it_was_given() {
        let original = queue(&["a", "b", "c", "d", "e", "f"]);
        for seed in 0..200_u64 {
            for keep in 0..=original.items.len() {
                let arranged = arranged(&original, seed, keep);
                assert_eq!(arranged.items.len(), original.items.len());
                let held: HashSet<PathBuf> = arranged
                    .items
                    .iter()
                    .map(|item| item.path.clone())
                    .collect();
                let asked: HashSet<PathBuf> = original
                    .items
                    .iter()
                    .map(|item| item.path.clone())
                    .collect();
                assert_eq!(held, asked, "seed {seed} at keep {keep} changed the set");
                // The queue's own identity travels with the arrangement: a
                // shuffled run is still the run it was.
                assert_eq!(arranged.album.as_deref(), Some("Geogaddi"));
                assert_eq!(arranged.provenance.as_deref(), Some("Road Trip"));
            }
        }
    }

    /// **`keep` is history and history does not re-order.** Turning the mode
    /// on halfway through a record must not shuffle what you have already
    /// heard, and must not move the track that is sounding.
    #[test]
    fn what_is_behind_the_needle_never_moves() {
        let original = queue(&["a", "b", "c", "d", "e", "f"]);
        for seed in 0..200_u64 {
            for keep in 0..=original.items.len() {
                let arranged = arranged(&original, seed, keep);
                assert_eq!(
                    titles(&arranged)[..keep],
                    titles(&original)[..keep],
                    "seed {seed} re-ordered the first {keep} rows"
                );
            }
        }
        // `keep` past the end is the whole run standing still rather than a
        // panic: the playing row is the engine's answer and this record may be
        // shorter than the one the answer was about.
        assert_eq!(
            titles(&arranged(&original, 1, 99)),
            titles(&original),
            "a keep past the end must be the identity, not an index panic"
        );
    }

    /// Deterministic, and actually shuffling.
    #[test]
    fn the_same_seed_is_the_same_order_and_different_seeds_differ() {
        let original = queue(&["a", "b", "c", "d", "e", "f"]);
        assert_eq!(
            titles(&arranged(&original, 0x5EED, 0)),
            titles(&arranged(&original, 0x5EED, 0))
        );
        // Over 6! = 720 orders, a fixed pair that collided would be a real
        // defect in the generator rather than luck.
        assert_ne!(
            titles(&arranged(&original, 9, 0)),
            titles(&arranged(&original, 10, 0))
        );
        // And it does not simply hand back what it was given.
        assert!(
            (0..64_u64).any(|seed| titles(&arranged(&original, seed, 0)) != titles(&original)),
            "no seed in 64 produced a different order"
        );
        // Empty and single-entry runs are the identity rather than a panic.
        assert!(arranged(&queue(&[]), 3, 0).is_empty());
        assert_eq!(titles(&arranged(&queue(&["only"]), 3, 0)), ["only"]);
    }

    /// **The clicked track leads.** A track click with the mode on means *this
    /// one*, and then whatever.
    #[test]
    fn a_named_row_is_hoisted_to_the_front() {
        let original = queue(&["a", "b", "c", "d"]);
        assert_eq!(titles(&leading(&original, 2)), ["c", "a", "b", "d"]);
        assert_eq!(titles(&leading(&original, 0)), ["a", "b", "c", "d"]);
        // A row this record does not hold asks for nothing — `queue_edit`'s
        // rule for a click on a stale picture.
        assert_eq!(titles(&leading(&original, 9)), titles(&original));
        // And hoist-then-arrange leaves the named track first for every seed,
        // which is the pairing `App::send_run` makes.
        for seed in 0..200_u64 {
            let run = arranged(&leading(&original, 3), seed, 1);
            assert_eq!(titles(&run)[0], "d", "seed {seed} moved the clicked track");
        }
    }

    /// **Turning shuffle off restores the unshuffled order** — the whole point
    /// of the mode, over every seed.
    #[test]
    fn restoring_puts_the_run_back_exactly_as_it_was() {
        let original = queue(&["a", "b", "c", "d", "e", "f"]);
        let order = source_order(&original);
        for seed in 0..200_u64 {
            let shuffled = arranged(&original, seed, 0);
            assert_eq!(
                titles(&restored(&shuffled, &order)),
                titles(&original),
                "seed {seed} did not come back"
            );
        }
        // Restoring what is already in order is the identity, and restoring
        // twice is the same as restoring once.
        let once = restored(&original, &order);
        assert_eq!(titles(&once), titles(&original));
        assert_eq!(titles(&restored(&once, &order)), titles(&original));
    }

    /// **A row deleted while shuffled stays deleted.** Turning shuffle off
    /// does not resurrect music the listener threw out.
    #[test]
    fn restoring_does_not_bring_back_a_row_that_was_removed() {
        let original = queue(&["a", "b", "c", "d"]);
        let order = source_order(&original);
        let mut shuffled = arranged(&original, 7, 0);
        shuffled.items.retain(|item| item.title != "b");
        let back = restored(&shuffled, &order);
        assert_eq!(titles(&back), ["a", "c", "d"]);
    }

    /// **A row appended while shuffled stays at the end**, because it is not in
    /// the retained order at all — which is exactly where the append put it, so
    /// *play next* keeps meaning next.
    #[test]
    fn restoring_leaves_an_appended_row_where_the_append_put_it() {
        let original = queue(&["a", "b", "c"]);
        let order = source_order(&original);
        let mut shuffled = arranged(&original, 11, 0);
        shuffled.items.push(item("z"));
        shuffled.items.push(item("y"));
        let back = restored(&shuffled, &order);
        assert_eq!(titles(&back), ["a", "b", "c", "z", "y"]);
    }

    /// A run listing one file twice is put back twice —
    /// [`crate::queue_edit`]'s position-not-identity rule, one layer up.
    #[test]
    fn a_repeated_file_is_restored_as_many_times_as_it_appears() {
        let mut original = queue(&["a", "b"]);
        original.items.push(item("a"));
        let order = source_order(&original);
        assert_eq!(order, paths(&["a", "b", "a"]));
        for seed in 0..64_u64 {
            let back = restored(&arranged(&original, seed, 0), &order);
            assert_eq!(titles(&back), ["a", "b", "a"], "seed {seed}");
        }
    }

    /// **The retained order is the paths, in order** — inert data and nothing
    /// richer, which is what keeps ADR-0023 §1's refusal of a live context
    /// object intact.
    #[test]
    fn the_source_order_is_the_paths_the_gesture_laid_out() {
        assert_eq!(
            source_order(&queue(&["a", "b", "c"])),
            paths(&["a", "b", "c"])
        );
        assert!(source_order(&queue(&[])).is_empty());
    }

    /// On and off and on again, over a run that is edited in between, always
    /// holds exactly the tracks that are still in it — the property a listener
    /// would notice breaking, asserted across the whole cycle rather than at
    /// each step.
    #[test]
    fn a_full_cycle_never_loses_or_duplicates_a_track() {
        let original = queue(&["a", "b", "c", "d", "e"]);
        for seed in 0..100_u64 {
            let order = source_order(&original);
            let on = arranged(&original, seed, 0);
            let mut edited = on.clone();
            edited.items.remove(0);
            edited.items.push(item("z"));
            let off = restored(&edited, &order);
            let mut left = titles(&off);
            left.sort();
            let mut wanted: Vec<String> = titles(&edited);
            wanted.sort();
            assert_eq!(left, wanted, "seed {seed}");
            // …and back on again, from where it now is.
            let again = arranged(&off, seed + 1, 0);
            let mut round = titles(&again);
            round.sort();
            assert_eq!(round, wanted, "seed {seed} on the return leg");
        }
    }
}
