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
//! - **The queue** ([`PlayerState::note_queue_sent`]): the engine echoes no
//!   event for [`SetQueue`](baz_core::protocol::Command::SetQueue), so what
//!   was queued is knowledge only the sender has. It decides whether a Play
//!   button can do anything at all, and — since the queue panel landed — it is
//!   also the *only* record of what is playing next, which is why the whole
//!   [`vm::QueueVm`] is kept rather than merely its length. See "The queue"
//!   below.
//! - **A pending transport command** ([`PlayerState::note_transport_sent`]):
//!   the documented "brief pending affordance". Pending clears on the *next
//!   event of any kind* (any event proves the engine processed past our
//!   command; clearing on any rather than the matching one means a command
//!   that raced into a no-op — pause just as the queue ended, say — cannot
//!   wedge the button forever). It never sets a phase.
//!
//! ## What pending is allowed to change on screen
//!
//! Nothing that occupies space, and nothing that carries meaning. It was
//! once allowed both: the toggle swapped its label to `…` and both transport
//! buttons disabled while a command was in flight. That is the bottom bar's
//! reported "text flash" — three simultaneous changes to the two controls
//! directly under the pointer (the toggle's glyph, the toggle's text color
//! as it fell to the disabled style, and Next's alongside it), each of them
//! reversed one to a few frames later. The window is short by construction —
//! the engine answers a transport command between pump iterations, bounded
//! by one `DeviceSink::write` (up to a device ring's worth of backpressure,
//! ~46 ms worst case at the shipped 2048-frame chunk) plus one iced
//! event-loop turn — which is exactly long enough to see as a blink and far
//! too short to read as information.
//!
//! So pending is now a *style* fact, not a content one:
//! [`PlayerState::play_pause`] answers from the confirmed phase alone, and
//! [`PlayerState::play_pause_enabled`] / [`PlayerState::next_enabled`] no
//! longer consult it. All that remains is
//! [`PlayerState::transport_pending`], which the view spends on the glyph's
//! opacity — a fixed-size control that dims a little and comes back. The
//! debounce the disable used to provide is not missed: the engine documents
//! redundant transport commands as no-ops that emit nothing, and a second
//! press of Next inside the window is a second skip, which is what pressing
//! it twice means everywhere else.
//!
//! The honesty rule is untouched by all of this. Phase still moves only in
//! [`PlayerState::apply`]; the glyph still shows what the engine last
//! *confirmed*, never what we just asked for.
//!
//! # The needle, and the two timestamps beside it
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
//! # The volume
//!
//! The same honesty rule, and the ADR that governs it says so in as many
//! words: *observe [`Event::VolumeChanged`] and follow it rather than your own
//! optimistic value, so two front ends on one engine agree* (ADR-0011, "What a
//! front end needs"). So [`PlayerState::apply`] is again the only place the
//! position, the mute flag, and the [`VolumePath`] move. Sending
//! `SetVolume` changes nothing by itself; a `VolumeChanged` carrying a
//! position we never asked for — another front end, or the engine clamping
//! ours — simply becomes the truth.
//!
//! Three things sit on top, all request-side and all modelled on the seek
//! bar's:
//!
//! - **A gesture** ([`PlayerState::press_volume`],
//!   [`PlayerState::drag_volume`]) with the same
//!   [`DRAG_THRESHOLD_PX`] click-vs-drag rule, for the same reason: without
//!   it, two pixels of tremor between button-down and button-up would move
//!   the level away from the position the user aimed at — and near the top of
//!   the travel, off unity. The one deliberate difference from the seek bar
//!   is *when* the request goes out. A seek commits on release, because
//!   seeking to every position the pointer passed through would be absurd; a
//!   fader commits on press and on every scrub step, because a volume control
//!   you cannot hear until you let go is not a volume control. Sub-threshold
//!   travel still moves nothing, so a click is exactly where it was pressed.
//! - **A pending position**, recorded by whichever of the methods above asked
//!   for it, shown until the confirming event arrives and cleared on the next
//!   event of any kind — the seek bar's affordance, wholesale, including its
//!   anti-wedging rule.
//! - **A hover** ([`PlayerState::hover_volume`]), previewing the level a click
//!   would set.
//!
//! The unit of the preview is **decibels**, and that is a decision rather than
//! a default. Percent-of-travel would be a number about the widget rather than
//! about the sound; percent-of-amplitude would disagree with where the handle
//! is, because the taper is a cube. dB is what
//! [`Volume::decibels`](baz_core::volume::Volume::decibels) exists to provide,
//! it is the unit the ADR labels the control in, and it is the only one in
//! which the detent's meaning is legible on sight: at unity the readout is not
//! "100 %" but the word `unity`, and one position below it is `-0.0 dB`.
//!
//! [`UNITY_SNAP_PX`] is the other half of making unity reachable: within a few
//! pixels of the top of the travel the position resolves to
//! [`MAX_POSITION`] exactly, so the
//! bit-perfect position is a place the hand lands rather than a place it has
//! to be threaded into.
//!
//! # The queue
//!
//! What baz handed the engine is request-side memory, exactly like the queue
//! *size* always was — and it has to be, because there is no command that asks
//! the engine what its queue is and no event that reports one. The honesty
//! rule therefore applies to the queue in a specific shape:
//!
//! - **The list** is what we sent ([`vm::QueueVm`], recorded whole by
//!   [`PlayerState::note_queue_sent`]). It is not a claim about the engine's
//!   state; it is a claim about ours, and it is exactly true.
//! - **The position in it** is engine truth and nothing else:
//!   [`Event::TrackStarted`] carries a zero-based queue position, and
//!   [`PlayerState::apply`] is the only place it is recorded. It clears the
//!   moment a session ends ([`Event::Stopped`], [`Event::QueueEnded`], or the
//!   engine going away) — a queue with nothing playing marks no row.
//! - **The two are reconciled, never assumed** ([`vm::QueueVm::playing`]): the
//!   engine's position is believed when the path it arrived with agrees with
//!   the path we recorded there, and the path wins when it does not. A track
//!   that is not in the queue we remember marks nothing at all.
//!
//! The list survives a stop, which is the engine's own behaviour rather than a
//! choice made here: `Stop` abandons the current run through the queue, but
//! "the queue itself is kept; a later `Play` starts from the top". So the panel
//! keeps showing what would play, with no row marked.
//!
//! What is deliberately *not* here is any way to change it. Reordering,
//! removing a track, and clicking a row to jump to it all need engine commands
//! that do not exist — see [`PlayerState::queue_list`] for exactly which.
//!
//! # The signal path
//!
//! [`Event::SignalPath`] reports what sits between the decoded file and the
//! output — the source's rate and declared depth, the rate the output is
//! running at, and whether the engine is converting between them (ADR-0009).
//! It arrives when a session starts and only when something changes, so it is
//! folded the same way as everything else: recorded on arrival, forgotten
//! when the session that it described ends (stop, queue end, engine gone).
//! There is no session, so there is no chain to report.
//!
//! [`PlayerState::signal_note`] is the render-ready reading, and ADR-0011
//! amends what it answers. Bit-exactness used to be one fact and is now the
//! conjunction of two — `SignalChain::Direct` **and**
//! [`VolumePath::is_transparent`] — so the note has three states rather than
//! two:
//!
//! - **Converting**: the chain, `48 → 44.1 kHz`, with the reason on hover.
//!   Unchanged, down to the wording.
//! - **Direct and transparent** ([`PlayerState::bit_exact`]): the word
//!   `bit-perfect`. This is the affirmative reading ADR-0009's "What a front
//!   end needs" leaves open — *nothing, or the same affordance in a resting
//!   state* — and persona P4 asks for by name: a readout that proves the
//!   chain rather than merely failing to complain about it.
//! - **Anything else** — a direct chain with the volume scaling the samples,
//!   or no session at all: nothing. Not because the fact is unimportant but
//!   because it is already on screen, six pixels away: the fader is visibly
//!   not at the top. A label that appeared every time somebody turned the
//!   music down would be exactly the nagging both ADRs rule out.
//!
//! The words stay flat in all three. No severity, no icon, no colour: the note
//! is two strings and the view has no decision left to make, which is what
//! keeps the tone out of the view layer's hands.
//!
//! # ReplayGain
//!
//! The same honesty rule once more, and ADR-0013 states it in the same words
//! ADR-0011 used: *observe `Event::ReplayGainChanged` and follow it rather
//! than your own optimistic copy.* So [`PlayerState::apply`] is again the only
//! place it moves, [`PlayerState::seed_replay_gain`] takes the engine's
//! reading once at start-up, and a control press changes nothing until the
//! engine confirms it. The state itself, the settings a control asks for, and
//! the words the readout says all live in [`crate::replaygain`], which is pure
//! and tested on its own; what is kept here is the fold site, so that there is
//! exactly one.
//!
//! It survives [`PlayerState::engine_closed`] for [`PlayerState::volume`]'s reason:
//! ReplayGain is engine state rather than session state, and the last reading
//! stays the honest answer to "how is it set".
//!
//! **The fidelity readout does not change.** [`PlayerState::bit_exact`] is
//! still `SignalChain::Direct` and [`VolumePath::is_transparent`], and it is
//! still the whole question. An active ReplayGain moves the volume path to
//! `SoftwareGain` with the fader at unity — that is correct, it is reported on
//! the channel it has always been reported on, and nothing about ReplayGain
//! adds a second answer to it (ADR-0013 §8).
//!
//! Engine availability ([`Availability`]) is seeded from the spawn result at
//! startup — that is a returned fact, not an assumption — and downgrades to
//! [`Availability::Closed`] when the event bridge reports the engine gone or
//! a send fails.
//!
//! Everything here is pure and iced-free, so the whole machine is unit
//! tested without a window or an audio device.

use std::path::{Path, PathBuf};
use std::time::Duration;

use baz_core::protocol::{ConversionReason, Event, ReplayGainSource, SignalChain, VolumePath};
use baz_core::replaygain::ReplayGainSettings;
use baz_core::traversal::Traversal;
use baz_core::volume::{MAX_POSITION, Volume};

use crate::replaygain::{ReplayGain, ReplayGainReadout};
use crate::vm::{self, AlbumVm, QueueItemVm, QueueVm};

/// Whether a playback engine exists to talk to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// The engine could not open an output device at startup; the shelf
    /// works, playback controls show this state. The string is the
    /// user-presentable reason.
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

/// What the play/pause toggle currently offers to do — the action a press
/// would request, read off the *confirmed* phase.
///
/// Deliberately not a string: the view turns it into a glyph and a tooltip,
/// and the point of the type is that exactly two answers exist. A command in
/// flight is not a third one (see the module's pending note).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayPause {
    /// A press asks the engine to start or resume.
    Play,
    /// A press asks the engine to pause.
    Pause,
}

impl PlayPause {
    /// The control's accessible name — its tooltip, and the closest thing
    /// iced 0.13 has to a label for an icon-only button.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Pause => "Pause",
        }
    }
}

/// The bottom bar's resolved current track.
///
/// The first three fields are what the bottom bar draws; the rest are the
/// catalogue facts a now-playing readout outside the window needs (MPRIS
/// metadata — see [`crate::mpris`]). They are resolved in the same pass
/// because they come from the same view-model lookup, and resolving them
/// twice would risk the two readouts disagreeing about what is playing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlaying {
    /// Shelf album containing the track, when the path resolves against the
    /// library — drives the playing-album highlight.
    pub album_id: Option<u64>,
    /// Display title (track title, else the file name).
    pub title: String,
    /// Album artist, when resolved.
    pub artist: Option<String>,
    /// The track's own artist tag, when it has one that the album's does not
    /// already cover.
    pub track_artist: Option<String>,
    /// Album title, when resolved.
    pub album: Option<String>,
    /// Track number within its disc, when the tags declared one.
    pub track_number: Option<u32>,
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
                    track_artist: track.artist.clone(),
                    album: album.title.clone(),
                    track_number: track.number,
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
        track_artist: None,
        album: None,
        track_number: None,
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
        Self {
            // No run at all, so nothing has been edited.
            x,
            width,
        }
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

/// What a click under a hovering pointer would ask for, ready to float over
/// the groove it was measured on.
///
/// One type for both grooves: the seek bar previews a timestamp and the
/// volume fader a level, and neither the placement arithmetic
/// ([`preview_offset`]) nor the lane that holds it cares which. The label is
/// already formatted — the view renders text, it does not compute it.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    /// What a click here would ask for, formatted for display.
    pub label: String,
    /// Distance from the bar's left edge, clamped onto the bar (logical px).
    pub x: f32,
    /// The bar's width the `x` was measured against (logical px).
    pub width: f32,
}

/// The two timestamps, render-ready — what used to flank a 260 px groove and
/// now sits beside the wall label in the bar's left zone (ADR-0017 §1.1).
///
/// The groove they were built for is gone; the *figures* are not, because
/// the product's standing rules permits replacing a slot with a better statement of the
/// same fact and not removing one. The needle states position; these state the
/// two numbers a line cannot: exactly how far in, and exactly how long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamps {
    /// Left timestamp: the position being shown — scrubbed, pending, or
    /// confirmed, in that order of precedence.
    pub elapsed: String,
    /// Right timestamp: the track's length, or `--:--` when undeclared.
    pub total: String,
    /// Whether the position shown is a *request* rather than a confirmed
    /// reading: the needle is being scrubbed, or a seek is awaiting its
    /// confirming event. The view marks it so the number is never mistaken
    /// for playback truth it has not earned yet.
    pub pending: bool,
}

/// The current song's render-ready seek line.
#[derive(Debug, Clone, PartialEq)]
pub struct NeedleBar {
    /// How far into the current song the playhead is, `0.0..=1.0`. Always 0
    /// when the track length is unknown — there is no proportion to show.
    pub position: f32,
    /// Whether the current song has a declared duration and can be sought.
    pub interactive: bool,
    /// Where the pointer is resting and what a click there would ask for —
    /// `None` unless the pointer is on a live needle with no scrub in
    /// progress (see the module's precedence rules).
    pub preview: Option<Preview>,
}

/// The placeholder shown where a track length would be, when the container
/// never declared one. Same width as a real `m:ss` so the bar does not jump.
const UNKNOWN_TOTAL: &str = "--:--";

/// How close to the top of the volume fader's travel counts as *at* the top,
/// in logical pixels.
///
/// Unity is the one position on this control that carries a guarantee — the
/// engine performs no arithmetic on the samples at all (ADR-0011 §5) — and on
/// a [`crate::theme::VOLUME_W`]-wide groove a single pixel is
/// ~10 control positions, so "very nearly at the top" is an easy place to land
/// and an invisible one to be in. Four pixels is the same figure
/// [`DRAG_THRESHOLD_PX`] uses and for the same reason: it is the scale of the
/// tremor a hand puts into a pointer, not the scale of an intention. Below the
/// snap the control is continuous; there is no second detent to fall into.
///
/// It is a *snap*, not a dead zone: positions inside it resolve to
/// [`MAX_POSITION`], which is the position the user was reaching for. The
/// resolution given up is the top 0.9 dB of a 60 dB taper.
pub const UNITY_SNAP_PX: f32 = 4.0;

/// One press of the volume keys, in control positions.
///
/// Chosen against the taper rather than against the widget. The cube's slope
/// is `d(dB)/d(position) = 60 / (position · ln 10)`, so at the top of the
/// travel 40 positions is **1.04 dB** — the smallest change a listener
/// reliably hears as one. Smaller and a press near unity would do nothing
/// audible; much larger and there would be no fine control where people
/// actually listen. It also means:
///
/// - 25 presses span the whole control, so Down-held reaches silence in about
///   a second of key repeat and Up-held returns.
/// - **40 divides 1000 exactly**, so stepping down and back up lands on
///   [`MAX_POSITION`] itself rather than on 999. Combined with the clamp at
///   the top, holding Up always ends at unity exactly — the keyboard can reach
///   the bit-perfect position as reliably as the snap can.
///
/// Lower down the same 40 positions is a coarser dB step (2.1 dB at
/// half-travel, 5.2 dB at a fifth); that is what a fader law *is*, and it is
/// the same curve the pointer feels.
pub const VOLUME_STEP: u16 = 40;

/// The volume control's render-ready state — a fader position, a mute glyph,
/// and a hover preview, with nothing about how to draw them.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "four independent facts about three different things — where the fader is, whether \
              the output is muted, whether there is an engine, and whether a mute command is in \
              flight. Folding them into a state machine would invent states the control does not \
              have (muted-and-inert-and-pending is a real combination) and put the enumeration \
              between this module and the view, which is the translation the type exists to avoid"
)]
pub struct VolumeBar {
    /// Handle position as a fraction of the fader's travel, `0.0..=1.0`.
    /// Linear in *travel*, not in amplitude: the taper lives in `baz-core`
    /// and the control shows where the control is.
    pub position: f32,
    /// Whether output is muted — separate state from [`Self::position`],
    /// exactly as the protocol keeps it (ADR-0011 §3).
    pub muted: bool,
    /// Whether the fader is sitting exactly on the unity detent.
    pub unity: bool,
    /// Whether the control can do anything: an engine to send to.
    pub interactive: bool,
    /// Whether a mute command is awaiting its confirming event. Spent on the
    /// speaker's ink and nothing else — the same fixed-size dim-and-return
    /// the transport buttons use, for the same reason.
    ///
    /// The *fader* needs no equivalent: a requested position is already
    /// visible as the handle sitting where the hand put it, which is the
    /// affordance the seek bar's amber timestamp stands in for.
    pub mute_pending: bool,
    /// The level a click under the pointer would set — `None` unless the
    /// pointer is resting on a live fader with no drag in progress.
    pub preview: Option<Preview>,
    /// The mute affordance's accessible name, naming the action a press
    /// would take rather than the state it is in.
    pub mute_label: &'static str,
}

/// Where one queue row sits relative to the one that is playing.
///
/// Three states rather than a pair of booleans, because they are exhaustive
/// and mutually exclusive, and because the view's whole job is to pick an ink
/// per state. When nothing is playing every row is [`Self::Upcoming`] — a
/// stopped queue has not been played, it is waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueRowState {
    /// **The pass has been past it**: it played, or the listener jumped over
    /// it. Behind the cursor *in the traversal*, which for an unshuffled run is
    /// exactly "above the playing row" and for a shuffled one is the honest
    /// answer a row number cannot give.
    Played,
    /// The row the engine last said it started.
    Playing,
    /// **The row that plays next** — the one fact this product would otherwise
    /// be concealing.
    ///
    /// The next track is chosen in advance (it has to be: the engine decodes it
    /// while the current one plays, `baz_core::traversal`), so baz *knows* it,
    /// and knowing something about what you are listening to and not saying it
    /// is not a thing this interface does. "Unknown" in *shuffle picks an
    /// unknown next track* describes how the choice is made, not a secret kept
    /// from the listener.
    ///
    /// With shuffle off it is the row under the playing one, which is where a
    /// reader's eye already was — the mark costs that case nothing and makes
    /// the shuffled case legible.
    Next,
    /// Still to come, and not next.
    Upcoming,
}

/// One row of the queue panel, render-ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRow {
    /// Position in the queue as the panel numbers it, from 1.
    pub position: usize,
    /// The track's title.
    pub title: String,
    /// Its own artist, when the album's header does not already cover it.
    pub artist: Option<String>,
    /// `m:ss`, or empty when the scan read no duration — never a `0:00` that
    /// would read as a real, very short track.
    pub duration: String,
    /// Where this row sits relative to what is playing.
    pub state: QueueRowState,
    /// The record this row **opens**, when it is the first row of one.
    ///
    /// `None` for every row that continues the record above it, and `None` for
    /// the queue's *first* record too — that one is named by
    /// [`QueueList::album`] and [`QueueList::artist`], which the popover already
    /// draws at the head of the list.
    ///
    /// This is what keeps ADR-0014's *"albums are listed as albums, never
    /// flattened"* true of a queue holding more than one of them. `Play all`
    /// stacks the whole wall; without a break per record the popover would print
    /// forty titles under one album's name, which is a flattening the data had
    /// not done.
    pub head: Option<QueueHead>,
}

/// The name of a record where it begins in the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueHead {
    /// Its title, when it has one. `None` is an unknown-album group, headed by
    /// its artist exactly as the wall's own label is.
    pub album: Option<String>,
    /// Who it is filed under, as the shelf labels it.
    pub artist: String,
}

/// The queue panel's render-ready state: what was queued, where the engine is
/// in it, and one summary line — with nothing about how to draw any of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueList {
    /// Title of the album the queue came from, when it has one.
    pub album: Option<String>,
    /// Who that album is filed under.
    pub artist: String,
    /// The one-line reading, led by the noun of its subject —
    /// `Run · 3 of 12 · 38:12 left` while something is playing,
    /// `Run · 12 tracks · 51:20` otherwise, with the run's provenance in the
    /// noun's place when it has one, and the time dropped when the scan read
    /// no durations to add up (see [`queue_summary`]).
    pub summary: String,
    /// The rows, in play order.
    pub rows: Vec<QueueRow>,
}

/// What a click on a track row of an album has to send to play from there.
///
/// Two answers, because ADR-0014 gives two commands and the difference is
/// audible. Which one applies is a question about the *queue the engine
/// holds*, not about the transport, so it is decided by
/// [`PlayerState::play_from`] from event-derived state and the request-side
/// queue record — never guessed at the call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayFrom {
    /// The engine is already holding exactly this album, so the click is a
    /// move within it: [`JumpTo`](baz_core::protocol::Command::JumpTo) alone.
    ///
    /// This is the case worth having. `JumpTo` keeps the queue the listener
    /// built, works from every transport state including stopped, and — unlike
    /// a re-queue — does not restart the run they are in the middle of.
    Jump {
        /// Zero-based queue position to play.
        position: usize,
    },
    /// The engine is holding something else (or nothing): the album has to be
    /// queued first, so this is
    /// [`SetQueue`](baz_core::protocol::Command::SetQueue) and then
    /// `JumpTo`.
    ///
    /// `SetQueue` is documented to stop what is playing, which is exactly
    /// right here — the listener asked for a different album — and the
    /// `JumpTo` that follows is what makes the click land on the row they
    /// pointed at rather than at the top of the album.
    Requeue {
        /// Zero-based position within the album to play once it is queued.
        position: usize,
    },
}

/// The chain the engine last reported, exactly as [`Event::SignalPath`]
/// stated it: what the file is, what the output is running at, and what (if
/// anything) sits between them.
///
/// Kept whole rather than reduced to a flag on arrival, because the reason a
/// conversion is in the path is what the note can actually explain — "this
/// device has no 48 kHz mode" and "the output is held at a fixed rate" are
/// different facts (ADR-0009 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalPath {
    /// Sample rate of the track playing, in Hz.
    pub source_rate_hz: u32,
    /// Channels the file carries, as distinct from the stereo the engine
    /// emits. Above `CHANNELS` means an ITU-R BS.775 downmix is in the path
    /// (ADR-0039), which is a conversion whatever `chain` says.
    pub source_channels: usize,
    /// Bit depth its container declares, when it declares one.
    pub source_bits: Option<u32>,
    /// Rate the output stream is running at, in Hz.
    pub output_rate_hz: u32,
    /// What the engine is doing between the two.
    pub chain: SignalChain,
}

/// The bottom bar's signal-path readout — present **only** while the engine
/// is converting (see the module's signal-path note).
///
/// Two strings and nothing else: no severity, no icon choice, no color. The
/// view has no decision left to make, which is what keeps the tone out of the
/// view layer's hands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalNote {
    /// The visible label: `48 → 44.1 kHz`, the chain in the same kHz spelling
    /// the side panel's encoding line uses. Short enough to read at a glance
    /// and factual enough to need no adjective.
    pub label: String,
    /// The hover sentence, naming the rate being played and why: "Playing at
    /// 44.1 kHz — this device has no 48 kHz mode".
    pub detail: String,
}

/// An actionable explanation of Baz's active sample-rate conversion.
///
/// Unlike [`SignalNote`], which is deliberately quiet enough for the resident
/// transport, this is warning copy for Settings and the canonical event log.
/// Keeping it beside the event-derived path makes both surfaces use exactly
/// the same rates, cause and remedy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalWarning {
    pub title: String,
    pub detail: String,
}

/// Session-scoped deduplication for the signal-path warning history.
#[derive(Debug, Default)]
pub struct SignalWarningState {
    active: Option<SignalPath>,
}

impl SignalWarningState {
    /// Observe the engine's newest path. Returns copy for the event history
    /// only when a Baz-owned conversion begins or materially changes.
    pub fn observe(&mut self, path: SignalPath) -> Option<SignalWarning> {
        let warning = path.warning();
        if warning.is_none() {
            self.active = None;
            return None;
        }
        if self.active == Some(path) {
            return None;
        }
        self.active = Some(path);
        warning
    }

    /// End the continuing condition when playback itself ends.
    pub fn clear(&mut self) {
        self.active = None;
    }
}

impl SignalPath {
    /// Describe an active Baz-owned boundary conversion. A direct path returns
    /// nothing even if an operating-system mixer may convert after Baz's
    /// stream: that downstream behavior is neither observed nor owned here.
    #[must_use]
    pub fn warning(self) -> Option<SignalWarning> {
        let reason = self.chain.conversion_reason()?;
        let source = vm::format_sample_rate(self.source_rate_hz);
        let output = vm::format_sample_rate(self.output_rate_hz);
        let remedy = match reason {
            ConversionReason::DeviceRateUnavailable => format!(
                "The selected audio device has no {source} mode. Select an output device that \
                 supports {source}, then restart Baz to restore a direct path."
            ),
            ConversionReason::FixedOutputRate => format!(
                "The output boundary is fixed at {output}. Use follow-source output instead to \
                 restore a direct path at the next track boundary."
            ),
            _ => "Choose a follow-source output path at the source rate to restore a direct path."
                .to_owned(),
        };
        Some(SignalWarning {
            title: format!("Baz is resampling {source} → {output}"),
            detail: format!(
                "{remedy} This warning describes Baz's boundary resampler; conversion later in \
                 the operating-system mixer is outside Baz and is not reported here."
            ),
        })
    }
}

/// The event-derived playback state behind every playback widget.
///
/// `PartialEq` but not `Eq`: pointer geometry is measured in floating-point
/// logical pixels, and there is no honest total equality over those.
///
/// It used to carry an `#[expect(clippy::struct_excessive_bools)]`, for a
/// fourth flag: whether shuffle was on. Shuffle is a `Traversal` now
/// (`baz_core::traversal`) rather than a boolean with a permutation behind it,
/// which takes the count back under the lint's threshold — the kind of
/// reduction that shows up as an allowance nobody needs any more.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "four, and each is a different kind of fact about the run rather \
              than a flag someone could fold into an enum: `pending` and \
              `mute_pending` are in-flight commands awaiting their event, \
              `muted` is engine state, and `queue_edited` is whether the run \
              has diverged from the file it came from. A state machine over \
              them would name a product of four independent axes"
)]
pub struct PlayerState {
    availability: Availability,
    phase: Phase,
    now_playing: Option<NowPlaying>,
    /// Path of the current track, to tell "the same track started again
    /// after a seek" from "a different track started".
    now_playing_path: Option<PathBuf>,
    /// Count of *distinct* tracks this state machine has seen start, so a
    /// consumer that must name the current track with a stable identity
    /// (MPRIS's `mpris:trackid`) has one that changes exactly when the track
    /// does — and never when a seek restarts the same file.
    track_seq: u64,
    /// The queue we last asked for (request-side; module docs). `None` until
    /// something has been queued at all.
    queue: Option<QueueVm>,
    /// Zero-based position in that queue the engine last said it started, for
    /// as long as a session is running. Engine truth, from
    /// [`Event::TrackStarted`] and nowhere else.
    queue_position: Option<usize>,
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
    /// The chain the engine last reported for the current session, if any.
    signal: Option<SignalPath>,
    /// The volume the engine last confirmed. Engine state, not session
    /// state: nothing about starting, stopping or skipping a track touches
    /// it (ADR-0011 §6), so it is deliberately absent from
    /// [`Self::reset_progress`] and from [`Self::engine_closed`]'s clearing.
    volume: Volume,
    /// Mute as the engine last confirmed it, independent of [`Self::volume`].
    muted: bool,
    /// Where the engine last said the volume is being applied.
    volume_path: VolumePath,
    /// The pointer gesture on the fader, if one is in progress.
    volume_gesture: Option<Gesture>,
    /// Where the pointer is resting on the fader, if it is.
    volume_hover: Option<Pointer>,
    /// A position a sent-but-unconfirmed
    /// [`Command::SetVolume`](baz_core::protocol::Command::SetVolume) asked
    /// for.
    volume_pending: Option<u16>,
    /// A [`Command::SetMute`](baz_core::protocol::Command::SetMute) awaiting
    /// its confirming event.
    mute_pending: bool,
    /// ReplayGain as the engine last reported it (ADR-0013). Engine state
    /// beside the volume, and folded here for the volume's reason: this is
    /// the one place engine events are allowed to change anything. The
    /// vocabulary and the arithmetic live in [`crate::replaygain`].
    replay_gain: ReplayGain,
    /// **The order the run is walked in** — the owner's decision, 2026-08-10:
    /// *"can you make shuffle a property of the player i.e. toggle on/off"*,
    /// and later the same day *"shuffle as a concept is more about going to an
    /// unknown next track rather than actually mutating the track list"*.
    ///
    /// A **standing decision**, not session state: the *mode* is persisted in
    /// `config.toml` beside ReplayGain, the group key and the density, and
    /// seeded at start-up by [`Self::seed_traversal`] for
    /// [`Self::seed_volume`]'s reason. Nothing about starting, stopping or
    /// skipping a track touches it, so it is deliberately absent from
    /// [`Self::reset_progress`] and from [`Self::engine_closed`].
    ///
    /// It is a property of the **player** rather than of a list, which is why
    /// it lives here: it governs `Play` on a record, `Play all`, a playlist's
    /// `Play` and a track click alike, and a flag hung on any one of those
    /// would have to be four flags that agreed.
    ///
    /// **The engine holds the same value**, which is the change of 2026-08-10:
    /// shuffle was a permutation this process applied to the paths it sent, and
    /// is now a traversal the engine is told about
    /// ([`Command::SetTraversal`](baz_core::protocol::Command::SetTraversal)),
    /// because only the engine knows the next track early enough to decode it
    /// (`baz_core::traversal`). What is kept here is a mirror of engine state,
    /// on exactly the terms [`Self::volume`] is.
    traversal: Traversal,
    /// Engine-confirmed Repeat current track property.
    repeat_one: bool,
    /// [`Self::traversal`] resolved against the queue this process last sent:
    /// the run's positions, in the order they will play.
    ///
    /// The same permutation the engine computed, from the same seed through the
    /// same public function — which is the whole reason
    /// [`Traversal::play_order`] is public. **Baz says what is next because it
    /// knows what is next**: this is what the run column's marks and the bar's
    /// continuation are read from, and there is no second answer for them to
    /// disagree with.
    ///
    /// A permutation of `0..queue.len()`, kept in step by every method that
    /// changes either input ([`Self::replan`]).
    order: Vec<usize>,
    /// **Whether the run has been edited since it was handed to the engine.**
    ///
    /// Set by [`Self::note_queue_edited`] and cleared by
    /// [`Self::note_queue_sent`] — the two calls that already draw ADR-0014's
    /// line between *a new run* and *an edit to the one sounding*. Its one
    /// reader is [`Self::run_origin`], which is what decides whether the run
    /// column's `Save as playlist` is an offer or a readout (ADR-0024 §A5.2).
    ///
    /// **Why this is not read off the undo history.** Design 14 §1.4 costed
    /// the fix as *"no new state"*, on the reasoning that `can_undo` already
    /// answers *"has this run been edited?"*. It does not, durably:
    /// `App::queue_undo` is cleared when the place is left
    /// (`App::note_place_left`), when the run column stands down
    /// (`App::set_run`) and when the run ends (`Event::QueueEnded`) — all
    /// three are P2's ends for an *edit history*, and none of them un-edits
    /// the run. Reading it here would make an edited run claim to be its
    /// source file again the moment you navigated away and back, which is the
    /// exact lie §A5 is removing. Divergence is a fact about the queue
    /// record, so it is kept beside the queue record.
    queue_edited: bool,
}

/// **What the run standing is** — the four states the summary strip has to
/// tell apart, and the reading its save word is drawn from.
///
/// Derived, never stored: the kind is the queue record's own
/// ([`crate::vm::RunSource`]) and divergence is
/// [`PlayerState::note_queue_edited`]'s. Nothing here is a *link* — ADR-0023
/// §3 makes provenance an origin, ADR-0024 §1 decouples the run from the file
/// in **both** directions, and no variant below ever writes one.
///
/// # The rule, in one sentence
///
/// > **The save word appears only for a run the listener assembled from
/// > nothing.**
///
/// The owner, 2026-08-10, in two passes. First the three kinds:
/// *"we should have playlists which are like 'fixed' i.e. a CD's track
/// listing and some which are unnamed i.e. when we just 'add to queue' and
/// some which are named i.e. we saved it already. the only one which has any
/// kind of indicator to save is the 2nd case."* Then the narrowing:
/// *"nah I think adding more stuff to an existing playlist is fine, that does
/// not need a save -- it's a low bar to edit a playlist"*.
///
/// | the run | the strip |
/// |---|---|
/// | [`Self::Fixed`] — a record, `All songs`, `Play all` | nothing |
/// | [`Self::Saved`] — reified from a file, unedited | `Saved as “Road Trip”` |
/// | [`Self::Diverged`] — reified from a file, since edited | `From “Road Trip”` |
/// | [`Self::Assembled`] — built by hand, or a fixed run since edited | `Save as playlist`, live |
///
/// **[`Self::Diverged`] stopped offering a new file**, which is the whole of
/// the second quote: a run that came from a playlist never needs a second
/// route to change one, because the playlist's own page is a cheap route to
/// the first. What it must **not** do is keep claiming to *be* that file — the
/// lie ADR-0024 §A5.2 removed — so it names its origin without asserting
/// identity, and that is why it carries a name where the variant it replaced
/// carried none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOrigin<'a> {
    /// **A list that already exists and is not the listener's to name** — a
    /// record's track listing, `All songs`, a `Play all`, a shuffle draw.
    ///
    /// No word at all. There is no creation act here: the list exists, and
    /// freezing it would make a second copy of something nobody assembled.
    Fixed,
    /// Reified from the named playlist file and unedited since — the run
    /// **is** that file, so the word is a readout rather than an offer.
    Saved(&'a str),
    /// Reified from the named file and edited since. A readout that names
    /// where the run came from and claims nothing about what it is now.
    Diverged(&'a str),
    /// **Assembled by the listener out of nothing** — the picker's `Queue`
    /// row, a track's `+`, or a fixed run they have since edited into
    /// something that exists nowhere else.
    ///
    /// The one state where freezing the run into a playlist is a genuine
    /// creation act (ADR-0024 §4), and therefore the one state with a live
    /// save word.
    Assembled,
}

impl PlayerState {
    /// Fresh state; `availability` is the engine spawn result.
    #[must_use]
    pub fn new(availability: Availability) -> Self {
        Self {
            // No run at all, so nothing has been edited.
            queue_edited: false,
            availability,
            phase: Phase::Stopped,
            now_playing: None,
            now_playing_path: None,
            track_seq: 0,
            queue: None,
            queue_position: None,
            pending: false,
            failed: 0,
            elapsed_ms: 0,
            track_ms: None,
            gesture: None,
            hover: None,
            seek_pending: None,
            signal: None,
            // A freshly spawned engine is at unity, unmuted, `Unity` — so
            // these are the right values before anybody has been asked
            // (ADR-0011, "The default is unaffected"). `seed_volume` replaces
            // them with the engine's own reading at start-up all the same,
            // because a second front end may have moved it already.
            volume: Volume::UNITY,
            muted: false,
            volume_path: VolumePath::Unity,
            volume_gesture: None,
            volume_hover: None,
            volume_pending: None,
            mute_pending: false,
            // Off, no pre-amp, clipping prevention armed — what a freshly
            // spawned engine is (ADR-0013 §2). `seed_replay_gain` replaces it
            // with the engine's own reading at start-up, for `seed_volume`'s
            // reason.
            replay_gain: ReplayGain::default(),
            // In order, over no run at all. `seed_traversal` replaces it from
            // the config at start-up, for `seed_volume`'s reason.
            traversal: Traversal::default(),
            repeat_one: false,
            order: Vec::new(),
        }
    }

    /// Seed the traversal from `config.toml` at start-up, so the control is lit
    /// on the first frame rather than on the first press —
    /// [`Self::seed_volume`]'s reason, for a standing decision that is baz's
    /// rather than the engine's.
    pub fn seed_traversal(&mut self, traversal: Traversal) {
        self.traversal = traversal;
        self.replan();
    }

    /// Whether shuffle is on — what the bar's crossed arrows ask.
    #[must_use]
    pub fn shuffle(&self) -> bool {
        self.traversal.is_shuffled()
    }

    /// Whether natural completion repeats the current queue entry.
    #[must_use]
    pub fn repeat_one(&self) -> bool {
        self.repeat_one
    }

    /// Seed the persisted standing mode before the first engine event.
    pub fn seed_repeat_one(&mut self, enabled: bool) {
        self.repeat_one = enabled;
    }

    /// Record the traversal the engine has been sent, and re-derive the plan.
    pub fn note_traversal(&mut self, traversal: Traversal) {
        self.traversal = traversal;
        self.replan();
    }

    /// Re-derive [`Self::order`] from the traversal and the run's length — the
    /// front-end half of the engine's own `replan`, and the same function.
    fn replan(&mut self) {
        let len = self.queue.as_ref().map_or(0, QueueVm::len);
        self.order = self.traversal.play_order(len);
    }

    /// **Where the pass begins** — the queue position a plain `Play` starts at.
    ///
    /// Zero for an unshuffled run, which is what "from the top" always meant;
    /// the bag's first entry otherwise. The engine parks at the same place for
    /// the same reason, so this is a reading rather than an instruction.
    #[must_use]
    pub fn first_of_the_pass(&self) -> usize {
        self.order.first().copied().unwrap_or(0)
    }

    /// **Where each queue position sits in the pass**: the inverse of
    /// [`Self::order`].
    ///
    /// Built once per reading rather than kept as a second field, because two
    /// vectors that have to agree are two vectors that can disagree — and the
    /// cost is one linear pass over a list the caller is about to walk anyway.
    fn slots(&self) -> Vec<usize> {
        let mut slots = vec![0; self.order.len()];
        for (slot, &at) in self.order.iter().enumerate() {
            if let Some(cell) = slots.get_mut(at) {
                *cell = slot;
            }
        }
        slots
    }

    /// **How far through the pass the run is, and what is still to come** —
    /// the reading every "what is next" surface is drawn from.
    ///
    /// Answers the playing entry's *slot* (not its row: under a shuffled pass
    /// the two differ, and it is the slot that means "the third of twelve") and
    /// the entries after it **in the order they will play**. `None` for a run
    /// nothing has started, whose whole pass is still to come.
    fn pass(&self) -> (Option<usize>, Vec<&QueueItemVm>) {
        let Some(queue) = self.queue.as_ref() else {
            return (None, Vec::new());
        };
        let cursor = self
            .playing_row()
            .and_then(|row| self.order.iter().position(|&at| at == row));
        let from = cursor.map_or(0, |slot| slot + 1);
        let rest = self
            .order
            .get(from..)
            .unwrap_or_default()
            .iter()
            .filter_map(|&at| queue.items.get(at))
            .collect();
        (cursor, rest)
    }

    /// Seed the volume from [`EngineHandle::volume`](baz_core::engine::EngineHandle::volume)
    /// at start-up — the pull-side snapshot ADR-0011 provides for exactly
    /// this moment, so the fader is right on the first frame instead of on
    /// the first change.
    ///
    /// A returned fact, not an assumption, in the same way
    /// [`Availability`] is seeded from the spawn result.
    ///
    /// Takes the three facts rather than the
    /// [`VolumeState`](baz_core::volume::VolumeState) carrying them:
    /// that type is `#[non_exhaustive]`, so a caller outside `baz-core` — a
    /// unit test here included — cannot build one, and a seeding path no test
    /// can exercise is a seeding path nobody has checked.
    pub fn seed_volume(&mut self, volume: Volume, muted: bool, path: VolumePath) {
        self.volume = volume;
        self.muted = muted;
        self.volume_path = path;
    }

    /// Seed ReplayGain from
    /// [`EngineHandle::replay_gain`](baz_core::engine::EngineHandle::replay_gain)
    /// at start-up — the same pull, for the same reason, as
    /// [`Self::seed_volume`], and the parts rather than the
    /// `#[non_exhaustive]` state type for the same reason too.
    pub fn seed_replay_gain(
        &mut self,
        settings: ReplayGainSettings,
        source: ReplayGainSource,
        applied_centidb: i16,
        clipping_prevented: bool,
    ) {
        self.replay_gain
            .seed(settings, source, applied_centidb, clipping_prevented);
    }

    /// Fold one engine event into the state. `albums` is the current shelf
    /// view model, used to resolve [`Event::TrackStarted`] paths.
    pub fn apply(&mut self, event: &Event, albums: &[AlbumVm]) {
        match event {
            Event::TrackStarted { path, position } => {
                self.phase = Phase::Playing;
                // Engine truth about where in the queue we are. Recorded on
                // every start, including the one a seek causes, because the
                // position is what the engine says it is (module docs).
                self.queue_position = Some(*position);
                // A seek restarts the *current* track, so TrackStarted is not
                // by itself news of a new track. Only a genuinely different
                // path resets the position; otherwise the bar would snap to
                // zero for the moment between a seek's confirming Progress
                // and the restarted track's audio arriving.
                if self.now_playing_path.as_deref() != Some(path.as_path()) {
                    self.now_playing_path = Some(path.clone());
                    self.track_seq = self.track_seq.wrapping_add(1);
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
                // The chain described a session; there is no session now.
                self.signal = None;
                // The queue itself survives — the engine keeps it, and a later
                // Play starts from the top — but nothing in it is playing, so
                // the panel marks no row.
                self.queue_position = None;
                self.reset_progress();
            }
            // The engine's own answer to "where is the playing track now",
            // after a queue it accepted changed shape (ADR-0014 §6). Taken in
            // preference to anything this side computed: the two differ exactly
            // when an edit races a track boundary, and the engine's is the one
            // the audio agrees with. `len` is not stored — the record this
            // process holds *is* the list it sent, so a mismatch would mean the
            // picture is stale, and the fix for that is to re-send the queue
            // rather than to patch a number.
            Event::QueueChanged { position, .. } => self.queue_position = *position,
            Event::TrackFailed { .. } => self.failed += 1,
            Event::Progress {
                elapsed_ms,
                track_ms,
            } => {
                self.elapsed_ms = *elapsed_ms;
                self.track_ms = *track_ms;
            }
            // Reported when a session starts and whenever the chain changes,
            // so each arrival simply replaces what was known (ADR-0009).
            Event::SignalPath {
                source_rate_hz,
                source_channels,
                source_bits,
                output_rate_hz,
                chain,
            } => {
                self.signal = Some(SignalPath {
                    source_rate_hz: *source_rate_hz,
                    source_channels: *source_channels,
                    source_bits: *source_bits,
                    output_rate_hz: *output_rate_hz,
                    chain: *chain,
                });
            }
            // The engine's own account of the volume, and the only thing
            // that moves it. What we asked for does not appear here at all:
            // a position we never sent (another front end, or a clamp) is
            // simply the truth from now on.
            Event::VolumeChanged {
                position,
                muted,
                path,
            } => {
                self.volume = Volume::new(*position);
                self.muted = *muted;
                self.volume_path = *path;
            }
            // ReplayGain's own account, folded by the module that owns its
            // vocabulary. It arrives on an accepted `SetReplayGain` *and* at
            // a track boundary where the resolved figure changes, and each
            // arrival replaces the whole reading (ADR-0013).
            Event::ReplayGainChanged { .. } => self.replay_gain.apply(event),
            Event::RepeatOneChanged { enabled } => self.repeat_one = *enabled,
            // `Event` is #[non_exhaustive]: tolerate unknown messages.
            _ => {}
        }
        // Any received event proves the engine made progress past whatever
        // we last sent (module docs). A gesture and a hover are the
        // pointer's business, not the engine's, so they survive.
        self.pending = false;
        self.seek_pending = None;
        self.volume_pending = None;
        self.mute_pending = false;
    }

    /// Forget where we were: a different track, or none at all.
    fn reset_progress(&mut self) {
        self.elapsed_ms = 0;
        self.track_ms = None;
        self.gesture = None;
        self.hover = None;
        self.seek_pending = None;
    }

    /// Record the queue that was just handed to the engine.
    ///
    /// The whole record, not merely its length: the engine echoes no event for
    /// [`SetQueue`](baz_core::protocol::Command::SetQueue), so this is the only
    /// account of what will play next that exists anywhere in the process (see
    /// the module's queue note). The previous queue's position goes with it —
    /// a position into a list that has been replaced means nothing.
    pub fn note_queue_sent(&mut self, queue: QueueVm) {
        self.queue = Some(queue);
        self.queue_position = None;
        self.failed = 0;
        // A new run is a new list, so the pass over it has a new length. The
        // *order* is untouched: it is the run the gesture built, and nothing
        // here permutes it.
        self.replan();
        // …and it is the list it was handed as, not a divergence from
        // anything. A run reified from a playlist file arrives here *as* that
        // file (see [`Self::run_origin`]).
        self.queue_edited = false;
    }

    /// Record an *edit* accepted by the engine's channel
    /// ([`UpdateQueue`](baz_core::protocol::Command::UpdateQueue)).
    ///
    /// The difference from [`Self::note_queue_sent`] is the whole of ADR-0014's
    /// bargain and it is one line: **the position survives.** `SetQueue` is the
    /// reset — it stops the music, so a position into what was playing means
    /// nothing afterwards — where an edit that does not touch the playing track
    /// does not disturb one delivered sample, and dropping the position here
    /// would blank the dot and the bar's `3 / 12` for the moment between the
    /// send and the engine's answer.
    ///
    /// It is not left *stale* either. The index may well have moved — remove
    /// two rows above the playing one and it is renumbered — and the engine
    /// re-derives the truth and announces it as
    /// [`Event::QueueChanged`], which [`Self::apply`] takes. In the gap between
    /// the two, the row is still found by *path*
    /// ([`QueueVm::playing`]), which is the same reconciliation every other
    /// reading in this module uses. So the mark is right before the event, and
    /// right after it, for two different reasons.
    ///
    /// The skipped-track count is deliberately not reset: an edit is not a new
    /// run, and the files that failed in this one still failed.
    pub fn note_queue_edited(&mut self, queue: QueueVm) {
        self.queue = Some(queue);
        // An edit can change the run's length, and the pass is a permutation of
        // its positions — so the plan is re-derived here exactly as the engine
        // re-derives its own on the `UpdateQueue` that carried the edit.
        self.replan();
        // And the run has diverged from whatever it was reified from, which is
        // permanent for this run: provenance is origin, never a live link
        // (ADR-0023 §3), so nothing here ever writes back and nothing ever
        // decides the run has become its file again.
        self.queue_edited = true;
    }

    /// Record the same edit while pinning one absolute row as the traversal's
    /// immediate successor, mirroring `Command::UpdateQueueNext`.
    pub fn note_queue_edited_next(&mut self, queue: QueueVm, next: usize) {
        let current_path = self.now_playing_path.clone();
        self.note_queue_edited(queue);
        let current = current_path.as_deref().and_then(|path| {
            self.queue
                .as_ref()
                .and_then(|queue| queue.items.iter().position(|item| item.path == path))
        });
        baz_core::traversal::force_next(&mut self.order, current, next);
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
        self.signal = None;
        // The list of what we queued is our own memory and stays true; where
        // the engine was in it is a fact only a running engine can supply.
        self.queue_position = None;
        self.reset_progress();
        // The volume itself survives — it is engine state, and the last
        // reading remains the honest answer to "where is the fader" — but a
        // gone engine cannot confirm anything, so nothing may stay pending
        // and the pointer has nothing left to hold.
        self.volume_gesture = None;
        self.volume_hover = None;
        self.volume_pending = None;
        self.mute_pending = false;
    }

    /// The pointer is resting on the bar at `pointer` with no button held:
    /// record it so the view can preview the timestamp a click would land
    /// on.
    ///
    /// A no-op (and a *clearing* one) when the bar is not seekable — without
    /// a track length there is no honest number to preview.
    pub fn hover_to(&mut self, pointer: Pointer) {
        self.hover = self.needle_live().then_some(pointer);
    }

    /// The pointer left the bar: the preview goes with it.
    pub fn hover_left(&mut self) {
        self.hover = None;
    }

    /// The bar was pressed at `pointer` — the start of a gesture that stays
    /// a click until it travels [`DRAG_THRESHOLD_PX`] (module docs). Nothing
    /// is requested and nothing on the bar moves yet.
    ///
    /// A no-op when the current song has no declared duration, because there
    /// is no honest position for a seek to land on.
    pub fn press(&mut self, pointer: Pointer) {
        if !self.needle_live() {
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
        let landing = if gesture.scrubbing {
            gesture.latest
        } else {
            gesture.anchor
        };
        // The pointer is demonstrably wherever the release left it: keep
        // previewing from there when that is still on the needle, and show
        // nothing when the release happened off the end of a scrub.
        self.hover = gesture.latest.is_over().then_some(gesture.latest);
        let position_ms = self.needle_target(landing)?;
        self.seek_pending = Some(position_ms);
        Some(position_ms)
    }

    /// Whether the current song has an honest pointer action.
    fn needle_live(&self) -> bool {
        self.seekable_total().is_some()
    }

    /// What a press at `pointer` is asking for — the needle's one decision,
    /// made here rather than in the widget (ADR-0006's layer boundary, and the
    /// same split [`crate::groove`] has always had).
    ///
    /// `None` when the current song has no declared duration, because a
    /// position would be a proportion of a number nobody has.
    fn needle_target(&self, pointer: Pointer) -> Option<u64> {
        Some(scale(self.seekable_total()?, pointer.fraction()))
    }

    /// The song seek line's render-ready state. Always answers — a line with
    /// nothing seekable draws its unfilled track and refuses the pointer, because a line
    /// that came and went with the music would be movement in the one place
    /// ADR-0020 forbids it.
    #[must_use]
    pub fn needle_bar(&self) -> NeedleBar {
        let total = self.seekable_total();
        // **The fill follows playback, a seek that has been asked for, and a
        // scrub — in that order of precedence.** Nothing here is a tween: the fill moves
        // because data arrived or a hand moved it, which is the distinction
        // the product's standing rules draws and ADR-0020 forbids crossing.
        let shown = self
            .scrub_ms()
            .or(self.seek_pending)
            .unwrap_or(self.elapsed_ms);
        NeedleBar {
            position: total.map_or(0.0, |total| fraction(shown, total)),
            interactive: self.needle_live(),
            preview: self.preview(),
        }
    }

    /// Seek `delta_ms` from where the bar is currently *showing* — the
    /// keyboard's and MPRIS's relative seek.
    ///
    /// Returns the absolute position to ask the engine for, recorded as
    /// pending exactly like a released drag, or `None` when there is nothing
    /// to seek within (the same [`Self::seekable_total`] test the bar uses:
    /// no engine, nothing playing, or a track of undeclared length — there is
    /// no honest "5 seconds from here" without a here).
    ///
    /// The base is the shown position rather than the last confirmed one, so
    /// three quick presses of Right move fifteen seconds instead of five: the
    /// engine reports progress at ~4 Hz, and a step taken from a reading that
    /// is up to a quarter-second stale would silently discard every press
    /// that landed inside the same reporting window. That is the same
    /// scrub → pending-seek → confirmed-progress precedence
    /// [`Self::stamps`] renders, so the number a press moves from is
    /// always the number the user was looking at.
    ///
    /// Backwards is clamped at zero. Forwards is clamped at the track length,
    /// where [`Command::Seek`](baz_core::protocol::Command::Seek) is
    /// documented to behave as [`Command::Next`](baz_core::protocol::Command::Next)
    /// — seeking off the end of a track moves to the next one, which is what
    /// holding the key means.
    pub fn seek_by(&mut self, delta_ms: i64) -> Option<u64> {
        let total = self.seekable_total()?;
        let base = self
            .scrub_ms()
            .or(self.seek_pending)
            .unwrap_or(self.elapsed_ms);
        let target = base.saturating_add_signed(delta_ms).min(total);
        self.seek_pending = Some(target);
        Some(target)
    }

    /// Seek to an absolute position (MPRIS `SetPosition`), clamped into the
    /// current track. `None` under the same conditions as [`Self::seek_by`].
    pub fn seek_to(&mut self, position_ms: u64) -> Option<u64> {
        let total = self.seekable_total()?;
        let target = position_ms.min(total);
        self.seek_pending = Some(target);
        Some(target)
    }

    // -----------------------------------------------------------------
    // The volume fader
    // -----------------------------------------------------------------

    /// The pointer is resting on the fader at `pointer`: record it so the
    /// view can preview the level a click would set. Cleared when the
    /// control is not live.
    pub fn hover_volume(&mut self, pointer: Pointer) {
        self.volume_hover = self.engine_ready().then_some(pointer);
    }

    /// The pointer left the fader; the preview goes with it.
    pub fn volume_left(&mut self) {
        self.volume_hover = None;
    }

    /// The fader was pressed at `pointer`. Returns the position to ask the
    /// engine for — a fader answers at once, so the press *is* the request
    /// (module docs) — or `None` when there is no engine to ask.
    pub fn press_volume(&mut self, pointer: Pointer) -> Option<u16> {
        if !self.engine_ready() {
            return None;
        }
        self.volume_gesture = Some(Gesture {
            anchor: pointer,
            latest: pointer,
            scrubbing: false,
        });
        let target = position_for(pointer);
        self.volume_pending = Some(target);
        Some(target)
    }

    /// The pointer moved to `pointer` with the fader held. Returns a
    /// position to ask for once the gesture has travelled
    /// [`DRAG_THRESHOLD_PX`] from the press, and `None` before that — so a
    /// click cannot smear a few pixels into a level nobody aimed at, while a
    /// real drag is heard as it happens.
    pub fn drag_volume(&mut self, pointer: Pointer) -> Option<u16> {
        let gesture = self.volume_gesture.as_mut()?;
        gesture.latest = pointer;
        if (pointer.x - gesture.anchor.x).abs() >= DRAG_THRESHOLD_PX {
            gesture.scrubbing = true;
        }
        if !gesture.scrubbing {
            return None;
        }
        let target = position_for(pointer);
        self.volume_pending = Some(target);
        Some(target)
    }

    /// The fader was released. Nothing new is requested — every position
    /// this gesture asked for went out as it happened — so this only ends
    /// the gesture and leaves the preview wherever the pointer actually is.
    pub fn release_volume(&mut self) {
        let Some(gesture) = self.volume_gesture.take() else {
            return;
        };
        self.volume_hover = gesture.latest.is_over().then_some(gesture.latest);
    }

    /// Whether the pointer still owns the volume fader.
    ///
    /// The shell uses this only to coalesce persistence: engine confirmations
    /// keep drawing the drag live, but `config.toml` is written when the hand
    /// lets go rather than once per pixel crossed.
    #[must_use]
    pub fn volume_gesture_active(&self) -> bool {
        self.volume_gesture.is_some()
    }

    /// Step the volume by `steps` × [`VOLUME_STEP`] positions — the
    /// keyboard's Up and Down. Returns the position to ask for, clamped into
    /// the control's travel, or `None` with no engine to ask.
    ///
    /// The base is the position a request is already in flight for when
    /// there is one, so presses inside one round trip accumulate instead of
    /// each landing on the same confirmed reading — the same rule, and the
    /// same reason, as [`Self::seek_by`].
    pub fn step_volume(&mut self, steps: i32) -> Option<u16> {
        if !self.engine_ready() {
            return None;
        }
        let base = i32::from(self.volume_pending.unwrap_or(self.volume.position()));
        let delta = steps.saturating_mul(i32::from(VOLUME_STEP));
        let target = base.saturating_add(delta).clamp(0, i32::from(MAX_POSITION));
        let target = u16::try_from(target).unwrap_or(MAX_POSITION);
        self.volume_pending = Some(target);
        Some(target)
    }

    /// Set the volume to an absolute position (MPRIS `Volume`), clamped into
    /// the control's travel. `None` with no engine to ask.
    pub fn set_volume(&mut self, position: u16) -> Option<u16> {
        if !self.engine_ready() {
            return None;
        }
        let target = Volume::new(position).position();
        self.volume_pending = Some(target);
        Some(target)
    }

    /// What a press of the mute affordance should ask for: the opposite of
    /// the *confirmed* mute state.
    ///
    /// The command is idempotent rather than a toggle (ADR-0011 §3), so this
    /// resolves the toggle against what the engine last said — never against
    /// a flag we flipped ourselves, which is how two front ends on one engine
    /// come to disagree about which way "toggle" points.
    pub fn toggle_mute(&mut self) -> Option<bool> {
        self.set_muted(!self.muted)
    }

    /// Ask for an absolute mute state (MPRIS, and the toggle above). `None`
    /// with no engine to ask.
    pub fn set_muted(&mut self, muted: bool) -> Option<bool> {
        if !self.engine_ready() {
            return None;
        }
        self.mute_pending = true;
        Some(muted)
    }

    /// The volume control's render-ready state.
    ///
    /// Always present, unlike [`Self::stamps`]: a fader has something to
    /// say whether or not anything is playing, because the volume is engine
    /// state and outlives every session. With no engine it renders inert
    /// rather than vanishing, which is what keeps the bottom bar's right-hand
    /// end the same width from launch onward.
    #[must_use]
    pub fn volume_bar(&self) -> VolumeBar {
        let shown = self.volume_gesture_position().or(self.volume_pending);
        let position = shown.unwrap_or(self.volume.position());
        VolumeBar {
            position: travel(position),
            muted: self.muted,
            unity: position == MAX_POSITION,
            interactive: self.engine_ready(),
            mute_pending: self.mute_pending,
            preview: self.volume_preview(),
            mute_label: if self.muted { "Unmute" } else { "Mute" },
        }
    }

    /// The position under the pointer while a fader drag is engaged.
    fn volume_gesture_position(&self) -> Option<u16> {
        let gesture = self.volume_gesture.filter(|gesture| gesture.scrubbing)?;
        Some(position_for(gesture.latest))
    }

    /// The hover preview: the level a click under the pointer would set.
    /// Suppressed while dragging, for the seek bar's reason — one pointer,
    /// one number.
    fn volume_preview(&self) -> Option<Preview> {
        if !self.engine_ready() || self.volume_gesture.is_some_and(|g| g.scrubbing) {
            return None;
        }
        let hover = self.volume_hover?;
        let width = hover.usable_width();
        Some(Preview {
            label: level_label(position_for(hover)),
            // From the same clamped geometry the label came from, so the
            // marker and the level can never disagree about where the
            // pointer is — including past either end of the travel.
            x: hover.fraction() * width,
            width,
        })
    }

    /// The volume as the engine last confirmed it.
    #[must_use]
    pub fn volume(&self) -> Volume {
        self.volume
    }

    /// Whether output is muted, as the engine last confirmed it.
    #[must_use]
    pub fn muted(&self) -> bool {
        self.muted
    }

    /// Whether baz is, right now, putting the decoder's samples on the wire
    /// unaltered — ADR-0011's amendment to ADR-0009, spelled once.
    ///
    /// Both halves are required and neither is inferred: the chain must be
    /// [`SignalChain::Direct`] (no rate conversion) **and** the volume path
    /// must be [`VolumePath::is_transparent`] (no gain stage). A session that
    /// has reported no chain answers `false` — the honest reading of "we have
    /// not been told" is not "yes".
    #[must_use]
    pub fn bit_exact(&self) -> bool {
        self.signal
            .is_some_and(|path| path.chain == SignalChain::Direct)
            && self.volume_path.is_transparent()
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
    pub fn stamps(&self) -> Option<Stamps> {
        if !self.engine_ready() || self.now_playing.is_none() {
            return None;
        }
        let shown = self
            .scrub_ms()
            .or(self.seek_pending)
            .unwrap_or(self.elapsed_ms);
        let pending = self.dragging() || self.seek_pending();
        let total = self.seekable_total();
        Some(Stamps {
            elapsed: format_ms(total.map_or(shown, |total| shown.min(total))),
            total: total.map_or_else(|| UNKNOWN_TOTAL.to_owned(), format_ms),
            pending,
        })
    }

    /// The queue place's render-ready state, or `None` when nothing has been
    /// queued in this session — in which case the place says so rather than
    /// drawing an empty list.
    ///
    /// # A view over engine truth, whose rows are now controls
    ///
    /// This function is still only a *reading* — it decides nothing and
    /// sends nothing — but the commands its doc once recorded as missing
    /// exist (ADR-0014 closed the engine half), and the place spends them
    /// on every row: a click is
    /// [`JumpTo`](baz_core::protocol::Command::JumpTo) (the row's index *is*
    /// a queue position, because these rows are drawn from the record of
    /// what was sent), and the ✕ and ▲▼ go out as whole-list
    /// [`UpdateQueue`](baz_core::protocol::Command::UpdateQueue)s through
    /// [`crate::queue_edit`]'s pure ops — an edit that does not touch the
    /// playing track disturbs no delivered sample, which is why removing or
    /// reordering under playback is ordinary here (doc 09 §8.2's edit
    /// parity).
    #[must_use]
    pub fn queue_list(&self) -> Option<QueueList> {
        let queue = self.queue.as_ref()?;
        let playing = self.playing_row();
        // **The list keeps the order the gesture gave it, always.** What the
        // traversal decides is which row is marked, never where a row sits —
        // that is the whole of the owner's *"shuffle… rather than actually
        // mutating the track list"*, drawn.
        let (cursor, rest) = self.pass();
        let slots = self.slots();
        // What plays next: the slot after the cursor, or the first slot of the
        // pass when nothing has started yet.
        let next = cursor.map_or(0, |slot| slot + 1);
        let rows = queue
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| QueueRow {
                position: index + 1,
                title: item.title.clone(),
                artist: item.artist.clone(),
                duration: item.duration.map(vm::format_duration).unwrap_or_default(),
                state: match (playing, slots.get(index).copied()) {
                    (Some(current), _) if current == index => QueueRowState::Playing,
                    (_, Some(slot)) if slot == next => QueueRowState::Next,
                    (_, Some(slot)) if cursor.is_some_and(|at| slot < at) => QueueRowState::Played,
                    _ => QueueRowState::Upcoming,
                },
                // A record's name where the record begins — and never on the
                // first row, whose record the list's own header already names.
                head: index
                    .checked_sub(1)
                    .and_then(|before| queue.items.get(before))
                    .filter(|previous| {
                        previous.album != item.album || previous.album_artist != item.album_artist
                    })
                    .map(|_| QueueHead {
                        album: item.album.clone(),
                        artist: item
                            .album_artist
                            .clone()
                            .unwrap_or_else(|| queue.artist.clone()),
                    }),
            })
            .collect();
        Some(QueueList {
            album: queue.album.clone(),
            artist: queue.artist.clone(),
            summary: queue_summary(
                queue,
                cursor,
                playing.and_then(|row| queue.items.get(row)),
                &rest,
                self.elapsed_ms,
            ),
            rows,
        })
    }

    /// The bar's **ambient continuation** — `then 2 albums · 1:58:00 left`, or
    /// `None` when the queue ends with the track that is playing.
    ///
    /// The line the critique specified for the bar's left zone and this
    /// codebase shipped without: *"Wall label bottom-left: Title — Artist ·
    /// elapsed **+ stack status when queued**"*. Without it the only route to
    /// what is coming was opening the popover, which makes knowing cost a
    /// click; the popover's job is *manipulating* the queue, and knowing what
    /// is next should cost nothing at all.
    ///
    /// The wording, every case it covers, and why the rest of the record now
    /// playing is counted rather than named are in [`continuation`]. What
    /// belongs here is where the answer comes from:
    ///
    /// - **The queue record this process sent** plus **the engine's own
    ///   confirmed position** ([`Self::playing_row`]) — the identical pair the
    ///   popover's rows and its summary are drawn from, so the ambient line and
    ///   the list can never disagree.
    /// - **Never optimistic.** No confirmed position, no line: a queue that has
    ///   been sent but has not started says nothing about what follows "this
    ///   track", because no track is this track yet. The same holds for a
    ///   `TrackStarted` naming a file this queue does not hold, and for a run
    ///   that has ended.
    #[must_use]
    pub fn continuation_note(&self) -> Option<String> {
        let queue = self.queue.as_ref()?;
        let current = queue.items.get(self.playing_row()?)?;
        let (_, rest) = self.pass();
        continuation(current, &rest, self.elapsed_ms, self.shuffle())
    }

    /// The queue this process handed the engine, if it has handed it one.
    ///
    /// The record an edit is computed *from*: [`crate::queue_edit`] takes it,
    /// returns the list the gesture means, and the caller sends that list's
    /// paths and hands the edited record back through
    /// [`Self::note_queue_edited`]. Borrowed rather than cloned because the
    /// ordinary answer to a click is "no edit" — a row that is not there.
    #[must_use]
    pub fn queue(&self) -> Option<&QueueVm> {
        self.queue.as_ref()
    }

    /// The run's **playing provenance**: the playlist file's name the queue
    /// was reified from, while that origin stands
    /// (`docs/design/09-implicit-playlists.md` §6).
    ///
    /// Session state about the *queue record*, read for two surfaces: the
    /// picker hoists the named list second, marked *playing*, and the queue
    /// place's summary leads with the name. `None` the moment a queue from
    /// any other gesture replaces the run — the record carries its own
    /// origin, so this is a read, not a bookkeeping site.
    #[must_use]
    pub fn queue_provenance(&self) -> Option<&str> {
        self.queue.as_ref()?.provenance()
    }

    /// The list identity carried by the current run, when it has one.
    #[must_use]
    pub fn queue_origin(&self) -> Option<&crate::origin::Origin> {
        self.queue.as_ref()?.origin.as_ref()
    }

    /// **What the run is** — the reading the run column's summary strip is
    /// drawn from ([`RunOrigin`] carries the rule and the owner's words).
    ///
    /// The record's own kind ([`crate::vm::RunSource`]) and the edit flag, put
    /// together in one place so that no view has to re-derive it.
    ///
    /// **The kind is what this used to be missing.** The reading was
    /// `queue_provenance()` — the playlist file's name, or nothing — and it
    /// could not tell a list that exists without a file from a list that does
    /// not exist at all, so a record's run and a hand-built queue were the
    /// same `Unfiled` and both were offered the creation act. That is the
    /// owner's *"I still see save as playlist on the queue when playing a
    /// CD"*, and the fix is upstream of every view.
    ///
    /// **A fixed run that has been edited is assembled.** An album's listing
    /// with two tracks pulled out of it is no longer the album's listing and
    /// there is no file to go and edit instead, so the creation act is the
    /// only route left and it is offered. That asymmetry with
    /// [`RunOrigin::Diverged`] is the owner's own: a named run has a cheap
    /// route to the file, and a fixed one has none.
    #[must_use]
    pub fn run_origin(&self) -> RunOrigin<'_> {
        let Some(queue) = self.queue.as_ref() else {
            return RunOrigin::Fixed;
        };
        match (&queue.source, self.queue_edited) {
            (vm::RunSource::Playlist(name), false) => RunOrigin::Saved(name),
            (vm::RunSource::Playlist(name), true) => RunOrigin::Diverged(name),
            (vm::RunSource::Assembled, _) | (vm::RunSource::Fixed, true) => RunOrigin::Assembled,
            (vm::RunSource::Fixed, false) => RunOrigin::Fixed,
        }
    }

    /// The sounding row's queue position, for the surfaces whose *subject*
    /// is the sounding track rather than a listed row — today the bar's
    /// context menu (doc 09 §5.2), whose transfer items spend the queue
    /// row's own messages on the row the engine confirms is playing.
    ///
    /// Exactly [`Self::playing_row`]'s answer, published: never optimistic,
    /// `None` while nothing is confirmed sounding, so the menu's verbs
    /// withdraw rather than aiming at a row that is not there.
    #[must_use]
    pub fn playing_queue_row(&self) -> Option<usize> {
        self.playing_row()
    }

    /// Which row of the recorded queue the engine is playing, reconciled
    /// against the path it named (module docs; [`QueueVm::playing`] carries the
    /// rule). Requires both an engine-reported position and a current track,
    /// so a stopped or replaced session marks nothing.
    fn playing_row(&self) -> Option<usize> {
        let queue = self.queue.as_ref()?;
        let path = self.now_playing_path.as_deref()?;
        queue.playing(self.queue_position?, path)
    }

    /// Which row of `tracks` is sounding — `None` unless those tracks are
    /// **exactly** the queue that is playing.
    ///
    /// This is what lets the album inspector carry the lamp dot the queue
    /// panel carries, which is the whole of "the inspector is a now-playing
    /// view of an album": for the only queue baz can build today, the album
    /// listed and the album queued are the same twelve rows, and the one thing
    /// the inspector failed to say was which of them was sounding.
    ///
    /// Two conditions, and both are load-bearing:
    ///
    /// 1. **The listed tracks are the queue** ([`QueueVm::holds_exactly`]) —
    ///    so an inspector showing a *different edition* of the album that is
    ///    playing marks nothing, rather than putting the dot on a file the
    ///    engine is not reading.
    /// 2. **Something is actually playing** ([`Self::playing_row`]) — engine
    ///    truth from [`Event::TrackStarted`], reconciled against the recorded
    ///    queue by path. A stopped or ended run marks no row, exactly as the
    ///    queue panel does.
    ///
    /// The index is a position in the queue, and the queue *is* the list, so
    /// it is a row of `tracks` by construction — including when the album
    /// lists one file twice, where the two occurrences stay distinguishable
    /// because neither side collapses them.
    #[must_use]
    pub fn playing_row_in(&self, tracks: &[vm::TrackVm]) -> Option<usize> {
        if !self.queue.as_ref()?.holds_exactly(tracks) {
            return None;
        }
        self.playing_row()
    }

    /// What a click on row `row` of `tracks` has to send (ADR-0014).
    ///
    /// `None` when the row is not in the list at all — a click on a stale
    /// picture asks for nothing rather than for something else.
    ///
    /// The decision is one question: **is the engine already holding exactly
    /// this album?** If it is, the click is a move within the queue the
    /// listener already has, and
    /// [`JumpTo`](baz_core::protocol::Command::JumpTo) alone is both
    /// sufficient and the only answer that does not disturb the queue —
    /// `SetQueue` would stop the music to hand the engine the list it is
    /// already playing. If it is not, the album has to be queued before a
    /// position in it means anything, so the pair goes out.
    ///
    /// Note what is deliberately *not* consulted: the transport. `JumpTo`
    /// starts a stopped engine, moves and resumes a paused one, and restarts
    /// the row it is aimed at — so a queue that has ended still takes a plain
    /// jump, and nothing here needs to know which of those it is asking for.
    #[must_use]
    pub fn play_from(&self, tracks: &[vm::TrackVm], row: usize) -> Option<PlayFrom> {
        if row >= tracks.len() {
            return None;
        }
        let held = self
            .queue
            .as_ref()
            .is_some_and(|queue| queue.holds_exactly(tracks));
        Some(if held {
            PlayFrom::Jump { position: row }
        } else {
            PlayFrom::Requeue { position: row }
        })
    }

    /// The position under the pointer while a scrub is engaged.
    fn scrub_ms(&self) -> Option<u64> {
        let gesture = self.gesture.filter(|gesture| gesture.scrubbing)?;
        self.needle_target(gesture.latest)
    }

    /// The hover preview: what a click under the pointer would seek to.
    /// Suppressed while scrubbing — the bar itself already shows that
    /// target, and two numbers chasing one pointer is noise.
    fn preview(&self) -> Option<Preview> {
        if self.dragging() {
            return None;
        }
        let hover = self.hover?;
        let width = hover.usable_width();
        let total = self.seekable_total()?;
        Some(Preview {
            label: format_ms(scale(total, hover.fraction())),
            // Derived from the (clamped) fraction rather than from `x`, so
            // the tip and the label can never disagree about where the pointer
            // is — including past either end of the line.
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
    #[cfg(test)]
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

    /// Confirmed position in the current track, in milliseconds — the last
    /// [`Event::Progress`] and nothing else. Never extrapolated between
    /// reports: a consumer that wants a smoother number must say so itself.
    #[must_use]
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }

    /// Confirmed length of the current track, when the engine reported one.
    #[must_use]
    pub fn track_ms(&self) -> Option<u64> {
        self.track_ms
    }

    /// A count that changes exactly when the playing track changes, for
    /// consumers that must give the current track a stable identity (module
    /// docs on [`NowPlaying`]).
    #[must_use]
    pub fn track_seq(&self) -> u64 {
        self.track_seq
    }

    /// **Whether anything has sounded in this process at all.** True from the
    /// first [`Event::TrackStarted`] onwards, and never false again — not on a
    /// stop, not when the queue ends, not when the engine goes away.
    ///
    /// The one question that separates *the run restored at launch and not yet
    /// begun* from *the run that has just played to its end*. Every other
    /// reading of this state machine gives those two states the same answers —
    /// the phase is [`Phase::Stopped`], the queue is loaded, and no row is
    /// playing in either — and the difference between them is the listener's
    /// place: the first must leave `session.toml` exactly as it found it, and
    /// the second must clear it (see `crate::app::next_snapshot`). It is also
    /// what `crate::views::home` reads to tell *nothing has begun* from
    /// *nothing is left*.
    ///
    /// It is [`Self::track_seq`] read as what its own documentation says it is
    /// — a **count** of distinct tracks started — rather than a second field
    /// holding the same fact. The counter is `wrapping_add`ed so that a very
    /// long-lived session cannot panic on overflow, never because the count is
    /// expected to come back round: at a track a millisecond it would take
    /// some six hundred million years.
    #[must_use]
    pub fn has_sounded(&self) -> bool {
        self.track_seq > 0
    }

    /// Path of the track the engine last said it started, while one is
    /// playing or paused.
    #[must_use]
    pub fn now_playing_path(&self) -> Option<&Path> {
        self.now_playing_path.as_deref()
    }

    /// Whether a seek can be asked for at all: an engine, a session, and a
    /// declared track length. The public spelling of the test the seek bar
    /// and [`Self::seek_by`] share.
    #[must_use]
    pub fn can_seek(&self) -> bool {
        self.seekable_total().is_some()
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

    /// What the play/pause toggle offers: the action a press would request,
    /// from the *confirmed* phase and nothing else.
    ///
    /// A transport command in flight does not change the answer — see the
    /// module's pending note for why swapping it mid-flight was the bottom
    /// bar's flash.
    #[must_use]
    pub fn play_pause(&self) -> PlayPause {
        match self.phase {
            Phase::Playing => PlayPause::Pause,
            Phase::Paused | Phase::Stopped => PlayPause::Play,
        }
    }

    /// Whether a transport command is awaiting its confirming event. The
    /// view spends this on a glyph opacity and nothing else — it changes no
    /// size, no label, and no enabled state (module docs).
    #[must_use]
    pub fn transport_pending(&self) -> bool {
        self.pending
    }

    /// Whether the play/pause toggle does anything: engine running, and a
    /// queue to (re)start when stopped.
    #[must_use]
    pub fn play_pause_enabled(&self) -> bool {
        self.engine_ready() && (self.queued() > 0 || self.phase != Phase::Stopped)
    }

    /// How many tracks are in the queue we last sent.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.queue.as_ref().map_or(0, QueueVm::len)
    }

    /// Whether Next does anything (it is a documented engine no-op while
    /// stopped).
    #[must_use]
    pub fn next_enabled(&self) -> bool {
        self.engine_ready() && self.phase != Phase::Stopped
    }

    /// Whether Previous does anything.
    ///
    /// The same condition as [`Self::next_enabled`], and for the same reason:
    /// both are *relative* commands and both are documented engine no-ops
    /// while stopped, because there is no current track to step from.
    ///
    /// It is not the same *availability*, though, and the protocol says so:
    /// `Next` runs out at the end of the queue, while `Previous` has no
    /// position at which it does nothing — at the head of the queue, and past
    /// [`PREVIOUS_RESTART_MS`](baz_core::engine::PREVIOUS_RESTART_MS) into any
    /// track, it restarts what is playing. So a running queue can advertise
    /// this unconditionally, which is what `CanGoPrevious` now reports.
    #[must_use]
    pub fn previous_enabled(&self) -> bool {
        self.engine_ready() && self.phase != Phase::Stopped
    }

    /// The bar's replacement line when there is no engine to report on;
    /// `None` while one is running (or when the bar is hidden entirely).
    #[must_use]
    pub fn availability_note(&self) -> Option<String> {
        match &self.availability {
            Availability::Ready => None,
            Availability::NoDevice(reason) => Some(format!("audio unavailable — {reason}")),
            Availability::Closed => Some("audio engine stopped".to_owned()),
        }
    }

    /// The chain the engine last reported, whatever it is — the whole
    /// reading, for anything that wants the direct case too.
    #[must_use]
    pub fn signal_path(&self) -> Option<SignalPath> {
        self.signal
    }

    /// The bottom bar's signal-path note — the chain when the engine is
    /// converting, `bit-perfect` when the whole path is transparent, and
    /// nothing otherwise (module docs give the three cases and why the third
    /// stays quiet).
    ///
    /// Presence is decided by what the engine reported and by nothing else:
    /// not by comparing the two rates (the engine, not the front end, knows
    /// whether it is converting), not by phase, not by depth, and not by
    /// where the fader looks like it is sitting — [`Self::bit_exact`] asks
    /// [`VolumePath::is_transparent`], which is the engine's own answer.
    ///
    /// The words are deliberately flat in every case. A rate is a fact about
    /// the chain in the same way a codec name is a fact about a file, and
    /// ADR-0009 §5 puts the tone in the decision itself: no "degraded", no
    /// "fallback", nothing that reads as a fault — and, on the other side,
    /// nothing that reads as a boast. The listener who cares can find it;
    /// everyone else can look straight past it.
    #[must_use]
    pub fn signal_note(&self) -> Option<SignalNote> {
        let path = self.signal?;
        let Some(reason) = path.chain.conversion_reason() else {
            if !self.bit_exact() {
                return None;
            }
            let source = vm::format_sample_rate(path.source_rate_hz);
            return Some(SignalNote {
                label: "bit-perfect".to_owned(),
                detail: format!(
                    "{source} reaching the output untouched — no rate conversion, \
                     and the volume is not scaling the samples"
                ),
            });
        };
        let source = vm::format_sample_rate(path.source_rate_hz);
        let output = vm::format_sample_rate(path.output_rate_hz);
        let because = match reason {
            ConversionReason::DeviceRateUnavailable => {
                format!("this device has no {source} mode")
            }
            ConversionReason::FixedOutputRate => {
                format!("the output is set to a fixed {output}")
            }
            // `ConversionReason` is #[non_exhaustive]: a cause this build has
            // not heard of still gets the honest half of the sentence.
            _ => format!("converted from {source}"),
        };
        Some(SignalNote {
            label: format!("{} → {output}", strip_unit(&source)),
            detail: format!("Playing at {output} — {because}"),
        })
    }

    /// Settings' active, actionable signal-path warning.
    #[must_use]
    pub fn signal_warning(&self) -> Option<SignalWarning> {
        self.signal?.warning()
    }

    /// ReplayGain exactly as the engine last reported it — what the settings
    /// panel's controls render themselves from, and what gets persisted.
    ///
    /// Copied out rather than borrowed: it is four small `Copy` fields, and
    /// handing out a reference into the state machine is how a view ends up
    /// holding one across an update.
    #[must_use]
    pub fn replay_gain(&self) -> ReplayGain {
        self.replay_gain
    }

    /// The ReplayGain figure in force for the track playing now, or `None`
    /// when there is nothing to report (ReplayGain off, or nothing playing).
    ///
    /// "Playing" is [`Self::now_playing`] — a track the engine has told us
    /// about — rather than the phase, so a paused track still explains the
    /// gain it is paused at.
    #[must_use]
    pub fn replay_gain_readout(&self) -> Option<ReplayGainReadout> {
        self.replay_gain.readout(self.now_playing.is_some())
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

/// The run column's one-line reading, **led by the noun of its subject**.
///
/// While something is playing it counts and then says **what is left**:
/// `Run · 3 of 12 · 38:12 left`. Otherwise it states the size and the whole running
/// time, because "0 of 12" would be a position that does not exist and
/// "remaining" is the same number as "total" before anything has started.
///
/// *Remaining*, not total, and that is a correction taken from prior art
/// (`docs/design/03-interface-prior-art.md` §5.3(3), R5). `MusicBee`'s queue
/// header and Elisa's *"%1/%2 tracks remaining"* both report what is ahead,
/// because a queue is a thing you are partway through: `51:20` describes a list,
/// where `38:12 left` answers the question the listener actually opened the
/// popover with. The figure is the rest of the playing track plus every track
/// after it, so it is a clock reading rather than a property of the list.
///
/// The time is appended only when there is one to state. A queue of tracks the
/// scan read no duration for says nothing rather than `0:00`, on the same
/// principle as the `--:--` the seek bar shows for an undeclared length: an
/// unknown is not a zero.
///
/// **The summary leads with the run's provenance** when the run has one —
/// `Road Trip · 3 of 12 · 38:12 left` — because *"what list is this run
/// from?"* was the one question the hierarchy sheet could not answer at all
/// (`docs/design/09-implicit-playlists.md` §6, §10). Origin, never a live
/// link: the name keeps leading through every edit, and goes only when a
/// different play gesture replaces the run.
///
/// **And when it has none, it leads with `Run`** (ADR-0024 §A5.1). The strip
/// was the one thing on that surface that could say what the column is, and
/// with no provenance it said `1 of 24 · 1:56:19 left` — a reading with no
/// subject, beside a word offering to save it and above the record's own
/// title. Both branches now open with a noun, so the strip's first token is
/// always *what this list is*, exactly as
/// [`PanelRow::counts`](crate::playlists::PanelRow::counts) makes the lane's
/// second line always open with what a row is.
fn queue_summary(
    queue: &QueueVm,
    cursor: Option<usize>,
    current: Option<&QueueItemVm>,
    rest: &[&QueueItemVm],
    elapsed_ms: u64,
) -> String {
    let reading = if let (Some(slot), Some(current)) = (cursor, current) {
        // **How far through the pass**, not which row: `3 of 12` means "the
        // third of twelve I will hear", and under a shuffled pass a row number
        // answers a different question nobody asked.
        let count = format!("{} of {}", slot + 1, queue.len());
        match left_note(Some(current), rest, elapsed_ms) {
            None => count,
            Some(left) => format!("{count} · {left}"),
        }
    } else {
        let count = match queue.len() {
            1 => "1 track".to_owned(),
            n => format!("{n} tracks"),
        };
        let total = queue.total_time();
        if total == Duration::ZERO {
            count
        } else {
            format!("{count} · {}", vm::format_duration(total))
        }
    };
    match queue.provenance() {
        Some(name) => format!("{name} · {reading}"),
        // **The strip names its subject** (ADR-0024 §A5.1). Without the noun
        // the left end read `1 of 24 · 1:56:19 left` — a reading with no
        // subject — while 340 px to its right stood the word
        // `Save as playlist`, and 57 px below stood the record's own title.
        // The word was optically a control *of the column*, and the column
        // was showing a CD. One word, in the string this branch already
        // builds: it costs no widget and no height, and it is what makes the
        // word beside it unambiguous.
        None => format!("Run · {reading}"),
    }
}

/// **The one remaining-time reading**, shared by the popover's summary line
/// and the bar's ambient continuation — `38:12 left`, or `None` when there is
/// no honest figure to state.
///
/// One function because the two surfaces are visible *at the same time*: the
/// popover floats directly over the bar that opened it, and a queue whose
/// header and whose bar disagreed about how much music was left would be the
/// most obviously broken thing on screen. Neither caller computes anything;
/// they only choose what to put in front of it.
///
/// The figure is the rest of the playing track plus every track after it — a
/// clock reading rather than a property of the list — clamped so a progress
/// report that lands past a track's declared length cannot go negative, and
/// absent rather than `0:00` when the scan read no durations to add up.
fn left_note(
    current: Option<&QueueItemVm>,
    rest: &[&QueueItemVm],
    elapsed_ms: u64,
) -> Option<String> {
    // **What is ahead in the pass**, which under an unshuffled run is the tail
    // of the list and under a shuffled one is a different set of the same size.
    // Either way it is what the listener has left to hear, which is what the
    // figure claims to be.
    let ahead: Duration = rest.iter().filter_map(|item| item.duration).sum();
    // The playing track counts only for what is left *of it*. The engine's
    // last confirmed position is the only honest source for that, and it is
    // clamped so a report that arrives a beat past the end cannot go negative.
    let current = current.and_then(|item| item.duration);
    let remaining = match current {
        Some(track) => ahead + track.saturating_sub(Duration::from_millis(elapsed_ms)),
        None => ahead,
    };
    (remaining != Duration::ZERO).then(|| format!("{} left", vm::format_duration(remaining)))
}

/// One thing the queue holds *after* the record now playing: a whole album, or
/// a loose song.
///
/// The distinction is the point. ADR-0017 §1.7 adopts the stack "with albums
/// listed as albums, never flattened", and a continuation that counted eleven
/// tracks where a listener stacked one record would be exactly that flattening,
/// stated in the one line they cannot avoid reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry<'a> {
    /// A run of consecutive items sharing an album title.
    Album(&'a str),
    /// An item queued on its own.
    Track(&'a str),
}

impl<'a> Entry<'a> {
    /// What to call it when it is the only thing left to name.
    fn name(self) -> &'a str {
        match self {
            Self::Album(name) | Self::Track(name) => name,
        }
    }

    /// Whether this is a whole record rather than a loose song.
    fn is_album(self) -> bool {
        matches!(self, Self::Album(_))
    }
}

/// The bar's ambient continuation: **what the queue holds after this track**,
/// or `None` when it holds nothing after it.
///
/// # The register
///
/// One line, always opening with `then`, at the coarsest grain that is still
/// honest — the critique's `then 2 sleeves · 1h 58m left`
/// (`docs/design/critique/02-surfaces.md`, *Playback*) in this codebase's own
/// vocabulary, where a sleeve is an album and a duration is spelled the one way
/// [`vm::format_duration`] spells it everywhere else:
///
/// | The queue after this track | The line |
/// |---|---|
/// | nothing | *(no line at all)* |
/// | more of the record now playing | `then 9 more · 38:12 left` |
/// | one album | `then Kid A · 1:02:14 left` |
/// | one loose song | `then Windowlicker · 8:12 left` |
/// | several albums | `then 2 albums · 1:58:00 left` |
/// | several loose songs | `then 3 tracks · 12:40 left` |
/// | a mixture | `then 2 albums and 1 track · 1:58:00 left` |
///
/// **The rest of the record you are already inside is counted, not named.** Its
/// title is not on the bar to be repeated (the bar states the *track* and its
/// artist), its running order is a property of the record rather than a choice
/// the listener made, and naming the next track there would put a second title
/// directly under the one that is sounding — two titles, one of them not
/// playing. What a listener wants from that case is *how much of this is left*,
/// and the count and the clock both say it.
///
/// **What follows is named, not counted, when there is exactly one of it.** One
/// album is `then Kid A`, because that is the whole of what is coming and a
/// name carries more than a numeral. Past one, names would not fit a line that
/// may not wrap, so the count takes over — and it counts *entries*, so a
/// stacked record is one thing and not eleven.
///
/// **The time is the whole queue's remainder**, the identical string
/// [`queue_summary`] puts in the popover ([`left_note`]), because there is only
/// one such figure in the product.
///
/// # Silence rather than an omission
///
/// The last track of the queue draws no line. Not `up next: nothing`, not `end
/// of queue` — the product's standing rules makes silence a feature, and the interface
/// announcing the silence it promised would be the announcement, not the
/// silence.
fn continuation(
    current: &QueueItemVm,
    tail: &[&QueueItemVm],
    elapsed_ms: u64,
    shuffled: bool,
) -> Option<String> {
    // How much of the record now playing is still ahead. Only a *named* album
    // can be continued: two adjacent loose songs are two things, not a run.
    //
    // `tail` is **the pass's remainder**, so a run through one record reads
    // `then 9 more` whether or not it is shuffled — every entry left is still
    // that record's, whatever order they come in.
    //
    // A shuffled pass over *several* records is where the adjacency grouping
    // below stops telling the truth, and `shuffled` is why this function takes
    // the flag at all. See the counted forms.
    let playing_album = current.album.as_deref();
    let rest = playing_album.map_or(0, |album| {
        tail.iter()
            .take_while(|item| item.album.as_deref() == Some(album))
            .count()
    });
    // …and everything past it, grouped so that consecutive items sharing an
    // album title are one entry.
    let mut entries: Vec<Entry<'_>> = Vec::new();
    let mut run: Option<&str> = None;
    for item in &tail[rest..] {
        match item.album.as_deref() {
            Some(album) if run == Some(album) => {}
            Some(album) => {
                entries.push(Entry::Album(album));
                run = Some(album);
            }
            None => {
                entries.push(Entry::Track(item.title.as_str()));
                run = None;
            }
        }
    }

    let what = match entries.as_slice() {
        [] if rest == 0 => return None,
        [] => format!("{rest} more"),
        [only] => only.name().to_owned(),
        many => {
            // **Under shuffle a record is counted once, however often the walk
            // returns to it.** The adjacency grouping above is a statement about
            // *the listener's own order*: a record stacked twice with something
            // between it is two things because the listener broke the run, which
            // is what `a_record_stacked_twice_is_two_entries` pins. Shuffle has
            // no such order to break — the walk simply visits — so counting each
            // visit reported the owner's *"way too many albums shown"*: three
            // records interleaved came out as thirty.
            //
            // Loose songs are already distinct things and are left alone; only
            // the album entries fold. The tracks figure is derived from what is
            // left rather than recomputed, so the two can never disagree.
            let mut counted = many.to_vec();
            if shuffled {
                let mut seen: Vec<&str> = Vec::new();
                counted.retain(|entry| match entry {
                    Entry::Album(name) if seen.contains(name) => false,
                    Entry::Album(name) => {
                        seen.push(name);
                        true
                    }
                    Entry::Track(_) => true,
                });
            }
            let albums = counted.iter().filter(|entry| entry.is_album()).count();
            let tracks = counted.len() - albums;
            match (albums, tracks) {
                (0, tracks) => plural(tracks, "track"),
                (albums, 0) => plural(albums, "album"),
                (albums, tracks) => {
                    format!(
                        "{} and {}",
                        plural(albums, "album"),
                        plural(tracks, "track")
                    )
                }
            }
        }
    };
    Some(match left_note(Some(current), tail, elapsed_ms) {
        None => format!("then {what}"),
        Some(left) => format!("then {what} · {left}"),
    })
}

/// `1 album` / `2 albums` — the one place this line pluralises.
fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
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

/// The control position a pointer on the volume fader is asking for, with
/// the unity snap applied.
///
/// Clamped at both ends — a held drag keeps reporting after the pointer
/// leaves the widget, so out-of-bounds pixels are ordinary input — and
/// snapped to [`MAX_POSITION`] within [`UNITY_SNAP_PX`] of the top of the
/// travel. The snap is refused on a groove no wider than the snap itself,
/// where "within four pixels of the top" would be true everywhere and the
/// control would have exactly one reachable value.
#[must_use]
pub fn position_for(pointer: Pointer) -> u16 {
    let width = pointer.usable_width();
    if width > UNITY_SNAP_PX && pointer.x.is_finite() && pointer.x >= width - UNITY_SNAP_PX {
        return MAX_POSITION;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the fraction is clamped to 0..=1, so the product is 0..=MAX_POSITION"
    )]
    let position = (pointer.fraction() * f32::from(MAX_POSITION)).round() as u16;
    position.min(MAX_POSITION)
}

/// Where `position` sits along the fader's travel, `0.0..=1.0`.
fn travel(position: u16) -> f32 {
    f32::from(position.min(MAX_POSITION)) / f32::from(MAX_POSITION)
}

/// A control position as the level it means, for the hover tip.
///
/// Three spellings, and the two special ones are the point:
///
/// - [`MAX_POSITION`] reads **`unity`**, not `0.0 dB`. It is the only
///   position on the control that carries a guarantee, and naming it is what
///   makes "I am at the top" and "I am nearly at the top" different things on
///   sight — the position below it reads `-0.0 dB`, which is exactly and
///   honestly what it is.
/// - Position 0 reads `-∞ dB` rather than a very large negative number,
///   because that is the true reading and
///   [`Volume::decibels`](baz_core::volume::Volume::decibels) declines to
///   invent one.
fn level_label(position: u16) -> String {
    let volume = Volume::new(position);
    if volume.is_unity() {
        return "unity".to_owned();
    }
    match volume.decibels() {
        None => "-∞ dB".to_owned(),
        Some(db) => format!("{db:.1} dB"),
    }
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

/// A formatted rate without its unit, so `48 kHz → 44.1 kHz` can be written
/// `48 → 44.1 kHz` — one unit for the pair, which is how a chain reads aloud
/// and how it stays short enough to ignore.
fn strip_unit(rate: &str) -> &str {
    rate.strip_suffix(" kHz").unwrap_or(rate)
}

/// Where to put the left edge of a `tip_width`-wide preview so that it is
/// centered over the previewed position without hanging off either end of
/// the bar.
///
/// Pure geometry, so the view function that places the tip carries no math
/// of its own: it asks for an offset and pushes a spacer that wide. A tip
/// wider than the bar pins to the left edge rather than going negative.
#[must_use]
pub fn preview_offset(preview: &Preview, tip_width: f32) -> f32 {
    let slack = (preview.width - tip_width).max(0.0);
    (preview.x - tip_width / 2.0).clamp(0.0, slack)
}

#[cfg(test)]
mod tests {
    use baz_core::library::AudioFormat;

    use crate::vm::{AlbumArtistVm, EditionKey, EditionVm, TrackVm};

    use super::*;

    fn track(path: &str, title: &str, number: u32) -> TrackVm {
        TrackVm {
            disc: None,
            number: Some(number),
            title: title.to_owned(),
            artist: None,
            duration: Some(Duration::from_secs(200)),
            path: PathBuf::from(path),
            bytes: None,
        }
    }

    /// One edition holding `tracks`, in `format`.
    fn edition(format: Option<AudioFormat>, tracks: Vec<TrackVm>) -> EditionVm {
        EditionVm {
            key: EditionKey(format),
            detail: None,
            bitrate: None,
            bit_depth: None,
            sample_rate: None,
            replay_gain: crate::vm::ReplayGainCoverage::default(),
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
                genre: None,
                first_seen_ns: None,
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
                genre: None,
                first_seen_ns: None,
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

    /// A queue record of `len` tracks, drawn from the shelf fixture's own
    /// files so the paths the state machine remembers are the ones
    /// [`started`] names, and padded with distinct synthetic files when a test
    /// wants a longer queue than the fixture holds.
    fn queue_of(len: usize) -> QueueVm {
        let albums = albums();
        let mut items: Vec<vm::QueueItemVm> = albums
            .iter()
            .flat_map(|album| {
                album.all_tracks().map(|track| vm::QueueItemVm {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: album.title.clone(),
                    album_artist: None,
                    duration: track.duration,
                    path: track.path.clone(),
                })
            })
            .collect();
        for extra in items.len()..len {
            items.push(vm::QueueItemVm {
                title: format!("Filler {extra}"),
                artist: None,
                album: Some("Filler".to_owned()),
                album_artist: None,
                duration: Some(Duration::from_secs(100)),
                path: PathBuf::from(format!("/m/filler/{extra}.flac")),
            });
        }
        items.truncate(len);
        QueueVm {
            album: Some("Geogaddi".to_owned()),
            artist: "Boards of Canada".to_owned(),
            items,
            origin: None,
            source: vm::RunSource::Fixed,
        }
    }

    fn ready_with_queue(len: usize) -> PlayerState {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(queue_of(len));
        player
    }

    #[test]
    fn started_paused_resumed_stopped_drives_phase_and_the_toggle() {
        let albums = albums();
        let mut player = ready_with_queue(2);
        player.note_transport_sent();
        assert_eq!(
            player.play_pause(),
            PlayPause::Play,
            "a command in flight does not move the toggle off the confirmed phase"
        );

        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(player.phase(), Phase::Playing);
        assert_eq!(player.play_pause(), PlayPause::Pause);
        assert!(
            !player.transport_pending(),
            "any event clears the pending note"
        );
        assert!(player.play_pause_enabled());
        assert!(player.next_enabled());
        let now = player.now_playing().expect("resolved current track");
        assert_eq!(now.title, "Ready Lets Go");
        assert_eq!(now.artist.as_deref(), Some("Boards of Canada"));
        assert_eq!(player.playing_album(), Some(11));

        player.apply(&Event::Paused, &albums);
        assert_eq!(player.phase(), Phase::Paused);
        assert_eq!(player.play_pause(), PlayPause::Play);
        assert!(player.next_enabled(), "Next skips-and-resumes while paused");
        assert!(
            player.now_playing().is_some(),
            "pause keeps the current track on the bar"
        );

        player.apply(&Event::Resumed, &albums);
        assert_eq!(player.phase(), Phase::Playing);
        assert_eq!(player.play_pause(), PlayPause::Pause);

        player.apply(&Event::Stopped, &albums);
        assert_eq!(player.phase(), Phase::Stopped);
        assert_eq!(player.play_pause(), PlayPause::Play);
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
        player.note_queue_sent(queue_of(1));
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
        assert_eq!(player.play_pause(), PlayPause::Play);
        assert!(
            player.play_pause_enabled(),
            "the engine keeps the queue; Play restarts from the top"
        );
        assert!(!player.next_enabled());
    }

    #[test]
    fn a_pending_transport_command_moves_nothing_the_layout_can_see() {
        // The reported flash, pinned: pressing the toggle used to swap its
        // label to `…` and disable both buttons for a frame or three, then
        // put them back. Everything the view sizes or reads from must now be
        // invariant across that window; the only thing left that pending can
        // reach is the glyph's opacity, which changes no geometry.
        let albums = albums();
        let mut player = ready_with_queue(2);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        let before = (
            player.play_pause(),
            player.play_pause().label(),
            player.play_pause_enabled(),
            player.next_enabled(),
        );

        player.note_transport_sent();
        assert!(player.transport_pending(), "the request is still recorded");
        assert_eq!(
            (
                player.play_pause(),
                player.play_pause().label(),
                player.play_pause_enabled(),
                player.next_enabled(),
            ),
            before,
            "a command in flight must not change the glyph, its label, or either button's state"
        );

        // And the glyph moves exactly once, when the engine confirms.
        player.apply(&Event::Paused, &albums);
        assert!(!player.transport_pending());
        assert_eq!(player.play_pause(), PlayPause::Play);
        assert_eq!(player.play_pause().label(), "Play");
    }

    #[test]
    fn the_toggle_never_shows_an_action_the_engine_has_not_confirmed() {
        // The honesty rule, restated for the toggle now that pending no
        // longer touches it: sending Pause does not make the button say
        // "Play" until Paused actually arrives.
        let albums = albums();
        let mut player = ready_with_queue(1);
        player.apply(&started("/m/strays/a.wav", 0), &albums);
        assert_eq!(player.play_pause(), PlayPause::Pause);
        player.note_transport_sent();
        assert_eq!(
            player.play_pause(),
            PlayPause::Pause,
            "still playing until the engine says otherwise"
        );
        // A Progress report clears pending without confirming the pause —
        // and must not move the toggle either.
        player.apply(&progress(1_000, Some(200_000)), &albums);
        assert!(!player.transport_pending());
        assert_eq!(player.play_pause(), PlayPause::Pause);
        player.apply(&Event::Paused, &albums);
        assert_eq!(player.play_pause(), PlayPause::Play);
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
            Some("audio unavailable — no default output")
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
            genre: None,
            first_seen_ns: None,
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

    /// The bar as these tests read it: the two timestamps and the needle in one
    /// value, because until ADR-0017 step 9 they were one widget and the
    /// assertions were written against that shape. `None` when there is nothing
    /// playing, which is what the timestamps answer.
    #[derive(Debug)]
    struct Bar {
        elapsed: String,
        total: String,
        pending: bool,
        position: f32,
        interactive: bool,
        preview: Option<Preview>,
    }

    fn reading(player: &PlayerState) -> Option<Bar> {
        let stamps = player.stamps()?;
        let line = player.needle_bar();
        Some(Bar {
            elapsed: stamps.elapsed,
            total: stamps.total,
            pending: stamps.pending,
            position: line.position,
            interactive: line.interactive,
            preview: line.preview,
        })
    }

    /// What a release on the song seek line asks for, in the shape
    /// [`PlayerState::release_drag`] answers in — so a test that expected a
    /// target and a test that expected nothing read the same way.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "the wrapper is the point: these are compared against an Option-returning
                  release, beside assertions that expect None"
    )]
    fn seek_to(position_ms: u64) -> Option<u64> {
        Some(position_ms)
    }

    /// A player mid-track: playing `/m/boc/geogaddi/01.flac`, 30 s into a
    /// 200 s track, with **one** entry queued.
    ///
    /// Every gesture is track-relative, regardless of how many entries the
    /// held queue contains.
    fn playing_with_progress() -> (Vec<AlbumVm>, PlayerState) {
        let albums = albums();
        let mut player = ready_with_queue(1);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.apply(&progress(30_000, Some(200_000)), &albums);
        (albums, player)
    }

    #[test]
    fn progress_drives_the_bar_and_both_timestamps() {
        let (albums, mut player) = playing_with_progress();
        let bar = reading(&player).expect("a playing track has a seek bar");
        assert_eq!(bar.elapsed, "0:30");
        assert_eq!(bar.total, "3:20");
        assert!((bar.position - 0.15).abs() < 1e-6, "30/200 of the way in");
        assert!(bar.interactive);
        assert!(!bar.pending, "a confirmed reading is not pending");

        // Later reports move it; the track length rides along on each one.
        player.apply(&progress(100_000, Some(200_000)), &albums);
        let bar = reading(&player).expect("bar");
        assert_eq!(bar.elapsed, "1:40");
        assert!((bar.position - 0.5).abs() < 1e-6);
    }

    /// A track of undeclared length: the elapsed time still counts up, and the
    /// needle refuses to invent a position inside it.
    ///
    /// A scalar bar with no total has nothing to scrub against, so the line is
    /// visible but inert and offers no timestamp preview.
    #[test]
    fn a_track_without_a_declared_length_shows_elapsed_but_does_not_scrub() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(vm::album_queue(&albums[1], None));
        player.apply(&started("/m/strays/a.wav", 0), &albums);
        player.apply(&progress(7_000, None), &albums);
        let bar = reading(&player).expect("bar");
        assert_eq!(bar.elapsed, "0:07");
        assert_eq!(bar.total, "--:--", "an undeclared length is not invented");
        assert!(
            (bar.position - 0.0).abs() < 1e-6,
            "no length, no proportion — the line is drawn unfilled"
        );
        assert!(!bar.interactive);
        player.hover_to(at(100.0));
        player.press(at(100.0));
        player.drag_to(at(160.0));
        assert!(!player.dragging());
        assert_eq!(player.release_drag(), None);
        assert!(reading(&player).expect("bar").preview.is_none());
    }

    #[test]
    fn dragging_shows_the_pointer_and_ignores_incoming_progress() {
        let (albums, mut player) = playing_with_progress();
        player.press(at(100.0));
        player.drag_to(at(150.0));
        assert!(player.dragging());
        let bar = reading(&player).expect("bar");
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
        let bar = reading(&player).expect("bar");
        assert_eq!(bar.elapsed, "2:30");
        assert!((bar.position - 0.75).abs() < 1e-6);
    }

    #[test]
    fn release_yields_a_target_then_shows_pending_until_an_event_confirms() {
        let (albums, mut player) = playing_with_progress();
        player.press(at(100.0));
        player.drag_to(at(50.0));
        assert_eq!(player.release_drag(), seek_to(50_000), "25% of 200 s");
        assert!(!player.dragging());
        assert!(player.seek_pending());

        // Pending: the bar holds the requested position rather than snapping
        // back to the last confirmed one.
        let bar = reading(&player).expect("bar");
        assert_eq!(bar.elapsed, "0:50");
        assert!((bar.position - 0.25).abs() < 1e-6);
        assert!(bar.pending);

        // The engine's confirming report clears pending and takes over.
        player.apply(&progress(50_000, Some(200_000)), &albums);
        assert!(!player.seek_pending());
        let bar = reading(&player).expect("bar");
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
        assert_eq!(player.release_drag(), seek_to(0));
        player.press(at(100.0));
        player.drag_to(at(1800.0));
        assert_eq!(player.release_drag(), seek_to(200_000));
    }

    #[test]
    fn a_seek_restarting_the_same_track_does_not_reset_the_bar() {
        // The engine re-emits TrackStarted for the track a seek restarted;
        // treating that as a new track would snap the bar to zero for a
        // frame. Only a different path resets it.
        let (albums, mut player) = playing_with_progress();
        player.press(at(20.0));
        player.drag_to(at(120.0));
        assert_eq!(player.release_drag(), seek_to(120_000));

        player.apply(&progress(120_000, Some(200_000)), &albums);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(
            reading(&player).expect("bar").elapsed,
            "2:00",
            "the restarted track keeps its position"
        );

        // A genuinely different track does reset it.
        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        let bar = reading(&player).expect("bar");
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
            reading(&player).is_none(),
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
        assert!(reading(&player).is_none());
    }

    #[test]
    fn a_paused_track_still_shows_and_accepts_a_seek() {
        let (albums, mut player) = playing_with_progress();
        player.apply(&Event::Paused, &albums);
        let bar = reading(&player).expect("pause keeps the bar on screen");
        assert!(bar.interactive, "seeking while paused is supported");
        assert_eq!(bar.elapsed, "0:30");
        player.press(at(20.0));
        player.drag_to(at(180.0));
        assert_eq!(player.release_drag(), seek_to(180_000));
    }

    #[test]
    fn a_closed_engine_takes_the_bar_with_it() {
        let (_albums, mut player) = playing_with_progress();
        player.engine_closed();
        assert!(reading(&player).is_none());
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
            seek_to(150_000),
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
        let bar = reading(&player).expect("bar");
        assert_eq!(bar.elapsed, "0:30", "still the engine's last report");
        assert!(!bar.pending, "nothing has been requested yet");
        assert_eq!(
            bar.preview.as_ref().map(|p| p.label.as_str()),
            Some("2:30"),
            "the preview keeps showing what the click will land on"
        );

        // Progress still flows underneath an undecided gesture.
        player.apply(&progress(31_000, Some(200_000)), &albums);
        assert_eq!(reading(&player).expect("bar").elapsed, "0:31");

        assert_eq!(player.release_drag(), seek_to(150_000));
        let bar = reading(&player).expect("bar");
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
            reading(&player).expect("bar").elapsed,
            "2:00",
            "the bar follows the pointer once the scrub engages"
        );
        player.drag_to(at(80.0));
        assert_eq!(
            player.release_drag(),
            seek_to(80_000),
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
        assert_eq!(player.release_drag(), seek_to(100_000));

        player.press(at(100.0));
        player.drag_to(at(100.0 + DRAG_THRESHOLD_PX));
        assert!(player.dragging(), "at the threshold the scrub engages");
        assert_eq!(player.release_drag(), seek_to(104_000));

        // Leftward travel counts the same.
        player.press(at(100.0));
        player.drag_to(at(100.0 - DRAG_THRESHOLD_PX));
        assert!(player.dragging());
        assert_eq!(player.release_drag(), seek_to(96_000));
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
        assert_eq!(player.release_drag(), seek_to(101_000));
    }

    #[test]
    fn a_press_outside_the_bar_still_resolves_against_it() {
        // The widget hands over whatever it measured; clamping is this
        // module's job, at both ends and for a bar of no width at all.
        let (_albums, mut player) = playing_with_progress();
        player.press(Pointer::new(-20.0, BAR));
        assert_eq!(player.release_drag(), seek_to(0));
        player.press(Pointer::new(BAR + 40.0, BAR));
        assert_eq!(player.release_drag(), seek_to(200_000));
        // A zero-width scalar line resolves to its start without dividing by
        // zero. A real layout will immediately give it width; this pins the
        // state machine's total answer during that first frame.
        player.press(Pointer::new(37.0, 0.0));
        assert_eq!(
            player.release_drag(),
            seek_to(0),
            "a zero-width song line resolves safely to its start"
        );
    }

    // -----------------------------------------------------------------
    // Hover preview
    // -----------------------------------------------------------------

    #[test]
    fn hovering_previews_the_timestamp_under_the_pointer() {
        let (_albums, mut player) = playing_with_progress();
        player.hover_to(at(50.0));
        let preview = reading(&player)
            .expect("bar")
            .preview
            .expect("a hovered seekable bar previews");
        assert_eq!(preview.label, "0:50", "25% of 200 s");
        assert!((preview.x - 50.0).abs() < 1e-6);
        assert!((preview.width - BAR).abs() < 1e-6);

        player.hover_to(at(160.0));
        assert_eq!(
            reading(&player)
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
        let preview = reading(&player).expect("bar").preview.expect("preview");
        assert_eq!(preview.label, "0:00");
        assert!((preview.x - 0.0).abs() < 1e-6, "the marker clamps too");

        player.hover_to(Pointer::new(BAR + 40.0, BAR));
        let preview = reading(&player).expect("bar").preview.expect("preview");
        assert_eq!(preview.label, "3:20");
        assert!((preview.x - BAR).abs() < 1e-6);

        // A zero-width scalar line still has the only honest preview: its
        // start. Nothing is divided by zero on the way to saying so.
        player.hover_to(Pointer::new(12.0, 0.0));
        let preview = reading(&player).expect("bar").preview.expect("preview");
        assert_eq!(preview.label, "0:00");
        assert!((preview.x - 0.0).abs() < f32::EPSILON);
        assert!((preview.width - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_pointer_leaving_the_bar_clears_the_preview() {
        let (_albums, mut player) = playing_with_progress();
        player.hover_to(at(50.0));
        assert!(reading(&player).expect("bar").preview.is_some());
        player.hover_left();
        assert!(
            reading(&player).expect("bar").preview.is_none(),
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
        let bar = reading(&player).expect("bar");
        assert_eq!(bar.elapsed, "0:40", "hovering does not freeze the bar");
        assert!(!bar.pending);
        assert_eq!(bar.preview.as_ref().map(|p| p.label.as_str()), Some("0:50"));

        player.press(at(50.0));
        player.drag_to(at(150.0));
        let bar = reading(&player).expect("bar");
        assert_eq!(bar.elapsed, "2:30", "the scrub owns the bar");
        assert!(bar.pending);
        assert!(
            bar.preview.is_none(),
            "one pointer, one number: the scrub suppresses the preview"
        );

        // Progress arriving mid-scrub changes neither.
        player.apply(&progress(41_000, Some(200_000)), &albums);
        let bar = reading(&player).expect("bar");
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
        assert_eq!(player.release_drag(), seek_to(150_000));
        assert_eq!(
            reading(&player)
                .expect("bar")
                .preview
                .map(|preview| preview.label),
            Some("2:30".to_owned())
        );

        // Released past the end of the bar: the pointer is not on it, so
        // there is nothing to preview.
        player.press(at(150.0));
        player.drag_to(Pointer::new(BAR + 60.0, BAR));
        assert_eq!(player.release_drag(), seek_to(200_000));
        assert!(reading(&player).expect("bar").preview.is_none());
    }

    #[test]
    fn a_pending_seek_shows_on_the_bar_without_hiding_the_preview() {
        // A pending seek is a request already sent; the pointer is still
        // free to shop for the next one.
        let (_albums, mut player) = playing_with_progress();
        player.press(at(100.0));
        assert_eq!(player.release_drag(), seek_to(100_000));
        player.hover_to(at(20.0));
        let bar = reading(&player).expect("bar");
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
        let preview = |x: f32, width: f32| Preview {
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
        let bar = reading(&player).expect("bar");
        assert_eq!(bar.elapsed, "1:02:03");
        assert_eq!(bar.total, "2:00:00");
    }

    // -----------------------------------------------------------------
    // The signal path
    // -----------------------------------------------------------------

    /// A `SignalPath` event for a `source` → `output` chain.
    fn signal(source: u32, output: u32, chain: SignalChain) -> Event {
        Event::SignalPath {
            source_rate_hz: source,
            source_channels: 2,
            source_bits: Some(24),
            output_rate_hz: output,
            chain,
        }
    }

    /// The `Converting` chain ADR-0009 measures: a 48 kHz master on a device
    /// that only offers 44.1 kHz.
    fn converting() -> Event {
        signal(
            48_000,
            44_100,
            SignalChain::Converting {
                reason: ConversionReason::DeviceRateUnavailable,
            },
        )
    }

    #[test]
    fn a_player_that_has_heard_nothing_reports_no_chain() {
        let albums = albums();
        let mut player = ready_with_queue(2);
        assert_eq!(player.signal_path(), None);
        assert_eq!(player.signal_note(), None, "no event, no readout");

        // A track starting is not itself a chain report: until the engine
        // says what it is doing, the bar says nothing about it.
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(player.signal_note(), None);
    }

    #[test]
    fn a_direct_chain_at_unity_is_recorded_and_reads_bit_perfect() {
        let albums = albums();
        let mut player = ready_with_queue(2);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.apply(&signal(48_000, 48_000, SignalChain::Direct), &albums);

        let path = player.signal_path().expect("the chain was reported");
        assert_eq!(path.chain, SignalChain::Direct);
        assert_eq!(path.source_rate_hz, 48_000);
        assert_eq!(path.output_rate_hz, 48_000);
        assert_eq!(path.source_bits, Some(24));
        // ADR-0011's amendment: a direct chain is only half of bit-exact,
        // and the default volume supplies the other half.
        assert!(player.bit_exact());
        let note = player.signal_note().expect("the affirmative reading");
        assert_eq!(note.label, "bit-perfect");
        assert_eq!(
            note.detail,
            "48 kHz reaching the output untouched — no rate conversion, \
             and the volume is not scaling the samples"
        );
    }

    #[test]
    fn a_converting_chain_names_the_rate_and_the_reason() {
        let albums = albums();
        let mut player = ready_with_queue(2);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.apply(&converting(), &albums);

        let note = player.signal_note().expect("converting is reported");
        assert_eq!(note.label, "48 → 44.1 kHz");
        assert_eq!(
            note.detail,
            "Playing at 44.1 kHz — this device has no 48 kHz mode"
        );

        // A fixed output rate is a different fact and says so.
        player.apply(
            &signal(
                96_000,
                44_100,
                SignalChain::Converting {
                    reason: ConversionReason::FixedOutputRate,
                },
            ),
            &albums,
        );
        let note = player.signal_note().expect("still converting");
        assert_eq!(note.label, "96 → 44.1 kHz");
        assert_eq!(
            note.detail,
            "Playing at 44.1 kHz — the output is set to a fixed 44.1 kHz"
        );
    }

    #[test]
    fn conversion_warning_names_owner_rates_and_action() {
        let path = SignalPath {
            source_rate_hz: 48_000,
            source_channels: 2,
            source_bits: Some(24),
            output_rate_hz: 44_100,
            chain: SignalChain::Converting {
                reason: ConversionReason::DeviceRateUnavailable,
            },
        };
        let warning = path.warning().expect("Baz is converting");
        assert_eq!(warning.title, "Baz is resampling 48 kHz → 44.1 kHz");
        assert!(warning.detail.contains("Select an output device"));
        assert!(warning.detail.contains("supports 48 kHz"));
        assert!(warning.detail.contains("operating-system mixer"));
        assert!(
            SignalPath {
                chain: SignalChain::Direct,
                ..path
            }
            .warning()
            .is_none()
        );
    }

    #[test]
    fn continuing_conversion_warns_once_and_direct_clears_it() {
        let converting = SignalPath {
            source_rate_hz: 96_000,
            source_channels: 2,
            source_bits: Some(24),
            output_rate_hz: 48_000,
            chain: SignalChain::Exclusive {
                conversion: Some(ConversionReason::FixedOutputRate),
            },
        };
        let mut state = SignalWarningState::default();
        assert!(state.observe(converting).is_some());
        assert!(
            state.observe(converting).is_none(),
            "continuation is deduplicated"
        );
        assert!(
            state
                .observe(SignalPath {
                    chain: SignalChain::Exclusive { conversion: None },
                    ..converting
                })
                .is_none()
        );
        assert!(
            state.observe(converting).is_some(),
            "a later condition is new"
        );
        let warning = converting.warning().expect("exclusive conversion counts");
        assert!(warning.detail.contains("follow-source output"));
    }

    #[test]
    fn the_note_never_reads_as_a_fault() {
        // The tone is part of the decision (ADR-0009 §5), so it is pinned
        // here rather than left to a reviewer's eye: nothing in the words
        // may suggest something has gone wrong.
        let albums = albums();
        let mut player = ready_with_queue(2);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        for event in [
            converting(),
            signal(
                96_000,
                48_000,
                SignalChain::Converting {
                    reason: ConversionReason::FixedOutputRate,
                },
            ),
            // The affirmative reading is held to the same standard: a note
            // that boasted would be as wrong as one that scolded.
            signal(48_000, 48_000, SignalChain::Direct),
        ] {
            player.apply(&event, &albums);
            let note = player.signal_note().expect("a reading");
            let words = format!("{} {}", note.label, note.detail).to_lowercase();
            for alarm in [
                "warning",
                "degraded",
                "error",
                "fallback",
                "unsupported",
                "failed",
                "!",
            ] {
                assert!(
                    !words.contains(alarm),
                    "{words:?} must not carry the word {alarm:?}"
                );
            }
        }
    }

    #[test]
    fn presence_is_derived_from_the_chain_and_from_nothing_else() {
        // The engine, not the front end, decides whether a conversion is in
        // the path: a `Direct` chain shows nothing even when the two rates
        // differ, and a `Converting` one shows the note even when they do
        // not. Inferring from the numbers would put the front end in the
        // business of second-guessing the engine.
        let albums = albums();
        let mut player = ready_with_queue(2);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);

        player.apply(&signal(48_000, 44_100, SignalChain::Direct), &albums);
        assert_eq!(
            player.signal_note().map(|note| note.label),
            Some("bit-perfect".to_owned()),
            "Direct is Direct, whatever the rates read"
        );

        player.apply(
            &signal(
                44_100,
                44_100,
                SignalChain::Converting {
                    reason: ConversionReason::FixedOutputRate,
                },
            ),
            &albums,
        );
        assert!(
            player.signal_note().is_some(),
            "Converting is Converting, whatever the rates read"
        );

        // Nor does anything else about the player move it: phase, pending
        // commands and pointer gestures leave the readout alone.
        player.apply(&Event::Paused, &albums);
        assert!(player.signal_note().is_some(), "pause is not a rate change");
        player.note_transport_sent();
        assert!(player.signal_note().is_some());
        player.press(at(100.0));
        player.drag_to(at(150.0));
        assert!(player.signal_note().is_some());
    }

    #[test]
    fn each_report_replaces_the_last_one_as_the_queue_moves() {
        // An album whose rate the device can follow, then one it cannot,
        // then back: the bar follows the engine's latest word every time.
        let albums = albums();
        let mut player = ready_with_queue(3);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.apply(&signal(44_100, 44_100, SignalChain::Direct), &albums);
        assert_eq!(
            player.signal_note().map(|note| note.label),
            Some("bit-perfect".to_owned())
        );

        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        player.apply(&converting(), &albums);
        assert_eq!(
            player.signal_note().map(|note| note.label),
            Some("48 → 44.1 kHz".to_owned())
        );

        player.apply(&started("/m/strays/a.wav", 2), &albums);
        player.apply(&signal(44_100, 44_100, SignalChain::Direct), &albums);
        assert_eq!(
            player.signal_note().map(|note| note.label),
            Some("bit-perfect".to_owned()),
            "back to a rate the device can follow"
        );

        // A track change on its own reports nothing new, and must not
        // resurrect a chain the engine has moved past.
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(
            player.signal_note().map(|note| note.label),
            Some("bit-perfect".to_owned())
        );
    }

    #[test]
    fn the_chain_does_not_outlive_the_session_it_described() {
        let albums = albums();
        for ending in [Event::Stopped, Event::QueueEnded] {
            let mut player = ready_with_queue(2);
            player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
            player.apply(&converting(), &albums);
            assert!(player.signal_note().is_some());

            player.apply(&ending, &albums);
            assert_eq!(player.signal_path(), None, "{ending:?} ends the session");
            assert_eq!(player.signal_note(), None);
        }

        // And an engine that goes away takes its chain with it.
        let mut player = ready_with_queue(2);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.apply(&converting(), &albums);
        player.engine_closed();
        assert_eq!(player.signal_path(), None);
        assert_eq!(player.signal_note(), None);
    }

    #[test]
    fn a_failed_track_does_not_disturb_the_chain() {
        // Decode-ahead finding a broken file says nothing about the rate the
        // audible track is playing at.
        let albums = albums();
        let mut player = ready_with_queue(3);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.apply(&converting(), &albums);
        player.apply(
            &Event::TrackFailed {
                path: PathBuf::from("/m/boc/geogaddi/02.flac"),
                reason: "decode error: oops".into(),
            },
            &albums,
        );
        assert_eq!(
            player.signal_note().map(|note| note.label),
            Some("48 → 44.1 kHz".to_owned())
        );
    }

    #[test]
    fn chain_labels_spell_rates_the_way_the_rest_of_the_interface_does() {
        let albums = albums();
        let mut player = ready_with_queue(1);
        player.apply(&started("/m/strays/a.wav", 0), &albums);
        for (source, output, label) in [
            (48_000, 44_100, "48 → 44.1 kHz"),
            (96_000, 48_000, "96 → 48 kHz"),
            (192_000, 176_400, "192 → 176.4 kHz"),
            (44_100, 48_000, "44.1 → 48 kHz"),
        ] {
            player.apply(
                &signal(
                    source,
                    output,
                    SignalChain::Converting {
                        reason: ConversionReason::DeviceRateUnavailable,
                    },
                ),
                &albums,
            );
            assert_eq!(
                player.signal_note().expect("converting").label,
                label,
                "{source} -> {output}"
            );
        }
    }

    /// A track playing at `elapsed_ms` of a 200-second track.
    fn seekable_at(elapsed_ms: u64) -> (Vec<AlbumVm>, PlayerState) {
        let albums = albums();
        let mut player = ready_with_queue(2);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.apply(
            &Event::Progress {
                elapsed_ms,
                track_ms: Some(200_000),
            },
            &albums,
        );
        (albums, player)
    }

    #[test]
    fn a_relative_seek_moves_from_the_confirmed_position() {
        let (_albums, mut player) = seekable_at(30_000);
        assert_eq!(player.seek_by(5_000), Some(35_000));
        assert!(
            player.seek_pending(),
            "the request is pending until an event confirms it"
        );
    }

    /// Presses inside one progress window must accumulate — otherwise every
    /// press after the first would land on the same stale reading.
    #[test]
    fn repeated_relative_seeks_accumulate_before_confirmation() {
        let (albums, mut player) = seekable_at(30_000);
        assert_eq!(player.seek_by(5_000), Some(35_000));
        assert_eq!(player.seek_by(5_000), Some(40_000));
        assert_eq!(player.seek_by(-30_000), Some(10_000));

        // The engine's confirming Progress resets the base to truth.
        player.apply(
            &Event::Progress {
                elapsed_ms: 10_000,
                track_ms: Some(200_000),
            },
            &albums,
        );
        assert!(!player.seek_pending());
        assert_eq!(player.seek_by(5_000), Some(15_000));
    }

    #[test]
    fn relative_seeks_clamp_at_both_ends_of_the_track() {
        let (_albums, mut player) = seekable_at(2_000);
        assert_eq!(
            player.seek_by(-30_000),
            Some(0),
            "before the start is the start"
        );

        let (_albums, mut player) = seekable_at(199_000);
        assert_eq!(
            player.seek_by(30_000),
            Some(200_000),
            "past the end is the end, where Command::Seek is documented to act as Next"
        );
    }

    #[test]
    fn an_absolute_seek_clamps_into_the_current_track() {
        let (_albums, mut player) = seekable_at(0);
        assert_eq!(player.seek_to(93_500), Some(93_500));
        assert_eq!(player.seek_to(999_999), Some(200_000));
        assert_eq!(player.seek_to(0), Some(0));
    }

    /// The same honesty test the bar makes: no engine, no session, or no
    /// declared length means there is no position to seek relative to.
    #[test]
    fn seeking_is_refused_where_there_is_nothing_honest_to_seek_within() {
        let albums = albums();

        let mut stopped = ready_with_queue(2);
        assert_eq!(stopped.seek_by(5_000), None, "nothing is playing");
        assert_eq!(stopped.seek_to(5_000), None);

        let mut unknown_length = ready_with_queue(2);
        unknown_length.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        unknown_length.apply(
            &Event::Progress {
                elapsed_ms: 1_000,
                track_ms: None,
            },
            &albums,
        );
        assert_eq!(
            unknown_length.seek_by(5_000),
            None,
            "no declared length, no proportion to seek within"
        );
        assert!(!unknown_length.seek_pending());

        let mut closed = ready_with_queue(2);
        closed.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        closed.engine_closed();
        assert_eq!(closed.seek_by(5_000), None, "no engine to ask");
    }

    /// The track sequence is what gives MPRIS a stable `mpris:trackid`: it
    /// must not move when a seek restarts the same file.
    #[test]
    fn the_track_sequence_counts_tracks_not_track_starts() {
        let albums = albums();
        let mut player = ready_with_queue(2);
        assert_eq!(player.track_seq(), 0);

        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        let first = player.track_seq();
        assert_eq!(first, 1);
        assert_eq!(
            player.now_playing_path(),
            Some(Path::new("/m/boc/geogaddi/01.flac"))
        );

        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(player.track_seq(), first, "a seek restarts the same track");

        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        assert_eq!(player.track_seq(), first + 1);
    }

    // -----------------------------------------------------------------
    // The volume fader
    // -----------------------------------------------------------------

    /// The fader these tests measure against: 100 logical px, so one pixel
    /// is exactly ten control positions and every expectation below is
    /// arithmetic anyone can check in their head.
    const FADER: f32 = 100.0;

    /// A pointer `x` px into [`FADER`].
    fn fader_at(x: f32) -> Pointer {
        Pointer::new(x, FADER)
    }

    fn volume_changed(position: u16, muted: bool, path: VolumePath) -> Event {
        Event::VolumeChanged {
            position,
            muted,
            path,
        }
    }

    /// The engine's ordinary report for a position below unity.
    fn turned_down(position: u16) -> Event {
        volume_changed(position, false, VolumePath::SoftwareGain)
    }

    #[test]
    fn a_fresh_player_is_at_unity_unmuted_and_transparent() {
        // ADR-0011: "a freshly spawned engine is at unity, unmuted,
        // VolumePath::Unity" — so a front end that has heard nothing must
        // show that rather than an invented zero.
        let player = ready_with_queue(0);
        let bar = player.volume_bar();
        assert!((bar.position - 1.0).abs() < 1e-6);
        assert!(bar.unity);
        assert!(!bar.muted);
        assert_eq!(player.volume(), Volume::UNITY);
        assert_eq!(bar.mute_label, "Mute");
    }

    #[test]
    fn seeding_takes_the_engines_own_reading_at_startup() {
        let mut player = ready_with_queue(0);
        player.seed_volume(Volume::new(500), true, VolumePath::SoftwareGain);
        let bar = player.volume_bar();
        assert!((bar.position - 0.5).abs() < 1e-6);
        assert!(bar.muted);
        assert!(!bar.unity);
        assert_eq!(bar.mute_label, "Unmute", "the affordance names the action");
        assert!(!player.bit_exact());
    }

    /// The honesty rule for the volume, stated as a test: what we *send*
    /// never becomes the reading, and an event we did not ask for does.
    #[test]
    fn the_fader_follows_events_and_never_its_own_sends() {
        let albums = albums();
        let mut player = ready_with_queue(1);

        // Ask for half. The confirmed volume has not moved.
        assert_eq!(player.set_volume(500), Some(500));
        assert_eq!(
            player.volume(),
            Volume::UNITY,
            "sending a command is not hearing one"
        );

        // The engine answers with something else entirely — another front
        // end moved it, or ours was clamped. That is the truth now.
        player.apply(&turned_down(750), &albums);
        assert_eq!(player.volume(), Volume::new(750));
        let bar = player.volume_bar();
        assert!((bar.position - 0.75).abs() < 1e-6);
        assert!(!bar.unity);

        // And a bare `Progress` clears the pending display without pretending
        // to confirm anything about the volume.
        assert_eq!(player.set_volume(200), Some(200));
        player.apply(&progress(1_000, Some(200_000)), &albums);
        assert_eq!(
            player.volume(),
            Volume::new(750),
            "an unrelated event clears pending but confirms no volume"
        );
    }

    #[test]
    fn mute_is_separate_state_and_leaves_the_fader_where_it_was() {
        let albums = albums();
        let mut player = ready_with_queue(1);
        player.apply(&turned_down(600), &albums);

        assert_eq!(player.toggle_mute(), Some(true));
        assert!(!player.muted(), "still unmuted until the engine says so");
        assert!(
            player.volume_bar().mute_pending,
            "the request is recorded for the glyph's ink"
        );

        player.apply(
            &volume_changed(600, true, VolumePath::SoftwareGain),
            &albums,
        );
        let bar = player.volume_bar();
        assert!(bar.muted);
        assert!(!bar.mute_pending);
        assert!(
            (bar.position - 0.6).abs() < 1e-6,
            "mute does not move the fader — it is the position mute restores"
        );

        // The toggle resolves against the confirmed state, so it now asks to
        // unmute rather than repeating itself.
        assert_eq!(player.toggle_mute(), Some(false));
        player.apply(&turned_down(600), &albums);
        assert!(!player.volume_bar().muted);
        assert!((player.volume_bar().position - 0.6).abs() < 1e-6);
    }

    /// The volume is engine state, not session state (ADR-0011 §6): nothing
    /// about starting, stopping or skipping a track may touch it.
    #[test]
    fn the_volume_outlives_every_session() {
        let albums = albums();
        for ending in [Event::Stopped, Event::QueueEnded] {
            let mut player = ready_with_queue(2);
            player.apply(&turned_down(300), &albums);
            player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
            assert_eq!(player.volume(), Volume::new(300));
            player.apply(&ending, &albums);
            assert_eq!(
                player.volume(),
                Volume::new(300),
                "{ending:?} is not a volume change"
            );
        }

        // A track boundary is not one either.
        let mut player = ready_with_queue(2);
        player.apply(
            &volume_changed(300, true, VolumePath::SoftwareGain),
            &albums,
        );
        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        assert_eq!(player.volume(), Volume::new(300));
        assert!(player.muted());

        // And an engine that goes away leaves the last honest reading on
        // screen, inert.
        player.engine_closed();
        let bar = player.volume_bar();
        assert!(!bar.interactive);
        assert!((bar.position - 0.3).abs() < 1e-6);
        assert!(bar.muted);
        assert_eq!(player.press_volume(fader_at(50.0)), None);
        assert_eq!(player.step_volume(1), None);
        assert_eq!(player.toggle_mute(), None);
        assert_eq!(player.volume_bar().preview, None);
    }

    // -----------------------------------------------------------------
    // Pixels to positions, and the unity detent
    // -----------------------------------------------------------------

    #[test]
    fn pointer_positions_map_linearly_along_the_travel_and_clamp() {
        assert_eq!(position_for(fader_at(0.0)), 0);
        assert_eq!(position_for(fader_at(25.0)), 250);
        assert_eq!(position_for(fader_at(50.0)), 500);
        assert_eq!(position_for(fader_at(93.0)), 930);
        // Past either end is the end: a held drag keeps reporting after the
        // pointer leaves the widget.
        assert_eq!(position_for(fader_at(-40.0)), 0);
        assert_eq!(position_for(fader_at(400.0)), MAX_POSITION);
        // Degenerate geometry reads silence rather than dividing by nothing.
        assert_eq!(position_for(Pointer::new(12.0, 0.0)), 0);
        assert_eq!(position_for(Pointer::new(f32::NAN, FADER)), 0);
        assert_eq!(position_for(Pointer::new(50.0, f32::NAN)), 0);
        // A groove no wider than the snap stays *linear* rather than
        // snapping: "within four pixels of the top" would otherwise be true
        // everywhere on it, leaving one reachable value.
        assert_eq!(position_for(Pointer::new(1.0, 2.0)), 500);
        assert_eq!(position_for(Pointer::new(2.0, 2.0)), MAX_POSITION);
    }

    /// The whole point of the detent: the bit-perfect position is where the
    /// hand lands, not where it has to be threaded.
    #[test]
    fn the_top_of_the_travel_snaps_to_unity() {
        assert!((UNITY_SNAP_PX - 4.0).abs() < f32::EPSILON);
        for x in [FADER, FADER - 0.5, FADER - UNITY_SNAP_PX, FADER + 20.0] {
            assert_eq!(
                position_for(fader_at(x)),
                MAX_POSITION,
                "{x} px is within the snap and must be unity exactly"
            );
        }
        // Just outside it the control is continuous again — no dead band,
        // and no second position that pretends to be unity.
        let just_below = position_for(fader_at(FADER - UNITY_SNAP_PX - 0.1));
        assert!(
            just_below < MAX_POSITION,
            "outside the snap the fader keeps its resolution: {just_below}"
        );
        assert_eq!(just_below, 959);
        assert!(!Volume::new(just_below).is_unity());
    }

    /// A click is exactly where it was pressed, and a hand's tremor between
    /// button-down and button-up cannot drag it off unity.
    #[test]
    fn a_press_commits_at_once_and_sub_threshold_travel_asks_for_nothing() {
        let mut player = ready_with_queue(1);
        assert_eq!(
            player.press_volume(fader_at(40.0)),
            Some(400),
            "a fader answers on press — there is nothing to wait for"
        );
        // Three pixels of wobble: under the threshold, so nothing more is
        // asked for and the bar keeps showing the pressed position.
        assert_eq!(player.drag_volume(fader_at(41.0)), None);
        assert_eq!(player.drag_volume(fader_at(43.0)), None);
        assert!((player.volume_bar().position - 0.4).abs() < 1e-6);
        player.release_volume();
        assert!((player.volume_bar().position - 0.4).abs() < 1e-6);

        // The same wobble at the top of the travel cannot cost unity.
        let mut player = ready_with_queue(1);
        assert_eq!(player.press_volume(fader_at(FADER)), Some(MAX_POSITION));
        assert_eq!(player.drag_volume(fader_at(FADER - 3.0)), None);
        assert!(player.volume_bar().unity);
    }

    #[test]
    fn a_drag_past_the_threshold_asks_for_every_step_it_passes_through() {
        let mut player = ready_with_queue(1);
        assert_eq!(player.press_volume(fader_at(80.0)), Some(800));
        assert_eq!(
            player.drag_volume(fader_at(80.0 - DRAG_THRESHOLD_PX)),
            Some(760),
            "at the threshold the drag engages and is heard as it happens"
        );
        assert_eq!(player.drag_volume(fader_at(20.0)), Some(200));
        assert!((player.volume_bar().position - 0.2).abs() < 1e-6);
        // A drag that wanders back past the press point is still a drag.
        assert_eq!(player.drag_volume(fader_at(80.0)), Some(800));
        player.release_volume();
        // Dragged off the end of the bar and released there: clamped, and no
        // preview because the pointer is not on the control.
        assert_eq!(player.press_volume(fader_at(50.0)), Some(500));
        assert_eq!(player.drag_volume(Pointer::new(-70.0, FADER)), Some(0));
        player.release_volume();
        assert_eq!(player.volume_bar().preview, None);
    }

    // -----------------------------------------------------------------
    // Losing the pointer mid-gesture
    //
    // A button that comes up over another window is a release baz never
    // sees, so [`crate::groove`] ends the gesture itself and publishes the
    // *ordinary* release-and-exit pair (its "Losing the pointer" docs carry
    // the argument, and its own tests pin the event handling). What follows
    // pins the other half — what that pair does to this state machine, on
    // both bars: the gesture **commits** at the last position it saw, and
    // nothing is left mid-drag to ignore the engine forever.
    // -----------------------------------------------------------------

    #[test]
    fn a_seek_scrub_that_loses_the_pointer_commits_at_the_last_position() {
        let (albums, mut player) = playing_with_progress();
        player.hover_to(at(100.0));
        player.press(at(100.0));
        player.drag_to(at(160.0));
        assert!(player.dragging());

        // The pointer crosses the window edge. The bar has been showing
        // 2:40 for the whole scrub, so 2:40 is what gets asked for —
        // snapping back to the engine's 0:30 would be a jump nobody asked
        // for and would read as baz dropping the input.
        assert_eq!(player.release_drag(), seek_to(160_000));
        player.hover_left();
        assert!(!player.dragging(), "no gesture survives the loss");
        let bar = reading(&player).expect("bar");
        assert_eq!(bar.elapsed, "2:40");
        assert!(bar.pending, "a request awaiting the engine's word");
        assert_eq!(bar.preview, None, "the preview left with the pointer");

        // Movement after the loss is not part of the dead gesture: it can
        // no longer move the bar. This is the reported bug — the groove
        // followed the pointer around the screen — stated as an assertion.
        player.drag_to(at(20.0));
        assert!(!player.dragging());
        assert_eq!(reading(&player).expect("bar").elapsed, "2:40");

        // And pending is not wedged: the engine's confirmation lands.
        player.apply(&progress(160_000, Some(200_000)), &albums);
        assert!(!player.seek_pending());
        let bar = reading(&player).expect("bar");
        assert_eq!(bar.elapsed, "2:40");
        assert!(!bar.pending);
    }

    #[test]
    fn a_seek_click_that_loses_the_pointer_commits_where_it_went_down() {
        // Sub-threshold travel is a click wherever the pointer ends up, and
        // losing the pointer does not change what the hand aimed at.
        let (_albums, mut player) = playing_with_progress();
        player.press(at(60.0));
        player.drag_to(at(62.0));
        assert!(!player.dragging());
        assert_eq!(player.release_drag(), seek_to(60_000));
        player.hover_left();
        assert_eq!(
            player.release_drag(),
            None,
            "the gesture is spent — a second loss asks for nothing"
        );
    }

    #[test]
    fn a_volume_drag_that_loses_the_pointer_keeps_the_level_it_was_left_at() {
        let mut player = ready_with_queue(1);
        player.hover_volume(fader_at(80.0));
        assert_eq!(player.press_volume(fader_at(80.0)), Some(800));
        assert_eq!(
            player.drag_volume(fader_at(30.0)),
            Some(300),
            "a fader is heard as it moves"
        );

        // The pointer leaves the window. Nothing new is asked for and
        // nothing is undone: 300 is the level the listener has been
        // hearing, so 300 is the level they keep. Rolling back would be an
        // audible change caused by nothing they did.
        player.release_volume();
        player.volume_left();
        assert!((player.volume_bar().position - 0.3).abs() < 1e-6);
        assert_eq!(player.volume_bar().preview, None);

        // The fader no longer follows the pointer — the worse half of the
        // reported bug, since every step of it committed.
        assert_eq!(
            player.drag_volume(fader_at(95.0)),
            None,
            "no gesture, no request"
        );
        assert!((player.volume_bar().position - 0.3).abs() < 1e-6);
    }

    #[test]
    fn a_fresh_press_after_a_lost_pointer_is_an_ordinary_gesture() {
        // Re-entering and clicking again must start clean on both bars —
        // the loss ended a gesture, it did not disable the control.
        let (_albums, mut player) = playing_with_progress();
        player.press(at(100.0));
        player.drag_to(at(160.0));
        player.release_drag();
        player.hover_left();

        player.hover_to(at(40.0));
        player.press(at(40.0));
        player.drag_to(at(80.0));
        assert!(player.dragging(), "a new press scrubs like any other");
        assert_eq!(player.release_drag(), seek_to(80_000));

        let mut player = ready_with_queue(1);
        player.press_volume(fader_at(80.0));
        player.drag_volume(fader_at(30.0));
        player.release_volume();
        player.volume_left();
        assert_eq!(player.press_volume(fader_at(70.0)), Some(700));
        assert_eq!(player.drag_volume(fader_at(20.0)), Some(200));
        player.release_volume();
        assert!((player.volume_bar().position - 0.2).abs() < 1e-6);
    }

    // -----------------------------------------------------------------
    // The level preview
    // -----------------------------------------------------------------

    #[test]
    fn hovering_previews_the_level_a_click_would_set() {
        let mut player = ready_with_queue(1);
        player.hover_volume(fader_at(50.0));
        let preview = player
            .volume_bar()
            .preview
            .expect("a hovered live fader previews");
        assert_eq!(preview.label, "-18.1 dB", "half travel on a cubic taper");
        assert!((preview.x - 50.0).abs() < 1e-6);
        assert!((preview.width - FADER).abs() < 1e-6);

        // The top of the travel names the guarantee rather than a number,
        // and one position below it is visibly a different thing.
        player.hover_volume(fader_at(FADER));
        assert_eq!(player.volume_bar().preview.expect("preview").label, "unity");
        player.hover_volume(fader_at(FADER - UNITY_SNAP_PX - 0.1));
        assert_eq!(
            player.volume_bar().preview.expect("preview").label,
            "-1.1 dB"
        );

        // Silence is -∞, not a very large negative number.
        player.hover_volume(fader_at(0.0));
        assert_eq!(player.volume_bar().preview.expect("preview").label, "-∞ dB");

        // The pointer leaving takes the preview with it.
        player.volume_left();
        assert_eq!(player.volume_bar().preview, None);
    }

    #[test]
    fn a_drag_suppresses_the_preview_and_the_ends_clamp() {
        let mut player = ready_with_queue(1);
        player.hover_volume(fader_at(30.0));
        assert!(player.volume_bar().preview.is_some());
        player.press_volume(fader_at(30.0));
        assert!(
            player.volume_bar().preview.is_some(),
            "a press that has not become a drag is still just a pointer"
        );
        player.drag_volume(fader_at(70.0));
        assert_eq!(
            player.volume_bar().preview,
            None,
            "one pointer, one number: the drag suppresses the preview"
        );
        player.release_volume();
        assert_eq!(
            player.volume_bar().preview.expect("preview").label,
            "-9.3 dB",
            "the release leaves the preview where the pointer actually is"
        );

        // Off either end the marker clamps onto the control, exactly as the
        // seek bar's does.
        player.hover_volume(Pointer::new(-40.0, FADER));
        let preview = player.volume_bar().preview.expect("preview");
        assert_eq!(preview.label, "-∞ dB");
        assert!((preview.x - 0.0).abs() < 1e-6);
        player.hover_volume(Pointer::new(FADER + 40.0, FADER));
        let preview = player.volume_bar().preview.expect("preview");
        assert_eq!(preview.label, "unity");
        assert!((preview.x - FADER).abs() < 1e-6);
    }

    #[test]
    fn the_level_label_names_unity_and_silence_and_reads_in_decibels() {
        assert_eq!(level_label(MAX_POSITION), "unity");
        assert_eq!(level_label(0), "-∞ dB");
        assert_eq!(level_label(500), "-18.1 dB");
        assert_eq!(level_label(100), "-60.0 dB");
        // Just below unity is `-0.0 dB`, which is exactly and honestly what
        // it is — and unmistakably not the word above it.
        assert_eq!(level_label(MAX_POSITION - 1), "-0.0 dB");
    }

    // -----------------------------------------------------------------
    // The keyboard step
    // -----------------------------------------------------------------

    #[test]
    fn the_volume_step_is_the_documented_constant() {
        assert_eq!(VOLUME_STEP, 40);
        // The property the step size is chosen for: it divides the travel
        // exactly, so a stepped grid always contains unity itself.
        assert_eq!(MAX_POSITION % VOLUME_STEP, 0);
        // And one press at the top is the ~1 dB a listener hears as a
        // change — the reason the number is 40 and not 25.
        let one_press = Volume::new(MAX_POSITION - VOLUME_STEP)
            .decibels()
            .expect("not silent");
        assert!(
            (-1.2..-0.9).contains(&one_press),
            "one press from unity should be about 1 dB: {one_press}"
        );
    }

    #[test]
    fn stepping_accumulates_before_confirmation_and_clamps_at_both_ends() {
        let albums = albums();
        let mut player = ready_with_queue(1);
        player.apply(&turned_down(500), &albums);

        assert_eq!(player.step_volume(-1), Some(460));
        assert_eq!(
            player.step_volume(-1),
            Some(420),
            "presses inside one round trip accumulate rather than restacking"
        );
        assert_eq!(player.step_volume(3), Some(540));

        // The engine's confirming report resets the base to truth.
        player.apply(&turned_down(420), &albums);
        assert_eq!(player.step_volume(1), Some(460));

        // Down clamps at silence…
        player.apply(&turned_down(20), &albums);
        assert_eq!(player.step_volume(-1), Some(0));
        assert_eq!(player.step_volume(-100), Some(0));
        // …and up clamps at unity, exactly, from anywhere.
        player.apply(&turned_down(993), &albums);
        assert_eq!(player.step_volume(1), Some(MAX_POSITION));
        assert_eq!(player.step_volume(i32::MAX), Some(MAX_POSITION));
    }

    /// Stepping down and back up returns to unity itself, not to a position
    /// that merely looks like it — the reason [`VOLUME_STEP`] divides
    /// [`MAX_POSITION`].
    #[test]
    fn stepping_down_and_back_up_lands_on_unity_exactly() {
        let albums = albums();
        let mut player = ready_with_queue(1);
        let mut position = MAX_POSITION;
        for _ in 0..6 {
            position = player.step_volume(-1).expect("engine ready");
            player.apply(&turned_down(position), &albums);
        }
        assert_eq!(position, MAX_POSITION - 6 * VOLUME_STEP);
        for _ in 0..6 {
            position = player.step_volume(1).expect("engine ready");
            player.apply(&turned_down(position), &albums);
        }
        assert_eq!(position, MAX_POSITION);
        assert!(Volume::new(position).is_unity());
        assert!(player.volume_bar().unity);
    }

    #[test]
    fn stepping_while_muted_prepares_the_level_without_unmuting() {
        let albums = albums();
        let mut player = ready_with_queue(1);
        player.apply(
            &volume_changed(600, true, VolumePath::SoftwareGain),
            &albums,
        );

        assert_eq!(player.step_volume(1), Some(640));
        let bar = player.volume_bar();
        assert!(bar.muted, "volume and mute remain independent controls");
        assert!((bar.position - 0.64).abs() < 1e-6);
        assert!(!bar.mute_pending, "a volume step asks for no mute change");
    }

    #[test]
    fn absolute_sets_clamp_into_the_travel() {
        let mut player = ready_with_queue(1);
        assert_eq!(player.set_volume(0), Some(0));
        assert_eq!(player.set_volume(618), Some(618));
        assert_eq!(player.set_volume(MAX_POSITION), Some(MAX_POSITION));
        assert_eq!(player.set_volume(u16::MAX), Some(MAX_POSITION));
        assert_eq!(player.set_muted(true), Some(true));
        assert_eq!(player.set_muted(false), Some(false));
    }

    // -----------------------------------------------------------------
    // Bit-exactness: the conjunction ADR-0011 introduced
    // -----------------------------------------------------------------

    /// Both halves are required, and neither is inferred from the other.
    #[test]
    fn bit_exactness_is_the_chain_and_the_volume_path_together() {
        let albums = albums();
        let direct = signal(48_000, 48_000, SignalChain::Direct);

        // Nothing reported at all is not a yes.
        let silent = ready_with_queue(1);
        assert!(!silent.bit_exact());
        assert_eq!(silent.signal_note(), None);

        // Direct + transparent — both spellings of transparent.
        for path in [VolumePath::Unity, VolumePath::DeviceAttenuator] {
            let mut player = ready_with_queue(1);
            player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
            player.apply(&direct, &albums);
            player.apply(&volume_changed(MAX_POSITION, false, path), &albums);
            assert!(player.bit_exact(), "{path:?} leaves the samples untouched");
            assert_eq!(
                player.signal_note().map(|note| note.label),
                Some("bit-perfect".to_owned())
            );
        }

        // Direct, but the volume is scaling: not bit-exact, and — the tone
        // rule — nothing at all on the bar rather than a complaint.
        let mut player = ready_with_queue(1);
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.apply(&direct, &albums);
        player.apply(&turned_down(750), &albums);
        assert!(!player.bit_exact());
        assert_eq!(
            player.signal_note(),
            None,
            "a volume below unity is stated by the fader, not by a label"
        );

        // Mute is a software gain of zero, and reads the same way.
        player.apply(
            &volume_changed(750, true, VolumePath::SoftwareGain),
            &albums,
        );
        assert!(!player.bit_exact());

        // Transparent, but the chain is converting: the chain wins the slot,
        // because it is the more specific fact about what is happening.
        player.apply(
            &volume_changed(MAX_POSITION, false, VolumePath::Unity),
            &albums,
        );
        player.apply(&converting(), &albums);
        assert!(!player.bit_exact());
        assert_eq!(
            player.signal_note().map(|note| note.label),
            Some("48 → 44.1 kHz".to_owned())
        );

        // A session ending takes the claim with it: there is no chain, so
        // there is nothing to be bit-exact about.
        player.apply(&direct, &albums);
        assert!(player.bit_exact());
        player.apply(&Event::Stopped, &albums);
        assert!(!player.bit_exact());
        assert_eq!(player.signal_note(), None);
    }

    /// The catalogue facts MPRIS metadata needs come from the same lookup the
    /// bottom bar uses, so the two readouts cannot disagree.
    #[test]
    fn now_playing_resolves_album_and_track_number_too() {
        let albums = albums();
        let resolved = resolve_now_playing(&albums, Path::new("/m/boc/geogaddi/02.flac"));
        assert_eq!(resolved.title, "Music Is Math");
        assert_eq!(resolved.album.as_deref(), Some("Geogaddi"));
        assert_eq!(resolved.artist.as_deref(), Some("Boards of Canada"));
        assert_eq!(resolved.track_number, Some(2));

        // An unknown path keeps its file name and claims nothing else.
        let stray = resolve_now_playing(&albums, Path::new("/m/gone/missing.flac"));
        assert_eq!(stray.title, "missing.flac");
        assert_eq!(stray.album, None);
        assert_eq!(stray.track_number, None);
        assert_eq!(stray.track_artist, None);
    }

    // -----------------------------------------------------------------
    // The queue panel's reading
    // -----------------------------------------------------------------

    /// The queue as `play_album` builds it: Geogaddi's two tracks, whose paths
    /// are the ones [`started`] names.
    fn geogaddi_queue() -> QueueVm {
        vm::album_queue(&albums()[0], None)
    }

    // -----------------------------------------------------------------
    // The shuffle property
    // -----------------------------------------------------------------

    /// **A fresh player plays in order, and the property is a property** —
    /// seeded from the config, moved by the toggle, and touched by nothing
    /// else.
    #[test]
    fn shuffle_is_off_until_something_says_otherwise() {
        let mut player = PlayerState::new(Availability::Ready);
        assert!(!player.shuffle(), "silence and order are both the default");

        // Seeded from `config.toml` at start-up, so the control is lit on the
        // first frame rather than on the first press.
        player.seed_traversal(Traversal::Shuffled { seed: 9 });
        assert!(player.shuffle());
        player.note_traversal(Traversal::InOrder);
        assert!(!player.shuffle());
        player.note_traversal(Traversal::Shuffled { seed: 9 });
        assert!(player.shuffle());

        // **Nothing about playback touches it.** A track starting, the queue
        // being replaced, the engine dying — the property is a standing
        // decision, and a standing decision that a track boundary could clear
        // would be session state wearing a toggle's clothes.
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums());
        assert!(player.shuffle());
        player.note_queue_sent(geogaddi_queue());
        assert!(player.shuffle());
        player.engine_closed();
        assert!(player.shuffle());
    }

    /// **The run keeps the order it was given, whatever the traversal says** —
    /// the owner's decision of 2026-08-10, asserted where the run is recorded.
    ///
    /// This is the test that replaced *"a shuffled run always carries the order
    /// it would have played in"*. That property existed to make the toggle's
    /// **off** possible, and it is not needed because off is not a restoration
    /// any more: the list was never permuted, so there is nothing to put back.
    /// What is asserted instead is the stronger thing — that no traversal, in
    /// any order, over any number of passes, moves a single row.
    #[test]
    fn the_run_is_never_permuted_by_a_traversal() {
        let mut player = PlayerState::new(Availability::Ready);
        let sent = queue_of(6);
        let rows: Vec<String> = sent.items.iter().map(|item| item.title.clone()).collect();
        player.note_queue_sent(sent);
        for traversal in [
            Traversal::Shuffled { seed: 4 },
            Traversal::Shuffled { seed: 5 },
            Traversal::InOrder,
            Traversal::Shuffled { seed: 6 },
        ] {
            player.note_traversal(traversal);
            let list = player.queue_list().expect("a run was sent");
            let drawn: Vec<String> = list.rows.iter().map(|row| row.title.clone()).collect();
            assert_eq!(drawn, rows, "{traversal:?} moved a row");
            // …and the pass is a permutation of that list, every time.
            let mut visited: Vec<usize> = player.order.clone();
            visited.sort_unstable();
            assert_eq!(visited, (0..rows.len()).collect::<Vec<_>>());
        }
    }

    /// **Baz says what is next, in both modes** — the row that plays next is
    /// marked, and the rows the pass has been past are dimmed.
    ///
    /// With shuffle off this is the row under the playing one, which costs the
    /// ordinary case nothing. With it on it is the only thing on screen that
    /// could tell a listener where the run is going, and the product does not
    /// conceal what it knows.
    #[test]
    fn the_run_column_marks_what_plays_next() {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(queue_of(6));
        let len = player.queue().expect("a run").len();

        // Nothing started yet: the whole pass is to come, and its first entry
        // is the one that is next.
        let list = player.queue_list().expect("a run");
        assert_eq!(list.rows[0].state, QueueRowState::Next);
        assert!(
            list.rows[1..]
                .iter()
                .all(|row| row.state == QueueRowState::Upcoming)
        );

        // Playing the first row: the second is next, and nothing is behind.
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums());
        let list = player.queue_list().expect("a run");
        assert_eq!(
            states(&list)[..3],
            [
                QueueRowState::Playing,
                QueueRowState::Next,
                QueueRowState::Upcoming
            ]
        );

        // **Shuffled, the marks follow the pass rather than the list.** The
        // rows do not move; the row that is next is wherever the bag says.
        let traversal = Traversal::Shuffled { seed: 11 };
        player.note_traversal(traversal);
        let order = traversal.play_order(len);
        let cursor = order.iter().position(|&at| at == 0).expect("row 0 is in");
        let list = player.queue_list().expect("a run");
        for (row, state) in list.rows.iter().enumerate() {
            let slot = order.iter().position(|&at| at == row).expect("in the bag");
            let want = if row == 0 {
                QueueRowState::Playing
            } else if slot == cursor + 1 {
                QueueRowState::Next
            } else if slot < cursor {
                QueueRowState::Played
            } else {
                QueueRowState::Upcoming
            };
            assert_eq!(state.state, want, "row {row} at slot {slot}");
        }
        // Exactly one row is next, and exactly one is playing.
        let list = player.queue_list().expect("a run");
        assert_eq!(
            list.rows
                .iter()
                .filter(|row| row.state == QueueRowState::Next)
                .count(),
            1
        );
    }

    fn states(list: &QueueList) -> Vec<QueueRowState> {
        list.rows.iter().map(|row| row.state).collect()
    }

    #[test]
    fn a_player_that_has_queued_nothing_has_no_queue_to_show() {
        let player = PlayerState::new(Availability::Ready);
        assert!(player.queue_list().is_none());
        assert_eq!(player.queued(), 0);
        assert!(
            !player.play_pause_enabled(),
            "nothing queued and nothing playing: the toggle can do nothing"
        );
    }

    /// A queue that has been sent but not started marks its first row as the
    /// one that plays next, lists the rest as upcoming, numbers from one, and
    /// counts rather than claiming a position — "0 of 12" is not a place.
    #[test]
    fn a_queued_but_unstarted_queue_lists_everything_as_upcoming() {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        let list = player.queue_list().expect("a queue was sent");

        assert_eq!(list.album.as_deref(), Some("Geogaddi"));
        assert_eq!(list.artist, "Boards of Canada");
        assert_eq!(list.summary, "Run · 2 tracks · 6:40");
        assert_eq!(
            list.rows.iter().map(|row| row.position).collect::<Vec<_>>(),
            vec![1, 2],
            "the panel numbers from one"
        );
        assert_eq!(
            list.rows
                .iter()
                .map(|r| r.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Ready Lets Go", "Music Is Math"]
        );
        assert_eq!(list.rows[0].duration, "3:20");
        assert_eq!(
            states(&list),
            vec![QueueRowState::Next, QueueRowState::Upcoming],
            "nothing has started, so the head of the pass is what plays next"
        );
        assert_eq!(player.queued(), 2);
        assert!(player.play_pause_enabled(), "a queue makes Play meaningful");
        assert!(
            list.rows.iter().all(|row| row.head.is_none()),
            "one record is one record: the list's own header names it and no \
             row breaks the run"
        );
    }

    /// **The summary leads with the run's provenance** — `Road Trip · 1 of 2
    /// · 6:40 left` — while the origin stands, through the run's end, and not
    /// one frame past the `SetQueue` that replaces it
    /// (`docs/design/09-implicit-playlists.md` §6; S4's and S9's criteria).
    #[test]
    fn the_summary_leads_with_provenance_until_a_new_run_replaces_it() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        let mut from_file = geogaddi_queue();
        from_file.source = vm::RunSource::Playlist("Road Trip".to_owned());
        player.note_queue_sent(from_file);
        assert_eq!(player.queue_provenance(), Some("Road Trip"));
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Road Trip · 2 tracks · 6:40"
        );
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Road Trip · 1 of 2 · 6:40 left"
        );
        // The origin survives the run ending: `QueueEnded` clears what is
        // *playing*, and provenance is a fact about the list that remains.
        player.apply(&Event::QueueEnded, &albums);
        assert_eq!(player.queue_provenance(), Some("Road Trip"));
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Road Trip · 2 tracks · 6:40"
        );
        // Replaced only when the queue is replaced: a play gesture from any
        // other origin hands over a record built with no provenance, and the
        // summary stops naming a list this run is not from.
        player.note_queue_sent(geogaddi_queue());
        assert_eq!(player.queue_provenance(), None);
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Run · 2 tracks · 6:40"
        );
    }

    /// **Both branches of the strip open with a noun** (ADR-0024 §A5.1).
    ///
    /// With no provenance the strip used to read `1 of 24 · 1:56:19 left` — a
    /// reading with no subject — while `Save as playlist` stood 340 px to its
    /// right and the record's own title 57 px below it. Nothing on the surface
    /// said the column was a **run** rather than the record it was showing, so
    /// the word read as a control of the record. `Run · ` is that missing
    /// noun, in the string this function already built.
    #[test]
    fn the_summary_names_its_subject_when_no_file_named_it() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Run · 2 tracks · 6:40",
            "before a run starts, the size and the whole time — with the noun"
        );
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Run · 1 of 2 · 6:40 left",
            "and the cursor reading takes the same noun"
        );

        // The noun is the *subject*, so a run that has one of its own keeps
        // it: a list's name is a better answer to "what is this?" than the
        // word `Run`, and printing both would be the strip saying it twice.
        let mut from_file = geogaddi_queue();
        from_file.source = vm::RunSource::Playlist("Road Trip".to_owned());
        player.note_queue_sent(from_file);
        let summary = player.queue_list().expect("a queue").summary;
        assert!(summary.starts_with("Road Trip · "), "{summary}");
        assert!(!summary.contains("Run · "), "{summary}");
    }

    /// **The save word appears only for a run the listener assembled from
    /// nothing** — the owner's rule of 2026-08-10, as a test over the
    /// predicate rather than over the widget.
    ///
    /// *"I still see save as playlist on the queue when playing a CD"*, and
    /// then *"nah I think adding more stuff to an existing playlist is fine,
    /// that does not need a save"*. Five states, and the two that carry the
    /// argument are C and E: a **named** run offers nothing however much it is
    /// edited, because its file has a cheap page of its own; a **fixed** run
    /// that has been edited offers everything, because what it has become
    /// exists nowhere else.
    ///
    /// Nothing here writes back in any state — ADR-0024 §1's decoupling and
    /// ADR-0023 §3's origin-not-a-link are both untouched by the narrowing.
    #[test]
    fn the_save_word_offers_only_over_a_run_the_listener_assembled() {
        let mut player = PlayerState::new(Availability::Ready);
        assert_eq!(
            player.run_origin(),
            RunOrigin::Fixed,
            "no run at all offers no creation act"
        );

        // A · a record's run — **fixed**. The list exists, it is not the
        // listener's to name, and this is the exact case the owner was
        // looking at when he reported the defect.
        player.note_queue_sent(geogaddi_queue());
        assert_eq!(player.run_origin(), RunOrigin::Fixed);

        // B · reified from a file, untouched — **named**. The run *is* that
        // file, and its name is already printed at the head of the very strip
        // the word sits in, so the strip states it instead of offering it.
        let mut from_file = geogaddi_queue();
        from_file.source = vm::RunSource::Playlist("Road Trip".to_owned());
        player.note_queue_sent(from_file);
        assert_eq!(player.run_origin(), RunOrigin::Saved("Road Trip"));

        // C · the same run, after one edit. **Still no save word** — the
        // narrowing — but it stops claiming to *be* the file, because it is
        // not one any more.
        let mut edited = geogaddi_queue();
        edited.source = vm::RunSource::Playlist("Road Trip".to_owned());
        player.note_queue_edited(edited);
        assert_eq!(player.run_origin(), RunOrigin::Diverged("Road Trip"));
        assert_eq!(
            player.queue_provenance(),
            Some("Road Trip"),
            "the origin still leads the strip: an edit diverges the run, it \
             does not erase where the run came from"
        );
        // …and divergence is a fact about the run, not about the surface
        // showing it. `App::queue_undo` is cleared by leaving the place and by
        // the run ending — neither of which un-edits anything — which is why
        // this is not read off `can_undo`.
        assert_eq!(player.run_origin(), RunOrigin::Diverged("Road Trip"));

        // D · a run built by hand — **unnamed**, and the one live word.
        let mut assembled = geogaddi_queue();
        assembled.source = vm::RunSource::Assembled;
        player.note_queue_sent(assembled);
        assert_eq!(player.run_origin(), RunOrigin::Assembled);

        // E · a record's run, edited. There is no file to go and edit
        // instead, so what is on screen exists nowhere else and the creation
        // act is the only route left. This is the asymmetry with C, and it is
        // the owner's own reasoning applied where his reason does not reach.
        player.note_queue_sent(geogaddi_queue());
        assert_eq!(player.run_origin(), RunOrigin::Fixed);
        player.note_queue_edited(geogaddi_queue());
        assert_eq!(player.run_origin(), RunOrigin::Assembled);

        // A new run is a new answer, in both directions.
        player.note_queue_sent(geogaddi_queue());
        assert_eq!(player.run_origin(), RunOrigin::Fixed);
    }

    /// **A queue holding several records lists them as records** — one name
    /// where each begins, and none where a record simply continues.
    ///
    /// The state `Play all` puts this surface in, and the thing ADR-0014's
    /// *"albums are listed as albums, never flattened"* actually costs: without
    /// a break per record the popover would print forty titles under one
    /// album's name.
    #[test]
    fn a_queue_of_several_records_names_each_one_where_it_begins() {
        let queue = QueueVm {
            album: Some("Laughing Stock".to_owned()),
            artist: "Talk Talk".to_owned(),
            items: vec![
                item("Myrrhman", "Laughing Stock", "Talk Talk"),
                item("Ascension Day", "Laughing Stock", "Talk Talk"),
                item("Ready Lets Go", "Geogaddi", "Boards of Canada"),
                item("Music Is Math", "Geogaddi", "Boards of Canada"),
                item("Sundown", "Sundown", "Gordon Lightfoot"),
            ],
            origin: None,
            source: vm::RunSource::Fixed,
        };
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(queue);
        let list = player.queue_list().expect("a queue was sent");

        // The first record is the list's own header, so its rows carry none.
        assert_eq!(list.album.as_deref(), Some("Laughing Stock"));
        assert_eq!(list.artist, "Talk Talk");
        assert!(list.rows[0].head.is_none());
        assert!(list.rows[1].head.is_none(), "a continuation is not a break");

        // The second and third are named where they begin, by their own
        // artists — the fact the queue's single header could never carry.
        let head = list.rows[2].head.clone().expect("a new record starts here");
        assert_eq!(head.album.as_deref(), Some("Geogaddi"));
        assert_eq!(head.artist, "Boards of Canada");
        assert!(list.rows[3].head.is_none());
        let head = list.rows[4].head.clone().expect("and another");
        assert_eq!(head.album.as_deref(), Some("Sundown"));
        assert_eq!(head.artist, "Gordon Lightfoot");

        // Exactly one break per record after the first — never one per track.
        assert_eq!(list.rows.iter().filter(|r| r.head.is_some()).count(), 2);
    }

    /// One queue item of `album`, filed under `artist`.
    fn item(title: &str, album: &str, artist: &str) -> vm::QueueItemVm {
        vm::QueueItemVm {
            title: title.to_owned(),
            artist: None,
            album: Some(album.to_owned()),
            album_artist: Some(artist.to_owned()),
            duration: Some(Duration::from_secs(200)),
            path: PathBuf::from(format!("/m/{artist}/{album}/{title}.flac")),
        }
    }

    /// The marking comes from `TrackStarted` and moves with it: everything
    /// behind the playing row is played, the row after it is next, everything
    /// beyond that is upcoming, and the summary counts the position.
    #[test]
    fn the_playing_row_is_marked_from_track_started_and_moves_with_it() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());

        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        let list = player.queue_list().expect("a queue");
        assert_eq!(
            states(&list),
            vec![QueueRowState::Playing, QueueRowState::Next]
        );
        // Nothing has elapsed yet, so what is left is the whole queue.
        assert_eq!(list.summary, "Run · 1 of 2 · 6:40 left");

        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        let list = player.queue_list().expect("a queue");
        assert_eq!(
            states(&list),
            vec![QueueRowState::Played, QueueRowState::Playing]
        );
        assert_eq!(
            list.summary, "Run · 2 of 2 · 3:20 left",
            "the first track is behind us and must not count towards what is left"
        );
    }

    /// **What is left, not what exists.** The summary is a clock reading: the
    /// rest of the playing track plus every track after it, so it falls as the
    /// music plays rather than restating a property of the list.
    ///
    /// Taken from prior art rather than invented — `MusicBee`'s queue header and
    /// Elisa's *"tracks remaining"* both report what is ahead
    /// (`docs/design/03-interface-prior-art.md` §5.3(3)).
    #[test]
    fn the_summary_counts_down_what_is_left_rather_than_up_what_exists() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());

        // Before a run starts there is no position, so the reading is the
        // list's own size and its whole running time — "remaining" and "total"
        // are the same number, and the plainer of the two words is right.
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Run · 2 tracks · 6:40"
        );

        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Run · 1 of 2 · 6:40 left"
        );
        player.apply(&progress(60_000, Some(200_000)), &albums);
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Run · 1 of 2 · 5:40 left",
            "a minute into the first track, a minute has come off the reading"
        );

        // A progress report that lands past the track's own declared length —
        // a rate change hands over, a container lied — must not produce a
        // negative remainder.
        player.apply(&progress(999_000, Some(200_000)), &albums);
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Run · 1 of 2 · 3:20 left"
        );
    }

    /// The **Queue** control's readout: the size of what the door opens onto,
    /// present exactly when there is a queue and absent — never zero — when
    /// The engine's position drives the mark, the popover's summary **and** the
    /// bar's ambient line, and it is absent rather than guessed at both ends of
    /// a run.
    ///
    /// `playing_row` is probed directly here because it is the fact under test:
    /// three surfaces read it, and what used to check it — the bar's `3 / 12`
    /// readout, and then the `Queue` door's own count — is no longer drawn at
    /// all. The merged now-playing surface states the size **with the cursor in
    /// it** (`2 of 24`), which is the same fact said better.
    #[test]
    fn the_position_is_the_engines_and_absent_when_the_engine_has_not_said() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        assert_eq!(player.playing_row(), None, "no queue is not position zero");

        player.note_queue_sent(geogaddi_queue());
        assert_eq!(
            player.playing_row(),
            None,
            "a queue that has not started has no position in it"
        );
        assert_eq!(
            player.continuation_note(),
            None,
            "and nothing follows a track that is not playing yet"
        );

        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(player.playing_row(), Some(0));
        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        assert_eq!(player.playing_row(), Some(1));

        // …and it is the same reading the list makes, or the bar and the
        // popover would be able to disagree about where the music is.
        let list = player.queue_list().expect("a queue");
        assert_eq!(list.summary, "Run · 2 of 2 · 3:20 left");

        player.apply(&Event::QueueEnded, &albums);
        assert_eq!(
            player.playing_row(),
            None,
            "an ended run is not still at its last track"
        );
        assert_eq!(player.continuation_note(), None);
    }

    /// A stacked queue: `(album, title, seconds)` per item, in play order, with
    /// a path derived from the position so [`playing_at`] can start any of them.
    ///
    /// `None` for an album is a **loose song** — one queued on its own rather
    /// than as part of a record. Consecutive items sharing a title are one
    /// album, which is the grouping the continuation counts in.
    fn stacked(items: &[(Option<&str>, &str, u64)]) -> QueueVm {
        QueueVm {
            album: items
                .first()
                .and_then(|(album, _, _)| album.map(ToOwned::to_owned)),
            artist: "Various".to_owned(),
            items: items
                .iter()
                .enumerate()
                .map(|(index, (album, title, secs))| vm::QueueItemVm {
                    title: (*title).to_owned(),
                    artist: None,
                    album: album.map(ToOwned::to_owned),
                    album_artist: album.map(|_| "Various".to_owned()),
                    duration: Some(Duration::from_secs(*secs)),
                    path: PathBuf::from(format!("/m/stack/{index}.flac")),
                })
                .collect(),
            origin: None,
            source: vm::RunSource::Fixed,
        }
    }

    /// A player holding `queue` with the engine reporting `index` playing.
    fn playing_at(queue: QueueVm, index: usize) -> PlayerState {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(queue);
        player.apply(&started(&format!("/m/stack/{index}.flac"), index), &[]);
        player
    }

    /// Two records: the one playing, and one stacked behind it.
    ///
    /// **One thing coming is named, not counted.** `then 1 album` would be the
    /// interface refusing to say the one word it knows.
    #[test]
    fn one_record_behind_this_one_is_named() {
        let player = playing_at(
            stacked(&[
                (Some("Geogaddi"), "Ready Lets Go", 200),
                (Some("Geogaddi"), "Music Is Math", 200),
                (Some("Kid A"), "Everything In Its Right Place", 300),
                (Some("Kid A"), "Kid A", 300),
            ]),
            0,
        );
        assert_eq!(
            player.continuation_note().as_deref(),
            Some("then Kid A · 16:40 left"),
            "the record stacked behind this one is named, and the clock is the \
             whole queue's remainder"
        );
    }

    /// Three records: past one, names will not fit a line that may not wrap, so
    /// the count takes over — and it counts **records**, not the tracks inside
    /// them (ADR-0017 §1.7: albums are listed as albums, never flattened).
    #[test]
    fn several_records_are_counted_as_records_with_the_time_left() {
        let player = playing_at(
            stacked(&[
                (Some("Geogaddi"), "Ready Lets Go", 1200),
                (Some("Geogaddi"), "Music Is Math", 1200),
                (Some("Kid A"), "Everything In Its Right Place", 1200),
                (Some("Kid A"), "Kid A", 1200),
                (Some("Amnesiac"), "Pyramid Song", 1200),
            ]),
            0,
        );
        assert_eq!(
            player.continuation_note().as_deref(),
            Some("then 2 albums · 1:40:00 left"),
            "two records follow — five tracks do, and saying five would flatten \
             the stack the listener built"
        );
    }

    /// A loose song is named when it is the only thing coming, and counted as a
    /// *track* when it is not — never as an album, and never merged with the
    /// one beside it.
    #[test]
    fn a_loose_song_is_named_alone_and_counted_in_company() {
        let one = playing_at(
            stacked(&[
                (Some("Geogaddi"), "Ready Lets Go", 200),
                (Some("Geogaddi"), "Music Is Math", 200),
                (None, "Windowlicker", 240),
            ]),
            0,
        );
        assert_eq!(
            one.continuation_note().as_deref(),
            Some("then Windowlicker · 10:40 left")
        );

        let three = playing_at(
            stacked(&[
                (Some("Geogaddi"), "Ready Lets Go", 200),
                (None, "Windowlicker", 240),
                (None, "Come to Daddy", 260),
                (None, "Avril 14th", 120),
            ]),
            0,
        );
        assert_eq!(
            three.continuation_note().as_deref(),
            Some("then 3 tracks · 13:40 left"),
            "three songs queued one by one are three things, not one album"
        );
    }

    /// A mixture says both halves rather than picking the flattering one. The
    /// alternative — calling four things "4 more" — would lose exactly the
    /// distinction the queue exists to keep.
    #[test]
    fn a_mixture_names_both_kinds() {
        let pair = playing_at(
            stacked(&[
                (Some("Geogaddi"), "Ready Lets Go", 200),
                (Some("Kid A"), "Everything In Its Right Place", 300),
                (Some("Kid A"), "Kid A", 300),
                (None, "Windowlicker", 240),
            ]),
            0,
        );
        assert_eq!(
            pair.continuation_note().as_deref(),
            Some("then 1 album and 1 track · 17:20 left"),
            "singular on both sides, and the album is one thing though it is two \
             tracks"
        );

        let more = playing_at(
            stacked(&[
                (Some("Geogaddi"), "Ready Lets Go", 200),
                (Some("Kid A"), "Kid A", 300),
                (Some("Amnesiac"), "Pyramid Song", 300),
                (None, "Windowlicker", 240),
                (None, "Avril 14th", 120),
            ]),
            0,
        );
        assert_eq!(
            more.continuation_note().as_deref(),
            Some("then 2 albums and 2 tracks · 19:20 left")
        );
    }

    /// **The rest of the record you are already inside is counted, not named.**
    /// Its title is not on the bar to be repeated, its running order is the
    /// record's rather than the listener's, and a second title under the one
    /// that is sounding would be two titles with one of them not playing.
    #[test]
    fn the_rest_of_this_record_is_counted_rather_than_named() {
        let queue = || {
            stacked(&[
                (Some("Geogaddi"), "Ready Lets Go", 200),
                (Some("Geogaddi"), "Music Is Math", 200),
                (Some("Geogaddi"), "Beware the Friendly Stranger", 200),
            ])
        };
        assert_eq!(
            playing_at(queue(), 0).continuation_note().as_deref(),
            Some("then 2 more · 10:00 left")
        );
        assert_eq!(
            playing_at(queue(), 1).continuation_note().as_deref(),
            Some("then 1 more · 6:40 left"),
            "one track of this record left is still a count, not the track's name"
        );
    }

    /// **The last track says nothing at all.**
    ///
    /// Not `up next: nothing`, not `end of queue`. the product's standing rules makes the
    /// silence after a queue a feature; an interface that announced it would be
    /// the announcement rather than the silence. The bar reserves the lane
    /// either way, so the absence costs no movement (`views::bottom_bar`).
    #[test]
    fn nothing_follows_the_last_track_and_the_bar_says_nothing() {
        let player = playing_at(
            stacked(&[
                (Some("Geogaddi"), "Ready Lets Go", 200),
                (Some("Geogaddi"), "Music Is Math", 200),
            ]),
            1,
        );
        assert_eq!(player.continuation_note(), None);
        // …and the popover still has a reading, because "what is left" and
        // "what comes after this" are different questions.
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Run · 2 of 2 · 3:20 left"
        );
    }

    /// An empty queue has nothing to continue, and the reading does not invent
    /// a zero.
    #[test]
    fn an_empty_queue_states_no_continuation() {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(QueueVm {
            album: None,
            artist: vm::UNKNOWN_ARTIST.to_owned(),
            items: Vec::new(),
            origin: None,
            source: vm::RunSource::Fixed,
        });
        assert_eq!(player.continuation_note(), None);
        // Even if the engine reports a position into it — which it cannot, but
        // the reading must not depend on that.
        player.apply(&started("/m/stack/0.flac", 0), &[]);
        assert_eq!(player.continuation_note(), None);
    }

    /// **Never optimistic.** With no confirmed position there is no "this
    /// track", so there is nothing to say follows it — and that holds for the
    /// two ways a position goes unknown: a queue that has been sent but has not
    /// started, and a `TrackStarted` naming a file this queue does not hold.
    #[test]
    fn a_queue_whose_position_is_unknown_continues_nothing() {
        let queue = || {
            stacked(&[
                (Some("Geogaddi"), "Ready Lets Go", 200),
                (Some("Kid A"), "Kid A", 300),
            ])
        };

        let mut sent = PlayerState::new(Availability::Ready);
        sent.note_queue_sent(queue());
        assert_eq!(sent.playing_row(), None);
        assert_eq!(
            sent.continuation_note(),
            None,
            "a queue that has not started has no track for anything to follow"
        );
        let mut stray = PlayerState::new(Availability::Ready);
        stray.note_queue_sent(queue());
        stray.apply(&started("/m/strays/a.wav", 0), &[]);
        assert_eq!(
            stray.continuation_note(),
            None,
            "the engine's index is believed only when the path at it agrees, and \
             a continuation drawn from a disagreeing index would name the wrong \
             record"
        );
    }

    /// **One computation, two surfaces.** The ambient line and the popover
    /// summary are visible at the same time — the popover floats directly over
    /// the bar that opened it — so they may not merely agree today, they must
    /// be the same string by construction ([`left_note`]).
    ///
    /// Asserted over a run: as the queue advances and the clock moves, the
    /// figure the bar states and the figure the popover states stay identical.
    #[test]
    fn the_ambient_line_and_the_popover_state_the_same_time_left() {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(stacked(&[
            (Some("Geogaddi"), "Ready Lets Go", 200),
            (Some("Geogaddi"), "Music Is Math", 200),
            (Some("Kid A"), "Kid A", 300),
        ]));

        let agree = |player: &PlayerState, expected: &str| {
            let ambient = player.continuation_note().expect("a continuation");
            let summary = player.queue_list().expect("a queue").summary;
            let left = ambient
                .split_once(" · ")
                .expect("the ambient line states a time")
                .1;
            assert_eq!(left, expected);
            assert!(
                summary.ends_with(left),
                "the bar says {left:?} and the popover says {summary:?}"
            );
        };

        player.apply(&started("/m/stack/0.flac", 0), &[]);
        agree(&player, "11:40 left");
        // A minute in, both readings have come down by a minute.
        player.apply(&progress(60_000, Some(200_000)), &[]);
        agree(&player, "10:40 left");
        // A report past the track's declared length clamps in both, rather
        // than going negative in one of them.
        player.apply(&progress(999_000, Some(200_000)), &[]);
        agree(&player, "8:20 left");
        // And across a track boundary the pair moves together.
        player.apply(&started("/m/stack/1.flac", 1), &[]);
        agree(&player, "8:20 left");
        assert_eq!(
            player.continuation_note().as_deref(),
            Some("then Kid A · 8:20 left"),
            "the record behind this one is named the moment the record playing \
             has nothing after it"
        );
    }

    /// A queue the scan read no durations for states what is coming and says
    /// nothing about how long it runs — an unknown is not a zero, the same rule
    /// the popover's summary and the seek bar's `--:--` follow.
    #[test]
    fn a_continuation_with_no_durations_states_no_time() {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(QueueVm {
            album: Some("Geogaddi".to_owned()),
            artist: "Boards of Canada".to_owned(),
            items: vec![
                vm::QueueItemVm {
                    title: "Ready Lets Go".to_owned(),
                    artist: None,
                    album: Some("Geogaddi".to_owned()),
                    album_artist: None,
                    duration: None,
                    path: PathBuf::from("/m/stack/0.flac"),
                },
                vm::QueueItemVm {
                    title: "Kid A".to_owned(),
                    artist: None,
                    album: Some("Kid A".to_owned()),
                    album_artist: None,
                    duration: None,
                    path: PathBuf::from("/m/stack/1.flac"),
                },
            ],
            origin: None,
            source: vm::RunSource::Fixed,
        });
        player.apply(&started("/m/stack/0.flac", 0), &[]);
        assert_eq!(player.continuation_note().as_deref(), Some("then Kid A"));
        assert_eq!(
            player.queue_list().expect("a queue").summary,
            "Run · 1 of 2"
        );
    }

    /// A record queued twice with something between the two goes back to being
    /// two things, because that is what it is: the run is broken, so the count
    /// counts it twice rather than merging it across the gap.
    #[test]
    fn a_record_stacked_twice_is_two_entries() {
        let player = playing_at(
            stacked(&[
                (Some("Geogaddi"), "Ready Lets Go", 200),
                (Some("Kid A"), "Kid A", 200),
                (Some("Geogaddi"), "Music Is Math", 200),
            ]),
            0,
        );
        assert_eq!(
            player.continuation_note().as_deref(),
            Some("then 2 albums · 10:00 left")
        );
    }

    /// **A shuffled pass counts records, not visits.** The rule above — a
    /// record stacked twice is two things — is a statement about *the
    /// listener's own order*: they broke the run, so the count says so. A
    /// shuffled walk has no such order to break; it simply returns. Counting
    /// each return is what produced the owner's *"the album count in the bottom
    /// bar when in shuffle mode is weird... way too many albums shown"*.
    ///
    /// Both readings of the same five items are asserted here, because the
    /// difference between them **is** the fix.
    #[test]
    fn a_shuffled_pass_counts_records_not_visits() {
        let queue = stacked(&[
            (Some("Geogaddi"), "Ready Lets Go", 200),
            (Some("Kid A"), "Everything In Its Right Place", 200),
            (Some("Geogaddi"), "Music Is Math", 200),
            (Some("Kid A"), "Kid A", 200),
            (Some("Geogaddi"), "Dawn Chorus", 200),
        ]);
        let items: Vec<&QueueItemVm> = queue.items.iter().collect();
        let (current, tail) = items.split_first().expect("a queue");

        assert_eq!(
            continuation(current, tail, 0, true).as_deref(),
            Some("then 2 albums · 16:40 left"),
            "two records are ahead — the walk visits them four times between it"
        );
        assert_eq!(
            continuation(current, tail, 0, false).as_deref(),
            Some("then 4 albums · 16:40 left"),
            "in the listener's own order the same five items are four stacks, \
             because they are the four the listener built"
        );
    }

    /// The property the fix is really claiming, held against the shuffle the
    /// player actually performs rather than a hand-built interleave: **however
    /// the walk falls, the line never claims more records than the run holds.**
    ///
    /// Seeds are swept because a single one proves only that one permutation
    /// is safe, and the defect this pins is precisely a permutation-dependent
    /// one — it does not appear until the walk returns to a record.
    #[test]
    fn no_shuffle_of_a_run_claims_more_records_than_it_holds() {
        let items: Vec<(Option<&str>, &str, u64)> = ["Geogaddi", "Kid A", "Amnesiac"]
            .iter()
            .flat_map(|album| (1..=6_u64).map(move |track| (Some(*album), "track", 60 + track)))
            .collect();

        for seed in 0..32_u64 {
            let mut player = PlayerState::new(Availability::Ready);
            player.seed_traversal(Traversal::Shuffled { seed });
            player.note_queue_sent(stacked(&items));
            player.apply(&started("/m/stack/0.flac", 0), &[]);

            let Some(note) = player.continuation_note() else {
                continue;
            };
            // `then N albums · …` — the only numeral before the word.
            if let Some((count, _)) = note.split_once(" album") {
                let claimed: usize = count
                    .rsplit(' ')
                    .next()
                    .expect("a word")
                    .parse()
                    .unwrap_or(0);
                assert!(
                    claimed <= 3,
                    "seed {seed} claimed {claimed} records of a run holding 3: {note}"
                );
            }
        }
    }

    /// **An edit does not blank the mark.** ADR-0014's whole bargain is that
    /// removing a track the listener is not listening to disturbs nothing, and
    /// a dot that vanished for the round trip would be the interface saying
    /// otherwise.
    ///
    /// Two mechanisms carry the mark across the edit, and this exercises both:
    /// before the engine answers, the row is found by *path* against the edited
    /// record; when `QueueChanged` arrives, the engine's re-derived position
    /// replaces the stale index outright.
    #[test]
    fn an_edit_keeps_the_playing_row_marked_and_then_takes_the_engines_answer() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        assert_eq!(player.playing_row(), Some(1));

        // Remove the row *above* the playing one. The recorded index (1) is now
        // stale — the playing track is row 0 of the edited list — and nothing
        // has come back from the engine yet.
        let edited = crate::queue_edit::without(player.queue().expect("a queue"), 0)
            .expect("row 0 is in the queue");
        player.note_queue_edited(edited);
        assert_eq!(
            player.playing_row(),
            Some(0),
            "the path still finds the row, so the mark survives the round trip"
        );
        let list = player.queue_list().expect("a queue");
        assert_eq!(states(&list), vec![QueueRowState::Playing]);

        // …and then the engine says where it is, and its answer wins.
        player.apply(
            &Event::QueueChanged {
                len: 1,
                position: Some(0),
            },
            &albums,
        );
        assert_eq!(player.playing_row(), Some(0));
        assert_eq!(
            states(&player.queue_list().expect("a queue")),
            vec![QueueRowState::Playing]
        );
    }

    /// **S7 / doc 09 §8.2 — reorder under playback: the cursor follows its
    /// track.** Given a run with the second row sounding, when the sounding
    /// row is stepped down (or the row above it is stepped past it), then
    /// the lamp marks the row the *track* moved to — never the index the
    /// engine last named — and the engine's `QueueChanged` confirms the same
    /// answer. The music was never asked to stop: the edit is the same
    /// whole-list `UpdateQueue` the ✕ sends.
    #[test]
    fn a_reorder_under_playback_keeps_the_cursor_on_its_track() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        assert_eq!(player.playing_row(), Some(1));

        // ▲ on the sounding row: it swaps with the row above, so the track
        // that is playing is now row 0 — and the mark is there before the
        // engine says a word, found by path.
        let edited = crate::queue_edit::shifted(player.queue().expect("a queue"), 1, -1)
            .expect("the sounding row has a row above");
        player.note_queue_edited(edited);
        assert_eq!(
            player.playing_row(),
            Some(0),
            "the cursor follows its track through the reorder"
        );
        assert_eq!(
            states(&player.queue_list().expect("a queue")),
            vec![QueueRowState::Playing, QueueRowState::Next],
            "what sounded second now reads first, still marked; nothing is 'played'"
        );

        // …and the engine's re-derived answer agrees rather than corrects.
        player.apply(
            &Event::QueueChanged {
                len: 2,
                position: Some(0),
            },
            &albums,
        );
        assert_eq!(player.playing_row(), Some(0));
    }

    /// **S9a / doc 09 §13 step 7 — the append accelerator's queue shape.**
    /// Given a record sounding, when another record is appended to the run
    /// (shift-click's `UpdateQueue`, the picker Queue row's exact edit),
    /// then the run keeps its header and its cursor, the addition arrives
    /// as its own headed group after the existing tail — albums listed as
    /// albums, never flattened (ADR-0014) — and nothing behind the cursor
    /// moves.
    #[test]
    fn an_appended_record_joins_the_tail_as_its_own_headed_group() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);

        // The append: the held run extended by the second record's items —
        // `App::append_to_run`'s exact edit over the same record.
        let mut edited = player.queue().expect("a queue").clone();
        edited.items.extend(vm::album_queue(&albums[1], None).items);
        player.note_queue_edited(edited);

        let list = player.queue_list().expect("a queue");
        assert_eq!(
            list.album.as_deref(),
            Some("Geogaddi"),
            "the run's header stays"
        );
        assert_eq!(player.playing_row(), Some(0), "the cursor did not move");
        assert_eq!(
            states(&list),
            vec![
                QueueRowState::Playing,
                QueueRowState::Next,
                QueueRowState::Upcoming
            ]
        );
        // The head breaks exactly where the appended record begins, naming it.
        let heads: Vec<Option<String>> = list
            .rows
            .iter()
            .map(|row| row.head.as_ref().map(|head| head.artist.clone()))
            .collect();
        assert_eq!(
            heads[0], None,
            "the first record is named by the list's header"
        );
        assert_eq!(
            heads[1], None,
            "a row continuing its record carries no head"
        );
        assert!(
            heads[2].is_some(),
            "the appended record opens under its own name"
        );
    }

    /// `QueueChanged { position: None }` is the engine saying nothing is
    /// playing in the queue it now holds — an edit that emptied it, or one
    /// applied while stopped. The mark goes, and it goes because the engine
    /// said so rather than because the front end guessed.
    #[test]
    fn a_queue_change_with_no_position_unmarks_every_row() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert!(player.playing_row().is_some());

        player.apply(
            &Event::QueueChanged {
                len: 2,
                position: None,
            },
            &albums,
        );
        assert_eq!(player.playing_row(), None);
        assert_eq!(
            states(&player.queue_list().expect("a queue")),
            vec![QueueRowState::Next, QueueRowState::Upcoming],
            "nothing is playing, so what is next is the head of the pass again"
        );
    }

    /// The one line that separates an edit from a reset: `SetQueue` drops the
    /// position because it stops the music, and `UpdateQueue` keeps it because
    /// it does not.
    #[test]
    fn a_reset_drops_the_position_where_an_edit_keeps_it() {
        let albums = albums();
        let playing = || {
            let mut player = PlayerState::new(Availability::Ready);
            player.note_queue_sent(geogaddi_queue());
            player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
            assert!(player.playing_row().is_some());
            player
        };

        let mut edited = playing();
        edited.note_queue_edited(geogaddi_queue());
        assert!(edited.playing_row().is_some(), "an edit keeps it");

        let mut reset = playing();
        reset.note_queue_sent(geogaddi_queue());
        assert_eq!(
            reset.playing_row(),
            None,
            "a reset stops the music, so a position into it means nothing"
        );
    }

    /// The engine's position is believed only when the path at it agrees. A
    /// `TrackStarted` naming a file this queue does not hold marks no row —
    /// never row zero by default.
    #[test]
    fn a_track_this_queue_does_not_hold_marks_nothing() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        player.apply(&started("/m/strays/a.wav", 0), &albums);

        let list = player.queue_list().expect("a queue");
        assert_eq!(
            states(&list),
            vec![QueueRowState::Next, QueueRowState::Upcoming],
            "an unknown track must not mark the row its position points at"
        );
        assert_eq!(list.summary, "Run · 2 tracks · 6:40");
    }

    /// Position and path disagreeing — an event from the queue before last —
    /// resolves by path, because the path is the track's identity.
    #[test]
    fn a_stale_position_is_corrected_by_the_path_it_arrived_with() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        // The engine says position 0; the file it names is the queue's second.
        player.apply(&started("/m/boc/geogaddi/02.flac", 0), &albums);
        assert_eq!(
            states(&player.queue_list().expect("a queue")),
            vec![QueueRowState::Played, QueueRowState::Playing]
        );
    }

    /// A session ending keeps the queue — the engine keeps it too, and a later
    /// Play starts from the top — but nothing in it is playing, so no row is
    /// marked.
    #[test]
    fn ending_a_session_keeps_the_queue_and_marks_no_row() {
        let albums = albums();
        for ending in [Event::QueueEnded, Event::Stopped] {
            let mut player = PlayerState::new(Availability::Ready);
            player.note_queue_sent(geogaddi_queue());
            player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
            player.apply(&ending, &albums);

            let list = player
                .queue_list()
                .expect("the queue survives the session that played it");
            assert_eq!(
                states(&list),
                vec![QueueRowState::Next, QueueRowState::Upcoming],
                "{ending:?} left a row playing"
            );
            assert_eq!(list.summary, "Run · 2 tracks · 6:40");
            assert_eq!(player.queued(), 2);
        }
    }

    /// A new queue replaces the list outright, and takes the old position with
    /// it: a position into a list that no longer exists means nothing.
    #[test]
    fn a_fresh_queue_replaces_the_list_and_forgets_the_position() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);

        player.note_queue_sent(vm::album_queue(&albums[1], None));
        let list = player.queue_list().expect("the new queue");
        assert_eq!(list.album.as_deref(), Some("Untitled"));
        assert_eq!(list.rows.len(), 1);
        assert_eq!(states(&list), vec![QueueRowState::Next]);
        assert_eq!(
            list.summary, "Run · 1 track · 3:20",
            "one track is a track, not tracks"
        );
    }

    /// A gone engine cannot say where playback is, so the marking clears —
    /// while the record of what *we* asked for, which is our own memory and
    /// still true, stays.
    #[test]
    fn a_closed_engine_keeps_the_list_and_drops_the_marking() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        player.engine_closed();

        let list = player.queue_list().expect("what we queued is still true");
        assert_eq!(
            states(&list),
            vec![QueueRowState::Next, QueueRowState::Upcoming],
            "no engine, no marking — but the run this process sent still has a head"
        );
    }

    /// A queue of tracks the scan read no duration for states no total, on the
    /// same principle as the seek bar's `--:--`: an unknown is not a zero.
    #[test]
    fn a_queue_with_no_durations_claims_no_total() {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(QueueVm {
            album: None,
            artist: vm::UNKNOWN_ARTIST.to_owned(),
            items: vec![vm::QueueItemVm {
                title: "stream.mp3".to_owned(),
                artist: None,
                album: None,
                album_artist: None,
                duration: None,
                path: PathBuf::from("/m/stream.mp3"),
            }],
            origin: None,
            source: vm::RunSource::Fixed,
        });
        let list = player.queue_list().expect("a queue");
        assert_eq!(list.summary, "Run · 1 track");
        assert_eq!(list.album, None);
        assert_eq!(list.rows[0].duration, "", "no duration, not 0:00");
    }

    /// A seek restarts the current track, and the `TrackStarted` it produces
    /// must not move the marking off the row that is playing.
    #[test]
    fn a_seek_within_the_playing_track_leaves_the_marking_where_it_was() {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(geogaddi_queue());
        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        // The engine restarts the same file at the same position after a seek.
        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        assert_eq!(
            states(&player.queue_list().expect("a queue")),
            vec![QueueRowState::Played, QueueRowState::Playing]
        );
    }

    /// The album inspector's dot: it marks the sounding row of the list it is
    /// showing, and only when that list is the queue that is sounding.
    #[test]
    fn the_inspector_marks_a_row_only_when_it_is_listing_the_playing_queue() {
        let albums = albums();
        let listed = &albums[0].editions[0].tracks;
        let other = &albums[1].editions[0].tracks;

        // Nothing queued at all: nothing to mark.
        let mut player = PlayerState::new(Availability::Ready);
        assert_eq!(player.playing_row_in(listed), None);

        // Queued but not started — the queue is recorded, the engine has
        // confirmed nothing. Marking here would be the optimistic reading the
        // module's honesty rule forbids.
        player.note_queue_sent(geogaddi_queue());
        assert_eq!(player.playing_row_in(listed), None);

        // Playing track 2 of the album the inspector is showing.
        player.apply(&started("/m/boc/geogaddi/02.flac", 1), &albums);
        assert_eq!(player.playing_row_in(listed), Some(1));
        // The queue panel and the inspector agree, row for row — they are two
        // views of one list and must never mark different ones.
        assert_eq!(
            states(&player.queue_list().expect("a queue")),
            vec![QueueRowState::Played, QueueRowState::Playing]
        );

        // A *different* album's inspector marks nothing, even though the same
        // engine is playing.
        assert_eq!(player.playing_row_in(other), None);

        // And the run ending un-marks it: a stopped queue has no sounding row.
        player.apply(&Event::QueueEnded, &albums);
        assert_eq!(player.playing_row_in(listed), None);
    }

    /// The near-miss the comparison exists for: the same album, playing, with
    /// the inspector switched to a format the engine is not reading.
    #[test]
    fn a_different_edition_of_the_playing_album_marks_nothing() {
        let mut albums = albums();
        // The album is owned twice — the FLAC rip and an MP3 rip of the same
        // two tracks, same titles, same order, different files.
        albums[0].editions.push(edition(
            Some(AudioFormat::Mp3),
            vec![
                track("/m/boc/geogaddi/mp3/01.mp3", "Ready Lets Go", 1),
                track("/m/boc/geogaddi/mp3/02.mp3", "Music Is Math", 2),
            ],
        ));
        let flac = &albums[0].editions[0].tracks;
        let mp3 = &albums[0].editions[1].tracks;

        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(vm::album_queue(&albums[0], None));
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);

        // The edition that is playing marks its row…
        assert_eq!(player.playing_row_in(flac), Some(0));
        // …and the one that is not marks nothing, rather than putting the dot
        // on a file the engine is not reading.
        assert_eq!(player.playing_row_in(mp3), None);
        // The album is still the playing album — the *shelf* highlight is a
        // different, coarser question, and it still answers yes.
        assert_eq!(player.playing_album(), Some(11));
    }

    /// A queue that lists one file twice marks the occurrence the engine
    /// named, not merely the first one that matches.
    #[test]
    fn a_track_listed_twice_is_marked_where_the_engine_says_it_is() {
        let repeated = track("/m/boc/geogaddi/01.flac", "Ready Lets Go", 1);
        let album = AlbumVm {
            id: 33,
            title: Some("Loop".into()),
            artist: AlbumArtistVm::Named("Boards of Canada".into()),
            track_artists_vary: false,
            year: None,
            genre: None,
            first_seen_ns: None,
            first_track: repeated.path.clone(),
            editions: vec![edition(
                Some(AudioFormat::Flac),
                vec![repeated.clone(), repeated.clone()],
            )],
        };
        let listed = &album.editions[0].tracks;
        let albums = vec![album.clone()];

        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(vm::album_queue(&album, None));

        // The engine names position 1, and the path there matches, so the
        // second occurrence is marked — not the first.
        player.apply(&started("/m/boc/geogaddi/01.flac", 1), &albums);
        assert_eq!(player.playing_row_in(listed), Some(1));

        // And position 0 marks the first, on the same file.
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(player.playing_row_in(listed), Some(0));

        // A position the queue cannot honour falls back to the first
        // occurrence — `QueueVm::playing`'s rule, unchanged by the inspector.
        player.apply(&started("/m/boc/geogaddi/01.flac", 9), &albums);
        assert_eq!(player.playing_row_in(listed), Some(0));
    }

    /// The decision a click on a track row makes: `JumpTo` alone when the
    /// engine is already holding this album, `SetQueue` + `JumpTo` when it is
    /// not (ADR-0014).
    #[test]
    fn clicking_a_row_jumps_when_the_album_is_the_queue_and_requeues_when_it_is_not() {
        let albums = albums();
        let listed = &albums[0].editions[0].tracks;
        let other = &albums[1].editions[0].tracks;

        // Nothing queued: the album has to be handed over before a position
        // in it means anything.
        let mut player = PlayerState::new(Availability::Ready);
        assert_eq!(
            player.play_from(listed, 1),
            Some(PlayFrom::Requeue { position: 1 })
        );

        // This album is the queue: a plain jump, which is the case worth
        // having — no `SetQueue`, so no `Stopped`, so the run is not replaced
        // in order to move within it.
        player.note_queue_sent(geogaddi_queue());
        assert_eq!(
            player.play_from(listed, 1),
            Some(PlayFrom::Jump { position: 1 })
        );
        // Including the row that is already playing: `JumpTo` restarts it,
        // which is plainly what clicking it means.
        player.apply(&started("/m/boc/geogaddi/01.flac", 0), &albums);
        assert_eq!(
            player.play_from(listed, 0),
            Some(PlayFrom::Jump { position: 0 })
        );

        // A different album, while this one plays: requeue, then jump.
        assert_eq!(
            player.play_from(other, 0),
            Some(PlayFrom::Requeue { position: 0 })
        );

        // The transport is deliberately not consulted. A queue that has ended
        // is still the queue the engine holds, so the click is still a jump —
        // `JumpTo` starts a stopped engine at the position it names.
        player.apply(&Event::QueueEnded, &albums);
        assert_eq!(player.phase(), Phase::Stopped);
        assert_eq!(
            player.play_from(listed, 1),
            Some(PlayFrom::Jump { position: 1 })
        );

        // A row that is not in the list asks for nothing at all, rather than
        // for a position the engine would answer with `QueueEnded`.
        assert_eq!(player.play_from(listed, listed.len()), None);
        assert_eq!(player.play_from(&[], 0), None);
    }
}
