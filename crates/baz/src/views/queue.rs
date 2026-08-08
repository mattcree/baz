//! **Queue**: the queue the engine holds, floating over the place, anchored
//! to the bar it describes.
//!
//! # What this surface is for
//!
//! Answering one question — *what is playing next* — and answering it where the
//! question is asked. A listener wondering what is coming is looking at the
//! bottom bar; the queue's only door used to be a toggle in the *top* bar, two
//! hundred pixels away from the thing it described, and opening it took two
//! columns of covers off the shelf to say so.
//!
//! # What changed, and what did not
//!
//! The rows are the rail's queue-panel rows, unchanged: the position (or the
//! lamp dot) in a fixed [`theme::TRACK_NO_W`] column, the title over its
//! artist, the duration right-aligned, the played rows falling to the faint ink
//! while the upcoming ones keep full paper. A listener who learned that list in
//! the rail has learned this one.
//!
//! What changed is the *container* and what the container costs. This is an
//! overlay: it reflows nothing, it is dismissed by a press anywhere outside it,
//! and it is gone in a second. The rail's ✕ has moved onto the header line
//! beside the title rather than owning a row of its own, which is 44 px of
//! vertical budget returned to the rows.
//!
//! # Not modal, and it says so
//!
//! iced 0.13 offers no focus containment and no accessibility tree, so this
//! cannot be a modal dialog and does not pretend to be one (§4.6 of the design
//! spec). <kbd>Esc</kbd> closes it; every other binding keeps working
//! underneath; the shelf still scrolls; the transport in the bar below is
//! never covered and stays live. There is no scrim — dimming ten thousand
//! covers to show twelve rows would contradict the palette rationale (§2.4).
//!
//! # One list with a cursor
//!
//! The model is `MusicBee`'s, adopted deliberately rather than arrived at:
//! **history behind the cursor, queue ahead, one surface**
//! (`docs/design/03-interface-prior-art.md` §5.3(3), R5). It is the model a
//! large share of baz's own audience already knows, it is simpler than the
//! two-structure arrangements the streaming products ended up with, and the
//! rows already expressed it — played rows fall to the faint ink, the playing
//! row is carded and dotted, upcoming rows keep full paper.
//!
//! What follows from naming it is the summary line: it reads **what is left**
//! (`3 of 12 · 38:12 left`), not what the list contains. A queue is a thing you
//! are partway through, and the total running time answers a question nobody
//! opened this to ask.
//!
//! # Marking
//!
//! The playing row is marked the way the shelf marks the playing album and the
//! way the inspector marks the playing track — the amber lamp dot, in place of
//! the row's number — because that is the one thing on this surface that *is*
//! playback truth, and the palette reserves the accent for exactly that.

use iced::widget::{
    Column, Space, button, column, container, image as iced_image, mouse_area, row, scrollable,
    text, tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::player::{PlayerState, QueueRow, QueueRowState};
use crate::views::close_button;
use crate::{icon, theme};

/// Inner padding of the popover (logical px).
///
/// [`theme::GAP_LG`], one rung below the rail panels' [`theme::GAP_XL`]: a
/// floating layer 360 px wide is a tighter room than a full-height column, and
/// the twenty-four-pixel inset that gave the rail its calm would here be spent
/// on air the rows need.
const POPOVER_PAD: f32 = theme::GAP_LG;

/// The **Queue** popover: a header carrying the title and the ✕, the summary
/// line, and the rows.
///
/// `max_height` is the ceiling the shell computes from the window
/// ([`theme::POPOVER_MAX_H`]); the popover is otherwise exactly as tall as its
/// contents, so a two-track queue is a small card rather than a mostly-empty
/// column.
///
/// Every string here is *owned*, straight from
/// [`PlayerState::queue_list`]'s render-ready reading, which is why the
/// element is `'static`: the popover's contents are a projection of engine
/// events and a request-side record, not a borrow of the library, so nothing
/// on screen can outlive a view-model rebuild mid-scan.
pub(crate) fn view(
    player: &PlayerState,
    max_height: f32,
    hovered: Option<usize>,
) -> Element<'static, Message> {
    // A row is only a control when there is an engine to send its command to,
    // exactly as `Play album` and the inspector's rows are.
    let live = player.engine_ready();
    let content = match player.queue_list() {
        None => empty_state(),
        Some(list) => {
            let rows: Vec<Element<'static, Message>> = list
                .rows
                .into_iter()
                .enumerate()
                .map(|(index, row_state)| queue_row(row_state, index, live, hovered == Some(index)))
                .collect();
            // Where the queue came from, in one line: the album if it has a
            // title, else the artist it is filed under. The design spec's
            // contents list names only the summary, and this is the one
            // addition to it — kept because the bar below shows the *track*
            // and nothing on screen otherwise says which record these twelve
            // rows are, which is the first thing a listener checks a queue
            // against.
            let source = list.album.unwrap_or(list.artist);
            column![
                column![
                    text(source)
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(theme::PAPER_DIM)
                        .wrapping(text::Wrapping::None),
                    text(list.summary)
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(theme::PAPER_FAINT),
                ]
                .spacing(theme::GAP_XXS),
                // The same reserved scrollbar lane the album inspector's track
                // list keeps, and for the same reason: this list has a duration
                // column against the same right edge, so a queue long enough to
                // scroll would clip it in exactly the same way.
                // (`side_panel::track_list` carries the argument.)
                scrollable(
                    Column::with_children(rows)
                        .spacing(theme::GAP_XXS)
                        .padding(theme::scroll_gutter())
                )
                .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
                .style(theme::scrollbar),
            ]
            .spacing(theme::GAP_SM)
            .into()
        }
    };
    let body = column![header_row(), content]
        .spacing(theme::GAP_MD)
        .width(Length::Fill);
    container(body)
        .width(Length::Fixed(theme::POPOVER_W))
        .max_height(max_height)
        .padding(POPOVER_PAD)
        .style(theme::popover)
        .into()
}

/// The popover's header: its name, and the ✕ on the same line.
///
/// **On the same line**, where the rail gave the ✕ a row of its own. That row
/// cost [`theme::TRANSPORT_HIT`] plus a column gap — 44 px of vertical budget
/// in the app's most contested column — for a control Escape already provided,
/// and in a floating card 360 px wide those pixels are rows.
fn header_row() -> Element<'static, Message> {
    row![
        text("Queue")
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .font(theme::MEDIUM),
        Space::with_width(Length::Fill),
        close_button("Close queue", Message::CloseQueue),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

/// Nothing queued yet: said plainly, with the gesture that fills it.
///
/// Quiet text rather than an illustration or a call to action — an empty queue
/// is the ordinary state of a player nobody has pressed play on, not a problem
/// to solve.
fn empty_state() -> Element<'static, Message> {
    container(
        column![
            text("Nothing queued")
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .color(theme::PAPER_DIM),
            text("Play an album and it appears here")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(theme::PAPER_FAINT),
        ]
        .spacing(theme::GAP_SM)
        .align_x(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding(theme::pad(theme::GAP_XL, 0.0))
    .align_x(alignment::Horizontal::Center)
    .into()
}

/// One queue row: position (or the lamp dot when it is playing), title over
/// its artist where there is one, right-aligned duration — and, now, two things a
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
/// that ADR-0014 exists to make and the reason this is not a `SetQueue`.
///
/// Two fixed-slot rules, because a list that changes under the reader is
/// exactly where movement is least affordable:
///
/// - the number column is [`theme::TRACK_NO_W`] wide whichever it holds, so the
///   dot arriving as a track starts moves no text;
/// - **the ✕'s slot is reserved whether or not the ✕ is in it.** The control
///   appears on hover — twelve permanent crosses would be twelve invitations to
///   destroy something in a surface built for a glance — but if its width came
///   and went with it, every duration in the list would slide sideways as the
///   pointer crossed a row.
fn queue_row(
    row_state: QueueRow,
    index: usize,
    live: bool,
    hovered: bool,
) -> Element<'static, Message> {
    let playing = row_state.state == QueueRowState::Playing;
    let ink = match row_state.state {
        QueueRowState::Played => theme::PAPER_FAINT,
        QueueRowState::Playing | QueueRowState::Upcoming => theme::PAPER,
    };
    let marker: Element<'static, Message> = if playing {
        lamp_dot()
    } else {
        text(row_state.position.to_string())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(theme::PAPER_FAINT)
            .into()
    };
    // The playing row's title gains the medium weight the now-playing bar
    // gives the same string; everything else keeps the list's regular face, so
    // the emphasis moves down the queue with the music.
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
                .color(theme::PAPER_DIM)
                .wrapping(text::Wrapping::None),
        );
    }
    let body = button(
        row![
            container(marker)
                .width(Length::Fixed(theme::TRACK_NO_W))
                .align_x(alignment::Horizontal::Right),
            container(title).width(Length::Fill),
            text(row_state.duration)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(theme::PAPER_FAINT),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding(theme::pad(theme::GAP_XS, theme::GAP_XS))
    .style(move |_theme, status| theme::track_row(status, playing))
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
/// sits in follows, and the same rule `Play album` follows: a control that
/// cannot act must not pretend it can.
fn remove_slot(index: usize, offered: bool) -> Element<'static, Message> {
    if !offered {
        return Space::with_width(Length::Fixed(theme::STEPPER_HIT)).into();
    }
    let mark = container(
        iced_image(icon::handle(icon::Glyph::Close))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::GLYPH_OPACITY),
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
            .style(theme::transport)
            .on_press(Message::RemoveQueued(index)),
        text("Remove from the queue")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Left,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(theme::tooltip)
    .into()
}

/// The playing row's lamp dot — the same amber circle the shelf puts beside
/// the playing album, and the same token behind it.
fn lamp_dot() -> Element<'static, Message> {
    container(Space::new(
        Length::Fixed(theme::DOT),
        Length::Fixed(theme::DOT),
    ))
    .style(theme::lamp_dot)
    .into()
}
