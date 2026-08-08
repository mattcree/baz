//! The play queue: what baz handed the engine, and where the engine is in it.
//!
//! # What this surface is for
//!
//! Answering one question — *what is playing next* — which baz could not
//! answer at all before it existed. Clicking an album queued its tracks and
//! played them, and the only evidence of the other eleven was that they
//! eventually arrived.
//!
//! # What it deliberately is not
//!
//! The centre of gravity. It shares the rail with the album panel rather than
//! adding a second one (the argument is in [`crate::panels`]), it is closed
//! until asked for, and the now-playing bar remains the transport: nothing
//! here plays, pauses, seeks or skips, and the elapsed time of the current
//! track appears once, in the bar, not twice.
//!
//! It is also not interactive, and that is a protocol fact rather than a
//! design preference. Clicking a row to jump to it, dragging a row to reorder,
//! and removing a track each need an engine command that does not exist;
//! [`PlayerState::queue_list`] names exactly which, so a follow-up unit is a
//! small change here rather than an investigation. Until then the rows carry
//! no hover affordance and no pointer cursor — the same rule the album panel's
//! track list already follows, for the same reason: an affordance that does
//! nothing is a lie.
//!
//! # Marking
//!
//! The playing row is marked the way the shelf marks the playing album — the
//! amber lamp dot, in place of the row's number — because that is the one
//! thing on this surface that *is* playback truth, and the palette reserves
//! the accent for exactly that. Rows behind it fall to the faint ink the room
//! gives spent things; rows ahead keep full paper. The count line says
//! `3 of 12`, which is the position at a glance and the only number this panel
//! adds to what the bottom bar already shows.

use iced::widget::{Column, Space, column, container, row, scrollable, text};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::player::{PlayerState, QueueRow, QueueRowState};
use crate::theme;
use crate::views::close_button;

/// Inner padding of the queue panel (logical px) — the album panel's, so the
/// two read as one slot rather than two surfaces that happen to be adjacent.
const PANEL_PAD: f32 = theme::GAP_XL;

/// The queue panel: a header with the ✕, the album the queue came from, the
/// count line, and the rows.
///
/// Every string here is *owned*, straight from
/// [`PlayerState::queue_list`]'s render-ready reading, which is why the
/// element is `'static`: the panel's contents are a projection of engine
/// events and a request-side record, not a borrow of the library, so nothing
/// on screen can outlive a view-model rebuild mid-scan.
pub(crate) fn view(player: &PlayerState) -> Element<'static, Message> {
    let content = match player.queue_list() {
        None => empty_state(),
        Some(list) => {
            let rows: Vec<Element<'static, Message>> =
                list.rows.into_iter().map(queue_row).collect();
            let mut heading = column![text("Queue").size(theme::SIZE_TITLE).font(theme::SEMIBOLD)]
                .spacing(theme::GAP_XS);
            if let Some(album) = list.album {
                heading = heading.push(
                    text(album)
                        .size(theme::SIZE_EMPHASIS)
                        .color(theme::PAPER_DIM)
                        .wrapping(text::Wrapping::None),
                );
            }
            heading = heading
                .push(
                    text(list.artist)
                        .size(theme::SIZE_META)
                        .color(theme::PAPER_DIM)
                        .wrapping(text::Wrapping::None),
                )
                .push(
                    text(list.summary)
                        .size(theme::SIZE_META)
                        .font(theme::MONO)
                        .color(theme::PAPER_FAINT),
                );
            column![
                heading,
                scrollable(Column::with_children(rows).spacing(theme::GAP_XXS))
                    .height(Length::Fill),
            ]
            .spacing(theme::GAP_MD)
            .into()
        }
    };
    let body = column![
        header_row(),
        content,
        text("Esc closes · Q toggles")
            .size(theme::SIZE_CAPTION)
            .color(theme::PAPER_FAINT),
    ]
    .spacing(theme::GAP_MD);
    container(body)
        .width(Length::Fixed(theme::PANEL_W))
        .height(Length::Fill)
        .padding(PANEL_PAD)
        .style(theme::panel)
        .into()
}

/// The panel's top row: the dismissal ✕, hugging the right edge.
///
/// Its own row, above everything, so the mark sits in the same place in this
/// panel as in the album panel beside it — one slot, one close control, one
/// position.
fn header_row() -> Element<'static, Message> {
    row![
        Space::with_width(Length::Fill),
        close_button("Close the queue"),
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
                .color(theme::PAPER_DIM),
            text("Play an album and it appears here")
                .size(theme::SIZE_META)
                .color(theme::PAPER_FAINT),
        ]
        .spacing(theme::GAP_SM)
        .align_x(iced::Alignment::Center),
    )
    .center(Length::Fill)
    .into()
}

/// One queue row: position (or the lamp dot when it is playing), title over
/// its artist where there is one, monospace duration.
///
/// The number column is [`theme::TRACK_NO_W`] wide whichever it holds, so the
/// dot arriving as a track starts moves no text — the same fixed-slot rule the
/// bottom bar is built on, applied to a list that changes under the reader.
fn queue_row(row_state: QueueRow) -> Element<'static, Message> {
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
            .font(theme::MONO)
            .color(theme::PAPER_FAINT)
            .into()
    };
    // The playing row's title gains the medium weight the now-playing bar
    // gives the same string; everything else keeps the list's regular face, so
    // the emphasis moves down the queue with the music.
    let heading = text(row_state.title)
        .size(theme::SIZE_BODY)
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
                .color(theme::PAPER_DIM)
                .wrapping(text::Wrapping::None),
        );
    }
    container(
        row![
            container(marker)
                .width(Length::Fixed(theme::TRACK_NO_W))
                .align_x(alignment::Horizontal::Right),
            container(title).width(Length::Fill),
            text(row_state.duration)
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER_FAINT),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .padding(theme::pad(theme::GAP_XS, theme::GAP_XS))
    .style(move |_theme| theme::queue_row(playing))
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
