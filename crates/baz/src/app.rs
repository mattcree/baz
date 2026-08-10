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
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use baz_core::history::{History, HistoryLedger};
use baz_core::index::{GroupKey, Library};
use baz_core::protocol::{self as protocol, Command, Event, SignalChain};
use baz_core::replaygain::ReplayGainSettings;
use iced::keyboard;
use iced::widget::scrollable::{AbsoluteOffset, Viewport};
use iced::widget::{column, image as iced_image, row, scrollable, text_input};
use iced::{Element, Point, Size, Subscription, Task, window};
use lru::LruCache;

use crate::motion::{Control, Ink, Keyed, Tween};
use crate::mpris::Mpris;
use crate::place::Place;
use crate::playback::{Playback, PlayerEvent};
use crate::player::{Availability, PlayerState};
use crate::scan::ScanUpdate;
use crate::{
    art, config, font, keys, menu, motion, mpris, player, queue_edit, scan, shelf, shuffle, theme,
    views, vm,
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
        // **baz closes itself.** iced 0.13 would close the window on the
        // compositor's request before the update loop saw it, and the one
        // thing that has to happen on the way out is writing where the run
        // got to (ADR-0023 §6). The request becomes `Message::Quit` — the
        // same message the desktop's own Quit sends, so there is one exit
        // path and it cannot drift.
        .exit_on_close_request(false)
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
        // The strip's floor **plus the returns lane's rail** is the window's
        // declared minimum width. At 600 the two-line strip holds every
        // tenant (doc 10 §4.3), and below it nothing further collapses —
        // there is no third regime, so the honest move is to not offer the
        // widths the layout does not answer. ADR-0030 puts a 96 px rail
        // permanently to the strip's left and the strip resolves against
        // `Shelf::body_width`, so the *window* has to be that much wider for
        // the same strip to fit: 600 + 96 = **696**. Height is left
        // unbounded; the study declares no floor for it.
        min_size: Some(Size::new(theme::TOP_BAR_FLOOR + theme::SIDEBAR_RAIL_W, 0.0)),
        ..window::Settings::default()
    };
    #[cfg(target_os = "linux")]
    {
        settings.platform_specific.application_id = String::from(mpris::DESKTOP_ENTRY);
    }
    settings
}

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
    println!("[msg] {total}/s  {}", listed.join("  ·  "));
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
    /// <kbd>Enter</kbd>, from the wall or from the well's own submit: while a
    /// query narrows the wall, **needle-drop the top-ranked song** — the
    /// Songs section's own first row, its record queued whole with the cursor
    /// on it (doc 09 §5, ADR-0023 §2's amendment; supersedes the album-level
    /// answer) — else the record the wall was last left for.
    ///
    /// Only defensible because the first match is the best match — ADR-0021
    /// ranks `Library::search` by fit, then field, then library order —
    /// which is why step 12 had to land before step 11 could.
    PlayFirstMatch,
    /// Step the density: a press on one of the three detent marks at the
    /// foot of the index rail's lane (ADR-0028), or its accelerators —
    /// <kbd>Ctrl</kbd>+<kbd>-</kbd> / <kbd>Ctrl</kbd>+<kbd>=</kbd> and
    /// <kbd>Ctrl</kbd>+scroll on the wall. `+1` loosens the hang and `-1`
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
    /// A panel row's name was pressed: open that playlist's page — or come
    /// back from it, when it is the page already showing ([`Place::playlist`]).
    OpenPlaylist(u64),
    /// **The album page's breadcrumb was pressed**: open that artist's page.
    ///
    /// The owner's *"we could add an Artist > album breadcrumb though. and
    /// have an artist page."* Carries [`crate::vm::artist_id`]'s hash rather
    /// than a name, for the reason every other place-opening message carries
    /// an id: a message is a value, and a borrowed name could not outlive the
    /// rescan that rebuilt the wall it came from.
    OpenArtist(u64),
    /// A pick-mode press on a panel row: append what the hand holds to that
    /// playlist's *file* — the run is untouched, whichever list it is, the
    /// playing one included (09 §6's decoupling; S4).
    PickPlaylist(u64),
    /// A pick-mode press on the picker's **Queue** row: append what the hand
    /// holds to the run — `UpdateQueue`, the music keeps playing, and
    /// appending to an empty stopped engine loads a queue without starting
    /// it (09 §8.1; `queue_playlist`'s exact shape).
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
    /// The playlist page's `Queue`: the playable subset appended to the run
    /// ([`Command::UpdateQueue`] — the music keeps playing, ADR-0014).
    PlaylistQueue,
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
    /// The playlist page's `Delete`: the file moves to the platform trash;
    /// the page leaves for the Library. One press, no confirm — the trash is
    /// the safety net the confirm dialog used to stand in for (doc 11 §5
    /// P2: reversibility first; the warning was the fallback and the
    /// fallback is no longer needed).
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
    /// The pointer entered a row of the wall's **Songs** section
    /// (doc 09 §5), so the row can offer its reserved `+` — the album page's
    /// hover mechanism, for the same toolkit reason.
    SongRowEntered(usize),
    /// The pointer left a songs row. Carries which, for the reason
    /// [`Self::QueueRowLeft`] carries which row.
    SongRowLeft(usize),
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
    /// **Play everything the wall shows** — the Library strip's `Play all`
    /// (doc 09 §7.1, S6).
    ///
    /// Reifies the wall's current scope — every visible record, whole, in
    /// the arrangement's own order — into the queue and plays from the top.
    /// The scope is always on screen: a query or a group key narrows it,
    /// "everything in the library" is the empty query, and an empty wall
    /// means nothing happens and nothing is claimed. What **order** it plays
    /// in is the player's shuffle property's answer, the same as every other
    /// play gesture's ([`App::send_run`]).
    PlayAll,
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
    /// it re-arranges the run in progress to match — forward of the needle
    /// only, through [`Command::UpdateQueue`], so nothing stops. Turning it off
    /// puts the run back into the order the gesture that started it built
    /// ([`shuffle::restored`]). See [`App::toggle_shuffle`].
    ToggleShuffle,
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
    /// The open context menu, if one stands (doc 09 §5.2) — `None` at rest.
    ///
    /// **One `Option` is the whole overlay state**, which is what makes
    /// "one menu at a time" structural: opening another replaces this one,
    /// and every close — <kbd>Esc</kbd> (the peel's outermost layer), a
    /// press outside, an item press, any navigation — is `None` by one
    /// assignment. The items are captured at open, so a press sends exactly
    /// what was offered on screen ([`crate::menu::Menu`]).
    menu: Option<menu::Menu>,
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
    /// Whether the returns lane opens open, read from the config for
    /// `group_key`'s reason and handed to the shelf the same way.
    lane_open: bool,
    /// **The lane, merged**: the shelf's recent records and every playlist,
    /// in [`crate::lane::resolve`]'s one order.
    ///
    /// Cached rather than rebuilt per frame, and re-merged only when one of
    /// its two halves says it moved ([`Self::lane_mark`]) — the merge is
    /// O(playlists), so this is thrift rather than necessity, but the
    /// contract is *no work per frame* and a cache that is only rebuilt on
    /// events is how that is kept true as the two halves grow.
    lane: Vec<crate::lane::Touched>,
    /// The two stamps [`Self::lane`] was built from: the shelf's and the
    /// playlists'.
    lane_mark: (u64, u64),
    /// What [`Self::request_offscreen_art`] last asked for: the lane's stamps
    /// and the place, which between them change exactly when one of the
    /// surfaces beside the wall changes what it draws.
    art_mark: ((u64, u64), Place),
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
    Shelf(Box<Shelf>),
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
        let lane_open = stored.as_ref().is_none_or(|config| config.sidebar_open);
        // **The shuffle property, restored.** A standing decision
        // (`config::Config::shuffle`), seeded rather than assumed for
        // `seed_volume`'s reason: the control must be lit on the first frame,
        // not on the first press.
        player.seed_shuffle(stored.as_ref().is_some_and(|config| config.shuffle));
        let resume = read_snapshot();
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
            match Shelf::open(dirs, group_key, density, lane_open) {
                Ok((shelf, task)) => (Screen::Shelf(Box::new(shelf)), task),
                Err(error) => (Screen::Setup(Setup::fresh(Some(error))), Task::none()),
            }
        };
        let mut app = Self {
            _history_ledger: history_ledger,
            group_key,
            settings_section: 0,
            density,
            lane_open,
            lane: Vec::new(),
            lane_mark: (u64::MAX, u64::MAX),
            art_mark: ((u64::MAX, u64::MAX), Place::Settings),
            was_scanning: true,
            resume: resume.clone(),
            written: (0, None, 0),
            modifiers: keyboard::Modifiers::empty(),
            started,
            first_frame_logged: false,
            screen,
            place: Place::default(),
            hovered_queue_row: None,
            queue_scroll: 0.0,
            hovered_playlist_row: None,
            hovered_album_row: None,
            drag: None,
            queue_undo: crate::undo::History::new(),
            menu: None,
            playlists: crate::playlists::Playlists::start(),
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
        // One publish before the first frame, so a desktop widget that asks
        // straight away gets the seeded volume and the real `Can*` flags
        // rather than the server's own defaults. The MPRIS thread may not
        // have reached its bus yet; the update simply waits in its channel.
        app.publish_mpris(false);
        (app, task)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.route(message);
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
        Task::batch([task, self.request_offscreen_art()])
    }

    /// Everything [`Self::update`] does except keep the lane true — the update
    /// loop proper, split out so that the one thing that must happen after
    /// every message can be one line rather than an arm in each of forty.
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
            Self::update_motion,
            Self::update_modified_input,
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
            // **The doors, and the one way back.** Every one of them is
            // navigation and nothing else: no panel opens, no width changes,
            // and the Library's own state — scroll, query, arrangement — is
            // untouched by all of them, which is what makes coming back free.
            Message::ToggleSettings => self.go(Place::settings),
            Message::ToggleQueue => {
                // Wherever the door leads, the place's scrollable starts at
                // the top the next time it exists (see `queue_scroll`).
                self.queue_scroll = 0.0;
                self.go(Place::queue)
            }
            // **Shift-click a sleeve queues the record** — the one-press
            // accelerator over the picker's Queue row (ADR-0023 §3's stack;
            // doc 09 §13 step 7). A plain press navigates, exactly as
            // before.
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
            // **The album page's breadcrumb**: up to the artist. `go` carries
            // the toggle, so pressing the artist you are already reading puts
            // the page down, exactly as a tile pressed twice does.
            Message::OpenArtist(id) => self.go(|place| place.artist(id)),
            Message::ShowPlayingAlbum => match self.player.playing_album() {
                // Nothing is sounding, so there is no record to be taken to.
                // The control is not offered in that state (see
                // `views::bottom_bar`), so this is the guard and not the case.
                None => Task::none(),
                Some(id) => self.open_album(id),
            },
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
            Message::PlayAll => {
                self.play_all();
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
            // Best effort by nature: a Wayland compositor is entitled to
            // refuse a focus request, and refusing is not an error here.
            Message::Raise => window::get_latest().and_then(window::gain_focus),
            Message::Undo => self.undo_edit(),
            message if matches!(self.screen, Screen::Setup(_)) => self.update_setup(message),
            message => match &mut self.screen {
                Screen::Shelf(state) => state.update(message),
                Screen::Setup(_) => Task::none(),
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

    /// Setup → Shelf: open the very first shelf over `dir`, or say in place
    /// why not. The one seam all three first-run doors — typed, picked,
    /// dropped — converge on.
    fn open_first_shelf(&mut self, dir: PathBuf) -> Task<Message> {
        let Screen::Setup(setup) = &mut self.screen else {
            return Task::none();
        };
        match Shelf::open(vec![dir], self.group_key, self.density, self.lane_open) {
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

    /// <kbd>Enter</kbd>: needle-drop the top-ranked **song** while a query
    /// stands (doc 09 §5, ADR-0023 §2's amendment), else play the record the
    /// wall was last left for (ADR-0017 §1.2's fall-through).
    ///
    /// Resolved on the shell because playing is the shell's job and the answer
    /// is the shelf's — the same split every other play route in this file
    /// takes. [`Shelf::enter_drops_needle`] and [`Shelf::enter_plays`] hold
    /// the choice; this holds the sound.
    ///
    /// The song path is [`Self::play_track`] — the record page's own needle
    /// drop, `SetQueue` (selected edition, whole, in order) + `JumpTo`
    /// through [`PlayerState::play_from`]'s decision — so <kbd>Enter</kbd> is
    /// exactly a press on the Songs section's first row, not a third play
    /// grammar.
    fn play_first_match(&mut self) -> Task<Message> {
        if let Screen::Shelf(state) = &self.screen {
            if let Some((id, row)) = state.enter_drops_needle() {
                self.play_track(id, row);
            } else if let Some(id) = state.enter_plays() {
                self.play_album(id);
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
                // The door to the page you are on puts it down, like a tile
                // pressed twice ([`Place::playlist`]'s toggle); any other
                // page is opened only once its file actually read.
                let leaving = self.place == Place::Playlist(*id);
                if leaving {
                    self.menu = None;
                    let from = self.place;
                    self.place = self.place.playlist(*id);
                    self.note_place_left(from);
                } else if let Screen::Shelf(state) = &self.screen
                    && self.playlists.open_page(*id, &state.library)
                {
                    // The place changes, so an open menu goes with it
                    // (`go`'s rule).
                    self.menu = None;
                    let from = self.place;
                    self.place = self.place.playlist(*id);
                    self.note_place_left(from);
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
                self.playlists.naming = Some(crate::playlists::NameEntry::default());
                return Some(text_input::focus(views::playlist_panel::new_name_id()));
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
            Message::PlaylistQueue => self.queue_playlist(),
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
                    open.renaming = Some(crate::playlists::NameEntry {
                        text: seeded,
                        error: None,
                    });
                    return Some(text_input::focus(views::playlist::rename_id()));
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
                    self.note_place_left(from);
                }
            }
            Message::PlaylistDelete => {
                let library = match &self.screen {
                    Screen::Shelf(state) => Some(&state.library),
                    Screen::Setup(_) => None,
                };
                if self.playlists.delete_open(library) && matches!(self.place, Place::Playlist(_)) {
                    // The page's subject is in the trash; the Library is the
                    // honest answer, by the same route Esc takes.
                    let from = self.place;
                    self.place = self.place.back();
                    self.note_place_left(from);
                }
            }
            Message::SaveQueueStart => {
                self.playlists.saving_queue = Some(crate::playlists::NameEntry::default());
                return Some(text_input::focus(views::queue::save_name_id()));
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
                        Screen::Setup(_) => None,
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

    /// Kick off decodes for every record the playlist sleeves quote that the
    /// thumbnail cache does not hold — the collage's whole supply line, and
    /// it is [`Shelf::request_thumbs`]'s, which is the wall's.
    fn request_playlist_art(&mut self) -> Task<Message> {
        let mut wanted: Vec<u64> = Vec::new();
        for row in &self.playlists.rows {
            wanted.extend(&row.art);
        }
        if let Some(open) = &self.playlists.open {
            wanted.extend(&open.art);
        }
        wanted.sort_unstable();
        wanted.dedup();
        match &mut self.screen {
            Screen::Shelf(state) => state.request_thumbs(&wanted),
            Screen::Setup(_) => Task::none(),
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

    /// The playlist page's `Queue`: the playable subset appended to the run
    /// through `UpdateQueue`, so the music keeps playing (ADR-0014's
    /// guarantee; "hear this later" is its own gesture, ADR-0023 §3).
    fn queue_playlist(&mut self) {
        let Some(addition) = self
            .playlists
            .open
            .as_ref()
            .map(|open| open.queue.clone())
            .filter(|queue| !queue.is_empty())
        else {
            return;
        };
        self.append_to_run(addition);
    }

    /// The picker's **Queue** row: what the hand holds, appended to the run —
    /// `queue_playlist`'s exact shape over the pick's own items (09 §8.1).
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
            provenance: None,
        });
    }

    /// Append `addition` to the run through `UpdateQueue` — the one shape
    /// every queue-destination pick and the page's `Queue` share. The music
    /// keeps playing; appending to an empty stopped engine loads the queue
    /// without starting it, so nothing sounds unasked (`app.rs`'s own rule,
    /// cited by 09 §8.1).
    fn append_to_run(&mut self, mut addition: vm::QueueVm) {
        // What the run held before the append — the empty list when it held
        // nothing — kept for the Queue place's `Undo` (doc 11 §5 P2: an
        // append is an edit a hand can take back, and taking back an append
        // to nothing restores nothing, which cannot sound).
        let before = self.player.queue().cloned().unwrap_or(vm::QueueVm {
            album: None,
            artist: String::new(),
            items: Vec::new(),
            provenance: None,
        });
        let edited = if let Some(held) = self.player.queue() {
            let mut edited = held.clone();
            edited.items.extend(addition.items);
            edited
        } else {
            // Appending to nothing gives the engine a queue without starting
            // it: `UpdateQueue` never begins playback, and nothing sounds
            // unasked. An append is not a play gesture, so whatever built
            // `addition` — a playlist page's `Queue` included — the loaded
            // run carries **no provenance** (09 §6: provenance is set by
            // reifying a file through a play gesture, and by nothing else).
            addition.provenance = None;
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
            Message::GoTo(to) => Some(self.go(move |place| place.go(to))),
            Message::ToggleLane => Some(self.toggle_lane()),
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
        iced::exit()
    }

    /// **`Resume`**: the run put back on where the band said it was — and the
    /// one play gesture in the product that navigates.
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
    ///    front end's own act: it does not wait on [`Event::TrackStarted`] to
    ///    land. A place that arrived a frame after the press would be the
    ///    interface acknowledging you late, and the shell does not need the
    ///    engine's permission to change which surface it is drawing.
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
        let (queue, _) = vm::restored_queue(
            &state.albums,
            &self.resume.paths,
            self.resume.cursor,
            self.resume.provenance.clone(),
        );
        if queue.is_empty() {
            return;
        }
        let paths = queue.paths();
        if self.playback.send(Command::SetQueue { paths }) {
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
            println!("[session] could not write {}: {error}", path.display());
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
    fn request_offscreen_art(&mut self) -> Task<Message> {
        let mark = (self.lane_mark, self.place);
        if mark == self.art_mark {
            return Task::none();
        }
        self.art_mark = mark;
        let width = self.body_width();
        let quoted: Vec<u64> = self
            .playlists
            .rows
            .iter()
            .flat_map(|row| row.art.iter().copied())
            .collect();
        // An open artist's records are the shelf's own, but *which* artist is
        // the shell's — so the id is read here and the records named below,
        // where both halves are in hand.
        let open_artist = match self.place {
            Place::Artist(id) => Some(id),
            _ => None,
        };
        let Screen::Shelf(state) = &mut self.screen else {
            return Task::none();
        };
        let mut ids = state.offscreen_art(width);
        if let Some(id) = open_artist {
            let theirs: Vec<u64> = crate::views::artist::records(state, id)
                .iter()
                .map(|album| album.id)
                .collect();
            ids.extend(theirs);
        }
        ids.extend(quoted);
        state.request_thumbs_for(&ids)
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
            Screen::Setup(_) => 0,
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
            Screen::Setup(_) => Vec::new(),
        };
        self.lane = crate::lane::resolve(lists, records);
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

    /// Put the lane in `open` — the marks at its foot,
    /// <kbd>Ctrl</kbd>+<kbd>B</kbd>, and every road to the well
    /// ([`Self::reach_the_well`]) — persisting the state and re-hanging the
    /// wall.
    ///
    /// It does nothing at all when the window cannot hold the expanded lane
    /// ([`theme::sidebar_can_expand`]) or when the lane is already in the state
    /// asked for. That second guard is what keeps the re-hang to the presses
    /// whose subject is the collection's width: reaching for the well while
    /// the lane is already open must not touch the grid.
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

    /// **Make the well reachable**, without asking for the caret: go to the
    /// Library, and open the lane if the well is in it and it is shut.
    ///
    /// Both halves are consequences of the owner's decision to move search
    /// into the lane, and both are about not typing into something you cannot
    /// see:
    ///
    /// - **The Library**, because the well searches the collection and the
    ///   collection is what answers. Type-anywhere has always filtered the wall
    ///   from anywhere; until the well was resident, doing it from `Home` or a
    ///   record's page filled a field that was not on screen and narrowed a
    ///   wall that was not either. The lane put the field in every place, and
    ///   this puts the wall back under it.
    /// - **The lane**, because at [`theme::SIDEBAR_RAIL_W`] there is no field
    ///   to focus until it opens. One frame, no tween (ADR-0030 §3.1), so the
    ///   caret lands in the same frame the key did.
    ///
    /// Below [`theme::SIDEBAR_FLOOR`] the second half is a no-op by
    /// [`Self::set_lane`]'s own guard, which is correct: there the well is in
    /// the strip ([`theme::strip_holds_the_well`]) and the Library is the only
    /// place that draws one.
    fn reach_the_well(&mut self) -> Task<Message> {
        if !matches!(self.screen, Screen::Shelf(_)) {
            return Task::none();
        }
        let there = self.go(|place| place.go(crate::lane::Destination::Library));
        let open = self.set_lane(true);
        Task::batch([there, open])
    }

    /// <kbd>/</kbd>, <kbd>Ctrl</kbd>+<kbd>F</kbd> and the collapsed lane's
    /// magnifier: reach the well, then put the caret in it.
    fn focus_the_well(&mut self) -> Task<Message> {
        if !matches!(self.screen, Screen::Shelf(_)) {
            return Task::none();
        }
        Task::batch([self.reach_the_well(), text_input::focus(search_id())])
    }

    /// **Type anywhere** (ADR-0017 §1.2) — the shell's half of it, which
    /// exists because the owner moved the well into the lane.
    ///
    /// The shelf still appends the text, filters and takes the caret
    /// ([`Shelf::type_into_query`]). What the shell adds, *first*, is a
    /// visible field to append into and the wall the filter is about. Every
    /// road to the query goes through [`Self::reach_the_well`], so the letter,
    /// the slash and the lane's magnifier all land in the same state.
    fn type_anywhere(&mut self, text: &str) -> Task<Message> {
        let reach = self.reach_the_well();
        let typed = match &mut self.screen {
            Screen::Shelf(state) => state.type_into_query(text),
            Screen::Setup(_) => Task::none(),
        };
        Task::batch([reach, typed])
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
            let from = self.place;
            self.place = door(self.place);
            self.note_place_left(from);
        }
        Task::none()
    }

    /// **Open a record's page** — a tile press, or the bar's now-playing
    /// block.
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
        // The place changes, so an open menu and any drag go with it
        // (`go`'s rule).
        self.menu = None;
        self.drag = None;
        let from = self.place;
        self.place = self.place.album(id);
        self.note_place_left(from);
        Task::none()
    }

    /// **Go home** — every place's `‹ Library`, and the first thing
    /// <kbd>Esc</kbd> does.
    ///
    fn leave(&mut self) -> Task<Message> {
        // The place changes, so an open menu and any drag go with it
        // (`go`'s rule).
        self.menu = None;
        self.drag = None;
        let from = self.place;
        self.place = self.place.back();
        self.note_place_left(from);
        // A place's transient fields do not outlive the place: a rename
        // field left standing behind a navigation would greet the next
        // visit mid-gesture.
        if let Some(open) = &mut self.playlists.open {
            open.renaming = None;
        }
        self.playlists.saving_queue = None;
        Task::none()
    }

    /// <kbd>Esc</kbd>'s place-level share of the peel: the transient fields
    /// standing *on* the current place — a rename mid-type, the queue's
    /// save field — each one press, before the place itself leaves. (The
    /// armed delete peeled here until doc 11 §5 P2 retired the confirm:
    /// deletion is one press into the trash now, so there is no armed layer
    /// left to peel.)
    fn peel_place_states(&mut self) -> bool {
        match self.place {
            Place::Playlist(_) => {
                let Some(open) = &mut self.playlists.open else {
                    return false;
                };
                open.renaming.take().is_some()
            }
            Place::Queue => self.playlists.saving_queue.take().is_some(),
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
            Screen::Setup(_) => Task::none(),
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
                    }
                    Event::TrackFailed { path, reason } => {
                        println!("[playback] track skipped: {} ({reason})", path.display());
                    }
                    Event::QueueEnded => {
                        println!("[playback] queue ended");
                        // The run the history described is over — the third
                        // of P2's three ends for an edit history (next
                        // edit, navigation, the run ending).
                        self.queue_undo.clear();
                    }
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
        // **The shuffle property applies to a new play gesture**, and it is
        // applied in exactly one place ([`Self::send_run`]): press `Play` on a
        // record with shuffle on and the record plays shuffled, with its own
        // sleeve order kept as the order to come back to. One construction,
        // two uses — the payload the engine is sent and the list the queue
        // panel shows come from the same value, so they cannot describe
        // different music (see [`vm::QueueVm`]).
        if self.send_run(queue, None).is_some() && self.playback.send(Command::Play) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
        // A queue where there was none moves `CanPlay`, and that is the one
        // MPRIS-visible change that arrives without an engine event.
        self.publish_mpris(false);
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
                // The row the click named, handed to the one arranger: with
                // shuffle off it is the position to jump to; with shuffle on
                // the track is hoisted to the front and the answer is 0.
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
        // A queue where there was none moves `CanPlay`, exactly as in
        // `play_album`, and that is the one MPRIS-visible change that arrives
        // without an engine event.
        self.publish_mpris(false);
    }

    /// **Arrange a run for the player's shuffle property, and send it.**
    ///
    /// The one place a `SetQueue` that *starts* something goes out, which is
    /// what makes "`Play` on a record, `Play all`, a playlist's `Play` and a
    /// track click all agree" a structural fact rather than four functions
    /// keeping a convention. Each caller builds the queue its own gesture
    /// means, in the order that gesture means, and hands it here; what happens
    /// to that order is decided once.
    ///
    /// - **Shuffle off**: the queue goes out exactly as built. Nothing is
    ///   retained, because there is nothing to come back from.
    /// - **Shuffle on**: the source order is retained
    ///   ([`shuffle::source_order`]), the run is permuted
    ///   ([`shuffle::arranged`]), and the two are recorded **together**
    ///   ([`PlayerState::note_shuffled_run`]) — so "a shuffled run the toggle
    ///   cannot turn off" is not a state this shell can reach.
    ///
    /// `lead` is a row the gesture named — a track click — which plays first
    /// and is not shuffled into the body ([`shuffle::leading`]). `None` is a
    /// plain `Play`, where the whole run is still *next*.
    ///
    /// Answers **the position playback should start at**: the named row with
    /// shuffle off, `0` with it on — the front of the run for a plain `Play`,
    /// and the hoisted track for a click. `None` when the engine would not take
    /// the queue, which is the caller's cue to stop rather than to send a
    /// transport command into a run that does not exist.
    fn send_run(&mut self, queue: vm::QueueVm, lead: Option<usize>) -> Option<usize> {
        if !self.player.shuffle() {
            let paths = queue.paths();
            if !self.playback.send(Command::SetQueue { paths }) {
                self.player.engine_closed();
                return None;
            }
            self.player.note_queue_sent(queue);
            return Some(lead.unwrap_or(0));
        }
        let source = shuffle::source_order(&queue);
        let arranged = match lead {
            // The clicked track leads and the rest follows by chance — the one
            // reading that honours both halves of "play this one" and "shuffle
            // is on".
            Some(row) => shuffle::arranged(&shuffle::leading(&queue, row), draw_seed(), 1),
            None => shuffle::arranged(&queue, draw_seed(), 0),
        };
        let paths = arranged.paths();
        if !self.playback.send(Command::SetQueue { paths }) {
            self.player.engine_closed();
            return None;
        }
        self.player.note_shuffled_run(arranged, source);
        Some(0)
    }

    /// **Turn shuffle on or off** — the now-playing bar's crossed arrows
    /// (the owner, 2026-08-10: *"can you make shuffle a property of the player
    /// i.e. toggle on/off"*).
    ///
    /// Three things, in this order: the property moves, the standing decision
    /// is written to `config.toml`, and the run in progress is re-arranged to
    /// match what the control now says.
    ///
    /// **Nothing stops.** The re-arrangement goes out as
    /// [`Command::UpdateQueue`], which ADR-0014 guarantees disturbs no
    /// delivered sample when the playing track stays where it is — and it
    /// always does, because both directions keep the needle put:
    ///
    /// - **On**: what is behind the needle is history and does not re-order;
    ///   what is in front of it is permuted ([`shuffle::arranged`], with
    ///   `keep` = the playing row + 1). The order the run had is retained as
    ///   the order to come back to.
    /// - **Off**: the run goes back into its retained order
    ///   ([`shuffle::restored`]), and the retained order is spent. A run with
    ///   none — restored from a snapshot, or re-ordered by hand since — is
    ///   left exactly as it stands and says so on stdout, which is the honest
    ///   answer rather than an invented one.
    ///
    /// A press with nothing playing moves the property and writes it, and that
    /// is the whole of what there is to do: the mode is about what plays
    /// **next**.
    fn toggle_shuffle(&mut self) {
        let on = self.player.set_shuffle(!self.player.shuffle());
        persist_shuffle(on);
        let Some(queue) = self.player.queue().cloned() else {
            println!("[shuffle] {}", if on { "on" } else { "off" });
            return;
        };
        // `playing_queue_row` is the engine's answer reconciled against this
        // record; `None` — queued but not started — means all of it is still
        // to come.
        let keep = self.player.playing_queue_row().map_or(0, |row| row + 1);
        // **On** retains the order it is about to destroy; **off** spends the
        // one it retained. A run with none to spend — restored from a snapshot,
        // or re-ordered by hand since — is left exactly as it stands.
        let arranged = if on {
            Some(shuffle::arranged(&queue, draw_seed(), keep))
        } else {
            self.player
                .source_order()
                .map(|order| shuffle::restored(&queue, order))
        };
        let Some(arranged) = arranged else {
            println!("[shuffle] off \u{2014} this run has no earlier order to return to");
            return;
        };
        let source = on.then(|| shuffle::source_order(&queue));
        let paths = arranged.paths();
        if !self.playback.send(Command::UpdateQueue { paths }) {
            self.player.engine_closed();
            return;
        }
        self.player.note_queue_edited(arranged);
        match source {
            Some(order) => self.player.retain_source_order(order),
            None => self.player.forget_source_order(),
        }
        println!(
            "[shuffle] {} \u{2014} the run re-arranged from row {keep}",
            if on { "on" } else { "off" }
        );
        self.publish_mpris(false);
    }

    /// **Play everything the wall shows** (doc 09 §7.1, S6): the wall is a
    /// list, so play it.
    ///
    /// One press reifies the wall's scope — every record `Shelf::visible`
    /// holds, whole, in the arrangement's own order — into the queue
    /// ([`vm::stacked_queue`], the shape shuffle already sends) and plays
    /// from the top. **The scope is the wall, always**: a query or a group
    /// key narrows it, a YEAR-arranged wall plays the collection in
    /// chronological order, and "everything in the library" is the empty
    /// query, one <kbd>Esc</kbd> away. Playing what you cannot see is refused
    /// (a standing rule of the product) — which is why this reads `visible` and nothing
    /// wider, and why an empty wall means nothing happens and nothing is
    /// claimed. No confirmation stands between the press and the sound at any
    /// scale: the queue place is virtualized (`crate::queue_window`, §7.1's
    /// named gate), so a five-figure run is an ordinary queue — readable,
    /// jumpable, editable, saveable, and it **ends**.
    ///
    /// **What order it plays in is not this function's business.** It was:
    /// `Play all` was *"shuffle's sibling, never a mode"*, the plain half of a
    /// pair whose other half drew eight records by chance. Shuffle became a
    /// property of the player on 2026-08-10 and the pairing dissolved — this
    /// builds the list the gesture means and [`Self::send_run`] decides the
    /// order, exactly as it does for a record, a playlist and a track click.
    fn play_all(&mut self) {
        let Screen::Shelf(state) = &self.screen else {
            return;
        };
        // **`Play all` is the All songs list's own `Play`.** It used to build
        // its own queue out of `state.visible`; it now resolves the implicit
        // playlist and plays that, which is what makes the two one concept
        // rather than two that had to be kept agreeing.
        let list = state.all_songs();
        if list.is_empty() {
            // The wall is showing nothing — an empty library, or a query
            // that matched no record. Nothing to play, so nothing happens
            // and nothing is claimed. Silence is the correct answer here
            // too.
            return;
        }
        println!("[all-songs] play — {}", list.counts());
        let queue = list.queue;
        if self.send_run(queue, None).is_some() && self.playback.send(Command::Play) {
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
            // **A hand reorder restates the order**, so the order shuffle would
            // return to is dropped with it (ADR-0023's amendment): the hand
            // beats the machine's memory, and turning shuffle off after a
            // stepper press leaves the run exactly as the press left it.
            self.player.forget_source_order();
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
            // [`Self::shift_queued`]'s rule, for the drag.
            self.player.forget_source_order();
        } else {
            self.player.engine_closed();
        }
        self.publish_mpris(false);
    }

    /// The place's transient `Undo`, resolved against **which list surface
    /// the window is showing** (doc 11 §5 P2): the Queue place takes back a
    /// run edit, an open playlist page takes back a file edit, and anywhere
    /// else the press asks for nothing — undo is one history per surface,
    /// never a global stack, and its accelerator is legal exactly where its
    /// visible twin stands.
    fn undo_edit(&mut self) -> Task<Message> {
        match self.place {
            Place::Queue => self.undo_queue_edit(),
            Place::Playlist(_) => {
                if let Screen::Shelf(state) = &self.screen {
                    self.playlists.undo_open(&state.library);
                }
            }
            _ => {}
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
    fn note_place_left(&mut self, from: Place) {
        if from == self.place {
            return;
        }
        if from == Place::Queue {
            self.queue_undo.clear();
        }
        if matches!(from, Place::Playlist(_)) {
            self.playlists.clear_undo();
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

    /// The sleeve the bottom bar draws beside the track and artist: **the
    /// sounding record's thumbnail, if the wall has decoded one**.
    ///
    /// Read from the wall's own cache with `peek` rather than `get`, so a
    /// frame cannot reorder an LRU — the bar observes the wall's art, it does
    /// not compete for it. `None` whenever the record has no decodable art,
    /// and the bar then draws exactly what it drew before the cover existed.
    fn bar_cover(&self) -> Option<iced_image::Handle> {
        let Screen::Shelf(state) = &self.screen else {
            return None;
        };
        let id = self.player.playing_album()?;
        state.thumbs.peek(&id).cloned()
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
        let collecting = self.playlists.collecting();
        let screen: Element<'_, Message> = match (&self.screen, self.place) {
            (Screen::Setup(setup), _) => return views::setup::view(setup),
            (Screen::Shelf(state), Place::Library) => {
                state.view(&self.player, lamp, collecting, ink)
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
                None => state.view(&self.player, lamp, collecting, ink),
            },
            (Screen::Shelf(state), Place::Artist(id)) => {
                // The artist vanished under a rescan while their page was
                // open — renamed, or their last record removed. The wall is
                // the honest answer, drawn rather than navigated to, exactly
                // as a vanished record's page is.
                if views::artist::label(state, id).is_some() {
                    views::artist::view(state, &self.player, id, self.body_width(), collecting)
                } else {
                    state.view(&self.player, lamp, collecting, ink)
                }
            }
            (Screen::Shelf(_), Place::Queue) => views::queue::view(
                &self.player,
                iced::Size::new(self.body_width(), self.window.height),
                // The hover slots go quiet while a row is in the hand: the
                // gesture's own statements — the ghost and the line — are
                // the surface's voice mid-drag.
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
                    self.body_width(),
                    self.drag
                        .as_ref()
                        .map_or(self.hovered_playlist_row, |_| None),
                    collecting,
                    self.drag.as_ref(),
                    self.playlists.can_undo_open(),
                ),
                // The playlist vanished under its page — deleted or renamed
                // on disk. The wall is the honest answer, drawn rather than
                // navigated to, exactly as a vanished record's page is.
                None => state.view(&self.player, lamp, collecting, ink),
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
                collecting,
            ),
            (Screen::Shelf(state), Place::NowPlaying) => {
                views::now_playing::view(state, &self.player, self.body_width(), self.body_height())
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
                    state.library_view(),
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
                    // head's `Now playing` dot, and *this record* is sounding
                    // lights its own row. They differ for a file the library
                    // does not hold — the head still answers, the list has
                    // nothing to mark.
                    self.player.playing_album(),
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
        // The persistent bottom bar lives under every place — unless this build
        // has no audio output at all, in which case playback UI is hidden
        // entirely.
        let whole: Element<'_, Message> = if *self.player.availability() == Availability::NotBuilt {
            screen
        } else {
            column![
                screen,
                views::bottom_bar::view(&self.player, self.place, ink, self.bar_cover()),
            ]
            .into()
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
            None => iced::widget::Space::new(0.0, 0.0).into(),
        };
        iced::widget::stack![whole, ghost].into()
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
    /// scrolling. The two new places wear no strip of their own — the returns
    /// lane is the route in and out of them — so nothing comes off the top.
    fn body_height(&self) -> f32 {
        if *self.player.availability() == Availability::NotBuilt {
            return self.window.height;
        }
        (self.window.height - theme::BAR_CONTENT_H - 1.0).max(0.0)
    }

    /// Whether the playlist panel is on screen: summoned, over a shelf, and
    /// not in Settings — the one place it is absent (ADR-0024 §5). Its open
    /// state *survives* the Settings round trip; only its pixels do not.
    fn panel_on_screen(&self) -> bool {
        matches!(self.screen, Screen::Shelf(_))
            && self.playlists.panel_open
            && self.place != Place::Settings
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
                // The window as a drop target (doc 11 §5 P1) — what the
                // toolkit actually delivers: winit 0.30 publishes these on
                // X11 and not on Wayland (see [`Message::FileDropped`]).
                // The setup screen answers them; everywhere else they fall
                // through the shelf's own arm to nothing.
                iced::Event::Window(window::Event::FileDropped(path)) => {
                    Some(Message::FileDropped(path))
                }
                iced::Event::Window(window::Event::FileHovered(_)) => Some(Message::FileHovered),
                iced::Event::Window(window::Event::FilesHoveredLeft) => {
                    Some(Message::FileHoverLeft)
                }
                _ => None,
            }),
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
    /// Indices into `albums` that survive the current query, in wall order.
    pub(crate) visible: Vec<usize>,
    /// How many of each shelf's albums survived it, in `groups` order — what
    /// [`shelf::Shelves`] lays the wall out from.
    visible_counts: Vec<usize>,
    /// The live search text.
    pub(crate) query: String,
    /// The **Songs** answers for the live query (doc 09 §5): the top
    /// [`vm::SONGS`] ranked matching tracks, rebuilt with the filter in
    /// [`Shelf::refilter`] so the section and the wall answer one query.
    /// Empty while the query is blank — the section is then absent, not
    /// empty.
    pub(crate) songs: Vec<vm::SongVm>,
    /// Which songs row the pointer is on, if any — the record page's
    /// [`App::hovered_album_row`] mechanism, for the same toolkit reason:
    /// the row's reserved `+` is a sibling the row itself cannot style.
    pub(crate) hovered_song: Option<usize>,
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
    /// The album range [`Shelf::request_visible_thumbs`] last asked about, so
    /// the two redundant requests every resize step delivers cost a
    /// comparison instead of a pass over the library. `None` until the first
    /// ask, and reset by anything that changes *which* albums the range names
    /// rather than where it sits — see [`Shelf::forget_requested`].
    last_requested: Option<(usize, usize)>,
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
    ///
    /// Crate-visible because the Library strip reads it to answer one
    /// question the strip's *own* width cannot: whether the returns lane can
    /// hold the search well ([`theme::strip_holds_the_well`]).
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
}

impl Shelf {
    /// Open the library DB, hydrate the shelf, persist the chosen folders, and
    /// kick off the scan worker. Errors are user-presentable strings.
    fn open(
        roots: Vec<PathBuf>,
        group_key: GroupKey,
        density: shelf::Density,
        lane_open: bool,
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
            songs: Vec::new(),
            hovered_song: None,
            opened: None,
            edition_choice: HashMap::new(),
            thumbs: LruCache::new(
                NonZeroUsize::new(art::THUMB_CACHE_ENTRIES).unwrap_or(NonZeroUsize::MIN),
            ),
            last_requested: None,
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
                WINDOW.height - theme::top_bar_h(WINDOW.width, lane_open),
            ),
            last_scan_log: Instant::now(),
            hovered_album: None,
            tile_hover: Keyed::new(),
            window_w: WINDOW.width,
            lane_open,
            lane_played: HashMap::new(),
            lane_recent: Vec::new(),
            lane_stamp: 0,
            collection: vm::Collection::default(),
        };
        // `rebuild_shelves` folds the ledger onto the records it has just
        // built (ADR-0030 §4): once, here, and never again from the file.
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
                // the lane its own bar reserves, so the grid is told what the
                // rows actually get — otherwise the estimate and the
                // measurement disagree by exactly the bar's width, and at a
                // boundary width that is one column too many.
                self.grid_size = Size::new(
                    (bounds.width - theme::WALL_SCROLLBAR_W).max(0.0),
                    bounds.height,
                );
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
                    (size.height - theme::top_bar_h(size.width, self.lane_open)).max(100.0),
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
            Message::SongRowEntered(row) => {
                self.hovered_song = Some(row);
                Task::none()
            }
            // Only if it is still the row that left, for the reason the
            // queue rows' pair is order-independent.
            Message::SongRowLeft(row) => {
                if self.hovered_song == Some(row) {
                    self.hovered_song = None;
                }
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

    /// **<kbd>Enter</kbd>'s album-level fall-through**: the record the wall
    /// was last left for when no query stands, else the top-ranked matching
    /// album — reached only when [`Self::enter_drops_needle`] answered
    /// nothing, which with a query standing means the top song could not be
    /// resolved onto its record (doc 09 §5 retargeted the with-query case to
    /// the song; this keeps the with-query album answer as the degradation
    /// rather than a dead key).
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
    /// **What <kbd>Enter</kbd> needle-drops** while a query stands: the Songs
    /// section's own first row — the top-ranked matching track (ADR-0021),
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
            scrollable::scroll_to(scroll_id(), AbsoluteOffset { x: 0.0, y: 0.0 }),
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
        // **Home's figures, counted here and nowhere else.** One pass over the
        // tracks that were just rebuilt, on the same schedule the rebuild runs
        // on — which is what keeps the `COLLECTION` footer off the per-frame
        // path (ADR-0030 §4). The arrangement does not change any of the four,
        // but re-counting is cheaper than reasoning about which caller changed
        // what.
        self.collection = vm::Collection::count(&self.albums, self.library.len());
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
                |path| history.track(path).last_played_unix_s,
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
        self.lane_recent = crate::lane::resolve(Vec::new(), touched);
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

    /// Recompute `visible` for the current query (wall order preserved —
    /// see [`vm::matching_album_ids`] for the track→album mapping), and with
    /// it how many albums each shelf has left.
    ///
    /// The two are computed in one pass from one filter, so the wall's layout
    /// and its contents cannot disagree about which albums survived.
    fn refilter(&mut self) {
        self.visible = vm::visible_indices(&self.albums, &self.library, &self.query);
        // The range guard names *positions*, and every album behind them has
        // just moved. Rows 0..24 of a filtered wall are not the rows 0..24 of
        // the wall before it.
        self.forget_requested();
        // The Songs section's rows: the ranked head of the same match set
        // that filtered the wall (doc 09 §5 — the two sections are one
        // query's two projections). Rebuilt here rather than per frame so a
        // redraw costs no corpus scan, and the hover cannot outlive the rows
        // it pointed into.
        self.songs = vm::song_hits(&self.library, &self.query, vm::SONGS);
        self.hovered_song = None;
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
    fn request_thumbs_for(&mut self, ids: &[u64]) -> Task<Message> {
        let mut tasks = Vec::new();
        for &id in ids {
            if self.thumbs.peek(&id).is_some()
                || self.pending.contains(&id)
                || self.no_art.contains(&id)
            {
                continue;
            }
            let Some(album) = self.albums.iter().find(|album| album.id == id) else {
                continue;
            };
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

    /// The ids the surfaces beside the wall are drawing **that the shelf can
    /// name**: the lane's recent records and the Home place's newest row. An
    /// open artist's records join them in [`App::request_offscreen_art`],
    /// which is where *which* artist is known.
    ///
    /// A lane row that is a *list* names no record here on purpose — the
    /// records it quotes are `Playlists`' to know, and the shell adds them in
    /// [`App::request_offscreen_art`] rather than this function reaching into
    /// a collection the shelf does not hold.
    ///
    /// **The artist's page is here because its tiles are outside the wall's
    /// visible range**, and the wall's thumbnail guard is exactly that range.
    /// Without this, an artist's records drew the deterministic gradient until
    /// one of them happened to scroll past on the wall — real artwork *by
    /// luck*, which is verbatim the defect the playlist collages had before
    /// their quotations were named here.
    fn offscreen_art(&self, width: f32) -> Vec<u64> {
        let mut ids: Vec<u64> = self
            .lane_recent
            .iter()
            .filter_map(|row| match row.subject {
                crate::lane::Subject::Record(id) => Some(id),
                crate::lane::Subject::Playlist(_) => None,
            })
            .collect();
        ids.extend(
            crate::views::home::newest(self, width)
                .iter()
                .map(|album| album.id),
        );
        ids
    }

    fn request_visible_thumbs(&mut self) -> Task<Message> {
        let (start, end) = self
            .shelves()
            .visible_albums(self.scroll_offset, self.grid_size.height);
        let (start, end) = (start.min(self.visible.len()), end.min(self.visible.len()));
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

    /// Kick off off-thread decodes for the albums in `ids` whose thumbnail is
    /// neither cached, in flight, nor known-absent — the playlist sleeves'
    /// supply line (ADR-0024 §A1), and deliberately nothing but a re-aim of
    /// [`Self::request_visible_thumbs`]'s pipeline: same cache, same decode
    /// path, same placeholder while it runs. An id the wall no longer holds
    /// is skipped; the collage cell keeps its gradient, which is the same
    /// honest reading a tile gives art that cannot be decoded.
    fn request_thumbs(&mut self, ids: &[u64]) -> Task<Message> {
        let mut tasks = Vec::new();
        for &id in ids {
            if self.thumbs.get(&id).is_some()
                || self.pending.contains(&id)
                || self.no_art.contains(&id)
            {
                continue;
            }
            let Some(album) = self.albums.iter().find(|album| album.id == id) else {
                continue;
            };
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
    fn view<'a>(
        &'a self,
        player: &'a PlayerState,
        lamp: f32,
        collecting: crate::playlists::Collecting,
        ink: Ink,
    ) -> Element<'a, Message> {
        column![
            views::top_bar::view(self, self.body_width(), ink),
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
                provenance: queue.provenance.clone(),
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
        println!("[session] no interrupted run");
    } else {
        println!(
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
        // Out to a record's page, on to the queue, on to the settings, home.
        let place = place.album(7);
        assert_eq!(place, Place::Album(7));
        let place = place.queue();
        assert_eq!(place, Place::Queue);
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
    /// the top match has the record page's `Play album`, the arrangement has
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
        const CONTROLS: [(&str, &str); 21] = [
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
            ("VolumeStep", "the bottom bar's volume fader"),
            ("ToggleMute", "the bottom bar's speaker button"),
            ("ToggleQueue", "the bottom bar's labelled `Queue` control"),
            ("ToggleSettings", "the top bar's Settings control"),
            ("FocusSearch", "the top bar's search well"),
            ("EscapePressed", "every place's `‹ Library`"),
            (
                "QueryTyped",
                "the top bar's search well — the field ADR-0017 §1.2 kept, \
                 which a pointer clicks into to type the same query",
            ),
            (
                "PlayFirstMatch",
                "the Songs section's first row while a query stands \
                 (doc 09 §5); the record page's `Play album` for the \
                 fall-through; the well's own Enter sends this too",
            ),
            (
                "DensityStep",
                "the three density marks at the foot of the index rail's \
                 lane (ADR-0028) — each sends this message with the exact \
                 delta the gesture would spend, so Ctrl+scroll and Ctrl+-/= \
                 are accelerators of a visible control now, not the control \
                 itself",
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
            "play_all",
            "play_playlist",
            "play_track",
            "play_playlist_track",
        ] {
            let body = body(gesture);
            assert!(
                body.contains("self.send_run("),
                "`{gesture}` starts a run without going through the arranger — \
                 shuffle would apply to some gestures and not others"
            );
            assert!(
                !body.contains("Command::SetQueue"),
                "`{gesture}` sends its own SetQueue past `send_run`"
            );
        }

        // And the arranger does the two things the mode is made of: it retains
        // the order it is about to destroy, and it records the pair together.
        let arranger = body("send_run");
        assert!(
            arranger.contains("shuffle::source_order"),
            "the order is kept"
        );
        assert!(
            arranger.contains("shuffle::arranged"),
            "and the run permuted"
        );
        assert!(
            arranger.contains("note_shuffled_run"),
            "the run and its source order are recorded together, so a shuffled \
             run the toggle cannot turn off is unreachable"
        );

        // **Turning it off never stops the music.** ADR-0014's bargain: an
        // edit that leaves the playing track alone disturbs no delivered
        // sample, so the toggle is an `UpdateQueue` and never a `SetQueue`.
        let toggle = body("toggle_shuffle");
        assert!(toggle.contains("Command::UpdateQueue"));
        assert!(
            !toggle.contains("Command::SetQueue") && !toggle.contains("Command::Play"),
            "the toggle restarted the music instead of re-ordering it"
        );
        assert!(
            toggle.contains("shuffle::restored"),
            "turning it off puts the run back into its retained order"
        );
        assert!(
            toggle.contains("persist_shuffle"),
            "a standing decision that is not written down is a session setting"
        );

        // **A hand reorder drops the retained order** — the hand beats the
        // machine's memory (ADR-0023's amendment).
        for reorder in ["shift_queued", "move_queued"] {
            assert!(
                body(reorder).contains("forget_source_order"),
                "`{reorder}` re-stated the order by hand and kept the old one"
            );
        }

        // **The pull and the wall's draw are gone, and nothing kept a stub of
        // either.** Named here so that a re-introduction is a deliberate act
        // with a test to move rather than a quiet reappearance. Spelled in two
        // pieces so these assertions are not their own counter-examples.
        for gone in ["draw_pull", "start_shuffle"] {
            let (head, tail) = gone.split_once('_').expect("a two-word name");
            assert!(
                !source.contains(&format!("fn {head}_{tail}")),
                "`{gone}` came back without its removal being reconsidered"
            );
        }
    }

    /// **S6 — `Play all` reifies the wall's scope and plays from the top**
    /// (doc 09 §7.1).
    ///
    /// Pinned over the source for
    /// [`Self::every_play_gesture_arranges_its_run_through_one_function`]'s
    /// reason — there is no `Shelf` to construct without a database and a scan
    /// thread — with each criterion named by the literal a reviewer would have
    /// to move:
    ///
    /// - *the scope is the wall*: the queue is the **All songs** list, which
    ///   is `state.visible` in its order as whole records
    ///   (`crate::all_songs`, whose own tests pin that and `vm`'s pin
    ///   `stacked_queue`'s order-preservation under it);
    /// - *the first track sounds*: the run goes out and `Play` follows, one
    ///   press, no confirmation at any scale — §7.1's answer to the
    ///   10 000-track question is the virtual window, not a dialog;
    /// - *an empty wall does nothing and claims nothing*.
    ///
    /// It used to carry a fourth: *shuffle's sibling, never a mode* — no pool
    /// claimed, the marks of a superseded draw taken off. Shuffle **is** a mode
    /// now (2026-08-10, the owner), there is no pool and no draw to supersede,
    /// and the relationship between the two is stated where it now lives: what
    /// order this run plays in is [`App::send_run`]'s answer, the same as every
    /// other gesture's.
    #[test]
    fn play_all_reifies_the_wall_in_order() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let start = source
            .find("fn play_all(&mut self")
            .expect("play_all exists");
        let rest = &source[start..];
        let play_all = &rest[..rest.find("\n    }\n").expect("a function ends")];

        assert!(
            play_all.contains("state.all_songs()"),
            "`Play all` is the All songs list's own Play — one concept, not two"
        );
        assert!(
            play_all.contains("list.queue"),
            "and it plays that list, rather than building a second one beside it"
        );
        assert!(
            play_all.contains("self.send_run(") && play_all.contains("Command::Play"),
            "one press, and the first track sounds"
        );
        assert!(
            play_all.contains("if list.is_empty()"),
            "an empty wall: nothing happens and nothing is claimed"
        );
    }

    /// **Step 7 — shift-click queues the record, and nothing sounds
    /// unasked** (doc 09 §13; ADR-0023 §3's stack).
    ///
    /// The accelerator resolves through the one append shape the picker's
    /// Queue row spends (`append_to_run` — `UpdateQueue`, never a play
    /// gesture), and the press arm consults the hand-kept modifier state
    /// because iced 0.13 reports a `button`'s press without it. The plain
    /// press still navigates.
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

        // The press arm: shift queues, plain opens — resolved against the
        // hand-kept modifiers, the one instrument a button press leaves.
        let arm_start = source
            .find("Message::AlbumClicked(id) => {")
            .expect("the tile press arm exists");
        let arm = &source[arm_start..arm_start + 400];
        assert!(arm.contains("self.modifiers.shift()"));
        assert!(arm.contains("self.queue_album(id)"));
        assert!(arm.contains("self.open_album(id)"));
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
            source[ended..ended + 400].contains("self.queue_undo.clear()"),
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
            !clear.contains("text_input::focus(search_id())"),
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

    /// **Every road to the query goes through the same two steps**, now that
    /// the owner has moved the well into the returns lane.
    ///
    /// <kbd>/</kbd>, <kbd>Ctrl</kbd>+<kbd>F</kbd>, the lane's collapsed
    /// magnifier and the first key of a type-anywhere query all reach
    /// `reach_the_well` first, and it does exactly two things: **go to the
    /// Library**, because the well searches the collection and the collection
    /// is what answers, and **open the lane**, because at `SIDEBAR_RAIL_W`
    /// there is no field to focus until it does. Source-pinned for
    /// [`Self::shuffle_starts_what_it_draws_and_queues_whole_records`]'s
    /// reason — there is no `Shelf` to build without a database and a scan
    /// thread — and the pins are the two calls plus the guard that keeps the
    /// re-hang off the presses that do not deserve one.
    #[test]
    fn every_road_to_the_query_reaches_the_well_the_same_way() {
        let source = include_str!("app.rs").replace("\r\n", "\n");
        let body = |signature: &str| {
            let rest = source
                .split_once(signature)
                .unwrap_or_else(|| panic!("`{signature}` exists"))
                .1;
            rest[..rest.find("\n    }\n").expect("a function ends")].to_owned()
        };

        let reach = body("fn reach_the_well(&mut self) -> Task<Message> {");
        assert!(
            reach.contains("place.go(crate::lane::Destination::Library)"),
            "reaching the well no longer puts the wall it filters on screen"
        );
        assert!(
            reach.contains("self.set_lane(true)"),
            "reaching the well no longer opens the lane it lives in"
        );

        // The caret is the second step and only the second: `reach_the_well`
        // stays free of it so that type-anywhere can reach the well and let
        // the shelf take the caret in its own arm.
        let focus = body("fn focus_the_well(&mut self) -> Task<Message> {");
        assert!(
            focus.contains("self.reach_the_well()") && focus.contains("text_input::focus"),
            "`/` and Ctrl+F no longer reach the well before focusing it"
        );

        let typed = body("fn type_anywhere(&mut self, text: &str) -> Task<Message> {");
        let at_reach = typed.find("reach_the_well").expect("the reach");
        let at_text = typed.find("type_into_query").expect("the text");
        assert!(
            at_reach < at_text,
            "type-anywhere appends the text before there is a field to see it in"
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

        // **The re-hang stays on the presses whose subject is the wall's
        // width** (ADR-0030 §3): reaching for the well while the lane is
        // already open must not touch the grid.
        let set = body("fn set_lane(&mut self, open: bool) -> Task<Message> {");
        assert!(
            set.contains("self.lane_open == open"),
            "`set_lane` re-hangs the wall for a state it is already in"
        );
        assert!(
            set.contains("theme::sidebar_can_expand(state.window_w)"),
            "`set_lane` can expand the lane at a width that cannot hold it"
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
        for place in [Place::Album(7), Place::Queue, Place::Settings] {
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
        assert_eq!(place, Place::Album(7));
        assert_eq!(place.album(7), Place::Library);
        // …and a different sleeve swaps the page rather than stacking one.
        assert_eq!(place.album(9), Place::Album(9));

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
            provenance: Some("Road Trip".to_owned()),
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
        ] {
            assert!(
                body.contains(cleared),
                "a place change must not outlive `{cleared}` — the three are \
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

    /// **`Resume` is the one play gesture that navigates, and it is the only
    /// one.**
    ///
    /// The owner asked for it by name (*"or takes you to now playing"*) and it
    /// is a deliberate exception, so it is pinned as one: the source of every
    /// other route into playback is swept for a place change, and only
    /// [`App::resume_the_run`] may carry it. `Play` on the wall's hover
    /// options, on a record's page and in a playlist all answer where you are
    /// standing rather than moving you, and an accidental `self.go(…)` in one
    /// of them would be the interface taking the wheel.
    #[test]
    fn resume_is_the_only_play_gesture_that_navigates() {
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
        // Named rather than discovered, and `body_of` panics on a name that
        // has moved — a sweep that quietly matched nothing would pass forever.
        for elsewhere in [
            "play_album",
            "play_track",
            "play_all",
            "play_playlist",
            "play_playlist_track",
            "play_first_match",
        ] {
            assert!(
                !body_of(elsewhere).contains("self.go("),
                "`{elsewhere}` navigates: a play gesture that answers *play \
                 this* must leave you where you are standing (`views::home`'s \
                 note on the exception)"
            );
        }
    }
}
