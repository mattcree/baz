//! The scan pipeline: a worker thread that streams the library scanner's
//! output to the UI in batches.
//!
//! [`spawn`] starts a `baz-scan` thread that drains
//! [`baz_core::library::scan`] and groups results with a [`Batcher`], sending
//! [`ScanUpdate`]s over a std `mpsc` channel. The UI polls the receiver on a
//! ~10 Hz subscription tick and applies **all pending batches per tick** —
//! one `Library::add_tracks` + one view-model rebuild per tick, never one
//! redraw per track — which is what lets the shelf populate live during a
//! scan without melting the frame budget.
//!
//! Failures follow the scanner's philosophy: per-file failures are counted
//! data ("N files skipped" in the status line), never a modal; only a scan
//! that cannot start at all surfaces as [`ScanUpdate::Error`].
//!
//! # Incremental by default
//!
//! [`spawn`] takes the [`KnownFiles`] snapshot the index already holds and
//! drives [`baz_core::library::scan_incremental`], so a launch re-reads tags
//! only for files that are new or whose size/mtime moved. An unchanged file
//! also produces no [`ScanUpdate::Batch`] entry at all, so a warm launch
//! costs the UI thread no `add_tracks` and no view-model rebuild either.
//!
//! Measured on a synthetic 10 000-file library
//! (`baz-core/benches/scan.rs`): the scan drops from **61.2 ms to 10.3 ms**
//! (5.9×) and the whole launch from **83.4 ms to 11.6 ms** (7.2×). Both are
//! lower bounds — see that bench's header for why.
//!
//! # Several roots, one pass
//!
//! [`spawn`] takes a **set** of library roots (ADR-0022) and walks them in the
//! order the listener listed them, reporting each batch with the root it came
//! from so the index can record it. A root that cannot be walked at all — an
//! unmounted NAS, a drive that is not plugged in, a folder somebody renamed —
//! is reported as [`ScanUpdate::RootUnavailable`] and the pass **continues with
//! the others**; it is not an error, and it prunes nothing from any root,
//! including its own.
//!
//! # Removal: only what was positively confirmed gone
//!
//! A scan that finishes also *prunes*, and the policy is deliberately the
//! conservative one — see [`vanished`]. Rows are never deleted for being
//! merely unseen; a path has to survive four independent checks, each of
//! which exists because of a specific way "I did not see it" is not "it is
//! not there": a root that is not this one, an unreadable directory, an absent
//! mount, and a root whose walk reached nothing at all.
//!
//! # Three ways the index refreshes, and they are different things
//!
//! - **At launch.** [`spawn`] with [`ScanMode::Incremental`], once, from
//!   `Shelf::open`. Unchanged files are never opened.
//! - **While running.** [`Refresh`] is the clock: the same incremental pass
//!   again, every [`REFRESH_INTERVAL`], measured from the moment the last one
//!   *finished*. baz does not watch the filesystem, and ADR-0022 §3 records the
//!   evaluation of `notify` that decided so.
//! - **Force sync.** [`ScanMode::Force`], from a control in the Settings place:
//!   every file is re-read whatever its stamp says. Distinct from a rescan,
//!   which is what the other two are.
//!
//! All three run on the `baz-scan` worker thread and hand the UI thread
//! batches; none of them touches the engine, which is a separate process-level
//! thread with its own queue. Nothing here can make a sample late.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use baz_core::library::{KnownFiles, ScanEntry, TrackMeta, is_confirmed_gone};

/// Flush a batch when it holds this many tracks, even mid-interval.
/// Bounds per-tick `add_tracks` cost (SQLite write + index re-sort).
pub const BATCH_MAX_TRACKS: usize = 256;

/// Flush a batch at least this often while entries trickle in, so a slow
/// directory (cold NAS, spinning disk) still paints albums promptly.
pub const BATCH_INTERVAL: Duration = Duration::from_millis(150);

/// How long baz waits between rescans **while it is running** — the second of
/// ADR-0022's three refresh mechanisms.
///
/// Measured from the moment the previous scan *finished*, so two passes can
/// never overlap and a slow library cannot build a backlog of them (see
/// [`Refresh`]).
///
/// Five minutes is chosen against what a pass costs and what a listener
/// notices. The warm pass is one `stat` per file — 10.3 ms per 10 000 files
/// measured (`baz-core/benches/scan.rs`), so ~100 ms for the 100k library the
/// search index is built for — on a worker thread, with the UI applying
/// batches at 10 Hz. A minute would spend that a hundred times an hour to
/// notice a rip a listener already knows they made; an hour would leave a fresh
/// import off the wall for most of an afternoon. Five minutes is under the time
/// it takes to rip a CD, which is the shortest interval at which a new record
/// actually appears.
pub const REFRESH_INTERVAL: Duration = Duration::from_secs(300);

/// What a scan does with the stamps the index already holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    /// Trust the stamp: a file whose size and mtime are unchanged is reported
    /// as unchanged and never opened. What a launch and the periodic refresh
    /// both do.
    Incremental,
    /// **Force sync**: ignore every freshness shortcut and re-read every file's
    /// tags, whatever its stamp says.
    ///
    /// A different act from a rescan, not a more thorough one. A rescan asks
    /// "what has changed?"; a force sync says "assume nothing about what I
    /// already believe" — which is the only answer for the case the stamp
    /// cannot see (a file rewritten in place to exactly its old length with its
    /// mtime restored, ADR-0010 §1), and for a listener who suspects the index
    /// rather than the disk.
    Force,
}

impl ScanMode {
    /// Whether this mode may skip a file whose stamp is unchanged.
    #[must_use]
    pub fn trusts_stamps(self) -> bool {
        matches!(self, Self::Incremental)
    }
}

/// One message from the scan worker to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanUpdate {
    /// A batch of progress: tracks read since the last update, plus how many
    /// files failed in the same window (already counted, not itemized).
    Batch {
        /// The library root these tracks were found under — what makes the
        /// write a `Library::add_tracks_under` rather than an `add_tracks`, and
        /// therefore what removal's second gate will later read.
        root: PathBuf,
        /// Successfully read tracks, ready for `Library::add_tracks_under`.
        tracks: Vec<TrackMeta>,
        /// Files/directories the scanner skipped in this window.
        failed: usize,
    },
    /// One root's walk finished. Sent per root, before the removal pass.
    RootDone {
        /// The root that was walked.
        root: PathBuf,
        /// When it finished, in nanoseconds since the Unix epoch — for
        /// `Library::record_scan`, so the Settings place can say when baz last
        /// looked at this folder.
        at_ns: i64,
        /// Files read for the first time under this root.
        added: usize,
        /// Known files re-read under this root.
        updated: usize,
        /// Known files skipped whole under this root.
        unchanged: usize,
        /// Files/directories skipped under this root.
        failed: usize,
    },
    /// A configured root could not be walked at all: it does not exist, or it
    /// is not a directory.
    ///
    /// **Not an error, and not fatal to the pass.** This is what an unmounted
    /// NAS looks like, and the whole point of recording roots is that one
    /// absent folder now costs exactly its own tracks' *freshness* and nothing
    /// else: the remaining roots are walked normally, and nothing is pruned
    /// from any root — including this one, whose rows survive precisely because
    /// its walk produced nothing (see [`vanished`]).
    RootUnavailable {
        /// The root that could not be walked.
        root: PathBuf,
        /// Why, in the scanner's own words.
        reason: String,
    },
    /// Index rows the scan positively confirmed are gone from disk (see
    /// [`vanished`]), for `Library::remove_tracks`. Sent once, just before
    /// [`ScanUpdate::Done`], and only when there is something to remove.
    Removed {
        /// The paths whose rows must go.
        paths: Vec<PathBuf>,
    },
    /// The scan finished; totals for the throughput log and status line.
    Done {
        /// Files read for the first time.
        added: usize,
        /// Known files re-read because their size or mtime moved — or, in
        /// [`ScanMode::Force`], because they were re-read regardless.
        updated: usize,
        /// Known files skipped whole: unchanged stamp, tags not re-read.
        /// Always zero after a force sync, which skips nothing.
        unchanged: usize,
        /// Rows removed as confirmed gone.
        removed: usize,
        /// Total files/directories skipped.
        failed: usize,
        /// Configured roots that could not be walked at all.
        unavailable: usize,
        /// Wall-clock scan duration.
        elapsed: Duration,
    },
    /// The worker thread could not be spawned.
    ///
    /// A root that cannot be walked is **not** this — it is
    /// [`ScanUpdate::RootUnavailable`], and the pass carries on.
    Error(String),
}

/// Groups scan entries into batches by count ([`BATCH_MAX_TRACKS`]) and time
/// ([`BATCH_INTERVAL`]). Pure state machine — the caller supplies `now`, so
/// the flush policy is unit-testable without sleeping.
///
/// One batcher per **root**: every batch it emits names the root it belongs to,
/// and a batch that mixed two roots could not be written with either one's
/// name. The worker therefore makes a fresh batcher for each root and drains it
/// before moving on.
#[derive(Debug)]
pub struct Batcher {
    root: PathBuf,
    tracks: Vec<TrackMeta>,
    failed: usize,
    last_flush: Instant,
    max_tracks: usize,
    interval: Duration,
}

impl Batcher {
    /// A batcher for one root, starting its interval clock at `now`.
    #[must_use]
    pub fn new(root: PathBuf, max_tracks: usize, interval: Duration, now: Instant) -> Self {
        Self {
            root,
            tracks: Vec::new(),
            failed: 0,
            last_flush: now,
            max_tracks,
            interval,
        }
    }

    /// Absorb one scan entry; returns a batch when the count cap or the
    /// interval says it is time to flush.
    pub fn push(&mut self, entry: ScanEntry, now: Instant) -> Option<ScanUpdate> {
        match entry {
            ScanEntry::Track(meta) => self.tracks.push(meta),
            ScanEntry::Failed { .. } => self.failed += 1,
            // Nothing to write: the row already in the index is current.
            // It is counted in `Done`, not carried through a batch — a warm
            // launch should cost the UI no `add_tracks` at all.
            ScanEntry::Unchanged { .. } => {}
        }
        let count_full = self.tracks.len() >= self.max_tracks;
        let interval_due = now.duration_since(self.last_flush) >= self.interval;
        if count_full || interval_due {
            self.flush(now)
        } else {
            None
        }
    }

    /// Emit whatever is pending (also restarting the interval clock), or
    /// `None` when there is nothing to report.
    pub fn flush(&mut self, now: Instant) -> Option<ScanUpdate> {
        self.last_flush = now;
        if self.tracks.is_empty() && self.failed == 0 {
            return None;
        }
        Some(ScanUpdate::Batch {
            root: self.root.clone(),
            tracks: std::mem::take(&mut self.tracks),
            failed: std::mem::take(&mut self.failed),
        })
    }
}

/// The clock behind the **second** refresh mechanism: a rescan every
/// [`REFRESH_INTERVAL`] while baz is running (ADR-0022 §3).
///
/// Pure state machine, like [`Batcher`] beside it — the caller supplies `now`,
/// so the policy is unit-testable without sleeping through five minutes.
///
/// Two rules, and both are about never letting the refresh become a problem of
/// its own:
///
/// 1. **Nothing is due while a scan is running.** The interval is a gap between
///    passes, not a schedule that a slow pass can fall behind.
/// 2. **The clock restarts when a scan finishes**, not when one starts, so two
///    passes can never overlap and a library that takes six minutes to walk is
///    rescanned every eleven rather than continuously.
#[derive(Debug)]
pub struct Refresh {
    interval: Duration,
    since: Instant,
}

impl Refresh {
    /// A clock that will next fire `interval` after `now`.
    #[must_use]
    pub fn new(interval: Duration, now: Instant) -> Self {
        Self {
            interval,
            since: now,
        }
    }

    /// Whether a rescan is due. `scanning` is the caller's own answer to "is
    /// one already running", and while it is true nothing is ever due.
    ///
    /// Asking does not restart the clock — [`Refresh::restarted`] does, and the
    /// caller calls it when a pass **finishes**.
    #[must_use]
    pub fn due(&self, now: Instant, scanning: bool) -> bool {
        !scanning && now.duration_since(self.since) >= self.interval
    }

    /// Restart the interval from `now` — called when a scan finishes, and when
    /// one is started for any other reason (a force sync, a folder added), so
    /// that a manual refresh also resets the automatic one.
    pub fn restarted(&mut self, now: Instant) {
        self.since = now;
    }
}

/// Start scanning `roots` on a worker thread; the returned receiver yields
/// [`ScanUpdate`]s until a final `Done` (or `Error`). Dropping the receiver
/// stops the worker at its next send.
///
/// The roots are walked in the order given — the order the listener listed
/// them — and a root that cannot be walked is reported and stepped over
/// ([`ScanUpdate::RootUnavailable`]) rather than ending the pass.
///
/// `known` is the index's own snapshot (`Library::known_files`). It is what
/// makes the scan incremental, and it is also the *only* list of rows the
/// removal pass will ever consider — a path the caller did not hand over
/// cannot be deleted by this worker under any circumstances.
///
/// `mode` decides whether the stamps in `known` are trusted at all; see
/// [`ScanMode`].
pub fn spawn(roots: Vec<PathBuf>, known: KnownFiles, mode: ScanMode) -> Receiver<ScanUpdate> {
    let (tx, rx) = channel();
    let worker_tx = tx.clone();
    let spawned = std::thread::Builder::new()
        .name("baz-scan".to_owned())
        .spawn(move || run_scan(&roots, &known, mode, &worker_tx));
    if let Err(err) = spawned {
        // Channel still open (rx just created); ignore-send is unreachable.
        let _ = tx.send(ScanUpdate::Error(format!("could not start scan: {err}")));
    }
    rx
}

/// What the walk observed, for the removal pass. Borrows its paths from
/// `known`, so recording a 100k-track library costs no allocation per file.
#[derive(Default)]
struct Walked<'a> {
    /// Known paths this scan laid eyes on — read, skipped as unchanged, or
    /// individually failed. All three prove the file is still there.
    seen: HashSet<&'a Path>,
    /// Paths `walkdir` could not traverse: unreadable directories, mostly.
    /// Nothing at or below one of these is removable.
    unreadable: Vec<PathBuf>,
    /// The roots whose walk produced **at least one entry** this pass.
    ///
    /// Per root rather than one flag for the whole scan, because that is the
    /// grain the danger has: with several folders configured, one of them being
    /// an empty mount point must cost that folder its pruning and cost the
    /// others nothing.
    productive: HashSet<PathBuf>,
}

/// Worker body: walk each root through its own [`Batcher`] into `tx`, then run
/// one removal pass over everything that was walked.
fn run_scan(roots: &[PathBuf], known: &KnownFiles, mode: ScanMode, tx: &Sender<ScanUpdate>) {
    let started = Instant::now();
    let mut walked = Walked::default();
    let mut totals = Counts::default();
    for root in roots {
        match walk_root(root, known, mode, tx, &mut walked) {
            Walk::Stopped => return, // UI hung up; prune nothing.
            Walk::Unavailable(reason) => {
                totals.unavailable += 1;
                if tx
                    .send(ScanUpdate::RootUnavailable {
                        root: root.clone(),
                        reason,
                    })
                    .is_err()
                {
                    return;
                }
            }
            Walk::Walked(counts) => {
                totals.add(&counts);
                if tx
                    .send(ScanUpdate::RootDone {
                        root: root.clone(),
                        at_ns: now_ns(),
                        added: counts.added,
                        updated: counts.updated,
                        unchanged: counts.unchanged,
                        failed: counts.failed,
                    })
                    .is_err()
                {
                    return;
                }
            }
        }
    }

    let gone = vanished(known, &walked);
    totals.removed = gone.len();
    if !gone.is_empty() && tx.send(ScanUpdate::Removed { paths: gone }).is_err() {
        return;
    }
    let _ = tx.send(ScanUpdate::Done {
        added: totals.added,
        updated: totals.updated,
        unchanged: totals.unchanged,
        removed: totals.removed,
        failed: totals.failed,
        unavailable: totals.unavailable,
        elapsed: started.elapsed(),
    });
}

/// How one root's walk ended.
enum Walk {
    /// The root was walked; here is what it found.
    Walked(Counts),
    /// The root could not be walked at all, in the scanner's words.
    Unavailable(String),
    /// The receiver hung up mid-walk. Nothing more may be sent, and nothing
    /// may be pruned — this pass never finished.
    Stopped,
}

/// Walk one root, streaming its batches, and record what it saw.
fn walk_root<'a>(
    root: &Path,
    known: &'a KnownFiles,
    mode: ScanMode,
    tx: &Sender<ScanUpdate>,
    walked: &mut Walked<'a>,
) -> Walk {
    // The one place the two modes differ. `scan` re-reads every file it finds;
    // `scan_incremental` consults the stamps first. Everything downstream —
    // the batching, the counts, the removal pass — is identical, which is what
    // makes "force sync" a mode rather than a second pipeline.
    let scan = if mode.trusts_stamps() {
        baz_core::library::scan_incremental(root, known)
    } else {
        baz_core::library::scan(root)
    };
    let scan = match scan {
        Ok(scan) => scan,
        Err(err) => return Walk::Unavailable(err.to_string()),
    };
    let mut batcher = Batcher::new(
        root.to_path_buf(),
        BATCH_MAX_TRACKS,
        BATCH_INTERVAL,
        Instant::now(),
    );
    let mut counts = Counts::default();
    for entry in scan {
        walked.productive.insert(root.to_path_buf());
        record(&entry, known, walked, &mut counts);
        if let Some(update) = batcher.push(entry, Instant::now())
            && tx.send(update).is_err()
        {
            return Walk::Stopped;
        }
    }
    if let Some(update) = batcher.flush(Instant::now())
        && tx.send(update).is_err()
    {
        return Walk::Stopped;
    }
    Walk::Walked(counts)
}

/// The moment now, in nanoseconds since the Unix epoch — what a finished root's
/// scan time is recorded as.
///
/// Saturating rather than panicking on an absurd clock, exactly as the index's
/// own first-seen stamp is: a wrong "last scanned" line is better than a scan
/// that refuses to report finishing.
fn now_ns() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => {
            i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, i64::saturating_neg)
        }
    }
}

/// The running tallies `Done` reports.
#[derive(Default)]
struct Counts {
    added: usize,
    updated: usize,
    unchanged: usize,
    removed: usize,
    failed: usize,
    unavailable: usize,
}

impl Counts {
    /// Fold one root's tallies into the pass's.
    fn add(&mut self, other: &Self) {
        self.added += other.added;
        self.updated += other.updated;
        self.unchanged += other.unchanged;
        self.failed += other.failed;
    }
}

/// Note one entry against the walk record and the added/updated/unchanged
/// tallies. A path is "added" exactly when the index did not already hold
/// it, which is the only place that distinction is visible.
fn record<'a>(
    entry: &ScanEntry,
    known: &'a KnownFiles,
    walked: &mut Walked<'a>,
    counts: &mut Counts,
) {
    let path = match entry {
        ScanEntry::Track(meta) => {
            if known.contains_key(&meta.path) {
                counts.updated += 1;
            } else {
                counts.added += 1;
            }
            meta.path.as_path()
        }
        ScanEntry::Unchanged { path } => {
            counts.unchanged += 1;
            path.as_path()
        }
        ScanEntry::Failed { path, .. } => {
            counts.failed += 1;
            // A directory that would not open, most often. Remember it so
            // nothing beneath it can be pruned — an unreadable directory is
            // not evidence about the files inside it.
            walked.unreadable.push(path.clone());
            path.as_path()
        }
    };
    if let Some((known_path, _)) = known.get_key_value(path) {
        walked.seen.insert(known_path.as_path());
    }
}

/// The rows this scan **proved** are gone, and may therefore be deleted.
///
/// The rule is *positive confirmation*, not absence of evidence: baz removes
/// only what it looked for and found missing, never "everything I did not
/// happen to see this pass". A row must clear all four of these, and each
/// one is a real way a library gets destroyed by the naive rule:
///
/// 1. **The row's own root produced something this pass.** A root whose walk
///    yielded no entry whatsoever is not proof that its folder is empty — it is
///    what an unmounted NAS or an unplugged drive looks like when the mount
///    point survives as an empty directory. Zero entries under a root prunes
///    zero rows *from that root*, and costs the other roots nothing.
/// 2. **The row names that root.** This is the gate ADR-0022 replaced, and the
///    replacement is the whole point of schema v8. It used to be
///    `path.starts_with(root_being_scanned)` — a guess that a file under a
///    folder must have come from it, which is true only while roots cannot
///    nest and no file is reachable from two of them. With several folders
///    configured, both assumptions fail immediately: `~/Music` and
///    `~/Music/Live` claim the same files, and so do a folder and a symlink
///    into it. Now the row says which root's walk read it
///    (`baz_core::library::KnownFile::root`), and only that root's scan may
///    nominate it. A row that names **no** root — pre-v8, unadopted — is
///    therefore prunable by nobody, which is the safe direction.
/// 3. **No unreadable ancestor.** If `walkdir` reported a directory as
///    [`ScanEntry::Failed`] — permissions, I/O error, a half-mounted share —
///    then everything under it was never looked at, and a scan that failed
///    partway must not delete what it could not reach.
/// 4. **The filesystem confirms it** — [`is_confirmed_gone`]: the file's
///    parent directory is present *and* the file itself stats as
///    `NotFound`. Requiring the parent is what makes an absent mount cost
///    nothing, since a missing directory answers `NotFound` for every path
///    below it whether those files were deleted or merely unplugged.
///
/// The price of rule 4 is stated rather than hidden: deleting a whole album
/// *folder* leaves its rows behind, because from the filesystem's side that
/// is indistinguishable from the folder being an unmounted mount point.
/// A stale row is a cosmetic wrong; deleting a present listener's library is
/// not. `docs/BACKLOG.md` carries the remaining case.
fn vanished(known: &KnownFiles, walked: &Walked<'_>) -> Vec<PathBuf> {
    let mut gone: Vec<PathBuf> = known
        .iter()
        .filter(|(path, _)| !walked.seen.contains(path.as_path()))
        // Gates 1 and 2 are one lookup: the row's recorded root has to be a
        // root whose walk actually produced something this pass.
        .filter(|(_, known)| {
            known
                .root
                .as_deref()
                .is_some_and(|root| walked.productive.contains(root))
        })
        .map(|(path, _)| path)
        .filter(|path| !has_unreadable_ancestor(path, &walked.unreadable))
        .filter(|path| is_confirmed_gone(path))
        .cloned()
        .collect();
    // `known` is a hash map, so sort: a list of files about to be deleted is
    // the last place to accept a run-to-run-varying order in a log.
    gone.sort_unstable();
    gone
}

/// Whether any path the walk could not traverse is `path` itself or one of
/// its ancestors.
fn has_unreadable_ancestor(path: &Path, unreadable: &[PathBuf]) -> bool {
    unreadable.iter().any(|bad| path.starts_with(bad))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::sync::Arc;
    use std::time::SystemTime;

    use baz_core::library::{FileStamp, KnownFile};
    use baz_core::replaygain::ReplayGainTags;

    fn track(n: u32) -> ScanEntry {
        ScanEntry::Track(TrackMeta {
            path: PathBuf::from(format!("/m/{n}.flac")),
            artist: None,
            album_artist: None,
            compilation: None,
            genre: None,
            album: None,
            title: None,
            track: Some(n),
            disc: None,
            year: None,
            duration: None,
            format: Some(baz_core::library::AudioFormat::Flac),
            bit_depth: Some(16),
            sample_rate: Some(44_100),
            bitrate: Some(900),
            stamp: None,
            replay_gain: ReplayGainTags::default(),
        })
    }

    fn failed() -> ScanEntry {
        ScanEntry::Failed {
            path: PathBuf::from("/m/bad"),
            reason: "nope".to_owned(),
        }
    }

    /// A batcher for the one root the pure batching tests use.
    fn batcher(max_tracks: usize, interval: Duration, now: Instant) -> Batcher {
        Batcher::new(PathBuf::from("/m"), max_tracks, interval, now)
    }

    /// A real, tiny, valid WAV at `root/rel` (parents created).
    fn wav(root: &Path, rel: &str) -> PathBuf {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).expect("wav");
        writer.write_sample(0i16).expect("sample");
        writer.finalize().expect("finalize");
        path
    }

    /// The index snapshot for files that are all currently on disk and were all
    /// recorded under `root` — what the index holds after a scan of it.
    fn known_under<'a>(root: &Path, paths: impl IntoIterator<Item = &'a Path>) -> KnownFiles {
        let root: Arc<Path> = Arc::from(root);
        paths
            .into_iter()
            .map(|path| {
                (
                    path.to_path_buf(),
                    KnownFile::new(FileStamp::of_path(path), Some(Arc::clone(&root))),
                )
            })
            .collect()
    }

    /// Everything a worker run produced, flattened.
    #[derive(Default)]
    struct Run {
        batched: Vec<(PathBuf, TrackMeta)>,
        removed: Vec<PathBuf>,
        roots_done: Vec<PathBuf>,
        unavailable: Vec<PathBuf>,
        done: Option<(usize, usize, usize, usize, usize, usize)>,
        error: Option<String>,
    }

    impl Run {
        /// The paths the run actually opened and read.
        fn read(&self) -> Vec<&PathBuf> {
            self.batched.iter().map(|(_, meta)| &meta.path).collect()
        }
    }

    /// Run the worker to completion over `roots` with `known` as the index.
    fn drive(roots: &[&Path], known: KnownFiles, mode: ScanMode) -> Run {
        let mut run = Run::default();
        let roots: Vec<PathBuf> = roots.iter().map(|root| root.to_path_buf()).collect();
        for update in spawn(roots, known, mode) {
            match update {
                ScanUpdate::Batch { root, tracks, .. } => run
                    .batched
                    .extend(tracks.into_iter().map(|meta| (root.clone(), meta))),
                ScanUpdate::Removed { paths } => run.removed.extend(paths),
                ScanUpdate::RootDone { root, .. } => run.roots_done.push(root),
                ScanUpdate::RootUnavailable { root, .. } => run.unavailable.push(root),
                ScanUpdate::Done {
                    added,
                    updated,
                    unchanged,
                    removed,
                    failed,
                    unavailable,
                    ..
                } => {
                    run.done = Some((added, updated, unchanged, removed, failed, unavailable));
                }
                ScanUpdate::Error(err) => run.error = Some(err),
            }
        }
        run
    }

    /// The ordinary case: one root, incremental, as every launch runs it.
    fn drive_one(root: &Path, known: KnownFiles) -> Run {
        drive(&[root], known, ScanMode::Incremental)
    }

    #[test]
    fn flushes_when_count_cap_is_reached() {
        let now = Instant::now();
        let mut batcher = batcher(3, Duration::from_secs(3600), now);
        assert_eq!(batcher.push(track(1), now), None);
        assert_eq!(batcher.push(track(2), now), None);
        let update = batcher.push(track(3), now).expect("cap flush");
        let ScanUpdate::Batch {
            root,
            tracks,
            failed,
        } = update
        else {
            panic!("expected a batch");
        };
        assert_eq!(tracks.len(), 3);
        assert_eq!(failed, 0);
        // Every batch names the root it belongs to, which is what lets the
        // index record where each row came from.
        assert_eq!(root, PathBuf::from("/m"));
        // Buffer is drained; nothing left to flush.
        assert_eq!(batcher.flush(now), None);
    }

    #[test]
    fn flushes_when_interval_elapses_even_below_cap() {
        let now = Instant::now();
        let mut batcher = batcher(1000, Duration::from_millis(150), now);
        assert_eq!(batcher.push(track(1), now), None);
        let later = now + Duration::from_millis(151);
        let update = batcher.push(track(2), later).expect("interval flush");
        assert!(matches!(update, ScanUpdate::Batch { ref tracks, .. } if tracks.len() == 2));
        // Interval clock restarted at the flush.
        assert_eq!(batcher.push(track(3), later), None);
    }

    #[test]
    fn failures_are_counted_and_flush_on_interval_alone() {
        let now = Instant::now();
        let mut batcher = batcher(1000, Duration::from_millis(150), now);
        assert_eq!(batcher.push(failed(), now), None);
        let later = now + Duration::from_millis(200);
        let update = batcher.push(failed(), later).expect("failed-only flush");
        assert_eq!(
            update,
            ScanUpdate::Batch {
                root: PathBuf::from("/m"),
                tracks: Vec::new(),
                failed: 2
            }
        );
    }

    #[test]
    fn empty_flush_is_none_but_restarts_clock() {
        let now = Instant::now();
        let mut batcher = batcher(10, Duration::from_millis(150), now);
        assert_eq!(batcher.flush(now + Duration::from_secs(1)), None);
    }

    /// An unchanged file carries no metadata to write, so it must not cost
    /// the UI an `add_tracks` — the whole point of the warm path.
    #[test]
    fn unchanged_entries_never_enter_a_batch() {
        let now = Instant::now();
        let mut batcher = batcher(1, Duration::from_secs(3600), now);
        let entry = ScanEntry::Unchanged {
            path: PathBuf::from("/m/1.flac"),
        };
        assert_eq!(batcher.push(entry, now), None);
        assert_eq!(batcher.flush(now), None, "nothing pending to flush");
    }

    /// The periodic refresh's whole policy, as arithmetic: a gap between
    /// passes, never a schedule a slow pass can fall behind (ADR-0022 §3).
    #[test]
    fn the_refresh_clock_is_a_gap_between_passes_not_a_schedule() {
        let start = Instant::now();
        let mut refresh = Refresh::new(Duration::from_secs(300), start);

        assert!(!refresh.due(start, false), "nothing is due immediately");
        assert!(
            !refresh.due(start + Duration::from_secs(299), false),
            "not due a second early"
        );
        assert!(
            refresh.due(start + Duration::from_secs(300), false),
            "due exactly on the interval"
        );

        // Rule 1: while a scan is running, nothing is ever due — however long
        // it has been. A pass that outlives the interval must not queue another.
        assert!(!refresh.due(start + Duration::from_secs(3600), true));

        // Rule 2: the clock restarts when the pass *finishes*, so a six-minute
        // walk is followed by five minutes of quiet rather than by another walk.
        let finished = start + Duration::from_secs(360);
        refresh.restarted(finished);
        assert!(!refresh.due(finished, false));
        assert!(!refresh.due(finished + Duration::from_secs(299), false));
        assert!(refresh.due(finished + Duration::from_secs(300), false));

        // Asking does not restart it: a caller that checks every tick still
        // gets a `true` on every tick until it actually runs a pass.
        assert!(refresh.due(finished + Duration::from_secs(301), false));
    }

    #[test]
    fn worker_streams_batches_then_done() {
        let dir = tempfile::tempdir().expect("tempdir");
        wav(dir.path(), "a.wav");
        wav(dir.path(), "b.wav");
        fs::write(dir.path().join("broken.flac"), b"not flac").expect("write");

        let run = drive_one(dir.path(), KnownFiles::new());
        assert!(run.error.is_none());
        assert_eq!(run.batched.len(), 2);
        // Two added, nothing updated, nothing unchanged, nothing removed,
        // one file skipped, no root unavailable.
        assert_eq!(run.done, Some((2, 0, 0, 0, 1, 0)));
        assert!(
            run.removed.is_empty(),
            "an empty index has nothing to prune"
        );
        // Every batch is stamped with the root that produced it.
        assert!(run.batched.iter().all(|(root, _)| root == dir.path()),);
        assert_eq!(run.roots_done, vec![dir.path().to_path_buf()]);
    }

    /// A root that is not there is news, not a failure: the pass reports it and
    /// carries on. It was an `Error` that ended the whole scan when baz held
    /// one folder; with several, one unmounted share must not stop the others.
    #[test]
    fn a_missing_root_is_reported_unavailable_rather_than_failing_the_scan() {
        let missing = PathBuf::from("/definitely/not/here/baz-test");
        let run = drive(
            &[missing.as_path()],
            KnownFiles::new(),
            ScanMode::Incremental,
        );
        assert_eq!(run.unavailable, vec![missing]);
        assert!(run.error.is_none(), "an absent folder is not an error");
        assert_eq!(run.done, Some((0, 0, 0, 0, 0, 1)));
    }

    /// A root that vanished under a library full of rows is the single most
    /// destructive moment for a "remove what you did not see" rule. It must
    /// remove nothing at all.
    #[test]
    fn a_scan_whose_root_is_missing_removes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = wav(dir.path(), "Artist/Album/01.wav");
        let gone_root = dir.path().join("not-here");
        let known = known_under(&gone_root, [a.as_path()]);

        let run = drive_one(&gone_root, known);
        assert_eq!(run.unavailable, vec![gone_root]);
        assert!(run.removed.is_empty());
    }

    /// An unmounted share leaves its mount point behind as an empty
    /// directory. Scanning it finds nothing — which is not the same fact as
    /// "the library is empty".
    #[test]
    fn a_scan_that_saw_nothing_removes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = wav(dir.path(), "Artist/Album/01.wav");
        let known = known_under(dir.path(), [a.as_path()]);
        // Now simulate the unmount: the whole tree disappears, the mount
        // point remains.
        fs::remove_dir_all(dir.path().join("Artist")).expect("unmount");

        let run = drive_one(dir.path(), known);
        assert_eq!(run.done, Some((0, 0, 0, 0, 0, 0)));
        assert!(
            run.removed.is_empty(),
            "an empty scan is not evidence of an empty library"
        );
    }

    #[test]
    fn a_deleted_files_row_is_removed_and_its_neighbours_are_not() {
        let dir = tempfile::tempdir().expect("tempdir");
        let keep = wav(dir.path(), "Artist/Album/01.wav");
        let doomed = wav(dir.path(), "Artist/Album/02.wav");
        let known = known_under(dir.path(), [keep.as_path(), doomed.as_path()]);
        fs::remove_file(&doomed).expect("delete");

        let run = drive_one(dir.path(), known);
        assert_eq!(run.removed, vec![doomed], "exactly the deleted file");
        assert_eq!(
            run.done,
            Some((0, 0, 1, 1, 0, 0)),
            "one unchanged, one removed"
        );
    }

    /// The conservative rule, stated as a test: a missing *directory* is not
    /// proof about the files inside it. An unplugged drive and a deleted
    /// folder are the same `NotFound` from below, so neither prunes.
    #[test]
    fn a_row_under_a_missing_directory_is_retained() {
        let dir = tempfile::tempdir().expect("tempdir");
        let present = wav(dir.path(), "Artist/Album/01.wav");
        // A row for a file on a share that is not mounted today. It lives
        // under the root, and its whole directory is absent.
        let absent = dir.path().join("Artist/NAS Album/07.wav");
        let mut known = known_under(dir.path(), [present.as_path()]);
        known.insert(
            absent.clone(),
            KnownFile::new(None, Some(Arc::from(dir.path()))),
        );

        let run = drive_one(dir.path(), known);
        assert!(
            run.removed.is_empty(),
            "a file whose directory is absent must survive: {:?}",
            run.removed
        );
        assert_eq!(run.done, Some((0, 0, 1, 0, 0, 0)));
    }

    /// Scanning one folder is silence about every other: rows recorded under a
    /// root this pass did not walk are untouchable, however gone their files
    /// are.
    #[test]
    fn scanning_a_second_root_leaves_the_first_roots_rows_alone() {
        let first = tempfile::tempdir().expect("tempdir");
        let second = tempfile::tempdir().expect("tempdir");
        let old = wav(first.path(), "Artist/Album/01.wav");
        let new = wav(second.path(), "Other/Album/01.wav");
        let mut known = known_under(first.path(), [old.as_path()]);
        known.extend(known_under(second.path(), [new.as_path()]));
        // The first root is even deleted outright — still none of ours to
        // judge while scanning the second.
        fs::remove_dir_all(first.path()).expect("remove first root");

        let run = drive_one(second.path(), known);
        assert!(run.removed.is_empty(), "removed {:?}", run.removed);
        assert_eq!(run.done, Some((0, 0, 1, 0, 0, 0)));
    }

    /// **The gate ADR-0022 replaced, in the case that broke it.** Two roots,
    /// one nested inside the other. Scanning only the inner one must not prune
    /// the outer one's rows — even though every one of those rows passes
    /// `starts_with(inner_root)`'s ancestor test in the other direction, and
    /// even though a row of the *inner* root sits in the same directory tree.
    #[test]
    fn a_nested_root_prunes_only_the_rows_that_name_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let outer = dir.path().to_path_buf();
        let inner = dir.path().join("Live");
        // One file per root, both deleted from disk, both under `inner`'s tree.
        let outers = wav(&inner, "Bootlegs/01.wav");
        let inners = wav(&inner, "Bootlegs/02.wav");
        // A third file keeps the inner walk productive.
        wav(&inner, "Bootlegs/03.wav");
        let mut known = known_under(&outer, [outers.as_path()]);
        known.extend(known_under(&inner, [inners.as_path()]));
        fs::remove_file(&outers).expect("delete");
        fs::remove_file(&inners).expect("delete");

        let run = drive_one(&inner, known);
        assert_eq!(
            run.removed,
            vec![inners],
            "only the row recorded under the root being scanned"
        );

        // And scanning both roots reaches the outer row too, because both
        // roots are now productive: the record is what decides, not the prefix.
        let known = known_under(&outer, [outers.as_path()]);
        let run = drive(
            &[outer.as_path(), inner.as_path()],
            known,
            ScanMode::Incremental,
        );
        assert_eq!(run.removed, vec![outers]);
    }

    /// A file reachable from two roots belongs to the root that recorded it,
    /// and to no other. The old prefix gate had no way to express that: with a
    /// symlinked folder, both roots' `starts_with` answers are "yes" for paths
    /// neither of them put there.
    #[cfg(unix)]
    #[test]
    fn a_file_reachable_from_two_roots_is_pruned_only_by_the_one_that_recorded_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let real = dir.path().join("Real");
        let doomed = wav(&real, "Album/01.wav");
        wav(&real, "Album/02.wav"); // keeps the walk productive
        // A second root that is a symlink onto the first's tree: the same files,
        // two names, one of which is the only one the index ever recorded.
        let mirror = dir.path().join("Mirror");
        std::os::unix::fs::symlink(&real, &mirror).expect("symlink");

        let known = known_under(&real, [doomed.as_path()]);
        fs::remove_file(&doomed).expect("delete");

        // Walking the mirror sees the same directory under a different name, so
        // it never lays eyes on the recorded path — and it must not prune it,
        // because the row is not its to judge.
        let run = drive_one(&mirror, known.clone());
        assert!(
            run.removed.is_empty(),
            "the mirror root recorded nothing and may prune nothing: {:?}",
            run.removed
        );

        // The root that did record it prunes it, on the same disk state.
        let run = drive_one(&real, known);
        assert_eq!(run.removed, vec![doomed]);
    }

    /// An unmounted NAS beside a healthy folder: the healthy one is scanned and
    /// pruned normally, and the absent one costs **nothing anywhere** — not its
    /// own rows, not the other root's, and not the pass.
    #[test]
    fn an_absent_root_prunes_nothing_anywhere_and_does_not_fail_the_scan() {
        let here = tempfile::tempdir().expect("tempdir");
        let nas = tempfile::tempdir().expect("tempdir");
        let nas_root = nas.path().to_path_buf();
        let kept = wav(here.path(), "Artist/Album/01.wav");
        let doomed = wav(here.path(), "Artist/Album/02.wav");
        let on_the_nas = wav(&nas_root, "Artist/Album/09.wav");

        let mut known = known_under(here.path(), [kept.as_path(), doomed.as_path()]);
        known.extend(known_under(&nas_root, [on_the_nas.as_path()]));

        fs::remove_file(&doomed).expect("delete");
        // The share goes away entirely — mount point and all.
        drop(nas);

        let run = drive(
            &[here.path(), nas_root.as_path()],
            known,
            ScanMode::Incremental,
        );
        assert!(run.error.is_none(), "an absent root is not a scan failure");
        assert_eq!(run.unavailable, vec![nas_root]);
        assert_eq!(
            run.removed,
            vec![doomed],
            "only the deleted file under the root that was actually walked"
        );
        assert_eq!(
            run.done,
            Some((0, 0, 1, 1, 0, 1)),
            "one unchanged, one removed, one root unavailable"
        );
    }

    /// A row that names no root — every row in a pre-v8 index that no launch
    /// has adopted — is prunable by nobody, whatever the disk says.
    #[test]
    fn an_unrooted_row_is_never_pruned() {
        let dir = tempfile::tempdir().expect("tempdir");
        wav(dir.path(), "Artist/Album/01.wav");
        let doomed = dir.path().join("Artist/Album/02.wav");
        let known: KnownFiles = HashMap::from([(doomed.clone(), KnownFile::stamped(None))]);

        let run = drive_one(dir.path(), known);
        assert!(
            run.removed.is_empty(),
            "a row belonging to no root is nobody's to remove: {:?}",
            run.removed
        );
    }

    /// Rule 3 in isolation: a directory the walk could not read shelters
    /// everything below it, so a scan that failed partway deletes only what
    /// it actually reached and confirmed.
    #[test]
    fn a_partially_failed_scan_removes_only_what_it_confirmed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let reached = dir.path().join("Readable/01.wav");
        let unreached = dir.path().join("Locked/02.wav");
        fs::create_dir_all(dir.path().join("Readable")).expect("mkdir");
        fs::create_dir_all(dir.path().join("Locked")).expect("mkdir");
        let root: Arc<Path> = Arc::from(dir.path());
        // Both files are gone from disk; only one of them was looked for.
        let known: KnownFiles = HashMap::from([
            (
                reached.clone(),
                KnownFile::new(None, Some(Arc::clone(&root))),
            ),
            (unreached.clone(), KnownFile::new(None, Some(root))),
        ]);
        let walked = Walked {
            seen: HashSet::new(),
            unreadable: vec![dir.path().join("Locked")],
            productive: HashSet::from([dir.path().to_path_buf()]),
        };

        let gone = vanished(&known, &walked);
        assert_eq!(gone, vec![reached]);
    }

    /// The stamp check, end to end through the worker: an unchanged file is
    /// not re-read (proved by replacing its bytes with garbage a real read
    /// would report as a failure), a touched file is, and a new file is
    /// added.
    #[test]
    fn a_warm_scan_re_reads_only_what_moved() {
        let dir = tempfile::tempdir().expect("tempdir");
        let quiet = wav(dir.path(), "Artist/Album/01 Quiet.wav");
        let touched = wav(dir.path(), "Artist/Album/02 Touched.wav");
        let known = known_under(dir.path(), [quiet.as_path(), touched.as_path()]);

        gut_but_keep_the_stamp(&quiet, &known);

        // `touched` keeps its contents but moves forward in time.
        fs::File::options()
            .write(true)
            .open(&touched)
            .expect("reopen")
            .set_modified(SystemTime::now() + Duration::from_secs(120))
            .expect("touch");

        let fresh = wav(dir.path(), "Artist/Album/03 New.wav");

        let run = drive_one(dir.path(), known);
        assert_eq!(
            run.done,
            Some((1, 1, 1, 0, 0, 0)),
            "one added, one updated, one unchanged, nothing removed or failed"
        );
        let read = run.read();
        assert!(read.contains(&&touched) && read.contains(&&fresh));
        assert!(
            !read.contains(&&quiet),
            "the unchanged file must not have been opened — its bytes are garbage"
        );
        // And every row baz writes carries the stamp the next scan compares.
        assert!(run.batched.iter().all(|(_, meta)| meta.stamp.is_some()));
    }

    /// **Force sync ignores the stamp**, which is the whole of what makes it a
    /// different act from a rescan (ADR-0022 §3).
    ///
    /// Proved rather than asserted: the file's bytes are replaced with garbage
    /// while its size *and* mtime are restored exactly, so the index's stamp
    /// still matches the disk. An incremental pass therefore reports it
    /// unchanged and never opens it; a force sync opens it and finds the
    /// garbage. The failure is the receipt that the file was really re-read.
    #[test]
    fn a_force_sync_re_reads_a_file_whose_stamp_has_not_moved() {
        let dir = tempfile::tempdir().expect("tempdir");
        let quiet = wav(dir.path(), "Artist/Album/01 Quiet.wav");
        let known = known_under(dir.path(), [quiet.as_path()]);
        gut_but_keep_the_stamp(&quiet, &known);

        let warm = drive(&[dir.path()], known.clone(), ScanMode::Incremental);
        assert_eq!(
            warm.done,
            Some((0, 0, 1, 0, 0, 0)),
            "the incremental pass trusts the stamp and skips the file whole"
        );
        assert!(warm.read().is_empty(), "nothing was opened");

        let forced = drive(&[dir.path()], known, ScanMode::Force);
        assert_eq!(
            forced.done,
            Some((0, 0, 0, 0, 1, 0)),
            "the forced pass opened it, and the garbage inside it failed to parse"
        );
    }

    /// A force sync re-reads a file whose stamp has not moved and **keeps the
    /// row** — the tags it comes back with are written, and the pass prunes
    /// exactly what an incremental one would.
    #[test]
    fn a_force_sync_rewrites_the_rows_it_re_reads_and_prunes_the_same_way() {
        let dir = tempfile::tempdir().expect("tempdir");
        let kept = wav(dir.path(), "Artist/Album/01.wav");
        let doomed = wav(dir.path(), "Artist/Album/02.wav");
        let known = known_under(dir.path(), [kept.as_path(), doomed.as_path()]);
        fs::remove_file(&doomed).expect("delete");

        let run = drive(&[dir.path()], known, ScanMode::Force);
        assert_eq!(
            run.read(),
            vec![&kept],
            "an untouched file is re-read anyway"
        );
        assert_eq!(run.removed, vec![doomed]);
        assert_eq!(
            run.done,
            Some((0, 1, 0, 1, 0, 0)),
            "nothing unchanged: a force sync skips nothing"
        );
    }

    /// Replace a file's bytes with something no parser accepts, keeping its
    /// length and restoring its mtime, so its [`FileStamp`] is untouched. A
    /// scan that opens it reports a failure; a scan that trusts the stamp
    /// reports nothing at all.
    fn gut_but_keep_the_stamp(path: &Path, known: &KnownFiles) {
        let stamp = known[path]
            .stamp
            .expect("a stamp for a freshly written file");
        let len = usize::try_from(stamp.size).expect("small file");
        fs::write(path, vec![0xABu8; len]).expect("overwrite");
        fs::File::options()
            .write(true)
            .open(path)
            .expect("reopen")
            .set_modified(stamp.modified())
            .expect("restore mtime");
        assert_eq!(
            FileStamp::of_path(path),
            Some(stamp),
            "the fixture must be identical in size and mtime"
        );
    }

    #[test]
    fn an_unreadable_ancestor_shelters_everything_beneath_it() {
        let bad = [PathBuf::from("/m/Locked")];
        assert!(has_unreadable_ancestor(Path::new("/m/Locked/a.flac"), &bad));
        assert!(has_unreadable_ancestor(Path::new("/m/Locked"), &bad));
        assert!(!has_unreadable_ancestor(Path::new("/m/Open/a.flac"), &bad));
        // Prefix matching is per component, not per byte.
        assert!(!has_unreadable_ancestor(
            Path::new("/m/Locked Out/a.flac"),
            &bad
        ));
    }
}
