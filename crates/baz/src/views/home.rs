//! **The Home place** — the interrupted run, and what is new.
//!
//! ADR-0030 §3.2 recommended a home *band* at the head of the Library's body
//! and drew this page in §9.4 as the alternative. **The owner chose the
//! page**, and `docs/REFUSALS.md`'s preamble says that settles it; the ADR
//! carries the amendment.
//!
//! This file is the frame. Its two sections — `CONTINUE` and `RECENTLY
//! ADDED` — land in the commit after the lane's, so that the lane ships as
//! one reviewable change and the page as another.

use iced::widget::{column, container, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::theme;

/// The Home place's body.
pub(crate) fn view<'a>(
    _shelf: &'a Shelf,
    _player: &'a PlayerState,
    _width: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    container(
        column![
            text("Home")
                .size(theme::SIZE_TITLE)
                .line_height(theme::LEADING_TITLE)
                .font(theme::SEMIBOLD),
            text("What you were in the middle of, and what is new.")
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
