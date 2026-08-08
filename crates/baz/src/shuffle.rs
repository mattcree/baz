//! **Shuffle, and the pull**: the two draws baz makes, and the one rule they
//! share — *you can see what they are drawing from*.
//!
//! Everything here is pure: a list of albums, the wall's own filter, a ledger
//! snapshot and a seed in; album ids out. No window, no engine, no clock of its
//! own. That is what makes the two features testable at all, and it is the same
//! separation [`crate::shelf`] and [`crate::rail`] already keep.
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
use std::time::SystemTime;

use baz_core::history::{History, PULL_NEVER_WEIGHT, Recency};

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

    /// The albums in the pool, in wall order.
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

/// **How strongly the pull should favour one record.**
///
/// [`History::pull_weight`] answers this per *track* — one per day since it was
/// last heard, capped at a year, [`PULL_NEVER_WEIGHT`] for a track never played,
/// never zero. An album is a stack of tracks, so the album's weight is the
/// weight of its **most recently played** track, which is its smallest: putting
/// side A on this morning means you have heard this record today, whatever the
/// rest of it says.
///
/// Every edition is considered, not just the one on screen. Hearing the FLAC rip
/// is hearing the record.
///
/// With no ledger at all every album weighs [`PULL_NEVER_WEIGHT`], which makes
/// the pull a uniform draw — the honest behaviour for a library baz has no
/// record of, and not a reason to refuse to pull.
pub(crate) fn album_weight(album: &AlbumVm, history: Option<&History>, now: SystemTime) -> u32 {
    let Some(history) = history else {
        return PULL_NEVER_WEIGHT;
    };
    album
        .editions
        .iter()
        .flat_map(|edition| edition.tracks.iter())
        .map(|track| history.pull_weight(&track.path, now))
        .min()
        .unwrap_or(PULL_NEVER_WEIGHT)
}

/// When this record was last heard, as the PLAYED key's own bucket — the fact
/// the pull states out loud.
///
/// The most recent play of any track of any edition, for [`album_weight`]'s
/// reason. [`Recency`] is ordered most-recent-first, so the smallest bucket is
/// the album's.
pub(crate) fn last_played(album: &AlbumVm, history: Option<&History>, now: SystemTime) -> Recency {
    let Some(history) = history else {
        return Recency::Never;
    };
    album
        .editions
        .iter()
        .flat_map(|edition| edition.tracks.iter())
        .map(|track| history.recency(&track.path, now))
        .min()
        .unwrap_or(Recency::Never)
}

/// The line the pull prints: *"Last played 3 years ago"*, or *"Never played"*.
///
/// The ledger has the date, so this is a reading rather than a claim
/// (`docs/REFUSALS.md`: history records, it never performs). It is the only
/// number the pull shows — no score, no weight, no percentage. A weight is an
/// implementation detail of a draw, and printing it would turn a suggestion into
/// a ranking.
pub(crate) fn pull_note(recency: Recency) -> String {
    match recency {
        Recency::Never => "Never played".to_owned(),
        Recency::Unrecorded => "No play recorded".to_owned(),
        bucket => format!("Last played {}", lowercase_first(&bucket.label())),
    }
}

/// `This evening` → `this evening`, so it can follow *Last played*.
fn lowercase_first(label: &str) -> String {
    let mut chars = label.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().collect::<String>() + chars.as_str()
    })
}

/// **The pull**: one sleeve from the pool, weighted toward the long unplayed.
///
/// `exclude` is the record the pull is already showing, and it is left out so
/// that pressing again gives you a *different* suggestion — the gesture is
/// "not that one, try again", and a re-pull that could answer with the same
/// sleeve would read as a control that did nothing. It is dropped when it is the
/// only record in the pool, because refusing to answer would be worse than
/// repeating.
///
/// `None` when the pool is empty. Nothing is ever weighted to zero
/// ([`History::pull_weight`]), so every record in the pool is reachable; the
/// weighting is a bias, never a filter.
pub(crate) fn pull(
    albums: &[AlbumVm],
    pool: &Pool,
    history: Option<&History>,
    now: SystemTime,
    seed: u64,
    exclude: Option<u64>,
) -> Option<u64> {
    let mut candidates: Vec<&AlbumVm> = pool
        .ids()
        .iter()
        .filter_map(|id| albums.iter().find(|album| album.id == *id))
        .collect();
    if candidates.len() > 1 {
        candidates.retain(|album| Some(album.id) != exclude);
    }
    if candidates.is_empty() {
        return None;
    }
    let weights: Vec<u64> = candidates
        .iter()
        .map(|album| u64::from(album_weight(album, history, now)))
        .collect();
    let total: u64 = weights.iter().sum();
    if total == 0 {
        // Unreachable while `pull_weight` never returns zero, and cheaper to
        // answer than to prove: the first candidate is a true member of the
        // pool, which is the only promise this function makes.
        return candidates.first().map(|album| album.id);
    }
    let mut ticket = next(&mut seed_state(seed)) % total;
    for (album, weight) in candidates.iter().zip(&weights) {
        if ticket < *weight {
            return Some(album.id);
        }
        ticket -= *weight;
    }
    candidates.last().map(|album| album.id)
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
    use std::time::{Duration, UNIX_EPOCH};

    use baz_core::history::{History, PULL_DAY_CAP, PULL_NEVER_WEIGHT, PlayRecord};

    use super::*;
    use crate::vm::{AlbumArtistVm, EditionKey, EditionVm, ReplayGainCoverage, TrackVm};

    const DAY: u64 = 24 * 60 * 60;
    /// A fixed "now" well past the epoch, so a ledger can carry dates before it.
    const NOW: u64 = 1_700_000_000;
    /// The length every fixture track declares, in milliseconds — long enough
    /// that a full listen is unambiguously a play rather than a skip.
    const TRACK_MS: u64 = 200_000;

    fn at(unix_s: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(unix_s)
    }

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

    /// A ledger holding one full play of `path` at `when`, per entry —
    /// written through `baz-core`'s own encoder, so the fixture is a file the
    /// real reader would meet rather than a shape invented here.
    fn ledger(plays: &[(&str, u64)]) -> History {
        let mut text = String::new();
        for (path, when) in plays {
            let record = PlayRecord::new(PathBuf::from(path), *when, TRACK_MS, Some(TRACK_MS))
                .expect("a full listen is a play");
            text.push_str(&record.to_line());
        }
        History::from_reader(text.as_bytes())
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
        assert_eq!(pull(&albums, &empty, None, at(NOW), 1, None), None);
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

    /// **The weighting**, against a ledger with known dates — the cap, the
    /// never-played weight, and the floor of one.
    #[test]
    fn an_albums_weight_is_its_most_recent_play_capped_at_a_year() {
        let albums = wall();
        let history = ledger(&[
            // `a` was heard this morning; its second side a year and a half ago.
            ("/m/a/1.flac", NOW - 2 * 60 * 60),
            ("/m/a/2.flac", NOW - 500 * DAY),
            // `b`, ten days ago.
            ("/m/b/1.flac", NOW - 10 * DAY),
            // `c`, well past the cap.
            ("/m/c/1.flac", NOW - 900 * DAY),
            // `d` was *skipped* only — never played.
        ]);
        let now = at(NOW);
        let weight = |name: &str| {
            album_weight(
                albums
                    .iter()
                    .find(|album| album.title.as_deref() == Some(name))
                    .expect("fixture album"),
                Some(&history),
                now,
            )
        };

        // The most recent play wins: hearing side A today is hearing the record
        // today, whatever side B's stamp says.
        assert_eq!(weight("a"), 1);
        assert_eq!(weight("b"), 11, "one per day since, plus the floor of one");
        assert_eq!(weight("c"), PULL_DAY_CAP + 1, "the 366 cap, and not 901");
        assert_eq!(weight("d"), PULL_NEVER_WEIGHT);
        assert_eq!(weight("e"), PULL_NEVER_WEIGHT);

        // Never-played beats a year ago, which is the whole point of the pull.
        assert!(weight("d") > weight("c"));
        assert!(weight("c") > weight("b"));
        assert!(weight("b") > weight("a"));
        // Nothing is ever zero: a record heard an hour ago is still reachable.
        assert!(["a", "b", "c", "d", "e"].iter().all(|n| weight(n) > 0));

        // No ledger at all is a uniform draw rather than a refusal.
        assert_eq!(album_weight(&albums[0], None, now), PULL_NEVER_WEIGHT);
    }

    /// The line the pull prints, from the ledger's own buckets.
    #[test]
    fn the_pull_states_when_the_record_was_last_heard() {
        let albums = wall();
        let history = ledger(&[
            ("/m/a/1.flac", NOW - 2 * 60 * 60),
            ("/m/b/1.flac", NOW - 800 * DAY),
        ]);
        let now = at(NOW);
        let of = |name: &str| {
            last_played(
                albums
                    .iter()
                    .find(|album| album.title.as_deref() == Some(name))
                    .expect("fixture album"),
                Some(&history),
                now,
            )
        };
        assert_eq!(pull_note(of("a")), "Last played this evening");
        assert_eq!(pull_note(of("b")), "Last played 2 years ago");
        assert_eq!(pull_note(of("e")), "Never played");
        assert_eq!(pull_note(Recency::Unrecorded), "No play recorded");
        // Without a ledger the honest statement is the one the ledger would
        // make about a library it has no record of.
        assert_eq!(last_played(&albums[0], None, now), Recency::Never);
    }

    /// The pull is weighted, not filtered: the long-unplayed dominate, and the
    /// recently played are still reachable.
    #[test]
    fn the_pull_leans_hard_toward_the_long_unplayed_without_excluding_anything() {
        let albums = wall();
        let all: Vec<usize> = (0..albums.len()).collect();
        let pool = Pool::from_wall(&albums, &all, None);
        // `a`–`d` were all heard today (weight 1); `e` never (weight 367).
        let history = ledger(&[
            ("/m/a/1.flac", NOW - 60),
            ("/m/b/1.flac", NOW - 60),
            ("/m/c/1.flac", NOW - 60),
            ("/m/d/1.flac", NOW - 60),
        ]);
        let now = at(NOW);
        let never = ids(&albums, &["e"])[0];

        let mut drew_never = 0;
        let mut drew_something_else = 0;
        for seed in 0..200_u64 {
            let drawn = pull(&albums, &pool, Some(&history), now, seed, None).expect("a draw");
            if drawn == never {
                drew_never += 1;
            } else {
                drew_something_else += 1;
            }
        }
        // 367 : 4 in the weights. Anything short of overwhelming here means the
        // weighting is not being applied.
        assert!(
            drew_never > 190,
            "the never-played record should dominate; drew it {drew_never} times"
        );
        // …and nothing is excluded. Over 200 seeds a weight-1 record among
        // weight-367 ones is expected about twice; asserting only that the
        // *mechanism* admits them keeps this test from being a coin flip.
        assert_eq!(drew_never + drew_something_else, 200);
        assert!(
            (0..2000_u64)
                .filter_map(|seed| pull(&albums, &pool, Some(&history), now, seed, None))
                .any(|drawn| drawn != never),
            "a recently played record must still be reachable"
        );
    }

    /// **A re-pull is a different suggestion.** `Ctrl+R` means "not that one".
    #[test]
    fn a_re_pull_never_answers_with_the_record_it_is_already_showing() {
        let albums = wall();
        let all: Vec<usize> = (0..albums.len()).collect();
        let pool = Pool::from_wall(&albums, &all, None);
        let now = at(NOW);
        for seed in 0..200_u64 {
            let showing = pull(&albums, &pool, None, now, seed, None).expect("a draw");
            let again = pull(&albums, &pool, None, now, seed + 1, Some(showing)).expect("a draw");
            assert_ne!(again, showing, "seed {seed} re-pulled the same sleeve");
        }
        // …unless the pool holds exactly one record, where refusing to answer
        // would be worse than repeating.
        let one = Pool::from_wall(&albums, &[3], None);
        let only = ids(&albums, &["d"])[0];
        assert_eq!(pull(&albums, &one, None, now, 5, Some(only)), Some(only));
    }

    /// Every draw the pull can make is a member of the pool — it can no more
    /// suggest an unseen record than shuffle can play one.
    #[test]
    fn the_pull_draws_only_from_what_the_wall_shows() {
        let albums = wall();
        let pool = Pool::from_wall(&albums, &[1, 2], None);
        let now = at(NOW);
        for seed in 0..200_u64 {
            let drawn = pull(&albums, &pool, None, now, seed, None).expect("a draw");
            assert!(pool.holds(drawn), "seed {seed} drew from outside the wall");
        }
    }
}
