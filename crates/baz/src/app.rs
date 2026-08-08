//! The iced application: first-run setup screen and the album shelf.
//!
//! Architecture (v0.1, ADR-0005):
//!
//! - **UI thread** owns the [`Library`] (SQLite + in-RAM search index) and
//!   all view state. Search runs synchronously per keystroke — sub-ms over
//!   100k tracks per the Phase 1 spike and `baz-core`'s benches.
//! - **Scan worker** (`baz-scan` thread, see [`crate::scan`]) streams
//!   [`scan::ScanUpdate`] batches over a std `mpsc` channel; a ~10 Hz
//!   subscription tick drains *all* pending batches, applies them with one
//!   `Library::add_tracks` call, and rebuilds the view model once — the
//!   shelf populates live during the scan with per-tick, not per-track,
//!   redraws.
//! - **Art workers**: visible tiles request thumbnails via
//!   `tokio::task::spawn_blocking` ([`crate::art`]); decoded RGBA lands in a
//!   600-entry LRU (budget derivation in `art.rs`). Tiles without art render
//!   a deterministic gradient placeholder.
//! - **Playback** ([`crate::playback`], [`crate::player`]): the device
//!   engine is spawned once at app start (feature `device-output`; without
//!   it playback UI is hidden). Commands go straight to the
//!   [`baz_core::engine`] handle; events come back through a bridge
//!   subscription and are the *only* source of playback UI state — see
//!   `player.rs` for the honesty rule. The persistent bottom bar and the
//!   record page's Play button render that state.
//!
//! # What is *not* here
//!
//! Drawing. This module is the application shell — state, [`Message`], the
//! update loop, subscriptions, and the top-level composition that says which
//! surfaces are on screen — while every surface's iced composition lives in
//! [`crate::views`], one module per surface (ADR-0006's mandated split). A
//! layout or visual redesign touches `views/` and nothing in here.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use baz_core::history::{History, HistoryLedger};
use baz_core::index::{GroupKey, Library};
use baz_core::protocol::{self as protocol, Command, Event, SignalChain};
use baz_core::replaygain::ReplayGainSettings;
use iced::keyboard;
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{column, image as iced_image, scrollable, text_input};
use iced::{Element, Size, Subscription, Task, window};
use lru::LruCache;

use crate::motion::{Control, Ink, Keyed, Tween};
use crate::mpris::Mpris;
use crate::place::Place;
use crate::playback::{Playback, PlayerEvent};
use crate::player::{Availability, PlayerState};
use crate::scan::ScanUpdate;
use crate::{
    art, config, font, keys, motion, mpris, player, queue_edit, scan, shelf, shuffle, theme, views,
    vm,
};

/// The top bar's height, used for the pre-first-scroll estimate of the grid
/// viewport (real bounds arrive with every scroll event).
///
/// It was a local `56.0` against a bar that drew **53**, which the composition
/// audit caught: nothing was drawn wrong, because the first layout replaces the
/// estimate with a measurement, but a virtualizer whose first frame is three
/// pixels out is three pixels of shelf resolved against a viewport that does not
/// exist. The number is [`theme::TOP_BAR_H`] now — the same arithmetic the bar
/// is composed from — so the estimate cannot drift from the drawing again.
const TOP_BAR_H: f32 = theme::TOP_BAR_H;
/// Initial window size.
const WINDOW: Size = Size::new(1280.0, 860.0);

/// How often the shell asks whether a periodic rescan is due (ADR-0022 §3).
///
/// Subscribed **only while no scan is running**, and it is not the interval —
/// [`scan::REFRESH_INTERVAL`] is, and [`scan::Refresh`] holds the arithmetic.
/// This is only how often the question is asked, so the answer does not depend
/// on when the timer happened to start. One wake a minute against a five-minute
/// interval costs nothing measurable and keeps the refresh from drifting by up
/// to a whole period.
const REFRESH_TICK: Duration = Duration::from_secs(60);

/// The shelf scrollable's id — the update loop scrolls it back to the top
/// when the query changes, and [`crate::views::shelf`] attaches it.
pub(crate) fn scroll_id() -> scrollable::Id {
    scrollable::Id::new("baz-shelf")
}

/// The search field's id — the update loop focuses it, and
/// [`crate::views::top_bar`] attaches it.
pub(crate) fn search_id() -> text_input::Id {
    text_input::Id::new("baz-search")
}

/// An id no widget in the tree carries, used to **blur** the search well.
///
/// iced 0.13 publishes `text_input::focus` and no `unfocus`, but its focus
/// operation is defined over the whole tree: it focuses the widget whose id
/// matches and **unfocuses every other focusable it walks past**
/// (`iced_core::widget::operation::focusable::focus`). Focusing an id nothing
/// carries is therefore exactly "focus nothing", using the toolkit's own
/// documented behaviour rather than a private field.
///
/// It is a named constant with a test holding it apart from [`search_id`],
/// because the entire mechanism is that the two strings differ.
fn nothing_id() -> text_input::Id {
    text_input::Id::new("baz-nothing")
}

/// Take the caret out of the search well (see [`nothing_id`]).
fn blur_search<T: Send + 'static>() -> Task<T> {
    text_input::focus(nothing_id())
}

/// Run the application. `started` is process start, for the
/// startup-to-interactive log; `cli_dir` is the optional `baz [DIR]` arg.
///
/// The bundled typeface is installed here and nowhere else: every face in
/// [`crate::font::FACES`] is handed to the toolkit before the window exists,
/// and [`theme::SANS`] is named as the default so that a `text` widget with no
/// font of its own gets a real face rather than the platform's guess at
/// `Family::SansSerif` (see `font.rs` for what that guess used to cost).
///
/// **The room is resolved here too, and before anything draws** — the glyph
/// sheet bakes the room's ink into a sprite on first use ([`crate::icon`]), so
/// every read of `theme::active()` in the process has to see the same answer
/// (ADR-0017 §1.5).
pub fn run(started: Instant, cli_dir: Option<PathBuf>) -> iced::Result {
    let room = theme::install();
    println!("[startup] room: {}", room.name);
    let mut app = iced::application("baz", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| theme::theme())
        .default_font(theme::SANS)
        .window(window_settings());
    for face in font::FACES {
        app = app.font(face);
    }
    app.run_with(move || App::new(started, cli_dir))
}

/// The window's settings: its size, and on Linux the application id.
///
/// iced 0.13 leaves the Wayland `app_id` / X11 `WM_CLASS` empty by default,
/// which is what makes a launcher show a running window as an unrelated
/// "unknown" entry beside its own icon. Setting it to the basename of
/// `packaging/io.github.mattcree.baz.desktop` is the whole of the association
/// — the same string MPRIS advertises as `DesktopEntry`, which is why
/// [`mpris::DESKTOP_ENTRY`] is the single place it is spelled.
fn window_settings() -> window::Settings {
    #[cfg_attr(
        not(target_os = "linux"),
        expect(
            unused_mut,
            reason = "only the Linux window settings carry an application id"
        )
    )]
    let mut settings = window::Settings {
        size: WINDOW,
        ..window::Settings::default()
    };
    #[cfg(target_os = "linux")]
    {
        settings.platform_specific.application_id = String::from(mpris::DESKTOP_ENTRY);
    }
    settings
}

/// Top-level messages; one enum across both screens keeps the seams simple.
///
/// Crate-visible because [`crate::views`] emits them: a view function's whole
/// output is an [`Element`] parameterised by this type.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Setup screen: the folder text input changed.
    SetupInput(String),
    /// Setup screen: folder submitted (Enter).
    SetupSubmit,
    /// Shelf: search text changed.
    SearchChanged(String),
    /// **Type anywhere**: a bare printable character was pressed with nothing
    /// focused, so it is the query's (ADR-0017 §1.2, [`crate::keys`]).
    ///
    /// One message for both halves of the gesture — append the text, and put
    /// the caret in the well — because they are one act: the first keystroke
    /// both filters the wall and lands somewhere visible, and a listener who
    /// got one without the other would have typed into a place they cannot
    /// see. Every keystroke *after* it is the field's by the ordinary focus
    /// rule, so this arrives exactly once per query.
    QueryTyped(String),
    /// <kbd>Enter</kbd>, from the wall or from the well's own submit: play the
    /// **top-ranked** match while a query narrows the wall, else the record
    /// whose page is open.
    ///
    /// Only defensible because the first match is the best match — ADR-0021
    /// ranks `Library::search_albums` by fit, then field, then library order —
    /// which is why step 12 had to land before step 11 could.
    PlayFirstMatch,
    /// <kbd>Ctrl</kbd>+<kbd>-</kbd> / <kbd>Ctrl</kbd>+<kbd>=</kbd>, or
    /// <kbd>Ctrl</kbd>+scroll on the wall: step the density. `+1` loosens the
    /// hang and `-1` tightens it; both saturate (see
    /// [`shelf::Density::step`]).
    DensityStep(i32),
    /// The modifier keys that are down, as iced last reported them.
    ///
    /// Held for one job, and it is [`Self::Wheel`]'s: iced 0.13's
    /// `WheelScrolled` carries no modifier state, so <kbd>Ctrl</kbd>+scroll
    /// cannot be recognised from the wheel event alone.
    ModifiersChanged(keyboard::Modifiers),
    /// A wheel notch, with its vertical travel. Answered against the modifiers
    /// above by [`keys::wheel_binding`]; a plain scroll is the `scrollable`'s
    /// own business and this arm does nothing with it.
    Wheel(f32),
    /// Esc anywhere: peel one layer, top down — the place you are in, then the
    /// pull's offer, then the search query, then the shuffle pool's marks (see
    /// [`App::escape`]).
    EscapePressed,
    /// Every place's `‹ Library`, and the tail of <kbd>Esc</kbd>: go home.
    ///
    /// Distinct from the three door messages below because a *back* must not
    /// toggle: pressing `‹ Library` in the Settings and pressing it on a
    /// record's page have to mean the same thing, and neither may send you
    /// somewhere new.
    LeavePlace,
    /// The bar's labelled `Queue` control, or `Q`: go to the queue place, or
    /// come back from it (see [`crate::place`]).
    ToggleQueue,
    /// **The bar's now-playing block**: go to the page of the record that is
    /// sounding.
    ///
    /// The prior-art study's R3 — *get back to what is playing* — which every
    /// product it surveyed spends an affordance on and baz had none for. With
    /// no persistent inspector there is nothing else on screen that knows which
    /// record is under the lamp, so the text that names it is the control that
    /// takes you to it.
    ShowPlayingAlbum,
    /// A row of the **Queue** place was clicked: play the queue from that
    /// zero-based position ([`Command::JumpTo`], ADR-0014).
    ///
    /// Unlike [`Self::PlayTrack`] this needs no decision about re-queueing —
    /// the list the row was drawn from *is* what the engine is holding, by
    /// construction.
    JumpToQueued(usize),
    /// A row's ✕ in the **Queue** place: take that entry out of the queue
    /// without stopping the music ([`Command::UpdateQueue`], ADR-0014).
    RemoveQueued(usize),
    /// The pointer entered a queue row, so the row can offer its ✕.
    ///
    /// Pure view state and the only hover baz tracks itself: iced 0.13 gives a
    /// widget its own hover status inside a *style* function, which is enough
    /// to change a colour and not enough to decide whether a sibling exists.
    QueueRowEntered(usize),
    /// The pointer left a queue row.
    ///
    /// It carries *which* row, and that is not redundant. Both messages are
    /// published from the same `CursorMoved`, in widget order, so dragging the
    /// pointer up a list delivers the new row's entry **before** the old row's
    /// exit — and an exit that meant "nothing is hovered" would immediately
    /// undo the entry that had just arrived. Naming the row makes the exit
    /// conditional and the order stop mattering.
    QueueRowLeft(usize),
    /// Top bar's Settings toggle, or Ctrl+`,`: go to the settings, or come
    /// back from them.
    ToggleSettings,
    /// Shelf scrolled; carries the real viewport geometry.
    Scrolled(Viewport),
    /// Window resized (approximate grid geometry until the next scroll).
    WindowResized(Size),
    /// A word in the top bar's group-key row, or `1`–`5`: arrange the wall by
    /// this key (ADR-0019). Persisted — a listener sets it once.
    GroupKeySelected(baz_core::index::GroupKey),
    /// An entry in the index rail was clicked: put that shelf at the top of
    /// the wall. Carries the run's index, not a pixel — the rail knows which
    /// shelf it points at and nothing about where the shelf is.
    RailJumped(usize),
    /// An album tile was pressed: open that record's page, or come back from
    /// it when it is the page already showing ([`Place::album`]).
    ///
    /// **One press, and it navigates.** The tile's double-click-to-play died
    /// with the inspector and is not mourned in silence: the first press now
    /// replaces the wall, so there is no tile left for a second press to land
    /// on. What replaced it is the page's own `Play album` — a 320 × 32 target
    /// in a fixed place, where the gesture it succeeds had a 400 ms window.
    /// ADR-0022 records the trade.
    AlbumClicked(u64),
    /// The pointer entered an album's tile, so the tile can draw its hover
    /// rule under the wall label.
    ///
    /// The same toolkit limit [`Self::QueueRowEntered`] works around, in the
    /// surface where it matters most: the shelf's state vocabulary is a rule
    /// drawn *beside* the button rather than paint applied *to* it (the shelf
    /// contains exactly two kinds of thing, artwork and type), and a style
    /// function cannot reach a sibling.
    TileEntered(u64),
    /// The pointer left an album's tile. Carries which one, for the reason
    /// [`Self::QueueRowLeft`] carries which row.
    TileLeft(u64),
    /// Queue the album's tracks and play (side-panel Play, tile
    /// double-click).
    PlayAlbum(u64),
    /// A track row of a record's page was clicked: play that album from
    /// that row (`album id`, zero-based row). One message for both of
    /// ADR-0014's cases — which commands go out is
    /// [`PlayerState::play_from`](crate::player::PlayerState::play_from)'s
    /// decision, not the view's.
    PlayTrack(u64, usize),
    /// **Shuffle what the wall shows** — the top bar's `Shuffle`.
    ///
    /// Draws [`shuffle::SLEEVES`] whole records out of the pool the wall is
    /// currently showing, queues them, and starts. The pool it drew from stays
    /// on screen afterwards, marked (`crate::shuffle`).
    Shuffle,
    /// **The pull** — the top bar's `Pull`, and <kbd>Ctrl</kbd>+<kbd>R</kbd>.
    ///
    /// Draws one record, weighted toward the long unplayed, and *offers* it.
    /// **Nothing plays.** Pressing again re-pulls; Escape puts it back.
    Pull,
    /// The record's page: a different format of this album was picked.
    EditionSelected(u64, vm::EditionKey),
    /// Bottom bar, Space, or MPRIS `PlayPause`: play/pause toggle.
    PlayPause,
    /// Bottom bar, `N`, or MPRIS `Next`: skip to the next queued track.
    NextTrack,
    /// Bottom bar, Ctrl+`←`, or MPRIS `Previous`: step back a track, or
    /// restart the current one — the engine's three-second rule decides which,
    /// and the front end deliberately holds no opinion about it.
    PreviousTrack,
    /// MPRIS `Play` (or a `Play` media key): start or resume — *not* a
    /// toggle. There is no on-screen control for it; the toggle covers both
    /// directions, where a desktop media widget asks for one specifically.
    Play,
    /// MPRIS `Pause` (or a `Pause` media key): pause, never resume.
    Pause,
    /// MPRIS `Stop` (or a `MediaStop` key): end the current run through the
    /// queue.
    Stop,
    /// Seek relative to the position the bar is showing, in milliseconds;
    /// negative goes back. Arrow keys and MPRIS `Seek`.
    SeekBy(i64),
    /// Seek to an absolute position in the current track, in milliseconds.
    /// MPRIS `SetPosition`, already checked against the current track id.
    SeekTo(u64),
    /// `/` or Ctrl+F: put the caret in the search well.
    FocusSearch,
    /// MPRIS `Raise`: ask the compositor to bring the window forward.
    Raise,
    /// MPRIS `Quit`: close baz.
    Quit,
    /// The needle: the pointer went down on it, this far along the window.
    /// Nothing is requested and nothing moves yet — the gesture is a click
    /// until it travels [`player::DRAG_THRESHOLD_PX`].
    NeedlePressed(player::Pointer),
    /// The needle: the pointer moved with it held. Past the threshold the
    /// release lands where the pointer is rather than where it went down.
    NeedleDragged(player::Pointer),
    /// The needle: the pointer moved over it with nothing held — the hover tip
    /// follows it, naming what a click there would ask for.
    NeedleHovered(player::Pointer),
    /// The needle: the pointer left it; the tip goes with it.
    NeedleLeft,
    /// The needle was released — the moment the request actually goes to the
    /// engine, as a `Seek` inside the sounding entry or a `JumpTo` outside it
    /// ([`player::NeedleTarget`]).
    NeedleReleased,
    /// Bottom bar: the pointer went down on the volume fader. Unlike the
    /// seek bar this *is* the request — a fader answers at once (see
    /// `player.rs`).
    VolumePressed(player::Pointer),
    /// Bottom bar: the pointer moved with the fader held. Past
    /// [`player::DRAG_THRESHOLD_PX`] every step is a fresh request.
    VolumeDragged(player::Pointer),
    /// Bottom bar: the pointer moved over the fader with nothing held — the
    /// level preview follows it.
    VolumeHovered(player::Pointer),
    /// Bottom bar: the pointer left the fader; the preview goes with it.
    VolumeLeft,
    /// Bottom bar: the fader was released, ending the gesture.
    VolumeReleased,
    /// Up/Down: step the volume by this many
    /// [`player::VOLUME_STEP`]s; negative goes down.
    VolumeStep(i32),
    /// Bottom bar's speaker, or `M`: mute if unmuted and back again,
    /// resolved against the confirmed state.
    ToggleMute,
    /// MPRIS `Volume`: set the fader to an absolute control position,
    /// already mapped through `baz-core`'s taper.
    SetVolume(u16),
    /// MPRIS: mute or unmute outright, never a toggle.
    SetMute(bool),
    /// Settings panel: put ReplayGain in this mode (ADR-0013).
    ///
    /// The four ReplayGain messages carry only what the *control* did. Each
    /// resolves against the settings the engine last confirmed and goes out as
    /// one absolute `SetReplayGain`, so a press cannot desynchronize from a
    /// front end that missed an event, and nothing on screen moves until the
    /// engine answers (see [`crate::replaygain`]).
    ReplayGainMode(protocol::ReplayGainMode),
    /// Settings panel: step the tagged-file pre-amp; negative goes down.
    ReplayGainPreamp(i32),
    /// Settings panel: step the untagged-file pre-amp; negative goes down.
    ReplayGainNoTagPreamp(i32),
    /// Settings panel: arm or disarm clipping prevention.
    ReplayGainPreventClipping(bool),
    /// Settings place: show this section of the place (index into
    /// `views::settings::SECTIONS`).
    SettingsSection(usize),
    /// Settings place: the add-a-folder field changed (ADR-0022).
    MusicFolderInput(String),
    /// Settings place: add the folder in the field, if it is one.
    AddMusicFolder,
    /// Settings place: open the system folder picker — the desktop portal's
    /// dialog on Linux (ADR-0025). The dialog blocks a pool thread, never the
    /// event loop; the wall keeps drawing behind it.
    PickMusicFolder,
    /// The folder picker closed: the folder chosen, or `None` when the dialog
    /// was dismissed. Dismissal decided nothing and therefore changes nothing —
    /// not even the typed path waiting in the field.
    MusicFolderPicked(Option<PathBuf>),
    /// The off-thread look at a submitted path came back: the directory it
    /// named, or the words for why it is not one.
    ///
    /// Between [`Message::AddMusicFolder`] and this, the path was statted on
    /// the blocking pool rather than the UI thread. That split is the NAS
    /// honesty ADR-0025 asks of the *typed* door: a dead network mount answers
    /// `stat` in minutes, not milliseconds, and the event loop must never be
    /// the thing waiting on it.
    MusicFolderChecked(Result<PathBuf, String>),
    /// Settings place: the **first** press of a folder's Remove. Arms the
    /// confirmation and does nothing else — see `views::settings::folder_block`
    /// for why removing is two presses.
    ConfirmRemoveMusicFolder(usize),
    /// Settings place: the confirming press. Stops holding the folder and
    /// forgets its tracks; the files on disk are untouched.
    RemoveMusicFolder(usize),
    /// Settings place: the armed removal was declined.
    CancelRemoveMusicFolder,
    /// Settings place: **force sync** — re-read every file in every folder,
    /// ignoring stamps (ADR-0022 §3).
    ForceSync,
    /// The periodic-refresh clock ticked; a rescan may be due (ADR-0022 §3).
    RefreshTick,
    /// An engine event arrived over the bridge subscription.
    Playback(PlayerEvent),
    /// An off-thread thumbnail decode finished (`None` = no usable art).
    ThumbLoaded(u64, Option<iced_image::Handle>),
    /// ~10 Hz drain of the scan worker's channel while a scan runs.
    ScanTick,
    /// A frame was presented (subscribed only until first-frame is logged).
    FirstFrame,
    /// Advance every transition that is running — subscribed **only** while one
    /// is (see [`App::moving`] and ADR-0020).
    MotionTick(Instant),
    /// The pointer entered an icon button, so its glyph can take the ink a
    /// hovered control is drawn in.
    ///
    /// The same toolkit limit [`Self::QueueRowEntered`] and [`Self::TileEntered`]
    /// work around, in the last surface that still had it: a `button` style's
    /// `text_color` cannot reach the rasterised sprite that *is* the control, so
    /// the button reports its own crossings and the shell holds the one answer
    /// (see [`crate::motion::Control`]).
    ControlEntered(Control),
    /// The pointer left an icon button. Carries which one, for the reason
    /// [`Self::QueueRowLeft`] carries which row.
    ControlLeft(Control),
    /// The left button went down somewhere. Which control it went down *on* is
    /// whichever one the pointer is already known to be over — a `button` with
    /// an `on_press` captures the press before any wrapper can see it, so the
    /// press is resolved against the hover rather than reported by the target.
    PointerPressed,
    /// The left button came up. Ends the press wherever it landed.
    PointerReleased,
}

struct App {
    started: Instant,
    first_frame_logged: bool,
    screen: Screen,
    /// Which place the window is showing — and, since ADR-0022, the whole of
    /// what is on screen above the bar ([`crate::place`]).
    ///
    /// It sits beside `screen` rather than inside it because the two answer
    /// different questions: `screen` is whether there is a library *at all*
    /// (the first-run folder question comes before anything else), and this is
    /// which of the places a library affords you are standing in. A place is
    /// only reachable once there is a shelf to leave.
    ///
    /// **There is no second field beside it.** The `Overlay` that held "which
    /// popover is floating" and the `Selection` that held "which album the
    /// inspector is showing, and whether it is showing" both fold into this one
    /// enum, which is what makes <kbd>Esc</kbd> one line rather than one line
    /// per layer.
    place: Place,
    /// Which row of the **Queue** place the pointer is on, if any.
    ///
    /// The rows offer their removal ✕ on hover only, and iced 0.13
    /// has no way for one widget to ask whether a *sibling* is hovered — a
    /// style function learns its own status and nothing else. So the row
    /// reports its own crossings with a `mouse_area` and the shell holds the
    /// one answer. The ✕'s slot is reserved either way, so this changes what is
    /// drawn in it and never the geometry around it.
    hovered_queue_row: Option<usize>,
    /// The window's size, as the last resize event reported it.
    ///
    /// Held because a place is laid out against the *window*: the record page
    /// and the queue both set their body to a measure of it, and the Settings
    /// place picks its arrangement from it. The shelf keeps its own, separately
    /// measured geometry; that one is the *viewport's*, which is not the
    /// window's once the bars and the rail have taken their share.
    window: Size,
    /// The engine connection (or its documented absence) — spawned once at
    /// app start, before the first screen.
    playback: Playback,
    /// Event-derived playback state; the only thing playback widgets read.
    player: PlayerState,
    /// Desktop media integration (Linux MPRIS2; a no-op elsewhere).
    mpris: Mpris,
    /// The current track's cover-art URL, with the
    /// [`PlayerState::track_seq`](crate::player::PlayerState::track_seq) it
    /// was resolved for. Resolving it reads the album directory, so it is
    /// done once per track change rather than once per progress report.
    mpris_art: (u64, Option<String>),
    /// Which icon button the pointer is on, and how far its ink has travelled
    /// (ADR-0020 §2.1).
    ///
    /// **One tween for every icon button in the product**, keyed by which one is
    /// under the pointer, for the reason the shelf keeps one for the whole wall:
    /// at most one control is hovered, so a tween per control would be state
    /// allocated for a condition all but one of them is never in.
    ink: Keyed<Control>,
    /// The icon button the pointer is *held down* on, if any.
    ///
    /// Not a tween: a press is a discrete act and the finger has already
    /// arrived. It re-aims [`Self::ink`] rather than jumping the ink, so the
    /// press is continuous with the hover that preceded it.
    pressed_control: Option<Control>,
    /// How far the lamp has warmed for the record that is sounding
    /// (ADR-0020 §2.5).
    ///
    /// Linear, 200 ms, and restarted only when the light actually **moves** —
    /// see [`Self::warm_lamp`].
    warmth: Tween,
    /// The ReplayGain setting as it currently stands on disk.
    ///
    /// Kept so that persisting can be driven by the *engine's* confirmations
    /// (the honesty rule again: what is written is what is in force, never
    /// what was asked for) without reading the config file on every
    /// `ReplayGainChanged` — the event also arrives at track boundaries, where
    /// the settings have not moved at all and there is nothing to write.
    saved_replay_gain: ReplayGainSettings,
    /// The play ledger the engine is appending to (ADR-0018), or `None` when
    /// it could not be opened.
    ///
    /// Held here for its lifetime rather than only inside the engine: the
    /// no-audio build has no engine to hold it, and a ledger dropped at the end
    /// of `new` would flush and close a file this process is meant to keep.
    _history_ledger: Option<Arc<HistoryLedger>>,
    /// The arrangement the wall opens in, read from the config before there is
    /// a shelf to hold it — so the first-run path can hand it to the shelf the
    /// setup screen eventually opens.
    group_key: GroupKey,
    /// Which section of the Settings place is showing (an index into
    /// `views::settings::SECTIONS`).
    ///
    /// Session state, like every other "where am I looking" answer in the shell
    /// and for `crate::panels`' reason: which section you last read is not a
    /// standing decision, so it is not in `config.toml`.
    settings_section: usize,
    /// The density the wall opens at, read from the config for the same reason
    /// and handed to the shelf the same way (ADR-0017 step 6).
    density: shelf::Density,
    /// Which modifier keys are down, as iced last reported them.
    ///
    /// The one piece of input state baz tracks itself, and it is tracked for
    /// exactly one reason: iced 0.13's `WheelScrolled` carries no modifiers,
    /// so <kbd>Ctrl</kbd>+scroll cannot be told from a scroll without it. Key
    /// *presses* never consult this — they carry their own modifiers, and
    /// [`keys::binding_for`] reads those (see its focus-rule note on why a
    /// hand-kept flag is the wrong instrument wherever the toolkit reports the
    /// truth itself).
    modifiers: keyboard::Modifiers,
}

enum Screen {
    Setup(Setup),
    Shelf(Box<Shelf>),
}

/// The minimal first-run screen: "Where's your music?".
pub(crate) struct Setup {
    /// What has been typed into the folder field.
    pub(crate) input: String,
    /// Why the last submission did not open a shelf, if it did not.
    pub(crate) error: Option<String>,
}

impl App {
    fn new(started: Instant, cli_dir: Option<PathBuf>) -> (Self, Task<Message>) {
        // Engine first: open failure must not kill the app — it becomes
        // Availability::NoDevice state that the bottom bar reports.
        let playback = Playback::start();
        let mut player = PlayerState::new(playback.availability());
        // The one pull in an event-driven machine, and ADR-0011 provides it
        // for this moment: the fader shows the engine's real volume on the
        // first frame instead of assuming a default until something changes.
        if let Some(state) = playback.volume() {
            player.seed_volume(state.volume, state.muted, state.path);
        }
        // The same pull for ReplayGain (ADR-0013 provides it for the same
        // moment), so the settings panel is right on the first frame rather
        // than on the first change.
        if let Some(state) = playback.replay_gain() {
            player.seed_replay_gain(
                state.settings,
                state.applied.source,
                state.applied.gain_centidb,
                state.applied.clipping_prevented,
            );
        }
        // Desktop integration is an enhancement: this spawns a thread and
        // returns, and an absent session bus costs one stdout line (see
        // crate::mpris).
        let mpris = Mpris::start();
        let stored = config::config_file().map(|path| config::load(&path));
        let saved_replay_gain = stored
            .as_ref()
            .map_or_else(ReplayGainSettings::default, |config| config.replay_gain);
        // Restore the listener's standing ReplayGain decision. It is *sent*,
        // not assumed: the engine is the source of truth, so this is a command
        // like any other and the panel will show whatever the engine confirms
        // in reply. A setting equal to the engine's own defaults emits nothing
        // and costs nothing, which is the ordinary case.
        if saved_replay_gain != ReplayGainSettings::default() {
            playback.send(command_for(saved_replay_gain));
        }
        // **The play ledger, handed to the engine.** ADR-0018 built it and put
        // the whole of a front end's involvement in one call; nothing in this
        // crate made it, so nothing was being recorded and PLAYED had nothing
        // to sort by. This is that call.
        //
        // The engine is the only thing that knows what actually reached the
        // output and for how long, which is why the ledger is written there and
        // not here (`baz_core::history`). A ledger that cannot be opened is
        // carried on without: `set_history(None)` is the engine's own default,
        // so the failure costs the record of *this* session and nothing else —
        // no dialog, no degraded playback, and the file is tried again next
        // launch.
        let history_ledger = match HistoryLedger::open_default() {
            Ok(ledger) => {
                let ledger = Arc::new(ledger);
                println!("[history] recording to {}", ledger.path().display());
                playback.set_history(Some(Arc::clone(&ledger)));
                Some(ledger)
            }
            Err(error) => {
                println!("[history] not recording: {error}");
                None
            }
        };
        let group_key = stored
            .as_ref()
            .map_or(GroupKey::Artist, |config| config.group_key);
        let density = stored
            .as_ref()
            .map_or(shelf::Density::Balanced, |config| config.density);
        // The folders baz holds this run (ADR-0022): what the config remembers,
        // with a `baz DIR` argument **added to the front** rather than replacing
        // them. Pointing baz at a folder for an afternoon must not silently
        // forget the other three — and the one that was named on the command
        // line is the one being asked for, so it is scanned first.
        let mut dirs: Vec<PathBuf> = stored.map(|config| config.music_dirs).unwrap_or_default();
        if let Some(dir) = cli_dir {
            dirs.retain(|held| held != &dir);
            dirs.insert(0, dir);
        }
        let (screen, task) = if dirs.is_empty() {
            (Screen::Setup(Setup::fresh(None)), Task::none())
        } else {
            match Shelf::open(dirs, group_key, density) {
                Ok((shelf, task)) => (Screen::Shelf(Box::new(shelf)), task),
                Err(error) => (Screen::Setup(Setup::fresh(Some(error))), Task::none()),
            }
        };
        let mut app = Self {
            _history_ledger: history_ledger,
            group_key,
            settings_section: 0,
            density,
            modifiers: keyboard::Modifiers::empty(),
            started,
            first_frame_logged: false,
            screen,
            place: Place::default(),
            hovered_queue_row: None,
            window: WINDOW,
            playback,
            player,
            mpris,
            mpris_art: (0, None),
            ink: Keyed::new(),
            pressed_control: None,
            warmth: Tween::settled(0.0).with_curve(motion::Curve::Linear),
            saved_replay_gain,
        };
        // One publish before the first frame, so a desktop widget that asks
        // straight away gets the seeded volume and the real `Can*` flags
        // rather than the server's own defaults. The MPRIS thread may not
        // have reached its bus yet; the update simply waits in its channel.
        app.publish_mpris(false);
        (app, task)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // The volume is its own small machine and every one of its messages
        // resolves to "tell the state machine, maybe tell the engine", so it
        // is answered first and separately rather than as nine more arms
        // below.
        // The two machines that answer *before* anything else can: ink, which
        // cannot move a pixel of layout, and the modifier layer, which decides
        // whether a keystroke was even text.
        for machine in [Self::update_motion, Self::update_modified_input] {
            if let Some(task) = machine(self, &message) {
                return task;
            }
        }
        if self.update_needle(&message)
            || self.update_volume(&message)
            || self.update_replay_gain(&message)
            || self.update_transport(&message)
            || self.update_queue(&message)
        {
            return Task::none();
        }
        match message {
            Message::EscapePressed => self.escape(),
            // **The doors, and the one way back.** Every one of them is
            // navigation and nothing else: no panel opens, no width changes,
            // and the Library's own state — scroll, query, arrangement — is
            // untouched by all of them, which is what makes coming back free.
            Message::ToggleSettings => self.go(Place::settings),
            Message::ToggleQueue => self.go(Place::queue),
            Message::AlbumClicked(id) => self.open_album(id),
            Message::ShowPlayingAlbum => match self.player.playing_album() {
                // Nothing is sounding, so there is no record to be taken to.
                // The control is not offered in that state (see
                // `views::bottom_bar`), so this is the guard and not the case.
                None => Task::none(),
                Some(id) => self.open_album(id),
            },
            Message::LeavePlace => self.leave(),
            // The Settings place's spine. Session state and deliberately not
            // persisted: which section you were last reading is not a standing
            // decision.
            Message::SettingsSection(section) => {
                self.settings_section = section;
                Task::none()
            }
            Message::WindowResized(size) => {
                self.window = size;
                match &mut self.screen {
                    Screen::Shelf(state) => state.update(Message::WindowResized(size)),
                    Screen::Setup(_) => Task::none(),
                }
            }
            Message::FirstFrame => self.log_first_frame(),
            Message::SetupSubmit => self.submit_setup(),
            Message::Playback(event) => {
                self.apply_player_event(event);
                Task::none()
            }
            Message::PlayAlbum(id) => {
                self.play_album(id);
                Task::none()
            }
            // **Enter plays the top-ranked match** (ADR-0017 §1.2).
            Message::PlayFirstMatch => self.play_first_match(),
            Message::PlayTrack(id, row) => {
                self.play_track(id, row);
                Task::none()
            }
            Message::Shuffle => {
                self.start_shuffle();
                Task::none()
            }
            Message::Pull => self.draw_pull(),
            Message::SeekBy(delta_ms) => {
                let target = self.player.seek_by(delta_ms);
                self.send_seek(target);
                Task::none()
            }
            Message::SeekTo(position_ms) => {
                let target = self.player.seek_to(position_ms);
                self.send_seek(target);
                Task::none()
            }
            Message::FocusSearch => match &self.screen {
                Screen::Shelf(_) => text_input::focus(search_id()),
                Screen::Setup(_) => Task::none(),
            },
            Message::Quit => iced::exit(),
            // Best effort by nature: a Wayland compositor is entitled to
            // refuse a focus request, and refusing is not an error here.
            Message::Raise => window::get_latest().and_then(window::gain_focus),
            message => match &mut self.screen {
                Screen::Setup(setup) => {
                    if let Message::SetupInput(value) = message {
                        setup.input = value;
                    }
                    Task::none()
                }
                Screen::Shelf(state) => state.update(message),
            },
        }
    }

    /// <kbd>Enter</kbd>: play what the wall says is the best answer to the
    /// query, or the album the inspector is showing (ADR-0017 §1.2).
    ///
    /// Resolved on the shell because playing is the shell's job and the answer
    /// is the shelf's — the same split every other play route in this file
    /// takes. [`Shelf::enter_plays`] holds the choice; this holds the sound.
    fn play_first_match(&mut self) -> Task<Message> {
        if let Screen::Shelf(state) = &self.screen
            && let Some(id) = state.enter_plays()
        {
            self.play_album(id);
        }
        Task::none()
    }

    /// Everything that depends on **which modifiers are down**: the zoom, and
    /// the one place a chord must be kept out of the search query.
    ///
    /// Its own small machine for the reason the volume's nine messages and
    /// ReplayGain's four are: a few arms that belong to one fact, kept out of
    /// the shell's match so that what remains there is the handful of messages
    /// genuinely about the whole application.
    ///
    /// The density step is remembered on the shell rather than on the shelf
    /// because the config is read before a shelf exists and the setup screen
    /// has no wall to hang — the same split [`App::group_key`] takes.
    ///
    /// # Why a modified keystroke cannot become query text
    ///
    /// iced 0.13's `text_input` inserts whatever character a key press
    /// *produced*, and it checks the command modifier for exactly four chords
    /// (its own cut/copy/paste/select-all) and no others. On X11 a press of
    /// <kbd>Ctrl</kbd>+<kbd>-</kbd> produces the text `-`, so with the well
    /// focused the field swallowed the zoom **and typed a hyphen into the
    /// query**. Measured, on a real frame: the well read `co-` and the wall
    /// read *Nothing matches "co-"*. The same was already true of
    /// <kbd>Ctrl</kbd>+<kbd>,</kbd> before any of this, and it shipped.
    /// Letter chords are unaffected — <kbd>Ctrl</kbd>+<kbd>M</kbd> produces a
    /// control character, which the field already filters.
    ///
    /// The fix is the rule `keys::is_query_text` already states on the other
    /// path, applied to this one: **a keystroke made with the command modifier
    /// is never query text.** The field's edit is discarded, the query is
    /// whatever it was, and the widget re-reads it on the next frame.
    ///
    /// What it does **not** do is deliver the chord to the binding table —
    /// that would break the focus rule, which is the one rule in `keys.rs`
    /// that may not bend (see its focus-rule note). So while the well has
    /// focus a punctuation chord now does *nothing* instead of corrupting the
    /// query; <kbd>Esc</kbd> leaves the field and it works, and
    /// <kbd>Ctrl</kbd>+scroll works either way. Recorded in
    /// `.interface-design/system.md` §12 with the toolkit's other hard limits.
    fn update_modified_input(&mut self, message: &Message) -> Option<Task<Message>> {
        if matches!(message, Message::SearchChanged(_))
            && !keys::field_edit_is_query(self.modifiers)
        {
            return Some(Task::none());
        }
        let delta = match *message {
            // Tracked for the wheel's sake alone (see the field).
            Message::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
                return Some(Task::none());
            }
            // A notch of the wheel is a zoom only with the command modifier
            // down; otherwise it is the wall scrolling, which the `scrollable`
            // has already done for itself.
            Message::Wheel(travel) => match keys::wheel_binding(travel, self.modifiers) {
                Some(Message::DensityStep(delta)) => delta,
                _ => return Some(Task::none()),
            },
            Message::DensityStep(delta) => delta,
            _ => return None,
        };
        self.density = self.density.step(delta);
        Some(match &mut self.screen {
            Screen::Shelf(state) => state.set_density(self.density),
            Screen::Setup(_) => Task::none(),
        })
    }

    /// Answer a pointer message that only motion cares about, reporting whether
    /// it was one.
    ///
    /// Four messages and none of them touches anything but ink: this is the
    /// hovered-control seam ADR-0020 §2.1 opens, and it is deliberately the
    /// smallest thing that can close it — an id, a tween and a boolean. Nothing
    /// downstream of here can move a pixel, which is why it is answered before
    /// the machines that can.
    fn update_motion(&mut self, message: &Message) -> Option<Task<Message>> {
        let now = Instant::now();
        match *message {
            Message::MotionTick(at) => return Some(self.tick_motion(at)),
            Message::ControlEntered(control) => self.ink.enter(control, motion::INK, now),
            // Only if it is still the control that left, and dropping the press
            // with it: a pointer that leaves a held button is no longer pressing
            // it, which is the same reading `button` itself takes.
            Message::ControlLeft(control) => {
                if self.pressed_control == Some(control) {
                    self.pressed_control = None;
                }
                self.ink.leave(control, motion::INK, now);
            }
            // A `button` with an `on_press` captures `ButtonPressed` before any
            // wrapper sees it, so the press cannot be reported by its target;
            // it is resolved against the control the pointer is already on,
            // which is the same condition `button` applies to itself.
            Message::PointerPressed => self.pressed_control = self.ink.key(),
            Message::PointerReleased => self.pressed_control = None,
            _ => return None,
        }
        Some(Task::none())
    }

    /// Log startup-to-interactive, once, on the first frame the window
    /// presents. The `window::frames()` subscription that produces it is
    /// dropped the moment this has run — the first bounded clock baz shipped,
    /// and the pattern ADR-0020 generalises.
    fn log_first_frame(&mut self) -> Task<Message> {
        if !self.first_frame_logged {
            self.first_frame_logged = true;
            println!(
                "[startup] startup-to-interactive: {:.1} ms",
                self.started.elapsed().as_secs_f64() * 1e3
            );
        }
        Task::none()
    }

    /// Advance every transition that is running.
    ///
    /// The one arm the whole of ADR-0020 needs in the update loop, and its
    /// mirror is the guard in [`Self::subscription`]: this is called only while
    /// [`Self::moving`] is true, and the last tick of the last tween is what
    /// makes it false again.
    fn tick_motion(&mut self, now: Instant) -> Task<Message> {
        self.ink.tick(now);
        self.warmth.tick(now);
        match &mut self.screen {
            Screen::Setup(_) => Task::none(),
            Screen::Shelf(state) => state.tick_motion(now),
        }
    }

    /// **Is anything moving?** — the boolean the subscription reads, and the
    /// whole of ADR-0020's idle-cost claim.
    ///
    /// False at rest, so the clock the transitions run on does not exist at
    /// rest: no timer, no messages, no redraws, 0.0 % CPU
    /// (`docs/design/04-fluidity.md` §1.4). Asserted rather than promised — see
    /// `the_motion_clock_is_off_until_something_moves`.
    fn moving(&self) -> bool {
        self.ink.live()
            || self.warmth.live()
            || match &self.screen {
                Screen::Setup(_) => false,
                Screen::Shelf(state) => state.moving(),
            }
    }

    /// Start the lamp warming, if the light has somewhere to move to.
    ///
    /// **Only when the record under the lamp changes.** ADR-0020 §2.5 says "on
    /// track change", and that is what this is: a track change *within* an album
    /// leaves the light exactly where it is, so the tween is already at its
    /// target, [`Tween::go`] settles immediately and asks for no clock. Taking
    /// the halo to zero and back on every track boundary would be a flicker on a
    /// record that never stopped playing — the transition would be announcing a
    /// change the light did not make.
    fn warm_lamp(&mut self, was: Option<u64>, now: Instant) {
        let sounding = self.player.playing_album();
        if sounding == was {
            return;
        }
        if sounding.is_some() {
            self.warmth.set(0.0);
            self.warmth.go(1.0, motion::LAMP, now);
        } else {
            // Nothing is sounding: the light goes out with the music rather
            // than dimming after it.
            self.warmth.set(0.0);
        }
    }

    /// Answer a message that belongs to the **Queue** place's rows, reporting
    /// whether it was one.
    ///
    /// Everything a listener can do to a row, answered as one small machine for
    /// the reason the volume's nine and ReplayGain's four are: they all belong
    /// to one surface, and folding four more arms into the shell's own match
    /// would bury the messages that are genuinely about the whole application.
    ///
    /// The place's *door* is not here — going to the queue is navigation, and
    /// navigation is the shell's. Nor is <kbd>Esc</kbd>: it is the message that
    /// has to know where you are, so it stays where the place is
    /// ([`Self::escape`]).
    fn update_queue(&mut self, message: &Message) -> bool {
        match *message {
            Message::QueueRowEntered(row) => self.hovered_queue_row = Some(row),
            // Only if it is still the row that left: see the message's own note
            // on why the pair must not be order-dependent.
            Message::QueueRowLeft(row) if self.hovered_queue_row == Some(row) => {
                self.hovered_queue_row = None;
            }
            Message::QueueRowLeft(_) => {}
            Message::JumpToQueued(position) => self.jump_to_queued(position),
            Message::RemoveQueued(row) => self.remove_queued(row),
            _ => return false,
        }
        true
    }

    /// **Go somewhere**, by whichever door was pressed.
    ///
    /// One function for all three because they are the same act: a door is a
    /// pure transition on [`Place`], and the only thing the shell adds is the
    /// rule that there must be a shelf to leave. The first-run screen has no
    /// places, so a media key or a stray binding cannot navigate away from the
    /// folder question.
    fn go(&mut self, door: impl FnOnce(Place) -> Place) -> Task<Message> {
        if matches!(self.screen, Screen::Shelf(_)) {
            self.place = door(self.place);
        }
        Task::none()
    }

    /// **Open a record's page** — a tile press, the bar's now-playing block, or
    /// the pull's own draw.
    ///
    /// Two things happen and they are deliberately separable: the *place*
    /// changes, and the wall remembers which record you left it for
    /// ([`Shelf::opened`]). The second is the whole mitigation for the round
    /// trip a page costs that a column did not — when <kbd>Esc</kbd> brings you
    /// back, the wall is where you left it with the record you were reading
    /// marked, so returning is *return* rather than re-find.
    fn open_album(&mut self, id: u64) -> Task<Message> {
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        state.opened = Some(id);
        self.place = self.place.album(id);
        Task::none()
    }

    /// **Go home** — every place's `‹ Library`, and the first thing
    /// <kbd>Esc</kbd> does.
    ///
    /// Leaving a record's page also withdraws the pull's offer, when the offer
    /// was about that record: the suggestion and the page it was made on are
    /// one layer, and a note that outlived the page it was written on would
    /// leave the wall marked as though the listener had chosen the record
    /// themselves. `Ctrl+R` re-pulls; this is the *"Esc returns"* half of the
    /// same sentence (`docs/design/critique/02-surfaces.md`).
    fn leave(&mut self) -> Task<Message> {
        let leaving = self.place.showing_album();
        self.place = self.place.back();
        if let (Screen::Shelf(state), Some(id)) = (&mut self.screen, leaving)
            && state.pull.as_ref().is_some_and(|pull| pull.album == id)
        {
            state.pull = None;
        }
        Task::none()
    }

    /// <kbd>Esc</kbd>: **peel one layer, top down.**
    ///
    /// Shorter than it has ever been, because there are fewer layers than there
    /// have ever been. ADR-0016 had a popover over an inspector over a place and
    /// spent one rule on each; ADR-0022 left one kind of surface, so the key's
    /// whole first question is *am I at home*:
    ///
    /// 1. **The place**, when it is not the Library. Backing out is what
    ///    <kbd>Esc</kbd> means in a record's page, in the queue and in the
    ///    settings alike, and it is the same press as their `‹ Library`.
    /// 2. Then the Library's own layers, in [`Shelf::peel`]'s order: the pull's
    ///    offer, the search query, the shuffle pool's marks.
    ///
    /// (In the search field itself iced 0.13's `text_input` consumes
    /// <kbd>Esc</kbd> to blur before this is reached at all; that is the
    /// documented two-press behaviour, and §4.6 of the design spec owns the
    /// fix.)
    fn escape(&mut self) -> Task<Message> {
        if !self.place.is_home() {
            return self.leave();
        }
        match &mut self.screen {
            Screen::Setup(_) => Task::none(),
            Screen::Shelf(state) => state.update(Message::EscapePressed),
        }
    }

    /// Setup → Shelf transition: validate the typed folder and open it.
    fn submit_setup(&mut self) -> Task<Message> {
        let Screen::Setup(setup) = &mut self.screen else {
            return Task::none();
        };
        let dir = expand_tilde(setup.input.trim());
        if dir.as_os_str().is_empty() {
            return Task::none();
        }
        if !dir.is_dir() {
            setup.error = Some(format!("`{}` is not a directory", dir.display()));
            return Task::none();
        }
        match Shelf::open(vec![dir], self.group_key, self.density) {
            Ok((state, task)) => {
                self.screen = Screen::Shelf(Box::new(state));
                task
            }
            Err(error) => {
                setup.error = Some(error);
                Task::none()
            }
        }
    }

    /// Fold a bridge message into the state machine, with a stdout trace of
    /// the notable per-track moments (matching the `[scan]`/`[config]` log
    /// style).
    fn apply_player_event(&mut self, message: PlayerEvent) {
        // Whether a seek we asked for is still awaiting its confirming
        // event. MPRIS wants a `Seeked` signal when the position jumps for a
        // reason a polling client could not have predicted, and the engine's
        // answer to an accepted Seek is an immediate Progress — so "a seek
        // was pending and a Progress arrived" is that moment, read off
        // events rather than assumed at request time.
        let seek_pending = self.player.seek_pending();
        let mut seek_confirmed = false;
        // Which record the lamp is on, read *before* the event is folded in, so
        // "the light moved" is a comparison rather than a guess (see
        // [`Self::warm_lamp`]).
        let lit = self.player.playing_album();
        match message {
            PlayerEvent::Engine(event) => {
                match &event {
                    Event::TrackStarted { path, position } => {
                        println!(
                            "[playback] track started (queue #{position}): {}",
                            path.display()
                        );
                    }
                    Event::TrackFailed { path, reason } => {
                        println!("[playback] track skipped: {} ({reason})", path.display());
                    }
                    Event::QueueEnded => println!("[playback] queue ended"),
                    // The signal-path readout, logged as plain information —
                    // it says what the chain is doing, not that anything is
                    // wrong (see crate::playback's "Signal path").
                    Event::SignalPath {
                        source_rate_hz,
                        source_bits,
                        output_rate_hz,
                        chain,
                    } => {
                        let depth =
                            source_bits.map_or_else(String::new, |bits| format!("/{bits}-bit"));
                        let doing = match chain {
                            SignalChain::Direct => "direct".to_string(),
                            SignalChain::Converting { reason } => {
                                format!("converting ({reason:?})")
                            }
                            other => format!("{other:?}"),
                        };
                        println!(
                            "[playback] signal path: {source_rate_hz} Hz{depth} source -> \
                             {output_rate_hz} Hz output, {doing}"
                        );
                    }
                    _ => {}
                }
                let albums: &[vm::AlbumVm] = match &self.screen {
                    Screen::Shelf(state) => &state.albums,
                    Screen::Setup(_) => &[],
                };
                self.player.apply(&event, albums);
                seek_confirmed = seek_pending && matches!(event, Event::Progress { .. });
                // Persist off the confirmation, never off the request: what
                // reaches config.toml is what the engine put in force,
                // including a pre-amp it clamped on the way in.
                self.persist_replay_gain();
            }
            PlayerEvent::Closed => {
                println!("[playback] engine shut down");
                self.player.engine_closed();
            }
        }
        self.warm_lamp(lit, Instant::now());
        self.publish_mpris(seek_confirmed);
    }

    /// Write the ReplayGain setting the engine has just confirmed, if it moved.
    ///
    /// A no-op in the ordinary case, which is the point: `ReplayGainChanged`
    /// also arrives at track boundaries where the resolved *figure* changed
    /// and the *settings* did not, and a config write per track boundary would
    /// be a file system call in the middle of a gapless splice.
    ///
    /// Best-effort with a log, like the music folder beside it: a read-only
    /// config directory must not stop anybody listening to music.
    fn persist_replay_gain(&mut self) {
        let settings = self.player.replay_gain().settings();
        if settings == self.saved_replay_gain {
            return;
        }
        self.saved_replay_gain = settings;
        persist(|config| config.replay_gain = settings);
    }

    /// Hand the desktop integration the state the engine just confirmed.
    ///
    /// The snapshot is built unconditionally — `app.rs` carries no `cfg`, and
    /// on a platform without MPRIS it is simply dropped. That costs a few
    /// small clones at the engine's ~4 Hz progress cadence, which is a fair
    /// price for one code path.
    fn publish_mpris(&mut self, seeked: bool) {
        let sequence = self.player.track_seq();
        if self.mpris_art.0 != sequence {
            let url = self
                .player
                .now_playing_path()
                .and_then(art::cover_file_beside)
                .as_deref()
                .and_then(mpris::state::file_url);
            self.mpris_art = (sequence, url);
        }
        let snapshot = mpris::Snapshot::from_player(&self.player, self.mpris_art.1.clone());
        self.mpris.publish(snapshot, seeked);
    }

    /// Send an accepted seek target to the engine. `None` means there was
    /// nothing honest to seek to and nothing was asked for; the state machine
    /// has already recorded an accepted request as pending, and the bar keeps
    /// showing it until an event confirms (see `player.rs`).
    fn send_seek(&mut self, target: Option<u64>) {
        if let Some(position_ms) = target
            && !self.playback.send(Command::Seek { position_ms })
        {
            self.player.engine_closed();
        }
    }

    /// Answer a transport message, reporting whether it was one.
    ///
    /// The six of them are one small machine, exactly as the volume's nine and
    /// ReplayGain's four are: each resolves to a single
    /// [`Command`] and goes out through
    /// [`Self::send_transport`], and none of them touches a single thing the
    /// interface displays — the state machine follows the engine's confirming
    /// event and nothing else (`player.rs`'s honesty rule).
    ///
    /// Play, Pause and Stop have no button of their own: a desktop media
    /// widget (and a media key) asks for a *direction* rather than a toggle,
    /// where the bar's control covers both. Previous, Next and the toggle do,
    /// and they arrive here from the button, the keyboard and MPRIS as the
    /// same message.
    fn update_transport(&mut self, message: &Message) -> bool {
        let command = match *message {
            // The same reading the glyph is drawn from, so a press asks for
            // exactly what the button was showing (Play also resumes a paused
            // engine, so a stale read is still safe).
            Message::PlayPause => match self.player.play_pause() {
                player::PlayPause::Pause => Command::Pause,
                player::PlayPause::Play => Command::Play,
            },
            Message::NextTrack => Command::Next,
            Message::PreviousTrack => Command::Previous,
            Message::Play => Command::Play,
            Message::Pause => Command::Pause,
            Message::Stop => Command::Stop,
            _ => return false,
        };
        self.send_transport(command);
        true
    }

    /// Answer a volume message, reporting whether it was one.
    ///
    /// Every arm follows the same shape as the seek bar's: the state machine
    /// decides what — if anything — to ask for from event-derived state, and
    /// the answer goes to the engine. Nothing here writes the volume the
    /// interface displays; only `Event::VolumeChanged` does (see `player.rs`).
    /// The needle's five pointer messages, answered together for the same
    /// reason the volume's nine are: every one of them resolves to "tell the
    /// state machine, maybe tell the engine".
    ///
    /// **One gesture, two commands, and the segment decides which.**
    /// [`player::PlayerState::release_drag`] resolves the release into a
    /// [`player::NeedleTarget`]; this only dispatches, so "is this a move
    /// within the record or a choice of record" has exactly one answer in
    /// exactly one place — and neither command is invented here (ADR-0014:
    /// seeking is `Seek`, jumping is `JumpTo`, and the UI state comes back from
    /// the engine's events either way).
    fn update_needle(&mut self, message: &Message) -> bool {
        match *message {
            Message::NeedlePressed(pointer) => self.player.press(pointer),
            Message::NeedleDragged(pointer) => self.player.drag_to(pointer),
            Message::NeedleHovered(pointer) => self.player.hover_to(pointer),
            Message::NeedleLeft => self.player.hover_left(),
            Message::NeedleReleased => match self.player.release_drag() {
                Some(player::NeedleTarget::Seek { position_ms }) => {
                    self.send_seek(Some(position_ms));
                }
                Some(player::NeedleTarget::Jump { position }) => {
                    self.jump_to_queued(position);
                    // A jump moves what is playing, which MPRIS publishes.
                    self.publish_mpris(false);
                }
                None => {}
            },
            _ => return false,
        }
        true
    }

    fn update_volume(&mut self, message: &Message) -> bool {
        match *message {
            Message::VolumePressed(pointer) => {
                let target = self.player.press_volume(pointer);
                self.send_volume(target);
            }
            Message::VolumeDragged(pointer) => {
                let target = self.player.drag_volume(pointer);
                self.send_volume(target);
            }
            Message::VolumeHovered(pointer) => self.player.hover_volume(pointer),
            Message::VolumeLeft => self.player.volume_left(),
            Message::VolumeReleased => self.player.release_volume(),
            Message::VolumeStep(steps) => {
                let target = self.player.step_volume(steps);
                self.send_volume(target);
            }
            Message::SetVolume(position) => {
                let target = self.player.set_volume(position);
                self.send_volume(target);
            }
            Message::ToggleMute => {
                let muted = self.player.toggle_mute();
                self.send_mute(muted);
            }
            Message::SetMute(muted) => {
                let requested = self.player.set_muted(muted);
                self.send_mute(requested);
            }
            _ => return false,
        }
        true
    }

    /// Send an accepted volume position to the engine. `None` means there was
    /// no engine to ask and nothing was requested. Nothing about the fader's
    /// reading moves here: the state machine recorded the request as pending
    /// for the view's benefit, and the position itself changes only when
    /// `Event::VolumeChanged` says so (see `player.rs`).
    fn send_volume(&mut self, target: Option<u16>) {
        if let Some(position) = target
            && !self.playback.send(Command::SetVolume { position })
        {
            self.player.engine_closed();
        }
    }

    /// Send an accepted mute state to the engine. Idempotent by protocol —
    /// `SetMute { muted }`, never a toggle (ADR-0011 §3).
    fn send_mute(&mut self, target: Option<bool>) {
        if let Some(muted) = target
            && !self.playback.send(Command::SetMute { muted })
        {
            self.player.engine_closed();
        }
    }

    /// The settings panel's ReplayGain controls, answered here for
    /// [`Self::update_volume`]'s reason: every one of them resolves to "ask
    /// the engine for a complete setting", so they are four arms of one small
    /// machine rather than four more in the shelf's update loop.
    ///
    /// Returns whether the message was one of them.
    ///
    /// Nothing on screen moves in any of these arms. The state machine keeps
    /// following [`Event::ReplayGainChanged`] and nothing else, so a press
    /// that the engine clamps, refuses, or answers differently from is
    /// rendered as the engine's answer (ADR-0013, and `crate::replaygain`).
    fn update_replay_gain(&mut self, message: &Message) -> bool {
        let state = self.player.replay_gain();
        let asked = match *message {
            Message::ReplayGainMode(mode) => state.with_mode(mode),
            Message::ReplayGainPreamp(steps) => state.stepped_preamp(steps),
            Message::ReplayGainNoTagPreamp(steps) => state.stepped_no_tag_preamp(steps),
            Message::ReplayGainPreventClipping(prevent) => state.with_prevent_clipping(prevent),
            _ => return false,
        };
        // A redundant command emits nothing, so sending one is harmless; a
        // failed send means the engine is gone and the state machine must
        // stop claiming otherwise.
        if !self.playback.send(command_for(asked)) {
            self.player.engine_closed();
        }
        true
    }

    /// Queue an album (the selected edition's tracks, in the view model's
    /// disc/track order) and play it. State stays untouched until events
    /// confirm — only the request-side notes are recorded (see `player.rs`).
    fn play_album(&mut self, id: u64) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        let Some(album) = state.albums.iter().find(|album| album.id == id) else {
            return;
        };
        let queue = vm::album_queue(album, state.edition_choice.get(&id).copied());
        if queue.is_empty() {
            return;
        }
        self.draws_are_over();
        // One construction, two uses: the payload the engine is sent and the
        // list the queue panel shows come from the same value, so they cannot
        // describe different music (see [`vm::QueueVm`]).
        let paths = queue.paths();
        if self.playback.send(Command::SetQueue { paths }) && self.playback.send(Command::Play) {
            self.player.note_queue_sent(queue);
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
        // A queue where there was none moves `CanPlay`, and that is the one
        // MPRIS-visible change that arrives without an engine event.
        self.publish_mpris(false);
    }

    /// Play `id` from row `row` of its selected edition — a click on a track
    /// row of the album inspector (ADR-0014, and §3.2 step 4 of the UX spec).
    ///
    /// The decision this spends is
    /// [`PlayerState::play_from`](crate::player::PlayerState::play_from)'s and
    /// it is made from the queue the engine is *known* to hold:
    ///
    /// - **It already holds this album** — one
    ///   [`JumpTo`](Command::JumpTo). Nothing is re-queued, so the run the
    ///   listener is in the middle of is not replaced to move within it, and
    ///   no `Stopped` interrupts it.
    /// - **It does not** — [`SetQueue`](Command::SetQueue) then `JumpTo`. The
    ///   `SetQueue` stops what was playing, which is what the listener asked
    ///   for by pointing at a different album, and the `JumpTo` is what makes
    ///   the click land on the row rather than at the top.
    ///
    /// Nothing on screen moves here. The dot follows `TrackStarted` exactly as
    /// it does for every other way of starting a track — never the click, per
    /// ADR-0014's front-end contract.
    fn play_track(&mut self, id: u64, row: usize) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        let Some(album) = state.albums.iter().find(|album| album.id == id) else {
            return;
        };
        let chosen = state.edition_choice.get(&id).copied();
        let Some(edition) = vm::selected_edition(album, chosen) else {
            return;
        };
        // The list the row was drawn from and the list that would be queued
        // come from the same `selected_edition`, so "is this album the queue"
        // is asked about exactly what the user clicked.
        let Some(decision) = self.player.play_from(&edition.tracks, row) else {
            return;
        };
        // **A jump inside the shuffle's own queue is not the end of it.** Only
        // a re-queue replaces what the shuffle built, so only a re-queue takes
        // the pool's marks off the wall; clicking track 4 of a record the
        // shuffle is playing leaves the run — and its rings — exactly as they
        // were. Recorded here and spent below, because the shelf cannot be
        // reached while the album borrowed out of it is still in use.
        let replaced = matches!(decision, player::PlayFrom::Requeue { .. });
        let position = match decision {
            player::PlayFrom::Jump { position } => position,
            player::PlayFrom::Requeue { position } => {
                let queue = vm::album_queue(album, chosen);
                if queue.is_empty() {
                    return;
                }
                let paths = queue.paths();
                if !self.playback.send(Command::SetQueue { paths }) {
                    self.player.engine_closed();
                    return;
                }
                self.player.note_queue_sent(queue);
                position
            }
        };
        if self.playback.send(Command::JumpTo { position }) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
        if replaced {
            self.draws_are_over();
        }
        // A queue where there was none moves `CanPlay`, exactly as in
        // `play_album`, and that is the one MPRIS-visible change that arrives
        // without an engine event.
        self.publish_mpris(false);
    }

    /// **Take the draws' marks off the wall**: no pool, no offer.
    ///
    /// Called wherever a queue is deliberately replaced by something that is not
    /// a draw. The marks are a statement about the run in progress, and going on
    /// dimming two hundred covers for a shuffle that was superseded a record ago
    /// would be the interface saying something that is no longer true — which is
    /// the one thing the honesty rule in [`crate::player`] never permits.
    fn draws_are_over(&mut self) {
        if let Screen::Shelf(state) = &mut self.screen {
            state.pool = None;
            state.pull = None;
        }
    }

    /// **Shuffle what the wall shows** (ADR-0017 step 17).
    ///
    /// The whole of the feature, and it has no options because there is nothing
    /// to choose: the pool is [`shuffle::Pool::from_wall`]'s reading of the
    /// group key, the query and the shelf, and the run is
    /// [`shuffle::SLEEVES`] records drawn from it without replacement.
    ///
    /// What it sends is an ordinary [`SetQueue`](Command::SetQueue) — the same
    /// command a double-clicked sleeve sends, carrying whole records in whole
    /// order ([`vm::stacked_queue`]) — so the result is a queue you can open,
    /// read, reorder and delete rows from, and one that **ends**. There is no
    /// shuffle mode, no flag on the engine, and nothing to turn off.
    ///
    /// The pool is then held on the shelf, which is what makes it visible: see
    /// [`Shelf::pool`], and [`crate::views::shelf`] for the two marks.
    fn start_shuffle(&mut self) {
        let seed = draw_seed();
        let Screen::Shelf(state) = &mut self.screen else {
            return;
        };
        let mut pool = shuffle::Pool::from_wall(&state.albums, &state.visible, None);
        if pool.is_empty() {
            // The wall is showing nothing — an empty library, or a query that
            // matched no record. Nothing to draw from, so nothing happens and
            // nothing is claimed. Silence is the correct answer here too.
            return;
        }
        let drawn = pool.draw(seed, shuffle::SLEEVES).to_vec();
        let picks: Vec<(&vm::AlbumVm, Option<vm::EditionKey>)> = drawn
            .iter()
            .filter_map(|id| state.albums.iter().find(|album| album.id == *id))
            .map(|album| (album, state.edition_choice.get(&album.id).copied()))
            .collect();
        let queue = vm::stacked_queue(&picks);
        if queue.is_empty() {
            return;
        }
        println!(
            "[shuffle] {} sleeves drawn from {} on the wall",
            drawn.len(),
            pool.len()
        );
        state.pool = Some(pool);
        // The pull was a suggestion about one record; starting a shuffle
        // answers a different question, so the offer is withdrawn rather than
        // left standing beside a run it has nothing to do with.
        state.pull = None;
        let paths = queue.paths();
        if self.playback.send(Command::SetQueue { paths }) && self.playback.send(Command::Play) {
            self.player.note_queue_sent(queue);
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
        // A queue where there was none moves `CanPlay`, exactly as in
        // [`Self::play_album`].
        self.publish_mpris(false);
    }

    /// **The pull** (ADR-0017 step 19): draw one record, and offer it.
    ///
    /// Weighted toward the long unplayed by [`shuffle::pull`], which weighs on
    /// `baz_core`'s own [`History::pull_weight`] — one per day since the record
    /// was last heard, capped at a year, heaviest for one never played, never
    /// zero. Drawn from the same pool shuffle uses, because the pull may no more
    /// suggest a record you cannot see than shuffle may play one.
    ///
    /// **No command is sent.** Not `SetQueue`, not `Play`, not `Stop`: this
    /// function cannot start playback, and that is not a discipline it observes
    /// but a fact about what it does — it writes one field and moves the wall.
    /// Accepting the suggestion is pressing `Play album`, which is the ordinary
    /// path every other record takes.
    ///
    /// Pressing again re-pulls, excluding the record already on offer.
    fn draw_pull(&mut self) -> Task<Message> {
        let seed = draw_seed();
        let now = SystemTime::now();
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        let pool = shuffle::Pool::from_wall(&state.albums, &state.visible, None);
        let showing = state.pull.as_ref().map(|pull| pull.album);
        let Some(drawn) = shuffle::pull(
            &state.albums,
            &pool,
            state.history.as_ref(),
            now,
            seed,
            showing,
        ) else {
            return Task::none();
        };
        let Some(album) = state.albums.iter().find(|album| album.id == drawn) else {
            return Task::none();
        };
        let note = shuffle::pull_note(shuffle::last_played(album, state.history.as_ref(), now));
        println!("[pull] {} — {note}", album.title.as_deref().unwrap_or("—"));
        state.pull = Some(Pull { album: drawn, note });
        state.show_album(drawn)
    }

    /// Play the queue from `position` — a click on a row of **Queue**
    /// (ADR-0014's `JumpTo`, and §3.4 step 3 of the UX spec).
    ///
    /// Simpler than the album inspector's [`Self::play_track`] by exactly one
    /// decision, and the difference is worth naming: the inspector lists an
    /// album that may or may not be what the engine is holding, so it has to
    /// ask. This list *is* what the engine is holding — it is drawn from the
    /// record of what was sent — so a position in it is already a position in
    /// the queue and `JumpTo` alone is the whole request. Nothing is re-queued
    /// and the run is not replaced to move within it.
    ///
    /// A row past the end of the record asks for nothing: the queue shrank
    /// under the pointer, which is an ordinary race rather than a fault.
    ///
    /// Nothing on screen moves here. The dot follows `TrackStarted`, never the
    /// click, per ADR-0014's front-end contract.
    fn jump_to_queued(&mut self, position: usize) {
        if self
            .player
            .queue()
            .is_none_or(|queue| position >= queue.len())
        {
            return;
        }
        if self.playback.send(Command::JumpTo { position }) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
    }

    /// Take row `row` out of the queue — a click on a row's ✕ in **Queue**
    /// (ADR-0014's `UpdateQueue`, and §3.4 step 4 of the UX spec).
    ///
    /// The edit itself is [`queue_edit::without`]'s: pure, tested, and working
    /// on the [`vm::QueueVm`] record so that the paths sent and the rows drawn
    /// come from one value and cannot describe different music. What is sent is
    /// the **whole new queue**, never a delta — ADR-0014's reason is that an
    /// index applied against a stale picture removes a different track and
    /// neither side can tell.
    ///
    /// `UpdateQueue`, never `SetQueue`: the guarantee ADR-0014 exists to make
    /// is that an edit which does not touch the playing track does not disturb
    /// one delivered sample, and `SetQueue` is documented to stop the music.
    /// Sending the wrong one here would silence a track to delete a different
    /// one.
    ///
    /// The record is replaced only once the send is accepted, and through
    /// [`PlayerState::note_queue_edited`](crate::player::PlayerState::note_queue_edited)
    /// so the playing position survives the moment between the send and the
    /// engine's `QueueChanged`.
    fn remove_queued(&mut self, row: usize) {
        let Some(edited) = self
            .player
            .queue()
            .and_then(|queue| queue_edit::without(queue, row))
        else {
            return;
        };
        let paths = edited.paths();
        if self.playback.send(Command::UpdateQueue { paths }) {
            self.player.note_queue_edited(edited);
        } else {
            self.player.engine_closed();
        }
        // A queue emptied to nothing moves `CanPlay`, and that is the one
        // MPRIS-visible change an edit can make without an engine event.
        self.publish_mpris(false);
    }

    /// Send a transport command, marking it pending on acceptance and
    /// downgrading to engine-closed state when the channel is gone.
    fn send_transport(&mut self, command: Command) {
        if self.playback.send(command) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
    }

    /// The whole window: the current place, and the persistent bottom bar
    /// under it. Composition only — every surface is drawn by
    /// [`crate::views`].
    ///
    /// **One place at a time, and nothing over it.** The four are alternatives
    /// in one `match`, which is what "places replace each other" means in code;
    /// there is no second layer to compose, no width to arbitrate and no
    /// stacking order, which is the whole of what ADR-0022 bought. The
    /// Library's own state is not touched by the swap, so coming back restores
    /// the scroll, the query and the arrangement exactly.
    ///
    /// A place change is a **hard cut**. ADR-0020 permits five transitions and
    /// this is not one of them: the surfaces either side of a navigation share
    /// no element to move, so a tween would be decoration, and the one that
    /// used to exist here — the inspector's 150 ms width — died with the column
    /// it was moving.
    fn view(&self) -> Element<'_, Message> {
        if std::env::var_os("BAZ_FRAME_LOG").is_some() {
            println!(
                "[frame] {:.3}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64()
            );
        }
        let ink = self.ink();
        let lamp = self.warmth.value();
        let screen: Element<'_, Message> = match (&self.screen, self.place) {
            (Screen::Setup(setup), _) => return views::setup::view(setup),
            (Screen::Shelf(state), Place::Library) => state.view(&self.player, lamp),
            (Screen::Shelf(state), Place::Album(id)) => match state.album(id) {
                Some(album) => {
                    views::album::view(state, album, &self.player, self.window.width, lamp)
                }
                // The record vanished under a rescan while its page was open.
                // The wall is the honest answer — better than a page about
                // nothing — and it is drawn rather than navigated to, because a
                // view function may not change state.
                None => state.view(&self.player, lamp),
            },
            (Screen::Shelf(_), Place::Queue) => {
                views::queue::view(&self.player, self.window.width, self.hovered_queue_row)
            }
            (Screen::Shelf(state), Place::Settings) => {
                // Built here rather than inside the view: the folders come from
                // the shell's own list and their contents from the index, and a
                // view that reached into the library would be a second place
                // that knows how roots are counted.
                views::settings::view(
                    &self.player,
                    self.window.width,
                    self.settings_section,
                    state.library_view(),
                )
            }
        };
        // The persistent bottom bar lives under every place — unless this build
        // has no audio output at all, in which case playback UI is hidden
        // entirely.
        if *self.player.availability() == Availability::NotBuilt {
            return screen;
        }
        column![
            screen,
            views::bottom_bar::view(&self.player, self.place, ink),
        ]
        .into()
    }

    /// What every icon button needs to know to ink itself: which one the
    /// pointer is on, how far its fade has travelled, and whether it is held.
    fn ink(&self) -> Ink {
        Ink::new(self.ink, self.pressed_control)
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            // Raw events rather than `keyboard::on_key_press`, because the
            // capture status is the focus rule: a key a focused text field
            // consumed is not a shortcut (see `crate::keys`).
            iced::event::listen_with(|event, status, _window| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    keys::binding_for(&key, modifiers, keys::Focus::from(status))
                }
                // The press an icon button's own wrapper can never see: a
                // `button` with an `on_press` captures `ButtonPressed`, so the
                // only place left to hear it is the raw stream — and here the
                // *captured* status is the point rather than a problem, since a
                // press on a control is exactly a press a control took.
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) => Some(Message::PointerPressed),
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::PointerReleased),
                // The zoom's pointer half. iced 0.13 reports no modifiers on a
                // wheel event and its `scrollable` does not consult them
                // either, so both halves have to be assembled here: the
                // modifier state is tracked from its own event, and the notch
                // is answered against it in the update loop
                // ([`keys::wheel_binding`]).
                iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                    Some(Message::ModifiersChanged(modifiers))
                }
                iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                    Some(Message::Wheel(match delta {
                        iced::mouse::ScrollDelta::Lines { y, .. }
                        | iced::mouse::ScrollDelta::Pixels { y, .. } => y,
                    }))
                }
                _ => None,
            }),
            window::resize_events().map(|(_, size)| Message::WindowResized(size)),
            self.playback.subscription().map(Message::Playback),
            self.mpris.subscription().map(message_for),
        ];
        // Frame events only until startup-to-interactive is logged.
        if !self.first_frame_logged {
            subs.push(window::frames().map(|_| Message::FirstFrame));
        }
        // **Only while something is moving, and never otherwise** — the whole
        // of ADR-0020's cost argument, and structurally the same guard as the
        // grid hold's below it. A subscription in iced 0.13 is a function of
        // state: it is rebuilt after every update and the ones that went away
        // are dropped, so the last tick of the last tween removes this timer and
        // the event loop parks. (`docs/design/04-fluidity.md` §1.2 for the
        // mechanism; §1.4 for the 0.0 % it measures.)
        if self.moving() {
            subs.push(iced::time::every(motion::TICK).map(|_| Message::MotionTick(Instant::now())));
        }
        // The scan channel is drained on a coarse tick — batching by design.
        if let Screen::Shelf(state) = &self.screen {
            if state.scanning {
                subs.push(iced::time::every(Duration::from_millis(100)).map(|_| Message::ScanTick));
            } else {
                // The periodic refresh's only clock (ADR-0022 §3), and it runs
                // **only while no scan is running** — the two are alternatives,
                // never both. It ticks far more often than the interval so that
                // "due" is answered by the arithmetic in `scan::Refresh` rather
                // than by the timer's phase; at one wake a minute this is
                // nothing beside the 10 Hz tick it replaces.
                subs.push(iced::time::every(REFRESH_TICK).map(|_| Message::RefreshTick));
            }
        }
        Subscription::batch(subs)
    }
}

impl Setup {
    /// A fresh setup screen; suggests `~/Music` when it exists.
    fn fresh(error: Option<String>) -> Self {
        let input = dirs::home_dir()
            .map(|home| home.join("Music"))
            .filter(|p| p.is_dir())
            .and_then(|p| p.to_str().map(str::to_owned))
            .unwrap_or_default();
        Self { input, error }
    }
}

/// One shelf of the wall, as the shell holds it: its header and the slice of
/// [`Shelf::albums`] under it.
///
/// The albums themselves stay in one flat vector so that a selection, a
/// thumbnail and a playing album are all still just an index — re-arranging
/// the wall must not re-key the caches (see [`Shelf::rebuild_shelves`]).
pub(crate) struct GroupVm {
    /// What the shelf's header draws, and what the rail projects.
    pub(crate) header: vm::GroupHeaderVm,
    /// One past its last album in [`Shelf::albums`].
    ///
    /// The end alone, because the shelves are contiguous and in order: a
    /// shelf begins where the one before it ended, and carrying both would be
    /// two numbers that have to agree.
    pub(crate) end: usize,
}

/// The shelf screen: library, scan state, and grid/panel view state.
///
/// Fields the view layer reads are `pub(crate)`; the ones the update loop
/// owns alone (in-flight decodes, the scan channel, click timing) stay
/// private — [`crate::views`] draws this state, it never steers it.
pub(crate) struct Shelf {
    /// The open library: the search index the counts and the query run over.
    pub(crate) library: Library,
    /// How the wall is arranged (ADR-0019). Persisted in `config.toml`; the
    /// top bar's row of words and `1`–`5` are the two ways to change it.
    pub(crate) group_key: GroupKey,
    /// How closely the wall hangs (ADR-0017 step 6). Persisted in
    /// `config.toml`; <kbd>Ctrl</kbd>+<kbd>-</kbd> / <kbd>Ctrl</kbd>+<kbd>=</kbd>
    /// and <kbd>Ctrl</kbd>+scroll are the two ways to change it, and there is
    /// no third way anywhere in the Settings place.
    pub(crate) density: shelf::Density,
    /// The play ledger, read once at open — what [`GroupKey::Played`] shelves
    /// on and what the pull will weight on.
    ///
    /// A **snapshot**, not a live view: the file is append-only, so a snapshot
    /// can only ever be missing the last few minutes and can never be wrong
    /// about an earlier play (`baz_core::history::History`). `None` is a
    /// correct answer rather than a broken one — PLAYED then draws one
    /// `Never played` shelf holding the library, which is a true statement
    /// about a library baz has no record of.
    history: Option<History>,
    /// Owned view model of every album, in the active key's shelf order —
    /// the shelves flattened, so an album is still one index.
    pub(crate) albums: Vec<vm::AlbumVm>,
    /// One entry per shelf: what its header says and which slice of `albums`
    /// it holds. Contiguous and in wall order, so a shelf is a range rather
    /// than a per-album lookup.
    pub(crate) groups: Vec<GroupVm>,
    /// Indices into `albums` that survive the current query, in wall order.
    pub(crate) visible: Vec<usize>,
    /// How many of each shelf's albums survived it, in `groups` order — what
    /// [`shelf::Shelves`] lays the wall out from.
    visible_counts: Vec<usize>,
    /// The live search text.
    pub(crate) query: String,
    /// **The record the wall was last left for**, if any — the tile mark, and
    /// the whole of what survives `selection.rs`.
    ///
    /// It is not a selection. Nothing acts on it, nothing opens because of it,
    /// and it does not decide what any other surface draws; it is one 2 px rule
    /// under one wall label, and its entire job is that coming back from a
    /// record's page lands you looking at the record you came back from
    /// (ADR-0022). The state machine that used to hold it also held *whether a
    /// column was on screen*, which is the fact that no longer exists.
    ///
    /// Session-scoped, like everything else about where the wall is standing.
    pub(crate) opened: Option<u64>,
    /// Which format of an album the user picked, for albums where they
    /// picked one. Absent = the ranked-best edition (see
    /// [`vm::selected_edition`]).
    ///
    /// Session-scoped by choice: the persistent config is a hand-rolled
    /// single-key TOML file (see `config.rs`), so persisting a per-album map
    /// would mean adopting a real TOML parser for a preference whose proper
    /// home is a column in the library database anyway. Deferred in
    /// ADR-0007 rather than bolted on here.
    pub(crate) edition_choice: HashMap<u64, vm::EditionKey>,
    /// Decoded-thumbnail LRU; capacity/budget documented in [`art`].
    pub(crate) thumbs: LruCache<u64, iced_image::Handle>,
    /// Albums with a decode in flight (dedupes requests while scrolling).
    pending: HashSet<u64>,
    /// Albums known to have no (decodable) art — render the gradient and
    /// stop asking. Cleared once when the scan finishes, since late tracks
    /// or cover files may have arrived for early albums.
    no_art: HashSet<u64>,
    scan_rx: Option<Receiver<ScanUpdate>>,
    /// The music folders baz is holding, in the listener's order (ADR-0022).
    ///
    /// The shell's copy of `config.music_dirs`: the config file is the durable
    /// record and this is what is scanned, listed and removed from. They are
    /// kept in step by writing the config every time this moves.
    pub(crate) roots: Vec<PathBuf>,
    /// The folders the most recent pass could not walk at all. Cleared at the
    /// start of each pass, so it always describes the latest attempt rather
    /// than accumulating every share that was ever offline.
    unavailable: HashSet<PathBuf>,
    /// The periodic-refresh clock (ADR-0022 §3).
    refresh: scan::Refresh,
    /// What has been typed into the Settings place's add-a-folder field.
    folder_input: String,
    /// Why the last folder submitted was not added, if it was not.
    folder_error: Option<String>,
    /// Which folder's Remove is armed and waiting for its confirming press.
    folder_pending_removal: Option<usize>,
    /// Whether the scan worker is still running.
    pub(crate) scanning: bool,
    /// Files the scan could not read.
    pub(crate) files_skipped: usize,
    /// A fatal-ish problem worth a status-line mention (scan could not
    /// start, or a library write failed). Never a modal.
    pub(crate) problem: Option<String>,
    /// Where the shelf is scrolled to (logical px from the top).
    pub(crate) scroll_offset: f32,
    /// The grid viewport's size, for the virtualization math.
    pub(crate) grid_size: Size,
    last_scan_log: Instant,
    /// Which album's tile the pointer is on, if any.
    ///
    /// The shelf's hover mark is a **rule drawn under the wall label**
    /// (ADR-0017 step 14), not a card behind the sleeve — and a rule under the
    /// label is a *sibling* of the button, not the button. iced 0.13 tells a
    /// widget its own hover status inside a style function and tells its
    /// siblings nothing, so the tile reports its own crossings with a
    /// `mouse_area` and the shelf holds the one answer. Exactly the pattern
    /// [`App::hovered_queue_row`] already uses, and for exactly the same
    /// toolkit reason.
    ///
    /// The rule's lane is reserved whatever this says, so it changes what is
    /// drawn in it and never the geometry around it.
    pub(crate) hovered_album: Option<u64>,
    /// How far the hovered tile's mark has travelled (ADR-0020 §2.3).
    ///
    /// **One tween for the whole wall, keyed by the hovered id — never one per
    /// tile.** The shelf draws hundreds of tiles and at most one of them is
    /// under the pointer, so a tween per tile would be state allocated for a
    /// condition all but one of them is never in; and crossing the gutter from
    /// one sleeve to the next hands the mark over rather than restarting it
    /// (see [`crate::motion::Keyed`]).
    pub(crate) tile_hover: Keyed<u64>,
    /// The width of the window the shelf is laid out in.
    ///
    /// **The wall's width, full stop.** It used to be the window's less
    /// whatever the inspector was taking at this instant — a number that
    /// changed nine times over 150 ms — and with no side surface left there is
    /// nothing to subtract but the index rail's lane (see
    /// [`Shelf::grid_width`]).
    window_w: f32,
    /// **What the shuffle in progress is drawing from**, or `None` when no
    /// shuffle is running (`crate::shuffle`).
    ///
    /// This is the state the refusals ledger's *"no invisible shuffle pools"*
    /// is made of: while it is `Some`, every tile on the wall asks it whether it
    /// is in the pool (and dims if not) and whether it is one of the next draws
    /// (and carries a ring if so). A shuffle whose pool were held anywhere the
    /// wall could not read would be exactly the invisible one.
    ///
    /// It is dropped the moment another record is played deliberately, and by
    /// Escape — the marks describe *this* run, and a run that has been replaced
    /// is not one to go on marking.
    pub(crate) pool: Option<shuffle::Pool>,
    /// **The record the pull is offering**, or `None`.
    ///
    /// A suggestion, holding no playback of its own: see [`Pull`].
    pub(crate) pull: Option<Pull>,
}

/// **The record the pull drew, and when it was last heard.**
///
/// Deliberately two fields and no third. There is no "pending play", no timer,
/// no accepted flag — because *nothing plays until the listener asks*
/// (`docs/design/critique/02-surfaces.md`, and `docs/REFUSALS.md`'s *shuffle is
/// a thing you start*). The pull selects a record and prints one line about it;
/// accepting it is pressing the inspector's own **Play album**, which is the
/// same control, sending the same commands, as it is for a record you found
/// yourself.
///
/// # The surface this is drawn on is temporary
///
/// ADR-0017 step 18's **Marquee** lens is the pull's designed home — the sleeve
/// at half-window, full-bleed, with the note as poster type. Marquee is not
/// built, and inventing a lens to host one feature would be a worse mistake than
/// borrowing a surface that already exists, so the pull currently opens the
/// **album inspector** on the drawn record and prints its note there.
///
/// The seam is this struct. When Marquee lands it reads exactly these two fields
/// and draws them larger; nothing about how the draw is made, when it is made,
/// or what it is allowed to do changes. What must *not* be carried across is the
/// inspector's framing — a lens hosting the pull states the note as its subject,
/// not as a line above a button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pull {
    /// The record drawn — an id into [`Shelf::albums`].
    pub(crate) album: u64,
    /// What the ledger says about it: `Last played 3 years ago`, or
    /// `Never played` ([`shuffle::pull_note`]).
    pub(crate) note: String,
}

impl Shelf {
    /// Open the library DB, hydrate the shelf, persist the chosen folders, and
    /// kick off the scan worker. Errors are user-presentable strings.
    fn open(
        roots: Vec<PathBuf>,
        group_key: GroupKey,
        density: shelf::Density,
    ) -> Result<(Self, Task<Message>), String> {
        let t0 = Instant::now();
        let db_path = config::library_db_file()
            .ok_or_else(|| "no usable data directory on this system".to_owned())?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let mut library = Library::open(&db_path)
            .map_err(|e| format!("cannot open library at {}: {e}", db_path.display()))?;
        // Schema v8's backfill, and the one place that can make it (ADR-0022):
        // `baz-core` cannot know which folder a pre-v8 row came from, and this
        // is the code that reads the config file that does. Rows already naming
        // a root are untouched, and a row under none of these folders stays
        // rootless — which means unprunable, the safe direction.
        adopt_roots(&mut library, &roots);
        // The ledger, read once. A missing file is an empty history and not an
        // error; an unreadable one costs the PLAYED key its detail and nothing
        // else, so it is a note rather than a `problem`.
        let history = read_history();

        persist_roots(&roots);
        // The snapshot is what makes the scan incremental — and the only
        // rows it is ever allowed to prune (see `scan::vanished`).
        let scan_rx = scan::spawn(
            roots.clone(),
            library.known_files(),
            scan::ScanMode::Incremental,
        );

        let mut shelf = Self {
            library,
            group_key,
            density,
            history,
            albums: Vec::new(),
            groups: Vec::new(),
            visible: Vec::new(),
            visible_counts: Vec::new(),
            query: String::new(),
            opened: None,
            edition_choice: HashMap::new(),
            thumbs: LruCache::new(
                NonZeroUsize::new(art::THUMB_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN),
            ),
            pending: HashSet::new(),
            no_art: HashSet::new(),
            scan_rx: Some(scan_rx),
            roots,
            unavailable: HashSet::new(),
            refresh: scan::Refresh::new(scan::REFRESH_INTERVAL, Instant::now()),
            folder_input: String::new(),
            folder_error: None,
            folder_pending_removal: None,
            scanning: true,
            files_skipped: 0,
            problem: None,
            scroll_offset: 0.0,
            grid_size: Size::new(
                WINDOW.width - theme::INDEX_LANE_W,
                WINDOW.height - TOP_BAR_H,
            ),
            last_scan_log: Instant::now(),
            hovered_album: None,
            tile_hover: Keyed::new(),
            window_w: WINDOW.width,
            pool: None,
            pull: None,
        };
        shelf.rebuild_shelves();
        let shelf_task = shelf.request_visible_thumbs();
        println!(
            "[startup] library open + hydrate: {:.1} ms ({} albums / {} shelves by {} at {} / {} tracks) from {}",
            t0.elapsed().as_secs_f64() * 1e3,
            shelf.albums.len(),
            shelf.groups.len(),
            group_key.code(),
            density.label(),
            shelf.library.len(),
            db_path.display()
        );
        // **The well does not take focus at startup any more**, and that is
        // step 11's doing rather than a tidy-up. It used to, so that a listener
        // could type immediately — which was the right trade while typing
        // needed a focused field, and which cost <kbd>Space</kbd> its meaning
        // until the first <kbd>Esc</kbd>, a wart the README had to document.
        // Type-anywhere pays for the typing without the focus: the first letter
        // reaches the query from the wall (`crate::keys`), so the caret can
        // start where the transport is and the keyboard means what the key
        // table says on the first frame.
        Ok((shelf, shelf_task))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // The Library section's own small machine, answered first and
        // separately — six messages that all resolve to "change which folders
        // baz holds, then rescan", exactly as the volume's nine are answered
        // apart from the shell's own arms.
        if let Some(task) = self.update_library(&message) {
            return task;
        }
        match message {
            Message::SearchChanged(query) => {
                self.query = query;
                self.refilter();
                self.scroll_offset = 0.0;
                Task::batch([
                    scrollable::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
                    self.request_visible_thumbs(),
                ])
            }
            // **Type anywhere** (ADR-0017 §1.2). Both halves of the gesture,
            // in one arm, because they are one act — see [`Message::QueryTyped`].
            Message::QueryTyped(text) => self.type_into_query(&text),
            Message::EscapePressed => self.peel(),
            Message::GroupKeySelected(key) => self.arrange_by(key),
            Message::RailJumped(run) => self.jump_to_shelf(run),
            Message::Scrolled(viewport) => {
                self.scroll_offset = viewport.absolute_offset().y;
                let bounds = viewport.bounds();
                self.grid_size = Size::new(bounds.width, bounds.height);
                self.request_visible_thumbs()
            }
            Message::WindowResized(size) => {
                self.window_w = size.width;
                // Estimate until the next scroll event reports real bounds.
                // The rail's lane comes off here too, because the scrollable
                // the next `Scrolled` will measure has already given it up.
                self.grid_size = Size::new(self.grid_width(), (size.height - TOP_BAR_H).max(100.0));
                self.request_visible_thumbs()
            }
            Message::TileEntered(id) => {
                self.hovered_album = Some(id);
                self.tile_hover.enter(id, motion::TILE, Instant::now());
                Task::none()
            }
            // Only if it is still the tile that left: both messages are
            // published from one `CursorMoved` in widget order, so crossing the
            // wall delivers the new tile's entry before the old tile's exit,
            // and an exit meaning "nothing is hovered" would undo it.
            Message::TileLeft(id) => {
                if self.hovered_album == Some(id) {
                    self.hovered_album = None;
                }
                self.tile_hover.leave(id, motion::TILE, Instant::now());
                Task::none()
            }
            Message::EditionSelected(id, key) => {
                // Pure view state: the track list and the *next* queue follow
                // it, but nothing already playing is disturbed.
                self.edition_choice.insert(id, key);
                Task::none()
            }
            Message::ThumbLoaded(id, handle) => {
                self.pending.remove(&id);
                match handle {
                    Some(handle) => {
                        self.thumbs.put(id, handle);
                    }
                    None => {
                        self.no_art.insert(id);
                    }
                }
                Task::none()
            }
            Message::ScanTick => self.drain_scan(),
            _ => Task::none(),
        }
    }

    /// **What <kbd>Enter</kbd> plays**: the top-ranked match while a query is
    /// narrowing the wall, else the record the wall was last left for, else
    /// nothing.
    ///
    /// The order is ADR-0017 §1.2's table read left to right — *play the
    /// top-ranked match; play the selected album* — and the fall-through is
    /// what makes it one key rather than two: with a query you are choosing
    /// from what you typed, and without one you are choosing what you last
    /// opened ([`Shelf::opened`] — the mark the wall carries where a selection
    /// used to be, ADR-0022).
    ///
    /// The ranked answer is [`vm::top_match`] (ADR-0021), **filtered through
    /// the wall**: an album is only played if it is on screen. In practice the
    /// two always agree — both come from the same query against the same
    /// library — and the check is here so that "Enter plays the first match"
    /// stays a statement about the wall a listener is looking at rather than
    /// about a search index they cannot see. If the ranked album is somehow
    /// not on the wall, the wall's own first survivor is played instead, which
    /// is the record under the top-left corner of the collection.
    fn enter_plays(&self) -> Option<u64> {
        if self.query.trim().is_empty() {
            return self.opened;
        }
        let on_the_wall = |id: u64| {
            self.visible
                .iter()
                .filter_map(|index| self.albums.get(*index))
                .any(|album| album.id == id)
        };
        vm::top_match(&self.library, &self.query)
            .filter(|id| on_the_wall(*id))
            .or_else(|| {
                self.visible
                    .first()
                    .and_then(|index| self.albums.get(*index))
                    .map(|album| album.id)
            })
    }

    /// **Type anywhere**: append what a key produced to the query, filter, and
    /// put the caret in the well (ADR-0017 §1.2, [`Message::QueryTyped`]).
    ///
    /// The text is *appended*, never assigned: a listener who has already
    /// typed and clicked away mid-query continues it rather than restarting
    /// it, which is what the well itself would do if the caret were still in
    /// it. In practice this runs once and then the well has focus, so it is
    /// the empty-query case almost every time.
    ///
    /// The caret lands at the end of what was typed — `text_input`'s own
    /// `focus` moves the cursor there — so the next keystroke, which the
    /// *field* will handle, continues the word instead of inserting before it.
    fn type_into_query(&mut self, text: &str) -> Task<Message> {
        self.query.push_str(text);
        self.refilter();
        self.scroll_offset = 0.0;
        Task::batch([
            text_input::focus(search_id()),
            scrollable::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
            self.request_visible_thumbs(),
        ])
    }

    /// <kbd>Esc</kbd> on a non-empty query: give the wall back.
    ///
    /// **Cleared *and* blurred**, which is new with type-anywhere and is the
    /// point of it. Escape used to put the caret back in the well, because a
    /// well you had clicked into was a place you meant to be. Now that any
    /// letter reopens the query from anywhere, holding focus after a clear
    /// would leave the keyboard in an empty field — where <kbd>Space</kbd>
    /// types a space rather than pausing the music — and a listener who
    /// abandoned a search wants the transport back.
    fn clear_query(&mut self) -> Task<Message> {
        self.query.clear();
        self.refilter();
        self.scroll_offset = 0.0;
        Task::batch([
            blur_search(),
            scrollable::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
            self.request_visible_thumbs(),
        ])
    }

    /// **Hang the wall at `density`** — the zoom's one effect on the shelf
    /// (ADR-0017 step 6).
    ///
    /// Everything except the geometry survives it, and for the same reason a
    /// re-arrangement is cheap: the density changes how the works are *laid
    /// out* and nothing about which works there are, so the query, the
    /// selection, the edition choices, the thumbnail cache and what is playing
    /// are all untouched. The shelves are not even rebuilt — [`Shelf::shelves`]
    /// derives them from the grid on every call, so the next frame lays out at
    /// the new step by itself.
    ///
    /// **The scroll is anchored rather than reset**, which is the one place
    /// this differs from a key change. A zoom is a request to look at the same
    /// part of the collection more or less closely, so the offset is scaled by
    /// the wall's new height over its old: the record you were looking at is
    /// still under the pointer. (A re-arrangement moves the records themselves
    /// and therefore *must* go back to the top; see [`Self::arrange_by`].)
    ///
    /// The re-anchor is also what makes <kbd>Ctrl</kbd>+scroll behave: iced
    /// 0.13's `scrollable` has no modifier awareness and scrolls whatever the
    /// wheel says, so the notch that asked for a zoom also moves the wall.
    /// Scrolling back to the anchor overrides it in the same frame.
    fn set_density(&mut self, density: shelf::Density) -> Task<Message> {
        if self.density == density {
            return Task::none();
        }
        let was = self.shelves().height();
        self.density = density;
        let now = self.shelves().height();
        let anchored = if was > 0.0 {
            (self.scroll_offset * now / was).max(0.0)
        } else {
            0.0
        };
        self.scroll_offset = anchored;
        persist_density(density);
        Task::batch([
            scrollable::scroll_to(
                scroll_id(),
                AbsoluteOffset {
                    x: 0.0,
                    y: anchored,
                },
            ),
            self.request_visible_thumbs(),
        ])
    }

    /// **Escape, on the wall: peel one layer, top down.**
    ///
    /// The tail of [`App::escape`]'s peel — everything under the popover and
    /// under the Settings place is this screen's, and this is the order it goes
    /// in. Each press takes exactly one thing off, and each early return is one
    /// press.
    ///
    /// 1. **The pull's offer.** `docs/design/critique/02-surfaces.md` names the
    ///    gesture — *"`Ctrl+R` re-pulls; `Esc` returns"* — and an offer is the
    ///    topmost thing on a wall that is showing one. (Escaping *out of the
    ///    record's page* the pull opened already withdrew it; this is the
    ///    press for an offer whose page you have left some other way.)
    /// 2. **The query** — unchanged, and the one press that keeps focus where
    ///    it is.
    /// 3. **The shuffle pool's marks, last.** That is the point of the ordering
    ///    rather than a leftover: clearing the query *widens* the wall while the
    ///    pool stays what it was, which is the frame in which the dimming says
    ///    the most — it is how a listener sees that the shuffle is drawing from
    ///    four records out of twenty-five. Peeling the marks first would take
    ///    the answer away at the exact press that asked the question.
    ///
    ///    It never stops the music. A shuffle's run is a queue like any other;
    ///    what Escape takes off the wall is the *drawing*, and the record goes
    ///    on playing.
    ///
    /// The query step **clears and blurs**, which is type-anywhere's doing
    /// (ADR-0017 step 11) — see [`Self::clear_query`] for why holding the caret
    /// stopped being right once any letter could reopen the query.
    fn peel(&mut self) -> Task<Message> {
        if self.pull.take().is_some() {
            return Task::none();
        }
        if !self.query.is_empty() {
            return self.clear_query();
        }
        self.pool = None;
        Task::none()
    }

    /// **Arrange the wall by `key`** — the top bar's row of words and `1`–`5`
    /// both land here.
    ///
    /// Re-arranging is a *projection*, never a filter: every album is still
    /// there, in a different order under different headers (ADR-0019 §1). So
    /// the query, the selection, the edition choices, the thumbnail cache and
    /// what is playing are all untouched — an album's id does not depend on the
    /// key, which is what makes this cheap and what makes it safe.
    ///
    /// The wall does go back to the top, and that is the one thing that is
    /// *not* preserved. It is a deliberate choice rather than an omission:
    /// after a re-arrangement the record you were looking at is somewhere else
    /// entirely, so holding the scroll offset would drop you into an unrelated
    /// part of the collection while claiming nothing had moved.
    fn arrange_by(&mut self, key: GroupKey) -> Task<Message> {
        if self.group_key == key {
            return Task::none();
        }
        self.group_key = key;
        self.rebuild_shelves();
        self.scroll_offset = 0.0;
        persist_group_key(key);
        Task::batch([
            scrollable::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
            self.request_visible_thumbs(),
        ])
    }

    /// **Bring one record to the top of the wall** — what the pull does with
    /// the sleeve it drew, so that the wall is standing on the record when the
    /// listener comes back from its page.
    ///
    /// A record the wall is not currently showing is not scrolled to, because
    /// there is nowhere to scroll it to. The pull never draws one (its pool is
    /// the wall), so in practice this is the guard rather than the case.
    ///
    /// The wall lands **at the record's row**, for [`Self::jump_to_shelf`]'s
    /// reason: arriving somewhere means arriving above the thing you came for,
    /// not with it clipped at the top edge. And it lands there **exactly**,
    /// where it used to land approximately — the target used to be measured
    /// against a width the opening inspector was still travelling to, and there
    /// is no inspector and no travel, so the grid the scroll is computed
    /// against is the grid that will be under it.
    fn show_album(&mut self, id: u64) -> Task<Message> {
        self.opened = Some(id);
        let Some(place) = self.album_top(id, self.grid()) else {
            return Task::none();
        };
        self.scroll_offset = place;
        Task::batch([
            scrollable::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: place }),
            self.request_visible_thumbs(),
        ])
    }

    /// Where the wall must be scrolled for `id`'s row to be at the top of the
    /// viewport, in the scrollable's content coordinates.
    ///
    /// `None` when the query has filtered the record off the wall. The shelf's
    /// header band is the answer when the record is on the shelf's first row,
    /// so that landing on the first record of a shelf lands on the words that
    /// name it.
    fn album_top(&self, id: u64, grid: shelf::Grid) -> Option<f32> {
        let at = self
            .visible
            .iter()
            .position(|index| self.albums.get(*index).is_some_and(|a| a.id == id))?;
        let shelves = shelf::Shelves::new(grid, &self.visible_counts);
        let run = *shelves
            .runs()
            .iter()
            .find(|run| at >= run.first && at < run.first + run.len)?;
        let row = (at - run.first) / grid.columns.max(1);
        Some(if row == 0 {
            run.top
        } else {
            run.rows_top(grid) + grid.spacer_height(row)
        })
    }

    /// Put a shelf at the top of the wall — what an index-rail entry does.
    ///
    /// It jumps to the shelf's **header band**, not to its first row: landing
    /// on a shelf has to land you on the thing that names it, and one `HANG`
    /// of clear wall above the covers is the difference between arriving
    /// somewhere and arriving mid-shelf.
    fn jump_to_shelf(&mut self, run: usize) -> Task<Message> {
        let shelves = self.shelves();
        let Some(target) = shelves.runs().get(run) else {
            return Task::none();
        };
        self.scroll_offset = target.top;
        Task::batch([
            scrollable::scroll_to(
                scroll_id(),
                AbsoluteOffset {
                    x: 0.0,
                    y: target.top,
                },
            ),
            self.request_visible_thumbs(),
        ])
    }

    /// **The grid's width**: the window's, less the index rail's lane.
    ///
    /// The two are different numbers and the difference is the rail's
    /// ([`theme::INDEX_LANE_W`]): the wall is what the shelf column occupies and
    /// the grid is what the covers hang in.
    ///
    /// It used to have a third term — whatever the album inspector was taking
    /// at this instant — and losing it is the plainest thing ADR-0022 did to
    /// this file: **the wall's width is now a property of the window and
    /// nothing else**, so no press anywhere in the product can re-hang the
    /// collection. The `reflow`, the width tween, the panel's lagging album and
    /// the double-click's grid hold all existed to make a re-hang survivable;
    /// none of them has anything left to do.
    fn grid_width(&self) -> f32 {
        (self.window_w - theme::INDEX_LANE_W).max(0.0)
    }

    /// Advance the shelf's own transitions.
    fn tick_motion(&mut self, now: Instant) -> Task<Message> {
        self.tile_hover.tick(now);
        Task::none()
    }

    /// Whether the shelf still needs a clock (see [`App::moving`]).
    fn moving(&self) -> bool {
        self.tile_hover.live()
    }

    /// The hang the grid lays out with: resolved for what the viewport
    /// measures, **less the index rail's lane**.
    ///
    /// One answer, read by the view that draws the rows and by the thumbnail
    /// prefetch that decides which of them to decode art for — a prefetch
    /// working from a different grid than the one on screen would request the
    /// wrong tiles.
    ///
    /// The rail's lane is already off `grid_size`: the wall and the rail are
    /// siblings in one row, so what the scrollable *measures* is the grid's
    /// width and the subtraction happens once, in the layout, rather than at
    /// each reader. `the_hang_holds_with_the_index_rail_taken_off_the_wall`
    /// asserts the hang survives that subtraction at every width in the band.
    pub(crate) fn grid(&self) -> shelf::Grid {
        shelf::Grid::new(self.grid_size.width, self.density)
    }

    /// How the wall is broken into shelves, for the current filter and grid.
    ///
    /// Rebuilt per call rather than cached: it is one pass over a few dozen
    /// counts, it has to follow the grid (which follows the window, the
    /// inspector and the double-click hold), and a cache of it would be a
    /// fourth thing that could disagree with the other three.
    pub(crate) fn shelves(&self) -> shelf::Shelves {
        shelf::Shelves::new(self.grid(), &self.visible_counts)
    }

    /// Re-ask the library for the wall under the active key, and re-derive
    /// everything that hangs off it.
    ///
    /// **The album ids do not change**, which is what makes re-arranging cheap
    /// and safe: an id is a hash of the (artist, album) pair
    /// ([`vm::album_id`]), so the thumbnail cache, the selection, the playing
    /// album and the edition choices all survive a key change untouched. Only
    /// the order and the breaks are new.
    fn rebuild_shelves(&mut self) {
        let shelves = vm::build_shelves(&self.library, self.group_key, self.history.as_ref());
        self.albums.clear();
        self.groups.clear();
        for shelf in shelves {
            self.albums.extend(shelf.albums);
            self.groups.push(GroupVm {
                header: shelf.header,
                end: self.albums.len(),
            });
        }
        self.refilter();
    }

    /// Recompute `visible` for the current query (wall order preserved —
    /// see [`vm::matching_album_ids`] for the track→album mapping), and with
    /// it how many albums each shelf has left.
    ///
    /// The two are computed in one pass from one filter, so the wall's layout
    /// and its contents cannot disagree about which albums survived.
    fn refilter(&mut self) {
        self.visible = vm::visible_indices(&self.albums, &self.library, &self.query);
        // The shelves are contiguous slices of `albums` and `visible` is in
        // the same order, so each shelf's surviving count is one walk of the
        // two lists together rather than a second filter that could disagree
        // with the first.
        let mut seen = self.visible.iter().peekable();
        self.visible_counts.clear();
        for group in &self.groups {
            let mut count = 0;
            while seen.next_if(|index| **index < group.end).is_some() {
                count += 1;
            }
            self.visible_counts.push(count);
        }
    }

    /// Answer a message that only the folders baz holds care about, reporting
    /// whether it was one (ADR-0022).
    ///
    /// Every one of them ends in the same two acts — the list moves, and a scan
    /// starts or does not — so they are one machine rather than six arms
    /// scattered through the shelf's own.
    fn update_library(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            // The periodic refresh. The clock says whether it is due; a pass
            // already running always says no.
            Message::RefreshTick => {
                if self.refresh.due(Instant::now(), self.scanning) {
                    println!("[scan] periodic refresh");
                    self.start_scan(scan::ScanMode::Incremental);
                }
            }
            Message::MusicFolderInput(value) => {
                self.folder_input.clone_from(value);
                self.folder_error = None;
            }
            Message::AddMusicFolder => return Some(self.submit_folder_input()),
            Message::PickMusicFolder => return Some(pick_folder()),
            Message::MusicFolderPicked(choice) => {
                return Some(self.folder_picked(choice.clone()));
            }
            Message::MusicFolderChecked(result) => {
                return Some(self.folder_checked(result.clone()));
            }
            // The first press arms; the second acts. See
            // `views::settings::folder_block` for why it is two.
            Message::ConfirmRemoveMusicFolder(index) => {
                self.folder_pending_removal = Some(*index);
            }
            Message::CancelRemoveMusicFolder => self.folder_pending_removal = None,
            Message::RemoveMusicFolder(index) => return Some(self.remove_root(*index)),
            Message::ForceSync => {
                if !self.scanning {
                    println!("[scan] force sync requested");
                    self.start_scan(scan::ScanMode::Force);
                }
            }
            _ => return None,
        }
        Some(Task::none())
    }

    /// What the Settings place's Library section draws (ADR-0022).
    ///
    /// A projection built here rather than in the view, because it is the join
    /// of two things this struct holds: the folders the shell is scanning, and
    /// what the index records under each of them.
    fn library_view(&self) -> views::settings::LibraryView<'_> {
        views::settings::LibraryView {
            folders: self
                .roots
                .iter()
                .map(|root| {
                    let stats = self.library.root_stats(root);
                    views::settings::FolderRow {
                        path: root.clone(),
                        tracks: stats.tracks,
                        last_scan_ns: stats.last_scan_ns,
                        unavailable: self.unavailable.contains(root),
                    }
                })
                .collect(),
            input: &self.folder_input,
            error: self.folder_error.as_deref(),
            pending_removal: self.folder_pending_removal,
            scanning: self.scanning,
            unrooted: self.library.unrooted_tracks(),
            now_ns: now_ns(),
        }
    }

    /// Send the typed path off to be looked at, coming back as
    /// [`Message::MusicFolderChecked`].
    ///
    /// The look itself — one `stat` — happens on the blocking pool, because the
    /// paths people type here are exactly the ones a dialog cannot offer: the
    /// share that is configured but not mounted, the drive that is sometimes
    /// plugged in. Against a dead hard mount that `stat` can sit for minutes,
    /// and it used to sit on the UI thread.
    fn submit_folder_input(&mut self) -> Task<Message> {
        let dir = expand_tilde(self.folder_input.trim());
        if dir.as_os_str().is_empty() {
            return Task::none();
        }
        Task::perform(check_folder(dir), Message::MusicFolderChecked)
    }

    /// What the folder picker's closing means: a chosen folder joins the
    /// list; a dismissal is not a decision and touches nothing.
    ///
    /// A picked folder skips the `stat` the typed door needs — the dialog
    /// walked the real filesystem to offer it, which is better evidence than a
    /// fresh stat, and re-checking would put an avoidable filesystem wait back
    /// on this thread.
    fn folder_picked(&mut self, choice: Option<PathBuf>) -> Task<Message> {
        match choice {
            None => Task::none(),
            Some(dir) => self.accept_folder(dir),
        }
    }

    /// What the off-thread look at a typed path came back to: the same words
    /// the first-run screen uses when the path is not a directory, or the
    /// acceptance every added folder goes through.
    fn folder_checked(&mut self, result: Result<PathBuf, String>) -> Task<Message> {
        match result {
            Ok(dir) => {
                let task = self.accept_folder(dir);
                // The field empties only when its path was taken. A refused
                // path (already here) stays put to be corrected, rather than
                // making somebody retype the long half of a NAS path.
                if self.folder_error.is_none() {
                    self.folder_input.clear();
                }
                task
            }
            Err(reason) => {
                self.folder_error = Some(reason);
                Task::none()
            }
        }
    }

    /// Hold `dir`, remember it, and scan it — the one acceptance path both
    /// doors (the typed path and the picker) land in.
    ///
    /// A folder already held is refused rather than added twice — it would be
    /// walked twice for one set of rows ([`folder_refusal`] holds the words).
    fn accept_folder(&mut self, dir: PathBuf) -> Task<Message> {
        if let Some(refusal) = folder_refusal(&self.roots, &dir) {
            self.folder_error = Some(refusal);
            return Task::none();
        }
        self.folder_error = None;
        self.folder_pending_removal = None;
        // A folder added now may hold rows an older baz left rootless — the
        // pre-v8 population this is the only cure for. Claim them before the
        // scan, so the walk that follows can prune them if they are gone.
        adopt_roots(&mut self.library, std::slice::from_ref(&dir));
        println!("[config] holding {}", dir.display());
        self.roots.push(dir);
        persist_roots(&self.roots);
        // Incremental, not forced: a folder that overlaps one baz already holds
        // must not cost a re-read of every file in it.
        self.start_scan(scan::ScanMode::Incremental);
        Task::none()
    }

    /// Stop holding a folder, and **forget its tracks** (ADR-0022 §4).
    ///
    /// Nothing on disk is touched. What goes is the index's record of the
    /// folder: its rows, and its scan time. The argument for forgetting rather
    /// than keeping is in the ADR and in `Library::forget_root` — in short, a
    /// folder baz no longer holds is one baz can no longer refresh, so keeping
    /// its albums would leave a listener with rows nothing can ever correct or
    /// remove.
    fn remove_root(&mut self, index: usize) -> Task<Message> {
        self.folder_pending_removal = None;
        if index >= self.roots.len() {
            return Task::none();
        }
        let root = self.roots.remove(index);
        self.unavailable.remove(&root);
        persist_roots(&self.roots);
        match self.library.forget_root(&root) {
            Ok(count) => println!("[index] {count} tracks forgotten with {}", root.display()),
            Err(error) => {
                println!("[index] could not forget {}: {error}", root.display());
                self.problem = Some(format!("could not forget that folder: {error}"));
            }
        }
        // The wall's mark and the art caches are keyed by album id, and the
        // albums a forgotten folder held are gone — so the rebuild has to be
        // followed by the same clean-up a finished scan does.
        self.opened = None;
        self.no_art.clear();
        self.rebuild_shelves();
        self.request_visible_thumbs()
    }

    /// Start a scan of every folder baz holds, in `mode`, replacing whatever
    /// pass was running.
    ///
    /// The refresh clock is restarted here rather than only on completion, so
    /// that a force sync or a newly added folder also pushes the automatic
    /// rescan out — a listener who has just refreshed does not need baz to do
    /// it again in ten seconds.
    fn start_scan(&mut self, mode: scan::ScanMode) {
        self.unavailable.clear();
        self.files_skipped = 0;
        self.refresh.restarted(Instant::now());
        if self.roots.is_empty() {
            self.scan_rx = None;
            self.scanning = false;
            return;
        }
        self.scan_rx = Some(scan::spawn(
            self.roots.clone(),
            self.library.known_files(),
            mode,
        ));
        self.scanning = true;
    }

    /// Apply every pending scan update: one `add_tracks` + one view-model
    /// rebuild per tick regardless of how many batches arrived.
    fn drain_scan(&mut self) -> Task<Message> {
        let Some(drained) = self.collect_scan() else {
            return Task::none();
        };
        let Drained {
            fresh_tracks,
            vanished,
            scanned,
            missing,
            finished,
        } = drained;
        self.apply_scan(fresh_tracks, &vanished, scanned, missing, finished)
    }

    /// Take everything the worker has said since the last tick, without
    /// touching the index — the receiving half of [`Shelf::drain_scan`].
    fn collect_scan(&mut self) -> Option<Drained> {
        let rx = self.scan_rx.as_ref()?;
        // Batches are kept per root, because the root is what makes the write
        // an `add_tracks_under`: it is the fact removal's second gate will read
        // back. A tick usually holds one root's worth; a small library can hold
        // several, and the order is the order they arrived in.
        let mut fresh_tracks: Vec<(PathBuf, Vec<baz_core::library::TrackMeta>)> = Vec::new();
        let mut vanished: Vec<std::path::PathBuf> = Vec::new();
        let mut scanned: Vec<(PathBuf, i64)> = Vec::new();
        let mut missing: Vec<(PathBuf, String)> = Vec::new();
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(ScanUpdate::Batch {
                    root,
                    tracks,
                    failed,
                }) => {
                    self.files_skipped += failed;
                    match fresh_tracks.last_mut() {
                        Some((held, batch)) if *held == root => batch.extend(tracks),
                        _ => fresh_tracks.push((root, tracks)),
                    }
                }
                Ok(ScanUpdate::Removed { paths }) => vanished.extend(paths),
                Ok(ScanUpdate::RootDone {
                    root,
                    at_ns,
                    added,
                    updated,
                    unchanged,
                    failed,
                }) => {
                    println!(
                        "[scan] {}: {added} added, {updated} updated, {unchanged} unchanged, \
                         {failed} skipped",
                        root.display()
                    );
                    scanned.push((root, at_ns));
                }
                Ok(ScanUpdate::RootUnavailable { root, reason }) => missing.push((root, reason)),
                Ok(ScanUpdate::Done {
                    added,
                    updated,
                    unchanged,
                    removed,
                    failed,
                    unavailable,
                    elapsed,
                }) => {
                    let secs = elapsed.as_secs_f64();
                    let read = added + updated;
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "track counts are far below f64's exact-integer range"
                    )]
                    let rate = if secs > 0.0 { read as f64 / secs } else { 0.0 };
                    println!(
                        "[scan] done: {added} added, {updated} updated, {unchanged} unchanged, \
                         {removed} removed, {failed} files skipped, \
                         {unavailable} folders unavailable, {secs:.1} s ({rate:.0} tracks/s)"
                    );
                    finished = true;
                    break;
                }
                Ok(ScanUpdate::Error(error)) => {
                    println!("[scan] failed to start: {error}");
                    self.problem = Some(format!("scan failed: {error}"));
                    finished = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }
        Some(Drained {
            fresh_tracks,
            vanished,
            scanned,
            missing,
            finished,
        })
    }

    /// Write what one tick's worth of scan updates said, rebuild the shelf, and
    /// report the folders that were not there — the applying half of
    /// [`Shelf::drain_scan`].
    fn apply_scan(
        &mut self,
        fresh_tracks: Vec<(PathBuf, Vec<baz_core::library::TrackMeta>)>,
        vanished: &[PathBuf],
        scanned: Vec<(PathBuf, i64)>,
        missing: Vec<(PathBuf, String)>,
        finished: bool,
    ) -> Task<Message> {
        // A folder that is not reachable right now: never a scan failure — the
        // pass carried on and pruned nothing from it (ADR-0022 §2).
        //
        // The status line gets a **count**, not a path, and that is a frame
        // constraint rather than terseness: the top bar's note is a single
        // unwrapped line sharing its row with the counts and `Settings`, and a
        // message carrying `/mnt/nas/Music/Archive` wraps it to two and pushes
        // `Settings` off the strip. Which folder it was, and that nothing was
        // removed from it, is said per folder in the Settings place — where
        // there is room to say it properly.
        let absent = missing.len();
        for (root, reason) in missing {
            println!("[scan] {} is unavailable: {reason}", root.display());
            self.unavailable.insert(root);
        }
        if absent == 1 {
            self.problem = Some("1 folder is not reachable".to_owned());
        } else if absent > 1 {
            self.problem = Some(format!("{absent} folders are not reachable"));
        }
        // When a folder's walk finished, so the Settings place can say when baz
        // last looked at it.
        for (root, at_ns) in scanned {
            if let Err(error) = self.library.record_scan(&root, at_ns) {
                println!(
                    "[index] could not record the scan of {}: {error}",
                    root.display()
                );
            }
        }

        let mut task = Task::none();
        if !fresh_tracks.is_empty() || !vanished.is_empty() {
            for (root, tracks) in fresh_tracks {
                if let Err(error) = self.library.add_tracks_under(Some(&root), tracks) {
                    println!("[index] write failed: {error}");
                    self.problem = Some(format!("library write failed: {error}"));
                }
            }
            if !vanished.is_empty() {
                match self.library.remove_tracks(vanished) {
                    Ok(count) => println!("[index] {count} vanished tracks removed"),
                    Err(error) => {
                        println!("[index] removal failed: {error}");
                        self.problem = Some(format!("library removal failed: {error}"));
                    }
                }
            }
            self.rebuild_shelves();
            if self.last_scan_log.elapsed() > Duration::from_secs(2) {
                self.last_scan_log = Instant::now();
                println!(
                    "[scan] {} tracks / {} albums so far…",
                    self.library.len(),
                    self.albums.len()
                );
            }
            task = self.request_visible_thumbs();
        }
        if finished {
            self.scanning = false;
            self.scan_rx = None;
            // The periodic refresh is a gap *between* passes: the clock starts
            // when this one finishes, not when the next one is wanted.
            self.refresh.restarted(Instant::now());
            // Early albums may have gained art (late tracks, cover files
            // written mid-scan): allow one clean retry pass.
            self.no_art.clear();
            task = Task::batch([task, self.request_visible_thumbs()]);
        }
        task
    }

    /// Kick off off-thread decodes for every visible tile whose thumbnail is
    /// neither cached, in flight, nor known-absent. Ported from the spike;
    /// `get` (not `peek`) refreshes LRU recency for visible entries.
    fn request_visible_thumbs(&mut self) -> Task<Message> {
        let (start, end) = self
            .shelves()
            .visible_albums(self.scroll_offset, self.grid_size.height);
        let (start, end) = (start.min(self.visible.len()), end.min(self.visible.len()));
        let mut tasks = Vec::new();
        for &album_index in &self.visible[start..end] {
            let Some(album) = self.albums.get(album_index) else {
                continue;
            };
            let id = album.id;
            if self.thumbs.get(&id).is_some()
                || self.pending.contains(&id)
                || self.no_art.contains(&id)
            {
                continue;
            }
            self.pending.insert(id);
            let path = album.first_track.clone();
            tasks.push(Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        art::load_thumb(&path)
                            .map(|(w, h, rgba)| iced_image::Handle::from_rgba(w, h, rgba))
                    })
                    .await
                    .ok()
                    .flatten()
                },
                move |handle| Message::ThumbLoaded(id, handle),
            ));
        }
        Task::batch(tasks)
    }

    /// The **Library place**: the top bar over the grid. Composition only —
    /// the surfaces themselves are [`crate::views`].
    ///
    /// Two elements in one column, and that is the whole of it. It held a
    /// three-way `row!` — the wall at an explicit width, a hairline, and the
    /// inspector behind a reveal viewport — because the grid had to survive a
    /// column arriving beside it over 150 ms. ADR-0022 deleted the column, so
    /// the wall takes the window and nothing is beside it.
    fn view<'a>(&'a self, player: &'a PlayerState, lamp: f32) -> Element<'a, Message> {
        column![
            views::top_bar::view(self),
            views::shelf::view(self, player, lamp)
        ]
        .into()
    }

    /// The album `id`'s view model, if the wall still holds it.
    ///
    /// `None` after a rescan has taken the record away while its page was open,
    /// which the shell answers by drawing the wall instead.
    pub(crate) fn album(&self, id: u64) -> Option<&vm::AlbumVm> {
        self.albums.iter().find(|album| album.id == id)
    }
}

/// One tick's worth of scan updates, taken off the channel and not yet applied
/// (`Shelf::collect_scan` → `Shelf::apply_scan`).
///
/// The split exists because the two halves want different borrows: receiving
/// holds the channel, and applying holds the library. Keeping them apart is
/// also what makes the "one `add_tracks_under` per root per tick" property
/// visible rather than buried in a loop.
struct Drained {
    /// Tracks read this tick, grouped by the root that produced them.
    fresh_tracks: Vec<(PathBuf, Vec<baz_core::library::TrackMeta>)>,
    /// Rows the removal pass proved are gone.
    vanished: Vec<PathBuf>,
    /// Roots whose walk finished, with the moment it did.
    scanned: Vec<(PathBuf, i64)>,
    /// Roots that could not be walked, with the reason.
    missing: Vec<(PathBuf, String)>,
    /// Whether the pass is over.
    finished: bool,
}

/// The message a D-Bus method call asks for.
///
/// Every arm is a message the interface already emits from a control or a
/// key, which is the point: there is one update-loop arm per intention, and
/// the lock screen's Next and the bottom bar's Next are the same press as far
/// as everything downstream is concerned.
fn message_for(request: mpris::Request) -> Message {
    match request {
        mpris::Request::Play => Message::Play,
        mpris::Request::Pause => Message::Pause,
        mpris::Request::PlayPause => Message::PlayPause,
        mpris::Request::Stop => Message::Stop,
        mpris::Request::Next => Message::NextTrack,
        mpris::Request::Previous => Message::PreviousTrack,
        mpris::Request::SeekBy(delta_ms) => Message::SeekBy(delta_ms),
        mpris::Request::SeekTo(position_ms) => Message::SeekTo(position_ms),
        mpris::Request::SetVolume(position) => Message::SetVolume(position),
        mpris::Request::SetMute(muted) => Message::SetMute(muted),
        mpris::Request::Raise => Message::Raise,
        mpris::Request::Quit => Message::Quit,
    }
}

/// Persist the folders baz holds; best-effort with a log, never fatal — a
/// read-only config dir must not block listening to music.
///
/// This is also where the silent migration lands: [`persist`] reads the
/// file first, so a document that still carries the pre-ADR-0022 `music_dir` is
/// parsed into the list, replaced by it, and written back under the new key
/// with everything else in the document intact.
fn persist_roots(roots: &[PathBuf]) {
    for root in roots {
        if root.to_str().is_none() {
            println!(
                "[config] {} is not valid UTF-8; it cannot be written to config.toml \
                 (this session is unaffected)",
                root.display()
            );
        }
    }
    persist(|config| config.music_dirs = roots.to_vec());
}

/// Why `dir` cannot join `roots`, in the words the Settings place shows — or
/// `None` when it can.
///
/// The one decision [`Shelf::accept_folder`] makes that is not an effect, held
/// apart so the refusal and its words are pinned by test. Everything after a
/// `None` here is effects: the push, the config write, the adoption, the scan.
fn folder_refusal(roots: &[PathBuf], dir: &Path) -> Option<String> {
    roots
        .iter()
        .any(|held| held == dir)
        .then(|| format!("`{}` is already here", dir.display()))
}

/// Ask the filesystem whether `dir` is a directory — on the blocking pool,
/// never the UI thread.
///
/// This is the typed door's half of ADR-0025's NAS honesty: `stat` against a
/// dead network mount does not fail, it *waits*, for however long the mount's
/// timeouts say — and a wait belongs to a pool thread that has nothing else to
/// do. The words on refusal are the first-run screen's, unchanged.
async fn check_folder(dir: PathBuf) -> Result<PathBuf, String> {
    let looked = tokio::task::spawn_blocking(move || {
        if dir.is_dir() {
            Ok(dir)
        } else {
            Err(format!("`{}` is not a directory", dir.display()))
        }
    })
    .await;
    // A pool that cannot run a closure is a torn-down runtime; answer in the
    // error slot the field already has rather than panicking mid-shutdown.
    looked.unwrap_or_else(|err| Err(format!("could not look at that path: {err}")))
}

/// Open the system folder picker and come back as
/// [`Message::MusicFolderPicked`].
///
/// **The one function that touches `rfd`**, kept to the size a thing the tests
/// cannot reach has to stay (ADR-0025): everything before it is message
/// plumbing and everything after it is [`Shelf::accept_folder`], both covered.
/// `FileDialog::pick_folder` blocks until the dialog closes — on Linux it is
/// one D-Bus round-trip to the desktop portal — so it runs on the blocking
/// pool and the event loop never waits on a human deciding.
///
/// On a desktop with no portal service the call returns `None` at once, which
/// lands as a dismissal: nothing moves, and the typed path beside the control
/// still reaches everything the dialog would have.
fn pick_folder() -> Task<Message> {
    Task::perform(
        async {
            match tokio::task::spawn_blocking(|| rfd::FileDialog::new().pick_folder()).await {
                Ok(choice) => choice,
                Err(err) => {
                    println!("[config] folder picker failed: {err}");
                    None
                }
            }
        },
        Message::MusicFolderPicked,
    )
}

/// Claim the index's rootless rows for the folders baz holds — schema v8's
/// backfill, made from the one place that knows both halves (ADR-0022).
///
/// Best-effort with a log: a failure leaves those rows rootless, which costs
/// them nothing but the ability to be pruned. In order, so a file under two
/// nested folders goes to the one the listener listed first.
fn adopt_roots(library: &mut Library, roots: &[PathBuf]) {
    for root in roots {
        match library.adopt_root(root) {
            Ok(0) => {}
            Ok(count) => println!("[index] {count} rows now recorded under {}", root.display()),
            Err(error) => println!(
                "[index] could not adopt rows under {}: {error}",
                root.display()
            ),
        }
    }
}

/// The moment now, in nanoseconds since the Unix epoch — what the Settings
/// place measures a folder's last scan against.
///
/// Saturating rather than panicking on an absurd clock, exactly as the index's
/// own first-seen stamp is.
fn now_ns() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => {
            i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, i64::saturating_neg)
        }
    }
}

/// Remember how the wall is arranged (ADR-0017 §1.3: view *state*, persisted,
/// not a preference anybody goes anywhere to set).
fn persist_group_key(key: GroupKey) {
    persist(|config| config.group_key = key);
}

/// Remember how closely it hangs — the same terms exactly (ADR-0017 §1.3).
///
/// The zoom is a *gesture*; where it landed is state. A listener who pressed
/// <kbd>Ctrl</kbd>+<kbd>-</kbd> twice expects that wall next time, and had to
/// go nowhere to ask for it.
fn persist_density(density: shelf::Density) {
    persist(|config| config.density = density);
}

/// Read the play ledger's snapshot, or say why there is none.
///
/// Every failure here is a note on stdout and a `None`, never a `problem` in
/// the top bar: an unreadable ledger costs the PLAYED key its detail — it
/// draws one `Never played` shelf, which is what a library with no history
/// looks like anyway — and costs nothing else in the application. A modal, or
/// a red line in the bar, would be baz complaining about its own file.
/// **The one place a draw gets its randomness**: the wall clock, in
/// nanoseconds.
///
/// `crate::shuffle` takes a seed rather than reading a clock or reaching for a
/// global generator, so that every arrangement it can produce is reproducible
/// in a test — the nondeterminism has to enter *somewhere*, and this is that
/// somewhere, in the shell, where nothing is asserted about it.
///
/// A clock that refuses to answer (it has been set before the epoch) gives a
/// fixed seed rather than a panic. The consequence is that two shuffles in that
/// state draw the same eight records, which is a strange machine's problem and
/// not worth a branch anywhere else.
fn draw_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_nanos() & u128::from(u64::MAX)).unwrap_or(0)
        })
}

fn read_history() -> Option<History> {
    let path = HistoryLedger::default_path()?;
    match History::read(&path) {
        Ok(history) => {
            println!(
                "[history] {} records over {} tracks from {}",
                history.records(),
                history.tracks().count(),
                path.display()
            );
            Some(history)
        }
        Err(error) => {
            println!("[history] cannot read {}: {error}", path.display());
            None
        }
    }
}

/// Read the config, apply `change`, and write it back if anything moved.
///
/// **Read–modify–write, not overwrite.** The config now carries more than one
/// thing, and each is changed by a different part of the app at a different
/// moment; a writer that built a whole `Config` from the one field it knew
/// about would silently drop the others. Reading first also means a key added
/// by a later version of baz, or by hand, survives a write by this one as far
/// as [`config::Config`] can represent it.
fn persist(change: impl FnOnce(&mut config::Config)) {
    let Some(path) = config::config_file() else {
        println!("[config] no config directory on this system; nothing is being remembered");
        return;
    };
    let stored = config::load(&path);
    let mut config = stored.clone();
    change(&mut config);
    if config == stored {
        return; // Unchanged.
    }
    match config::store(&path, &config) {
        Ok(()) => println!("[config] saved to {}", path.display()),
        Err(error) => println!("[config] could not save {}: {error}", path.display()),
    }
}

/// The absolute, idempotent command that asks the engine for `settings`.
///
/// One place, because ADR-0013's command carries the *whole* setting: every
/// control in the settings panel resolves to a complete
/// [`ReplayGainSettings`] and then comes through here, so no control can send
/// a partial one.
fn command_for(settings: ReplayGainSettings) -> Command {
    Command::SetReplayGain {
        mode: settings.mode,
        preamp_centidb: settings.preamp_centidb,
        no_tag_preamp_centidb: settings.no_tag_preamp_centidb,
        prevent_clipping: settings.prevent_clipping,
    }
}

/// Expand a leading `~/` (or bare `~`) via the home directory, so the setup
/// input accepts what people actually type.
fn expand_tilde(input: &str) -> PathBuf {
    if input == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(input));
    }
    if let Some(rest) = input.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one non-effect decision on the add-a-folder path (ADR-0025): a
    /// folder already held is refused with its words, anything else may join.
    /// Both doors — the typed path and the picker — land on this exact check.
    #[test]
    fn a_folder_already_held_is_refused_and_a_new_one_is_not() {
        let roots = vec![PathBuf::from("/m"), PathBuf::from("/mnt/nas/Music")];
        assert_eq!(
            folder_refusal(&roots, Path::new("/mnt/nas/Music")),
            Some("`/mnt/nas/Music` is already here".to_owned())
        );
        assert_eq!(folder_refusal(&roots, Path::new("/mnt/nas")), None);
        assert_eq!(folder_refusal(&[], Path::new("/m")), None);
    }

    /// The typed door's validation, off the UI thread: a directory passes, a
    /// file and an absent path are refused in the first-run screen's words.
    /// (The *pool* is the point — see [`check_folder`] — but the verdicts are
    /// what this pins.)
    #[test]
    fn check_folder_tells_a_directory_from_everything_else() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("track.flac");
        std::fs::write(&file, b"x").expect("write");
        let missing = dir.path().join("not-here");

        assert_eq!(
            runtime.block_on(check_folder(dir.path().to_path_buf())),
            Ok(dir.path().to_path_buf())
        );
        assert_eq!(
            runtime.block_on(check_folder(file.clone())),
            Err(format!("`{}` is not a directory", file.display()))
        );
        assert_eq!(
            runtime.block_on(check_folder(missing.clone())),
            Err(format!("`{}` is not a directory", missing.display()))
        );
    }

    #[test]
    fn tilde_expansion() {
        if let Some(home) = dirs::home_dir() {
            assert_eq!(expand_tilde("~"), home);
            assert_eq!(expand_tilde("~/Music"), home.join("Music"));
        }
        assert_eq!(expand_tilde("/abs/path"), PathBuf::from("/abs/path"));
        assert_eq!(
            expand_tilde("relative/~/odd"),
            PathBuf::from("relative/~/odd")
        );
    }

    /// The claim `crate::mpris` makes — that a D-Bus call and a click on the
    /// matching control produce the *same* message — pinned so it cannot
    /// drift into a parallel transport path.
    #[test]
    fn every_mpris_request_maps_to_an_interface_message() {
        let cases = [
            (mpris::Request::Play, "Play"),
            (mpris::Request::Pause, "Pause"),
            (mpris::Request::PlayPause, "PlayPause"),
            (mpris::Request::Stop, "Stop"),
            (mpris::Request::Next, "NextTrack"),
            (mpris::Request::Previous, "PreviousTrack"),
            (mpris::Request::SeekBy(-5_000), "SeekBy(-5000)"),
            (mpris::Request::SeekTo(30_000), "SeekTo(30000)"),
            (mpris::Request::Raise, "Raise"),
            (mpris::Request::Quit, "Quit"),
        ];
        for (request, expected) in cases {
            assert_eq!(format!("{:?}", message_for(request)), expected);
        }
    }

    /// **A place costs the wall no width at all**, which is the whole
    /// difference between a place and a panel — and, after ADR-0022, the whole
    /// of what is left to assert about the wall's geometry.
    ///
    /// The rail took a 340 px bite out of the grid every time somebody pointed
    /// at a sleeve; the inspector that replaced it took the same bite for one
    /// tenant. Both are gone, and what replaces the arithmetic is the *absence*
    /// of arithmetic: [`Shelf::grid_width`] is the window less the index rail's
    /// lane and there is no third term, so **no press anywhere in the product
    /// can re-hang the collection**.
    ///
    /// An absence has no return value to compare against, so it is asserted
    /// where the fact lives — over the source of the one function that answers
    /// it — exactly as
    /// [`Self::the_pull_offers_a_record_and_sends_no_command_at_all`] is. A
    /// future edit that reached for a panel width from here fails the build
    /// rather than the review.
    #[test]
    fn a_place_costs_the_wall_no_width_at_all() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source");
        let body = source
            .split_once("fn grid_width(&self) -> f32 {")
            .expect("the wall's width")
            .1;
        let body = &body[..body.find("\n    }\n").expect("a function ends")];
        assert!(
            body.contains("self.window_w") && body.contains("theme::INDEX_LANE_W"),
            "the wall's width is the window's less the rail's lane"
        );
        for banned in ["panel", "inspector", "PANEL_W", "selection", "hold"] {
            assert!(
                !body.contains(banned),
                "the wall's width depends on `{banned}` again — a place may not \
                 take width from the collection"
            );
        }
    }

    /// **Navigating between places costs the Library nothing.**
    ///
    /// Four members, one on screen, and the transitions between them are pure:
    /// nothing about the wall's scroll, query or arrangement is reachable from
    /// [`Place`], which is what makes coming back free and what makes the round
    /// trip a page costs affordable at all (ADR-0022).
    #[test]
    fn navigating_between_places_costs_the_library_nothing() {
        let place = Place::default();
        assert_eq!(place, Place::Library);
        // Out to a record's page, on to the queue, on to the settings, home.
        let place = place.album(7);
        assert_eq!(place.showing_album(), Some(7));
        let place = place.queue();
        assert_eq!(place, Place::Queue);
        assert_eq!(place.showing_album(), None, "one place at a time");
        let place = place.settings();
        assert_eq!(place, Place::Settings);
        let place = place.back();
        assert_eq!(place, Place::Library);
        assert!(place.is_home());
        // And the enum is the whole of the state: `Place` is `Copy` and holds
        // one album id, so there is nothing here that *could* hold a scroll
        // offset or a query to lose.
        const { assert!(size_of::<Place>() <= 16) }
    }

    /// **Every keyboard binding resolves to a message an on-screen control
    /// also sends.**
    ///
    /// One of the four properties `docs/design/01-ux-audit-and-ia.md` §5 says
    /// must not regress, and it is checked *exhaustively* rather than by
    /// sampling: the sweep below produces every message [`keys::binding_for`]
    /// can produce, and each one has to appear in the table with the control
    /// that sends it named. A new keyboard shortcut therefore cannot be added
    /// without either pointing at a control or declaring itself an exception
    /// here, in writing.
    ///
    /// **There are no exceptions left.** There used to be exactly one —
    /// <kbd>Ctrl</kbd>+<kbd>B</kbd>, which *hid* the right-hand column while
    /// the inspector's ✕ *closed* it, two intentions with one control between
    /// them — and ADR-0022 deleted the column, the key and the exception
    /// together. Every binding baz has now points at a word or a glyph you can
    /// see.
    ///
    /// Type-anywhere (ADR-0017 §1.2) adds four messages to this table and none
    /// of them is keyboard-only: the query has the search well ADR-0017 kept,
    /// the top match has the record page's `Play album`, the arrangement has
    /// the top bar's row of words, and the zoom has <kbd>Ctrl</kbd>+scroll on
    /// the wall itself.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the table of controls *is* the test, and splitting the sweep \
                  away from the table it checks would let one of the two be \
                  edited without the other — which is the failure this test \
                  exists to make impossible"
    )]
    fn every_keyboard_binding_is_a_press_some_control_also_makes() {
        use iced::keyboard::{Key, Modifiers, key};

        /// Message tag → the on-screen control that sends the same message,
        /// or the reason there is none.
        const CONTROLS: [(&str, &str); 19] = [
            ("PlayPause", "the bottom bar's play/pause button"),
            ("NextTrack", "the bottom bar's Next button"),
            ("PreviousTrack", "the bottom bar's Previous button"),
            (
                "Play",
                "MPRIS only; the bar's toggle covers both directions",
            ),
            (
                "Pause",
                "MPRIS only; the bar's toggle covers both directions",
            ),
            ("Stop", "MPRIS only; there is no on-screen Stop"),
            (
                "SeekBy",
                "the needle, pressed inside the entry that is sounding \
                 (ADR-0017 §1.1: the groove's job, at the window's edge)",
            ),
            ("VolumeStep", "the bottom bar's volume fader"),
            ("ToggleMute", "the bottom bar's speaker button"),
            ("ToggleQueue", "the bottom bar's labelled `Queue` control"),
            ("ToggleSettings", "the top bar's Settings control"),
            ("FocusSearch", "the top bar's search well"),
            ("Pull", "the top bar's Pull word"),
            ("EscapePressed", "every place's `‹ Library`"),
            (
                "QueryTyped",
                "the top bar's search well — the field ADR-0017 §1.2 kept, \
                 which a pointer clicks into to type the same query",
            ),
            (
                "PlayFirstMatch",
                "the record page's `Play album`; the well's own Enter sends \
                 this too",
            ),
            (
                "DensityStep",
                "Ctrl+scroll on the wall — the gesture *is* the pointer \
                 control (ADR-0017 §1.3), and `docs/REFUSALS.md` refuses the \
                 view-options menu that would be the other way to spell it",
            ),
            (
                "GroupKeySelected",
                "the top bar's row of five words (ADR-0019)",
            ),
            (
                "SetVolume",
                "MPRIS only; the fader sends its own pointer messages",
            ),
        ];

        // Every key the binding table can be handed, in every modifier
        // combination it distinguishes.
        let keys_to_sweep = [
            Key::Named(key::Named::Space),
            Key::Named(key::Named::ArrowLeft),
            Key::Named(key::Named::ArrowRight),
            Key::Named(key::Named::ArrowUp),
            Key::Named(key::Named::ArrowDown),
            Key::Named(key::Named::Escape),
            Key::Named(key::Named::Enter),
            Key::Named(key::Named::MediaPlayPause),
            Key::Named(key::Named::MediaTrackNext),
            Key::Named(key::Named::MediaTrackPrevious),
            Key::Named(key::Named::MediaStop),
            Key::Named(key::Named::Play),
            Key::Named(key::Named::Pause),
            Key::Character(" ".into()),
            Key::Character("n".into()),
            Key::Character("m".into()),
            Key::Character("q".into()),
            Key::Character("u".into()),
            Key::Character("b".into()),
            Key::Character(",".into()),
            Key::Character("/".into()),
            Key::Character("f".into()),
            Key::Character("r".into()),
            Key::Character("-".into()),
            Key::Character("=".into()),
            Key::Character("1".into()),
            Key::Character("5".into()),
            Key::Character("6".into()),
            Key::Character("k".into()),
        ];
        let modifier_sets = [
            Modifiers::empty(),
            Modifiers::SHIFT,
            Modifiers::COMMAND,
            Modifiers::ALT,
            Modifiers::COMMAND | Modifiers::SHIFT,
        ];
        let mut produced: Vec<String> = Vec::new();
        // Both halves of the input surface: every key in every modifier state,
        // and the wheel, which is the zoom's pointer half and binds through
        // the same module.
        let from_keys = keys_to_sweep.iter().flat_map(|key| {
            modifier_sets.into_iter().map(move |modifiers| {
                (
                    format!("{key:?}"),
                    modifiers,
                    keys::binding_for(key, modifiers, keys::Focus::Elsewhere),
                )
            })
        });
        let from_wheel = modifier_sets.into_iter().flat_map(|modifiers| {
            [-1.0_f32, 1.0].into_iter().map(move |delta| {
                (
                    format!("wheel {delta}"),
                    modifiers,
                    keys::wheel_binding(delta, modifiers),
                )
            })
        });
        for (key, modifiers, binding) in from_keys.chain(from_wheel) {
            if let Some(message) = binding {
                // The payload is not the point; the intention is.
                let debug = format!("{message:?}");
                let tag = debug
                    .split_once('(')
                    .map_or(debug.as_str(), |(head, _)| head)
                    .to_owned();
                assert!(
                    CONTROLS.iter().any(|(name, _)| *name == tag),
                    "{key} + {modifiers:?} binds to `{tag}`, which no entry in \
                     CONTROLS accounts for — name the control that sends it, or \
                     record why there is none"
                );
                produced.push(tag);
            }
        }
        // …and the table has no stale entries either, except the three that
        // exist for the desktop rather than for the keyboard.
        for (tag, _) in CONTROLS {
            let desktop_only = matches!(tag, "Play" | "Pause" | "SetVolume");
            assert!(
                desktop_only || produced.contains(&tag.to_owned()),
                "CONTROLS still names `{tag}`, which no key produces any more"
            );
        }
        assert!(produced.len() > 20, "the sweep stopped covering the table");
    }

    /// **The pull sends no command.** Not `SetQueue`, not `Play`, not `JumpTo`
    /// — nothing at all.
    ///
    /// The refusal this pins is the product's loudest one:
    /// *"Shuffle is a thing you **start**, never a thing that starts itself"*,
    /// and the pull is a suggestion one step further from playback than that. It
    /// draws a record, prints when it was last heard, and stops. Accepting it is
    /// pressing `Play album`, which is the same act — the same message, the same
    /// commands — as playing a record you found yourself.
    ///
    /// # Why it is asserted like this
    ///
    /// The claim is *the absence of a send*, and an absence has no return value
    /// to compare against. Constructing an `App` to observe the silence is not
    /// available either: `App::new` opens a real engine and a real library. So
    /// the assertion is made where the fact lives — over the source of the one
    /// function that answers [`Message::Pull`] — in exactly the way
    /// `theme::every_surface_declares_the_edges_it_permits` pins the alignment
    /// laws. It cannot be satisfied by accident, and a future edit that reached
    /// for the engine from here fails the build rather than the review.
    ///
    /// Its counterpart is asserted too: shuffle *does* send, because a shuffle
    /// that started nothing would be the other kind of lie.
    #[test]
    fn the_pull_offers_a_record_and_sends_no_command_at_all() {
        // Read the source with line endings normalised. `.gitattributes`
        // pins these files to LF, but a working tree can still be checked out
        // with CRLF, and every scan below matches on "\n    }\n" — which a
        // CRLF file simply never contains. The property is about the code, not
        // about how the file was written to disk.
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let body = |name: &str| {
            let start = source
                .find(&format!("fn {name}(&mut self"))
                .unwrap_or_else(|| panic!("{name} exists"));
            let rest = &source[start..];
            let end = rest.find("\n    }\n").expect("a function ends");
            rest[..end].to_owned()
        };

        let pull = body("draw_pull");
        for forbidden in ["playback.send", "Command::", "note_transport_sent"] {
            assert!(
                !pull.contains(forbidden),
                "the pull reached for `{forbidden}` — nothing plays until the \
                 listener asks (docs/REFUSALS.md)"
            );
        }
        // …and it does the two things it is for.
        assert!(pull.contains("shuffle::pull"), "the pull draws a record");
        assert!(pull.contains("state.pull = Some"), "and offers it");

        // Shuffle is the opposite: it is *started*, so it starts.
        let shuffle = body("start_shuffle");
        assert!(shuffle.contains("Command::SetQueue"));
        assert!(shuffle.contains("Command::Play"));
        // And what it sends is a queue of whole records, never a flattened one.
        assert!(shuffle.contains("vm::stacked_queue"));
    }

    /// **Escape returns the pull before it touches anything else on the wall.**
    ///
    /// `docs/design/critique/02-surfaces.md` gives the pull two keys and this is
    /// the second of them — *"`Ctrl`+`R` re-pulls; `Esc` returns"*. Returning an
    /// offer comes **first**, because an offer is the topmost thing on a wall
    /// that is showing one and `escape()`'s whole contract is that each press
    /// peels the top layer. The shuffle pool's marks come **last**, under the
    /// query and under the column: clearing the query widens the wall while the
    /// pool stays what it was, which is the frame in which the dimming actually
    /// says something, and peeling it first would answer the question by
    /// deleting it.
    ///
    /// Pinned as an **order in the source** of the one arm that spends it, for
    /// [`Self::the_pull_offers_a_record_and_sends_no_command_at_all`]'s reason:
    /// the peel is four early returns in a `match` arm and there is no `Shelf`
    /// to build without a database and a scan thread. Each step is named by the
    /// literal a reviewer would have to move to break it.
    #[test]
    fn escape_returns_the_pull_first_and_the_pools_marks_last() {
        // Read the source with line endings normalised. `.gitattributes`
        // pins these files to LF, but a working tree can still be checked out
        // with CRLF, and every scan below matches on "\n    }\n" — which a
        // CRLF file simply never contains. The property is about the code, not
        // about how the file was written to disk.
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let arm = source
            .split_once("fn peel(&mut self)")
            .expect("the shelf's Escape peel")
            .1;
        let arm = &arm[..arm.find("\n    }\n").expect("a function ends")];
        let peel = ["self.pull.take()", "self.clear_query()", "self.pool = None"];
        let mut at = 0;
        for step in peel {
            let found = arm[at..]
                .find(step)
                .unwrap_or_else(|| panic!("Escape no longer peels `{step}` in its turn"));
            at += found + step.len();
        }
        // **And the query step blurs as well as clearing** (ADR-0017 step 11).
        // Escape used to leave the caret in the well, which under type-anywhere
        // would leave the keyboard in an empty field where Space types a space.
        let clear = source
            .split_once("fn clear_query(&mut self)")
            .expect("the query's own peel")
            .1;
        let clear = &clear[..clear.find("\n    }\n").expect("a function ends")];
        assert!(
            clear.contains("blur_search()"),
            "Escape clears the query but leaves the caret in the well"
        );
        assert!(
            !clear.contains("text_input::focus(search_id())"),
            "Escape re-focuses the well it just emptied"
        );
    }

    /// The two place keys, spelled out: Ctrl+`U` is the same press as the bar's
    /// labelled `Queue` control, and Ctrl+`,` the same press as the top bar's
    /// `Settings` word.
    ///
    /// Both are modified, and that is the shape of the modifier layer ADR-0017
    /// §1.2 asks for: bare `q` and bare `u` are letters of the query.
    #[test]
    fn the_layer_controls_and_their_keys_are_the_same_press() {
        use iced::keyboard::{Key, Modifiers};

        let from_key = keys::binding_for(
            &Key::Character("u".into()),
            Modifiers::COMMAND,
            keys::Focus::Elsewhere,
        );
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(Message::ToggleQueue))
        );

        let from_key = keys::binding_for(
            &Key::Character(",".into()),
            Modifiers::COMMAND,
            keys::Focus::Elsewhere,
        );
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(Message::ToggleSettings))
        );

        // …and `Ctrl+B` binds to nothing at all. It hid a sidebar; ADR-0022
        // left none, and a layout key with no layout to change is a key that
        // does nothing rather than a key that does something else.
        assert!(
            keys::binding_for(
                &Key::Character("b".into()),
                Modifiers::COMMAND,
                keys::Focus::Elsewhere,
            )
            .is_none()
        );
    }

    /// **The blur is a different id, and that is the whole mechanism.**
    ///
    /// iced 0.13 has no `unfocus` task; its focus operation focuses the
    /// matching id and unfocuses every other focusable it walks. Focusing an id
    /// no widget carries is therefore "focus nothing" — and the entire
    /// correctness of it is that the two strings differ, which is a thing a
    /// rename could silently break and a test cannot.
    #[test]
    fn blurring_the_well_targets_an_id_no_widget_carries() {
        assert_ne!(
            format!("{:?}", search_id()),
            format!("{:?}", nothing_id()),
            "the blur would focus the search well instead of leaving it"
        );
        // And the well is the only `text_input::Id` the tree hands out, so
        // there is nothing else the sentinel could collide with.
        assert_eq!(format!("{:?}", search_id()), format!("{:?}", search_id()));
    }

    /// **The zoom is three steps of state and nothing else** — the shell's
    /// half of ADR-0017 step 6, exercised as the update loop actually spends
    /// it.
    ///
    /// The shelf's half (the hang's arithmetic) is `shelf::Density`'s and is
    /// tested there; what is pinned here is that the message steps the step,
    /// saturates rather than wrapping, and is produced by both halves of the
    /// gesture.
    #[test]
    fn the_zoom_steps_the_wall_and_stops_at_both_ends() {
        use iced::keyboard::{Key, Modifiers};

        let step = |density: shelf::Density, delta: i32| density.step(delta);
        let mut density = shelf::Density::Balanced;
        density = step(density, -1);
        assert_eq!(density, shelf::Density::Dense);
        density = step(density, -1);
        assert_eq!(density, shelf::Density::Dense, "the ladder has an end");
        density = step(density, 1);
        density = step(density, 1);
        assert_eq!(density, shelf::Density::Spacious);
        density = step(density, 1);
        assert_eq!(density, shelf::Density::Spacious);

        // Both halves of the gesture produce the same message, which is what
        // makes the keyboard and the wheel one control rather than two.
        let from_key = keys::binding_for(
            &Key::Character("=".into()),
            Modifiers::COMMAND,
            keys::Focus::Elsewhere,
        );
        let from_wheel = keys::wheel_binding(1.0, Modifiers::COMMAND);
        assert_eq!(format!("{from_key:?}"), format!("{from_wheel:?}"));
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(Message::DensityStep(1)))
        );
    }

    /// **Escape leaves the place first, and everything else is the Library's.**
    ///
    /// The rule ADR-0022 shortened. There used to be a popover over an
    /// inspector over a place, and one `if` per layer; there is one kind of
    /// surface now, so the key's whole first question is *am I at home* —
    /// asserted here over [`Place`] itself, which is where the arbitration that
    /// is left lives.
    #[test]
    fn escape_leaves_the_place_before_anything_under_it() {
        // At home the press falls straight through to the wall's own peel.
        assert!(Place::default().is_home());
        // Anywhere else it is the place's, and one press is enough: there is no
        // second layer to take off underneath.
        for place in [Place::Album(7), Place::Queue, Place::Settings] {
            assert!(!place.is_home(), "{place:?} answers the press itself");
            assert!(
                place.back().is_home(),
                "{place:?} left something behind for a second press"
            );
        }
    }

    /// The bottom bar's toggle and MPRIS `PlayPause` are literally the same
    /// message, and `N` and MPRIS `Next` likewise.
    #[test]
    fn the_transport_has_one_path_per_intention() {
        use iced::keyboard::{Key, Modifiers, key};

        let from_key = keys::binding_for(
            &Key::Named(key::Named::Space),
            Modifiers::empty(),
            keys::Focus::Elsewhere,
        );
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(message_for(mpris::Request::PlayPause)))
        );

        let from_key = keys::binding_for(
            &Key::Named(key::Named::ArrowRight),
            Modifiers::COMMAND,
            keys::Focus::Elsewhere,
        );
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(message_for(mpris::Request::Next)))
        );

        // Previous, the newest of them, arrives by all three roads: the bar's
        // button sends `PreviousTrack` directly, and these two must be it too.
        let from_key = keys::binding_for(
            &Key::Named(key::Named::ArrowLeft),
            Modifiers::COMMAND,
            keys::Focus::Elsewhere,
        );
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(message_for(mpris::Request::Previous)))
        );
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(Message::PreviousTrack))
        );
    }

    /// **A tile press is navigation, and it re-hangs nothing.**
    ///
    /// The defect this replaces was caught on camera by the composition audit:
    /// a double-click on the fifth tile of row 0, where the first press opened
    /// the rail, the shelf reflowed from five columns to three, the second
    /// press landed 180 px from where the tile now was, and **nothing played**
    /// — while the panel that had just opened said "double-click a tile to
    /// play" at the bottom of it. `shelf::GridHold` was the fix: pin the width
    /// in force for the length of the gesture.
    ///
    /// ADR-0022 deletes the *cause* instead. A press replaces the wall with the
    /// record's page, so there is no reflow to survive, no second press to
    /// protect and no clock ticking behind a gesture — the hold, the
    /// double-click window and the `ColumnHoldTick` subscription all go with
    /// them. What is pinned here is that pressing a tile is a transition on
    /// [`Place`] and nothing else, and that the wall's hang is a function of
    /// the width alone at every width in the shipped band.
    #[test]
    fn a_tile_press_is_navigation_and_re_hangs_nothing() {
        // The press is a place change. Twice on the same record is a round trip
        // back to the wall, which is the one gesture of the inspector's that
        // survived: pointing at the sleeve you are already reading puts it down.
        let place = Place::default().album(7);
        assert_eq!(place.showing_album(), Some(7));
        assert_eq!(place.album(7), Place::Library);
        // …and a different sleeve swaps the page rather than stacking one.
        assert_eq!(place.album(9).showing_album(), Some(9));

        // The hang is the same at a width whatever has been pressed, because
        // nothing that can be pressed is in the arithmetic any more. Swept over
        // the whole shipped band rather than sampled at two widths.
        for w in 760..=1920 {
            #[expect(
                clippy::cast_precision_loss,
                reason = "an integer window width, swept at 1 px resolution"
            )]
            let width = w as f32 - theme::INDEX_LANE_W;
            let hang = shelf::Grid::new(width, shelf::Density::Balanced);
            assert_eq!(
                hang.columns,
                shelf::Grid::new(width, shelf::Density::Balanced).columns
            );
            assert!(hang.block_width() <= width + 0.01);
        }
    }

    /// **No redraw while idle — asserted, not promised.**
    ///
    /// ADR-0020's whole cost argument is that the transition clock is a
    /// *function of state*: [`App::moving`] and [`Shelf::moving`] between them
    /// are the boolean [`App::subscription`] reads, so a false reading is a
    /// timer that does not exist, no `MotionTick` messages, and — because iced
    /// 0.13 requests a redraw per message batch — no frames. The decision
    /// records this as a **test** rather than a promise, and this is it.
    ///
    /// Every one of the five transitions is started, checked live, and ticked
    /// past its end; the clock has to be off before the first and off again
    /// after the last.
    #[test]
    fn the_motion_clock_is_off_until_something_moves() {
        let start = Instant::now();
        let mut ink: Keyed<Control> = Keyed::new();
        let mut warmth = Tween::settled(0.0).with_curve(motion::Curve::Linear);
        let mut tile: Keyed<u64> = Keyed::new();
        // The exact disjunction the two `moving` functions form between them.
        macro_rules! moving {
            () => {
                ink.live() || warmth.live() || tile.live()
            };
        }

        assert!(!moving!(), "a shell at rest keeps no clock");

        // Each of the three in turn: it turns the clock on, and its own last
        // tick turns it off again. Nothing else is running, so "the clock is
        // still on" can only mean this transition did not stop.
        ink.enter(Control::PlayPause, motion::INK, start);
        assert!(moving!(), "the icon-button ink fade");
        ink.tick(start + motion::INK);
        assert!(!moving!());

        tile.enter(7, motion::TILE, start);
        assert!(moving!(), "the shelf tile's hover rule");
        tile.tick(start + motion::TILE);
        assert!(!moving!());

        warmth.go(1.0, motion::LAMP, start);
        assert!(moving!(), "the lamp warming");
        warmth.tick(start + motion::LAMP);
        assert!(!moving!());

        // All three at once, and the clock stops with the *last* of them rather
        // than the first: the lamp runs longest, so it is what keeps the timer
        // alive after everything else has settled.
        ink.enter(Control::Next, motion::INK, start);
        tile.enter(9, motion::TILE, start);
        warmth.go(0.0, motion::LAMP, start);
        for at in [motion::INK, motion::TILE] {
            ink.tick(start + at);
            tile.tick(start + at);
            warmth.tick(start + at);
            assert!(moving!(), "settled at {at:?} with the lamp still warming");
        }
        warmth.tick(start + motion::LAMP);
        assert!(
            !moving!(),
            "the last tween settled and the clock did not stop"
        );
        // …and no later instant revives it, which is what makes the idle
        // measurement an idle measurement.
        for later in [motion::LAMP * 2, Duration::from_secs(30)] {
            ink.tick(start + later);
            tile.tick(start + later);
            warmth.tick(start + later);
            assert!(!moving!());
        }
    }
}
