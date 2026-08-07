//! The playback state machine: engine [`Event`]s in, render-ready state out.
//!
//! # The honesty rule
//!
//! UI playback state derives **only** from events the engine actually
//! emitted — never from optimistic assumption after sending a command. The
//! engine is the source of truth (commands can fail on a closed engine, and
//! redundant commands emit nothing), so [`PlayerState::apply`] is the sole
//! place phase, current track, and failure counts change. What the app *is*
//! allowed to remember about its own requests is exactly two things, both
//! request-side knowledge rather than playback state:
//!
//! - **Queue size** ([`PlayerState::note_queue_sent`]): the engine echoes no
//!   event for [`SetQueue`](baz_core::protocol::Command::SetQueue), and
//!   "did we ever hand the engine a queue" is what decides whether a Play
//!   button can do anything at all.
//! - **A pending transport command** ([`PlayerState::note_transport_sent`]):
//!   the documented "brief pending affordance". While a Play/Pause/Next is
//!   in flight the toggle shows `…` and both transport buttons disable —
//!   which also debounces double-presses. Pending clears on the *next event
//!   of any kind* (any event proves the engine processed past our command;
//!   clearing on any rather than the matching one means a command that
//!   raced into a no-op — pause just as the queue ended, say — cannot wedge
//!   the button forever). It never sets a phase.
//!
//! # The seek bar
//!
//! Position comes from [`Event::Progress`] and nothing else — the same
//! honesty rule. Two pieces of request-side state sit on top of it, both
//! purely about what the *pointer* is doing:
//!
//! - **A drag in progress** ([`PlayerState::drag_to`]): while the handle is
//!   held, the bar shows where the pointer is and incoming `Progress` is
//!   recorded but not displayed. Anything else would fight the user's hand.
//! - **A seek awaiting confirmation** ([`PlayerState::release_drag`]): on
//!   release the bar shows the requested position, marked pending, until an
//!   event confirms. This is the transport buttons' affordance applied to
//!   the bar, and it clears the same way — on the next event of any kind,
//!   for the same anti-wedging reason. In practice that event is the
//!   `Progress` the engine emits immediately on accepting a `Seek`; a
//!   cadence report that was already in flight can clear it one report
//!   early, which costs at most a quarter-second of showing the position the
//!   seek came from. That is the honest trade against a pending state that
//!   could stick forever, and it is the trade the transport buttons already
//!   make.
//!
//! Engine availability ([`Availability`]) is seeded from the spawn result at
//! startup — that is a returned fact, not an assumption — and downgrades to
//! [`Availability::Closed`] when the event bridge reports the engine gone or
//! a send fails.
//!
//! Everything here is pure and iced-free, so the whole machine is unit
//! tested on the host without a window, an audio device, or the
//! `device-output` feature.

use std::path::{Path, PathBuf};
use std::time::Duration;

use baz_core::protocol::Event;

use crate::vm::{self, AlbumVm};

/// Whether a playback engine exists to talk to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// Compiled without the `device-output` feature: playback UI is hidden
    /// entirely (see `src/playback.rs`).
    NotBuilt,
    /// The engine could not open an output device at startup; the shelf
    /// works, playback controls show this state. The string is the
    /// user-presentable reason.
    #[cfg_attr(
        all(not(feature = "device-output"), not(test)),
        expect(
            dead_code,
            reason = "only the device build (and the tests) construct this; the \
                      no-audio build still matches on it so the machine stays whole"
        )
    )]
    NoDevice(String),
    /// The engine is running and accepting commands.
    Ready,
    /// The engine was running but has shut down (bridge disconnect or a
    /// failed send).
    Closed,
}

/// Transport phase, exactly as confirmed by engine events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No session: nothing started yet, stopped, or the queue ended.
    Stopped,
    /// Audio is reaching the sink ([`Event::TrackStarted`] /
    /// [`Event::Resumed`]).
    Playing,
    /// Paused mid-session ([`Event::Paused`]).
    Paused,
}

/// The bottom bar's resolved current track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlaying {
    /// Shelf album containing the track, when the path resolves against the
    /// library — drives the playing-album highlight.
    pub album_id: Option<u64>,
    /// Display title (track title, else the file name).
    pub title: String,
    /// Album artist, when resolved.
    pub artist: Option<String>,
}

/// Resolve a queue path against the shelf's view model: the engine
/// addresses tracks by path ([`Event::TrackStarted`] carries one), the UI
/// answers with title/artist/album. A path the library does not know (a
/// file deleted mid-queue, say) falls back to its file name with no album
/// highlight — playback truth outranks library staleness.
///
/// Every edition of every album is searched ([`AlbumVm::all_tracks`]): what
/// is playing keeps its name even after the user switches the panel to a
/// different format of the same album.
#[must_use]
pub fn resolve_now_playing(albums: &[AlbumVm], path: &Path) -> NowPlaying {
    for album in albums {
        for track in album.all_tracks() {
            if track.path == path {
                return NowPlaying {
                    album_id: Some(album.id),
                    title: track.title.clone(),
                    artist: album.artist.name().map(str::to_owned),
                };
            }
        }
    }
    NowPlaying {
        album_id: None,
        title: path
            .file_name()
            .map_or_else(|| String::from("?"), |n| n.to_string_lossy().into_owned()),
        artist: None,
    }
}

/// The seek bar's render-ready state — everything the view needs to compose
/// a slider and two timestamps, and nothing about how to draw them.
#[derive(Debug, Clone, PartialEq)]
pub struct SeekBar {
    /// Handle position as a fraction of the track, `0.0..=1.0`. Always 0
    /// when the track length is unknown (there is no proportion to show).
    pub position: f32,
    /// Left timestamp: the position being shown — dragged, pending, or
    /// confirmed, in that order of precedence.
    pub elapsed: String,
    /// Right timestamp: the track's length, or `--:--` when undeclared.
    pub total: String,
    /// Whether dragging the bar can do anything. False when the engine is
    /// unavailable, nothing is playing, or the track declares no length —
    /// there is no honest position to seek *to* without one.
    pub interactive: bool,
    /// Whether the position shown is a *request* rather than a confirmed
    /// reading: the handle is being dragged, or a seek is awaiting its
    /// confirming event. The view marks it so the number is never mistaken
    /// for playback truth it has not earned yet.
    pub pending: bool,
}

/// The placeholder shown where a track length would be, when the container
/// never declared one. Same width as a real `m:ss` so the bar does not jump.
const UNKNOWN_TOTAL: &str = "--:--";

/// The event-derived playback state behind every playback widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    availability: Availability,
    phase: Phase,
    now_playing: Option<NowPlaying>,
    /// Path of the current track, to tell "the same track started again
    /// after a seek" from "a different track started".
    now_playing_path: Option<PathBuf>,
    /// Tracks in the queue we last asked for (request-side; module docs).
    queued: usize,
    /// A transport command awaiting its confirming event (module docs).
    pending: bool,
    /// [`Event::TrackFailed`] count since the last queue request.
    failed: usize,
    /// Confirmed position in the current track, from [`Event::Progress`].
    elapsed_ms: u64,
    /// Confirmed track length, when the engine reported one.
    track_ms: Option<u64>,
    /// Position under the pointer while the bar is being dragged.
    drag_ms: Option<u64>,
    /// Position a sent-but-unconfirmed [`Command::Seek`](baz_core::protocol::Command::Seek)
    /// asked for.
    seek_pending: Option<u64>,
}

impl PlayerState {
    /// Fresh state; `availability` is the engine spawn result.
    #[must_use]
    pub fn new(availability: Availability) -> Self {
        Self {
            availability,
            phase: Phase::Stopped,
            now_playing: None,
            now_playing_path: None,
            queued: 0,
            pending: false,
            failed: 0,
            elapsed_ms: 0,
            track_ms: None,
            drag_ms: None,
            seek_pending: None,
        }
    }

    /// Fold one engine event into the state. `albums` is the current shelf
    /// view model, used to resolve [`Event::TrackStarted`] paths.
    pub fn apply(&mut self, event: &Event, albums: &[AlbumVm]) {
        match event {
            Event::TrackStarted { path, .. } => {
                self.phase = Phase::Playing;
                // A seek restarts the *current* track, so TrackStarted is not
                // by itself news of a new track. Only a genuinely different
                // path resets the position; otherwise the bar would snap to
                // zero for the moment between a seek's confirming Progress
                // and the restarted track's audio arriving.
                if self.now_playing_path.as_deref() != Some(path.as_path()) {
                    self.now_playing_path = Some(path.clone());
                    self.reset_progress();
                }
                self.now_playing = Some(resolve_now_playing(albums, path));
            }
            Event::Paused => self.phase = Phase::Paused,
            Event::Resumed => self.phase = Phase::Playing,
            Event::Stopped | Event::QueueEnded => {
                self.phase = Phase::Stopped;
                self.now_playing = None;
                self.now_playing_path = None;
                self.reset_progress();
            }
            Event::TrackFailed { .. } => self.failed += 1,
            Event::Progress {
                elapsed_ms,
                track_ms,
            } => {
                self.elapsed_ms = *elapsed_ms;
                self.track_ms = *track_ms;
            }
            // `Event` is #[non_exhaustive]: tolerate unknown messages.
            _ => {}
        }
        // Any received event proves the engine made progress past whatever
        // we last sent (module docs). A drag is the pointer's business, not
        // the engine's, so it survives.
        self.pending = false;
        self.seek_pending = None;
    }

    /// Forget where we were: a different track, or none at all.
    fn reset_progress(&mut self) {
        self.elapsed_ms = 0;
        self.track_ms = None;
        self.drag_ms = None;
        self.seek_pending = None;
    }

    /// Record that a new queue of `len` tracks was requested.
    pub fn note_queue_sent(&mut self, len: usize) {
        self.queued = len;
        self.failed = 0;
    }

    /// Record that a transport command (Play/Pause/Next) was accepted by
    /// the engine's channel and awaits its confirming event.
    pub fn note_transport_sent(&mut self) {
        self.pending = true;
    }

    /// The engine is gone: the bridge reported disconnect, or a send
    /// failed. Only a running engine can close — startup states are kept so
    /// their (more useful) message stays on screen.
    pub fn engine_closed(&mut self) {
        if self.availability == Availability::Ready {
            self.availability = Availability::Closed;
        }
        self.phase = Phase::Stopped;
        self.now_playing = None;
        self.now_playing_path = None;
        self.pending = false;
        self.reset_progress();
    }

    /// The pointer moved to `fraction` (`0.0..=1.0`) of the track with the
    /// bar held. Records the drag position; the bar shows it in place of the
    /// engine's reports until [`Self::release_drag`].
    ///
    /// A no-op when [`Self::seek_bar`] reports the bar non-interactive —
    /// there is nothing a fraction could mean without a track length.
    pub fn drag_to(&mut self, fraction: f32) {
        let Some(total) = self.seekable_total() else {
            return;
        };
        self.drag_ms = Some(scale(total, fraction));
    }

    /// The bar was released. Returns the position to ask the engine for, and
    /// records it as pending so the bar keeps showing it until an event
    /// confirms. `None` when no drag was in progress.
    pub fn release_drag(&mut self) -> Option<u64> {
        let target = self.drag_ms.take()?;
        self.seek_pending = Some(target);
        Some(target)
    }

    /// The track length to scrub against, or `None` when scrubbing would be
    /// a lie: no engine, nothing playing, or a track of undeclared length.
    fn seekable_total(&self) -> Option<u64> {
        if !self.engine_ready() || self.phase == Phase::Stopped {
            return None;
        }
        self.track_ms.filter(|&total| total > 0)
    }

    /// The seek bar's render-ready state, or `None` when there is no track
    /// to report on at all (nothing playing, or no engine to play it) — in
    /// which case the view omits the bar rather than drawing an empty one.
    ///
    /// The position shown is the drag under the pointer if there is one,
    /// else the seek awaiting confirmation, else what the engine last
    /// reported.
    #[must_use]
    pub fn seek_bar(&self) -> Option<SeekBar> {
        if !self.engine_ready() || self.now_playing.is_none() {
            return None;
        }
        let shown = self
            .drag_ms
            .or(self.seek_pending)
            .unwrap_or(self.elapsed_ms);
        let total = self.seekable_total();
        Some(SeekBar {
            position: total.map_or(0.0, |total| fraction(shown, total)),
            elapsed: format_ms(total.map_or(shown, |total| shown.min(total))),
            total: total.map_or_else(|| UNKNOWN_TOTAL.to_owned(), format_ms),
            interactive: total.is_some(),
            pending: self.dragging() || self.seek_pending(),
        })
    }

    /// Whether the bar is currently being dragged — the view uses this to
    /// keep the handle lit, and the tests to pin the state machine.
    #[must_use]
    pub fn dragging(&self) -> bool {
        self.drag_ms.is_some()
    }

    /// Whether a seek has been sent and not yet confirmed.
    #[must_use]
    pub fn seek_pending(&self) -> bool {
        self.seek_pending.is_some()
    }

    /// Current engine availability.
    #[must_use]
    pub fn availability(&self) -> &Availability {
        &self.availability
    }

    /// Whether the engine is running and worth sending commands to.
    #[must_use]
    pub fn engine_ready(&self) -> bool {
        self.availability == Availability::Ready
    }

    /// Confirmed transport phase.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The resolved current track, while one is playing or paused.
    #[must_use]
    pub fn now_playing(&self) -> Option<&NowPlaying> {
        self.now_playing.as_ref()
    }

    /// Album to highlight on the shelf: the current track's, when resolved.
    #[must_use]
    pub fn playing_album(&self) -> Option<u64> {
        self.now_playing.as_ref().and_then(|now| now.album_id)
    }

    /// Play/pause toggle label: the action a press would request, from the
    /// *confirmed* phase — or `…` while a command is pending confirmation.
    #[must_use]
    pub fn play_pause_label(&self) -> &'static str {
        if !self.engine_ready() {
            return "Play";
        }
        if self.pending {
            return "…";
        }
        match self.phase {
            Phase::Playing => "Pause",
            Phase::Paused | Phase::Stopped => "Play",
        }
    }

    /// Whether the play/pause toggle does anything: engine running, nothing
    /// pending, and a queue to (re)start when stopped.
    #[must_use]
    pub fn play_pause_enabled(&self) -> bool {
        self.engine_ready() && !self.pending && (self.queued > 0 || self.phase != Phase::Stopped)
    }

    /// Whether Next does anything (it is a documented engine no-op while
    /// stopped).
    #[must_use]
    pub fn next_enabled(&self) -> bool {
        self.engine_ready() && !self.pending && self.phase != Phase::Stopped
    }

    /// The bar's replacement line when there is no engine to report on;
    /// `None` while one is running (or when the bar is hidden entirely).
    #[must_use]
    pub fn availability_note(&self) -> Option<String> {
        match &self.availability {
            Availability::NotBuilt | Availability::Ready => None,
            Availability::NoDevice(reason) => Some(format!("no audio device — {reason}")),
            Availability::Closed => Some("audio engine stopped".to_owned()),
        }
    }

    /// Unobtrusive skip note: `N track(s) skipped` once any track in the
    /// current queue failed (the engine already continued past it).
    #[must_use]
    pub fn skipped_note(&self) -> Option<String> {
        match self.failed {
            0 => None,
            1 => Some("1 track skipped".to_owned()),
            n => Some(format!("{n} tracks skipped")),
        }
    }
}

/// A position in `0..=total` from a `0.0..=1.0` fraction, clamped at both
/// ends so a pointer dragged off the widget cannot produce a nonsense
/// target.
fn scale(total: u64, fraction: f32) -> u64 {
    let fraction = f64::from(fraction.clamp(0.0, 1.0));
    // Track lengths are far below f64's exact-integer range, and the product
    // is bounded by `total` by construction.
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "0 <= fraction*total <= total, and total is far below 2^52"
    )]
    let scaled = (total as f64 * fraction).round() as u64;
    scaled.min(total)
}

/// The inverse of [`scale`]: where `position` sits in `0..=total`.
fn fraction(position: u64, total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "track lengths are far below f32's useful range for a 0..1 ratio"
    )]
    let ratio = position as f32 / total as f32;
    ratio.clamp(0.0, 1.0)
}

/// Milliseconds as the shelf's `m:ss` / `h:mm:ss` timestamp.
fn format_ms(ms: u64) -> String {
    vm::format_duration(Duration::from_millis(ms))
}

#[cfg(test)]
mod tests {
    use baz_core::library::AudioFormat;

    use crate::vm::{AlbumArtistVm, EditionKey, EditionVm, TrackVm};

    use super::*;

    fn track(path: &str, title: &str, number: u32) -> TrackVm {
        TrackVm {
            number: Some(number),
            title: title.to_owned(),
            artist: None,
            duration: Some(Duration::from_secs(200)),
            path: PathBuf::from(path),
        }
    }

    /// One edition holding `tracks`, in `format`.
    fn edition(format: Option<AudioFormat>, tracks: Vec<TrackVm>) -> EditionVm {
        EditionVm {
            key: EditionKey(format),
            detail: None,
            tracks,
        }
    }

    fn albums() -> Vec<AlbumVm> {
        vec![
            AlbumVm {
                id: 11,
                title: Some("Geogaddi".into()),
                artist: AlbumArtistVm::Named("Boards of Canada".into()),
                track_artists_vary: false,
                year: Some(2002),
                first_track: PathBuf::from("/m/boc/geogaddi/01.flac"),
                editions: vec![edition(
                    Some(AudioFormat::Flac),
                    vec![
                        track("/m/boc/geogaddi/01.flac", "Ready Lets Go", 1),
                        track("/m/boc/geogaddi/02.flac", "Music Is Math", 2),
                    ],
                )],
            },
            AlbumVm {
                id: 22,
                title: Some("Untitled".into()),
                artist: AlbumArtistVm::Unknown,
                track_artists_vary: false,
                year: None,
                first_track: PathBuf::from("/m/strays/a.wav"),
                editions: vec![edition(
                    Some(AudioFormat::Wav),
                    vec![track("/m/strays/a.wav", "a.wav", 1)],
                )],
            },
        ]
    }

    fn started(path: &str, position: usize) -> Event {
        Event::TrackStarted {
            path: PathBuf::from(path),
            position,
        }
    }

    fn ready_with_queue(len: usize) -> PlayerState {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(len);
        player
    }

    #[test]
    fn started_paused_resumed_stopped_drives_phase_and_labels() {
        let albums = albums();
        let mut player = ready_with_queue(2);
        player.note_transport_sent();
        assert_eq!(player.play_pause_label(), "…");
        assert!(!player.play_pause_enabled(), "pending disables the toggle");
        assert!(!player.next_enabled());

        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(player.phase(), Phase::Playing);
        assert_eq!(player.play_pause_label(), "Pause");
        assert!(player.play_pause_enabled(), "any event clears pending");
        assert!(player.next_enabled());
        let now = player.now_playing().expect("resolved current track");
        assert_eq!(now.title, "Ready Lets Go");
        assert_eq!(now.artist.as_deref(), Some("Boards of Canada"));
        assert_eq!(player.playing_album(), Some(11));

        player.apply(&Event::Paused, &albums);
        assert_eq!(player.phase(), Phase::Paused);
        assert_eq!(player.play_pause_label(), "Play");
        assert!(player.next_enabled(), "Next skips-and-resumes while paused");
        assert!(
            player.now_playing().is_some(),
            "pause keeps the current track on the bar"
        );

        player.apply(&Event::Resumed, &albums);
        assert_eq!(player.phase(), Phase::Playing);
        assert_eq!(player.play_pause_label(), "Pause");

        player.apply(&Event::Stopped, &albums);
        assert_eq!(player.phase(), Phase::Stopped);
        assert_eq!(player.play_pause_label(), "Play");
        assert!(player.now_playing().is_none());
        assert_eq!(player.playing_album(), None);
        assert!(
            player.play_pause_enabled(),
            "a queue was requested, so Play can restart it"
        );
        assert!(!player.next_enabled(), "Next is a no-op while stopped");
    }

    #[test]
    fn track_failed_mid_queue_counts_without_interrupting() {
        let albums = albums();
        let mut player = ready_with_queue(3);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert!(player.skipped_note().is_none());

        // Decode-ahead finds track 2 broken while track 1 is still audible.
        player.apply(
            &Event::TrackFailed {
                path: PathBuf::from("/m/boc/geogaddi/02.flac"),
                reason: "decode error: oops".into(),
            },
            &albums,
        );
        assert_eq!(player.phase(), Phase::Playing, "playback continues");
        assert_eq!(
            player.now_playing().map(|n| n.title.as_str()),
            Some("Ready Lets Go"),
            "the audible track stays current"
        );
        assert_eq!(player.skipped_note().as_deref(), Some("1 track skipped"));

        player.apply(
            &Event::TrackFailed {
                path: PathBuf::from("/m/boc/geogaddi/03.flac"),
                reason: "decode error: again".into(),
            },
            &albums,
        );
        assert_eq!(player.skipped_note().as_deref(), Some("2 tracks skipped"));

        // A fresh queue request resets the count.
        player.note_queue_sent(1);
        assert!(player.skipped_note().is_none());
    }

    #[test]
    fn queue_ended_returns_to_restartable_stopped() {
        let albums = albums();
        let mut player = ready_with_queue(1);
        player.apply(&started("/m/strays/a.wav", 0), &albums);
        player.apply(&Event::QueueEnded, &albums);
        assert_eq!(player.phase(), Phase::Stopped);
        assert!(player.now_playing().is_none());
        assert_eq!(player.play_pause_label(), "Play");
        assert!(
            player.play_pause_enabled(),
            "the engine keeps the queue; Play restarts from the top"
        );
        assert!(!player.next_enabled());
    }

    #[test]
    fn engine_closed_disables_everything_with_a_note() {
        let albums = albums();
        let mut player = ready_with_queue(2);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.note_transport_sent();

        player.engine_closed();
        assert_eq!(*player.availability(), Availability::Closed);
        assert!(!player.engine_ready());
        assert_eq!(player.phase(), Phase::Stopped);
        assert!(player.now_playing().is_none());
        assert!(!player.play_pause_enabled());
        assert!(!player.next_enabled());
        assert_eq!(
            player.availability_note().as_deref(),
            Some("audio engine stopped")
        );
    }

    #[test]
    fn no_device_state_is_reported_and_never_upgraded_by_events() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::NoDevice("no default output".into()));
        assert!(!player.play_pause_enabled());
        assert!(!player.next_enabled());
        assert_eq!(
            player.availability_note().as_deref(),
            Some("no audio device — no default output")
        );
        // engine_closed on a never-opened engine keeps the startup reason.
        player.engine_closed();
        assert_eq!(
            *player.availability(),
            Availability::NoDevice("no default output".into())
        );
        // Defensive: even a stray event cannot enable controls.
        player.apply(&started("/m/strays/a.wav", 0), &albums);
        assert!(!player.play_pause_enabled());
    }

    #[test]
    fn unknown_paths_resolve_to_file_name_without_highlight() {
        let now = resolve_now_playing(&albums(), Path::new("/gone/deleted mid-queue.flac"));
        assert_eq!(now.title, "deleted mid-queue.flac");
        assert_eq!(now.album_id, None);
        assert_eq!(now.artist, None);
    }

    #[test]
    fn unknown_artist_album_resolves_without_artist() {
        let now = resolve_now_playing(&albums(), Path::new("/m/strays/a.wav"));
        assert_eq!(now.title, "a.wav");
        assert_eq!(now.album_id, Some(22));
        assert_eq!(now.artist, None);
    }

    #[test]
    fn a_track_from_any_edition_resolves_to_its_album() {
        // The same album owned twice. Whichever edition was queued, the
        // playing file must still name its album on the bar — including
        // after the panel has been switched to the other format.
        let albums = vec![AlbumVm {
            id: 33,
            title: Some("Northwest Passage".into()),
            artist: AlbumArtistVm::Named("Stan Rogers".into()),
            track_artists_vary: false,
            year: Some(1981),
            first_track: PathBuf::from("/m/flac/01.flac"),
            editions: vec![
                edition(
                    Some(AudioFormat::Flac),
                    vec![track("/m/flac/01.flac", "Northwest Passage", 1)],
                ),
                edition(
                    Some(AudioFormat::Mp3),
                    vec![track("/m/mp3/01.mp3", "Northwest Passage", 1)],
                ),
            ],
        }];
        for path in ["/m/flac/01.flac", "/m/mp3/01.mp3"] {
            let now = resolve_now_playing(&albums, Path::new(path));
            assert_eq!(now.album_id, Some(33), "{path} must resolve");
            assert_eq!(now.artist.as_deref(), Some("Stan Rogers"));
            assert_eq!(now.title, "Northwest Passage");
        }
    }

    // -----------------------------------------------------------------
    // Progress, drag, and the pending seek affordance
    // -----------------------------------------------------------------

    fn progress(elapsed_ms: u64, track_ms: Option<u64>) -> Event {
        Event::Progress {
            elapsed_ms,
            track_ms,
        }
    }

    /// A player mid-track: playing `/m/boc/geogaddi/01.flac`, 30 s into a
    /// 200 s track.
    fn playing_with_progress() -> (Vec<AlbumVm>, PlayerState) {
        let albums = albums();
        let mut player = ready_with_queue(2);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.apply(&progress(30_000, Some(200_000)), &albums);
        (albums, player)
    }

    #[test]
    fn progress_drives_the_bar_and_both_timestamps() {
        let (albums, mut player) = playing_with_progress();
        let bar = player.seek_bar().expect("a playing track has a seek bar");
        assert_eq!(bar.elapsed, "0:30");
        assert_eq!(bar.total, "3:20");
        assert!((bar.position - 0.15).abs() < 1e-6, "30/200 of the way in");
        assert!(bar.interactive);
        assert!(!bar.pending, "a confirmed reading is not pending");

        // Later reports move it; the track length rides along on each one.
        player.apply(&progress(100_000, Some(200_000)), &albums);
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "1:40");
        assert!((bar.position - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_track_without_a_declared_length_shows_elapsed_but_does_not_scrub() {
        let albums = albums();
        let mut player = ready_with_queue(1);
        player.apply(&started("/m/strays/a.wav", 0), &albums);
        player.apply(&progress(7_000, None), &albums);
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "0:07");
        assert_eq!(bar.total, "--:--", "an undeclared length is not invented");
        assert!(!bar.interactive, "there is no proportion to drag against");
        // Dragging it does nothing at all.
        player.drag_to(0.5);
        assert!(!player.dragging());
        assert_eq!(player.release_drag(), None);
    }

    #[test]
    fn dragging_shows_the_pointer_and_ignores_incoming_progress() {
        let (albums, mut player) = playing_with_progress();
        player.drag_to(0.75);
        assert!(player.dragging());
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "2:30", "the bar follows the pointer");
        assert!((bar.position - 0.75).abs() < 1e-6);
        assert!(
            bar.pending,
            "a dragged position is a request, not a reading"
        );

        // Reports keep arriving from the still-playing engine; the bar must
        // not fight the hand holding it.
        player.apply(&progress(35_000, Some(200_000)), &albums);
        assert!(player.dragging(), "an event does not end a drag");
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "2:30");
        assert!((bar.position - 0.75).abs() < 1e-6);
    }

    #[test]
    fn release_yields_a_target_then_shows_pending_until_an_event_confirms() {
        let (albums, mut player) = playing_with_progress();
        player.drag_to(0.25);
        assert_eq!(player.release_drag(), Some(50_000), "25% of 200 s");
        assert!(!player.dragging());
        assert!(player.seek_pending());

        // Pending: the bar holds the requested position rather than snapping
        // back to the last confirmed one.
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "0:50");
        assert!((bar.position - 0.25).abs() < 1e-6);
        assert!(bar.pending);

        // The engine's confirming report clears pending and takes over.
        player.apply(&progress(50_000, Some(200_000)), &albums);
        assert!(!player.seek_pending());
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "0:50");
        assert!(!bar.pending, "confirmed by the engine's report");
    }

    #[test]
    fn a_release_without_a_drag_asks_for_nothing() {
        let (_albums, mut player) = playing_with_progress();
        assert_eq!(player.release_drag(), None);
        assert!(!player.seek_pending());
    }

    #[test]
    fn drag_positions_clamp_to_the_track() {
        let (_albums, mut player) = playing_with_progress();
        player.drag_to(-3.0);
        assert_eq!(player.release_drag(), Some(0));
        player.drag_to(9.0);
        assert_eq!(player.release_drag(), Some(200_000));
    }

    #[test]
    fn a_seek_restarting_the_same_track_does_not_reset_the_bar() {
        // The engine re-emits TrackStarted for the track a seek restarted;
        // treating that as a new track would snap the bar to zero for a
        // frame. Only a different path resets it.
        let (albums, mut player) = playing_with_progress();
        player.drag_to(0.6);
        let target = player.release_drag().expect("target");
        assert_eq!(target, 120_000);

        player.apply(&progress(120_000, Some(200_000)), &albums);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(
            player.seek_bar().expect("bar").elapsed,
            "2:00",
            "the restarted track keeps its position"
        );

        // A genuinely different track does reset it.
        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "0:00");
        assert_eq!(bar.total, "--:--", "no length until the engine reports one");
    }

    #[test]
    fn stopping_clears_the_bar_entirely() {
        let (albums, mut player) = playing_with_progress();
        player.drag_to(0.5);
        player.apply(&Event::Stopped, &albums);
        assert!(
            player.seek_bar().is_none(),
            "nothing playing, nothing to seek"
        );
        assert!(!player.dragging());
        assert!(!player.seek_pending());

        // The queue ending is the same story.
        let (albums, mut player) = playing_with_progress();
        player.apply(&Event::QueueEnded, &albums);
        assert!(player.seek_bar().is_none());
    }

    #[test]
    fn a_paused_track_still_shows_and_accepts_a_seek() {
        let (albums, mut player) = playing_with_progress();
        player.apply(&Event::Paused, &albums);
        let bar = player.seek_bar().expect("pause keeps the bar on screen");
        assert!(bar.interactive, "seeking while paused is supported");
        assert_eq!(bar.elapsed, "0:30");
        player.drag_to(0.9);
        assert_eq!(player.release_drag(), Some(180_000));
    }

    #[test]
    fn a_closed_engine_takes_the_bar_with_it() {
        let (_albums, mut player) = playing_with_progress();
        player.engine_closed();
        assert!(player.seek_bar().is_none());
        player.drag_to(0.5);
        assert_eq!(player.release_drag(), None);
    }

    #[test]
    fn hour_long_tracks_get_hour_timestamps() {
        let albums = albums();
        let mut player = ready_with_queue(1);
        player.apply(&started("/m/strays/a.wav", 0), &albums);
        player.apply(&progress(3_723_000, Some(7_200_000)), &albums);
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "1:02:03");
        assert_eq!(bar.total, "2:00:00");
    }
}
