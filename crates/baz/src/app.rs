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

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use baz_core::index::Library;
use baz_core::protocol::{Command, Event};
use iced::keyboard::{self, key};
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{
    Column, Space, button, column, container, horizontal_rule, image as iced_image, row,
    scrollable, slider, text, text_input, vertical_rule,
};
use iced::{Color, Element, Length, Size, Subscription, Task, alignment, window};
use lru::LruCache;

use crate::playback::{Playback, PlayerEvent};
use crate::player::{Availability, Phase, PlayerState};
use crate::scan::ScanUpdate;
use crate::shelf::{ART_PX, CELL_H, CELL_W, GRID_PADDING};
use crate::{art, config, scan, shelf, theme, vm};

/// Side-panel width (logical px).
const PANEL_W: f32 = 340.0;
/// Side-panel inner padding (logical px).
const PANEL_PAD: f32 = theme::GAP_XL;
/// Approximate top-bar height, used only for the pre-first-scroll estimate
/// of the grid viewport (real bounds arrive with every scroll event).
const TOP_BAR_H: f32 = 56.0;
/// Initial window size.
const WINDOW: Size = Size::new(1280.0, 860.0);
/// Horizontal tile padding: centers [`ART_PX`] artwork inside [`CELL_W`].
const TILE_PAD_H: f32 = (CELL_W - ART_PX) / 2.0;
/// Vertical tile padding.
const TILE_PAD_V: f32 = theme::GAP_MD;
/// The search field's width in the top bar (logical px).
const SEARCH_W: f32 = 360.0;
/// The first-run screen's folder input width (logical px).
const SETUP_INPUT_W: f32 = 460.0;
/// Width of the track-number column in the side panel (logical px).
const TRACK_NO_W: f32 = 24.0;
/// Two clicks on the same tile within this window play the album.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

fn scroll_id() -> scrollable::Id {
    scrollable::Id::new("baz-shelf")
}

fn search_id() -> text_input::Id {
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
#[derive(Debug, Clone)]
enum Message {
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
    /// Bottom bar: the seek handle moved to this fraction of the track.
    /// Fires on press and on every drag step (iced's slider has no separate
    /// press event), which is exactly when the bar should follow the
    /// pointer.
    SeekDragged(f32),
    /// Bottom bar: the seek handle was released — the moment the request
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
struct Setup {
    input: String,
    error: Option<String>,
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
                // Choose the action from the *confirmed* phase (Play also
                // resumes a paused engine, so a stale read is still safe).
                let command = match self.player.phase() {
                    Phase::Playing => Command::Pause,
                    Phase::Paused | Phase::Stopped => Command::Play,
                };
                self.send_transport(command);
                Task::none()
            }
            Message::NextTrack => {
                self.send_transport(Command::Next);
                Task::none()
            }
            Message::SeekDragged(fraction) => {
                self.player.drag_to(fraction);
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

    fn view(&self) -> Element<'_, Message> {
        let screen: Element<'_, Message> = match &self.screen {
            Screen::Setup(setup) => return setup.view(),
            Screen::Shelf(state) => state.view(&self.player),
        };
        // The persistent bottom bar lives under the shelf — unless this
        // build has no audio output at all, in which case playback UI is
        // hidden entirely.
        if *self.player.availability() == Availability::NotBuilt {
            return screen;
        }
        column![screen, bottom_bar(&self.player)].into()
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

    fn view(&self) -> Element<'_, Message> {
        let heading = column![
            text("baz")
                .size(theme::SIZE_EMPHASIS)
                .font(theme::MONO)
                .color(theme::LAMP),
            text("Where's your music?")
                .size(theme::SIZE_HERO)
                .font(theme::SEMIBOLD),
            text("Point baz at a folder — the shelf fills as it scans.")
                .size(theme::SIZE_EMPHASIS)
                .color(theme::PAPER_DIM),
        ]
        .spacing(theme::GAP_SM)
        .align_x(iced::Alignment::Center);
        let mut content = column![
            heading,
            text_input("/path/to/your/music", &self.input)
                .on_input(Message::SetupInput)
                .on_submit(Message::SetupSubmit)
                .padding(theme::pad(theme::GAP_SM + 2.0, theme::GAP_MD))
                .size(theme::SIZE_EMPHASIS)
                .width(Length::Fixed(SETUP_INPUT_W))
                .style(theme::input),
        ]
        .spacing(theme::GAP_XL)
        .align_x(iced::Alignment::Center);
        if let Some(error) = &self.error {
            content = content.push(
                text(error.as_str())
                    .size(theme::SIZE_META)
                    .color(theme::ALERT),
            );
        }
        content = content.push(
            text("Enter confirms · next time, `baz` remembers (or run `baz DIR`)")
                .size(theme::SIZE_CAPTION)
                .color(theme::PAPER_FAINT),
        );
        container(content).center(Length::Fill).into()
    }
}

/// The shelf screen: library, scan state, and grid/panel view state.
struct Shelf {
    library: Library,
    /// Owned view model of every album, in `Library::albums` order.
    albums: Vec<vm::AlbumVm>,
    /// Indices into `albums` that survive the current query.
    visible: Vec<usize>,
    query: String,
    selected: Option<u64>,
    /// Which format of an album the user picked, for albums where they
    /// picked one. Absent = the ranked-best edition (see
    /// [`vm::selected_edition`]).
    ///
    /// Session-scoped by choice: the persistent config is a hand-rolled
    /// single-key TOML file (see `config.rs`), so persisting a per-album map
    /// would mean adopting a real TOML parser for a preference whose proper
    /// home is a column in the library database anyway. Deferred in
    /// ADR-0007 rather than bolted on here.
    edition_choice: HashMap<u64, vm::EditionKey>,
    /// Decoded-thumbnail LRU; capacity/budget documented in [`art`].
    thumbs: LruCache<u64, iced_image::Handle>,
    /// Albums with a decode in flight (dedupes requests while scrolling).
    pending: HashSet<u64>,
    /// Albums known to have no (decodable) art — render the gradient and
    /// stop asking. Cleared once when the scan finishes, since late tracks
    /// or cover files may have arrived for early albums.
    no_art: HashSet<u64>,
    scan_rx: Option<Receiver<ScanUpdate>>,
    scanning: bool,
    files_skipped: usize,
    /// A fatal-ish problem worth a status-line mention (scan could not
    /// start, or a library write failed). Never a modal.
    problem: Option<String>,
    scroll_offset: f32,
    grid_size: Size,
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

    fn view<'a>(&'a self, player: &'a PlayerState) -> Element<'a, Message> {
        let body: Element<'_, Message> = match self.selected_album() {
            Some(album) => row![
                self.grid(player),
                vertical_rule(1).style(theme::hairline),
                self.side_panel(album, player)
            ]
            .into(),
            None => self.grid(player),
        };
        column![self.top_bar(), body].into()
    }

    fn selected_album(&self) -> Option<&vm::AlbumVm> {
        let id = self.selected?;
        self.albums.iter().find(|album| album.id == id)
    }

    /// The slim top bar: the search well on the left, quiet status on the
    /// right, a hairline rule below.
    fn top_bar(&self) -> Element<'_, Message> {
        let search = text_input("Search artists, albums, tracks…", &self.query)
            .id(search_id())
            .on_input(Message::SearchChanged)
            .padding(theme::pad(theme::GAP_SM, theme::GAP_MD))
            .size(theme::SIZE_BODY)
            .width(Length::Fixed(SEARCH_W))
            .style(theme::input);
        let mut status = row![
            text(self.counts_line())
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER_FAINT)
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center);
        if self.scanning {
            status = status.push(
                text("scanning…")
                    .size(theme::SIZE_META)
                    .font(theme::MONO)
                    .color(theme::LAMP),
            );
        }
        if self.files_skipped > 0 {
            status = status.push(
                text(format!("{} files skipped", self.files_skipped))
                    .size(theme::SIZE_META)
                    .font(theme::MONO)
                    .color(theme::PAPER_FAINT),
            );
        }
        if let Some(problem) = &self.problem {
            status = status.push(
                text(problem.as_str())
                    .size(theme::SIZE_META)
                    .color(theme::ALERT),
            );
        }
        column![
            container(
                row![search, Space::with_width(Length::Fill), status]
                    .spacing(theme::GAP_LG)
                    .align_y(iced::Alignment::Center),
            )
            .padding(theme::pad(theme::GAP_SM + 2.0, theme::GAP_LG)),
            horizontal_rule(1).style(theme::hairline),
        ]
        .into()
    }

    /// The unobtrusive count text: album/track counts, or the filtered
    /// count while a query narrows the shelf. Status, not modal — by
    /// design; scan/skip/problem notes render as separate colored segments.
    fn counts_line(&self) -> String {
        if self.query.trim().is_empty() {
            format!(
                "{} albums · {} tracks",
                self.albums.len(),
                self.library.len()
            )
        } else {
            format!("{} / {} albums", self.visible.len(), self.albums.len())
        }
    }

    /// The virtualized grid: spacer, visible rows, spacer (see [`shelf`]).
    /// The grid block is centered in the viewport; spacers are
    /// width-shrunk so the column keeps the rows' width and partial last
    /// rows stay left-aligned within the shelf.
    fn grid<'a>(&'a self, player: &'a PlayerState) -> Element<'a, Message> {
        if self.visible.is_empty() {
            return self.empty_state();
        }
        let cols = shelf::columns(self.grid_size.width);
        let total_rows = shelf::total_rows(self.visible.len(), cols);
        let (first_row, end_row) =
            shelf::visible_rows(self.scroll_offset, self.grid_size.height, total_rows);

        let mut grid = column![].padding(GRID_PADDING);
        grid = grid.push(Space::with_height(Length::Fixed(shelf::spacer_height(
            first_row,
        ))));
        for r in first_row..end_row {
            let mut cells = row![];
            for c in 0..cols {
                let Some(&album_index) = self.visible.get(r * cols + c) else {
                    break;
                };
                if let Some(album) = self.albums.get(album_index) {
                    cells = cells.push(self.tile(album, player.playing_album() == Some(album.id)));
                }
            }
            grid = grid.push(container(cells).height(Length::Fixed(CELL_H)));
        }
        grid = grid.push(Space::with_height(Length::Fixed(shelf::spacer_height(
            total_rows - end_row,
        ))));

        scrollable(
            container(grid)
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
        )
        .id(scroll_id())
        .on_scroll(Message::Scrolled)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// The shelf with nothing to show: a zero-result search, the first
    /// moments of a scan, or a genuinely empty folder. Quiet text, no modal.
    fn empty_state(&self) -> Element<'_, Message> {
        let query = self.query.trim();
        let (line, hint) = if query.is_empty() {
            if self.scanning {
                (
                    "The shelf fills as the scan finds your music…".to_owned(),
                    None,
                )
            } else {
                (
                    "No albums here yet".to_owned(),
                    Some("baz rescans this folder each time it starts"),
                )
            }
        } else {
            (
                format!("Nothing matches “{query}”"),
                Some("Esc clears the search"),
            )
        };
        let mut content = column![
            text(line)
                .size(theme::SIZE_EMPHASIS)
                .color(theme::PAPER_DIM)
        ]
        .spacing(theme::GAP_SM)
        .align_x(iced::Alignment::Center);
        if let Some(hint) = hint {
            content = content.push(text(hint).size(theme::SIZE_META).color(theme::PAPER_FAINT));
        }
        container(content).center(Length::Fill).into()
    }

    /// One album tile: the sleeve (thumbnail or gradient placeholder, with
    /// a soft shelf shadow) over a quiet two-line caption. The playing
    /// album swaps the shadow for a lamp-amber halo and gains a lamp dot by
    /// its title; selection and hover raise the tile's card.
    fn tile<'a>(&'a self, album: &'a vm::AlbumVm, playing: bool) -> Element<'a, Message> {
        let art: Element<'_, Message> = match self.thumbs.peek(&album.id) {
            Some(handle) => iced_image(handle.clone())
                .width(Length::Fixed(ART_PX))
                .height(Length::Fixed(ART_PX))
                .into(),
            None => gradient_block(album.id, ART_PX),
        };
        let sleeve = container(art).style(move |_theme| theme::sleeve(playing));
        let title = album.title.as_deref().unwrap_or("Unknown Album");
        // The *album* artist: one tile per album, captioned by whoever the
        // album is filed under, not by whichever composer happened to be
        // first (see `vm::AlbumArtistVm`).
        let artist = album.artist.label();
        let caption = match album.year {
            Some(year) => format!("{artist} · {year}"),
            None => artist.to_owned(),
        };
        let mut title_row = row![]
            .spacing(theme::GAP_XS)
            .align_y(iced::Alignment::Center);
        if playing {
            title_row = title_row.push(lamp_dot());
        }
        title_row = title_row.push(
            text(title)
                .size(theme::SIZE_BODY)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        );
        let selected = self.selected == Some(album.id);
        button(
            column![
                sleeve,
                column![
                    title_row,
                    text(caption)
                        .size(theme::SIZE_META)
                        .color(theme::PAPER_DIM)
                        .wrapping(text::Wrapping::None),
                ]
                .spacing(theme::GAP_XXS),
            ]
            .spacing(theme::GAP_SM)
            .width(Length::Fixed(ART_PX)),
        )
        .width(Length::Fixed(CELL_W))
        .height(Length::Fixed(CELL_H))
        .padding(theme::pad(TILE_PAD_V, TILE_PAD_H))
        .style(move |_theme, status| theme::tile(status, selected))
        .on_press(Message::AlbumClicked(album.id))
        .into()
    }

    /// The album side panel: large art, a title/artist/meta header, the
    /// edition selector when the album is owned in more than one format, the
    /// primary Play action, and the selected edition's numbered track list
    /// (durations in monospace, right-hugged). In a build without audio
    /// output the button is hidden; with an unusable or closed engine it
    /// renders disabled.
    fn side_panel<'a>(
        &'a self,
        album: &'a vm::AlbumVm,
        player: &'a PlayerState,
    ) -> Element<'a, Message> {
        let playing = player.playing_album() == Some(album.id);
        let art_edge = PANEL_W - 2.0 * PANEL_PAD;
        let art: Element<'_, Message> = match self.thumbs.peek(&album.id) {
            Some(handle) => iced_image(handle.clone())
                .width(Length::Fixed(art_edge))
                .into(),
            None => gradient_block(album.id, art_edge),
        };
        let sleeve = container(art).style(move |_theme| theme::sleeve(playing));
        let chosen = self.edition_choice.get(&album.id).copied();
        let edition = vm::selected_edition(album, chosen);
        // A soundtrack grouped under one album artist keeps its per-cue
        // composer credits; an ordinary album gains no extra line.
        let per_track_artists = album.track_artists_vary;
        let rows: Vec<Element<'_, Message>> = edition
            .map(|edition| {
                edition
                    .tracks
                    .iter()
                    .map(|track| track_row(track, per_track_artists))
                    .collect()
            })
            .unwrap_or_default();

        let mut content = column![sleeve, album_header(album, edition)].spacing(theme::GAP_MD);
        // Only a genuinely multi-format album gets a control; a single-format
        // album must look exactly as it always did.
        if album.editions.len() > 1 {
            content = content.push(edition_selector(album, edition));
        }
        if *player.availability() != Availability::NotBuilt {
            content = content.push(
                button(
                    container(
                        text("Play album")
                            .size(theme::SIZE_BODY)
                            .font(theme::MEDIUM),
                    )
                    .width(Length::Fill)
                    .align_x(alignment::Horizontal::Center),
                )
                .width(Length::Fill)
                .padding(theme::pad(theme::GAP_SM, 0.0))
                .style(theme::primary)
                .on_press_maybe(
                    player
                        .engine_ready()
                        .then_some(Message::PlayAlbum(album.id)),
                ),
            );
        }
        let hint = if *player.availability() == Availability::NotBuilt {
            "Esc closes · built without audio output"
        } else {
            "Esc closes · double-click a tile to play"
        };
        content = content
            .push(
                scrollable(Column::with_children(rows).spacing(theme::GAP_XXS))
                    .height(Length::Fill),
            )
            .push(
                text(hint)
                    .size(theme::SIZE_CAPTION)
                    .color(theme::PAPER_FAINT),
            );

        container(content)
            .width(Length::Fixed(PANEL_W))
            .height(Length::Fill)
            .padding(PANEL_PAD)
            .style(theme::panel)
            .into()
    }
}

/// The side panel's header: album title over artist over a quiet
/// year · tracks · total-time meta line, and — when the scan read one — the
/// selected edition's encoding fingerprint under it.
///
/// The counts describe `edition`, not the album: with two rips on disk, "24
/// tracks" would be a number nothing on screen adds up to.
fn album_header<'a>(
    album: &'a vm::AlbumVm,
    edition: Option<&'a vm::EditionVm>,
) -> Element<'a, Message> {
    let title = album.title.as_deref().unwrap_or("Unknown Album");
    let artist = album.artist.label();
    let tracks = edition.map_or(0, |edition| edition.tracks.len());
    let mut meta: Vec<String> = Vec::new();
    if let Some(year) = album.year {
        meta.push(year.to_string());
    }
    meta.push(match tracks {
        1 => "1 track".to_owned(),
        n => format!("{n} tracks"),
    });
    let total: Duration = edition
        .into_iter()
        .flat_map(|edition| edition.tracks.iter())
        .filter_map(|t| t.duration)
        .sum();
    if total > Duration::ZERO {
        meta.push(vm::format_duration(total));
    }
    let mut header = column![
        text(title).size(theme::SIZE_TITLE).font(theme::SEMIBOLD),
        text(artist)
            .size(theme::SIZE_EMPHASIS)
            .color(theme::PAPER_DIM),
        text(meta.join(" · "))
            .size(theme::SIZE_META)
            .font(theme::MONO)
            .color(theme::PAPER_FAINT),
    ]
    .spacing(theme::GAP_XS);
    if let Some(line) = edition.and_then(vm::EditionVm::encoding_line) {
        header = header.push(
            text(line)
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER_FAINT),
        );
    }
    header.into()
}

/// The edition selector: a quiet segmented control, one segment per format
/// the album is owned in, in the library's best-first order.
///
/// Shown only when there is a choice to make — a single-format album carries
/// no control at all, so the ordinary case gains no chrome. The choice
/// changes what the panel lists and what Play queues, and nothing else; it
/// never interrupts what is already playing.
fn edition_selector<'a>(
    album: &'a vm::AlbumVm,
    selected: Option<&'a vm::EditionVm>,
) -> Element<'a, Message> {
    let selected_key = selected.map(|edition| edition.key);
    let mut segments = row![].spacing(theme::GAP_XXS);
    for edition in &album.editions {
        let is_selected = selected_key == Some(edition.key);
        segments = segments.push(
            button(
                container(
                    text(edition.key.label())
                        .size(theme::SIZE_META)
                        .font(theme::MEDIUM)
                        .wrapping(text::Wrapping::None),
                )
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
            .style(move |_theme, status| theme::segment(status, is_selected))
            .on_press(Message::EditionSelected(album.id, edition.key)),
        );
    }
    container(segments)
        .width(Length::Fill)
        .padding(theme::SEGMENT_INSET)
        .style(theme::segmented)
        .into()
}

/// One track-list row: right-aligned number, title, monospace duration.
/// Rows are not interactive in v0.1, so they carry no hover affordance —
/// no false signals.
///
/// With `show_artist`, the track's own artist sits under its title in the
/// quiet meta style — the same title-over-artist stack the now-playing bar
/// uses. It is passed in rather than decided here because the answer is a
/// property of the whole album ([`vm::AlbumVm::track_artists_vary`]): every
/// row of a soundtrack shows its composer, or none does.
fn track_row(track: &vm::TrackVm, show_artist: bool) -> Element<'_, Message> {
    let number = track.number.map(|n| n.to_string()).unwrap_or_default();
    let duration = track.duration.map(vm::format_duration).unwrap_or_default();
    let mut title = column![
        text(track.title.as_str())
            .size(theme::SIZE_BODY)
            .wrapping(text::Wrapping::None)
    ]
    .spacing(theme::GAP_XXS);
    if let Some(artist) = track.artist.as_deref().filter(|_| show_artist) {
        title = title.push(
            text(artist)
                .size(theme::SIZE_META)
                .color(theme::PAPER_DIM)
                .wrapping(text::Wrapping::None),
        );
    }
    container(
        row![
            container(
                text(number)
                    .size(theme::SIZE_META)
                    .font(theme::MONO)
                    .color(theme::PAPER_FAINT)
            )
            .width(Length::Fixed(TRACK_NO_W))
            .align_x(alignment::Horizontal::Right),
            container(title).width(Length::Fill),
            text(duration)
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER_FAINT),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .padding(theme::pad(theme::GAP_XS, theme::GAP_XS))
    .into()
}

/// The persistent now-playing bar: transport controls on the left, the
/// current track as a title-over-artist stack (or the engine's
/// plainly-stated absence as quiet status text), the seek bar and its
/// timestamps in the middle, skip notes on the right. Every label,
/// position, and enabled-state comes from [`PlayerState`] — event-derived,
/// tested in `player.rs`.
fn bottom_bar(player: &PlayerState) -> Element<'_, Message> {
    let toggle = button(
        container(
            text(player.play_pause_label())
                .size(theme::SIZE_BODY)
                .font(theme::MEDIUM),
        )
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center),
    )
    .width(Length::Fixed(84.0))
    .padding(theme::pad(theme::GAP_SM, 0.0))
    .style(theme::transport)
    .on_press_maybe(player.play_pause_enabled().then_some(Message::PlayPause));
    let next = button(
        container(text("Next").size(theme::SIZE_BODY).font(theme::MEDIUM))
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center),
    )
    .width(Length::Fixed(64.0))
    .padding(theme::pad(theme::GAP_SM, 0.0))
    .style(theme::transport)
    .on_press_maybe(player.next_enabled().then_some(Message::NextTrack));

    let line: Element<'_, Message> = if let Some(note) = player.availability_note() {
        text(note)
            .size(theme::SIZE_META)
            .color(theme::PAPER_FAINT)
            .into()
    } else if let Some(now) = player.now_playing() {
        let mut stack = column![
            text(now.title.as_str())
                .size(theme::SIZE_BODY)
                .font(theme::MEDIUM)
        ]
        .spacing(theme::GAP_XXS);
        if let Some(artist) = &now.artist {
            stack = stack.push(
                text(artist.as_str())
                    .size(theme::SIZE_META)
                    .color(theme::PAPER_DIM),
            );
        }
        stack.into()
    } else {
        text("Nothing playing")
            .size(theme::SIZE_META)
            .color(theme::PAPER_FAINT)
            .into()
    };

    let mut bar = row![toggle, next, line, Space::with_width(Length::Fill)]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::Center);
    if let Some(seek) = player.seek_bar() {
        bar = bar.push(seek_bar(seek));
        bar = bar.push(Space::with_width(Length::Fill));
    }
    if let Some(skipped) = player.skipped_note() {
        bar = bar.push(
            text(skipped)
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER_FAINT),
        );
    }
    column![
        horizontal_rule(1).style(theme::hairline),
        container(bar)
            .width(Length::Fill)
            .padding(theme::pad(theme::GAP_MD, theme::GAP_LG))
            .style(theme::bar),
    ]
    .into()
}

/// The seek bar: elapsed timestamp, groove, total timestamp — a row that
/// reads left to right the way the track plays. Timestamps are monospace so
/// the digits do not shuffle the groove sideways as they tick.
///
/// A track whose length was never declared gets the inert groove: the
/// elapsed time still counts up (that much is known), but there is nothing
/// to scrub against and the widget says so by refusing the drag rather than
/// by looking identical and doing nothing.
fn seek_bar(state: crate::player::SeekBar) -> Element<'static, Message> {
    let stamp = |value: String, color| {
        text(value)
            .size(theme::SIZE_META)
            .font(theme::MONO)
            .color(color)
    };
    // While a position is being asked for rather than reported, the elapsed
    // timestamp warms to lamp amber — the same accent the rest of the room
    // reserves for playback truth, here saying "this is where you are asking
    // to be". It cools back to the quiet default the moment the engine
    // confirms.
    let elapsed_color = if state.pending {
        theme::LAMP
    } else {
        theme::PAPER_FAINT
    };
    let groove = slider(0.0..=1.0, state.position, Message::SeekDragged)
        .step(0.001)
        .height(theme::RAIL_HIT)
        .width(Length::Fixed(theme::SEEK_W));
    let groove = if state.interactive {
        groove.on_release(Message::SeekReleased).style(theme::seek)
    } else {
        groove.style(theme::seek_inert)
    };
    row![
        stamp(state.elapsed, elapsed_color),
        groove,
        stamp(state.total, theme::PAPER_FAINT),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .into()
}

/// The playing album's lamp dot: a small amber circle, the amplifier's
/// power light.
fn lamp_dot() -> Element<'static, Message> {
    container(Space::new(
        Length::Fixed(theme::DOT),
        Length::Fixed(theme::DOT),
    ))
    .style(theme::lamp_dot)
    .into()
}

/// A `size`×`size` block filled with the album's deterministic two-color
/// gradient (hash → HSL, see [`vm::gradient_colors`]) — a stand-in sleeve,
/// square-cornered like the artwork it substitutes.
fn gradient_block(album_id: u64, size: f32) -> Element<'static, Message> {
    let (c1, c2) = vm::gradient_colors(album_id);
    let to_color = |c: [u8; 3]| Color::from_rgb8(c[0], c[1], c[2]);
    let gradient = iced::gradient::Linear::new(iced::Radians(2.4))
        .add_stop(0.0, to_color(c1))
        .add_stop(1.0, to_color(c2));
    container(Space::new(Length::Fixed(size), Length::Fixed(size)))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Gradient(gradient.into())),
            ..container::Style::default()
        })
        .into()
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
