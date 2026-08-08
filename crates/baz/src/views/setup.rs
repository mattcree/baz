//! The first-run screen: "Where's your music?".
//!
//! One question, one field, one hint — the whole surface. It is the only
//! screen that exists before there is a library to draw, so it carries no
//! chrome of its own.

use iced::widget::{Space, column, container, text, text_input};
use iced::{Element, Length};

use crate::app::{Message, Setup};
use crate::theme;

/// The first-run screen's folder input width (logical px).
///
/// **360, where it was 460**, and the hundred pixels are the audit's defect 12.
/// The block is centred to the pixel and its ink is not: the lines are
/// left-aligned and ragged-right, so the longest of them reached 773 of 870 and
/// **93 px of the block's right half was the outline of an empty field**. The
/// well is the width of the copy it stands under now, which is the width the
/// block always optically had.
const SETUP_INPUT_W: f32 = 360.0;

/// How much of the window's slack sits above the first-run block, as a portion
/// against [`BELOW`].
///
/// A single question on an empty wall belongs above the middle — the optical
/// convention and the rule of thirds agree, and the audit measured the block's
/// centre at 0.501 H, which is the one place it should not be. Two parts above
/// to three below puts it near 0.42 H at the shipped window.
const ABOVE: u16 = 2;
/// The portion of the window's slack below the block. See [`ABOVE`].
const BELOW: u16 = 3;

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
    let room = theme::active();
    let heading = column![
        // The wordmark, deliberately unlit. This screen is the first frame baz
        // ever draws and there is nothing playing on it, so the one accent the
        // room reserves for playback truth has no business here (`theme`'s
        // accent-discipline note).
        text("baz")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .font(theme::MEDIUM)
            .color(room.heading()),
        text("Where's your music?")
            .size(theme::SIZE_HERO)
            .line_height(theme::LEADING_HERO)
            .font(theme::SEMIBOLD)
            .color(room.paper),
    ]
    .spacing(theme::GAP_SM)
    .align_x(iced::Alignment::Start);
    let mut content = column![
        heading,
        column![
            text_input("/path/to/your/music", &setup.input)
                .on_input(Message::SetupInput)
                .on_submit(Message::SetupSubmit)
                // The product's one control height, like the search well it is
                // the first-run cousin of (law L7): this field stood **40 px**
                // against a published floor of 32.
                .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .width(Length::Fixed(SETUP_INPUT_W))
                .style(move |_theme, status| theme::input(room, status)),
            // One line of copy, and it says what happens next rather than what
            // to do — the folder is the only instruction the screen needs and
            // the field already carries it.
            text("Covers land on the wall as they are read. Nothing waits for the scan.")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
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
                .color(room.alert),
        );
    }
    content = content.push(
        text("Enter confirms · next time baz remembers, or run baz DIR")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION)
            .color(room.heading()),
    );
    // The block is exactly as wide as the field it is built around, and that
    // block is centred *horizontally* in the window — so the copy wraps against
    // the same right edge the input has, and every line starts on the input's
    // left edge.
    //
    // Vertically it is **not** centred, and that is the correction: the slack is
    // split [`ABOVE`] : [`BELOW`], which lands the block's centre near 0.42 H
    // instead of 0.501 H. A hero block on an empty screen sits above the middle
    // or it reads as having sunk.
    container(
        column![
            Space::with_height(Length::FillPortion(ABOVE)),
            content.width(Length::Fixed(SETUP_INPUT_W)),
            Space::with_height(Length::FillPortion(BELOW)),
        ]
        .align_x(iced::Alignment::Center),
    )
    .center_x(Length::Fill)
    .height(Length::Fill)
    .into()
}
