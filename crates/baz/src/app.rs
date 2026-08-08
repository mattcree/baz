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
use baz_core::protocol::{Command, Event, SignalChain};
use iced::keyboard;
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{column, image as iced_image, row, scrollable, text_input, vertical_rule};
use iced::{Element, Size, Subscription, Task, window};
use lru::LruCache;

use crate::mpris::Mpris;
use crate::panels::{Panels, Rail};
use crate::playback::{Playback, PlayerEvent};
use crate::player::{Availability, PlayerState};
use crate::scan::ScanUpdate;
use crate::theme::PANEL_W;
use crate::{art, config, keys, mpris, player, scan, shelf, theme, views, vm};

/// Approximate top-bar height, used only for the pre-first-scroll estimate
/// of the grid viewport (real bounds arrive with every scroll event).
const TOP_BAR_H: f32 = 56.0;
/// Initial window size.
const WINDOW: Size = Size::new(1280.0, 860.0);
/// Two clicks on the same tile within this window play the album.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

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
pub fn run(started: Instant, cli_dir: Option<PathBuf>) -> iced::Result {
    iced::application("baz", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_| theme::theme())
        .window(window_settings())
        .run_with(move || App::new(started, cli_dir))
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
    /// Esc anywhere: clear the search, else close what the rail is showing.
    EscapePressed,
    /// Top bar's Queue toggle, or `Q`: show the play queue, or put back
    /// whatever it was covering (see [`crate::panels`]).
    ToggleQueue,
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
    /// click within [`DOUBLE_CLICK`] plays the album).
    AlbumClicked(u64),
    /// Queue the album's tracks and play (side-panel Play, tile
    /// double-click).
    PlayAlbum(u64),
    /// Side panel: a different format of the selected album was picked.
    EditionSelected(u64, vm::EditionKey),
    /// Bottom bar, Space, or MPRIS `PlayPause`: play/pause toggle.
    PlayPause,
    /// Bottom bar, `N`, or MPRIS `Next`: skip to the next queued track.
    NextTrack,
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
        // Desktop integration is an enhancement: this spawns a thread and
        // returns, and an absent session bus costs one stdout line (see
        // crate::mpris).
        let mpris = Mpris::start();
        let stored = config::config_file().and_then(|path| config::load(&path));
        let dir = cli_dir.or(stored.map(|c| c.music_dir));
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
            playback,
            player,
            mpris,
            mpris_art: (0, None),
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
        if self.update_volume(&message) {
            return Task::none();
        }
        match message {
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
            Message::PlayPause => {
                // The same reading the glyph is drawn from, so a press asks
                // for exactly what the button was showing (Play also resumes
                // a paused engine, so a stale read is still safe).
                let command = match self.player.play_pause() {
                    player::PlayPause::Pause => Command::Pause,
                    player::PlayPause::Play => Command::Play,
                };
                self.send_transport(command);
                Task::none()
            }
            Message::NextTrack => {
                self.send_transport(Command::Next);
                Task::none()
            }
            // Play/Pause/Stop have no button of their own; a desktop media
            // widget (and a media key) asks for a direction rather than a
            // toggle, and they take the same path to the engine as the
            // toggle does.
            Message::Play => {
                self.send_transport(Command::Play);
                Task::none()
            }
            Message::Pause => {
                self.send_transport(Command::Pause);
                Task::none()
            }
            Message::Stop => {
                self.send_transport(Command::Stop);
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
            }
            PlayerEvent::Closed => {
                println!("[playback] engine shut down");
                self.player.engine_closed();
            }
        }
        self.publish_mpris(seek_confirmed);
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

    /// Send a transport command, marking it pending on acceptance and
    /// downgrading to engine-closed state when the channel is gone.
    fn send_transport(&mut self, command: Command) {
        if self.playback.send(command) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
    }

    /// The whole window: the current screen, with the persistent bottom bar
    /// under it. Composition only — every surface is drawn by
    /// [`crate::views`].
    fn view(&self) -> Element<'_, Message> {
        let screen: Element<'_, Message> = match &self.screen {
            Screen::Setup(setup) => return views::setup::view(setup),
            Screen::Shelf(state) => state.view(&self.player),
        };
        // The persistent bottom bar lives under the shelf — unless this
        // build has no audio output at all, in which case playback UI is
        // hidden entirely.
        if *self.player.availability() == Availability::NotBuilt {
            return screen;
        }
        column![screen, views::bottom_bar::view(&self.player)].into()
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
        if let Screen::Shelf(state) = &self.screen
            && state.scanning
        {
            subs.push(iced::time::every(Duration::from_millis(100)).map(|_| Message::ScanTick));
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
                self.request_visible_thumbs()
            }
            Message::ToggleQueue => self.reflow(Panels::toggle_queue),
            Message::TogglePanels => self.reflow(Panels::toggle_hidden),
            Message::ClosePanel => self.reflow(Panels::close),
            Message::AlbumClicked(id) => {
                let now = Instant::now();
                let double = self
                    .last_click
                    .take()
                    .is_some_and(|(last, at)| last == id && now.duration_since(at) <= DOUBLE_CLICK);
                if double {
                    // Second press of a double-click. The first press already
                    // ran the selection toggle, so just make sure the album
                    // ends up on screen (re-select if the first press toggled
                    // it *off*), then hand play upward.
                    let task = if self.panels.showing_album(id) {
                        Task::none()
                    } else {
                        self.reflow(|panels| panels.select(id))
                    };
                    return Task::batch([task, Task::done(Message::PlayAlbum(id))]);
                }
                self.last_click = Some((id, now));
                self.reflow(|panels| panels.select(id))
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
    fn reflow(&mut self, transition: impl FnOnce(&mut Panels)) -> Task<Message> {
        let before = rail_width(&self.panels);
        transition(&mut self.panels);
        self.grid_size.width += before - rail_width(&self.panels);
        self.request_visible_thumbs()
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
        let cols = shelf::columns(self.grid_size.width);
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
    /// One rail, one panel: the album panel and the queue occupy the same
    /// slot, so this is a three-way choice rather than a stack of optional
    /// columns, and the shelf's width has exactly two values.
    fn view<'a>(&'a self, player: &'a PlayerState) -> Element<'a, Message> {
        let rail: Option<Element<'_, Message>> = match self.panels.rail() {
            None => None,
            Some(Rail::Queue) => Some(views::queue_panel::view(player)),
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
        column![views::top_bar::view(self, player), body].into()
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
        mpris::Request::SeekBy(delta_ms) => Message::SeekBy(delta_ms),
        mpris::Request::SeekTo(position_ms) => Message::SeekTo(position_ms),
        mpris::Request::SetVolume(position) => Message::SetVolume(position),
        mpris::Request::SetMute(muted) => Message::SetMute(muted),
        mpris::Request::Raise => Message::Raise,
        mpris::Request::Quit => Message::Quit,
    }
}

/// Persist the chosen music dir (config module); best-effort with a log,
/// never fatal — a read-only config dir must not block listening to music.
fn persist_music_dir(music_dir: &std::path::Path) {
    let Some(path) = config::config_file() else {
        println!("[config] no config directory on this system; not persisting music dir");
        return;
    };
    let config = config::Config {
        music_dir: music_dir.to_path_buf(),
    };
    if config::load(&path).as_ref() == Some(&config) {
        return; // Unchanged.
    }
    match config::store(&path, &config) {
        Ok(()) => println!("[config] music dir saved to {}", path.display()),
        Err(error) => println!("[config] could not save {}: {error}", path.display()),
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

        panels.toggle_queue();
        assert!(
            (rail_width(&panels) - PANEL_W).abs() < f32::EPSILON,
            "album -> queue is a swap, not a second panel"
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

    /// Every panel affordance the interface offers resolves to the same
    /// message the keyboard sends — the transport's "one path per intention"
    /// rule, applied to the rail.
    #[test]
    fn the_panel_controls_and_their_keys_are_the_same_press() {
        use iced::keyboard::{Key, Modifiers};

        let from_key = keys::binding_for(
            &Key::Character("q".into()),
            Modifiers::empty(),
            keys::Focus::Elsewhere,
        );
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(Message::ToggleQueue))
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
    }
}
