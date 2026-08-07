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
//! Engine availability ([`Availability`]) is seeded from the spawn result at
//! startup — that is a returned fact, not an assumption — and downgrades to
//! [`Availability::Closed`] when the event bridge reports the engine gone or
//! a send fails.
//!
//! Everything here is pure and iced-free, so the whole machine is unit
//! tested on the host without a window, an audio device, or the
//! `device-output` feature.

use std::path::Path;

use baz_core::protocol::Event;

use crate::vm::AlbumVm;

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
#[must_use]
pub fn resolve_now_playing(albums: &[AlbumVm], path: &Path) -> NowPlaying {
    for album in albums {
        for track in &album.tracks {
            if track.path == path {
                return NowPlaying {
                    album_id: Some(album.id),
                    title: track.title.clone(),
                    artist: album.artist.clone(),
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

/// The event-derived playback state behind every playback widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    availability: Availability,
    phase: Phase,
    now_playing: Option<NowPlaying>,
    /// Tracks in the queue we last asked for (request-side; module docs).
    queued: usize,
    /// A transport command awaiting its confirming event (module docs).
    pending: bool,
    /// [`Event::TrackFailed`] count since the last queue request.
    failed: usize,
}

impl PlayerState {
    /// Fresh state; `availability` is the engine spawn result.
    #[must_use]
    pub fn new(availability: Availability) -> Self {
        Self {
            availability,
            phase: Phase::Stopped,
            now_playing: None,
            queued: 0,
            pending: false,
            failed: 0,
        }
    }

    /// Fold one engine event into the state. `albums` is the current shelf
    /// view model, used to resolve [`Event::TrackStarted`] paths.
    pub fn apply(&mut self, event: &Event, albums: &[AlbumVm]) {
        match event {
            Event::TrackStarted { path, .. } => {
                self.phase = Phase::Playing;
                self.now_playing = Some(resolve_now_playing(albums, path));
            }
            Event::Paused => self.phase = Phase::Paused,
            Event::Resumed => self.phase = Phase::Playing,
            Event::Stopped | Event::QueueEnded => {
                self.phase = Phase::Stopped;
                self.now_playing = None;
            }
            Event::TrackFailed { .. } => self.failed += 1,
            // `Event` is #[non_exhaustive]: tolerate unknown messages.
            _ => {}
        }
        // Any received event proves the engine made progress past whatever
        // we last sent (module docs).
        self.pending = false;
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
        self.pending = false;
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use crate::vm::TrackVm;

    use super::*;

    fn albums() -> Vec<AlbumVm> {
        let track = |path: &str, title: &str, number| TrackVm {
            number: Some(number),
            title: title.to_owned(),
            duration: Some(Duration::from_secs(200)),
            path: PathBuf::from(path),
        };
        vec![
            AlbumVm {
                id: 11,
                title: Some("Geogaddi".into()),
                artist: Some("Boards of Canada".into()),
                year: Some(2002),
                first_track: PathBuf::from("/m/boc/geogaddi/01.flac"),
                tracks: vec![
                    track("/m/boc/geogaddi/01.flac", "Ready Lets Go", 1),
                    track("/m/boc/geogaddi/02.flac", "Music Is Math", 2),
                ],
            },
            AlbumVm {
                id: 22,
                title: Some("Untitled".into()),
                artist: None,
                year: None,
                first_track: PathBuf::from("/m/strays/a.wav"),
                tracks: vec![track("/m/strays/a.wav", "a.wav", 1)],
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
}
