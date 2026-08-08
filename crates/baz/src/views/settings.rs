//! The **Settings place**: baz's standing decisions, and the shape every
//! setting after the first one takes.
//!
//! # What this surface is for
//!
//! Everything that is a standing decision rather than a transport action. It
//! holds one section today — **ReplayGain** (ADR-0013), whose content moved
//! here from the rail panel *verbatim* — and it is built as the container for
//! the ones the vision promises next: the output chain and exclusive mode, a
//! signal-path readout, library roots and watch folders, the enrichment toggles
//! that are off by default.
//!
//! # Why it is a place, and no longer a panel
//!
//! It was a panel in the right-hand rail, sharing 340 px with the album
//! inspector and the queue, and the argument for that is answered at length in
//! [`crate::place`] and ADR-0016. In short: a preference is not a glance, the
//! rail was simultaneously too narrow and 60% empty for the settings that
//! exist, and none of the settings that are *coming* is a section in a 292 px
//! column. Leaving the shelf is the right cost — you are not browsing while you
//! set a pre-amp — and it is free to reverse, because the Library's scroll,
//! query and selection live in one struct that nothing here touches.
//!
//! # The shape a section takes
//!
//! One heading, one sentence of what the section is for, the controls, and —
//! where the engine has something to say about the here and now — a readout
//! underneath. A future section is another entry in the list on the left and
//! another block in the same scroll, in the same order, with the same three
//! type sizes. **Nothing about the layout has to be revisited to add one**,
//! which is the property a place buys that a panel could not.
//!
//! The section list has one entry today, and it is drawn anyway. A spine with
//! one vertebra looks like an over-build for a week and like the obvious place
//! to put the next thing forever after; the alternative is that the second
//! section arrives and has to invent the navigation as well as itself.
//!
//! # The frame does not move
//!
//! Places replace each other, and the two frames a listener sees either side of
//! that replacement — the top strip and the now-playing bar — must be the same
//! height in both, or navigating would look like the window resizing. So this
//! place's header carries the same padding and the same hairline as the
//! Library's top bar, and the bar below is drawn by the shell for both.
//!
//! # Tone
//!
//! Every string about ReplayGain comes from [`crate::replaygain`] already
//! written, and this module chooses no words of its own about what the engine is
//! doing. That is deliberate and it is the same rule the bottom bar's signal
//! note follows: the vocabulary is unit-tested where it is decided, and the view
//! cannot soften or sharpen it. Nothing here is styled as a fault, and no
//! reading gets the lamp amber — the accent means playback truth (ADR-0013 §8,
//! ADR-0009 §5), and how a gain stage is configured is not a claim about the
//! music.

use iced::widget::{
    Column, Space, button, checkbox, column, container, horizontal_rule, row, scrollable, text,
};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::player::PlayerState;
use crate::replaygain::{self, MODES};
use crate::theme;

/// Inner padding of the place's content area (logical px).
///
/// [`theme::HANG`], not `GAP_XL`: a place fills the window, so its content hangs
/// from the **one window gutter** every other window-edge surface hangs from
/// (law L1). `GAP_XL` is padding *inside* a panel and was never a window margin;
/// spending it as one is how baz ended up with three of them — 16 for the
/// chrome, 24 here, 40 on the wall — and nothing in either bar aligned with
/// anything in the collection.
const PLACE_PAD: f32 = theme::HANG;

/// The sections this place holds, in the order they are listed.
///
/// One today. It is a `const` rather than an inline string because the next one
/// is an entry here and nothing else — which is the whole claim the place is
/// making about how settings grow.
const SECTIONS: [&str; 1] = ["Playback"];

/// The Settings place: a header with the way back, a list of sections, and the
/// current section's content.
///
/// `window_width` decides the arrangement and nothing else: at
/// [`theme::SETTINGS_BREAKPOINT`] and above, the section list is a column on
/// the left and the content sits beside it; below it the two stack, because
/// under a thousand pixels the list and a 640 px form cannot both have their
/// width and the form is the one being used.
pub(crate) fn view(player: &PlayerState, window_width: f32) -> Element<'_, Message> {
    let room = theme::active();
    let beside_the_list = window_width >= theme::SETTINGS_BREAKPOINT;
    let content = container(
        scrollable(
            Column::with_children(vec![replay_gain_section(player)])
                .spacing(theme::GAP_XL)
                .padding(theme::scroll_gutter()),
        )
        .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .height(Length::Fill),
    )
    .width(Length::Fixed(content_width(window_width, beside_the_list)))
    .height(Length::Fill);

    let body: Element<'_, Message> = if beside_the_list {
        row![section_list(), content]
            .spacing(theme::GAP_XL)
            .height(Length::Fill)
            .into()
    } else {
        column![
            text(SECTIONS[0])
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .font(theme::MEDIUM),
            content,
        ]
        .spacing(theme::GAP_MD)
        .height(Length::Fill)
        .into()
    };

    column![
        header(),
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(PLACE_PAD),
    ]
    .into()
}

/// How wide the form gets: what the window has left for it, capped at
/// [`theme::SETTINGS_CONTENT_W`].
///
/// Computed rather than expressed as a maximum on the container, and the
/// difference is not cosmetic: a `max_width` bounds the *limits* a child is
/// laid out in, and a `Fill` child inside a `Shrink` container resolves against
/// what the row actually handed it. Measuring the rendered pixels is what
/// caught that — the segmented control ran 998 px wide in a 640 px cap — so the
/// width is arithmetic the view does itself and `theme.rs` asserts.
///
/// The floor matters as much as the cap: at a small window the form gets
/// whatever there is, because a stepper row that will not fit is worse than a
/// long one.
///
/// # It answers the window now
///
/// The cap used to be the constant [`theme::SETTINGS_CONTENT_W`], so the form's
/// right edge landed on **878 at a 1280 px window and 878 at a 1920 px one** —
/// 0.686 W and then 0.457 W, with a thousand pixels of empty wall beside it and
/// one right-aligned line of type stranded in it (the audit's defect 9). A
/// measure has a comfortable range rather than a single right answer, so the
/// target is half the window, clamped into
/// `[SETTINGS_CONTENT_W, SETTINGS_CONTENT_MAX]` — 55 to 75 characters of body
/// text — and bounded by what the window actually has left.
fn content_width(window_width: f32, beside_the_list: bool) -> f32 {
    let taken = if beside_the_list {
        2.0f32.mul_add(PLACE_PAD, theme::SETTINGS_NAV_W) + theme::GAP_XL
    } else {
        2.0 * PLACE_PAD
    };
    let measure =
        (0.5 * window_width).clamp(theme::SETTINGS_CONTENT_W, theme::SETTINGS_CONTENT_MAX);
    (window_width - taken).clamp(theme::PANEL_W - 2.0 * theme::GAP_XL, measure)
}

/// The place's top strip: the way back, and the place's name.
///
/// It occupies the Library's top-bar geometry exactly — the same vertical
/// padding, the same horizontal inset, the same hairline underneath — so that
/// moving between the two places does not slide the content area up or down by
/// a pixel. The frame is the frame in every place (§4.3).
///
/// **Back is a word, not a chevron.** baz draws its glyphs itself from a small
/// deliberate set ([`crate::icon`]), and a back arrow would be a new one for a
/// control that has a short and unambiguous name — the same argument that keeps
/// `Settings` a word in the top bar it returns to. It sends the message
/// <kbd>Ctrl</kbd>+<kbd>,</kbd> sends and the message the top bar's control
/// sends, so all three are one press.
fn header() -> Element<'static, Message> {
    let room = theme::active();
    let back = button(
        // Centred in its own box, like `Settings` across the frame from it
        // (law L3).
        container(
            text("‹ Library")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    // The same height as the top bar's `Settings`, which is the control this
    // one swaps places with: the two strips are one frame, and a way-back that
    // stood shorter than the control it replaced would make the header jump on
    // every navigation.
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(Message::ToggleSettings);
    column![
        container(
            row![
                back,
                text("Settings")
                    .size(theme::SIZE_EMPHASIS)
                    .line_height(theme::LEADING_EMPHASIS)
                    .font(theme::MEDIUM),
                Space::with_width(Length::Fill),
                text("Kept in config.toml, and remembered next time.")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(theme::GAP_LG)
            .align_y(iced::Alignment::Center),
        )
        // The Library's top bar's geometry, exactly — the same padding, the same
        // one window gutter, the same hairline — because the two strips are one
        // frame and navigating between the places may not slide it.
        .padding(theme::pad(theme::TOP_BAR_PAD_V, theme::HANG)),
        horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
    ]
    .into()
}

/// The section list: the place's spine.
///
/// Styled with the same segmented control the edition selector and the
/// ReplayGain mode use, for the same reason the room answers *which one of
/// these few* the same way everywhere. The current section is the only one
/// there is, so it is the only one selected — and it is not a control that can
/// do anything yet, which is why it is drawn as a selected segment rather than
/// as a live button that would go nowhere.
fn section_list() -> Element<'static, Message> {
    let room = theme::active();
    let mut list = column![].spacing(theme::GAP_XXS);
    for (index, section) in SECTIONS.iter().enumerate() {
        let current = index == 0;
        list = list.push(
            container(
                text(*section)
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .font(theme::MEDIUM)
                    .wrapping(text::Wrapping::None),
            )
            .width(Length::Fill)
            // One control height (law L7): the entry is a nav target and stands
            // `TRANSPORT_HIT`, not the 36 px its own padding used to make it.
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .align_y(alignment::Vertical::Center)
            .padding(theme::pad(0.0, theme::GAP_MD))
            .style(move |_theme| {
                let style = theme::segment(room, iced::widget::button::Status::Active, current);
                container::Style {
                    background: style.background,
                    text_color: Some(style.text_color),
                    border: style.border,
                    ..container::Style::default()
                }
            }),
        );
    }
    container(list)
        .width(Length::Fixed(theme::SETTINGS_NAV_W))
        .height(Length::Fill)
        .into()
}

/// The ReplayGain section: the mode, what that mode does, the two pre-amps,
/// clipping prevention, and what it all came to for the track playing now.
fn replay_gain_section(player: &PlayerState) -> Element<'_, Message> {
    let room = theme::active();
    let state = player.replay_gain();
    // No engine, nothing to configure — the same rule the album panel's Play
    // button follows, and for the same reason: a control that cannot act must
    // not pretend it can.
    let live = player.engine_ready();

    let mut section = column![
        section_heading(
            "ReplayGain",
            "Play everything at the loudness its tags declare.",
        ),
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
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
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
            // **A checkbox is a pointer target too** (law L7). It was
            // `SIZE_BODY` — a **13 px** box, the smallest control in the product
            // by a factor of two and the only one with no floor at all. It takes
            // [`theme::STEPPER_HIT`], the named secondary target, and its row
            // stands the full `TRANSPORT_HIT` so the tick sits on the same line
            // rhythm as the stepper rows above it.
            container(
                checkbox("Keep peaks below full scale", state.prevent_clipping())
                    .size(theme::STEPPER_HIT)
                    .text_size(theme::SIZE_META)
                    .text_line_height(theme::LEADING_META)
                    .spacing(theme::GAP_SM)
                    .style(move |_theme, status| theme::check(room, status))
                    .on_toggle_maybe(live.then_some(Message::ReplayGainPreventClipping)),
            )
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .align_y(alignment::Vertical::Center),
        );

    // What is in force right now — present only while a track is playing and
    // ReplayGain is on. Off states no figure at all: the engine performs no
    // ReplayGain arithmetic in that mode, and a `0.00 dB` here would describe
    // arithmetic that is not happening (ADR-0013 §2).
    if let Some(readout) = player.replay_gain_readout() {
        section = section.push(readout_block(vec![
            (readout.gain, room.paper),
            (readout.detail, room.paper_faint),
        ]));
    }

    if let Some(note) = player.availability_note() {
        section = section.push(readout_block(vec![(note.clone(), room.paper_faint)]));
    }

    section.into()
}

/// A section's first two lines: **its name, then one sentence saying what it
/// is for.**
///
/// The shape every setting after the first one takes, and the reason it is a
/// function rather than two `text` calls copied into the next section: a place
/// whose sections each invented their own heading treatment is a junk drawer
/// with headings. Name in the emphasis size and the medium weight, sentence in
/// the meta size and the dim ink, one rung of the ladder between them.
///
/// One sentence, present tense, about what the setting *does* rather than what
/// it is — the vocabulary rule the whole product follows.
fn section_heading(name: &'static str, sentence: &'static str) -> Element<'static, Message> {
    let room = theme::active();
    column![
        text(name)
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .font(theme::MEDIUM)
            .color(room.paper),
        text(sentence)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_dim),
    ]
    .spacing(theme::GAP_XXS)
    .into()
}

/// The last part of a section's shape: **what the engine has to say about the
/// here and now**, under a hairline.
///
/// Set apart from the controls above it because it is a different kind of
/// sentence: everything above is a decision the listener is making, and this is
/// the machine reporting what that decision came to for the track playing right
/// now. Without the rule the two read as one list and the readout looks like
/// another setting with its control missing.
///
/// A hairline is the whole of the separation — no surface step, no card. This
/// is a fourth structural rule beyond the three
/// `.interface-design/system.md` §2 names, and it earns the place the same way
/// they do: it divides two kinds of content inside one column, which is exactly
/// what the inspector's rule against the shelf does.
fn readout_block(lines: Vec<(String, iced::Color)>) -> Element<'static, Message> {
    let room = theme::active();
    let mut block =
        column![horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall))]
            .spacing(theme::GAP_SM);
    let mut readings = column![].spacing(theme::GAP_XXS);
    for (line, ink) in lines {
        readings = readings.push(
            text(line)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(ink),
        );
    }
    block = block.push(readings);
    block.into()
}

/// The mode control: the same quiet segmented control the album panel's
/// edition selector uses.
///
/// Reused rather than invented, because it is the same question — *which one
/// of these few* — and the room should answer it the same way twice. The
/// order is [`MODES`]', which is Off first: it is the default and the one that
/// changes nothing.
fn mode_selector(state: replaygain::ReplayGain, live: bool) -> Element<'static, Message> {
    let room = theme::active();
    let mut segments = row![].spacing(theme::GAP_XXS);
    for mode in MODES {
        let selected = state.mode() == mode;
        segments = segments.push(
            button(
                container(
                    text(replaygain::mode_label(mode))
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .font(theme::MEDIUM)
                        .wrapping(text::Wrapping::None),
                )
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
            .style(move |_theme, status| theme::segment(room, status, selected))
            .on_press_maybe(live.then_some(Message::ReplayGainMode(mode))),
        );
    }
    container(segments)
        .width(Length::Fill)
        .padding(theme::SEGMENT_INSET)
        .style(move |_theme| theme::segmented(room))
        .into()
}

/// One numeric setting: its name, its value, and a `−`/`+` pair.
///
/// The value sits in a [`theme::SETTING_VALUE_W`] slot, so a repeated press
/// cannot move the button under the pointer holding it — the same fixed-slot
/// rule the bottom bar is built on, and it holds in a proportional face because
/// Plex Sans's figures are tabular. A stepper at the end of its travel renders
/// disabled rather than absorbing the press.
fn stepper_row(
    label: &'static str,
    value: String,
    can_decrease: bool,
    can_increase: bool,
    decrease: Message,
    increase: Message,
) -> Element<'static, Message> {
    let room = theme::active();
    container(
        row![
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None),
            Space::with_width(Length::Fill),
            container(
                text(value)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper)
                    .wrapping(text::Wrapping::None)
            )
            .width(Length::Fixed(theme::SETTING_VALUE_W))
            .align_x(alignment::Horizontal::Right),
            stepper("\u{2212}", can_decrease, decrease),
            stepper("+", can_increase, increase),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    // One row pitch, and it is the product's one control height: the row is
    // `TRANSPORT_HIT` tall around a `STEPPER_HIT` pair, so two stepper rows are
    // 32 apart on the 4 px lattice rather than 24 apart on nothing.
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .align_y(alignment::Vertical::Center)
    .into()
}

/// One `−` or `+` button: the transport's quiet card in a smaller square.
///
/// A minus sign (U+2212), not a hyphen: it is the character that matches the
/// `+` in width and in height, and these two sit side by side.
fn stepper(glyph: &'static str, enabled: bool, message: Message) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text(glyph)
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .color(if enabled {
                    room.paper
                } else {
                    room.paper_muted
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
    .style(move |_theme, status| theme::transport(room, room.wall, status))
    .on_press_maybe(enabled.then_some(message))
    .into()
}
