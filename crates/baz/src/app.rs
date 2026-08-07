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
use iced::keyboard::{self, key};
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{column, image as iced_image, row, scrollable, text_input, vertical_rule};
use iced::{Element, Size, Subscription, Task, window};
use lru::LruCache;

use crate::playback::{Playback, PlayerEvent};
use crate::player::{Availability, PlayerState};
use crate::scan::ScanUpdate;
use crate::views::side_panel::PANEL_W;
use crate::{art, config, player, scan, shelf, theme, views, vm};

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
        .window_size(WINDOW)
        .run_with(move || App::new(started, cli_dir))
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
    /// Esc anywhere: clear the search, else close the side panel.
    EscapePressed,
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
    /// Bottom bar: play/pause toggle.
    PlayPause,
    /// Bottom bar: skip to the next queued track.
    NextTrack,
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
        let player = PlayerState::new(playback.availability());
        let stored = config::config_file().and_then(|path| config::load(&path));
        let dir = cli_dir.or(stored.map(|c| c.music_dir));
        let (screen, task) = match dir {
            None => (Screen::Setup(Setup::fresh(None)), Task::none()),
            Some(dir) => match Shelf::open(dir) {
                Ok((shelf, task)) => (Screen::Shelf(Box::new(shelf)), task),
                Err(error) => (Screen::Setup(Setup::fresh(Some(error))), Task::none()),
            },
        };
        (
            Self {
                started,
                first_frame_logged: false,
                screen,
                playback,
                player,
            },
            task,
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
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
                // Nothing is assumed about the new position: the engine
                // answers with Progress, and until it does the bar shows the
                // request as pending (see player.rs).
                if let Some(position_ms) = self.player.release_drag()
                    && !self.playback.send(Command::Seek { position_ms })
                {
                    self.player.engine_closed();
                }
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
            }
            PlayerEvent::Closed => {
                println!("[playback] engine shut down");
                self.player.engine_closed();
            }
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
        let paths = vm::album_queue(album, state.edition_choice.get(&id).copied());
        if paths.is_empty() {
            return;
        }
        let queued = paths.len();
        if self.playback.send(Command::SetQueue { paths }) && self.playback.send(Command::Play) {
            self.player.note_queue_sent(queued);
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
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
            keyboard::on_key_press(|k, _mods| match k {
                keyboard::Key::Named(key::Named::Escape) => Some(Message::EscapePressed),
                _ => None,
            }),
            window::resize_events().map(|(_, size)| Message::WindowResized(size)),
            self.playback.subscription().map(Message::Playback),
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
    /// The album whose side panel is open, if any.
    pub(crate) selected: Option<u64>,
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
        let scan_rx = scan::spawn(music_dir);

        let mut shelf = Self {
            library,
            visible: (0..albums.len()).collect(),
            albums,
            query: String::new(),
            selected: None,
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
                    if self.selected.take().is_some() {
                        self.grid_size.width += PANEL_W;
                    }
                    Task::none()
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
                let panel = if self.selected.is_some() {
                    PANEL_W
                } else {
                    0.0
                };
                self.grid_size =
                    Size::new(size.width - panel, (size.height - TOP_BAR_H).max(100.0));
                self.request_visible_thumbs()
            }
            Message::AlbumClicked(id) => {
                let now = Instant::now();
                let double = self
                    .last_click
                    .take()
                    .is_some_and(|(last, at)| last == id && now.duration_since(at) <= DOUBLE_CLICK);
                if double {
                    // Second press of a double-click. The first press
                    // already ran the selection toggle, so just make sure
                    // the album ends up selected (re-select if the first
                    // press toggled it *off*), then hand play upward.
                    if self.selected != Some(id) {
                        if self.selected.is_none() {
                            self.grid_size.width -= PANEL_W;
                        }
                        self.selected = Some(id);
                    }
                    return Task::batch([
                        self.request_visible_thumbs(),
                        Task::done(Message::PlayAlbum(id)),
                    ]);
                }
                self.last_click = Some((id, now));
                if self.selected == Some(id) {
                    self.selected = None;
                    self.grid_size.width += PANEL_W;
                } else {
                    if self.selected.is_none() {
                        self.grid_size.width -= PANEL_W;
                    }
                    self.selected = Some(id);
                }
                self.request_visible_thumbs()
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
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(ScanUpdate::Batch { tracks, failed }) => {
                    self.files_skipped += failed;
                    fresh_tracks.extend(tracks);
                }
                Ok(ScanUpdate::Done {
                    tracks,
                    failed,
                    elapsed,
                }) => {
                    let secs = elapsed.as_secs_f64();
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "track counts are far below f64's exact-integer range"
                    )]
                    let rate = if secs > 0.0 {
                        tracks as f64 / secs
                    } else {
                        0.0
                    };
                    println!(
                        "[scan] done: {tracks} tracks read, {failed} files skipped, {secs:.1} s ({rate:.0} tracks/s)"
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
        if !fresh_tracks.is_empty() {
            if let Err(error) = self.library.add_tracks(fresh_tracks) {
                println!("[index] write failed: {error}");
                self.problem = Some(format!("library write failed: {error}"));
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

    /// The shelf screen: the top bar over the grid, with the side panel
    /// beside it when an album is selected. Composition only — the surfaces
    /// themselves are [`crate::views`].
    fn view<'a>(&'a self, player: &'a PlayerState) -> Element<'a, Message> {
        let body: Element<'_, Message> = match self.selected_album() {
            Some(album) => row![
                views::shelf::view(self, player),
                vertical_rule(1).style(theme::hairline),
                views::side_panel::view(self, album, player)
            ]
            .into(),
            None => views::shelf::view(self, player),
        };
        column![views::top_bar::view(self), body].into()
    }

    fn selected_album(&self) -> Option<&vm::AlbumVm> {
        let id = self.selected?;
        self.albums.iter().find(|album| album.id == id)
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
}
