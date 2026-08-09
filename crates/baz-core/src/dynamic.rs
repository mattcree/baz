//! **Dynamic playlists**: a rule you can say out loud, and the draw it makes.
//!
//! A *dynamic playlist* in baz is not a list. It is a **rule over the library
//! that produces an ordered list of records when a person presses it** — and
//! the rule, not the list, is the durable thing. Nothing here stores a result,
//! watches a folder, or re-derives itself in the background: `docs/REFUSALS.md`
//! requires that generation be *an act, never a condition*, and a module that
//! only ever answers a call is that requirement expressed as a type.
//!
//! The design study is [`docs/design/14-dynamic-playlists.md`]; the decision is
//! [`docs/adr/0033-dynamic-playlists.md`]. This module is §3 of the study made
//! executable, and it is deliberately the *whole* of the machinery — the point
//! the study argues at length is that these rules are ledger arithmetic and
//! nothing more, and a reader who doubts it should be able to finish this file.
//!
//! # The sentence rule
//!
//! > **Every rule states itself in one sentence, and the sentence is what the
//! > listener presses.**
//!
//! [`Rule::sentence`] is not a label for a row; it *is* the row, and
//! `every_rule_states_itself_in_one_sentence` holds the set to it. A rule whose
//! sentence cannot be written is a rule baz does not get to have — which is the
//! test that keeps this surface from becoming the thing the product exists
//! against, a list you were given for reasons you cannot inspect.
//!
//! # What it costs, and what it reads
//!
//! Three ledger fields ([`History::track`]'s `last_played_unix_s` and nothing
//! else, per rule) and one index fact (which tracks a record has). No model, no
//! embedding, no training, no file of its own, no thread and no clock. The
//! ledger's own three-surface rule (`crates/baz-core/src/history/read.rs`'s —
//! *"There is deliberately no fourth"*) is **untouched**: everything below is
//! built from the public [`History::track`] and [`History::recency`], so this
//! module adds no way to ask the ledger anything it would not already answer.
//!
//! Cost is linear in tracks and it is one hash lookup per track. Measured in
//! release on the development machine over a synthetic 10 000-record library of
//! 10 tracks each — 100 000 tracks, at the scale `docs/research/05-personas.md`
//! calls Marta's — against a 30 000-play ledger: **3.2–3.3 ms per rule**, best
//! of five. That is a fifth of a 60 Hz frame and so far too much to spend on
//! one, and nothing at all beside a place change. Hence ADR-0033's surface
//! rule — *draw when the page is entered and when a play lands, never per
//! frame* — which is the discipline ADR-0030 §4 already gave the returns lane.
//!
//! The three rules share their whole per-track loop, so a fused pass would make
//! three rules cost about what one does. It is **not** written that way, because
//! 10 ms once on a navigation is not a problem anybody has yet, and the study's
//! own standing rule is that an optimisation needs a measurement first.
//!
//! [`docs/design/14-dynamic-playlists.md`]: https://github.com/mattcree/baz/blob/main/docs/design/14-dynamic-playlists.md
//! [`docs/adr/0033-dynamic-playlists.md`]: https://github.com/mattcree/baz/blob/main/docs/adr/0033-dynamic-playlists.md

use std::path::Path;
use std::time::SystemTime;

use crate::history::{History, YEAR_DAYS};

/// Seconds in a day — the ledger's own unit, restated here so the year below
/// is arithmetic rather than a magic number.
const DAY_SECS: u64 = 24 * 60 * 60;

/// **A rule the listener could state in a sentence.**
///
/// The set is **closed**, and that is the design rather than an implementation
/// state. Every member answers a question about the listener's *own shelf*
/// that no other surface in baz answers: the returns lane says what you have
/// touched, `RECENTLY ADDED` says what arrived, `CONTINUE` says where you
/// stopped — and none of them says what you own and have not heard.
///
/// A sixth variant needs the argument ADR-0033 §4 sets out, and *"there is
/// room on the page"* is not one. What is refused by name, with reasons, is in
/// the study's §3.4: anything ranked by play count, anything about skips,
/// anything that reads a mood out of a title, and anything whose pool is
/// larger than a sentence.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// Records the ledger has never recorded a play of.
    ///
    /// A **skip is not a play** here, exactly as it is not one for
    /// [`History::recency`]: starting a record and leaving it is not having
    /// heard it, and a rule that counted it would quietly empty itself for a
    /// listener who auditions.
    NeverPlayed,
    /// Records last played more than [`YEAR_DAYS`] ago.
    ///
    /// This is **the pull's arithmetic, re-homed**. `crates/baz/src/shuffle.rs`
    /// weighted a draw by days-since-last-play and offered one sleeve;
    /// ADR-0033 keeps the fact and drops the offer, because the fact is
    /// something a listener asked for and the offer was something baz
    /// volunteered.
    NotForAYear,
    /// Records with at least one track played and at least one never played.
    ///
    /// The album-first question no track-first product asks. It is a statement
    /// about the *record* — this one is unfinished — and never about the
    /// listener, which is what keeps it on the right side of
    /// `docs/REFUSALS.md`'s *history records, it never performs*.
    PartlyHeard,
}

impl Rule {
    /// Every rule, in the order a surface draws them.
    ///
    /// Ordered by how specific the question is, narrowest last: what you have
    /// never played, what you have not played lately, and what you left in the
    /// middle. A surface may draw fewer (a rule that does not stand is absent —
    /// [`Rule::stands`]); it may not draw them in another order, because the
    /// order is the only thing on the page that is not a sentence.
    pub const ALL: [Self; 3] = [Self::NeverPlayed, Self::NotForAYear, Self::PartlyHeard];

    /// **The sentence.** What the row says, and the whole of what it promises.
    ///
    /// `count` is what [`Rule::draw`] returned, so the sentence states the size
    /// of the pool before the pool is drawn — the anti-invisible-pool rule
    /// (`docs/REFUSALS.md`) met by saying how big the thing behind the door is.
    /// It is a fact about the shelf, never about the listener: there is no
    /// figure here that could be read as a score, a streak or a total.
    #[must_use]
    pub fn sentence(self, count: usize) -> String {
        let records = if count == 1 { "record" } else { "records" };
        match self {
            Self::NeverPlayed => format!("{count} {records} you have never played"),
            Self::NotForAYear => {
                format!("{count} {records} you have not played in over a year")
            }
            Self::PartlyHeard => format!("{count} {records} you have only heard part of"),
        }
    }

    /// Whether the row stands at all, given what it drew and how big the
    /// library is.
    ///
    /// Two conditions, and both are ADR-0030 §6's *a section is absent, not
    /// empty* applied one level down:
    ///
    /// 1. **An empty draw draws nothing.** A row reading *"0 records you have
    ///    never played"* is a control that cannot act, and `docs/REFUSALS.md`'s
    ///    accessibility entry has no time for those.
    /// 2. **A draw that is the whole library draws nothing either.** On a
    ///    freshly scanned collection every record is unplayed, so
    ///    [`Rule::NeverPlayed`] would be a row saying *"your library"* — true,
    ///    useless, and a door to the place one row above it. This is the same
    ///    clause ADR-0030 §6 already gives `RECENTLY ADDED` (*absent* when
    ///    every row came from one first scan), for the same reason.
    #[must_use]
    pub fn stands(self, drawn: usize, library: usize) -> bool {
        drawn > 0 && drawn < library
    }

    /// **Draw the rule**: the records that satisfy it, in the rule's own order.
    ///
    /// `records` is the library as `(id, tracks)` — every track of every
    /// edition, because hearing the FLAC rip is hearing the record
    /// (`crates/baz/src/shuffle.rs::album_weight` settled that and this agrees
    /// with it deliberately). Borrowed rather than owned throughout: a draw
    /// allocates the answer and nothing else.
    ///
    /// The order:
    ///
    /// - [`Rule::NeverPlayed`] and [`Rule::PartlyHeard`] keep **the order they
    ///   were given**, which is the caller's library order. There is no second
    ///   ordering to explain, and no score.
    /// - [`Rule::NotForAYear`] is **longest unheard first**, which is the
    ///   rule's own axis rather than a ranking over it — the one fact the
    ///   sentence already names. Ties keep input order (the sort is stable), so
    ///   two launches over one ledger draw one list.
    ///
    /// A record with no tracks cannot be heard and is never drawn.
    pub fn draw<'tracks, Records, Tracks>(
        self,
        records: Records,
        history: &History,
        now: SystemTime,
    ) -> Vec<u64>
    where
        Records: IntoIterator<Item = (u64, Tracks)>,
        Tracks: IntoIterator<Item = &'tracks Path>,
    {
        let mut drawn: Vec<(u64, u64)> = Vec::new();
        for (id, tracks) in records {
            let mut played = 0_usize;
            let mut total = 0_usize;
            // The most recent play of any track is the record's own last play,
            // which is `shuffle::last_played`'s rule: putting side A on this
            // morning means you have heard this record today.
            let mut last: Option<u64> = None;
            for track in tracks {
                total += 1;
                if let Some(at) = history.track(track).last_played_unix_s {
                    played += 1;
                    last = Some(last.map_or(at, |had: u64| had.max(at)));
                }
            }
            if total == 0 {
                continue;
            }
            let keep = match self {
                Self::NeverPlayed => played == 0,
                Self::NotForAYear => last.is_none_or(|at| stale(at, now)),
                Self::PartlyHeard => played > 0 && played < total,
            };
            if keep {
                // A never-played record is unheard for longer than any played
                // one, so it sorts first under `NotForAYear`'s order — the same
                // judgement `PULL_NEVER_WEIGHT` makes one past the day cap.
                drawn.push((id, last.unwrap_or(0)));
            }
        }
        if self == Self::NotForAYear {
            drawn.sort_by_key(|&(_, last)| last);
        }
        drawn.into_iter().map(|(id, _)| id).collect()
    }
}

/// Whether a play at `at` (Unix seconds, UTC) is more than a year before `now`.
///
/// A timestamp in the future — a clock that was wrong, or has since been
/// corrected — reads as *recent* rather than underflowing into *ancient*, which
/// is [`History::recency`]'s own choice and the safe direction: the failure is
/// a record missing from a draw, not a record you played this morning being
/// offered as forgotten.
fn stale(at: u64, now: SystemTime) -> bool {
    let now = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    now.saturating_sub(at) > YEAR_DAYS * DAY_SECS
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{DAY_SECS, Rule};
    use crate::history::{History, PlayRecord};
    use crate::protocol::PlayOutcome;

    /// A fixed "now" well past the epoch, so a year can be subtracted from it.
    const NOW: u64 = 1_800_000_000;

    fn at(unix_s: u64) -> std::time::SystemTime {
        std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(unix_s)
    }

    fn path(name: &str) -> PathBuf {
        PathBuf::from(format!("/music/{name}.flac"))
    }

    /// A ledger built from `(track, outcome, when)` triples.
    ///
    /// Through the real line format rather than by constructing a [`History`]
    /// directly, so these tests exercise the file every rule below will
    /// actually meet. The outcome is set explicitly rather than derived from
    /// `listened_ms`: what is under test here is what a rule does with a
    /// *played* line versus a *skipped* one, and `classify`'s threshold is
    /// `crate::history`'s own business and already has its own tests.
    fn ledger(rows: &[(&str, PlayOutcome, u64)]) -> History {
        let mut lines = String::new();
        for (name, outcome, when) in rows {
            let record = PlayRecord {
                started_unix_s: *when,
                outcome: *outcome,
                listened_ms: 120_000,
                track_ms: Some(240_000),
                path: path(name),
            };
            lines.push_str(&record.to_line());
        }
        History::from_reader(lines.as_bytes())
    }

    /// Two records of two tracks each, in library order.
    fn library() -> Vec<(u64, Vec<PathBuf>)> {
        vec![
            (1, vec![path("a1"), path("a2")]),
            (2, vec![path("b1"), path("b2")]),
        ]
    }

    fn draw(rule: Rule, records: &[(u64, Vec<PathBuf>)], history: &History, now: u64) -> Vec<u64> {
        rule.draw(
            records
                .iter()
                .map(|(id, tracks)| (*id, tracks.iter().map(PathBuf::as_path))),
            history,
            at(now),
        )
    }

    /// The rule the whole surface rests on: no sentence, no row.
    #[test]
    fn every_rule_states_itself_in_one_sentence() {
        let mut seen = HashSet::new();
        for rule in Rule::ALL {
            let sentence = rule.sentence(3);
            assert!(sentence.starts_with("3 records "), "{rule:?}: {sentence}");
            assert!(
                !sentence.contains('.') && !sentence.contains(';'),
                "one sentence, unpunctuated — the surface supplies the full stop: {sentence}"
            );
            assert!(
                seen.insert(sentence),
                "two rules may not say the same thing"
            );
        }
        assert_eq!(
            Rule::ALL.len(),
            3,
            "the set is closed; adding needs ADR-0033 §4"
        );
    }

    /// Singular and plural, because a row reading "1 records" is the kind of
    /// seam this product does not ship.
    #[test]
    fn the_sentence_counts_one_record_in_the_singular() {
        assert_eq!(
            Rule::NeverPlayed.sentence(1),
            "1 record you have never played"
        );
        assert_eq!(
            Rule::NeverPlayed.sentence(0),
            "0 records you have never played"
        );
    }

    #[test]
    fn a_skip_is_not_a_play() {
        let history = ledger(&[("a1", PlayOutcome::Skipped, NOW - 60)]);
        assert_eq!(
            draw(Rule::NeverPlayed, &library(), &history, NOW),
            vec![1, 2],
            "a record you started and abandoned is a record you have not heard"
        );
        assert!(
            draw(Rule::PartlyHeard, &library(), &history, NOW).is_empty(),
            "and it has not been partly heard either"
        );
    }

    #[test]
    fn partly_heard_needs_one_played_track_and_one_unplayed() {
        let history = ledger(&[
            ("a1", PlayOutcome::Played, NOW - 60),
            ("b1", PlayOutcome::Played, NOW - 60),
            ("b2", PlayOutcome::Played, NOW - 60),
        ]);
        assert_eq!(
            draw(Rule::PartlyHeard, &library(), &history, NOW),
            vec![1],
            "record 1 is half heard; record 2 is finished"
        );
        assert!(draw(Rule::NeverPlayed, &library(), &history, NOW).is_empty());
    }

    #[test]
    fn not_for_a_year_takes_the_most_recent_play_of_any_track() {
        let year = 366 * DAY_SECS;
        let history = ledger(&[
            // Record 1: one track heard long ago, one heard this morning.
            ("a1", PlayOutcome::Played, NOW - year),
            ("a2", PlayOutcome::Played, NOW - 3600),
            // Record 2: heard only long ago.
            ("b1", PlayOutcome::Played, NOW - year),
        ]);
        assert_eq!(
            draw(Rule::NotForAYear, &library(), &history, NOW),
            vec![2],
            "putting side B on this morning means you have heard the record"
        );
    }

    #[test]
    fn not_for_a_year_draws_the_longest_unheard_first() {
        let history = ledger(&[
            ("a1", PlayOutcome::Played, NOW - 400 * DAY_SECS),
            ("b1", PlayOutcome::Played, NOW - 900 * DAY_SECS),
        ]);
        assert_eq!(
            draw(Rule::NotForAYear, &library(), &history, NOW),
            vec![2, 1],
            "record 2 has waited longer, so it leads — the rule's own axis"
        );
    }

    #[test]
    fn a_record_never_played_leads_the_year_draw() {
        let history = ledger(&[("a1", PlayOutcome::Played, NOW - 400 * DAY_SECS)]);
        assert_eq!(
            draw(Rule::NotForAYear, &library(), &history, NOW),
            vec![2, 1],
            "never heard outranks heard-once-in-2024, as PULL_NEVER_WEIGHT does"
        );
    }

    #[test]
    fn a_clock_that_ran_backwards_reads_as_recent() {
        let history = ledger(&[
            ("a1", PlayOutcome::Played, NOW + 10 * DAY_SECS),
            ("a2", PlayOutcome::Played, NOW + 10 * DAY_SECS),
            ("b1", PlayOutcome::Played, NOW - 900 * DAY_SECS),
            ("b2", PlayOutcome::Played, NOW - 900 * DAY_SECS),
        ]);
        assert_eq!(
            draw(Rule::NotForAYear, &library(), &history, NOW),
            vec![2],
            "a future stamp is a wrong clock, and the safe reading is 'heard'"
        );
    }

    #[test]
    fn no_ledger_at_all_is_an_answerable_state() {
        let history = History::default();
        assert_eq!(
            draw(Rule::NeverPlayed, &library(), &history, NOW),
            vec![1, 2]
        );
        assert_eq!(
            draw(Rule::NotForAYear, &library(), &history, NOW),
            vec![1, 2]
        );
        assert!(draw(Rule::PartlyHeard, &library(), &history, NOW).is_empty());
    }

    #[test]
    fn a_record_with_no_tracks_is_never_drawn() {
        let records = vec![(9_u64, Vec::<PathBuf>::new())];
        for rule in Rule::ALL {
            assert!(
                draw(rule, &records, &History::default(), NOW).is_empty(),
                "{rule:?} drew a record that has nothing to play"
            );
        }
    }

    #[test]
    fn a_row_is_absent_rather_than_empty_or_total() {
        assert!(!Rule::NeverPlayed.stands(0, 400), "nothing to offer");
        assert!(
            !Rule::NeverPlayed.stands(400, 400),
            "a fresh library: the row would be a door to the Library place"
        );
        assert!(Rule::NeverPlayed.stands(399, 400));
        assert!(Rule::NeverPlayed.stands(1, 2));
    }

    /// Same ledger, same library, same answer — twice. A draw a test cannot
    /// pin is a draw a listener cannot trust.
    #[test]
    fn a_draw_is_deterministic() {
        let history = ledger(&[
            ("a1", PlayOutcome::Played, NOW - 900 * DAY_SECS),
            ("b1", PlayOutcome::Played, NOW - 900 * DAY_SECS),
        ]);
        for rule in Rule::ALL {
            let once = draw(rule, &library(), &history, NOW);
            let twice = draw(rule, &library(), &history, NOW);
            assert_eq!(once, twice, "{rule:?}");
        }
    }

    /// The scale claim in this module's docs, asserted for correctness rather
    /// than for time: a timing assertion on a shared CI runner is a flake, and
    /// the measured figure belongs in the study where it can be re-measured.
    #[test]
    fn a_hundred_thousand_tracks_draw_correctly() {
        let records: Vec<(u64, Vec<PathBuf>)> = (0..10_000_u64)
            .map(|id| {
                let tracks = (0..10)
                    .map(|track| PathBuf::from(format!("/music/{id}/{track}.flac")))
                    .collect();
                (id, tracks)
            })
            .collect();
        let history = History::default();
        let drawn = draw(Rule::NeverPlayed, &records, &history, NOW);
        assert_eq!(drawn.len(), 10_000);
        assert_eq!(drawn.first(), Some(&0), "library order is kept");
        assert_eq!(drawn.last(), Some(&9_999));
    }
}
