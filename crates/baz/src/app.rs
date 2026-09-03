//! The iced application: first-run setup screen and the album shelf.
//!
//! Architecture (v0.1, ADR-0005):
//!
//! - **UI thread** owns the [`baz_core::index::Library`] (SQLite + in-RAM
//!   search index) and all view state — held by [`crate::collection::Shelf`],
//!   which left this file on 2026-09-02. Search runs synchronously per
//!   keystroke — sub-ms over 100k tracks per the Phase 1 spike and
//!   `baz-core`'s benches.
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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use baz_core::history::{History, HistoryLedger};
use baz_core::index::{GroupKey, IndexError};
use baz_core::protocol::{self as protocol, Command, Event, SignalChain};
use baz_core::replaygain::ReplayGainSettings;
use baz_core::traversal::Traversal;
use baz_core::volume::Volume;
use iced::keyboard;
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{column, image as iced_image, row, scrollable};
use iced::{Element, Point, Size, Subscription, Task, window};

use crate::collection::{Hero, Shelf};
use crate::motion::{Control, Ink, Keyed, Tween};
use crate::mpris::Mpris;
use crate::place::{History as PlaceHistory, Place};
use crate::playback::{OutputChoice, Playback, PlayerEvent};
use crate::player::{PlayerState, SignalPath, SignalWarningState};
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
pub(crate) const WINDOW: Size = Size::new(1280.0, 860.0);

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
/// **An empty layer**, so a stack level never has to appear and disappear.
///
/// iced diffs the widget tree by position: a level that comes and goes hands
/// every widget beneath it a fresh state on the frame it changes. A layer
/// that is always there and sometimes empty costs a zero-sized `Space` that
/// captures nothing, and costs no widget below it its scroll offset, its
/// hover, or a gesture in flight.
fn nothing() -> Element<'static, Message> {
    iced::widget::Space::new().width(0.0).height(0.0).into()
}

fn nothing_id() -> iced::widget::Id {
    iced::widget::Id::new("baz-nothing")
}

/// Take the caret out of the search well (see [`nothing_id`]).
pub(crate) fn blur_search<T: Send + 'static>() -> Task<T> {
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
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
            Some(Message::PointerPressed)
        }
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
            Some(Message::PointerReleased)
        }
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
        iced::Event::Window(window::Event::FileDropped(path)) => Some(Message::FileDropped(path)),
        iced::Event::Window(window::Event::FileHovered(_)) => Some(Message::FileHovered),
        iced::Event::Window(window::Event::FilesHoveredLeft) => Some(Message::FileHoverLeft),
        iced::Event::Window(window::Event::Focused) => Some(Message::WindowFocused(true)),
        iced::Event::Window(window::Event::Unfocused) => Some(Message::WindowFocused(false)),
        _ => None,
    }
}

fn search_event_message(
    event: iced::Event,
    status: iced::event::Status,
    window: window::Id,
) -> Option<Message> {
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = &event {
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

/// **While a menu stands, the keyboard is the menu's.**
///
/// The same shape as `search_event_message` above and for the same
/// reason: a modal thing on screen owns the keys its shape implies,
/// and it owns them *before* the capture rule, because nothing under
/// the backdrop should answer while it is up. Up and Down walk the
/// verbs, Enter presses the lit one, and Escape closes — which is the
/// grammar every desktop menu has had for forty years, so it is the
/// one a listener will try first.
fn menu_event_message(
    event: iced::Event,
    status: iced::event::Status,
    window: window::Id,
) -> Option<Message> {
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = &event
        && modifiers.is_empty()
    {
        match key {
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                return Some(Message::MenuMoved(1));
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                return Some(Message::MenuMoved(-1));
            }
            keyboard::Key::Named(keyboard::key::Named::Enter) => {
                return Some(Message::MenuActivated);
            }
            keyboard::Key::Named(keyboard::key::Named::Escape) => {
                return Some(Message::CloseMenu);
            }
            _ => {}
        }
    }
    event_message(event, status, window)
}

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

/// **A sleep timer in flight**: when it fires, and what it was set for.
#[derive(Debug, Clone, Copy)]
struct Sleep {
    minutes: u32,
    fires_at: Instant,
}

/// The durations the Settings row offers, and the words for them. Five
/// choices and an off, which is what every player in the field offers and
/// what a listener can pick from without reading.
pub(crate) struct SleepChoice {
    pub(crate) label: &'static str,
    pub(crate) minutes: Option<u32>,
}

/// **What the crossfade offers** (ADR-0044 §6): an off and five lengths.
///
/// A small set rather than a free field, on the sleep timer's argument — six
/// numbers a listener picks from without reading beat a text box that can hold
/// `0.5`. The lengths climb the way a listener thinks about a fade rather than
/// linearly: two seconds is a boundary softened, twelve is two records playing
/// together on purpose.
pub(crate) const CROSSFADE_CHOICES: [CrossfadeChoice; 6] = [
    CrossfadeChoice {
        label: "Off",
        ms: 0,
    },
    CrossfadeChoice {
        label: "2 s",
        ms: 2_000,
    },
    CrossfadeChoice {
        label: "4 s",
        ms: 4_000,
    },
    CrossfadeChoice {
        label: "6 s",
        ms: 6_000,
    },
    CrossfadeChoice {
        label: "8 s",
        ms: 8_000,
    },
    CrossfadeChoice {
        label: "12 s",
        ms: crate::config::MAX_CROSSFADE_MS,
    },
];

/// One offered crossfade length, and the word for it.
pub(crate) struct CrossfadeChoice {
    pub(crate) label: &'static str,
    /// The overlap in milliseconds; zero is off.
    pub(crate) ms: u32,
}

/// What the sleep timer offers.
pub(crate) const SLEEP_CHOICES: [SleepChoice; 6] = [
    SleepChoice {
        label: "Off",
        minutes: None,
    },
    SleepChoice {
        label: "15 min",
        minutes: Some(15),
    },
    SleepChoice {
        label: "30 min",
        minutes: Some(30),
    },
    SleepChoice {
        label: "45 min",
        minutes: Some(45),
    },
    SleepChoice {
        label: "1 hour",
        minutes: Some(60),
    },
    SleepChoice {
        label: "2 hours",
        minutes: Some(120),
    },
];

/// **The equaliser as the shell holds it** — the config's three fields, kept
/// together because they travel to the engine as one command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct EqualizerSettings {
    /// Whether it is in the path at all.
    pub(crate) enabled: bool,
    /// Ten band gains in centidecibels, low to high.
    pub(crate) bands_centidb: [i16; 10],
    /// The stated attenuation, in centidecibels.
    pub(crate) preamp_centidb: i16,
    /// Whether the pre-amp follows the curve rather than the listener.
    pub(crate) auto_gain: bool,
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
    /// **Esc while the caret is in the search well** — the one binding that
    /// survives the focus rule, because the field's own handling of this key
    /// is *half* of what the listener asked for (see [`App::escape_in_field`]).
    EscapeInField,
    /// **A missing playlist entry, pointed at a file that is there**
    /// (ADR-0024 §3): the display row, and the candidate the listener
    /// confirmed. The only message in the product that rewrites an entry's
    /// path, and it exists only as a press on a candidate the `Locate…` card
    /// was showing.
    PlaylistRepairEntry(usize, std::path::PathBuf),
    /// **Open or close the shortcuts card** (`?`). One message for both,
    /// because the key is the only way in and the only way out besides `Esc`
    /// and a press outside — a control that toggles is honest about that.
    ToggleShortcuts,
    /// **The equaliser's four gestures.** Each is a whole new settings value
    /// sent to the engine as one `SetEqualizer`, because the curve and its
    /// pre-amp are one decision (`baz_core::protocol`).
    EqualizerEnabled(bool),
    /// **A band set to a stated value**, which is what a fader sends: a drag
    /// reports where the handle *is*, not how far it moved.
    EqualizerBandSet(usize, i16),
    /// The pre-amp set to a stated value.
    EqualizerPreampSet(i16),
    /// **The gesture ended.** A drag publishes a value every frame and the
    /// engine takes each one — the sound follows the hand — but the config is
    /// written once, here, rather than a hundred times down a fader.
    EqualizerCommitted,
    /// Open or close the equaliser panel (the app bar's fader mark).
    ToggleEqualizer,
    /// The equaliser panel: one of `baz_core::equalizer::PRESETS` was chosen.
    ///
    /// This replaced a `Flat` button. Flat is now the first offered preset, so
    /// there is one control for *choose a curve* rather than one button for
    /// the only curve baz was willing to name — see
    /// `baz_core::equalizer::PRESETS`.
    EqualizerPresetChosen(crate::views::equalizer::Choice),
    /// **Save the current curve**: open the name field.
    EqualizerSaveStart,
    /// The name field's text changed.
    EqualizerSaveName(String),
    /// Commit the name — the curve joins the picker and the config.
    EqualizerSaveCommit,
    /// Put the name field away without saving.
    EqualizerSaveCancel,
    /// Forget the saved curve currently being edited.
    EqualizerForget,
    /// Put the edited curve back to what its name means.
    EqualizerReset,
    /// Write the faders over the saved curve being edited, under its own name.
    EqualizerSaveOver,
    /// **Auto gain**: whether the pre-amp follows the curve.
    ///
    /// Two labels ago this was a button called `Suggest a pre-amp`, then one
    /// called `Make room`. Both were the wrong *shape* as well as the wrong
    /// words: pressing something once to fix headroom you are about to change
    /// again by dragging a band is a chore, not a control. The owner:
    /// *"make it 'auto gain' as a checkbox"* — which is what it always
    /// wanted to be, a standing mode rather than an act.
    EqualizerAutoGain(bool),
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
    /// Show or hide the words that narrow the request.
    VibeWords(bool),
    /// The debounce clock: ask whether the words have been still long enough
    /// to be worth a text embedding.
    VibeCountTick,
    /// A settled phrase came back from the local text tower, beside the phrase
    /// it was asked about so a stale answer can be discarded.
    VibeEmbedded(String, Result<Vec<f32>, String>),
    /// Select a row of the result so it explains itself, or put the
    /// explanation away.
    VibePreviewSelected(usize),
    /// **The contour's own gestures** — the shape a generated list is asked
    /// to follow (`crate::contour`). The drag carries the raw geometry the
    /// pointer described and `crate::vibe` decides what a line may be; the
    /// release exists so a recomposition costs one gesture rather than one
    /// pixel.
    ContourDragged(usize, usize, f32, f32),
    ContourReleased,
    /// **Set or clear the sleep timer**, in whole minutes. `None` is *off*.
    SleepTimerSet(Option<u32>),
    /// **Set the crossfade between records**, in milliseconds; zero is off
    /// (ADR-0044 §6).
    CrossfadeSet(u32),
    /// **Hang the collection as a wall or as a list** ([`shelf::Layout`]).
    LayoutSet(shelf::Layout),
    /// One second of the sleep timer's own clock, which exists only while it
    /// is armed.
    SleepTimerTick,
    /// **The pointer entered one row of the composed preview**, so the contour
    /// can light that track's own place on the line. The owner: *"when we
    /// hover the playlist items it is showing where on the curve it's meant to
    /// be… so a person can see it really worked."*
    VibePreviewEntered(usize),
    /// **…and left it**, guarded by the row it names — see
    /// [`Self::DraftRowLeft`] for why the two cross.
    VibePreviewLeft(usize),
    /// **The pointer entered one row of the manual draft**, so the row's card
    /// can reach its editing controls. The same toolkit limit
    /// [`Self::QueueRowEntered`] works around, on the one draft list that had
    /// no hover answer of its own.
    DraftRowEntered(usize),
    /// **…and left it.** Separate from the enter, and guarded by the row it
    /// names, because the two cross: moving from row 3 to row 4 delivers
    /// row 4's enter *before* row 3's exit, so an exit that cleared the state
    /// unconditionally would unlight the row the pointer is actually on.
    DraftRowLeft(usize),
    /// **Choose a picture for the playlist `id`** — the platform's own file
    /// dialog, off the event loop. The owner: *"lets allow setting an
    /// image/removing the image for a playlist."*
    PlaylistImageChoose(u64),
    /// The chooser closed: a file, or `None` for a dismissal, which changes
    /// nothing.
    PlaylistImagePicked(u64, Option<PathBuf>),
    /// **Take the authored picture off the playlist `id`** — to the trash, and
    /// the collage comes back.
    PlaylistImageRemove(u64),
    /// One authored sleeve finished decoding: the handle, or `None` where the
    /// file could not be read as an image and the list keeps its collage.
    PlaylistImageLoaded(u64, Option<iced_image::Handle>),
    /// **The pointer entered one row of Favourites**, for the same reason as
    /// [`Self::DraftRowEntered`].
    FavouriteRowEntered(usize),
    /// **…and left it**, guarded exactly as [`Self::DraftRowLeft`] is.
    FavouriteRowLeft(usize),
    /// **Start from one of the offered moods** — its words, its shape and its
    /// length, all of which stay editable. The owner: *"as part of the wizard
    /// we should be asking users if they want to make a preset one."*
    VibeRecipe(usize),
    /// **How many points the line carries** — two for a straight line, up to
    /// six. Replaces the deleted `−`/`+` stepper with a control that says
    /// where in the range you are.
    ContourPoints(usize),
    /// **Open or close the per-dimension lines** — design 21 §5's labelled
    /// expander. Closing puts every line back on the first one's curve, which
    /// is why it says *back to one line* rather than *close*.
    /// **Show one of the five lines, or all of them.** A view, not a request.
    VibeLine(Option<usize>),
    /// Edit the in-memory preview without touching music or playlist files.
    VibePreviewRemove(usize),
    VibePreviewShift(usize, i32),
    /// Put the edited preview on as the run. This is Vibe's explicit playback act.
    VibePlay,
    /// Write the previewed, ordinary playlist file and open it without playing.
    VibeSubmit,
    /// Open the canonical playlist-creation place at its chooser.
    NewPlaylistOpen,
    /// Leave the door with an empty request, to write your own words.
    VibeStartBlank,
    /// **The smart playlist's own door.** Lands on the moods rather than on
    /// the form, and on a library Baz has never heard it lands on the one
    /// step that has to come first.
    NewSmartPlaylistOpen,
    PlaylistCreationName(String),
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
    /// **Shuffle set to a value**, from MPRIS's `Shuffle` property. The
    /// toggle above is the control's spelling; this is the protocol's, and
    /// they resolve to one function ([`App::set_shuffle`]).
    SetShuffle(bool),
    /// **Repeat set to a value**, from MPRIS's `LoopStatus` property.
    SetRepeat(baz_core::protocol::Repeat),
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
    /// **The pointer has come near the chromeless frame, or left it.**
    ///
    /// One message on crossing rather than one per mouse move: the reveal band
    /// is a `mouse_area`, so this fires twice per approach and never while the
    /// hand is still.
    ChromeApproached(bool),
    /// **Measure ReplayGain for the library**, or for all of it again.
    ///
    /// The engine has been able to do this since ADR-0015 and nothing could
    /// ask it to: the pass was reachable from `AnalysisCommand` and from
    /// nowhere a listener could press.
    MeasureLoudness {
        /// Re-measure files baz has already measured. `false` — the ordinary
        /// case — measures only what has no figure yet, and never a file whose
        /// own tags carry one.
        redo: bool,
    },
    /// Stop the running pass. What it has measured is kept, and starting again
    /// carries on from there.
    CancelLoudness,
    /// Read the pass's progress and empty its channel.
    LoudnessTick,
    /// **Ask whether there is a newer baz.** Sent by the button in Settings,
    /// by somebody who is therefore waiting for an answer.
    CheckForUpdate,
    /// The check answered — an update, or nothing, or why not.
    UpdateChecked(Result<Option<baz_update::Update>, String>),
    /// The listener turned the background check on or off.
    CheckOnStartToggled(bool),
    /// **Download it, prove it, and stage it** — or, where nothing can be
    /// staged, hand it straight to the thing that opens it.
    InstallUpdate,
    /// The download answered.
    UpdateFetched(Result<std::path::PathBuf, String>),
    /// **The quiet one.** What the background pass left staged for
    /// `baz-boot`, or nothing, or why not — and nothing on this path is ever
    /// drawn unasked (ADR-0043 §5).
    UpdateStaged(Result<Option<String>, String>),
    /// **Play everything selected**, as one run, in the order it is drawn.
    MarkedPlay,
    /// Append everything selected to what is playing.
    MarkedQueue,
    /// Put everything selected in front of the playlist picker.
    MarkedAddToPlaylist,
    /// Take everything selected out of the list it is in — the queue, or a
    /// playlist's page. Never offered anywhere else (`views::marks`).
    MarkedRemove,
    /// Put the selection down without spending it.
    MarksClear,
    /// **Move the keyboard's ring to the next focus stop**, in the order the
    /// place builds its controls — see [`crate::focus`].
    FocusNext,
    /// The same, backwards: <kbd>Shift</kbd>+<kbd>Tab</kbd>.
    FocusPrevious,
    /// **An arrow, while the ring is on the collection** — see
    /// [`crate::grid`]. Published by the wall's own focus stop rather than by
    /// the binding table, which is what keeps the arrows' global meanings
    /// (volume, seek) untouched everywhere the keyboard has not been put.
    WallStep(crate::search::Direction),
    /// **The keyboard has arrived at the collection.** The wall draws no ring
    /// of its own, so this is what makes its arrival visible: it lights the
    /// first record when nothing is lit, and the lit record is the one the
    /// arrows move from.
    WallReached,
    /// **Chromeless**: take the frame away from around Now playing.
    ///
    /// Distinct from [`Self::ToggleFullscreen`], and composes with it: that
    /// one asks the *window manager* for the whole screen and leaves baz's
    /// own bars where they are; this one is about what baz draws inside
    /// whatever window it has. Both at once is the reading the owner's
    /// *"really shows off the now playing view"* is after.
    ToggleChromeless,
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
    /// **Move the open menu's cursor**, +1 or −1, wrapping — see
    /// [`crate::menu::Menu::moved`].
    MenuMoved(isize),
    /// **Press the item the keyboard is on**, or do nothing when it is on
    /// none: a menu that opens with nothing lit must not answer a stray
    /// Enter with its first verb.
    MenuActivated,
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
pub(crate) struct App {
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
    /// Which row of the built-in **Favourites** place the pointer is on — the
    /// same mechanism again, so the row's card reaches its heart.
    hovered_favourite_row: Option<usize>,
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
    /// Whether the shortcuts card is up. Session state and deliberately not
    /// persisted: a card you asked for once is not a preference.
    shortcuts_open: bool,
    /// The equaliser, as the listener has it. The engine holds its own copy;
    /// this is what the controls draw and what is written to the config.
    equalizer: EqualizerSettings,
    /// Whether the equaliser panel is up. Session state: a panel you opened
    /// once is not a preference.
    equalizer_open: bool,
    /// **Curves the listener saved**, loaded from the config and written back
    /// whenever one is added or removed.
    equalizer_presets: Vec<crate::config::SavedCurve>,
    /// **Which saved curve is on the faders**, if one was chosen.
    ///
    /// A *selection*, not a match. Comparing the ten bands against the saved
    /// list answers *are these numbers one of my curves*, which is a different
    /// question from *am I editing one of my curves* — and the second is the
    /// one the panel needs, because the moment a listener nudges a band the
    /// first says no and the relationship the panel should be offering to save
    /// or reset is gone.
    equalizer_editing: Option<usize>,
    /// The name being typed for a curve about to be saved, if one is.
    ///
    /// `None` is the ordinary state. It is a field on the app rather than on
    /// `EqualizerSettings` because that one is `Copy` and travels to the
    /// engine; a half-typed name has no business crossing that boundary.
    equalizer_naming: Option<String>,
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
    /// **Whether the frame is away from around Now playing.**
    ///
    /// Deliberately **not persisted**. Every other view preference here
    /// survives a restart; this one would reopen baz into a window with no
    /// app bar, no lane and — on the platforms where baz owns its chrome — no
    /// close button, from a press the listener made days ago. A mode you take
    /// the frame off for is a mode you enter on purpose each time.
    chromeless: bool,
    /// **The loudness measurement service**, spawned on first use.
    ///
    /// Lazily, because it opens a second connection to the library and holds a
    /// thread: a listener who never asks baz to measure anything should not
    /// pay for the machinery that would. The same rule every clock in this
    /// file follows — a cost with no reader is not paid.
    loudness: Option<baz_core::analysis::AnalysisHandle>,
    /// The pass's own event stream, drained rather than read.
    ///
    /// Kept because the worker sends on it and an `mpsc` grows without bound:
    /// one event per track over a large library is thousands of retained
    /// paths. Nothing here needs them — [`baz_core::analysis::AnalysisProgress`]
    /// carries every number the readout states, and the pass counts its
    /// failures rather than itemising them (a wall of red for a handful of
    /// corrupt files helps nobody) — so the tick empties the channel and reads
    /// the snapshot.
    loudness_events: Option<std::sync::mpsc::Receiver<baz_core::protocol::Event>>,
    /// What the running, or last, measurement pass has done.
    loudness_progress: baz_core::analysis::AnalysisProgress,
    /// **Where the update is up to**, and the whole of the updater's state.
    updating: views::settings::Updating,
    /// The update a check found, kept so the button that downloads it does
    /// not have to ask again.
    update_found: Option<baz_update::Update>,
    /// **How lit the chromeless frame's controls are**, 0 gone to 1 whole.
    ///
    /// Settled at zero because chromeless is entered from a press on the bar
    /// itself: the pointer is already up there, `chrome_near` is about to say
    /// so, and starting lit would flash the bar on before fading it out.
    /// Outside chromeless nothing reads it — the bar is simply always whole.
    chrome_veil: motion::Tween,
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
    /// **When the sleep timer will pause the music**, and how long it was set
    /// for. Session state and deliberately not persisted: a timer that
    /// survived a restart would pause a listener who never set one.
    sleep: Option<Sleep>,
    /// **The crossfade between records, in milliseconds**; zero is off
    /// (ADR-0044 §6). Mirrors `config.toml` so the control can be drawn lit
    /// on the first frame rather than after a round trip.
    crossfade_ms: u32,
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
    /// **What shape the collection is hung in** ([`shelf::Layout`]), mirroring
    /// `config.toml` so a new shelf opens in the shape the listener left.
    layout: shelf::Layout,
    /// **Whether a drag from the file manager is over the window.**
    ///
    /// The one piece of drag state baz keeps: what it is *for* is the sentence
    /// in the strip saying where the drop will land, and a drop's destination
    /// is the place, so nothing about the path being dragged is needed.
    drop_hover: bool,
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
/// Everything [`baz_core::index::Library::open`] can fail with folds into
/// these. The fold is
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
    pub(crate) fn of(error: &IndexError) -> Self {
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

/// **Which of a queue's seams a crossfade may cross** (ADR-0044 §2), read off
/// the wall.
///
/// A free function rather than a method because every caller is already
/// borrowing `self.playback` to send the command it is building.
///
/// Without a shelf there is no wall to ask, and every seam is `false` — the
/// direction that protects a record. That covers Setup and Blocked, where
/// nothing is playing anyway.
fn fade_seams_of(screen: &Screen, paths: &[std::path::PathBuf]) -> Vec<bool> {
    match screen {
        Screen::Shelf(state) => vm::fade_seams(&state.albums, paths),
        Screen::Setup(_) | Screen::Blocked(_) => vec![false; paths.len()],
    }
}

/// Everything the shell needs and cannot make: the outside world, gathered.
///
/// [`App::assemble`] derives the whole of `App` from one of these, so the
/// boundary between "what baz reads from the machine" and "what baz decides"
/// is a struct rather than a convention. [`Outside::real`] is what `main`
/// runs; a test builds one by hand.
struct Outside {
    stored: Option<config::Config>,
    output_choices: Vec<OutputChoice>,
    output_devices_error: Option<String>,
    playback: Playback,
    mpris: Mpris,
    history_ledger: Option<Arc<HistoryLedger>>,
    resume: crate::session::Snapshot,
}

impl Outside {
    /// The real one: read the config, open the device, take the bus, open the
    /// ledger, and pick up an interrupted session.
    fn real() -> Self {
        let stored = config::config_file().map(|path| config::load(&path));
        let configured_output = stored
            .as_ref()
            .and_then(|config| config.output_device.as_deref());
        let (output_choices, output_devices_error) =
            crate::playback::output_choices(configured_output);
        // Engine first: open failure must not kill the app — it becomes
        // Availability::NoDevice state that the bottom bar reports.
        let playback = Playback::start(configured_output);
        // Desktop integration is an enhancement: this spawns a thread and
        // returns, and an absent session bus costs one stdout line (see
        // crate::mpris).
        let mpris = Mpris::start();
        // **The play ledger, handed to the engine.** ADR-0018 built it and put
        // the whole of a front end's involvement in one call; nothing in this
        // crate made it, so nothing was being recorded and PLAYED had nothing
        // to sort by. This is that call.
        //
        // The engine is the only thing that knows what actually reached the
        // output and for how long, which is why the ledger is written there
        // and not here (`baz_core::history`). A ledger that cannot be opened
        // is carried on without: `set_history(None)` is the engine's own
        // default, so the failure costs the record of *this* session and
        // nothing else — no dialog, no degraded playback, and the file is
        // tried again next launch.
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
        Self {
            stored,
            output_choices,
            output_devices_error,
            playback,
            mpris,
            history_ledger,
            resume: read_snapshot(),
        }
    }

    /// An outside world that touches nothing, for tests.
    ///
    /// No device, no session bus, no ledger, and — the part that matters most
    /// — **no XDG path at all**. `stored` is handed over rather than read, so
    /// a test cannot pick up the config of whoever is running it, and cannot
    /// write over it either. `music_dirs` is left empty by the caller unless
    /// it means otherwise, which keeps `Shelf::open` and the library database
    /// out of the picture entirely.
    ///
    /// The returned recorder is where the shell's asks of the engine land.
    #[cfg(test)]
    fn none(stored: config::Config) -> (Self, Arc<std::sync::Mutex<Vec<crate::playback::Ask>>>) {
        let (playback, asks) = Playback::recording();
        (
            Self {
                stored: Some(stored),
                output_choices: Vec::new(),
                output_devices_error: None,
                playback,
                mpris: Mpris::silent(),
                history_ledger: None,
                resume: crate::session::Snapshot::default(),
            },
            asks,
        )
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
                // An imported room stands immediately, like a picked one
                // (item 54): a listener editing a room and pasting it wants to
                // *see* it, and asking them to restart to find out is what
                // made the schema hard to work against.
                self.theme_notice = Some(match crate::theme_file::resolve(&selection) {
                    Ok(room) => {
                        crate::theme::stand_in(room);
                        format!("Imported {} and standing in it now.", path.display())
                    }
                    Err(reason) => format!(
                        "Imported {} and selected it, but could not stand in it: {reason}.",
                        path.display()
                    ),
                });
            }
            Err(error) => self.theme_notice = Some(error),
        }
        Task::none()
    }

    fn new(started: Instant, cli_dir: Option<PathBuf>) -> (Self, Task<Message>) {
        Self::assemble(Outside::real(), started, cli_dir)
    }

    /// A real `App`, assembled against an outside world that touches nothing.
    ///
    /// This is the seam the audit asked for. Until it existed, nothing in the
    /// suite constructed an `App` at all, so the shell's behaviour was pinned
    /// by reading its own source for substrings — assertions that go on
    /// passing when the code they describe moves into a helper, and that can
    /// only ever say "the source mentions this", never "baz did this".
    ///
    /// What it costs to be safe: no audio device, no session bus (a developer
    /// machine *has* one, and a test that spawned the real handle would put
    /// `org.mpris.MediaPlayer2.baz` on it and take the desktop's media keys),
    /// no play ledger, and no XDG path — the config is handed in, not read,
    /// and with no `music_dirs` the library database is never opened. The
    /// returned recorder holds every ask the shell made of the engine.
    ///
    /// It lands in [`Screen::Setup`], which is what a first run is. A test
    /// needing a populated shelf needs more than this and does not have it
    /// yet; `docs/BACKLOG.md` #12 says so rather than leaving it to be
    /// discovered.
    #[cfg(test)]
    pub(crate) fn headless(
        stored: config::Config,
    ) -> (Self, Arc<std::sync::Mutex<Vec<crate::playback::Ask>>>) {
        let (outside, asks) = Outside::none(stored);
        let (app, _task) = Self::assemble(outside, Instant::now(), None);
        (app, asks)
    }

    /// Build the shell from an already-gathered outside world.
    ///
    /// Everything `App` cannot make for itself — the config file, the audio
    /// engine, the desktop bus, the play ledger, the interrupted session —
    /// arrives in [`Outside`], and everything below this line is derivation
    /// from it. The split exists so a test can hand over an outside world that
    /// touches nothing: no device, no bus, no ledger, and above all none of
    /// the XDG paths that belong to whoever is running the suite.
    ///
    /// It is deliberately the *same* function the product runs, not a
    /// simplified twin. A twin is how you end up asserting about a shell that
    /// nobody ships.
    #[expect(
        clippy::too_many_lines,
        reason = "a launch is one composition of independent restores — the \
                  library, the config's standing decisions, the run's snapshot \
                  — and each is three lines that only mean anything beside the \
                  others. It has crossed and re-crossed the limit as those \
                  decisions came and went; splitting it would name four \
                  functions after the order they happen to run. What *was* \
                  separable came out on 2026-08-24: everything it reads from \
                  the machine is `Outside` now, which is a boundary rather \
                  than a slice of the sequence"
    )]
    fn assemble(
        outside: Outside,
        started: Instant,
        cli_dir: Option<PathBuf>,
    ) -> (Self, Task<Message>) {
        let Outside {
            stored,
            output_choices,
            output_devices_error,
            playback,
            mpris,
            history_ledger,
            resume,
        } = outside;
        let configured_output = stored
            .as_ref()
            .and_then(|config| config.output_device.as_deref());
        let output_choice = OutputChoice::from_config(configured_output);
        let active_output_choice = output_choice.clone();
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
            playback.send(transport::command_for(saved_replay_gain));
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
        let group_key = stored
            .as_ref()
            .map_or(GroupKey::Artist, |config| config.group_key);
        let density = stored
            .as_ref()
            .map_or(shelf::Density::Balanced, |config| config.density);
        let layout = stored
            .as_ref()
            .map_or(shelf::Layout::Wall, |config| config.layout);
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
        // **The equaliser is sent only when it is on.** A fresh install and a
        // listener who has never touched it both reach the engine having sent
        // no equaliser command at all, which is one fewer thing between the
        // decoder and the sink on the path this product is judged by.
        let equalizer = stored
            .as_ref()
            .map_or_else(EqualizerSettings::default, |config| EqualizerSettings {
                enabled: config.equalizer_enabled,
                bands_centidb: config.equalizer_bands_centidb,
                preamp_centidb: config.equalizer_preamp_centidb,
                auto_gain: config.equalizer_auto_gain,
            });
        if equalizer.enabled {
            playback.send(Command::SetEqualizer {
                enabled: true,
                bands_centidb: equalizer.bands_centidb,
                preamp_centidb: equalizer.preamp_centidb,
            });
        }
        // **The crossfade is sent only when it is on**, for the equaliser's
        // reason exactly: a listener who has never touched it reaches the
        // engine having sent no crossfade command, and the bit-perfect claim
        // is untouched by a feature they are not using (ADR-0044 §5).
        let crossfade_ms = stored.as_ref().map_or(0, |config| config.crossfade_ms);
        if crossfade_ms > 0 {
            playback.send(Command::SetCrossfade { ms: crossfade_ms });
        }
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
            match Shelf::open(dirs.clone(), group_key, density, layout, lane_open) {
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
            sleep: None,
            crossfade_ms: stored.as_ref().map_or(0, |config| config.crossfade_ms),
            resource_sampled: None,
            theme_json: String::new(),
            theme_notice: None,
            density,
            layout,
            lane_open,
            lane: crate::lane::Lane::default(),
            lane_mark: (u64::MAX, u64::MAX),
            art_mark: ((u64::MAX, u64::MAX), Place::Settings, usize::MAX, false),
            was_scanning: true,
            resume: resume.clone(),
            written: (0, None, 0),
            drop_hover: false,
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
            hovered_favourite_row: None,
            drag: None,
            queue_undo: crate::undo::History::new(),
            menu: None,
            status_open: false,
            shortcuts_open: false,
            equalizer,
            equalizer_open: false,
            // **A curve loaded from the config is one you are editing**, if it
            // is one of yours.
            //
            // Without this, launching onto a saved curve and nudging a band
            // offered `Save` — a new name for a curve that already had one —
            // rather than `Save`/`Reset` over the one it plainly is. The
            // relationship has to survive the first nudge, so it cannot be
            // re-derived from the bands each frame: that is the reading that
            // stops being true the moment the listener changes anything, which
            // is precisely when it is needed.
            equalizer_editing: stored.as_ref().and_then(|config| {
                config
                    .equalizer_presets
                    .iter()
                    .position(|curve| curve.bands_centidb == config.equalizer_bands_centidb)
            }),
            equalizer_presets: stored
                .as_ref()
                .map(|config| config.equalizer_presets.clone())
                .unwrap_or_default(),
            equalizer_naming: None,
            playlists: crate::playlists::Playlists::start(),
            window: WINDOW,
            window_maximized: false,
            fullscreen: false,
            chromeless: false,
            chrome_veil: motion::Tween::settled(0.0),
            loudness: None,
            loudness_events: None,
            loudness_progress: baz_core::analysis::AnalysisProgress::default(),
            updating: views::settings::Updating::Idle,
            update_found: None,
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
        app.sync_playlist_corpus();
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
            Message::EqualizerEnabled(on) => {
                self.equalizer.enabled = on;
                self.send_equalizer();
                Task::none()
            }
            Message::EqualizerBandSet(index, centidb) => {
                if let Some(band) = self.equalizer.bands_centidb.get_mut(index) {
                    *band = centidb.clamp(-1200, 1200);
                }
                // **Auto gain follows the hand.** The headroom is recomputed
                // on the same frame as the band, so the pre-amp fader tracks
                // the drag rather than jumping when it ends — and the sound
                // never passes through a moment of being too loud on its way
                // to a curve that fits.
                self.apply_auto_gain();
                // **Sent, not written.** The engine hears every frame of the
                // drag so the sound follows the hand; the config waits for
                // the release.
                self.send_equalizer_only();
                Task::none()
            }
            Message::EqualizerPreampSet(centidb) => {
                // **Taking hold of the pre-amp is how you claim it.** With
                // auto gain on, the next band would overwrite whatever this
                // drag set — so rather than fight the listener, or let a
                // control move and mean nothing, the drag turns the mode off
                // and the checkbox unticks where they can see it.
                self.equalizer.auto_gain = false;
                self.equalizer.preamp_centidb = centidb.clamp(-1200, 1200);
                self.send_equalizer_only();
                Task::none()
            }
            Message::EqualizerCommitted => {
                self.persist_equalizer();
                Task::none()
            }
            Message::ToggleEqualizer => {
                self.equalizer_open = !self.equalizer_open;
                Task::none()
            }
            Message::EqualizerPresetChosen(crate::views::equalizer::Choice::Saved(index, _)) => {
                if let Some(curve) = self.equalizer_presets.get(index) {
                    self.equalizer_editing = Some(index);
                    // **A saved curve brings its own headroom**, unlike an
                    // offered one which derives it: this is what the listener
                    // had set, and someone who deliberately left themselves
                    // three decibels of room did not mean to save half of it.
                    self.equalizer.bands_centidb = curve.bands_centidb;
                    self.equalizer.preamp_centidb = curve.preamp_centidb;
                    self.send_equalizer();
                    self.persist_equalizer();
                }
                Task::none()
            }
            Message::EqualizerSaveStart => {
                self.equalizer_naming = Some(String::new());
                // **And the caret goes into it.** Without this the field opens
                // unfocused and baz's type-anywhere rule takes the keystrokes
                // to the app-bar search instead — a listener naming a curve
                // would watch their words appear at the top of the window and
                // filter their library.
                iced::widget::operation::focus(crate::views::equalizer::name_id())
            }
            Message::EqualizerSaveName(text) => {
                if self.equalizer_naming.is_some() {
                    self.equalizer_naming = Some(crate::config::clip_curve_name(&text));
                }
                Task::none()
            }
            Message::EqualizerSaveCancel => {
                self.equalizer_naming = None;
                blur_search()
            }
            Message::EqualizerSaveCommit => {
                let Some(name) = self.equalizer_naming.take() else {
                    return Task::none();
                };
                let name = crate::config::settle_curve_name(&name);
                // An empty name saves nothing and says nothing: the field
                // simply closes, which is what pressing Enter on an empty
                // field means everywhere else in this product.
                if name.is_empty() {
                    return Task::none();
                }
                let curve = crate::config::SavedCurve {
                    name,
                    bands_centidb: self.equalizer.bands_centidb,
                    preamp_centidb: self.equalizer.preamp_centidb,
                };
                // **Saving a name you already used replaces that curve**
                // rather than making a second entry with the same label. Two
                // rows reading `Kitchen` in one picker is a list that cannot
                // be used, and the listener's intent — *this is what Kitchen
                // means now* — is the ordinary reading.
                if let Some(at) = self
                    .equalizer_presets
                    .iter()
                    .position(|saved| saved.name == curve.name)
                {
                    self.equalizer_presets[at] = curve;
                    self.equalizer_editing = Some(at);
                } else if self.equalizer_presets.len() < crate::config::MAX_SAVED_CURVES {
                    self.equalizer_presets.push(curve);
                    self.equalizer_editing = Some(self.equalizer_presets.len() - 1);
                } else {
                    crate::baz_log!(
                        "[equaliser] {} saved curves is the limit; {:?} was not kept",
                        crate::config::MAX_SAVED_CURVES,
                        curve.name
                    );
                    return Task::none();
                }
                self.persist_equalizer_presets();
                Task::none()
            }
            Message::EqualizerForget => {
                if let Some(at) = self.equalizer_editing.take()
                    && at < self.equalizer_presets.len()
                {
                    self.equalizer_presets.remove(at);
                    self.persist_equalizer_presets();
                }
                Task::none()
            }
            Message::EqualizerReset => {
                // **Back to what the name means.** The bands *and* the
                // headroom, because a saved curve keeps its own pre-amp and
                // restoring half of it would leave the listener somewhere
                // they had never been.
                if let Some(curve) = self
                    .equalizer_editing
                    .and_then(|at| self.equalizer_presets.get(at))
                {
                    self.equalizer.bands_centidb = curve.bands_centidb;
                    self.equalizer.preamp_centidb = curve.preamp_centidb;
                    self.send_equalizer();
                    self.persist_equalizer();
                }
                Task::none()
            }
            Message::EqualizerSaveOver => {
                // Saving over the curve you are editing needs no name: it
                // already has one, and asking for it again would invite a
                // second entry under a spelling one character different.
                if let Some(curve) = self
                    .equalizer_editing
                    .and_then(|at| self.equalizer_presets.get_mut(at))
                {
                    curve.bands_centidb = self.equalizer.bands_centidb;
                    curve.preamp_centidb = self.equalizer.preamp_centidb;
                    self.persist_equalizer_presets();
                }
                Task::none()
            }
            Message::EqualizerPresetChosen(crate::views::equalizer::Choice::Builtin(index)) => {
                if let Some(preset) = baz_core::equalizer::PRESETS.get(index) {
                    // An offered curve is not one of the listener's, so
                    // whatever they were editing is no longer what is on the
                    // faders.
                    self.equalizer_editing = None;
                    self.equalizer.bands_centidb = preset.bands_centidb;
                    // **The headroom comes with the curve.** A preset that
                    // boosts is a preset that needs room, and leaving the
                    // pre-amp where the last one left it is how a listener
                    // gets a distorted first impression of a curve baz
                    // offered them. `Flat` suggests zero, so choosing it
                    // hands the whole signal back.
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "derived from bands already clamped to ±12 dB"
                    )]
                    {
                        self.equalizer.preamp_centidb =
                            (preset.bands().suggested_preamp() * 100.0).round() as i16;
                    }
                    self.send_equalizer();
                    self.persist_equalizer();
                }
                Task::none()
            }
            Message::EqualizerAutoGain(on) => {
                self.equalizer.auto_gain = on;
                // Turning it **on** takes effect at once — a checkbox that
                // needed a band nudged before it did anything would look
                // broken. Turning it off leaves the headroom where it stands:
                // that is the value the listener is now in charge of, and
                // snapping it to zero would undo work they never asked to
                // undo.
                self.apply_auto_gain();
                self.send_equalizer();
                self.persist_equalizer();
                Task::none()
            }
            Message::ToggleShortcuts => {
                self.shortcuts_open = !self.shortcuts_open;
                Task::none()
            }
            Message::EscapePressed => self.escape(),
            Message::EscapeInField => self.escape_in_field(),
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
            // **The plain door makes a plain playlist.** It opened the
            // *smart* page: `NewPlaylistOpen` passed no mode, and the routing
            // that had been written when there was one door drew the composing
            // page for `None`. Two doors, two things made, named at the door.
            Message::NewPlaylistOpen => {
                self.open_playlist_creation(Some(crate::playlists::CreationMode::Manual))
            }
            Message::VibeStartBlank => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.choosing = false;
                }
                Task::none()
            }
            // **Opening this door is the request.**
            //
            // The owner: *"what's the point of me having to make the system
            // listen to my music? surely it already knows/can know it needs
            // to?"* He is right about the press, and the distinction is where
            // the consent actually lives:
            //
            // - Listening **unasked, at first launch**, is a real consent
            //   question — hours of CPU and a laptop's battery spent on
            //   something nobody asked for. Design 21 §11 decision 1 answered
            //   *no*, and that answer still stands: nothing below runs until
            //   somebody opens this door.
            // - Listening **because you just opened the smart-playlist
            //   door** is not a second decision. You have asked for the one
            //   feature that cannot work without it, and baz already knows
            //   exactly which tracks it has not heard. Asking again is a toll
            //   on a choice already made.
            //
            // So arriving here starts it, visibly and stoppably. The door
            // shows what it is doing and offers to stop; nothing is hidden,
            // and nothing runs for anybody who never comes here.
            Message::NewSmartPlaylistOpen => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.begin_choosing();
                }
                let open = self.open_playlist_creation(Some(crate::playlists::CreationMode::Vibe));
                Task::batch([open, self.start_listening()])
            }
            Message::PlaylistCreationName(name) => {
                self.playlists.creation.name = name.chars().take(96).collect();
                self.playlists.creation.name_is_suggested = false;
                self.playlists.creation.error = None;
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
                // **The room changes now**, which is item 54 and the owner's
                // own words: *"ideally can we apply them upon selection."*
                // Everything that reads `theme::active()` per frame follows on
                // the next one; the two things that *bake* a colour — the
                // glyph sheets and the jewel case's textures — are keyed on
                // `theme::generation()` and so miss rather than serve a
                // picture painted in the room before.
                //
                // A room that cannot be resolved leaves the one standing and
                // says so, rather than dropping the listener into Closing Time
                // for a typo in a file they are editing.
                match crate::theme_file::resolve(selection) {
                    Ok(room) => {
                        crate::theme::stand_in(room);
                        self.theme_notice = Some(format!("{} is standing now.", room.name));
                    }
                    Err(reason) => {
                        self.theme_notice = Some(format!(
                            "Could not stand in that room: {reason}. \
                             The one you are in is unchanged."
                        ));
                    }
                }
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
                self.sync_playlist_corpus();
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
            // **The property spellings**, which only MPRIS sends: a desktop's
            // shuffle switch and its repeat menu state a value rather than
            // asking for the next one.
            Message::SetShuffle(on) => {
                self.set_shuffle(on);
                Task::none()
            }
            Message::SetRepeat(repeat) => {
                self.set_repeat(repeat);
                Task::none()
            }
            Message::SleepTimerSet(minutes) => {
                self.set_sleep_timer(minutes);
                Task::none()
            }
            Message::CrossfadeSet(ms) => {
                self.set_crossfade(ms);
                Task::none()
            }
            Message::LayoutSet(layout) => self.set_layout(layout),
            Message::SleepTimerTick => {
                self.tick_sleep_timer();
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
            // **The traversal is iced's own operation, over baz's own stops.**
            // The order is the widget tree's (see [`crate::focus`]), so
            // nothing here has to know what is on screen — which is the point:
            // a per-place focus index would be a second statement of the
            // layout, kept by hand, free to disagree with the first.
            Message::FocusNext => iced::advanced::widget::operate(
                iced::advanced::widget::operation::focusable::focus_next(),
            ),
            Message::FocusPrevious => iced::advanced::widget::operate(
                iced::advanced::widget::operation::focusable::focus_previous(),
            ),
            Message::WallStep(direction) => self.wall_step(direction),
            Message::WallReached => {
                if let Screen::Shelf(state) = &mut self.screen
                    && state.selection.selected().is_none()
                    && let Some(album) = state.albums.first()
                {
                    state.selection.select(Content::Album(album.id));
                }
                Task::none()
            }
            Message::MeasureLoudness { redo } => {
                self.measure_loudness(redo);
                Task::none()
            }
            Message::CancelLoudness => {
                if let Some(loudness) = &self.loudness {
                    // A cancel is a request the service answers by winding up
                    // the edition it is inside — the pass keeps what it has
                    // measured, so `running` stays true for a moment and the
                    // tick below is what notices it stop.
                    let _ = loudness
                        .send(baz_core::protocol::AnalysisCommand::CancelReplayGainAnalysis);
                }
                Task::none()
            }
            Message::LoudnessTick => {
                self.read_loudness();
                Task::none()
            }
            // **Both halves run on a worker**, because both block: one on a
            // request, one on a download that can be tens of megabytes. The
            // shell holds only the answer.
            Message::CheckForUpdate => {
                self.updating = views::settings::Updating::Checking;
                Task::perform(tokio::task::spawn_blocking(baz_update::check), |joined| {
                    Message::UpdateChecked(joined.unwrap_or_else(|error| Err(error.to_string())))
                })
            }
            Message::UpdateChecked(answer) => {
                self.updating = match answer {
                    Ok(Some(update)) => {
                        let found = views::settings::Updating::Found(update.version.clone());
                        self.update_found = Some(update);
                        found
                    }
                    Ok(None) => views::settings::Updating::UpToDate,
                    // Reported, because whoever sees this pressed a button and
                    // is waiting for an answer. The background pass below is
                    // the one that stays silent.
                    Err(why) => views::settings::Updating::Failed(why),
                };
                Task::none()
            }
            // **The background pass, and it draws nothing.**
            //
            // No network, no permission and no release are all the same
            // answer here: silence. A listener who never opens Settings finds
            // out about a new baz from `baz-boot` at the next launch, and a
            // failed check is not a thing that happened to their music.
            Message::UpdateStaged(answer) => {
                match answer {
                    Ok(Some(version)) => {
                        crate::baz_log!("[update] {version} staged for the next launch");
                        self.updating = views::settings::Updating::Staged(version);
                    }
                    Ok(None) => {}
                    Err(why) => crate::baz_log!("[update] nothing staged: {why}"),
                }
                Task::none()
            }
            Message::CheckOnStartToggled(on) => {
                persist(|config| config.check_for_updates = on);
                Task::none()
            }
            Message::InstallUpdate => {
                let Some(update) = self.update_found.clone() else {
                    return Task::none();
                };
                self.updating = views::settings::Updating::Fetching(update.version.clone());
                Task::perform(
                    tokio::task::spawn_blocking(move || baz_update::fetch_verified(&update)),
                    |joined| {
                        Message::UpdateFetched(
                            joined.unwrap_or_else(|error| Err(error.to_string())),
                        )
                    },
                )
            }
            // **What a finished download becomes, and it is not an install.**
            //
            // An installer cannot replace a file the running application holds
            // open, so baz never starts one — `baz-boot` does, at the next
            // launch, with nothing standing in the field (ADR-0043 §5). Here
            // the download simply becomes a staged file and a sentence saying
            // so.
            //
            // **Where nothing can be staged there is no launcher either**, and
            // a Linux archive is that case: unpacking it over an existing
            // installation is a decision about a directory only its owner
            // knows. So the verified file is handed to the desktop, exactly as
            // it always was, and [`baz_update::handed_off_note`] says what
            // will appear.
            Message::UpdateFetched(answer) => {
                self.updating = match answer {
                    Ok(_) if baz_update::installs_itself() => {
                        let version = self
                            .update_found
                            .as_ref()
                            .map_or_else(String::new, |update| update.version.clone());
                        views::settings::Updating::Staged(version)
                    }
                    Ok(path) => match baz_update::hand_off(&path) {
                        Ok(()) => views::settings::Updating::HandedOff,
                        Err(why) => views::settings::Updating::Failed(why),
                    },
                    Err(why) => views::settings::Updating::Failed(why),
                };
                Task::none()
            }
            // **The bulk verbs** (`views::marks`, `docs/WORK.md` item 62).
            // Every one of them spends a path that already existed for one
            // object; what is new is that the set is turned into queue rows
            // *once*, in the order it is drawn, and handed over whole.
            Message::MarkedPlay => {
                let items = self.marked_items();
                if items.is_empty() {
                    return Task::none();
                }
                self.start_marked(items)
            }
            Message::MarkedQueue => {
                let items = self.marked_items();
                self.append_items_to_run(items);
                Task::none()
            }
            Message::MarkedAddToPlaylist => {
                let items = self.marked_items();
                if items.is_empty() {
                    return Task::none();
                }
                if let Screen::Shelf(state) = &self.screen {
                    let entries = crate::playlists::entries_for_items(&items);
                    let label = format!("Add {} tracks", items.len());
                    self.playlists
                        .begin_pick(Some(&state.library), label, entries, items);
                }
                Task::none()
            }
            Message::MarkedRemove => self.remove_marked(),
            Message::MarksClear => {
                self.clear_marks();
                Task::none()
            }
            Message::ChromeApproached(near) => {
                self.chrome_veil.go(
                    if near { 1.0 } else { 0.0 },
                    motion::CHROME_REVEAL,
                    Instant::now(),
                );
                Task::none()
            }
            Message::ToggleChromeless => {
                self.chromeless = !self.chromeless;
                Task::none()
            }
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
            // **A file dropped on a running baz is music to play.**
            //
            // The Setup screen's own drop answers *where is your music*; once
            // the library is open the question is different, and the answer
            // every player in the world gives is the queue. It is also the
            // only answer that cannot lose anything: nothing is scanned,
            // nothing is written, no root is added, and the run you were
            // listening to keeps playing with the drop behind it.
            //
            // A **folder** is walked for the audio it holds, in path order, so
            // dropping an album folder queues the album. Anything baz cannot
            // decode is counted and reported rather than silently ignored —
            // dropping a folder of FLACs and one PDF should not leave a
            // listener wondering which track went missing.
            Message::FileDropped(path) if matches!(self.screen, Screen::Shelf(_)) => {
                self.drop_hover = false;
                self.take_drop(&path)
            }
            // **The hover says where it will land**, in the strip at the foot
            // of the place (`views::marks::hint`). It used to say nothing at
            // all, on the reasoning that the queue's own count is the receipt
            // — which is true *after* the drop and no help at all during it,
            // when the question is whether baz will take this and what it will
            // do with it. X11 only: Wayland's protocol does not report a
            // hovering drag to a client that has not accepted it.
            Message::FileHovered if matches!(self.screen, Screen::Shelf(_)) => {
                self.drop_hover = true;
                Task::none()
            }
            Message::FileHoverLeft if matches!(self.screen, Screen::Shelf(_)) => {
                self.drop_hover = false;
                Task::none()
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
            self.layout,
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
                match Shelf::open(
                    roots,
                    self.group_key,
                    self.density,
                    self.layout,
                    self.lane_open,
                ) {
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
        match Shelf::open(
            roots.clone(),
            self.group_key,
            self.density,
            self.layout,
            self.lane_open,
        ) {
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

    /// **Whichever selection the visible place owns.**
    ///
    /// The search dropover keeps its own, because it is a list *over* the
    /// place rather than in it, and every reader of a selection has to make
    /// the same choice `press_content` does.
    fn live_selection(&self) -> Option<&crate::selection::State> {
        let Screen::Shelf(state) = &self.screen else {
            return None;
        };
        Some(if state.search_open {
            &state.search_selection
        } else {
            &state.selection
        })
    }

    /// Everything selected, in the order it was marked.
    fn marked(&self) -> Vec<Content> {
        self.live_selection()
            .map(|selection| selection.marked().to_vec())
            .unwrap_or_default()
    }

    /// **The selected set as queue rows**, ready for any of the bulk verbs.
    ///
    /// One conversion for all of them, because the alternative is four places
    /// that each have to agree about what a marked album means. A marked
    /// *record* means its tracks, in its own order; a marked row means that
    /// row and nothing around it — ADR-0023 §3's queue rule, which is that a
    /// listener who pointed at a track gets the track.
    ///
    /// Anything that cannot be resolved contributes nothing rather than a
    /// placeholder: a row whose list has changed under the selection is gone,
    /// and a run with a hole in it would be worse than a shorter one.
    fn marked_items(&self) -> Vec<vm::QueueItemVm> {
        let Screen::Shelf(state) = &self.screen else {
            return Vec::new();
        };
        let mut items = Vec::new();
        for content in self.marked() {
            match content {
                Content::Album(id) => {
                    let chosen = state.edition_choice.get(&id).copied();
                    if let Some(album) = state.albums.iter().find(|album| album.id == id) {
                        items.extend(vm::album_queue(album, chosen).items);
                    }
                }
                Content::AlbumTrack { album, row } | Content::SearchTrack { album, row } => {
                    if let Some(item) = self.album_row_item(album, row) {
                        items.push(item);
                    }
                }
                Content::PlaylistTrack { playlist, row } => {
                    if let Some(item) = self
                        .playlists
                        .page(playlist)
                        .and_then(|open| open.queue.items.get(row))
                    {
                        items.push(item.clone());
                    }
                }
                Content::QueueTrack(row) => {
                    if let Some(item) = self.player.queue().and_then(|queue| queue.items.get(row)) {
                        items.push(item.clone());
                    }
                }
                // The tiles that stand for a list are selected one at a time
                // and have no bulk verb — `selection::Run::bulkable`.
                Content::Playlist(_) | Content::AllSongs | Content::ArtistSongs(_) => {}
            }
        }
        items
    }

    /// One album page row as a queue row, which is what a marked track means.
    fn album_row_item(&self, album: u64, row: usize) -> Option<vm::QueueItemVm> {
        let Screen::Shelf(state) = &self.screen else {
            return None;
        };
        let held = state.albums.iter().find(|held| held.id == album)?;
        let chosen = state.edition_choice.get(&album).copied();
        let track = vm::selected_edition(held, chosen)?.tracks.get(row)?;
        Some(vm::QueueItemVm {
            title: track.title.clone(),
            artist: track.artist.clone().filter(|_| held.track_artists_vary),
            album: held.title.clone(),
            album_artist: Some(held.artist.label().to_owned()),
            duration: track.duration,
            path: track.path.clone(),
        })
    }

    /// **Play the marked set as one assembled run.**
    ///
    /// `Assembled`, the same provenance a run built one pick at a time carries
    /// — because that is exactly what it is, gathered in one gesture instead
    /// of several. It is the one kind the save word is for.
    fn start_marked(&mut self, items: Vec<vm::QueueItemVm>) -> Task<Message> {
        let (album, artist) = items.first().map_or((None, String::new()), |item| {
            (
                item.album.clone(),
                item.album_artist.clone().unwrap_or_default(),
            )
        });
        let queue = vm::QueueVm {
            album,
            artist,
            items,
            origin: None,
            source: vm::RunSource::Assembled,
        };
        self.start_and_show(queue);
        Task::none()
    }

    /// **Take the marked rows out of the list they are in**, bottom row first.
    ///
    /// Descending, and that is the whole of the arithmetic: every removal
    /// shifts the rows below it up, so removing row 2 before row 5 would take
    /// out the row that *was* row 6. Going from the bottom leaves every row
    /// still to be removed at the index it was marked at.
    fn remove_marked(&mut self) -> Task<Message> {
        let marked = self.marked();
        if !crate::views::marks::removable(&marked) {
            return Task::none();
        }
        let mut rows: Vec<usize> = marked
            .iter()
            .filter_map(|content| match content {
                // The two lists a listener owns, and the only two `removable`
                // lets through — see `views::marks`.
                Content::QueueTrack(row) | Content::PlaylistTrack { row, .. } => Some(*row),
                _ => None,
            })
            .collect();
        rows.sort_unstable();
        rows.dedup();
        let queue = matches!(marked.first(), Some(Content::QueueTrack(_)));
        for row in rows.into_iter().rev() {
            if queue {
                self.remove_queued(row);
            } else if let Screen::Shelf(state) = &self.screen {
                let library = &state.library;
                self.playlists.remove_entry(row, library);
            }
        }
        // The rows the set named are gone; a selection of indices into a list
        // that has changed underneath is not a selection of anything.
        self.clear_marks();
        Task::none()
    }

    /// Put the selection down, in whichever place owns it.
    fn clear_marks(&mut self) {
        if let Screen::Shelf(state) = &mut self.screen {
            if state.search_open {
                state.search_selection.clear();
            } else {
                state.selection.clear();
            }
        }
    }

    /// **The list `content` lives in, in the order it is drawn.**
    ///
    /// What <kbd>Shift</kbd> needs to mean *everything from there to here*
    /// (`crate::selection::State::extend`), and it lives here rather than in
    /// the selection module for the reason that module's note gives: a range
    /// is a slice of what a listener can *see*, so the surface that draws it
    /// is what should say what it holds.
    ///
    /// **Tiles answer nothing**, deliberately. <kbd>Shift</kbd>-click on a
    /// sleeve is the established Queue accelerator (doc 09 §13 step 7) and has
    /// a printed accelerator in the tile menu; taking it away to give the wall
    /// a range would break a taught gesture to add an untaught one. Ctrl still
    /// builds a set of records by hand, which is the half that has no other
    /// route. An empty run makes `extend` an ordinary selection, which is the
    /// honest answer rather than a guess.
    fn run_of(&self, content: Content) -> Vec<Content> {
        use crate::selection::Run;
        let Screen::Shelf(state) = &self.screen else {
            return Vec::new();
        };
        match content.run() {
            Run::Records | Run::Lists => Vec::new(),
            Run::Queue => self.player.queue().map_or_else(Vec::new, |queue| {
                (0..queue.items.len()).map(Content::QueueTrack).collect()
            }),
            Run::AlbumTracks(album) => state
                .albums
                .iter()
                .find(|held| held.id == album)
                .and_then(|held| {
                    vm::selected_edition(held, state.edition_choice.get(&album).copied())
                })
                .map_or_else(Vec::new, |edition| {
                    (0..edition.tracks.len())
                        .map(|row| Content::AlbumTrack { album, row })
                        .collect()
                }),
            Run::PlaylistTracks(playlist) => self
                .playlists
                .open
                .as_ref()
                .filter(|open| open.id == playlist)
                .map_or_else(Vec::new, |open| {
                    (0..open.rows.len())
                        .map(|row| Content::PlaylistTrack { playlist, row })
                        .collect()
                }),
            // **The results in the order they are drawn**, tracks only: the
            // album and playlist results in the same list are doors rather
            // than rows, and a range that swept one up would carry a whole
            // record into a set of songs.
            Run::SearchTracks => (0..state.search_result_count())
                .filter_map(|index| state.search_result_content(index))
                .filter(|found| matches!(found, Content::SearchTrack { .. }))
                .collect(),
        }
    }

    /// One click selects; the second click on the same playable object inside
    /// the shared interval activates. Shift-click on an album retains its
    /// established explicit Queue accelerator.
    ///
    /// **<kbd>Ctrl</kbd> and <kbd>Shift</kbd> build a set instead**
    /// (`crate::selection`, `docs/WORK.md` item 62) — Ctrl adds or removes
    /// one, Shift takes the range from the anchor. Neither can activate: two
    /// modified presses building a set of two adjacent rows must not start
    /// playing one of them.
    fn press_content(&mut self, content: Content) -> Task<Message> {
        if let Content::Album(id) = content
            && self.modifiers.shift()
        {
            return self.queue_album(id);
        }
        if self.modifiers.command() {
            self.mark_content(content, None);
            return Task::none();
        }
        if self.modifiers.shift() {
            let run = self.run_of(content);
            self.mark_content(content, Some(run));
            return Task::none();
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

    /// **A modified press**, against whichever selection the place owns.
    ///
    /// `range` is `Some` for <kbd>Shift</kbd> and `None` for <kbd>Ctrl</kbd>.
    /// The search dropover keeps its own selection — it is a list over the
    /// place rather than in it — and that split is the one this has to respect
    /// rather than reinvent, so it mirrors `press_content`'s own choice.
    fn mark_content(&mut self, content: Content, range: Option<Vec<Content>>) {
        let Screen::Shelf(state) = &mut self.screen else {
            return;
        };
        let searching = state.search_open && state.search_result_index(content).is_some();
        let selection = if searching {
            &mut state.search_selection
        } else {
            &mut state.selection
        };
        match range {
            Some(run) => selection.extend(content, &run),
            None => selection.toggle(content),
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
            // **A playlist row is a door**, whichever of the two the keyboard
            // happens to be resting on: the row draws exactly one control and
            // it says `Open`. The chooser's default action is `Play`, so this
            // arm is what makes <kbd>Enter</kbd> mean the same thing the one
            // visible control means.
            (Content::Playlist(id), _) => {
                let clear = match &mut self.screen {
                    Screen::Shelf(state) => state.clear_query(),
                    Screen::Setup(_) | Screen::Blocked(_) => Task::none(),
                };
                Task::batch([clear, self.open_playlist(id)])
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
    /// **Move the wall's selection by one tile**, and bring it into view.
    ///
    /// The arithmetic is [`crate::grid::step`]'s and is tested without a
    /// window; what is here is the join — the shelves' ends, the grid's
    /// column count, and the scroll that has to follow, because a selection
    /// that moved off screen would be the keyboard losing its own place.
    ///
    /// **The first press selects rather than moves.** Tab to a wall nobody has
    /// clicked on and there is no selection to step from, so the honest answer
    /// to the first arrow is the first record — an arrow that did nothing
    /// would read as the ring being decorative.
    fn wall_step(&mut self, direction: crate::search::Direction) -> Task<Message> {
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        let ends: Vec<usize> = state.groups.iter().map(|group| group.end).collect();
        let columns = state.grid().columns;
        let at = match state.selection.selected() {
            Some(Content::Album(id)) => state.albums.iter().position(|album| album.id == id),
            _ => None,
        };
        let to = match at {
            Some(at) => crate::grid::step(at, direction, columns, &ends),
            None => (!state.albums.is_empty()).then_some(0),
        };
        let Some(to) = to else {
            return Task::none();
        };
        let Some(album) = state.albums.get(to) else {
            return Task::none();
        };
        state.selection.select(Content::Album(album.id));

        // **And bring it into view**, or the keyboard has lost its own place:
        // the ring says the collection has focus, the selection says which
        // record, and a record you cannot see says neither. Only when it is
        // actually off screen — an arrow that scrolled a visible row would
        // make the wall lurch under a listener stepping across one line.
        let hang = state.grid();
        let shelves = state.shelves();
        let runs = shelves.runs();
        let shelf_index = ends.iter().position(|end| to < *end).unwrap_or(0);
        let start = if shelf_index == 0 {
            0
        } else {
            ends[shelf_index - 1]
        };
        let Some(run) = runs.get(shelf_index) else {
            return Task::none();
        };
        let row = (to - start) / columns.max(1);
        let top = run.rows_top(hang) + hang.spacer_height(row);
        let bottom = top + hang.row_h;
        let viewport = state.grid_size.height;
        let offset = state.scroll_offset;
        let target = if top < offset {
            top
        } else if bottom > offset + viewport {
            bottom - viewport
        } else {
            return Task::none();
        };
        let target = target.max(0.0);
        state.scroll_offset = target;
        iced::widget::operation::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: target })
    }

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
        // **The ladder is also the way back.** Pressing a size detent while the
        // collection is hung as a list means *this size, on the wall* — the
        // marks say how big, and asking how big is asking about works. Without
        // this a listener who switched to a list could change the row pitch and
        // never find their way out of it, because the shape mark goes lit and
        // inert once it is the fact.
        if self.layout == shelf::Layout::List {
            let wall = shelf::Layout::Wall;
            self.layout = wall;
            persist(move |config| config.layout = wall);
            if let Screen::Shelf(state) = &mut self.screen {
                state.layout = wall;
            }
        }
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
            // **The same tick drives two things and only one of them is
            // everywhere.** Away from Now playing the case is not drawn and the
            // history is not read — the backdrop there is one soft ground built
            // from the live frame ([`crate::glass`]) — so this arm does nothing
            // but ask for the repaint the subscription has already decided is
            // worth having.
            Message::CaseTick(now) => {
                if self.place == Place::NowPlaying && self.visualization.foreground.draws_case() {
                    self.case_rotation.tick(now);
                }
                if self.place == Place::NowPlaying && self.visualization.mode.records_history() {
                    // A frame the tap could not read consistently is skipped,
                    // not captured. The ring scrolls, so writing a zeroed
                    // frame here would draw a notch of silence and then carry
                    // it across the whole display — an artefact the eye
                    // follows, standing in for a frame nobody would have
                    // noticed missing.
                    if let Some(audio) = self.playback.visualization() {
                        self.visualization_history
                            .capture(self.visualization.mode, &audio);
                    }
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
    ///
    /// **Visible is no longer the same as *on Now playing*.** Since the
    /// backdrop became weather (2026-08-22) it is drawn behind every place, so
    /// the tap has to follow the *drawing* rather than the room — and it did
    /// not. The clock was fixed that day and this gate was left, which left a
    /// backdrop that repainted ten times a second from a frame nobody was
    /// filling: alive by every measure except the picture.
    ///
    /// The owner, 2026-08-23: *"I think it still sort of pauses despite being
    /// blurred and showing through."* It did, and this is why.
    ///
    /// The condition is otherwise unchanged, and it is still a gate: nothing
    /// sounding, or a mode that is off, and the copy is not paid anywhere.
    fn sync_visualization_tap(&self) {
        self.playback.set_visualization_enabled(
            self.player.now_playing().is_some() && self.visualization.mode.active(),
        );
    }

    /// Log startup-to-interactive, once, on the first frame the window
    /// presents. The `window::frames()` subscription that produces it is
    /// dropped the moment this has run — the first bounded clock baz shipped,
    /// and the pattern ADR-0020 generalises.
    fn log_first_frame(&mut self) -> Task<Message> {
        if self.first_frame_logged {
            return Task::none();
        }
        self.first_frame_logged = true;
        crate::baz_log!(
            "[startup] startup-to-interactive: {:.1} ms",
            self.started.elapsed().as_secs_f64() * 1e3
        );
        // **The background pass, and it is here rather than in `new`.**
        //
        // After the first frame, so a listener never waits on a socket to see
        // their music. Once per launch and never again in the session: the
        // answer changes a few times a year, and a music player that reaches
        // the network while you are listening is doing something you did not
        // ask for.
        //
        // **It draws nothing at all.** The owner, 2026-08-20: *"honestly we
        // don't need to show that a new version in the app"*. What it does is
        // leave a verified installer where `baz-boot` will find it before baz
        // next starts — which is the one moment an installer can replace baz,
        // because baz is not running (ADR-0043 §5).
        let wanted =
            config::config_file().is_some_and(|path| config::load(&path).check_for_updates);
        // `installs_itself` is the whole gate: a download nothing can install
        // is bandwidth spent on a file that would sit in a cache forever.
        if !wanted || !baz_update::installs_itself() || !baz_update::Route::detect().can_install() {
            return Task::none();
        }
        Task::perform(tokio::task::spawn_blocking(stage_an_update), |joined| {
            Message::UpdateStaged(joined.unwrap_or_else(|error| Err(error.to_string())))
        })
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
        self.chrome_veil.tick(now);
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
            || self.chrome_veil.live()
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
                // The chooser's items come from the index rather than from
                // the facts, because a candidate *is* an index row (the
                // target's own note).
                let listed = match target {
                    menu::Target::LocatePlaylistEntry { row } => self.locate_items(*row),
                    other => menu::items(*other, &self.menu_facts()),
                };
                self.menu = (!listed.is_empty()).then(|| menu::Menu {
                    at: *at,
                    items: listed,
                    cursor: None,
                });
                Some(Task::none())
            }
            Message::CloseMenu => {
                self.menu = None;
                Some(Task::none())
            }
            Message::MenuMoved(delta) => {
                if let Some(menu) = &mut self.menu {
                    menu.cursor = menu.moved(*delta);
                }
                Some(Task::none())
            }
            Message::MenuActivated => {
                let index = self.menu.as_ref().and_then(|menu| menu.cursor);
                index.map_or_else(
                    || Some(Task::none()),
                    |index| Some(self.update(Message::MenuItemPressed(index))),
                )
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
            Message::PlaylistRepairEntry(row, to) => {
                if let Screen::Shelf(state) = &self.screen {
                    self.playlists.repair_entry(*row, to, &state.library);
                }
            }
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
                    // never the composing place, which is the one `from` that
                    // now answers with real work (it releases the Vibe text
                    // tower). The machine this arm lives in answers `bool`,
                    // so the task is discarded deliberately rather than by
                    // omission — and `every_place_that_leaves_work_behind_is_awaited`
                    // is what keeps a third of these from appearing quietly.
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
                    // never the composing place, which is the one `from` that
                    // now answers with real work (it releases the Vibe text
                    // tower). The machine this arm lives in answers `bool`,
                    // so the task is discarded deliberately rather than by
                    // omission — and `every_place_that_leaves_work_behind_is_awaited`
                    // is what keeps a third of these from appearing quietly.
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
                // **A shape on its own is a request.** This required a
                // non-empty description, which was right while the words
                // *were* the request and became a press that silently did
                // nothing the moment design note 25 made them optional — and
                // then the default, once `All songs` shipped as the standing
                // choice. The result pane has promised the opposite in
                // writing the whole time: *the shape on its own is a
                // perfectly good request. Compose, and the songs appear
                // here.*
                if state.vibe.preparing {
                    return Some(Task::none());
                }
                state.vibe.begin_request();
                if state.vibe.has_features() {
                    let answer = state.vibe.compose(&state.albums, &state.edition_choice);
                    return Some(Self::after_compose(answer));
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
            Message::VibeAnalyze => Some(self.start_listening()),
            Message::VibePrepared(result) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.accept_preparation(result.clone());
                    // **What listening learned**, from whatever the index
                    // already held. Cheap enough to run on a partial store,
                    // and doing it here means a returning listener sees the
                    // reading without waiting for a scan they already paid
                    // for.
                    state
                        .vibe
                        .rebuild_profile(&state.albums, &state.edition_choice);
                    if !state.vibe.analyzing && state.vibe.awaiting_create {
                        let answer = state.vibe.compose(&state.albums, &state.edition_choice);
                        return Some(Task::batch([
                            Self::after_compose(answer),
                            self.next_vibe_job(),
                        ]));
                    }
                }
                Some(self.next_vibe_job())
            }
            Message::VibeAnalyzed(result) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.accept_analysis(result.clone());
                    // Once, when the scan settles — not per track, which
                    // would sort the whole library a few thousand times.
                    if !state.vibe.analyzing {
                        state
                            .vibe
                            .rebuild_profile(&state.albums, &state.edition_choice);
                    }
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
                        let answer = state.vibe.compose(&state.albums, &state.edition_choice);
                        return Some(Task::batch([
                            Self::after_compose(answer),
                            self.next_vibe_job(),
                        ]));
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
                Some(self.recompose())
            }
            // **A word from the vocabulary**, appended with a comma. Design 21
            // §4: a chip is a way of writing the one request, never a second
            // input beside it.
            // **The words have been still for 400 ms.** Embed once, off this
            // thread; the count and the closest three are computed against
            // vectors already in memory when it comes back.
            Message::VibeWords(open) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.set_words(*open);
                }
                Some(Task::none())
            }
            Message::VibeCountTick => {
                let settled = match &mut self.screen {
                    Screen::Shelf(state) => state.vibe.settled_prompt(),
                    _ => None,
                };
                Some(settled.map_or_else(Task::none, |prompt| {
                    Task::perform(crate::vibe::embed(prompt), |(prompt, result)| {
                        Message::VibeEmbedded(prompt, result)
                    })
                }))
            }
            Message::VibeEmbedded(prompt, embedding) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    let (albums, chosen) = (&state.albums, &state.edition_choice);
                    state
                        .vibe
                        .accept_embedding(prompt, embedding, albums, chosen);
                }
                // The words have settled and been counted, which is the
                // moment they are worth composing from.
                Some(self.recompose())
            }
            // **A row explains itself.** Selecting one marks its dot, drops a
            // tick to the axis and writes the why-line; selecting it again
            // puts the explanation away.
            Message::VibePreviewSelected(row) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.select_row(*row);
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
            // **The gesture's end is when the line is worth composing from.**
            // Not during it: design 21 §6's refusal stands, because a list
            // that changed under a dragging hand could not be read and you
            // would be tuning against a moving target.
            Message::ContourReleased => Some(self.recompose()),
            Message::PlaylistImageChoose(id) => Some(pick_playlist_image(*id)),
            Message::PlaylistImagePicked(id, choice) => {
                let (id, choice) = (*id, choice.clone());
                let Some(path) = choice else {
                    // A dismissal changes nothing, and says nothing: the
                    // listener closed a dialog they opened.
                    return Some(Task::none());
                };
                let library = match &self.screen {
                    Screen::Shelf(state) => Some(&state.library),
                    Screen::Setup(_) | Screen::Blocked(_) => None,
                };
                match self.playlists.set_image(id, &path, library) {
                    Ok(_) => {
                        if let Screen::Shelf(state) = &mut self.screen {
                            // The path can be the same and the bytes
                            // different, so the cached decode is dropped
                            // rather than compared.
                            state.forget_playlist_image(id);
                        }
                        Some(self.request_playlist_art())
                    }
                    Err(reason) => {
                        crate::baz_log!("[playlists] {reason}");
                        Some(Task::none())
                    }
                }
            }
            Message::PlaylistImageRemove(id) => {
                let id = *id;
                let library = match &self.screen {
                    Screen::Shelf(state) => Some(&state.library),
                    Screen::Setup(_) | Screen::Blocked(_) => None,
                };
                self.playlists.remove_image(id, library);
                if let Screen::Shelf(state) = &mut self.screen {
                    state.forget_playlist_image(id);
                }
                Some(self.request_playlist_art())
            }
            Message::PlaylistImageLoaded(id, handle) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.finish_playlist_image(*id, handle.clone());
                }
                Some(Task::none())
            }
            Message::FavouriteRowEntered(row) => {
                self.hovered_favourite_row = Some(*row);
                Some(Task::none())
            }
            Message::FavouriteRowLeft(row) => {
                if self.hovered_favourite_row == Some(*row) {
                    self.hovered_favourite_row = None;
                }
                Some(Task::none())
            }
            Message::DraftRowEntered(row) => {
                self.playlists.creation.hovered_row = Some(*row);
                Some(Task::none())
            }
            Message::DraftRowLeft(row) => {
                if self.playlists.creation.hovered_row == Some(*row) {
                    self.playlists.creation.hovered_row = None;
                }
                Some(Task::none())
            }
            Message::VibePreviewEntered(row) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.hover_row(Some(*row));
                }
                Some(Task::none())
            }
            Message::VibePreviewLeft(row) => {
                if let Screen::Shelf(state) = &mut self.screen
                    && state.vibe.hovered_row == Some(*row)
                {
                    state.vibe.hover_row(None);
                }
                Some(Task::none())
            }
            Message::VibeRecipe(index) => {
                if let Screen::Shelf(state) = &mut self.screen
                    && let Some(recipe) = crate::vibe::Recipe::ALL.get(*index)
                {
                    state.vibe.start_from(*recipe);
                    // The suggested name follows the words, exactly as typing
                    // them would — a mood is the form filled in, not a second
                    // way of asking.
                    self.playlists.suggest_creation_name(recipe.prompt);
                    // **One press to a list**, which design 21 §11 calls the
                    // strongest moment in the feature — and only once there is
                    // something to compose from, because a press that composed
                    // nothing would be a worse first moment than a press that
                    // filled the form.
                    if state.vibe.has_features() && !state.vibe.preparing {
                        let (albums, chosen) = (&state.albums, &state.edition_choice);
                        let answer = state.vibe.compose(albums, chosen);
                        return Some(Self::after_compose(answer));
                    }
                }
                Some(Task::none())
            }
            Message::ContourPoints(count) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.set_points(*count);
                }
                Some(self.recompose())
            }
            Message::VibeLine(lane) => {
                if let Screen::Shelf(state) = &mut self.screen {
                    state.vibe.show_line(*lane);
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

    /// Number of concurrent local Vibe analyzers — [`config::DEFAULT_VIBE_WORKERS`],
    /// which is four and is a **measured** memory decision rather than a
    /// guess: each one costs about 145 MiB of ONNX Runtime arena, so eight of
    /// them is where the owner's 1.8 GB came from
    /// (`docs/design/impl/vibe-memory/`). `BAZ_VIBE_WORKERS` and the config's
    /// `vibe_workers` still buy speed with memory, up to sixteen.
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
        // **The authored sleeves ride with the collages**, because they are
        // the same question asked of the same folder: which lists exist, and
        // what does each one draw. The set is every list that has a picture
        // rather than the visible slice — one small decode per list the
        // listener chose a file for, and passing the whole set is also what
        // drops the cache entry for a list that is gone.
        let pictures: Vec<(u64, PathBuf)> = self
            .playlists
            .rows
            .iter()
            .filter_map(|row| row.image.clone().map(|path| (row.id, path)))
            .collect();
        match &mut self.screen {
            Screen::Shelf(state) => Task::batch([
                state.request_thumbs(&wanted),
                state.request_playlist_images(&pictures),
            ]),
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
        //
        // **And it shows Now playing**, which it did not until the owner said
        // so: *"when the play button is pressed for a playlist it does not go
        // to the now playing screen."* It had its own copy of
        // [`Self::start_and_show`]'s four lines minus the one that matters —
        // which is the whole argument for the shared tail existing, and the
        // reason the duplicate was worth deleting rather than adding a `go`
        // beside it. Pressing Play on a record and pressing Play on a playlist
        // are the same gesture and now take the same path, confirmation
        // boundary included: the place changes when a track actually starts,
        // not when the channel accepts the command.
        self.start_and_show(queue);
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
        // **The whole-list Play shows Now playing; a row press does not**, and
        // that difference is the product's, not an accident of which function
        // was written first. Pressing `Play` on a list is a decision to listen
        // to it; pressing one of its rows is a decision made *while browsing*,
        // and taking the browser away from the list they are reading would be
        // answering a question they did not ask. `play_track` on a record's
        // page draws the same line.
        let Some(lead) = lead else {
            self.start_and_show(queue);
            return;
        };
        let Some(position) = self.send_run(queue, Some(lead)) else {
            return;
        };
        if self.playback.send(Command::JumpTo { position }) {
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
        if self.playback.send(Command::UpdateQueueNext {
            fade_into_next: fade_seams_of(&self.screen, &paths),
            paths,
            next: at,
        }) {
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
        let fade_into_next = fade_seams_of(&self.screen, &paths);
        if self.playback.send(Command::UpdateQueue {
            paths,
            fade_into_next,
        }) {
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
            persist(|config| config.last_place = self.place.to_reopen_in());
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
        let fade_into_next = fade_seams_of(&self.screen, &paths);
        if self.playback.send(Command::SetQueue {
            paths,
            origin,
            fade_into_next,
        }) {
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
        self.sync_playlist_corpus();
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

    /// **Keep the list in step with the request**, when a control settles.
    ///
    /// The owner: *"it seems like we should just create a playlist
    /// immediately and change it based on how they change the options."* So a
    /// changed option composes, and there is always a list to look at.
    ///
    /// **Deterministically**, which is what makes this bearable rather than
    /// dizzying: this recomposes at the seed the request already stands at,
    /// so the same request gives the same list and only what changed shows up
    /// as a change. Pressing *Compose* is still the thing that draws a
    /// *different* one.
    ///
    /// It is called from the settling of a control, never from the middle of
    /// a gesture — design 21 §6's one deliberate refusal, which this does not
    /// touch: a result that changed under a dragging hand could not be read,
    /// so the curve recomposes on release.
    /// **Turn a compose's answer into the work it still needs.**
    ///
    /// `Compose::NeedsEmbedding` means the words have no vector yet, and
    /// getting one is a 350 MiB model on a shared mutex — so it goes to the
    /// blocking pool and comes back as `VibeEmbedded`, which composes again
    /// with it in hand. The same task the debounced live count already used;
    /// what changed is that the interface thread no longer does it inline
    /// (audit finding 3).
    fn after_compose(answer: crate::vibe::Compose) -> Task<Message> {
        match answer {
            crate::vibe::Compose::Done => Task::none(),
            crate::vibe::Compose::NeedsEmbedding(prompt) => {
                Task::perform(crate::vibe::embed(prompt), |(prompt, result)| {
                    Message::VibeEmbedded(prompt, result)
                })
            }
        }
    }

    /// Compose again at the seed the request stands at, and hand back whatever
    /// that still needs — a length detent and a released contour handle both
    /// arrive here, and neither may run the text tower between two frames.
    fn recompose(&mut self) -> Task<Message> {
        if let Screen::Shelf(state) = &mut self.screen
            && state.vibe.has_features()
            && !state.vibe.preparing
            && state.vibe.open
        {
            let (albums, chosen) = (&state.albums, &state.edition_choice);
            let answer = state.vibe.recompose(albums, chosen);
            return Self::after_compose(answer);
        }
        Task::none()
    }

    /// **Begin listening to whatever has not been heard**, or do nothing if
    /// there is nothing to hear or a pass is already running.
    ///
    /// One path for both callers: the explicit press on the door, and the
    /// door's own arrival. Idempotent by construction, so opening the place
    /// twice does not start two scans.
    fn start_listening(&mut self) -> Task<Message> {
        let Some(index) = config::vibe_db_file() else {
            if let Screen::Shelf(state) = &mut self.screen {
                state.vibe.error = Some(
                    "This system offers no data folder for the local analysis index.".to_owned(),
                );
            }
            return Task::none();
        };
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        // Already working: an arrival must not restart a pass in flight.
        if state.vibe.preparing || state.vibe.analyzing {
            return Task::none();
        }
        let paths = crate::vibe::library_paths(&state.albums, &state.edition_choice);
        // Nothing to hear — an empty library, or one baz has heard entirely.
        // Preparing anyway would flicker a progress reading over a finished
        // job, which is worse than silence.
        if paths.is_empty() || state.vibe.analysed() >= paths.len() {
            return Task::none();
        }
        state.vibe.start_preparing();
        Task::perform(crate::vibe::prepare(index, paths), Message::VibePrepared)
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
        self.enter_playlist_place(id)
    }

    /// **Queue what was dropped.**
    ///
    /// One `FileDropped` arrives per path, so a multi-file drop is several
    /// calls and the queue grows in the order the platform delivered them —
    /// which is the order the file manager showed them.
    ///
    /// Paths are taken as they are, not looked up in the library: the engine
    /// plays paths (ADR-0024 §3's reasoning, from the other direction), so a
    /// folder that was never scanned plays anyway. What the library *is* used
    /// for is the metadata: a dropped file baz already knows keeps its title
    /// and artist, and one it does not is named by its filename rather than by
    /// nothing.
    /// **Where a drop will land, and what to call it.**
    ///
    /// `docs/WORK.md` item 70's missing half. A drop used to mean *queue it*
    /// wherever it fell, which is the right answer almost everywhere and the
    /// wrong one in the two places a listener is plainly building a list:
    /// standing on a playlist's page, or on the draft of a new one. Dropping a
    /// folder of files onto an open playlist and having them play instead is
    /// baz answering a question nobody asked.
    ///
    /// It is decided by the **place**, not by the pointer. A drop is delivered
    /// with a position on some platforms and not on others, and a gesture
    /// whose meaning depended on which half of a window it landed in would be
    /// a gesture nobody could learn.
    fn drop_lands(&self) -> DropTo {
        match self.place {
            Place::Playlist(id) => self.playlists.page(id).map_or(DropTo::Run, |open| {
                DropTo::Playlist(id, open.name().to_owned())
            }),
            Place::NewPlaylist => DropTo::Draft,
            _ => DropTo::Run,
        }
    }

    /// Take a drop to wherever [`Self::drop_lands`] says it belongs.
    fn take_drop(&mut self, path: &Path) -> Task<Message> {
        let found = crate::drop::audio_under(path);
        if found.is_empty() {
            crate::baz_log!("[drop] {path:?} holds nothing baz can play");
            return Task::none();
        }
        let Screen::Shelf(state) = &self.screen else {
            return Task::none();
        };
        let items: Vec<vm::QueueItemVm> = found
            .iter()
            .map(|path| vm::dropped_item(&state.library, path))
            .collect();
        match self.drop_lands() {
            DropTo::Run => return self.queue_dropped(path),
            DropTo::Playlist(id, name) => {
                let entries = crate::playlists::entries_for_items(&items);
                let count = entries.len();
                let Screen::Shelf(state) = &self.screen else {
                    return Task::none();
                };
                // The library is borrowed twice over — once to build the
                // entries above and once to write them — so the write takes
                // its own borrow rather than holding one across the call.
                let library = &state.library;
                self.playlists.append(id, entries, library);
                crate::baz_log!("[drop] added {count} to {name:?}");
                // The page is re-read so the rows the listener is looking at
                // are the ones the file now holds.
                if let Screen::Shelf(state) = &self.screen {
                    let library = &state.library;
                    self.playlists.reload_open(library);
                }
            }
            DropTo::Draft => {
                let before = self.playlists.creation.items.len();
                for item in items {
                    if !self
                        .playlists
                        .creation
                        .items
                        .iter()
                        .any(|held| held.path == item.path)
                    {
                        self.playlists.creation.items.push(item);
                    }
                }
                crate::baz_log!(
                    "[drop] added {} to the draft",
                    self.playlists.creation.items.len() - before
                );
            }
        }
        Task::none()
    }

    fn queue_dropped(&mut self, path: &Path) -> Task<Message> {
        let found = crate::drop::audio_under(path);
        if found.is_empty() {
            crate::baz_log!("[drop] {path:?} holds nothing baz can play");
            return Task::none();
        }
        let Screen::Shelf(state) = &self.screen else {
            return Task::none();
        };
        let items: Vec<vm::QueueItemVm> = found
            .iter()
            .map(|path| vm::dropped_item(&state.library, path))
            .collect();
        crate::baz_log!(
            "[drop] queued {} from {path:?}",
            crate::drop::phrase(items.len())
        );
        self.append_items_to_run(items);
        Task::none()
    }

    /// **Send the equaliser and write it down**, in that order.
    ///
    /// One command carries all three parts because they are one decision; the
    /// engine re-designs its filters once rather than three times, and a
    /// half-applied curve never reaches a block.
    fn send_equalizer(&mut self) {
        self.send_equalizer_only();
        self.persist_equalizer();
    }

    /// Hand the engine the current settings and nothing else.
    fn send_equalizer_only(&mut self) {
        if !self.playback.send(Command::SetEqualizer {
            enabled: self.equalizer.enabled,
            bands_centidb: self.equalizer.bands_centidb,
            preamp_centidb: self.equalizer.preamp_centidb,
        }) {
            self.player.engine_closed();
        }
    }

    /// Write the curve down. Separate from the send because a drag is a
    /// hundred sends and one decision.
    /// Write the saved curves back to the config.
    ///
    /// Separate from [`Self::persist_equalizer`] because they change on
    /// different gestures: the live curve is written when a drag ends, and
    /// this when a curve is named or forgotten.
    /// **Put the pre-amp where the curve needs it**, when the listener has
    /// asked for that.
    ///
    /// `suggested_preamp` answers zero or a negative number of decibels: the
    /// amount the whole signal has to come down by so the largest boost fits
    /// without clipping. A curve that only cuts needs nothing, and gets zero
    /// — auto gain never makes anything *louder*, because turning a quiet
    /// recording up is a decision about taste and this is a decision about
    /// arithmetic.
    fn apply_auto_gain(&mut self) {
        if !self.equalizer.auto_gain {
            return;
        }
        let suggested = baz_core::equalizer::Bands::from_centidb(self.equalizer.bands_centidb)
            .suggested_preamp();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "derived from bands already clamped to ±12 dB"
        )]
        {
            self.equalizer.preamp_centidb = (suggested * 100.0).round() as i16;
        }
    }

    fn persist_equalizer_presets(&mut self) {
        let curves = self.equalizer_presets.clone();
        persist(|config| {
            config.equalizer_presets = curves;
        });
    }

    fn persist_equalizer(&mut self) {
        let settings = self.equalizer;
        persist(|config| {
            config.equalizer_enabled = settings.enabled;
            config.equalizer_bands_centidb = settings.bands_centidb;
            config.equalizer_preamp_centidb = settings.preamp_centidb;
            config.equalizer_auto_gain = settings.auto_gain;
        });
    }

    /// **Keep the searchable playlist corpus in step with the folder.**
    ///
    /// Called after every `playlists.refresh`, which is the only thing that
    /// can change what lists exist — so there is one writer and no clock. It
    /// re-filters as well, because a list renamed or deleted while a query
    /// stands must not go on being offered by the chooser.
    fn sync_playlist_corpus(&mut self) {
        let corpus = self.playlists.corpus();
        if let Screen::Shelf(state) = &mut self.screen {
            if state.playlist_names == corpus {
                return;
            }
            state.playlist_names = corpus;
            state.refilter();
        }
    }

    /// **Open a playlist's page by id**, from wherever asked.
    ///
    /// Added with the chooser's playlist section (2026-08-18), which needed a
    /// door that was not also a play gesture: `activate_content` opens *and*
    /// plays, which is right for a press on the Playlists place and wrong for
    /// a search result labelled `Open`.
    ///
    /// Favourites is a playlist id with a place of its own, and that fork
    /// lives here so every caller inherits it rather than each remembering.
    fn open_playlist(&mut self, id: u64) -> Task<Message> {
        if id == crate::playlists::FAVOURITES_ID {
            return self.go(|_| Place::Favourites);
        }
        let opened = match &self.screen {
            Screen::Shelf(state) => self.playlists.open_page(id, &state.library),
            Screen::Setup(_) | Screen::Blocked(_) => false,
        };
        if !opened {
            return Task::none();
        }
        self.enter_playlist_place(id)
    }

    /// Put the shell in the playlist's place, once the page is loaded. Shared
    /// by the creation flow and [`Self::open_playlist`] so the two cannot
    /// arrive in different states.
    fn enter_playlist_place(&mut self, id: u64) -> Task<Message> {
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
    /// **Esc with the caret in the search well: the press belongs to the well.**
    ///
    /// It used to take two. iced's `text_input` consumes Esc to blur itself
    /// and reports the press captured, so the focus rule in [`crate::keys`]
    /// dropped it and the three letters a listener had typed stayed on the
    /// wall until they pressed again — recorded for a long time as a toolkit
    /// limit rather than a design choice, which it was. The toolkit's capture
    /// report is the missing half: it says *the caret is in the well*, and
    /// that is enough to clear the query on the same press iced is blurring
    /// on. One press, wall back, which is what "peel the query" always meant.
    ///
    /// It deliberately peels **nothing else**. The layers `escape` walks —
    /// fullscreen, a drag, the menu, the place — are all reachable with the
    /// caret in the well, and letting one press take the field's blur *and* a
    /// layer underneath would trade a key that did too little for one that
    /// does too much. With an empty query this press is spent on the blur
    /// alone, which is why clicking into an empty well and pressing Esc puts
    /// the caret away rather than sending you home.
    /// **The `Locate…` card for a missing playlist entry** (ADR-0024 §3).
    ///
    /// Built here rather than in `crate::menu` because a candidate is an
    /// index row and the index is the shell's. Each item is one path, labelled
    /// by where it sits, and pressing it is the confirmation — the only thing
    /// in the product that writes a new path into a playlist file.
    ///
    /// An entry with nothing to propose returns nothing, which opens no card
    /// at all. That is the same rule every other target follows and it is the
    /// honest answer: there is no file of that name under any current root, so
    /// there is nothing to offer and a card of one greyed apology would be
    /// worse than silence. The row's own path is still on screen underneath,
    /// which is where the listener finds out what is being looked for.
    fn locate_items(&self, row: usize) -> Vec<menu::Item> {
        let Screen::Shelf(state) = &self.screen else {
            return Vec::new();
        };
        let Some(open) = self.playlists.open.as_ref() else {
            return Vec::new();
        };
        let Some(page_row) = open.rows.get(row).filter(|page_row| page_row.missing) else {
            return Vec::new();
        };
        let found = crate::repair::candidates(&page_row.path, &state.library);
        let listed: Vec<menu::Item> = found
            .shown
            .iter()
            .map(|path| menu::Item {
                label: crate::repair::location(path),
                presses: vec![Message::PlaylistRepairEntry(row, path.clone())],
                accelerator: None,
            })
            .collect();
        // **The cap is never silent, and never a row either.** A card that
        // showed eight of forty without saying so would read as "these are
        // the matches", and the listener is the one deciding. But the obvious
        // fix — a final line reading `32 more elsewhere` — is exactly the
        // thing this module's own mirror test calls a lie: *an inert item*
        // presses nothing and is a word dressed as a control. So the overflow
        // goes to the health log, which is a place a person can actually read
        // it (Settings → Debug), and the card stays a list of things that can
        // be pressed.
        if found.total > listed.len() {
            crate::baz_log!(
                "[playlists] {:?} has {} matches under a current root;                  offering the {} closest",
                page_row.path,
                found.total,
                listed.len()
            );
        }
        listed
    }

    fn escape_in_field(&mut self) -> Task<Message> {
        if let Screen::Shelf(state) = &mut self.screen
            && state.search_open
        {
            return state.clear_query();
        }
        Task::none()
    }

    fn escape(&mut self) -> Task<Message> {
        // **Chromeless is the outermost layer there is**, because while it
        // stands it is the only thing hiding the controls that would undo it.
        // Esc brings the frame back before it does anything else — the same
        // promise every other layer here makes, applied to the one whose
        // absence is most alarming if you cannot remember which mark you
        // pressed.
        if self.chromeless {
            self.chromeless = false;
            return Task::none();
        }
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
        // The shortcuts card floats over every other layer, including the
        // menu, because it is the one surface a listener opens *while lost*.
        // It peels first for that reason: whatever it is covering is what they
        // were trying to get back to.
        if self.shortcuts_open {
            self.shortcuts_open = false;
            return Task::none();
        }
        if self.equalizer_open {
            self.equalizer_open = false;
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
        // **A selection of more than one peels before the place does.** It is
        // something the listener assembled and has not spent, and leaving the
        // place with it standing would lose the work; one selected row is the
        // ordinary state of every list and is not a layer at all
        // (`views::marks`).
        if self.live_selection().is_some_and(|held| held.count() > 1) {
            self.clear_marks();
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
        let fade_into_next = fade_seams_of(&self.screen, &paths);
        if !self.playback.send(Command::SetQueue {
            paths,
            origin,
            fade_into_next,
        }) {
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
        self.set_shuffle(!self.player.shuffle());
    }

    /// **Shuffle at a stated value**, which a toggle cannot express.
    ///
    /// MPRIS's `Shuffle` is a *property*: a client writes `true`, and writing
    /// `true` to something already true must leave it true. Routing that
    /// through [`Self::toggle_shuffle`] would turn it off — the classic bug of
    /// serving a property with a verb — so the toggle is now written in terms
    /// of this rather than the other way round.
    fn set_shuffle(&mut self, on: bool) {
        if self.player.shuffle() == on {
            return;
        }
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
    /// How long the sleep timer has left, or `None` when it is off.
    fn sleep_remaining(&self) -> Option<Duration> {
        self.sleep
            .map(|sleep| sleep.fires_at.saturating_duration_since(Instant::now()))
    }

    /// Arm the sleep timer, or turn it off. Setting it again while it is
    /// running restarts it, which is what pressing a duration means.
    /// **Set the crossfade and remember it** (ADR-0044 §6).
    ///
    /// A standing preference, so it is persisted and handed to the engine,
    /// which applies it at the next boundary rather than by tearing down the
    /// play in progress. Where it may *happen* is not sent here — that travels
    /// with the queue, as `fade_into_next`, and is recomputed whenever the run
    /// changes.
    fn set_crossfade(&mut self, ms: u32) {
        let ms = ms.min(crate::config::MAX_CROSSFADE_MS);
        self.crossfade_ms = ms;
        persist(move |config| config.crossfade_ms = ms);
        self.playback.send(Command::SetCrossfade { ms });
    }

    /// **Hang the collection as a wall or as a list** (the owner, 2026-08-22).
    ///
    /// A standing preference like the density beside it, so it is persisted and
    /// survives a restart. The scroll offset is left where it is on purpose:
    /// the shelves are the same shelves in either shape, so the record a
    /// listener was looking at is still the record they are looking at — and
    /// `Shelves` re-derives every run against the new row pitch on the next
    /// frame, which is what keeps the offset meaning the same place.
    fn set_layout(&mut self, layout: shelf::Layout) -> Task<Message> {
        if self.layout == layout {
            return Task::none();
        }
        self.layout = layout;
        persist(move |config| config.layout = layout);
        if let Screen::Shelf(state) = &mut self.screen {
            state.layout = layout;
            // A list wants a different thumbnail size than the wall, and the
            // rows now on screen are not the rows that were: ask for what is
            // visible rather than waiting for a scroll to do it.
            return state.request_visible_thumbs();
        }
        Task::none()
    }

    fn set_sleep_timer(&mut self, minutes: Option<u32>) {
        self.sleep = minutes.map(|minutes| Sleep {
            minutes,
            fires_at: Instant::now() + Duration::from_secs(u64::from(minutes) * 60),
        });
        match minutes {
            Some(minutes) => crate::baz_log!("[sleep] pausing in {minutes} minutes"),
            None => crate::baz_log!("[sleep] off"),
        }
    }

    /// One tick of the sleep timer's own clock.
    ///
    /// **It pauses**, and does not stop, close or quit: pausing keeps the run,
    /// the position and the queue exactly where they are, so the next press
    /// carries on. It is recorded in the event history because music stopping
    /// on its own is exactly the kind of thing a listener should be able to
    /// look up rather than wonder about.
    fn tick_sleep_timer(&mut self) {
        let Some(sleep) = self.sleep else {
            return;
        };
        if Instant::now() < sleep.fires_at {
            return;
        }
        self.sleep = None;
        if self.player.now_playing().is_some() && !self.playback.send(Command::Pause) {
            self.player.engine_closed();
            return;
        }
        if let Screen::Shelf(state) = &mut self.screen {
            state.health.record(
                crate::health::Level::Ready,
                "Sleep timer",
                format!(
                    "Playback paused after {} minutes. Press play to carry on where you were.",
                    sleep.minutes
                ),
            );
        }
        crate::baz_log!("[sleep] paused playback");
    }

    fn cycle_repeat(&mut self) {
        use baz_core::protocol::Repeat;
        self.set_repeat(match self.player.repeat() {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        });
    }

    /// **Repeat at a stated value** — [`Self::set_shuffle`]'s reason, for
    /// MPRIS's `LoopStatus`, which is likewise a property and not a cycle.
    fn set_repeat(&mut self, repeat: baz_core::protocol::Repeat) {
        if self.player.repeat() == repeat {
            return;
        }
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
        // Both `All songs` gestures are whole-list plays, so both show — the
        // same rule `play_album` and `play_playlist` keep. See
        // [`Self::start_and_show`] for the confirmation boundary they share.
        self.start_and_show(list.queue);
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
        let fade_into_next = fade_seams_of(&self.screen, &paths);
        if self.playback.send(Command::UpdateQueue {
            paths,
            fade_into_next,
        }) {
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
        let fade_into_next = fade_seams_of(&self.screen, &paths);
        if self.playback.send(Command::UpdateQueue {
            paths,
            fade_into_next,
        }) {
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
        let fade_into_next = fade_seams_of(&self.screen, &paths);
        if self.playback.send(Command::UpdateQueue {
            paths,
            fade_into_next,
        }) {
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
        let fade_into_next = fade_seams_of(&self.screen, &paths);
        if self.playback.send(Command::UpdateQueue {
            paths,
            fade_into_next,
        }) {
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
        // **Chromeless is a Now playing mode, and going anywhere ends it.**
        //
        // The owner asked for it of one control — *"when we click the source
        // link it should take it out of full screen mode as well"* — and the
        // general form is the honest one: every route off this page lands
        // somewhere that needs the lane and the bars, so singling out the
        // source link would leave the same trap behind the next one. The
        // marks in the strip are the exception by construction: they change
        // nothing about the place.
        if self.chromeless && self.place != Place::NowPlaying {
            self.chromeless = false;
        }
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
        // **The composing place puts itself away.** Its result, its selected
        // row, its live count and its debounce clock were all only true while
        // it was on screen; the request it was built from is kept, because
        // walking off to check something in the Library is not a reason to
        // lose what you were asking for.
        //
        // The creation draft is deliberately *not* cleared: the manual route
        // exists to be filled from elsewhere in the product — *use the app-bar
        // search and choose Add to playlist* — so navigating away is that
        // route working, not that route being abandoned.
        if from == Place::NewPlaylist
            && let Screen::Shelf(state) = &mut self.screen
        {
            state.vibe.leave_page();
            // **And the text tower goes with the page.** It is the largest
            // single thing baz holds — roughly 370 MiB resident once ONNX
            // Runtime has its arena — and until this it was kept for the life
            // of the process after one compose. Measured on
            // `docs/design/impl/vibe-memory/`'s harness rather than reasoned
            // about, which is that directory's whole rule.
            //
            // It reopens on the next request. That is the right way round:
            // leaving this page is common, coming back to it is not, and a
            // listener who composed a playlist an hour ago and has been
            // listening since was holding a model for nothing.
            return Task::future(crate::vibe::release_text()).discard();
        }
        Task::none()
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
    /// **The track the bar names when nothing is sounding** — the owner,
    /// 2026-08-17: *"should we just default to showing the last thing that was
    /// playing in the bottom bar since we already seem to know? it only makes
    /// sense to have the nothing playing state when the user really has never
    /// played anything before"*.
    ///
    /// He is right that baz already knows: [`Self::restore_the_run`] hands the
    /// engine the whole remembered run at launch, so the file, its record and
    /// its position are all in memory while the bar says `Nothing playing`.
    ///
    /// The subject is [`views::home::standing`]'s, unchanged — the same
    /// reading `CONTINUE` has always drawn, so the band and the bar cannot
    /// name different tracks. It answers `None` while anything is sounding
    /// (then there is a real now-playing to draw) and `None` on a first run
    /// with no history, which is the state he says the empty bar is *for*.
    ///
    /// **It states identity and never playback.** The transport beside it
    /// offers `Play` rather than `Pause` and the timecode is blank, which is
    /// how a listener reads *stopped* — so this adds the name of the thing and
    /// claims nothing about it. Nothing here starts, seeks or queues anything.
    fn bar_standing(&self) -> Option<crate::player::NowPlaying> {
        let Screen::Shelf(state) = &self.screen else {
            return None;
        };
        if self.player.now_playing().is_some() {
            return None;
        }
        let (path, _) = views::home::standing(&self.player, &self.resume)?;
        Some(crate::player::resolve_now_playing(&state.albums, path))
    }

    fn bar_cover(&self) -> Option<views::bottom_bar::Cover> {
        let Screen::Shelf(state) = &self.screen else {
            return None;
        };
        // The sounding record, else the standing one — so the bar's sleeve
        // and its words are always about the same track.
        let id = self
            .player
            .playing_album()
            .or_else(|| self.bar_standing().and_then(|now| now.album_id))?;
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
        // **Is there a moving picture behind the collection?**
        //
        // The backdrop is stacked under every place (`over_field` below is
        // unconditional), so the wall's ground is that picture under
        // `views::now_playing::FROST`. What changes is whether the picture is
        // *moving*: with a live audio tap it is weather, and with none it is
        // the frozen soft ground, which sits near enough to `wall` to be
        // indistinguishable from it.
        //
        // The one reader is the pinned heading's band, and it is the whole of
        // why it needs to know: a band that must hide the covers passing under
        // it can be opaque `wall` where the ground is `wall`, and cannot be
        // where the ground is a picture. See `theme::STICKY_BAND` for the case
        // that is still open.
        // The same expression `Self::sync_visualization_tap` gates the tap on,
        // deliberately: the picture moves exactly when the tap is paid for.
        let weather = self.player.now_playing().is_some() && self.visualization.mode.active();
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
            (Screen::Shelf(state), Place::Library) => {
                state.view(&self.player, lamp, collecting, weather)
            }
            (Screen::Shelf(state), Place::Playlists) => views::playlists::view(
                state,
                &self.playlists,
                &self.player,
                state.grid(),
                self.playlists_scroll,
                weather,
            ),
            (Screen::Shelf(state), Place::NewPlaylist) => views::new_playlist::view(
                state,
                &self.playlists,
                iced::Size::new(self.body_width(), self.body_height()),
            ),
            (Screen::Shelf(state), Place::Favourites) => views::favourites::view(
                state,
                &self.player,
                self.body_width(),
                self.hovered_favourite_row,
            ),
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
                None => state.view(&self.player, lamp, collecting, weather),
            },
            (Screen::Shelf(state), Place::Artist(id)) => {
                // The artist vanished under a rescan while their page was
                // open — renamed, or their last record removed. The wall is
                // the honest answer, drawn rather than navigated to, exactly
                // as a vanished record's page is.
                if views::artist::label(state, id).is_some() {
                    views::artist::view(state, &self.player, id, state.grid(), collecting)
                } else {
                    state.view(&self.player, lamp, collecting, weather)
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
                    weather,
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
                    .then(|| self.playback.visualization())
                    .flatten();
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
                    self.sleep_remaining(),
                    self.loudness_progress.into(),
                    &self.updating,
                    config::config_file().is_some_and(|path| config::load(&path).check_for_updates),
                    self.crossfade_ms,
                    self.ink(),
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
        // **The lane runs the whole height of the window**, and the bar is the
        // *body's* bar rather than the window's.
        //
        // The owner: *"make the left hand bar go to the top and include the
        // back and forward icons in it. the search then becomes part of the
        // main area's top bar."* So the frame is a `row!` of lane and
        // everything-else, and the everything-else is a `column!` of bar over
        // body — where it used to be a `column!` of bar over a `row!` of lane
        // and body.
        //
        // The app's mark and the history pair moved into the lane's head with
        // that change: the mark's centre was already on the lane's own glyph
        // column (`SIDEBAR_HEAD_GLYPH_X`), which is why it lands where it
        // already looked like it was.
        let lane: Option<Element<'_, Message>> = match &self.screen {
            // Chromeless takes the lane with the bottom band. `wears_lane` is
            // a fact about the *place*; this is a fact about the frame, and
            // both have to be true for the lane to stand.
            Screen::Shelf(state) if self.place.wears_lane() && !self.chromeless => {
                Some(views::lane::view(
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
                    self.place_history.can_back(),
                    self.place_history.can_forward(),
                    ink,
                ))
            }
            _ => None,
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
        .then(|| crate::visualizer::State {
            chromeless: self.chromeless,
            ..self.visualization
        });
        let Screen::Shelf(state) = &self.screen else {
            unreachable!("setup and blocked screens return before app-bar composition")
        };
        // **Chromeless keeps the tree's shape and empties the slot.**
        //
        // Not `if !chromeless { column![bar, screen] } else { screen }`, which
        // is the obvious spelling and is the bug this file spends a long
        // comment on thirty lines below: iced diffs the widget tree by
        // position, so a column that loses its first child hands every widget
        // under it a fresh state. Toggling the frame would scroll the run back
        // to the top and drop whatever the place was holding.
        //
        // So the column always has two children and the first one is either
        // the bar or nothing at all.
        // **A field behind the bar means the bar can be glass.**
        //
        // On Now playing the page is a wash of the record's own colour, and
        // that is also where chromeless is used. The bar draws no ground in
        // either case; what changes is whether anything is drawn *under* it —
        // see the composition below.
        // **Glass everywhere, not only over the record.**
        //
        // The owner, 2026-08-20: *"can we make sure the top bar is transparent
        // for all modes not just now playing"*. It was conditional because the
        // condition used to matter — the bar was a plane above the page, and
        // its own `recess` ground was what separated the two. It does not
        // matter now: the bar and the page are one column, so a transparent
        // bar shows the window's own ground rather than anything that scrolls,
        // and the strip of `recess` it was drawing was a stripe across the top
        // of every place with nothing on the other side of it.
        //
        // The hairline goes with the ground, for the reason the bar's own
        // note gives: a seam belongs to the surface it divides, and with no
        // surface there is nothing for a line to divide.
        let over_field = true;
        let bar: Element<'_, Message> = {
            views::app_bar::view(
                state,
                self.window.width,
                hangs_works,
                self.layout,
                visualization,
                self.window_maximized,
                owns_chrome(),
                over_field,
                self.health_summary(),
                // **Chromeless keeps every control and lights none of them
                // until the hand comes near.** The owner: *"keep all windows
                // controls existing just invisible until you start moving your
                // mouse near to them"* — *existing* being the load-bearing
                // word. A control that is absent takes no space, so the bar
                // reflows as you approach; one that is merely unlit keeps its
                // place, its hit box and its tooltip, and the pointer restores
                // only the ink. Everywhere else the veil is 1 and this is the
                // bar it always was.
                if self.chromeless {
                    ink.veiled(self.chrome_veil.value())
                } else {
                    ink
                },
            )
        };
        // **The band that hears the approach**, and only in chromeless.
        //
        // A `mouse_area` rather than a tracked cursor: one message on crossing
        // in, one on crossing out, and none at all while the hand is still —
        // where reading a position would be a message per mouse move, which is
        // the per-frame cost every other clock in this file is guarded against.
        let bar: Element<'_, Message> = if self.chromeless {
            iced::widget::mouse_area(bar)
                .on_enter(Message::ChromeApproached(true))
                .on_exit(Message::ChromeApproached(false))
                .into()
        } else {
            bar
        };
        // **Over the page, not above it, wherever the bar is glass.**
        //
        // The owner, on chromeless: *"make sure the top bar is transparent
        // essentially… all the same icons as the normal one, but the
        // visualiser and background colour etc should be showing where it is
        // currently black."*
        //
        // The bar had been transparent since the field arrived behind it —
        // and read black anyway, because a `column!` gives the page the
        // window *less* the bar's height, so there was nothing under the glass
        // but the window's own ground. Stacking puts the page's full height
        // beneath it; `body_height` stops subtracting the bar to match.
        //
        // Chromeless carries the **same** bar rather than a reduced strip. It
        // asks for the lane and the bottom band to go, not for the doors to;
        // an immersive mode you cannot change the visualiser from is a mode
        // you leave to change the visualiser.
        // **The strip that spends a selection**, at the foot of the body —
        // always in the tree and empty at rest, for the reason this file
        // states at length about every other conditional layer: iced diffs by
        // position, so a strip that appeared would hand the place beneath it a
        // fresh state, scrolling the list back to the top at the exact moment
        // somebody ticked their fifth row in it (`views::marks`).
        // **One slot, two tenants**, and they cannot both be wanted: you are
        // either assembling a selection or dragging something in from outside.
        // The hover wins while it is happening.
        let marks: Element<'_, Message> = if self.drop_hover {
            views::marks::hint(self.drop_lands().hint())
        } else {
            views::marks::view(
                self.live_selection()
                    .map(crate::selection::State::marked)
                    .unwrap_or_default(),
                collecting.available,
            )
        };
        // The bar belongs to the body now, and the lane stands beside both.
        //
        // **There was a band here once**, between the two, offering an update
        // the startup check had just found. It is gone: the owner's *"we
        // don't need to show that a new version in the app"* moved that
        // question to `baz-boot`, before baz starts, where answering it yes
        // costs nothing (ADR-0043 §5). Nothing about somebody's collection
        // has to move aside for it any more.
        let screen: Element<'_, Message> = column![bar, screen, marks].into();
        let screen: Element<'_, Message> = match lane {
            Some(lane) => row![lane, screen].into(),
            None => screen,
        };
        // The GUI is always an audio build, so the persistent bottom bar lives
        // under every place. A missing device is represented in the bar rather
        // than by changing the application's composition.
        let standing = self.bar_standing();
        let bottom: Element<'_, Message> = if self.chromeless {
            // The same rule as the bar above: the slot stays, the tenant
            // leaves.
            iced::widget::Space::new()
                .width(iced::Length::Fill)
                .height(0.0)
                .into()
        } else {
            views::bottom_bar::view(
                &self.player,
                ink,
                self.bar_cover(),
                self.now_playing_source()
                    .map(|_| Message::OpenPlayingSource),
                self.window.width,
                standing.as_ref(),
                // The same reading Now playing's title line takes: the
                // sounding file, and whether the library holds it as a
                // favourite. `None` is a sounding file with no library row,
                // which cannot be favourited at all — the bar keeps the slot
                // and draws the action inert rather than dropping it, because
                // a slot that came and went would move the title lane beside
                // it, which is the one thing this bar may not do.
                // **The heart follows the subject**, sounding or standing:
                // a track the bar names is a track you can keep.
                self.player
                    .now_playing()
                    .filter(|now| now.album_id.is_some())
                    .and_then(|_| self.player.now_playing_path())
                    .or_else(|| {
                        standing
                            .as_ref()
                            .filter(|now| now.album_id.is_some())
                            .and_then(|_| {
                                views::home::standing(&self.player, &self.resume)
                                    .map(|(path, _)| path)
                            })
                    })
                    .map(|path| (path, is_favourite(state, path))),
            )
        };
        let whole: Element<'_, Message> = column![screen, bottom].into();
        // **The record's wash runs behind everything, the bar included.**
        //
        // On Now playing the field and the spectrum are the window's backdrop
        // rather than the page's, so the glass bar has something under it —
        // see `views::now_playing::backdrop` for the version of this that
        // stacked the *bar* over the page instead and pushed the lane off the
        // top of the window.
        let whole: Element<'_, Message> = if over_field {
            let Screen::Shelf(state) = &self.screen else {
                unreachable!("over_field is a shelf place")
            };
            let audio = self
                .visualization
                .mode
                .active()
                .then(|| self.playback.visualization())
                .flatten();
            iced::widget::stack![
                views::now_playing::backdrop(
                    state,
                    &self.player,
                    views::now_playing::Visual {
                        rotation: self.case_rotation,
                        foreground: self.visualization.foreground,
                        mode: self.visualization.mode,
                        audio: audio.as_ref(),
                        history: &self.visualization_history,
                        favourite: None,
                    },
                    self.window,
                    self.place != Place::NowPlaying,
                ),
                whole,
            ]
            .into()
        } else {
            whole
        };
        // **Every floating layer is stacked always**, empty at rest.
        //
        // This is the drag ghost's rule below, applied to the three layers
        // that predate it — and it is a fix rather than a tidy-up. iced diffs
        // the widget tree by position, so a stack level that appears only
        // when a layer opens moves every widget under it one level down at
        // that moment, and each of them is handed a fresh state. The owner
        // found it as *"right clicking in a playlist seems to reset scroll
        // position"* (`docs/WORK.md` item 81): the scrollable was not
        // scrolled back, it was replaced by a new one that had never been
        // scrolled at all. Search and the status panel did the same thing to
        // whatever was under them.
        let whole: Element<'_, Message> = iced::widget::stack![
            whole,
            match &self.screen {
                Screen::Shelf(state) if state.search_open => views::search::layer(
                    state,
                    &self.player,
                    self.window,
                    matches!(self.place, Place::Playlist(_) | Place::NewPlaylist),
                ),
                Screen::Shelf(_) | Screen::Setup(_) | Screen::Blocked(_) => nothing(),
            },
        ]
        .into();
        let whole: Element<'_, Message> = iced::widget::stack![
            whole,
            match &self.screen {
                Screen::Shelf(state) if self.status_open =>
                    views::status::layer(&state.health, self.health_summary(), self.window),
                Screen::Shelf(_) | Screen::Setup(_) | Screen::Blocked(_) => nothing(),
            },
        ]
        .into();
        // **The shortcuts card**, over everything else it could be covering —
        // it is opened when a listener does not know where they are, so the
        // one thing it must never be is underneath.
        let whole: Element<'_, Message> = iced::widget::stack![
            whole,
            if self.shortcuts_open {
                views::shortcuts::layer(self.window)
            } else {
                nothing()
            },
        ]
        .into();
        let whole: Element<'_, Message> = iced::widget::stack![
            whole,
            if self.equalizer_open {
                views::equalizer::layer(
                    self.equalizer,
                    &self.equalizer_presets,
                    self.equalizer_editing,
                    self.equalizer_naming.as_deref(),
                    self.window,
                )
            } else {
                nothing()
            },
        ]
        .into();
        // **The context menu** (doc 09 §5.2), floated at the pointer by the
        // same ADR-0016 mechanics as the panel — but stacked over the *whole
        // window*, bar included, because the bar's own now-playing menu
        // opens over the bar. Under the card sits a full-window backdrop
        // whose left press puts the menu down; a right press falls through
        // it to whatever row is beneath, whose own `menu::area` replaces
        // the menu — one at a time by construction. Wheel travel passes
        // beside both, and nothing reflows by a pixel: layers, not columns.
        let whole: Element<'_, Message> = iced::widget::stack![
            whole,
            match &self.menu {
                Some(open) if matches!(self.screen, Screen::Shelf(_)) =>
                    views::context_menu::layer(open, self.window),
                _ => nothing(),
            },
        ]
        .into();
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
            None => nothing(),
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
            // **Chromeless has no lane to take off.** The owner: *"when in full
            // screen mode there is a black gap at the right hand side"*, then
            // *"the visualisation just isn't being drawn at the full size of
            // the window."* Both are this: the composition hid the lane and
            // this went on subtracting it, so every place that sizes itself
            // against the body — the field, the spectrum, the record's own
            // column — was drawn a lane's width short of the glass.
            Screen::Shelf(state) if self.place.wears_lane() && !self.chromeless => {
                state.body_width()
            }
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
    /// **Start measuring**, spawning the service if this is the first ask.
    ///
    /// A failure to spawn is reported where every other background failure is
    /// — the bell's health log — rather than swallowed: a press that appears
    /// to do nothing is worse than one that says why.
    fn measure_loudness(&mut self, redo: bool) {
        if self.loudness.is_none() {
            let Some(db_path) = config::library_db_file() else {
                return;
            };
            match baz_core::analysis::spawn(db_path) {
                Ok((handle, events)) => {
                    self.loudness = Some(handle);
                    self.loudness_events = Some(events);
                }
                Err(error) => {
                    if let Screen::Shelf(state) = &mut self.screen {
                        state.health.record(
                            crate::health::Level::Error,
                            "Could not measure loudness",
                            format!("The measurement service would not start: {error}"),
                        );
                    }
                    return;
                }
            }
        }
        if let Some(loudness) = &self.loudness {
            let _ = loudness
                .send(baz_core::protocol::AnalysisCommand::StartReplayGainAnalysis { redo });
            // Read straight back so the readout says *running* on this frame
            // rather than on the tick after it — the press must look answered.
            self.loudness_progress = loudness.progress();
        }
    }

    /// Empty the pass's channel and take its latest counts — see
    /// [`Self::loudness_events`] for why the events are dropped.
    fn read_loudness(&mut self) {
        if let Some(events) = &self.loudness_events {
            while events.try_recv().is_ok() {}
        }
        if let Some(loudness) = &self.loudness {
            self.loudness_progress = loudness.progress();
        }
    }

    fn body_height(&self) -> f32 {
        // The bottom bar's band and its hairline are gone with the frame; the
        // app bar's strip is not, because chromeless keeps a shorter one for
        // the window's own controls.
        let below = if self.chromeless {
            0.0
        } else {
            theme::BAR_CONTENT_H + 1.0
        };
        (self.window.height - theme::APP_BAR_H - below).max(0.0)
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
        if let Some(every) = visualization_clock(
            self.place,
            self.player.now_playing().is_some(),
            self.visualization,
        ) {
            subs.push(iced::time::every(every).map(|_| Message::CaseTick(Instant::now())));
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
        // **The sleep timer's clock runs only while it is armed**, which is
        // the same rule the resource meter's and the visualizer's follow: a
        // second-by-second wake-up that exists when nothing is scheduled is a
        // cost with no reader.
        if self.sleep.is_some() {
            subs.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::SleepTimerTick));
        }
        // **Only while a pass is running**, the same rule as every other clock
        // here. Twice a second rather than per event: the service emits one
        // event per measured track, a few per second, and a readout that
        // repainted on each of them would be spending frames to move a number
        // nobody can read at that rate.
        if self.loudness_progress.running {
            subs.push(iced::time::every(Duration::from_millis(500)).map(|_| Message::LoudnessTick));
        }
        if let Screen::Shelf(state) = &self.screen {
            if state.scanning {
                subs.push(iced::time::every(Duration::from_millis(100)).map(|_| Message::ScanTick));
            } else {
                subs.push(iced::time::every(REFRESH_TICK).map(|_| Message::RefreshTick));
            }
            // **The live count's clock runs only while a phrase is settling.**
            // Same rule as the sleep timer and the resource meter: a wake-up
            // with nothing to answer is a cost with no reader, and the words
            // are still the vast majority of the time.
            if state.vibe.awaiting_count() {
                subs.push(
                    iced::time::every(Duration::from_millis(120)).map(|_| Message::VibeCountTick),
                );
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let events = if self.menu.is_some() {
            iced::event::listen_with(menu_event_message)
        } else if matches!(&self.screen, Screen::Shelf(state) if state.search_open) {
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

/// **Leave a verified installer where `baz-boot` will find it.**
///
/// Blocking, and the whole of what baz's background pass does. ADR-0043 §5.
///
/// **A stage that is already good is left alone**, which is what stops the
/// second launch after a release from downloading 190 MB again. The check is
/// the full one — newer than this baz, and the file still matching the digest
/// it was staged with — because a half-written stage from a session that was
/// killed mid-download must be replaced rather than trusted.
///
/// `Ok(None)` is the ordinary answer and means *nothing to do*.
fn stage_an_update() -> Result<Option<String>, String> {
    if let Some(pending) = baz_update::stage::pending()
        && baz_update::is_newer(&pending.version, env!("CARGO_PKG_VERSION"))
        && baz_update::stage::ready(&pending).is_some()
    {
        return Ok(Some(pending.version));
    }
    let Some(update) = baz_update::check()? else {
        return Ok(None);
    };
    let version = update.version.clone();
    baz_update::fetch_verified(&update)?;
    Ok(Some(version))
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
        mpris::Request::SetShuffle(on) => Message::SetShuffle(on),
        mpris::Request::SetRepeat(repeat) => Message::SetRepeat(repeat),
        mpris::Request::Raise => Message::Raise,
        mpris::Request::Quit => Message::Quit,
    }
}

pub(crate) fn shifted_index(len: usize, index: usize, delta: i8) -> Option<usize> {
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
pub(crate) fn folder_refusal(roots: &[PathBuf], dir: &Path) -> Option<String> {
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
pub(crate) async fn check_folder(dir: PathBuf) -> Result<PathBuf, String> {
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
pub(crate) fn pick_folder() -> Task<Message> {
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

/// **Choose a playlist's sleeve** — the second thing in the product that opens
/// the platform's file dialog, and it follows [`pick_folder`]'s rule exactly:
/// the call blocks until the listener decides, so it runs on the blocking pool
/// and the event loop never waits on a human.
///
/// The filter names the extensions the storage layer will actually store
/// (`baz_core::playlist::IMAGE_EXTENSIONS`); a listener who defeats the filter
/// gets the same refusal from `Folder::set_image`, which is where the rule
/// lives.
fn pick_playlist_image(id: u64) -> Task<Message> {
    Task::perform(
        async move {
            match tokio::task::spawn_blocking(|| {
                rfd::FileDialog::new()
                    .add_filter("Picture", &baz_core::playlist::IMAGE_EXTENSIONS)
                    .pick_file()
            })
            .await
            {
                Ok(choice) => choice,
                Err(error) => {
                    crate::baz_log!("[playlists] image picker failed: {error}");
                    None
                }
            }
        },
        move |choice| Message::PlaylistImagePicked(id, choice),
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

/// The moment now, in nanoseconds since the Unix epoch — what the Settings
/// place measures a folder's last scan against.
///
/// Saturating rather than panicking on an absurd clock, exactly as the index's
/// own first-seen stamp is.
pub(crate) fn now_ns() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => {
            i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, i64::saturating_neg)
        }
    }
}

/// Remember how the wall is arranged (ADR-0017 §1.3: view *state*, persisted,
/// not a preference anybody goes anywhere to set).
pub(crate) fn persist_group_key(key: GroupKey) {
    persist(|config| config.group_key = key);
}

/// Remember how closely it hangs — the same terms exactly (ADR-0017 §1.3).
///
/// The zoom is a *gesture*; where it landed is state. A listener who pressed
/// <kbd>Ctrl</kbd>+<kbd>-</kbd> twice expects that wall next time, and had to
/// go nowhere to ask for it.
pub(crate) fn persist_density(density: shelf::Density) {
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

/// **Where a dropped file lands** — see [`App::drop_lands`].
#[derive(Debug, Clone, PartialEq, Eq)]
enum DropTo {
    /// Behind whatever is playing, which is what a drop means everywhere the
    /// listener is not plainly building a list.
    Run,
    /// The open playlist's file, by id and name.
    Playlist(u64, String),
    /// The draft on the New playlist place.
    Draft,
}

impl DropTo {
    /// What the strip says while a drag is over the window.
    fn hint(&self) -> String {
        match self {
            Self::Run => "Drop to play it after this".to_owned(),
            Self::Playlist(_, name) => format!("Drop to add to \u{201c}{name}\u{201d}"),
            Self::Draft => "Drop to add to this list".to_owned(),
        }
    }
}

/// The transport, needle, fader and ReplayGain sub-machine — every message
/// that resolves to one engine command and moves nothing on screen. Step 2
/// of `BACKLOG.md`'s `app.rs` proposal: the `update` half splits by
/// sub-machine along the seams `route` already delegates along, one pure
/// move per commit.
mod transport;

#[cfg(test)]
mod drop_destination {
    use super::DropTo;

    /// **Three destinations, three sentences, and each names what it will do.**
    ///
    /// The hint is the whole of the feedback a drag gets (`views::marks::hint`),
    /// so a sentence that did not distinguish *play this after what is on* from
    /// *add this to Road Trip* would leave a listener guessing at the moment
    /// they can still let go somewhere else.
    #[test]
    fn every_destination_says_what_it_will_do() {
        let run = DropTo::Run.hint();
        let draft = DropTo::Draft.hint();
        let list = DropTo::Playlist(1, "Road Trip".to_owned()).hint();
        assert!(list.contains("Road Trip"), "{list}");
        assert!(run != draft && draft != list && run != list);
        for words in [&run, &draft, &list] {
            assert!(words.starts_with("Drop to "), "{words}");
        }
    }

    /// **A playlist's name travels with its id**, because the sentence names
    /// the list and an id names nothing a listener has seen.
    #[test]
    fn the_playlist_hint_is_about_a_playlist_and_not_an_id() {
        let hint = DropTo::Playlist(99, "Sunday Morning".to_owned()).hint();
        assert!(!hint.contains("99"), "{hint}");
        assert!(hint.contains("Sunday Morning"), "{hint}");
    }
}

/// Remember the radio-like foreground choice on the same view-state footing
/// as density: its control lives on Now Playing and its result survives it.
fn persist_visualization_foreground(foreground: crate::visualizer::Foreground) {
    persist(|config| config.visualization_foreground = foreground);
}

/// **How often the visualisation's clock ticks here**, or `None` for no clock.
///
/// Focus is intentionally not an input. A sounding record and a visual that
/// actually changes are the cost gate; what changed on 2026-08-22 is that
/// *place* is no longer part of it, only the rate.
///
/// The owner: *"can you make sure when we switch to other screens and the
/// visualizer stays in the background that it continues animating"*. It did
/// not — the backdrop was drawn everywhere and frozen everywhere but one
/// place, which is why it read as a still.
///
/// **The veil is what buys the cheaper clock.** Away from Now playing the
/// backdrop is one soft ground behind a frost pane
/// (`views::now_playing::backdrop`), and nothing about it is legible at
/// thirty frames that is not legible at ten. So the cadence there is
/// [`GLASS_TICK`], which is a third of the wake-ups for a picture nobody can
/// tell apart — and `Mode::Off` still means no clock at all, anywhere.
fn visualization_clock(
    place: Place,
    sounding: bool,
    visualization: crate::visualizer::State,
) -> Option<Duration> {
    if !sounding {
        return None;
    }
    if place == Place::NowPlaying {
        return (visualization.mode.active() || visualization.foreground.draws_case())
            .then_some(crate::jewel_case::TICK);
    }
    visualization.mode.active().then_some(GLASS_TICK)
}

/// The cadence of the veiled backdrop away from Now playing — see
/// [`visualization_clock`].
const GLASS_TICK: Duration = Duration::from_millis(100);

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
pub(crate) fn hero_target(
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

pub(crate) fn read_history() -> Option<History> {
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
pub(crate) fn persist(change: impl FnOnce(&mut config::Config)) {
    let Some(path) = config::config_file() else {
        crate::baz_log!("[config] no config directory on this system; nothing is being remembered");
        return;
    };
    // **A document baz could not read is not a document baz may overwrite.**
    //
    // `config::load` answers a parse failure with the defaults, which is right
    // for reading — one bad key must not stop baz starting. It was wrong here:
    // this read the defaults, applied one field, and wrote the complete
    // document back, so a single unbalanced quote plus any later volume nudge
    // cost a listener their folders, theme, equaliser curves, Vibe curves,
    // crossfade and last place. `persist` fires from about twenty sites,
    // including `last_place` on every clean quit, so the window was every
    // session. Audit finding 2, 2026-08-23.
    //
    // Refusing is the whole fix: the file stays exactly as the listener left
    // it, and it is the only copy of what they chose. Settings not being
    // remembered for a session is a smaller loss than settings being deleted,
    // and the log line says which key to go and look at.
    if let Err(why) = config::readable(&path) {
        crate::baz_log!(
            "[config] {} could not be read ({why}); nothing will be written over it \
             until it parses — this session's changes are not being remembered",
            path.display()
        );
        return;
    }
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

/// Expand a leading `~/` (or bare `~`) via the home directory, so the setup
/// input accepts what people actually type.
pub(crate) fn expand_tilde(input: &str) -> PathBuf {
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
    /// **The shell's own source — both files of it.**
    ///
    /// `app.rs` held the whole of ADR-0006's layer 2 until 2026-09-02, when
    /// [`crate::collection::Shelf`] and its 2 888 lines left for a file of
    /// their own. Every scan below that looks for `fn grid_width`,
    /// `fn clear_query` or `fn refilter` is asking about *the shell*, which is
    /// now two modules — and a scan that read only this one would not fail,
    /// it would `expect` its way to a panic or, worse, find a shorter function
    /// with the same prefix somewhere else.
    ///
    /// Concatenated rather than searched in turn so that every `split_once`
    /// below reads exactly as it did before the move.
    ///
    /// **Through [`crate::shipped::code`], and that is not tidiness.** These
    /// scans split on a signature and read to the next `\n    }\n`, and this
    /// module's own tests hold those signatures as string literals. With the
    /// shipped code of both files the real function is the first match, as it
    /// always was; with the test modules left in, `fn grid_width(&self) ->
    /// f32 {` matched the literal in `a_place_costs_the_wall_no_width_at_all`
    /// and the "body" that came back was the rest of that test — including its
    /// own list of banned words, so it failed against itself.
    fn shell_source() -> String {
        format!(
            "{}\n{}",
            crate::shipped::code(include_str!("app.rs")),
            crate::shipped::code(include_str!("collection.rs")),
        )
    }

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
                chromeless: false,
                foreground,
                mode: crate::visualizer::Mode::Off,
                facts: false,
            };
            let spectral = crate::visualizer::State {
                mode: crate::visualizer::Mode::Spectrum,
                ..still
            };
            assert_eq!(
                visualization_clock(Place::NowPlaying, true, still).is_some(),
                foreground.draws_case(),
                "{foreground:?} without spectrum"
            );
            assert_eq!(
                visualization_clock(Place::NowPlaying, true, spectral),
                Some(crate::jewel_case::TICK),
                "{foreground:?} with spectrum"
            );
            // **Away from the place it keeps moving, and more slowly.** The
            // owner asked for the background to go on animating everywhere;
            // the veil over it (`views::now_playing::backdrop`) is what makes
            // a third of the wake-ups an invisible saving rather than a
            // visible one.
            assert_eq!(
                visualization_clock(Place::Library, true, spectral),
                Some(GLASS_TICK),
                "the backdrop froze away from Now playing"
            );
            assert!(GLASS_TICK > crate::jewel_case::TICK);
            // **Off is off, everywhere.** A listener who turned the
            // visualisation off did not ask for a cheaper one.
            assert_eq!(visualization_clock(Place::Library, true, still), None);
            assert_eq!(
                visualization_clock(Place::NowPlaying, false, spectral),
                None
            );
            assert_eq!(visualization_clock(Place::Library, false, spectral), None);
        }
    }

    #[test]
    fn focus_is_not_part_of_the_visible_visual_clock() {
        let state = crate::visualizer::State {
            chromeless: false,
            foreground: crate::visualizer::Foreground::None,
            mode: crate::visualizer::Mode::Waveform,
            facts: false,
        };
        // There is deliberately no focus argument: a visible Now Playing
        // remains live while another application owns the keyboard.
        assert!(visualization_clock(Place::NowPlaying, true, state).is_some());
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
        let source = shell_source();
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

    /// The pointer route to a keyboard binding's intention, in the one form a
    /// test can actually hold.
    ///
    /// The `CONTROLS` table in
    /// [`every_keyboard_binding_is_a_press_some_control_also_makes`] used to be
    /// prose alone, and prose naming a door that was deleted
    /// two releases ago reads exactly like prose naming one that is still
    /// there. It happened: the `TogglePlaylists` row named "the Library
    /// strip's labelled `Playlists` door" for months after 44f2b76 removed
    /// that strip in favour of the returns lane.
    #[derive(Clone, Copy)]
    enum Pointer {
        /// A control in the view layer sends this message. Checked against
        /// the source of every module that builds controls, so deleting the
        /// control turns the claim red.
        Sends(&'static str),
        /// Not machine-checked, and the exact set of these is pinned below so
        /// that a new one is a deliberate edit rather than a quiet one.
        Prose,
    }

    /// Every module that builds controls, with comments and test modules
    /// removed, so that a `Message::` in a doc line or a unit test cannot
    /// stand in for a control on screen.
    ///
    /// `app.rs` is excluded because it *defines* and *handles* every message —
    /// including it would let this check pass on the strength of the very
    /// thing it is checking — and `keys.rs` because it is the keyboard, which
    /// is the side of the mirror being verified.
    ///
    /// Two limits, stated rather than left to be discovered. A control whose
    /// message is *handed to it* by `app.rs` — the now-playing block takes its
    /// door as an argument — is invisible here, so no row may claim one. And
    /// this reads source at all, which is the weakest kind of assertion; it is
    /// here only until there is a headless `App` to draw and interrogate, which
    /// would answer "does a control send this" directly and close both gaps.
    fn control_source() -> String {
        let mut source = String::new();
        let mut stack = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("the crate's own src is readable") {
                let path = entry.expect("a readable directory entry").path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let name = path
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .unwrap_or_default();
                let is_rust = path.extension().is_some_and(|ext| ext == "rs");
                if !is_rust || matches!(name, "app.rs" | "keys.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("a readable module");
                // Shipped code, comments removed — `crate::shipped` says why
                // both halves matter and carries the tests for them.
                source.push_str(&crate::shipped::code(&text));
                source.push('\n');
            }
        }
        source
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

        /// Message tag → the pointer route to the same intention, then the
        /// prose naming the control. The middle field is the half a test can
        /// hold: prose that names a deleted door reads exactly like prose
        /// that names a live one, and that is how this table came to claim a
        /// `Playlists` word the returns lane removed in 44f2b76.
        const CONTROLS: [(&str, Pointer, &str); 24] = [
            (
                "ToggleLane",
                Pointer::Sends("ToggleLane"),
                "the `Collapse` control at the returns lane's foot (ADR-0030 §3) — \
                 the state you are in at full ink and inert, the other \
                 pressable, in the density detents' exact anatomy",
            ),
            (
                "Undo",
                Pointer::Sends("Undo"),
                "the transient `Undo` word beside the Queue place's summary \
                 and the playlist page's counts (doc 11 §5 P2) — present \
                 exactly while there is an edit to take back, which is \
                 exactly when the chord acts",
            ),
            (
                "ToggleShortcuts",
                Pointer::Sends("ToggleShortcuts"),
                "Settings' `Show shortcuts` word — the card `?` opens, given a \
                 visible door because a key that is the only way to a card \
                 about keys reaches exactly the people who did not need it",
            ),
            (
                "PlayPause",
                Pointer::Sends("PlayPause"),
                "the bottom bar's play/pause button",
            ),
            (
                "TogglePlaylists",
                Pointer::Sends("AddAlbumToPlaylist"),
                "`Add to playlist`, on a record's context menu and its page — \
                 which summons this same panel as the picker (09 §8.1) and \
                 leaves it standing afterwards. **That is a weaker equivalence \
                 than every other row here and is written down rather than \
                 rounded up**: the pointer can only summon the panel *into a \
                 pick*, and the Library strip's `Playlists` word this row named \
                 until 2026-08-24 has not existed since 44f2b76 replaced the \
                 strip with the returns lane. The lane's `Playlists` row is a \
                 door to the *place*, which is a different surface. So at rest \
                 this chord is the only summon, and `drop_drag`'s `over_panel` \
                 branch — dragging a record from the wall onto a list — needs \
                 the panel already open. Backlog #8 carries the decision",
            ),
            (
                "NextTrack",
                Pointer::Sends("NextTrack"),
                "the bottom bar's Next button",
            ),
            (
                "PreviousTrack",
                Pointer::Sends("PreviousTrack"),
                "the bottom bar's Previous button",
            ),
            (
                "Play",
                Pointer::Sends("PlayPause"),
                "MPRIS only; the bar's toggle covers both directions",
            ),
            (
                "Pause",
                Pointer::Sends("PlayPause"),
                "MPRIS only; the bar's toggle covers both directions",
            ),
            (
                "Stop",
                Pointer::Prose,
                "MPRIS only; there is no on-screen Stop",
            ),
            (
                "SeekBy",
                Pointer::Sends("NeedleDragged"),
                "the needle, pressed inside the entry that is sounding \
                 (ADR-0017 §1.1: the groove's job, at the window's edge)",
            ),
            (
                "Direction",
                Pointer::Sends("SearchAction"),
                "the selected row/action in the open search chooser; outside \
                 search, the bottom bar's needle and volume fader",
            ),
            (
                "ToggleMute",
                Pointer::Sends("ToggleMute"),
                "the bottom bar's speaker button",
            ),
            (
                "ShowNowPlaying",
                Pointer::Sends("GoTo"),
                "the returns lane's labelled `Now playing` row",
            ),
            (
                "ToggleSettings",
                Pointer::Sends("ToggleSettings"),
                "the top bar's Settings control",
            ),
            (
                "HistoryBack",
                Pointer::Sends("HistoryBack"),
                "the app bar's visible Back arrow",
            ),
            (
                "HistoryForward",
                Pointer::Sends("HistoryForward"),
                "the app bar's visible Forward arrow",
            ),
            ("FocusSearch", Pointer::Prose, "the top bar's search well"),
            (
                "EscapePressed",
                Pointer::Sends("DismissSearch"),
                "every place's `‹ Library`, and — for the query layer the peel \
                 ends on — the well's own clear mark, which is this key's \
                 pointer route into the identical function (ADR-0036 §4)",
            ),
            (
                "QueryTyped",
                Pointer::Sends("SearchChanged"),
                "the top bar's search well — the field ADR-0017 §1.2 kept, \
                 which a pointer clicks into to type the same query",
            ),
            (
                "PlayFirstMatch",
                Pointer::Sends("SearchConfirmed"),
                "the selected app-bar search result while its chooser stands; \
                 the record page's `Play album` for the fall-through",
            ),
            (
                "DensityStep",
                Pointer::Sends("DensityStep"),
                "the density marks — at the foot of the index rail's lane on \
                 the Library, and on the block's own section rule on Home and \
                 an artist's page (ADR-0028 and its fourth-step amendment). \
                 Each sends this message with the exact delta the gesture \
                 would spend, so Ctrl+scroll and Ctrl+-/= are accelerators of \
                 a visible control now, not the control itself",
            ),
            (
                "GroupKeySelected",
                Pointer::Sends("GroupKeySelected"),
                "the top bar's row of six words (ADR-0019); the first two, \
                 A–Z and ARTIST, are the same order broken into letter \
                 shelves and into a shelf per artist (ADR-0035, as thrice \
                 amended)",
            ),
            (
                "SetVolume",
                Pointer::Sends("VolumeDragged"),
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
                    CONTROLS.iter().any(|(name, _, _)| *name == tag),
                    "{key} + {modifiers:?} binds to `{tag}`, which no entry in \
                     CONTROLS accounts for — name the control that sends it, or \
                     record why there is none"
                );
                produced.push(tag);
            }
        }
        // …and the table has no stale entries either, except the three that
        // exist for the desktop rather than for the keyboard.
        for (tag, _, _) in CONTROLS {
            let desktop_only = matches!(tag, "Play" | "Pause" | "SetVolume");
            assert!(
                desktop_only || produced.contains(&tag.to_owned()),
                "CONTROLS still names `{tag}`, which no key produces any more"
            );
        }
        assert!(produced.len() > 20, "the sweep stopped covering the table");

        // **And the pointer half, which until now was only prose.** Every
        // route the table claims has to be a message some control really
        // sends; a door removed in a refactor fails here instead of quietly
        // leaving a binding with nothing behind it.
        let controls = control_source();
        for (tag, pointer, description) in CONTROLS {
            let Pointer::Sends(sent) = pointer else {
                continue;
            };
            assert!(
                controls.contains(&format!("Message::{sent}")),
                "`{tag}`'s pointer route is `Message::{sent}` — \"{description}\" — \
                 but no module outside app.rs and keys.rs sends it. Either the \
                 control was removed, in which case the binding has no pointer \
                 route and this table must say so, or it was renamed and this \
                 row is stale."
            );
        }

        // The exceptions, named. Two bindings have no message-sending control
        // behind them and both are deliberate; pinning the set is what stops a
        // third being added by writing `Pointer::Prose` and moving on.
        let unchecked: Vec<&str> = CONTROLS
            .iter()
            .filter(|(_, pointer, _)| matches!(pointer, Pointer::Prose))
            .map(|(tag, _, _)| *tag)
            .collect();
        assert_eq!(
            unchecked,
            ["Stop", "FocusSearch"],
            "the set of bindings whose pointer route is unverified prose has \
             changed. `Stop` has no on-screen Stop at all; `FocusSearch` has a \
             control — the search well — that a pointer focuses by clicking, \
             which iced does without sending a message. A new entry here is a \
             new keyboard-only capability and needs to be argued, not added."
        );
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
            // Through `start_and_show`, which the assertion below holds to the
            // arranger in turn. Two hops rather than one because the tail now
            // shares the confirmation boundary with the album and the playlist
            // — that is the point of it, not a layer to see past.
            body("start").contains("self.start_and_show("),
            "the shared tail stopped going through the confirmed start"
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
        //
        // Read from `set_shuffle` rather than `toggle_shuffle` since
        // 2026-08-18: MPRIS's `Shuffle` is a property and needed a stated
        // value, so the toggle is now written in terms of the setter and the
        // setter is where the rule lives. The toggle is asserted to be exactly
        // that delegation below, so there is still only one path.
        let toggle = body("set_shuffle");
        assert!(toggle.contains("Command::SetTraversal"));
        assert!(
            !toggle.contains("Command::SetQueue")
                && !toggle.contains("Command::UpdateQueue")
                && !toggle.contains("Command::Play"),
            "the toggle touched the queue instead of the walk"
        );
        // …and the toggle is nothing but the setter, so a press and a protocol
        // write cannot drift apart.
        let delegate = body("toggle_shuffle");
        assert!(
            delegate.contains("self.set_shuffle(!self.player.shuffle())"),
            "the toggle grew a second path to shuffle"
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

    /// **Every whole-list Play shows Now playing**, and it took the owner to
    /// notice one that did not: *"when the play button is pressed for a
    /// playlist it does not go to the now playing screen."*
    ///
    /// A separate test from `every_play_gesture_arranges_its_run_through_one_function`
    /// because it is a different question about the same list of gestures.
    /// That one asks whether the run goes through the arranger —
    /// `play_playlist` satisfied it while carrying its own copy of
    /// `start_and_show`'s four lines minus the one that changes the place.
    /// Two others were the same: `play_favourites` with no lead, and the
    /// `All songs` tail.
    #[test]
    fn a_whole_list_play_shows_now_playing_and_a_row_press_does_not() {
        // Normalised the same way its sibling normalises, and for the same
        // reason: every scan below matches on "\n    }\n", which a CRLF file
        // never contains.
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let body = |name: &str| {
            let at = source
                .find(&format!("fn {name}("))
                .unwrap_or_else(|| panic!("`{name}` is gone"));
            let tail = &source[at..];
            let end = tail.find("\n    }\n").expect("the function's end");
            tail[..end].to_owned()
        };

        for gesture in ["play_album", "play_playlist", "start"] {
            assert!(
                body(gesture).contains("self.start_and_show("),
                "`{gesture}` is a whole-list Play that does not show Now \
                 playing — it has its own copy of the tail, or it forgot the \
                 line that changes the place"
            );
        }
        assert!(
            body("play_favourites").contains("self.start_and_show(queue)"),
            "the Favourites Play button no longer shows Now playing"
        );

        // **A row press is deliberately not on that list.** Pressing `Play` on
        // a list is a decision to listen to it; pressing one of its rows is a
        // decision made *while browsing*, and taking the reader away from the
        // list they are reading would answer a question they did not ask.
        for gesture in ["play_track", "play_playlist_track"] {
            assert!(
                !body(gesture).contains("self.start_and_show("),
                "`{gesture}` is a row press and has started navigating away \
                 from the list the listener is reading"
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
        let code = crate::shipped::code(&source);
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
            // `start_and_show` is where both halves live now — the arranger
            // and the `Play` — so the tail names it instead of spelling them.
            // The gesture is unchanged: one press, and the first track sounds.
            tail.contains("self.start_and_show("),
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
        let code = crate::shipped::code(&source);
        assert!(code.contains("|config| config.volume"));
        assert!(code.contains("position: saved_volume.position()"));
        assert!(code.contains("matches!(&event, Event::VolumeChanged { .. })"));
        assert!(code.contains("self.persist_volume()"));

        // The fader's own machine lives in `app/transport.rs` since the
        // 2026-09-03 split; the seam and the wheel arm are read from there.
        let transport = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/transport.rs"),
        )
        .expect("the transport source")
        .replace("\r\n", "\n");
        let code = crate::shipped::code(&transport);
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
    /// **A place that leaves work behind has that work awaited.**
    ///
    /// `note_place_left` answered `Task::none()` for its whole life, so two
    /// call sites wrote `let _ =` and reasoned, correctly at the time, that
    /// there was nothing to lose. Then leaving the composing place started
    /// releasing the Vibe text tower — 370 MiB and an `malloc_trim` — and a
    /// discarded task became a silently skipped release.
    ///
    /// It cost a measurement run to notice, because the failure is invisible:
    /// the memory is simply still there, which is what it looked like before.
    /// The two existing sites are safe (both leave a playlist place, never the
    /// composing one) and documented as such; this pins the count so a third
    /// has to be argued for rather than typed.
    #[test]
    fn every_place_that_leaves_work_behind_is_awaited() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("app.rs is this file");
        // **`head` and not `code`**: the last assertion in this test requires
        // a *comment* to be present — the sentence saying why a discarding
        // call site is safe — so this is one of the two scans in the crate
        // that read prose on purpose.
        let shipped = crate::shipped::head(&source);
        let discarded = shipped.matches("let _ = self.note_place_left").count();
        assert_eq!(
            discarded, 2,
            "there are {discarded} call sites discarding `note_place_left`'s \
             task, not the two documented ones. Leaving the composing place \
             releases the Vibe text tower through that task, so a site that \
             drops it drops the release — and nothing fails, the memory is \
             just still held. Return the task, or say in a comment why this \
             `from` can never be the composing place."
        );
        assert!(
            shipped.contains("never the composing place"),
            "the discarding call sites have lost the comment explaining why \
             they are safe"
        );
    }

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
        let source = shell_source();
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
        let source = shell_source();
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

    /// **The sleep timer pauses, and pauses only once.**
    ///
    /// Pausing is the one ending that keeps the run, the position and the
    /// queue where they are, so the next press carries on rather than
    /// starting over — and the timer clears itself, because a timer that
    /// fired every second after its deadline would pause a listener who had
    /// just pressed play.
    #[test]
    fn the_sleep_timer_pauses_once_and_clears_itself() {
        // **Run the timer instead of reading it.** The forbidden list this
        // replaces — `Command::Stop`, `Message::Quit`, `SetVolume` — was the
        // weakest kind of assertion: it went green the moment any of those
        // moved behind a helper call, and it could only ever name the three
        // wrongs somebody had thought of. Driving a real `App` and comparing
        // the *whole* record of what it asked the engine for is exhaustive by
        // construction, so nothing has to be guessed in advance.
        let (mut app, asks) = App::headless(config::Config::default());
        app.player.note_queue_sent(restored());
        app.player.apply(
            &Event::TrackStarted {
                path: PathBuf::from("/m/2.flac"),
                position: 1,
            },
            &[],
        );
        app.sleep = Some(Sleep {
            minutes: 30,
            fires_at: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .expect("a second ago"),
        });
        asks.lock().expect("the recorder").clear();

        app.tick_sleep_timer();
        assert!(app.sleep.is_none(), "the sleep timer did not disarm itself");
        assert_eq!(
            *asks.lock().expect("the recorder"),
            vec![crate::playback::Ask::Command(Command::Pause)],
            "the sleep timer asked the engine for something other than a pause"
        );

        // And it fires once. A second tick on a disarmed timer is silence,
        // not a pause a listener who just pressed play would feel.
        asks.lock().expect("the recorder").clear();
        app.tick_sleep_timer();
        assert!(
            asks.lock().expect("the recorder").is_empty(),
            "a disarmed sleep timer went off again"
        );
        // Its clock exists only while it is armed — the rule every other
        // per-second wake-up in this file follows.
        //
        // Still read from source, because a `Subscription` cannot be looked
        // inside: there is nothing to interrogate about one but the code that
        // built it. The head is taken at the *test module* rather than at the
        // first `#[cfg(test)]` — this test used to do the latter and broke the
        // day a `#[cfg(test)]` helper was added above the function it wanted,
        // which is the fragility that made these scans a finding in the first
        // place.
        let source = include_str!("app.rs").replace("\r\n", "\n");
        let head = source
            .rsplit_once("#[cfg(test)]\nmod ")
            .map_or(source.as_str(), |(head, _)| head);
        let subs = head
            .split_once("fn add_place_clocks")
            .expect("the clocks")
            .1;
        assert!(
            subs.contains("if self.sleep.is_some() {"),
            "the sleep timer's clock is no longer conditional on its being set"
        );
        // Six choices and an off, which is what a listener can pick from
        // without reading.
        assert_eq!(SLEEP_CHOICES.len(), 6);
        assert!(SLEEP_CHOICES[0].minutes.is_none());
        assert!(
            SLEEP_CHOICES
                .iter()
                .skip(1)
                .all(|choice| choice.minutes.is_some_and(|minutes| minutes > 0)),
        );
    }

    /// **A floating layer is stacked always, and empty at rest.**
    ///
    /// iced diffs the widget tree by position, so a stack level that appears
    /// only when its layer opens moves every widget beneath it one level down
    /// on that frame — and each of them is handed a fresh state. The drag
    /// ghost learned this the hard way (its own note records the ghost
    /// freezing at the lift); the menu, search and status layers kept the
    /// conditional form until the owner found it as *"right clicking in a
    /// playlist seems to reset scroll position"*. The scrollable was not
    /// scrolled back — it was replaced by one that had never been scrolled.
    ///
    /// Pinned at the source because the failure is invisible in a unit test
    /// and costs a whole headless run to see: what it forbids is the shape,
    /// which is exactly what a reader of this file can check.
    #[test]
    fn no_floating_layer_comes_and_goes_from_the_tree() {
        // **`head` and not `code` here**, deliberately: the region this scan
        // reads is delimited by a comment, so stripping the prose would strip
        // the anchor. It is one of two scans in this crate that read comments
        // on purpose — see `every_place_that_leaves_work_behind_is_awaited`
        // for the other — and the other forty moved to `code` on 2026-09-02.
        let source = crate::shipped::head(include_str!("app.rs"));
        let assembly = source
            .split_once("// **Every floating layer is stacked always**")
            .expect("the layer assembly")
            .1;
        let assembly = &assembly[..assembly
            .find("crate::window_frame::resize_frame")
            .expect("the frame the layers go into")];
        let drawn: String = assembly
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !drawn.contains("=> whole"),
            "a layer is falling back to the unstacked tree again, which resets \
             every widget under it"
        );
        // Six layers — search, status, the shortcuts card, the equaliser
        // panel, the context menu, the drag ghost — and every one of them has
        // a resting form.
        assert_eq!(
            drawn.matches("nothing()").count(),
            6,
            "a floating layer has no empty form, or one has been added without one"
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
        let source = crate::shipped::code(include_str!("app.rs"));
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
        // **And it never refuses a request that has no words in it.** The
        // line is the request since design note 25 and `All songs` is the
        // standing choice, so a guard on an empty prompt is a press that does
        // nothing on the page's own default — which is exactly how it was
        // found: *"seems to not calculate a playlist until I change the
        // playlist length option."*
        assert!(
            !drawn.contains("prompt.trim().is_empty()"),
            "Compose is refusing a shape-only request again"
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
        let source = shell_source();
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

        // **The signature this named had four parameters ago**, and the test
        // passed anyway: `split_once` found the literal on this very line,
        // read the rest of *this test* as the function's body, and every
        // assertion below matched the strings written under it. Reading the
        // shell's shipped code — both files of it, tests excluded — is what
        // finally made it say so, on 2026-09-02. Matched on the name alone
        // now, which is the part that cannot silently stop referring to
        // anything.
        let enqueue = body("fn enqueue_search_track(");
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

    /// **The sample tap follows the drawing, not the room.**
    ///
    /// The backdrop became weather on 2026-08-22 and is drawn behind every
    /// place; the clock was fixed that day and the *tap* was not, so away from
    /// Now playing the veiled ground repainted ten times a second from a frame
    /// nobody was filling. The owner, 2026-08-23: *"I think it still sort of
    /// pauses despite being blurred and showing through."*
    ///
    /// A source guard because the failure is invisible to everything else: the
    /// clock ticks, the widget repaints, every frame is well-formed, and the
    /// picture is identical. It cannot be caught by a screenshot either — the
    /// isolated harness routes ALSA to a null device, which consumes a track
    /// as fast as it can read it, so nothing plays in real time to animate.
    #[test]
    fn the_visualization_tap_does_not_depend_on_the_place() {
        // **Asked of a real `App`, not of this file's text.** This used to
        // read `sync_visualization_tap`'s source and assert it contained no
        // `Place::` — which passes just as happily when the gate moves into a
        // helper, and which could never say what the tap actually did. Now
        // every place is visited and the recorded ask is compared.
        let (mut app, asks) = App::headless(config::Config::default());
        app.visualization.mode = crate::visualizer::Mode::Spectrum;
        assert!(
            app.visualization.mode.active(),
            "the fixture picked an inactive mode; the sweep below would prove nothing"
        );
        app.player.note_queue_sent(restored());
        app.player.apply(
            &Event::TrackStarted {
                path: PathBuf::from("/m/2.flac"),
                position: 1,
            },
            &[],
        );

        let places = [
            Place::Library,
            Place::Playlists,
            Place::NewPlaylist,
            Place::Favourites,
            Place::Home,
            Place::NowPlaying,
            Place::Queue,
            Place::Artist(7),
            Place::Album(42),
            Place::Playlist(987_654_321),
            Place::Settings,
        ];
        for place in places {
            app.place = place;
            asks.lock().expect("the recorder").clear();
            app.sync_visualization_tap();
            assert_eq!(
                *asks.lock().expect("the recorder"),
                vec![crate::playback::Ask::Visualization(true)],
                "the tap answered differently in {place:?} — the backdrop is drawn \
                 everywhere, so a place-gated tap buys a frozen frame"
            );
        }

        // And it is still a gate. Nothing sounding is nothing to sample, in
        // every one of those places, so the copy is not paid at all.
        app.player.apply(&Event::Stopped, &[]);
        for place in places {
            app.place = place;
            asks.lock().expect("the recorder").clear();
            app.sync_visualization_tap();
            assert_eq!(
                *asks.lock().expect("the recorder"),
                vec![crate::playback::Ask::Visualization(false)],
                "with nothing playing the tap stayed on in {place:?}"
            );
        }
    }
}
