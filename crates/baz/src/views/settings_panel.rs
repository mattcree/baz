//! The settings panel: baz's first settings surface, and the shape every
//! setting after this one takes.
//!
//! # What this surface is for
//!
//! Everything that is a standing decision rather than a transport action. It
//! holds one section today — **ReplayGain** (ADR-0013), which the engine has
//! supported in full and which nothing on screen could switch on — and it is
//! built as the container for the ones the vision promises next: the output
//! chain, watch folders, the enrichment toggles that are off by default.
//!
//! Why it is a panel in the right-hand rail rather than a popover from a gear
//! is argued in [`crate::panels`], where the state machine that raises it
//! lives. In short: the rail *is* the "one deliberate layer down" the vision's
//! progressive-disclosure pillar names, it cannot cover the covers or the
//! transport, and it inherits every dismissal baz already has instead of
//! hand-rolling three that iced 0.13 gives no primitive for.
//!
//! # The shape a section takes
//!
//! One heading, one sentence of what the section is for, the controls, and —
//! where the engine has something to say about the here and now — a readout
//! underneath. A future section is another block in the same scroll, in the
//! same order, with the same three type sizes; nothing about the panel has to
//! be revisited to add one.
//!
//! # Tone
//!
//! Every string here comes from [`crate::replaygain`] already written, and
//! this module chooses no words of its own about what the engine is doing.
//! That is deliberate and it is the same rule the bottom bar's signal note
//! follows: the vocabulary is unit-tested where it is decided, and the view
//! cannot soften or sharpen it. Nothing here is styled as a fault, and no
//! reading gets the lamp amber — the accent means playback truth (ADR-0013 §8,
//! ADR-0009 §5), and how a gain stage is configured is not a claim about the
//! music.

use iced::widget::{Column, Space, button, checkbox, column, container, row, scrollable, text};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::player::PlayerState;
use crate::replaygain::{self, MODES};
use crate::theme;
use crate::views::close_button;

/// Inner padding of the settings panel (logical px) — the album and queue
/// panels', so the three read as one slot rather than three surfaces that
/// happen to be adjacent.
const PANEL_PAD: f32 = theme::GAP_XL;

/// The settings panel: the ✕, the heading, and one section per thing baz can
/// be told to do differently.
pub(crate) fn view(player: &PlayerState) -> Element<'_, Message> {
    let heading = column![
        text("Settings")
            .size(theme::SIZE_TITLE)
            .font(theme::SEMIBOLD),
        text("Kept in config.toml, and remembered next time.")
            .size(theme::SIZE_META)
            .color(theme::PAPER_FAINT),
    ]
    .spacing(theme::GAP_XS);

    let body = column![
        header_row(),
        heading,
        scrollable(
            Column::with_children(vec![replay_gain_section(player)])
                .spacing(theme::GAP_XL)
                .padding(theme::scroll_gutter())
        )
        .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
        .style(theme::scrollbar)
        .height(Length::Fill),
        text("Esc closes · Ctrl+, toggles")
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

/// The panel's top row: the dismissal ✕, hugging the right edge — the same
/// slot, in the same panel width, as the album and queue panels'.
fn header_row() -> Element<'static, Message> {
    row![
        Space::with_width(Length::Fill),
        close_button("Close the settings", Message::ClosePanel),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

/// The ReplayGain section: the mode, what that mode does, the two pre-amps,
/// clipping prevention, and what it all came to for the track playing now.
fn replay_gain_section(player: &PlayerState) -> Element<'_, Message> {
    let state = player.replay_gain();
    // No engine, nothing to configure — the same rule the album panel's Play
    // button follows, and for the same reason: a control that cannot act must
    // not pretend it can.
    let live = player.engine_ready();

    let mut section = column![
        text("ReplayGain")
            .size(theme::SIZE_EMPHASIS)
            .font(theme::MEDIUM),
        text("Play everything at the loudness its tags declare.")
            .size(theme::SIZE_META)
            .color(theme::PAPER_DIM),
        mode_selector(state, live),
        // The mode's own sentence, in the quiet ink: present in every mode, so
        // choosing one is never a guess — and in a slot of
        // [`theme::SETTING_NOTE_H`], reserved for the longest of them, so that
        // switching modes moves nothing below it. Without the reservation,
        // pressing *Album* (whose sentence wraps to two lines) would push the
        // pre-amps down by a line, taking the control out from under the
        // pointer that had just chosen it.
        container(
            text(state.mode_note())
                .size(theme::SIZE_META)
                .color(theme::PAPER_FAINT),
        )
        .height(Length::Fixed(theme::SETTING_NOTE_H)),
    ]
    .spacing(theme::GAP_SM);

    section = section
        .push(stepper_row(
            "Pre-amp",
            state.preamp_label(),
            live && state.preamp_can_step(-1),
            live && state.preamp_can_step(1),
            Message::ReplayGainPreamp(-1),
            Message::ReplayGainPreamp(1),
        ))
        .push(stepper_row(
            "Untagged files",
            state.no_tag_preamp_label(),
            live && state.no_tag_preamp_can_step(-1),
            live && state.no_tag_preamp_can_step(1),
            Message::ReplayGainNoTagPreamp(-1),
            Message::ReplayGainNoTagPreamp(1),
        ))
        .push(
            checkbox("Keep peaks below full scale", state.prevent_clipping())
                .size(theme::SIZE_BODY)
                .text_size(theme::SIZE_META)
                .spacing(theme::GAP_SM)
                .style(theme::check)
                .on_toggle_maybe(live.then_some(Message::ReplayGainPreventClipping)),
        );

    // What is in force right now — present only while a track is playing and
    // ReplayGain is on. Off states no figure at all: the engine performs no
    // ReplayGain arithmetic in that mode, and a `0.00 dB` here would describe
    // arithmetic that is not happening (ADR-0013 §2).
    if let Some(readout) = player.replay_gain_readout() {
        section = section.push(
            column![
                text(readout.gain)
                    .size(theme::SIZE_META)
                    .font(theme::MONO)
                    .color(theme::PAPER),
                text(readout.detail)
                    .size(theme::SIZE_META)
                    .color(theme::PAPER_FAINT),
            ]
            .spacing(theme::GAP_XXS),
        );
    }

    if let Some(note) = player.availability_note() {
        section = section.push(text(note).size(theme::SIZE_META).color(theme::PAPER_FAINT));
    }

    section.into()
}

/// The mode control: the same quiet segmented control the album panel's
/// edition selector uses.
///
/// Reused rather than invented, because it is the same question — *which one
/// of these few* — and the room should answer it the same way twice. The
/// order is [`MODES`]', which is Off first: it is the default and the one that
/// changes nothing.
fn mode_selector(state: replaygain::ReplayGain, live: bool) -> Element<'static, Message> {
    let mut segments = row![].spacing(theme::GAP_XXS);
    for mode in MODES {
        let selected = state.mode() == mode;
        segments = segments.push(
            button(
                container(
                    text(replaygain::mode_label(mode))
                        .size(theme::SIZE_META)
                        .font(theme::MEDIUM)
                        .wrapping(text::Wrapping::None),
                )
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
            .style(move |_theme, status| theme::segment(status, selected))
            .on_press_maybe(live.then_some(Message::ReplayGainMode(mode))),
        );
    }
    container(segments)
        .width(Length::Fill)
        .padding(theme::SEGMENT_INSET)
        .style(theme::segmented)
        .into()
}

/// One numeric setting: its name, its value, and a `−`/`+` pair.
///
/// The value sits in a [`theme::SETTING_VALUE_W`] slot in [`theme::MONO`], so
/// a repeated press cannot move the button under the pointer holding it — the
/// same fixed-slot rule the bottom bar is built on. A stepper at the end of
/// its travel renders disabled rather than absorbing the press.
fn stepper_row(
    label: &'static str,
    value: String,
    can_decrease: bool,
    can_increase: bool,
    decrease: Message,
    increase: Message,
) -> Element<'static, Message> {
    row![
        text(label)
            .size(theme::SIZE_META)
            .color(theme::PAPER_DIM)
            .wrapping(text::Wrapping::None),
        Space::with_width(Length::Fill),
        container(
            text(value)
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER)
                .wrapping(text::Wrapping::None)
        )
        .width(Length::Fixed(theme::SETTING_VALUE_W))
        .align_x(alignment::Horizontal::Right),
        stepper("\u{2212}", can_decrease, decrease),
        stepper("+", can_increase, increase),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .into()
}

/// One `−` or `+` button: the transport's quiet card in a smaller square.
///
/// A minus sign (U+2212), not a hyphen: it is the character that matches the
/// `+` in width and in height, and these two sit side by side.
fn stepper(glyph: &'static str, enabled: bool, message: Message) -> Element<'static, Message> {
    button(
        container(
            text(glyph)
                .size(theme::SIZE_BODY)
                .font(theme::MONO)
                .color(if enabled {
                    theme::PAPER
                } else {
                    theme::PAPER_MUTED
                }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fixed(theme::STEPPER_HIT))
    .height(Length::Fixed(theme::STEPPER_HIT))
    .padding(0)
    .style(theme::transport)
    .on_press_maybe(enabled.then_some(message))
    .into()
}
