//! **A playlist's page**: one list a person made, at the width of the window
//! (ADR-0024 §4) — the record page's sibling in arrangement, the queue
//! place's sibling in row anatomy.
//!
//! # The record page's composition, the queue place's rows
//!
//! Since the sleeve amendment (ADR-0024 §A2) the page wears the album page's
//! own two-column arrangement: **the object beside what is written about
//! it** — the collage sleeve at [`theme::ALBUM_SLEEVE`] over `Play` and the
//! quieter acts in the aside, the name at hero scale over the rows in the
//! main column, stacking below [`theme::ALBUM_BREAKPOINT`] by the same
//! arithmetic. The rows themselves stay the queue place's — one anatomy for
//! every list in baz — plus the two reserved edit slots a durable artefact
//! earns: the ✕ that takes an entry out and the ▲▼ steppers that reorder,
//! the no-drag pointer route the visible-control rule requires, with
//! drag-to-reorder deferred to the shared pointer-capture widget
//! (ADR-0024 §6 layer 3).
//!
//! The declared hierarchy (law L6) is the album page's, re-read for a made
//! thing: **the work ≫ `Play` → the name → the rows** — the sleeve is a
//! collage of quotations and it is the only image of the playlist on
//! screen, so it is first by declaration exactly as the record's is.
//!
//! # The honesty on this surface
//!
//! - **A missing entry stays.** It renders dimmed from its path's stem, with
//!   the path itself on the row, unplayable — never silently pruned
//!   (ADR-0024 §3), and the summary says the arithmetic out loud:
//!   `38 of 40 · 2 missing`.
//! - **A row click is the same rule every list surface spends**:
//!   [`PlayerState::play_from`] over the playable subset — a jump when the
//!   engine already holds exactly this list, `SetQueue` + `JumpTo` when it
//!   does not.
//! - **The lamp dot marks a row only when the queue is exactly this list**
//!   ([`PlayerState::playing_row_in`]) — a page listing something other than
//!   what the engine holds marks nothing.
//! - **Delete is one press, into the platform trash** (doc 11 §5 P2): the
//!   confirm dialog and its sentence — *"The file goes; your music stays"* —
//!   retired with honour, because the trash keeps the promise the sentence
//!   made. Forgiveness beats warning: reversibility first, per the 1992
//!   HIG's own ranking, and the desktop's Restore is the road back.
//! - **Undo stands beside the counts while there is an edit to take back**
//!   (P2 again): remove, reorder and append are whole-file rewrites, and
//!   the file as it stood is one press — or <kbd>Ctrl</kbd>+<kbd>Z</kbd> —
//!   away, through the same fingerprint guard as the edit it reverses.

use iced::widget::{
    Column, Space, button, column, container, image as iced_image, mouse_area, row, scrollable,
    text, text_input, tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::playlists::{Collecting, NameEntry, OpenPlaylist, PageRow};
use crate::views::{place_header, place_pad, playlist_sleeve, section_rule};
use crate::{icon, theme};

/// The rename field's id, so the caret can land in it the moment `Rename` is
/// pressed.
pub(crate) fn rename_id() -> text_input::Id {
    text_input::Id::new("baz-playlist-rename")
}

/// The playlist's page: the header strip, then **the object beside what is
/// written about it** — the record page's own two-column arrangement, worn
/// by its sibling (ADR-0024 §A2). The aside holds the sleeve, the page's one
/// commitment and its quieter acts; the main column holds the name at hero
/// scale, the counts and the rows. Below [`theme::ALBUM_BREAKPOINT`] the two
/// columns stack, by the album page's own arithmetic and for its reason.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is one independent reading the page renders — \
              the drag in flight and the undo affordance arrived from two \
              different studies, and bundling them into a struct would name \
              nothing the call site does not already say"
)]
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    open: &'a OpenPlaylist,
    player: &'a PlayerState,
    window_width: f32,
    hovered: Option<usize>,
    collecting: Collecting,
    drag: Option<&'a crate::drag::DragState>,
    can_undo: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let content = (window_width - 2.0 * theme::HANG - theme::SCROLLBAR_LANE).max(0.0);
    let side_by_side = window_width >= theme::ALBUM_BREAKPOINT;
    let measure = if side_by_side {
        (content - theme::ALBUM_ASIDE_W - theme::GAP_XL).clamp(0.0, theme::LIST_MEASURE)
    } else {
        content.min(theme::LIST_MEASURE)
    };
    let live = player.engine_ready();
    // Which display row carries the lamp: the engine's confirmed row in the
    // playable subset, mapped back through each row's own subset position —
    // and nothing at all unless the queue is exactly this list.
    let playing_playable = player.playing_row_in(&open.tracks);

    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    for (index, page_row) in open.rows.iter().enumerate() {
        if let Some((album, artist)) = &page_row.head {
            rows.push(record_head(album, artist, index == 0));
        }
        let playing =
            page_row.playable_position.is_some() && page_row.playable_position == playing_playable;
        rows.push(entry_row(
            page_row,
            index,
            open.rows.len(),
            live,
            playing,
            hovered == Some(index),
            collecting,
            drag.and_then(|held| held.line_for_row(crate::drag::List::Playlist, index)),
            drag.is_some_and(|held| held.list == crate::drag::List::Playlist),
        ));
    }
    let body: Element<'a, Message> = if open.rows.is_empty() {
        // The words the armed mode left behind went with it (doc 09 §9):
        // the route in is the transfer gesture — a row's `+`, or the
        // record page's `Add to playlist…`, then this list in the picker.
        text("Nothing here yet. Press + on any track row, or Add to playlist… on a record's page, and pick this list.")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into()
    } else {
        Column::with_children(rows).spacing(theme::GAP_XS).into()
    };
    let main = column![
        identity_block(open, can_undo),
        column![section_rule("Tracks"), body].spacing(theme::GAP_SM),
    ]
    .spacing(theme::GAP_XL);
    let page: Element<'a, Message> = if side_by_side {
        row![
            container(aside(shelf, open, live)).width(Length::Fixed(theme::ALBUM_ASIDE_W)),
            container(main).width(Length::Fixed(measure)),
        ]
        .spacing(theme::GAP_XL)
        .align_y(iced::Alignment::Start)
        .into()
    } else {
        column![
            container(aside(shelf, open, live)).width(Length::Fixed(theme::ALBUM_ASIDE_W)),
            container(main).width(Length::Fixed(measure)),
        ]
        .spacing(theme::GAP_XL)
        .into()
    };
    column![
        place_header("Playlist", "Esc returns to Library"),
        scrollable(
            container(page)
                .width(Length::Fill)
                .padding(place_pad())
                .align_x(alignment::Horizontal::Center)
        )
        .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .into()
}

/// The left column: **the object, the one thing you can do to it, and its
/// quieter acts** — the album page's aside, worn by the playlist. The sleeve
/// is the collage of quotations at [`theme::ART_MAX`] (§A1), `Play` stands
/// directly under it at the sleeve's whole width exactly as `Play album`
/// does, and the acts that redefine or destroy the artefact sit below with
/// whichever of their fields is standing.
fn aside<'a>(shelf: &'a Shelf, open: &'a OpenPlaylist, live: bool) -> Element<'a, Message> {
    let playable = !open.queue.is_empty();
    let mut block = column![
        playlist_sleeve(shelf, &open.art, open.name(), theme::ALBUM_SLEEVE),
        play_control(live && playable),
        row![
            word_act("Queue", live && playable, Message::PlaylistQueue),
            word_act("Rename", true, Message::PlaylistRenameStart),
            // One press, into the platform trash (doc 11 §5 P2): the
            // confirm died when the act became reversible — the desktop's
            // own Restore is the road back, so a warning would be the
            // fallback posture shipped as the default.
            word_act("Delete", true, Message::PlaylistDelete),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(theme::GAP_MD);
    if let Some(renaming) = &open.renaming {
        block = block.push(rename_field(renaming));
    }
    block.into()
}

/// The main column's identity block: **the name at hero scale over the
/// counts** — `38 of 40 · 2 missing` when entries are missing — the album
/// header's falling order with the fields a made thing has. Beside the
/// counts, exactly while there is an edit to take back, stands the
/// transient `Undo` (doc 11 §5 P2) — the queue place's word, on the page
/// that is its sibling editor.
fn identity_block(open: &OpenPlaylist, can_undo: bool) -> Element<'_, Message> {
    let room = theme::active();
    let mut summary = row![
        text(open.counts_line())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center);
    if can_undo {
        summary = summary.push(undo_control());
    }
    column![
        container(
            text(open.name().to_owned())
                .size(theme::SIZE_HERO)
                .line_height(theme::LEADING_HERO)
                .font(theme::SEMIBOLD)
                .color(room.paper)
        )
        .max_height(2.0 * theme::LINE_HERO)
        .clip(true),
        summary,
    ]
    .spacing(theme::GAP_XS)
    .into()
}

/// **Undo** — the file as it stood before the last recorded edit, one press
/// away (doc 11 §5 P2): the queue place's word and rule, worn by the page.
/// Drawn only while the history holds something; `Ctrl+Z` is the
/// accelerator this word makes legal.
fn undo_control() -> Element<'static, Message> {
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

/// **Play** — the page's one commitment, in `Play album`'s clothes: the lamp
/// outline, the paper triangle, and the only accent on the surface. It sends
/// the playable subset (ADR-0024 §3); the counts line directly above it is
/// where the page says so.
fn play_control(live: bool) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            row![
                iced_image(icon::handle(icon::Glyph::Play))
                    .width(Length::Fixed(theme::ICON_PX))
                    .height(Length::Fixed(theme::ICON_PX))
                    .opacity(theme::glyph_opacity(live, false)),
                text("Play")
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .font(theme::SEMIBOLD)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::primary(room, status))
    .on_press_maybe(live.then_some(Message::PlaylistPlay))
    .into()
}

/// A quiet word act — `Queue`, `Rename`, `Delete` — at the product's one
/// control height, no accent.
fn word_act(label: &'static str, enabled: bool, message: Message) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .color(if enabled {
                    room.paper
                } else {
                    room.paper_muted
                })
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::transport(room, room.wall, status))
    .on_press_maybe(enabled.then_some(message))
    .into()
}

/// The rename field, with the storage layer's refusal under it when the last
/// submission was refused — the same anatomy as the panel's name field.
fn rename_field(entry: &NameEntry) -> Element<'_, Message> {
    let room = theme::active();
    let mut block = column![
        text_input("New name…", &entry.text)
            .id(rename_id())
            .on_input(Message::PlaylistRenameInput)
            .on_submit(Message::PlaylistRenameSubmit)
            .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .width(Length::Fill)
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

/// A record's name where its run begins — the queue place's group-header
/// rule, drawn over consecutive same-record runs so the playlist stays a
/// track list that still says where things came from.
fn record_head(album: &str, artist: &str, first: bool) -> Element<'static, Message> {
    let room = theme::active();
    let air = if first { 0.0 } else { theme::GAP_MD };
    let mut block = column![
        text(album.to_owned())
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .font(theme::MEDIUM)
            .color(room.paper_dim)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XXS);
    if !artist.is_empty() {
        block = block.push(
            text(artist.to_owned())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.heading())
                .wrapping(text::Wrapping::None),
        );
    }
    container(block).padding(theme::pad(air, 0.0)).into()
}

/// One entry's row: position (or the lamp dot), title over its artist — or
/// over its path, when the entry is missing — duration, and the three
/// reserved edit slots.
///
/// The queue row's fixed-slot rules, extended by two: the number column is
/// [`theme::TRACK_NO_W`] whichever it holds, and the ▲▼ and ✕ slots are
/// reserved whether or not their controls are in them, so durations never
/// slide as the pointer crosses a row. The controls appear on hover — a
/// column of permanent crosses down a list you are reading is a column of
/// invitations to destroy something — and hover is not their only route in
/// the product's terms: the file itself is the user's, editable in any text
/// editor, and the page re-reads it (ADR-0024 §2).
///
/// Since doc 09 §13 step 4 the row also carries the **transfer `+`** in the
/// queue row's outer slot — the last piece of §8.2's "same editor" claim,
/// and the visible twin the row's context-menu items mirror (§5.2). A
/// missing entry gets no `+` for the ✕'s opposite reason: there is nothing
/// there to transfer.
/// **The row's body is a drag source** (doc 09 §13 step 8; [`crate::drag`]):
/// press and travel lifts the row for a reorder — one saved file on the
/// drop, [`crate::playlists::Playlists::move_entry`] — or a carry to the
/// standing panel's rows to add. A missing entry drags too (its position
/// is real) but carries no payload, so a panel drop moves nothing — the
/// `+`'s own refusal, held by the drag. Sugar only: the ▲▼, ✕ and `+`
/// remain, and the sub-threshold press is the row's ordinary click.
#[expect(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    reason = "a row is one anatomy — marker, title, duration, four reserved \
              slots — and splitting it would put half the reservation rules \
              out of sight of the other half"
)]
fn entry_row(
    page_row: &PageRow,
    index: usize,
    total: usize,
    live: bool,
    playing: bool,
    hovered: bool,
    collecting: Collecting,
    insert_line: Option<crate::drag::Edge>,
    observing: bool,
) -> Element<'_, Message> {
    let room = theme::active();
    let ink = if page_row.missing {
        room.paper_faint
    } else {
        room.paper
    };
    let marker: Element<'_, Message> = if playing {
        lamp_dot()
    } else {
        text(page_row.position.to_string())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into()
    };
    let heading = text(page_row.title.clone())
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
    if page_row.missing {
        // The path, one glance away (ADR-0024 §3): a missing entry's row is
        // drawn from its stem, and this line is where it went.
        title = title.push(
            text(page_row.path.display().to_string())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_muted)
                .wrapping(text::Wrapping::None),
        );
    } else if let Some(artist) = &page_row.artist {
        title = title.push(
            text(artist.clone())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None),
        );
    }
    let body = button(
        row![
            container(marker)
                .width(Length::Fixed(theme::TRACK_NO_W))
                .height(Length::Fixed(theme::CAPTION_LINE_H))
                .align_x(alignment::Horizontal::Right)
                .align_y(alignment::Vertical::Center),
            container(title).width(Length::Fill),
            container(
                text(page_row.duration.clone())
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
    .padding(theme::pad(theme::GAP_XS, 0.0))
    .style(move |_theme, status| theme::track_row(room, room.wall, status, playing))
    // A missing entry is not a control: pressing a row plays from it, and
    // there is nothing there to play.
    .on_press_maybe((live && !page_row.missing).then_some(Message::PlaylistPlayTrack(index)));
    // The drag wrapper owns the pointer for the body (crate::drag): every
    // row of the artefact is draggable — a file edit needs no engine, the
    // steppers' own rule — and the sub-threshold click keeps the button's
    // exact gate.
    let mut source = crate::drag::Source::new(body, room).wires(crate::drag::Wires::new(
        move |at| Message::DragLift(crate::drag::List::Playlist, index, at),
        Message::DragMoved,
        Message::DragDropped,
        (live && !page_row.missing).then_some(Message::PlaylistPlayTrack(index)),
    ));
    if observing {
        source = source.observe(move |before| {
            Message::DragOverRow(crate::drag::List::Playlist, index, before)
        });
    }
    let body: Element<'_, Message> = source.line(insert_line).into();
    let offered = hovered;
    let mut slots = row![
        body,
        step_slot(
            icon::Glyph::ArrowUp,
            "Move up",
            index > 0,
            offered,
            Message::PlaylistShiftEntry(index, -1)
        ),
        step_slot(
            icon::Glyph::ArrowDown,
            "Move down",
            index + 1 < total,
            offered,
            Message::PlaylistShiftEntry(index, 1),
        ),
        remove_slot(index, offered),
    ]
    .spacing(theme::GAP_XS)
    .align_y(iced::Alignment::Center);
    if collecting.available {
        // The transfer slot, in the queue row's outer position and by its
        // rule: no engine needed (a pick can land in a file), offered on
        // hover and at rest while the panel stands. A missing entry keeps
        // the reserved space and no control.
        slots = slots.push(transfer_slot(
            index,
            !page_row.missing && (collecting.panel_open || hovered),
        ));
    }
    // The row's right press opens its mirror menu (doc 09 §5.2): play and
    // the transfer verbs, each a press this row's own controls already
    // make. A missing entry's menu offers nothing, exactly as its row does.
    crate::menu::area(
        mouse_area(slots)
            .on_enter(Message::PlaylistRowEntered(index))
            .on_exit(Message::PlaylistRowLeft(index)),
        crate::menu::Target::PlaylistTrack {
            row: index,
            missing: page_row.missing,
        },
    )
}

/// The row's transfer `+` — the queue row's exact anatomy and tooltip, the
/// drawn [`icon::Glyph::Plus`] since doc 10 §3.6, sending the page's own
/// message ([`Message::PlaylistAddEntry`]): hold this row's track, open the
/// panel as the picker (doc 09 §8.1's one gesture, reaching the page's rows
/// at step 4 as §8.2's parity promised).
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
            .on_press(Message::PlaylistAddEntry(index)),
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

/// One reorder stepper's reserved slot: the drawn ↑ or ↓ while the pointer
/// is on the row, and a space of exactly the same width when it is not —
/// the settings steppers' size, the queue ✕'s reservation rule.
///
/// The arrows were U+2191/U+2193 borrowed from the face (IBM Plex Sans
/// carries no triangles, so the docs' ▲▼ shorthand rasterised as tofu);
/// they are [`icon::Glyph::ArrowUp`]/[`icon::Glyph::ArrowDown`] now
/// (doc 10 §3.6): a control slot carries a drawn glyph or a word, never a
/// borrowed character, and the drawn pair matches the ✕ beside it in
/// stroke and ink. Icon-only, so the tooltip carries the name.
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

/// The per-row removal target — the queue row's exact anatomy, sending the
/// file's edit rather than the engine's.
fn remove_slot(index: usize, offered: bool) -> Element<'static, Message> {
    let room = theme::active();
    if !offered {
        return Space::with_width(Length::Fixed(theme::STEPPER_HIT)).into();
    }
    let mark = container(
        iced_image(icon::handle(icon::Glyph::Close))
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
            .on_press(Message::PlaylistRemoveEntry(index)),
        text("Remove from the playlist")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Left,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// The playing row's lamp dot — the same amber circle, the same token, as
/// every other list surface's.
fn lamp_dot() -> Element<'static, Message> {
    let room = theme::active();
    container(Space::new(
        Length::Fixed(theme::DOT),
        Length::Fixed(theme::DOT),
    ))
    .style(move |_theme| theme::lamp_dot(room))
    .into()
}
