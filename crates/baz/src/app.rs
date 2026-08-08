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
//!   side panel's Play button render that state.
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
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use baz_core::index::Library;
use baz_core::protocol::{self as protocol, Command, Event, SignalChain};
use baz_core::replaygain::ReplayGainSettings;
use iced::keyboard;
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{
    Space, column, container, image as iced_image, mouse_area, opaque, row, scrollable, stack,
    text_input, vertical_rule,
};
use iced::{Element, Length, Size, Subscription, Task, alignment, window};
use lru::LruCache;

use crate::mpris::Mpris;
use crate::overlay::{Overlay, Popover};
use crate::panels::{Panels, Rail};
use crate::playback::{Playback, PlayerEvent};
use crate::player::{Availability, PlayerState};
use crate::scan::ScanUpdate;
use crate::theme::PANEL_W;
use crate::{art, config, font, keys, mpris, player, queue_edit, scan, shelf, theme, views, vm};

/// Approximate top-bar height, used only for the pre-first-scroll estimate
/// of the grid viewport (real bounds arrive with every scroll event).
const TOP_BAR_H: f32 = 56.0;
/// Initial window size.
const WINDOW: Size = Size::new(1280.0, 860.0);

/// How often the grid checks whether its held column count has expired.
///
/// Subscribed **only** while a hold stands (see [`shelf::ColumnHold`]), which
/// is at most [`shelf::DOUBLE_CLICK`] after a tile click, so this is a handful
/// of ticks per click and nothing at all the rest of the time. Coarse on
/// purpose: the hold's job is to survive one gesture, and the reflow landing a
/// few tens of milliseconds late is invisible where landing 300 ms early
/// broke a documented one.
const COLUMN_HOLD_TICK: Duration = Duration::from_millis(40);

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

/// Run the application. `started` is process start, for the
/// startup-to-interactive log; `cli_dir` is the optional `baz [DIR]` arg.
///
/// The bundled typeface is installed here and nowhere else: every face in
/// [`crate::font::FACES`] is handed to the toolkit before the window exists,
/// and [`theme::SANS`] is named as the default so that a `text` widget with no
/// font of its own gets a real face rather than the platform's guess at
/// `Family::SansSerif` (see `font.rs` for what that guess used to cost).
pub fn run(started: Instant, cli_dir: Option<PathBuf>) -> iced::Result {
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
    /// Esc anywhere: peel one layer, top down — the popover, then the search
    /// query, then the album inspector (see [`App::escape`]).
    EscapePressed,
    /// The bar's now-playing block, or `Q`: show what is playing next, or put
    /// it away (see [`crate::overlay`]).
    ToggleUpNext,
    /// The popover's ✕, or a press anywhere outside it: close **Up next**.
    ///
    /// Distinct from [`Self::ToggleUpNext`] because click-outside must not be
    /// a toggle — the press that dismisses a popover cannot be the press that
    /// re-opens it.
    CloseUpNext,
    /// A row of the **Up next** popover was clicked: play the queue from that
    /// zero-based position ([`Command::JumpTo`], ADR-0014).
    ///
    /// Unlike [`Self::PlayTrack`] this needs no decision about re-queueing —
    /// the list the row was drawn from *is* what the engine is holding, by
    /// construction.
    JumpToQueued(usize),
    /// A row's ✕ in the **Up next** popover: take that entry out of the queue
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
    /// Top bar's Settings toggle, or Ctrl+`,`: show the settings, or put back
    /// whatever they were covering.
    ToggleSettings,
    /// Ctrl+B: dismiss the right-hand rail and give the shelf its width back,
    /// or bring back the panel that was dismissed.
    TogglePanels,
    /// A panel's ✕: close whichever panel the rail is showing.
    ClosePanel,
    /// Shelf scrolled; carries the real viewport geometry.
    Scrolled(Viewport),
    /// Window resized (approximate grid geometry until the next scroll).
    WindowResized(Size),
    /// An album tile was clicked (toggles selection / side panel; a second
    /// click within [`shelf::DOUBLE_CLICK`] plays the album).
    AlbumClicked(u64),
    /// The grid's held column count may have expired — ticked only while one
    /// is held (see [`shelf::ColumnHold`]).
    ColumnHoldTick,
    /// Queue the album's tracks and play (side-panel Play, tile
    /// double-click).
    PlayAlbum(u64),
    /// A track row of the album inspector was clicked: play that album from
    /// that row (`album id`, zero-based row). One message for both of
    /// ADR-0014's cases — which commands go out is
    /// [`PlayerState::play_from`](crate::player::PlayerState::play_from)'s
    /// decision, not the view's.
    PlayTrack(u64, usize),
    /// Side panel: a different format of the selected album was picked.
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
    /// Bottom bar: the pointer went down on the seek bar, this far along it.
    /// Nothing is requested and nothing moves yet — the gesture is a click
    /// until it travels [`player::DRAG_THRESHOLD_PX`].
    SeekPressed(player::Pointer),
    /// Bottom bar: the pointer moved with the seek bar held. Past the
    /// threshold this is the scrub, and the bar follows the pointer.
    SeekDragged(player::Pointer),
    /// Bottom bar: the pointer moved over the seek bar with nothing held —
    /// the hover preview follows it.
    SeekHovered(player::Pointer),
    /// Bottom bar: the pointer left the seek bar; the preview goes with it.
    SeekLeft,
    /// Bottom bar: the seek bar was released — the moment the request
    /// actually goes to the engine.
    SeekReleased,
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
    /// An engine event arrived over the bridge subscription.
    Playback(PlayerEvent),
    /// An off-thread thumbnail decode finished (`None` = no usable art).
    ThumbLoaded(u64, Option<iced_image::Handle>),
    /// ~10 Hz drain of the scan worker's channel while a scan runs.
    ScanTick,
    /// A frame was presented (subscribed only until first-frame is logged).
    FirstFrame,
}

struct App {
    started: Instant,
    first_frame_logged: bool,
    screen: Screen,
    /// The overlay layer: which popover, if any, is floating over the place.
    ///
    /// It lives on the shell rather than on the shelf because a popover
    /// anchored to the *bar* belongs to every place the bar is in, which is all
    /// of them (see [`crate::overlay`]).
    overlay: Overlay,
    /// Which row of the **Up next** popover the pointer is on, if any.
    ///
    /// The popover's rows offer their removal ✕ on hover only, and iced 0.13
    /// has no way for one widget to ask whether a *sibling* is hovered — a
    /// style function learns its own status and nothing else. So the row
    /// reports its own crossings with a `mouse_area` and the shell holds the
    /// one answer. The ✕'s slot is reserved either way, so this changes what is
    /// drawn in it and never the geometry around it.
    hovered_queue_row: Option<usize>,
    /// The window's size, as the last resize event reported it.
    ///
    /// Held for one job: an overlay has no parent to be a fraction of, so the
    /// popover's height ceiling ([`theme::POPOVER_MAX_H`]) has to be computed
    /// against the window itself. The shelf keeps its own, separately measured
    /// geometry — that one is the *viewport's*, which is not the window's once
    /// the bars and the rail have taken their share.
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
    /// The ReplayGain setting as it currently stands on disk.
    ///
    /// Kept so that persisting can be driven by the *engine's* confirmations
    /// (the honesty rule again: what is written is what is in force, never
    /// what was asked for) without reading the config file on every
    /// `ReplayGainChanged` — the event also arrives at track boundaries, where
    /// the settings have not moved at all and there is nothing to write.
    saved_replay_gain: ReplayGainSettings,
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
        let dir = cli_dir.or_else(|| stored.and_then(|config| config.music_dir));
        let (screen, task) = match dir {
            None => (Screen::Setup(Setup::fresh(None)), Task::none()),
            Some(dir) => match Shelf::open(dir) {
                Ok((shelf, task)) => (Screen::Shelf(Box::new(shelf)), task),
                Err(error) => (Screen::Setup(Setup::fresh(Some(error))), Task::none()),
            },
        };
        let mut app = Self {
            started,
            first_frame_logged: false,
            screen,
            overlay: Overlay::new(),
            hovered_queue_row: None,
            window: WINDOW,
            playback,
            player,
            mpris,
            mpris_art: (0, None),
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
        if self.update_volume(&message)
            || self.update_replay_gain(&message)
            || self.update_transport(&message)
            || self.update_overlay(&message)
        {
            return Task::none();
        }
        match message {
            Message::EscapePressed => self.escape(),
            Message::WindowResized(size) => {
                self.window = size;
                match &mut self.screen {
                    Screen::Shelf(state) => state.update(Message::WindowResized(size)),
                    Screen::Setup(_) => Task::none(),
                }
            }
            Message::FirstFrame => {
                if !self.first_frame_logged {
                    self.first_frame_logged = true;
                    println!(
                        "[startup] startup-to-interactive: {:.1} ms",
                        self.started.elapsed().as_secs_f64() * 1e3
                    );
                }
                Task::none()
            }
            Message::SetupSubmit => self.submit_setup(),
            Message::Playback(event) => {
                self.apply_player_event(event);
                Task::none()
            }
            Message::PlayAlbum(id) => {
                self.play_album(id);
                Task::none()
            }
            Message::PlayTrack(id, row) => {
                self.play_track(id, row);
                Task::none()
            }
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
            Message::SeekPressed(pointer) => {
                self.player.press(pointer);
                Task::none()
            }
            Message::SeekDragged(pointer) => {
                self.player.drag_to(pointer);
                Task::none()
            }
            Message::SeekHovered(pointer) => {
                self.player.hover_to(pointer);
                Task::none()
            }
            Message::SeekLeft => {
                self.player.hover_left();
                Task::none()
            }
            Message::SeekReleased => {
                let target = self.player.release_drag();
                self.send_seek(target);
                Task::none()
            }
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

    /// Answer an overlay message, reporting whether it was one.
    ///
    /// The **Up next** popover and everything a listener can do to a row of it,
    /// answered as one small machine for the reason the volume's nine and
    /// ReplayGain's four are: they all belong to one surface, and folding six
    /// more arms into the shell's own match would bury the four messages that
    /// are genuinely about the whole application.
    ///
    /// <kbd>Esc</kbd> is deliberately *not* here. It is not an overlay message;
    /// it is the message that has to know about every layer, so it stays in the
    /// shell where the layers are ([`Self::escape`]).
    fn update_overlay(&mut self, message: &Message) -> bool {
        match *message {
            Message::ToggleUpNext => self.overlay.toggle_up_next(),
            Message::CloseUpNext => {
                self.overlay.close();
                // A popover that is gone has no row under the pointer, and the
                // rows will not be there to report their own exit.
                self.hovered_queue_row = None;
            }
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

    /// <kbd>Esc</kbd>: **peel one layer, top down.**
    ///
    /// The whole of the rule the redesign bought, and the reason it is short:
    /// the window holds one place, one inspector attached to it, and one
    /// popover attached to the bar, so at any moment exactly one of them is the
    /// top layer and the key has nothing to arbitrate (ADR-0015 §2.2 rule 5).
    /// It used to mean "clear the search, else close whichever of three
    /// unrelated panels the rail happened to be showing".
    ///
    /// The order is the stacking order, outermost first:
    ///
    /// 1. **The popover**, when one is floating. [`Overlay::close`] reports
    ///    whether it had anything to close, so an empty overlay does not eat
    ///    the press.
    /// 2. **The search query**, when the well is not empty — the layer the
    ///    README documents, and the one press that keeps focus where it is.
    /// 3. **The album inspector**, which is the last thing left to peel.
    ///
    /// (In the search field itself iced 0.13's `text_input` consumes
    /// <kbd>Esc</kbd> to blur before this is reached at all; that is the
    /// documented two-press behaviour, and §4.6 of the design spec owns the
    /// fix.)
    fn escape(&mut self) -> Task<Message> {
        if self.overlay.close() {
            return Task::none();
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
        match Shelf::open(dir) {
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
        // A queue where there was none moves `CanPlay`, exactly as in
        // `play_album`, and that is the one MPRIS-visible change that arrives
        // without an engine event.
        self.publish_mpris(false);
    }

    /// Play the queue from `position` — a click on a row of **Up next**
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

    /// Take row `row` out of the queue — a click on a row's ✕ in **Up next**
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

    /// The whole window: the current place, whatever is floating over it, and
    /// the persistent bottom bar under both. Composition only — every surface
    /// is drawn by [`crate::views`].
    fn view(&self) -> Element<'_, Message> {
        let screen: Element<'_, Message> = match &self.screen {
            Screen::Setup(setup) => return views::setup::view(setup),
            Screen::Shelf(state) => state.view(&self.player),
        };
        // The persistent bottom bar lives under the shelf — unless this
        // build has no audio output at all, in which case playback UI is
        // hidden entirely. With no bar there is nothing to anchor a popover
        // to and no queue to put in one, so the overlay goes with it.
        if *self.player.availability() == Availability::NotBuilt {
            return screen;
        }
        column![
            self.floating_over(screen),
            views::bottom_bar::view(&self.player, self.overlay.is_open()),
        ]
        .into()
    }

    /// Put the overlay layer over `place`, if anything is floating.
    ///
    /// Three stacked layers and each one is load-bearing (§2.4, §4.6 — the
    /// primitives were verified against `iced_widget` 0.13.4 before the
    /// surface was specified):
    ///
    /// 1. **the place**, untouched — it does not reflow, it does not dim, and
    ///    it goes on scrolling under the popover, because a `mouse_area` that
    ///    handles only presses passes a wheel event straight through;
    /// 2. **a full-bleed `mouse_area`** whose press is the popover's
    ///    dismissal. This is the click-outside iced 0.13 gives no other route
    ///    to, and it is the layer that makes the overlay feel like an overlay;
    /// 3. **the popover**, wrapped in `opaque`, which is documented to capture
    ///    mouse presses inside its own bounds precisely so that events do not
    ///    fall through a stack. It is aligned bottom-right inside a plain
    ///    container that fills the layer, so the *container* passes presses on
    ///    to layer 2 while the popover itself keeps them.
    ///
    /// Note what the stack sits inside: only the place, never the bar. The
    /// popover is anchored above the transport and cannot cover a pixel of it,
    /// which is the one promise the rail's defenders were right to extract
    /// (§2.4) — and it means the transport stays live and clickable while the
    /// queue is open, which is what "explicitly non-modal" means in practice.
    fn floating_over<'a>(&self, place: Element<'a, Message>) -> Element<'a, Message> {
        let Some(popover) = self.overlay.showing() else {
            return place;
        };
        let content = match popover {
            Popover::UpNext => views::up_next::view(
                &self.player,
                self.window.height * theme::POPOVER_MAX_H,
                self.hovered_queue_row,
            ),
        };
        stack![
            place,
            mouse_area(Space::new(Length::Fill, Length::Fill)).on_press(Message::CloseUpNext),
            container(opaque(content))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(alignment::Horizontal::Right)
                .align_y(alignment::Vertical::Bottom)
                .padding(theme::GAP_LG),
        ]
        .into()
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
        // The scan channel is drained on a coarse tick — batching by design.
        if let Screen::Shelf(state) = &self.screen {
            if state.scanning {
                subs.push(iced::time::every(Duration::from_millis(100)).map(|_| Message::ScanTick));
            }
            // Only while a tile click is holding the grid's columns still, and
            // never otherwise: the hold's expiry is the one layout change with
            // no input behind it, so it is the one thing that needs a clock to
            // notice it. (See [`shelf::ColumnHold`].)
            if state.column_hold.holding() {
                subs.push(iced::time::every(COLUMN_HOLD_TICK).map(|_| Message::ColumnHoldTick));
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

/// The shelf screen: library, scan state, and grid/panel view state.
///
/// Fields the view layer reads are `pub(crate)`; the ones the update loop
/// owns alone (in-flight decodes, the scan channel, click timing) stay
/// private — [`crate::views`] draws this state, it never steers it.
pub(crate) struct Shelf {
    /// The open library: the search index the counts and the query run over.
    pub(crate) library: Library,
    /// Owned view model of every album, in `Library::albums` order.
    pub(crate) albums: Vec<vm::AlbumVm>,
    /// Indices into `albums` that survive the current query.
    pub(crate) visible: Vec<usize>,
    /// The live search text.
    pub(crate) query: String,
    /// What the right-hand rail is showing, and what it would show — the
    /// album selection included, since selection and visibility are one
    /// question (see [`crate::panels`]).
    pub(crate) panels: Panels,
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
    /// Last tile click, for double-click-to-play detection.
    last_click: Option<(u64, Instant)>,
    /// The column count pinned across the reflow a tile click causes, so the
    /// tile does not move between the two presses of a double-click (see
    /// [`shelf::ColumnHold`]).
    pub(crate) column_hold: shelf::ColumnHold,
}

impl Shelf {
    /// Open the library DB, hydrate the shelf, persist the chosen dir, and
    /// kick off the scan worker. Errors are user-presentable strings.
    fn open(music_dir: PathBuf) -> Result<(Self, Task<Message>), String> {
        let t0 = Instant::now();
        let db_path = config::library_db_file()
            .ok_or_else(|| "no usable data directory on this system".to_owned())?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let library = Library::open(&db_path)
            .map_err(|e| format!("cannot open library at {}: {e}", db_path.display()))?;
        let albums = vm::build_albums(&library);
        println!(
            "[startup] library open + hydrate: {:.1} ms ({} albums / {} tracks) from {}",
            t0.elapsed().as_secs_f64() * 1e3,
            albums.len(),
            library.len(),
            db_path.display()
        );

        persist_music_dir(&music_dir);
        // The snapshot is what makes the scan incremental — and the only
        // rows it is ever allowed to prune (see `scan::vanished`).
        let scan_rx = scan::spawn(music_dir, library.known_files());

        let mut shelf = Self {
            library,
            visible: (0..albums.len()).collect(),
            albums,
            query: String::new(),
            panels: Panels::new(),
            edition_choice: HashMap::new(),
            thumbs: LruCache::new(
                NonZeroUsize::new(art::THUMB_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN),
            ),
            pending: HashSet::new(),
            no_art: HashSet::new(),
            scan_rx: Some(scan_rx),
            scanning: true,
            files_skipped: 0,
            problem: None,
            scroll_offset: 0.0,
            grid_size: Size::new(WINDOW.width, WINDOW.height - TOP_BAR_H),
            last_scan_log: Instant::now(),
            last_click: None,
            column_hold: shelf::ColumnHold::default(),
        };
        let task = Task::batch([
            text_input::focus(search_id()),
            shelf.request_visible_thumbs(),
        ]);
        Ok((shelf, task))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
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
            Message::EscapePressed => {
                if self.query.is_empty() {
                    self.reflow(Panels::close)
                } else {
                    self.query.clear();
                    self.refilter();
                    self.scroll_offset = 0.0;
                    Task::batch([
                        text_input::focus(search_id()),
                        scrollable::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
                        self.request_visible_thumbs(),
                    ])
                }
            }
            Message::Scrolled(viewport) => {
                self.scroll_offset = viewport.absolute_offset().y;
                let bounds = viewport.bounds();
                self.grid_size = Size::new(bounds.width, bounds.height);
                self.request_visible_thumbs()
            }
            Message::WindowResized(size) => {
                // Estimate until the next scroll event reports real bounds.
                self.grid_size = Size::new(
                    size.width - rail_width(&self.panels),
                    (size.height - TOP_BAR_H).max(100.0),
                );
                // Dragging the window's edge is not the gesture the hold
                // protects, and holding a stale count through a resize would
                // draw columns the window no longer has room for.
                self.column_hold.release();
                self.request_visible_thumbs()
            }
            Message::ToggleSettings => self.reflow(Panels::toggle_settings),
            Message::TogglePanels => self.reflow(Panels::toggle_hidden),
            Message::ClosePanel => self.reflow(Panels::close),
            Message::AlbumClicked(id) => {
                let now = Instant::now();
                let double = self.last_click.take().is_some_and(|(last, at)| {
                    last == id && now.duration_since(at) <= shelf::DOUBLE_CLICK
                });
                if double {
                    // Second press of a double-click. The first press already
                    // ran the selection toggle, so just make sure the album
                    // ends up on screen (re-select if the first press toggled
                    // it *off*), then hand play upward.
                    let task = if self.panels.showing_album(id) {
                        Task::none()
                    } else {
                        self.reflow_holding_columns(now, |panels| panels.select(id))
                    };
                    return Task::batch([task, Task::done(Message::PlayAlbum(id))]);
                }
                self.last_click = Some((id, now));
                self.reflow_holding_columns(now, |panels| panels.select(id))
            }
            Message::ColumnHoldTick => {
                if self.column_hold.expire(Instant::now()) {
                    // The gesture is over: let the reflow the click deferred
                    // actually land, and fetch art for whatever it revealed.
                    return self.request_visible_thumbs();
                }
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

    /// Apply a panel transition and keep the grid's width estimate in step
    /// with it.
    ///
    /// Every change to what the rail is showing goes through here, which is
    /// what makes the reflow a single fact rather than a rule repeated at each
    /// call site (it was, and one of the copies was already wrong: Escape used
    /// to widen the grid whether or not the panel it closed had been on
    /// screen). The width moves by exactly one [`PANEL_W`] and only when the
    /// rail's *occupancy* changes — swapping the album panel for the queue
    /// moves nothing, because both are that wide (see [`crate::panels`]).
    ///
    /// The estimate is adjusted rather than recomputed so that the real
    /// viewport bounds a scroll event last reported are not thrown away; the
    /// next [`Message::Scrolled`] replaces it with measured geometry either
    /// way.
    /// A hold, if one stands, is dropped here: it exists to protect one
    /// pointer gesture, and every route into this function that is not that
    /// gesture (Escape, the ✕, `Q`, `Ctrl`+`B`) is a deliberate request to see
    /// the layout change now. [`Self::reflow_holding_columns`] re-takes it
    /// afterwards for the one route that is.
    fn reflow(&mut self, transition: impl FnOnce(&mut Panels)) -> Task<Message> {
        let before = rail_width(&self.panels);
        transition(&mut self.panels);
        self.grid_size.width += before - rail_width(&self.panels);
        self.column_hold.release();
        self.request_visible_thumbs()
    }

    /// A *tile click's* reflow: the panel transition, and then the column hold
    /// that keeps the grid still for the rest of the gesture.
    ///
    /// This is the whole of the double-click repair. The audit caught the
    /// defect on camera: a double-click on the fifth tile of row 0, where the
    /// first press opened the rail, the shelf reflowed from five columns to
    /// three, the second press landed 180 px from where the tile now was, and
    /// **nothing played** — while the panel that had just opened said
    /// "double-click a tile to play" at the bottom of it. It worked for the
    /// first column and failed for the last, on arithmetic the user cannot
    /// see.
    ///
    /// The hold is taken only when the rail's *occupancy* actually changed,
    /// because that is the only case in which the grid's width moves: swapping
    /// the album panel for the queue costs no reflow (both are one
    /// [`PANEL_W`]), and a click that changed nothing has nothing to protect.
    /// Everything else about the reflow — including the width arithmetic
    /// itself — is untouched; it is deferred by up to
    /// [`shelf::DOUBLE_CLICK`], never cancelled.
    fn reflow_holding_columns(
        &mut self,
        now: Instant,
        transition: impl FnOnce(&mut Panels),
    ) -> Task<Message> {
        // Read the count the grid is *currently* laying out with, before the
        // transition (and before `reflow` drops any hold that stands).
        let held = self.columns();
        let occupied = self.panels.rail().is_some();
        let task = self.reflow(transition);
        if self.panels.rail().is_some() != occupied {
            self.column_hold.hold(held, now);
        }
        task
    }

    /// The column count the grid lays out with: what the viewport measures,
    /// unless a tile click is holding it still (see [`shelf::ColumnHold`]).
    ///
    /// One answer, read by the view that draws the rows and by the thumbnail
    /// prefetch that decides which of them to decode art for — a prefetch
    /// working from a different grid than the one on screen would request the
    /// wrong tiles for exactly the 400 ms the hold lasts.
    pub(crate) fn columns(&self) -> usize {
        self.column_hold
            .columns(shelf::columns(self.grid_size.width))
    }

    /// Recompute `visible` for the current query (shelf order preserved —
    /// see [`vm::matching_album_ids`] for the track→album mapping).
    fn refilter(&mut self) {
        self.visible = vm::visible_indices(&self.albums, &self.library, &self.query);
    }

    /// Apply every pending scan update: one `add_tracks` + one view-model
    /// rebuild per tick regardless of how many batches arrived.
    fn drain_scan(&mut self) -> Task<Message> {
        let Some(rx) = &self.scan_rx else {
            return Task::none();
        };
        let mut fresh_tracks: Vec<baz_core::library::TrackMeta> = Vec::new();
        let mut vanished: Vec<std::path::PathBuf> = Vec::new();
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(ScanUpdate::Batch { tracks, failed }) => {
                    self.files_skipped += failed;
                    fresh_tracks.extend(tracks);
                }
                Ok(ScanUpdate::Removed { paths }) => vanished.extend(paths),
                Ok(ScanUpdate::Done {
                    added,
                    updated,
                    unchanged,
                    removed,
                    failed,
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
                         {removed} removed, {failed} files skipped, {secs:.1} s ({rate:.0} tracks/s)"
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

        let mut task = Task::none();
        if !fresh_tracks.is_empty() || !vanished.is_empty() {
            if let Err(error) = self.library.add_tracks(fresh_tracks) {
                println!("[index] write failed: {error}");
                self.problem = Some(format!("library write failed: {error}"));
            }
            if !vanished.is_empty() {
                match self.library.remove_tracks(&vanished) {
                    Ok(count) => println!("[index] {count} vanished tracks removed"),
                    Err(error) => {
                        println!("[index] removal failed: {error}");
                        self.problem = Some(format!("library removal failed: {error}"));
                    }
                }
            }
            self.albums = vm::build_albums(&self.library);
            self.refilter();
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
        let cols = self.columns();
        let rows = shelf::total_rows(self.visible.len(), cols);
        let (first_row, end_row) =
            shelf::visible_rows(self.scroll_offset, self.grid_size.height, rows);
        let start = (first_row * cols).min(self.visible.len());
        let end = (end_row * cols).min(self.visible.len());
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

    /// The shelf screen: the top bar over the grid, with the rail's panel
    /// beside it when one is showing. Composition only — the surfaces
    /// themselves are [`crate::views`].
    ///
    /// One rail, one panel: the album inspector and the settings occupy the
    /// same slot, so this is a three-way choice rather than a stack of optional
    /// columns, and the shelf's width still has exactly two values.
    fn view<'a>(&'a self, player: &'a PlayerState) -> Element<'a, Message> {
        let rail: Option<Element<'_, Message>> = match self.panels.rail() {
            None => None,
            Some(Rail::Settings) => Some(views::settings_panel::view(player)),
            // A selection whose album vanished under a rescan renders no
            // panel rather than an empty one; the next scroll event squares
            // the grid estimate up.
            Some(Rail::Album) => self
                .selected_album()
                .map(|album| views::side_panel::view(self, album, player)),
        };
        let body: Element<'_, Message> = match rail {
            Some(panel) => row![
                views::shelf::view(self, player),
                vertical_rule(1).style(theme::hairline),
                panel
            ]
            .into(),
            None => views::shelf::view(self, player),
        };
        column![views::top_bar::view(self), body].into()
    }

    fn selected_album(&self) -> Option<&vm::AlbumVm> {
        let id = self.panels.selected()?;
        self.albums.iter().find(|album| album.id == id)
    }
}

/// How much width the right-hand rail is taking from the shelf.
///
/// The one place pixels meet [`crate::panels`]'s pure state machine: the
/// machine answers *whether* a panel is showing, and both panels are
/// [`PANEL_W`] wide, so this is the whole conversion.
fn rail_width(panels: &Panels) -> f32 {
    if panels.rail().is_some() {
        PANEL_W
    } else {
        0.0
    }
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

/// Persist the chosen music dir; best-effort with a log, never fatal — a
/// read-only config dir must not block listening to music.
fn persist_music_dir(music_dir: &std::path::Path) {
    if music_dir.to_str().is_none() {
        println!(
            "[config] music dir is not valid UTF-8; it cannot be written to config.toml \
             (this session is unaffected)"
        );
    }
    persist(|config| config.music_dir = Some(music_dir.to_path_buf()));
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

    /// The reflow, in the one place its arithmetic lives: the shelf loses
    /// exactly one panel width when the rail becomes occupied, gets it back
    /// when the rail empties, and neither notices nor moves when the rail
    /// swaps one panel for the other.
    #[test]
    fn the_rail_costs_the_shelf_one_panel_width_and_a_swap_costs_nothing() {
        let mut panels = Panels::new();
        assert!(rail_width(&panels).abs() < f32::EPSILON, "closed: no cost");

        panels.select(1);
        assert!((rail_width(&panels) - PANEL_W).abs() < f32::EPSILON);

        panels.toggle_settings();
        assert!(
            (rail_width(&panels) - PANEL_W).abs() < f32::EPSILON,
            "inspector -> settings is a swap, not a second panel"
        );
        panels.select(2);
        assert!((rail_width(&panels) - PANEL_W).abs() < f32::EPSILON);

        panels.toggle_hidden();
        assert!(
            rail_width(&panels).abs() < f32::EPSILON,
            "hiding gives the width back"
        );
        panels.toggle_hidden();
        assert!((rail_width(&panels) - PANEL_W).abs() < f32::EPSILON);
        panels.close();
        assert!(rail_width(&panels).abs() < f32::EPSILON);
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
    /// There is exactly one exception and it predates the redesign:
    /// <kbd>Ctrl</kbd>+<kbd>B</kbd> *hides* the right-hand column while
    /// remembering the selection, and the inspector's ✕ *closes* it — those are
    /// different messages because they are different intentions, and no control
    /// on screen sends the first. It is recorded rather than papered over.
    #[test]
    fn every_keyboard_binding_is_a_press_some_control_also_makes() {
        use iced::keyboard::{Key, Modifiers, key};

        /// Message tag → the on-screen control that sends the same message,
        /// or the reason there is none.
        const CONTROLS: [(&str, &str); 15] = [
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
            ("SeekBy", "the bottom bar's seek groove"),
            ("VolumeStep", "the bottom bar's volume fader"),
            ("ToggleMute", "the bottom bar's speaker button"),
            ("ToggleUpNext", "the bottom bar's now-playing block"),
            ("ToggleSettings", "the top bar's Settings control"),
            ("FocusSearch", "the top bar's search well"),
            ("EscapePressed", "each layer's own ✕"),
            (
                "TogglePanels",
                "no control: hiding the column is not closing it (see the doc \
                 comment)",
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
            Key::Character("b".into()),
            Key::Character(",".into()),
            Key::Character("/".into()),
            Key::Character("f".into()),
        ];
        let modifier_sets = [
            Modifiers::empty(),
            Modifiers::SHIFT,
            Modifiers::COMMAND,
            Modifiers::ALT,
            Modifiers::COMMAND | Modifiers::SHIFT,
        ];
        let mut produced: Vec<String> = Vec::new();
        for key in &keys_to_sweep {
            for modifiers in modifier_sets {
                if let Some(message) = keys::binding_for(key, modifiers, keys::Focus::Elsewhere) {
                    // The payload is not the point; the intention is.
                    let debug = format!("{message:?}");
                    let tag = debug
                        .split_once('(')
                        .map_or(debug.as_str(), |(head, _)| head)
                        .to_owned();
                    assert!(
                        CONTROLS.iter().any(|(name, _)| *name == tag),
                        "{key:?} + {modifiers:?} binds to `{tag}`, which no entry in \
                         CONTROLS accounts for — name the control that sends it, or \
                         record why there is none"
                    );
                    produced.push(tag);
                }
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

    /// The two layer keys, spelled out: `Q` is the same press as the bar's
    /// now-playing block, and Ctrl+`,` the same press as the top bar's
    /// Settings control.
    #[test]
    fn the_layer_controls_and_their_keys_are_the_same_press() {
        use iced::keyboard::{Key, Modifiers};

        let from_key = keys::binding_for(
            &Key::Character("q".into()),
            Modifiers::empty(),
            keys::Focus::Elsewhere,
        );
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(Message::ToggleUpNext))
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

        let from_key = keys::binding_for(
            &Key::Character("b".into()),
            Modifiers::COMMAND,
            keys::Focus::Elsewhere,
        );
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(Message::TogglePanels))
        );
    }

    /// **Escape peels one layer, top down**, and peels exactly one per press.
    ///
    /// The rule the redesign is actually for: with the popover floating, the
    /// key is the popover's and nothing underneath hears it; with nothing
    /// floating, it reaches the layer below. Exercised through
    /// [`Overlay`] itself, which is where the arbitration lives.
    #[test]
    fn escape_peels_the_popover_before_anything_under_it() {
        let mut overlay = Overlay::new();
        assert!(
            !overlay.close(),
            "with nothing floating the press must fall through"
        );

        overlay.toggle_up_next();
        assert!(overlay.close(), "the popover answers the press itself");
        assert!(!overlay.is_open());
        assert!(
            !overlay.close(),
            "and the next press falls through to the layer below"
        );
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
            &Key::Character("n".into()),
            Modifiers::empty(),
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

    /// The hold that repairs double-click-to-play, exercised through the state
    /// the update loop actually keeps: the column count the grid lays out with
    /// does not move while a click's gesture could still be completed by a
    /// second press, and does move once it cannot.
    ///
    /// The timing rule itself is `shelf::ColumnHold`'s and is tested there;
    /// what is pinned here is that `app.rs` spends it on the *same* answer the
    /// grid and the thumbnail prefetch both read.
    #[test]
    fn a_tile_click_holds_the_grids_columns_for_the_double_click_window() {
        let now = Instant::now();
        let mut panels = Panels::new();
        let mut hold = shelf::ColumnHold::default();

        // The shipped window with the rail closed: five columns.
        let mut width = WINDOW.width;
        let columns = |hold: shelf::ColumnHold, width: f32| hold.columns(shelf::columns(width));
        assert_eq!(columns(hold, width), 5);

        // A tile click opens the inspector. The shelf's width drops by one
        // panel — which on its own is a five-to-three reflow, and the tile the
        // pointer is over moves 180 px.
        let pinned = columns(hold, width);
        let occupied = panels.rail().is_some();
        panels.select(1);
        width -= rail_width(&panels);
        assert_eq!(shelf::columns(width), 3, "the measured grid did reflow");
        assert_ne!(panels.rail().is_some(), occupied);
        hold.hold(pinned, now);

        // …and the grid keeps laying out five, so the second press of the
        // double-click lands on the tile it was aimed at.
        assert_eq!(columns(hold, width), 5);
        assert!(hold.holding(), "the app ticks while this stands");
        assert!(!hold.expire(now + shelf::DOUBLE_CLICK / 2));
        assert_eq!(columns(hold, width), 5);

        // Once a second press could no longer be part of the gesture, the
        // reflow the click asked for lands — deferred, never cancelled.
        assert!(hold.expire(now + shelf::DOUBLE_CLICK));
        assert_eq!(columns(hold, width), 3);
        assert!(!hold.holding(), "and the tick subscription goes away");
    }

    /// A swap costs no reflow, so it takes no hold: the album inspector giving
    /// way to the settings moves nothing, and holding there would delay a
    /// layout change that never happens.
    #[test]
    fn a_panel_swap_moves_no_tile_and_so_holds_nothing() {
        let mut panels = Panels::new();
        panels.select(1);
        let before = rail_width(&panels);
        let occupied = panels.rail().is_some();
        panels.toggle_settings();
        assert!((rail_width(&panels) - before).abs() < f32::EPSILON);
        assert_eq!(
            panels.rail().is_some(),
            occupied,
            "occupancy is the condition the hold is taken on"
        );
    }
}
