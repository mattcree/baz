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

use iced::widget::{
    Column, Space, button, column, container, image as iced_image, mouse_area, row, scrollable,
    text, tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::player::{PlayerState, QueueRow, QueueRowState};
use crate::views::{place_header, place_pad};
use crate::{icon, theme};

/// The **Queue** place: the header strip, the summary, and the rows.
///
/// `window_width` decides one thing — how wide the list is set. It grows with
/// the window until [`theme::LIST_MEASURE`] and then stops, centring in what is
/// left, for the reason the record's page does the same: a row whose title is
/// at one end of 1800 px and whose duration is at the other is two words, not a
/// row.
///
/// Every string here is *owned*, straight from [`PlayerState::queue_list`]'s
/// render-ready reading, which is why the element is `'static`: the contents
/// are a projection of engine events and a request-side record, not a borrow of
/// the library, so nothing on screen can outlive a view-model rebuild mid-scan.
pub(crate) fn view(
    player: &PlayerState,
    window_width: f32,
    hovered: Option<usize>,
) -> Element<'static, Message> {
    let room = theme::active();
    let measure =
        (window_width - 2.0 * theme::HANG - theme::SCROLLBAR_LANE).clamp(0.0, theme::LIST_MEASURE);
    // A row is only a control when there is an engine to send its command to.
    let live = player.engine_ready();
    let body: Element<'static, Message> = match player.queue_list() {
        None => empty_state(),
        Some(list) => {
            // A record's name where the record begins, then its tracks —
            // **albums listed as albums, never flattened** (ADR-0014).
            let mut rows: Vec<Element<'static, Message>> = Vec::new();
            for (index, row_state) in list.rows.into_iter().enumerate() {
                if let Some(head) = row_state.head.clone() {
                    rows.push(album_group(
                        head.album.as_deref(),
                        &head.artist,
                        // One `GAP_MD` of air before a new record, taken above
                        // the name rather than below it, so the break belongs
                        // to the record it opens.
                        theme::GAP_MD,
                    ));
                }
                rows.push(queue_row(row_state, index, live, hovered == Some(index)));
            }
            column![
                text(list.summary)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None),
                column![
                    album_group(list.album.as_deref(), &list.artist, 0.0),
                    Column::with_children(rows).spacing(theme::GAP_XS),
                ]
                .spacing(theme::GAP_XS),
            ]
            .spacing(theme::GAP_LG)
            .into()
        }
    };
    column![
        place_header("Queue", "Esc returns to the wall"),
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
        .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .into()
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
            // which every other player would have started something.
            text("When a queue ends, baz stops.")
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
/// artist where there is one, right-aligned duration — and two things a
/// listener can do to it.
///
/// **Clicking the row plays from there.** ADR-0014's `JumpTo`, and this list is
/// the one place it needs no decision at all: the rows are drawn from the
/// record of what was handed to the engine, so a row's index *is* a queue
/// position. Nothing is re-queued, and the mark follows `TrackStarted` rather
/// than the click.
///
/// **The ✕ takes the entry out**, through `UpdateQueue` and the pure
/// [`queue_edit`](crate::queue_edit) helper — an edit that does not touch the
/// playing track does not disturb one delivered sample, which is the guarantee
/// ADR-0014 exists to make and the reason this is not a `SetQueue`.
///
/// Two fixed-slot rules, because a list that changes under the reader is
/// exactly where movement is least affordable:
///
/// - the number column is [`theme::TRACK_NO_W`] wide whichever it holds, so the
///   dot arriving as a track starts moves no text;
/// - **the ✕'s slot is reserved whether or not the ✕ is in it.** The control
///   appears on hover — a column of permanent crosses down a list of what you
///   are about to hear is a column of invitations to destroy something — but if
///   its width came and went with it, every duration would slide sideways as
///   the pointer crossed a row.
fn queue_row(
    row_state: QueueRow,
    index: usize,
    live: bool,
    hovered: bool,
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
    mouse_area(
        row![body, remove_slot(index, live && hovered)]
            .spacing(theme::GAP_XS)
            .align_y(iced::Alignment::Center),
    )
    .on_enter(Message::QueueRowEntered(index))
    .on_exit(Message::QueueRowLeft(index))
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
            // The one glyph in baz drawn at its hovered weight: this control
            // exists only while the pointer is on its row, so its resting
            // reading and its hovered reading are the same reading.
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
