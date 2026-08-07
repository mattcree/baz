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
//! Known v0.1 limitation: the pipeline is upsert-only (a rescan updates and
//! adds), so files deleted from disk linger in the index until `baz-core`
//! grows a removal API — tracked for the next library iteration.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use baz_core::library::{ScanEntry, TrackMeta};

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
    /// The scan finished; totals for the throughput log and status line.
    Done {
        /// Total tracks read.
        tracks: usize,
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
pub fn spawn(root: std::path::PathBuf) -> Receiver<ScanUpdate> {
    let (tx, rx) = channel();
    let worker_tx = tx.clone();
    let spawned = std::thread::Builder::new()
        .name("baz-scan".to_owned())
        .spawn(move || run_scan(&root, &worker_tx));
    if let Err(err) = spawned {
        // Channel still open (rx just created); ignore-send is unreachable.
        let _ = tx.send(ScanUpdate::Error(format!("could not start scan: {err}")));
    }
    rx
}

/// Worker body: drain the scanner through a [`Batcher`] into `tx`.
fn run_scan(root: &std::path::Path, tx: &Sender<ScanUpdate>) {
    let started = Instant::now();
    let scan = match baz_core::library::scan(root) {
        Ok(scan) => scan,
        Err(err) => {
            let _ = tx.send(ScanUpdate::Error(err.to_string()));
            return;
        }
    };
    let mut batcher = Batcher::new(BATCH_MAX_TRACKS, BATCH_INTERVAL, started);
    let mut total_tracks = 0;
    let mut total_failed = 0;
    for entry in scan {
        if let Some(update) = batcher.push(entry, Instant::now())
            && send_counted(tx, update, &mut total_tracks, &mut total_failed).is_err()
        {
            return; // UI hung up (window closed); stop scanning.
        }
    }
    if let Some(update) = batcher.flush(Instant::now())
        && send_counted(tx, update, &mut total_tracks, &mut total_failed).is_err()
    {
        return;
    }
    let _ = tx.send(ScanUpdate::Done {
        tracks: total_tracks,
        failed: total_failed,
        elapsed: started.elapsed(),
    });
}

/// Send a batch, accumulating its counts into the running totals.
fn send_counted(
    tx: &Sender<ScanUpdate>,
    update: ScanUpdate,
    total_tracks: &mut usize,
    total_failed: &mut usize,
) -> Result<(), std::sync::mpsc::SendError<ScanUpdate>> {
    if let ScanUpdate::Batch { tracks, failed } = &update {
        *total_tracks += tracks.len();
        *total_failed += failed;
    }
    tx.send(update)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn track(n: u32) -> ScanEntry {
        ScanEntry::Track(TrackMeta {
            path: PathBuf::from(format!("/m/{n}.flac")),
            artist: None,
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
        })
    }

    fn failed() -> ScanEntry {
        ScanEntry::Failed {
            path: PathBuf::from("/m/bad"),
            reason: "nope".to_owned(),
        }
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

    #[test]
    fn worker_streams_batches_then_done() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Two real (empty-but-valid) WAVs and one corrupt file.
        for name in ["a.wav", "b.wav"] {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 8_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::create(dir.path().join(name), spec).expect("wav");
            writer.write_sample(0i16).expect("sample");
            writer.finalize().expect("finalize");
        }
        std::fs::write(dir.path().join("broken.flac"), b"not flac").expect("write");

        let rx = spawn(dir.path().to_path_buf());
        let mut tracks = 0;
        let mut failed = 0;
        let mut done = None;
        for update in rx {
            match update {
                ScanUpdate::Batch {
                    tracks: t,
                    failed: f,
                } => {
                    tracks += t.len();
                    failed += f;
                }
                ScanUpdate::Done {
                    tracks: t,
                    failed: f,
                    ..
                } => done = Some((t, f)),
                ScanUpdate::Error(err) => panic!("unexpected scan error: {err}"),
            }
        }
        assert_eq!(done, Some((2, 1)), "totals in Done match the stream");
        assert_eq!((tracks, failed), (2, 1));
    }

    #[test]
    fn missing_root_reports_error_not_panic() {
        let rx = spawn(PathBuf::from("/definitely/not/here/baz-test"));
        let update = rx.recv().expect("one message");
        assert!(matches!(update, ScanUpdate::Error(_)));
    }
}
