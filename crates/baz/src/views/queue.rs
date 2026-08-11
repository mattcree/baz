//! **The run column**: what the engine is holding, and where it is in it.
//!
//! # It was a popover, then a rail panel, and is a place again for unsaved runs
//!
//! ADR-0016 moved the queue out of the right-hand rail and into a 360 px card
//! floating over the wall, anchored to the bar it describes. ADR-0022 removed
//! every side surface baz had — the owner's verdict on the pair was *"I really
//! hate the way queue and selected albums appear"* — so the queue became a
//! **place**, at the width of the window.
//!
//! The queue was then folded into Now playing while the two surfaces were being
//! reconciled. That made Now playing a second album/playlist page, which was
//! the wrong destination: it is now deliberately only the current song. The
//! owner subsequently gave one run a real reason to reopen this surface:
//! playing `All songs` materializes an **unsaved playlist**, and its source
//! road needs a location where that list can be inspected, edited and saved.
//! `Place::Queue` is that location. It has no resident navigation row and
//! reserves no space in Now playing; the source footer and bottom bar lead here
//! only while the current run is unsaved.
//!
//! **Every fact and gesture survives in this dormant renderer** — the rows,
//! one list with a cursor, the summary that reads *what is left*, click-to-jump,
//! the per-row ✕, the steppers, the transfer `+`, the drag, `Save as playlist`,
//! `Undo`, the album headers and the virtual window. What did not is the header
//! strip from its former place integration.
//!
//! The bar's own third line still states the continuation ambiently
//! ([`crate::views::bottom_bar`]) — and earns its place harder than before,
//! since it is now the only statement about the run outside this surface.
//!
//! # One list with a cursor
//!
//! The model is `MusicBee`'s, adopted deliberately rather than arrived at:
//! **history behind the cursor, queue ahead, one surface**
//! (`docs/design/03-interface-prior-art.md` §5.3(3), R5). What follows from
//! naming it is the summary line: it reads **what is left** (`3 of 12 · 38:12
//! left`), not what the list contains. A queue is a thing you are partway
//! through, and the total running time answers a question nobody came here to
//! ask.
//!
//! # Marking
//!
//! The playing row is marked the way the shelf marks the playing album and the
//! way the record's page marks the playing track — the amber lamp dot, in place
//! of the row's number — because that is the one thing on this surface that
//! *is* playback truth, and the palette reserves the accent for exactly that.
//!
//! # Edit parity, and the virtual window
//!
//! Since doc 09 §13 step 5 the rows carry the playlist page's whole reserved
//! edit set — ▲▼ steppers, ✕, and the transfer `+` — so the queue place and
//! the playlist page are **the same editor** (09 §8.2), differing only in
//! their header blocks: the artefact's name and acts there, the run's
//! noun-led summary and `Save as playlist` here.
//!
//! **That is the whole of the difference, and until 2026-08-10 the surface
//! never said so.** The owner: *"'save as playlist' really makes no sense on
//! the playlist page for a CD"* — he was reading this column, and the reading
//! was fair: the strip said `1 of 24 · 1:56:19 left`, a run reading with no
//! subject; the word beside it offered to save something; and 57 px below
//! stood the record's own title. So the strip now leads with a noun in both
//! branches (`Run · …`, or the list's name) and the word states what it is
//! saving — see [`save_control`] and ADR-0024 §A5.
//!
//! And since `Play all`
//! (09 §7.1) can reify a whole library into this list, the rows are drawn
//! through [`crate::queue_window`]'s virtual window — everything off screen
//! is two spacers, the wall's own discipline at list scale.

use std::borrow::Cow;

use iced::widget::{
    Column, Space, button, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::{PlayerState, QueueRow, QueueRowState, RunOrigin};
use crate::playlists::{Collecting, NameEntry};
use crate::queue_window::{self, RowShape};
use crate::views::{page, place_header_led, place_name, place_pad, playlist_sleeve};
use crate::{icon, theme};

/// The `Save as playlist` field's id, so the caret can land in it the moment
/// the word becomes a field.
pub(crate) fn save_name_id() -> text_input::Id {
    text_input::Id::new("baz-queue-save")
}

/// **The run column's own scrollable**, named so the shell can drive it.
///
/// The owner, 2026-08-10: *"ideally the currently playing item in the playlist
/// is where our scroll goes to i.e. it should be visible when we change
/// track"*. A follow is a `scrollable::scroll_to` and iced 0.13 addresses a
/// widget by id, so the column has one — the same construction the save field
/// above already uses for its caret.
pub(crate) fn run_scroll_id() -> scrollable::Id {
    scrollable::Id::new("baz-run-column")
}

/// **Where the rows column begins inside the scrollable's content** — the
/// place's top pad, the summary strip, the column's own gaps, and the run's
/// head block where there is one.
///
/// Published rather than inlined because **two surfaces have to agree about
/// it**: this module builds the column from it, and the shell computes the
/// playing-row follow offset from the same geometry. A private
/// copy on each side is a pair of numbers that drift, and the symptom would be
/// a follow that lands a header or a strip's height off the row it was aiming
/// at.
///
/// The identity head's editable title has the same declared head height as its
/// resting title; validation text is the only transient variation, and the
/// virtual window's [`queue_window::MARGIN`] absorbs that one quiet line.
#[must_use]
pub(crate) fn rows_top(pad_top: f32, head_h: f32, head_two_line: bool) -> f32 {
    pad_top
        + head_h
        + theme::TRANSPORT_HIT
        + 2.0 * theme::GAP_LG
        + theme::LINE_BODY
        + if head_two_line {
            theme::GAP_XXS + theme::LINE_META
        } else {
            0.0
        }
        + theme::GAP_XS
}

/// Everything the run column needs of the surface drawing it.
///
/// `measure` is the width the rows are set at, `viewport_h` bounds the virtual
/// window's span, `scroll` is where the one scrollable last said it was
/// ([`Message::QueueScrolled`]), and `pad` is the gutter the column hangs from.
///
/// **There was a fifth, `clearance`** — air reserved above the summary for the
/// place's top-right layer. That layer was the `Run` word and the owner removed
/// it, so the field went with it: height held for a control that does not exist
/// is the thing the place's own arithmetic refuses everywhere else. Step A6's
/// `Ambient` door brings its own back if it claims that corner.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frame {
    /// The measure the rows are set at, scrollbar lane already taken off.
    pub(crate) measure: f32,
    /// The viewport the virtual window is computed against.
    pub(crate) viewport_h: f32,
    /// Where the scrollable last said it was.
    pub(crate) scroll: f32,
    /// The gutter the column hangs from — [`crate::views::place_pad`] when the
    /// column owns
    /// the body's whole width, and the right-hand column's own inset when it
    /// stands beside the record.
    pub(crate) pad: iced::Padding,
}

/// The unsaved playlist as a full place: its standard header and the retained
/// run editor beneath it.
#[expect(
    clippy::too_many_arguments,
    reason = "the call site hands this dedicated place the same independent \
              editor readings its run column consumes; wrapping them in a \
              second state type would only duplicate `Frame` and the column's \
              named arguments"
)]
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    window: iced::Size,
    hovered: Option<usize>,
    saving: Option<&'a NameEntry>,
    collecting: Collecting,
    scroll: f32,
    drag: Option<&'a crate::drag::DragState>,
    can_undo: bool,
) -> Element<'a, Message> {
    let measure =
        (window.width - 2.0 * theme::HANG - theme::SCROLLBAR_LANE).clamp(0.0, theme::LIST_MEASURE);
    let name = unsaved_name(player.queue_origin());
    let head = identity_head(shelf, player, &name, saving, window.width);
    column![
        place_header_led(place_name(&name), None),
        run_column(
            player,
            Frame {
                measure,
                viewport_h: window.height,
                scroll,
                pad: place_pad(),
            },
            Some(head),
            hovered,
            saving,
            collecting,
            drag,
            can_undo,
        ),
    ]
    .into()
}

/// The title of an unsaved list. An artist's implicit list is deliberately
/// named like the playlist it is about to become: `All Anne-Marie Puig`, not
/// the generic action label printed on the artist tile.
pub(crate) fn unsaved_name(origin: Option<&crate::origin::Origin>) -> String {
    match origin {
        Some(crate::origin::Origin::Artist { name, .. }) => format!("All {name}"),
        Some(crate::origin::Origin::AllSongs) => "All songs".to_owned(),
        Some(origin) => origin.name().to_owned(),
        None => "Queue".to_owned(),
    }
}

/// The first four distinct records represented by the queue, in queue order.
///
/// Queue rows already carry the record title and filed-under artist used by
/// the shelf, so resolving those pairs is both cheaper and more faithful than
/// walking every path in every edition on every frame. Four ids are the whole
/// supply [`playlist_sleeve`] can spend.
pub(crate) fn unsaved_art(shelf: &Shelf, player: &PlayerState) -> Vec<u64> {
    let Some(queue) = player.queue() else {
        return Vec::new();
    };
    let mut art = Vec::new();
    for item in &queue.items {
        let filed_under = item.album_artist.as_deref().unwrap_or(&queue.artist);
        let Some(album) = shelf
            .albums
            .iter()
            .find(|album| album.title == item.album && album.artist.label() == filed_under)
        else {
            continue;
        };
        if !art.contains(&album.id) {
            art.push(album.id);
            if art.len() == 4 {
                break;
            }
        }
    }
    art
}

/// Give the unsaved run the same identity vocabulary as a stored playlist:
/// collage, prominent sans name, kind/byline, then counts. When saving begins,
/// that prominent name itself becomes the field; editing controls remain in
/// the run strip immediately below.
fn identity_head<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    name: &str,
    saving: Option<&'a NameEntry>,
    window_width: f32,
) -> (Element<'a, Message>, f32) {
    let art = unsaved_art(shelf, player);
    let queue = player.queue();
    let tracks = queue.map_or(0, |queue| queue.items.len());
    let records = queue.map_or(0, |queue| {
        let mut groups: Vec<(Option<&str>, &str)> = Vec::new();
        for item in &queue.items {
            let group = (
                item.album.as_deref(),
                item.album_artist.as_deref().unwrap_or(&queue.artist),
            );
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
        groups.len()
    });
    let mut facts = match tracks {
        1 => "1 track".to_owned(),
        tracks => format!("{tracks} tracks"),
    };
    if let Some(queue) = queue {
        let time = queue.total_time();
        if time > std::time::Duration::ZERO {
            facts.push_str(" · ");
            facts.push_str(&crate::vm::format_duration(time));
        }
    }
    let byline = match records {
        1 => "Unsaved playlist · 1 record".to_owned(),
        records => format!("Unsaved playlist · {records} records"),
    };
    let identity = page::identity_block(page::Identity {
        name: name.to_owned(),
        face: theme::SEMIBOLD,
        edit: saving.map(|entry| page::NameEdit {
            value: &entry.text,
            error: entry.error.as_deref(),
            id: save_name_id(),
            on_input: Message::SaveQueueInput,
            on_submit: Message::SaveQueueSubmit,
        }),
        byline,
        facts,
        beside_facts: None,
    });
    let sleeve = playlist_sleeve(shelf, &art, name, theme::ALBUM_SLEEVE);
    if window_width >= theme::ALBUM_BREAKPOINT {
        (
            row![sleeve, container(identity).width(Length::Fill)]
                .spacing(theme::GAP_XL)
                .align_y(iced::Alignment::Start)
                .into(),
            theme::ALBUM_SLEEVE,
        )
    } else {
        (
            column![sleeve, identity].spacing(theme::GAP_XL).into(),
            theme::ALBUM_SLEEVE + theme::GAP_XL + 80.0,
        )
    }
}

/// **The run column**: the summary, the acts beside it, and the rows —
/// **virtualized**, so `Play all`'s five-figure run costs the frame what a
/// twelve-track record does (doc 09 §7.1's implementation gate;
/// [`crate::queue_window`] owns the arithmetic, this file draws the slice it
/// is handed, exactly as the wall's `views/shelf.rs` does for
/// [`crate::shelf::Grid`]).
///
/// It is the body of [`crate::place::Place::Queue`]. `head` remains generic
/// because the renderer once stood beside Now playing's record column; the
/// dedicated place now passes its playlist identity block and sleeve.
///
/// Every string here is *owned*, straight from [`PlayerState::queue_list`]'s
/// render-ready reading, which is why the element is `'static`: the contents
/// are a projection of engine events and a request-side record, not a borrow of
/// the library, so nothing on screen can outlive a view-model rebuild mid-scan.
// The `too_many_lines` expectation this carried is **gone rather than
// silenced**: the rows loop no longer spells a row's anatomy, so the column is
// the summary strip and the windowed loop, and that fits.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is one independent reading the column renders — \
              the drag in flight and the undo affordance arrived from two \
              different studies, and bundling them into a struct would name \
              nothing the call site does not already say"
)]
pub(crate) fn run_column<'a>(
    player: &'a PlayerState,
    frame: Frame,
    head: Option<(Element<'a, Message>, f32)>,
    hovered: Option<usize>,
    saving: Option<&'a NameEntry>,
    collecting: Collecting,
    drag: Option<&'a crate::drag::DragState>,
    can_undo: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let Frame {
        measure,
        viewport_h,
        scroll,
        pad,
    } = frame;
    let (head, head_h) = match head {
        Some((element, height)) => (Some(element), height + theme::GAP_XL),
        None => (None, 0.0),
    };
    // A row is only a control when there is an engine to send its command to.
    let live = player.engine_ready();
    let body: Element<'a, Message> = match player.queue_list() {
        None => empty_state(),
        Some(list) => {
            // The window over the rows: every element outside it is part of
            // one of two spacers. What the arithmetic needs of each row is
            // its shape — a header opening above it, an artist line under
            // its title — and the offsets are handed straight to the module
            // (`queue_window::MARGIN` absorbs the estimate below).
            let shapes: Vec<RowShape> = list
                .rows
                .iter()
                .map(|row_state| RowShape {
                    head: row_state.head.as_ref().map(|head| head.album.is_some()),
                    two_line: row_state.artist.is_some(),
                })
                .collect();
            // Where the rows column begins inside the scrollable content:
            // the place's top pad, the summary strip, the column gaps and
            // the list's own head block. Validation can add one quiet line to
            // the editable identity; the module's margin absorbs it.
            let rows_top = rows_top(pad.top, head_h, list.album.is_some());
            let win = queue_window::window(&shapes, scroll - rows_top, viewport_h);
            // A record's name where the record begins, then its tracks —
            // **albums listed as albums, never flattened** (ADR-0014). Each
            // element is boxed at exactly the pitch the module declared for
            // it (spacing 0; the gap is folded into the pitch), so the
            // spacers and the drawn slice cannot disagree about the list.
            let mut rows: Vec<Element<'static, Message>> = Vec::new();
            rows.push(Space::with_height(Length::Fixed(win.top)).into());
            for index in win.first..win.end {
                let row_state = list.rows[index].clone();
                if let Some(head) = row_state.head.clone() {
                    rows.push(
                        // A head that is not the list's own opener takes its
                        // `GAP_MD` of air above, so the break belongs to the
                        // record it opens.
                        container(page::list_head(head.album.as_deref(), &head.artist, false))
                            .height(Length::Fixed(queue_window::header_pitch(
                                head.album.is_some(),
                            )))
                            .align_y(alignment::Vertical::Top)
                            .into(),
                    );
                }
                let two_line = row_state.artist.is_some();
                rows.push(
                    container(queue_row(
                        row_state,
                        index,
                        list.rows.len(),
                        live,
                        hovered == Some(index),
                        collecting,
                        drag.and_then(|held| held.line_for_row(crate::drag::List::Queue, index)),
                        drag.is_some_and(|held| held.list == crate::drag::List::Queue),
                    ))
                    .height(Length::Fixed(queue_window::row_pitch(two_line)))
                    .align_y(alignment::Vertical::Top)
                    .into(),
                );
            }
            rows.push(Space::with_height(Length::Fixed(win.bottom)).into());
            column![
                // The summary shares its line with the one act the transient
                // earns: freezing tonight's run into a file (ADR-0024 §4,
                // prior art's W19). A new file and nothing else — the queue
                // does not become linked to the playlist it seeded.
                row![
                    text(list.summary)
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_faint)
                        .wrapping(text::Wrapping::None),
                    // The transient `Undo`, beside the summary (doc 11 §5
                    // P2): present exactly while there is an edit to take
                    // back, gone otherwise — no toast, no popover, no
                    // timer, a word in a strip in the product's own
                    // grammar. Ctrl+Z is its accelerator; this word is
                    // what makes the accelerator legal.
                    undo_control(can_undo),
                    Space::with_width(Length::Fill),
                    // …and the save word, which now reads the run rather
                    // than only its own name field (ADR-0024 §A5.2). The
                    // reading is the player's — no argument from the place
                    // above, because the place knows nothing about this that
                    // `PlayerState` does not already hold.
                    save_control(saving.is_none(), player.run_origin()),
                ]
                .spacing(theme::GAP_SM)
                .align_y(iced::Alignment::Center),
                column![
                    page::list_head(list.album.as_deref(), &list.artist, true),
                    Column::with_children(rows),
                ]
                .spacing(theme::GAP_XS),
            ]
            .spacing(theme::GAP_LG)
            .into()
        }
    };
    // The head — the record, when the body is too narrow to stand it beside
    // the run (§5.5a) — is *inside* the scroll, and that is deliberate: at
    // this width the surface has become the editor, and an editor whose
    // first 300 px are a fixed hero is an editor you scroll past to use.
    let body: Element<'a, Message> = match head {
        None => body,
        Some(head) => column![head, body].spacing(theme::GAP_XL).into(),
    };
    // **No clearance strip.** The column used to open with
    // `TRANSPORT_HIT + GAP_LG` of air, reserved for the place's top-right
    // layer; that layer was the `Run` word and the owner has removed it
    // (`views::now_playing`'s module docs). Air held for a control that does
    // not exist is the defect `now_playing`'s own `BELOW` already refuses on
    // the other column, so it goes with the word, and the summary starts at
    // the place's own gutter.
    //
    // Step A6's `Ambient` door, if it claims that corner, brings its own
    // clearance back with it — whole, and measured against what it draws.

    // One scroll for the run, with the bar's lane reserved whether or not
    // the list overflows — the same reserved-slot rule the durations
    // depend on, and the reason a thirteenth track arriving shunts none of
    // them sideways.
    scrollable(
        container(container(body).width(Length::Fixed(measure)))
            .width(Length::Fill)
            .padding(pad)
            .align_x(alignment::Horizontal::Center),
    )
    .id(run_scroll_id())
    .on_scroll(Message::QueueScrolled)
    .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
    .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// **Undo** — the run as it stood before the last edit, restored
/// (doc 11 §5 P2). Drawn only while there is an edit to take back: a
/// standing "Undo" over a list nobody has edited would be a control that
/// cannot act pretending it can. A word in the summary strip rather than a
/// toast, because forgiveness is a fact about the place, not an
/// interruption; quiet, because restoring a list is an act on the run, not
/// on playback — nothing sounds because of it.
fn undo_control(offered: bool) -> Element<'static, Message> {
    if !offered {
        return Space::with_width(Length::Fixed(0.0)).into();
    }
    let room = theme::active();
    button(
        container(
            text("Undo")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(Message::Undo)
    .into()
}

/// **Save as playlist** — the transient frozen into an artefact
/// (ADR-0024 §4): a labelled word beside the summary, quiet because it is an
/// act on a file rather than on playback.
///
/// Offered only while the name field is closed: the page title becomes that
/// field mid-gesture, and drawing both would be one act with two live buttons.
///
/// # …and only when it can usefully act (ADR-0024 §A5.2)
///
/// The owner, 2026-08-10: *"'save as playlist' really makes no sense on the
/// playlist page for a CD"*. He is reading this word, on the surface the
/// queue merged into, and he is right twice over: the strip beside it prints
/// the run's provenance, so the control offered to save a thing whose name
/// was already two inches to its left; and over a record's run it manufactured
/// a one-record playlist whose sleeve is that record's own cover, which then
/// landed in the lane above the record it came from wearing its face
/// (design 14 §0). It was conditioned on **nothing** but whether its own name
/// field was open.
///
/// # …and only for a run the listener assembled (the owner, 2026-08-10)
///
/// *"I still see save as playlist on the queue when playing a CD... we should
/// only be showing that in a situation where there isn't an existing
/// playlist"*, and, narrowing it the same afternoon, *"nah I think adding more
/// stuff to an existing playlist is fine, that does not need a save -- it's a
/// low bar to edit a playlist"*.
///
/// So it reads [`PlayerState::run_origin`], which now knows the **three kinds
/// of list** ([`crate::vm::RunSource`]) rather than only whether a file was
/// named, and takes the shape that reading permits:
///
/// | the run | the strip |
/// |---|---|
/// | [`RunOrigin::Fixed`] — a record's, `All songs`, `Play all` | **nothing**, in a reserved slot |
/// | [`RunOrigin::Saved`] — reified from a file, unedited | `Saved as “Road Trip”`, a **readout** |
/// | [`RunOrigin::Diverged`] — reified from a file, since edited | `From “Road Trip”`, a **readout** |
/// | [`RunOrigin::Assembled`] — built by hand, or a fixed run since edited | `Save as playlist`, live |
///
/// **The precedent is eleven lines up.** [`undo_control`] is drawn only while
/// there is an edit to take back, because *"a standing `Undo` over a list
/// nobody has edited would be a control that cannot act pretending it can"*.
/// This was that defect and this is that cure.
///
/// **`Diverged` says `From`, not `Saved as`, and not `Save as new playlist`.**
/// Two refusals in one word, and they pull opposite ways so both have to be
/// stated:
///
/// - it may not read `Saved as “Road Trip”`, because after an edit the run is
///   **not** that file — that is exactly the lie ADR-0024 §A5.2 removed, and
///   it must not come back through this door;
/// - it may not offer a new file either, because the owner's second quote
///   says a run that came from a playlist has a cheap route to changing one
///   and does not need a second.
///
/// So it names its origin and asserts nothing about its identity.
///
/// **Nothing here writes back**, and the narrowing did not change that.
/// *"A low bar to edit a playlist"* is an argument about how easy the
/// playlist's own page is to reach, not an instruction to make the run an
/// editor of files: ADR-0024 §1 decouples the two in **both** directions and
/// ADR-0023 §3 makes provenance an origin rather than a live link, so a queue
/// edit made for tonight still cannot touch a file somebody owns.
///
/// Every state is built at this control's own height and inset — including the
/// empty one, which is a `Space` of exactly [`theme::TRANSPORT_HIT`] rather
/// than an absence. The strip is the same strip in all four states, so nothing
/// above or below it moves when a run is edited, and the run column's
/// `rows_top` arithmetic stays true whichever word is in it.
fn save_control(offered: bool, origin: RunOrigin<'_>) -> Element<'static, Message> {
    let room = theme::active();
    let readout = |line: String| -> Element<'static, Message> {
        container(
            container(
                text(line)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None),
            )
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center),
        )
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(theme::pad(0.0, theme::GAP_SM))
        .into()
    };
    let label = match origin {
        RunOrigin::Assembled => "Save as playlist".to_owned(),
        // A list that already exists says nothing, and holds its height.
        RunOrigin::Fixed => {
            return Space::with_height(Length::Fixed(theme::TRANSPORT_HIT)).into();
        }
        RunOrigin::Saved(name) => return readout(format!("Saved as “{name}”")),
        RunOrigin::Diverged(name) => return readout(format!("From “{name}”")),
    };
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press_maybe(offered.then_some(Message::SaveQueueStart))
    .into()
}

/// Nothing queued yet: said plainly, with the gesture that fills it.
///
/// Quiet text rather than an illustration or a call to action — an empty queue
/// is the ordinary state of a player nobody has pressed play on, not a problem
/// to solve.
pub(crate) fn empty_state() -> Element<'static, Message> {
    let room = theme::active();
    container(
        column![
            text("Nothing queued")
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .color(room.paper_dim),
            text("Play an album and it appears here.")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
            // Silence is a feature (a standing rule of the product), and the empty queue is
            // the one surface where saying so costs nothing: this is what a
            // listener sees the moment a record ends, and it is the frame in
            // which every other player would have started something. Since
            // doc 11 §5 P6.3 the line carries its missing half — the
            // refusal stated *with* the answers ADR-0023 §5 says exist in
            // advance, at the exact moment the refusal is felt. ("Plays the
            // Library", not "the wall": room vocabulary stays internal,
            // P4's rule, applied to P6's own sentence.)
            text(
                "When a queue ends, baz stops. All songs is a tile on Home; \
                 Play all plays the wall.",
            )
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_muted),
        ]
        .spacing(theme::GAP_SM)
        .align_x(iced::Alignment::Start),
    )
    .width(Length::Fill)
    .align_x(alignment::Horizontal::Left)
    .into()
}

/// One queue row: position (or the lamp dot when it is playing), title over its
/// artist where there is one, right-aligned duration — and the full reserved
/// edit set a list in baz carries (doc 09 §8.2: **the queue place and the
/// playlist page are the same editor**).
///
/// **Clicking the row plays from there.** ADR-0014's `JumpTo`, and this list is
/// the one place it needs no decision at all: the rows are drawn from the
/// record of what was handed to the engine, so a row's index *is* a queue
/// position. Nothing is re-queued, and the mark follows `TrackStarted` rather
/// than the click.
///
/// **The ▲▼ steppers reorder** — the playlist page's exact slots
/// (`views/playlist.rs`), sending the whole edited list as `UpdateQueue`
/// through the pure [`queue_edit::shifted`](crate::queue_edit::shifted): the
/// music keeps playing (ADR-0014's guarantee), and the cursor follows its
/// track — the sounding row moves like any other, because the engine finds
/// it again by path.
///
/// **The ✕ takes the entry out**, through `UpdateQueue` and the pure
/// [`queue_edit`](crate::queue_edit) helper — an edit that does not touch the
/// playing track does not disturb one delivered sample, which is the guarantee
/// ADR-0014 exists to make and the reason this is not a `SetQueue`.
///
/// **The `+` is the transfer slot** (doc 09 §8.1): one press holds this row's
/// track and opens the panel as the picker — a destination list, the Queue's
/// own row first among them — so a run being auditioned can seed a kept list
/// row by row (S9a). It is on the sounding row too: the track you are hearing
/// is the one most worth keeping. Drawn at the row's outer edge, where the
/// album page's rows put the same slot; the ▲▼✕ keep the playlist page's
/// exact positions.
///
/// Two fixed-slot rules, because a list that changes under the reader is
/// exactly where movement is least affordable:
///
/// - the number column is [`theme::TRACK_NO_W`] wide whichever it holds, so the
///   dot arriving as a track starts moves no text;
/// - **every edit slot is reserved whether or not its control is in it.** The
///   controls appear on hover — a column of permanent crosses down a list of
///   what you are about to hear is a column of invitations to destroy
///   something — but if their width came and went with them, every duration
///   would slide sideways as the pointer crossed a row. (The `+` alone is
///   also drawn at rest while the panel stands, the album page's own rule:
///   the picker on screen is the task the mark belongs to.)
///
/// **The row's body is a drag source** (doc 09 §13 step 8; [`crate::drag`]
/// carries the gesture and its laws): press and move past the threshold to
/// lift the row — reorder against the insertion line, or carry it over the
/// standing panel's rows to add — while a sub-threshold press stays this
/// row's ordinary click. Sugar only: the ▲▼ steppers, the ✕ and the `+`
/// remain exactly as above. `line` is the insertion edge this row draws
/// while a drag is in flight; `observing` has the row measure the held
/// pointer against its own bounds — both from the shell's one drag state.
#[expect(
    clippy::too_many_arguments,
    reason = "a row is one anatomy and these are its readings; a struct \
              would name this call site and nothing else"
)]
fn queue_row(
    row_state: QueueRow,
    index: usize,
    total: usize,
    live: bool,
    hovered: bool,
    collecting: Collecting,
    insert_line: Option<crate::drag::Edge>,
    observing: bool,
) -> Element<'static, Message> {
    let room = theme::active();
    let playing = row_state.state == QueueRowState::Playing;
    let ink = match row_state.state {
        QueueRowState::Played => room.paper_faint,
        QueueRowState::Playing | QueueRowState::Next | QueueRowState::Upcoming => room.paper,
    };
    // **Filled means sounding; open means about to.** The two marks share the
    // number lane, the dot's size and its lattice, so the run column says what
    // is playing and what is next in one vocabulary and one column of pixels.
    //
    // It earns its place hardest under shuffle, where the next track is *not*
    // the row below and nothing else on screen could tell you which one it is
    // — but it is drawn in both modes, because the fact is true in both and an
    // interface that only marks what is next when it is surprising is one that
    // has decided when you are allowed to know.
    //
    // **The ring is this surface's own**, and it is the one part of the row a
    // page cannot use: a document has no cursor, so it has no *next*. The dot
    // beside it is [`page::lamp_dot`], shared, because *sounding* is a fact
    // every surface states.
    let marker: Element<'static, Message> = match row_state.state {
        QueueRowState::Playing => page::lamp_dot(),
        QueueRowState::Next => next_ring(),
        _ => text(row_state.position.to_string())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into(),
    };
    let body = page::track_row(page::TrackRow {
        marker,
        artwork: None,
        title: row_state.title.into(),
        ink,
        under: row_state
            .artist
            .map(|artist| (Cow::Owned(artist), room.paper_dim, None)),
        context: None,
        duration: row_state.duration.into(),
        playing,
        press: live.then_some(Message::JumpToQueued(index)),
    });
    // The drag wrapper owns the pointer for the body (crate::drag's module
    // docs): live rows lift on threshold and still click under it; every
    // row of the list measures a drag in flight against its own bounds.
    let mut source = crate::drag::Source::new(body, room);
    if live {
        source = source.wires(crate::drag::Wires::new(
            move |at| Message::DragLift(crate::drag::List::Queue, index, at),
            Message::DragMoved,
            Message::DragDropped,
            Some(Message::JumpToQueued(index)),
        ));
    }
    if observing {
        source = source
            .observe(move |before| Message::DragOverRow(crate::drag::List::Queue, index, before));
    }
    let body: Element<'static, Message> = source.line(insert_line).into();
    let offered = live && hovered;
    let mut slots = row![
        body,
        // The reorder pair and the removal cross, in the composition's one
        // slot anatomy ([`page::icon_slot`]) — the same three this file drew
        // for itself in three private functions that were byte-for-byte the
        // shared one.
        page::icon_slot(
            icon::Glyph::ArrowUp,
            "Move up",
            index > 0,
            offered,
            Message::ShiftQueued(index, -1)
        ),
        page::icon_slot(
            icon::Glyph::ArrowDown,
            "Move down",
            index + 1 < total,
            offered,
            Message::ShiftQueued(index, 1),
        ),
        page::icon_slot(
            icon::Glyph::Close,
            "Remove from the queue",
            true,
            offered,
            Message::RemoveQueued(index),
        ),
    ]
    .spacing(theme::GAP_XS)
    .align_y(iced::Alignment::Center);
    if collecting.available {
        // The transfer slot needs no engine — a pick can land in a file —
        // so it is offered on hover alone, and at rest while the panel
        // stands (the album page's own rule).
        slots = slots.push(page::transfer_slot(
            collecting.panel_open || hovered,
            Message::AddQueuedToPlaylist(index),
        ));
    }
    // The row's right press opens its mirror menu (doc 09 §5.2): play,
    // the transfer verbs, remove — each a press this row's own controls
    // already make.
    crate::menu::area(
        mouse_area(slots)
            .on_enter(Message::QueueRowEntered(index))
            .on_exit(Message::QueueRowLeft(index)),
        crate::menu::Target::QueueRow { row: index },
    )
}

/// **The next row's mark**: the lamp dot's ring, unlit.
///
/// The same [`theme::DOT`] box in the same lane, drawn as a hairline circle in
/// [`theme::Palette::paper_dim`] instead of a filled disc in the lamp colour.
/// Deliberately **not** the accent: the accent is playback truth — it means
/// *this is sounding now* everywhere in the product (`theme`'s
/// accent-discipline note) — and a track that has not started is not sounding.
/// So the shape carries the relationship (this is the dot's sibling) and the
/// ink carries the difference (it is not lit yet).
fn next_ring() -> Element<'static, Message> {
    let room = theme::active();
    container(Space::new(
        Length::Fixed(theme::DOT),
        Length::Fixed(theme::DOT),
    ))
    .style(move |_theme| iced::widget::container::Style {
        border: iced::Border {
            color: room.paper_dim,
            // The product's hairline, the literal `theme` itself uses for
            // every 1 px border it draws.
            width: 1.0,
            radius: (theme::DOT / 2.0).into(),
        },
        ..iced::widget::container::Style::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::unsaved_name;

    #[test]
    fn an_artist_run_is_named_as_the_unsaved_playlist_it_can_become() {
        let origin = crate::origin::Origin::Artist {
            id: 7,
            name: "Mahavishnu Orchestra".to_owned(),
        };
        assert_eq!(unsaved_name(Some(&origin)), "All Mahavishnu Orchestra");
        assert_eq!(
            unsaved_name(Some(&crate::origin::Origin::AllSongs)),
            "All songs"
        );
    }

    /// **The place draws the window's slice and nothing else** (doc 09 §7.1's
    /// gate on `Play all`), and **its rows carry the playlist page's whole
    /// edit set** (09 §8.2 — one editor).
    ///
    /// Pinned over the source the way `views/shelf.rs` pins its own ruler:
    /// the properties are about which widgets this file builds, there is no
    /// `PlayerState` to construct without an engine, and the literals below
    /// are exactly what a reviewer would have to delete to break them.
    #[test]
    fn the_queue_place_is_virtual_and_its_rows_are_the_playlist_editors() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/queue.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        // **The code half only** — `views::page`'s own `pages()` rule, and here
        // it is load-bearing rather than tidy.
        //
        // This test searches **the file it lives in**, so until 2026-08-10
        // every needle below was satisfied by the assertion that spelled it and
        // the test could not fail. It had gone stale twice without anyone
        // noticing: it looked for `window.height` after that argument was
        // renamed `viewport_h`, and it looked for a reserved-slot literal that
        // had moved to `views::page` entirely. Both passed.
        //
        // `now_playing.rs` and `implicit.rs` solve this by spelling their
        // needles in halves; that works for a handful and not for twenty, so
        // this one takes the same cut `views::page::tests::pages` takes and
        // searches only what the module actually builds.
        let source = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head")
            .to_owned();

        // The window: the rows loop spends `queue_window`'s slice, both
        // spacers are built, and every drawn element is boxed at the pitch
        // the module declared — which is what keeps the spacers honest.
        assert!(
            source.contains("queue_window::window(&shapes, scroll - rows_top, viewport_h)"),
            "the rows are windowed by the pure module"
        );
        assert!(
            source.contains("for index in win.first..win.end"),
            "only the window's slice is built"
        );
        assert!(
            source.contains("Space::with_height(Length::Fixed(win.top))")
                && source.contains("Space::with_height(Length::Fixed(win.bottom))"),
            "everything off screen is two spacers"
        );
        assert!(
            source.contains("queue_window::row_pitch(two_line)")
                && source.contains("queue_window::header_pitch("),
            "drawn elements are boxed at the module's own pitches"
        );

        // The parity slots: ▲▼ on the queue's own edit message, the ✕, and
        // the transfer `+` toward the picker — every slot reserved.
        for spent in [
            "Message::ShiftQueued(index, -1)",
            "Message::ShiftQueued(index, 1)",
            "Message::RemoveQueued(index)",
            "Message::AddQueuedToPlaylist(index)",
        ] {
            assert!(
                source.contains(spent),
                "a queue row's reserved slots spend `{spent}`"
            );
        }
        // …and the reservation itself is `views::page::icon_slot`'s now — this
        // file had three private copies of it, byte-for-byte, until the rows
        // became one anatomy. The claim is unchanged and the address moved.
        let shared = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/page.rs"),
        )
        .expect("the shared composition's source")
        .replace("\r\n", "\n");
        assert!(
            shared.contains("Space::with_width(Length::Fixed(theme::STEPPER_HIT))"),
            "an unoffered slot is a space of exactly the control's width"
        );
        assert!(
            source.matches("page::icon_slot(").count() == 3
                && source.contains("page::transfer_slot("),
            "a queue row's four reserved slots are the composition's own"
        );

        // Step 8: the row's body is a drag source, and the drag is sugar —
        // it wraps the body *beside* the reserved slots above, all of
        // which the assertions before this one prove still stand. The
        // sub-threshold click is the row's own press, and the observation
        // wire is what makes the insertion index exact under this place's
        // virtualization.
        assert!(
            source.contains("crate::drag::Source::new(body, room)"),
            "the row's body is wrapped as a drag source"
        );
        assert!(
            source.contains("Some(Message::JumpToQueued(index)),"),
            "a sub-threshold press is still the row's click"
        );
        assert!(
            source.contains("Message::DragOverRow(crate::drag::List::Queue, index, before)"),
            "every row measures a drag in flight against its own bounds"
        );
    }

    /// **The save word takes the shape the run permits** (ADR-0024 §A5.2, and
    /// the owner's narrowing of 2026-08-10) — **four** states, one slot, one
    /// height.
    ///
    /// The predicate itself is tested where it lives
    /// (`player::tests::the_save_word_offers_only_over_a_run_the_listener_assembled`);
    /// what is pinned here is that this file *spends* it, that every state is
    /// drawn at the button's own box so nothing in the strip moves when a run
    /// is edited — the empty one included, which is a reserved `Space` rather
    /// than an absence — and that no state offers a write-back. Over the
    /// source, for this module's own reason: there is no `PlayerState` to
    /// construct without an engine.
    #[test]
    fn the_save_word_is_a_readout_over_a_run_that_is_already_a_file() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/queue.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");

        assert!(
            source.contains("save_control(saving.is_none(), player.run_origin())"),
            "the strip's word reads the run, not only its own name field"
        );
        for (arm, word) in [
            ("RunOrigin::Assembled =>", "\"Save as playlist\""),
            ("RunOrigin::Saved(name) =>", "\"Saved as “{name}”\""),
            ("RunOrigin::Diverged(name) =>", "\"From “{name}”\""),
        ] {
            assert!(
                source.contains(arm) && source.contains(word),
                "the `{arm}` run no longer wears {word}"
            );
        }
        // **A fixed run says nothing, and holds its height.** The slot is the
        // strip's own `TRANSPORT_HIT`, because the run column's `rows_top`
        // sums that box whichever of the four states is in it — a strip that
        // shrank when a CD started playing would window the wrong rows.
        let fixed = source
            .split("RunOrigin::Fixed => {")
            .nth(1)
            .expect("the fixed arm");
        let fixed = &fixed[..fixed.find("\n        }").unwrap_or(fixed.len())];
        assert!(
            fixed.contains("Space::with_height(Length::Fixed(theme::TRANSPORT_HIT))"),
            "a fixed run's slot stopped reserving the strip's height"
        );
        assert!(
            !fixed.contains("text(") && !fixed.contains("button("),
            "a list that already exists is being offered something"
        );
        // The readout is a statement, so it is not a button and it sends
        // nothing — the panel's `Queue` row's own form at rest. A disabled
        // button would still be a control claiming an act.
        let readout = source
            .split("let readout = |line: String|")
            .nth(1)
            .expect("the readout builder");
        let readout = &readout[..readout.find("\n    };").unwrap_or(readout.len())];
        assert!(
            !readout.contains("button(") && !readout.contains("on_press"),
            "a readout states; it does not offer"
        );
        assert!(
            readout.contains("Length::Fixed(theme::TRANSPORT_HIT)")
                && readout.contains("theme::pad(0.0, theme::GAP_SM)"),
            "…at the word's own box and inset, so the strip does not move"
        );
        // And a write-back is refused, in the code as well as in the record:
        // ADR-0024's 2026-08-09 amendment item 6 and ADR-0023 §3. The prose
        // is stripped first, because naming a refusal in a doc comment is how
        // the refusal survives the next reader.
        let code: String = source
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                !line.starts_with("///") && !line.starts_with("//!") && !line.starts_with("//")
            })
            .collect::<Vec<_>>()
            .join("\n");
        // Spelled in two halves so this assertion is not itself the thing it
        // is looking for.
        let write_back = concat!("Save changes", " to");
        assert!(
            !code.contains(write_back),
            "provenance is an origin, never a live link — a run that wrote \
             itself back would be the two-structure confusion returning"
        );
        // The creation act survives, which is the guard that makes this a fix
        // rather than a removal (`every_queue_affordance_survives_the_merge`).
        assert!(
            source.contains("Message::SaveQueueStart"),
            "freezing a transient into a file is still one press away"
        );
    }
}
