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
    let heading = column![
        text("baz")
            .size(theme::SIZE_EMPHASIS)
            .font(theme::MONO)
            .color(theme::LAMP),
        text("Where's your music?")
            .size(theme::SIZE_HERO)
            .font(theme::SEMIBOLD),
        text("Point baz at a folder — the shelf fills as it scans.")
            .size(theme::SIZE_EMPHASIS)
            .color(theme::PAPER_DIM),
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
            .width(Length::Fixed(SETUP_INPUT_W))
            .style(theme::input),
    ]
    .spacing(theme::GAP_XL)
    .align_x(iced::Alignment::Center);
    if let Some(error) = &setup.error {
        content = content.push(
            text(error.as_str())
                .size(theme::SIZE_META)
                .color(theme::ALERT),
        );
    }
    content = content.push(
        text("Enter confirms · next time, `baz` remembers (or run `baz DIR`)")
            .size(theme::SIZE_CAPTION)
            .color(theme::PAPER_FAINT),
    );
    container(content).center(Length::Fill).into()
}
