//! **A playlist's page**: one list a person made, at the width of the window
//! (ADR-0024 §4) — the sibling of the record's page, in the queue place's
//! anatomy.
//!
//! # Three list surfaces, one composition
//!
//! Header strip, summary, rows at [`theme::LIST_MEASURE`], one scroll — the
//! queue place's shape exactly, because a listener who has read one list in
//! baz must not have to learn another. What a *durable* artefact earns over
//! the transient queue is the header block (the name at hero scale, the
//! page's acts) and the two reserved edit slots per row: the ✕ that takes an
//! entry out and the ▲▼ steppers that reorder — the no-drag pointer route the
//! visible-control rule requires, at the settings steppers' own size, with
//! drag-to-reorder deferred to the shared pointer-capture widget
//! (ADR-0024 §6 layer 3).
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
//! - **Delete states what survives**: *"The file goes; your music stays"* —
//!   every destructive confirmation in baz names the survivor (ADR-0022's
//!   voice, adopted by ADR-0024).

use iced::widget::{
    Column, Space, button, column, container, image as iced_image, mouse_area, row, scrollable,
    text, text_input, tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::player::PlayerState;
use crate::playlists::{NameEntry, OpenPlaylist, PageRow};
use crate::views::{place_header, place_pad, section_rule};
use crate::{icon, theme};

/// The rename field's id, so the caret can land in it the moment `Rename` is
/// pressed.
pub(crate) fn rename_id() -> text_input::Id {
    text_input::Id::new("baz-playlist-rename")
}

/// The playlist's page: the header strip, the name and its acts, the rows.
pub(crate) fn view<'a>(
    open: &'a OpenPlaylist,
    player: &'a PlayerState,
    window_width: f32,
    hovered: Option<usize>,
) -> Element<'a, Message> {
    let room = theme::active();
    let measure =
        (window_width - 2.0 * theme::HANG - theme::SCROLLBAR_LANE).clamp(0.0, theme::LIST_MEASURE);
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
        ));
    }
    let body: Element<'a, Message> = if open.rows.is_empty() {
        text("Nothing here yet. Arm this playlist in the panel, or press + on a record's page.")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .into()
    } else {
        Column::with_children(rows).spacing(theme::GAP_XS).into()
    };
    let content = column![
        header_block(open, live),
        column![section_rule("Tracks"), body].spacing(theme::GAP_SM),
    ]
    .spacing(theme::GAP_XL);
    column![
        place_header("Playlist", "Esc returns to the wall"),
        scrollable(
            container(container(content).width(Length::Fixed(measure)))
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

/// The page's identity and its acts: the name at hero scale, the counts —
/// `38 of 40 · 2 missing` when entries are missing — then `Play`, `Queue`,
/// `Rename`, `Delete`, and whichever of the rename field or the delete
/// confirmation is standing.
fn header_block(open: &OpenPlaylist, live: bool) -> Element<'_, Message> {
    let room = theme::active();
    let mut block = column![
        container(
            text(open.name().to_owned())
                .size(theme::SIZE_HERO)
                .line_height(theme::LEADING_HERO)
                .font(theme::SEMIBOLD)
                .color(room.paper)
        )
        .max_height(2.0 * theme::LINE_HERO)
        .clip(true),
        text(open.counts_line())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint),
    ]
    .spacing(theme::GAP_XS);
    let playable = !open.queue.is_empty();
    block = block.push(
        row![
            play_control(live && playable),
            word_act("Queue", live && playable, Message::PlaylistQueue),
            word_act("Rename", true, Message::PlaylistRenameStart),
            word_act("Delete", true, Message::PlaylistDeleteArm),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    );
    if let Some(renaming) = &open.renaming {
        block = block.push(rename_field(renaming));
    }
    if open.delete_armed {
        block = block.push(delete_confirm(open.name()));
    }
    block.into()
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
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_XL))
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

/// The armed delete, in the roots ADR's voice: every destructive
/// confirmation in baz states what survives.
fn delete_confirm(name: &str) -> Element<'_, Message> {
    let room = theme::active();
    row![
        text(format!(
            "Delete \u{201c}{name}\u{201d}? The file goes; your music stays."
        ))
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.paper_dim),
        word_act("Delete", true, Message::PlaylistDeleteConfirm),
        word_act("Keep", true, Message::PlaylistDeleteCancel),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .into()
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
fn entry_row(
    page_row: &PageRow,
    index: usize,
    total: usize,
    live: bool,
    playing: bool,
    hovered: bool,
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
    .style(move |_theme, status| theme::track_row(room, status, playing))
    // A missing entry is not a control: pressing a row plays from it, and
    // there is nothing there to play.
    .on_press_maybe((live && !page_row.missing).then_some(Message::PlaylistPlayTrack(index)));
    let offered = hovered;
    mouse_area(
        row![
            body,
            step_slot(
                "\u{25b4}",
                index > 0,
                offered,
                Message::PlaylistShiftEntry(index, -1)
            ),
            step_slot(
                "\u{25be}",
                index + 1 < total,
                offered,
                Message::PlaylistShiftEntry(index, 1),
            ),
            remove_slot(index, offered),
        ]
        .spacing(theme::GAP_XS)
        .align_y(iced::Alignment::Center),
    )
    .on_enter(Message::PlaylistRowEntered(index))
    .on_exit(Message::PlaylistRowLeft(index))
    .into()
}

/// One reorder stepper's reserved slot: ▲ or ▼ while the pointer is on the
/// row, and a space of exactly the same width when it is not — the settings
/// steppers' size, the queue ✕'s reservation rule.
fn step_slot(
    glyph: &'static str,
    can: bool,
    offered: bool,
    message: Message,
) -> Element<'static, Message> {
    let room = theme::active();
    if !offered {
        return Space::with_width(Length::Fixed(theme::STEPPER_HIT)).into();
    }
    button(
        container(
            text(glyph)
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .color(if can { room.paper } else { room.paper_muted }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fixed(theme::STEPPER_HIT))
    .height(Length::Fixed(theme::STEPPER_HIT))
    .padding(0)
    .style(move |_theme, status| theme::transport(room, room.wall, status))
    .on_press_maybe(can.then_some(message))
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
