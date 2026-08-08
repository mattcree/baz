//! The first-run screen: "Where's your music?".
//!
//! One question, one field, one hint — the whole surface. It is the only
//! screen that exists before there is a library to draw, so it carries no
//! chrome of its own.

use iced::widget::{column, container, text, text_input};
use iced::{Element, Length};

use crate::app::{Message, Setup};
use crate::theme;

/// The first-run screen's folder input width (logical px).
const SETUP_INPUT_W: f32 = 460.0;

/// The first-run screen: heading, folder input, an error line when the last
/// attempt failed, and the footnote that explains what Enter does.
pub(crate) fn view(setup: &Setup) -> Element<'_, Message> {
    let room = theme::active();
    let heading = column![
        // The wordmark, deliberately unlit. This screen is the first frame baz
        // ever draws and there is nothing playing on it, so the one accent the
        // room reserves for playback truth has no business here (`theme`'s
        // accent-discipline note).
        text("baz")
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .color(room.paper_faint),
        text("Where's your music?")
            .size(theme::SIZE_HERO)
            .line_height(theme::LEADING_HERO)
            .font(theme::SEMIBOLD),
        text("Point baz at a folder — the shelf fills as it scans.")
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .color(room.paper_dim),
    ]
    .spacing(theme::GAP_SM)
    .align_x(iced::Alignment::Center);
    let mut content = column![
        heading,
        text_input("/path/to/your/music", &setup.input)
            .on_input(Message::SetupInput)
            .on_submit(Message::SetupSubmit)
            .padding(theme::pad(theme::GAP_SM + 2.0, theme::GAP_MD))
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .width(Length::Fixed(SETUP_INPUT_W))
            .style(move |_theme, status| theme::input(room, status)),
    ]
    .spacing(theme::GAP_XL)
    .align_x(iced::Alignment::Center);
    if let Some(error) = &setup.error {
        content = content.push(
            text(error.as_str())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.alert),
        );
    }
    content = content.push(
        text("Enter confirms · next time, `baz` remembers (or run `baz DIR`)")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION)
            .color(room.paper_faint),
    );
    container(content).center(Length::Fill).into()
}
