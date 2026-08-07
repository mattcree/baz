//! The slim top bar: the search well on the left, quiet status on the right.

use iced::widget::{Space, column, container, horizontal_rule, row, text, text_input};
use iced::{Element, Length};

use crate::app::{Message, Shelf, search_id};
use crate::theme;

/// The search field's width in the top bar (logical px).
const SEARCH_W: f32 = 360.0;

/// The slim top bar: the search well on the left, quiet status on the
/// right, a hairline rule below.
pub(crate) fn view(shelf: &Shelf) -> Element<'_, Message> {
    let search = text_input("Search artists, albums, tracks…", &shelf.query)
        .id(search_id())
        .on_input(Message::SearchChanged)
        .padding(theme::pad(theme::GAP_SM, theme::GAP_MD))
        .size(theme::SIZE_BODY)
        .width(Length::Fixed(SEARCH_W))
        .style(theme::input);
    let mut status = row![
        text(counts_line(shelf))
            .size(theme::SIZE_META)
            .font(theme::MONO)
            .color(theme::PAPER_FAINT)
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center);
    if shelf.scanning {
        status = status.push(
            text("scanning…")
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::LAMP),
        );
    }
    if shelf.files_skipped > 0 {
        status = status.push(
            text(format!("{} files skipped", shelf.files_skipped))
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER_FAINT),
        );
    }
    if let Some(problem) = &shelf.problem {
        status = status.push(
            text(problem.as_str())
                .size(theme::SIZE_META)
                .color(theme::ALERT),
        );
    }
    column![
        container(
            row![search, Space::with_width(Length::Fill), status]
                .spacing(theme::GAP_LG)
                .align_y(iced::Alignment::Center),
        )
        .padding(theme::pad(theme::GAP_SM + 2.0, theme::GAP_LG)),
        horizontal_rule(1).style(theme::hairline),
    ]
    .into()
}

/// The unobtrusive count text: album/track counts, or the filtered
/// count while a query narrows the shelf. Status, not modal — by
/// design; scan/skip/problem notes render as separate colored segments.
fn counts_line(shelf: &Shelf) -> String {
    if shelf.query.trim().is_empty() {
        format!(
            "{} albums · {} tracks",
            shelf.albums.len(),
            shelf.library.len()
        )
    } else {
        format!("{} / {} albums", shelf.visible.len(), shelf.albums.len())
    }
}
