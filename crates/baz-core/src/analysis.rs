//! The ReplayGain analysis pass: measure the library, store what is measured.
//!
//! [`crate::replaygain`] reads the figures a file already carries and
//! [`crate::loudness`] is the meter. This module is the **service** that puts
//! them together over a whole library: it decides what needs measuring, decodes
//! it, and writes the answers into the index. The governing decision is
//! ADR-0015.
//!
//! # It is a separate service from the playback engine, deliberately
//!
//! [`crate::engine`] is given *paths* and owns no library — ADR-0013 §7, and
//! what makes a queue the library has never seen still play at the right level.
//! An analysis pass is the mirror image: it owns a library and decodes audio
//! **nobody hears**. Putting them in one service would have given the engine a
//! database and the analyser a sink, and neither has any business with the
//! other's.
//!
//! So the two speak different command vocabularies ([`AnalysisCommand`] here,
//! [`Command`](crate::protocol::Command) there) and the *same* event
//! vocabulary, because a front end has one event loop. A command addressed to
//! the wrong service does not compile, which is the only kind of routing
//! honesty worth having.
//!
//! # Analysis makes no sound
//!
//! It decodes; it does not play. There is no [`Sink`](crate::playback::Sink)
//! here, no ring buffer and no device: [`AudioSource`] blocks go from the
//! decoder into the meter and are dropped. A pass running while music plays is
//! two threads decoding different files, which is all it has ever been.
//!
//! # Where the work is committed, and what that buys
//!
//! The unit of work is the **album edition** (ADR-0007): one shelf tile in one
//! codec, which is the set an album gain is a property of. An edition's
//! measurements are written in one transaction, so the database never holds an
//! album whose tracks were measured against an album gain that was never
//! stored.
//!
//! That makes the pass **resumable at edition granularity**. A cancelled pass
//! keeps every track figure it had measured, and a later start re-plans against
//! what the index now holds: completed editions are skipped entirely, and the
//! edition that was interrupted is measured again — because an album figure
//! needs its tracks as a set, and the 400 ms blocks that would let one be
//! assembled from stored per-track summaries are not something to keep in a
//! database.
//!
//! # What is skipped
//!
//! - **A track whose file already carries the figure.** Tags win in the
//!   selection rule ([`ReplayGainSettings::resolve_with`](crate::replaygain::ReplayGainSettings::resolve_with)), so measuring one
//!   would spend a decode to produce a number nothing would use.
//! - **A track baz has already measured**, unless the file has changed since
//!   (the measurement carries the stamp it was taken at) or the caller asked
//!   for `redo`.
//! - **An edition where every track has everything it needs**, which after one
//!   completed pass is every edition — so running the pass again over an
//!   unchanged library costs a plan and nothing else.
//!
//! An edition that needs an **album** figure is measured **whole**, including
//! its already-tagged tracks: an album gain computed from the subset that
//! happened to be untagged would be a different number, and a wrong one.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

use crate::index::{IndexError, Library};
use crate::library::FileStamp;
use crate::loudness::{self, Loudness, LoudnessMeter};
use crate::playback::{AudioSource, CHANNELS, PlaybackError};
use crate::protocol::{AnalysisCommand, Event};
use crate::replaygain::{ComputedReplayGain, ReplayGainTags};

/// The analysis service could not be started.
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    /// The library database could not be opened. The worker holds its own
    /// connection (see [`spawn`]), so this is that open failing.
    #[error("the analysis worker could not open the library: {0}")]
    Index(#[from] IndexError),
    /// The worker thread could not be spawned.
    #[error("the analysis worker could not be started: {0}")]
    Io(#[from] std::io::Error),
}

/// The analysis service has shut down, so the command was not accepted.
#[derive(Debug, thiserror::Error)]
#[error("the ReplayGain analysis service has shut down")]
pub struct AnalysisClosed;

/// Everything a front end can read about an analysis pass without waiting for
/// an event — the pull-side twin of the `ReplayGainAnalysis*` events, on
/// exactly the terms [`EngineHandle::replay_gain`](crate::engine::EngineHandle::replay_gain)
/// sets out.
///
/// The fields are loaded independently, so a caller racing a change can mix a
/// count from before it with one from after. That is a status readout and the
/// events are the ordered account; a torn read corrects itself on the next one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct AnalysisProgress {
    /// Whether a pass is running now.
    pub running: bool,
    /// Tracks the running (or last) pass set out to measure.
    pub tracks: usize,
    /// Album editions those tracks belong to.
    pub editions: usize,
    /// Tracks finished with, including failures.
    pub analysed: usize,
    /// Tracks that could not be measured or stored.
    pub failed: usize,
    /// Whether the **last finished** pass stopped because it was cancelled.
    /// Meaningless while `running` is true, where it is the previous pass's
    /// answer.
    pub cancelled: bool,
}

/// The pass state shared between the worker thread (sole writer) and any
/// [`AnalysisHandle`] (reads it, from any thread).
///
/// Atomics only, for [`SharedReplayGain`](crate::replaygain)'s reason: a
/// status readout must never make a caller wait on the worker, and the worker
/// must never wait on a caller.
#[derive(Debug, Default)]
struct SharedAnalysis {
    running: AtomicBool,
    tracks: AtomicUsize,
    editions: AtomicUsize,
    analysed: AtomicUsize,
    failed: AtomicUsize,
    cancelled: AtomicBool,
}

impl SharedAnalysis {
    fn snapshot(&self) -> AnalysisProgress {
        AnalysisProgress {
            running: self.running.load(Ordering::Acquire),
            tracks: self.tracks.load(Ordering::Acquire),
            editions: self.editions.load(Ordering::Acquire),
            analysed: self.analysed.load(Ordering::Acquire),
            failed: self.failed.load(Ordering::Acquire),
            cancelled: self.cancelled.load(Ordering::Acquire),
        }
    }
}

/// A front end's connection to a running analysis service: send
/// [`AnalysisCommand`]s, read progress, shut down.
///
/// Dropping the handle shuts the service down cleanly — a running pass is
/// cancelled and the worker joined, so no thread outlives the handle. The drop
/// blocks until that happens, bounded by one decode block.
#[derive(Debug)]
pub struct AnalysisHandle {
    commands: Option<Sender<AnalysisCommand>>,
    thread: Option<JoinHandle<()>>,
    /// The out-of-band stop, so a cancel reaches a worker that is inside a
    /// decode loop rather than waiting at the channel. The command channel
    /// carries the *intent*; this carries it the last few milliseconds.
    cancel: Arc<AtomicBool>,
    shared: Arc<SharedAnalysis>,
}

impl AnalysisHandle {
    /// Send a command to the analysis service.
    ///
    /// # Errors
    ///
    /// [`AnalysisClosed`] if the worker is no longer running.
    pub fn send(&self, command: AnalysisCommand) -> Result<(), AnalysisClosed> {
        // A cancel has to overtake the queue: the worker reads commands
        // between decode blocks, and a Start sitting ahead of this one in the
        // channel must not delay it. The flag is cleared by the next accepted
        // pass, so an early cancel cannot poison a later start.
        if matches!(command, AnalysisCommand::CancelReplayGainAnalysis) {
            self.cancel.store(true, Ordering::Release);
        }
        self.commands
            .as_ref()
            .ok_or(AnalysisClosed)?
            .send(command)
            .map_err(|_| AnalysisClosed)
    }

    /// What the running (or last) pass has done so far.
    #[must_use]
    pub fn progress(&self) -> AnalysisProgress {
        self.shared.snapshot()
    }

    /// Shut the service down and wait for its thread to finish. Equivalent to
    /// dropping the handle; provided so intent reads explicitly.
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for AnalysisHandle {
    fn drop(&mut self) {
        // Stop first, then close the channel: a worker inside a decode loop
        // learns from the flag, and one waiting at the channel learns from the
        // disconnect.
        self.cancel.store(true, Ordering::Release);
        self.commands = None;
        if let Some(handle) = self.thread.take() {
            // A panicked worker is a bug (docs/ENGINEERING.md); all drop can do
            // is not propagate it into the caller's unwind.
            let _ = handle.join();
        }
    }
}

/// Start the ReplayGain analysis service over the library database at
/// `db_path`.
///
/// Returns the control handle and the event receiver — **single consumer**, on
/// exactly the terms [`crate::engine`]'s is. Nothing is measured until an
/// [`AnalysisCommand::StartReplayGainAnalysis`] arrives.
///
/// # A second connection to the same database
///
/// The worker opens its own [`Library`]. That is what makes the pass
/// self-contained — it decides what needs doing and it is the thing that
/// persists the answers, which is what "resumable" means — and SQLite in WAL
/// mode (which [`Library::open`] selects) is built for exactly this shape: one
/// writer, concurrent readers, no reader ever blocked by the writer. The two
/// connections also cannot collide over a *value*: a scan writes the tag
/// columns and never the computed ones, and this pass writes the computed
/// columns and never the tags (schema v6).
///
/// The price is a second in-RAM index, and it is not waste: the album grouping
/// that index computes ([`Library::albums`]) **is** the plan, because an album
/// gain is a property of an edition.
///
/// A front end's own [`Library`] does not learn what a pass stored until it
/// [`reload`](Library::reload)s — which it should do when
/// [`Event::ReplayGainAnalysisFinished`] arrives, together with handing the
/// fresh [`computed_gains`](Library::computed_gains) snapshot to
/// [`EngineHandle::set_computed_gains`](crate::engine::EngineHandle::set_computed_gains).
///
/// # Errors
///
/// [`AnalysisError::Index`] if the database cannot be opened or migrated, and
/// [`AnalysisError::Io`] if the worker thread cannot be spawned. Both are
/// answered before this returns, so a handle that exists is a service that
/// started.
pub fn spawn(
    db_path: impl AsRef<Path>,
) -> Result<(AnalysisHandle, Receiver<Event>), AnalysisError> {
    let db_path = db_path.as_ref().to_path_buf();
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    let (ack_tx, ack_rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&cancel);
    let shared = Arc::new(SharedAnalysis::default());
    let published = Arc::clone(&shared);
    let thread = thread::Builder::new()
        .name("baz-replaygain".into())
        .spawn(move || match Library::open(&db_path) {
            Ok(library) => {
                if ack_tx.send(Ok(())).is_err() {
                    return; // the caller gave up before we finished opening
                }
                Worker {
                    library,
                    commands: cmd_rx,
                    events: event_tx,
                    cancel: stop,
                    shared: published,
                }
                .run();
            }
            Err(error) => {
                let _ = ack_tx.send(Err(error));
            }
        })?;
    let handle = AnalysisHandle {
        commands: Some(cmd_tx),
        thread: Some(thread),
        cancel,
        shared,
    };
    match ack_rx.recv() {
        Ok(Ok(())) => Ok((handle, event_rx)),
        // Dropping `handle` joins the (finished) thread.
        Ok(Err(error)) => Err(AnalysisError::Index(error)),
        Err(_) => Err(AnalysisError::Index(IndexError::CorruptStoredPath)),
    }
}

/// One album edition's worth of work: which files to measure, and whether the
/// edition needs an album figure computed across them.
#[derive(Debug, PartialEq, Eq)]
struct EditionJob {
    /// The files to decode, with the stamp the index holds for each — which is
    /// what a measurement is recorded against, so that a later scan noticing
    /// the file moved is what makes the measurement stale.
    measure: Vec<(PathBuf, Option<FileStamp>)>,
    /// Whether to compute an album figure across `measure`. When true,
    /// `measure` is the edition's **whole** track list.
    album: bool,
}

/// The analysis worker: owns the library, the command channel and the pass.
struct Worker {
    library: Library,
    commands: Receiver<AnalysisCommand>,
    events: Sender<Event>,
    cancel: Arc<AtomicBool>,
    shared: Arc<SharedAnalysis>,
}

impl Worker {
    /// The service loop: wait for a command, act on it, wait again.
    fn run(mut self) {
        while let Ok(command) = self.commands.recv() {
            match command {
                AnalysisCommand::StartReplayGainAnalysis { redo } => self.pass(redo),
                // Nothing is running, so there is nothing to stop — and a
                // redundant command emits nothing, like every other command in
                // this protocol. Clearing the flag here is what keeps a cancel
                // sent while idle from stopping the *next* pass.
                AnalysisCommand::CancelReplayGainAnalysis => {
                    self.cancel.store(false, Ordering::Release);
                }
            }
        }
    }

    /// One whole pass over the library.
    fn pass(&mut self, redo: bool) {
        // A cancel that arrived while idle is not a cancel of this pass.
        self.cancel.store(false, Ordering::Release);
        // Plan against what the database holds *now*, not against what it held
        // when this worker started: the scanner has its own connection and has
        // very likely written since.
        let reloaded = self.library.reload();
        let jobs = plan(&self.library, redo);
        let tracks: usize = jobs.iter().map(|job| job.measure.len()).sum();

        // State before event, every time (the ordering contract `Control::
        // settle_replay_gain` states): a front end that sees `Started` and then
        // reads `progress()` must not be told the pass has not begun.
        self.shared.running.store(true, Ordering::Release);
        self.shared.cancelled.store(false, Ordering::Release);
        self.shared.tracks.store(tracks, Ordering::Release);
        self.shared.editions.store(jobs.len(), Ordering::Release);
        self.shared.analysed.store(0, Ordering::Release);
        self.shared.failed.store(0, Ordering::Release);
        let _ = self.events.send(Event::ReplayGainAnalysisStarted {
            tracks,
            editions: jobs.len(),
        });

        let mut done = Tally {
            analysed: 0,
            // A library that could not be re-read is a plan made from a stale
            // snapshot. Counting it once as a failure is the honest signal that
            // the pass knows less than it should; it is not a reason to refuse
            // to measure what it does know about.
            failed: usize::from(reloaded.is_err()),
        };
        let cancelled = jobs.iter().any(|job| !self.run_job(job, &mut done));

        self.shared.running.store(false, Ordering::Release);
        self.shared.cancelled.store(cancelled, Ordering::Release);
        let _ = self.events.send(Event::ReplayGainAnalysisFinished {
            analysed: done.analysed,
            failed: done.failed,
            cancelled,
        });
    }

    /// Measure one edition and store what it measured. Returns whether the
    /// edition ran to its end (`false` means cancelled).
    fn run_job(&mut self, job: &EditionJob, done: &mut Tally) -> bool {
        let mut measured: Vec<(&PathBuf, Option<FileStamp>, Loudness)> = Vec::new();
        let mut completed = true;
        for (path, stamp) in &job.measure {
            if self.cancelled() {
                completed = false;
                break;
            }
            match measure_track(path, &self.cancel) {
                Ok(Some(loudness)) => measured.push((path, *stamp, loudness)),
                // Cancelled part-way through this file: it has no measurement
                // and must not be counted as one.
                Ok(None) => {
                    completed = false;
                    break;
                }
                Err(_) => done.failed += 1,
            }
            done.analysed += 1;
            self.shared.analysed.store(done.analysed, Ordering::Release);
            self.shared.failed.store(done.failed, Ordering::Release);
            let _ = self.events.send(Event::ReplayGainAnalysisProgress {
                path: path.clone(),
                analysed: done.analysed,
                tracks: self.shared.tracks.load(Ordering::Acquire),
                failed: done.failed,
            });
        }
        // An album figure only when the edition ran whole: a gain computed
        // across the tracks a cancel happened to reach would be a number about
        // an album that does not exist. The track figures already measured are
        // stored either way — a cancel costs the work it interrupted and not
        // the work it did.
        let album = (job.album && completed)
            .then(|| {
                let readings = measured.iter().map(|(_, _, loudness)| loudness);
                let lufs = loudness::album_lufs(readings.clone())?;
                let peak = loudness::album_sample_peak(readings)?;
                Some((loudness::gain_centidb(lufs), loudness::peak_micro(peak)))
            })
            .flatten();
        let batch: Vec<(PathBuf, ComputedReplayGain)> = measured
            .iter()
            .map(|(path, stamp, reading)| {
                let figures = ReplayGainTags {
                    track_gain_centidb: reading.integrated_lufs().map(loudness::gain_centidb),
                    track_peak_micro: Some(loudness::peak_micro(reading.sample_peak())),
                    album_gain_centidb: album.map(|(gain, _)| gain),
                    album_peak_micro: album.map(|(_, peak)| peak),
                };
                (
                    (*path).clone(),
                    ComputedReplayGain {
                        figures,
                        stamp: *stamp,
                    },
                )
            })
            .collect();
        if !batch.is_empty() && self.library.store_computed_replay_gain(batch).is_err() {
            // The measurements were taken and could not be kept, which is a
            // failure of exactly as many tracks as were in the batch.
            done.failed += measured.len();
            self.shared.failed.store(done.failed, Ordering::Release);
        }
        completed
    }

    /// Whether the pass should stop: the out-of-band flag, or a cancel waiting
    /// in the command channel.
    ///
    /// Both, because a front end may reasonably use either — and because a
    /// [`AnalysisCommand::StartReplayGainAnalysis`] that arrives mid-pass has
    /// to be drained rather than left to start a second pass the moment this
    /// one ends.
    fn cancelled(&self) -> bool {
        loop {
            match self.commands.try_recv() {
                Ok(AnalysisCommand::CancelReplayGainAnalysis) => {
                    self.cancel.store(true, Ordering::Release);
                }
                // A pass is already running; two over one library would fight
                // over the same rows. Ignored, and it emits nothing.
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                // The handle was dropped: shut down, which is a cancel.
                Err(TryRecvError::Disconnected) => return true,
            }
        }
        self.cancel.load(Ordering::Acquire)
    }
}

/// Running counts for one pass.
struct Tally {
    analysed: usize,
    failed: usize,
}

/// What a pass has to do, in a deterministic order: album by album in library
/// order, edition by edition in ranked order (ADR-0007).
///
/// Determinism is not decoration. It is what makes a cancelled pass resume
/// where it stopped rather than somewhere else, and what lets a test assert
/// that the second pass does strictly less work than the first.
fn plan(library: &Library, redo: bool) -> Vec<EditionJob> {
    let mut jobs = Vec::new();
    for album in library.albums() {
        for edition in &album.editions {
            // An album figure is a property of the edition as a set, so the
            // question is asked of the edition and not of a track: if any track
            // still needs one, every track has to be measured.
            let needs_album = edition.tracks.iter().any(|meta| {
                meta.replay_gain.album_gain_centidb.is_none()
                    && (redo
                        || library
                            .computed_replay_gain(&meta.path)
                            .album_gain_centidb
                            .is_none())
            });
            let wanted = |meta: &crate::library::TrackMeta| {
                needs_album
                    || (meta.replay_gain.track_gain_centidb.is_none()
                        && (redo
                            || library
                                .computed_replay_gain(&meta.path)
                                .track_gain_centidb
                                .is_none()))
            };
            let measure: Vec<(PathBuf, Option<FileStamp>)> = edition
                .tracks
                .iter()
                .filter(|meta| wanted(meta))
                .map(|meta| (meta.path.clone(), meta.stamp))
                .collect();
            if !measure.is_empty() {
                jobs.push(EditionJob {
                    measure,
                    album: needs_album,
                });
            }
        }
    }
    jobs
}

/// Decode one file and measure it.
///
/// `Ok(None)` means the pass was cancelled part-way through the file, which is
/// not a failure and not a measurement — the decode is simply abandoned. The
/// flag is checked once per decoded block (a few milliseconds of audio), which
/// is what bounds how long a cancel takes to land.
///
/// The meter is built with the **source's** channel count, not the stereo the
/// decoder emits: see [`AudioSource::channels`] for the 3.01 LU that depends
/// on it.
fn measure_track(path: &Path, cancel: &AtomicBool) -> Result<Option<Loudness>, PlaybackError> {
    let mut source = AudioSource::open(path)?;
    let channels = source.channels().clamp(1, CHANNELS);
    let Some(mut meter) = LoudnessMeter::new(source.sample_rate(), channels) else {
        // A stream that declares no sample rate cannot be measured; the
        // decoder would have refused it too.
        return Err(PlaybackError::UnknownSampleRate);
    };
    // Reused across blocks so a five-minute track costs one allocation. Only
    // needed for a mono source, where the meter must see one channel of the
    // duplicated pair the decoder produces.
    let mut mono: Vec<f32> = Vec::new();
    while let Some(block) = source.next_block()? {
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        if channels == CHANNELS {
            meter.push(block);
        } else {
            mono.clear();
            mono.extend(block.iter().step_by(CHANNELS));
            meter.push(&mono);
        }
    }
    Ok(Some(meter.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cancel sent while nothing is running must not stop the pass that
    /// comes after it — the flag is state, and stale state is how a feature
    /// comes to refuse to work for reasons nobody can see.
    #[test]
    fn a_cancel_while_idle_does_not_poison_the_next_pass() {
        let cancel = Arc::new(AtomicBool::new(true));
        // What `Worker::pass` does first.
        cancel.store(false, Ordering::Release);
        assert!(!cancel.load(Ordering::Acquire));
    }

    /// The shared readout starts at "nothing has happened", which is what a
    /// front end reading it before the first pass must be told.
    #[test]
    fn the_shared_progress_starts_empty() {
        assert_eq!(
            SharedAnalysis::default().snapshot(),
            AnalysisProgress::default()
        );
    }
}
