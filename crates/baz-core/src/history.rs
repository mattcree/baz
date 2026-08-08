//! The play-history ledger: an append-only, plain-text record of what was
//! played, kept beside the library in the user's own data directory (ADR-0018).
//!
//! History is the one thing in baz that **cannot be backfilled**. A missing
//! thumbnail cache rebuilds; a missing ReplayGain figure re-measures; a day of
//! listening that nobody wrote down is gone. That is why this exists before the
//! surfaces that read it, and why the writing end is deliberately duller than
//! the reading end.
//!
//! # What it is
//!
//! One file. One line per play. Tab-separated. UTF-8. Appended to, and — this
//! is a guarantee rather than an implementation detail — **never rewritten**:
//! no compaction, no de-duplication, no rotation, no in-place fixes. The only
//! operation this module performs on the file is `write(2)` at the end of it.
//! A user can therefore treat it the way they treat a log: `tail -f` it, copy
//! it mid-write, back it up with `rsync --append`, split it by year with
//! `grep`, or delete it. It is theirs.
//!
//! ```text
//! # baz play history. One line per play, appended, never rewritten.
//! # Fields, tab-separated: started_utc, outcome, listened_ms, track_ms, path
//! 2026-08-08T19:04:11Z	played	231480	245013	/home/matt/Music/Talk Talk/01 Myrrhman.flac
//! 2026-08-08T19:08:20Z	skipped	9200	402000	/home/matt/Music/Talk Talk/02 Ascension Day.flac
//! ```
//!
//! ## The fields
//!
//! | # | field | meaning |
//! |---|---|---|
//! | 1 | `started_utc` | ISO-8601 UTC, seconds, `Z` — when the track's **first audio** was heard |
//! | 2 | `outcome` | `played` or `skipped` (the rule is below) |
//! | 3 | `listened_ms` | milliseconds of this track's audio actually delivered to the output |
//! | 4 | `track_ms` | the track's own length, or `-` when the file declares none |
//! | 5 | `path` | the file, escaped (below) |
//!
//! Integer milliseconds, for [`crate::protocol`]'s reasons: one canonical
//! encoding, and no float ever printed to a file that is compared byte for
//! byte. UTC with no offset, because a ledger whose lines sorted differently
//! from the order they happened would be worse than useless — `sort` on this
//! file *is* chronological order, and `grep '^2026-08'` is "August".
//!
//! The timestamp is when the play **started**, not when it ended: it is the
//! answer to "when did you listen to this", it is what the `PLAYED` group key
//! and the inspector card want, and it is the same convention every scrobbler
//! uses.
//!
//! ## Escaping
//!
//! Everything except the path travels as ASCII digits and words. The path is
//! escaped just enough to keep the format line-oriented, tab-delimited and
//! reversible:
//!
//! | in the path | in the file |
//! |---|---|
//! | `\` | `\\` |
//! | tab | `\t` |
//! | newline | `\n` |
//! | carriage return | `\r` |
//! | any other C0 control, or `DEL` | `\xHH` |
//! | a byte that is not part of valid UTF-8 | `\xHH` |
//!
//! Nothing else is touched. `/mnt/nas/音楽/坂本龍一/01.flac` is written exactly
//! as it is, which is the property that makes `grep 'Talk Talk' history.tsv`
//! do what a human expects. The escaped form is **always valid UTF-8**, even
//! for a path that is not, so the file never breaks `less`, an editor, or a
//! locale-aware `grep`.
//!
//! (Non-UTF-8 paths are a Unix reality and are recorded exactly. On platforms
//! whose paths are UTF-16, the only unrepresentable sequences are unpaired
//! surrogates, which are recorded with the replacement character — the same
//! trade [`crate::protocol`] already makes for paths on the wire.)
//!
//! ## Damage, and the truncated tail
//!
//! A line is written with one `write_all` of the whole line, newline included,
//! to a handle opened `O_APPEND`, and followed by an `fsync`. A process that
//! dies mid-append can still leave a partial final line, and a filesystem that
//! loses its tail can leave a shorter one.
//!
//! Both ends handle it without losing the file:
//!
//! - **Reading**: [`History::read`] stops at the last line that ends in a
//!   newline. A partial final line is not a record — it is skipped, and every
//!   complete line before it is read. A line that *is* terminated but cannot be
//!   parsed (a hand edit, a concatenated backup) is skipped and counted
//!   ([`History::malformed`]); it never aborts the read. The same rule is what
//!   makes reading safe while the engine is appending: a reader sees a prefix
//!   of the file, and a prefix of an append-only file is always a true, if
//!   slightly old, account.
//! - **Writing**: [`HistoryLedger::open`] checks whether the file ends in a
//!   newline and, if it does not, **appends a terminator** —
//!   `\t\t# incomplete line, closed off by baz` and a newline — before
//!   appending anything else. That closes the broken line off as its own
//!   unparseable record, permanently skipped by every reader, instead of gluing
//!   the next play onto its end. A bare newline would not have been enough:
//!   `…\tplayed\t99\t100\t/music/Talk Ta` *is* a well-formed line, naming a file
//!   that never existed, so the terminator is chosen to make the line
//!   unparseable wherever the cut fell. It is still an append: bytes are added,
//!   none are changed.
//!
//! # What counts as a play
//!
//! A track is recorded as `played` when the audio actually delivered for it
//! reaches **half the track's length, or [`PLAY_THRESHOLD_CAP_MS`] (four
//! minutes), whichever comes first** — [`play_threshold_ms`], the convention
//! `Last.fm` established and `ListenBrainz` kept. Anything less, and it is recorded
//! as `skipped`. Nothing at all is recorded for a track that delivered no
//! audio: a queue entry the listener jumped straight past was never met.
//!
//! Two deliberate departures from the scrobbling convention, both in the
//! direction of recording more truth:
//!
//! - **No minimum track length.** `Last.fm` refuses to scrobble anything under
//!   thirty seconds. That rule exists to stop people gaming a public
//!   leaderboard — an anti-abuse measure for a scoreboard baz does not have. A
//!   twelve-second track played to its end is a play, and a file the listener
//!   keeps on their own disk is not evidence in anyone's competition.
//! - **Skips are recorded too**, as their own outcome. Three arguments, in
//!   order of weight. It is *more honest*: a threshold that decides which
//!   listening is worth writing down silently discards the other half of the
//!   evidence. It is *more useful*: "you have started this four times and never
//!   finished it" is the strongest signal in the file, and the pull's weighting
//!   and the card both want it. And it is *free to ignore*: `grep played` is
//!   the played-only view, exactly, so a reader who does not want skips pays
//!   nothing for their being there. The cost — one line of about a hundred
//!   bytes per skip — is a few megabytes for a listening lifetime.
//!
//!   The argument against, stated because it is real: a record of what you
//!   abandoned feels more like surveillance than a record of what you finished.
//!   The answer is the same one that governs everything here — the file is
//!   local, plain, documented, and the user's to delete.
//!
//! `listened_ms` counts **audio delivered to the output**, not wall-clock time
//! and not the position in the track. Pausing for an hour adds nothing. Seeking
//! backwards and hearing a passage twice counts it twice, because it was heard
//! twice. Seeking forwards past a passage does not count it, because it was
//! not. A seek within a track continues one play rather than starting another;
//! anything that starts a track from its beginning — the next track, `Next`,
//! `Previous`, a click on a queue row — ends the play in progress and begins a
//! new one.
//!
//! # Where it is written from
//!
//! **The engine, never a front end.** [`crate::engine`] is the only thing that
//! knows what is playing and for how long, and putting the writer there means a
//! front end that crashes loses nothing and a second front end attached to the
//! same engine cannot double-write. A front end's whole involvement is handing
//! the engine a ledger ([`crate::engine::EngineHandle::set_history`]) — the
//! same seam ADR-0015 used for computed ReplayGain figures.
//!
//! **And never on the pump path.** The engine thread decides that a play has
//! finished and hands the record to this module; the actual `write` and `fsync`
//! happen on the ledger's own thread. The engine thread's cost is one channel
//! send per track, taken between pump iterations exactly where event emission
//! already happens (`docs/ENGINEERING.md`: no I/O, no locking, no allocation on
//! the realtime path).
//!
//! That is also what makes [`Event::PlayRecorded`] mean something precise:
//! it is emitted by the writer **after** the line is in the file and synced, so
//! a front end that reacts to it by re-reading the ledger always finds the play
//! it was told about. State before event, as the rest of the protocol has it.
//!
//! # Privacy
//!
//! There is nothing in this file that is not already on the user's disk: paths
//! they chose, times their own clock reported, durations their own files have.
//! No identifier, no machine ID, no session key, no hash of anything. It is
//! written to their data directory beside the library, it is documented in its
//! own header, and **nothing sends it anywhere** — baz has no network code in
//! this path and no account to attach it to (`docs/VISION.md`, the sovereignty
//! pillar).
//!
//! ## The scrobbling seam
//!
//! Scrobbling is explicitly out of scope here, and the design's phrasing is the
//! reason: *Last.fm scrobbling = optional output, never a dependency.* The seam
//! it would attach to is [`Event::PlayRecorded`] — a scrobbler is a consumer of
//! that event (or, for catching up after being offline, of [`History`] over the
//! file), and it is deliberately *downstream* of the ledger rather than beside
//! it. Concretely, that means the ledger is complete whether or not a scrobbler
//! exists, whether or not it is configured, and whether or not the network is
//! up; a scrobbler that fails, or is removed, or never existed, changes nothing
//! about what was recorded. No code in this module knows what a scrobbler is.
//!
//! [`Event::PlayRecorded`]: crate::protocol::Event::PlayRecorded

// The example above is a sample of the file, so its separators are the real
// ones. Spaces would misdescribe the format this module's whole job is to pin.
#![allow(clippy::tabs_in_doc_comments)]

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::protocol::{Event, PlayOutcome};

mod format;
mod read;

pub use read::{
    EVENING_SECS, History, MONTH_DAYS, PULL_DAY_CAP, PULL_NEVER_WEIGHT, Recency, TrackHistory,
    WEEK_DAYS, YEAR_DAYS, bucket,
};

/// The file the ledger lives in, inside baz's data directory — beside
/// `library.db`, which is the database this is deliberately *not* kept in.
///
/// A `.tsv` because that is what it is, and because the extension tells every
/// tool on the machine how to split it. The comment lines the file opens with
/// are the one liberty taken with the format, and every reader here skips them.
pub const LEDGER_FILE: &str = "history.tsv";

/// The point at which listening long enough stops depending on the track's
/// length: four minutes, in milliseconds.
///
/// The scrobbling convention, unchanged, and it is a good rule for a reason
/// worth stating: half of a two-minute pop song and half of a twenty-minute
/// side of Tago Mago are not equally strong evidence that someone listened to
/// it, and past about four minutes the fraction stops mattering. Somebody four
/// minutes into a long piece is listening to it.
pub const PLAY_THRESHOLD_CAP_MS: u64 = 240_000;

/// How much of a track has to be heard before it counts as played.
///
/// Half its length, or [`PLAY_THRESHOLD_CAP_MS`], whichever is less. A track
/// whose container declares no length (`track_ms` is `None`) has only the cap
/// to go on, which is the right answer for a stream: four minutes of it is
/// listening by any measure.
///
/// Public, and a pure function of its inputs, so that a front end explaining
/// the rule quotes the engine's answer rather than a copy of it — the same
/// reason [`PREVIOUS_RESTART_MS`](crate::engine::PREVIOUS_RESTART_MS) is
/// public.
#[must_use]
pub fn play_threshold_ms(track_ms: Option<u64>) -> u64 {
    match track_ms {
        Some(total) => (total / 2).min(PLAY_THRESHOLD_CAP_MS),
        None => PLAY_THRESHOLD_CAP_MS,
    }
}

/// What to record for a track that delivered `listened_ms` of audio, or `None`
/// if there is nothing to record.
///
/// `None` means exactly one thing: no audio was delivered at all. A queue entry
/// the listener stepped straight past, or a file that failed to open, was never
/// met and gets no line — a ledger of things that did not happen is not a
/// ledger. Everything that *was* heard gets a line, whether it reached the
/// threshold or not.
#[must_use]
pub fn classify(listened_ms: u64, track_ms: Option<u64>) -> Option<PlayOutcome> {
    if listened_ms == 0 {
        return None;
    }
    Some(if listened_ms >= play_threshold_ms(track_ms) {
        PlayOutcome::Played
    } else {
        PlayOutcome::Skipped
    })
}

/// One line of the ledger, as a value.
///
/// Constructed by [`crate::engine`] when a play ends and by
/// [`History`] when one is read back; the two use the same type so that
/// "what is written" and "what is read" cannot drift apart.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlayRecord {
    /// When the track's first audio was heard, in seconds since the Unix
    /// epoch (UTC).
    pub started_unix_s: u64,
    /// Whether it met [`play_threshold_ms`].
    pub outcome: PlayOutcome,
    /// Milliseconds of this track's audio delivered to the output.
    pub listened_ms: u64,
    /// The track's own length, when the file declares one.
    pub track_ms: Option<u64>,
    /// The file that was played.
    pub path: PathBuf,
}

impl PlayRecord {
    /// A record of `path`, classified by [`classify`], or `None` when nothing
    /// was heard.
    #[must_use]
    pub fn new(
        path: PathBuf,
        started_unix_s: u64,
        listened_ms: u64,
        track_ms: Option<u64>,
    ) -> Option<Self> {
        Some(Self {
            started_unix_s,
            outcome: classify(listened_ms, track_ms)?,
            listened_ms,
            track_ms,
            path,
        })
    }

    /// When the play started, as a [`SystemTime`].
    #[must_use]
    pub fn started(&self) -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_secs(self.started_unix_s)
    }

    /// This record as it appears in the file, newline included.
    ///
    /// The format is documented in this module's docs; this is the one
    /// implementation of it, exposed so that a test — or a user's own tool —
    /// can assert on the bytes rather than on an intention.
    #[must_use]
    pub fn to_line(&self) -> String {
        format::encode(self)
    }

    /// Read one line back, or `None` if it is not a record (a comment, a blank
    /// line, a truncated tail, or damage of any other kind).
    #[must_use]
    pub fn from_line(line: &str) -> Option<Self> {
        format::decode(line)
    }
}

/// Something went wrong with the ledger file itself.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    /// The file could not be opened, read, or created.
    #[error("history ledger {}: {source}", path.display())]
    Io {
        /// The file in question.
        path: PathBuf,
        /// What the operating system said.
        #[source]
        source: std::io::Error,
    },
    /// The platform has no data directory to put the ledger in, so
    /// [`HistoryLedger::open_default`] has nowhere to write.
    #[error("no user data directory on this platform; open the ledger by path instead")]
    NoDataDirectory,
}

/// The append-only writer.
///
/// Owns one open file and one thread. [`Self::record`] hands a line to that
/// thread and returns; the `write` and the `fsync` happen there, which is what
/// keeps file I/O off the engine's pump path. Dropping the ledger drains the
/// queue and joins the thread, so nothing queued is lost at shutdown.
///
/// Cheap to share: put it in an `Arc` and hand it to
/// [`EngineHandle::set_history`](crate::engine::EngineHandle::set_history).
#[derive(Debug)]
pub struct HistoryLedger {
    path: PathBuf,
    records: Option<Sender<Message>>,
    thread: Option<JoinHandle<()>>,
    written: Arc<AtomicUsize>,
    failures: Arc<AtomicUsize>,
}

/// What the writer thread is asked to do.
enum Message {
    /// Append this record, then announce it on the channel if one was given.
    Append(Box<(PlayRecord, Option<Sender<Event>>)>),
    /// Answer once everything queued before this has been written.
    Barrier(SyncSender<()>),
}

impl HistoryLedger {
    /// Open (or create) the ledger at `path` and start its writer thread.
    ///
    /// Creates the parent directory if it is missing, writes the documenting
    /// header if the file is new or empty, and — if the file does not end in a
    /// newline — appends one, closing off a line some earlier process was
    /// interrupted in the middle of. None of that rewrites a byte that was
    /// already there.
    ///
    /// # Errors
    ///
    /// [`HistoryError::Io`] if the directory cannot be created, the file cannot
    /// be opened for appending, or the header cannot be written.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, HistoryError> {
        let path = path.into();
        let fail = |source: std::io::Error| HistoryError::Io {
            path: path.clone(),
            source,
        };
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)
            .map_err(fail)?;
        prepare(&mut file).map_err(fail)?;
        let (tx, rx) = mpsc::channel();
        let written = Arc::new(AtomicUsize::new(0));
        let failures = Arc::new(AtomicUsize::new(0));
        let counters = (Arc::clone(&written), Arc::clone(&failures));
        let thread = std::thread::Builder::new()
            .name("baz-history".into())
            .spawn(move || writer(file, &rx, &counters.0, &counters.1))
            .map_err(fail)?;
        Ok(Self {
            path,
            records: Some(tx),
            thread: Some(thread),
            written,
            failures,
        })
    }

    /// Open the ledger at [`Self::default_path`] — beside the library, in the
    /// user's own data directory.
    ///
    /// # Errors
    ///
    /// [`HistoryError::NoDataDirectory`] if the platform has no data directory,
    /// or whatever [`Self::open`] reports.
    pub fn open_default() -> Result<Self, HistoryError> {
        Self::open(Self::default_path().ok_or(HistoryError::NoDataDirectory)?)
    }

    /// `$XDG_DATA_HOME/baz/history.tsv` and its equivalents — the same
    /// directory the library database lives in, because the two are the same
    /// kind of thing: baz's own record of the user's collection, in the user's
    /// own space, in an open format.
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        Some(dirs::data_dir()?.join("baz").join(LEDGER_FILE))
    }

    /// The file this ledger appends to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Queue `record` for appending.
    ///
    /// Returns immediately: no file I/O happens on the calling thread, which is
    /// what lets the engine call this from the same thread that runs the pump.
    /// When `announce` is `Some`, [`Event::PlayRecorded`] is sent on it **after**
    /// the line is in the file, so the event is never news about a line that is
    /// not there yet.
    pub fn record(&self, record: PlayRecord, announce: Option<Sender<Event>>) {
        if let Some(records) = &self.records {
            let _ = records.send(Message::Append(Box::new((record, announce))));
        }
    }

    /// Block until everything queued so far has been written and synced.
    ///
    /// For shutdown paths and for tests that want to assert on the file. Not
    /// needed for correctness in ordinary use — dropping the ledger does the
    /// same thing — and never called from the engine thread.
    pub fn flush(&self) {
        let Some(records) = &self.records else {
            return;
        };
        let (tx, rx) = mpsc::sync_channel(0);
        if records.send(Message::Barrier(tx)).is_ok() {
            let _ = rx.recv();
        }
    }

    /// How many records have reached the file.
    #[must_use]
    pub fn written(&self) -> usize {
        self.written.load(Ordering::Acquire)
    }

    /// How many records could not be written — a full disk, a removed volume.
    ///
    /// Counted rather than raised: a failed append must not take the music
    /// down with it, and the honest report is a number a diagnostic can ask
    /// for. A record that fails to write emits no
    /// [`Event::PlayRecorded`], because that event's whole meaning is that the
    /// line is in the file.
    #[must_use]
    pub fn write_failures(&self) -> usize {
        self.failures.load(Ordering::Acquire)
    }
}

impl Drop for HistoryLedger {
    /// Drain and join: closing the channel is the writer thread's shutdown
    /// signal, and everything already queued is written before it exits.
    fn drop(&mut self) {
        self.records = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Make an opened ledger fit to append to: header if empty, newline if the
/// last append was interrupted.
fn prepare(file: &mut File) -> std::io::Result<()> {
    let len = file.metadata()?.len();
    if len == 0 {
        file.write_all(format::HEADER.as_bytes())?;
        return file.sync_data();
    }
    file.seek(SeekFrom::Start(len - 1))?;
    let mut last = [0u8; 1];
    match file.read_exact(&mut last) {
        Ok(()) => {}
        // A file that shrank under us has nothing to close off.
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(()),
        Err(error) => return Err(error),
    }
    if last[0] != b'\n' {
        // An interrupted append left a partial line. Close it off so it stands
        // as its own permanently-skipped record instead of the next play being
        // glued onto its end — and so that it cannot *itself* read as a record
        // naming a truncated path, which a bare newline would have allowed
        // ([`format::TRUNCATED`] carries the argument). Appending, still: no
        // byte that was already in the file is touched.
        file.write_all(format::TRUNCATED.as_bytes())?;
        file.sync_data()?;
    }
    Ok(())
}

/// The writer thread: append, sync, announce, repeat.
fn writer(
    mut file: File,
    records: &mpsc::Receiver<Message>,
    written: &AtomicUsize,
    failures: &AtomicUsize,
) {
    while let Ok(message) = records.recv() {
        match message {
            Message::Append(payload) => {
                let (record, announce) = *payload;
                let line = format::encode(&record);
                // One `write_all` of the whole line, then a sync. The write is
                // what makes the line visible to every other reader on the
                // machine; the sync is what makes it survive the machine.
                let ok = file
                    .write_all(line.as_bytes())
                    .and_then(|()| file.sync_data())
                    .is_ok();
                if !ok {
                    failures.fetch_add(1, Ordering::Release);
                    continue;
                }
                written.fetch_add(1, Ordering::Release);
                // State before event: the line is in the file, and only then
                // is anyone told about it.
                if let Some(events) = announce {
                    let _ = events.send(Event::PlayRecorded {
                        path: record.path,
                        started_unix_s: record.started_unix_s,
                        listened_ms: record.listened_ms,
                        track_ms: record.track_ms,
                        outcome: record.outcome,
                    });
                }
            }
            Message::Barrier(ack) => {
                let _ = ack.send(());
            }
        }
    }
}

/// Now, in seconds since the Unix epoch (UTC).
///
/// A clock set before 1970 reads as the epoch rather than failing: a wrong
/// timestamp is one wrong field, and refusing would be one lost play.
pub(crate) fn now_unix_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;

    fn lines(path: &Path) -> Vec<String> {
        let file = File::open(path).expect("open");
        std::io::BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .collect()
    }

    fn record(path: &str, listened_ms: u64, track_ms: Option<u64>) -> PlayRecord {
        PlayRecord::new(PathBuf::from(path), 1_786_000_000, listened_ms, track_ms)
            .expect("something was heard")
    }

    // ----- the threshold, as a pure function -------------------------------

    #[test]
    fn the_threshold_is_half_the_track_or_four_minutes() {
        // A three-minute song: half of it.
        assert_eq!(play_threshold_ms(Some(180_000)), 90_000);
        // A ten-minute one: the cap bites first.
        assert_eq!(play_threshold_ms(Some(600_000)), PLAY_THRESHOLD_CAP_MS);
        // Exactly at the crossover — eight minutes, half of which is the cap.
        assert_eq!(play_threshold_ms(Some(480_000)), PLAY_THRESHOLD_CAP_MS);
        assert_eq!(play_threshold_ms(Some(480_001)), PLAY_THRESHOLD_CAP_MS);
        assert_eq!(play_threshold_ms(Some(479_998)), 239_999);
        // No declared length: the cap alone.
        assert_eq!(play_threshold_ms(None), PLAY_THRESHOLD_CAP_MS);
        assert_eq!(play_threshold_ms(Some(0)), 0);
    }

    #[test]
    fn the_threshold_is_met_at_it_not_past_it() {
        assert_eq!(classify(89_999, Some(180_000)), Some(PlayOutcome::Skipped));
        assert_eq!(classify(90_000, Some(180_000)), Some(PlayOutcome::Played));
        assert_eq!(classify(90_001, Some(180_000)), Some(PlayOutcome::Played));
    }

    #[test]
    fn nothing_heard_is_nothing_recorded() {
        assert_eq!(classify(0, Some(180_000)), None);
        assert_eq!(classify(0, None), None);
        assert!(PlayRecord::new(PathBuf::from("/a.flac"), 0, 0, Some(1)).is_none());
    }

    /// The one departure from the scrobbling convention that changes an
    /// outcome: baz has no minimum track length.
    #[test]
    fn a_very_short_track_played_through_is_a_play() {
        // Twelve seconds, heard to the end. `Last.fm` would refuse this.
        assert_eq!(classify(12_000, Some(12_000)), Some(PlayOutcome::Played));
        // And one second of it is still a skip, not a silence.
        assert_eq!(classify(1_000, Some(12_000)), Some(PlayOutcome::Skipped));
    }

    #[test]
    fn a_stream_of_unknown_length_needs_four_minutes() {
        assert_eq!(classify(239_999, None), Some(PlayOutcome::Skipped));
        assert_eq!(classify(240_000, None), Some(PlayOutcome::Played));
    }

    // ----- the file --------------------------------------------------------

    #[test]
    fn a_new_ledger_opens_with_its_own_documentation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("history.tsv");
        {
            let _ledger = HistoryLedger::open(&path).expect("open");
        }
        let text = std::fs::read_to_string(&path).expect("read");
        assert_eq!(text, format::HEADER);
        assert!(text.ends_with('\n'));
        assert_eq!(History::read(&path).expect("read").records(), 0);
    }

    #[test]
    fn the_default_path_sits_beside_the_library() {
        // Only assert the shape; the directory itself is the platform's.
        if let Some(path) = HistoryLedger::default_path() {
            assert!(path.ends_with("baz/history.tsv") || path.ends_with("baz\\history.tsv"));
        }
    }

    #[test]
    fn records_reach_the_file_as_the_documented_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("history.tsv");
        let ledger = HistoryLedger::open(&path).expect("open");
        ledger.record(record("/music/a.flac", 231_480, Some(245_013)), None);
        ledger.record(record("/music/b.flac", 9_200, Some(402_000)), None);
        ledger.flush();
        assert_eq!(ledger.written(), 2);
        assert_eq!(ledger.write_failures(), 0);
        assert_eq!(
            lines(&path),
            vec![
                "2026-08-06T07:06:40Z\tplayed\t231480\t245013\t/music/a.flac",
                "2026-08-06T07:06:40Z\tskipped\t9200\t402000\t/music/b.flac",
            ]
        );
    }

    /// Append-only means append-only: an existing file's bytes are a prefix of
    /// the file after any number of further records.
    #[test]
    fn the_file_is_only_ever_appended_to() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("history.tsv");
        let mut snapshots: Vec<Vec<u8>> = Vec::new();
        {
            let ledger = HistoryLedger::open(&path).expect("open");
            for i in 0..8u64 {
                snapshots.push(std::fs::read(&path).expect("read"));
                ledger.record(record("/music/a.flac", 200_000 + i, Some(300_000)), None);
                ledger.flush();
            }
            snapshots.push(std::fs::read(&path).expect("read"));
        }
        // Reopening must not rewrite anything either.
        {
            let _ledger = HistoryLedger::open(&path).expect("open");
        }
        snapshots.push(std::fs::read(&path).expect("read"));
        for pair in snapshots.windows(2) {
            assert!(
                pair[1].starts_with(&pair[0]),
                "the file stopped being a prefix of its later self"
            );
            assert!(pair[1].len() >= pair[0].len());
        }
    }

    /// The truncated-tail contract at the write end.
    #[test]
    fn an_interrupted_final_line_is_closed_off_rather_than_glued_to() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("history.tsv");
        {
            let ledger = HistoryLedger::open(&path).expect("open");
            ledger.record(record("/music/a.flac", 231_480, Some(245_013)), None);
            ledger.flush();
        }
        // Simulate a process killed mid-append.
        let mut damaged = std::fs::read(&path).expect("read");
        damaged.extend_from_slice(b"2026-08-06T07:06:40Z\tplayed\t99\t100\t/music/tr");
        std::fs::write(&path, &damaged).expect("write");

        let ledger = HistoryLedger::open(&path).expect("reopen");
        ledger.record(record("/music/b.flac", 300_000, Some(400_000)), None);
        ledger.flush();

        let after = std::fs::read(&path).expect("read");
        assert!(
            after.starts_with(&damaged),
            "the damaged bytes were rewritten"
        );
        let history = History::read(&path).expect("read");
        // The good record before the damage, and the one after it. The partial
        // line is skipped; it did not swallow the record that followed it, and
        // it did not resurrect as a record naming `/music/tr`.
        assert_eq!(history.records(), 2);
        assert_eq!(history.malformed(), 1);
        assert_eq!(history.track(Path::new("/music/a.flac")).plays, 1);
        assert_eq!(history.track(Path::new("/music/b.flac")).plays, 1);
        assert_eq!(
            history.track(Path::new("/music/tr")),
            TrackHistory::default()
        );
    }

    /// A truncated line is unparseable wherever the cut fell — including the
    /// cuts that would leave a *plausible* record behind a bare newline.
    #[test]
    fn a_line_cut_at_any_field_boundary_stays_unparseable() {
        let whole = record("/music/a.flac", 231_480, Some(245_013)).to_line();
        let whole = whole.trim_end_matches('\n');
        for cut in 0..whole.len() {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("history.tsv");
            let mut damaged = format::HEADER.as_bytes().to_vec();
            damaged.extend_from_slice(&whole.as_bytes()[..cut]);
            std::fs::write(&path, &damaged).expect("write");
            {
                let ledger = HistoryLedger::open(&path).expect("reopen");
                ledger.record(record("/music/b.flac", 300_000, Some(400_000)), None);
                ledger.flush();
            }
            let history = History::read(&path).expect("read");
            assert_eq!(history.records(), 1, "cut at {cut}: {:?}", &whole[..cut]);
            assert_eq!(history.track(Path::new("/music/b.flac")).plays, 1);
        }
    }

    /// Reading while writing: every snapshot a reader takes is a valid,
    /// monotonically growing account, and no read ever fails or blocks a write.
    #[test]
    fn a_reader_can_run_while_the_ledger_is_being_appended_to() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("history.tsv");
        let ledger = HistoryLedger::open(&path).expect("open");
        let reader_path = path.clone();
        let reader = std::thread::spawn(move || {
            let mut seen = 0usize;
            for _ in 0..200 {
                let history = History::read(&reader_path).expect("read");
                assert_eq!(history.malformed(), 0, "a reader saw a damaged line");
                assert!(history.records() >= seen, "the account went backwards");
                seen = history.records();
                std::thread::yield_now();
            }
            seen
        });
        for i in 0..100u64 {
            ledger.record(record("/music/a.flac", 200_000 + i, Some(300_000)), None);
        }
        ledger.flush();
        let _ = reader.join().expect("reader");
        let history = History::read(&path).expect("read");
        assert_eq!(history.records(), 100);
        assert_eq!(history.track(Path::new("/music/a.flac")).plays, 100);
    }

    /// A path holding the separator and a newline survives the whole trip:
    /// value → file → value.
    #[test]
    fn an_awkward_path_survives_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("history.tsv");
        let awkward = PathBuf::from("/music/a\tb\nc\\d.flac");
        let ledger = HistoryLedger::open(&path).expect("open");
        ledger.record(
            PlayRecord::new(awkward.clone(), 1_786_000_000, 200_000, Some(300_000))
                .expect("record"),
            None,
        );
        ledger.flush();
        assert_eq!(lines(&path).len(), 1, "the path broke the line format");
        let history = History::read(&path).expect("read");
        assert_eq!(history.track(&awkward).plays, 1);
    }

    #[test]
    fn a_line_round_trips_through_its_own_text() {
        let original = record("/music/a.flac", 231_480, Some(245_013));
        let line = original.to_line();
        assert_eq!(PlayRecord::from_line(&line), Some(original));
    }

    #[test]
    fn dropping_the_ledger_writes_what_was_queued() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("history.tsv");
        {
            let ledger = HistoryLedger::open(&path).expect("open");
            for i in 0..32u64 {
                ledger.record(record("/music/a.flac", 200_000 + i, Some(300_000)), None);
            }
            // No flush: the drop is the guarantee under test.
        }
        assert_eq!(History::read(&path).expect("read").records(), 32);
    }

    /// The event is news about a line that is already there.
    #[test]
    fn the_event_arrives_after_the_line_is_in_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("history.tsv");
        let ledger = HistoryLedger::open(&path).expect("open");
        let (tx, rx) = mpsc::channel();
        ledger.record(record("/music/a.flac", 231_480, Some(245_013)), Some(tx));
        let event = rx.recv().expect("the play is announced");
        assert_eq!(
            event,
            Event::PlayRecorded {
                path: PathBuf::from("/music/a.flac"),
                started_unix_s: 1_786_000_000,
                listened_ms: 231_480,
                track_ms: Some(245_013),
                outcome: PlayOutcome::Played,
            }
        );
        // Read *after* the event, with no flush: the contract is that the line
        // is already there.
        assert_eq!(History::read(&path).expect("read").records(), 1);
    }
}
