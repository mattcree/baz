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
//! - **Art workers**: visible tiles request thumbnails through a two-job,
//!   visibility-first scheduler; each job uses `tokio::task::spawn_blocking`
//!   ([`crate::art`]). Decoded RGBA lands in the bounded LRU whose budget is
//!   derived in `art.rs`. Tiles without art render a deterministic gradient
//!   placeholder.
//! - **Playback** ([`crate::playback`], [`crate::player`]): the device
//!   engine is spawned once at app start. Commands go straight to the
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

use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use baz_core::history::{History, HistoryLedger};
use baz_core::index::{GroupKey, IndexError, Library};
use baz_core::protocol::{self as protocol, Command, Event, SignalChain};
use baz_core::replaygain::ReplayGainSettings;
use baz_core::traversal::Traversal;
use baz_core::volume::Volume;
use iced::keyboard;
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{column, image as iced_image, row, scrollable};
use iced::{Element, Point, Size, Subscription, Task, window};
use lru::LruCache;

use crate::motion::{Control, Ink, Keyed, Tween};
use crate::mpris::Mpris;
use crate::place::{History as PlaceHistory, Place};
use crate::playback::{OutputChoice, Playback, PlayerEvent};
use crate::player::{PlayerState, SignalPath, SignalWarningState};
use crate::scan::ScanUpdate;
use crate::selection::{Content, Press};
use crate::{
    art, config, font, keys, menu, motion, mpris, player, queue_edit, scan, shelf, theme, views, vm,
};

// The top bar's height — used for the pre-first-scroll estimate of the grid
// viewport (real bounds arrive with every scroll event) — is
// [`theme::top_bar_h`] **resolved against the window's width**, never the
// single-line constant. It was a local `56.0` against a bar that drew 53,
// which the composition audit caught; it became `theme::TOP_BAR_H` so the
// estimate could not drift from the drawing — and with the strip's two-line
// regime (doc 10 §4.3) the same discipline means asking the theme which
// regime the width resolves to, because an estimate 40 px out below 960
// would be the rail's capacity bug all over again.
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
/// Do not launch the comparatively expensive filesystem walk while the
/// listener is actively scrolling, resizing or choosing music.
const REFRESH_IDLE: Duration = Duration::from_secs(30);
/// Quiet time after the last fader wheel step before its confirmed position is
/// persisted. Audio answers every step immediately; disk sees one settled act.
const VOLUME_WHEEL_SETTLE: Duration = Duration::from_millis(240);

/// The shelf scrollable's id — the update loop scrolls it back to the top
/// when the query changes, and [`crate::views::shelf`] attaches it.
pub(crate) fn scroll_id() -> iced::widget::Id {
    iced::widget::Id::new("baz-shelf")
}

/// The search field's id — the update loop focuses it, and
/// [`crate::views::top_bar`] attaches it.
pub(crate) fn search_id() -> iced::widget::Id {
    iced::widget::Id::new("baz-search")
}

/// An id no widget in the tree carries, used to **blur** the search well.
///
/// iced 0.13 publishes `iced::widget::operation::focus` and no `unfocus`, but its focus
/// operation is defined over the whole tree: it focuses the widget whose id
/// matches and **unfocuses every other focusable it walks past**
/// (`iced_core::widget::operation::focusable::focus`). Focusing an id nothing
/// carries is therefore exactly "focus nothing", using the toolkit's own
/// documented behaviour rather than a private field.
///
/// It is a named constant with a test holding it apart from [`search_id`],
/// because the entire mechanism is that the two strings differ.
fn nothing_id() -> iced::widget::Id {
    iced::widget::Id::new("baz-nothing")
}

/// Take the caret out of the search well (see [`nothing_id`]).
fn blur_search<T: Send + 'static>() -> Task<T> {
    iced::widget::operation::focus(nothing_id())
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
    let selected_theme = config::config_file().map_or_else(
        || crate::theme_file::DEFAULT_SELECTION.to_owned(),
        |path| config::load(&path).theme,
    );
    let room = theme::install(&selected_theme);
    crate::baz_log!("[startup] room: {}", room.name);
    let mut app = iced::application(
        move || App::new(started, cli_dir.clone()),
        App::update,
        App::view,
    )
    .title("baz")
    .subscription(App::subscription)
    // **baz closes itself.** iced 0.13 would close the window on the
    // compositor's request before the update loop saw it, and the one
    // thing that has to happen on the way out is writing where the run
    // got to (ADR-0023 §6). The request becomes `Message::Quit` — the
    // same message the desktop's own Quit sends, so there is one exit
    // path and it cannot drift.
    .exit_on_close_request(false)
    .theme(app_theme)
    .default_font(theme::SANS)
    .window(window_settings());
    for face in font::FACES {
        app = app.font(face);
    }
    app.run()
}

fn app_theme(_app: &App) -> iced::Theme {
    theme::theme()
}

/// Run an action against baz's sole application window, if it still exists.
fn latest_window<T: Send + 'static>(
    action: impl Fn(window::Id) -> Task<T> + Send + 'static,
) -> Task<T> {
    window::latest().then(move |id| id.map_or_else(Task::none, &action))
}

/// **Whether baz draws the window's chrome itself.**
///
/// One answer, read here and nowhere else: `app.rs` turns the platform's
/// decorations off with it, and the app bar asks it whether to draw the window
/// buttons. The owner, 2026-08-10, looking at the shipped state: *"until we
/// have no window chrome, remove the window controls..."* — with the system
/// title bar above baz's own band, minimise, maximise and close appeared
/// twice, four pixels apart, and one pair did nothing the other did not.
///
/// So the buttons are not *removed*; they are **conditional on baz owning the
/// chrome**, which is the honest rule and the one that needs no second edit
/// now that borderless ownership is the default. The bar keeps its drag and its
/// double-press to maximise either way: those *add* a way to move a window
/// that already had one, where a second close button subtracts clarity from a
/// window that already had one of those too.
fn owns_chrome() -> bool {
    std::env::var_os("BAZ_NATIVE_CHROME").is_none()
}

/// The window's settings: its size, on Linux the application id, and Baz-owned
/// chrome by default.
///
/// # `BAZ_NATIVE_CHROME=1`
///
/// Restores the platform title bar for comparison and diagnostics. Ordinarily
/// `decorations` is false, the app bar draws the window controls, and
/// [`crate::window_frame`] spends iced 0.14's `window::drag_resize` across a
/// six-pixel eight-way edge/corner band. Maximized windows disable that band.
///
/// iced leaves the Wayland `app_id` / X11 `WM_CLASS` empty by default,
/// which is what makes a launcher show a running window as an unrelated
/// "unknown" entry beside its own icon. Setting it to the basename of
/// `packaging/io.github.mattcree.baz.desktop` is the whole of the association
/// — the same string MPRIS advertises as `DesktopEntry`, which is why
/// [`mpris::DESKTOP_ENTRY`] is the single place it is spelled.
fn window_settings() -> window::Settings {
    let mut settings = window::Settings {
        size: WINDOW,
        decorations: !owns_chrome(),
        // The window's declared minimum width is [`theme::WINDOW_FLOOR_W`]:
        // the width at which both strips still hold — the app bar's own line
        // needs 702 (see `theme::APP_BAR_LINE`) and the place strip below it
        // needs the strip's 600 with the lane's collapsed rail beside it.
        // ADR-0030 puts a 64 px rail permanently to the strip's left and the
        // strip resolves against `Shelf::body_width`, so the *window* has to
        // be that much wider for the same strip to fit. Height is left
        // unbounded; the study declares no floor for it.
        min_size: Some(Size::new(theme::WINDOW_FLOOR_W, theme::WINDOW_FLOOR_H)),
        ..window::Settings::default()
    };
    settings.icon = window_icon();
    #[cfg(target_os = "linux")]
    {
        settings.platform_specific.application_id = String::from(mpris::DESKTOP_ENTRY);
    }
    settings
}

/// Decode baz's canonical red circle for platforms that support a per-window
/// icon (Windows and X11). Wayland obtains the same mark from the desktop
/// entry's hicolor icon instead.
fn window_icon() -> Option<window::Icon> {
    let rgba = ::image::load_from_memory(include_bytes!(
        "../assets/icons/logo-transparent-circle-red.png"
    ))
    .ok()?
    .into_rgba8();
    let (width, height) = rgba.dimensions();
    window::icon::from_rgba(rgba.into_raw(), width, height).ok()
}

/// How close two presses on the app bar have to be to count as a double —
/// **400 ms**, the interval every mainstream desktop uses as its default and
/// the one GNOME ships (`org.gnome.desktop.peripherals.mouse double-click`).
///
/// A constant rather than the desktop's own `double-click` setting, and the
/// reason is what a wrong answer costs: a double-click window 100 ms from the
/// system's is a gesture that occasionally has to be repeated, which is not
/// worth a `gsettings` spawn at startup and a dconf dependency to avoid. (The
/// bar's *side* was a different question with a different answer — see
/// [`crate::views::app_bar`] — and the owner settled it by declining the
/// per-platform path there too.)
const BAR_DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// **The message meter** — `BAZ_MSG_LOG=1`, the sibling of `BAZ_FRAME_LOG`.
///
/// Prints one line a second naming every message variant that arrived in it
/// and how many times, busiest first, and nothing at all in a second where
/// nothing arrived. It exists because *"something is firing a lot"* is a
/// hypothesis a log can settle in ten seconds and a reader cannot settle at
/// all: the shell's messages come from six subscriptions, a scrollable that
/// republishes its viewport on every layout change, and a window that
/// reconfigures on every drag step, and which of those is the loud one is not
/// a thing to reason about.
///
/// Off by default and **free when off**: one relaxed atomic load per message,
/// resolved once from the environment. The variant name is taken from `Debug`
/// up to its first `(`, which is the same trick `menu.rs`'s mirror test uses,
/// and it is only formatted when the meter is on.
fn note_message(message: &Message) {
    use std::sync::atomic::{AtomicU8, Ordering};
    /// 0 unresolved, 1 off, 2 on.
    static STATE: AtomicU8 = AtomicU8::new(0);
    static TALLY: LazyLock<Mutex<(HashMap<String, u32>, Instant)>> =
        LazyLock::new(|| Mutex::new((HashMap::new(), Instant::now())));

    let mut state = STATE.load(Ordering::Relaxed);
    if state == 0 {
        state = if std::env::var_os("BAZ_MSG_LOG").is_some() {
            2
        } else {
            1
        };
        STATE.store(state, Ordering::Relaxed);
    }
    if state == 1 {
        return;
    }
    let debug = format!("{message:?}");
    let name = debug
        .split_once('(')
        .map_or(debug.as_str(), |(head, _)| head);
    let Ok(mut tally) = TALLY.lock() else {
        return;
    };
    *tally.0.entry(name.to_owned()).or_default() += 1;
    let now = Instant::now();
    if now.duration_since(tally.1) < Duration::from_secs(1) {
        return;
    }
    let mut counted: Vec<(String, u32)> = tally.0.drain().collect();
    tally.1 = now;
    drop(tally);
    counted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let total: u32 = counted.iter().map(|(_, n)| n).sum();
    let listed: Vec<String> = counted
        .iter()
        .map(|(name, n)| format!("{name} {n}"))
        .collect();
    crate::baz_log!("[msg] {total}/s  {}", listed.join("  ·  "));
}

/// Top-level messages; one enum across both screens keeps the seams simple.
///
/// Crate-visible because [`crate::views`] emits them: a view function's whole
/// output is an [`Element`] parameterised by this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CoverAction {
    #[default]
    Play,
    Queue,
    Open,
}

impl CoverAction {
    fn moved(self, delta: i32, engine: bool) -> Self {
        let actions: &[Self] = if engine {
            &[Self::Play, Self::Queue, Self::Open]
        } else {
            &[Self::Open]
        };
        let current = actions
            .iter()
            .position(|action| *action == self)
            .unwrap_or(0);
        let target = if delta < 0 {
            current.saturating_sub(1)
        } else {
            current.saturating_add(1).min(actions.len() - 1)
        };
        actions[target]
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Setup screen: the folder text input changed.
    SetupInput(String),
    /// Setup screen: folder submitted (Enter).
    SetupSubmit,
    /// Blocked screen: open the library again, with everything unchanged —
    /// for the failures something outside baz can fix while the screen is up
    /// ([`Blocked::can_retry`]).
    LibraryRetry,
    /// Blocked screen: show (`true`) or put away (`false`) what starting a new
    /// index would cost. **This message never moves a file** — that is the
    /// point of it; see [`Blocked::setting_aside`].
    LibrarySetAsideAsked(bool),
    /// Blocked screen: confirmed. Rename the library out of the way and build
    /// a new index over the same folders.
    LibrarySetAside,
    /// Shelf: search text changed.
    SearchChanged(String),
    /// **The well's clear mark** — the `×` the owner asked for (2026-08-10:
    /// *"maybe a little x or esc to clear would make sense too"*).
    ///
    /// It is <kbd>Esc</kbd>'s pointer route and nothing more: both land in
    /// [`Shelf::clear_query`], so the query goes, the caret leaves the field
    /// and the transport gets the keyboard back. Present exactly while a query
    /// stands, which is exactly when <kbd>Esc</kbd> has that layer to peel —
    /// the rule ADR-0036 §4 states, that no pointer route may exist for a
    /// state the keyboard cannot reach and none may act where the key would
    /// not.
    ClearSearch,
    /// Put the search dropover away. Dismissal and clearing are one act: a
    /// standing query is never hidden behind the unchanged place.
    DismissSearch,
    /// A bare arrow, routed to search while its dropover is open and to the
    /// established transport control otherwise. Search claims this before a
    /// focused well can spend Left/Right on its caret.
    Direction(crate::search::Direction),
    /// Confirm the selected result/action in the search dropover.
    SearchConfirmed,
    /// A pointer press on one of a search row's explicit actions.
    SearchAction(crate::selection::Content, crate::search::Action),
    /// The one result scroll surface reporting its viewport.
    SearchScrolled(scrollable::Viewport),
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
    /// <kbd>Enter</kbd> outside a focused field: confirm the open search
    /// chooser's selected result/action, otherwise activate the current
    /// content selection.
    ///
    /// Only defensible because the first match is the best match — ADR-0021
    /// ranks `Library::search` by fit, then field, then library order —
    /// which is why step 12 had to land before step 11 could.
    PlayFirstMatch,
    /// Step the density: a press on one of the four detent marks (ADR-0028
    /// as amended, and ADR-0040 §5) — **in the app bar's display-options
    /// slot**, in every place that hangs works — or its accelerators,
    /// <kbd>Ctrl</kbd>+<kbd>-</kbd> / <kbd>Ctrl</kbd>+<kbd>=</kbd>
    /// and <kbd>Ctrl</kbd>+scroll. Those work in every place, and since the
    /// three places that hang works all read one grid, they are visible
    /// wherever they are legal. `+1` loosens the hang and `-1`
    /// tightens it; both saturate, and a mark sends the exact signed notch
    /// count between here and its step (see [`shelf::Density::step`],
    /// [`shelf::Density::steps_to`]).
    DensityStep(i32),
    /// The modifier keys that are down, as iced last reported them.
    ///
    /// Held for the two inputs iced 0.13 reports without modifier state:
    /// [`Self::Wheel`] (`WheelScrolled` carries none, so
    /// <kbd>Ctrl</kbd>+scroll cannot be recognised from the wheel event
    /// alone) and a `button`'s press ([`Self::AlbumClicked`] resolves
    /// shift-click against it, doc 09 §13 step 7).
    ModifiersChanged(keyboard::Modifiers),
    /// A wheel notch, with its vertical travel. Answered against the modifiers
    /// above by [`keys::wheel_binding`]; a plain scroll is the `scrollable`'s
    /// own business and this arm does nothing with it.
    Wheel(f32),
    /// Esc anywhere: peel one layer, top down — the place you are in, then the
    /// search query, then the shuffle pool's marks (see [`App::escape`]).
    EscapePressed,
    /// The app bar's browser-style place-history arrows, also Alt+Left/Right.
    HistoryBack,
    HistoryForward,
    /// **The returns lane's Now playing row, and <kbd>Ctrl</kbd>+<kbd>U</kbd>**:
    /// go to `Now playing`.
    ///
    /// The prior-art study's R3 — *get back to what is playing* — which every
    /// product it surveyed spends an affordance on and baz had none for. It
    /// used to open the *record's page*, and that was right while the record's
    /// page was the only surface that knew what was sounding. `Now playing`
    /// exists now and is that surface, so the dedicated lane row leads there.
    /// The persistent bar's track block instead follows the same provenance
    /// road as the source footer: saved playlist, unsaved queue, or album.
    ///
    /// **`Message::ShowTheRun` folded into this one** when the `Run` word was
    /// removed. That message was this message plus *turn the density on*, and
    /// with one density left it was this message with a longer name. So
    /// <kbd>Ctrl</kbd>+<kbd>U</kbd> sends this, and stays legal on the twin it
    /// always had: the returns lane's `Now playing` row, which is the same
    /// destination and is visible at rest. It does **not** toggle — a
    /// destination never closes itself ([`crate::place::Place::go`]) — and
    /// <kbd>Esc</kbd> is the way out.
    ShowNowPlaying,
    /// Open or close the bottom-right application status and event history.
    ToggleStatus,
    /// Dismiss the application status layer without changing any place.
    CloseStatus,
    /// Retry the recoverable library-health conditions with one incremental scan.
    RetryHealth,
    /// Pressing the current-song block in the bottom bar: open its source and,
    /// for a saved playlist, bring the sounding entry into view.
    OpenPlayingSource,
    /// Open the current run as its unsaved playlist.
    ShowQueue,
    /// The subtle provenance link on Now playing: open the album the sounding
    /// track belongs to, without inheriting a wall tile's shift-click queue
    /// gesture.
    OpenAlbum(u64),
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
    /// A row's ▲ (`-1`) or ▼ (`+1`) stepper in the **Queue** place: swap the
    /// entry with its neighbour — the playlist page's reorder, on the run
    /// (doc 09 §8.2; [`Command::UpdateQueue`], so the music keeps playing
    /// and the cursor follows its track).
    ShiftQueued(usize, i32),
    /// A row's `+` in the **Queue** place: hold that row's track and open
    /// the panel as the picker (doc 09 §8.1's transfer gesture, reaching the
    /// queue's own editor at step 5) — pick a destination, a file or the
    /// run itself.
    AddQueuedToPlaylist(usize),
    /// The **Queue** place scrolled; carries the real viewport geometry.
    ///
    /// What [`Self::Scrolled`] is to the wall this is to the queue place:
    /// the offset [`crate::queue_window`]'s virtual window is computed
    /// against, held so `Play all`'s five-figure run costs the frame what a
    /// record does (doc 09 §7.1's gate).
    QueueScrolled(Viewport),
    /// The returns lane scrolled; used to request collage art only for the
    /// playlist rows around its viewport.
    LaneScrolled(Viewport),
    /// A saved playlist's track table scrolled; retained so the page builds
    /// only the visible row window and requests artwork for that window.
    PlaylistScrolled(Viewport),
    FavouritesScrolled(Viewport),
    /// The saved-playlist collection scrolled; retained for tile
    /// virtualisation and viewport-scoped collage requests.
    PlaylistsScrolled(Viewport),
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
    /// **`Resume` on the Home place's `CONTINUE` placard**: put the
    /// interrupted run back on, at the track and the second it was
    /// interrupted at.
    ///
    /// The ordinary `Play` (ADR-0030 §6), aimed at the snapshot's cursor:
    /// `JumpTo` there, then `Seek` to the position. It is the one press on
    /// that page that starts audio, and the only thing that spends the
    /// snapshot's elapsed milliseconds.
    ResumeRun,
    /// Ctrl+`P`: summon the playlist panel, or close it (ADR-0024 §5). A
    /// float over the place, not a place — the wall does not reflow by a
    /// pixel.
    ///
    /// Its strip door is gone: the returns lane is the resident index of
    /// lists, so a labelled door to a *second* index would be two controls
    /// answering one question (L8.6). The panel keeps its key and its job as
    /// the picker for `Add to…` (ADR-0031's card at the pointer is not
    /// built), and the key is now its only summons.
    TogglePlaylists,
    /// Create the requested mix, or preserve it while first-use consent opens.
    VibeCreate,
    /// Inspect the selected library editions and begin missing local analysis.
    VibeAnalyze,
    /// The persistent sonic cache was checked away from the UI thread.
    VibePrepared(Result<crate::vibe::Preparation, String>),
    /// One bounded track-analysis task completed.
    VibeAnalyzed(crate::vibe::AnalysisResult),
    /// Stop scheduling analysis after the currently running track returns.
    VibeAnalysisCancel,
    /// Edit the ordinary-language request without generating or playing.
    VibePrompt(String),
    /// Set the requested listening duration.
    VibeLength(crate::vibe::MixLength),
    /// **The contour's own gestures** — the shape a generated list is asked
    /// to follow (`crate::contour`). The drag carries the raw geometry the
    /// pointer described and `crate::vibe` decides what a line may be; the
    /// release exists so a recomposition costs one gesture rather than one
    /// pixel.
    ContourDragged(usize, usize, f32, f32),
    ContourReleased,
    /// **The pointer entered or left one row of the composed preview**, so
    /// the contour can light that track's own place on the line. The owner:
    /// *"when we hover the playlist items it is showing where on the curve
    /// it's meant to be… so a person can see it really worked."*
    VibePreviewHovered(Option<usize>),
    /// Load one of the drawn shapes over the current line.
    ContourShape(usize),
    /// Give one line another turn, or take its last one back.
    ContourPointAdded(usize),
    ContourPointRemoved(usize),
    /// **Draw a second or third musical dimension, or stop drawing one.** The
    /// owner: *"can we have more than one of these for different musical
    /// dimensions — this obviously kinda rolls up several aspects of a song
    /// into one value."*
    ContourDimension(crate::vibe::Dimension),
    /// Explicitly explore another deterministic version of this request.
    VibeAnother,
    /// Edit the in-memory preview without touching music or playlist files.
    VibePreviewRemove(usize),
    VibePreviewShift(usize, i32),
    /// Put the edited preview on as the run. This is Vibe's explicit playback act.
    VibePlay,
    /// Write the previewed, ordinary playlist file and open it without playing.
    VibeSubmit,
    /// Open the canonical playlist-creation place at its chooser.
    NewPlaylistOpen,
    /// Open the creation place with Vibe already chosen (Home shortcut).
    NewPlaylistOpenVibe,
    PlaylistCreationMode(crate::playlists::CreationMode),
    PlaylistCreationBack,
    PlaylistCreationName(String),
    PlaylistCreationExample(&'static str),
    PlaylistCreationRemove(usize),
    PlaylistCreationShift(usize, i32),
    PlaylistCreationSave,
    /// Toggle durable song-level Favourites membership without selecting or playing.
    ToggleFavourite(PathBuf),
    FavouritesPlay,
    FavouritesPlayTrack(usize),
    /// **The returns lane's head, pressed**: go to that destination
    /// (ADR-0030 as the owner amended it). Not a toggle — see [`Place::go`].
    GoTo(crate::lane::Destination),
    /// **The lane's collapse**, from either of the two marks at its foot or
    /// from Ctrl+`B`.
    ///
    /// The one press in the product whose subject is the collection's width,
    /// and therefore the one press that may re-hang the wall. It lands
    /// outside the wall, so no gesture on the wall can be in flight when it
    /// fires (ADR-0030 §3).
    ToggleLane,
    /// Arrange the full Playlists page by name, creation date or last play.
    PlaylistOrderSelected(crate::playlists::PlaylistOrder),
    /// The pointer entered a saved-playlist tile.
    PlaylistTileEntered(u64),
    /// The pointer left a saved-playlist tile.
    PlaylistTileLeft(u64),
    /// A playlist tile or panel row was pressed: open that playlist's page.
    /// Repeating the press is a no-op ([`Place::playlist`]).
    OpenPlaylist(u64),
    /// Play a saved playlist directly from its collection tile.
    PlayPlaylist(u64),
    /// Playlists overview: ask before moving this saved file to trash.
    PlaylistOverviewDeleteStart(u64),
    /// Playlists overview: cancel the pending deletion.
    PlaylistOverviewDeleteCancel,
    /// Playlists overview: confirm the trash-backed deletion.
    PlaylistOverviewDelete,
    /// **The album page's breadcrumb was pressed**: open that artist's page.
    ///
    /// The owner's *"we could add an Artist > album breadcrumb though. and
    /// have an artist page."* Carries [`crate::vm::artist_id`]'s hash rather
    /// than a name, for the reason every other place-opening message carries
    /// an id: a message is a value, and a borrowed name could not outlive the
    /// rescan that rebuilt the wall it came from.
    OpenArtist(u64),
    /// The artist page's quiet external door: ask the desktop to open a
    /// Wikipedia search. Baz performs no network request itself.
    LookUpArtist(u64),
    /// Completion of that desktop request; failures become a status event.
    ArtistLookUpFinished(Result<(), String>),
    /// A pick-mode press on a panel row: append what the hand holds to that
    /// playlist's *file* — the run is untouched, whichever list it is, the
    /// playing one included (09 §6's decoupling; S4).
    PickPlaylist(u64),
    /// A pick-mode press on the picker's **Queue** row: append what the hand
    /// holds to the run — `UpdateQueue`, the music keeps playing, and
    /// appending to an empty stopped engine loads a queue without starting
    /// it (09 §8.1).
    PickQueue,
    /// The panel's `New playlist` row was pressed: become a name field.
    NewPlaylistStart,
    /// The name field changed.
    NewPlaylistInput(String),
    /// The name field was submitted; the storage layer's name rule decides,
    /// and its refusal lands under the field in its own words.
    NewPlaylistSubmit,
    /// The record page's `Add to playlist…`: the record, whole (the selected
    /// edition), held while the panel opens as the picker — pick a
    /// destination, the Queue first among them (09 §8.1).
    AddAlbumToPlaylist(u64),
    /// A track row's reserved-slot `+` on the record's page: one track
    /// toward the picker, by the same rule (`album id`, zero-based row).
    AddTrackToPlaylist(u64, usize),
    /// The playlist page's `Play`: the playable subset as the queue, from the
    /// top ([`Command::SetQueue`] then [`Command::Play`] — ADR-0024 §4, and
    /// the counts line on the page is where the subset is declared).
    PlaylistPlay,
    /// A playlist row was clicked: play this list from that row, through the
    /// same [`PlayerState::play_from`] rule every list surface uses. Carries
    /// the display row; the playable-subset position is resolved against the
    /// open page.
    PlaylistPlayTrack(usize),
    /// A playlist row's ✕: take that entry out of the *file* — an edit to the
    /// artefact, saved atomically, no engine involved.
    PlaylistRemoveEntry(usize),
    /// A playlist row's ▲ (`-1`) or ▼ (`+1`) stepper: swap the entry with its
    /// neighbour — the no-drag reorder route the visible-control rule
    /// requires (ADR-0024 §4).
    PlaylistShiftEntry(usize, i32),
    /// A playlist row's `+`: hold that row's track and open the panel as the
    /// picker — the transfer slot the queue's rows carry, completing
    /// doc 09 §8.2's "same editor" anatomy on the page's side (and the
    /// visible twin §5.2's mirror rule requires of the page rows' menu
    /// items). File edits stay where they were: this reads the row, writes
    /// nothing.
    PlaylistAddEntry(usize),
    /// The playlist page's `Rename`: open the name field, seeded with the
    /// current name.
    PlaylistRenameStart,
    /// The rename field changed.
    PlaylistRenameInput(String),
    /// The rename field was submitted: a filesystem rename keeping the
    /// extension, refused in place by the storage layer's rule. The place
    /// moves with the name.
    PlaylistRenameSubmit,
    /// The playlist page's first `Delete` press: replace the ordinary acts
    /// with Cancel and the explicit trash confirmation.
    PlaylistDeleteStart,
    /// Withdraw the playlist delete confirmation.
    PlaylistDeleteCancel,
    /// The confirming `Move to Trash`: the playlist file moves to the
    /// platform trash and the page leaves for the Library.
    PlaylistDelete,
    /// The place's transient `Undo` word, and <kbd>Ctrl</kbd>+<kbd>Z</kbd>
    /// over it: take back the last recorded edit on the list surface the
    /// window is showing — the Queue place's run, or the open playlist
    /// page's file (doc 11 §5 P2; [`crate::undo`]).
    ///
    /// **Nothing sounds because of an undo.** A queue undo restores the
    /// *list* through [`Command::UpdateQueue`] — never the playback
    /// position, never a `Play` — and a playlist undo is one atomic file
    /// rewrite through the same fingerprint guard as the edit it reverses.
    Undo,
    /// The queue place's `Save as playlist`: become a name field
    /// (ADR-0024 §4 — the transient frozen into an artefact).
    SaveQueueStart,
    /// The save field changed.
    SaveQueueInput(String),
    /// The save field was submitted: a new file holding exactly what the
    /// queue holds, and nothing else.
    SaveQueueSubmit,
    /// The pointer entered a playlist row, so the row can offer its ✕ and its
    /// steppers — the queue rows' hover mechanism, for the same toolkit
    /// reason.
    PlaylistRowEntered(usize),
    /// The pointer left a playlist row. Carries which, for the reason
    /// [`Self::QueueRowLeft`] carries which row.
    PlaylistRowLeft(usize),
    /// A row's press travelled past [`crate::drag::THRESHOLD_PX`]: the row
    /// is in the hand (doc 09 §13 step 8, doc 11 P5 — the reorder drag,
    /// sugar over the steppers and the picker, which all remain). Carries
    /// which editor, which row, and where the pointer was when the gesture
    /// became a drag.
    DragLift(crate::drag::List, usize, Point),
    /// The held pointer moved — anywhere, the [`crate::groove`] discipline —
    /// so the ghost can follow it.
    DragMoved(Point),
    /// A row of the dragged list measured the held pointer inside its own
    /// bounds: which row, and whether the pointer is in its upper half —
    /// the insertion slot is decided from exactly this
    /// ([`crate::drag::slot`]), which is what keeps the index exact under
    /// [`crate::queue_window`]'s virtualization with no window-coordinate
    /// estimate anywhere.
    DragOverRow(crate::drag::List, usize, bool),
    /// The held pointer entered a panel playlist row: the drop becomes that
    /// file's append — drag-to-add, the picker row's own gesture made
    /// direct (09 §8.1; the picker remains the route when the panel is
    /// closed).
    DragOverPanel(u64),
    /// The held pointer left a panel playlist row. Carries which, for the
    /// reason [`Self::QueueRowLeft`] carries which row.
    DragLeftPanel(u64),
    /// The drag ended — an ordinary release, or the pointer stopped being
    /// ours (`CursorLeft`/`Unfocused`, doc 04 §2.2). One commit: a
    /// whole-list `UpdateQueue`, one saved file, or one append — decided
    /// against [`crate::drag::DragState`], and a drop on the no-op slot
    /// asks for nothing. <kbd>Esc</kbd> is the discard and never sends this.
    DragDropped,
    /// The pointer entered a track row on the record's page, so the row can
    /// offer its `+` when the panel is closed.
    AlbumRowEntered(usize),
    /// The pointer left a track row on the record's page.
    AlbumRowLeft(usize),
    /// Shelf scrolled; carries the real viewport geometry.
    Scrolled(Viewport),
    /// Window resized (approximate grid geometry until the next scroll).
    WindowResized(Size),
    /// A word in the top bar's group-key row, or `1`–`6`: arrange the wall by
    /// this key (ADR-0019). Persisted — a listener sets it once.
    GroupKeySelected(baz_core::index::GroupKey),
    /// An entry in the index rail was clicked: put that shelf at the top of
    /// the wall. Carries the run's index, not a pixel — the rail knows which
    /// shelf it points at and nothing about where the shelf is.
    RailJumped(usize),
    /// An entry in the saved-playlist collection's index rail was clicked.
    /// Carries the **run** it names, not a pixel — the same currency
    /// [`Self::RailJumped`] carries for the record wall, since both walls are
    /// laid out by [`crate::shelf::Shelves`]. The shared collection scaffold
    /// owns the rail, while each collection owns its content geometry.
    PlaylistRailJumped(usize),
    /// An explicit record-opening route: the veil/menu's labelled `Open`, a
    /// record link or source navigation. Ordinary tile presses instead send
    /// [`Self::ContentPressed`] and use the shared select/double-click grammar.
    ///
    /// **Shift held, the same press queues the record instead** (doc 09 §13
    /// step 7): the one-press accelerator over the picker's Queue row —
    /// see [`App::queue_album`] for how the visible-control rule is met. A
    /// `button`'s press carries no modifier state in iced 0.13, so the arm
    /// resolves it against the hand-kept `modifiers` — and every control
    /// that sends this message gains the accelerator with it (the sleeve,
    /// and the songs section's record door), which is the consistency the
    /// shared message makes structural: shift turns *open the record* into
    /// *queue the record*, wherever it is said.
    AlbumClicked(u64),
    /// A playable tile or row was pressed. One product-wide state machine
    /// selects on the first press and activates the same object on the second.
    ContentPressed(Content),
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
    /// **Append the record to the run** — the wall tile's hover `Queue`
    /// option, and exactly what shift-clicking a sleeve has always done
    /// ([`Self::AlbumClicked`] with shift, and [`App::queue_album`] under
    /// both). A message rather than a modifier because the option is a
    /// visible control and a button press carries one message; the gesture
    /// and the option now spend the same one, which is what stops the two
    /// routes drifting.
    ///
    /// Nothing sounds: an append is not a play gesture (ADR-0023 §3).
    QueueAlbum(u64),
    /// A track row of a record's page was clicked: play that album from
    /// that row (`album id`, zero-based row). One message for both of
    /// ADR-0014's cases — which commands go out is
    /// [`PlayerState::play_from`](crate::player::PlayerState::play_from)'s
    /// decision, not the view's.
    PlayTrack(u64, usize),
    /// **Home's `All songs` tile was pressed**: play everything you own.
    ///
    /// **The only `play everything` gesture there is**, since the owner
    /// removed the Library strip's `Play all` on 2026-08-10 (ADR-0040). That
    /// one plays the wall *as arranged*; this one plays the collection whole
    /// (`crate::implicit::ImplicitList::everything`), because **Home shows no
    /// wall** and a tile that applied a filter set on another page would be
    /// acting on state the listener cannot see from where they are standing.
    /// The tile states its own scope in its counts line, so what it will play
    /// is on screen beside it.
    PlayEverything,
    /// The `All songs` tile on an artist page: play that artist's implicit
    /// list in release chronology.
    PlayArtistSongs(u64),
    /// The pointer entered (`true`) or left (`false`) an `All songs` tile.
    ///
    /// The wall's [`Self::TileEntered`] mechanism for the one tile that is not
    /// a record, and a `bool` rather than an id because there is only ever one
    /// of it. iced 0.13 tells a widget its own hover status and its siblings
    /// nothing, so the tile reports its crossings and the shelf holds the
    /// answer — the pattern [`Self::TileEntered`] and [`Self::QueueRowEntered`]
    /// already use, for the same toolkit reason.
    AllSongsHovered(bool),
    /// **The playlist panel's `All songs` row**: go to the list.
    ///
    /// The list is the wall (`crate::all_songs`), so this is the Library —
    /// the same destination the lane's `Library` row names, reached from the
    /// object that names the list rather than the frame.
    ShowAllSongs,
    /// **Turn the player's shuffle property on or off** — the now-playing
    /// bar's crossed arrows.
    ///
    /// A *mode*, not an act. It says what order things play in from here, and
    /// it changes **the walk, never the list** — the run keeps the order the
    /// gesture that started it built, in both positions of the control, so
    /// turning it off is a `SetTraversal` and nothing else. Nothing stops: the
    /// sounding track plays to its end and what follows is re-planned
    /// (`baz_core::traversal`). See [`App::toggle_shuffle`].
    ToggleShuffle,
    /// Turn Repeat current track on or off.
    CycleRepeat,
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
    /// MPRIS `Quit`: close baz — and, since ADR-0040, the app bar's own
    /// close button. One exit path, two doors to it.
    Quit,
    /// **The app bar's minimise button** (ADR-0040 §3): put the window down.
    WindowMinimised,
    /// **The app bar's maximise button**: fill the screen, or come back off
    /// it. One control and one message, because `window::toggle_maximize` is
    /// one action — the button's *drawing* is what carries the state.
    WindowMaximiseToggled,
    /// F11: fill the window's current monitor, or return to its windowed size.
    ToggleFullscreen,
    /// The current mode read back before an F11 toggle.
    WindowModeRead(window::Mode),
    /// Begin the compositor's native resize gesture from one window edge or
    /// corner. Emitted only by the borderless frame's narrow hit band.
    WindowResize(window::Direction),
    /// **A press anywhere in the app bar that no control took**: move the
    /// window, or — if it is the second press inside
    /// [`BAR_DOUBLE_CLICK`] — maximise or restore it.
    ///
    /// One message for both because iced 0.13's `mouse_area` has no
    /// `on_double_click` (0.14 adds one), so the second press has to be
    /// recognised here, against the first's clock. Every control in the bar is
    /// a `button` and captures its own press before the bar's `mouse_area`
    /// sees it, so this only ever arrives from the gaps, the window's name and
    /// the empty slots — which is exactly the surface a platform title bar
    /// treats as its handle.
    WindowDragged,
    /// **A right press in the app bar**: ask the platform for the window menu
    /// (move, resize, always-on-top, workspace — whatever this desktop puts
    /// in it).
    ///
    /// It is best-effort by nature. `window::show_system_menu` is serviced on
    /// the backends that have such a menu and is a no-op on the ones that do
    /// not, which is the correct behaviour for a gesture that offers the
    /// platform's own affordance: baz does not grow a menu of its own to fill
    /// the gap, because a window menu that is baz's would not contain the
    /// entries the desktop's does.
    ///
    /// **It is also the standing answer to the resize question on GNOME**: the
    /// system menu's own `Resize` is a keyboard resize the compositor drives,
    /// and it is reachable from here without an edge to grab.
    WindowMenuRequested,
    /// The window's maximised state, as the window itself reports it.
    ///
    /// Asked for after every resize, because a maximise or an unmaximise is
    /// always a resize and there is no event that says so directly in
    /// iced 0.13. The cost is one oneshot per resize message against a full
    /// relayout per resize message, which is not a cost; what it buys is a
    /// maximise button that says `Restore` on a maximised window, which is the
    /// icon-only law's *stable in every state* clause (doc 10 §3.1) holding in
    /// the one state anybody checks.
    WindowMaximizedChanged(bool),
    /// Whether the compositor says this window has focus. The jewel case's
    /// idle clock is absent while false, so background windows pay no redraws.
    WindowFocused(bool),
    /// Advance the Now Playing jewel case's slow unattended turn.
    CaseTick(Instant),
    /// The pointer took hold of the jewel case.
    CasePressed(Point),
    /// The held pointer moved over the jewel case.
    CaseDragged(Point),
    /// The pointer released the jewel case.
    CaseReleased,
    /// Choose which record object stands in the Now Playing foreground.
    VisualizationForeground(crate::visualizer::Foreground),
    /// Advance through Off, Spectrum, Waveform and Spectrogram.
    NextVisualization,
    /// Show or hide the local one-line fact feed.
    ToggleFacts,
    /// Advance the sounding record's fixed fact cycle (timer or press).
    AdvanceFact,
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
    /// engine as a seek within the current song.
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
    /// Vertical wheel travel over the live fader, normalized by the groove to
    /// deliberate signed steps. It never changes mute.
    VolumeWheel(i32),
    /// The coalescing clock for wheel-driven volume persistence.
    VolumeWheelSettled(Instant),
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
    /// Settings: remember a shared-mode output endpoint. It takes effect on
    /// the next launch, when the engine can be opened on it before any run is
    /// restored.
    OutputDeviceSelected(OutputChoice),
    /// Settings: choose how many local CLAP model sessions a Vibe scan may use.
    VibeWorkers(usize),
    /// Settings → Appearance: persist one stable built-in selection code.
    ThemeSelected(&'static str),
    /// The local JSON paste field changed. No parsing or filesystem work yet.
    ThemeJsonChanged(String),
    /// Validate and install the JSON currently pasted into Settings.
    ThemeImportPasted,
    /// Open a local JSON theme file.
    ThemePickFile,
    /// The local theme file picker/read completed.
    ThemeFilePicked(Result<String, String>),
    /// Fill the paste field with a round-trippable v1 template.
    ThemeLoadTemplate,
    /// Export the selected room through a save dialog.
    ThemeExport,
    /// The local export completed.
    ThemeExported(Result<PathBuf, String>),
    /// Settings place: show this section of the place (index into
    /// `views::settings::SECTIONS`).
    SettingsSection(usize),
    /// Settings → Debug: sample this process's own RAM and CPU
    /// ([`crate::resource`]). Carries the instant so the rate divides by the
    /// interval that actually elapsed rather than by the timer's nominal one.
    ///
    /// Its clock exists **only** while that section is the visible one, which
    /// is what keeps a resource meter from being a resource cost.
    ResourceTick(Instant),
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
    /// Settings: move one held music folder one slot earlier.
    MoveMusicFolderUp(usize),
    /// Settings: move one held music folder one slot later.
    MoveMusicFolderDown(usize),
    /// Settings place: the armed removal was declined.
    CancelRemoveMusicFolder,
    /// Settings: reveal the exact missing paths before any index change.
    ConfirmPruneMissing,
    /// Settings: forget the previewed missing paths, preserving first-seen
    /// tombstones and touching no files, playlists, history or playback.
    PruneMissing,
    /// Settings: dismiss the missing-path confirmation unchanged.
    CancelPruneMissing,
    /// Settings: reveal legacy rows assigned to no configured folder.
    ConfirmPruneUnrooted,
    /// Settings: remove the previewed rootless rows from the index only.
    PruneUnrooted,
    /// Settings: dismiss the rootless-row confirmation unchanged.
    CancelPruneUnrooted,
    /// Settings: show the listener-owned playlists folder in the file manager.
    OpenPlaylistsFolder,
    /// Settings place: **force sync** — re-read every file in every folder,
    /// ignoring stamps (ADR-0022 §3).
    ForceSync,
    /// The periodic-refresh clock ticked; a rescan may be due (ADR-0022 §3).
    RefreshTick,
    /// An engine event arrived over the bridge subscription.
    Playback(PlayerEvent),
    /// An off-thread thumbnail decode finished (`None` = no usable art), with
    /// **the decode's own shortest edge** beside the handle — the number that
    /// keeps *no artwork is ever drawn larger than its source* true on the Now
    /// playing place before that record's hero has landed (see
    /// [`Shelf::thumb_px`]).
    ThumbLoaded(u64, u32, Duration, Option<(f32, usize, iced_image::Handle)>),
    /// An off-thread **hero** decode finished — the Now playing place's own
    /// tier ([`art::load_hero`], doc 12 §5.2). `None` = no usable art, which
    /// is the same answer [`Self::ThumbLoaded`] gives and is recorded in the
    /// same known-absent set.
    HeroLoaded(u64, Option<Hero>),
    /// A listener-provided local artist portrait finished decoding.
    ArtistImageLoaded(u64, Option<iced_image::Handle>),
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
    /// A right press on one of §5.2's four menu objects (doc 09): open the
    /// context menu for `target` at the pointer — the [`Point`] is the
    /// press's window position, read by [`crate::menu::area`] because
    /// `mouse_area`'s own `on_right_press` message carries none and the
    /// float opens *at the pointer*, flipped inside the window at its
    /// edges.
    ///
    /// Opening while another menu stands replaces it — the overlay state is
    /// a single `Option`, so "one menu at a time" is structure, not policy.
    OpenMenu(menu::Target, Point),
    /// A left press on the open menu's backdrop: put the menu down. The
    /// press is spent on the closing — it reaches no control underneath —
    /// which is what a press outside an open menu means everywhere else on
    /// the desktop.
    CloseMenu,
    /// The open menu's item `index` was pressed: close the menu and make
    /// the presses the item mirrors (§5.2's rule — every one a message
    /// some visible control also sends; [`crate::menu::Item`]).
    MenuItemPressed(usize),
    /// A folder (or file) was dropped on the window — the first-run screen's
    /// drop target (doc 11 §5 P1: see-and-point; the era's window-as-target
    /// since drag and drop existed).
    ///
    /// Wired for what the toolkit actually delivers: winit 0.30 publishes
    /// `DroppedFile` on X11 and **not on Wayland** (its Wayland backend has
    /// no data-device handling at all), so this is an accelerator where the
    /// platform provides it and absent where it does not — the `Browse…`
    /// button and the typed path are the routes that exist everywhere,
    /// which is why the screen's copy does not advertise dropping. The
    /// deferral is recorded against ADR-0025 per P1's adopt-modified text.
    FileDropped(PathBuf),
    /// A file drag entered the window (X11 only, as above): the first-run
    /// screen says where it would land.
    FileHovered,
    /// The file drag left the window without dropping.
    FileHoverLeft,
}

// The `clippy::struct_excessive_bools` expectation went away on its own when
// the run column's density left — the shell held one flag per remembered
// *view* decision, and removing the `Run` word took the count back under the
// lint's threshold. It is back, and the honest thing is to say what put it
// back rather than to leave the note claiming a reduction that no longer
// holds: `window_maximized` (ADR-0040 §3) is one more flag, and it is one the
// shell cannot avoid holding, because iced 0.13 publishes no event for a
// window being maximised and the app bar's button has to draw one of two
// glyphs.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the shell's flags are each a distinct fact about a distinct \
              subsystem — no two of them are a state machine in disguise, \
              which is what the lint is for"
)]
struct App {
    started: Instant,
    /// Most recent foreground message. Periodic library maintenance waits for
    /// this to go quiet so its I/O and metadata churn never competes with the
    /// interaction the listener can feel.
    last_interaction: Instant,
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
    /// The visited-place cursor behind the resident app-bar Back/Forward
    /// arrows. This is deliberately separate from [`Self::place`]: one says
    /// what is on screen; the other remembers the route that got here.
    place_history: PlaceHistory,
    /// Which row of the **Queue** place the pointer is on, if any.
    ///
    /// The rows offer their removal ✕ on hover only, and iced 0.13
    /// has no way for one widget to ask whether a *sibling* is hovered — a
    /// style function learns its own status and nothing else. So the row
    /// reports its own crossings with a `mouse_area` and the shell holds the
    /// one answer. The ✕'s slot is reserved either way, so this changes what is
    /// drawn in it and never the geometry around it.
    hovered_queue_row: Option<usize>,
    /// How far the **Queue** place is scrolled, as its scrollable last
    /// reported ([`Message::QueueScrolled`]) — the offset the place's
    /// virtual window is computed against (`crate::queue_window`).
    ///
    /// Reset to the top when the place is entered, because that is where a
    /// fresh scrollable actually stands: iced 0.13 keys widget state by
    /// tree position, so leaving the place unmounts the scrollable and
    /// coming back re-creates it at zero — a remembered offset would window
    /// rows the widget is not showing.
    queue_scroll: f32,
    /// Absolute offset of the returns lane's single list scroller.
    lane_scroll: f32,
    /// A deliberate start whose first successfully decoded track should open
    /// Now Playing. Paths identify the requested run without trusting command
    /// acceptance as playback truth; the marker is spent only by a matching
    /// [`Event::TrackStarted`] and cleared when another run supersedes it.
    show_on_start: Option<Vec<PathBuf>>,
    /// Absolute offset of the saved-playlist page's row scroller.
    playlist_scroll: f32,
    /// Absolute offset of the saved-playlist collection grid.
    playlists_scroll: f32,
    /// Which row of a **playlist's page** the pointer is on — the same
    /// mechanism as [`Self::hovered_queue_row`], for the page's ✕ and ▲▼
    /// slots.
    hovered_playlist_row: Option<usize>,
    /// Which track row of the **record's page** the pointer is on — the same
    /// mechanism again, for the row's reserved `+` slot (ADR-0024 §6).
    hovered_album_row: Option<usize>,
    /// The reorder drag in flight, `None` at rest ([`crate::drag`],
    /// doc 09 §13 step 8). **One `Option` is the whole gesture state** —
    /// the menu's own construction — so one drag at a time is structural,
    /// and <kbd>Esc</kbd> discards it by one assignment before any other
    /// layer peels.
    drag: Option<crate::drag::DragState>,
    /// The Queue place's edit history: the run as it stood before each of
    /// the last few edits — remove, reorder, append — newest last
    /// ([`crate::undo`], doc 11 §5 P2). Cleared when the run ends and when
    /// the Queue place is left; restored lists go out as
    /// [`Command::UpdateQueue`] and nothing else, so an undo can never
    /// sound.
    queue_undo: crate::undo::History<vm::QueueVm>,
    /// Consecutive search `Next` presses append after the prior insertion
    /// rather than reversing themselves at `cursor + 1`.
    enqueue_next: crate::search::NextAnchor,
    /// The open context menu, if one stands (doc 09 §5.2) — `None` at rest.
    ///
    /// **One `Option` is the whole overlay state**, which is what makes
    /// "one menu at a time" structural: opening another replaces this one,
    /// and every close — <kbd>Esc</kbd> (the peel's outermost layer), a
    /// press outside, an item press, any navigation — is `None` by one
    /// assignment. The items are captured at open, so a press sends exactly
    /// what was offered on screen ([`crate::menu::Menu`]).
    menu: Option<menu::Menu>,
    /// Whether the bottom-right application health/event card is visible.
    status_open: bool,
    /// The playlist surfaces: the panel, the open page, and the shelf of
    /// files behind both ([`crate::playlists`], ADR-0024 §4–§6).
    ///
    /// Beside `place` rather than inside it because the panel is not a place:
    /// it floats over Library, Album and Queue alike, and its open/closed
    /// state survives moving between them while a collecting task is under
    /// way. Session state throughout — which surface you were collecting
    /// into is not a standing decision, so none of it is in `config.toml`
    /// (the same argument as [`Self::settings_section`]).
    playlists: crate::playlists::Playlists,
    /// The window's size, as the last resize event reported it.
    ///
    /// Held because a place is laid out against the *window*: the record page
    /// and the queue both set their body to a measure of it, and the Settings
    /// place picks its arrangement from it. The shelf keeps its own, separately
    /// measured geometry; that one is the *viewport's*, which is not the
    /// window's once the bars and the rail have taken their share.
    window: Size,
    /// Whether the window is maximised, as the window itself last reported it
    /// ([`Message::WindowMaximizedChanged`]).
    ///
    /// Read by exactly one control — the app bar's maximise button, which
    /// draws a square when it will maximise and two offset squares when it
    /// will restore. It is *asked for* after every resize rather than tracked
    /// optimistically, because a button that flipped its own drawing and then
    /// found the compositor had refused would be a control that lies about the
    /// window: on Wayland a maximise request is a request.
    window_maximized: bool,
    /// Whether Baz most recently placed its sole window in fullscreen mode.
    fullscreen: bool,
    /// When the app bar was last pressed, for the double-press that maximises
    /// ([`Message::WindowDragged`]). `None` at rest and immediately after a
    /// double, so that three presses are a double and a single.
    last_bar_press: Option<Instant>,
    /// The Now Playing jewel case's yaw, pitch and drag gesture.
    case_rotation: crate::jewel_case::Rotation,
    /// The foreground object and independent audio background in Now Playing.
    visualization: crate::visualizer::State,
    /// Fixed ring storage for history-based Now Playing visualizers.
    visualization_history: crate::visualizer::History,
    /// Position in the sounding record's fixed local-fact cycle.
    fact_index: usize,
    /// The engine connection (or its documented absence) — spawned once at
    /// app start, before the first screen.
    playback: Playback,
    /// Shared-mode endpoints found at launch, plus the system-default choice.
    output_choices: Vec<OutputChoice>,
    /// The endpoint written in config (or the system default).
    output_choice: OutputChoice,
    /// The endpoint this process actually opened. A picker change intentionally
    /// does not tear the current run down; it becomes active only at launch.
    active_output_choice: OutputChoice,
    /// Enumeration failure, shown in Settings and recorded in status.
    output_devices_error: Option<String>,
    /// Event-derived playback state; the only thing playback widgets read.
    player: PlayerState,
    /// The Baz-owned rate conversion already admitted to the event history.
    /// Repeated engine reports of the same continuing condition stay quiet;
    /// a direct report clears it so a later conversion is a fresh warning.
    signal_warning: SignalWarningState,
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
    /// The volume position as it currently stands on disk.
    ///
    /// Like [`Self::saved_replay_gain`], this makes persistence follow the
    /// engine's confirmation without rereading `config.toml` for every volume
    /// event. Mute and output-path changes report through the same event but do
    /// not move this value, so they cost no write.
    saved_volume: Volume,
    /// The last wheel step's settling boundary. While armed, engine
    /// confirmations redraw normally but do not write config one step at a
    /// time; the short subscription below commits the final confirmed value.
    volume_wheel_settles: Option<Instant>,
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
    /// Whether the returns lane opens open, read from the config for
    /// `group_key`'s reason and handed to the shelf the same way.
    lane_open: bool,
    /// **The lane, merged**: every playlist in one section and the shelf's
    /// recent records in the other, both in [`crate::lane::resolve`]'s one
    /// order (ADR-0030 §1 as its sixth amendment splits it).
    ///
    /// Cached rather than rebuilt per frame, and re-merged only when one of
    /// its two halves says it moved ([`Self::lane_mark`]) — the merge is
    /// O(playlists), so this is thrift rather than necessity, but the
    /// contract is *no work per frame* and a cache that is only rebuilt on
    /// events is how that is kept true as the two halves grow.
    lane: crate::lane::Lane,
    /// The two stamps [`Self::lane`] was built from: the shelf's and the
    /// playlists'.
    lane_mark: (u64, u64),
    /// What [`Self::request_offscreen_art`] last asked for: the lane's stamps,
    /// the place, and the lane's first visible row, which together change
    /// exactly when one of the surfaces beside the wall changes what it draws.
    art_mark: ((u64, u64), Place, usize, bool),
    /// Whether a scan was running when the last message was answered — the
    /// falling edge is when the lists are re-read (see
    /// [`Self::sync_lists_with_the_library`]).
    was_scanning: bool,
    /// **The interrupted run** (ADR-0023 §6, `crate::session`): what was
    /// playing when baz was last closed, read once at launch.
    ///
    /// It is *held* rather than consumed, because the Home place's `CONTINUE`
    /// draws from it and `Resume` spends it — and because a snapshot the shell
    /// forgot the moment it restored the queue would leave nothing to say
    /// where in the track the listener actually was.
    resume: crate::session::Snapshot,
    /// What the run looked like when the snapshot was last written: the
    /// queue's length, the cursor, and the track sequence.
    ///
    /// Three integers rather than a path comparison, so "has the run moved?"
    /// is asked on every message and costs nothing between the moments it
    /// has — a track boundary, a queue replaced, a queue edited.
    written: (usize, Option<usize>, u64),
    /// Which section of the Settings place is showing (an index into
    /// `views::settings::SECTIONS`).
    ///
    /// Session state, like every other "where am I looking" answer in the shell
    /// and for `crate::panels`' reason: which section you last read is not a
    /// standing decision, so it is not in `config.toml`.
    settings_section: usize,
    /// The rolling RAM/CPU observer behind Settings → Debug.
    ///
    /// Session state, and **only alive while that section is visible**: its
    /// clock is installed by `add_place_clocks` under the same guard every
    /// other place-owned clock carries, and leaving the section resets it so
    /// that returning warms up again rather than dividing a fresh counter by
    /// however long the listener spent elsewhere (`crate::resource`).
    resource_meter: crate::resource::Meter,
    /// What that observer last said, or `None` before the first tick.
    resource_reading: Option<crate::resource::Reading>,
    /// When the meter was last sampled, so the rate divides by a real
    /// interval rather than by the timer's nominal one — a tick the event
    /// loop delivered late would otherwise read as a spike.
    resource_sampled: Option<Instant>,
    /// Settings → Appearance paste/import field; session-only until validated.
    theme_json: String,
    /// Exact result of the most recent local theme operation.
    theme_notice: Option<String>,
    /// The density the wall opens at, read from the config for the same reason
    /// and handed to the shelf the same way (ADR-0017 step 6).
    density: shelf::Density,
    /// Which modifier keys are down, as iced last reported them.
    ///
    /// The one piece of input state baz tracks itself, consulted only where
    /// iced 0.13 reports an input without its modifiers: `WheelScrolled`
    /// (so <kbd>Ctrl</kbd>+scroll cannot be told from a scroll without it)
    /// and a `button`'s `on_press` (so shift-click-queues-the-record,
    /// doc 09 §13 step 7, cannot be told from a click without it). Key
    /// *presses* never consult this — they carry their own modifiers, and
    /// [`keys::binding_for`] reads those (see its focus-rule note on why a
    /// hand-kept flag is the wrong instrument wherever the toolkit reports the
    /// truth itself).
    modifiers: keyboard::Modifiers,
}

enum Screen {
    Setup(Setup),
    /// **The library is there and baz will not open it** (ADR-0041): the
    /// downgrade, the corrupt file, the machine with nowhere to keep an index.
    /// Distinct from [`Screen::Setup`] because it answers a different
    /// question — see [`Blocked`].
    Blocked(Blocked),
    Shelf(Box<Shelf>),
}

/// Shared read for every track-row heart. Keeping it here prevents views from
/// inventing their own membership cache beside the durable library truth.
pub(crate) fn is_favourite(shelf: &Shelf, path: &Path) -> bool {
    shelf.library.is_favourite(path)
}

/// The minimal first-run screen: "Where's your music?".
pub(crate) struct Setup {
    /// What has been typed into the folder field.
    pub(crate) input: String,
    /// Why the last submission did not open a shelf, if it did not.
    pub(crate) error: Option<String>,
    /// Whether a file drag is over the window right now
    /// ([`Message::FileHovered`] — X11 only; see [`Message::FileDropped`]).
    /// The screen answers with one quiet line saying the drop will be taken.
    pub(crate) hovering_drop: bool,
}

/// **The blocked-library screen's state** — the one baz draws when the library
/// exists and this build will not open it (ADR-0041).
///
/// It exists because the shell used to answer *every* failure to open the
/// library by drawing [`Setup`], and the owner met the worst case of that on
/// 2026-08-10: he ran an older binary against his current library and baz
/// asked him *"where's your music?"*. His music was where he left it. baz had
/// correctly refused a database from a newer build
/// ([`IndexError::SchemaTooNew`]) and had then said the most alarming thing it
/// is capable of saying — *you have no library* — in the one case where
/// **nothing is wrong with the listener's data at all**.
///
/// The two screens answer two different questions, which is why one is not a
/// better sentence on the other:
///
/// | [`Setup`] | [`Blocked`] |
/// |---|---|
/// | *Where's your music?* — a **question** | *Here is what happened* — a **statement** |
/// | The listener has not answered yet | The listener answered; the answer is fine |
/// | Naming a folder is the fix | Naming a folder cannot help |
///
/// **One screen, three reasons** ([`Blockage`]), rather than three screens.
/// The shape is identical in all three — say what happened, say what is safe,
/// say what to do — and only the words and the available controls differ,
/// which is what "a different sentence" properly means. What the reasons do
/// *not* share is disposition: for [`Blockage::Unreadable`] a new index is the
/// repair, and for [`Blockage::NewerBaz`] it is the wrong move offered only
/// because refusing to offer anything would leave a listener with no way to
/// use baz at all.
pub(crate) struct Blocked {
    /// What happened, as a kind rather than as a sentence.
    pub(crate) why: Blockage,
    /// Where the library file is, when there is one to name — so the listener
    /// can find it with a file manager, and so the set-aside has something to
    /// move. `None` only when the system offered no data directory.
    pub(crate) db_path: Option<PathBuf>,
    /// The folders the shelf would have opened over. Kept so that `Try again`
    /// and the set-aside **finish the launch** rather than dropping the
    /// listener back at a first-run screen they have already been past.
    roots: Vec<PathBuf>,
    /// Whether the second door's statement of what a new index costs is
    /// showing.
    ///
    /// **The two-step is the whole safeguard.** The quiet word does not act;
    /// it reveals a paragraph naming what is lost and a second word that does.
    /// Nothing on this screen may rewrite the database on one press, and the
    /// press that does it is never the primary one.
    pub(crate) setting_aside: bool,
    /// What the last attempt to act said, when it failed — a retry that failed
    /// the same way, or a set-aside the filesystem refused.
    pub(crate) trouble: Option<String>,
}

/// **Why the library could not be opened**, in the three shapes the shell can
/// say something useful about.
///
/// Everything [`Library::open`] can fail with folds into these. The fold is
/// deliberately lossy in one direction only: the underlying words are always
/// carried through to the screen, so a case nobody anticipated is still
/// *reported* even though it is grouped under [`Self::Unreadable`].
pub(crate) enum Blockage {
    /// **The database was written by a newer baz.** The downgrade — a beta
    /// tester installing a release and then running an older build, which is
    /// the shape of trying something and going back.
    ///
    /// Nothing is wrong with the listener's data and nothing has been touched:
    /// `baz_core::index::Library::open` reads `user_version` before it sets a
    /// single pragma, and `a_too_new_database_is_refused_without_a_byte_being_written`
    /// asserts the file is unchanged across three refused opens.
    NewerBaz {
        /// The schema version the database declares. This build reads
        /// `baz_core::index::SCHEMA_VERSION`.
        found: i64,
    },
    /// **The file is there and this build cannot read it** — permissions, a
    /// corrupt page, a truncated write, a full disk. `detail` is the
    /// underlying error's own words, shown verbatim rather than paraphrased.
    Unreadable {
        /// What SQLite, or the index, actually said.
        detail: String,
    },
    /// **There is nowhere on this system to keep a library** — no data
    /// directory, or one that cannot be created. The only reason with no
    /// database behind it, and so the only one that offers no set-aside.
    Nowhere {
        /// What the platform said, or which directory could not be made.
        detail: String,
    },
}

impl Blockage {
    /// Read a `Library::open` failure. The newer-baz case is the one the shell
    /// has distinct words for; everything else is reported as itself.
    fn of(error: &IndexError) -> Self {
        match error {
            IndexError::SchemaTooNew { found } => Self::NewerBaz { found: *found },
            other => Self::Unreadable {
                detail: other.to_string(),
            },
        }
    }
}

impl Blocked {
    /// The screen, from a blockage and the folders the launch was carrying.
    ///
    /// Crate-visible so that `views::blocked`'s tests can build one, and
    /// deliberately **not** a test-only constructor. Several tests in
    /// `views` read this file's source and stop at its first test attribute —
    /// `every_place_that_hangs_works_hangs_them_on_one_grid` is one — so a
    /// gated helper up here would silently truncate what they can see. (That
    /// is not a hypothetical: adding one blinded that test, and it failed
    /// rather than passing vacuously, which is the design working.)
    pub(crate) fn new(why: Blockage, db_path: Option<PathBuf>, roots: Vec<PathBuf>) -> Self {
        Self {
            why,
            db_path,
            roots,
            setting_aside: false,
            trouble: None,
        }
    }

    /// **Whether there is a file to move out of the way.** The set-aside door
    /// is absent, not disabled, where there is nothing behind it (ADR-0028's
    /// rule, which this screen keeps): a machine with no data directory has no
    /// library to set aside, and neither has a `Nowhere`.
    pub(crate) fn can_set_aside(&self) -> bool {
        !matches!(self.why, Blockage::Nowhere { .. })
            && self.db_path.as_ref().is_some_and(|path| path.exists())
    }

    /// **Whether trying again could give a different answer.** A refusal on
    /// the schema version is deterministic — the same file, the same build,
    /// the same number — so `Try again` does not appear on it. A permission,
    /// a lock or a missing directory can all be fixed from outside baz while
    /// this screen is up, so there it does.
    pub(crate) fn can_retry(&self) -> bool {
        !matches!(self.why, Blockage::NewerBaz { .. })
    }
}

impl App {
    /// Validate and install the Settings paste field as a local custom theme.
    /// Selection is persisted only after the complete document is safe and on
    /// disk, so a failed import cannot strand the next launch.
    fn import_theme_json(&mut self) -> Task<Message> {
        match crate::theme_file::import(&self.theme_json) {
            Ok((selection, path)) => {
                let saved = selection.clone();
                persist(move |config| config.theme = saved);
                self.theme_notice = Some(format!(
                    "Imported {} and selected it for the next launch.",
                    path.display()
                ));
            }
            Err(error) => self.theme_notice = Some(error),
        }
        Task::none()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "a launch is one composition of independent restores — the \
                  engine, the library, the config's standing decisions, the \
                  run's snapshot — and each is three lines that only mean \
                  anything beside the others. It has crossed and re-crossed \
                  the limit as those decisions came and went; splitting it \
                  would name four functions after the order they happen to run"
    )]
    fn new(started: Instant, cli_dir: Option<PathBuf>) -> (Self, Task<Message>) {
        let stored = config::config_file().map(|path| config::load(&path));
        let configured_output = stored
            .as_ref()
            .and_then(|config| config.output_device.as_deref());
        let output_choice = OutputChoice::from_config(configured_output);
        let active_output_choice = output_choice.clone();
        let (output_choices, output_devices_error) =
            crate::playback::output_choices(configured_output);
        // Engine first: open failure must not kill the app — it becomes
        // Availability::NoDevice state that the bottom bar reports.
        let playback = Playback::start(configured_output);
        let availability = playback.availability();
        let mut player = PlayerState::new(availability.clone());
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
        let saved_volume = stored
            .as_ref()
            .map_or(Volume::UNITY, |config| config.volume);
        // Restore the fader's standing position as an engine command, never an
        // optimistic UI write. The confirming `VolumeChanged` is what moves
        // the player mirror; unity is already the engine default and needs no
        // round trip.
        if saved_volume != player.volume() {
            playback.send(Command::SetVolume {
                position: saved_volume.position(),
            });
        }
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
                crate::baz_log!("[history] recording to {}", ledger.path().display());
                playback.set_history(Some(Arc::clone(&ledger)));
                Some(ledger)
            }
            Err(error) => {
                crate::baz_log!("[history] not recording: {error}");
                None
            }
        };
        let group_key = stored
            .as_ref()
            .map_or(GroupKey::Artist, |config| config.group_key);
        let density = stored
            .as_ref()
            .map_or(shelf::Density::Balanced, |config| config.density);
        let lane_open = stored.as_ref().is_none_or(|config| config.sidebar_open);
        let saved_place = stored
            .as_ref()
            .map_or_else(Place::default, |config| config.last_place);
        let saved_visualization_foreground = stored
            .as_ref()
            .map_or(crate::visualizer::Foreground::JewelCase, |config| {
                config.visualization_foreground
            });
        // **The shuffle property, restored.** A standing decision
        // (`config::Config::shuffle`), seeded rather than assumed for
        // `seed_volume`'s reason: the control must be lit on the first frame,
        // not on the first press.
        //
        // The *mode* is what is persisted; the seed belongs to a run, so a
        // fresh one is rolled here rather than remembered. Two launches with
        // shuffle on are two different passes, which is what a listener means
        // by shuffle and what remembering a seed would quietly break.
        //
        // **Sent, not assumed**, on exactly the terms the ReplayGain settings
        // above are: the traversal is engine state now, so the config's
        // standing decision reaches it as a command and this process keeps a
        // mirror. `InOrder` is the engine's own default and is not sent, which
        // is the ordinary case and costs nothing.
        let standing = traversal(stored.as_ref().is_some_and(|config| config.shuffle));
        if standing != Traversal::InOrder {
            playback.send(Command::SetTraversal {
                traversal: standing,
            });
        }
        player.seed_traversal(standing);
        let repeat = stored
            .as_ref()
            .map_or(baz_core::protocol::Repeat::Off, |config| config.repeat);
        if repeat != baz_core::protocol::Repeat::Off {
            playback.send(Command::SetRepeat { repeat });
        }
        player.seed_repeat(repeat);
        let resume = read_snapshot();
        // The folders baz holds this run (ADR-0022): what the config remembers,
        // with a `baz DIR` argument **added to the front** rather than replacing
        // them. Pointing baz at a folder for an afternoon must not silently
        // forget the other three — and the one that was named on the command
        // line is the one being asked for, so it is scanned first.
        let mut dirs: Vec<PathBuf> = stored
            .as_ref()
            .map(|config| config.music_dirs.clone())
            .unwrap_or_default();
        if let Some(dir) = cli_dir {
            dirs.retain(|held| held != &dir);
            dirs.insert(0, dir);
        }
        let (screen, task) = if dirs.is_empty() {
            (Screen::Setup(Setup::fresh(None)), Task::none())
        } else {
            // **A library that will not open is no longer a first run**
            // (ADR-0041). This line used to read `Setup::fresh(Some(error))`,
            // which answered *"this library is from a newer baz"* by asking
            // *"where's your music?"* — the defect the owner reported.
            match Shelf::open(dirs.clone(), group_key, density, lane_open) {
                Ok((shelf, task)) => (Screen::Shelf(Box::new(shelf)), task),
                Err(why) => (
                    Screen::Blocked(Blocked::new(why, config::library_db_file(), dirs)),
                    Task::none(),
                ),
            }
        };
        let mut app = Self {
            _history_ledger: history_ledger,
            group_key,
            settings_section: 0,
            resource_meter: crate::resource::Meter::default(),
            resource_reading: None,
            resource_sampled: None,
            theme_json: String::new(),
            theme_notice: None,
            density,
            lane_open,
            lane: crate::lane::Lane::default(),
            lane_mark: (u64::MAX, u64::MAX),
            art_mark: ((u64::MAX, u64::MAX), Place::Settings, usize::MAX, false),
            was_scanning: true,
            resume: resume.clone(),
            written: (0, None, 0),
            modifiers: keyboard::Modifiers::empty(),
            started,
            last_interaction: Instant::now(),
            first_frame_logged: false,
            screen,
            place: Place::default(),
            place_history: PlaceHistory::new(Place::default()),
            hovered_queue_row: None,
            enqueue_next: crate::search::NextAnchor::default(),
            queue_scroll: 0.0,
            lane_scroll: 0.0,
            show_on_start: None,
            playlist_scroll: 0.0,
            playlists_scroll: 0.0,
            hovered_playlist_row: None,
            hovered_album_row: None,
            drag: None,
            queue_undo: crate::undo::History::new(),
            menu: None,
            status_open: false,
            playlists: crate::playlists::Playlists::start(),
            window: WINDOW,
            window_maximized: false,
            fullscreen: false,
            last_bar_press: None,
            case_rotation: crate::jewel_case::Rotation::new(Instant::now()),
            visualization: crate::visualizer::State {
                foreground: saved_visualization_foreground,
                facts: stored
                    .as_ref()
                    .is_none_or(|config| config.now_playing_facts),
                ..crate::visualizer::State::default()
            },
            visualization_history: crate::visualizer::History::default(),
            fact_index: 0,
            playback,
            output_choices,
            output_choice,
            active_output_choice,
            output_devices_error,
            player,
            signal_warning: SignalWarningState::default(),
            mpris,
            mpris_art: (0, None),
            ink: Keyed::new(),
            pressed_control: None,
            warmth: Tween::settled(0.0).with_curve(motion::Curve::Linear),
            saved_replay_gain,
            saved_volume,
            volume_wheel_settles: None,
        };
        if let Screen::Shelf(state) = &mut app.screen {
            if let crate::player::Availability::NoDevice(reason) = &availability {
                state.health.record(
                    crate::health::Level::Error,
                    "Audio output unavailable",
                    reason,
                );
            }
            if let Some(error) = &app.output_devices_error {
                state.health.record(
                    crate::health::Level::Warning,
                    "Could not list audio outputs",
                    error,
                );
            }
        }
        // **The run, handed back to the engine — silent.** `SetQueue` and
        // nothing else: it replaces the queue and starts nothing
        // (`baz_core::engine`'s command table), so the queue survives the quit
        // exactly as ADR-0023 §6 asks and **nothing sounds unasked**. The
        // cursor and the elapsed position stay in the snapshot, where
        // `CONTINUE`'s one press spends them — see `crate::session` for why
        // the engine cannot be handed a loaded-and-paused run at a non-zero
        // cursor without changing it, which §6 costed at zero.
        app.restore_the_run();
        // **The lists, re-read against the library** — once, here.
        // `Playlists::start` lists the folder before there is a library to
        // resolve entry paths against, so every sleeve came back empty; that
        // was invisible while the only surface showing them was a panel you
        // had to summon (and summoning refreshed them). The returns lane is
        // resident, so the first frame shows them, and a list wearing the
        // rest tile on launch and its collage after the first press would be
        // one object drawn two ways.
        if let Screen::Shelf(state) = &app.screen {
            app.playlists.refresh(Some(&state.library));
        }
        app.restore_place(saved_place);
        app.place_history = PlaceHistory::new(app.place);
        let artist_image = match app.place {
            Place::Artist(id) => app.request_artist_image(id),
            _ => Task::none(),
        };
        // **And the lists that were played in an earlier session** — the half
        // of the owner's defect that could not be fixed until the ledger
        // remembered which list a run came from. After the refresh, because it
        // credits rows the refresh has just listed.
        app.credit_the_lists_that_were_played();
        // One publish before the first frame, so a desktop widget that asks
        // straight away gets the seeded volume and the real `Can*` flags
        // rather than the server's own defaults. The MPRIS thread may not
        // have reached its bus yet; the update simply waits in its channel.
        app.publish_mpris(false);
        (app, Task::batch([task, artist_image]))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let now = Instant::now();
        if matches!(message, Message::RefreshTick)
            && now.duration_since(self.last_interaction) < REFRESH_IDLE
        {
            return Task::none();
        }
        if !matches!(
            message,
            Message::RefreshTick
                | Message::Playback(_)
                | Message::ThumbLoaded(..)
                | Message::HeroLoaded(..)
                | Message::ArtistImageLoaded(..)
                | Message::ScanTick
                | Message::FirstFrame
                | Message::MotionTick(_)
                | Message::CaseTick(_)
                | Message::VolumeWheelSettled(_)
                | Message::WindowFocused(_)
                | Message::WindowMaximizedChanged(_)
        ) {
            self.last_interaction = now;
        }
        let task = self.route(message);
        self.sync_visualization_tap();
        self.sync_lists_with_the_library();
        self.sync_snapshot();
        // **The lane, re-merged when — and only when — one of its two halves
        // says it moved**, and *after* the message rather than before it:
        // iced draws the frame this call produced, so a sync that ran first
        // would leave the lane one message behind whatever it describes.
        self.sync_lane();
        // **The art the surfaces beside the wall need.** The wall's own
        // prefetch is a range over the wall and answers nothing about a
        // record drawn next to it, so the lane's rows and Home's newest row
        // ask for their own — through the same cache, so a sleeve is one
        // decode however many surfaces draw it.
        let art = self.request_hero();
        let artist_image = match self.place {
            Place::Artist(id) => self.request_artist_image(id),
            _ => Task::none(),
        };
        // Queue rows now wear the saved playlist page's artwork and Album
        // cells. Ask for the visible slice after every queue message, exactly
        // as the saved page does on its scroll callback; the cache deduplicates
        // already-resident handles.
        let playlist_art = if self.place == Place::Queue {
            self.request_playlist_art()
        } else {
            Task::none()
        };
        // **And what the Now playing place draws of the record**, settled
        // after the ask rather than before it: the two things that can change
        // that surface's picture are the engine naming another record and a
        // hero landing, and both have already happened by here
        // ([`Shelf::settle_art`]).
        self.settle_art();
        Task::batch([
            task,
            self.request_offscreen_art(),
            art,
            artist_image,
            playlist_art,
        ])
    }

    /// Hand the sounding record to [`Shelf::settle_art`], which owns the whole
    /// of the crossfade's decision.
    ///
    /// The split is [`Self::request_hero`]'s: the shell knows what is sounding
    /// and the shelf knows what is decoded, and neither reaches into the other.
    fn settle_art(&mut self) {
        let sounding = self.player.playing_album();
        // **The transition belongs to a surface, so it runs only where that
        // surface is drawn.** The *commitment* is unconditional — arriving at
        // the place must find the right picture, whenever the record changed —
        // but a tween is a clock, and a clock spent easing a hero nobody is
        // looking at would redraw whatever place *is* on screen twelve times
        // for nothing. That is the one cost ADR-0020's argument does not
        // license, and it is the owner's standing rule about responsiveness.
        //
        // A `None` foreground is also not watching artwork: no invisible
        // dissolve is allowed to keep the bounded motion clock alive.
        let watching = self.place == Place::NowPlaying && self.visualization.foreground.draws_art();
        if let Screen::Shelf(state) = &mut self.screen {
            state.settle_art(sounding, watching, Instant::now());
        }
    }

    /// Everything [`Self::update`] does except keep the lane true — the update
    /// loop proper, split out so that the one thing that must happen after
    /// every message can be one line rather than an arm in each of forty.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per message that is not already routed to a \
                  sub-machine above; the routing table is clearest read whole"
    )]
    fn route(&mut self, message: Message) -> Task<Message> {
        note_message(&message);
        // The volume is its own small machine and every one of its messages
        // resolves to "tell the state machine, maybe tell the engine", so it
        // is answered first and separately rather than as nine more arms
        // below.
        // The two machines that answer *before* anything else can: ink, which
        // cannot move a pixel of layout, and the modifier layer, which decides
        // whether a keystroke was even text.
        for machine in [
            Self::update_lane,
            Self::update_menu,
            Self::update_case,
            Self::update_motion,
            Self::update_modified_input,
            Self::update_vibe,
            Self::update_playlists,
            Self::update_drag,
        ] {
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
            Message::HistoryBack => self.travel_history(true),
            Message::HistoryForward => self.travel_history(false),
            Message::DismissSearch => match &mut self.screen {
                Screen::Shelf(state) => state.clear_query(),
                Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
            },
            Message::Direction(direction) => self.direction(direction),
            Message::SearchConfirmed => self.confirm_search(),
            Message::SearchAction(content, action) => self.search_action(content, action),
            // **The doors, and the one way back.** Every one of them is
            // navigation and nothing else: no panel opens, no width changes,
            // and the Library's own state — scroll, query, arrangement — is
            // untouched by all of them, which is what makes coming back free.
            Message::ToggleSettings => self.go(Place::settings),
            Message::ContentPressed(content) => self.press_content(content),
            // **Shift-click a sleeve queues the record** — the one-press
            // accelerator over the picker's Queue row (ADR-0023 §3's stack;
            // doc 09 §13 step 7). Explicit Open routes arrive here directly;
            // ordinary tile presses are handled by `press_content` above.
            Message::AlbumClicked(id) => {
                if self.modifiers.shift() {
                    self.queue_album(id)
                } else {
                    self.open_album(id)
                }
            }
            // The wall's hover `Queue` option: the shift-click gesture's own
            // append, reached by a named control instead of a held key.
            Message::QueueAlbum(id) => self.queue_album(id),
            // **The album page's breadcrumb**: up to the artist. Subject
            // routes are idempotent, so a repeated pointer event stays put.
            Message::OpenArtist(id) => Task::batch([
                self.go(|place| place.artist(id)),
                self.request_artist_image(id),
            ]),
            Message::LookUpArtist(id) => {
                let Some(name) = (match &self.screen {
                    Screen::Shelf(state) => views::artist::label(state, id).map(str::to_owned),
                    Screen::Setup(_) | Screen::Blocked(_) => None,
                }) else {
                    return Task::none();
                };
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || crate::desktop::look_up_artist(&name))
                            .await
                            .unwrap_or_else(|error| Err(error.to_string()))
                    },
                    Message::ArtistLookUpFinished,
                )
            }
            Message::ArtistLookUpFinished(result) => {
                if let Err(error) = result
                    && let Screen::Shelf(state) = &mut self.screen
                {
                    state.health.record(
                        crate::health::Level::Warning,
                        "Could not open the browser",
                        error,
                    );
                }
                Task::none()
            }
            Message::ShowNowPlaying => {
                self.go(|place| place.go(crate::lane::Destination::NowPlaying))
            }
            Message::ToggleStatus => {
                if self.status_open
                    && let Screen::Shelf(state) = &mut self.screen
                {
                    state.health.acknowledge();
                }
                self.status_open = !self.status_open;
                self.menu = None;
                Task::none()
            }
            Message::RetryHealth => {
                if let Screen::Shelf(state) = &mut self.screen
                    && !state.scanning
                {
                    state.start_scan(scan::ScanMode::Incremental);
                }
                Task::none()
            }
            Message::CloseStatus => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.health.acknowledge();
                }
                self.status_open = false;
                Task::none()
            }
            Message::OpenPlayingSource => self.open_playing_source(),
            Message::ShowQueue => self.go(|_| Place::Queue),
            Message::OpenAlbum(id) => self.open_album(id),
            // The Settings place's spine. Session state and deliberately not
            // persisted: which section you were last reading is not a standing
            // decision.
            Message::SettingsSection(section) => {
                self.settings_section = section;
                // Leaving Debug ends the reading rather than freezing it: a
                // stale figure redrawn on return would be a measurement of a
                // moment nobody asked about.
                if section != views::settings::DEBUG_SECTION {
                    self.resource_meter.reset();
                    self.resource_reading = None;
                    self.resource_sampled = None;
                }
                Task::none()
            }
            // The Debug section's own clock, and nothing else's.
            Message::ResourceTick(now) => {
                if let Some(sample) = crate::resource::sample() {
                    let interval = self
                        .resource_sampled
                        .map_or(Duration::ZERO, |was| now.duration_since(was));
                    self.resource_sampled = Some(now);
                    self.resource_reading = Some(self.resource_meter.observe(sample, interval));
                } else {
                    self.resource_reading = Some(crate::resource::Reading::Unavailable);
                }
                Task::none()
            }
            Message::OutputDeviceSelected(choice) => {
                if choice == self.output_choice {
                    return Task::none();
                }
                let configured = choice.device().map(str::to_owned);
                let label = choice.to_string();
                self.output_choice = choice;
                persist(move |config| config.output_device = configured);
                if let Screen::Shelf(state) = &mut self.screen {
                    state.health.record(
                        crate::health::Level::Ready,
                        "Audio output changed",
                        format!("{label} will be used the next time baz starts."),
                    );
                }
                Task::none()
            }
            Message::VibeWorkers(workers) => {
                let workers = workers.clamp(1, config::MAX_VIBE_WORKERS);
                persist(move |config| config.vibe_workers = workers);
                Task::none()
            }
            Message::NewPlaylistOpen => self.open_playlist_creation(None),
            Message::NewPlaylistOpenVibe => {
                self.open_playlist_creation(Some(crate::playlists::CreationMode::Vibe))
            }
            Message::PlaylistCreationMode(mode) => {
                self.playlists.creation.mode = Some(mode);
                self.playlists.creation.error = None;
                if mode == crate::playlists::CreationMode::Manual
                    && self.playlists.creation.name.is_empty()
                {
                    self.playlists.creation.name_is_suggested = false;
                }
                if mode == crate::playlists::CreationMode::Vibe {
                    let prompt = match &self.screen {
                        Screen::Shelf(state) => state.vibe.prompt.clone(),
                        Screen::Setup(_) | Screen::Blocked(_) => String::new(),
                    };
                    self.playlists.suggest_creation_name(&prompt);
                }
                Task::none()
            }
            Message::PlaylistCreationBack => {
                self.playlists.creation.mode = None;
                Task::none()
            }
            Message::PlaylistCreationName(name) => {
                self.playlists.creation.name = name.chars().take(96).collect();
                self.playlists.creation.name_is_suggested = false;
                self.playlists.creation.error = None;
                Task::none()
            }
            Message::PlaylistCreationExample(example) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.set_prompt(example);
                }
                self.playlists.suggest_creation_name(example);
                Task::none()
            }
            Message::PlaylistCreationRemove(index) => {
                if index < self.playlists.creation.items.len() {
                    self.playlists.creation.items.remove(index);
                }
                Task::none()
            }
            Message::PlaylistCreationShift(index, delta) => {
                let neighbour = match delta {
                    value if value < 0 => index.checked_sub(1),
                    value if value > 0 => index.checked_add(1),
                    _ => None,
                };
                if let Some(neighbour) =
                    neighbour.filter(|neighbour| *neighbour < self.playlists.creation.items.len())
                {
                    self.playlists.creation.items.swap(index, neighbour);
                }
                Task::none()
            }
            Message::PlaylistCreationSave => self.save_playlist_creation(),
            Message::ThemeSelected(selection) => {
                persist(move |config| selection.clone_into(&mut config.theme));
                self.theme_notice = Some(format!(
                    "{} will be used the next time baz starts.",
                    crate::theme_file::preview(selection)
                        .map_or_else(|_| selection.to_owned(), |preview| preview.name)
                ));
                Task::none()
            }
            Message::ThemeJsonChanged(text) => {
                self.theme_json = text;
                self.theme_notice = None;
                Task::none()
            }
            Message::ThemeLoadTemplate => {
                self.theme_json = crate::theme_file::template();
                self.theme_notice = Some(
                    "Template loaded below; edit its id, name and colours, then import it."
                        .to_owned(),
                );
                Task::none()
            }
            Message::ThemeImportPasted => self.import_theme_json(),
            Message::ThemePickFile => pick_theme_file(),
            Message::ThemeFilePicked(Ok(text)) => {
                self.theme_json = text;
                self.import_theme_json()
            }
            Message::ThemeFilePicked(Err(error)) => {
                self.theme_notice = Some(error);
                Task::none()
            }
            Message::ThemeExport => {
                let selection = config::config_file().map_or_else(
                    || crate::theme_file::DEFAULT_SELECTION.to_owned(),
                    |path| config::load(&path).theme,
                );
                export_theme(selection)
            }
            Message::ThemeExported(result) => {
                self.theme_notice = Some(match result {
                    Ok(path) => format!("Theme exported to {}.", path.display()),
                    Err(error) => error,
                });
                Task::none()
            }
            Message::WindowResized(size) => {
                let playlist_before = match self.place {
                    Place::Playlist(_) => Some((
                        true,
                        views::playlist_page::layout(self.body_width()).side_by_side(),
                        self.playlist_scroll,
                    )),
                    Place::Queue => Some((
                        false,
                        views::playlist_page::layout(self.body_width()).side_by_side(),
                        self.queue_scroll,
                    )),
                    _ => None,
                };
                self.window = size;
                self.art_mark = ((u64::MAX, u64::MAX), Place::Settings, usize::MAX, false);
                let laid_out = match &mut self.screen {
                    Screen::Shelf(state) => state.update(Message::WindowResized(size)),
                    Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
                };
                let restore_playlist =
                    playlist_before.map_or_else(Task::none, |(saved, before, scroll)| {
                        let after = views::playlist_page::layout(self.body_width()).side_by_side();
                        let y = views::playlist_page::reflow_scroll_offset(scroll, before, after);
                        if saved {
                            self.playlist_scroll = y;
                        } else {
                            self.queue_scroll = y;
                        }
                        let restore = iced::widget::operation::scroll_to(
                            views::page::scroll_id(),
                            AbsoluteOffset { x: 0.0, y },
                        );
                        if saved {
                            Task::batch([restore, self.request_playlist_art()])
                        } else {
                            restore
                        }
                    });
                // A maximise and an unmaximise are both resizes, and iced 0.13
                // publishes no event for either — so the state the app bar's
                // button draws is asked for here (`Message::WindowMaximizedChanged`).
                Task::batch([
                    laid_out,
                    restore_playlist,
                    latest_window(window::is_maximized).map(Message::WindowMaximizedChanged),
                ])
            }
            Message::FirstFrame => self.log_first_frame(),
            Message::SetupSubmit => self.submit_setup(),
            Message::Playback(event) => self.apply_player_event(event),
            Message::PlayAlbum(id) => {
                if self.play_album(id) {
                    self.complete_search_launch()
                } else {
                    Task::none()
                }
            }
            // Enter confirms the chooser's explicit selection/action.
            Message::PlayFirstMatch => self.play_first_match(),
            Message::PlayTrack(id, row) => {
                let searching = matches!(&self.screen, Screen::Shelf(state) if state.search_open);
                if self.play_track(id, row) && searching {
                    self.show_current_run_on_start();
                    self.complete_search_launch()
                } else {
                    Task::none()
                }
            }
            Message::ToggleFavourite(path) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    if let Err(error) = state.library.toggle_favourite(&path) {
                        state.health.record(
                            crate::health::Level::Error,
                            "Could not update Favourites",
                            error.to_string(),
                        );
                    }
                    self.playlists.refresh(Some(&state.library));
                }
                self.request_playlist_art()
            }
            Message::FavouritesPlay => {
                self.play_favourites(None);
                Task::none()
            }
            Message::FavouritesPlayTrack(row) => {
                self.play_favourites(Some(row));
                Task::none()
            }
            Message::FavouritesScrolled(viewport) => {
                self.playlist_scroll = viewport.absolute_offset().y;
                self.request_playlist_art()
            }
            Message::ShowAllSongs => {
                // The panel closes with the press, exactly as picking a
                // destination closes it: a panel that stayed open over the
                // place it just sent you to would be a float with no subject.
                self.playlists.close_panel();
                self.go(Place::back)
            }
            Message::ToggleShuffle => {
                self.toggle_shuffle();
                Task::none()
            }
            Message::CycleRepeat => {
                self.cycle_repeat();
                Task::none()
            }
            Message::PlayEverything => {
                self.play_everything();
                Task::none()
            }
            Message::PlayArtistSongs(id) => {
                self.play_artist_songs(id);
                Task::none()
            }
            Message::AllSongsHovered(over) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.hovered_all_songs = over;
                }
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
            Message::FocusSearch => self.focus_the_well(),
            Message::QueryTyped(text) => self.type_anywhere(&text),
            Message::Quit => self.leave_for_good(),
            // **The three window controls.** Each is the platform's own
            // action, spent through iced's own task — baz does not
            // reimplement any of them, which is the whole argument for
            // drawing the bar rather than the behaviour.
            Message::WindowMinimised => latest_window(|id| window::minimize(id, true)),
            Message::WindowMaximiseToggled => latest_window(window::toggle_maximize),
            Message::ToggleFullscreen => latest_window(window::mode).map(Message::WindowModeRead),
            Message::WindowModeRead(mode) => {
                let target = fullscreen_target(mode);
                self.fullscreen = target == window::Mode::Fullscreen;
                latest_window(move |id| window::set_mode(id, target))
            }
            Message::OpenPlaylistsFolder => {
                if let Some(path) = self.playlists.folder_path()
                    && let Err(error) = crate::desktop::open_folder(path)
                    && let Screen::Shelf(state) = &mut self.screen
                {
                    state.health.record(
                        crate::health::Level::Error,
                        "Could not open playlists folder",
                        error,
                    );
                }
                Task::none()
            }
            Message::WindowResize(direction) => {
                latest_window(move |id| window::drag_resize(id, direction))
            }
            // **Move, or maximise on the second press.** The gesture a title
            // bar makes: press and travel moves the window, press twice in
            // place toggles the maximised state. The first press still starts
            // an interactive move — it has to, because the compositor owns the
            // gesture from the moment the button goes down and there is no way
            // to know yet whether a second press is coming — and a move that
            // travels nowhere costs nothing, which is why the double press can
            // simply act on top of it.
            Message::WindowDragged => {
                let now = Instant::now();
                let doubled = self
                    .last_bar_press
                    .is_some_and(|last| now.duration_since(last) <= BAR_DOUBLE_CLICK);
                // Cleared on the double so that three presses are one double
                // and one single, rather than two overlapping doubles.
                self.last_bar_press = (!doubled).then_some(now);
                if doubled {
                    latest_window(window::toggle_maximize)
                } else {
                    latest_window(window::drag)
                }
            }
            Message::WindowMenuRequested => latest_window(window::show_system_menu),
            Message::WindowMaximizedChanged(maximized) => {
                self.window_maximized = maximized;
                Task::none()
            }
            // Best effort by nature: a Wayland compositor is entitled to
            // refuse a focus request, and refusing is not an error here.
            Message::Raise => latest_window(window::gain_focus),
            Message::Undo => self.undo_edit(),
            message @ Message::Scrolled(_) => {
                // The page is about to admit a new row of thumbnails. Ask the
                // lane for its visible rows again first so their cache entries
                // become recent and page scrolling evicts offscreen page art,
                // not persistent chrome.
                self.art_mark = ((u64::MAX, u64::MAX), Place::Settings, usize::MAX, false);
                match &mut self.screen {
                    Screen::Shelf(state) => state.update(message),
                    Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
                }
            }
            message if matches!(self.screen, Screen::Setup(_)) => self.update_setup(message),
            message if matches!(self.screen, Screen::Blocked(_)) => self.update_blocked(&message),
            message => match &mut self.screen {
                Screen::Shelf(state) => state.update(message),
                Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
            },
        }
    }

    /// The first-run screen's own messages: the typed field, the `Browse…`
    /// picker, and the drop target (doc 11 §5 P1). Reached only while the
    /// screen *is* the setup screen — the same messages over a shelf belong
    /// to the Settings place's folder machine.
    ///
    /// All three doors converge on [`Self::open_first_shelf`], and the two
    /// that name an unvetted path — the typed field and the drop — go
    /// through [`check_folder`] on the blocking pool first (ADR-0025's NAS
    /// honesty, now on the first door too: a dead mount's `stat` waits for
    /// minutes, and the first frame must never wait with it). A picked
    /// folder skips the stat for the Settings door's own reason: the dialog
    /// walked the real filesystem to offer it.
    fn update_setup(&mut self, message: Message) -> Task<Message> {
        let Screen::Setup(setup) = &mut self.screen else {
            return Task::none();
        };
        match message {
            Message::SetupInput(value) => {
                setup.input = value;
                Task::none()
            }
            Message::PickMusicFolder => pick_folder(),
            // A picked folder opens without a fresh stat (the dialog walked
            // the real filesystem to offer it); a checked path arrives
            // already vetted by the pool.
            Message::MusicFolderPicked(Some(dir)) | Message::MusicFolderChecked(Ok(dir)) => {
                self.open_first_shelf(dir)
            }
            Message::MusicFolderChecked(Err(words)) => {
                setup.error = Some(words);
                Task::none()
            }
            Message::FileDropped(path) => {
                setup.hovering_drop = false;
                setup.error = None;
                // The dropped path lands in the field too, so whatever the
                // check says is said about something the listener can see —
                // and correct by typing, if a file was dropped where its
                // folder was meant.
                if let Some(text) = path.to_str() {
                    text.clone_into(&mut setup.input);
                }
                Task::perform(check_folder(path), Message::MusicFolderChecked)
            }
            Message::FileHovered => {
                setup.hovering_drop = true;
                Task::none()
            }
            Message::FileHoverLeft => {
                setup.hovering_drop = false;
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// Setup → Shelf: open the very first shelf over `dir`. The one seam all
    /// three first-run doors — typed, picked, dropped — converge on.
    ///
    /// **A folder named here cannot fix a library that will not open**, so a
    /// failure leaves this screen rather than annotating it (ADR-0041). The
    /// first-run screen keeps its error line for the one thing it *can* fix —
    /// a path that is not a folder ([`Message::MusicFolderChecked`]) — and
    /// hands everything else to [`Screen::Blocked`].
    ///
    /// This is the exact loop the owner was in: the screen told him the schema
    /// version *"if I pick any directory"*, because every directory he picked
    /// went straight back into the same refusal.
    fn open_first_shelf(&mut self, dir: PathBuf) -> Task<Message> {
        if !matches!(self.screen, Screen::Setup(_)) {
            return Task::none();
        }
        match Shelf::open(
            vec![dir.clone()],
            self.group_key,
            self.density,
            self.lane_open,
        ) {
            Ok((state, task)) => {
                self.screen = Screen::Shelf(Box::new(state));
                task
            }
            Err(why) => {
                self.screen =
                    Screen::Blocked(Blocked::new(why, config::library_db_file(), vec![dir]));
                Task::none()
            }
        }
    }

    /// **The blocked screen's own messages**, and nothing else reaches it.
    ///
    /// `Try again` re-runs the identical launch — same folders, same
    /// everything — so a permission fixed in another window, or a lock that
    /// has gone, finishes the launch instead of restarting the application.
    /// The set-aside is two presses by construction: the first only *reveals*
    /// what a new index costs.
    fn update_blocked(&mut self, message: &Message) -> Task<Message> {
        let Screen::Blocked(blocked) = &mut self.screen else {
            return Task::none();
        };
        match message {
            Message::LibraryRetry => {
                blocked.trouble = None;
                let roots = blocked.roots.clone();
                match Shelf::open(roots, self.group_key, self.density, self.lane_open) {
                    Ok((state, task)) => {
                        crate::baz_log!("[library] retry opened the library");
                        self.screen = Screen::Shelf(Box::new(state));
                        task
                    }
                    Err(why) => {
                        // The **reason** is replaced too, not only the words:
                        // a disk that came back and a database that turned out
                        // to be from a newer baz are different screens, and a
                        // retry is exactly when that can change.
                        let roots = std::mem::take(&mut blocked.roots);
                        let mut again = Blocked::new(why, config::library_db_file(), roots);
                        again.trouble = Some("Still the same answer.".to_owned());
                        self.screen = Screen::Blocked(again);
                        Task::none()
                    }
                }
            }
            Message::LibrarySetAsideAsked(showing) => {
                blocked.setting_aside = *showing;
                blocked.trouble = None;
                Task::none()
            }
            Message::LibrarySetAside => self.set_the_library_aside(),
            _ => Task::none(),
        }
    }

    /// **Move the library out of the way and finish the launch.**
    ///
    /// The second press of the two-step, and the only thing in baz that
    /// touches a database this build has refused to read. It **renames**;
    /// it does not delete and it does not rewrite (`baz_core::index::set_aside`
    /// and its round-trip test), so the sentence the screen shows above this
    /// press — *nothing is deleted, renaming it back restores it exactly* —
    /// is a property of the code rather than a reassurance.
    fn set_the_library_aside(&mut self) -> Task<Message> {
        let Screen::Blocked(blocked) = &mut self.screen else {
            return Task::none();
        };
        let Some(db_path) = blocked.db_path.clone() else {
            return Task::none();
        };
        let aside = match baz_core::index::set_aside(&db_path) {
            Ok(aside) => aside,
            Err(error) => {
                blocked.trouble = Some(format!("Could not move the library: {error}"));
                return Task::none();
            }
        };
        crate::baz_log!("[library] set aside to {}", aside.display());
        let roots = std::mem::take(&mut blocked.roots);
        match Shelf::open(roots.clone(), self.group_key, self.density, self.lane_open) {
            Ok((state, task)) => {
                self.screen = Screen::Shelf(Box::new(state));
                task
            }
            // The file moved and the new index still would not open. Say so
            // with the fresh reason and name where the old library went, so
            // nobody has to guess whether it survived.
            Err(why) => {
                let mut again = Blocked::new(why, config::library_db_file(), roots);
                again.trouble = Some(format!(
                    "The old library is safe at {} — but the new one would not open either.",
                    aside.display()
                ));
                self.screen = Screen::Blocked(again);
                Task::none()
            }
        }
    }

    /// <kbd>Enter</kbd>: confirm the open search chooser, otherwise activate
    /// the current shared content selection. The older query fall-through is
    /// retained defensively for a restored state that predates the dropover.
    ///
    /// Resolved on the shell because playing is the shell's job and the answer
    /// is the shelf's — the same split every other play route in this file
    /// takes. [`Shelf::enter_drops_needle`] and [`Shelf::enter_plays`] hold
    /// the choice; this holds the sound.
    ///
    /// The song path is [`Self::play_track`] — the record page's own needle
    /// drop, `SetQueue` (selected edition, whole, in order) + `JumpTo`
    /// through [`PlayerState::play_from`]'s decision — so <kbd>Enter</kbd> is
    /// exactly a press on the selected track result, not a third play
    /// grammar.
    fn play_first_match(&mut self) -> Task<Message> {
        if matches!(&self.screen, Screen::Shelf(state) if state.search_open) {
            return self.confirm_search();
        }
        let query_stands = matches!(
            &self.screen,
            Screen::Shelf(state) if !state.query.trim().is_empty()
        );
        if !query_stands {
            let selected = match &self.screen {
                Screen::Shelf(state) => state.selection.selected(),
                Screen::Setup(_) | Screen::Blocked(_) => None,
            };
            return selected.map_or_else(Task::none, |content| self.activate_content(content));
        }
        let selected = match &self.screen {
            Screen::Shelf(state) => state.selected_search_track(),
            Screen::Setup(_) | Screen::Blocked(_) => None,
        };
        if let Some(content) = selected {
            return self.activate_content(content);
        }
        let needle = match &self.screen {
            Screen::Shelf(state) => state.enter_drops_needle(),
            Screen::Setup(_) | Screen::Blocked(_) => None,
        };
        if let Some((id, row)) = needle {
            if self.play_track(id, row) {
                self.show_current_run_on_start();
                return self.complete_search_launch();
            }
            return Task::none();
        }
        let album = match &self.screen {
            Screen::Shelf(state) => state.enter_plays(),
            Screen::Setup(_) | Screen::Blocked(_) => None,
        };
        if let Some(id) = album {
            return if self.play_album(id) {
                self.complete_search_launch()
            } else {
                Task::none()
            };
        }
        Task::none()
    }

    /// One click selects; the second click on the same playable object inside
    /// the shared interval activates. Shift-click on an album retains its
    /// established explicit Queue accelerator.
    fn press_content(&mut self, content: Content) -> Task<Message> {
        if let Content::Album(id) = content
            && self.modifiers.shift()
        {
            return self.queue_album(id);
        }
        let press = match &mut self.screen {
            Screen::Shelf(state) => {
                let is_search_result =
                    state.search_open && state.search_result_index(content).is_some();
                if is_search_result && matches!(content, Content::SearchTrack { .. }) {
                    state.search_action = crate::search::Action::Play;
                }
                if !is_search_result && matches!(content, Content::Album(_)) {
                    state.cover_action = CoverAction::Play;
                }
                if is_search_result {
                    state.search_selection.press(content, Instant::now())
                } else {
                    state.selection.press(content, Instant::now())
                }
            }
            Screen::Setup(_) | Screen::Blocked(_) => return Task::none(),
        };
        match press {
            Press::Selected => Task::none(),
            Press::Activated => self.activate_content(content),
        }
    }

    /// Spend an activation through the existing play/jump paths. Labelled
    /// Play controls keep sending those direct messages and bypass timing.
    fn activate_content(&mut self, content: Content) -> Task<Message> {
        match content {
            Content::Album(id) => {
                let action = match &self.screen {
                    Screen::Shelf(state) => state.cover_action,
                    Screen::Setup(_) | Screen::Blocked(_) => CoverAction::Play,
                };
                match action {
                    CoverAction::Play if self.play_album(id) => self.complete_search_launch(),
                    CoverAction::Play => Task::none(),
                    CoverAction::Queue => self.queue_album(id),
                    CoverAction::Open => self.open_album(id),
                }
            }
            Content::Playlist(id) => {
                if id == crate::playlists::FAVOURITES_ID {
                    return self.go(|_| Place::Favourites);
                }
                let opened = match &self.screen {
                    Screen::Shelf(state) => self.playlists.open_page(id, &state.library),
                    Screen::Setup(_) | Screen::Blocked(_) => false,
                };
                if opened {
                    self.play_playlist();
                }
                Task::none()
            }
            Content::AllSongs => {
                self.play_everything();
                Task::none()
            }
            Content::ArtistSongs(id) => {
                self.play_artist_songs(id);
                Task::none()
            }
            Content::AlbumTrack { album, row } => {
                self.play_track(album, row);
                Task::none()
            }
            Content::SearchTrack { album, row } => {
                if self.play_track(album, row) {
                    self.show_current_run_on_start();
                    self.complete_search_launch()
                } else {
                    Task::none()
                }
            }
            Content::PlaylistTrack { playlist, row } if self.place == Place::Playlist(playlist) => {
                self.play_playlist_track(row);
                Task::none()
            }
            Content::QueueTrack(row) if self.place == Place::Queue => {
                self.jump_to_queued(row);
                Task::none()
            }
            Content::PlaylistTrack { .. } | Content::QueueTrack(_) => Task::none(),
        }
    }

    /// Complete a play gesture made on the app-wide search results.
    ///
    /// Search is a way to reach music, not a mode the listener should have to
    /// dismiss after an accepted request. A result press therefore clears and
    /// blurs the query immediately; [`Self::apply_player_event`] moves to Now
    /// Playing only when the engine confirms a matching track actually began.
    fn complete_search_launch(&mut self) -> Task<Message> {
        match &mut self.screen {
            Screen::Shelf(state) if !state.query.trim().is_empty() => state.clear_query(),
            Screen::Shelf(_) | Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
        }
    }

    /// Put exactly one search answer in the list the current place presents.
    /// On a saved playlist page that means the playlist file; everywhere else
    /// it means the live run. Neither route starts playback.
    fn enqueue_search_track(
        &mut self,
        album: u64,
        row: usize,
        position: crate::search::Action,
    ) -> Task<Message> {
        let item = match &self.screen {
            Screen::Shelf(state) => state
                .albums
                .iter()
                .find(|record| record.id == album)
                .and_then(|record| {
                    let queue = vm::album_queue(record, state.edition_choice.get(&album).copied());
                    queue.items.get(row).cloned()
                }),
            Screen::Setup(_) | Screen::Blocked(_) => None,
        };
        if let Some(item) = item {
            if self.place == Place::NewPlaylist
                && self.playlists.creation.mode == Some(crate::playlists::CreationMode::Manual)
            {
                if !self
                    .playlists
                    .creation
                    .items
                    .iter()
                    .any(|held| held.path == item.path)
                {
                    self.playlists.creation.items.push(item);
                }
                self.enqueue_next.clear();
            } else if let Place::Playlist(id) = self.place {
                if let Screen::Shelf(state) = &self.screen {
                    let entries = crate::playlists::entries_for_items(std::slice::from_ref(&item));
                    self.playlists.append(id, entries, &state.library);
                }
                self.enqueue_next.clear();
            } else if position == crate::search::Action::Next {
                self.insert_items_next(vec![item]);
            } else {
                self.append_items_to_run(vec![item]);
            }
        }
        Task::none()
    }

    fn search_action(&mut self, content: Content, action: crate::search::Action) -> Task<Message> {
        if let Screen::Shelf(state) = &mut self.screen {
            state.search_selection.select(content);
            state.search_action = action;
        }
        match (content, action) {
            (
                Content::SearchTrack { album, row },
                crate::search::Action::Next | crate::search::Action::End,
            ) => self.enqueue_search_track(album, row, action),
            (Content::Album(id), crate::search::Action::End) => {
                let clear = match &mut self.screen {
                    Screen::Shelf(state) => state.clear_query(),
                    Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
                };
                Task::batch([clear, self.open_album(id)])
            }
            (Content::Album(id), crate::search::Action::Play) => {
                if self.play_album(id) {
                    self.complete_search_launch()
                } else {
                    Task::none()
                }
            }
            (_, crate::search::Action::Play) => self.activate_content(content),
            (_, crate::search::Action::Next | crate::search::Action::End) => Task::none(),
        }
    }

    fn confirm_search(&mut self) -> Task<Message> {
        let choice = match &self.screen {
            Screen::Shelf(state) if state.search_open => state
                .search_selection
                .selected()
                .and_then(|content| state.search_result_index(content).map(|_| content))
                .map(|content| (content, state.search_action)),
            Screen::Shelf(_) | Screen::Setup(_) | Screen::Blocked(_) => None,
        };
        choice.map_or_else(Task::none, |(content, action)| {
            self.search_action(content, action)
        })
    }

    /// Give bare arrows to the open search chooser and retain their existing
    /// volume/seek meaning everywhere else. The open chooser's raw-event seam
    /// deliberately delivers Left/Right even while the query field owns the
    /// caret, then this blur makes subsequent arrows unambiguous.
    fn direction(&mut self, direction: crate::search::Direction) -> Task<Message> {
        let searching = matches!(&self.screen, Screen::Shelf(state) if state.search_open);
        if searching {
            return match direction {
                crate::search::Direction::Up => match &mut self.screen {
                    Screen::Shelf(state) => state.move_search_selection(-1),
                    Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
                },
                crate::search::Direction::Down => match &mut self.screen {
                    Screen::Shelf(state) => state.move_search_selection(1),
                    Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
                },
                crate::search::Direction::Left | crate::search::Direction::Right => {
                    if let Screen::Shelf(state) = &mut self.screen {
                        let delta = if direction == crate::search::Direction::Left {
                            -1
                        } else {
                            1
                        };
                        match state.search_selection.selected() {
                            Some(Content::SearchTrack { .. }) => {
                                let split = !matches!(self.place, Place::Playlist(_))
                                    && self.player.queued() > 0;
                                state.search_action = state.search_action.moved(delta, split);
                            }
                            Some(Content::Album(_)) => {
                                state.search_action = state.search_action.moved(delta, false);
                            }
                            _ => {}
                        }
                    }
                    blur_search()
                }
            };
        }
        if let Screen::Shelf(state) = &mut self.screen
            && matches!(state.selection.selected(), Some(Content::Album(_)))
            && matches!(
                direction,
                crate::search::Direction::Left | crate::search::Direction::Right
            )
        {
            let delta = if direction == crate::search::Direction::Left {
                -1
            } else {
                1
            };
            state.cover_action = state.cover_action.moved(delta, self.player.engine_ready());
            return Task::none();
        }
        match direction {
            crate::search::Direction::Up => {
                let target = self.player.step_volume(1);
                self.send_volume(target);
            }
            crate::search::Direction::Down => {
                let target = self.player.step_volume(-1);
                self.send_volume(target);
            }
            crate::search::Direction::Left => {
                let target = self.player.seek_by(-keys::SEEK_STEP_MS);
                self.send_seek(target);
            }
            crate::search::Direction::Right => {
                let target = self.player.seek_by(keys::SEEK_STEP_MS);
                self.send_seek(target);
            }
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
            Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
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

    /// The jewel case's one continuous scalar and its direct-manipulation
    /// gesture. Kept separate from bounded hover tweens because this clock is
    /// intentionally continuous while the surface is being watched.
    fn update_case(&mut self, message: &Message) -> Option<Task<Message>> {
        match *message {
            Message::WindowFocused(focused) => {
                if !focused {
                    self.case_rotation.release();
                }
            }
            Message::CaseTick(now) => {
                if self.place == Place::NowPlaying && self.visualization.foreground.draws_case() {
                    self.case_rotation.tick(now);
                }
                if self.place == Place::NowPlaying && self.visualization.mode.records_history() {
                    let audio = self.playback.visualization();
                    self.visualization_history
                        .capture(self.visualization.mode, &audio);
                }
            }
            Message::CasePressed(at)
                if self.place == Place::NowPlaying
                    && self.visualization.foreground.draws_case() =>
            {
                self.case_rotation.press(at);
            }
            Message::CaseDragged(at)
                if self.place == Place::NowPlaying
                    && self.visualization.foreground.draws_case() =>
            {
                self.case_rotation.drag(at);
            }
            Message::CaseReleased
                if self.place == Place::NowPlaying
                    && self.visualization.foreground.draws_case() =>
            {
                self.case_rotation.release();
            }
            Message::VisualizationForeground(foreground) if self.place == Place::NowPlaying => {
                if self.visualization.foreground != foreground {
                    self.visualization.foreground = foreground;
                    persist_visualization_foreground(foreground);
                }
                self.case_rotation.release();
            }
            Message::NextVisualization if self.place == Place::NowPlaying => {
                self.visualization.mode = self.visualization.mode.next();
                self.visualization_history = crate::visualizer::History::default();
            }
            Message::ToggleFacts if self.place == Place::NowPlaying => {
                self.visualization.facts = !self.visualization.facts;
                persist(|config| config.now_playing_facts = self.visualization.facts);
            }
            Message::AdvanceFact if self.place == Place::NowPlaying && self.visualization.facts => {
                self.fact_index = self.fact_index.wrapping_add(1);
            }
            Message::CasePressed(_)
            | Message::CaseDragged(_)
            | Message::CaseReleased
            | Message::VisualizationForeground(_)
            | Message::NextVisualization
            | Message::ToggleFacts
            | Message::AdvanceFact => {}
            _ => return None,
        }
        Some(Task::none())
    }

    /// Pay the sample-copy cost only while an audio visualization is visible.
    fn sync_visualization_tap(&self) {
        self.playback.set_visualization_enabled(
            self.place == Place::NowPlaying
                && self.player.now_playing().is_some()
                && self.visualization.mode.active(),
        );
    }

    /// Log startup-to-interactive, once, on the first frame the window
    /// presents. The `window::frames()` subscription that produces it is
    /// dropped the moment this has run — the first bounded clock baz shipped,
    /// and the pattern ADR-0020 generalises.
    fn log_first_frame(&mut self) -> Task<Message> {
        if !self.first_frame_logged {
            self.first_frame_logged = true;
            crate::baz_log!(
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
            Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
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
                Screen::Setup(_) | Screen::Blocked(_) => false,
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

    /// The context menu's own small machine (doc 09 §5.2): open at the
    /// pointer, close, and make an item's presses. First among the machines
    /// because the menu is the topmost layer wherever it stands — a message
    /// that is the menu's is nobody else's.
    fn update_menu(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::OpenMenu(target, at) => {
                // Not over a drag: a right press mid-hold would float a
                // menu over a gesture whose release is still owed, and the
                // stack level it adds would reshape the tree under the
                // held row (the ghost layer's own note). The hand finishes
                // one gesture before it starts another.
                if self.drag.is_some() {
                    return Some(Task::none());
                }
                // The items are decided now, against the facts as they
                // stand, and captured — the menu shows what the listener
                // saw, and a press sends exactly what was on screen. A
                // target none of whose verbs can act offers nothing: no
                // card of disabled words, and no card at all.
                let listed = menu::items(*target, &self.menu_facts());
                self.menu = (!listed.is_empty()).then(|| menu::Menu {
                    at: *at,
                    items: listed,
                });
                Some(Task::none())
            }
            Message::CloseMenu => {
                self.menu = None;
                Some(Task::none())
            }
            Message::MenuItemPressed(index) => {
                let Some(open) = self.menu.take() else {
                    return Some(Task::none());
                };
                let Some(item) = open.items.into_iter().nth(*index) else {
                    return Some(Task::none());
                };
                // **The accelerator makes the presses the hand would have
                // made** — each message re-enters the ordinary update loop,
                // so a menu press and a control press are one code path by
                // construction (the mirror rule's mechanical half).
                let panel_was_open = self.playlists.panel_open;
                let tasks: Vec<Task<Message>> = item
                    .presses
                    .into_iter()
                    .map(|press| self.update(press))
                    .collect();
                // The picker summoned by the item's own intermediate press
                // and completed by its last does not outlive the gesture: a
                // right-click `Queue` must not leave a panel standing the
                // listener never asked for. A panel that was already open
                // stays open (its counts just changed — closing it would
                // hide the effect of the press), and an item whose *point*
                // is the picker — `Add to playlist…` — leaves a pick in
                // flight, so the panel stays for it too.
                if !panel_was_open && self.playlists.pending.is_none() {
                    self.playlists.close_panel();
                }
                Some(Task::batch(tasks))
            }
            _ => None,
        }
    }

    /// The readings the menu builder decides items against
    /// ([`menu::Facts`]) — snapshots, so [`menu::items`] stays a pure
    /// function the mirror test can sweep without an `App`.
    fn menu_facts(&self) -> menu::Facts {
        menu::Facts {
            engine_ready: self.player.engine_ready(),
            collecting: self.playlists.available(),
            current: self.current_playlist(),
            playing_album: self.player.playing_album(),
            playing_queue_row: self.player.playing_queue_row(),
        }
    }

    /// The **current playlist** (09 §6): playing provenance naming a file
    /// that still exists — checked against the folder itself rather than
    /// the panel's rows, which are only refreshed while the panel is used.
    /// A rename or delete under the run answers `None`, and the menu's
    /// `Add to "{name}"` withdraws rather than dangling.
    fn current_playlist(&self) -> Option<(u64, String)> {
        let name = self.player.queue_provenance()?;
        self.playlists
            .holds(name)
            .then(|| (crate::playlists::playlist_id(name), name.to_owned()))
    }

    /// The one quiet road out of Now playing: a saved playlist's page, the
    /// unsaved playlist represented by the current run, or the sounding
    /// track's resolved album.
    fn now_playing_source(&self) -> Option<views::now_playing::Source> {
        let now = self.player.now_playing()?;
        if let Some((id, name)) = self.current_playlist() {
            return Some(views::now_playing::Source::Playlist { id, name });
        }
        if matches!(
            self.player.run_origin(),
            crate::player::RunOrigin::Assembled
        ) {
            let name = views::queue::unsaved_name(self.player.queue_origin());
            return Some(views::now_playing::Source::Queue { name });
        }
        Some(views::now_playing::Source::Album {
            id: now.album_id?,
            name: now
                .album
                .clone()
                .unwrap_or_else(|| "Unknown Album".to_owned()),
        })
    }

    /// Follow the bottom bar's current-song block to the list that supplied
    /// it. A saved playlist opens at the engine-confirmed playable position;
    /// the other source kinds retain their existing destinations.
    fn open_playing_source(&mut self) -> Task<Message> {
        let Some(source) = self.now_playing_source() else {
            return Task::none();
        };
        match source {
            views::now_playing::Source::Playlist { id, .. } => {
                let playing = self.player.playing_queue_row();
                let opened = match &self.screen {
                    Screen::Shelf(state) => self.playlists.open_page(id, &state.library),
                    Screen::Setup(_) | Screen::Blocked(_) => false,
                };
                if !opened {
                    return Task::none();
                }
                let offset = playing.and_then(|playing| {
                    views::playlist::scroll_offset(self.playlists.page(id)?, playing)
                });
                self.menu = None;
                self.drag = None;
                let from = self.place;
                self.place = Place::Playlist(id);
                self.place_history.visit(self.place);
                let entering = self.note_place_left(from);
                match offset {
                    Some(y) => {
                        self.playlist_scroll = y;
                        Task::batch([
                            entering,
                            iced::widget::operation::scroll_to(
                                views::page::scroll_id(),
                                AbsoluteOffset { x: 0.0, y },
                            ),
                            self.request_playlist_art(),
                        ])
                    }
                    None => entering,
                }
            }
            views::now_playing::Source::Queue { .. } => self.go(|_| Place::Queue),
            views::now_playing::Source::Album { id, .. } => self.open_album(id),
        }
    }

    /// Answer a message that belongs to the **playlist surfaces** — the
    /// panel, the page, the adds and the queue place's save — reporting
    /// whether it was one (`Some`), and with which follow-up task.
    ///
    /// One machine for the same reason the volume's nine and the library's
    /// six are one: every arm resolves to "tell the playlists state machine,
    /// maybe tell the engine, maybe move the caret", and two dozen more arms
    /// in the shell's own match would bury the messages genuinely about the
    /// whole application. The engine effects (`Play`, `Queue`, a row click)
    /// live in their own named helpers below, in `play_album`'s exact shape.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per playlist message, each a few lines; splitting \
                  the machine would scatter one surface's grammar across \
                  several functions"
    )]
    fn update_playlists(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::TogglePlaylists => {
                // Only once there is a shelf (playlists resolve against the
                // library), and never in Settings — the panel is absent there
                // (ADR-0024 §5), so the key falls dead rather than opening a
                // surface the place will not show.
                if let Screen::Shelf(state) = &self.screen
                    && self.place != Place::Settings
                {
                    self.playlists.toggle_panel(Some(&state.library));
                }
            }
            Message::OpenPlaylist(id) => {
                if *id == crate::playlists::FAVOURITES_ID {
                    return Some(self.go(|_| Place::Favourites));
                }
                // Repeating an explicit Open must not reread, reset or leave
                // the page; opening a subject is not a disguised Back action.
                if self.place == Place::Playlist(*id) {
                    return Some(Task::none());
                }
                if let Screen::Shelf(state) = &self.screen
                    && self.playlists.open_page(*id, &state.library)
                {
                    if let Screen::Shelf(state) = &mut self.screen {
                        state.selection.select(Content::Playlist(*id));
                    }
                    self.playlist_scroll = 0.0;
                    // The place changes, so an open menu goes with it
                    // (`go`'s rule).
                    self.menu = None;
                    let from = self.place;
                    self.place = self.place.playlist(*id);
                    self.place_history.visit(self.place);
                    let entering = self.note_place_left(from);
                    return Some(Task::batch([
                        entering,
                        iced::widget::operation::scroll_to(
                            views::page::scroll_id(),
                            AbsoluteOffset { x: 0.0, y: 0.0 },
                        ),
                        self.request_playlist_art(),
                    ]));
                }
            }
            Message::PlayPlaylist(id) => {
                if *id == crate::playlists::FAVOURITES_ID {
                    self.play_favourites(None);
                    return Some(Task::none());
                }
                let opened = match &self.screen {
                    Screen::Shelf(state) => self.playlists.open_page(*id, &state.library),
                    Screen::Setup(_) | Screen::Blocked(_) => false,
                };
                if opened {
                    self.play_playlist();
                }
            }
            Message::PlaylistOrderSelected(order) => self.playlists.order = *order,
            Message::PlaylistRailJumped(run) => {
                let hang = match &self.screen {
                    Screen::Shelf(state) => state.grid(),
                    Screen::Setup(_) | Screen::Blocked(_) => return Some(Task::none()),
                };
                // The wall is grouped, so a rail entry names a **run** and the
                // jump lands on that run's heading — the Library's own
                // `jump_to_shelf`, over the same `Shelves` the view lays out.
                let wall = self.playlists.wall();
                let shelves = shelf::Shelves::new(hang, &wall.counts);
                let Some(target) = shelves.runs().get(*run).map(|run| run.top) else {
                    return Some(Task::none());
                };
                self.playlists_scroll = target;
                return Some(Task::batch([
                    iced::widget::operation::scroll_to(
                        views::playlists::scroll_id(),
                        AbsoluteOffset { x: 0.0, y: target },
                    ),
                    self.request_playlist_art(),
                ]));
            }
            Message::PlaylistTileEntered(id) => self.playlists.hovered = Some(*id),
            Message::PlaylistTileLeft(id) => {
                if self.playlists.hovered == Some(*id) {
                    self.playlists.hovered = None;
                }
            }
            Message::PlaylistOverviewDeleteStart(id) => {
                self.playlists.confirming_overview_delete = Some(*id);
                self.playlists.hovered = None;
            }
            Message::PlaylistOverviewDeleteCancel => {
                self.playlists.confirming_overview_delete = None;
            }
            Message::PlaylistOverviewDelete => {
                let Some(id) = self.playlists.confirming_overview_delete else {
                    return Some(Task::none());
                };
                let before = self.playlists.rows.iter().position(|row| row.id == id);
                let library = match &self.screen {
                    Screen::Shelf(state) => Some(&state.library),
                    Screen::Setup(_) | Screen::Blocked(_) => None,
                };
                if self.playlists.delete_id(id, library)
                    && let Screen::Shelf(state) = &mut self.screen
                {
                    if let Some(row) = before.and_then(|index| {
                        let last = self.playlists.rows.len().saturating_sub(1);
                        self.playlists.rows.get(index.min(last))
                    }) {
                        state.selection.select(Content::Playlist(row.id));
                    } else {
                        state.selection.clear();
                    }
                }
            }
            Message::PickPlaylist(id) => {
                if let Screen::Shelf(state) = &self.screen {
                    self.playlists.pick(*id, &state.library);
                }
            }
            Message::PickQueue => {
                if let Some(pending) = self.playlists.pick_queue() {
                    self.append_items_to_run(pending.items);
                }
            }
            Message::NewPlaylistStart => {
                let held = self
                    .playlists
                    .pending
                    .take()
                    .map(|pending| pending.items)
                    .unwrap_or_default();
                self.playlists.panel_open = false;
                self.playlists.naming = None;
                self.playlists.begin_creation();
                self.playlists.creation.mode = Some(crate::playlists::CreationMode::Manual);
                for item in held {
                    if !self
                        .playlists
                        .creation
                        .items
                        .iter()
                        .any(|existing| existing.path == item.path)
                    {
                        self.playlists.creation.items.push(item);
                    }
                }
                return Some(self.go(|_| Place::NewPlaylist));
            }
            Message::NewPlaylistInput(text) => {
                if let Some(naming) = &mut self.playlists.naming {
                    naming.text.clone_from(text);
                    naming.error = None;
                }
            }
            Message::NewPlaylistSubmit => {
                if let Screen::Shelf(state) = &self.screen {
                    self.playlists.submit_new(&state.library);
                }
            }
            Message::AddAlbumToPlaylist(id) => self.add_album_to_playlist(*id),
            Message::AddTrackToPlaylist(id, row) => self.add_track_to_playlist(*id, *row),
            Message::AddQueuedToPlaylist(row) => self.add_queued_to_playlist(*row),
            Message::PlaylistPlay => self.play_playlist(),
            Message::PlaylistPlayTrack(row) => self.play_playlist_track(*row),
            Message::PlaylistRemoveEntry(row) => {
                if let Screen::Shelf(state) = &self.screen {
                    self.playlists.remove_entry(*row, &state.library);
                }
            }
            Message::PlaylistShiftEntry(row, delta) => {
                if let Screen::Shelf(state) = &self.screen {
                    self.playlists.shift_entry(*row, *delta, &state.library);
                }
            }
            Message::PlaylistAddEntry(row) => self.add_playlist_entry_to_picker(*row),
            Message::PlaylistRenameStart => {
                if let Some(open) = &mut self.playlists.open {
                    let seeded = open.name().to_owned();
                    open.confirming_delete = false;
                    open.renaming = Some(crate::playlists::NameEntry {
                        text: seeded,
                        error: None,
                    });
                    return Some(iced::widget::operation::focus(views::playlist::rename_id()));
                }
            }
            Message::PlaylistRenameInput(text) => {
                if let Some(renaming) = self
                    .playlists
                    .open
                    .as_mut()
                    .and_then(|open| open.renaming.as_mut())
                {
                    renaming.text.clone_from(text);
                    renaming.error = None;
                }
            }
            Message::PlaylistRenameSubmit => {
                if let Screen::Shelf(state) = &self.screen
                    && let Some(renamed) = self.playlists.submit_rename(&state.library)
                    && matches!(self.place, Place::Playlist(_))
                {
                    // The place follows the name: the id *is* the name,
                    // hashed, so a rename mints a new one.
                    let from = self.place;
                    self.place = Place::Playlist(renamed);
                    self.place_history.visit(self.place);
                    // A playlist door, or the Library after a delete —
                    // never `Now playing`, so the only task this can answer
                    // with is `Task::none()`, and the machine this arm lives
                    // in answers `bool`. Discarded deliberately rather than
                    // by omission.
                    let _ = self.note_place_left(from);
                }
            }
            Message::PlaylistDeleteStart => {
                if let Some(open) = &mut self.playlists.open {
                    open.renaming = None;
                    open.confirming_delete = true;
                }
            }
            Message::PlaylistDeleteCancel => {
                if let Some(open) = &mut self.playlists.open {
                    open.confirming_delete = false;
                }
            }
            Message::PlaylistDelete => {
                if !self
                    .playlists
                    .open
                    .as_ref()
                    .is_some_and(|open| open.confirming_delete)
                {
                    return Some(Task::none());
                }
                let library = match &self.screen {
                    Screen::Shelf(state) => Some(&state.library),
                    Screen::Setup(_) | Screen::Blocked(_) => None,
                };
                let id = self.playlists.open.as_ref().map(|open| open.id);
                if id.is_some_and(|id| self.playlists.delete_id(id, library))
                    && matches!(self.place, Place::Playlist(_))
                {
                    // The page's subject is in the trash; its collection root
                    // is the honest answer.
                    let from = self.place;
                    self.place = Place::Playlists;
                    self.place_history.visit(self.place);
                    // A playlist door, or the Library after a delete —
                    // never `Now playing`, so the only task this can answer
                    // with is `Task::none()`, and the machine this arm lives
                    // in answers `bool`. Discarded deliberately rather than
                    // by omission.
                    let _ = self.note_place_left(from);
                }
            }
            Message::SaveQueueStart => {
                self.playlists.saving_queue = Some(crate::playlists::NameEntry {
                    text: views::queue::unsaved_name(self.player.queue_origin()),
                    error: None,
                });
                return Some(iced::widget::operation::focus(views::queue::save_name_id()));
            }
            Message::SaveQueueInput(text) => {
                if let Some(saving) = &mut self.playlists.saving_queue {
                    saving.text.clone_from(text);
                    saving.error = None;
                }
            }
            Message::SaveQueueSubmit => {
                if let Some(queue) = self.player.queue() {
                    let queue = queue.clone();
                    let library = match &self.screen {
                        Screen::Shelf(state) => Some(&state.library),
                        Screen::Setup(_) | Screen::Blocked(_) => None,
                    };
                    self.playlists.submit_queue_save(&queue, library);
                }
            }
            Message::PlaylistRowEntered(row) => {
                self.hovered_playlist_row = Some(*row);
                return Some(Task::none());
            }
            Message::PlaylistRowLeft(row) => {
                if self.hovered_playlist_row == Some(*row) {
                    self.hovered_playlist_row = None;
                }
                return Some(Task::none());
            }
            Message::PlaylistScrolled(viewport) => {
                self.playlist_scroll = viewport.absolute_offset().y;
                return Some(self.request_playlist_art());
            }
            Message::PlaylistsScrolled(viewport) => {
                self.playlists_scroll = viewport.absolute_offset().y;
                return Some(self.request_playlist_art());
            }
            Message::AlbumRowEntered(row) => {
                self.hovered_album_row = Some(*row);
                return Some(Task::none());
            }
            Message::AlbumRowLeft(row) => {
                if self.hovered_album_row == Some(*row) {
                    self.hovered_album_row = None;
                }
                return Some(Task::none());
            }
            _ => return None,
        }
        // Whatever the act just changed, the sleeves may now quote records
        // whose thumbnails are not decoded yet — ask for exactly those, off
        // thread, through the wall's own pipeline (ADR-0024 §A1).
        Some(self.request_playlist_art())
    }

    /// Home's opt-in local sonic analyzer and playlist composer.
    #[allow(clippy::too_many_lines)]
    fn update_vibe(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::VibeCreate => {
                let Screen::Shelf(state) = &mut self.screen else {
                    return Some(Task::none());
                };
                if !self.playlists.available() {
                    return Some(Task::none());
                }
                if state.vibe.prompt.trim().is_empty() || state.vibe.preparing {
                    return Some(Task::none());
                }
                state.vibe.begin_request();
                if state.vibe.has_features() {
                    state.vibe.create(
                        config::vibe_db_file().as_deref(),
                        &state.albums,
                        &state.edition_choice,
                    );
                    return Some(Task::none());
                }
                // **A cold index is the ordinary first run, not a reason to
                // do nothing.** This arm required the store to *already
                // exist* — `.filter(|path| path.exists())` — which was
                // survivable while a separate `Analyse locally & create`
                // button created it, and became a press that silently did
                // nothing the moment the consent gate folded into this one
                // (item 50). `prepare` creates the store; the only real
                // failure is a system with no data directory at all, which
                // is what `VibeAnalyze` says out loud and this now says too.
                let Some(index) = config::vibe_db_file() else {
                    state.vibe.error = Some(
                        "This system offers no data folder for the local analysis index."
                            .to_owned(),
                    );
                    return Some(Task::none());
                };
                let paths = crate::vibe::library_paths(&state.albums, &state.edition_choice);
                state.vibe.start_preparing();
                Some(Task::perform(
                    crate::vibe::prepare(index, paths),
                    Message::VibePrepared,
                ))
            }
            Message::VibeAnalyze => {
                let Some(index) = config::vibe_db_file() else {
                    if let Screen::Shelf(state) = &mut self.screen {
                        state.vibe.error = Some(
                            "This system offers no data folder for the local analysis index."
                                .to_owned(),
                        );
                    }
                    return Some(Task::none());
                };
                let Screen::Shelf(state) = &mut self.screen else {
                    return Some(Task::none());
                };
                let paths = crate::vibe::library_paths(&state.albums, &state.edition_choice);
                state.vibe.start_preparing();
                Some(Task::perform(
                    crate::vibe::prepare(index, paths),
                    Message::VibePrepared,
                ))
            }
            Message::VibePrepared(result) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.accept_preparation(result.clone());
                    if !state.vibe.analyzing && state.vibe.awaiting_create {
                        state.vibe.create(
                            config::vibe_db_file().as_deref(),
                            &state.albums,
                            &state.edition_choice,
                        );
                    }
                }
                Some(self.next_vibe_job())
            }
            Message::VibeAnalyzed(result) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.accept_analysis(result.clone());
                    if !state.vibe.analyzing
                        && state.vibe.failed > 0
                        && let Some(detail) = state.vibe.failure_note()
                    {
                        state.health.record(
                            crate::health::Level::Warning,
                            "Sonic analysis skipped tracks",
                            detail,
                        );
                    }
                    if !state.vibe.analyzing && state.vibe.awaiting_create {
                        state.vibe.create(
                            config::vibe_db_file().as_deref(),
                            &state.albums,
                            &state.edition_choice,
                        );
                    }
                }
                Some(self.next_vibe_job())
            }
            Message::VibeAnalysisCancel => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.cancel_analysis();
                }
                Some(Task::none())
            }
            Message::VibePrompt(prompt) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.set_prompt(prompt);
                }
                self.playlists.suggest_creation_name(prompt);
                Some(Task::none())
            }
            Message::VibeLength(length) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.set_length(*length);
                }
                Some(Task::none())
            }
            Message::ContourDragged(lane, index, at, level) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.drag_contour(*lane, *index, *at, *level);
                }
                Some(Task::none())
            }
            // The gesture's end changes nothing on its own: the line is
            // already where it was dragged to, and the list is composed when
            // the listener asks for it. It exists so the widget has one
            // message to publish on release rather than a silent edge.
            Message::ContourReleased => Some(Task::none()),
            Message::VibePreviewHovered(row) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.hover_row(*row);
                }
                Some(Task::none())
            }
            Message::ContourShape(index) => {
                if let Screen::Shelf(state) = &mut self.screen
                    && let Some(shape) = crate::vibe::Shape::ALL.get(*index)
                {
                    state.vibe.set_shape(*shape);
                }
                Some(Task::none())
            }
            Message::ContourPointAdded(lane) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.add_contour_point(*lane);
                }
                Some(Task::none())
            }
            Message::ContourPointRemoved(lane) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.remove_contour_point(*lane);
                }
                Some(Task::none())
            }
            Message::ContourDimension(dimension) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.toggle_dimension(*dimension);
                }
                Some(Task::none())
            }
            Message::VibeAnother => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.another(
                        config::vibe_db_file().as_deref(),
                        &state.albums,
                        &state.edition_choice,
                    );
                }
                Some(Task::none())
            }
            Message::VibePreviewRemove(row) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.remove_preview(*row);
                }
                Some(Task::none())
            }
            Message::VibePreviewShift(row, delta) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.shift_preview(*row, *delta);
                }
                Some(Task::none())
            }
            Message::VibePlay => {
                let items = match &self.screen {
                    Screen::Shelf(state) => state
                        .vibe
                        .preview
                        .as_ref()
                        .map(|preview| preview.items.clone())
                        .unwrap_or_default(),
                    Screen::Setup(_) | Screen::Blocked(_) => Vec::new(),
                };
                if items.is_empty() {
                    return Some(Task::none());
                }
                let queue = vm::QueueVm {
                    album: None,
                    artist: "Various artists".to_owned(),
                    items,
                    origin: Some(crate::origin::Origin::Hand { was: None }),
                    source: vm::RunSource::Assembled,
                };
                if self.send_run(queue, None).is_some() && self.playback.send(Command::Play) {
                    self.player.note_transport_sent();
                } else {
                    self.player.engine_closed();
                }
                self.publish_mpris(false);
                Some(Task::none())
            }
            Message::VibeSubmit => Some(self.save_playlist_creation()),
            _ => None,
        }
    }

    fn next_vibe_job(&mut self) -> Task<Message> {
        let Some(index) = config::vibe_db_file() else {
            return Task::none();
        };
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        let jobs = state.vibe.next_jobs(Self::configured_vibe_workers());
        Task::batch(jobs.into_iter().map(|(run, path)| {
            Task::perform(
                crate::vibe::analyze(index.clone(), run, path),
                Message::VibeAnalyzed,
            )
        }))
    }

    /// Number of concurrent local Vibe analyzers. Four model sessions are a
    /// reasonable baseline, while this temporary workload benefits from
    /// spending more memory to finish sooner. `BAZ_VIBE_WORKERS` tunes the
    /// trade-off for the current machine and is capped to sixteen workers.
    fn configured_vibe_workers() -> usize {
        let configured = config::config_file().map_or(config::DEFAULT_VIBE_WORKERS, |path| {
            config::load(&path).vibe_workers
        });
        std::env::var("BAZ_VIBE_WORKERS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .map_or(configured, |workers| {
                workers.clamp(1, config::MAX_VIBE_WORKERS)
            })
    }

    /// Ask for the playlist artwork belonging to the current place only.
    /// An open playlist needs its header and visible track rows; the unsaved
    /// state needs the same; the collection root needs its tiles. Other places
    /// leave playlist collages to the lane's viewport-aware background request.
    /// **Which collages the saved-playlist wall can see**, read off the same
    /// projection the view draws (`playlists::Wall`) and laid out by the same
    /// [`shelf::Shelves`].
    ///
    /// It has to be the same one. The wall groups now, so a heading band
    /// stands between every run and the visible tiles are no longer
    /// `scroll / row_h`: asking the flat grid decodes the collages of tiles a
    /// screen away while the ones on screen stay gradients — the exact failure
    /// item 37 fixed on the record wall, arriving by a different route.
    fn visible_playlist_collages(&self) -> Vec<u64> {
        let Screen::Shelf(state) = &self.screen else {
            return Vec::new();
        };
        let hang = state.grid();
        let wall = self.playlists.wall();
        let shelves = shelf::Shelves::new(hang, &wall.counts);
        let (first_run, end_run) = shelves.visible_runs(self.playlists_scroll, self.body_height());
        let mut wanted = Vec::new();
        for run in &shelves.runs()[first_run..end_run] {
            let (first_row, end_row) = hang.visible_rows(
                self.playlists_scroll - run.rows_top(hang),
                self.body_height(),
                run.rows,
            );
            let first_cell = run.first + first_row.saturating_mul(hang.columns);
            let end_cell = (run.first + end_row.saturating_mul(hang.columns))
                .min(run.first + run.len)
                .min(wall.cells.len());
            for cell in &wall.cells[first_cell.min(end_cell)..end_cell] {
                if let crate::playlists::Cell::List(row) = cell {
                    wanted.extend(&row.art);
                }
            }
        }
        wanted
    }

    fn request_playlist_art(&mut self) -> Task<Message> {
        let mut wanted: Vec<u64> = Vec::new();
        match self.place {
            Place::Playlists => wanted.extend(self.visible_playlist_collages()),
            Place::Playlist(_) => {
                if let Some(open) = &self.playlists.open {
                    wanted.extend(&open.art);
                    let window = views::playlist::row_window(
                        open.rows.len(),
                        self.playlist_scroll,
                        self.body_height(),
                    );
                    wanted.extend(
                        open.rows[window.first..window.end]
                            .iter()
                            .filter_map(|row| row.album_id),
                    );
                }
            }
            Place::Favourites => {
                if let Screen::Shelf(state) = &self.screen {
                    wanted.extend(self.playlists.favourite.art.iter().copied());
                    let queue = views::favourites::queue(state);
                    let window = views::playlist::row_window(
                        queue.items.len(),
                        self.playlist_scroll,
                        self.body_height(),
                    );
                    for item in &queue.items[window.first..window.end] {
                        let filed_under = item
                            .album_artist
                            .as_deref()
                            .unwrap_or(queue.artist.as_str());
                        wanted.extend(item.album.as_deref().and_then(|title| {
                            state
                                .albums
                                .iter()
                                .find(|album| {
                                    album.title.as_deref() == Some(title)
                                        && album.artist.label() == filed_under
                                })
                                .map(|album| album.id)
                        }));
                    }
                }
            }
            Place::Queue => {
                if let Screen::Shelf(state) = &self.screen
                    && let Some(queue) = self.player.queue()
                {
                    wanted.extend(views::queue::unsaved_art(state, &self.player));
                    let window = views::playlist::row_window(
                        queue.items.len(),
                        views::playlist_page::layout(self.body_width())
                            .rows_scroll(self.queue_scroll),
                        self.body_height(),
                    );
                    for item in &queue.items[window.first..window.end] {
                        let filed_under = item
                            .album_artist
                            .as_deref()
                            .unwrap_or(queue.artist.as_str());
                        let id = item.album.as_deref().and_then(|title| {
                            state
                                .albums
                                .iter()
                                .find(|album| {
                                    album.title.as_deref() == Some(title)
                                        && album.artist.label() == filed_under
                                })
                                .map(|album| album.id)
                        });
                        wanted.extend(id);
                    }
                }
            }
            _ => {}
        }
        wanted.sort_unstable();
        wanted.dedup();
        match &mut self.screen {
            Screen::Shelf(state) => state.request_thumbs(&wanted),
            Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
        }
    }

    /// The record, whole, held while the panel serves as the picker
    /// (09 §8.1). What is held is the **selected edition** — the same tracks
    /// the page lists and `Play album` would queue — in both shapes a pick
    /// can land as: file entries, and queue items.
    fn add_album_to_playlist(&mut self, id: u64) {
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
        let entries = crate::playlists::entries_for_tracks(&edition.tracks, album.artist.label());
        let items = vm::album_queue(album, chosen).items;
        let label = format!(
            "Add \u{201c}{}\u{201d}",
            album.title.as_deref().unwrap_or("Unknown Album")
        );
        self.playlists
            .begin_pick(Some(&state.library), label, entries, items);
    }

    /// One track toward the picker, by the same rule. The track does not
    /// smuggle its album in — the listener pointed at a track (ADR-0023 §3's
    /// queue rule, applied to collecting): queued, it is its own one-row
    /// group, headed by its record's name.
    fn add_track_to_playlist(&mut self, id: u64, row: usize) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        let Some(album) = state.albums.iter().find(|album| album.id == id) else {
            return;
        };
        let chosen = state.edition_choice.get(&id).copied();
        let Some(track) = vm::selected_edition(album, chosen)
            .and_then(|edition| edition.tracks.get(row))
            .cloned()
        else {
            return;
        };
        let entries = crate::playlists::entries_for_tracks(
            std::slice::from_ref(&track),
            album.artist.label(),
        );
        let items = vec![vm::QueueItemVm {
            title: track.title.clone(),
            artist: track.artist.clone().filter(|_| album.track_artists_vary),
            album: album.title.clone(),
            album_artist: Some(album.artist.label().to_owned()),
            duration: track.duration,
            path: track.path.clone(),
        }];
        let label = format!("Add \u{201c}{}\u{201d}", track.title);
        self.playlists
            .begin_pick(Some(&state.library), label, entries, items);
    }

    /// One **queue row's** track toward the picker — the queue place's `+`
    /// (doc 09 §8.2, the place reaching §8.1's one transfer gesture): hold
    /// what the row shows, summon the panel as the picker.
    ///
    /// The track is read from the request-side queue record
    /// ([`PlayerState::queue`]), which is the same value the row was drawn
    /// from — so what the picker holds is exactly what was pointed at, and a
    /// row a fresh edit has just removed asks for nothing. It works on the
    /// sounding row too: the track you are hearing is the one most worth
    /// keeping (S8's whole premise, row-sized).
    fn add_queued_to_playlist(&mut self, row: usize) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        let Some(item) = self
            .player
            .queue()
            .and_then(|queue| queue.items.get(row))
            .cloned()
        else {
            return;
        };
        let entries = crate::playlists::entries_for_items(std::slice::from_ref(&item));
        let label = format!("Add \u{201c}{}\u{201d}", item.title);
        self.playlists
            .begin_pick(Some(&state.library), label, entries, vec![item]);
    }

    /// One **playlist-page row's** track toward the picker — the page's `+`
    /// (doc 09 §8.2's "same editor" anatomy, the page's own side of the slot
    /// the queue rows carry; the visible twin §5.2's mirror rule requires of
    /// the page rows' menu items).
    ///
    /// The track is read through the row's `playable_position` into the
    /// page's own queue shape — exactly what a press on the row would play —
    /// so a missing entry (no position) asks for nothing, and what the
    /// picker holds is what was pointed at. The file is not touched: this is
    /// a read of the page, never an edit to it.
    fn add_playlist_entry_to_picker(&mut self, row: usize) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        let Some(item) = self.playlists.open.as_ref().and_then(|open| {
            let position = open.rows.get(row)?.playable_position?;
            open.queue.items.get(position).cloned()
        }) else {
            return;
        };
        let entries = crate::playlists::entries_for_items(std::slice::from_ref(&item));
        let label = format!("Add \u{201c}{}\u{201d}", item.title);
        self.playlists
            .begin_pick(Some(&state.library), label, entries, vec![item]);
    }

    /// The playlist page's `Play`: the playable subset as the queue, playing
    /// (ADR-0024 §4). `SetQueue` then `Play` — `play_album`'s exact shape,
    /// because playing a playlist **copies** it into the queue and from that
    /// instant the two are decoupled (the MPD boundary, ADR-0024 §1).
    fn play_playlist(&mut self) {
        let Some(queue) = self
            .playlists
            .open
            .as_ref()
            .map(|open| open.queue.clone())
            .filter(|queue| !queue.is_empty())
        else {
            return;
        };
        // Shuffle on shuffles the *copy*, and the file's own order is what
        // turning it off returns to ([`Self::send_run`]). ADR-0024 §1's honesty
        // clause is amended to say so: the file is still verbatim, and what the
        // mode re-orders is the run, never the list.
        if self.send_run(queue, None).is_some() && self.playback.send(Command::Play) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
        self.publish_mpris(false);
    }

    /// Play the available members of the built-in Favourites list. Missing
    /// members remain durable library data but never become engine rows.
    fn play_favourites(&mut self, lead: Option<usize>) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        let queue = views::favourites::queue(state);
        if queue.is_empty() || lead.is_some_and(|row| row >= queue.items.len()) {
            return;
        }
        let Some(position) = self.send_run(queue, lead) else {
            return;
        };
        let command = lead.map_or(Command::Play, |_| Command::JumpTo { position });
        if self.playback.send(command) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
        self.publish_mpris(false);
    }

    /// The picker's **Queue** row: what the hand holds, appended to the run —
    /// one `UpdateQueue` over the pick's own items (09 §8.1).
    fn append_items_to_run(&mut self, items: Vec<vm::QueueItemVm>) {
        if items.is_empty() {
            return;
        }
        // The addition's own header names the first record, exactly as a
        // stacked queue's does — spent only when the run is empty; an
        // existing run keeps its header and its provenance.
        let (album, artist) = items.first().map_or((None, String::new()), |item| {
            (
                item.album.clone(),
                item.album_artist.clone().unwrap_or_default(),
            )
        });
        self.append_to_run(vm::QueueVm {
            album,
            artist,
            items,
            origin: None,
            // **Assembled**: this is the listener building a run by hand, one
            // pick at a time, and it is the one kind the save word is for.
            source: vm::RunSource::Assembled,
        });
    }

    /// Insert after the sounding cursor, or after the preceding search `Next`
    /// insertion while that same run/track still stands. The whole edited list
    /// remains the protocol payload; the anchor only prevents repeated presses
    /// from reversing one another locally.
    fn insert_items_next(&mut self, items: Vec<vm::QueueItemVm>) {
        if items.is_empty() {
            return;
        }
        let Some(before) = self.player.queue().cloned() else {
            self.append_items_to_run(items);
            return;
        };
        let at = self.enqueue_next.insertion(
            self.player.track_seq(),
            self.player.playing_queue_row(),
            before.items.len(),
        );
        let Some(edited) = queue_edit::inserted(&before, at, items) else {
            self.enqueue_next.clear();
            return;
        };
        let paths = edited.paths();
        if self
            .playback
            .send(Command::UpdateQueueNext { paths, next: at })
        {
            self.player.note_queue_edited_next(edited, at);
            self.queue_undo.push(before);
        } else {
            self.enqueue_next.clear();
            self.player.engine_closed();
        }
        self.publish_mpris(false);
    }

    /// Append `addition` to the run through `UpdateQueue` — the one shape
    /// every queue-destination pick and the page's `Queue` share. The music
    /// keeps playing; appending to an empty stopped engine loads the queue
    /// without starting it, so nothing sounds unasked (`app.rs`'s own rule,
    /// cited by 09 §8.1).
    fn append_to_run(&mut self, mut addition: vm::QueueVm) {
        self.enqueue_next.clear();
        // What the run held before the append — the empty list when it held
        // nothing — kept for the Queue place's `Undo` (doc 11 §5 P2: an
        // append is an edit a hand can take back, and taking back an append
        // to nothing restores nothing, which cannot sound).
        let before = self.player.queue().cloned().unwrap_or(vm::QueueVm {
            album: None,
            artist: String::new(),
            items: Vec::new(),
            origin: None,
            source: vm::RunSource::Assembled,
        });
        let edited = if let Some(held) = self.player.queue() {
            let mut edited = held.clone();
            edited.items.extend(addition.items);
            edited
        } else {
            // Appending to nothing gives the engine a queue without starting
            // it: `UpdateQueue` never begins playback, and nothing sounds
            // unasked. An append is not a play gesture, so whatever built
            // `addition`, the loaded
            // run carries **no provenance** (09 §6: provenance is set by
            // reifying a file through a play gesture, and by nothing else) —
            // and it is **assembled**, whatever it was built from, because a
            // run that exists only because somebody appended to nothing is a
            // run somebody assembled.
            addition.source = vm::RunSource::Assembled;
            addition
        };
        let paths = edited.paths();
        if self.playback.send(Command::UpdateQueue { paths }) {
            self.player.note_queue_edited(edited);
            self.queue_undo.push(before);
        } else {
            self.player.engine_closed();
        }
        self.publish_mpris(false);
    }

    /// A click on a playlist row: play this list from there, by the same
    /// [`PlayerState::play_from`] decision every list surface spends
    /// (ADR-0024 §4). The engine already holding exactly this list makes it a
    /// jump; anything else queues the playable subset and drops the needle on
    /// the clicked row.
    fn play_playlist_track(&mut self, row: usize) {
        let Some(open) = self.playlists.open.as_ref() else {
            return;
        };
        // The display row maps to its position in the playable subset — the
        // index `JumpTo` speaks; a missing row has none and asks for nothing.
        let Some(position) = open.rows.get(row).and_then(|row| row.playable_position) else {
            return;
        };
        let Some(decision) = self.player.play_from(&open.tracks, position) else {
            return;
        };
        let queue = open.queue.clone();
        let position = match decision {
            player::PlayFrom::Jump { position } => position,
            player::PlayFrom::Requeue { position } => {
                // [`Self::play_track`]'s rule, on the playlist page's own rows.
                let Some(at) = self.send_run(queue, Some(position)) else {
                    return;
                };
                at
            }
        };
        if self.playback.send(Command::JumpTo { position }) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
        self.publish_mpris(false);
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
            Message::ShiftQueued(row, delta) => self.shift_queued(row, delta),
            Message::QueueScrolled(viewport) => {
                self.queue_scroll = viewport.absolute_offset().y;
            }
            _ => return false,
        }
        true
    }

    /// **The returns lane's own small machine** — the shape `update_playlists`
    /// and `update_queue` already have: the lane's own two presses, answered
    /// apart from the shell's forty arms.
    fn update_lane(&mut self, message: &Message) -> Option<Task<Message>> {
        match *message {
            // **A destination, not a door** — `go` takes a transition, and
            // this one ignores where you were (see [`Place::go`]).
            Message::GoTo(to) => {
                let task = self.go(move |place| place.go(to));
                let art = match to {
                    crate::lane::Destination::Library => match &mut self.screen {
                        Screen::Shelf(state) => {
                            state.forget_requested();
                            state.request_visible_thumbs()
                        }
                        Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
                    },
                    crate::lane::Destination::Playlists => self.request_playlist_art(),
                    crate::lane::Destination::Home | crate::lane::Destination::NowPlaying => {
                        Task::none()
                    }
                };
                Some(Task::batch([task, art]))
            }
            Message::ToggleLane => Some(self.toggle_lane()),
            Message::LaneScrolled(viewport) => {
                self.lane_scroll = viewport.absolute_offset().y;
                Some(Task::none())
            }
            Message::ResumeRun => Some(self.resume_the_run()),
            _ => None,
        }
    }

    /// **The way out**, and the one moment the *elapsed* position is worth
    /// writing (ADR-0023 §6): once, here.
    ///
    /// Every exit route lands on this — the window's close request (`run`'s
    /// `exit_on_close_request(false)`) and the desktop's own Quit — so there
    /// is one exit path and it cannot drift.
    fn leave_for_good(&mut self) -> Task<Message> {
        self.remember_the_run(self.player.elapsed_ms());
        // Setup and Blocked are launch conditions rather than places. Keep the
        // last usable preference when the library did not open, instead of
        // replacing it with the latent `Library` value behind either screen.
        if matches!(self.screen, Screen::Shelf(_)) {
            let place = if self.place == Place::NewPlaylist {
                Place::Playlists
            } else {
                self.place
            };
            persist(|config| config.last_place = place);
        }
        iced::exit()
    }

    /// Restore the last screen once both the library and saved playlists can
    /// validate any subject it names.
    ///
    /// A vanished album or artist returns to the collection. A vanished
    /// playlist returns to the playlists root, which is the nearest surviving
    /// place and makes the disappearance understandable rather than looking
    /// like arbitrary navigation.
    fn restore_place(&mut self, saved: Place) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        self.place = match saved {
            Place::Album(id) if state.album(id).is_some() => saved,
            Place::Artist(id) if views::artist::label(state, id).is_some() => saved,
            Place::Playlist(id) => {
                if self.playlists.open_page(id, &state.library) {
                    saved
                } else {
                    Place::Playlists
                }
            }
            Place::NewPlaylist => Place::Playlists,
            Place::Album(_) | Place::Artist(_) => Place::Library,
            place => place,
        };
    }

    /// **`Resume`**: the run put back on where the band said it was — and the
    /// one play gesture in the product that navigates immediately.
    ///
    /// **Two shapes**, because [`views::home::standing`] has two things the
    /// band can be describing and this must not disagree with it:
    ///
    /// - **A paused session.** The engine already holds the track and the
    ///   position, so the press is a plain [`Command::Play`] and nothing is
    ///   jumped or sought. Spending the snapshot's cursor here would seek a
    ///   run back to the start of the track it is halfway through — the
    ///   snapshot's position is written at track boundaries, so by then it
    ///   reads zero.
    /// - **The interrupted run, at launch.** `JumpTo` at the cursor then
    ///   `Seek` to the position: the two commands the snapshot exists to
    ///   spend, and the one press ADR-0023 §6 promises. The cursor is resolved
    ///   *by path* against the queue as it stands, so a rescan that dropped
    ///   rows before it does not resume the wrong track.
    ///
    /// It does nothing at all rather than something approximate when the
    /// track is gone: playing something the listener did not point at is the
    /// failure ADR-0023 §2 already refuses by name.
    ///
    /// **Then it goes to `Now playing`** — the owner: *"or takes you to now
    /// playing"*. Three things about that are deliberate:
    ///
    /// 1. **It is part of this press**, not a second gesture, and it is the
    ///    front end's own act: unlike a fresh album start, it does not wait on
    ///    [`Event::TrackStarted`] to land. Resume names a run the engine is
    ///    already holding (or one restored and validated at launch), while an
    ///    album `Play` must not claim a dead or wholly unplayable run began.
    /// 2. **It happens last**, after the commands are away and after the
    ///    MPRIS publish, for the reason every other route here follows: this
    ///    codebase has been bitten by *announcing* a state before publishing
    ///    it, never by the reverse.
    /// 3. **Only where something was actually asked for.** A `Now playing`
    ///    place reached by a press that sent nothing would read "Nothing
    ///    playing.", which is a worse answer than staying put.
    fn resume_the_run(&mut self) -> Task<Message> {
        // **A paused run is already where it needs to be.** The engine is
        // holding the track and the position; all it is waiting for is to be
        // let go.
        if self.player.now_playing_path().is_some() {
            if !self.playback.send(Command::Play) {
                self.player.engine_closed();
                return Task::none();
            }
            self.player.note_transport_sent();
            self.publish_mpris(false);
            return self.go(|place| place.go(crate::lane::Destination::NowPlaying));
        }
        let Some(path) = self.resume.current().map(std::path::Path::to_path_buf) else {
            return Task::none();
        };
        let Some(position) = self
            .player
            .queue()
            .and_then(|queue| queue.items.iter().position(|item| item.path == path))
        else {
            return Task::none();
        };
        let position_ms = self.resume.position_ms;
        if !self.playback.send(Command::JumpTo { position }) {
            self.player.engine_closed();
            return Task::none();
        }
        self.player.note_transport_sent();
        // The seek follows the jump: the engine starts the track from its
        // beginning and this moves the needle to where it was. Zero is not
        // sent — a `Seek` to 0 immediately after a start is a redundant
        // drain-and-restart of a session that is already there.
        if position_ms > 0 {
            self.playback.send(Command::Seek { position_ms });
        }
        self.publish_mpris(false);
        self.go(|place| place.go(crate::lane::Destination::NowPlaying))
    }

    /// Hand the snapshot's run back to the engine at launch, silently.
    ///
    /// A run whose files the library no longer holds is dropped row by row
    /// ([`vm::restored_queue`]); a run with nothing left is no run, and the
    /// engine is not told about it.
    fn restore_the_run(&mut self) {
        if self.resume.is_empty() {
            return;
        }
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        // **The two keys, read in one fixed order** (`session::Snapshot`'s
        // own note): a file's name wins, because a run reified from a playlist
        // is that kind whatever else the file says; otherwise the remembered
        // `assembled` flag decides, and its absence is `Fixed` — the reading
        // that offers nothing.
        let source = match self.resume.provenance.clone() {
            Some(name) => vm::RunSource::Playlist(name),
            None if self.resume.assembled => vm::RunSource::Assembled,
            None => vm::RunSource::Fixed,
        };
        let (queue, _) = vm::restored_queue(
            &state.albums,
            &self.resume.paths,
            self.resume.cursor,
            source,
        );
        if queue.is_empty() {
            return;
        }
        let paths = queue.paths();
        let origin = run_origin(&queue);
        if self.playback.send(Command::SetQueue { paths, origin }) {
            self.player.note_queue_sent(queue);
        }
    }

    /// **Write the snapshot when the run moves** — a track boundary, a queue
    /// replaced, a queue edited — and never between.
    ///
    /// The position written here is the *start* of the current track, which is
    /// deliberate: between two of these moments that is the correct place to
    /// resume from if baz is killed rather than closed. The exact elapsed
    /// position is picked up once, on the way out ([`Self::remember_the_run`]).
    fn sync_snapshot(&mut self) {
        let mark = (
            self.player.queued(),
            self.player.playing_queue_row(),
            self.player.track_seq(),
        );
        if mark == self.written {
            return;
        }
        self.written = mark;
        // What may and may not be written is [`next_snapshot`]'s single
        // answer, shared with the exit path — the guard that protects the
        // listener's place must not exist in two copies.
        self.remember_the_run(0);
    }

    /// Write the snapshot, with `position_ms` into the track the cursor is on
    /// — or leave the file alone, when [`next_snapshot`] says this process has
    /// nothing truer to say than it already does.
    ///
    /// Best effort by nature, and every failure is a line on stdout: a player
    /// that could not remember where it got to is a player that starts at the
    /// top, not a player that stops.
    fn remember_the_run(&mut self, position_ms: u64) {
        let Some(path) = crate::session::session_file() else {
            return;
        };
        let Some(snapshot) = next_snapshot(&self.player, position_ms) else {
            return;
        };
        self.resume.clone_from(&snapshot);
        if let Err(error) = crate::session::store(&path, &snapshot) {
            crate::baz_log!("[session] could not write {}: {error}", path.display());
        }
    }

    /// **Re-read the lists when a scan finishes**, and at no other time.
    ///
    /// A playlist's sleeve is a collage of the records it quotes, resolved
    /// against the library (ADR-0024 §A1) — so on a first run, where the
    /// library is empty until the scan lands, every list wears the rest tile.
    /// That was invisible while the only surface showing lists was a panel you
    /// had to summon, because summoning refreshed them. The returns lane is
    /// resident and shows them on the first frame, so the falling edge of the
    /// scan is where the folder is re-read: one pass, at the moment the facts
    /// it needs exist, and never per frame.
    fn sync_lists_with_the_library(&mut self) {
        let scanning = matches!(&self.screen, Screen::Shelf(state) if state.scanning);
        if self.was_scanning
            && !scanning
            && let Screen::Shelf(state) = &self.screen
        {
            self.playlists.refresh(Some(&state.library));
        }
        self.was_scanning = scanning;
    }

    /// Ask for the art the lane and the Home place draw, when either of them
    /// has changed what it draws.
    ///
    /// The guard is the lane's own two stamps plus the place: between them
    /// they change exactly when a new record appears in one of those
    /// surfaces, so this is a comparison of three small values on every other
    /// message.
    ///
    /// **The lists' quotations are asked for by name**, and that is a
    /// correction. A playlist's sleeve is a collage of the records it quotes
    /// (ADR-0024 §A1), read out of the wall's own thumbnail cache — and
    /// nothing was ever putting those records *into* it. `Shelf::offscreen_art`
    /// yields the lane's **records**; a list's quotations are the shell's,
    /// because the shell is what holds [`crate::playlists::Playlists`]. So a
    /// list drew the deterministic gradient until one of the records it quotes
    /// happened to scroll onto the wall — real artwork by luck, which is not
    /// what ADR-0030 §2 claims. Four ids per list, on the same guard, through
    /// the same cache: a sleeve is one decode however many surfaces draw it.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "finite non-negative pixel counts are clamped before becoming row indices"
    )]
    fn request_offscreen_art(&mut self) -> Task<Message> {
        let lane_first = (self.lane_scroll.max(0.0) / theme::SIDEBAR_ROW_PITCH).floor() as usize;
        let mark = (
            self.lane_mark,
            self.place,
            lane_first,
            self.playlists.panel_open,
        );
        if mark == self.art_mark {
            return Task::none();
        }
        self.art_mark = mark;
        // The mixed lane can hold every playlist and the recent records, but
        // only a small window can be seen. Keep a generous two-row overscan
        // either side (the heading is intentionally absorbed by that slack)
        // and ask for exactly those rows' covers or collages.
        let first = lane_first.saturating_sub(2).min(self.lane.rows.len());
        let visible = (self.body_height().max(0.0) / theme::SIDEBAR_ROW_PITCH).ceil() as usize + 5;
        let end = (first + visible).min(self.lane.rows.len());
        let mut quoted: Vec<u64> = Vec::new();
        let mut lane_records = Vec::new();
        for touched in &self.lane.rows[first..end] {
            match touched.subject {
                crate::lane::Subject::Record(id) => lane_records.push(id),
                crate::lane::Subject::Playlist(id) => {
                    if let Some(row) = self.playlists.row(id) {
                        quoted.extend_from_slice(&row.art);
                    }
                }
            }
        }
        if std::env::var_os("BAZ_PERF_LOG").is_some() {
            crate::baz_log!(
                "[art] lane rows={} window={first}..{end} records={} collage-records={}",
                self.lane.rows.len(),
                lane_records.len(),
                quoted.len(),
            );
        }
        // An open artist's records are the shelf's own, but *which* artist is
        // the shell's — so the id is read here and the records named below,
        // where both halves are in hand.
        let open_artist = match self.place {
            Place::Artist(id) => Some(id),
            _ => None,
        };
        let open_unsaved = self.place == Place::Queue;
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        // Each pin set belongs to one kind of current surface. A place change
        // retires the old set before the new surface's request below fills its
        // own; the handles return to the bounded LRU rather than being dropped.
        if self.place != Place::Library {
            state.thumbs.focus_wall(std::iter::empty());
        }
        if !matches!(
            self.place,
            Place::Playlists | Place::Playlist(_) | Place::Queue
        ) {
            state.thumbs.focus_page(std::iter::empty());
        }
        let mut ids = lane_records;
        if self.place == Place::Home {
            ids.extend(state.home_art());
            if let Some((path, _)) = crate::views::home::standing(&self.player, &self.resume)
                && let Some(album) = state.albums.iter().find(|album| {
                    album
                        .editions
                        .iter()
                        .flat_map(|edition| &edition.tracks)
                        .any(|track| track.path == path)
                })
            {
                ids.push(album.id);
            }
        }
        if self.playlists.panel_open {
            // The panel is a real visible sleeve consumer layered over any
            // non-Settings place. It has no independent cache and constructs
            // its complete (normally short) directory in one scrollable, so
            // pin the quotations it can expose, including All songs.
            ids.extend(state.all_songs().art);
            ids.extend(
                self.playlists
                    .rows
                    .iter()
                    .flat_map(|playlist| playlist.art.iter().copied()),
            );
        }
        if let Some(id) = open_artist {
            let theirs: Vec<u64> = crate::views::artist::records(state, id)
                .iter()
                .map(|album| album.id)
                .collect();
            ids.extend(theirs);
            ids.extend(state.artist_also_on(id).into_iter().map(|album| album.id));
        }
        if open_unsaved {
            ids.extend(views::queue::unsaved_art(state, &self.player));
        }
        if let Some(id) = self.player.playing_album() {
            ids.push(id);
        }
        ids.extend(quoted);
        state.request_thumbs_for(&ids)
    }

    /// **The sounding record's hero decode**, asked for after every message
    /// and answered at most once per record (doc 12 §5.2).
    ///
    /// Placed beside [`Self::request_offscreen_art`] in [`Self::update`] rather
    /// than hung off `TrackStarted`, for a reason that is a bug avoided rather
    /// than a preference: the engine can confirm a track before the scan has
    /// resolved its album, and a one-shot request on the event would leave that
    /// record on its 320 px thumbnail for the whole session. Asked every
    /// message, the ask **self-heals** — the first message after the library
    /// knows the record gets it — and the cost of not needing one is an
    /// `Option` compare and a hash lookup ([`Shelf::request_hero`] is the
    /// guard).
    ///
    /// **Not gated on being in the place while an album object is selected.**
    /// That keeps the chosen 2D/3D surface ready to open. `None` deliberately
    /// declines the decode: when no album object is drawn, paying artwork work
    /// in advance would make its zero-cost claim false. Album detail remains
    /// independent and always requests its own visible hero.
    fn request_hero(&mut self) -> Task<Message> {
        // A record page needs a detail-sized sleeve just as Now playing does.
        // Prefer the thing visibly occupying the page; the sounding record is
        // requested again as soon as that page is left.
        let sounding = hero_target(
            self.place,
            self.player.playing_album(),
            self.visualization.foreground,
        );
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        state.request_hero(sounding)
    }

    fn request_artist_image(&mut self, artist: u64) -> Task<Message> {
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        state.request_artist_image(artist)
    }

    /// Re-merge [`Self::lane`] if either half has been rebuilt since it was
    /// last built.
    ///
    /// The playlists half is *every* list, always — that is what lets the
    /// panel stop being the only index — and the records half is the shelf's
    /// already-trimmed 24. Both arrive pre-sorted; the merge re-sorts the
    /// union because a merge of two sorted lists on one key is a sort of the
    /// union and spelling it as one is spelling it once.
    fn sync_lane(&mut self) {
        let shelf_stamp = match &self.screen {
            Screen::Shelf(state) => state.lane_stamp,
            Screen::Setup(_) | Screen::Blocked(_) => 0,
        };
        let mark = (shelf_stamp, self.playlists.stamp());
        if mark == self.lane_mark {
            return;
        }
        self.lane_mark = mark;
        let lists: Vec<crate::lane::Touched> = self
            .playlists
            .rows
            .iter()
            .map(|entry| crate::lane::Touched {
                subject: crate::lane::Subject::Playlist(entry.id),
                name: entry.name.clone(),
                under: entry.counts(),
                // The later of the file's mtime and this run's play — both are
                // ways of touching a list (`Playlists::touched`).
                at: self.playlists.touched(entry),
            })
            .collect();
        let records = match &self.screen {
            Screen::Shelf(state) => state.lane_recent.clone(),
            Screen::Setup(_) | Screen::Blocked(_) => Vec::new(),
        };
        self.lane = crate::lane::resolve(lists, records);
    }

    /// **The lists the ledger says were played**, credited at launch — the
    /// cross-quit half of the owner's attribution defect (ADR-0034).
    ///
    /// The owner: *"when I play a song from a playlist it should only bump the
    /// recency of that playlist, not the underlying albums"*. The live half
    /// has worked since `lane::played_list`: a run reified from a list touches
    /// the **list** and not the records it quotes. It could not reach across a
    /// quit, because `Playlists::played` is not persisted and the only thing
    /// baz writes about what was played is the play ledger — which was per
    /// *path*, and never told a run's provenance.
    ///
    /// It is now. Each `# baz run` marker names the list its plays came from,
    /// so this is the same attribution, folded out of the file instead of held
    /// in memory. Runs arrive in the order they happened, so the last one to
    /// name a list is the one whose moment stands.
    ///
    /// Once, at launch, over a snapshot already in memory — the same budget
    /// `fold_history_onto_records` pays, and for the same reason: what the
    /// lane's contract forbids is paying it *per frame*.
    fn credit_the_lists_that_were_played(&mut self) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        let Some(history) = state.history.as_ref() else {
            return;
        };
        // Collected before anything is credited, because the ledger is
        // borrowed out of the screen and the lists are not.
        let played: Vec<(u64, u64)> = history
            .runs()
            .iter()
            .filter_map(|run| {
                let at = run.last_played_unix_s?;
                let origin = crate::origin::Origin::decode(run.origin.as_deref()?)?;
                match crate::lane::subject_of(&origin)? {
                    crate::lane::Subject::Playlist(id) => Some((id, at)),
                    // A record's run is already the lane's records half, folded
                    // out of the play lines themselves. Crediting it here would
                    // be the same fact counted twice.
                    crate::lane::Subject::Record(_) => None,
                }
            })
            .collect();
        let runs = played.len();
        for (id, at) in played {
            self.playlists.note_played(id, at);
        }
        if runs > 0 {
            crate::baz_log!("[history] {runs} list runs credited from the ledger");
        }
    }

    /// **Collapse the lane, or open it** — the one press whose subject is the
    /// collection's width.
    ///
    /// A **hard cut, one frame** (ADR-0030 §3.1): the state flips, the wall is
    /// re-hung once, and the wall keeps the *shelf* that was at the top of the
    /// viewport rather than its pixel offset. No tween — tweening the width
    /// would re-resolve `Grid::new` on every frame of the slide and pop
    /// columns mid-flight.
    ///
    /// Inert below [`theme::SIDEBAR_FLOOR`]: there is nothing to toggle when
    /// the window can only hold the rail, and the mark says so in its ink.
    fn toggle_lane(&mut self) -> Task<Message> {
        self.set_lane(!self.lane_open)
    }

    /// Put the lane in `open` — from the marks at its foot or
    /// <kbd>Ctrl</kbd>+<kbd>B</kbd> — persisting the state and re-hanging the
    /// wall.
    ///
    /// It does nothing at all when the window cannot hold the expanded lane
    /// ([`theme::sidebar_can_expand`]) or when the lane is already in the state
    /// asked for. That second guard is what keeps the re-hang to the presses
    /// whose subject is the collection's width.
    fn set_lane(&mut self, open: bool) -> Task<Message> {
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        if !theme::sidebar_can_expand(state.window_w) || self.lane_open == open {
            return Task::none();
        }
        self.lane_open = open;
        state.lane_open = open;
        persist_lane(open);
        state.rehang()
    }

    /// <kbd>/</kbd> and <kbd>Ctrl</kbd>+<kbd>F</kbd>: focus the resident app-bar
    /// well without changing the place underneath it.
    fn focus_the_well(&mut self) -> Task<Message> {
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        if !state.query.trim().is_empty() {
            state.search_open = true;
        }
        self.menu = None;
        self.status_open = false;
        iced::widget::operation::focus(search_id())
    }

    /// **Type anywhere** (ADR-0017 §1.2): append into the resident app-bar
    /// field and reveal results over the current place.
    fn type_anywhere(&mut self, text: &str) -> Task<Message> {
        self.menu = None;
        self.status_open = false;
        match &mut self.screen {
            Screen::Shelf(state) => state.type_into_query(text),
            Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
        }
    }

    /// **Go somewhere**, by whichever door was pressed.
    ///
    /// One function for all three because they are the same act: a door is a
    /// pure transition on [`Place`], and the only thing the shell adds is the
    /// rule that there must be a shelf to leave. The first-run screen has no
    /// places, so a media key or a stray binding cannot navigate away from the
    /// folder question.
    fn go(&mut self, door: impl FnOnce(Place) -> Place) -> Task<Message> {
        if let Screen::Shelf(state) = &mut self.screen {
            // A menu is about something *in* the place it was opened over;
            // it does not survive the place leaving (a keyboard door can
            // navigate under an open menu — the pointer routes all close it
            // on their own press). A drag is about rows in the place, so
            // the same rule discards it — a keyboard door mid-hold must not
            // leave a ghost over a place with no rows to land on.
            self.menu = None;
            self.status_open = false;
            self.drag = None;
            // **And the hovered tile, for the same reason.** `TileLeft` is
            // published by a `mouse_area` the pointer actually leaves, so
            // navigating *out from under* the pointer — a keyboard door, or
            // the tile's own press — leaves the mark set on a record the
            // pointer is no longer near. That was invisible while the wall
            // was the only surface drawing tiles, because coming back put the
            // pointer where it had left it. It stopped being invisible the
            // moment a second and third place drew the wall's own tile: Home's
            // `RECENTLY ADDED` row and the Artist place would offer a
            // record's hover options unbidden, on the record you had happened
            // to touch on the way out.
            state.hovered_album = None;
            // Home's `All songs` tile is the same case exactly: its own press
            // navigates out from under the pointer, so without this the veil
            // would be waiting on it when you came back.
            state.hovered_all_songs = false;
            let from = self.place;
            self.place = door(self.place);
            self.place_history.visit(self.place);
            if self.place == Place::Playlists && from != Place::Playlists {
                self.playlists_scroll = 0.0;
            }
            return self.note_place_left(from);
        }
        Task::none()
    }

    fn open_playlist_creation(
        &mut self,
        mode: Option<crate::playlists::CreationMode>,
    ) -> Task<Message> {
        self.playlists.begin_creation();
        if let Some(mode) = mode {
            self.playlists.creation.mode = Some(mode);
            if mode == crate::playlists::CreationMode::Vibe {
                let prompt = match &self.screen {
                    Screen::Shelf(state) => state.vibe.prompt.clone(),
                    Screen::Setup(_) | Screen::Blocked(_) => String::new(),
                };
                self.playlists.suggest_creation_name(&prompt);
            }
        }
        self.go(|_| Place::NewPlaylist)
    }

    fn save_playlist_creation(&mut self) -> Task<Message> {
        let generated = match self.playlists.creation.mode {
            Some(crate::playlists::CreationMode::Manual) => None,
            Some(crate::playlists::CreationMode::Vibe) => match &self.screen {
                Screen::Shelf(state) => state.vibe.preview.clone(),
                Screen::Setup(_) | Screen::Blocked(_) => None,
            },
            None => return Task::none(),
        };
        let id = match &self.screen {
            Screen::Shelf(state) => self
                .playlists
                .save_creation(generated.as_ref(), &state.library),
            Screen::Setup(_) | Screen::Blocked(_) => None,
        };
        let Some(id) = id else {
            return Task::none();
        };
        let opened = match &self.screen {
            Screen::Shelf(state) => self.playlists.open_page(id, &state.library),
            Screen::Setup(_) | Screen::Blocked(_) => false,
        };
        if !opened {
            return Task::none();
        }
        if let Screen::Shelf(state) = &mut self.screen {
            state.vibe.close();
            state.selection.select(Content::Playlist(id));
        }
        self.playlist_scroll = 0.0;
        self.go(|place| place.playlist(id))
    }

    /// Walk the existing history cursor without recording a new visit.
    ///
    /// A vanished subject is resolved through the same safe fallback as a
    /// restored session. The cursor still moves — otherwise an old album that
    /// was deleted during a scan could trap the listener between two arrows.
    fn travel_history(&mut self, backward: bool) -> Task<Message> {
        let target = if backward {
            self.place_history.back()
        } else {
            self.place_history.forward()
        };
        let Some(target) = target else {
            return Task::none();
        };
        self.menu = None;
        self.status_open = false;
        self.drag = None;
        {
            let Screen::Shelf(state) = &mut self.screen else {
                return Task::none();
            };
            state.hovered_album = None;
            state.hovered_all_songs = false;
        }
        let from = self.place;
        self.restore_place(target);
        if self.place == Place::Playlists && from != Place::Playlists {
            self.playlists_scroll = 0.0;
        }
        self.note_place_left(from)
    }

    /// **Open a record's page** — an explicit Open control, or source
    /// navigation from Now playing and the persistent bar.
    ///
    /// Two things happen and they are deliberately separable: the *place*
    /// changes, and the wall remembers which record you left it for
    /// ([`Shelf::opened`]). The second is the whole mitigation for the round
    /// trip a page costs that a column did not — when <kbd>Esc</kbd> brings you
    /// back, the wall is where you left it with the record you were reading
    /// marked, so returning is *return* rather than re-find.
    fn open_album(&mut self, id: u64) -> Task<Message> {
        // Repeating an explicit Open leaves every bit of page and shelf state
        // untouched.
        if self.place == Place::Album(id) {
            return Task::none();
        }
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        state.opened = Some(id);
        state.selection.select(Content::Album(id));
        // The place changes, so an open menu and any drag go with it
        // (`go`'s rule).
        self.menu = None;
        self.status_open = false;
        self.drag = None;
        let from = self.place;
        self.place = self.place.album(id);
        self.place_history.visit(self.place);
        // A record's page, never `Now playing` — the task is `Task::none()`
        // by construction, and returning it keeps that true if the door ever
        // changes where it lands.
        self.note_place_left(from)
    }

    /// **Go home** — every place's `‹ Library`, and the first thing
    /// <kbd>Esc</kbd> does.
    ///
    fn leave(&mut self) -> Task<Message> {
        // The place changes, so an open menu and any drag go with it
        // (`go`'s rule).
        self.menu = None;
        self.status_open = false;
        self.drag = None;
        let from = self.place;
        self.place = Place::Library;
        self.place_history.visit(self.place);
        let entering = self.note_place_left(from);
        // A place's transient fields do not outlive the place: a rename
        // field left standing behind a navigation would greet the next
        // visit mid-gesture.
        if let Some(open) = &mut self.playlists.open {
            open.renaming = None;
            open.confirming_delete = false;
        }
        self.playlists.saving_queue = None;
        entering
    }

    /// <kbd>Esc</kbd>'s place-level share of the peel: the transient field
    /// standing *on* the current place — a playlist rename mid-type or delete
    /// confirmation — takes one press before the place itself leaves.
    fn peel_place_states(&mut self) -> bool {
        match self.place {
            Place::Home => match &mut self.screen {
                Screen::Shelf(state) if state.vibe.open => {
                    state.vibe.close();
                    true
                }
                Screen::Setup(_) | Screen::Blocked(_) | Screen::Shelf(_) => false,
            },
            Place::Queue => self.playlists.saving_queue.take().is_some(),
            Place::Playlist(_) => {
                let Some(open) = &mut self.playlists.open else {
                    return false;
                };
                if open.renaming.take().is_some() {
                    true
                } else {
                    std::mem::take(&mut open.confirming_delete)
                }
            }
            _ => false,
        }
    }

    /// <kbd>Esc</kbd>: **peel one layer, top down.**
    ///
    /// Shorter than it has ever been, because there are fewer layers than there
    /// have ever been. ADR-0016 had a popover over an inspector over a place and
    /// spent one rule on each; ADR-0022 left one kind of surface, so the key's
    /// whole first question is *am I at home*:
    ///
    /// 1. **The context menu**, when one stands (doc 09 §5.2): it opens at
    ///    the pointer over everything — the panel included — so it is the
    ///    outermost layer and the first one down.
    /// 2. **The playlist panel's layers**, when it is summoned: its name
    ///    field, a pick in flight, then the panel — it floats over every
    ///    place it exists in ([`crate::playlists::Playlists::peel`]; the
    ///    armed layer died with the collecting mode, 09 §9).
    /// 3. **The place's own transient fields** ([`Self::peel_place_states`]):
    ///    a rename mid-type, an armed delete, the queue's save field.
    /// 4. **The place**, when it is not the Library. Backing out is what
    ///    <kbd>Esc</kbd> means in a record's page, in the queue and in the
    ///    settings alike, and it is the same press as their `‹ Library`.
    /// 5. Then the Library's own layer: the search query.
    ///
    /// (In the search field itself iced 0.13's `text_input` consumes
    /// <kbd>Esc</kbd> to blur before this is reached at all; that is the
    /// documented two-press behaviour, and §4.6 of the design spec owns the
    /// fix.)
    fn escape(&mut self) -> Task<Message> {
        // Fullscreen is a window layer around every place. Leave it before
        // changing the place or peeling any in-place layer, so the kiosk's
        // first Escape returns the same Now Playing composition to its prior
        // window instead of unexpectedly navigating away.
        if self.fullscreen {
            self.fullscreen = false;
            return latest_window(|id| window::set_mode(id, window::Mode::Windowed));
        }
        // A drag in flight peels before every layer: the hand is
        // mid-gesture, and Esc is the gesture's one explicit discard —
        // the lifted row goes back, nothing is sent ([`crate::drag`];
        // commit belongs to the release, never to Esc).
        if self.drag.take().is_some() {
            return Task::none();
        }
        // The context menu is the outermost layer wherever it stands — it
        // floats over the panel itself — so it peels before everything, one
        // layer per press (doc 09 §5.2).
        if self.menu.take().is_some() {
            return Task::none();
        }
        if self.status_open {
            self.status_open = false;
            if let Screen::Shelf(state) = &mut self.screen {
                state.health.acknowledge();
            }
            return Task::none();
        }
        if let Screen::Shelf(state) = &mut self.screen
            && state.search_open
        {
            return state.clear_query();
        }
        // The playlist panel floats *over* every place it exists in, so its
        // layers peel first: the name field, a pick in flight, the panel
        // itself — one per press (ADR-0024 §5–§6, as amended by doc 09).
        if self.playlists.peel() {
            return Task::none();
        }
        // Then whatever transient field is standing on the place itself.
        if self.peel_place_states() {
            return Task::none();
        }
        if !self.place.is_library() {
            return self.leave();
        }
        match &mut self.screen {
            Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
            Screen::Shelf(state) => state.update(Message::EscapePressed),
        }
    }

    /// Setup → Shelf transition: send the typed folder off to be looked at
    /// on the **blocking pool** ([`check_folder`]), coming back as
    /// [`Message::MusicFolderChecked`].
    ///
    /// It used to `stat` right here, on the UI thread — the defect ADR-0025
    /// §3 cited when it deferred the picker from this screen. Reusing the
    /// Settings door's off-thread look removes the stat instead of
    /// inheriting it (doc 11 §5 P1): a typed path can name the share that is
    /// configured but not mounted, and against a dead hard mount that stat
    /// sits for minutes.
    fn submit_setup(&mut self) -> Task<Message> {
        let Screen::Setup(setup) = &mut self.screen else {
            return Task::none();
        };
        let dir = expand_tilde(setup.input.trim());
        if dir.as_os_str().is_empty() {
            return Task::none();
        }
        Task::perform(check_folder(dir), Message::MusicFolderChecked)
    }

    /// Fold a bridge message into the state machine, with a stdout trace of
    /// the notable per-track moments (matching the `[scan]`/`[config]` log
    /// style).
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive fold over the engine protocol; splitting event arms would hide \
                  the shared apply/publish order that makes engine events playback truth"
    )]
    fn apply_player_event(&mut self, message: PlayerEvent) -> Task<Message> {
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
        let mut show_now_playing = false;
        match message {
            PlayerEvent::Engine(event) => {
                let volume_confirmed = matches!(&event, Event::VolumeChanged { .. });
                match &event {
                    Event::TrackStarted { path, position } => {
                        self.fact_index = 0;
                        crate::baz_log!(
                            "[playback] track started (queue #{position}): {}",
                            path.display()
                        );
                        // **The lane's one live update** (ADR-0030 §4): a
                        // play moves one row to the head, and 24 rows are
                        // re-sorted. The ledger is not re-read — it is a
                        // snapshot taken at launch, and re-reading it here
                        // would be the per-frame file read the contract
                        // refuses. The moment is now; the two agree to within
                        // the length of the play.
                        //
                        // **Which row it moves is the run's provenance, not
                        // the track's path** — `lane::played_subject` carries
                        // the owner's defect and the argument. A run reified
                        // from a list touches the *list*; every other origin
                        // touches the record.
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |since| since.as_secs());
                        match crate::lane::played_list(self.player.queue_provenance()) {
                            Some(id) => {
                                self.playlists.note_played(id, now);
                            }
                            None => {
                                if let Screen::Shelf(state) = &mut self.screen {
                                    state.record_played(path, now);
                                }
                            }
                        }
                        // Command acceptance is not playback truth. Spend the
                        // pending destination only when this event belongs to
                        // the run the deliberate start requested.
                        if self
                            .show_on_start
                            .as_ref()
                            .is_some_and(|paths| paths.contains(path))
                        {
                            self.show_on_start = None;
                            show_now_playing = true;
                        }
                    }
                    Event::TrackFailed { path, reason } => {
                        crate::baz_log!("[playback] track skipped: {} ({reason})", path.display());
                        if let Screen::Shelf(state) = &mut self.screen {
                            state.health.record(
                                crate::health::Level::Warning,
                                "Track could not be played",
                                format!("{}\n{reason}", path.display()),
                            );
                        }
                    }
                    Event::QueueEnded => {
                        crate::baz_log!("[playback] queue ended");
                        self.show_on_start = None;
                        self.signal_warning.clear();
                        // The run the history described is over — the third
                        // of P2's three ends for an edit history (next
                        // edit, navigation, the run ending).
                        self.queue_undo.clear();
                    }
                    Event::Stopped => {
                        self.signal_warning.clear();
                    }
                    // The resident readout stays factual; an active Baz-owned
                    // resampler is additionally an actionable, deduplicated
                    // warning in the canonical event history.
                    Event::SignalPath {
                        source_rate_hz,
                        source_channels,
                        source_bits,
                        output_rate_hz,
                        chain,
                    } => {
                        let path = SignalPath {
                            source_rate_hz: *source_rate_hz,
                            source_channels: *source_channels,
                            source_bits: *source_bits,
                            output_rate_hz: *output_rate_hz,
                            chain: *chain,
                        };
                        if let Some(warning) = self.signal_warning.observe(path)
                            && let Screen::Shelf(state) = &mut self.screen
                        {
                            state.health.record(
                                crate::health::Level::Warning,
                                warning.title,
                                warning.detail,
                            );
                        }
                        let depth =
                            source_bits.map_or_else(String::new, |bits| format!("/{bits}-bit"));
                        // Named only when there is something to say: a
                        // multichannel file is being folded to stereo and the
                        // log line is where that is admitted (ADR-0039).
                        let fold = if *source_channels > baz_core::playback::CHANNELS {
                            format!("/{source_channels}ch downmixed")
                        } else {
                            String::new()
                        };
                        let doing = match chain {
                            SignalChain::Direct => "direct".to_string(),
                            SignalChain::Converting { reason } => {
                                format!("converting ({reason:?})")
                            }
                            other => format!("{other:?}"),
                        };
                        crate::baz_log!(
                            "[playback] signal path: {source_rate_hz} Hz{depth}{fold} source -> \
                             {output_rate_hz} Hz output, {doing}"
                        );
                    }
                    Event::PlayRecorded { .. } => {
                        if let Screen::Shelf(state) = &mut self.screen {
                            state.history = read_history();
                        }
                    }
                    _ => {}
                }
                let albums: &[vm::AlbumVm] = match &self.screen {
                    Screen::Shelf(state) => &state.albums,
                    Screen::Setup(_) | Screen::Blocked(_) => &[],
                };
                self.player.apply(&event, albums);
                seek_confirmed = seek_pending && matches!(event, Event::Progress { .. });
                // Persist off the confirmation, never off the request: what
                // reaches config.toml is what the engine put in force,
                // including a pre-amp it clamped on the way in.
                self.persist_replay_gain();
                if volume_confirmed {
                    self.persist_volume();
                }
            }
            PlayerEvent::Closed => {
                crate::baz_log!("[playback] engine shut down");
                self.show_on_start = None;
                self.volume_wheel_settles = None;
                self.player.engine_closed();
            }
        }
        self.warm_lamp(lit, Instant::now());
        self.publish_mpris(seek_confirmed);
        if show_now_playing {
            self.go(|place| place.go(crate::lane::Destination::NowPlaying))
        } else {
            Task::none()
        }
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

    /// Write the fader position the engine has just confirmed, if it moved.
    ///
    /// `VolumeChanged` also reports mute and output-path changes. Comparing
    /// only the control position means those independent facts never turn
    /// into config writes, while drag, keyboard and MPRIS volume gestures all
    /// share this one confirmation-driven persistence path.
    fn persist_volume(&mut self) {
        if self.player.volume_gesture_active() || self.volume_wheel_settles.is_some() {
            return;
        }
        let volume = self.player.volume();
        if volume == self.saved_volume {
            return;
        }
        self.saved_volume = volume;
        persist(|config| config.volume = volume);
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
    /// The player resolves the gesture to a current-song timestamp; this
    /// method only dispatches the resulting `Seek` command.
    fn update_needle(&mut self, message: &Message) -> bool {
        match *message {
            Message::NeedlePressed(pointer) => {
                self.player.press(pointer);
            }
            Message::NeedleDragged(pointer) => self.player.drag_to(pointer),
            Message::NeedleHovered(pointer) => {
                self.player.hover_to(pointer);
            }
            Message::NeedleLeft => self.player.hover_left(),
            Message::NeedleReleased => {
                let position_ms = self.player.release_drag();
                self.send_seek(position_ms);
            }
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
            Message::VolumeReleased => {
                self.player.release_volume();
                // Confirmed positions heard during the drag were deliberately
                // not written one pixel at a time. Commit the latest one when
                // the hand lets go; a final in-flight confirmation will update
                // it once more when it arrives.
                self.persist_volume();
            }
            Message::VolumeWheel(steps) => {
                let target = self.player.step_volume(steps);
                self.send_volume(target);
                if target.is_some() && self.player.engine_ready() {
                    self.volume_wheel_settles = Some(Instant::now() + VOLUME_WHEEL_SETTLE);
                }
            }
            Message::VolumeWheelSettled(now) => {
                if self.volume_wheel_settles.is_some_and(|at| now >= at) {
                    self.volume_wheel_settles = None;
                    self.persist_volume();
                }
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
    /// disc/track order), ask it to play, and arrange to show Now Playing only
    /// after the engine confirms that one of its tracks began.
    fn play_album(&mut self, id: u64) -> bool {
        let Screen::Shelf(state) = &self.screen else {
            return false;
        };
        let Some(album) = state.albums.iter().find(|album| album.id == id) else {
            return false;
        };
        let queue = vm::album_queue(album, state.edition_choice.get(&id).copied());
        if queue.is_empty() {
            return false;
        }
        self.start_and_show(queue)
    }

    /// One request path for a run whose successful start should become the
    /// visible Now Playing place. Channel acceptance is necessary but not
    /// sufficient: the path set is held until a matching `TrackStarted`.
    fn start_and_show(&mut self, queue: vm::QueueVm) -> bool {
        let paths = queue.paths();
        if self.send_run(queue, None).is_some() && self.playback.send(Command::Play) {
            self.player.note_transport_sent();
            self.show_on_start = Some(paths);
            // A queue where there was none moves `CanPlay`, and that is the
            // one MPRIS-visible change that arrives without an engine event.
            self.publish_mpris(false);
            true
        } else {
            self.player.engine_closed();
            false
        }
    }

    /// Apply the same confirmation boundary to a search needle-drop, whose
    /// queue may already have been held and therefore did not pass through
    /// [`Self::start_and_show`].
    fn show_current_run_on_start(&mut self) {
        self.show_on_start = self.player.queue().map(vm::QueueVm::paths);
    }

    /// **Append the record to the run** — a shift-click on its sleeve (or on
    /// any control that opens its page), the one-press accelerator over the
    /// picker's **Queue** row (ADR-0023 §3's stack; doc 09 §13 step 7).
    ///
    /// The visible-control rule (a standing rule of the product: no action's only route
    /// is a gesture) is satisfied by the picker's Queue row: `Add to playlist…` on
    /// the record's page → the picker's first row sends the identical
    /// append, on screen, in two presses — this gesture is an accelerator
    /// over that control, exactly as a key binding is over a button, and it
    /// resolves to the same act ([`Self::append_to_run`]'s one shape).
    ///
    /// **Nothing sounds unasked**: an append is `UpdateQueue`, never a play
    /// gesture — the music keeps playing, the record joins the tail as its
    /// own headed group (albums listed as albums, never flattened,
    /// ADR-0014), and appending to an empty stopped engine loads the queue
    /// without starting it.
    fn queue_album(&mut self, id: u64) -> Task<Message> {
        let Screen::Shelf(state) = &self.screen else {
            return Task::none();
        };
        let Some(album) = state.albums.iter().find(|album| album.id == id) else {
            return Task::none();
        };
        let addition = vm::album_queue(album, state.edition_choice.get(&id).copied());
        if addition.is_empty() {
            return Task::none();
        }
        self.append_to_run(addition);
        Task::none()
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
    fn play_track(&mut self, id: u64, row: usize) -> bool {
        let Screen::Shelf(state) = &self.screen else {
            return false;
        };
        let Some(album) = state.albums.iter().find(|album| album.id == id) else {
            return false;
        };
        let chosen = state.edition_choice.get(&id).copied();
        let Some(edition) = vm::selected_edition(album, chosen) else {
            return false;
        };
        // The list the row was drawn from and the list that would be queued
        // come from the same `selected_edition`, so "is this album the queue"
        // is asked about exactly what the user clicked.
        let Some(decision) = self.player.play_from(&edition.tracks, row) else {
            return false;
        };
        let position = match decision {
            player::PlayFrom::Jump { position } => position,
            player::PlayFrom::Requeue { position } => {
                let queue = vm::album_queue(album, chosen);
                if queue.is_empty() {
                    return false;
                }
                // The row the click named, handed to the one arranger: with
                // shuffle off it is the position to jump to; with shuffle on
                // the track is hoisted to the front and the answer is 0.
                let Some(at) = self.send_run(queue, Some(position)) else {
                    return false;
                };
                at
            }
        };
        if self.playback.send(Command::JumpTo { position }) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
            return false;
        }
        // A queue where there was none moves `CanPlay`, exactly as in
        // `play_album`, and that is the one MPRIS-visible change that arrives
        // without an engine event.
        self.publish_mpris(false);
        true
    }

    /// **Send a run, and say how it is to be walked.**
    ///
    /// The one place a `SetQueue` that *starts* something goes out, which is
    /// what makes "`Play` on a record, `Play all`, a playlist's `Play` and a
    /// track click all agree" a structural fact rather than four functions
    /// keeping a convention. Each caller builds the queue its own gesture
    /// means, in the order that gesture means, and hands it here.
    ///
    /// **What happens to that order is: nothing.** This function used to
    /// permute the run when the mode was on and keep a copy of what it had
    /// permuted; the owner's reading of shuffle — *"going to an unknown next
    /// track rather than actually mutating the track list"* — took both away.
    /// The run goes out as built, in every mode, and the mode goes out beside
    /// it as a traversal the engine walks by (`baz_core::traversal`).
    ///
    /// A **fresh seed per run**, because the same seed over a re-played record
    /// would be the same shuffle twice.
    ///
    /// `lead` is a row the gesture named — a track click. It needs no special
    /// handling any more: starting at a row and continuing by the plan is what
    /// the engine does with `JumpTo`, so *this one, then whatever* is one
    /// command rather than a hoist and a permutation.
    ///
    /// Answers **the position playback should start at**: the named row, or the
    /// head of the pass for a plain `Play`. `None` when the engine would not
    /// take the queue, which is the caller's cue to stop rather than to send a
    /// transport command into a run that does not exist.
    fn send_run(&mut self, queue: vm::QueueVm, lead: Option<usize>) -> Option<usize> {
        // Any new run supersedes a still-unconfirmed start. A late event from
        // that run must not navigate after the listener chose another one.
        self.show_on_start = None;
        let origin = run_origin(&queue);
        // **A fresh pass per run**, and only when the mode is on. The same seed
        // over a re-played record would be the same shuffle twice, which is the
        // one thing about a shuffle a listener notices immediately.
        if self.player.shuffle() {
            let traversal = Traversal::Shuffled { seed: draw_seed() };
            if !self.playback.send(Command::SetTraversal { traversal }) {
                self.player.engine_closed();
                return None;
            }
            self.player.note_traversal(traversal);
        }
        // **The queue goes out exactly as the gesture built it, in every mode.**
        // There is no branch here any more and that is the reduction: what
        // shuffle changes is the walk, which the engine was told about above.
        let paths = queue.paths();
        if !self.playback.send(Command::SetQueue { paths, origin }) {
            self.player.engine_closed();
            return None;
        }
        self.player.note_queue_sent(queue);
        // The row the gesture named is the row to start on — under either mode.
        // It used to be hoisted to the front of a permuted list so that a click
        // could mean *this one* and *then whatever*; a traversal means both by
        // construction, because starting at a row and continuing by the plan is
        // exactly what the engine does with `JumpTo`.
        Some(lead.unwrap_or_else(|| self.player.first_of_the_pass()))
    }

    /// **Turn shuffle on or off** — the now-playing bar's crossed arrows
    /// (the owner, 2026-08-10: *"can you make shuffle a property of the player
    /// i.e. toggle on/off"*, and *"shuffle as a concept is more about going to
    /// an unknown next track rather than actually mutating the track list"*).
    ///
    /// Three things, in this order: the engine is told how to walk, the standing
    /// decision is written to `config.toml`, and this process records the same
    /// traversal so that what it draws and what the engine plays are one answer.
    ///
    /// **The queue is not touched, in either direction.** That is the whole
    /// shape of the second decision: shuffle was a permutation this function
    /// applied to the run and undid from a retained copy, and it is now a
    /// property of the walk — so *on* sends a fresh pass and *off* sends
    /// `InOrder`, and the run is in its own order again because it never left
    /// it. Everything the old version needed to be careful about — a retained
    /// order that a delete could stale, an append that had to survive the
    /// restore, a run with no order to go back to — is gone rather than handled.
    ///
    /// **Nothing stops.** `SetTraversal` lets the sounding track play to its end
    /// and continues on the new plan after it (`baz_core::traversal`), which is
    /// the bargain `UpdateQueue` already made and the same one boundary's cost.
    ///
    /// A press with nothing playing moves the property and writes it, and that
    /// is the whole of what there is to do: the mode is about what plays
    /// **next**.
    fn toggle_shuffle(&mut self) {
        let on = !self.player.shuffle();
        let traversal = traversal(on);
        if !self.playback.send(Command::SetTraversal { traversal }) {
            self.player.engine_closed();
            return;
        }
        self.player.note_traversal(traversal);
        persist_shuffle(on);
        crate::baz_log!(
            "[shuffle] {} \u{2014} the run keeps its own order; the walk changed",
            if on { "on" } else { "off" }
        );
        self.publish_mpris(false);
    }

    /// **Cycle the one Repeat control** through the three states every player
    /// has, in the order they are universally cycled: off → the list → this
    /// track → off.
    ///
    /// One control rather than two, because a listener asks *"does this go
    /// round?"* once and the answer has three values, not two independent
    /// booleans that can contradict each other.
    fn cycle_repeat(&mut self) {
        use baz_core::protocol::Repeat;
        let repeat = match self.player.repeat() {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        };
        if !self.playback.send(Command::SetRepeat { repeat }) {
            self.player.engine_closed();
            return;
        }
        // Mirror immediately, as shuffle does, so the resident control answers
        // the accepted press without waiting a frame for its confirmation.
        self.player.seed_repeat(repeat);
        persist(|config| config.repeat = repeat);
        self.publish_mpris(false);
    }

    /// **Play everything you own** — Home's `All songs` tile (the owner,
    /// 2026-08-10: *"again I wanted the Play all, to be more like a tile on the
    /// home screen, a special 'playlist'"*).
    ///
    /// It resolves the implicit `everything` list and plays it — the list
    /// type, the origin, the queue shape and the arranger are
    /// `crate::implicit`'s, which is the reason this is four lines rather than
    /// a gesture of its own.
    ///
    /// **Why Home's tile does not read the wall's query.** The strip's
    /// `Play all` lived beside the query and the arrangement that decide the
    /// wall, and its contract was *exactly what you can see*. Home shows no
    /// wall, and the strip's control is gone besides. A tile
    /// there that applied a filter set on another page would be acting on state
    /// the listener cannot see or clear from where they are standing — the same
    /// rule, on a surface where "what you can see" is a different set. What this
    /// tile will play is stated on the tile, in its counts line.
    fn play_everything(&mut self) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        let list = state.everything();
        if list.is_empty() {
            // An empty library. Nothing to play, so nothing happens and nothing
            // is claimed — the rule every play gesture in baz keeps.
            return;
        }
        crate::baz_log!("[all-songs] play everything — {}", list.counts());
        self.start(list);
    }

    /// Play the open artist's implicit `All songs` list.
    fn play_artist_songs(&mut self, artist: u64) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        let Some(list) = state.artist_songs(artist) else {
            return;
        };
        if list.is_empty() {
            return;
        }
        crate::baz_log!(
            "[artist-songs] play {} — {}",
            list.origin.name(),
            list.counts()
        );
        self.start(list);
    }

    /// Send an implicit list's run and start it — the tail both `All songs`
    /// gestures share, so their one difference stays their scope.
    fn start(&mut self, list: crate::implicit::ImplicitList) {
        if self.send_run(list.queue, None).is_some() && self.playback.send(Command::Play) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
        self.publish_mpris(false);
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
        let Some((before, edited)) = self
            .player
            .queue()
            .and_then(|queue| Some((queue.clone(), queue_edit::without(queue, row)?)))
        else {
            return;
        };
        let paths = edited.paths();
        if self.playback.send(Command::UpdateQueue { paths }) {
            self.player.note_queue_edited(edited);
            // The list the edit replaced, kept for the place's `Undo`
            // (doc 11 §5 P2) — pushed only on an accepted send, so the
            // history never records an edit the engine never saw.
            self.queue_undo.push(before);
        } else {
            self.player.engine_closed();
        }
        // A queue emptied to nothing moves `CanPlay`, and that is the one
        // MPRIS-visible change an edit can make without an engine event.
        self.publish_mpris(false);
    }

    /// Swap row `row` with its neighbour `delta` away — a click on a row's
    /// ▲▼ stepper in **Queue** (doc 09 §8.2: the playlist page's reorder,
    /// grown onto the run's own editor).
    ///
    /// [`Self::remove_queued`]'s exact shape over
    /// [`queue_edit::shifted`]'s pure edit: the whole new queue as
    /// [`Command::UpdateQueue`], never a delta and never a `SetQueue` — the
    /// music keeps playing (ADR-0014's guarantee), the sounding row moves
    /// like any other, and the cursor follows its track because both sides
    /// find it again by path (the engine re-derives and announces
    /// [`baz_core::protocol::Event::QueueChanged`];
    /// until it does, [`vm::QueueVm::playing`] reconciles the same way).
    fn shift_queued(&mut self, row: usize, delta: i32) {
        let Some((before, edited)) = self
            .player
            .queue()
            .and_then(|queue| Some((queue.clone(), queue_edit::shifted(queue, row, delta)?)))
        else {
            return;
        };
        let paths = edited.paths();
        if self.playback.send(Command::UpdateQueue { paths }) {
            self.player.note_queue_edited(edited);
            // **A hand reorder needs nothing undone.** It used to drop the
            // order shuffle would return to, because shuffle owned an order of
            // its own that the hand had just contradicted. Shuffle owns no
            // order now — the run's order is the run's — so a stepper press is
            // an ordinary edit and turning shuffle off after one leaves the run
            // exactly as the press left it, by construction rather than by rule.
            // [`Self::remove_queued`]'s history rule, for the reorder.
            self.queue_undo.push(before);
        } else {
            self.player.engine_closed();
        }
        self.publish_mpris(false);
    }

    /// Everything the reorder **drag** says while it is in flight
    /// (doc 09 §13 step 8; [`crate::drag`] holds the state machine and the
    /// arithmetic, this routes). Its own small machine for the volume's
    /// reason: a handful of arms that belong to one fact — a single
    /// `Option` on the shell — kept out of the big match.
    fn update_drag(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::DragLift(list, index, at) => self.lift_row(*list, *index, *at),
            Message::DragMoved(at) => {
                if let Some(drag) = &mut self.drag {
                    drag.at = *at;
                }
            }
            Message::DragOverRow(list, index, before) => {
                if let Some(drag) = &mut self.drag
                    && drag.list == *list
                {
                    drag.over_row(*index, *before);
                }
            }
            Message::DragOverPanel(id) => {
                if let Some(drag) = &mut self.drag {
                    drag.over_panel = Some(*id);
                }
            }
            // Conditional, for [`Message::QueueRowLeft`]'s reason: entering
            // the next row and leaving the last arrive from one move, in
            // widget order.
            Message::DragLeftPanel(id) => {
                if let Some(drag) = &mut self.drag
                    && drag.over_panel == Some(*id)
                {
                    drag.over_panel = None;
                }
            }
            Message::DragDropped => self.drop_drag(),
            _ => return None,
        }
        Some(Task::none())
    }

    /// A row crossed the drag threshold: put it in the hand. The payload is
    /// read from the same record the row was drawn from — the queue's
    /// request-side record, the page's own queue shape — so what the drag
    /// holds is exactly what was pointed at, and a row a fresh edit just
    /// removed lifts nothing ([`crate::queue_edit`]'s stale-picture rule,
    /// applied at the lift).
    fn lift_row(&mut self, list: crate::drag::List, index: usize, at: Point) {
        self.drag = match list {
            crate::drag::List::Queue => self.player.queue().and_then(|queue| {
                let item = queue.items.get(index)?.clone();
                Some(crate::drag::DragState::begin(
                    list,
                    index,
                    queue.items.len(),
                    item.title.clone(),
                    Some(item),
                    at,
                ))
            }),
            crate::drag::List::Playlist => self.playlists.open.as_ref().and_then(|open| {
                let row = open.rows.get(index)?;
                // A missing entry reorders — its position is real — but
                // transfers nothing: no payload, so a panel drop is a no-op
                // (the `+`'s own rule, held by the drag).
                let payload = row
                    .playable_position
                    .and_then(|position| open.queue.items.get(position).cloned());
                Some(crate::drag::DragState::begin(
                    list,
                    index,
                    open.rows.len(),
                    row.title.clone(),
                    payload,
                    at,
                ))
            }),
        };
    }

    /// The drag ended: one commit, decided against the state the line and
    /// the ghost were drawn from — so what happens is what was on screen.
    /// A drop on a panel row appends to that file (the picker row's own
    /// append, made direct); anywhere else commits the insertion slot as
    /// one reorder; the no-op slot asks for nothing.
    fn drop_drag(&mut self) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        // `panel_on_screen` re-checked at the drop: a keyboard door can
        // dismiss the panel under a held pointer, and no exit event retires
        // `over_panel` for an unmounted row — the drop must not append to a
        // list that is no longer on screen.
        if let Some(id) = drag.over_panel
            && self.panel_on_screen()
        {
            if let (Some(item), Screen::Shelf(state)) = (drag.payload, &self.screen) {
                let entries = crate::playlists::entries_for_items(std::slice::from_ref(&item));
                self.playlists.append(id, entries, &state.library);
            }
            return;
        }
        let Some(to) = drag.destination() else {
            return;
        };
        match drag.list {
            crate::drag::List::Queue => self.move_queued(drag.from, to),
            crate::drag::List::Playlist => {
                if let Screen::Shelf(state) = &self.screen {
                    self.playlists.move_entry(drag.from, to, &state.library);
                }
            }
        }
    }

    /// Reposition queue row `from` at `to` — the drag's commit on the run.
    /// [`Self::shift_queued`]'s exact shape over [`queue_edit::moved`]'s
    /// pure edit: the whole new queue as one [`Command::UpdateQueue`], the
    /// music keeps playing, the cursor follows its track by path.
    fn move_queued(&mut self, from: usize, to: usize) {
        let Some(edited) = self
            .player
            .queue()
            .and_then(|queue| queue_edit::moved(queue, from, to))
        else {
            return;
        };
        let paths = edited.paths();
        if self.playback.send(Command::UpdateQueue { paths }) {
            self.player.note_queue_edited(edited);
        } else {
            self.player.engine_closed();
        }
        self.publish_mpris(false);
    }

    /// The place's transient `Undo`, resolved against **which list surface
    /// the window is showing** (doc 11 §5 P2). Only an open playlist page is
    /// an editor now; everywhere else the press asks for nothing. Undo is one
    /// history per visible surface, never a global stack, and its accelerator
    /// is legal exactly where its visible twin stands.
    fn undo_edit(&mut self) -> Task<Message> {
        if let Place::Playlist(_) = self.place
            && let Screen::Shelf(state) = &self.screen
        {
            self.playlists.undo_open(&state.library);
        }
        Task::none()
    }

    /// Restore the run as it stood before the last recorded edit.
    ///
    /// **The list, never the playback position** (P2's exact scope): the
    /// restored queue goes out as [`Command::UpdateQueue`] — ADR-0014's
    /// guarantee that no delivered sample is disturbed — with no `Play`, no
    /// `SetQueue` and no `JumpTo` anywhere on this path, so nothing ever
    /// sounds, stops, or moves because of an undo. The cursor finds its
    /// track again by path, exactly as it does through every other edit.
    // Retained with the dormant queue renderer: if that editor gains a
    // dedicated surface again, its bounded undo path returns with it rather
    // than being reimplemented from scratch.
    #[allow(dead_code)]
    fn undo_queue_edit(&mut self) {
        let Some(restored) = self.queue_undo.pop() else {
            return;
        };
        let paths = restored.paths();
        if self.playback.send(Command::UpdateQueue { paths }) {
            self.player.note_queue_edited(restored);
        } else {
            self.player.engine_closed();
        }
        self.publish_mpris(false);
    }

    /// Bookkeeping for a place change: an edit history belongs to the
    /// surface that shows its `Undo` word, and leaving that surface is one
    /// of the three things that end it (P2: "until the next edit, a
    /// navigation, or the run ending").
    ///
    fn note_place_left(&mut self, from: Place) -> Task<Message> {
        if from == self.place {
            return Task::none();
        }
        if from == Place::Queue {
            self.queue_undo.clear();
        }
        if from == Place::Playlists {
            self.playlists.hovered = None;
        }
        if matches!(from, Place::Playlist(_)) {
            self.playlists.clear_undo();
        }
        Task::none()
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

    /// The sleeve the bottom bar draws beside the track and artist: the
    /// sounding record's thumbnail, or its already-prefetched Now playing
    /// hero while that thumbnail has not otherwise been needed.
    ///
    /// The hero is requested for every sounding record whether Now playing is
    /// open or not. Falling back to it closes the gap where the bottom bar
    /// remained blank until opening a playlist happened to request the same
    /// record's thumbnail. Both reads use `peek`, so a frame cannot reorder an
    /// LRU merely by observing it.
    fn bar_cover(&self) -> Option<views::bottom_bar::Cover> {
        let Screen::Shelf(state) = &self.screen else {
            return None;
        };
        let id = self.player.playing_album()?;
        let image = state
            .thumb(id)
            .cloned()
            .or_else(|| state.hero(id).map(|hero| hero.handle.clone()));
        Some(image.map_or(
            views::bottom_bar::Cover::Placeholder(id),
            views::bottom_bar::Cover::Image,
        ))
    }

    fn health_summary(&self) -> crate::health::Summary {
        match &self.screen {
            Screen::Shelf(state) => crate::health::Summary::resolve(
                state.scanning,
                state.unavailable.len(),
                state.files_skipped,
                state.problem.is_some() || !self.player.engine_ready(),
                state.health.attention(),
            ),
            Screen::Setup(_) | Screen::Blocked(_) => {
                crate::health::Summary::resolve(false, 0, 0, false, None)
            }
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
    #[expect(
        clippy::too_many_lines,
        reason = "one match arm per place and screen — the routing table is \
                  clearest read whole, and the arms are calls, not logic"
    )]
    fn view(&self) -> Element<'_, Message> {
        if std::env::var_os("BAZ_FRAME_LOG").is_some() {
            crate::baz_log!(
                "[frame] {:.3}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64()
            );
        }
        let ink = self.ink();
        let lamp = self.warmth.value();
        let collecting = self.playlists.collecting();
        let screen: Element<'_, Message> = match (&self.screen, self.place) {
            // **The two pre-library screens return early**, before the lane,
            // the app bar and the bottom bar are composed. Neither is a place:
            // the bar's display options need a wall and its gear opens a place
            // inside a library that has not opened, and a bar with two dead
            // zones states less than no bar (`views::blocked`'s own docs).
            (Screen::Setup(setup), _) => {
                return crate::window_frame::resize_frame(
                    views::setup::view(setup),
                    owns_chrome() && !self.window_maximized,
                );
            }
            (Screen::Blocked(blocked), _) => {
                return crate::window_frame::resize_frame(
                    views::blocked::view(blocked),
                    owns_chrome() && !self.window_maximized,
                );
            }
            (Screen::Shelf(state), Place::Library) => state.view(&self.player, lamp, collecting),
            (Screen::Shelf(state), Place::Playlists) => views::playlists::view(
                state,
                &self.playlists,
                &self.player,
                state.grid(),
                self.playlists_scroll,
            ),
            (Screen::Shelf(state), Place::NewPlaylist) => {
                views::new_playlist::view(state, &self.playlists, &self.player, self.body_width())
            }
            (Screen::Shelf(state), Place::Favourites) => {
                views::favourites::view(state, &self.player, self.body_width())
            }
            (Screen::Shelf(state), Place::Album(id)) => match state.album(id) {
                Some(album) => views::album::view(
                    state,
                    album,
                    &self.player,
                    self.body_width(),
                    lamp,
                    self.playlists.collecting(),
                    self.hovered_album_row,
                ),
                // The record vanished under a rescan while its page was open.
                // The wall is the honest answer — better than a page about
                // nothing — and it is drawn rather than navigated to, because a
                // view function may not change state.
                None => state.view(&self.player, lamp, collecting),
            },
            (Screen::Shelf(state), Place::Artist(id)) => {
                // The artist vanished under a rescan while their page was
                // open — renamed, or their last record removed. The wall is
                // the honest answer, drawn rather than navigated to, exactly
                // as a vanished record's page is.
                if views::artist::label(state, id).is_some() {
                    views::artist::view(state, &self.player, id, state.grid(), collecting)
                } else {
                    state.view(&self.player, lamp, collecting)
                }
            }
            (Screen::Shelf(state), Place::Queue) => views::queue::view(
                state,
                &self.player,
                iced::Size::new(self.body_width(), self.body_height()),
                self.drag.as_ref().map_or(self.hovered_queue_row, |_| None),
                self.playlists.saving_queue.as_ref(),
                collecting,
                self.queue_scroll,
                self.drag.as_ref(),
                self.queue_undo.can_undo(),
            ),
            (Screen::Shelf(state), Place::Playlist(id)) => match self.playlists.page(id) {
                Some(open) => views::playlist::view(
                    state,
                    open,
                    &self.player,
                    iced::Size::new(self.body_width(), self.body_height()),
                    self.drag
                        .as_ref()
                        .map_or(self.hovered_playlist_row, |_| None),
                    collecting,
                    self.playlist_scroll,
                    self.drag.as_ref(),
                    self.playlists.can_undo_open(),
                ),
                // The playlist vanished under its page — its collection root
                // is the honest fallback, not the record library.
                None => views::playlists::view(
                    state,
                    &self.playlists,
                    &self.player,
                    state.grid(),
                    self.playlists_scroll,
                ),
            },
            // **Home** and **Now playing** — the two places the owner added
            // to ADR-0030 (`place.rs` records the overrule). Their bodies land
            // in the two commits after this one; what is here is the routing
            // and the frame, so the lane's head is a live control from the
            // moment it exists rather than two rows that go nowhere.
            (Screen::Shelf(state), Place::Home) => views::home::view(
                state,
                &self.player,
                &self.resume,
                self.body_width(),
                // **The wall's own grid**, not a second one resolved for this
                // page's width: a record is drawn at the same size wherever
                // it is drawn, and the density step reaches every place that
                // hangs works rather than only the Library (ADR-0028's
                // fourth-step amendment §2).
                state.grid(),
                collecting,
            ),
            (Screen::Shelf(state), Place::NowPlaying) => {
                // Snapshot the lock-free tap only when its layer is visible.
                // Cover-only and jewel-case-only frames do no sample reads.
                let audio = self
                    .visualization
                    .mode
                    .active()
                    .then(|| self.playback.visualization());
                let fact = self
                    .visualization
                    .facts
                    .then(|| crate::facts::current(state, &self.player))
                    .and_then(|facts| {
                        (!facts.is_empty()).then(|| facts[self.fact_index % facts.len()].clone())
                    });
                views::now_playing::view(
                    state,
                    &self.player,
                    self.body_width(),
                    self.body_height(),
                    self.now_playing_source(),
                    views::now_playing::Visual {
                        rotation: self.case_rotation,
                        foreground: self.visualization.foreground,
                        mode: self.visualization.mode,
                        audio: audio.as_ref(),
                        history: &self.visualization_history,
                        favourite: self
                            .player
                            .now_playing()
                            .filter(|now| now.album_id.is_some())
                            .and_then(|_| self.player.now_playing_path())
                            .map(|path| (path, is_favourite(state, path))),
                    },
                    fact.as_ref(),
                )
            }
            (Screen::Shelf(state), Place::Settings) => {
                // Built here rather than inside the view: the folders come from
                // the shell's own list and their contents from the index, and a
                // view that reached into the library would be a second place
                // that knows how roots are counted.
                views::settings::view(
                    &self.player,
                    self.body_width(),
                    self.settings_section,
                    state.library_view(self.playlists.folder_path()),
                    views::settings::OutputView {
                        choices: &self.output_choices,
                        selected: &self.output_choice,
                        active: &self.active_output_choice,
                        error: self.output_devices_error.as_deref(),
                    },
                    config::config_file().map_or(config::DEFAULT_VIBE_WORKERS, |path| {
                        config::load(&path).vibe_workers
                    }),
                    views::settings::ThemeView {
                        selected: config::config_file().map_or_else(
                            || crate::theme_file::DEFAULT_SELECTION.to_owned(),
                            |path| config::load(&path).theme,
                        ),
                        json: &self.theme_json,
                        notice: self.theme_notice.as_deref(),
                    },
                    if self.settings_section == views::settings::DEBUG_SECTION {
                        crate::diagnostic::snapshot()
                    } else {
                        Vec::new()
                    },
                    self.resource_reading,
                )
            }
        };
        // **The returns lane**, to the left of the place (ADR-0030 §1 as the
        // owner amended it): resident, in every place but Settings, and a
        // *column* rather than a layer — it takes width, which is why
        // `Shelf::grid_width` has a second term and why the collapse is the
        // one press that may re-hang the collection.
        //
        // It is outside the place rather than inside each of them so that the
        // frame is the frame: navigating cannot slide the lane by a pixel,
        // and the place's own strip resolves against `body_width` — the
        // window less the lane — rather than against the window.
        let screen: Element<'_, Message> = match &self.screen {
            Screen::Shelf(state) if self.place.wears_lane() => row![
                views::lane::view(
                    state,
                    &self.playlists,
                    self.place,
                    &self.lane,
                    self.player.now_playing_path().is_some(),
                    // Two facts, not one: *anything* is sounding lights the
                    // head's `Now playing` dot, and *the row the run came
                    // from* lights its own. They differ for a file the library
                    // does not hold — the head still answers, the list has
                    // nothing to mark.
                    //
                    // **The row is the run's origin, not the sounding file's
                    // record** — the owner's *"it is showing next to the album
                    // rather than the playlist"*. `lane::sounding_subject` is
                    // the same call the recency ordering makes, so the dot and
                    // the order cannot say different things about one run.
                    crate::lane::sounding_subject(
                        self.player.now_playing_path().is_some(),
                        self.player.queue_provenance(),
                        self.player.playing_album(),
                    ),
                    self.window.width,
                ),
                screen
            ]
            .into(),
            _ => screen,
        };
        // **The playlist panel**, floated over the place by ADR-0016's
        // verified mechanics: a `stack`, the panel wrapped in `opaque` so a
        // press inside it cannot fall through to a tile underneath, no scrim
        // (refused), and wheel events beside it passing straight through to
        // the wall. The wall is not re-laid by a pixel — the panel is a
        // layer, not a column — and the bar below stays untouched because the
        // stack holds only the place.
        let screen: Element<'_, Message> = if let Screen::Shelf(state) = &self.screen
            && self.panel_on_screen()
        {
            iced::widget::stack![
                screen,
                iced::widget::container(iced::widget::opaque(views::playlist_panel::view(
                    state,
                    &self.playlists,
                    &self.player,
                    self.drag.as_ref(),
                )))
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .align_x(iced::alignment::Horizontal::Right),
            ]
            .into()
        } else {
            screen
        };
        // **The body has one physical clip, outside every place and inside
        // both resident bars.** A scrollable's cached active/inactive state is
        // no longer trusted to be the only renderer scissor around images;
        // navigation, resize, density and conditional overlay transitions all
        // pass through this stable boundary for paint and pointer input.
        let screen = crate::window_frame::body_clip(screen);
        // **The app bar, over everything** (ADR-0040): the band a platform
        // title bar occupies, drawn by baz, resident and identical in all
        // nine places.
        //
        // It is composed **here**, outside the lane and outside the place,
        // for the reason the lane is composed outside the place: a surface
        // that is the same everywhere must be assembled once, or it is nine
        // surfaces that happen to agree. And it spans the *window* rather than
        // the body, because the window controls in its right corner belong to
        // the window and may not be inset by a lane whose width changes.
        //
        // **Which places hang works is answered here, not in the view.** The
        // display options are drawn where there is a wall of records to hang
        // and absent where there is not (ADR-0028's *absent, not disabled*, as
        // ADR-0040 §5 preserves it), and that is a fact about the composition
        // — this `match` — rather than something a view file should be
        // guessing from a `Place`.
        let hangs_works = match (&self.screen, self.place) {
            (
                Screen::Shelf(state),
                Place::Library | Place::Playlists | Place::Home | Place::Artist(_),
            ) => Some(state.grid().density),
            // A record's page, a playlist's, Now playing and Settings hang
            // rows or nothing, and density's unit is the column (ADR-0028's
            // amendment §2). No marks — **absent, not disabled**.
            _ => None,
        };
        let visualization = matches!(
            (&self.screen, self.place),
            (Screen::Shelf(_), Place::NowPlaying)
        )
        .then_some(self.visualization);
        let Screen::Shelf(state) = &self.screen else {
            unreachable!("setup and blocked screens return before app-bar composition")
        };
        let screen: Element<'_, Message> = column![
            views::app_bar::view(
                state,
                self.window.width,
                hangs_works,
                visualization,
                self.place_history.can_back(),
                self.place_history.can_forward(),
                self.window_maximized,
                owns_chrome(),
                self.health_summary(),
                ink,
            ),
            screen
        ]
        .into();
        // The GUI is always an audio build, so the persistent bottom bar lives
        // under every place. A missing device is represented in the bar rather
        // than by changing the application's composition.
        let whole: Element<'_, Message> = column![
            screen,
            views::bottom_bar::view(
                &self.player,
                ink,
                self.bar_cover(),
                self.now_playing_source()
                    .map(|_| Message::OpenPlayingSource),
                self.window.width,
                // The same reading Now playing's title line takes: the
                // sounding file, and whether the library holds it as a
                // favourite. `None` is a sounding file with no library row,
                // which cannot be favourited at all — the bar keeps the slot
                // and draws the action inert rather than dropping it, because
                // a slot that came and went would move the title lane beside
                // it, which is the one thing this bar may not do.
                self.player
                    .now_playing()
                    .filter(|now| now.album_id.is_some())
                    .and_then(|_| self.player.now_playing_path())
                    .map(|path| (path, is_favourite(state, path))),
            ),
        ]
        .into();
        let whole: Element<'_, Message> = match &self.screen {
            Screen::Shelf(state) if state.search_open => iced::widget::stack![
                whole,
                views::search::layer(
                    state,
                    &self.player,
                    self.window,
                    matches!(self.place, Place::Playlist(_) | Place::NewPlaylist),
                ),
            ]
            .into(),
            Screen::Shelf(_) | Screen::Setup(_) | Screen::Blocked(_) => whole,
        };
        let whole: Element<'_, Message> = if self.status_open {
            match &self.screen {
                Screen::Shelf(state) => iced::widget::stack![
                    whole,
                    views::status::layer(&state.health, self.health_summary(), self.window),
                ]
                .into(),
                Screen::Setup(_) | Screen::Blocked(_) => whole,
            }
        } else {
            whole
        };
        // **The context menu** (doc 09 §5.2), floated at the pointer by the
        // same ADR-0016 mechanics as the panel — but stacked over the *whole
        // window*, bar included, because the bar's own now-playing menu
        // opens over the bar. Under the card sits a full-window backdrop
        // whose left press puts the menu down; a right press falls through
        // it to whatever row is beneath, whose own `menu::area` replaces
        // the menu — one at a time by construction. Wheel travel passes
        // beside both, and nothing reflows by a pixel: layers, not columns.
        let whole: Element<'_, Message> = match &self.menu {
            Some(open) if matches!(self.screen, Screen::Shelf(_)) => {
                iced::widget::stack![whole, views::context_menu::layer(open, self.window)].into()
            }
            _ => whole,
        };
        // **The drag's ghost** — the lifted row's title following the
        // pointer (doc 09 §13 step 8) — on its own topmost layer: it rides
        // over the panel it may be headed for. The layer is all
        // pass-through — text in a container captures nothing — so unlike
        // the menu it costs no press and blocks no row underneath from
        // measuring the pointer.
        //
        // The layer is stacked **always**, an empty pass-through at rest,
        // and this is load-bearing rather than tidiness: iced diffs the
        // widget tree by position and tag, so a stack level that appeared
        // only at the lift would reshape the tree under every widget on
        // screen at exactly that moment — resetting, among everything
        // else, the drag source's own held phase, and the gesture would
        // die the frame it began. (Measured, not conjectured: the first
        // headless probe of the drag shipped the conditional form and the
        // ghost froze at the lift point.)
        let ghost: Element<'_, Message> = match &self.drag {
            Some(drag) => views::drag_ghost::layer(drag, self.window),
            None => iced::widget::Space::new().width(0.0).height(0.0).into(),
        };
        crate::window_frame::resize_frame(
            iced::widget::stack![whole, ghost],
            owns_chrome() && !self.window_maximized,
        )
    }

    /// **The width a place's body gets**: the window, less the returns lane
    /// where the place wears one.
    ///
    /// Every place resolves its own breakpoints against this rather than
    /// against the window — the strip's two-line split, the album page's two
    /// columns, the Settings measure. A body that split against the window
    /// would split at the wrong moment the instant a column appeared beside
    /// it, which is exactly the class of bug a resident surface introduces.
    fn body_width(&self) -> f32 {
        match &self.screen {
            Screen::Shelf(state) if self.place.wears_lane() => state.body_width(),
            _ => self.window.width,
        }
    }

    /// **The height a place's body gets**: the window, less the now-playing
    /// bar and its hairline.
    ///
    /// [`Self::body_width`]'s other half, and it exists for the same reason: a
    /// place that sized itself against the *window* would compose over the bar
    /// and have its last row cut off by it — which is exactly what the first
    /// render of the Now playing place did, with the artwork clipped at the
    /// top and the transport off the bottom edge.
    ///
    /// Only that place asks. It is the one place whose composition is bounded
    /// in both axes, because it is the one place that must fit without
    /// scrolling. It wears no *strip* of its own — the returns lane is the
    /// route in and out of it — but since ADR-0040 it wears the **app bar**,
    /// like every other place, and that does come off the top.
    fn body_height(&self) -> f32 {
        (self.window.height - theme::APP_BAR_H - theme::BAR_CONTENT_H - 1.0).max(0.0)
    }

    /// Whether the playlist panel is on screen: summoned, over a shelf, and
    /// not in Settings — the one place it is absent (ADR-0024 §5). Its open
    /// state *survives* the Settings round trip; only its pixels do not.
    fn panel_on_screen(&self) -> bool {
        matches!(self.screen, Screen::Shelf(_))
            && self.playlists.panel_open
            && !matches!(self.place, Place::Settings | Place::NewPlaylist)
    }

    /// What every icon button needs to know to ink itself: which one the
    /// pointer is on, how far its fade has travelled, and whether it is held.
    fn ink(&self) -> Ink {
        Ink::new(self.ink, self.pressed_control)
    }

    fn add_place_clocks(&self, subs: &mut Vec<Subscription<Message>>) {
        if visualization_clock(
            self.place,
            self.player.now_playing().is_some(),
            self.visualization,
        ) {
            subs.push(
                iced::time::every(crate::jewel_case::TICK)
                    .map(|_| Message::CaseTick(Instant::now())),
            );
        }
        if fact_clock(
            self.place,
            self.player.now_playing().is_some(),
            self.visualization.facts,
        ) {
            subs.push(iced::time::every(Duration::from_secs(20)).map(|_| Message::AdvanceFact));
        }
        // **The resource meter's clock is the Debug section's**, so it does
        // not exist anywhere else in the product — which is the whole of what
        // makes a resource meter honest: one that ran while you listened would
        // be a cost of its own inside the number it reports.
        if self.place == Place::Settings && self.settings_section == views::settings::DEBUG_SECTION
        {
            subs.push(
                iced::time::every(Duration::from_secs(1))
                    .map(|_| Message::ResourceTick(Instant::now())),
            );
        }
        if let Screen::Shelf(state) = &self.screen {
            if state.scanning {
                subs.push(iced::time::every(Duration::from_millis(100)).map(|_| Message::ScanTick));
            } else {
                subs.push(iced::time::every(REFRESH_TICK).map(|_| Message::RefreshTick));
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        fn event_message(
            event: iced::Event,
            status: iced::event::Status,
            _window: window::Id,
        ) -> Option<Message> {
            match event {
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
                // either, so both halves have to be assembled here.
                iced::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                    Some(Message::ModifiersChanged(modifiers))
                }
                iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                    Some(Message::Wheel(match delta {
                        iced::mouse::ScrollDelta::Lines { y, .. }
                        | iced::mouse::ScrollDelta::Pixels { y, .. } => y,
                    }))
                }
                iced::Event::Window(window::Event::FileDropped(path)) => {
                    Some(Message::FileDropped(path))
                }
                iced::Event::Window(window::Event::FileHovered(_)) => Some(Message::FileHovered),
                iced::Event::Window(window::Event::FilesHoveredLeft) => {
                    Some(Message::FileHoverLeft)
                }
                iced::Event::Window(window::Event::Focused) => Some(Message::WindowFocused(true)),
                iced::Event::Window(window::Event::Unfocused) => {
                    Some(Message::WindowFocused(false))
                }
                _ => None,
            }
        }

        fn search_event_message(
            event: iced::Event,
            status: iced::event::Status,
            window: window::Id,
        ) -> Option<Message> {
            if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) =
                &event
            {
                // The visible chooser, not the caret, owns its advertised bare
                // arrow grammar. This is resolved before capture status so a
                // focused well cannot swallow Left/Right.
                if let Some(direction) = crate::search::chooser_direction(key, *modifiers) {
                    return Some(Message::Direction(direction));
                }
                // `text_input` captures Escape after blurring itself. Search
                // owns that first press while its chooser stands, so dismissal
                // must be heard before the focused-field filter drops it.
                if key == &keyboard::Key::Named(keyboard::key::Named::Escape) {
                    return Some(Message::DismissSearch);
                }
            }
            event_message(event, status, window)
        }

        let events = if matches!(&self.screen, Screen::Shelf(state) if state.search_open) {
            iced::event::listen_with(search_event_message)
        } else {
            iced::event::listen_with(event_message)
        };
        let mut subs = vec![
            // Raw events rather than `keyboard::on_key_press`, because the
            // capture status is the focus rule: a key a focused text field
            // consumed is not a shortcut (see `crate::keys`).
            events,
            window::resize_events().map(|(_, size)| Message::WindowResized(size)),
            // The close request, answered by the shell rather than by the
            // toolkit: see `run`'s `exit_on_close_request(false)`.
            window::close_requests().map(|_| Message::Quit),
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
        if self.volume_wheel_settles.is_some() {
            subs.push(
                iced::time::every(Duration::from_millis(40))
                    .map(|_| Message::VolumeWheelSettled(Instant::now())),
            );
        }
        // Now Playing owns the only intentionally continuous visuals in Baz:
        // the turning case and the optional delivered-audio background. The
        // timer is absent for plain cover/no-object with the spectrum off and
        // always absent away from this visible place or without a sounding
        // record. Keyboard focus is deliberately irrelevant: Now Playing is
        // ambient content meant to remain alive on a second monitor.
        // Place-owned animation, facts and scan/refresh clocks are added only
        // while their corresponding surface or operation is alive.
        self.add_place_clocks(&mut subs);
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
        Self {
            input,
            error,
            hovering_drop: false,
        }
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

/// **One record decoded at the Now playing place's tier** — the artwork, the
/// number that bounds it, and the field derived from it (doc 12 §5.2, §5.3).
///
/// All three come out of **one** decode on **one** worker call, because all
/// three are readings of the same pixels and a second pass over them would be
/// a second chance to disagree.
#[derive(Debug, Clone)]
pub(crate) struct Hero {
    /// The cover at up to [`art::HERO_PX`] per edge.
    pub(crate) handle: iced_image::Handle,
    /// A real rear insert, when the files or tags carry one. `None` asks the
    /// jewel case to typeset the album's track list instead.
    pub(crate) back: Option<iced_image::Handle>,
    /// `min(width, height)` of what the decode actually returned — **the
    /// source's own pixels**, and the third term of the Now playing place's
    /// `art_edge`. Not [`art::HERO_PX`], which is only the decoder's ceiling:
    /// a 500 px cover yields 500 here and is drawn at 500.
    pub(crate) px: f32,
    /// The record's ambient field, or `None` when the cover carries no hue
    /// worth reading — a monochrome sleeve gets the room (story S7).
    pub(crate) field: Option<crate::field::Field>,
}

/// **What a newly-committed picture asks of the Now playing place** — the whole
/// of the crossfade's predicate, as a function of the two pictures and of
/// nothing else.
///
/// A free decision rather than three lines inside [`Shelf::settle_art`],
/// because *when a transition may run* is the part of this feature that is
/// worth being able to state and to test without a window, a library or a
/// player (ADR-0006 layer 1's habit, applied to a rule that could not quite
/// live in [`crate::motion`] — it is about an `iced` handle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    /// **Draw the new picture and keep no clock.** Everything that is not two
    /// distinct covers: the first record of a session, a record with no art at
    /// either end, and — the case the owner would notice — a picture that has
    /// not actually changed.
    Cut,
    /// **Dissolve, over [`motion::DISSOLVE`].** Two decoded covers that are not
    /// the same picture.
    Dissolve,
}

impl Change {
    /// The rule: **two pictures, and they must differ.**
    ///
    /// `None` is *a record with no art*, which draws the wall's deterministic
    /// gradient — a stand-in rather than artwork, and dissolving a stand-in is
    /// decoration (ADR-0020 §3).
    ///
    /// Distinctness is `Handle`'s own equality, which for a decoded image is
    /// its allocation id and its pixels: **the handle being drawn**, not the
    /// track and not the record. Two clones of one decode are equal, so a
    /// surface redrawing what it already had can never start a flight. Two
    /// *separate* decodes of byte-identical covers — one record ripped twice —
    /// are not equal and would run a dissolve, which is invisible by
    /// construction (`X · (1 − t) + X · t` is `X` at every `t`) and costs the
    /// 200 ms of clock a rarity may cost.
    fn between(from: Option<&Hero>, to: Option<&Hero>) -> Self {
        match (from, to) {
            (Some(from), Some(to)) if from.handle != to.handle => Self::Dissolve,
            _ => Self::Cut,
        }
    }
}

/// **The Now playing place's artwork, mid-change and at rest** — what to draw,
/// what to draw it over, and how far between them the surface stands.
///
/// One value rather than three readings, for the reason
/// `views::now_playing::work`'s caller needs: the cover and the field are two
/// readings of one decode, and a surface that fetched them separately could
/// draw one record's picture over another record's room for a frame.
pub(crate) struct Showing<'a> {
    /// The picture the surface has committed to. `None` before the first hero
    /// of a session lands, and for a record with no art — both of which fall
    /// through to the thumbnail and then the gradient.
    pub(crate) hero: Option<&'a Hero>,
    /// The picture it is dissolving away from, while it is dissolving.
    pub(crate) from: Option<&'a Hero>,
    /// [`Self::hero`]'s own opacity over [`Self::from`], in `[0, 1]`. **`1.0`
    /// at rest**, which is one picture at full strength and no clock.
    pub(crate) t: f32,
}

/// `min(w, h)` as the `f32` a layout wants.
///
/// Named because both tiers spend it and because what it *means* is the same
/// sentence in both: **the largest square that may be drawn from this decode
/// without inventing a pixel.**
fn shortest_edge(w: u32, h: u32) -> f32 {
    f32::from(u16::try_from(w.min(h)).unwrap_or(u16::MAX))
}

/// A finished thumbnail decode, as the message carries it: its shortest edge
/// and its handle.
///
/// One function because three call sites build it — the wall's visible range,
/// the surfaces beside the wall, and the playlist collages — and three copies
/// of *decode, measure, wrap* is three places for the measurement to drift
/// away from the pixels it describes.
fn decoded((w, h, rgba): (u32, u32, Vec<u8>)) -> (f32, usize, iced_image::Handle) {
    let bytes = rgba.len();
    (
        shortest_edge(w, h),
        bytes,
        iced_image::Handle::from_rgba(w, h, rgba),
    )
}

struct ThumbEntry {
    handle: iced_image::Handle,
    decoded_bytes: usize,
}

/// The thumbnail cache with an un-evictable resident tier for what the current
/// frame can show.
///
/// The old single LRU could evict a visible sleeve when lane, playlist or
/// artist work filled the same 64 slots. Worse, the wall's unchanged-range
/// guard then declined to request it again. Prepared disk art made the reload
/// cheap but did not prevent the visible blank. This keeps the 64-entry LRU
/// for everything off screen and moves current targets into a separate map;
/// leaving the target returns a handle to the LRU immediately.
struct ThumbCache {
    recent: LruCache<u64, ThumbEntry>,
    resident: HashMap<u64, ThumbEntry>,
    /// Handles that have actually reached a visible target in this process.
    ///
    /// Moving away must not turn an already-present sleeve back into a
    /// gradient. Retaining only entries that were resident (rather than every
    /// speculative completion) makes the cost proportional to artwork the
    /// listener has visited.
    ///
    /// **It is an LRU now, and it was a `HashMap`.** "Bounded above by the
    /// indexed collection" was the whole of its bound, which is to say it had
    /// none: a large library retained every cover it ever showed, and the
    /// figures this project published were measurements of what that came to
    /// on the owner's 393 albums rather than a limit anything enforced. It is
    /// ordered so that [`art::THUMB_BUDGET_BYTES`] can be enforced against the
    /// **least recently visited** art, which is the only ordering under which
    /// trimming is not arbitrary.
    ///
    /// Its capacity is the byte budget at the *smallest* entry the tier can
    /// hold, so the count never binds before the bytes do — the bound that
    /// matters is [`ThumbCache::trim_to_budget`]'s, and this one exists only
    /// because `LruCache` requires a capacity.
    retained: LruCache<u64, ThumbEntry>,
    wall: HashSet<u64>,
    chrome: HashSet<u64>,
    page: HashSet<u64>,
}

impl ThumbCache {
    fn new(capacity: NonZeroUsize) -> Self {
        Self {
            recent: LruCache::new(capacity),
            resident: HashMap::new(),
            retained: LruCache::new(art::retained_capacity()),
            wall: HashSet::new(),
            chrome: HashSet::new(),
            page: HashSet::new(),
        }
    }

    fn peek(&self, id: u64) -> Option<&iced_image::Handle> {
        self.resident
            .get(&id)
            .or_else(|| self.retained.peek(&id))
            .or_else(|| self.recent.peek(&id))
            .map(|entry| &entry.handle)
    }

    /// Is this id's art already decoded — and, if it is, say so **and mark it
    /// used**.
    ///
    /// The promotion is the point and is why this takes `&mut`: it is called
    /// on every target of every re-aim, so "recently used" means "recently on
    /// screen", which is exactly the order [`Self::trim_to_budget`] has to
    /// trim against. `retained` moved from a `HashMap` to an LRU for this
    /// reason as much as for the popping.
    fn touch(&mut self, id: u64) -> bool {
        self.resident.contains_key(&id)
            || self.retained.get(&id).is_some()
            || self.recent.get(&id).is_some()
    }

    fn put(&mut self, id: u64, handle: iced_image::Handle, decoded_bytes: usize) {
        let entry = ThumbEntry {
            handle,
            decoded_bytes,
        };
        if self.is_pinned(id) {
            self.recent.pop(&id);
            self.retained.pop(&id);
            self.resident.insert(id, entry);
        } else {
            self.resident.remove(&id);
            self.retained.pop(&id);
            self.recent.put(id, entry);
        }
        self.trim_to_budget();
    }

    /// **Hold the stated budget** ([`art::THUMB_BUDGET_BYTES`]), by dropping
    /// the least valuable decoded artwork until the total fits.
    ///
    /// The order is the tiering argument, spent: **speculative first** — art a
    /// decode completed for that no surface ever displayed — and then the
    /// **least recently visited retained** art. Nothing the current frame can
    /// draw is ever dropped; the resident tier is exempt, because a visible
    /// sleeve turning back into a gradient is the defect this whole tier
    /// exists to prevent (item 20), and the loop stops rather than reaching
    /// for it. `the_visible_wall_can_never_exhaust_the_art_budget` is what
    /// makes that exemption safe to state.
    ///
    /// The running total is carried rather than recomputed: the sum is a walk
    /// of every entry, and recomputing it inside the loop would make trimming
    /// a large overflow quadratic in the size of the cache.
    fn trim_to_budget(&mut self) {
        let mut held = self.decoded_bytes();
        while held > art::THUMB_BUDGET_BYTES {
            let dropped = self
                .recent
                .pop_lru()
                .or_else(|| self.retained.pop_lru())
                .map(|(_, entry)| entry.decoded_bytes);
            let Some(dropped) = dropped else {
                // Only the resident tier is left, and it is not ours to take.
                break;
            };
            held = held.saturating_sub(dropped);
        }
    }

    fn clear_handles(&mut self) {
        self.recent.clear();
        self.resident.clear();
        self.retained.clear();
    }

    fn len(&self) -> usize {
        self.recent.len() + self.resident.len() + self.retained.len()
    }

    fn resident_len(&self) -> usize {
        self.resident.len()
    }

    fn retained_len(&self) -> usize {
        self.retained.len()
    }

    fn decoded_bytes(&self) -> usize {
        self.resident
            .values()
            .chain(self.retained.iter().map(|(_, entry)| entry))
            .chain(self.recent.iter().map(|(_, entry)| entry))
            .map(|entry| entry.decoded_bytes)
            .sum()
    }

    fn focus_wall(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.wall = ids.into_iter().collect();
        self.reconcile();
    }

    fn focus_chrome(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.chrome = ids.into_iter().collect();
        self.reconcile();
    }

    fn focus_page(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.page = ids.into_iter().collect();
        self.reconcile();
    }

    fn is_pinned(&self, id: u64) -> bool {
        self.wall.contains(&id) || self.chrome.contains(&id) || self.page.contains(&id)
    }

    /// One ordered snapshot of every target the current composition can draw.
    /// Wall and page work lead the resident chrome, but no category replaces
    /// another category's queue.
    fn targets(&self) -> Vec<u64> {
        let mut seen = HashSet::new();
        self.wall
            .iter()
            .chain(&self.page)
            .chain(&self.chrome)
            .copied()
            .filter(|id| seen.insert(*id))
            .collect()
    }

    fn reconcile(&mut self) {
        let wanted: HashSet<u64> = self
            .wall
            .iter()
            .chain(&self.chrome)
            .chain(&self.page)
            .copied()
            .collect();
        let leaving: Vec<u64> = self
            .resident
            .keys()
            .filter(|id| !wanted.contains(id))
            .copied()
            .collect();
        for id in leaving {
            if let Some(entry) = self.resident.remove(&id) {
                self.retained.put(id, entry);
            }
        }
        for id in wanted {
            if self.resident.contains_key(&id) {
                continue;
            }
            if let Some(entry) = self.retained.pop(&id) {
                self.resident.insert(id, entry);
            } else if let Some(entry) = self.recent.pop(&id) {
                self.resident.insert(id, entry);
            }
        }
        // A composition change can only ever move art *into* the resident
        // tier or out of it, never decode more — but art arriving from the
        // speculative tier stops being trimmable when it does, so the budget
        // is re-checked here as well as after a decode.
        self.trim_to_budget();
    }
}

/// The bounded thumbnail work list.
///
/// Foreground requests replace one another: after a fast scroll, covers from
/// the old viewport must not stand ahead of the viewport now on screen.
/// The visible lane has its own queue behind the page, so its album covers and
/// playlist collages are not crowded out by a fast page scroll. The two
/// in-flight jobs are never
/// cancelled because image decoders are blocking; bounding their count makes
/// letting them finish cheaper and safer than pretending cancellation could
/// stop the underlying work.
#[derive(Debug, Default)]
struct ThumbJobs {
    foreground: VecDeque<u64>,
    queued: HashSet<u64>,
    in_flight: HashSet<u64>,
    started: u64,
    completed: u64,
    peak: usize,
}

impl ThumbJobs {
    fn focus(&mut self, ids: impl IntoIterator<Item = u64>) {
        for id in self.foreground.drain(..) {
            self.queued.remove(&id);
        }
        for id in ids {
            if self.in_flight.contains(&id) {
                continue;
            }
            if self.queued.insert(id) {
                self.foreground.push_back(id);
            }
        }
    }

    fn retry(&mut self, id: u64) {
        if !self.in_flight.contains(&id) && self.queued.insert(id) {
            self.foreground.push_front(id);
        }
    }

    fn pop(&mut self) -> Option<u64> {
        let id = self.foreground.pop_front()?;
        self.queued.remove(&id);
        Some(id)
    }

    fn started(&mut self, id: u64) {
        self.in_flight.insert(id);
        self.started += 1;
        self.peak = self.peak.max(self.in_flight.len());
    }

    fn finished(&mut self, id: u64) {
        self.in_flight.remove(&id);
        self.completed += 1;
    }
}

/// The shelf screen: library, scan state, and grid/panel view state.
///
/// Fields the view layer reads are `pub(crate)`; the ones the update loop
/// owns alone (in-flight decodes, the scan channel, click timing) stay
/// private — [`crate::views`] draws this state, it never steers it.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the booleans are independent UI facts (scan, hover, lane, and \
              chooser visibility), not variants of one state machine"
)]
pub(crate) struct Shelf {
    /// The open library: the search index the counts and the query run over.
    pub(crate) library: Library,
    /// How the wall is arranged (ADR-0019). Persisted in `config.toml`; the
    /// top bar's row of words and `1`–`6` are the two ways to change it.
    pub(crate) group_key: GroupKey,
    /// How closely the wall hangs (ADR-0017 step 6). Persisted in
    /// `config.toml`; <kbd>Ctrl</kbd>+<kbd>-</kbd> / <kbd>Ctrl</kbd>+<kbd>=</kbd>
    /// and <kbd>Ctrl</kbd>+scroll are the two ways to change it, and there is
    /// no third way anywhere in the Settings place.
    pub(crate) density: shelf::Density,
    /// The play ledger, read once at open — what [`GroupKey::Played`] shelves
    /// on, and the returns lane's order key for a record.
    ///
    /// It had a second consumer, the pull's weighting; that went with the pull
    /// on 2026-08-10 (ADR-0018's amended third surface).
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
    /// Indices into `albums` drawn by the wall, in wall order. App-bar search
    /// covers the current place instead of filtering this collection.
    pub(crate) visible: Vec<usize>,
    /// How many of each shelf's albums survived it, in `groups` order — what
    /// [`shelf::Shelves`] lays the wall out from.
    visible_counts: Vec<usize>,
    /// The live search text.
    pub(crate) query: String,
    /// Relevance-ordered track answers for the live app-bar query. The result
    /// surface virtualizes this complete bounded set instead of truncating it
    /// to the old Library Songs section.
    pub(crate) songs: Vec<vm::SongVm>,
    /// Relevance-ordered album answers for the dropover, as stable wall ids.
    pub(crate) search_albums: Vec<u64>,
    /// Whether the non-empty query's dropover is currently exposed.
    pub(crate) search_open: bool,
    /// The selected track row's inline action. Albums keep their established
    /// activation and explicit Open grammar instead.
    pub(crate) search_action: crate::search::Action,
    /// Keyboard-selected action on the selected album cover.
    pub(crate) cover_action: CoverAction,
    /// Search's own selection/activation clock. It is separate from the place
    /// underneath so dismissing the dropover exposes that unchanged mark.
    pub(crate) search_selection: crate::selection::State,
    /// Absolute offset and measured height of the dropover's sole scroller.
    pub(crate) search_scroll_offset: f32,
    pub(crate) search_viewport_h: f32,
    /// **The record the wall was last left for**, if any. This remains the
    /// wall's navigation anchor; [`Self::selection`] separately owns the
    /// visible/actionable selection restored by ADR-0022's 2026-08-12
    /// amendment. Explicitly opening a record updates both facts, while
    /// selecting one without opening updates only the latter.
    ///
    /// Session-scoped, like everything else about where the wall is standing.
    pub(crate) opened: Option<u64>,
    /// The one selection/activation machine shared by playable tiles and rows.
    pub(crate) selection: crate::selection::State,
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
    /// Decoded thumbnails: current-frame residents plus the bounded off-screen
    /// LRU. Access goes through [`Self::thumb`] so a view cannot accidentally
    /// bypass the residency guarantee.
    thumbs: ThumbCache,
    /// Small, bounded cache of local artist portraits visited this session.
    artist_images: LruCache<u64, iced_image::Handle>,
    /// Artist portrait decodes currently running off the UI thread.
    artist_image_pending: HashSet<u64>,
    /// Artists already found not to carry a local portrait.
    no_artist_image: HashSet<u64>,
    /// **The shortest edge of each decoded thumbnail**, in pixels.
    ///
    /// [`art::load_thumb`] downscales only, so this is `min(w, h)` of a
    /// picture that is either the source itself (a cover smaller than
    /// [`art::THUMB_PX`]) or a faithful reduction of it — **in both cases a
    /// true upper bound on what may be drawn from this handle**. It is what
    /// keeps *no artwork is ever drawn larger than its source* true on the
    /// Now playing place for the frames between arriving and the hero landing,
    /// rather than only afterwards.
    ///
    /// A plain map rather than an entry in [`Self::thumbs`]: the six surfaces
    /// that draw a thumbnail want the handle and nothing else, and widening
    /// the LRU's value would have touched all six to serve one. Four bytes per
    /// album the process has ever decoded — the same unbounded-by-design
    /// shape, and two orders of magnitude smaller than, [`Self::no_art`].
    thumb_px: HashMap<u64, f32>,
    /// **Decoded-hero LRU** — the Now playing place's own decode tier
    /// ([`art::HERO_CACHE_ENTRIES`] entries, 8 MiB worst case).
    heroes: LruCache<u64, Hero>,
    /// The record [`Self::request_hero`] has a decode in flight for.
    hero_pending: Option<u64>,
    /// The album range [`Shelf::request_visible_thumbs`] last asked about, so
    /// the two redundant requests every resize step delivers cost a
    /// comparison instead of a pass over the library. `None` until the first
    /// ask, and reset by anything that changes *which* albums the range names
    /// rather than where it sits — see [`Shelf::forget_requested`].
    last_requested: Option<(usize, usize)>,
    /// Visibility-first, bounded thumbnail scheduler.
    thumb_jobs: ThumbJobs,
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
    /// Bounded session history shown by the bottom-right status control.
    pub(crate) health: crate::health::Log,
    /// The periodic-refresh clock (ADR-0022 §3).
    refresh: scan::Refresh,
    /// What has been typed into the Settings place's add-a-folder field.
    folder_input: String,
    /// Why the last folder submitted was not added, if it was not.
    folder_error: Option<String>,
    /// Which folder's Remove is armed and waiting for its confirming press.
    folder_pending_removal: Option<usize>,
    /// Paths under successfully scanned roots whose own parent directories
    /// are absent. They require explicit confirmation and are never automatic.
    prunable: Vec<PathBuf>,
    /// Whether Settings is showing the exact bulk-prune consequence.
    prune_pending: bool,
    /// Whether Settings is showing the exact rootless-row consequence.
    unrooted_pending: bool,
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
    /// Whether the pointer is on Home's **All songs** tile.
    ///
    /// [`Self::hovered_album`]'s mechanism for the one tile that is not a
    /// record — a `bool` because there is exactly one of it, where the wall has
    /// hundreds and needs an id. It carries no tween for the same reason: the
    /// wall's keyed tween exists so that crossing a gutter *hands the mark over*
    /// from one sleeve to the next, and a lone tile has nothing to hand it to.
    /// The hover options themselves were always a boolean reveal rather than a
    /// tween (`views::shelf`'s `hover_options`), so this tile's layer appears
    /// exactly as the wall's does.
    pub(crate) hovered_all_songs: bool,
    /// How far the hovered tile's mark has travelled (ADR-0020 §2.3).
    ///
    /// **One tween for the whole wall, keyed by the hovered id — never one per
    /// tile.** The shelf draws hundreds of tiles and at most one of them is
    /// under the pointer, so a tween per tile would be state allocated for a
    /// condition all but one of them is never in; and crossing the gutter from
    /// one sleeve to the next hands the mark over rather than restarting it
    /// (see [`crate::motion::Keyed`]).
    pub(crate) tile_hover: Keyed<u64>,
    /// Home's explicit, local metadata-playlist request composer.
    pub(crate) vibe: crate::vibe::State,
    /// **What the Now playing place has committed to drawing of the record** —
    /// the record's id, and its hero when the answer was a picture.
    ///
    /// The word is *committed* rather than *sounding*, and the difference is
    /// the whole of [`Self::settle_art`]: a record whose hero has not finished
    /// decoding has no answer yet, so this still names the record before it and
    /// the surface goes on drawing what it was drawing. `Some((id, None))` is
    /// a record the decode has answered *no art* for — the gradient
    /// placeholder, and nothing to dissolve.
    ///
    /// **It costs no memory.** The `Hero` is a clone whose handle is an `Arc`
    /// over the same decoded pixels, and the record it names is always the
    /// freshest entry of the two-entry hero LRU — [`Self::request_hero`] `get`s
    /// the sounding record on every message, which is what keeps it there.
    art_shown: Option<(u64, Hero)>,
    /// **The picture the hero is dissolving away from**, for as long as
    /// [`Self::art_dissolve`] is live, and `None` at every other instant.
    ///
    /// The second entry of the hero LRU is what makes this free: the record
    /// that just stopped is still decoded, so both pictures are alive at once
    /// and the crossfade needs no cache of its own. Checked rather than
    /// assumed — see [`Self::settle_art`].
    art_prior: Option<Hero>,
    /// **The incoming hero's opacity**, `0` → `1` over [`motion::DISSOLVE`],
    /// linear (ADR-0020's third amendment).
    ///
    /// Settled at `1` at rest, which is the surface drawing one picture at full
    /// strength and keeping no clock.
    art_dissolve: Tween,
    /// The width of the window the shelf is laid out in.
    ///
    /// **The wall's width, full stop.** It used to be the window's less
    /// whatever the inspector was taking at this instant — a number that
    /// changed nine times over 150 ms — and with no side surface left there is
    /// nothing to subtract but the index rail's lane (see
    /// [`Shelf::grid_width`]).
    ///
    /// Crate-visible because the view layer resolves the app bar, lane and
    /// virtualized surfaces from this same measurement.
    pub(crate) window_w: f32,
    /// **Whether the returns lane stands open** (ADR-0030 §3), as the config
    /// remembers it.
    ///
    /// It lives here rather than on the shell because [`Shelf::grid_width`]
    /// reads it: the lane's width is a term in the wall's, and the wall's
    /// width is resolved in exactly one place.
    pub(crate) lane_open: bool,
    /// **When each record was last played**, in seconds since the Unix epoch —
    /// the ledger folded onto records, once.
    ///
    /// Built at launch from the [`History`] snapshot in one pass over the
    /// library, and thereafter maintained by *events*: a `TrackStarted`
    /// updates one entry. That is ADR-0030 §4's responsiveness contract made
    /// literal — **never a per-frame file read, and no watcher**.
    lane_played: HashMap<u64, u64>,
    /// The lane's records half, resolved and trimmed to
    /// [`crate::lane::RECENT_ALBUMS`]: what the lane draws, less the lists.
    ///
    /// Cached rather than derived per frame because deriving it walks every
    /// album; the lists are merged in at view time, which is O(playlists) and
    /// independent of the library's size.
    pub(crate) lane_recent: Vec<crate::lane::Touched>,
    /// Bumped whenever [`Self::lane_recent`] is rebuilt — the shell's cue to
    /// re-merge, without comparing two vectors of strings.
    pub(crate) lane_stamp: u64,
    /// **The collection's four figures**, for the Home place's `COLLECTION`
    /// footer ([`vm::Collection`]).
    ///
    /// Cached here for ADR-0030 §4's reason: three of the four are a pass over
    /// every track, and the contract forbids paying that per frame. It is
    /// rebuilt exactly where the albums it counts are —
    /// [`Self::rebuild_shelves`] — so it cannot describe a library that is no
    /// longer on the shelf.
    pub(crate) collection: vm::Collection,
    /// Offline facts for each artist, rebuilt with the album wall and read by
    /// the artist page without walking tracks during a frame.
    pub(crate) artist_facts: HashMap<u64, vm::ArtistFacts>,
    /// Album indices for records on which an artist is credited but which are
    /// filed under somebody else. See [`vm::artist_inventory`].
    artist_also_on: HashMap<u64, Vec<usize>>,
}

impl Shelf {
    /// Current play-ledger snapshot for the local Now Playing fact feed.
    pub(crate) fn history(&self) -> Option<&History> {
        self.history.as_ref()
    }
}

impl Shelf {
    /// Open the library DB, hydrate the shelf, persist the chosen folders, and
    /// kick off the scan worker.
    ///
    /// **The error is a [`Blockage`] and not a sentence**, which is the whole
    /// of ADR-0041 at this seam: every failure here used to arrive at the
    /// first-run screen as a string, and a string cannot be routed. A caller
    /// that knows *which* failure it has can draw the newer-baz statement, and
    /// can decide that `Try again` means something here and nothing there.
    ///
    /// **Nothing on the failing path writes.** The directory is created —
    /// which a genuine first run needs and which costs an empty folder at
    /// worst — and then `Library::open` reads `user_version` before it sets a
    /// pragma. `adopt_roots`, `persist_roots` and the scan all sit *after* the
    /// open, so a refused library leaves the config file, the database and the
    /// listener's folders exactly as they were.
    #[expect(
        clippy::too_many_lines,
        reason = "opening the shelf initializes its complete session view model in one auditable place"
    )]
    fn open(
        roots: Vec<PathBuf>,
        group_key: GroupKey,
        density: shelf::Density,
        lane_open: bool,
    ) -> Result<(Self, Task<Message>), Blockage> {
        let t0 = Instant::now();
        let db_path = config::library_db_file().ok_or_else(|| Blockage::Nowhere {
            detail: "this system offers no data directory for baz to keep an index in".to_owned(),
        })?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Blockage::Nowhere {
                detail: format!("cannot create {}: {e}", parent.display()),
            })?;
        }
        let mut library = Library::open(&db_path).map_err(|e| Blockage::of(&e))?;
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
            songs: Vec::new(),
            search_albums: Vec::new(),
            search_open: false,
            search_action: crate::search::Action::Play,
            cover_action: CoverAction::Play,
            search_selection: crate::selection::State::default(),
            search_scroll_offset: 0.0,
            search_viewport_h: 0.0,
            opened: None,
            selection: crate::selection::State::default(),
            edition_choice: HashMap::new(),
            thumbs: ThumbCache::new(
                NonZeroUsize::new(art::THUMB_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN),
            ),
            artist_images: LruCache::new(
                NonZeroUsize::new(art::ARTIST_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN),
            ),
            artist_image_pending: HashSet::new(),
            no_artist_image: HashSet::new(),
            thumb_px: HashMap::new(),
            heroes: LruCache::new(
                NonZeroUsize::new(art::HERO_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN),
            ),
            hero_pending: None,
            last_requested: None,
            thumb_jobs: ThumbJobs::default(),
            no_art: HashSet::new(),
            scan_rx: Some(scan_rx),
            roots,
            unavailable: HashSet::new(),
            health: crate::health::Log::default(),
            refresh: scan::Refresh::new(scan::REFRESH_INTERVAL, Instant::now()),
            folder_input: String::new(),
            folder_error: None,
            folder_pending_removal: None,
            prunable: Vec::new(),
            prune_pending: false,
            unrooted_pending: false,
            scanning: true,
            files_skipped: 0,
            problem: None,
            scroll_offset: 0.0,
            grid_size: Size::new(
                WINDOW.width - theme::INDEX_LANE_W,
                WINDOW.height - theme::APP_BAR_H - theme::top_bar_h(WINDOW.width, lane_open),
            ),
            last_scan_log: Instant::now(),
            hovered_album: None,
            hovered_all_songs: false,
            tile_hover: Keyed::new(),
            vibe: crate::vibe::State::default(),
            art_shown: None,
            art_prior: None,
            art_dissolve: Tween::settled(1.0).with_curve(motion::Curve::Linear),
            window_w: WINDOW.width,
            lane_open,
            lane_played: HashMap::new(),
            lane_recent: Vec::new(),
            lane_stamp: 0,
            collection: vm::Collection::default(),
            artist_facts: HashMap::new(),
            artist_also_on: HashMap::new(),
        };
        shelf.health.record(
            crate::health::Level::Working,
            "Library scan started",
            format!("Checking {} configured folders", shelf.roots.len()),
        );
        // `rebuild_shelves` folds the ledger onto the records it has just
        // built (ADR-0030 §4): once, here, and never again from the file.
        shelf.rebuild_shelves();
        let shelf_task = shelf.request_visible_thumbs();
        crate::baz_log!(
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
                self.search_selection.clear();
                self.refilter();
                self.search_open = !self.query.trim().is_empty();
                self.search_scroll_offset = 0.0;
                iced::widget::operation::scroll_to(
                    views::search::scroll_id(),
                    AbsoluteOffset { x: 0.0, y: 0.0 },
                )
            }
            // **The `×`, which is `Esc`'s pointer route** — the identical
            // function, so the two cannot drift (ADR-0036 §4).
            Message::ClearSearch => self.clear_query(),
            Message::SearchScrolled(viewport) => {
                self.search_scroll_offset = viewport.absolute_offset().y;
                self.search_viewport_h = viewport.bounds().height;
                Task::none()
            }
            // **Type anywhere** has no arm here any more. Its message is
            // answered by the shell (`App::type_anywhere`), which reaches the
            // well — the Library, and the lane opened if the well is in it —
            // before handing the text down to [`Self::type_into_query`]. The
            // shelf cannot do that half: the place and the lane are the
            // shell's state, and the owner's move put the field in the lane.
            Message::EscapePressed => self.peel(),
            Message::GroupKeySelected(key) => self.arrange_by(key),
            Message::RailJumped(run) => self.jump_to_shelf(run),
            Message::Scrolled(viewport) => {
                self.scroll_offset = viewport.absolute_offset().y;
                let bounds = viewport.bounds();
                // The scrollable's *outer* bounds. The rows are laid out inside
                // the lanes it reserves, so the grid is told what the rows
                // actually get — otherwise the estimate and the measurement
                // disagree, and at a boundary width that is one column too
                // many. The reservation is [`theme::WALL_RESERVE`]: the bar's
                // 4 px **and** the index rail's 108, because the scrollable now
                // takes the whole body width so its bar can be drawn on the
                // window's edge and the rail is stacked under it
                // (`views::shelf::view`). It was the bar's width alone while
                // the rail was a `row!` sibling that took its lane first.
                // A scrollable leaving the tree can briefly report an empty
                // viewport. That is not a new one-column layout: keep the last
                // real width until either another real viewport or a window
                // resize supplies its replacement. This matters to the other
                // collection places, which share this grid but have no Library
                // wall of their own to measure it again.
                if bounds.width > theme::WALL_RESERVE {
                    self.grid_size.width = bounds.width - theme::WALL_RESERVE;
                }
                if bounds.height > 0.0 {
                    self.grid_size.height = bounds.height;
                }
                self.request_visible_thumbs()
            }
            Message::WindowResized(size) => {
                self.window_w = size.width;
                // Estimate until the next scroll event reports real bounds.
                // The rail's lane comes off here too, because the scrollable
                // the next `Scrolled` will measure has already given it up.
                // The strip's height is *resolved* against the window **and
                // the lane's state** — below the split the strip is two lines,
                // and an estimate that assumed one would mis-virtualize 40 px
                // of shelf. It took only the width until now, while the strip
                // itself was drawn at `App::body_width`; between a 1000 and a
                // 1056 px window with the lane open the two disagreed by
                // exactly those 40 px. One function, both facts.
                self.grid_size = Size::new(
                    self.grid_width(),
                    (size.height - theme::APP_BAR_H - theme::top_bar_h(size.width, self.lane_open))
                        .max(100.0),
                );
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
            Message::ThumbLoaded(id, edge, elapsed, decoded) => {
                self.finish_thumb(id, edge, elapsed, decoded)
            }
            // The hero tier's answer, in the thumbnail tier's own shape: a
            // decode that found nothing is recorded in the **same**
            // known-absent set, because both tiers ran the same resolution
            // order and a second ask would be a question already answered.
            Message::HeroLoaded(id, hero) => {
                if self.hero_pending == Some(id) {
                    self.hero_pending = None;
                }
                match hero {
                    Some(hero) => {
                        self.heroes.put(id, hero);
                    }
                    None => {
                        self.no_art.insert(id);
                    }
                }
                Task::none()
            }
            Message::ArtistImageLoaded(id, image) => {
                self.artist_image_pending.remove(&id);
                match image {
                    Some(image) => {
                        self.artist_images.put(id, image);
                    }
                    None => {
                        self.no_artist_image.insert(id);
                    }
                }
                Task::none()
            }
            Message::ScanTick => self.drain_scan(),
            _ => Task::none(),
        }
    }

    /// **<kbd>Enter</kbd>'s defensive album-level fall-through** outside the
    /// open chooser: the record the wall was last left for when no query
    /// stands, else the top-ranked matching album. A normal standing query has
    /// the chooser open and never reaches this older compatibility path.
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
    /// **What the defensive query fallback needle-drops**: the Songs section's
    /// own first row — the top-ranked matching track (ADR-0021),
    /// as (its record's wall id, its row in the selected edition) for
    /// [`App::play_track`] to spend (doc 09 §5; ADR-0023 §2's amendment).
    ///
    /// `None` with no query, and `None` when nothing matches — <kbd>Enter</kbd>
    /// then falls through to [`Self::enter_plays`], whose empty-query answer
    /// (the record the wall was last left for) is unchanged. The row is
    /// resolved by [`vm::song_row`], so what <kbd>Enter</kbd> plays is
    /// exactly what a press on the section's first row plays — one answer,
    /// two routes.
    pub(crate) fn enter_drops_needle(&self) -> Option<(u64, usize)> {
        if self.query.trim().is_empty() {
            return None;
        }
        let song = self.songs.first()?;
        let album = self.albums.iter().find(|album| album.id == song.album_id)?;
        let chosen = self.edition_choice.get(&album.id).copied();
        let row = vm::song_row(album, chosen, song)?;
        Some((album.id, row))
    }

    /// The selected search row, only while it still belongs to the current
    /// ranked result set. A query edit may make an old content key stale; Enter
    /// must never activate a row no longer on screen.
    fn selected_search_track(&self) -> Option<Content> {
        let Content::SearchTrack { album, row } = self.search_selection.selected()? else {
            return None;
        };
        self.songs
            .iter()
            .any(|song| {
                if song.album_id != album {
                    return false;
                }
                let Some(record) = self.albums.iter().find(|record| record.id == album) else {
                    return false;
                };
                let chosen = self.edition_choice.get(&album).copied();
                vm::song_row(record, chosen, song) == Some(row)
            })
            .then_some(Content::SearchTrack { album, row })
    }

    pub(crate) fn search_result_count(&self) -> usize {
        self.songs.len() + self.search_albums.len()
    }

    pub(crate) fn search_result_content(&self, index: usize) -> Option<Content> {
        if let Some(song) = self.songs.get(index) {
            let album = self.albums.iter().find(|album| album.id == song.album_id)?;
            let chosen = self.edition_choice.get(&album.id).copied();
            let row = vm::song_row(album, chosen, song)?;
            return Some(Content::SearchTrack {
                album: album.id,
                row,
            });
        }
        self.search_albums
            .get(index.checked_sub(self.songs.len())?)
            .copied()
            .map(Content::Album)
    }

    fn search_result_index(&self, content: Content) -> Option<usize> {
        (0..self.search_result_count())
            .find(|index| self.search_result_content(*index) == Some(content))
    }

    fn move_search_selection(&mut self, delta: i32) -> Task<Message> {
        let selected = self
            .search_selection
            .selected()
            .and_then(|content| self.search_result_index(content));
        let Some(index) = crate::search::moved_index(selected, self.search_result_count(), delta)
        else {
            return Task::none();
        };
        let Some(content) = self.search_result_content(index) else {
            return Task::none();
        };
        self.search_selection.select(content);
        self.search_action = crate::search::Action::Play;

        let top = crate::search::result_top(index, self.songs.len());
        let bottom = top + crate::search::ROW_H;
        let viewport = self.search_viewport_h;
        if viewport <= 0.0 {
            return blur_search();
        }
        let target = if top < self.search_scroll_offset {
            Some(top)
        } else if bottom > self.search_scroll_offset + viewport {
            Some((bottom - viewport).max(0.0))
        } else {
            None
        };
        Task::batch([
            blur_search(),
            target.map_or_else(Task::none, |y| {
                iced::widget::operation::scroll_to(
                    views::search::scroll_id(),
                    AbsoluteOffset { x: 0.0, y },
                )
            }),
        ])
    }

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
        self.search_selection.clear();
        self.refilter();
        self.search_open = true;
        self.search_scroll_offset = 0.0;
        Task::batch([
            iced::widget::operation::focus(search_id()),
            iced::widget::operation::scroll_to(
                views::search::scroll_id(),
                AbsoluteOffset { x: 0.0, y: 0.0 },
            ),
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
        self.search_selection.clear();
        self.refilter();
        self.search_open = false;
        self.search_scroll_offset = 0.0;
        Task::batch([
            blur_search(),
            iced::widget::operation::scroll_to(
                views::search::scroll_id(),
                AbsoluteOffset { x: 0.0, y: 0.0 },
            ),
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
        let needs_larger_art = density.art_max() > self.density.art_max();
        self.density = density;
        if needs_larger_art {
            // A tighter density deliberately decodes fewer pixels. Moving
            // back to a looser one must not stretch those smaller handles;
            // the prepared disk cache makes refilling this bounded LRU cheap.
            self.thumbs.clear_handles();
            self.thumb_px.clear();
            self.last_requested = None;
        }
        let now = self.shelves().height();
        let anchored = if was > 0.0 {
            (self.scroll_offset * now / was).max(0.0)
        } else {
            0.0
        };
        self.scroll_offset = anchored;
        persist_density(density);
        Task::batch([
            iced::widget::operation::scroll_to(
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
    /// The **query**, and since 2026-08-10 that is the whole of it. The layer
    /// under it was the shuffle pool's marks, and it went when shuffle stopped
    /// being a draw from the wall and became a property of the player: there is
    /// no pool on the wall to peel. Escape never stopped the music before and
    /// still does not.
    ///
    /// The query step **clears and blurs**, which is type-anywhere's doing
    /// (ADR-0017 step 11) — see [`Self::clear_query`] for why holding the caret
    /// stopped being right once any letter could reopen the query.
    fn peel(&mut self) -> Task<Message> {
        if !self.query.is_empty() {
            return self.clear_query();
        }
        Task::none()
    }

    /// **Arrange the wall by `key`** — the top bar's row of words and `1`–`6`
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
            iced::widget::operation::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
            self.request_visible_thumbs(),
        ])
    }

    /// **The All songs list, resolved from this wall** (`crate::implicit`).
    ///
    /// Built on demand rather than held, because the list *is* the wall and the
    /// wall is recomputed: an implicit playlist that cached itself would be a
    /// snapshot claiming to be a view, and would go stale the moment a query
    /// was typed. It costs one pass over `visible`, and is asked for only when
    /// something is about to be played or drawn.
    pub(crate) fn all_songs(&self) -> crate::implicit::ImplicitList {
        crate::implicit::ImplicitList::all_songs(&self.albums, &self.visible, |id| {
            self.edition_choice.get(&id).copied()
        })
    }

    /// **All songs over the whole library**, whatever the wall is filtered to —
    /// what Home's tile draws and plays (`crate::implicit`).
    ///
    /// [`Self::all_songs`]'s sibling and not its variant: same origin, same
    /// name, same sleeve, different scope. `ImplicitList::everything` carries
    /// the argument for why Home's scope is the collection rather than the
    /// wall's, and it is short — Home shows no wall and no query, so a filter
    /// set on another page has nothing on screen to be read from.
    pub(crate) fn everything(&self) -> crate::implicit::ImplicitList {
        crate::implicit::ImplicitList::everything(&self.albums, |id| {
            self.edition_choice.get(&id).copied()
        })
    }

    /// One artist's chronological implicit `All songs` list, resolved from
    /// the same records and edition choices their page draws.
    pub(crate) fn artist_songs(&self, artist: u64) -> Option<crate::implicit::ImplicitList> {
        let name = crate::views::artist::label(self, artist)?;
        Some(crate::implicit::ImplicitList::artist(
            &self.albums,
            artist,
            name,
            |id| self.edition_choice.get(&id).copied(),
        ))
    }

    /// Records carrying this artist as a track credit while filed under a
    /// different album artist, in the wall's current order.
    pub(crate) fn artist_also_on(&self, artist: u64) -> Vec<&vm::AlbumVm> {
        self.artist_also_on
            .get(&artist)
            .into_iter()
            .flatten()
            .filter_map(|index| self.albums.get(*index))
            .collect()
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
            iced::widget::operation::scroll_to(
                scroll_id(),
                AbsoluteOffset {
                    x: 0.0,
                    y: target.top,
                },
            ),
            self.request_visible_thumbs(),
        ])
    }

    /// **Re-hang the wall after the lane changed width** — the one re-hang
    /// the product permits, and the whole of what makes it safe.
    ///
    /// Two things happen, in this order. The viewport estimate is corrected
    /// (the next `Scrolled` will report the real bounds), and then **the wall
    /// scrolls so the shelf that was at the top of the viewport is still at
    /// the top**. Not the pixel offset: the columns changed, so every row
    /// moved, and a preserved offset would land on a different shelf. The
    /// machinery is [`shelf::Shelves::run_at`], which already maps an offset onto the
    /// run it is inside, and [`Self::jump_to_shelf`], which is what the index
    /// rail spends.
    ///
    /// The last-opened record's 2 px rule is drawn from data rather than from
    /// geometry, so it is still on the right tile afterwards — which is the
    /// anchor the eye actually uses.
    fn rehang(&mut self) -> Task<Message> {
        let here = self.shelves().run_at(self.scroll_offset);
        self.grid_size = Size::new(self.grid_width(), self.grid_size.height);
        if let Some(run) = here {
            return self.jump_to_shelf(run);
        }
        // Above the first shelf — the top of the wall stays the top.
        self.scroll_offset = 0.0;
        Task::batch([
            iced::widget::operation::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
            self.request_visible_thumbs(),
        ])
    }

    /// **The grid's width**: the window's, less the returns lane and the index
    /// rail's lane.
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
        (self.window_w
            - theme::sidebar_w(self.window_w, self.lane_open)
            - theme::INDEX_LANE_W
            - theme::WALL_SCROLLBAR_W)
            .max(0.0)
    }

    /// **The width the place's own body gets**: the window, less the returns
    /// lane.
    ///
    /// The strip, the place headers and every breakpoint inside a place read
    /// this rather than the window: the lane is a *column*, so a body that
    /// resolved its two-line split against the window would split at the wrong
    /// moment and hang its content off a line that is no longer there.
    pub(crate) fn body_width(&self) -> f32 {
        (self.window_w - theme::sidebar_w(self.window_w, self.lane_open)).max(0.0)
    }

    /// Advance the shelf's own transitions.
    fn tick_motion(&mut self, now: Instant) -> Task<Message> {
        self.tile_hover.tick(now);
        if !self.art_dissolve.tick(now) {
            // **A settled dissolve holds no picture.** The same rule
            // [`Keyed::tick`] follows when it drops its key: a transition at
            // rest must not keep a reference to the thing it moved, or the
            // outgoing hero's 4 MiB would outlive the 200 ms that needed it and
            // the LRU's budget would be a fiction.
            self.art_prior = None;
        }
        Task::none()
    }

    /// Whether the shelf still needs a clock (see [`App::moving`]).
    fn moving(&self) -> bool {
        self.tile_hover.live() || self.art_dissolve.live()
    }

    /// **Commit what the Now playing place draws of the record, and start the
    /// dissolve when the picture — not the track — has changed** (ADR-0020's
    /// third amendment; the owner, 2026-08-10: *"when changing track there
    /// isn't any kind of nice visual transition for album art in now playing.
    /// we should have something a bit nicer, like a quick fade"*).
    ///
    /// Called after **every** message, for [`App::request_hero`]'s reason and
    /// at its cost: the two moments that can change this surface's artwork are
    /// the engine naming another record and a hero decode landing, and asking
    /// on both by asking always is one call site instead of a list that has to
    /// stay complete. At rest it is an `Option` compare.
    ///
    /// # The three rules, and each is a defect avoided
    ///
    /// 1. **The picture, never the track.** Consecutive tracks on one record
    ///    share a cover, and the first line out of this function is the record
    ///    the surface is already committed to — so a twelve-track album is
    ///    *twelve* track changes and **no** transition, no clock and no frame.
    ///    Where two records genuinely differ, the predicate is still the
    ///    picture: [`Change::between`] compares the handles being drawn.
    /// 2. **The new art, not the new track.** A record whose hero is still
    ///    decoding has **no answer**, and this returns without touching
    ///    anything — the surface goes on drawing the picture it has. Starting
    ///    on `TrackStarted` instead would dissolve to whatever was ready, which
    ///    is a 320 px thumbnail or nothing at all, and then pop when the hero
    ///    landed: worse than the cut it replaces, twice over.
    /// 3. **Two pictures, or no transition.** A record with no art draws the
    ///    wall's deterministic gradient, which is a *stand-in* rather than
    ///    artwork; dissolving one is decoration, and ADR-0020 §3 forbids that.
    ///    So art → no art, and no art → art, stay the hard cuts they are today.
    /// 4. **`watching` — the surface is on screen.** The commitment is
    ///    unconditional, so opening the place finds the right picture whenever
    ///    the record changed; the *tween* is not, because a clock easing a hero
    ///    nobody is looking at would redraw whatever place is on screen a dozen
    ///    times for nothing. See [`App::settle_art`] for why this differs from
    ///    [`Self::request_hero`], which is ungated on purpose.
    ///
    /// # The second LRU entry, checked rather than trusted
    ///
    /// This needs both pictures alive at once and adds no cache to get them.
    /// [`art::HERO_CACHE_ENTRIES`] is 2 and [`Self::request_hero`] `get`s the
    /// *sounding* record — so when the incoming hero is `put`, the entry it
    /// would evict is the third-oldest and there is no third: the record that
    /// just stopped is still decoded, and `art_prior`'s handle is an `Arc` onto
    /// those same pixels rather than a copy of them. Asserted rather than
    /// assumed, because the entry that makes it true was written for a prefetch
    /// this product does not have yet:
    /// `the_hero_lru_holds_both_records_a_dissolve_needs`.
    fn settle_art(&mut self, sounding: Option<u64>, watching: bool, now: Instant) {
        let Some(id) = sounding else {
            // Nothing is sounding: there is no record on this surface to draw,
            // so there is nothing to dissolve *to* and the transition is
            // abandoned rather than run out. The light goes out with the music
            // and so does the picture ([`App::warm_lamp`]'s own rule).
            self.art_shown = None;
            self.art_prior = None;
            self.art_dissolve.set(1.0);
            return;
        };
        if self
            .art_shown
            .as_ref()
            .is_some_and(|(shown, _)| *shown == id)
        {
            return;
        }
        // **The answer, or nothing at all** — rule 2. `None` here is "the
        // decode has not come back", which is not the same as "there is no
        // art"; the second is an answer and lives in `no_art`.
        let answer = if let Some(hero) = self.hero(id) {
            Some(hero.clone())
        } else if self.no_art.contains(&id) {
            None
        } else {
            return;
        };
        let prior = self.art_shown.take().map(|(_, hero)| hero);
        let change = Change::between(prior.as_ref(), answer.as_ref());
        self.art_shown = answer.map(|hero| (id, hero));
        match change {
            // **Committed, but cut** — the picture changed while the place was
            // not on screen. The surface is correct the moment it is opened and
            // no clock was spent easing something nobody saw.
            Change::Dissolve if !watching => {
                self.art_prior = None;
                self.art_dissolve.set(1.0);
            }
            Change::Dissolve => {
                self.art_prior = prior;
                self.art_dissolve.set(0.0);
                self.art_dissolve.go(1.0, motion::DISSOLVE, now);
            }
            Change::Cut => {
                self.art_prior = None;
                self.art_dissolve.set(1.0);
            }
        }
    }

    /// **What the Now playing place draws of the record, and how far through a
    /// change it is** — one answer, so the cover and the field derived from it
    /// cannot disagree about which record they are of.
    pub(crate) fn showing(&self) -> Showing<'_> {
        Showing {
            hero: self.art_shown.as_ref().map(|(_, hero)| hero),
            from: self.art_prior.as_ref(),
            t: self.art_dissolve.value(),
        }
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
        // **Home's figures, counted here and nowhere else.** One pass over the
        // tracks that were just rebuilt, on the same schedule the rebuild runs
        // on — which is what keeps the `COLLECTION` footer off the per-frame
        // path (ADR-0030 §4). The arrangement does not change any of the four,
        // but re-counting is cheaper than reasoning about which caller changed
        // what.
        self.collection = vm::Collection::count(&self.albums, self.library.len());
        (self.artist_facts, self.artist_also_on) = vm::artist_inventory(&self.albums);
        // Rebuilding changes the album behind every virtual position even
        // though app-bar search itself no longer changes the wall.
        self.forget_requested();
        self.refilter();
        // The album ids survive a re-arrangement (see above), so the fold does
        // too — but a *rescan* can add and remove records, and the lane must
        // not go on naming one that is gone.
        if !self.lane_played.is_empty() || self.history.is_some() {
            self.fold_history_onto_records();
        }
    }

    /// **The ledger, folded onto records** — the whole of the lane's reading
    /// of history, and it happens twice in a process: at launch, and whenever
    /// the library itself is rebuilt.
    ///
    /// One pass over every track. That is the cost ADR-0030 §4 budgets and it
    /// is paid where the file is already being read; what the contract forbids
    /// is paying it *per frame*, which is why the result lives in
    /// [`Self::lane_played`] and is thereafter maintained by events.
    fn fold_history_onto_records(&mut self) {
        self.lane_played = match self.history.as_ref() {
            Some(history) => crate::lane::by_record(
                self.albums.iter().flat_map(|album| {
                    album
                        .editions
                        .iter()
                        .flat_map(|edition| edition.tracks.iter())
                        .map(move |track| (album.id, track.path.as_path()))
                }),
                // **The plays that were a *record* being put on**, which is
                // not every play of its tracks (ADR-0034). A run reified from
                // a list touched the list; re-deriving the records from those
                // play lines is exactly the attribution the live fix removes,
                // and doing it here is what made a list played last week come
                // back as its albums. `last_played_unlisted` is
                // `last_played_unix_s` minus those plays — and for a ledger
                // with no markers, which is every ledger written before this
                // shipped, it *is* `last_played_unix_s`.
                |path| history.last_played_unlisted(path),
            ),
            None => HashMap::new(),
        };
        self.rebuild_lane_recent();
    }

    /// The lane's records half, re-resolved from [`Self::lane_played`].
    ///
    /// O(albums) — a sort of the touched ones and a truncation to 24. Called
    /// when the fold is rebuilt and when one play moves one record, which is
    /// exactly the *"a `TrackStarted` updates one entry and re-sorts 24"* the
    /// contract promises.
    fn rebuild_lane_recent(&mut self) {
        let touched: Vec<crate::lane::Touched> = self
            .albums
            .iter()
            .filter_map(|album| {
                let at = self.lane_played.get(&album.id).copied()?;
                Some(crate::lane::Touched {
                    subject: crate::lane::Subject::Record(album.id),
                    // The wall's own two lines, verbatim — a record must not
                    // be named one thing on a tile and another in the lane.
                    name: album
                        .title
                        .clone()
                        .unwrap_or_else(|| "Unknown Album".to_owned()),
                    under: album.artist.label().to_owned(),
                    at: Some(at),
                })
            })
            .collect();
        self.lane_recent = crate::lane::recent(touched);
        self.lane_stamp = self.lane_stamp.wrapping_add(1);
    }

    /// A play was recorded: the record it belongs to is now the most recently
    /// touched thing in the lane.
    ///
    /// The moment is *now* rather than the ledger's, because the ledger is a
    /// snapshot read at launch and re-reading it here would be the per-frame
    /// file read the contract refuses. The two agree to within the length of
    /// the play.
    pub(crate) fn record_played(&mut self, path: &std::path::Path, at: u64) {
        let Some(album) = self.albums.iter().find(|album| {
            album
                .editions
                .iter()
                .any(|edition| edition.tracks.iter().any(|track| track.path == path))
        }) else {
            return;
        };
        let id = album.id;
        if self.lane_played.insert(id, at) == Some(at) {
            return;
        }
        self.rebuild_lane_recent();
    }

    /// Keep the wall's complete projection and rebuild the two relevance-
    /// ordered app-bar result sets for the current query.
    fn refilter(&mut self) {
        // Search no longer filters or replaces the Library body: the current
        // place remains unchanged under the app-wide dropover. The wall's
        // projection is therefore always the complete arranged collection.
        self.visible = (0..self.albums.len()).collect();
        // The dropover's two relevance-ordered projections. SEARCH_LIMIT is a
        // work bound, not an eight-row presentation cap; the view virtualizes
        // this result set into one scroll surface.
        self.songs = vm::song_hits(&self.library, &self.query, vm::SEARCH_LIMIT);
        self.search_albums = vm::album_hits(&self.library, &self.query, vm::SEARCH_LIMIT);
        self.search_action = crate::search::Action::Play;
        // The shelves are contiguous slices of `albums` and `visible` is in
        // the same order, so each shelf's surviving count is one walk of the
        // two lists together rather than a second filter that could disagree
        // with the first.
        self.visible_counts = surviving_per_shelf(&self.visible, &self.groups);
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
                    crate::baz_log!("[scan] periodic refresh");
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
            Message::MoveMusicFolderUp(index) => self.move_root(*index, -1),
            Message::MoveMusicFolderDown(index) => self.move_root(*index, 1),
            Message::ConfirmPruneMissing => self.prune_pending = true,
            Message::CancelPruneMissing => self.prune_pending = false,
            Message::PruneMissing => return Some(self.prune_missing()),
            Message::ConfirmPruneUnrooted => self.unrooted_pending = true,
            Message::CancelPruneUnrooted => self.unrooted_pending = false,
            Message::PruneUnrooted => return Some(self.prune_unrooted()),
            Message::ForceSync => {
                if !self.scanning {
                    crate::baz_log!("[scan] force sync requested");
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
    fn library_view<'a>(
        &'a self,
        playlists: Option<&'a std::path::Path>,
    ) -> views::settings::LibraryView<'a> {
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
            unrooted: self.library.unrooted_paths(),
            unrooted_pending: self.unrooted_pending,
            playlists,
            prunable: &self.prunable,
            prune_pending: self.prune_pending,
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
        crate::baz_log!("[config] holding {}", dir.display());
        self.roots.push(dir);
        persist_roots(&self.roots);
        // Incremental, not forced: a folder that overlaps one baz already holds
        // must not cost a re-read of every file in it.
        self.start_scan(scan::ScanMode::Incremental);
        Task::none()
    }

    fn move_root(&mut self, index: usize, delta: i8) {
        if self.scanning || self.folder_pending_removal.is_some() {
            return;
        }
        let Some(to) = shifted_index(self.roots.len(), index, delta) else {
            return;
        };
        self.roots.swap(index, to);
        persist_roots(&self.roots);
        crate::baz_log!(
            "[config] music folder moved from {} to {}",
            index + 1,
            to + 1
        );
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
            Ok(count) => {
                crate::baz_log!("[index] {count} tracks forgotten with {}", root.display());
            }
            Err(error) => {
                crate::baz_log!("[index] could not forget {}: {error}", root.display());
                self.problem = Some(format!("could not forget that folder: {error}"));
            }
        }
        // The wall's mark and the art caches are keyed by album id, and the
        // albums a forgotten folder held are gone — so the rebuild has to be
        // followed by the same clean-up a finished scan does.
        self.opened = None;
        self.no_art.clear();
        self.no_artist_image.clear();
        self.rebuild_shelves();
        self.request_visible_thumbs()
    }

    /// Forget the exact missing-path preview the last completed scan produced.
    /// `forget_paths` is one transactional source of truth with folder removal
    /// and preserves first-seen tombstones, so a mistaken confirmation is
    /// repaired by bringing the files back and scanning again.
    fn prune_missing(&mut self) -> Task<Message> {
        self.prune_pending = false;
        if self.prunable.is_empty() {
            return Task::none();
        }
        match self.library.forget_paths(&self.prunable) {
            Ok(count) => {
                crate::baz_log!("[index] {count} confirmed missing tracks forgotten");
                self.health.record(
                    crate::health::Level::Ready,
                    "Missing albums pruned",
                    format!(
                        "{count} index entries removed. Audio, playlists and listening history were untouched."
                    ),
                );
                self.prunable.clear();
                self.opened = None;
                self.no_art.clear();
                self.no_artist_image.clear();
                self.rebuild_shelves();
                self.request_visible_thumbs()
            }
            Err(error) => {
                self.problem = Some(format!("could not prune missing albums: {error}"));
                self.health.record(
                    crate::health::Level::Error,
                    "Could not prune missing albums",
                    error.to_string(),
                );
                Task::none()
            }
        }
    }

    /// Forget rootless legacy rows after showing every path. Unlike a scan,
    /// this is an explicit listener decision, so it can safely address rows
    /// no configured root is able to prove absent.
    fn prune_unrooted(&mut self) -> Task<Message> {
        self.unrooted_pending = false;
        let paths = self.library.unrooted_paths();
        if paths.is_empty() {
            return Task::none();
        }
        match self.library.forget_paths(&paths) {
            Ok(count) => {
                crate::baz_log!("[index] {count} rootless legacy tracks forgotten");
                self.health.record(
                    crate::health::Level::Ready,
                    "Unheld tracks removed from the index",
                    format!(
                        "{count} index entries removed. Audio, playlists and listening history were untouched."
                    ),
                );
                self.opened = None;
                self.no_art.clear();
                self.no_artist_image.clear();
                self.rebuild_shelves();
                self.request_visible_thumbs()
            }
            Err(error) => {
                self.problem = Some(format!("could not prune unheld tracks: {error}"));
                self.health.record(
                    crate::health::Level::Error,
                    "Could not remove unheld tracks",
                    error.to_string(),
                );
                Task::none()
            }
        }
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
        if self
            .problem
            .as_deref()
            .is_some_and(|problem| problem.contains("not reachable"))
        {
            self.problem = None;
        }
        self.files_skipped = 0;
        self.refresh.restarted(Instant::now());
        if self.roots.is_empty() {
            self.scan_rx = None;
            self.scanning = false;
            return;
        }
        self.health.record(
            crate::health::Level::Working,
            "Library scan started",
            format!("Checking {} configured folders", self.roots.len()),
        );
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
            prunable,
            scanned,
            missing,
            finished,
        } = drained;
        self.apply_scan(
            fresh_tracks,
            &vanished,
            prunable,
            scanned,
            missing,
            finished,
        )
    }

    /// Take everything the worker has said since the last tick, without
    /// touching the index — the receiving half of [`Shelf::drain_scan`].
    #[expect(
        clippy::too_many_lines,
        reason = "one receiver drain deliberately keeps every scan-worker message's state transition together"
    )]
    fn collect_scan(&mut self) -> Option<Drained> {
        let rx = self.scan_rx.as_ref()?;
        // Batches are kept per root, because the root is what makes the write
        // an `add_tracks_under`: it is the fact removal's second gate will read
        // back. A tick usually holds one root's worth; a small library can hold
        // several, and the order is the order they arrived in.
        let mut fresh_tracks: Vec<(PathBuf, Vec<baz_core::library::TrackMeta>)> = Vec::new();
        let mut vanished: Vec<std::path::PathBuf> = Vec::new();
        let mut prunable: Vec<std::path::PathBuf> = Vec::new();
        let mut scanned: Vec<(PathBuf, i64)> = Vec::new();
        let mut missing: Vec<(PathBuf, String)> = Vec::new();
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(ScanUpdate::Batch {
                    root,
                    tracks,
                    failed,
                    failures,
                }) => {
                    self.files_skipped += failed;
                    for (path, reason) in failures {
                        self.health.record(
                            crate::health::Level::Warning,
                            "File skipped",
                            format!("{}\n{reason}", path.display()),
                        );
                    }
                    match fresh_tracks.last_mut() {
                        Some((held, batch)) if *held == root => batch.extend(tracks),
                        _ => fresh_tracks.push((root, tracks)),
                    }
                }
                Ok(ScanUpdate::Removed { paths }) => vanished.extend(paths),
                Ok(ScanUpdate::Prunable { paths }) => prunable.extend(paths),
                Ok(ScanUpdate::RootDone {
                    root,
                    at_ns,
                    added,
                    updated,
                    unchanged,
                    failed,
                }) => {
                    record_root_scan(&mut self.health, &root, [added, updated, unchanged, failed]);
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
                    crate::baz_log!(
                        "[scan] done: {added} added, {updated} updated, {unchanged} unchanged, \
                         {removed} removed, {failed} files skipped, \
                         {unavailable} folders unavailable, {secs:.1} s ({rate:.0} tracks/s)"
                    );
                    self.health.record(
                        if failed > 0 || unavailable > 0 {
                            crate::health::Level::Warning
                        } else {
                            crate::health::Level::Ready
                        },
                        "Library scan complete",
                        format!(
                            "{added} added · {updated} updated · {removed} removed · \
                             {failed} files skipped · {unavailable} folders unavailable"
                        ),
                    );
                    finished = true;
                    break;
                }
                Ok(ScanUpdate::Error(error)) => {
                    crate::baz_log!("[scan] failed to start: {error}");
                    self.problem = Some(format!("scan failed: {error}"));
                    self.health
                        .record(crate::health::Level::Error, "Library scan failed", error);
                    finished = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.health.record(
                        crate::health::Level::Error,
                        "Library scan stopped",
                        "The scan worker disconnected before reporting completion",
                    );
                    self.problem = Some("scan stopped unexpectedly".to_owned());
                    finished = true;
                    break;
                }
            }
        }
        Some(Drained {
            fresh_tracks,
            vanished,
            prunable,
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
        prunable: Vec<PathBuf>,
        scanned: Vec<(PathBuf, i64)>,
        missing: Vec<(PathBuf, String)>,
        finished: bool,
    ) -> Task<Message> {
        if !prunable.is_empty() || finished {
            self.prunable = prunable;
            self.prune_pending = false;
        }
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
            crate::baz_log!("[scan] {} is unavailable: {reason}", root.display());
            self.health.record(
                crate::health::Level::Warning,
                "Folder unavailable",
                format!(
                    "{}\n{reason}\nThe existing library entries were kept.",
                    root.display()
                ),
            );
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
                crate::baz_log!(
                    "[index] could not record the scan of {}: {error}",
                    root.display()
                );
                self.health.record(
                    crate::health::Level::Error,
                    "Could not record scan time",
                    format!("{}\n{error}", root.display()),
                );
            }
        }

        let mut task = Task::none();
        if !fresh_tracks.is_empty() || !vanished.is_empty() {
            for (root, tracks) in fresh_tracks {
                if let Err(error) = self.library.add_tracks_under(Some(&root), tracks) {
                    crate::baz_log!("[index] write failed: {error}");
                    self.problem = Some(format!("library write failed: {error}"));
                    self.health.record(
                        crate::health::Level::Error,
                        "Library write failed",
                        error.to_string(),
                    );
                }
            }
            if !vanished.is_empty() {
                match self.library.remove_tracks(vanished) {
                    Ok(count) => crate::baz_log!("[index] {count} vanished tracks removed"),
                    Err(error) => {
                        crate::baz_log!("[index] removal failed: {error}");
                        self.problem = Some(format!("library removal failed: {error}"));
                        self.health.record(
                            crate::health::Level::Error,
                            "Library removal failed",
                            error.to_string(),
                        );
                    }
                }
            }
            self.rebuild_shelves();
            if self.last_scan_log.elapsed() > Duration::from_secs(2) {
                self.last_scan_log = Instant::now();
                crate::baz_log!(
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
            self.no_artist_image.clear();
            task = Task::batch([task, self.request_visible_thumbs()]);
        }
        task
    }

    /// Drop the range guard in [`Shelf::request_visible_thumbs`].
    ///
    /// Called wherever *which* albums a range names has changed — a new
    /// filter, a new arrangement, a scan that added records. The guard is an
    /// answer cached against a question about positions, and these are the
    /// events that change what a position means.
    fn forget_requested(&mut self) {
        self.last_requested = None;
    }

    /// Kick off off-thread decodes for every visible tile whose thumbnail is
    /// neither cached, in flight, nor known-absent. Ported from the spike;
    /// `get` (not `peek`) refreshes LRU recency for visible entries.
    /// **Decode art for records that are on screen but not on the wall** —
    /// the returns lane's rows and the Home place's `RECENTLY ADDED` row.
    ///
    /// The wall's own prefetch is a range over the *visible slice of the
    /// wall*, which is the right guard for the wall and answers nothing about
    /// a record drawn beside it: a recently-added record two thousand rows
    /// down, or a lane row for something played last week, is on screen with
    /// its decode never asked for, and falls back to the gradient forever.
    ///
    /// The same decode path, the same cache, the same in-flight and
    /// known-absent sets — so a record's sleeve is one decode however many
    /// surfaces are drawing it, and asking twice costs one set lookup.
    /// **Decode the sounding record at [`art::HERO_PX`]**, once, and derive its
    /// field while the pixels are already in hand (doc 12 §5.2, §5.3).
    ///
    /// Everything expensive happens on the blocking worker: the decode, and
    /// [`crate::field::Field::derive`]'s one pass over the sampled pixels. What
    /// crosses back is a handle, one `f32`, and three hue angles — **the UI
    /// thread never sees a pixel of a cover**, which is what keeps the field's
    /// per-frame cost at three colour conversions.
    ///
    /// `no_art` is shared with the thumbnail tier deliberately: the two tiers
    /// run the *same* resolution order and the same decode, so a record with no
    /// decodable art has none in both and asking twice would be asking a
    /// question already answered.
    ///
    /// # The successor is not prefetched, and cannot be yet
    ///
    /// Doc 12 §5.2 budgets the two entries as *"the sounding record and the one
    /// after it"*. **The one after it cannot be named from here.** The UI's
    /// record of the run is [`vm::QueueVm`], whose rows carry a title, an
    /// artist and an album *string* and **no path and no album id** — the
    /// engine holds the paths. Resolving the next record would mean matching
    /// two strings against the wall and hoping no listener owns two editions of
    /// the same record, which is a worse answer than not having one.
    ///
    /// So the second entry is spent on **the record that was sounding a moment
    /// ago**, which the LRU gives for free and which a `Prev` press or a jump
    /// back up the run collects. Naming the successor is
    /// [ADR-0034](../../docs/adr/0034-the-run-and-its-list.md)'s `Origin` work
    /// — step M3, which is what puts identity on a run's rows — and it is one
    /// line here once that lands.
    fn request_hero(&mut self, sounding: Option<u64>) -> Task<Message> {
        let Some(id) = sounding else {
            return Task::none();
        };
        // `get`, not `peek`: asking for the sounding record's hero is what
        // keeps it the freshest of the two entries, so the one the LRU drops
        // is always the one that stopped playing.
        if self.heroes.get(&id).is_some()
            || self.hero_pending == Some(id)
            || self.no_art.contains(&id)
        {
            return Task::none();
        }
        let Some(album) = self.albums.iter().find(|album| album.id == id) else {
            return Task::none();
        };
        self.hero_pending = Some(id);
        let path = album.first_track.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let (w, h, rgba) = art::load_hero_cached(&path)?;
                    let back = art::load_back(&path)
                        .map(|(w, h, rgba)| iced_image::Handle::from_rgba(w, h, rgba));
                    Some(Hero {
                        field: crate::field::Field::derive(w, h, &rgba),
                        px: shortest_edge(w, h),
                        handle: iced_image::Handle::from_rgba(w, h, rgba),
                        back,
                    })
                })
                .await
                .ok()
                .flatten()
            },
            move |hero| Message::HeroLoaded(id, hero),
        )
    }

    fn request_artist_image(&mut self, artist: u64) -> Task<Message> {
        if self.artist_images.get(&artist).is_some()
            || self.artist_image_pending.contains(&artist)
            || self.no_artist_image.contains(&artist)
        {
            return Task::none();
        }
        let Some(album) = self
            .albums
            .iter()
            .find(|album| vm::artist_id(&album.artist) == artist)
        else {
            return Task::none();
        };
        self.artist_image_pending.insert(artist);
        let path = album.first_track.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    art::load_artist(&path)
                        .map(|(w, h, rgba)| iced_image::Handle::from_rgba(w, h, rgba))
                })
                .await
                .ok()
                .flatten()
            },
            move |image| Message::ArtistImageLoaded(artist, image),
        )
    }

    pub(crate) fn artist_image(&self, artist: u64) -> Option<&iced_image::Handle> {
        self.artist_images.peek(&artist)
    }

    /// The sounding record's hero, when one is decoded — the Now playing
    /// place's artwork and the source of its field.
    pub(crate) fn hero(&self, id: u64) -> Option<&Hero> {
        self.heroes.peek(&id)
    }

    /// The shortest edge of `id`'s decoded **thumbnail**, in pixels.
    ///
    /// What the Now playing place clamps its artwork against for the frames
    /// between arriving and its hero landing. See [`Self::thumb_px`]'s field
    /// for why this is a true bound rather than a guess.
    pub(crate) fn thumb_edge(&self, id: u64) -> Option<f32> {
        self.thumb_px.get(&id).copied()
    }

    /// A decoded thumbnail from either the resident tier or the bounded
    /// off-screen LRU. Views are read-only, so observation never changes
    /// eviction order; residency is updated from measured viewport events.
    pub(crate) fn thumb(&self, id: u64) -> Option<&iced_image::Handle> {
        self.thumbs.peek(id)
    }

    fn request_thumbs_for(&mut self, ids: &[u64]) -> Task<Message> {
        self.thumbs.focus_chrome(ids.iter().copied());
        self.request_target_thumbs()
    }

    /// Re-aim the scheduler from one complete target snapshot. Updating the
    /// wall, a page or resident chrome can no longer discard still-visible
    /// work nominated by either of the other two.
    ///
    /// # `focus` replaces, so it has to be given everything
    ///
    /// [`ThumbJobs::focus`] **drains the whole foreground queue and re-adds
    /// only its argument** — that is what "re-aim" means, and it is right: a
    /// wall that has scrolled past a record should stop waiting to decode it.
    /// But this function was handing it a **delta** — the targets that were
    /// neither cached nor *already queued* — so every re-aim threw away every
    /// job that was merely waiting its turn, and re-added nothing in its place.
    ///
    /// **That is the cold start, exactly.** iced emits `Scrolled` once the
    /// scrollable measures its real bounds (and `WindowResized` when the first
    /// resize lands); each handler recomputes the visible range and calls this;
    /// and the last one flushes the batch the scan drain had just queued,
    /// before two workers could consume more than two of it. Nothing else
    /// happens on an untouched window, so the wall sits on gradients until a
    /// scroll re-aims a range whose ids are now missing again and re-queues
    /// them. Measured on a fresh 25-album library at 1280 × 860 with no
    /// interaction at all: **two** decodes completed, and frames at 6, 9, 12
    /// and 15 seconds pixel-identical.
    ///
    /// The repair is to pass the **complete snapshot** rather than the delta —
    /// drop the `thumb_jobs.contains` exclusion and keep the other two. It is
    /// safe because `focus` already skips in-flight ids and `queued.insert` is
    /// idempotent, so drain-then-re-add now *preserves* queued work instead of
    /// discarding it, while still dropping whatever left the target set. The
    /// two exclusions that stay are the ones that are facts about the id rather
    /// than about the queue: `touch` says it is already decoded (and marks it
    /// recently used, which is why it must still be called on every target),
    /// and `no_art` says there is nothing on disk to decode.
    ///
    /// `request_thumbs` (a page) and `request_thumbs_for` (resident chrome)
    /// come through here too and get the same repair. `request_visible_thumbs`
    /// keeps its `last_requested` range guard, which is the dedupe for
    /// *identical* re-aims and is a separate concern from this one.
    /// [`ThumbJobs::retry`] — the density-grew retry — pushes to the front
    /// without draining and is untouched, which is item 30's shipped contract.
    fn request_target_thumbs(&mut self) -> Task<Message> {
        let mut wanted = Vec::new();
        for id in self.thumbs.targets() {
            if self.thumbs.touch(id) || self.no_art.contains(&id) {
                continue;
            }
            wanted.push(id);
        }
        self.thumb_jobs.focus(wanted);
        self.start_queued_thumbs()
    }

    /// Every thumbnail Home can draw: the All songs collage plus the visible
    /// newest-record row. Lane rows are resolved separately by the shell from
    /// the lane's exact mixed viewport.
    fn home_art(&self) -> Vec<u64> {
        let mut ids = self.everything().art;
        ids.extend(
            crate::views::home::newest(self, self.grid())
                .iter()
                .map(|album| album.id),
        );
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    fn request_visible_thumbs(&mut self) -> Task<Message> {
        let (start, end) = self
            .shelves()
            .visible_albums(self.scroll_offset, self.grid_size.height);
        let tiles = self.visible.len();
        let (start, end) = (start.min(tiles), end.min(tiles));
        let visible_ids: Vec<u64> = self.visible[start..end]
            .iter()
            .filter_map(|&album_index| self.albums.get(album_index).map(|album| album.id))
            .collect();
        self.thumbs.focus_wall(visible_ids.iter().copied());
        // **Nothing new is on screen, so there is nothing to ask for.**
        //
        // Every resize step delivers *three* of these — `WindowResized` with
        // its estimate, then `Scrolled` when the scrollable measures its real
        // bounds, then `Scrolled` again when the grid that changed underneath
        // it changed the content's height (iced republishes a viewport whose
        // `content_bounds` moved, `iced_widget-0.13.4/src/scrollable.rs:1249`).
        // Measured at 87 messages a second under a dragged edge, and the work
        // behind each is a pass over every group in the library plus a walk of
        // the visible slice. Two of the three ask for exactly what the first
        // asked for.
        //
        // The guard is the *answer*, not the question: the range of albums on
        // screen. A drag that reveals no new record now costs one comparison.
        if self.last_requested == Some((start, end)) {
            return Task::none();
        }
        self.last_requested = Some((start, end));
        self.request_target_thumbs()
    }

    /// Kick off off-thread decodes for the albums in `ids` whose thumbnail is
    /// neither cached, in flight, nor known-absent — the playlist sleeves'
    /// supply line (ADR-0024 §A1), and deliberately nothing but a re-aim of
    /// [`Self::request_visible_thumbs`]'s pipeline: same cache, same decode
    /// path, same placeholder while it runs. An id the wall no longer holds
    /// is skipped; the collage cell keeps its gradient, which is the same
    /// honest reading a tile gives art that cannot be decoded.
    fn request_thumbs(&mut self, ids: &[u64]) -> Task<Message> {
        self.thumbs.focus_page(ids.iter().copied());
        self.request_target_thumbs()
    }

    fn finish_thumb(
        &mut self,
        id: u64,
        requested_edge: u32,
        elapsed: Duration,
        decoded: Option<(f32, usize, iced_image::Handle)>,
    ) -> Task<Message> {
        self.thumb_jobs.finished(id);
        match decoded {
            Some((px, bytes, handle)) if requested_edge >= self.density.art_max_px() => {
                self.thumb_px.insert(id, px);
                self.thumbs.put(id, handle, bytes);
            }
            Some(_) => {
                // Density grew while this blocking decode was in flight. The
                // smaller result is correct data but no longer enough pixels
                // for the active layout, so immediately replace the one job.
                self.thumb_jobs.retry(id);
            }
            None => {
                self.no_art.insert(id);
            }
        }
        let task = self.start_queued_thumbs();
        if std::env::var_os("BAZ_PERF_LOG").is_some() {
            let decoded_bytes = self.thumbs.decoded_bytes();
            let decoded_mib = decoded_bytes / (1024 * 1024);
            let decoded_tenths = (decoded_bytes % (1024 * 1024)) * 10 / (1024 * 1024);
            crate::baz_log!(
                "[art] thumb {id} in {:.1} ms; cache={} resident={} retained={} decoded={decoded_mib}.{decoded_tenths} MiB queued={} in-flight={} completed={} peak={}",
                elapsed.as_secs_f64() * 1e3,
                self.thumbs.len(),
                self.thumbs.resident_len(),
                self.thumbs.retained_len(),
                self.thumb_jobs.queued.len(),
                self.thumb_jobs.in_flight.len(),
                self.thumb_jobs.completed,
                self.thumb_jobs.peak,
            );
        }
        task
    }

    /// Fill the two decoder slots from the current page first, then from the
    /// visible returns lane. Importantly, blocking
    /// jobs are spawned only after a slot is acquired; putting a semaphore
    /// *inside* hundreds of already-spawned jobs would still grow Tokio's
    /// blocking pool and retain every queued task allocation.
    fn start_queued_thumbs(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        let thumb_edge = self.density.art_max_px().min(art::THUMB_PX);
        while self.thumb_jobs.in_flight.len() < art::THUMB_DECODE_CONCURRENCY {
            let Some(id) = self.thumb_jobs.pop() else {
                break;
            };
            if self.thumbs.peek(id).is_some() || self.no_art.contains(&id) {
                continue;
            }
            let Some(path) = self
                .albums
                .iter()
                .find(|album| album.id == id)
                .map(|album| album.first_track.clone())
            else {
                continue;
            };
            self.thumb_jobs.started(id);
            tasks.push(Task::perform(
                async move {
                    let started = Instant::now();
                    let decoded = tokio::task::spawn_blocking(move || {
                        art::load_thumb_cached(&path, thumb_edge).map(decoded)
                    })
                    .await
                    .ok()
                    .flatten();
                    (started.elapsed(), decoded)
                },
                move |(elapsed, handle)| Message::ThumbLoaded(id, thumb_edge, elapsed, handle),
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
    fn view<'a>(
        &'a self,
        player: &'a PlayerState,
        lamp: f32,
        collecting: crate::playlists::Collecting,
    ) -> Element<'a, Message> {
        column![
            views::top_bar::view(self, self.body_width()),
            views::shelf::view(self, player, lamp, collecting)
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

fn record_root_scan(health: &mut crate::health::Log, root: &std::path::Path, counts: [usize; 4]) {
    let [added, updated, unchanged, failed] = counts;
    crate::baz_log!(
        "[scan] {}: {added} added, {updated} updated, {unchanged} unchanged, {failed} skipped",
        root.display()
    );
    health.record(
        if failed > 0 {
            crate::health::Level::Warning
        } else {
            crate::health::Level::Ready
        },
        "Folder scanned",
        format!(
            "{}\n{added} added · {updated} updated · {unchanged} unchanged · {failed} skipped",
            root.display()
        ),
    );
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
    /// Missing paths whose absent parent makes them manual-confirmation only.
    prunable: Vec<PathBuf>,
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
            crate::baz_log!(
                "[config] {} is not valid UTF-8; it cannot be written to config.toml \
                 (this session is unaffected)",
                root.display()
            );
        }
    }
    persist(|config| config.music_dirs = roots.to_vec());
}

fn shifted_index(len: usize, index: usize, delta: i8) -> Option<usize> {
    match delta {
        -1 if index > 0 && index < len => Some(index - 1),
        1 if index + 1 < len => Some(index + 1),
        _ => None,
    }
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
                    crate::baz_log!("[config] folder picker failed: {err}");
                    None
                }
            }
        },
        Message::MusicFolderPicked,
    )
}

/// Read a listener-selected JSON theme without blocking the event loop.
fn pick_theme_file() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(|| {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("Baz theme", &["json"])
                    .pick_file()
                else {
                    return Err("Theme import cancelled; nothing changed.".to_owned());
                };
                std::fs::read_to_string(&path)
                    .map_err(|error| format!("Could not read {}: {error}", path.display()))
            })
            .await
            .unwrap_or_else(|error| Err(format!("Theme picker failed: {error}")))
        },
        Message::ThemeFilePicked,
    )
}

/// Save a round-trippable copy of the selected room locally.
fn export_theme(selection: String) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let suggested = selection
                    .strip_prefix("custom:")
                    .unwrap_or(&selection)
                    .to_owned();
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("Baz theme", &["json"])
                    .set_file_name(format!("{suggested}.json"))
                    .save_file()
                else {
                    return Err("Theme export cancelled; nothing changed.".to_owned());
                };
                crate::theme_file::write_export(&path, &selection)
            })
            .await
            .unwrap_or_else(|error| Err(format!("Theme export failed: {error}")))
        },
        Message::ThemeExported,
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
            Ok(count) => {
                crate::baz_log!("[index] {count} rows now recorded under {}", root.display());
            }
            Err(error) => crate::baz_log!(
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

/// **How many of each shelf's tiles survived the query**, in shelf order.
///
/// The shelves are contiguous slices of the flat list and `surviving` is in
/// the same order, so this is one walk of the two lists together rather than
/// a second filter that could disagree with the first.
fn surviving_per_shelf(surviving: &[usize], groups: &[GroupVm]) -> Vec<usize> {
    let mut seen = surviving.iter().peekable();
    groups
        .iter()
        .map(|group| {
            let mut count = 0;
            while seen.next_if(|index| **index < group.end).is_some() {
                count += 1;
            }
            count
        })
        .collect()
}

/// Remember how closely it hangs — the same terms exactly (ADR-0017 §1.3).
///
/// The zoom is a *gesture*; where it landed is state. A listener who pressed
/// <kbd>Ctrl</kbd>+<kbd>-</kbd> twice expects that wall next time, and had to
/// go nowhere to ask for it.
fn persist_density(density: shelf::Density) {
    persist(|config| config.density = density);
}

/// Remember whether shuffle is on — `persist_density`'s argument again, and
/// the reason it is a *standing* decision rather than session state is in
/// [`config::Config::shuffle`]: it governs what the first `Play` of the next
/// session does, so a shuffle that forgot itself overnight would be a mode a
/// listener had to re-assert every morning.
fn persist_shuffle(on: bool) {
    persist(|config| config.shuffle = on);
}

/// Remember whether the returns lane stands open — `persist_density`'s
/// argument exactly (ADR-0030 §3: one bool in `config.toml`, beside the
/// density step and the group key, and **no Settings row**).
fn persist_lane(open: bool) {
    persist(|config| config.sidebar_open = open);
}

/// Remember the radio-like foreground choice on the same view-state footing
/// as density: its control lives on Now Playing and its result survives it.
fn persist_visualization_foreground(foreground: crate::visualizer::Foreground) {
    persist(|config| config.visualization_foreground = foreground);
}

/// Whether Now Playing owns a continuous redraw clock in this state.
///
/// Focus is intentionally not an input. The place being visible, a sounding
/// record, and a visual that actually changes are the complete cost gate.
fn visualization_clock(
    place: Place,
    sounding: bool,
    visualization: crate::visualizer::State,
) -> bool {
    place == Place::NowPlaying
        && sounding
        && (visualization.mode.active() || visualization.foreground.draws_case())
}

/// Whether the 20-second fact-feed clock exists. It is absent everywhere the
/// line cannot be seen, so enabling it has no idle cost in other places.
fn fact_clock(place: Place, sounding: bool, on: bool) -> bool {
    place == Place::NowPlaying && sounding && on
}

fn fullscreen_target(mode: window::Mode) -> window::Mode {
    if mode == window::Mode::Fullscreen {
        window::Mode::Windowed
    } else {
        window::Mode::Fullscreen
    }
}

/// Which record, if any, currently earns a hero decode.
///
/// Album detail always draws one. Elsewhere the sounding record is prefetched
/// only while the selected Now Playing foreground can actually draw it.
fn hero_target(
    place: Place,
    sounding: Option<u64>,
    foreground: crate::visualizer::Foreground,
) -> Option<u64> {
    match place {
        Place::Album(id) => Some(id),
        _ if foreground.draws_art() => sounding,
        _ => None,
    }
}

/// **What `session.toml` should say about the run** — or `None` for *leave the
/// file exactly as it is*.
///
/// Pure, and the **single** answer to that question: both writers go through
/// it ([`App::sync_snapshot`] on every move of the run, [`App::leave_for_good`]
/// on the way out), because a guard that protects the listener's place must
/// not exist in two copies that can drift apart.
///
/// # Nothing has sounded ⇒ nothing is written
///
/// The clause the whole feature turns on, and it is one line. Launch hands the
/// restored queue back to the engine ([`App::restore_the_run`]), which moves
/// every mark this shell watches — and a write at that moment records a cursor
/// of 0 and a position of 0, overwriting the interrupted point with *the fact
/// that it was restored*. **The listener would lose their place by opening
/// baz**, which is the exact opposite of what ADR-0023 §6 is for.
///
/// Stating it as *has anything sounded* rather than as *is a row playing*
/// closes two holes that the narrower reading left open, and both are real:
///
/// - **The way out.** [`App::leave_for_good`] writes unconditionally, so
///   opening baz and closing it again without pressing anything used to spend
///   the interrupted position exactly as a restore-time write would have. The
///   run is now still the run you left.
/// - **A library that is not mounted yet.** A snapshot whose files do not
///   resolve produces no queue at all, and the old *no queue ⇒ write an empty
///   snapshot* arm then deleted the run outright. A NAS that was not up when
///   baz opened no longer costs the listener their place.
///
/// It also has a quiet second consequence the Home place depends on: while
/// nothing has sounded, `App::resume` cannot change under the `CONTINUE` band
/// that is reading it, so what the band shows cannot drift mid-frame.
///
/// # And once something has
///
/// The engine's account, whatever it is. A row is playing (or paused) and that
/// row is the run; **the queue has ended and the run is written away**, because
/// a run played to its end is not a run that was interrupted and an offer to
/// carry on with something you completed is the interface remembering
/// something that is over — the same judgement `views::home::standing` makes on
/// screen, so the two cannot disagree across a restart.
///
/// A queue merely *replaced* is deliberately not that: the phase is still
/// whatever it was and the engine's next `TrackStarted` is already on its way,
/// so the file is left alone rather than blanked and rewritten a millisecond
/// later.
fn next_snapshot(player: &PlayerState, position_ms: u64) -> Option<crate::session::Snapshot> {
    if !player.has_sounded() {
        return None;
    }
    match player.queue() {
        Some(queue) if !queue.is_empty() => match player.playing_queue_row() {
            Some(cursor) => Some(crate::session::Snapshot {
                paths: queue.paths(),
                cursor,
                position_ms,
                provenance: queue.provenance().map(str::to_owned),
                // The *kind* survives the quit, so the strip offers the same
                // word tomorrow that it offers tonight. The **edit flag does
                // not** and deliberately: `queue_edited` is a fact about this
                // session, so a fixed run edited tonight comes back fixed,
                // which is the same rule every other session-scoped reading
                // here already follows.
                assembled: matches!(queue.source, vm::RunSource::Assembled),
            }),
            None if player.phase() == player::Phase::Stopped => {
                Some(crate::session::Snapshot::default())
            }
            None => None,
        },
        // Something sounded and there is no queue behind it any more: there is
        // no run left to remember.
        _ => Some(crate::session::Snapshot::default()),
    }
}

/// **The interrupted run, read once** (ADR-0023 §6).
///
/// A missing file is an empty snapshot and not an error: a fresh install has
/// no run to continue, which is a state the Home place already draws — the
/// band is absent, not empty.
fn read_snapshot() -> crate::session::Snapshot {
    let snapshot = crate::session::session_file()
        .map(|path| crate::session::load(&path))
        .unwrap_or_default();
    if snapshot.is_empty() {
        crate::baz_log!("[session] no interrupted run");
    } else {
        crate::baz_log!(
            "[session] {} tracks held, cursor {} at {} ms",
            snapshot.paths.len(),
            snapshot.cursor,
            snapshot.position_ms
        );
    }
    snapshot
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
/// `baz_core::traversal` takes a seed rather than reading a clock or reaching
/// for a global generator, so that every pass it can produce is reproducible in
/// a test and identical on both sides of the protocol — the nondeterminism has
/// to enter *somewhere*, and this is that somewhere, in the shell, where
/// nothing is asserted about it.
///
/// A clock that refuses to answer (it has been set before the epoch) gives a
/// fixed seed rather than a panic. The consequence is that two runs on that
/// machine shuffle the same way, which is a strange machine's problem and not
/// worth a branch anywhere else.
/// **The traversal the shuffle control's two positions mean.**
///
/// One function so the start-up seed and the toggle cannot disagree about what
/// "on" is, and so the seed enters in exactly one place. Off is
/// [`Traversal::InOrder`] and carries nothing: there is no order to remember,
/// because the run never left its own.
fn traversal(on: bool) -> Traversal {
    if on {
        Traversal::Shuffled { seed: draw_seed() }
    } else {
        Traversal::InOrder
    }
}

fn draw_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_nanos() & u128::from(u64::MAX)).unwrap_or(0)
        })
}

/// **What the engine is told a run came from** — the encoded
/// [`Origin`](crate::origin::Origin) that rides on `SetQueue` and ends up as
/// the run's marker in the play ledger (ADR-0034 §2).
///
/// One function, so that both `SetQueue` sends in this file — `send_run`'s and
/// the snapshot's `restore_the_run` — say the same thing, and a third could not
/// quietly say something else.
///
/// The queue now carries the list identity separately from [`vm::RunSource`].
/// Two origins are durable attribution: a playlist file and an artist's
/// implicit `All songs`. Both exclude the records they quote from record
/// recency. Library-wide `All songs` deliberately does not: it has no row of
/// its own and retaining the records' touches is the useful reading of that
/// collection-wide gesture. The provenance fallback keeps restored and older
/// playlist queues honest.
///
/// # Why not `RunSource`, which is right here
///
/// [`vm::RunSource`] answers whether the queue can be saved, not which list it
/// came from. Its `Fixed` bucket includes records, artist lists, All songs and
/// draws, whose attribution rules differ; spending it here would conflate
/// them again.
fn run_origin(queue: &vm::QueueVm) -> Option<String> {
    use crate::origin::Origin;

    match queue.origin.as_ref() {
        Some(origin @ (Origin::Playlist { .. } | Origin::Artist { .. })) => Some(origin.encode()),
        Some(Origin::Album { .. } | Origin::AllSongs | Origin::Draw | Origin::Hand { .. }) => None,
        None => queue
            .provenance()
            .map(|name| Origin::playlist(name).encode()),
    }
}

fn read_history() -> Option<History> {
    let path = HistoryLedger::default_path()?;
    match History::read(&path) {
        Ok(history) => {
            crate::baz_log!(
                "[history] {} records over {} tracks from {}",
                history.records(),
                history.tracks().count(),
                path.display()
            );
            Some(history)
        }
        Err(error) => {
            crate::baz_log!("[history] cannot read {}: {error}", path.display());
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
        crate::baz_log!("[config] no config directory on this system; nothing is being remembered");
        return;
    };
    let stored = config::load(&path);
    let mut config = stored.clone();
    change(&mut config);
    if config == stored {
        return; // Unchanged.
    }
    match config::store(&path, &config) {
        Ok(()) => crate::baz_log!("[config] saved to {}", path.display()),
        Err(error) => crate::baz_log!("[config] could not save {}: {error}", path.display()),
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
    use crate::player::Availability;

    #[test]
    fn all_six_visual_states_keep_their_independent_costs() {
        for foreground in [
            crate::visualizer::Foreground::Cover,
            crate::visualizer::Foreground::JewelCase,
            crate::visualizer::Foreground::None,
        ] {
            let still = crate::visualizer::State {
                foreground,
                mode: crate::visualizer::Mode::Off,
                facts: false,
            };
            let spectral = crate::visualizer::State {
                mode: crate::visualizer::Mode::Spectrum,
                ..still
            };
            assert_eq!(
                visualization_clock(Place::NowPlaying, true, still),
                foreground.draws_case(),
                "{foreground:?} without spectrum"
            );
            assert!(
                visualization_clock(Place::NowPlaying, true, spectral),
                "{foreground:?} with spectrum"
            );
            assert!(!visualization_clock(Place::Library, true, spectral));
            assert!(!visualization_clock(Place::NowPlaying, false, spectral));
        }
    }

    #[test]
    fn focus_is_not_part_of_the_visible_visual_clock() {
        let state = crate::visualizer::State {
            foreground: crate::visualizer::Foreground::None,
            mode: crate::visualizer::Mode::Waveform,
            facts: false,
        };
        // There is deliberately no focus argument: a visible Now Playing
        // remains live while another application owns the keyboard.
        assert!(visualization_clock(Place::NowPlaying, true, state));
    }

    #[test]
    fn the_fact_clock_exists_only_for_a_visible_sounding_feed() {
        assert!(fact_clock(Place::NowPlaying, true, true));
        assert!(!fact_clock(Place::NowPlaying, true, false));
        assert!(!fact_clock(Place::NowPlaying, false, true));
        assert!(!fact_clock(Place::Library, true, true));
    }

    #[test]
    fn f11_round_trips_windowed_and_fullscreen_modes() {
        assert_eq!(
            fullscreen_target(window::Mode::Windowed),
            window::Mode::Fullscreen
        );
        assert_eq!(
            fullscreen_target(window::Mode::Fullscreen),
            window::Mode::Windowed
        );
        assert_eq!(
            fullscreen_target(window::Mode::Hidden),
            window::Mode::Fullscreen
        );
    }

    #[test]
    fn none_stops_hero_work_without_costing_album_detail() {
        use crate::visualizer::Foreground;
        assert_eq!(
            hero_target(Place::Library, Some(7), Foreground::Cover),
            Some(7)
        );
        assert_eq!(
            hero_target(Place::NowPlaying, Some(7), Foreground::JewelCase),
            Some(7)
        );
        assert_eq!(
            hero_target(Place::NowPlaying, Some(7), Foreground::None),
            None
        );
        assert_eq!(
            hero_target(Place::Album(9), Some(7), Foreground::None),
            Some(9)
        );
    }

    #[test]
    fn visible_art_replaces_stale_work() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([1, 2, 3]);
        jobs.focus([3, 4]);

        assert_eq!(jobs.foreground, VecDeque::from([3, 4]));
        assert_eq!(jobs.pop(), Some(3));

        jobs.focus([5]);
        assert_eq!(jobs.foreground, VecDeque::from([5]));
        assert!(!jobs.queued.contains(&4), "the old viewport was discarded");
    }

    #[test]
    fn in_flight_art_is_deduplicated_but_never_cancelled_by_a_new_viewport() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([7, 8]);
        assert_eq!(jobs.pop(), Some(7));
        jobs.started(7);

        jobs.focus([7, 9]);
        assert_eq!(jobs.foreground, VecDeque::from([9]));
        assert!(jobs.in_flight.contains(&7));
        assert_eq!(jobs.peak, 1);

        jobs.finished(7);
        assert_eq!(jobs.completed, 1);
        assert!(!jobs.in_flight.contains(&7));
    }

    #[test]
    fn one_complete_target_snapshot_keeps_page_and_chrome_work() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([10, 11, 20, 21]);

        assert_eq!(jobs.pop(), Some(10));
        assert_eq!(jobs.pop(), Some(11));
        assert_eq!(jobs.pop(), Some(20));
        assert_eq!(jobs.pop(), Some(21));
    }

    /// **A re-aim that changes nothing must lose nothing** — the cold start,
    /// as arithmetic.
    ///
    /// `focus` replaces, which is right: a wall that scrolled past a record
    /// should stop waiting to decode it. What was wrong was *what it was
    /// given*. `request_target_thumbs` handed it the targets that were neither
    /// cached nor **already queued**, so a re-aim over an unchanged viewport
    /// passed the empty set and the replace threw the whole queue away.
    ///
    /// On an untouched cold start that happens twice — iced emits `Scrolled`
    /// when the scrollable measures its real bounds, and `WindowResized` when
    /// the first resize lands — and there is no third event to re-queue
    /// anything, so the wall sits on gradients until someone touches it.
    /// Measured before the fix on a fresh 25-album library at 1280 × 860 with
    /// no interaction: **2** decodes completed and frames at 6, 9, 12 and 15
    /// seconds pixel-identical. After: **8**, the whole visible wall.
    ///
    /// This is the pure half of that, and it is deliberately written as the
    /// *shape of the call* rather than as a screenshot: the defect is that a
    /// caller passed a delta to a replacing queue, so what has to be pinned is
    /// that a snapshot survives the replace and a delta does not.
    #[test]
    fn re_aiming_with_the_whole_snapshot_keeps_queued_work_that_a_delta_would_drop() {
        // The snapshot: everything still wanted, including what is already
        // queued. Two workers have taken the first two; the rest are waiting.
        let mut jobs = ThumbJobs::default();
        jobs.focus([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(jobs.pop(), Some(1));
        jobs.started(1);
        assert_eq!(jobs.pop(), Some(2));
        jobs.started(2);

        // The re-aim iced delivers on its own, over an unchanged viewport.
        jobs.focus([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            jobs.foreground,
            VecDeque::from([3, 4, 5, 6, 7, 8]),
            "the six waiting decodes were discarded by a re-aim that asked for \
             exactly what was already asked for"
        );
        assert!(
            jobs.in_flight.contains(&1) && jobs.in_flight.contains(&2),
            "a re-aim must not re-queue what two workers are already decoding"
        );

        // And the delta the old caller would have computed — nothing is
        // uncached-and-unqueued, so it is empty — takes the queue with it.
        let mut delta = ThumbJobs::default();
        delta.focus([1, 2, 3, 4, 5, 6, 7, 8]);
        delta.pop();
        delta.pop();
        delta.focus(std::iter::empty());
        assert!(
            delta.foreground.is_empty(),
            "this is the defect, held here so the difference between the two \
             calls is visible in one place"
        );
    }

    /// The other half of the same rule: a re-aim still **drops** what left the
    /// target set, which is what makes `focus` a re-aim rather than an append.
    /// Passing the whole snapshot buys back the waiting work without buying
    /// back the work that scrolled away.
    #[test]
    fn a_snapshot_re_aim_still_drops_what_left_the_viewport() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([1, 2, 3, 4]);
        jobs.focus([3, 4, 5, 6]);
        assert_eq!(jobs.foreground, VecDeque::from([3, 4, 5, 6]));
        assert!(!jobs.queued.contains(&1) && !jobs.queued.contains(&2));
    }

    /// **The density retry still prepends rather than replacing** — item 30's
    /// shipped contract, re-asserted beside the change that altered how the
    /// queue is filled.
    ///
    /// A decode that completed too small after the density grew re-queues one
    /// id. It must not take the rest of the visible wall with it, which is
    /// exactly what it would do if it reached for `focus`.
    #[test]
    fn the_density_retry_prepends_and_keeps_the_rest_of_the_wall() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([1, 2, 3]);
        jobs.retry(9);
        assert_eq!(jobs.foreground, VecDeque::from([9, 1, 2, 3]));
    }

    /// **The stated budget is stated, derived from, and reachable** — item 37's
    /// first half, which is a decision rather than a repair.
    ///
    /// The owner: the art machinery *"was introduced to try to keep RAM usage
    /// down but we never specified a sensible limit."* Everything this asserts
    /// is the arithmetic of that limit, so the numbers in `art`'s prose cannot
    /// drift away from the constants underneath them.
    #[test]
    fn the_art_budget_is_a_stated_decision_the_tiers_derive_from() {
        const MIB: usize = 1024 * 1024;
        // The figure, and the two it is chosen against: the owner's 393-album
        // index at Spacious's 320 px ceiling, every cover square.
        const OWNERS_ALBUMS: usize = 393;
        const SPACIOUS_ENTRY: usize = 320 * 320 * 4;
        // The two smaller tiers, for the whole-process figure below.
        const HERO: usize = 1024 * 1024 * 4 * art::HERO_CACHE_ENTRIES;
        const ARTIST: usize = 256 * 256 * 4 * art::ARTIST_CACHE_ENTRIES;
        const {
            assert!(art::THUMB_BUDGET_BYTES == 160 * MIB);
            assert!(OWNERS_ALBUMS * SPACIOUS_ENTRY < art::THUMB_BUDGET_BYTES);
            // …and it is the smallest 32 MiB step that clears it, so the
            // headroom is not a second undeclared decision.
            assert!(OWNERS_ALBUMS * SPACIOUS_ENTRY > art::THUMB_BUDGET_BYTES - 32 * MIB);
        }
        // The speculative sub-budget is what the entry count is derived *from*,
        // and it comes to the count the tier has always had — so stating the
        // decision in bytes changed no behaviour.
        const {
            assert!(art::SPECULATIVE_BUDGET_BYTES == 25 * MIB);
            assert!(art::THUMB_CACHE_ENTRIES == 64);
            assert!(art::THUMB_CACHE_ENTRIES * SPACIOUS_ENTRY == art::SPECULATIVE_BUDGET_BYTES);
            assert!(art::SPECULATIVE_BUDGET_BYTES < art::THUMB_BUDGET_BYTES);
        }
        // And all decoded artwork in the process is the figure worth quoting,
        // which is the one a process monitor shows — and which Settings →
        // Debug now shows the resident set beside.
        const {
            assert!(HERO == 8 * MIB);
            assert!(ARTIST == 2 * MIB);
            assert!(art::THUMB_BUDGET_BYTES + HERO + ARTIST == 170 * MIB);
        }
    }

    /// **The resident tier's exemption is safe**, which is the budget's one
    /// hole and therefore the one thing that has to be argued rather than
    /// assumed.
    ///
    /// `trim_to_budget` will not evict art the current frame can draw — item
    /// 20's rule, and the reason this whole tier exists. That means a window
    /// whose *visible wall alone* exceeded [`art::THUMB_BUDGET_BYTES`] would
    /// exceed it. It cannot: the widest window baz supports pins **51 MiB** at
    /// its worst density, a little under a third of the budget.
    ///
    /// The bound is deliberately generous — a full 4K window, every tile at
    /// the density's *smallest* work so the count is maximal, no room taken by
    /// the two bars or the captions — and the margin it clears by is **stated
    /// rather than assumed**, because it is nearer than it looks: the worst
    /// density comes to about a third of the budget, not a hundredth. A tier
    /// that cannot be evicted is worth knowing the size of.
    ///
    /// Each density is costed against **its own** decode ceiling
    /// ([`crate::shelf::Density::art_max_px`]), which is the pairing that
    /// actually happens — Dense hangs the most tiles *and* decodes the
    /// smallest, so the two do not compound. Costing every density at
    /// [`art::THUMB_PX`] would be a worst case the product cannot reach and
    /// would fail this test for a reason that is not true.
    #[test]
    fn the_visible_wall_can_never_exhaust_the_art_budget() {
        use crate::shelf::Density;

        // Far past any window baz is dragged to, and the bars take none of it.
        let (window_w, window_h) = (3840.0_f32, 2160.0_f32);
        let worst = Density::ALL
            .iter()
            .map(|density| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a count of tiles across a window is small and non-negative"
                )]
                let tiles = ((window_w / density.art_min()).ceil() as usize)
                    * ((window_h / density.art_min()).ceil() as usize);
                let edge = density.art_max_px() as usize;
                (tiles * edge * edge * 4, *density, tiles)
            })
            .max_by_key(|(bytes, _, _)| *bytes)
            .expect("the densities are not empty");
        let (bytes, density, tiles) = worst;
        assert!(
            bytes * 2 < art::THUMB_BUDGET_BYTES,
            "the widest supported window at {density:?} pins {tiles} tiles, \
             {} MiB, against a {} MiB budget — the resident tier is exempt from \
             the trim, so it must stay comfortably under it",
            bytes / (1024 * 1024),
            art::THUMB_BUDGET_BYTES / (1024 * 1024)
        );
    }

    /// **The budget is enforced, and it is enforced in the right order.**
    ///
    /// Speculative art — decoded for, never displayed — goes first; then the
    /// least recently *visited* retained art; and the resident tier is never
    /// touched. Written over a tiny budget so the arithmetic is legible; the
    /// tiering is what is being asserted, not the size.
    #[test]
    fn the_budget_trims_speculative_art_first_then_the_least_recently_visited() {
        // One entry per byte-budget slot: `put_pixel` writes 4 bytes.
        let mut cache = ThumbCache::new(NonZeroUsize::new(8).expect("a cache"));

        // Three covers the listener has actually looked at, in order.
        for id in [1, 2, 3] {
            cache.focus_wall([id]);
            put_pixel(&mut cache, id);
            cache.focus_wall([]);
        }
        assert_eq!(cache.retained_len(), 3);

        // One the listener is looking at now.
        cache.focus_wall([9]);
        put_pixel(&mut cache, 9);
        assert_eq!(cache.resident_len(), 1);

        // And some speculative completions behind them.
        for id in [20, 21] {
            put_pixel(&mut cache, id);
        }
        assert_eq!(cache.recent.len(), 2);

        // Now squeeze. The trim runs on `put`, so a budget this small is
        // easier to exercise directly.
        cache.trim_to_budget();
        assert_eq!(
            cache.decoded_bytes(),
            6 * 4,
            "nothing should be dropped while the whole cache is far under budget"
        );

        // Re-touching a retained id makes it the most recent, which is the
        // ordering the trim depends on and the reason `retained` is an LRU.
        assert!(cache.touch(1));
        let order: Vec<u64> = cache.retained.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            order.first(),
            Some(&1),
            "visiting retained art did not make it recent, so a trim would drop \
             the art the listener just looked at"
        );

        // Resident art survives a trim that has nothing else left to take.
        cache.recent.clear();
        cache.retained.clear();
        cache.trim_to_budget();
        assert!(
            cache.peek(9).is_some(),
            "the trim reached into the current frame"
        );
    }

    /// **The trim actually drops things, and drops them in the stated order.**
    ///
    /// The test above establishes the ordering and the resident exemption; this
    /// one makes the budget *bind*, which needs entries big enough to reach it.
    /// The sizes are declared rather than decoded — `decoded_bytes` is a number
    /// the caller hands the cache from the real decode, so a 1 × 1 handle that
    /// claims a quarter of the budget exercises exactly the accounting under
    /// test without allocating 160 MiB in a unit test.
    #[test]
    fn the_budget_binds_and_takes_speculative_art_before_visited_art() {
        // Quarter-budget entries: four fit exactly, a fifth must displace one.
        let quarter = art::THUMB_BUDGET_BYTES / 4;
        let mut cache = ThumbCache::new(NonZeroUsize::new(64).expect("a cache"));
        let put = |cache: &mut ThumbCache, id: u64| {
            cache.put(id, pixel_handle(1), quarter);
        };

        // Two the listener has visited, oldest first, then two speculative.
        for id in [1, 2] {
            cache.focus_wall([id]);
            put(&mut cache, id);
            cache.focus_wall([]);
        }
        put(&mut cache, 20);
        put(&mut cache, 21);
        assert_eq!(cache.decoded_bytes(), 4 * quarter);
        assert!(cache.decoded_bytes() <= art::THUMB_BUDGET_BYTES);

        // A fifth decode. Speculative art goes first — art nobody has seen is
        // worth less than art the listener has.
        put(&mut cache, 22);
        assert!(
            cache.decoded_bytes() <= art::THUMB_BUDGET_BYTES,
            "the budget is not enforced"
        );
        assert!(
            cache.peek(1).is_some() && cache.peek(2).is_some(),
            "visited art was dropped while speculative art was still held"
        );
        assert!(
            cache.peek(20).is_none(),
            "the oldest speculative entry survived"
        );

        // With no speculative art left to absorb the overflow, the **least
        // recently visited** retained entry is what goes — and the one just
        // re-visited stays, which is the whole reason `retained` is ordered.
        cache.recent.clear();
        assert!(cache.touch(1), "1 is now the most recently visited");
        for id in [3, 4, 5] {
            cache.focus_wall([id]);
            put(&mut cache, id);
            cache.focus_wall([]);
        }
        assert!(cache.decoded_bytes() <= art::THUMB_BUDGET_BYTES);
        assert!(
            cache.peek(1).is_some(),
            "the trim dropped art the listener had looked at more recently than              art it kept"
        );
        assert!(
            cache.peek(2).is_none(),
            "the least recently visited retained art should have gone first"
        );
        assert!(
            [4, 5].iter().all(|id| cache.peek(*id).is_some()),
            "the art just visited was dropped"
        );
    }

    fn pixel_handle(red: u8) -> iced_image::Handle {
        iced_image::Handle::from_rgba(1, 1, vec![red, 0, 0, 255])
    }

    fn put_pixel(cache: &mut ThumbCache, id: u64) {
        cache.put(
            id,
            pixel_handle(u8::try_from(id % 255).expect("bounded color")),
            4,
        );
    }

    #[test]
    fn a_loaded_visible_sleeve_cannot_be_evicted_by_cache_churn() {
        let mut old = LruCache::new(NonZeroUsize::new(2).expect("a cache"));
        old.put(1, pixel_handle(1));
        old.put(2, pixel_handle(2));
        old.put(3, pixel_handle(3));
        assert!(
            old.peek(&1).is_none(),
            "reproduction: the old undifferentiated LRU evicts the visible sleeve"
        );

        let mut cache = ThumbCache::new(NonZeroUsize::new(2).expect("a cache"));
        put_pixel(&mut cache, 1);
        put_pixel(&mut cache, 2);
        cache.focus_wall([1]);

        for id in 3..20 {
            put_pixel(&mut cache, id);
        }

        assert!(cache.peek(1).is_some(), "the visible handle disappeared");
        assert_eq!(cache.resident_len(), 1);
        assert_eq!(cache.recent.len(), 2, "off-screen work stays bounded");
    }

    #[test]
    fn leaving_the_viewport_retains_art_that_was_actually_displayed() {
        let mut cache = ThumbCache::new(NonZeroUsize::new(2).expect("a cache"));
        cache.focus_wall([1]);
        cache.focus_chrome([1]);
        put_pixel(&mut cache, 1);

        cache.focus_wall([]);
        assert_eq!(
            cache.resident_len(),
            1,
            "another visible surface still quotes it"
        );
        cache.focus_chrome([]);
        assert_eq!(cache.resident_len(), 0);
        assert_eq!(cache.retained_len(), 1);
        assert!(
            cache.peek(1).is_some(),
            "unpinning does not drop the handle"
        );

        for id in 2..5 {
            put_pixel(&mut cache, id);
        }
        assert!(
            cache.peek(1).is_some(),
            "displayed art cannot become a gradient after unrelated churn"
        );
    }

    #[test]
    fn scroll_away_past_sixty_four_covers_and_return_keeps_every_shown_handle() {
        let mut cache = ThumbCache::new(
            NonZeroUsize::new(art::THUMB_CACHE_ENTRIES).expect("the production bound"),
        );
        let first = 1..=18;
        cache.focus_wall(first.clone());
        for id in first.clone() {
            put_pixel(&mut cache, id);
        }

        for page in 1..45 {
            let start = page * 18 + 1;
            let end = start + 17;
            cache.focus_wall(start..=end);
            for id in start..=end {
                put_pixel(&mut cache, id);
            }
        }

        cache.focus_wall(first.clone());
        for id in first {
            assert!(cache.peek(id).is_some(), "shown target {id} was evicted");
        }
        assert_eq!(cache.resident_len(), 18);
        assert_eq!(cache.retained_len(), 44 * 18);
        assert_eq!(cache.recent.len(), 0, "every fixture reached the viewport");
        assert_eq!(cache.decoded_bytes(), 45 * 18 * 4);
    }

    #[test]
    fn stale_density_retry_does_not_discard_other_visible_work() {
        let mut jobs = ThumbJobs::default();
        jobs.focus([10, 11, 12]);
        assert_eq!(jobs.pop(), Some(10));
        jobs.started(10);
        jobs.finished(10);
        jobs.retry(10);

        assert_eq!(jobs.foreground, VecDeque::from([10, 11, 12]));
    }

    #[test]
    fn queue_page_art_is_not_unpinned_by_the_resident_chrome_pass() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("app source");
        assert!(
            source.contains("Place::Playlists | Place::Playlist(_) | Place::Queue"),
            "the after-message chrome pass can evict visible Queue-row sleeves"
        );
        assert!(
            source.contains("ids.extend(state.all_songs().art)")
                && source.contains("self.playlists.panel_open"),
            "the floating playlist panel has visible collages but no residency supply"
        );
        assert!(
            source.contains("self.playlists.panel_open,")
                && source.contains("crate::views::home::standing"),
            "panel-open and Home Continue must participate in the target snapshot"
        );
    }

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

    #[test]
    fn folder_order_moves_only_to_an_existing_neighbour() {
        assert_eq!(shifted_index(3, 1, -1), Some(0));
        assert_eq!(shifted_index(3, 1, 1), Some(2));
        assert_eq!(shifted_index(3, 0, -1), None);
        assert_eq!(shifted_index(3, 2, 1), None);
        assert_eq!(shifted_index(3, 9, -1), None);
        assert_eq!(shifted_index(3, 1, 0), None);
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
    /// [`Self::shuffle_starts_what_it_draws_and_queues_whole_records`] is. A
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
        // Out to a record's page, on to Now playing, on to the settings, home.
        let place = place.album(7);
        assert_eq!(place, Place::Album(7));
        let place = place.go(crate::lane::Destination::NowPlaying);
        assert_eq!(place, Place::NowPlaying);
        assert!(!matches!(place, Place::Album(_)), "one place at a time");
        let place = place.settings();
        assert_eq!(place, Place::Settings);
        let place = place.back();
        assert_eq!(place, Place::Library);
        assert!(place.is_library());
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
    /// the chooser confirmation has its selected row/action, the arrangement has
    /// the top bar's row of words, and the zoom has the density marks at the
    /// foot of the index rail's lane (ADR-0028 — the row that once argued
    /// the gesture was its own control).
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
        const CONTROLS: [(&str, &str); 23] = [
            (
                "ToggleLane",
                "the `Collapse` control at the returns lane's foot (ADR-0030 §3) — \
                 the state you are in at full ink and inert, the other \
                 pressable, in the density detents' exact anatomy",
            ),
            (
                "Undo",
                "the transient `Undo` word beside the Queue place's summary \
                 and the playlist page's counts (doc 11 §5 P2) — present \
                 exactly while there is an edit to take back, which is \
                 exactly when the chord acts",
            ),
            ("PlayPause", "the bottom bar's play/pause button"),
            (
                "TogglePlaylists",
                "the Library strip's labelled `Playlists` door (ADR-0024 §5)",
            ),
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
            (
                "Direction",
                "the selected row/action in the open search chooser; outside \
                 search, the bottom bar's needle and volume fader",
            ),
            ("ToggleMute", "the bottom bar's speaker button"),
            (
                "ShowNowPlaying",
                "the returns lane's labelled `Now playing` row",
            ),
            ("ToggleSettings", "the top bar's Settings control"),
            ("HistoryBack", "the app bar's visible Back arrow"),
            ("HistoryForward", "the app bar's visible Forward arrow"),
            ("FocusSearch", "the top bar's search well"),
            (
                "EscapePressed",
                "every place's `‹ Library`, and — for the query layer the peel \
                 ends on — the well's own clear mark, which is this key's \
                 pointer route into the identical function (ADR-0036 §4)",
            ),
            (
                "QueryTyped",
                "the top bar's search well — the field ADR-0017 §1.2 kept, \
                 which a pointer clicks into to type the same query",
            ),
            (
                "PlayFirstMatch",
                "the selected app-bar search result while its chooser stands; \
                 the record page's `Play album` for the fall-through",
            ),
            (
                "DensityStep",
                "the density marks — at the foot of the index rail's lane on \
                 the Library, and on the block's own section rule on Home and \
                 an artist's page (ADR-0028 and its fourth-step amendment). \
                 Each sends this message with the exact delta the gesture \
                 would spend, so Ctrl+scroll and Ctrl+-/= are accelerators of \
                 a visible control now, not the control itself",
            ),
            (
                "GroupKeySelected",
                "the top bar's row of six words (ADR-0019); the first two, \
                 A–Z and ARTIST, are the same order broken into letter \
                 shelves and into a shelf per artist (ADR-0035, as thrice \
                 amended)",
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
            Key::Character("p".into()),
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
            Key::Character("7".into()),
            Key::Character("k".into()),
            Key::Character("z".into()),
            Key::Character("[".into()),
            Key::Character("]".into()),
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

    /// **Every play gesture goes through one arranger, and the mode cannot be
    /// half-applied.**
    ///
    /// This test has been rewritten twice by the owner's decisions and both
    /// halves of what it used to say are worth keeping in view. It pinned
    /// **the pull's silence** — a draw that sent no command at all — until the
    /// pull was removed (2026-08-10). And it pinned *"shuffle is a thing you
    /// **start**"* over `start_shuffle`, the wall's draw, until shuffle became
    /// a property of the player on the same day and there stopped being an act
    /// to start.
    ///
    /// What replaces both is the property the mode is actually judged on:
    /// **one place decides what order a run plays in.** [`App::send_run`] is
    /// that place, and every gesture that starts a run reaches it — press
    /// `Play` on a record, `Play all`, a playlist's `Play`, a track click.
    /// Four functions keeping a convention is how they would fall out of step;
    /// one function they all call is how they cannot.
    ///
    /// Pinned over the **source** rather than over behaviour, and that is the
    /// point of it: there is no `Shelf` to construct without a database and a
    /// scan thread, so the property is asserted as a fact about the text — in
    /// exactly the way `theme::every_surface_declares_the_edges_it_permits`
    /// pins the alignment laws. It cannot be satisfied by accident, and a
    /// future edit that sent its own `SetQueue` from a play gesture fails the
    /// build rather than the review.
    #[test]
    fn every_play_gesture_arranges_its_run_through_one_function() {
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

        // **Every gesture that starts a run.** Named individually rather than
        // swept, because the list *is* the claim: these four are what the
        // owner asked to agree.
        for gesture in [
            "play_album",
            "play_everything",
            "play_playlist",
            "play_track",
            "play_playlist_track",
        ] {
            let body = body(gesture);
            assert!(
                // The two `All songs` gestures reach it through `start`, the
                // four-line tail they share so that their one difference stays
                // their *scope* — which is itself the claim, so it is spelled
                // rather than papered over.
                body.contains("self.send_run(")
                    || body.contains("self.start(list)")
                    || body.contains("self.start_and_show(queue)"),
                "`{gesture}` starts a run without going through the arranger — \
                 shuffle would apply to some gestures and not others"
            );
            assert!(
                !body.contains("Command::SetQueue"),
                "`{gesture}` sends its own SetQueue past `send_run`"
            );
        }
        assert!(
            body("start").contains("self.send_run("),
            "the shared tail stopped going through the arranger"
        );
        assert!(
            body("start_and_show").contains("self.send_run("),
            "the confirmed album-start tail stopped going through the arranger"
        );

        // **The arranger sends the run as it was built, and says how to walk
        // it.** The two halves of the owner's second decision: the queue is
        // never permuted here, and the traversal is what carries shuffle.
        let arranger = body("send_run");
        assert!(
            arranger.contains("Command::SetTraversal"),
            "the walk is what shuffle changes, and the engine has to be told"
        );
        assert!(
            arranger.contains("queue.paths()") && arranger.contains("note_queue_sent"),
            "the run goes out as the gesture built it"
        );

        // **Nothing in this shell permutes a run any more.** Swept over the
        // whole file rather than over one function, because the value of the
        // decision is that there is nowhere left for a permutation to live.
        // Spelled in halves so these needles are not their own counter-examples.
        for (head, tail) in [
            ("shuffle", "::arranged"),
            ("shuffle", "::restored"),
            ("source", "_order"),
            ("note_shuffled", "_run"),
        ] {
            let gone = format!("{head}{tail}");
            assert!(
                !source.contains(&gone),
                "`{gone}` came back: shuffle is a property of the walk, and a \
                 run that gets re-ordered has a list being mutated again"
            );
        }

        // **Turning it off never stops the music, and never touches the run.**
        // `SetTraversal` lets the sounding track play out and re-plans what
        // follows; a queue command here would be the old design returning.
        let toggle = body("toggle_shuffle");
        assert!(toggle.contains("Command::SetTraversal"));
        assert!(
            !toggle.contains("Command::SetQueue")
                && !toggle.contains("Command::UpdateQueue")
                && !toggle.contains("Command::Play"),
            "the toggle touched the queue instead of the walk"
        );
        assert!(
            toggle.contains("persist_shuffle"),
            "a standing decision that is not written down is a session setting"
        );

        // **The pull, the wall's draw and the retained-order machinery are
        // gone, and nothing kept a stub of any of them.** Named here so that a
        // re-introduction is a deliberate act with a test to move rather than a
        // quiet reappearance. Spelled in two pieces for the reason above.
        for gone in ["draw_pull", "start_shuffle", "forget_source"] {
            let (head, tail) = gone.split_once('_').expect("a two-word name");
            assert!(
                !source.contains(&format!("fn {head}_{tail}")),
                "`{gone}` came back without its removal being reconsidered"
            );
        }
    }

    /// **S6 — the `All songs` gesture reifies its scope and plays from the
    /// top** (doc 09 §7.1).
    ///
    /// There were two of these until 2026-08-10. The strip's `Play all` played
    /// **the wall as arranged**; Home's `All songs` tile plays **the
    /// collection**. The owner removed the first that evening — *"please
    /// remove the 'Play all' button at the top of the library"* (ADR-0040) —
    /// and the action went with the control rather than lingering as a message
    /// nothing sends: an action with no visible control is the visible-control
    /// rule failing in the direction nobody checks for.
    ///
    /// So what is pinned here is the survivor, over the source for
    /// [`Self::every_play_gesture_arranges_its_run_through_one_function`]'s
    /// reason — there is no `Shelf` to construct without a database and a scan
    /// thread — with each criterion named by the literal a reviewer would have
    /// to move:
    ///
    /// - *the scope is the collection, never a query set on another page*;
    /// - *the first track sounds*: the run goes out and `Play` follows, one
    ///   press, no confirmation at any scale — §7.1's answer to the
    ///   10 000-track question is the virtual window, not a dialog;
    /// - *an empty library does nothing and claims nothing*.
    #[test]
    fn the_all_songs_gesture_reifies_its_scope_in_order() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");

        // **`Play all` is gone, control and action together.** A message no
        // control sends is the removal half-done. Read off the shipped half
        // of the file only — this test names both literals, and a sweep that
        // found its own assertion would never be able to pass.
        let code = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        assert!(
            !code.contains("fn play_all(&mut self"),
            "`play_all` outlived the button the owner removed"
        );
        assert!(
            !code.contains("PlayAll"),
            "`Message::PlayAll` is still in the enum with nothing to send it"
        );

        // **Home's tile plays the collection**, which is the owner's *"more
        // like a tile on the home screen, a special 'playlist'"*. It reads
        // `everything()` rather than `all_songs()`, so it cannot silently
        // apply a filter set on a page the listener is not standing on.
        let start = source
            .find("fn play_everything(&mut self")
            .expect("play_everything exists");
        let rest = &source[start..];
        let everything = &rest[..rest.find("\n    }\n").expect("a function ends")];
        assert!(
            everything.contains("state.everything()"),
            "Home's tile plays the collection, not whatever the wall is filtered to"
        );
        assert!(
            !everything.contains("state.all_songs()"),
            "Home's tile read a query it has nowhere to show"
        );
        assert!(everything.contains("self.start(list)"));
        assert!(everything.contains("if list.is_empty()"));

        // One press, and the first track sounds — asserted on the tail both
        // gestures spend, so neither can grow a confirmation of its own.
        let start = source.find("fn start(&mut self").expect("start exists");
        let rest = &source[start..];
        let tail = &rest[..rest.find("\n    }\n").expect("a function ends")];
        assert!(
            tail.contains("self.send_run(") && tail.contains("Command::Play"),
            "one press, and the first track sounds"
        );
    }

    /// A live source link and a durable history marker are related but not
    /// identical promises. Artist and file-backed lists own their run;
    /// library-wide All songs keeps ordinary record recency.
    #[test]
    fn only_lists_with_specific_attribution_mark_the_ledger_run() {
        let queue = |origin, source| vm::QueueVm {
            album: None,
            artist: String::new(),
            items: Vec::new(),
            origin,
            source,
        };
        let artist = crate::origin::Origin::Artist {
            id: 17,
            name: "Broadcast".to_owned(),
        };
        assert_eq!(
            run_origin(&queue(Some(artist.clone()), vm::RunSource::Fixed)),
            Some(artist.encode())
        );
        assert_eq!(
            run_origin(&queue(
                Some(crate::origin::Origin::AllSongs),
                vm::RunSource::Fixed
            )),
            None
        );
        assert_eq!(
            run_origin(&queue(
                None,
                vm::RunSource::Playlist("Road Trip".to_owned())
            )),
            Some(crate::origin::Origin::playlist("Road Trip").encode()),
            "older restored playlist queues retain their attribution"
        );
    }

    /// The fader's standing position crosses both halves of a restart: config
    /// is sent back to the engine at launch, and only the engine's confirmed
    /// answer is written for next time.
    #[test]
    fn volume_is_restored_and_persisted_from_confirmation() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("the shell source")
        .replace("\r\n", "\n");
        let code = source
            .split("#[cfg(test)]")
            .next()
            .expect("production code");
        assert!(code.contains("|config| config.volume"));
        assert!(code.contains("position: saved_volume.position()"));
        assert!(code.contains("matches!(&event, Event::VolumeChanged { .. })"));
        assert!(code.contains("self.persist_volume()"));

        let start = code
            .find("fn persist_volume(&mut self)")
            .expect("the persistence seam exists");
        let body = &code[start..];
        let body = &body[..body.find("\n    }\n").expect("the function ends")];
        assert!(body.contains("self.player.volume()"));
        assert!(body.contains("config.volume = volume"));
        assert!(
            body.contains("volume_gesture_active()"),
            "a drag must not write config once per pixel"
        );
        assert!(
            body.contains("volume_wheel_settles.is_some()"),
            "a touchpad stroke must settle before writing its confirmed volume"
        );

        let wheel = code
            .find("Message::VolumeWheel(steps) =>")
            .expect("the fader wheel route exists");
        let wheel = &code[wheel..];
        let wheel = &wheel[..wheel.find("\n            }").expect("the arm ends")];
        assert!(wheel.contains("self.player.step_volume(steps)"));
        assert!(wheel.contains("VOLUME_WHEEL_SETTLE"));
        assert!(
            !wheel.contains("toggle_mute") && !wheel.contains("set_muted"),
            "wheel travel prepares the fader while muted; it never unmutes"
        );
    }

    /// **Step 7 — shift-click queues the record, and nothing sounds
    /// unasked** (doc 09 §13; ADR-0023 §3's stack).
    ///
    /// The accelerator resolves through the one append shape the picker's
    /// Queue row spends (`append_to_run` — `UpdateQueue`, never a play
    /// gesture), and the press arm consults the hand-kept modifier state
    /// because iced 0.13 reports a `button`'s press without it. The plain
    /// press still enters the shared selection machine.
    #[test]
    fn shift_click_queues_the_record_and_nothing_sounds_unasked() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");

        let start = source
            .find("fn queue_album(&mut self")
            .expect("queue_album exists");
        let rest = &source[start..];
        let queue_album = &rest[..rest.find("\n    }\n").expect("a function ends")];
        assert!(
            queue_album.contains("vm::album_queue"),
            "the record whole — the selected edition, ADR-0014's group"
        );
        assert!(
            queue_album.contains("self.append_to_run(addition)"),
            "the picker Queue row's exact append — one shape for every route"
        );
        for forbidden in ["Command::SetQueue", "Command::Play", "note_transport_sent"] {
            assert!(
                !queue_album.contains(forbidden),
                "shift-click reached for `{forbidden}` — an append is not a \
                 play gesture, and nothing sounds unasked (ADR-0023 §3)"
            );
        }

        // The content-press arm: shift queues before the selection clock is
        // touched. Plain presses proceed to select/activate.
        let arm_start = source
            .find("fn press_content(&mut self, content: Content)")
            .expect("the tile press arm exists");
        let rest = &source[arm_start..];
        let arm = &rest[..rest.find("\n    }\n").expect("the press function ends")];
        assert!(arm.contains("self.modifiers.shift()"));
        assert!(arm.contains("self.queue_album(id)"));
        assert!(arm.contains("state.selection.press(content"));
        assert!(arm.contains("state.search_selection.press(content"));
    }

    /// **An undo restores the list, and nothing ever sounds because of it**
    /// (doc 11 §5 P2's exact scope). The queue's undo path is pinned the
    /// way shift-click's is: it goes out as `UpdateQueue` — ADR-0014's
    /// no-sample-disturbed edit — and reaches for no transport verb, no
    /// `SetQueue`, no `JumpTo`: the *list* comes back, never the playback
    /// position.
    #[test]
    fn an_undo_restores_the_list_and_never_sounds() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let start = source
            .find("fn undo_queue_edit(&mut self")
            .expect("undo_queue_edit exists");
        let rest = &source[start..];
        let undo = &rest[..rest.find("\n    }\n").expect("a function ends")];
        assert!(
            undo.contains("Command::UpdateQueue"),
            "the restored run goes out as the whole-list edit"
        );
        assert!(
            undo.contains("self.queue_undo.pop()"),
            "undo spends the bounded history and nothing else"
        );
        for forbidden in [
            "Command::SetQueue",
            "Command::Play",
            "Command::JumpTo",
            "note_transport_sent",
        ] {
            assert!(
                !undo.contains(forbidden),
                "undo reached for `{forbidden}` — a queue undo restores the \
                 list, not the playback position, and nothing sounds because \
                 of an undo (doc 11 §5 P2)"
            );
        }

        // The history's three ends (P2: the next edit replaces, a navigation
        // clears, the run ending clears): the clears are wired where the
        // navigation and the run's end actually happen.
        assert!(
            source.contains("fn note_place_left"),
            "leaving a surface clears its history"
        );
        let ended = source
            .find("Event::QueueEnded => {")
            .expect("the run's end is handled");
        assert!(
            source[ended..ended + 600].contains("self.queue_undo.clear()"),
            "the run ending clears the run's edit history"
        );
    }

    /// **Escape clears the query, and on the wall that is now the whole of
    /// it.**
    ///
    /// The peel was a triple: the pull's offer, then the query, then the
    /// shuffle pool's marks. Both of the owner's decisions on 2026-08-10 took
    /// a layer off — the pull was removed, and shuffle became a property of the
    /// player, which left no pool on the wall to un-mark. The query keeps its
    /// place and its behaviour: it is the one press that clears and blurs,
    /// which is type-anywhere's doing.
    ///
    /// Pinned as an **order in the source** of the one arm that spends it, for
    /// [`Self::shuffle_starts_what_it_draws_and_queues_whole_records`]'s reason:
    /// the peel is a pair of early returns in a `match` arm and there is no `Shelf`
    /// to build without a database and a scan thread. Each step is named by the
    /// literal a reviewer would have to move to break it.
    #[test]
    fn escape_clears_the_query_and_stops_there() {
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
        // It was a triple, and both of 2026-08-10's decisions took a layer
        // off it: the pull's offer peeled first until the pull was removed,
        // and the shuffle pool's marks peeled last until shuffle stopped being
        // a draw from the wall. What is left on the wall to peel is the query.
        let peel = ["self.clear_query()"];
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
            !clear.contains("iced::widget::operation::focus(search_id())"),
            "Escape re-focuses the well it just emptied"
        );
    }

    /// **The context menu's state machine, pinned in the source of the arms
    /// that spend it** (doc 09 §5.2) — for
    /// [`Self::shuffle_starts_what_it_draws_and_queues_whole_records`]'s
    /// reason: there is no `Shelf` to build without a database and a scan
    /// thread, and the items themselves are `menu::items`' — a pure
    /// function, swept exhaustively in `menu.rs`. What must hold *here* is
    /// the shell's contract:
    ///
    /// - **One menu at a time is structure, not policy**: the whole overlay
    ///   state is a single `Option` field, so opening another replaces the
    ///   first by assignment and there is nothing else that *could* hold a
    ///   second card.
    /// - **<kbd>Esc</kbd> peels the menu first** — it floats over the
    ///   panel, so it is the outermost layer, and one press takes exactly
    ///   one layer (the peel's standing rule).
    /// - **An item press closes and then fires**: the menu is `take`n
    ///   before a single press is dispatched, each press re-enters the
    ///   ordinary update loop (`self.update` — the mirror rule's mechanical
    ///   half: a menu press and a control press are one code path), and the
    ///   picker summoned mid-gesture by a completed composite does not
    ///   outlive it.
    /// - **An empty answer opens nothing**: a target none of whose verbs
    ///   can act offers no card of disabled words.
    #[test]
    fn the_menu_opens_once_peels_first_and_an_item_press_closes_then_fires() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        // One Option field is the whole overlay state. (The needle is
        // assembled at runtime so this test's own source is not a match.)
        let field = ["menu: Option<menu::", "Menu>,"].concat();
        assert_eq!(
            source.matches(&field).count(),
            1,
            "the overlay state is one `Option` field — a second holder would \
             let two menus stand"
        );
        // Esc: the menu peels before every panel layer.
        let escape = source
            .split_once("fn escape(&mut self)")
            .expect("the shell's Escape")
            .1;
        let escape = &escape[..escape.find("\n    }\n").expect("a function ends")];
        let menu_peel = escape
            .find("self.menu.take()")
            .expect("Escape peels the menu");
        let panel_peel = escape
            .find("self.playlists.peel()")
            .expect("Escape peels the panel");
        assert!(
            menu_peel < panel_peel,
            "the menu floats over the panel, so it must peel first"
        );
        // The item arm: take, then dispatch through the one update loop,
        // then put the gesture's own scaffolding away.
        let arm = source
            .split_once("Message::MenuItemPressed(index) => {")
            .expect("the item arm exists")
            .1;
        let arm = &arm[..arm.find("\n            }\n").expect("an arm ends")];
        let took = arm.find("self.menu.take()").expect("the press closes");
        let fired = arm
            .find("self.update(press)")
            .expect("the press fires through the ordinary update loop");
        assert!(took < fired, "closed before a single press is dispatched");
        assert!(
            arm.contains("self.playlists.close_panel()"),
            "a completed composite's picker does not outlive the gesture"
        );
        // The open arm refuses an empty card.
        let open = source
            .split_once("Message::OpenMenu(target, at) => {")
            .expect("the open arm exists")
            .1;
        assert!(
            open[..open.find("\n            }\n").expect("an arm ends")]
                .contains("!listed.is_empty()"),
            "a target with nothing to offer opens nothing"
        );
    }

    /// **<kbd>Enter</kbd> retargets to the top song while a query stands**
    /// (doc 09 §5, S1; ADR-0023 §2's amendment) — and it does so through the
    /// record page's own needle-drop path, never a new one.
    ///
    /// Pinned as an order in the source of the one arm that spends it, for
    /// [`Self::shuffle_starts_what_it_draws_and_queues_whole_records`]'s
    /// reason: there is no `Shelf` to build without a database and a scan
    /// thread, and the decision itself — which song is top, which row it is
    /// on its record — is [`vm::song_hits`]/[`vm::song_row`]'s, tested as
    /// pure functions in `vm`. What must hold *here* is the wiring:
    ///
    /// - `play_first_match` asks for the song **before** the album, and
    ///   spends it as `play_track` — `SetQueue` (selected edition, whole) +
    ///   `JumpTo` by [`PlayerState::play_from`]'s decision — before
    ///   `play_album` is even considered;
    /// - `enter_drops_needle` answers only while a query stands, from the
    ///   same ranked rows the section renders (`songs.first()`), resolved by
    ///   the same [`vm::song_row`] a click resolves through;
    /// - the section's rows are rebuilt with the filter (`refilter` calls
    ///   [`vm::song_hits`]), so <kbd>Enter</kbd>, the section and the wall
    ///   answer one query.
    #[test]
    fn enter_retargets_to_the_top_song_while_a_query_stands() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let body = |name: &str| {
            let start = source
                .find(&format!("fn {name}(&"))
                .unwrap_or_else(|| panic!("{name} exists"));
            let rest = &source[start..];
            let end = rest.find("\n    }\n").expect("a function ends");
            rest[..end].to_owned()
        };

        // The song outranks the album, and it sounds through play_track.
        let enter = body("play_first_match");
        let song = enter
            .find("enter_drops_needle")
            .expect("Enter asks for the top song");
        let album = enter
            .find("enter_plays")
            .expect("the album-level answer is still the fall-through");
        assert!(song < album, "the song is asked for before the album");
        let track = enter.find("play_track").expect("the song is a needle-drop");
        let whole = enter
            .find("play_album")
            .expect("the fall-through still plays a record");
        assert!(track < whole, "play_track before play_album");

        // The choice is the section's own first row, only while a query
        // stands, resolved by the one row-resolution a click also uses.
        let choice = body("enter_drops_needle");
        assert!(
            choice.contains("self.query.trim().is_empty()"),
            "no query, no song — Enter with a blank query stays the \
             selection's press"
        );
        assert!(
            choice.contains("self.songs.first()"),
            "Enter plays the row the section shows first, not a second query"
        );
        assert!(
            choice.contains("vm::song_row"),
            "the row is resolved exactly as a click on it is"
        );

        // And the rows Enter reads are rebuilt with the filter, from the one
        // ranked search the wall also answers.
        let filter = body("refilter");
        assert!(
            filter.contains("vm::song_hits"),
            "the songs section and the wall answer one query"
        );
    }

    /// **The one press works on a cold index**, which is the only index a
    /// first run has.
    ///
    /// `Message::VibeCreate` required the analysis store to *already exist*
    /// before it would read the library — survivable while a second button
    /// (`Analyse locally & create`) created it, and a press that silently did
    /// nothing the moment item 50 folded the consent gate into this one. It
    /// was caught by rendering the flow rather than by any test, which is why
    /// the regression is pinned here at the source: the arm may branch on a
    /// *missing data directory*, and never on a missing file that its own
    /// `prepare` creates.
    #[test]
    fn a_cold_index_still_composes_on_the_one_press() {
        let source = include_str!("app.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("a head");
        let arm = source
            .split_once("Message::VibeCreate => {")
            .expect("the compose arm")
            .1;
        let arm = &arm[..arm.find("Message::VibeAnalyze").expect("the next arm")];
        // Comments stripped: the note beside the fix names the call it
        // removed, and a rule that could not be written down would be a rule
        // nobody could explain.
        let drawn: String = arm
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !drawn.contains("path.exists()"),
            "a first Compose is doing nothing again: the store does not exist \
             until `prepare` makes it"
        );
        assert!(
            drawn.contains("state.vibe.start_preparing()")
                && drawn.contains("crate::vibe::prepare"),
            "the compose arm no longer reads the library on a cold index"
        );
    }

    /// A play gesture made on a search answer completes the search at command
    /// acceptance, but shows Now Playing only after the engine confirms that
    /// the requested run began. Search is app-wide, so neither half has a
    /// Library-place guard.
    #[test]
    fn playing_a_search_answer_clears_then_confirmation_opens_now_playing() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let transition = source
            .split_once("fn complete_search_launch(&mut self)")
            .expect("the search completion exists")
            .1;
        let transition = &transition[..transition.find("\n    }\n").expect("the transition ends")];
        assert!(
            !transition.contains("self.place != Place::Library"),
            "an app-wide search launch is still restricted to Library"
        );
        assert!(
            transition.contains("state.clear_query()"),
            "starting a search answer clears and blurs the query"
        );
        assert!(
            !transition.contains("Destination::NowPlaying"),
            "command acceptance pretends playback has started"
        );

        for arm in [
            "Message::PlayAlbum(id) => {",
            "Message::PlayTrack(id, row) => {",
        ] {
            let routed = source.split_once(arm).expect("the play arm exists").1;
            let routed = &routed[..routed.find("\n            }").expect("the arm ends")];
            assert!(
                routed.contains("self.complete_search_launch()"),
                "{arm} bypasses search completion"
            );
        }
        let enter = source
            .split_once("fn play_first_match(&mut self)")
            .expect("Enter's play route exists")
            .1;
        let enter = &enter[..enter.find("\n    }\n").expect("Enter's route ends")];
        assert_eq!(
            enter.matches("self.complete_search_launch()").count(),
            2,
            "both Enter outcomes complete the search"
        );

        let confirmed = source
            .split_once("fn apply_player_event(&mut self, message: PlayerEvent)")
            .expect("the engine event fold exists")
            .1;
        let confirmed = &confirmed[..confirmed.find("\n    }\n").expect("the event fold ends")];
        assert!(
            confirmed.contains("Event::TrackStarted")
                && confirmed.contains("self.show_on_start")
                && confirmed.contains("paths.contains(path)")
                && confirmed.contains("Destination::NowPlaying"),
            "only a matching TrackStarted spends the pending destination"
        );
        assert!(
            confirmed.contains("Event::QueueEnded")
                && confirmed.contains("PlayerEvent::Closed")
                && confirmed.matches("self.show_on_start = None;").count() >= 3,
            "success, an exhausted run and a dead engine all settle the pending destination"
        );
    }

    /// The two place keys, spelled out: Ctrl+`U` is the same press as the
    /// lane's `Now playing` row, and Ctrl+`,` the same press as the top bar's
    /// `Settings` word.
    ///
    /// Ctrl+`U` used to be that row **plus the place's `Run` word**, which is
    /// the construction ADR-0023's amendment blesses for an accelerator that
    /// sends two messages. The word is gone (the owner, 2026-08-10) and so is
    /// the second message: the chord is now literally the message two visible
    /// controls send, which is the simpler legality.
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
            format!("{:?}", Some(Message::ShowNowPlaying))
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

        // …and `Ctrl+B` is the returns lane's, again. Doc 07 §5.3 unbound it
        // because *"its subject was a sidebar that no longer exists"*;
        // ADR-0030 built the subject again and the key came back **with its
        // old meaning unchanged**, which is the only condition on which a
        // retired reflex may be revived.
        let from_key = keys::binding_for(
            &Key::Character("b".into()),
            Modifiers::COMMAND,
            keys::Focus::Elsewhere,
        );
        assert_eq!(
            format!("{from_key:?}"),
            format!("{:?}", Some(Message::ToggleLane))
        );
    }

    /// Every road to the resident query preserves the place underneath it.
    /// `/`, Ctrl+F and type-anywhere reveal/focus the app-bar control; none
    /// navigates to Library or changes the returns lane.
    #[test]
    fn every_road_to_the_query_preserves_the_current_place() {
        let source = include_str!("app.rs").replace("\r\n", "\n");
        let body = |signature: &str| {
            let rest = source
                .split_once(signature)
                .unwrap_or_else(|| panic!("`{signature}` exists"))
                .1;
            rest[..rest.find("\n    }\n").expect("a function ends")].to_owned()
        };

        let focus = body("fn focus_the_well(&mut self) -> Task<Message> {");
        assert!(
            focus.contains("iced::widget::operation::focus(search_id())")
                && !focus.contains("self.go(")
                && !focus.contains("set_lane"),
            "`/` and Ctrl+F must focus search without navigating or re-hanging"
        );

        let typed = body("fn type_anywhere(&mut self, text: &str) -> Task<Message> {");
        assert!(
            typed.contains("state.type_into_query(text)")
                && !typed.contains("self.go(")
                && !typed.contains("set_lane"),
            "type-anywhere must reveal results over the current place"
        );

        // **And the shelf no longer answers the message at all** — the place
        // and the lane are the shell's state, so the shelf cannot do the half
        // the owner's move added.
        // (Spelled in two halves so this assertion is not its own needle.)
        let old_arm = format!(
            "Message::QueryTyped(text) => self.{}(&text)",
            "type_into_query"
        );
        assert!(
            !source.contains(&old_arm),
            "the shelf still answers type-anywhere on its own"
        );
    }

    #[test]
    fn search_waits_for_a_choice_and_adds_to_the_playlist_on_screen() {
        let source = include_str!("app.rs").replace("\r\n", "\n");
        let body = |signature: &str| {
            let rest = source
                .split_once(signature)
                .unwrap_or_else(|| panic!("`{signature}` exists"))
                .1;
            rest[..rest.find("\n    }\n").expect("a function ends")].to_owned()
        };

        let refilter = body("fn refilter(&mut self) {");
        assert!(
            !refilter.contains("search_result_content(0)")
                && !refilter.contains("search_selection.select"),
            "typing a query implicitly selects its first result again"
        );

        let enqueue =
            body("fn enqueue_search_track(&mut self, album: u64, row: usize) -> Task<Message> {");
        assert!(
            enqueue.contains("if let Place::Playlist(id) = self.place")
                && enqueue.contains("self.playlists.append(id, entries, &state.library)")
                && enqueue.contains("self.append_items_to_run(vec![item])"),
            "search no longer distinguishes the playlist file on screen from the live run"
        );

        let chooser = include_str!("views/search.rs");
        assert!(
            chooser.contains("↑↓ select · ←→ action · Enter confirm")
                && chooser.contains("\"Add to playlist\"")
                && chooser.contains("\"Enqueue\""),
            "the chooser stopped teaching its keys or naming the action's destination"
        );
    }

    /// **The `×` is <kbd>Esc</kbd>'s pointer route, and it is the same
    /// function** — ADR-0036 §4, the owner's *"maybe a little x or esc to clear
    /// would make sense too"*.
    ///
    /// He named both roads in one sentence, which is the requirement stated:
    /// they must not merely agree, they must be one act. So both arms call
    /// [`Shelf::clear_query`] — the query goes, the caret leaves the field and
    /// the transport gets the keyboard back — and neither has a body of its
    /// own to drift in.
    ///
    /// The other half of the rule is *when*: the mark is drawn exactly while a
    /// query stands, which is exactly the condition under which the key has
    /// that layer to peel. A cross over an empty field would be a control that
    /// does nothing, and a key that clears with no query is the same defect
    /// from the other side.
    #[test]
    fn the_wells_clear_mark_and_escape_are_one_act() {
        let source = include_str!("app.rs").replace("\r\n", "\n");
        assert!(
            source.contains("Message::ClearSearch => self.clear_query(),"),
            "the well's `×` no longer resolves to the query's one clear"
        );
        assert!(
            source.contains("Message::DismissSearch => match &mut self.screen"),
            "the app-wide Escape route no longer reaches the shelf clear"
        );
        let rest = source
            .split_once("fn peel(&mut self) -> Task<Message> {")
            .expect("the shelf's Escape peel")
            .1;
        let peel = &rest[..rest.find("\n    }\n").expect("a function ends")];
        assert!(
            peel.contains("self.clear_query()"),
            "Escape's query layer and the `×` have stopped being one function"
        );
        assert!(
            peel.contains("!self.query.is_empty()"),
            "Escape clears a query that is not there, so the `×` it mirrors \
             would be a control with nothing to act on"
        );
        // And the one resident well draws the mark under that same predicate.
        let well = include_str!("views/search.rs");
        assert!(
            well.contains("let mark: Element<'_, Message> = if filtering {")
                && well.contains("clear_mark(room.recess)"),
            "the app-bar well draws its clear mark on something other than a \
             live query, or not at all"
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
        // And the well is the only `iced::widget::Id` the tree hands out, so
        // there is nothing else the sentinel could collide with.
        assert_eq!(format!("{:?}", search_id()), format!("{:?}", search_id()));
    }

    /// **The zoom is a ladder of state and nothing else** — the shell's half
    /// of ADR-0017 step 6, exercised as the update loop actually spends it.
    ///
    /// The shelf's half (the hang's arithmetic) is `shelf::Density`'s and is
    /// tested there; what is pinned here is that the message steps the step,
    /// saturates rather than wrapping, and is produced by both halves of the
    /// gesture.
    ///
    /// The ladder is walked by `Density::ALL`'s length rather than by a
    /// written-out count, so the owner's fourth step (2026-08-10) cost this
    /// test no number — which is the property `ALL`'s doc promises.
    #[test]
    fn the_zoom_steps_the_wall_and_stops_at_both_ends() {
        use iced::keyboard::{Key, Modifiers};

        let step = |density: shelf::Density, delta: i32| density.step(delta);
        let rungs = i32::try_from(shelf::Density::ALL.len()).expect("a small ladder");
        let mut density = shelf::Density::Balanced;
        for _ in 0..rungs {
            density = step(density, -1);
        }
        assert_eq!(density, shelf::Density::Dense);
        density = step(density, -1);
        assert_eq!(density, shelf::Density::Dense, "the ladder has an end");
        for _ in 0..rungs {
            density = step(density, 1);
        }
        assert_eq!(density, shelf::Density::Spacious);
        density = step(density, 1);
        assert_eq!(density, shelf::Density::Spacious);

        // Both halves of the gesture produce the same message — and the
        // density marks send the same message with the mirror delta
        // (`shelf::Density::steps_to`, `views::shelf`'s mirror test) — which
        // is what makes keys, wheel and marks one control rather than three.
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
        assert!(Place::default().is_library());
        // Anywhere else it is the place's, and one press is enough: there is no
        // second layer to take off underneath.
        for place in [Place::Album(7), Place::NowPlaying, Place::Settings] {
            assert!(!place.is_library(), "{place:?} answers the press itself");
            assert!(
                place.back().is_library(),
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

    /// **A tile press selects, a double activates, and neither re-hangs.**
    ///
    /// The defect this replaces was caught on camera by the composition audit:
    /// a double-click on the fifth tile of row 0, where the first press opened
    /// the rail, the shelf reflowed from five columns to three, the second
    /// press landed 180 px from where the tile now was, and **nothing played**
    /// — while the panel that had just opened said "double-click a tile to
    /// play" at the bottom of it. `shelf::GridHold` was the fix: pin the width
    /// in force for the length of the gesture.
    ///
    /// ADR-0022 deleted the reflow cause. Its 2026-08-12 amendment restores
    /// double-click as one content grammar over a wall whose width is now a
    /// function of the window alone. What is pinned here is that the first
    /// press only selects, the second activates the record, and neither state
    /// enters the grid arithmetic.
    #[test]
    fn a_tile_press_selects_and_activation_re_hangs_nothing() {
        let start = Instant::now();
        let mut selection = crate::selection::State::default();
        let album = Content::Album(7);
        assert_eq!(selection.press(album, start), Press::Selected);
        assert_eq!(selection.selected(), Some(album));
        assert_eq!(
            selection.press(album, start + crate::selection::DOUBLE_CLICK),
            Press::Activated
        );

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
        // The hero's dissolve, at its resting value: **1.0**, which is one
        // picture at full strength. The other three rest at 0; this one rests
        // at the end of its own flight, because "no transition" here means the
        // incoming picture is all there is.
        let mut dissolve = Tween::settled(1.0).with_curve(motion::Curve::Linear);
        // The exact disjunction the two `moving` functions form between them.
        macro_rules! moving {
            () => {
                ink.live() || warmth.live() || tile.live() || dissolve.live()
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

        // **The hero's dissolve** (ADR-0020's third amendment). The record
        // changed, so the picture crosses — and the surface is static again the
        // instant it lands, which is the half of this feature the owner's
        // responsiveness rule is about.
        dissolve.set(0.0);
        dissolve.go(1.0, motion::DISSOLVE, start);
        assert!(moving!(), "the hero crossing to another record");
        dissolve.tick(start + motion::DISSOLVE);
        assert!(!moving!());
        assert!(
            (dissolve.value() - 1.0).abs() < f32::EPSILON,
            "a settled dissolve is the new picture, whole"
        );

        // All four at once, and the clock stops with the *last* of them rather
        // than the first: the lamp and the dissolve are one number and run
        // longest, so they are what keep the timer alive after the two 90 ms
        // fades have settled.
        ink.enter(Control::Next, motion::INK, start);
        tile.enter(9, motion::TILE, start);
        warmth.go(0.0, motion::LAMP, start);
        dissolve.set(0.0);
        dissolve.go(1.0, motion::DISSOLVE, start);
        for at in [motion::INK, motion::TILE] {
            ink.tick(start + at);
            tile.tick(start + at);
            warmth.tick(start + at);
            dissolve.tick(start + at);
            assert!(moving!(), "settled at {at:?} with the lamp still warming");
        }
        warmth.tick(start + motion::LAMP);
        // The light and the picture land on the same tick — one event, one
        // number (`motion::the_dissolve_is_the_lamps_own_number`).
        assert!(
            moving!(),
            "the dissolve outlived the lamp it shares a clock with"
        );
        dissolve.tick(start + motion::DISSOLVE);
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
            dissolve.tick(start + later);
            assert!(!moving!());
        }
    }

    /// A 1 × 1 decode standing in for a cover: a distinct [`Hero`] every call,
    /// which is what makes the handle comparisons below mean anything.
    fn a_hero() -> Hero {
        Hero {
            handle: iced_image::Handle::from_rgba(1, 1, vec![0_u8; 4]),
            back: None,
            px: 1.0,
            field: None,
        }
    }

    /// **The dissolve's predicate is the picture, and it is a refusal three
    /// ways out of four** (ADR-0020's third amendment; [`Change::between`]).
    ///
    /// The case that matters most is the third: consecutive tracks on one
    /// record share a cover, and fading a picture into an identical picture is
    /// a flight, a clock and 25 wakes announcing a change nothing made.
    #[test]
    fn a_dissolve_needs_two_pictures_that_are_not_the_same_picture() {
        let (first, second) = (a_hero(), a_hero());
        assert_eq!(
            Change::between(Some(&first), Some(&second)),
            Change::Dissolve,
            "two decoded covers that differ"
        );
        // The same picture, arrived at twice — the surface redrawing what it
        // already had. A clone shares the handle, so this is an identity test
        // and not a coincidence of contents.
        assert_eq!(
            Change::between(Some(&first), Some(&first.clone())),
            Change::Cut,
            "a picture that has not changed may not start a flight"
        );
        // A stand-in is not artwork: the wall's deterministic gradient is what
        // a record with no cover draws, and dissolving one is decoration.
        assert_eq!(Change::between(None, Some(&second)), Change::Cut);
        assert_eq!(Change::between(Some(&first), None), Change::Cut);
        // The first record of a session has nothing behind it.
        assert_eq!(Change::between(None, None), Change::Cut);
    }

    /// **The transition needs both records decoded at once, and the two-entry
    /// hero LRU already holds them** — checked rather than trusted, because the
    /// entry that makes it true was written for a *prefetch* this product does
    /// not have yet ([`art::HERO_CACHE_ENTRIES`]'s own note).
    ///
    /// The discipline being reproduced is the one the shell really runs:
    /// [`Shelf::request_hero`] `get`s the **sounding** record on every message,
    /// which keeps it the freshest entry, and `HeroLoaded` `put`s the decode
    /// when it lands. Under that discipline the entry a `put` evicts is always
    /// the record *before last*, so the record that just stopped is still
    /// decoded for exactly as long as the dissolve needs it — and
    /// [`Shelf::art_prior`] is an `Arc` onto those same pixels rather than a
    /// copy of them.
    #[test]
    fn the_hero_lru_holds_both_records_a_dissolve_needs() {
        let mut heroes: LruCache<u64, Hero> = LruCache::new(
            NonZeroUsize::new(art::HERO_CACHE_ENTRIES).expect("the hero tier has entries"),
        );
        assert_eq!(art::HERO_CACHE_ENTRIES, 2, "the whole of the claim");

        // Four records in a row, which is more than the cache holds — so if
        // the second slot were spent on anything but the last record, one of
        // these rounds would find it gone.
        let mut previous: Option<u64> = None;
        for id in 1..=4 {
            // `request_hero`: ask for the sounding record, miss, decode.
            assert!(heroes.get(&id).is_none(), "record {id} was not decoded yet");
            // `HeroLoaded`: the decode lands.
            heroes.put(id, a_hero());
            // …and the dissolve asks for the record that just stopped.
            if let Some(was) = previous {
                assert!(
                    heroes.peek(&was).is_some(),
                    "record {was} was evicted before {id}'s dissolve could use it"
                );
                // Both alive at once is the whole requirement.
                assert!(heroes.peek(&id).is_some());
                assert_eq!(heroes.len(), 2);
            }
            previous = Some(id);
        }
        // And the one *before* that is gone, which is the budget holding: two
        // entries, 8 MiB, and the crossfade adds no third.
        assert!(heroes.peek(&2).is_none());
    }

    /// The run baz launched with, as [`App::restore_the_run`] hands it back to
    /// the engine: three files, queued and silent.
    fn restored() -> vm::QueueVm {
        let item = |title: &str, path: &str| vm::QueueItemVm {
            title: title.to_owned(),
            artist: None,
            album: Some("Anhydrous".to_owned()),
            album_artist: None,
            duration: Some(Duration::from_secs(387)),
            path: PathBuf::from(path),
        };
        vm::QueueVm {
            album: Some("Anhydrous".to_owned()),
            artist: "Bola".to_owned(),
            items: vec![
                item("Anhydrous 1", "/m/1.flac"),
                item("Anhydrous 2", "/m/2.flac"),
                item("Anhydrous 3", "/m/3.flac"),
            ],
            origin: Some(crate::origin::Origin::playlist("Road Trip")),
            source: vm::RunSource::Playlist("Road Trip".to_owned()),
        }
    }

    /// **Opening baz and closing it again keeps the listener's place.**
    ///
    /// The bug this guards is the one the `CONTINUE` band exists to serve and
    /// the one that would silently destroy it: restoring the run moves every
    /// mark the shell watches, so a write at that moment records cursor 0 and
    /// position 0 and the interrupted point is gone. It is checked on **both**
    /// writers, because the narrower *is a row playing* reading of this guard
    /// protected [`App::sync_snapshot`] and left [`App::leave_for_good`] —
    /// which writes unconditionally, on the way out — spending the position
    /// anyway.
    #[test]
    fn opening_baz_and_closing_it_again_keeps_the_listeners_place() {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(restored());
        assert!(!player.has_sounded(), "the queue is loaded and silent");
        assert_eq!(
            next_snapshot(&player, 0),
            None,
            "the run moving at launch may not write: this is the restore, not \
             a move, and the file already says where the listener was"
        );
        assert_eq!(
            next_snapshot(&player, 192_000),
            None,
            "and neither may the way out — `leave_for_good` writes the elapsed \
             position, and nothing has elapsed"
        );
    }

    /// **A library that is not mounted yet costs no one their place.**
    ///
    /// A snapshot whose files do not resolve produces no queue at all, and the
    /// old *no queue ⇒ write an empty snapshot* arm then deleted the run
    /// outright. A NAS that was not up when baz opened is an ordinary thing to
    /// meet (ADR-0025 says so by name) and it must not be a way to lose where
    /// you were.
    #[test]
    fn a_library_that_is_not_mounted_costs_no_one_their_place() {
        let player = PlayerState::new(Availability::Ready);
        assert_eq!(next_snapshot(&player, 0), None);
        assert_eq!(next_snapshot(&player, 192_000), None);
    }

    /// **Once something has sounded, the file is the engine's account.**
    ///
    /// The run is written where the engine says it is — by row, at the
    /// position handed in — and the provenance travels with it.
    #[test]
    fn the_run_is_written_where_the_engine_says_it_is() {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(restored());
        player.apply(
            &Event::TrackStarted {
                path: PathBuf::from("/m/2.flac"),
                position: 1,
            },
            &[],
        );
        let written = next_snapshot(&player, 192_000).expect("the engine said where it is");
        assert_eq!(written.cursor, 1);
        assert_eq!(written.position_ms, 192_000);
        assert_eq!(written.provenance.as_deref(), Some("Road Trip"));
        assert_eq!(written.current(), Some(Path::new("/m/2.flac")));
    }

    /// **A run played to its end is written away.**
    ///
    /// The same judgement `views::home::standing` makes on screen, made once
    /// more on disk so the two cannot disagree across a restart: a finished run
    /// is not an interrupted one, and the `CONTINUE` band must not come back
    /// after a relaunch offering to replay something the listener completed.
    ///
    /// Note what makes this state distinguishable at all — the phase, the
    /// queue and the playing row are *identical* to the launch state above.
    /// Only [`PlayerState::has_sounded`] separates them.
    #[test]
    fn a_run_played_to_its_end_is_written_away() {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(restored());
        player.apply(
            &Event::TrackStarted {
                path: PathBuf::from("/m/3.flac"),
                position: 2,
            },
            &[],
        );
        player.apply(&Event::QueueEnded, &[]);
        assert_eq!(player.playing_queue_row(), None);
        assert!(
            player.queued() > 0,
            "the engine keeps the list it was given"
        );
        assert_eq!(
            next_snapshot(&player, 0),
            Some(crate::session::Snapshot::default()),
            "the run is over and the file says so"
        );
    }

    /// **A queue merely replaced leaves the file alone until the engine
    /// speaks.**
    ///
    /// `SetQueue` clears the row this side of the bridge while the phase is
    /// still whatever it was and the engine's next `TrackStarted` is already on
    /// its way. Blanking the file for that millisecond and rewriting it
    /// immediately would be two writes and one window in which a crash costs
    /// the run — and it is *not* the run ending, which is the state above.
    #[test]
    fn a_queue_replaced_leaves_the_file_alone_until_the_engine_speaks() {
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(restored());
        player.apply(
            &Event::TrackStarted {
                path: PathBuf::from("/m/1.flac"),
                position: 0,
            },
            &[],
        );
        player.note_queue_sent(restored());
        assert_eq!(player.playing_queue_row(), None);
        assert_eq!(player.phase(), player::Phase::Playing);
        assert_eq!(next_snapshot(&player, 0), None);
    }

    /// **A place change clears the hovered tile**, with the open menu and the
    /// drag it already cleared.
    ///
    /// `TileLeft` is published by a `mouse_area` the pointer actually leaves,
    /// so navigating *out from under* the pointer — a keyboard door, or the
    /// tile's own press — never delivers one. The mark survived, and while the
    /// wall was the only surface drawing tiles that was invisible: coming back
    /// put the pointer where it had left it. Home's `RECENTLY ADDED` row and
    /// the Artist place both draw `views::shelf::tile`, so the stale mark
    /// became a record's hover options offered unbidden on another place, for
    /// a record the pointer is nowhere near.
    #[test]
    fn navigating_leaves_no_tile_under_a_pointer_that_moved_on() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source");
        let body = source
            .split_once("fn go(&mut self, door: impl FnOnce(Place) -> Place)")
            .expect("the one place transition")
            .1;
        let body = &body[..body.find("\n    }\n").expect("a function ends")];
        for cleared in [
            "self.menu = None;",
            "self.drag = None;",
            "hovered_album = None;",
            "hovered_all_songs = false;",
        ] {
            assert!(
                body.contains(cleared),
                "a place change must not outlive `{cleared}` — the four are \
                 one rule: what was about the place you left does not follow you"
            );
        }
        // …and the tile's own press goes through `go`'s rule rather than
        // around it, which is what makes the clearing total.
        let opened = source
            .split_once("fn open_album(&mut self, id: u64) -> Task<Message> {")
            .expect("the tile's press")
            .1;
        let opened = &opened[..opened.find("\n    }\n").expect("a function ends")];
        assert!(
            opened.contains("self.menu = None;") && opened.contains("self.drag = None;"),
            "open_album keeps `go`'s rule by hand; if it stops, it must call `go`"
        );
    }

    /// **The breadcrumb's door and the page it opens agree on who the artist
    /// is**, and a page whose artist has gone answers with the wall.
    ///
    /// Two files have to hold one identity here — `views/album.rs` builds the
    /// door and `views/artist.rs` decides which records belong behind it — and
    /// if either reached for the artist's *label* instead of
    /// [`vm::artist_id`], the door would open a page that is empty for exactly
    /// the records the marker bytes exist to keep apart (a nameless
    /// compilation, and a band called "Various Artists"). Nothing else in the
    /// product can catch that, because both halves would still compile and the
    /// common case would still work.
    #[test]
    fn the_breadcrumb_and_the_artist_page_are_one_identity() {
        let read = |name: &str| {
            std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("src/views")
                    .join(name),
            )
            .unwrap_or_else(|_| panic!("{name} is still a view"))
        };
        let door = read("album.rs");
        assert!(
            door.contains("vm::artist_id(&album.artist)")
                && door.contains("Message::OpenArtist(artist)"),
            "the breadcrumb's door names the artist by id, not by label"
        );
        let page = read("artist.rs");
        assert!(
            page.contains("vm::artist_id(&album.artist) == artist"),
            "the artist page picks its records by the same id the door sends"
        );
        // The label is a *reading* on that page and never a key: an artist
        // resolved by name would merge the two states above.
        assert!(
            !page.contains("artist.label() == ") && !page.contains("label() =="),
            "the artist page compares labels somewhere — ids are the identity"
        );

        // …and a page whose artist vanished under a rescan answers with the
        // wall, drawn rather than navigated to, exactly as a vanished record's
        // page does (a view function may not change state).
        let arm = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source");
        let arm = arm
            .split_once("(Screen::Shelf(state), Place::Artist(id)) => {")
            .expect("the Artist place is routed")
            .1;
        let arm = &arm[..arm.find("\n            }\n").expect("an arm ends")];
        assert!(
            arm.contains("views::artist::label(state, id).is_some()")
                && arm.contains("state.view("),
            "an artist the library no longer holds must fall back to the wall"
        );
    }

    /// **`Resume` navigates an already-held run immediately; a deliberate
    /// album start navigates only when the engine confirms it.**
    ///
    /// The owner asked for it by name (*"or takes you to now playing"*) and it
    /// is a deliberate exception to the confirmation boundary. A fresh album
    /// `Play`, however, must not land on an empty Now Playing page when every
    /// file is refused or the engine is dead. The request path therefore only
    /// arms a destination; the event fold owns the actual place change.
    #[test]
    fn deliberate_play_navigates_at_the_right_truth_boundary() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source");
        let body_of = |name: &str| {
            let body = source
                .split_once(&format!("fn {name}("))
                .unwrap_or_else(|| panic!("{name} is still a function here"))
                .1;
            body[..body.find("\n    }\n").expect("a function ends")].to_owned()
        };
        let door = "Destination::NowPlaying";
        assert!(
            body_of("resume_the_run").matches(door).count() == 2,
            "`Resume` starts the run *and* goes to `Now playing`, on both of \
             the two shapes it has — the paused session and the interrupted run"
        );
        let album = body_of("play_album");
        assert!(
            album.contains("self.start_and_show(queue)") && !album.contains("self.go("),
            "album Play must use the shared confirmed-start route"
        );
        let requested = body_of("start_and_show");
        assert!(
            requested.contains("self.send_run(queue, None)")
                && requested.contains("Command::Play")
                && requested.contains("self.show_on_start = Some(paths)")
                && !requested.contains("self.go("),
            "an accepted command arms the destination but does not navigate"
        );
        let confirmed = body_of("apply_player_event");
        assert!(
            confirmed.contains("Event::TrackStarted")
                && confirmed.contains("paths.contains(path)")
                && confirmed.contains(door),
            "the matching engine confirmation owns fresh-start navigation"
        );
        // Named rather than discovered, and `body_of` panics on a name that
        // has moved — a sweep that quietly matched nothing would pass forever.
        for elsewhere in [
            "play_track",
            "play_playlist",
            "play_playlist_track",
            "play_first_match",
        ] {
            assert!(
                !body_of(elsewhere).contains("self.go("),
                "`{elsewhere}` navigates around the deliberate start boundary"
            );
        }
    }
}
