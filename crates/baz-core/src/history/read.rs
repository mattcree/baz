//! Reading the ledger back: the surfaces `docs/design/critique` names, and
//! nothing else.
//!
//! [`History`] is a snapshot of the file, folded per track. It answers exactly
//! two questions:
//!
//! 1. **[`History::track`]** — how many times, and when: the inspector card's
//!    stamp.
//! 2. **[`History::recency`]** — which [`Recency`] bucket a track falls in:
//!    the PLAYED group key, `THIS EVENING` through to `NEVER`.
//!
//! Since ADR-0034 it also answers a question about **runs** rather than
//! tracks — [`History::runs`], and the per-track reading that goes with it,
//! [`History::last_played_unlisted`]. That is a second *fold*, not a third
//! question about a track: [`TrackHistory`] gains nothing, because *how many
//! times did I play this track* must not become a question about a list.
//! ADR-0018 §6's refusal is inherited whole — no totals by list, no most-played
//! list, no charts. A run marker makes an engagement statistic **easier** to
//! build, which is exactly why the refusal is restated rather than assumed.
//!
//! There is deliberately no third. **There was one** — `pull_weight`, the
//! weighting behind the strip's `Pull` — and it went with the control on
//! 2026-08-10 (the owner: *"please can we remove pull since it doesn't make
//! sense here"*). ADR-0018 §6 is amended to record that. It came out with its
//! one consumer rather than being left standing as a convenience, because the
//! design's constraint is that history *records* and never *performs* — no
//! charts, no streaks, no year in review — and the way to not build those is to
//! not leave the surface that makes them easy lying around.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::history::format::{self, DAY};
use crate::history::{HistoryError, PlayRecord};
use crate::protocol::PlayOutcome;

/// How recent "this evening" is, in seconds: the last six hours.
///
/// The design's first bucket is `THIS EVENING`, and a ledger that carries UTC
/// seconds and no timezone database cannot know when the listener's evening
/// began. Six hours is the honest approximation — long enough to hold an
/// evening's listening, short enough that yesterday's is not in it — and the
/// bucket is defined as "within the last six hours" rather than as a wall-clock
/// range, which is a thing this type can actually promise.
pub const EVENING_SECS: u64 = 6 * 60 * 60;

/// The number of days in the bucket named "this week".
pub const WEEK_DAYS: u64 = 7;

/// The number of days [`Recency`] counts as a month.
///
/// Thirty, not a calendar month: the buckets are elapsed-time bands, and a band
/// whose width depended on which month it was would make the group key jump
/// about for no reason a listener could see.
pub const MONTH_DAYS: u64 = 30;

/// The number of days [`Recency`] counts as a year.
pub const YEAR_DAYS: u64 = 365;

/// How long ago a track was last played — the PLAYED group key.
///
/// Ordered from most to least recent, so a front end can sort by it directly
/// and get the group order the design asks for. [`Self::Never`] is last, as it
/// should be: a record you have never played is the least recently played thing
/// you own.
///
/// The bands are elapsed time, not calendar arithmetic; the constants above say
/// how wide each is.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Recency {
    /// Within the last [`EVENING_SECS`] — the design's `THIS EVENING`.
    ThisEvening,
    /// Within the last day, but not the last six hours.
    Today,
    /// Within the last [`WEEK_DAYS`] days.
    ThisWeek,
    /// Within the last [`MONTH_DAYS`] days.
    ThisMonth,
    /// Whole [`MONTH_DAYS`]-day months ago, `1..=12`.
    MonthsAgo(u32),
    /// Whole [`YEAR_DAYS`]-day years ago, `1` and up.
    YearsAgo(u32),
    /// Never played. Not "no data": the ledger is the record of what was
    /// played, so a track absent from it was not played.
    Never,
    /// **No moment was recorded at all** — the only bucket that is not about
    /// listening, and the one the ADDED group key needs
    /// (`docs/adr/0019-group-keys.md`).
    ///
    /// It is genuinely distinct from [`Self::Never`], which is a *positive*
    /// statement the ledger can make: "this was not played". `Unrecorded` says
    /// baz has no timestamp at all — an index row that predates the first-seen
    /// column (schema v7), for which no honest date exists and none was
    /// invented. Last in the order because a shelf you know nothing about
    /// belongs behind every shelf you know something about.
    ///
    /// [`History::recency`] never returns it: a ledger absence is a `Never`.
    Unrecorded,
}

impl Recency {
    /// The group header this bucket draws, and the value the index rail
    /// projects for the ADDED and PLAYED keys.
    ///
    /// Typography — the design draws headers at 9–10 px in caps — is the
    /// view's business; this is the text.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::ThisEvening => "This evening".to_owned(),
            Self::Today => "Today".to_owned(),
            Self::ThisWeek => "This week".to_owned(),
            Self::ThisMonth => "This month".to_owned(),
            Self::MonthsAgo(1) => "1 month ago".to_owned(),
            Self::MonthsAgo(months) => format!("{months} months ago"),
            Self::YearsAgo(1) => "1 year ago".to_owned(),
            Self::YearsAgo(years) => format!("{years} years ago"),
            Self::Never => "Never played".to_owned(),
            Self::Unrecorded => "Not recorded".to_owned(),
        }
    }
}

/// What the ledger says about one track — the inspector card's stamp.
///
/// Counts and the two dates that bracket them, rather than every play's
/// timestamp: a card says "played 14 times, last on Tuesday", and keeping a
/// timestamp per play per track would hold a 100 000-track library's whole
/// listening life in memory to render a line of text. The file still has every
/// play, and `grep` still finds them.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TrackHistory {
    /// How many times this track met the play threshold.
    pub plays: u32,
    /// How many times it was started and left before the threshold.
    ///
    /// Recorded because it is real ([`crate::history`] argues it), and offered
    /// here so a front end that wants "you keep skipping this" can have it
    /// without reading the file itself.
    pub skips: u32,
    /// When it was first played, in seconds since the Unix epoch (UTC).
    /// `None` if it has only ever been skipped.
    pub first_played_unix_s: Option<u64>,
    /// When it was last played, in seconds since the Unix epoch (UTC).
    pub last_played_unix_s: Option<u64>,
    /// When it was last *touched* — played or skipped.
    pub last_touched_unix_s: Option<u64>,
    /// Total milliseconds of this track's audio delivered, across every play
    /// and skip.
    pub listened_ms: u64,
}

impl TrackHistory {
    /// Whether this track has ever been played (as opposed to only skipped, or
    /// never met at all).
    #[must_use]
    pub fn ever_played(&self) -> bool {
        self.last_played_unix_s.is_some()
    }

    /// Fold one more record in.
    fn absorb(&mut self, record: &PlayRecord) {
        let at = record.started_unix_s;
        self.listened_ms = self.listened_ms.saturating_add(record.listened_ms);
        self.last_touched_unix_s = Some(self.last_touched_unix_s.map_or(at, |had| had.max(at)));
        if record.outcome == PlayOutcome::Skipped {
            self.skips = self.skips.saturating_add(1);
        } else {
            self.plays = self.plays.saturating_add(1);
            self.first_played_unix_s = Some(self.first_played_unix_s.map_or(at, |had| had.min(at)));
            self.last_played_unix_s = Some(self.last_played_unix_s.map_or(at, |had| had.max(at)));
        }
    }
}

/// **One run the ledger recorded**: the list it was reified from, when it
/// started, and what it played (ADR-0034 §4–§5).
///
/// A run is a `# baz run` marker and the play lines under it. A ledger written
/// before ADR-0034 has no markers at all and therefore no runs, which is the
/// honest reading: those plays happened, and which list each came from was
/// never written down.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlayedRun {
    /// **The list this run was a reification of**, exactly as the marker
    /// spelled it — `kind:key:name`, the encoding ADR-0034 §3 defines.
    ///
    /// Opaque here on purpose. `baz-core` carries the string, writes it and
    /// reads it back; what a `kind` word means is the front end's, and a word
    /// this baz does not know must be *its* problem to shrug at rather than
    /// this reader's to reject. `None` is the marker's dash: the run happened
    /// and its list was not recorded.
    pub origin: Option<String>,
    /// When the run's first play started, in seconds since the Unix epoch.
    pub started_unix_s: u64,
    /// How many of this run's tracks met [`play_threshold_ms`](super::play_threshold_ms).
    pub plays: u32,
    /// When the last of them started. `None` for a run that was skipped
    /// straight through, and for a marker with nothing under it at all.
    pub last_played_unix_s: Option<u64>,
}

/// A snapshot of the ledger, folded per track.
///
/// Built by reading the file once. It is a snapshot rather than a live view on
/// purpose: the file is append-only, so a snapshot can only ever be missing
/// *later* plays — it can never be wrong about an earlier one — and re-reading
/// is the whole update mechanism. A front end reloads when it wants to, and a
/// stale snapshot degrades to "does not yet know about the last few minutes",
/// which is the correct failure for a group key and a weighting.
#[derive(Clone, Debug, Default)]
pub struct History {
    by_path: HashMap<PathBuf, TrackHistory>,
    /// The plays that happened **outside** a run that named a list, for the
    /// tracks where that differs from [`Self::by_path`] — which is exactly the
    /// tracks a named list has ever quoted.
    ///
    /// A key means *this track has been played inside a list*; the value is
    /// when it was last played outside one, or `None` if it never was. Every
    /// other track is absent and [`Self::last_played_unlisted`] falls through
    /// to `by_path`, so a ledger with no markers — every ledger that exists
    /// today — costs this map exactly nothing.
    unlisted: HashMap<PathBuf, Option<u64>>,
    runs: Vec<PlayedRun>,
    records: usize,
    malformed: usize,
    skipped_tail: bool,
}

impl History {
    /// Read the ledger at `path`.
    ///
    /// A ledger that does not exist yet reads as an empty history rather than
    /// as an error: a library nobody has played yet is a real state and every
    /// query has a correct answer for it.
    ///
    /// # Concurrency
    ///
    /// Safe to call while the engine is appending. The reader stops at the last
    /// complete line it sees, so a record half-written at the moment of reading
    /// is simply not in this snapshot — it will be in the next one. Nothing is
    /// locked and nothing is written, so a reader can never block or damage a
    /// writer.
    ///
    /// # Damage
    ///
    /// A line that cannot be parsed is skipped and counted
    /// ([`Self::malformed`]); it never aborts the read. This is what keeps a
    /// truncated tail — the one thing an interrupted append can leave — from
    /// costing the file.
    ///
    /// # Errors
    ///
    /// [`HistoryError::Io`] if the file exists but cannot be opened or read.
    pub fn read(path: &Path) -> Result<Self, HistoryError> {
        match File::open(path) {
            Ok(file) => Ok(Self::from_reader(BufReader::new(file))),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(HistoryError::Io {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    /// [`Self::read`] over anything readable — the seam the tests and any
    /// future importer use.
    ///
    /// I/O errors mid-stream end the read at the last complete line rather than
    /// being reported: a ledger that could not be read to the end is still a
    /// true account of the part that was read, and the alternative is throwing
    /// away years of history because the last block was unreadable.
    pub fn from_reader<R: Read>(reader: R) -> Self {
        let mut history = Self::default();
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        // Which run's marker was last seen, and whether it named a list. A
        // play line with no marker before it is `None` — that is every line in
        // every existing ledger, and it is the honest reading (ADR-0034 §4).
        let mut open: Option<usize> = None;
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line) {
                // Nothing left, or nothing readable: either way this snapshot
                // ends here, and what was read is a true account of a prefix.
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if line.last() != Some(&b'\n') {
                // No terminator: either the file is being appended to right
                // now, or an append was interrupted. Either way this is not a
                // record yet, and the rest of the file is untouched.
                history.skipped_tail = true;
                break;
            }
            let Ok(text) = std::str::from_utf8(&line) else {
                history.malformed += 1;
                continue;
            };
            let trimmed = text.trim_end_matches(['\n', '\r']);
            if trimmed.is_empty() || trimmed.starts_with('#') {
                // Blank lines, the header and a run marker are not damage. The
                // marker is read here rather than in `decode` because it is a
                // comment first: that is the property that lets an older baz
                // read a v1.1 ledger with `malformed()` still 0.
                if let Some((started_unix_s, origin)) = format::decode_run(trimmed) {
                    open = Some(history.runs.len());
                    history.runs.push(PlayedRun {
                        origin,
                        started_unix_s,
                        plays: 0,
                        last_played_unix_s: None,
                    });
                }
                continue;
            }
            let Some(record) = format::decode(text) else {
                history.malformed += 1;
                continue;
            };
            history.records += 1;
            history.attribute(open, &record);
            match history.by_path.entry(record.path.clone()) {
                Entry::Occupied(mut slot) => slot.get_mut().absorb(&record),
                Entry::Vacant(slot) => slot.insert(TrackHistory::default()).absorb(&record),
            }
        }
        history
    }

    /// File one play against the run it happened in — the half of the fold
    /// that is about lists rather than about tracks.
    ///
    /// Called **before** the record reaches [`Self::by_path`], because the
    /// seed a newly-listed track takes is *what was known before this play*.
    fn attribute(&mut self, open: Option<usize>, record: &PlayRecord) {
        if record.outcome == PlayOutcome::Skipped {
            // The lane's question is *when did you last listen to this*, and
            // starting a track and leaving it is not having listened to it —
            // the same rule `recency` already keeps.
            return;
        }
        let at = record.started_unix_s;
        let mut listed = false;
        if let Some(run) = open.and_then(|index| self.runs.get_mut(index)) {
            run.plays = run.plays.saturating_add(1);
            run.last_played_unix_s = Some(run.last_played_unix_s.map_or(at, |had| had.max(at)));
            listed = run.origin.is_some();
        }
        if listed {
            // Mark the track as one a list has quoted, seeded with what the
            // ledger already knew about it — a track played on its own on
            // Monday and inside a list on Tuesday is still a record you
            // touched on Monday.
            let seed = self
                .by_path
                .get(&record.path)
                .and_then(|had| had.last_played_unix_s);
            self.unlisted.entry(record.path.clone()).or_insert(seed);
        } else if let Some(slot) = self.unlisted.get_mut(&record.path) {
            *slot = Some(slot.map_or(at, |had| had.max(at)));
        }
    }

    /// What the ledger says about `path` — all zeroes for a track it has never
    /// seen, which is the honest answer rather than an absence to unwrap.
    #[must_use]
    pub fn track(&self, path: &Path) -> TrackHistory {
        self.by_path.get(path).copied().unwrap_or_default()
    }

    /// Which [`Recency`] bucket `path` belongs in, as of `now` — the PLAYED
    /// group key.
    ///
    /// Buckets on the last **play**, not the last skip: starting a track and
    /// abandoning it is not having heard it, and a group key that said
    /// otherwise would move a record you did not listen to into `THIS EVENING`.
    ///
    /// A timestamp in the future (a clock that was wrong, or has since been
    /// corrected) reads as the most recent bucket rather than underflowing.
    #[must_use]
    pub fn recency(&self, path: &Path, now: SystemTime) -> Recency {
        let Some(last) = self.track(path).last_played_unix_s else {
            return Recency::Never;
        };
        bucket(to_unix_s(now).saturating_sub(last))
    }

    /// **When `path` was last played by a run that named no list** — the
    /// reading the returns lane folds onto records (ADR-0034).
    ///
    /// The owner: *"when I play a song from a playlist it should only bump the
    /// recency of that playlist, not the underlying albums"*. A run reified
    /// from a list touched the **list**; the records it quoted were not what
    /// he pointed at, and a relaunch that re-derived them from the play lines
    /// alone is what put them back at the head of his lane.
    ///
    /// So this is [`TrackHistory::last_played_unix_s`] minus the plays that
    /// belong to a list — and for a ledger with no markers, which is every
    /// ledger written before this shipped, it **is**
    /// [`TrackHistory::last_played_unix_s`], exactly. An old file folds the way
    /// it always did.
    ///
    /// Note what this is *not*: a claim that those plays did not happen.
    /// [`Self::track`] still counts every one of them, because how many times
    /// you played a track is a question about the track. This is the answer to
    /// a different question — *when did I last put this record on* — and the
    /// two are allowed to differ.
    #[must_use]
    pub fn last_played_unlisted(&self, path: &Path) -> Option<u64> {
        match self.unlisted.get(path) {
            Some(when) => *when,
            None => self.track(path).last_played_unix_s,
        }
    }

    /// **Every run the ledger recorded**, in the order the file holds them —
    /// which is the order they happened (ADR-0034 §5).
    ///
    /// Empty for a ledger with no markers. A front end reads this to credit a
    /// list that was played in a *previous* session: the marker is what
    /// survives the quit that a queue's provenance does not.
    #[must_use]
    pub fn runs(&self) -> &[PlayedRun] {
        &self.runs
    }

    /// Every track the ledger mentions, with what it says about each. The order
    /// is unspecified.
    pub fn tracks(&self) -> impl Iterator<Item = (&Path, &TrackHistory)> {
        self.by_path
            .iter()
            .map(|(path, track)| (path.as_path(), track))
    }

    /// How many records this snapshot read.
    #[must_use]
    pub fn records(&self) -> usize {
        self.records
    }

    /// How many lines were damaged and skipped.
    ///
    /// Not an error and not a warning to raise at a listener: a ledger that has
    /// been hand-edited, concatenated from backups, or interrupted mid-append
    /// is an ordinary file to meet. It is exposed so that a diagnostic can say
    /// so if asked.
    #[must_use]
    pub fn malformed(&self) -> usize {
        self.malformed
    }

    /// Whether the read stopped at an unterminated final line — the signature
    /// of a file being appended to right now, or of an append that was
    /// interrupted.
    #[must_use]
    pub fn skipped_unterminated_tail(&self) -> bool {
        self.skipped_tail
    }
}

/// The bucket an elapsed time in seconds falls in.
///
/// Free-standing and total so the boundaries can be tested without a file.
#[must_use]
pub fn bucket(elapsed_secs: u64) -> Recency {
    let days = elapsed_secs / DAY;
    if elapsed_secs < EVENING_SECS {
        Recency::ThisEvening
    } else if days < 1 {
        Recency::Today
    } else if days < WEEK_DAYS {
        Recency::ThisWeek
    } else if days < MONTH_DAYS {
        Recency::ThisMonth
    } else if days < YEAR_DAYS {
        // `days / MONTH_DAYS` is 1..=12 in this range, so the cast is exact.
        #[allow(clippy::cast_possible_truncation)]
        Recency::MonthsAgo((days / MONTH_DAYS) as u32)
    } else {
        Recency::YearsAgo(u32::try_from(days / YEAR_DAYS).unwrap_or(u32::MAX))
    }
}

/// `SystemTime` as the ledger counts time. A clock before the epoch reads as
/// the epoch, which is the same rule the writer follows.
fn to_unix_s(at: SystemTime) -> u64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;
    use std::time::Duration;

    const NOW: u64 = 1_786_000_000;

    fn at(unix_s: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(unix_s)
    }

    /// A ledger with known contents, built by writing lines rather than by
    /// calling the encoder on records the test also asserts about.
    fn ledger() -> String {
        let mut text = String::from(format::HEADER);
        let _ = writeln!(
            text,
            "{}\tplayed\t231480\t245013\t/music/a.flac",
            format::format_timestamp(NOW - 3 * DAY)
        );
        let _ = writeln!(
            text,
            "{}\tplayed\t240000\t245013\t/music/a.flac",
            format::format_timestamp(NOW - 100 * DAY)
        );
        let _ = writeln!(
            text,
            "{}\tskipped\t4000\t245013\t/music/a.flac",
            format::format_timestamp(NOW - 3600)
        );
        let _ = writeln!(
            text,
            "{}\tskipped\t2000\t180000\t/music/b.flac",
            format::format_timestamp(NOW - 60)
        );
        let _ = writeln!(
            text,
            "{}\tplayed\t95000\t-\t/music/c stream.mp3",
            format::format_timestamp(NOW - 400 * DAY)
        );
        text
    }

    fn read() -> History {
        History::from_reader(ledger().as_bytes())
    }

    #[test]
    fn the_card_gets_counts_and_dates() {
        let history = read();
        let a = history.track(Path::new("/music/a.flac"));
        assert_eq!(a.plays, 2);
        assert_eq!(a.skips, 1);
        assert_eq!(a.first_played_unix_s, Some(NOW - 100 * DAY));
        assert_eq!(a.last_played_unix_s, Some(NOW - 3 * DAY));
        assert_eq!(a.last_touched_unix_s, Some(NOW - 3600));
        assert_eq!(a.listened_ms, 231_480 + 240_000 + 4_000);
        assert!(a.ever_played());
    }

    /// A track that has only ever been skipped has been *met*, not played —
    /// and the two must not be confused by anything that reads this.
    #[test]
    fn a_track_only_ever_skipped_has_no_play_date() {
        let history = read();
        let b = history.track(Path::new("/music/b.flac"));
        assert_eq!(b.plays, 0);
        assert_eq!(b.skips, 1);
        assert_eq!(b.first_played_unix_s, None);
        assert_eq!(b.last_played_unix_s, None);
        assert_eq!(b.last_touched_unix_s, Some(NOW - 60));
        assert!(!b.ever_played());
        assert_eq!(
            history.recency(Path::new("/music/b.flac"), at(NOW)),
            Recency::Never
        );
    }

    #[test]
    fn a_track_the_ledger_never_saw_reads_as_nothing() {
        let history = read();
        let unknown = Path::new("/music/never.flac");
        assert_eq!(history.track(unknown), TrackHistory::default());
        assert_eq!(history.recency(unknown, at(NOW)), Recency::Never);
    }

    #[test]
    fn the_group_key_buckets_the_synthesized_ledger() {
        let history = read();
        assert_eq!(
            history.recency(Path::new("/music/a.flac"), at(NOW)),
            Recency::ThisWeek
        );
        assert_eq!(
            history.recency(Path::new("/music/c stream.mp3"), at(NOW)),
            Recency::YearsAgo(1)
        );
    }

    /// Every boundary, from both sides.
    #[test]
    fn the_buckets_land_exactly_on_their_boundaries() {
        assert_eq!(bucket(0), Recency::ThisEvening);
        assert_eq!(bucket(EVENING_SECS - 1), Recency::ThisEvening);
        assert_eq!(bucket(EVENING_SECS), Recency::Today);
        assert_eq!(bucket(DAY - 1), Recency::Today);
        assert_eq!(bucket(DAY), Recency::ThisWeek);
        assert_eq!(bucket(WEEK_DAYS * DAY - 1), Recency::ThisWeek);
        assert_eq!(bucket(WEEK_DAYS * DAY), Recency::ThisMonth);
        assert_eq!(bucket(MONTH_DAYS * DAY - 1), Recency::ThisMonth);
        assert_eq!(bucket(MONTH_DAYS * DAY), Recency::MonthsAgo(1));
        assert_eq!(bucket(59 * DAY), Recency::MonthsAgo(1));
        assert_eq!(bucket(60 * DAY), Recency::MonthsAgo(2));
        assert_eq!(bucket(YEAR_DAYS * DAY - 1), Recency::MonthsAgo(12));
        assert_eq!(bucket(YEAR_DAYS * DAY), Recency::YearsAgo(1));
        assert_eq!(bucket(2 * YEAR_DAYS * DAY), Recency::YearsAgo(2));
        assert_eq!(bucket(u64::MAX), Recency::YearsAgo(u32::MAX));
    }

    /// The order the group key is rendered in falls out of the type.
    #[test]
    fn the_buckets_sort_most_recent_first() {
        let mut buckets = vec![
            Recency::Never,
            Recency::YearsAgo(2),
            Recency::MonthsAgo(3),
            Recency::ThisEvening,
            Recency::YearsAgo(1),
            Recency::ThisMonth,
            Recency::MonthsAgo(12),
            Recency::Today,
            Recency::ThisWeek,
        ];
        buckets.sort_unstable();
        assert_eq!(
            buckets,
            vec![
                Recency::ThisEvening,
                Recency::Today,
                Recency::ThisWeek,
                Recency::ThisMonth,
                Recency::MonthsAgo(3),
                Recency::MonthsAgo(12),
                Recency::YearsAgo(1),
                Recency::YearsAgo(2),
                Recency::Never,
            ]
        );
    }

    /// A clock that ran backwards must not underflow into "a billion years
    /// ago".
    #[test]
    fn a_future_timestamp_reads_as_the_most_recent_bucket() {
        let history = read();
        let a = Path::new("/music/a.flac");
        assert_eq!(history.recency(a, at(NOW - 10 * DAY)), Recency::ThisEvening);
    }

    /// The corrupt-tail contract, at the read end: a final line with no
    /// terminator is skipped and every line before it survives.
    #[test]
    fn a_truncated_final_line_costs_only_itself() {
        let mut text = ledger();
        text.push_str("2026-08-06T07:06:40Z\tplayed\t9999\t245013\t/music/tr");
        let history = History::from_reader(text.as_bytes());
        assert_eq!(history.records(), 5);
        assert_eq!(history.malformed(), 0);
        assert!(history.skipped_unterminated_tail());
        assert_eq!(history.track(Path::new("/music/a.flac")).plays, 2);
    }

    /// A line damaged in the middle of the file is skipped; the rest is read.
    #[test]
    fn a_damaged_line_in_the_middle_costs_only_itself() {
        let mut text = ledger();
        text.push_str("this line is not a record at all\n");
        let _ = writeln!(
            text,
            "{}\tplayed\t120000\t200000\t/music/d.flac",
            format::format_timestamp(NOW - 10)
        );
        let history = History::from_reader(text.as_bytes());
        assert_eq!(history.records(), 6);
        assert_eq!(history.malformed(), 1);
        assert_eq!(history.track(Path::new("/music/d.flac")).plays, 1);
    }

    /// Raw bytes that are not UTF-8 at all are damage, not a panic.
    #[test]
    fn a_line_of_raw_garbage_bytes_is_skipped() {
        let mut bytes = ledger().into_bytes();
        bytes.extend_from_slice(&[0xff, 0xfe, 0xfd, b'\n']);
        bytes.extend_from_slice(b"2026-08-06T07:06:40Z\tplayed\t1\t2\t/music/e.flac\n");
        let history = History::from_reader(&bytes[..]);
        assert_eq!(history.malformed(), 1);
        assert_eq!(history.track(Path::new("/music/e.flac")).plays, 1);
    }

    #[test]
    fn the_header_and_blank_lines_are_not_damage() {
        let history = History::from_reader(b"\n\n# a comment\n\n".as_slice());
        assert_eq!(history.records(), 0);
        assert_eq!(history.malformed(), 0);
    }

    #[test]
    fn a_ledger_that_does_not_exist_yet_reads_as_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let history = History::read(&dir.path().join("nothing-here.tsv")).expect("read");
        assert_eq!(history.records(), 0);
        assert_eq!(history.recency(Path::new("/a"), at(NOW)), Recency::Never);
    }

    // ----- runs (ADR-0034) -------------------------------------------------

    /// A ledger with two runs: an album's, and a list's quoting two records
    /// the listener last touched long before it.
    fn marked_ledger() -> String {
        let mut text = String::from(format::HEADER);
        let _ = write!(text, "{}", format::encode_run(NOW - 40 * DAY, None));
        let _ = writeln!(
            text,
            "{}\tplayed\t231480\t245013\t/music/ochre/1.flac",
            format::format_timestamp(NOW - 40 * DAY)
        );
        let _ = writeln!(
            text,
            "{}\tplayed\t240000\t245013\t/music/violet/1.flac",
            format::format_timestamp(NOW - 39 * DAY)
        );
        let _ = write!(
            text,
            "{}",
            format::encode_run(NOW - 3 * DAY, Some("playlist:1f:Road Trip"))
        );
        let _ = writeln!(
            text,
            "{}\tplayed\t231480\t245013\t/music/ochre/1.flac",
            format::format_timestamp(NOW - 3 * DAY)
        );
        let _ = writeln!(
            text,
            "{}\tplayed\t231480\t245013\t/music/violet/1.flac",
            format::format_timestamp(NOW - 3 * DAY + 300)
        );
        text
    }

    /// **The list rises, and the records it quoted do not** — the owner's
    /// defect, read back out of the file a week later.
    #[test]
    fn a_run_from_a_list_credits_the_list_and_not_the_records_it_quoted() {
        let history = History::from_reader(marked_ledger().as_bytes());
        let ochre = Path::new("/music/ochre/1.flac");
        let violet = Path::new("/music/violet/1.flac");

        // Both tracks were played three days ago, and the ledger says so.
        assert_eq!(history.track(ochre).plays, 2);
        assert_eq!(history.track(ochre).last_played_unix_s, Some(NOW - 3 * DAY));
        assert_eq!(history.recency(ochre, at(NOW)), Recency::ThisWeek);

        // …but the lane's reading stops at the last time they were played on
        // their own, which is where the records were before the list ran.
        assert_eq!(history.last_played_unlisted(ochre), Some(NOW - 40 * DAY));
        assert_eq!(history.last_played_unlisted(violet), Some(NOW - 39 * DAY));

        // And the list itself is what the run credits.
        let runs = history.runs();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].origin, None);
        assert_eq!(runs[0].plays, 2);
        assert_eq!(runs[1].origin.as_deref(), Some("playlist:1f:Road Trip"));
        assert_eq!(runs[1].started_unix_s, NOW - 3 * DAY);
        assert_eq!(runs[1].plays, 2);
        assert_eq!(runs[1].last_played_unix_s, Some(NOW - 3 * DAY + 300));
    }

    /// **An old ledger folds exactly as it always did.** Every file that
    /// exists today has no markers, and the whole design rests on that
    /// reading being untouched.
    #[test]
    fn a_ledger_with_no_markers_reads_as_it_always_did() {
        let history = read();
        assert!(history.runs().is_empty());
        assert_eq!(history.malformed(), 0);
        for path in ["/music/a.flac", "/music/b.flac", "/music/c stream.mp3"] {
            let path = Path::new(path);
            assert_eq!(
                history.last_played_unlisted(path),
                history.track(path).last_played_unix_s,
                "{path:?} folded differently with no marker in the file"
            );
        }
        // A track the file never mentions, on both readings.
        assert_eq!(history.last_played_unlisted(Path::new("/never")), None);
    }

    /// **Markers are not damage**, which is the property that lets an older
    /// baz read this file without losing a play.
    #[test]
    fn run_markers_cost_the_file_nothing() {
        let history = History::from_reader(marked_ledger().as_bytes());
        assert_eq!(history.records(), 4);
        assert_eq!(history.malformed(), 0);
        assert!(!history.skipped_unterminated_tail());
    }

    /// A track played inside a list and **never** outside one has no unlisted
    /// moment at all — it is not a record you put on, and the lane must not
    /// invent a date for it.
    #[test]
    fn a_track_only_ever_played_inside_a_list_has_no_unlisted_moment() {
        let mut text = String::from(format::HEADER);
        let _ = write!(
            text,
            "{}",
            format::encode_run(NOW - 60, Some("playlist:1f:Road Trip"))
        );
        let _ = writeln!(
            text,
            "{}\tplayed\t231480\t245013\t/music/only-in-a-list.flac",
            format::format_timestamp(NOW - 60)
        );
        let history = History::from_reader(text.as_bytes());
        let path = Path::new("/music/only-in-a-list.flac");
        assert_eq!(history.track(path).plays, 1);
        assert_eq!(history.last_played_unlisted(path), None);
        assert_eq!(history.recency(path, at(NOW)), Recency::ThisEvening);
    }

    /// A play *after* a list's run, outside one, moves the record again — the
    /// list does not poison the track for ever.
    #[test]
    fn playing_a_track_on_its_own_after_a_list_moves_the_record_again() {
        let mut text = marked_ledger();
        let _ = writeln!(
            text,
            "{}\tplayed\t231480\t245013\t/music/ochre/1.flac",
            format::format_timestamp(NOW - 60)
        );
        let history = History::from_reader(text.as_bytes());
        // The trailing line sits under the list's marker in the file, but it
        // is a play like any other for the *track*; what decides the record is
        // the marker, so a genuinely unlisted play needs its own run.
        assert_eq!(
            history.last_played_unlisted(Path::new("/music/ochre/1.flac")),
            Some(NOW - 40 * DAY)
        );

        let mut text = marked_ledger();
        let _ = write!(text, "{}", format::encode_run(NOW - 60, None));
        let _ = writeln!(
            text,
            "{}\tplayed\t231480\t245013\t/music/ochre/1.flac",
            format::format_timestamp(NOW - 60)
        );
        let history = History::from_reader(text.as_bytes());
        assert_eq!(
            history.last_played_unlisted(Path::new("/music/ochre/1.flac")),
            Some(NOW - 60)
        );
    }

    /// A skip inside a list is not a play of the list, on the same rule that
    /// keeps a skip out of `recency`.
    #[test]
    fn a_skip_does_not_credit_the_run_it_happened_in() {
        let mut text = String::from(format::HEADER);
        let _ = write!(
            text,
            "{}",
            format::encode_run(NOW - 60, Some("playlist:1f:Road Trip"))
        );
        let _ = writeln!(
            text,
            "{}\tskipped\t2000\t245013\t/music/a.flac",
            format::format_timestamp(NOW - 60)
        );
        let history = History::from_reader(text.as_bytes());
        assert_eq!(history.runs().len(), 1);
        assert_eq!(history.runs()[0].plays, 0);
        assert_eq!(history.runs()[0].last_played_unix_s, None);
        // And the track's own record of the skip is untouched.
        assert_eq!(history.track(Path::new("/music/a.flac")).skips, 1);
    }

    /// **A marker with no plays after it means nothing** — a run started and
    /// skipped through leaves a dangling marker, and a reader neither counts
    /// it as damage nor lets it swallow the next run's plays (ADR-0034 §4).
    #[test]
    fn a_dangling_marker_is_read_and_costs_nothing() {
        let mut text = String::from(format::HEADER);
        let _ = write!(
            text,
            "{}",
            format::encode_run(NOW - 500, Some("draw::Shuffle"))
        );
        let _ = write!(
            text,
            "{}",
            format::encode_run(NOW - 60, Some("playlist:1f:Road Trip"))
        );
        let _ = writeln!(
            text,
            "{}\tplayed\t231480\t245013\t/music/a.flac",
            format::format_timestamp(NOW - 60)
        );
        let history = History::from_reader(text.as_bytes());
        assert_eq!(history.malformed(), 0);
        assert_eq!(history.runs().len(), 2);
        assert_eq!(history.runs()[0].plays, 0);
        assert_eq!(history.runs()[1].plays, 1);
    }

    /// An origin this baz does not understand is carried, not rejected — so a
    /// ledger written by a later baz stays readable by this one.
    #[test]
    fn an_origin_from_a_later_baz_is_carried_verbatim() {
        let mut text = String::from(format::HEADER);
        let _ = write!(
            text,
            "{}",
            format::encode_run(NOW - 60, Some("moodboard:ff:Rainy Tuesday"))
        );
        let _ = writeln!(
            text,
            "{}\tplayed\t231480\t245013\t/music/a.flac",
            format::format_timestamp(NOW - 60)
        );
        let history = History::from_reader(text.as_bytes());
        assert_eq!(history.malformed(), 0);
        assert_eq!(
            history.runs()[0].origin.as_deref(),
            Some("moodboard:ff:Rainy Tuesday")
        );
    }

    #[test]
    fn every_track_mentioned_is_reachable() {
        let history = read();
        let mut paths: Vec<_> = history.tracks().map(|(path, _)| path.to_owned()).collect();
        paths.sort();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/music/a.flac"),
                PathBuf::from("/music/b.flac"),
                PathBuf::from("/music/c stream.mp3"),
            ]
        );
    }
}
