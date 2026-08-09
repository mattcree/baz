//! The first-run screen: "Where's your music?".
//!
//! One question, two doors, one hint — the whole surface. It is the only
//! screen that exists before there is a library to draw, so it carries no
//! chrome of its own.
//!
//! # The two doors (doc 11 §5 P1)
//!
//! The question is answered by **pointing or by typing** — `Browse…` opens
//! the desktop's own folder dialog beside the typed well, ADR-0025 §1's
//! two-door shape arriving at the screen it was first deferred from
//! (ADR-0025 §3's deferral, superseded by that ADR's own argument: two doors
//! are still one question). The typed field stays for the NAS case a dialog
//! structurally cannot offer, and its `stat` now runs on the blocking pool
//! (`app::check_folder`), which retires the deferral's other ground.
//!
//! The window is also a **drop target** where the toolkit delivers one:
//! winit 0.30 publishes file-drop events on X11 and not on Wayland, so the
//! drop is an accelerator, never a promise — the screen's copy does not
//! advertise it, and the one line that mentions it appears only while a drag
//! is actually over the window (an event only the delivering platform ever
//! sends). P1's adopt-modified text governs: the button is not blocked on
//! the platforms the event is missing from.

use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Element, Length, alignment};

use crate::app::{Message, Setup};
use crate::theme;

/// The first-run screen's folder input width (logical px).
///
/// **360, where it was 460**, and the hundred pixels are the audit's defect 12.
/// The block is centred to the pixel and its ink is not: the lines are
/// left-aligned and ragged-right, so the longest of them reached 773 of 870 and
/// **93 px of the block's right half was the outline of an empty field**. The
/// well is the width of the copy it stands under now, which is the width the
/// block always optically had. `Browse…` stands *inside* this measure, beside
/// the field — the block did not widen to gain its second door.
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

/// The first-run screen: **one question, one input beside its picker, one
/// line of copy.**
///
/// The one surface every new user meets, and the only one with no data behind
/// it, so it is the only place in baz where the type is the whole design. The
/// block is centred in the window and its lines are **left-aligned within it**,
/// against the input's left edge: four centred lines of different lengths make
/// a diamond, and the one thing a wall label is never is a diamond. The eye
/// starts every line on the same pixel and the field starts there too, so the
/// question, the doors and the footnote read as one object.
///
/// The error line, when the last attempt failed, sits **between** the doors and
/// the footnote — where the eye already is after pressing Enter — and its slot
/// is not reserved, because this is the one screen in the product where nothing
/// is playing and nothing may move under a pointer that is not there. (The
/// drop-hover line takes the same unreserved slot for the same reason: it
/// exists only while a drag holds the pointer, and only on the platform that
/// reports one.)
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
            // The two doors on one line (ADR-0025 §1's shape): the typed
            // well flexing beside the picker's word. A human placeholder,
            // not `/path/to/your/music` — see-and-point means never asking
            // a first-timer to recall filesystem syntax, even by example.
            row![
                text_input("Your music folder", &setup.input)
                    .on_input(Message::SetupInput)
                    .on_submit(Message::SetupSubmit)
                    // The product's one control height, like the search well
                    // it is the first-run cousin of (law L7): this field
                    // stood **40 px** against a published floor of 32.
                    .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
                    .size(theme::SIZE_EMPHASIS)
                    .line_height(theme::LEADING_EMPHASIS)
                    .width(Length::Fill)
                    .style(move |_theme, status| theme::input(room, status)),
                browse_control(),
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
            // One line of copy, and it says what happens next rather than what
            // to do — the folder is the only instruction the screen needs and
            // the doors already carry it.
            text("Covers land as they are read. Nothing waits for the scan.")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
        ]
        .spacing(theme::GAP_SM)
        .align_x(iced::Alignment::Start),
    ]
    .spacing(theme::GAP_XL)
    .align_x(iced::Alignment::Start);
    if setup.hovering_drop {
        // Only while a drag is over the window — the event arrives on X11
        // alone (module docs), so this line can never promise a drop the
        // platform will not deliver.
        content = content.push(
            text("Drop the folder to open it.")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
        );
    }
    if let Some(error) = &setup.error {
        content = content.push(
            text(error.as_str())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.alert),
        );
    }
    content = content.push(
        // The CLI teaching (`…or run baz DIR`) moved to `--help` and the
        // README, where its audience lives (doc 11 §5 P1): a first-run
        // screen teaching terminal invocation syntax was remember-and-type
        // twice over.
        text("Enter confirms · next time baz remembers")
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

/// **`Browse…`** — the pointing answer, in the Settings door's own anatomy
/// (ADR-0025 §1): a quiet word at the product's one control height, the
/// ellipsis honestly promising the dialog. Sends the identical message the
/// Settings control sends; which screen answers it is the shell's routing,
/// so the two doors cannot drift apart in behaviour.
fn browse_control() -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text("Browse\u{2026}")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(Message::PickMusicFolder)
    .into()
}
