//! **The Now playing place** — the sounding record at the size it deserves.
//!
//! The owner's extension of the lane's head: *"as an extension we will want a
//! Now playing page at the top with the Home and Library"*. Its subject is
//! *what is sounding*, which is the bottom bar's subject on a page — and the
//! reason it is not `Place::Album`, whose subject is the record you pointed
//! at.
//!
//! This file is the frame; the surface lands in the third commit, designed so
//! that the kiosk full-screen mode is this same surface at a larger size
//! (`docs/design/12-now-playing-and-kiosk.md`, unfinished, read for intent).

use iced::widget::{column, container, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::theme;

/// The Now playing place's body.
pub(crate) fn view<'a>(
    _shelf: &'a Shelf,
    _player: &'a PlayerState,
    _width: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    container(
        column![
            text("Now playing")
                .size(theme::SIZE_TITLE)
                .line_height(theme::LEADING_TITLE)
                .font(theme::SEMIBOLD),
            text("The record that is sounding.")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        ]
        .spacing(theme::GAP_SM)
        .align_x(alignment::Horizontal::Center),
    )
    .center(Length::Fill)
    .into()
}
