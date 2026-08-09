//! **Shuffle**: the draw baz makes, and the rule it keeps — *you can see what
//! it is drawing from*.
//!
//! Everything here is pure: a list of albums, the wall's own filter and a seed
//! in; album ids out. No window, no engine, no clock of its own. That is what
//! makes the feature testable at all, and it is the same separation
//! [`crate::shelf`] and [`crate::rail`] already keep.
//!
//! # The pull is gone
//!
//! This module drew twice: shuffle, and **the pull** — one record, weighted
//! toward the long unplayed, offered rather than played. The owner removed it
//! on 2026-08-10 (*"please can we remove pull since it doesn't make sense
//! here"*), which also answers `docs/design/11-jobs-era-critique.md` **P9**
//! (*"Pull: explain it or rename it"*) a third way. What went with it:
//! `baz_core::history::pull_weight` and its two constants, `Ctrl+R`, the strip
//! word, and the record page's offer line. What stayed is [`Pool::from_wall`],
//! which the pull borrowed and shuffle owns.
//!
//! # The pool is the wall
//!
//! `docs/REFUSALS.md`: **no invisible shuffle pools.** *"Shuffle draws only from
//! what the wall currently shows — a shelf, the filter's matches, everything —
//! and the pool is visible… A shuffle whose source you cannot see is a
//! recommendation engine wearing a dice icon."*
//!
//! So [`Pool::from_wall`] takes exactly the three things that decide what the
//! wall shows and nothing else:
//!
//! | input | what it decides |
//! |---|---|
//! | `albums` | the collection, already in the active **group key**'s order (`crate::vm::build_shelves`) |
//! | `visible` | which of them survive the **filter query** (`crate::vm::visible_indices`) |
//! | `shelf` | which slice of `albums` one **shelf** covers, or the whole wall |
//!
//! There is no fourth input and there is deliberately nowhere to put one. A
//! future `MOOD` group key, or a mood spelled as a query, arrives here as a
//! different `albums`/`visible` pair and needs not one line of new code and not
//! one new control — which is the whole of what
//! `docs/design/critique/02-surfaces.md` means by *"vibe shuffle is a group key
//! or a filter, not a feature"*.
//!
//! # The unit is the sleeve, always
//!
//! The wall is made of **albums**, including when the query matched *tracks*:
//! `crate::vm::matching_album_ids` maps every matched track onto the album it is
//! filed under and the wall then shows that album **whole**. So a shuffle over a
//! filtered wall queues whole records too — never the three matching tracks
//! pulled out of three albums, which would be a pool the wall never showed.
//! ADR-0014 forbids flattening an album in the queue; this is the same rule one
//! step earlier, at the point where the queue is decided.
//!
//! # It ends
//!
//! A shuffle draws [`SLEEVES`] records and stops. Not because a bounded list is
//! easier — because *"the queue empties and there is silence"* is a refusal, and
//! a shuffle that refilled itself would be the radio the ledger rules out. What
//! it produces is an ordinary queue: visible in the popover, editable there, and
//! over when it is over.
//!
//! # Determinism
//!
//! Both draws take a `seed` rather than reading a clock or a global generator,
//! so every arrangement this module can produce is reproducible in a test. The
//! generator is `SplitMix64` — sixteen lines, no dependency, and good enough for
//! choosing a record.

use std::collections::HashSet;
use std::ops::Range;

use crate::vm::AlbumVm;

/// How many sleeves one shuffle draws.
///
/// An evening, not a radio station. Eight records is between four and six hours
/// — longer than anyone sits down for, short enough that the run has an end you
/// can see in the popover and reach without editing. When it ends there is
/// silence, and starting another shuffle is one press.
pub(crate) const SLEEVES: usize = 8;

/// How many of the coming draws carry a ring on the wall.
///
/// Two, from `docs/design/critique/02-surfaces.md`: *"next two draws carry faint
/// ink rings"*. Enough to say *this is where it is going next* and few enough
/// that the mark stays a mark rather than becoming a second selection.
pub(crate) const RINGED: usize = 2;

/// **What the wall is showing, and what a shuffle drew from it.**
///
/// Held by the shelf for exactly as long as the shuffle it describes is the run
/// in progress, and read by the wall to draw the two marks the refusals ledger
/// requires: non-pool sleeves dim, the next [`RINGED`] draws carry a ring.
///
/// It is a *record of a moment*. The wall goes on changing after a shuffle
/// starts — the query empties, the key changes — and when it does the pool is
/// visibly a subset of what is on screen, which is precisely the state the marks
/// exist to state. A pool that silently re-derived itself would be a pool you
/// could not see, and would be the invisible one the ledger refuses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Pool {
    /// The albums the wall showed, in wall order.
    ids: Vec<u64>,
    /// The same, for membership.
    members: HashSet<u64>,
    /// The sleeves this shuffle queued, in the order they will play.
    drawn: Vec<u64>,
}

impl Pool {
    /// **The pool: what the wall currently shows.**
    ///
    /// `visible` is the wall's own filtered index list ([`crate::vm::visible_indices`])
    /// — indices into `albums`, in wall order, which is the active group key's
    /// order. `shelf` narrows to one shelf's half-open slice of `albums`;
    /// `None` is the whole wall.
    ///
    /// The order is the wall's, not a ranking: the pool is a *place* the draw
    /// happens in, and the draw does its own shuffling.
    pub(crate) fn from_wall(
        albums: &[AlbumVm],
        visible: &[usize],
        shelf: Option<&Range<usize>>,
    ) -> Self {
        let ids: Vec<u64> = visible
            .iter()
            .filter(|index| shelf.is_none_or(|range| range.contains(index)))
            .filter_map(|&index| albums.get(index))
            .map(|album| album.id)
            .collect();
        let members = ids.iter().copied().collect();
        Self {
            ids,
            members,
            drawn: Vec::new(),
        }
    }

    /// How many sleeves the wall was showing.
    pub(crate) fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether the wall was showing nothing at all — an empty library, or a
    /// query that matched no record. A draw from it is `None` rather than an
    /// error: there is simply nothing to put on.
    pub(crate) fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The albums in the pool, **in wall order** — the pool's own order, as
    /// distinct from the order a draw puts them in.
    ///
    /// The wall itself reads the pool through [`Self::holds`] and
    /// [`Self::ringed`]; the order was the pull's, and the pull is gone
    /// (module docs). What is left needs it is the property the tests state —
    /// *the pool is the wall, in the wall's order* — so it is kept for them
    /// and gated to them, rather than left as a public accessor nothing calls.
    #[cfg(test)]
    pub(crate) fn ids(&self) -> &[u64] {
        &self.ids
    }

    /// Whether `id` is one of the sleeves this pool was drawn from — the
    /// question every tile on the wall asks to decide whether it dims.
    pub(crate) fn holds(&self, id: u64) -> bool {
        self.members.contains(&id)
    }

    /// **Draw the run**: up to [`SLEEVES`] sleeves, without replacement, in a
    /// shuffled order fixed by `seed`.
    ///
    /// Without replacement because a shuffle that could put the same record on
    /// twice in eight is not shuffling a shelf, it is rolling a die; and because
    /// the run is *visible* in the popover, where a repeat would read as a bug.
    /// A pool smaller than `count` yields the whole pool, shuffled.
    pub(crate) fn draw(&mut self, seed: u64, count: usize) -> &[u64] {
        let mut order = self.ids.clone();
        shuffle_in_place(&mut order, seed);
        order.truncate(count);
        self.drawn = order;
        &self.drawn
    }

    /// **Whether `id` is one of the next [`RINGED`] draws**, given the album the
    /// engine says is sounding.
    ///
    /// Counted from the playing record's place in the run, so the rings walk
    /// down the queue as it plays. A `playing` album this run does not hold —
    /// nothing started yet, or something else was played over the top — puts the
    /// rings on the front of the run, which is what *next* means before there is
    /// a *now*.
    ///
    /// A record that appears once in the run is ringed once; the run holds no
    /// duplicates ([`Self::draw`]), so there is no second occurrence to argue
    /// about.
    pub(crate) fn ringed(&self, id: u64, playing: Option<u64>) -> bool {
        let from = playing
            .and_then(|album| self.drawn.iter().position(|drawn| *drawn == album))
            .map_or(0, |at| at + 1);
        self.drawn
            .get(from..)
            .is_some_and(|rest| rest.iter().take(RINGED).any(|drawn| *drawn == id))
    }
}

/// Fisher–Yates over `items`, driven by `seed`.
///
/// In place, unbiased, and deterministic: the same seed over the same pool is
/// the same run every time, which is what lets a test pin an arrangement
/// instead of asserting that a shuffle is "random enough".
fn shuffle_in_place(items: &mut [u64], seed: u64) {
    let mut state = seed_state(seed);
    for index in (1..items.len()).rev() {
        // The modulus is `index + 1`, which is at most `items.len()`, so the
        // remainder is a valid index by construction and the conversion back
        // cannot lose anything. The modulo bias over a range this small against
        // 2^64 is not measurable; the alternative is rejection sampling for a
        // record player.
        let span = u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1);
        let pick = usize::try_from(next(&mut state) % span).unwrap_or(0);
        items.swap(index, pick);
    }
}

/// The generator's starting state. Named so that both draws seed identically.
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
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;
    use crate::vm::{AlbumArtistVm, EditionKey, EditionVm, ReplayGainCoverage, TrackVm};

    /// The length every fixture track declares, in milliseconds.
    const TRACK_MS: u64 = 200_000;

    fn track(path: &str) -> TrackVm {
        TrackVm {
            number: None,
            disc: None,
            title: path.to_owned(),
            artist: None,
            duration: Some(Duration::from_millis(TRACK_MS)),
            path: PathBuf::from(path),
            bytes: None,
        }
    }

    /// One album, `name`, with one edition holding two tracks under `/m/name/`.
    fn album(name: &str) -> AlbumVm {
        let tracks: Vec<TrackVm> = (1..=2)
            .map(|side| track(&format!("/m/{name}/{side}.flac")))
            .collect();
        AlbumVm {
            id: crate::vm::album_id(baz_core::index::AlbumArtist::Named(name), Some(name)),
            title: Some(name.to_owned()),
            artist: AlbumArtistVm::Named(name.to_owned()),
            track_artists_vary: false,
            year: None,
            genre: None,
            first_seen_ns: None,
            first_track: tracks[0].path.clone(),
            editions: vec![EditionVm {
                key: EditionKey(None),
                detail: None,
                bitrate: None,
                bit_depth: None,
                sample_rate: None,
                replay_gain: ReplayGainCoverage {
                    album: 0,
                    track: 0,
                    total: tracks.len(),
                },
                tracks,
            }],
        }
    }

    /// Five albums, `a`..`e`, two tracks each.
    fn wall() -> Vec<AlbumVm> {
        ["a", "b", "c", "d", "e"].iter().map(|n| album(n)).collect()
    }

    fn ids(albums: &[AlbumVm], names: &[&str]) -> Vec<u64> {
        names
            .iter()
            .map(|name| {
                albums
                    .iter()
                    .find(|album| album.title.as_deref() == Some(*name))
                    .expect("fixture album")
                    .id
            })
            .collect()
    }

    fn names(albums: &[AlbumVm], ids: &[u64]) -> Vec<String> {
        ids.iter()
            .map(|id| {
                albums
                    .iter()
                    .find(|album| album.id == *id)
                    .and_then(|album| album.title.clone())
                    .unwrap_or_default()
            })
            .collect()
    }

    /// **The pool is the wall**, in every combination of the three things that
    /// decide what the wall shows.
    #[test]
    fn the_pool_is_what_the_key_the_query_and_the_shelf_leave_visible() {
        let albums = wall();
        let all: Vec<usize> = (0..albums.len()).collect();

        // No query, no shelf: the whole wall, in the key's own order.
        let whole = Pool::from_wall(&albums, &all, None);
        assert_eq!(names(&albums, whole.ids()), ["a", "b", "c", "d", "e"]);

        // A query: only the matches, still in wall order.
        let filtered = Pool::from_wall(&albums, &[1, 3], None);
        assert_eq!(names(&albums, filtered.ids()), ["b", "d"]);

        // A shelf: only that shelf's slice of the wall.
        let shelf = Pool::from_wall(&albums, &all, Some(&(1..3)));
        assert_eq!(names(&albums, shelf.ids()), ["b", "c"]);

        // A shelf *and* a query: the intersection, and nothing else.
        let both = Pool::from_wall(&albums, &[0, 2, 4], Some(&(1..4)));
        assert_eq!(names(&albums, both.ids()), ["c"]);

        // A different group key is a different `albums` order, and the pool is
        // that order — no re-sorting of its own.
        let mut reordered = albums.clone();
        reordered.reverse();
        let by_other_key = Pool::from_wall(&reordered, &all, None);
        assert_eq!(
            names(&reordered, by_other_key.ids()),
            ["e", "d", "c", "b", "a"]
        );
    }

    /// Every empty case, because each one is a real state of the wall.
    #[test]
    fn an_empty_wall_is_an_empty_pool_and_draws_nothing() {
        let albums = wall();
        // An empty library.
        assert!(Pool::from_wall(&[], &[], None).is_empty());
        // A query that matched nothing.
        assert!(Pool::from_wall(&albums, &[], None).is_empty());
        // A shelf that the query emptied.
        assert!(Pool::from_wall(&albums, &[0], Some(&(2..4))).is_empty());
        // A shelf range past the end of the wall.
        assert!(Pool::from_wall(&albums, &[0, 1], Some(&(9..12))).is_empty());
        // An index the wall no longer holds is skipped rather than panicking:
        // the filter and the album list are rebuilt separately under a scan.
        assert!(Pool::from_wall(&albums, &[99], None).is_empty());

        let mut empty = Pool::from_wall(&albums, &[], None);
        assert!(empty.draw(1, SLEEVES).is_empty());
    }

    /// **The queue a shuffle builds**: whole sleeves, no repeats, bounded, and
    /// the same every time for a given seed.
    #[test]
    fn a_draw_is_deterministic_bounded_and_never_repeats_a_sleeve() {
        let albums = wall();
        let all: Vec<usize> = (0..albums.len()).collect();

        let mut pool = Pool::from_wall(&albums, &all, None);
        let first = pool.draw(0x5EED_u64, 3).to_vec();
        assert_eq!(first.len(), 3, "the draw is capped at what was asked for");

        // Deterministic: the same seed over the same pool is the same run.
        let mut again = Pool::from_wall(&albums, &all, None);
        assert_eq!(again.draw(0x5EED_u64, 3), first.as_slice());

        // No repeats, and every draw is a member of the pool.
        let unique: HashSet<u64> = first.iter().copied().collect();
        assert_eq!(unique.len(), first.len());
        assert!(first.iter().all(|id| pool.holds(*id)));

        // A pool smaller than the ask yields the pool, shuffled — never padded.
        let mut small = Pool::from_wall(&albums, &[2], None);
        assert_eq!(small.draw(7, SLEEVES).len(), 1);

        // A different seed is a different arrangement (over 5! = 120 orders,
        // a fixed pair that collided would be a real defect in the generator).
        let mut other = Pool::from_wall(&albums, &all, None);
        assert_ne!(
            other.draw(9, 5),
            Pool::from_wall(&albums, &all, None).draw(10, 5)
        );

        // Every seed yields a permutation of the whole pool when it asks for one.
        for seed in 0..64_u64 {
            let mut each = Pool::from_wall(&albums, &all, None);
            let run: HashSet<u64> = each.draw(seed, 5).iter().copied().collect();
            assert_eq!(run.len(), 5, "seed {seed} lost or repeated a sleeve");
        }
    }

    /// The two marks the refusals ledger requires: everything outside the pool
    /// dims, and the next [`RINGED`] draws are ringed.
    #[test]
    fn the_pool_marks_its_members_and_the_next_two_draws() {
        let albums = wall();
        let mut pool = Pool::from_wall(&albums, &[0, 1, 2], None);
        let run = pool.draw(4, SLEEVES).to_vec();
        let outside = ids(&albums, &["d", "e"]);

        for id in pool.ids() {
            assert!(pool.holds(*id), "a pool member never dims");
        }
        for id in &outside {
            assert!(!pool.holds(*id), "what the shuffle cannot play dims");
        }

        // Nothing playing yet: the rings sit on the front of the run.
        assert!(pool.ringed(run[0], None));
        assert!(pool.ringed(run[1], None));
        assert!(!pool.ringed(run[2], None));

        // Once the first is sounding, the rings walk on.
        assert!(!pool.ringed(run[0], Some(run[0])));
        assert!(pool.ringed(run[1], Some(run[0])));
        assert!(pool.ringed(run[2], Some(run[0])));

        // At the end of the run there is nothing next, and nothing is ringed —
        // silence is not announced with a mark either.
        assert!(!pool.ringed(run[0], Some(run[2])));
        assert!(!pool.ringed(run[1], Some(run[2])));

        // Something else was played over the top: the run has not started, so
        // the rings are back on its front rather than nowhere.
        assert!(pool.ringed(run[0], Some(outside[0])));

        // A record outside the run is never ringed, whatever is playing.
        assert!(!pool.ringed(outside[1], None));
    }
}
