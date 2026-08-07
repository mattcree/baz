//! The persistent now-playing bar: current track, transport, seek row.

use iced::widget::{
    Space, button, column, container, horizontal_rule, image as iced_image, row, text, tooltip,
};
use iced::{Color, Element, Length, alignment};

use crate::app::Message;
use crate::player::PlayerState;
use crate::{icon, player, seek, theme};

/// The persistent now-playing bar, in three zones: the current track on the
/// left, the transport centred over its seek bar in the middle, quiet status
/// on the right.
///
/// The transport sits *above* the groove rather than beside it because that
/// is where a listener looks for it — the controls and the position they act
/// on read as one block, and the block is the only thing in the bar that is
/// centred. The two flanking zones are equal-weight fills, which is what
/// keeps the centre column optically centred no matter how long a track
/// title runs; both clip rather than push.
///
/// Nothing in here changes size as playback moves. The centre column is
/// [`theme::SEEK_ROW_W`] wide with fixed-width timestamps, the seek row's
/// height is reserved even when there is nothing to seek, the signal-path
/// slot is [`theme::SIGNAL_W`] wide whether or not it says anything, and the
/// transport glyphs live in fixed boxes — so starting a track, crossing the
/// hour mark, sending a command, or meeting a device that cannot follow the
/// music cannot reflow the bar. Every glyph, position, and enabled-state
/// comes from [`PlayerState`] — event-derived, tested in `player.rs`.
pub(crate) fn view(player: &PlayerState) -> Element<'_, Message> {
    let mut status = row![].spacing(theme::GAP_SM);
    if let Some(skipped) = player.skipped_note() {
        status = status.push(
            text(skipped)
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(theme::PAPER_FAINT),
        );
    }
    status = status.push(signal_path(player));
    let bar = row![
        container(now_playing_line(player))
            .width(Length::Fill)
            .clip(true),
        transport_stack(player),
        container(status)
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Right)
            .clip(true),
    ]
    .spacing(theme::GAP_LG)
    .align_y(iced::Alignment::Center);
    column![
        horizontal_rule(1).style(theme::hairline),
        container(bar)
            .width(Length::Fill)
            .padding(theme::pad(theme::GAP_MD, theme::GAP_LG))
            .style(theme::bar),
    ]
    .into()
}

/// The bar's left zone: the current track as a title-over-artist stack, or
/// the engine's plainly-stated absence as quiet status text.
///
/// Neither line wraps. A bar that grew a second row under a long album title
/// would shove the shelf up by a line, which is exactly the kind of movement
/// this bar is built not to make; the enclosing zone clips instead.
fn now_playing_line(player: &PlayerState) -> Element<'_, Message> {
    if let Some(note) = player.availability_note() {
        return text(note)
            .size(theme::SIZE_META)
            .color(theme::PAPER_FAINT)
            .wrapping(text::Wrapping::None)
            .into();
    }
    let Some(now) = player.now_playing() else {
        return text("Nothing playing")
            .size(theme::SIZE_META)
            .color(theme::PAPER_FAINT)
            .wrapping(text::Wrapping::None)
            .into();
    };
    let mut stack = column![
        text(now.title.as_str())
            .size(theme::SIZE_BODY)
            .font(theme::MEDIUM)
            .wrapping(text::Wrapping::None)
    ]
    .spacing(theme::GAP_XXS);
    if let Some(artist) = &now.artist {
        stack = stack.push(
            text(artist.as_str())
                .size(theme::SIZE_META)
                .color(theme::PAPER_DIM)
                .wrapping(text::Wrapping::None),
        );
    }
    stack.into()
}

/// The signal path, in the quietest terms the room has: a short monospace
/// `48 → 44.1 kHz` in the same faint ink as the track durations and the
/// counts, with one plain sentence on hover.
///
/// Drawn **only** when [`PlayerState::signal_note`] answers — that is, only
/// while the engine is converting (ADR-0009 §5). The direct case, which is
/// the ordinary one, puts nothing here at all.
///
/// Everything about the treatment is chosen to be ignorable. No lamp amber:
/// the accent means playback truth, and a rate the device happens to be
/// running at is not a claim about the music. No icon, no rule, no
/// background — the label is the same weight as the "3 tracks skipped" note
/// it sits beside. And the slot is [`theme::SIGNAL_W`] wide either way, so
/// the note *appearing* moves nothing: a listener who is not looking for it
/// will never see it arrive.
fn signal_path(player: &PlayerState) -> Element<'_, Message> {
    let Some(note) = player.signal_note() else {
        return Space::with_width(Length::Fixed(theme::SIGNAL_W)).into();
    };
    let label = container(
        text(note.label)
            .size(theme::SIZE_META)
            .font(theme::MONO)
            .color(theme::PAPER_FAINT)
            .wrapping(text::Wrapping::None),
    )
    .width(Length::Fixed(theme::SIGNAL_W))
    .align_x(alignment::Horizontal::Right);
    tooltip(
        label,
        text(note.detail).size(theme::SIZE_CAPTION),
        tooltip::Position::Top,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(theme::tooltip)
    .into()
}

/// The bar's centre: the transport row over the seek row, both centred in a
/// fixed-width column.
///
/// When there is nothing to seek — no engine, or nothing playing — the seek
/// row's space is *reserved* rather than dropped. The transport is the one
/// thing in this bar that is always in the same place, and a bar that
/// changed height the moment a track started would undo that.
fn transport_stack(player: &PlayerState) -> Element<'_, Message> {
    let pending = player.transport_pending();
    let toggle = player.play_pause();
    let transport = row![
        transport_button(
            toggle.into(),
            toggle.label(),
            player.play_pause_enabled(),
            pending,
            Message::PlayPause,
        ),
        transport_button(
            icon::Glyph::Next,
            "Next track",
            player.next_enabled(),
            pending,
            Message::NextTrack,
        ),
    ]
    .spacing(theme::GAP_SM);
    let seek: Element<'_, Message> = match player.seek_bar() {
        Some(state) => seek_bar(state),
        None => Space::new(
            Length::Fixed(theme::SEEK_ROW_W),
            Length::Fixed(theme::SEEK_ROW_H),
        )
        .into(),
    };
    column![transport, seek]
        .spacing(theme::GAP_SM)
        .width(Length::Fixed(theme::SEEK_ROW_W))
        .align_x(iced::Alignment::Center)
        .into()
}

/// One transport control: a glyph in a fixed square, named by a tooltip.
///
/// The size is fixed in both axes and the glyph is drawn into a box of its
/// own, so swapping play for pause moves nothing. `pending` reaches the ink
/// and only the ink (see [`theme::glyph_opacity`]).
///
/// The tooltip is the control's accessible name. iced 0.13 publishes no
/// accessibility tree and its buttons take no keyboard focus, so a hover
/// label plus a target comfortably larger than the mark is the whole of what
/// the toolkit can offer here — stated plainly rather than papered over.
fn transport_button(
    glyph: icon::Glyph,
    label: &str,
    enabled: bool,
    pending: bool,
    message: Message,
) -> Element<'_, Message> {
    let mark = container(
        iced_image(icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::glyph_opacity(enabled, pending)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let control = button(mark)
        .width(Length::Fixed(theme::TRANSPORT_HIT))
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(0)
        .style(theme::transport)
        .on_press_maybe(enabled.then_some(message));
    tooltip(
        control,
        text(label).size(theme::SIZE_CAPTION),
        tooltip::Position::Top,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(theme::tooltip)
    .into()
}

/// The seek bar: elapsed timestamp, groove, total timestamp — a row that
/// reads left to right the way the track plays, with a lane above the groove
/// where the hover preview floats. Timestamps are monospace so the digits do
/// not shuffle the groove sideways as they tick.
///
/// The groove is [`seek::Groove`] rather than iced's `slider`: it reports
/// pointer *geometry*, which is what the click-vs-scrub threshold, the hover
/// preview, and the cursor affordance are all built from (that module's docs
/// carry the evidence for why the built-ins cannot).
///
/// A track whose length was never declared gets the inert groove: the
/// elapsed time still counts up (that much is known), but there is nothing
/// to scrub against and the widget says so by refusing the pointer — and by
/// leaving the cursor alone — rather than by looking identical and doing
/// nothing.
fn seek_bar(state: player::SeekBar) -> Element<'static, Message> {
    // While a position is being asked for rather than reported, the elapsed
    // timestamp warms to lamp amber — the same accent the rest of the room
    // reserves for playback truth, here saying "this is where you are asking
    // to be". It cools back to the quiet default the moment the engine
    // confirms.
    let elapsed_color = if state.pending {
        theme::LAMP
    } else {
        theme::PAPER_FAINT
    };
    let groove = seek::Groove::new(state.position, theme::seek)
        .width(Length::Fixed(theme::SEEK_W))
        .height(theme::RAIL_HIT);
    let groove: Element<'static, Message> = if state.interactive {
        groove
            .on_pointer(
                Message::SeekPressed,
                Message::SeekDragged,
                Message::SeekHovered,
                Message::SeekReleased,
                Message::SeekLeft,
            )
            .into()
    } else {
        seek::Groove::new(state.position, theme::seek_inert)
            .width(Length::Fixed(theme::SEEK_W))
            .height(theme::RAIL_HIT)
            .into()
    };
    row![
        seek_stamp(state.elapsed, elapsed_color, alignment::Horizontal::Right),
        column![preview_lane(state.preview), groove],
        seek_stamp(state.total, theme::PAPER_FAINT, alignment::Horizontal::Left),
    ]
    .spacing(theme::GAP_SM)
    .into()
}

/// One of the seek bar's timestamps, carrying the same preview lane as the
/// groove above it so that the digits line up with the rail rather than with
/// the lane-plus-rail block.
///
/// The stamp is [`theme::STAMP_W`] wide whatever it says, hugging the groove
/// it belongs to. Sizing it to its own digits would slide the groove
/// sideways the moment a track crossed the hour — and, since the whole
/// centre column is what the transport centres over, would drag the buttons
/// with it.
fn seek_stamp(
    value: String,
    color: Color,
    align: alignment::Horizontal,
) -> Element<'static, Message> {
    column![
        Space::with_height(Length::Fixed(theme::PREVIEW_H)),
        container(
            text(value)
                .size(theme::SIZE_META)
                .font(theme::MONO)
                .color(color)
                .wrapping(text::Wrapping::None)
        )
        .width(Length::Fixed(theme::STAMP_W))
        .height(Length::Fixed(theme::RAIL_HIT))
        .align_x(align)
        .align_y(alignment::Vertical::Center),
    ]
    .into()
}

/// The lane above the groove where the hover preview floats: a fixed-height
/// strip, empty until the pointer rests on the bar, then carrying a small
/// tip centered on the pointer with the timestamp a click would seek to.
///
/// The strip is reserved whether or not anything is hovering, so the bottom
/// bar never changes height under the pointer; the horizontal placement is
/// [`player::preview_offset`], which keeps the tip whole and on the bar at
/// both ends (pure, and tested there).
fn preview_lane(preview: Option<player::SeekPreview>) -> Element<'static, Message> {
    let mut lane = row![];
    if let Some(preview) = preview {
        let offset = player::preview_offset(&preview, theme::PREVIEW_W);
        lane = lane.push(Space::with_width(Length::Fixed(offset))).push(
            container(
                text(preview.label)
                    .size(theme::SIZE_CAPTION)
                    .font(theme::MONO),
            )
            .width(Length::Fixed(theme::PREVIEW_W))
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(theme::preview_tip),
        );
    }
    container(lane)
        .width(Length::Fixed(theme::SEEK_W))
        .height(Length::Fixed(theme::PREVIEW_H))
        .into()
}
