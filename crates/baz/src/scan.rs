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
//! # Removal: only what was positively confirmed gone
//!
//! A scan that finishes also *prunes*, and the policy is deliberately the
//! conservative one — see [`vanished`]. Rows are never deleted for being
//! merely unseen; a path has to survive four independent checks, each of
//! which exists because of a specific way "I did not see it" is not "it is
//! not there": a second music root, an unreadable directory, an absent
//! mount, and a scan that reached nothing at all.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use baz_core::library::{KnownFiles, ScanEntry, TrackMeta, is_confirmed_gone};

/// Flush a batch when it holds this many tracks, even mid-interval.
/// Bounds per-tick `add_tracks` cost (SQLite write + index re-sort).
pub const BATCH_MAX_TRACKS: usize = 256;

/// Flush a batch at least this often while entries trickle in, so a slow
/// directory (cold NAS, spinning disk) still paints albums promptly.
pub const BATCH_INTERVAL: Duration = Duration::from_millis(150);

/// One message from the scan worker to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanUpdate {
    /// A batch of progress: tracks read since the last update, plus how many
    /// files failed in the same window (already counted, not itemized).
    Batch {
        /// Successfully read tracks, ready for `Library::add_tracks`.
        tracks: Vec<TrackMeta>,
        /// Files/directories the scanner skipped in this window.
        failed: usize,
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
        /// Known files re-read because their size or mtime moved.
        updated: usize,
        /// Known files skipped whole: unchanged stamp, tags not re-read.
        unchanged: usize,
        /// Rows removed as confirmed gone.
        removed: usize,
        /// Total files/directories skipped.
        failed: usize,
        /// Wall-clock scan duration.
        elapsed: Duration,
    },
    /// The scan could not start (root missing / not a directory) or the
    /// worker thread could not be spawned.
    Error(String),
}

/// Groups scan entries into batches by count ([`BATCH_MAX_TRACKS`]) and time
/// ([`BATCH_INTERVAL`]). Pure state machine — the caller supplies `now`, so
/// the flush policy is unit-testable without sleeping.
#[derive(Debug)]
pub struct Batcher {
    tracks: Vec<TrackMeta>,
    failed: usize,
    last_flush: Instant,
    max_tracks: usize,
    interval: Duration,
}

impl Batcher {
    /// A batcher that starts its interval clock at `now`.
    #[must_use]
    pub fn new(max_tracks: usize, interval: Duration, now: Instant) -> Self {
        Self {
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
            tracks: std::mem::take(&mut self.tracks),
            failed: std::mem::take(&mut self.failed),
        })
    }
}

/// Start scanning `root` on a worker thread; the returned receiver yields
/// [`ScanUpdate`]s until a final `Done` (or `Error`). Dropping the receiver
/// stops the worker at its next send.
///
/// `known` is the index's own snapshot (`Library::known_files`). It is what
/// makes the scan incremental, and it is also the *only* list of rows the
/// removal pass will ever consider — a path the caller did not hand over
/// cannot be deleted by this worker under any circumstances.
pub fn spawn(root: PathBuf, known: KnownFiles) -> Receiver<ScanUpdate> {
    let (tx, rx) = channel();
    let worker_tx = tx.clone();
    let spawned = std::thread::Builder::new()
        .name("baz-scan".to_owned())
        .spawn(move || run_scan(&root, &known, &worker_tx));
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
    /// Whether the walk produced any entry at all under `root`.
    saw_anything: bool,
}

/// Worker body: drain the scanner through a [`Batcher`] into `tx`, then run
/// the removal pass.
fn run_scan(root: &Path, known: &KnownFiles, tx: &Sender<ScanUpdate>) {
    let started = Instant::now();
    let scan = match baz_core::library::scan_incremental(root, known) {
        Ok(scan) => scan,
        Err(err) => {
            // The root itself is missing or is not a directory — the single
            // most dangerous moment to prune, and we prune nothing.
            let _ = tx.send(ScanUpdate::Error(err.to_string()));
            return;
        }
    };
    let mut batcher = Batcher::new(BATCH_MAX_TRACKS, BATCH_INTERVAL, started);
    let mut walked = Walked::default();
    let mut counts = Counts::default();
    for entry in scan {
        walked.saw_anything = true;
        record(&entry, known, &mut walked, &mut counts);
        if let Some(update) = batcher.push(entry, Instant::now())
            && tx.send(update).is_err()
        {
            return; // UI hung up (window closed); stop scanning — and prune
            // nothing, because this walk never finished.
        }
    }
    if let Some(update) = batcher.flush(Instant::now())
        && tx.send(update).is_err()
    {
        return;
    }

    let gone = vanished(root, known, &walked);
    counts.removed = gone.len();
    if !gone.is_empty() && tx.send(ScanUpdate::Removed { paths: gone }).is_err() {
        return;
    }
    let _ = tx.send(ScanUpdate::Done {
        added: counts.added,
        updated: counts.updated,
        unchanged: counts.unchanged,
        removed: counts.removed,
        failed: counts.failed,
        elapsed: started.elapsed(),
    });
}

/// The running tallies `Done` reports.
#[derive(Default)]
struct Counts {
    added: usize,
    updated: usize,
    unchanged: usize,
    removed: usize,
    failed: usize,
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
/// 1. **The walk saw something.** A scan that produced no entry whatsoever
///    is not proof of an empty library — it is what an unmounted NAS or
///    unplugged drive looks like when the mount point survives as an empty
///    directory. Zero entries prunes zero rows.
/// 2. **The path is under the root just scanned.** The index may hold
///    several roots (the user re-pointed baz at a different folder, or a
///    stray import landed rows elsewhere). Scanning one root says nothing
///    about another, so rows outside it are untouchable here.
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
fn vanished(root: &Path, known: &KnownFiles, walked: &Walked<'_>) -> Vec<PathBuf> {
    if !walked.saw_anything {
        return Vec::new();
    }
    let mut gone: Vec<PathBuf> = known
        .keys()
        .filter(|path| !walked.seen.contains(path.as_path()))
        .filter(|path| path.starts_with(root))
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
    use std::time::SystemTime;

    use baz_core::library::FileStamp;
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

    /// The index snapshot for a set of files that are all currently on disk.
    fn known_of<'a>(paths: impl IntoIterator<Item = &'a Path>) -> KnownFiles {
        paths
            .into_iter()
            .map(|path| (path.to_path_buf(), FileStamp::of_path(path)))
            .collect()
    }

    /// Everything a worker run produced, flattened.
    #[derive(Default)]
    struct Run {
        batched: Vec<TrackMeta>,
        removed: Vec<PathBuf>,
        done: Option<(usize, usize, usize, usize, usize)>,
        error: Option<String>,
    }

    /// Run the worker to completion against `root` with `known` as the index.
    fn drive(root: &Path, known: KnownFiles) -> Run {
        let mut run = Run::default();
        for update in spawn(root.to_path_buf(), known) {
            match update {
                ScanUpdate::Batch { tracks, .. } => run.batched.extend(tracks),
                ScanUpdate::Removed { paths } => run.removed.extend(paths),
                ScanUpdate::Done {
                    added,
                    updated,
                    unchanged,
                    removed,
                    failed,
                    ..
                } => run.done = Some((added, updated, unchanged, removed, failed)),
                ScanUpdate::Error(err) => run.error = Some(err),
            }
        }
        run
    }

    #[test]
    fn flushes_when_count_cap_is_reached() {
        let now = Instant::now();
        let mut batcher = Batcher::new(3, Duration::from_secs(3600), now);
        assert_eq!(batcher.push(track(1), now), None);
        assert_eq!(batcher.push(track(2), now), None);
        let update = batcher.push(track(3), now).expect("cap flush");
        let ScanUpdate::Batch { tracks, failed } = update else {
            panic!("expected a batch");
        };
        assert_eq!(tracks.len(), 3);
        assert_eq!(failed, 0);
        // Buffer is drained; nothing left to flush.
        assert_eq!(batcher.flush(now), None);
    }

    #[test]
    fn flushes_when_interval_elapses_even_below_cap() {
        let now = Instant::now();
        let mut batcher = Batcher::new(1000, Duration::from_millis(150), now);
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
        let mut batcher = Batcher::new(1000, Duration::from_millis(150), now);
        assert_eq!(batcher.push(failed(), now), None);
        let later = now + Duration::from_millis(200);
        let update = batcher.push(failed(), later).expect("failed-only flush");
        assert_eq!(
            update,
            ScanUpdate::Batch {
                tracks: Vec::new(),
                failed: 2
            }
        );
    }

    #[test]
    fn empty_flush_is_none_but_restarts_clock() {
        let now = Instant::now();
        let mut batcher = Batcher::new(10, Duration::from_millis(150), now);
        assert_eq!(batcher.flush(now + Duration::from_secs(1)), None);
    }

    /// An unchanged file carries no metadata to write, so it must not cost
    /// the UI an `add_tracks` — the whole point of the warm path.
    #[test]
    fn unchanged_entries_never_enter_a_batch() {
        let now = Instant::now();
        let mut batcher = Batcher::new(1, Duration::from_secs(3600), now);
        let entry = ScanEntry::Unchanged {
            path: PathBuf::from("/m/1.flac"),
        };
        assert_eq!(batcher.push(entry, now), None);
        assert_eq!(batcher.flush(now), None, "nothing pending to flush");
    }

    #[test]
    fn worker_streams_batches_then_done() {
        let dir = tempfile::tempdir().expect("tempdir");
        wav(dir.path(), "a.wav");
        wav(dir.path(), "b.wav");
        fs::write(dir.path().join("broken.flac"), b"not flac").expect("write");

        let run = drive(dir.path(), KnownFiles::new());
        assert!(run.error.is_none());
        assert_eq!(run.batched.len(), 2);
        // Two added, nothing updated, nothing unchanged, nothing removed,
        // one file skipped.
        assert_eq!(run.done, Some((2, 0, 0, 0, 1)));
        assert!(
            run.removed.is_empty(),
            "an empty index has nothing to prune"
        );
    }

    #[test]
    fn missing_root_reports_error_not_panic() {
        let rx = spawn(
            PathBuf::from("/definitely/not/here/baz-test"),
            KnownFiles::new(),
        );
        let update = rx.recv().expect("one message");
        assert!(matches!(update, ScanUpdate::Error(_)));
    }

    /// A root that vanished under a library full of rows is the single most
    /// destructive moment for a "remove what you did not see" rule. It must
    /// remove nothing at all.
    #[test]
    fn a_scan_whose_root_is_missing_removes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = wav(dir.path(), "Artist/Album/01.wav");
        let known = known_of([a.as_path()]);
        let gone_root = dir.path().join("not-here");

        let run = drive(&gone_root, known);
        assert!(run.error.is_some(), "the scan could not start");
        assert!(run.removed.is_empty());
    }

    /// An unmounted share leaves its mount point behind as an empty
    /// directory. Scanning it finds nothing — which is not the same fact as
    /// "the library is empty".
    #[test]
    fn a_scan_that_saw_nothing_removes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = wav(dir.path(), "Artist/Album/01.wav");
        let known = known_of([a.as_path()]);
        // Now simulate the unmount: the whole tree disappears, the mount
        // point remains.
        fs::remove_dir_all(dir.path().join("Artist")).expect("unmount");

        let run = drive(dir.path(), known);
        assert_eq!(run.done, Some((0, 0, 0, 0, 0)));
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
        let known = known_of([keep.as_path(), doomed.as_path()]);
        fs::remove_file(&doomed).expect("delete");

        let run = drive(dir.path(), known);
        assert_eq!(run.removed, vec![doomed], "exactly the deleted file");
        assert_eq!(
            run.done,
            Some((0, 0, 1, 1, 0)),
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
        let mut known = known_of([present.as_path()]);
        known.insert(absent.clone(), None);

        let run = drive(dir.path(), known);
        assert!(
            run.removed.is_empty(),
            "a file whose directory is absent must survive: {:?}",
            run.removed
        );
        assert_eq!(run.done, Some((0, 0, 1, 0, 0)));
    }

    /// Re-pointing baz at another folder must not cost the first folder its
    /// rows: scanning one root is silence about every other.
    #[test]
    fn scanning_a_second_root_leaves_the_first_roots_rows_alone() {
        let first = tempfile::tempdir().expect("tempdir");
        let second = tempfile::tempdir().expect("tempdir");
        let old = wav(first.path(), "Artist/Album/01.wav");
        let new = wav(second.path(), "Other/Album/01.wav");
        let known = known_of([old.as_path(), new.as_path()]);
        // The first root is even deleted outright — still none of ours to
        // judge while scanning the second.
        fs::remove_dir_all(first.path()).expect("remove first root");

        let run = drive(second.path(), known);
        assert!(run.removed.is_empty(), "removed {:?}", run.removed);
        assert_eq!(run.done, Some((0, 0, 1, 0, 0)));
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
        // Both files are gone from disk; only one of them was looked for.
        let known: KnownFiles = HashMap::from([(reached.clone(), None), (unreached.clone(), None)]);
        let walked = Walked {
            seen: HashSet::new(),
            unreadable: vec![dir.path().join("Locked")],
            saw_anything: true,
        };

        let gone = vanished(dir.path(), &known, &walked);
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
        let known = known_of([quiet.as_path(), touched.as_path()]);

        // Rewrite `quiet` with bytes no parser would accept, keeping its
        // length and restoring its mtime. If the scan opened it, it would
        // come back as a failure; if it trusts the stamp, nothing happens.
        let stamp = known[&quiet].expect("a stamp for a freshly written file");
        let len = usize::try_from(stamp.size).expect("small file");
        fs::write(&quiet, vec![0xABu8; len]).expect("overwrite");
        fs::File::options()
            .write(true)
            .open(&quiet)
            .expect("reopen")
            .set_modified(stamp.modified())
            .expect("restore mtime");
        assert_eq!(
            FileStamp::of_path(&quiet),
            Some(stamp),
            "the fixture must be identical in size and mtime"
        );

        // `touched` keeps its contents but moves forward in time.
        fs::File::options()
            .write(true)
            .open(&touched)
            .expect("reopen")
            .set_modified(SystemTime::now() + Duration::from_secs(120))
            .expect("touch");

        let fresh = wav(dir.path(), "Artist/Album/03 New.wav");

        let run = drive(dir.path(), known);
        assert_eq!(
            run.done,
            Some((1, 1, 1, 0, 0)),
            "one added, one updated, one unchanged, nothing removed or failed"
        );
        let read: Vec<&PathBuf> = run.batched.iter().map(|meta| &meta.path).collect();
        assert!(read.contains(&&touched) && read.contains(&&fresh));
        assert!(
            !read.contains(&&quiet),
            "the unchanged file must not have been opened — its bytes are garbage"
        );
        // And every row baz writes carries the stamp the next scan compares.
        assert!(run.batched.iter().all(|meta| meta.stamp.is_some()));
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
