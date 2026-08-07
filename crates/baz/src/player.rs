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
//! honesty rule. Three pieces of request-side state sit on top of it, all
//! purely about what the *pointer* is doing:
//!
//! - **A gesture in progress** ([`PlayerState::press`],
//!   [`PlayerState::drag_to`]): a press starts a gesture that is a *click*
//!   until the pointer travels [`DRAG_THRESHOLD_PX`] from where it went
//!   down, at which point it becomes a *scrub* — one-way, because a hand
//!   that wandered and came back was still dragging. While scrubbing, the
//!   bar shows where the pointer is and incoming `Progress` is recorded but
//!   not displayed; anything else would fight the user's hand. A gesture
//!   that never crossed the threshold moves nothing at all until release,
//!   so a click cannot smear into a scrub of a few pixels.
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
//! - **A hover** ([`PlayerState::hover_to`]): where the pointer rests on the
//!   bar, so the view can preview the timestamp a click would land on before
//!   it is committed to.
//!
//! ## What a release asks for
//!
//! A **scrub** asks for the position under the pointer *at release*: the bar
//! followed the pointer the whole way, so that is the number the user was
//! looking at. A **click** asks for the position where the button went
//! *down*, discarding the sub-threshold travel: the bar never moved during
//! the gesture, so the place the user aimed at is the place they pressed —
//! and a click is then exactly reproducible rather than carrying up to
//! [`DRAG_THRESHOLD_PX`] of hand tremor into the target (on a 260 px bar
//! over a 3-minute track, 4 px is ~3 seconds). In both cases the target is
//! resolved once, at release, from a single pointer position — never
//! accumulated along the path.
//!
//! ## Precedence
//!
//! Exactly one number can occupy the bar, and the order is **scrub > pending
//! seek > confirmed progress**. The hover preview is a *separate, weaker*
//! channel: it never moves the bar, it is suppressed entirely while a scrub
//! is in progress (two numbers chasing one pointer is noise), and it
//! disappears when the pointer leaves. Live `Progress` keeps updating the
//! bar underneath a hover — hovering is not an interaction, and pretending
//! it pauses playback truth would be a lie.
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

/// How far the pointer may travel between press and release and still count
/// as a click rather than a scrub, in logical pixels.
///
/// Four is the smallest of the platform conventions — Windows' `SM_CXDRAG`
/// is 4 px, GTK's `gtk-dnd-drag-threshold` 8, Qt's `startDragDistance` 10 —
/// and small is right here: a deliberate scrub of a seek bar is tens of
/// pixels long, so recognizing one late is never a risk, while the travel to
/// reject is the 1–2 px of tremor a mouse picks up between button-down and
/// button-up. It also bounds the error a click can carry: on the bar's
/// ~260 px, 4 px is 1.5 % of the track — about 3 seconds of a 3-minute song,
/// which is exactly the "my click got treated as a drag" symptom.
///
/// Only horizontal travel counts. Vertical wander does not change where a
/// horizontal bar would seek to, so charging it against the threshold would
/// only make clicks harder to land.
pub const DRAG_THRESHOLD_PX: f32 = 4.0;

/// Where a pointer is along the seek bar, exactly as the widget measured it.
///
/// This is the whole vocabulary the view layer needs to report: a distance
/// from the bar's left edge and the width it was measured against, both in
/// logical pixels. Keeping *pixels* (rather than a pre-divided fraction) is
/// what lets the click/scrub threshold be expressed in the unit the user's
/// hand actually works in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    /// Distance from the bar's left edge. May fall outside `0..=width`: a
    /// held scrub keeps reporting after the pointer leaves the widget.
    pub x: f32,
    /// The bar's width when the report was taken. Zero (or non-finite) means
    /// there is no bar to speak of; [`Pointer::fraction`] reads 0 rather
    /// than dividing by it.
    pub width: f32,
}

impl Pointer {
    /// A pointer `x` logical pixels into a bar `width` logical pixels wide.
    #[must_use]
    pub fn new(x: f32, width: f32) -> Self {
        Self { x, width }
    }

    /// Where the pointer sits along the bar, `0.0..=1.0`, clamped at both
    /// ends. A zero-width or non-finite bar reads `0.0` — the only honest
    /// answer when there is no geometry to divide by.
    #[must_use]
    pub fn fraction(self) -> f32 {
        if !self.width.is_finite() || self.width <= 0.0 || !self.x.is_finite() {
            return 0.0;
        }
        (self.x / self.width).clamp(0.0, 1.0)
    }

    /// Whether the pointer is on the bar it was measured against.
    fn is_over(self) -> bool {
        self.width.is_finite() && self.width > 0.0 && self.x >= 0.0 && self.x <= self.width
    }

    /// The bar width, sanitized to something a layout can use.
    fn usable_width(self) -> f32 {
        if self.width.is_finite() && self.width > 0.0 {
            self.width
        } else {
            0.0
        }
    }
}

/// A press-to-release gesture on the bar.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Gesture {
    /// Where the button went down — the target of a click.
    anchor: Pointer,
    /// The most recent report of this gesture — the target of a scrub.
    latest: Pointer,
    /// Whether the pointer has travelled [`DRAG_THRESHOLD_PX`] from the
    /// anchor. One-way: past the threshold the gesture is a scrub even if
    /// the pointer comes back.
    scrubbing: bool,
}

/// The timestamp under a hovering pointer, ready to float over the bar.
#[derive(Debug, Clone, PartialEq)]
pub struct SeekPreview {
    /// The position the pointer is over, formatted like the elapsed stamp.
    pub label: String,
    /// Distance from the bar's left edge, clamped onto the bar (logical px).
    pub x: f32,
    /// The bar's width the `x` was measured against (logical px).
    pub width: f32,
}

/// The seek bar's render-ready state — everything the view needs to compose
/// a groove, two timestamps, and a hover preview, and nothing about how to
/// draw them.
#[derive(Debug, Clone, PartialEq)]
pub struct SeekBar {
    /// Handle position as a fraction of the track, `0.0..=1.0`. Always 0
    /// when the track length is unknown (there is no proportion to show).
    pub position: f32,
    /// Left timestamp: the position being shown — scrubbed, pending, or
    /// confirmed, in that order of precedence.
    pub elapsed: String,
    /// Right timestamp: the track's length, or `--:--` when undeclared.
    pub total: String,
    /// Whether dragging the bar can do anything. False when the engine is
    /// unavailable, nothing is playing, or the track declares no length —
    /// there is no honest position to seek *to* without one.
    pub interactive: bool,
    /// Whether the position shown is a *request* rather than a confirmed
    /// reading: the bar is being scrubbed, or a seek is awaiting its
    /// confirming event. The view marks it so the number is never mistaken
    /// for playback truth it has not earned yet.
    pub pending: bool,
    /// Where the pointer is resting and what a click there would seek to —
    /// `None` unless the pointer is on a seekable bar with no scrub in
    /// progress (see the module's precedence rules).
    pub preview: Option<SeekPreview>,
}

/// The placeholder shown where a track length would be, when the container
/// never declared one. Same width as a real `m:ss` so the bar does not jump.
const UNKNOWN_TOTAL: &str = "--:--";

/// The event-derived playback state behind every playback widget.
///
/// `PartialEq` but not `Eq`: pointer geometry is measured in floating-point
/// logical pixels, and there is no honest total equality over those.
#[derive(Debug, Clone, PartialEq)]
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
    /// The press-to-release gesture in progress, if any.
    gesture: Option<Gesture>,
    /// Where the pointer is resting on the bar, if it is.
    hover: Option<Pointer>,
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
            gesture: None,
            hover: None,
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
        // we last sent (module docs). A gesture and a hover are the
        // pointer's business, not the engine's, so they survive.
        self.pending = false;
        self.seek_pending = None;
    }

    /// Forget where we were: a different track, or none at all.
    fn reset_progress(&mut self) {
        self.elapsed_ms = 0;
        self.track_ms = None;
        self.gesture = None;
        self.hover = None;
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

    /// The pointer is resting on the bar at `pointer` with no button held:
    /// record it so the view can preview the timestamp a click would land
    /// on.
    ///
    /// A no-op (and a *clearing* one) when the bar is not seekable — without
    /// a track length there is no honest number to preview.
    pub fn hover_to(&mut self, pointer: Pointer) {
        self.hover = self.seekable_total().and(Some(pointer));
    }

    /// The pointer left the bar: the preview goes with it.
    pub fn hover_left(&mut self) {
        self.hover = None;
    }

    /// The bar was pressed at `pointer` — the start of a gesture that stays
    /// a click until it travels [`DRAG_THRESHOLD_PX`] (module docs). Nothing
    /// is requested and nothing on the bar moves yet.
    ///
    /// A no-op when the bar is not seekable: there is nothing a position
    /// could mean without a track length.
    pub fn press(&mut self, pointer: Pointer) {
        if self.seekable_total().is_none() {
            return;
        }
        self.gesture = Some(Gesture {
            anchor: pointer,
            latest: pointer,
            scrubbing: false,
        });
    }

    /// The pointer moved to `pointer` with the bar held. Past
    /// [`DRAG_THRESHOLD_PX`] of travel from the press this engages the
    /// scrub, and the bar shows the pointer in place of the engine's reports
    /// until [`Self::release_drag`].
    ///
    /// A no-op when no gesture is in progress — a pointer moving over the
    /// bar with no button down is a [hover](Self::hover_to).
    pub fn drag_to(&mut self, pointer: Pointer) {
        let Some(gesture) = self.gesture.as_mut() else {
            return;
        };
        gesture.latest = pointer;
        if (pointer.x - gesture.anchor.x).abs() >= DRAG_THRESHOLD_PX {
            gesture.scrubbing = true;
        }
    }

    /// The bar was released. Returns the position to ask the engine for —
    /// the pointer's position for a scrub, the press position for a click
    /// (module docs) — and records it as pending so the bar keeps showing it
    /// until an event confirms. `None` when no gesture was in progress, or
    /// when the track stopped being seekable under it.
    pub fn release_drag(&mut self) -> Option<u64> {
        let gesture = self.gesture.take()?;
        let total = self.seekable_total()?;
        let landing = if gesture.scrubbing {
            gesture.latest
        } else {
            gesture.anchor
        };
        let target = scale(total, landing.fraction());
        self.seek_pending = Some(target);
        // The pointer is demonstrably wherever the release left it: keep
        // previewing from there when that is still on the bar, and show
        // nothing when the release happened off the end of a scrub.
        self.hover = gesture.latest.is_over().then_some(gesture.latest);
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
    /// The position shown is the scrub under the pointer if there is one,
    /// else the seek awaiting confirmation, else what the engine last
    /// reported (module docs pin the precedence).
    #[must_use]
    pub fn seek_bar(&self) -> Option<SeekBar> {
        if !self.engine_ready() || self.now_playing.is_none() {
            return None;
        }
        let shown = self
            .scrub_ms()
            .or(self.seek_pending)
            .unwrap_or(self.elapsed_ms);
        let total = self.seekable_total();
        Some(SeekBar {
            position: total.map_or(0.0, |total| fraction(shown, total)),
            elapsed: format_ms(total.map_or(shown, |total| shown.min(total))),
            total: total.map_or_else(|| UNKNOWN_TOTAL.to_owned(), format_ms),
            interactive: total.is_some(),
            pending: self.dragging() || self.seek_pending(),
            preview: self.preview(),
        })
    }

    /// The position under the pointer while a scrub is engaged.
    fn scrub_ms(&self) -> Option<u64> {
        let gesture = self.gesture.filter(|gesture| gesture.scrubbing)?;
        Some(scale(self.seekable_total()?, gesture.latest.fraction()))
    }

    /// The hover preview: what a click under the pointer would seek to.
    /// Suppressed while scrubbing — the bar itself already shows that
    /// target, and two numbers chasing one pointer is noise.
    fn preview(&self) -> Option<SeekPreview> {
        if self.dragging() {
            return None;
        }
        let total = self.seekable_total()?;
        let hover = self.hover?;
        let width = hover.usable_width();
        Some(SeekPreview {
            label: format_ms(scale(total, hover.fraction())),
            // Derived from the (clamped) fraction rather than from `x`, so
            // the marker and the timestamp can never disagree about where
            // the pointer is — including past either end of the bar.
            x: hover.fraction() * width,
            width,
        })
    }

    /// Whether the bar is currently being scrubbed — the view uses this to
    /// keep the handle lit, and the tests to pin the state machine. A press
    /// that has not yet crossed [`DRAG_THRESHOLD_PX`] is not a scrub.
    #[must_use]
    pub fn dragging(&self) -> bool {
        self.gesture.is_some_and(|gesture| gesture.scrubbing)
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

/// Where to put the left edge of a `tip_width`-wide preview so that it is
/// centered over the previewed position without hanging off either end of
/// the bar.
///
/// Pure geometry, so the view function that places the tip carries no math
/// of its own: it asks for an offset and pushes a spacer that wide. A tip
/// wider than the bar pins to the left edge rather than going negative.
#[must_use]
pub fn preview_offset(preview: &SeekPreview, tip_width: f32) -> f32 {
    let slack = (preview.width - tip_width).max(0.0);
    (preview.x - tip_width / 2.0).clamp(0.0, slack)
}

#[cfg(test)]
mod tests {
    use baz_core::library::AudioFormat;

    use crate::vm::{EditionKey, EditionVm, TrackVm};

    use super::*;

    fn track(path: &str, title: &str, number: u32) -> TrackVm {
        TrackVm {
            number: Some(number),
            title: title.to_owned(),
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
                artist: Some("Boards of Canada".into()),
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
                artist: None,
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
            artist: Some("Stan Rogers".into()),
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
    // Progress, pointer gestures, hover preview, and the pending seek
    // affordance
    // -----------------------------------------------------------------

    fn progress(elapsed_ms: u64, track_ms: Option<u64>) -> Event {
        Event::Progress {
            elapsed_ms,
            track_ms,
        }
    }

    /// The bar these tests measure against: 200 logical px, so 1 px is
    /// exactly 1/200 of the track and every expectation below is arithmetic
    /// anyone can check in their head (the fixture track is 200 s, so 1 px
    /// is 1 s).
    const BAR: f32 = 200.0;

    /// A pointer `x` px into [`BAR`].
    fn at(x: f32) -> Pointer {
        Pointer::new(x, BAR)
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
        // Pointing at it does nothing at all — no gesture, no preview.
        player.hover_to(at(100.0));
        player.press(at(100.0));
        player.drag_to(at(160.0));
        assert!(!player.dragging());
        assert_eq!(player.release_drag(), None);
        assert!(
            player.seek_bar().expect("bar").preview.is_none(),
            "no length, no honest timestamp to preview"
        );
    }

    #[test]
    fn dragging_shows_the_pointer_and_ignores_incoming_progress() {
        let (albums, mut player) = playing_with_progress();
        player.press(at(100.0));
        player.drag_to(at(150.0));
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
        player.press(at(100.0));
        player.drag_to(at(50.0));
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
    fn a_release_without_a_press_asks_for_nothing() {
        let (_albums, mut player) = playing_with_progress();
        assert_eq!(player.release_drag(), None);
        assert!(!player.seek_pending());
    }

    #[test]
    fn drag_positions_clamp_to_the_track() {
        // A held scrub keeps reporting after the pointer leaves the widget,
        // so out-of-bounds pixels are normal input, not a bug to reject.
        let (_albums, mut player) = playing_with_progress();
        player.press(at(100.0));
        player.drag_to(at(-600.0));
        assert_eq!(player.release_drag(), Some(0));
        player.press(at(100.0));
        player.drag_to(at(1800.0));
        assert_eq!(player.release_drag(), Some(200_000));
    }

    #[test]
    fn a_seek_restarting_the_same_track_does_not_reset_the_bar() {
        // The engine re-emits TrackStarted for the track a seek restarted;
        // treating that as a new track would snap the bar to zero for a
        // frame. Only a different path resets it.
        let (albums, mut player) = playing_with_progress();
        player.press(at(20.0));
        player.drag_to(at(120.0));
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
        player.hover_to(at(100.0));
        player.press(at(20.0));
        player.drag_to(at(100.0));
        player.apply(&Event::Stopped, &albums);
        assert!(
            player.seek_bar().is_none(),
            "nothing playing, nothing to seek"
        );
        assert!(!player.dragging());
        assert!(!player.seek_pending());
        assert_eq!(
            player.release_drag(),
            None,
            "a gesture does not survive the track it was on"
        );

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
        player.press(at(20.0));
        player.drag_to(at(180.0));
        assert_eq!(player.release_drag(), Some(180_000));
    }

    #[test]
    fn a_closed_engine_takes_the_bar_with_it() {
        let (_albums, mut player) = playing_with_progress();
        player.engine_closed();
        assert!(player.seek_bar().is_none());
        player.hover_to(at(100.0));
        player.press(at(100.0));
        player.drag_to(at(150.0));
        assert!(!player.dragging());
        assert_eq!(player.release_drag(), None);
    }

    // -----------------------------------------------------------------
    // Click vs scrub: the movement threshold
    // -----------------------------------------------------------------

    #[test]
    fn a_press_with_a_tiny_wobble_is_a_click_to_where_it_went_down() {
        let (_albums, mut player) = playing_with_progress();
        player.press(at(150.0));
        // Three pixels of tremor between button-down and button-up: under
        // the threshold, so this is a click and the wobble is discarded.
        player.drag_to(at(151.0));
        player.drag_to(at(153.0));
        assert!(!player.dragging(), "sub-threshold travel is not a scrub");
        assert_eq!(
            player.release_drag(),
            Some(150_000),
            "the click lands where the button went down, not where it came up"
        );
    }

    #[test]
    fn a_click_moves_nothing_on_the_bar_until_it_is_released() {
        // The visual half of the same rule: during a sub-threshold gesture
        // the bar keeps showing playback truth, so the user never sees the
        // handle smear a few pixels and wonder what they did.
        let (albums, mut player) = playing_with_progress();
        player.hover_to(at(150.0));
        player.press(at(150.0));
        player.drag_to(at(152.0));
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "0:30", "still the engine's last report");
        assert!(!bar.pending, "nothing has been requested yet");
        assert_eq!(
            bar.preview.as_ref().map(|p| p.label.as_str()),
            Some("2:30"),
            "the preview keeps showing what the click will land on"
        );

        // Progress still flows underneath an undecided gesture.
        player.apply(&progress(31_000, Some(200_000)), &albums);
        assert_eq!(player.seek_bar().expect("bar").elapsed, "0:31");

        assert_eq!(player.release_drag(), Some(150_000));
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "2:30", "released: now it is a request");
        assert!(bar.pending);
    }

    #[test]
    fn a_press_that_travels_past_the_threshold_scrubs_with_the_pointer() {
        let (_albums, mut player) = playing_with_progress();
        player.press(at(150.0));
        player.drag_to(at(120.0));
        assert!(player.dragging());
        assert_eq!(
            player.seek_bar().expect("bar").elapsed,
            "2:00",
            "the bar follows the pointer once the scrub engages"
        );
        player.drag_to(at(80.0));
        assert_eq!(
            player.release_drag(),
            Some(80_000),
            "a scrub lands where the pointer was released, not where it started"
        );
    }

    #[test]
    fn the_threshold_is_exactly_four_pixels() {
        // Pinning the boundary in both directions: one pixel either side of
        // DRAG_THRESHOLD_PX decides click vs scrub, so a change to the
        // constant is a deliberate, visible edit rather than a drift.
        assert!((DRAG_THRESHOLD_PX - 4.0).abs() < f32::EPSILON);

        let (_albums, mut player) = playing_with_progress();
        player.press(at(100.0));
        player.drag_to(at(100.0 + DRAG_THRESHOLD_PX - 0.1));
        assert!(!player.dragging(), "just under the threshold is a click");
        assert_eq!(player.release_drag(), Some(100_000));

        player.press(at(100.0));
        player.drag_to(at(100.0 + DRAG_THRESHOLD_PX));
        assert!(player.dragging(), "at the threshold the scrub engages");
        assert_eq!(player.release_drag(), Some(104_000));

        // Leftward travel counts the same.
        player.press(at(100.0));
        player.drag_to(at(100.0 - DRAG_THRESHOLD_PX));
        assert!(player.dragging());
        assert_eq!(player.release_drag(), Some(96_000));
    }

    #[test]
    fn a_scrub_that_wanders_back_to_the_press_point_stays_a_scrub() {
        // Once the hand has visibly dragged the bar, coming back to the
        // start is a scrub *to the start* — not a click that happens to have
        // taken a detour.
        let (_albums, mut player) = playing_with_progress();
        player.press(at(100.0));
        player.drag_to(at(180.0));
        player.drag_to(at(101.0));
        assert!(player.dragging(), "the threshold is crossed one-way");
        assert_eq!(player.release_drag(), Some(101_000));
    }

    #[test]
    fn a_press_outside_the_bar_still_resolves_against_it() {
        // The widget hands over whatever it measured; clamping is this
        // module's job, at both ends and for a bar of no width at all.
        let (_albums, mut player) = playing_with_progress();
        player.press(Pointer::new(-20.0, BAR));
        assert_eq!(player.release_drag(), Some(0));
        player.press(Pointer::new(BAR + 40.0, BAR));
        assert_eq!(player.release_drag(), Some(200_000));
        player.press(Pointer::new(37.0, 0.0));
        assert_eq!(
            player.release_drag(),
            Some(0),
            "a zero-width bar has no position to click but must not divide by it"
        );
    }

    // -----------------------------------------------------------------
    // Hover preview
    // -----------------------------------------------------------------

    #[test]
    fn hovering_previews_the_timestamp_under_the_pointer() {
        let (_albums, mut player) = playing_with_progress();
        player.hover_to(at(50.0));
        let preview = player
            .seek_bar()
            .expect("bar")
            .preview
            .expect("a hovered seekable bar previews");
        assert_eq!(preview.label, "0:50", "25% of 200 s");
        assert!((preview.x - 50.0).abs() < 1e-6);
        assert!((preview.width - BAR).abs() < 1e-6);

        player.hover_to(at(160.0));
        assert_eq!(
            player
                .seek_bar()
                .expect("bar")
                .preview
                .expect("preview")
                .label,
            "2:40"
        );
    }

    #[test]
    fn hover_previews_clamp_at_both_ends_and_survive_a_zero_width_bar() {
        let (_albums, mut player) = playing_with_progress();
        player.hover_to(Pointer::new(-40.0, BAR));
        let preview = player.seek_bar().expect("bar").preview.expect("preview");
        assert_eq!(preview.label, "0:00");
        assert!((preview.x - 0.0).abs() < 1e-6, "the marker clamps too");

        player.hover_to(Pointer::new(BAR + 40.0, BAR));
        let preview = player.seek_bar().expect("bar").preview.expect("preview");
        assert_eq!(preview.label, "3:20");
        assert!((preview.x - BAR).abs() < 1e-6);

        player.hover_to(Pointer::new(12.0, 0.0));
        let preview = player.seek_bar().expect("bar").preview.expect("preview");
        assert_eq!(preview.label, "0:00");
        assert!((preview.x - 0.0).abs() < 1e-6);
        assert!((preview.width - 0.0).abs() < 1e-6);
    }

    #[test]
    fn the_pointer_leaving_the_bar_clears_the_preview() {
        let (_albums, mut player) = playing_with_progress();
        player.hover_to(at(50.0));
        assert!(player.seek_bar().expect("bar").preview.is_some());
        player.hover_left();
        assert!(
            player.seek_bar().expect("bar").preview.is_none(),
            "no pointer on the bar, nothing to preview"
        );
    }

    #[test]
    fn a_scrub_outranks_the_hover_preview_which_outranks_nothing() {
        // Precedence, in one test: a scrub owns the bar *and* suppresses the
        // preview; a hover owns neither and never disturbs live progress.
        let (albums, mut player) = playing_with_progress();
        player.hover_to(at(50.0));
        player.apply(&progress(40_000, Some(200_000)), &albums);
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "0:40", "hovering does not freeze the bar");
        assert!(!bar.pending);
        assert_eq!(bar.preview.as_ref().map(|p| p.label.as_str()), Some("0:50"));

        player.press(at(50.0));
        player.drag_to(at(150.0));
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "2:30", "the scrub owns the bar");
        assert!(bar.pending);
        assert!(
            bar.preview.is_none(),
            "one pointer, one number: the scrub suppresses the preview"
        );

        // Progress arriving mid-scrub changes neither.
        player.apply(&progress(41_000, Some(200_000)), &albums);
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "2:30");
        assert!(bar.preview.is_none());
    }

    #[test]
    fn releasing_leaves_the_preview_where_the_pointer_actually_is() {
        let (_albums, mut player) = playing_with_progress();
        // Released on the bar: the preview picks up from there.
        player.hover_to(at(50.0));
        player.press(at(50.0));
        player.drag_to(at(150.0));
        assert_eq!(player.release_drag(), Some(150_000));
        assert_eq!(
            player
                .seek_bar()
                .expect("bar")
                .preview
                .map(|preview| preview.label),
            Some("2:30".to_owned())
        );

        // Released past the end of the bar: the pointer is not on it, so
        // there is nothing to preview.
        player.press(at(150.0));
        player.drag_to(Pointer::new(BAR + 60.0, BAR));
        assert_eq!(player.release_drag(), Some(200_000));
        assert!(player.seek_bar().expect("bar").preview.is_none());
    }

    #[test]
    fn a_pending_seek_shows_on_the_bar_without_hiding_the_preview() {
        // A pending seek is a request already sent; the pointer is still
        // free to shop for the next one.
        let (_albums, mut player) = playing_with_progress();
        player.press(at(100.0));
        assert_eq!(player.release_drag(), Some(100_000));
        player.hover_to(at(20.0));
        let bar = player.seek_bar().expect("bar");
        assert_eq!(bar.elapsed, "1:40", "the pending request holds the bar");
        assert!(bar.pending);
        assert_eq!(bar.preview.as_ref().map(|p| p.label.as_str()), Some("0:20"));
    }

    // -----------------------------------------------------------------
    // Pure geometry
    // -----------------------------------------------------------------

    #[test]
    fn pointer_fractions_clamp_and_refuse_to_divide_by_nothing() {
        assert!((Pointer::new(50.0, 200.0).fraction() - 0.25).abs() < 1e-6);
        assert!((Pointer::new(0.0, 200.0).fraction() - 0.0).abs() < 1e-6);
        assert!((Pointer::new(200.0, 200.0).fraction() - 1.0).abs() < 1e-6);
        assert!((Pointer::new(-9.0, 200.0).fraction() - 0.0).abs() < 1e-6);
        assert!((Pointer::new(999.0, 200.0).fraction() - 1.0).abs() < 1e-6);
        // Degenerate geometry reads zero rather than NaN or infinity.
        assert!((Pointer::new(50.0, 0.0).fraction() - 0.0).abs() < 1e-6);
        assert!((Pointer::new(50.0, -30.0).fraction() - 0.0).abs() < 1e-6);
        assert!((Pointer::new(50.0, f32::NAN).fraction() - 0.0).abs() < 1e-6);
        assert!((Pointer::new(f32::NAN, 200.0).fraction() - 0.0).abs() < 1e-6);
        assert!((Pointer::new(f32::INFINITY, 200.0).fraction() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn preview_offsets_center_the_tip_and_keep_it_on_the_bar() {
        let tip = 50.0;
        let preview = |x: f32, width: f32| SeekPreview {
            label: String::new(),
            x,
            width,
        };
        // Mid-bar: centered under the pointer.
        assert!((preview_offset(&preview(100.0, 200.0), tip) - 75.0).abs() < 1e-6);
        // Both ends: pinned flush rather than hanging off.
        assert!((preview_offset(&preview(0.0, 200.0), tip) - 0.0).abs() < 1e-6);
        assert!((preview_offset(&preview(200.0, 200.0), tip) - 150.0).abs() < 1e-6);
        assert!((preview_offset(&preview(10.0, 200.0), tip) - 0.0).abs() < 1e-6);
        // A tip wider than the bar (or a bar of no width) pins left instead
        // of going negative and pushing the layout around.
        assert!((preview_offset(&preview(20.0, 30.0), tip) - 0.0).abs() < 1e-6);
        assert!((preview_offset(&preview(0.0, 0.0), tip) - 0.0).abs() < 1e-6);
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
