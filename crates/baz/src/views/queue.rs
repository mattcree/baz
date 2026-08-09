//! **The queue place**: what the engine is holding, and where it is in it.
//!
//! # It was a popover, and before that a rail panel
//!
//! ADR-0016 moved the queue out of the right-hand rail and into a 360 px card
//! floating over the wall, anchored to the bar it describes. ADR-0022 removed
//! every side surface baz had — the owner's verdict on the pair was *"I really
//! hate the way queue and selected albums appear"* — so the queue is a
//! **place**, at the width of the window, and the float is gone with the
//! column.
//!
//! What survives is every fact and every gesture: the rows, one list with a
//! cursor, the summary that reads *what is left*, click-to-jump, the per-row ✕.
//! What changes is the container and what the container costs:
//!
//! | | the popover | this place |
//! |---|---|---|
//! | width | `POPOVER_W` 360, fixed | the window, capped at [`theme::LIST_MEASURE`] |
//! | height | `0.6 × window`, then scroll | the window |
//! | dismissal | `Esc`, ✕, click-outside, the door again | `Esc`, `‹ Library`, the door again |
//! | arrival | a 140 ms fade and an 8 px rise | a hard cut, like every other place change |
//! | what it costs the wall | nothing — it floated | the wall, while you are here |
//!
//! The last row is the honest price and ADR-0022 states it: knowing what is
//! next used to cost nothing and now costs leaving the shelf. The mitigation is
//! that it mostly should not be paid — the bar's own third line states the
//! continuation ambiently ([`crate::views::bottom_bar`]), so this place is for
//! *changing* the queue rather than for reading it.
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
//! provenance-led summary and `Save as playlist` here. And since `Play all`
//! (09 §7.1) can reify a whole library into this list, the rows are drawn
//! through [`crate::queue_window`]'s virtual window — everything off screen
//! is two spacers, the wall's own discipline at list scale.

use iced::widget::{
    Column, Space, button, column, container, image as iced_image, mouse_area, row, scrollable,
    text, text_input, tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::player::{PlayerState, QueueRow, QueueRowState};
use crate::playlists::{Collecting, NameEntry};
use crate::queue_window::{self, RowShape};
use crate::views::{place_header, place_pad};
use crate::{icon, theme};

/// The `Save as playlist` field's id, so the caret can land in it the moment
/// the word becomes a field.
pub(crate) fn save_name_id() -> text_input::Id {
    text_input::Id::new("baz-queue-save")
}

/// The **Queue** place: the header strip, the summary, and the rows —
/// **virtualized**, so `Play all`'s five-figure queue costs the frame what a
/// twelve-track record does (doc 09 §7.1's implementation gate;
/// [`crate::queue_window`] owns the arithmetic, this file draws the slice it
/// is handed, exactly as the wall's `views/shelf.rs` does for
/// [`crate::shelf::Grid`]).
///
/// `window` decides two things: the width sets the list's measure — it grows
/// until [`theme::LIST_MEASURE`] and then stops, centring in what is left,
/// for the reason the record's page does the same — and the height bounds
/// the virtual window's span. `scroll` is where the place's one scrollable
/// last said it was ([`Message::QueueScrolled`]).
///
/// Every string here is *owned*, straight from [`PlayerState::queue_list`]'s
/// render-ready reading, which is why the element is `'static`: the contents
/// are a projection of engine events and a request-side record, not a borrow of
/// the library, so nothing on screen can outlive a view-model rebuild mid-scan.
#[expect(
    clippy::too_many_lines,
    reason = "the place is one composition — the summary strip, the save \
              field, and the windowed rows loop — and the loop's boxing \
              rules must stay in sight of the spacers they keep honest"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is one independent reading the place renders — \
              the drag in flight and the undo affordance arrived from two \
              different studies, and bundling them into a struct would name \
              nothing the call site does not already say"
)]
pub(crate) fn view<'a>(
    player: &'a PlayerState,
    window: iced::Size,
    hovered: Option<usize>,
    saving: Option<&'a NameEntry>,
    collecting: Collecting,
    scroll: f32,
    drag: Option<&'a crate::drag::DragState>,
    can_undo: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let measure =
        (window.width - 2.0 * theme::HANG - theme::SCROLLBAR_LANE).clamp(0.0, theme::LIST_MEASURE);
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
            // the list's own head block. An estimate — the save field, when
            // open, moves it by less than the module's margin absorbs.
            let head_two_line = list.album.is_some();
            let rows_top = theme::HANG
                + theme::TRANSPORT_HIT
                + 2.0 * theme::GAP_LG
                + theme::LINE_BODY
                + if head_two_line {
                    theme::GAP_XXS + theme::LINE_META
                } else {
                    0.0
                }
                + theme::GAP_XS;
            let win = queue_window::window(&shapes, scroll - rows_top, window.height);
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
                        container(album_group(
                            head.album.as_deref(),
                            &head.artist,
                            // One `GAP_MD` of air around a new record's name,
                            // so the break belongs to the record it opens.
                            theme::GAP_MD,
                        ))
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
                    save_control(saving.is_none()),
                ]
                .spacing(theme::GAP_SM)
                .align_y(iced::Alignment::Center),
                save_field(saving),
                column![
                    album_group(list.album.as_deref(), &list.artist, 0.0),
                    Column::with_children(rows),
                ]
                .spacing(theme::GAP_XS),
            ]
            .spacing(theme::GAP_LG)
            .into()
        }
    };
    column![
        place_header("Queue", "Esc returns to Library"),
        // One scroll for the place, with the bar's lane reserved whether or not
        // the list overflows — the same reserved-slot rule the durations
        // depend on, and the reason a thirteenth track arriving shunts none of
        // them sideways.
        scrollable(
            container(container(body).width(Length::Fixed(measure)))
                .width(Length::Fill)
                .padding(place_pad())
                .align_x(alignment::Horizontal::Center)
        )
        .on_scroll(Message::QueueScrolled)
        .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .into()
}

/// **Save as playlist** — the transient frozen into an artefact
/// (ADR-0024 §4): a labelled word beside the summary, quiet because it is an
/// act on a file rather than on playback.
///
/// Offered only while the name field is closed: the field below is the same
/// control mid-gesture, and drawing both would be one act with two live
/// buttons.
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

fn save_control(offered: bool) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text("Save as playlist")
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

/// The name field `Save as playlist` becomes, with the storage layer's
/// refusal surfaced plainly under it — or nothing at all while the word is at
/// rest.
fn save_field(saving: Option<&NameEntry>) -> Element<'_, Message> {
    let room = theme::active();
    let Some(entry) = saving else {
        return Space::with_height(Length::Fixed(0.0)).into();
    };
    let mut block = column![
        text_input("Name tonight's run…", &entry.text)
            .id(save_name_id())
            .on_input(Message::SaveQueueInput)
            .on_submit(Message::SaveQueueSubmit)
            .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .width(Length::Fixed(theme::SETTINGS_CONTENT_MIN))
            .style(move |_theme, status| theme::input(room, status)),
    ]
    .spacing(theme::GAP_XS);
    if let Some(error) = &entry.error {
        block = block.push(
            text(error.clone())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.alert),
        );
    }
    block.into()
}

/// The header over one album's run of rows: the record's title, and who it is
/// filed under, in the room's quietest voice.
///
/// **Albums are listed as albums, never flattened** — `docs/REFUSALS.md` by way
/// of the critique's stack. baz's queue is one list with a cursor and today it
/// usually holds one album, so there is usually one of these; a shuffle's run
/// already draws several, and a second album is a second header in this same
/// column with **no other part of this surface changing**.
fn album_group(album: Option<&str>, artist: &str, air: f32) -> Element<'static, Message> {
    let room = theme::active();
    let title = album.unwrap_or(artist);
    let mut block = column![
        text(title.to_owned())
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .font(theme::MEDIUM)
            .color(room.paper_dim)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XXS);
    // Only when it is not already the line above: an album with no title is
    // headed by its artist, and repeating the artist under itself says nothing.
    if album.is_some() {
        block = block.push(
            text(artist.to_owned())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.heading())
                .wrapping(text::Wrapping::None),
        );
    }
    // **On the place's own heading lane**, with no inset of its own — two
    // x-edges in this surface rather than four (law L5, and the audit's
    // defect 11).
    container(block).padding(theme::pad(air, 0.0)).into()
}

/// Nothing queued yet: said plainly, with the gesture that fills it.
///
/// Quiet text rather than an illustration or a call to action — an empty queue
/// is the ordinary state of a player nobody has pressed play on, not a problem
/// to solve.
fn empty_state() -> Element<'static, Message> {
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
            // Silence is a feature (`docs/REFUSALS.md`), and the empty queue is
            // the one surface where saying so costs nothing: this is what a
            // listener sees the moment a record ends, and it is the frame in
            // which every other player would have started something. Since
            // doc 11 §5 P6.3 the line carries its missing half — the
            // refusal stated *with* the answers ADR-0023 §5 says exist in
            // advance, at the exact moment the refusal is felt. ("Plays the
            // Library", not "the wall": room vocabulary stays internal,
            // P4's rule, applied to P6's own sentence.)
            text("When a queue ends, baz stops. Shuffle draws again; Play all plays the Library.")
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
    clippy::too_many_lines,
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
        QueueRowState::Playing | QueueRowState::Upcoming => room.paper,
    };
    let marker: Element<'static, Message> = if playing {
        lamp_dot()
    } else {
        text(row_state.position.to_string())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into()
    };
    // The playing row's title gains the medium weight the now-playing bar gives
    // the same string; everything else keeps the list's regular face, so the
    // emphasis moves down the queue with the music.
    let heading = text(row_state.title)
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .color(ink)
        .wrapping(text::Wrapping::None);
    let heading = if playing {
        heading.font(theme::MEDIUM)
    } else {
        heading
    };
    let mut title = column![heading].spacing(theme::GAP_XXS);
    if let Some(artist) = row_state.artist {
        title = title.push(
            text(artist)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None),
        );
    }
    let body = button(
        row![
            // Centred on the title's own line rather than on the row's block,
            // and the row top-aligned to keep it there — the same fix, the same
            // lane and the same argument as `album::track_row`, because these
            // are the same twelve rows.
            container(marker)
                .width(Length::Fixed(theme::TRACK_NO_W))
                .height(Length::Fixed(theme::CAPTION_LINE_H))
                .align_x(alignment::Horizontal::Right)
                .align_y(alignment::Vertical::Center),
            container(title).width(Length::Fill),
            container(
                text(row_state.duration)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None)
            )
            .width(Length::Fixed(theme::DURATION_W))
            .height(Length::Fixed(theme::CAPTION_LINE_H))
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Start),
    )
    .width(Length::Fill)
    // One indent lane for rows and one heading lane above them — no third edge
    // introduced by a row's own padding (law L5).
    .padding(theme::pad(theme::GAP_XS, 0.0))
    .style(move |_theme, status| theme::track_row(room, status, playing))
    .on_press_maybe(live.then_some(Message::JumpToQueued(index)));
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
        step_slot(
            icon::Glyph::ArrowUp,
            "Move up",
            index > 0,
            offered,
            Message::ShiftQueued(index, -1)
        ),
        step_slot(
            icon::Glyph::ArrowDown,
            "Move down",
            index + 1 < total,
            offered,
            Message::ShiftQueued(index, 1),
        ),
        remove_slot(index, offered),
    ]
    .spacing(theme::GAP_XS)
    .align_y(iced::Alignment::Center);
    if collecting.available {
        // The transfer slot needs no engine — a pick can land in a file —
        // so it is offered on hover alone, and at rest while the panel
        // stands (the album page's own rule).
        slots = slots.push(transfer_slot(index, collecting.panel_open || hovered));
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

/// One reorder stepper's reserved slot: the drawn ↑ or ↓ while the pointer
/// is on the row, and a space of exactly the same width when it is not —
/// the playlist page's `step_slot`, spending the queue's own message
/// ([`Message::ShiftQueued`]) so the two editors stay one anatomy
/// (doc 09 §8.2).
///
/// **Drawn glyphs, not font characters** (doc 10 §3.6): the slot row used
/// to hold the drawn ✕ beside U+2191/U+2193 borrowed from the face at a
/// visibly different stroke weight — the accidental fourth vocabulary the
/// study names — and a control slot now carries a drawn glyph or a word,
/// never a borrowed character. Icon-only, so the tooltip carries its name
/// (§3.1's rule; ADR-0017 §4c), and it draws at the hovered weight for the
/// ✕'s own reason: a control that exists only under the pointer has one
/// reading.
fn step_slot(
    glyph: icon::Glyph,
    name: &'static str,
    can: bool,
    offered: bool,
    message: Message,
) -> Element<'static, Message> {
    let room = theme::active();
    if !offered {
        return Space::with_width(Length::Fixed(theme::STEPPER_HIT)).into();
    }
    let mark = container(
        iced_image(icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(if can {
                theme::GLYPH_OPACITY_HOVER
            } else {
                theme::GLYPH_OPACITY_DISABLED
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    tooltip(
        button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.wall, status))
            .on_press_maybe(can.then_some(message)),
        text(name)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Left,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// The row's transfer slot: the album page's `+` anatomy — the drawn
/// [`icon::Glyph::Plus`] since doc 10 §3.6 — holding this row's track and
/// opening the picker (doc 09 §8.1 — pick a destination, the Queue first
/// among them).
fn transfer_slot(index: usize, offered: bool) -> Element<'static, Message> {
    let room = theme::active();
    if !offered {
        return Space::with_width(Length::Fixed(theme::STEPPER_HIT)).into();
    }
    let mark = container(
        iced_image(icon::handle(icon::Glyph::Plus))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::GLYPH_OPACITY_HOVER),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    tooltip(
        button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.wall, status))
            .on_press(Message::AddQueuedToPlaylist(index)),
        text("Add to a playlist, or the queue")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Left,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// The per-row removal target: a ✕ while the pointer is on the row, and an
/// empty space of exactly the same width when it is not.
///
/// [`theme::STEPPER_HIT`] rather than [`theme::TRANSPORT_HIT`], the same square
/// the settings' `−`/`+` pair uses: a destructive control inside a list row
/// should be reachable without being the largest thing in the row, and 24 px is
/// the size this room already gives a secondary target.
///
/// Inert when there is no engine to send the edit to — the same rule the row it
/// sits in follows: a control that cannot act must not pretend it can.
fn remove_slot(index: usize, offered: bool) -> Element<'static, Message> {
    let room = theme::active();
    if !offered {
        return Space::with_width(Length::Fixed(theme::STEPPER_HIT)).into();
    }
    let mark = container(
        iced_image(icon::handle(icon::Glyph::Close))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            // Drawn at the hovered weight, like every glyph in the slot row:
            // a control that exists only while the pointer is on its row has
            // one reading, so its resting weight and its hovered weight are
            // the same weight.
            .opacity(theme::GLYPH_OPACITY_HOVER),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    tooltip(
        button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.wall, status))
            .on_press(Message::RemoveQueued(index)),
        text("Remove from the queue")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Left,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// The playing row's lamp dot — the same amber circle the shelf puts beside
/// the playing album, and the same token behind it.
fn lamp_dot() -> Element<'static, Message> {
    let room = theme::active();
    container(Space::new(
        Length::Fixed(theme::DOT),
        Length::Fixed(theme::DOT),
    ))
    .style(move |_theme| theme::lamp_dot(room))
    .into()
}

#[cfg(test)]
mod tests {
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

        // The window: the rows loop spends `queue_window`'s slice, both
        // spacers are built, and every drawn element is boxed at the pitch
        // the module declared — which is what keeps the spacers honest.
        assert!(
            source.contains("queue_window::window(&shapes, scroll - rows_top, window.height)"),
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
        assert!(
            source.contains("Space::with_width(Length::Fixed(theme::STEPPER_HIT))"),
            "an unoffered slot is a space of exactly the control's width"
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
}
