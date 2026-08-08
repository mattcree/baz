//! The persistent now-playing bar: current track, transport, seek row,
//! volume — and, now, the door to what is next.
//!
//! # The left zone gained a door
//!
//! The audit's finding was that there was no route to "what is next" from the
//! transport, which is where a listener looks for it: the only door was a
//! toggle in the *top* bar, two hundred pixels from the thing it described. So
//! the bar now carries an **Queue** control beside the track title, with a
//! `3 / 12` readout in it that answers the question outright for the listener
//! who does not want to open anything at all.
//!
//! It is a **labelled** control rather than a gesture on the now-playing block,
//! and that is the one place this module departs from the design spec as
//! written — on evidence the spec did not have. See [`queue_button`].
//!
//! Every addition is a **reserved slot**, which is the promise this module is
//! built on: [`theme::UP_NEXT_W`] and [`theme::POSITION_W`] are that wide
//! whether or not anything is playing, and [`theme::now_playing`]'s border is
//! 1 px in all four states so that finding the control with the pointer does
//! not move the title under it. The bar gained a route to a new surface and did
//! not move a pixel.

use iced::widget::{
    Space, button, column, container, horizontal_rule, image as iced_image, row, text, tooltip,
};
use iced::{Color, Element, Length, alignment};

use crate::app::Message;
use crate::player::PlayerState;
use crate::{groove, icon, player, theme};

/// The persistent now-playing bar, in three zones: the current track on the
/// left, the transport centred over its seek bar in the middle, quiet status
/// and the volume on the right.
///
/// The transport sits *above* the groove rather than beside it because that
/// is where a listener looks for it — the controls and the position they act
/// on read as one block, and the block is the only thing in the bar that is
/// centred. The two flanking zones are equal-weight fills, which is what
/// keeps the centre column optically centred no matter how long a track
/// title runs; both clip rather than push.
///
/// The **volume goes at the far right**, with the signal-path readout
/// immediately to its left. That is where a listener reaches for it, and the
/// adjacency is the point: the fader is the one control on screen that can
/// take the path out of bit-exactness, and the note that says whether it is
/// bit-exact sits next to it. Reading them together needs no explanation and
/// no icon.
///
/// Nothing in here changes size as playback moves. The centre column is
/// [`theme::SEEK_ROW_W`] wide with fixed-width timestamps, the seek row's
/// height is reserved even when there is nothing to seek, the signal-path
/// slot is [`theme::SIGNAL_W`] wide whether or not it says anything, the
/// volume block is [`theme::VOLUME_BLOCK_W`] wide in every state, and every
/// glyph lives in a fixed box — so starting a track, crossing the hour mark,
/// sending a command, moving the fader, muting, or a device that cannot
/// follow the music cannot reflow the bar. The right-hand zone is aligned to
/// its right edge as well, so even the rarely-seen skipped-tracks note grows
/// leftward into the gutter instead of shifting anything beside it. Every
/// glyph, position, and enabled-state comes from [`PlayerState`] —
/// event-derived, tested in `player.rs`.
pub(crate) fn view(player: &PlayerState, queue_open: bool) -> Element<'_, Message> {
    let room = theme::active();
    let mut status = row![]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center);
    if let Some(skipped) = player.skipped_note() {
        status = status.push(
            text(skipped)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        );
    }
    status = status.push(signal_path(player)).push(volume(player));
    let bar = row![
        container(now_playing_block(player, queue_open))
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
        horizontal_rule(1).style(move |_theme| theme::hairline(room)),
        container(bar)
            .width(Length::Fill)
            .padding(theme::pad(theme::GAP_MD, theme::GAP_LG))
            .style(move |_theme| theme::bar(room)),
    ]
    .into()
}

/// The bar's left zone: the now-playing lines, and the **Queue** control
/// beside them.
///
/// This is the *place* a listener looks for what is coming, which is the whole
/// argument for moving the queue's door here from the top bar.
///
/// **The now-playing text itself is deliberately not the control.** The design
/// spec offered the whole block as a press target; the prior-art study then
/// found that the most-supported affordance in the field — *get back to what is
/// playing*, which scrolls the shelf to the sounding album — is the gesture
/// every other product spends a click on the now-playing block for
/// (`docs/design/03-interface-prior-art.md` R3). Two surfaces wanted one
/// target, so the popover takes the labelled control beside the text and the
/// text is left free for the one that has no other home. Resolved on purpose
/// rather than by whichever landed first.
fn now_playing_block(player: &PlayerState, open: bool) -> Element<'_, Message> {
    row![
        container(now_playing_line(player))
            .width(Length::Fill)
            .clip(true),
        queue_button(player, open),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .into()
}

/// The **Queue** control: the word, the `3 / 12` readout, and the press that
/// opens the popover.
///
/// Three properties, each of them load-bearing:
///
/// - **It is labelled, and it is always there.** Not a gesture, not a bare
///   figure, not an icon — the word says what the press opens. The study behind
///   this (`docs/design/03-interface-prior-art.md` §5.3(1)) is unambiguous: an
///   unlabelled route to a transient surface produces users who cannot tell
///   what they just did. It is offered with nothing queued too, because the
///   popover has an honest empty state and a control that came and went with
///   the music would be a moving target in the one row that does not move.
/// - **The readout is a reserved slot.** [`theme::POSITION_W`] wide whether or
///   not there is a position to report, so a queue starting moves no title;
///   and `None` rather than `0 / 12`, because a queue that has not started has
///   no position in it.
/// - **The whole control is a reserved slot too** ([`theme::UP_NEXT_W`]), and
///   [`theme::now_playing`] varies only colour, never geometry — so hovering it
///   and opening the popover both leave every pixel where it was. The lit state
///   is the anchor the toolkit will not let the popover draw as a notch.
///
/// It is the same message <kbd>Q</kbd> sends.
fn queue_button(player: &PlayerState, open: bool) -> Element<'_, Message> {
    let room = theme::active();
    let readout: Element<'_, Message> = match player.queue_position_note() {
        None => Space::with_width(Length::Fixed(theme::POSITION_W)).into(),
        Some(note) => container(
            text(note)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        )
        .width(Length::Fixed(theme::POSITION_W))
        .align_x(alignment::Horizontal::Right)
        .into(),
    };
    button(
        row![
            text("Queue")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
            readout,
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fixed(theme::UP_NEXT_W))
    .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
    .style(move |_theme, status| theme::now_playing(room, status, open))
    .on_press(Message::ToggleQueue)
    .into()
}

/// The now-playing lines proper: the current track as a title-over-artist
/// stack, or the engine's plainly-stated absence as quiet status text.
///
/// Neither line wraps. A bar that grew a second row under a long album title
/// would shove the shelf up by a line, which is exactly the kind of movement
/// this bar is built not to make; the enclosing zone clips instead.
fn now_playing_line(player: &PlayerState) -> Element<'_, Message> {
    let room = theme::active();
    if let Some(note) = player.availability_note() {
        return text(note)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None)
            .into();
    }
    let Some(now) = player.now_playing() else {
        return text("Nothing playing")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None)
            .into();
    };
    let mut stack = column![
        text(now.title.as_str())
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .font(theme::MEDIUM)
            .wrapping(text::Wrapping::None)
    ]
    .spacing(theme::GAP_XXS);
    if let Some(artist) = &now.artist {
        stack = stack.push(
            text(artist.as_str())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None),
        );
    }
    stack.into()
}

/// The signal path, in the quietest terms the room has: a short
/// `48 → 44.1 kHz` or `bit-perfect` in the same faint ink as the track
/// durations and the counts, with one plain sentence on hover.
///
/// Drawn **only** when [`PlayerState::signal_note`] answers — the engine is
/// converting, or the whole path is transparent (ADR-0009 §5, as ADR-0011
/// amends it). The in-between case, a direct chain with the volume scaling
/// the samples, puts nothing here: that fact is already legible in the fader
/// two controls to the right, which is visibly not at the top.
///
/// Everything about the treatment is chosen to be ignorable, and the
/// affirmative reading gets exactly the same treatment as the converting one
/// so that neither can read as the other's verdict. No lamp amber: the accent
/// means playback truth, and what the chain is doing is not a claim about the
/// music. No icon, no rule, no background — the label is the same weight as
/// the "3 tracks skipped" note it sits beside. And the slot is
/// [`theme::SIGNAL_W`] wide in every case, so the note *appearing* moves
/// nothing: a listener who is not looking for it will never see it arrive.
fn signal_path(player: &PlayerState) -> Element<'_, Message> {
    let room = theme::active();
    let Some(note) = player.signal_note() else {
        return Space::with_width(Length::Fixed(theme::SIGNAL_W)).into();
    };
    let label = container(
        text(note.label)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    )
    .width(Length::Fixed(theme::SIGNAL_W))
    .align_x(alignment::Horizontal::Right);
    tooltip(
        label,
        text(note.detail)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Top,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
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
    // Previous, play/pause, Next — in that order and symmetric about the
    // toggle, which is where every listener's hand already expects to find
    // them. `|◀` was the most-missed control in the app: the engine command
    // and its restart-versus-step-back rule were both already specified, and
    // there was no button, no key and no MPRIS flag to reach them by.
    //
    // Three fixed [`theme::TRANSPORT_HIT`] squares in a fixed
    // [`theme::SEEK_ROW_W`] column, so the row gained a control and moved
    // nothing: the block is still centred on the same pixel, and the seek bar
    // under it did not shift by one.
    let transport = row![
        glyph_button(
            icon::Glyph::Previous,
            "Previous track",
            player.previous_enabled(),
            pending,
            Message::PreviousTrack,
        ),
        glyph_button(
            toggle.into(),
            toggle.label(),
            player.play_pause_enabled(),
            pending,
            Message::PlayPause,
        ),
        glyph_button(
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

/// One icon-only control: a glyph in a fixed square, named by a tooltip.
///
/// The size is fixed in both axes and the glyph is drawn into a box of its
/// own, so swapping play for pause moves nothing. `pending` reaches the ink
/// and only the ink (see [`theme::glyph_opacity`]).
///
/// The tooltip is the control's accessible name. iced 0.13 publishes no
/// accessibility tree and its buttons take no keyboard focus, so a hover
/// label plus a target comfortably larger than the mark is the whole of what
/// the toolkit can offer here — stated plainly rather than papered over.
fn glyph_button(
    glyph: icon::Glyph,
    label: &str,
    enabled: bool,
    pending: bool,
    message: Message,
) -> Element<'_, Message> {
    let room = theme::active();
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
        .style(move |_theme, status| theme::transport(room, status))
        .on_press_maybe(enabled.then_some(message));
    tooltip(
        control,
        text(label)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Top,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// The seek bar: elapsed timestamp, groove, total timestamp — a row that
/// reads left to right the way the track plays, with a lane above the groove
/// where the hover preview floats. The timestamps' digits are tabular — a
/// property of the bundled Sans rather than of a second face — and each sits in
/// a fixed [`theme::STAMP_W`] slot, so they cannot shuffle the groove sideways
/// as they tick.
///
/// The rail is [`groove::Groove`] rather than iced's `slider`: it reports
/// pointer *geometry*, which is what the click-vs-scrub threshold, the hover
/// preview, and the cursor affordance are all built from (that module's docs
/// carry the evidence for why the built-ins cannot). The volume fader below
/// is the same widget for the same reasons.
///
/// A track whose length was never declared gets the inert groove: the
/// elapsed time still counts up (that much is known), but there is nothing
/// to scrub against and the widget says so by refusing the pointer — and by
/// leaving the cursor alone — rather than by looking identical and doing
/// nothing.
fn seek_bar(state: player::SeekBar) -> Element<'static, Message> {
    let room = theme::active();
    // While a position is being asked for rather than reported, the elapsed
    // timestamp warms to lamp amber — the same accent the rest of the room
    // reserves for playback truth, here saying "this is where you are asking
    // to be". It cools back to the quiet default the moment the engine
    // confirms.
    let elapsed_color = if state.pending {
        room.lamp
    } else {
        room.paper_faint
    };
    let rail: Element<'static, Message> = if state.interactive {
        groove::Groove::new(state.position, room, theme::seek)
            .width(Length::Fixed(theme::SEEK_W))
            .height(theme::RAIL_HIT)
            .on_pointer(
                Message::SeekPressed,
                Message::SeekDragged,
                Message::SeekHovered,
                Message::SeekReleased,
                Message::SeekLeft,
            )
            .into()
    } else {
        groove::Groove::new(state.position, room, theme::seek_inert)
            .width(Length::Fixed(theme::SEEK_W))
            .height(theme::RAIL_HIT)
            .into()
    };
    row![
        seek_stamp(state.elapsed, elapsed_color, alignment::Horizontal::Right),
        column![
            preview_lane(state.preview, theme::SEEK_W, theme::PREVIEW_W),
            rail
        ],
        seek_stamp(state.total, room.paper_faint, alignment::Horizontal::Left),
    ]
    .spacing(theme::GAP_SM)
    .into()
}

/// The volume control: a mute affordance and a fader, with a lane above the
/// fader where the level preview floats.
///
/// The fader is the same [`groove::Groove`] as the seek bar — the same
/// cursor affordance, the same hover preview, the same 4 px click-vs-drag
/// threshold — plus the one thing a fader needs and a scrub bar does not: a
/// **unity detent**, drawn as a small mark above the rail at the top of the
/// travel. It is faint until the handle is on it and full paper when it is,
/// which is what makes "at unity" and "a pixel below unity" different on
/// sight rather than only in the readout. The other half of reaching it is
/// [`player::UNITY_SNAP_PX`], where the hand's aim is resolved.
///
/// Not lamp amber, and the knob does not grow: the reasons are in
/// [`theme::volume`]. Muting swaps the glyph and the fader's ink and moves
/// nothing at all — the block is [`theme::VOLUME_BLOCK_W`] × the sum of its
/// reserved lanes in every state this control has.
fn volume(player: &PlayerState) -> Element<'_, Message> {
    let room = theme::active();
    let state = player.volume_bar();
    let detent = groove::Detent {
        at: 1.0,
        engaged: state.unity && !state.muted,
    };
    let style = match (state.interactive, state.muted) {
        (false, _) => theme::volume_inert,
        (true, true) => theme::volume_muted,
        (true, false) => theme::volume,
    };
    let fader = groove::Groove::new(state.position, room, style)
        .width(Length::Fixed(theme::VOLUME_W))
        .height(theme::VOLUME_HIT)
        .detent(detent);
    let fader: Element<'_, Message> = if state.interactive {
        fader
            .on_pointer(
                Message::VolumePressed,
                Message::VolumeDragged,
                Message::VolumeHovered,
                Message::VolumeReleased,
                Message::VolumeLeft,
            )
            .into()
    } else {
        fader.into()
    };
    row![
        // **The mute glyph sits on the fader's rail**, not on the centre of the
        // block the fader is in — the one alignment defect a listener named
        // unprompted, and it was 7.5 px. The button is *placed* rather than
        // centred: [`theme::MUTE_TOP`] above it, which is half a hit target
        // above the rail. The argument and the measurement are on that token;
        // `seek_stamp` below makes the same move for the seek groove's
        // timestamps, which is where the pattern comes from.
        column![
            Space::with_height(Length::Fixed(theme::MUTE_TOP)),
            glyph_button(
                icon::Glyph::speaker(state.muted),
                state.mute_label,
                state.interactive,
                state.mute_pending,
                Message::ToggleMute,
            ),
        ],
        column![
            preview_lane(state.preview, theme::VOLUME_W, theme::LEVEL_W),
            fader
        ],
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Start)
    .width(Length::Fixed(theme::VOLUME_BLOCK_W))
    .height(Length::Fixed(theme::VOLUME_ROW_H))
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
                .line_height(theme::LEADING_META)
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

/// The lane above a groove where its hover preview floats: a fixed-height
/// strip `width` wide, empty until the pointer rests on the bar, then
/// carrying a `tip_width` tip centered on the pointer with what a click
/// there would ask for — a timestamp over the seek bar, a level over the
/// fader.
///
/// The strip is reserved whether or not anything is hovering, so the bottom
/// bar never changes height under the pointer; the horizontal placement is
/// [`player::preview_offset`], which keeps the tip whole and on the bar at
/// both ends (pure, and tested there).
fn preview_lane(
    preview: Option<player::Preview>,
    width: f32,
    tip_width: f32,
) -> Element<'static, Message> {
    let room = theme::active();
    let mut lane = row![];
    if let Some(preview) = preview {
        let offset = player::preview_offset(&preview, tip_width);
        lane = lane.push(Space::with_width(Length::Fixed(offset))).push(
            container(
                text(preview.label)
                    .size(theme::SIZE_CAPTION)
                    .line_height(theme::LEADING_CAPTION),
            )
            .width(Length::Fixed(tip_width))
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(move |_theme| theme::preview_tip(room)),
        );
    }
    container(lane)
        .width(Length::Fixed(width))
        .height(Length::Fixed(theme::PREVIEW_H))
        .into()
}

#[cfg(test)]
mod tests {
    use crate::player::{Availability, Phase, PlayPause, PlayerState};
    use crate::{icon, theme};

    use baz_core::protocol::Event;

    /// The bar's reserved-slot rule, re-checked for the row that just gained a
    /// third control.
    ///
    /// `theme.rs` already pins the pair (`2 × TRANSPORT_HIT + GAP_SM`); this
    /// is the same claim for the three the transport now draws, asserted here
    /// rather than there so the module that composes the row owns the check
    /// for what it composes. The row is what makes the bar's centre column
    /// stable, and a Previous button that did not fit would push the seek bar
    /// out from under it.
    #[test]
    fn the_transport_row_still_fits_the_column_it_centres_in() {
        // Three fixed squares and the two gaps between them.
        const TRANSPORT_ROW_W: f32 = 3.0 * theme::TRANSPORT_HIT + 2.0 * theme::GAP_SM;
        const { assert!(TRANSPORT_ROW_W < theme::SEEK_ROW_W) }
        // With room left over on *both* sides, or the row would be centred by
        // its own edges rather than within the column — the seek bar below it
        // is only `SEEK_W` wide inside the same column, and the two blocks
        // have to share a centre line.
        const { assert!(TRANSPORT_ROW_W < theme::SEEK_W) }
        // And the whole bar still fits the narrowest shipped window beside its
        // two flanking zones.
        const { assert!(theme::SEEK_ROW_W + theme::VOLUME_BLOCK_W + theme::SIGNAL_W < 760.0) }
    }

    /// Every glyph the transport row can draw is the same sprite square in the
    /// same fixed box, Previous included — so no transport state moves a pixel
    /// of the bar.
    #[test]
    fn every_transport_glyph_occupies_the_same_box() {
        let glyphs = [
            icon::Glyph::from(PlayPause::Play),
            icon::Glyph::from(PlayPause::Pause),
            icon::Glyph::Previous,
            icon::Glyph::Next,
            icon::Glyph::speaker(false),
            icon::Glyph::speaker(true),
        ];
        // One stable handle each (the sheet is rasterized once), and the view
        // draws every one of them into an `ICON_PX` box inside a
        // `TRANSPORT_HIT` button — so the only thing that varies between
        // states is which sprite is sampled.
        for glyph in glyphs {
            assert_eq!(icon::handle(glyph).id(), icon::handle(glyph).id());
        }
        const { assert!(theme::TRANSPORT_HIT > theme::ICON_PX) }
        // Previous is genuinely a distinct sprite, so this is not vacuous.
        assert_ne!(
            icon::handle(icon::Glyph::Previous).id(),
            icon::handle(icon::Glyph::Next).id()
        );
    }

    /// **The bar reserves every slot it can be in** — re-checked for the zone
    /// that just became a control.
    ///
    /// One of the four properties `docs/design/01-ux-audit-and-ia.md` §5 says
    /// must not regress. The left zone gained a labelled control carrying a
    /// readout that comes and goes with the music. Both are reservations rather
    /// than additions — the control is [`theme::UP_NEXT_W`] and the readout
    /// inside it [`theme::POSITION_W`], whether either says anything or not,
    /// and the control's border is present in every state — so the bar carries
    /// a route to a whole new surface and still cannot move.
    #[test]
    fn the_left_zone_reserves_the_queue_position_in_every_state_it_has() {
        use baz_core::protocol::Event;

        // The zone's own budget at the shipped window: the readout, the gap to
        // it, and the button's horizontal padding all come out of the fill
        // zone, and what is left has to be a real title lane. (The *narrow*
        // window is a different question, and a known one — §1.5 of the audit
        // caught the left zone wrapping to three lines below ~900 px, and the
        // fix is a maximum width on the zone, which is step 10 of the plan.
        // Nothing here makes that worse: the readout is 72 px in a zone that
        // already clips.)
        const SHIPPED: f32 = 1280.0;
        const ZONE: f32 = SHIPPED
            - 2.0 * theme::GAP_LG // the bar's own padding
            - 2.0 * theme::GAP_LG // the gaps between its three zones
            - theme::SEEK_ROW_W
            - theme::SIGNAL_W
            - theme::GAP_SM
            - theme::VOLUME_BLOCK_W;
        const TITLE_LANE: f32 = ZONE - theme::UP_NEXT_W - theme::GAP_SM;
        // The zone is also shorter than the centre column, so the control's
        // padding cannot be what sets the bar's height.
        const LEFT_H: f32 = theme::SIZE_BODY * theme::LEADING_BODY
            + theme::GAP_XXS
            + theme::SIZE_META * theme::LEADING_META
            + 2.0 * theme::GAP_XS;
        const CENTRE_H: f32 = theme::TRANSPORT_HIT + theme::GAP_SM + theme::SEEK_ROW_H;
        const { assert!(TITLE_LANE > 200.0) }
        const { assert!(LEFT_H < CENTRE_H) }

        let mut player = PlayerState::new(Availability::Ready);
        // Nothing playing: no readout, and the slot is still that wide.
        assert_eq!(player.queue_position_note(), None);

        player.apply(
            &Event::TrackStarted {
                path: std::path::PathBuf::from("/music/a/01.flac"),
                position: 0,
            },
            &[],
        );
        // Without a recorded queue there is still nothing to be a position in;
        // the front end never invents one (see `player.rs`'s honesty rule).
        assert_eq!(player.queue_position_note(), None);
    }

    /// Previous is offered exactly when it can act, and its enabled-ness is a
    /// property of the transport rather than of the queue's length — the
    /// difference from Next that ADR-0014's protocol notes call out.
    #[test]
    fn previous_is_live_whenever_a_run_is() {
        let mut player = PlayerState::new(Availability::Ready);
        // Stopped: a relative command has nothing to be relative to.
        assert_eq!(player.phase(), Phase::Stopped);
        assert!(!player.previous_enabled());

        player.apply(
            &Event::TrackStarted {
                path: std::path::PathBuf::from("/music/a/01.flac"),
                position: 0,
            },
            &[],
        );
        assert!(
            player.previous_enabled(),
            "a running queue can always go back"
        );
        // At the head of the queue too: `Previous` restarts the track there
        // rather than declining, so there is no position at which it is dead.
        assert!(player.previous_enabled());

        player.apply(&Event::Paused, &[]);
        assert!(
            player.previous_enabled(),
            "paused moves and resumes, so the control is live"
        );

        player.apply(&Event::QueueEnded, &[]);
        assert!(!player.previous_enabled());

        // No engine at all: nothing in the transport is offered.
        let dead = PlayerState::new(Availability::NoDevice("no device".to_owned()));
        assert!(!dead.previous_enabled());
    }
}
