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

/// The first-run screen: **one question, one input, one line of copy.**
///
/// The one surface every new user meets, and the only one with no data behind
/// it, so it is the only place in baz where the type is the whole design. The
/// block is centred in the window and its lines are **left-aligned within it**,
/// against the input's left edge: four centred lines of different lengths make
/// a diamond, and the one thing a wall label is never is a diamond. The eye
/// starts every line on the same pixel and the field starts there too, so the
/// question, the field and the footnote read as one object.
///
/// The error line, when the last attempt failed, sits **between** the field and
/// the footnote — where the eye already is after pressing Enter — and its slot
/// is not reserved, because this is the one screen in the product where nothing
/// is playing and nothing may move under a pointer that is not there.
pub(crate) fn view(setup: &Setup) -> Element<'_, Message> {
    let heading = column![
        // The wordmark, deliberately unlit. This screen is the first frame baz
        // ever draws and there is nothing playing on it, so the one accent the
        // room reserves for playback truth has no business here (`theme`'s
        // accent-discipline note).
        text("baz")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .font(theme::MEDIUM)
            .color(theme::heading_ink()),
        text("Where's your music?")
            .size(theme::SIZE_HERO)
            .line_height(theme::LEADING_HERO)
            .font(theme::SEMIBOLD)
            .color(theme::PAPER),
    ]
    .spacing(theme::GAP_SM)
    .align_x(iced::Alignment::Start);
    let mut content = column![
        heading,
        column![
            text_input("/path/to/your/music", &setup.input)
                .on_input(Message::SetupInput)
                .on_submit(Message::SetupSubmit)
                .padding(theme::pad(theme::GAP_SM + 2.0, theme::GAP_MD))
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .width(Length::Fixed(SETUP_INPUT_W))
                .style(theme::input),
            // One line of copy, and it says what happens next rather than what
            // to do — the folder is the only instruction the screen needs and
            // the field already carries it.
            text("Covers land on the wall as they are read. Nothing waits for the scan.")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(theme::PAPER_DIM),
        ]
        .spacing(theme::GAP_SM)
        .align_x(iced::Alignment::Start),
    ]
    .spacing(theme::GAP_XL)
    .align_x(iced::Alignment::Start);
    if let Some(error) = &setup.error {
        content = content.push(
            text(error.as_str())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(theme::ALERT),
        );
    }
    content = content.push(
        text("Enter confirms · next time baz remembers, or run baz DIR")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION)
            .color(theme::heading_ink()),
    );
    // The block is exactly as wide as the field it is built around, and that
    // block is centred in the window — so the copy wraps against the same right
    // edge the input has, and every line starts on the input's left edge.
    container(content.width(Length::Fixed(SETUP_INPUT_W)))
        .center(Length::Fill)
        .into()
}
