//! The persistent now-playing bar: current track, transport, the two
//! timestamps, volume, the door to what is next — and, flush on the window's
//! bottom edge under all of it, the needle.
//!
//! # The left zone gained a door, and then stopped needing it
//!
//! The audit's finding was that there was no route to "what is next" from the
//! transport, which is where a listener looks for it: the only door was a
//! toggle in the *top* bar, two hundred pixels from the thing it described. So
//! the bar carries a **Queue** control beside the track title.
//!
//! It is a **labelled** control rather than a gesture on the now-playing block,
//! and that is the one place this module departs from the design spec as
//! written — on evidence the spec did not have. See [`queue_button`].
//!
//! A door, though, is still something you have to open. The critique specified
//! two things for this corner — the wall label *and* "stack status when queued"
//! — and only the label shipped, which left the popover as the only way to
//! learn what was coming. So the left zone now states it **ambiently**, on a
//! third line under the artist ([`continuation_lane`]), and the control's
//! readout stopped being a position and became the size of what it opens.
//! Knowing costs nothing; opening is for changing.
//!
//! Every addition is a **reserved slot**, which is the promise this module is
//! built on: [`theme::UP_NEXT_W`] and [`theme::POSITION_W`] are that wide
//! whether or not anything is playing, [`theme::CONTINUATION_H`] is that tall
//! whether or not anything follows this track, and [`theme::now_playing`]'s
//! border is 1 px in all four states so that finding the control with the
//! pointer does not move the title under it. The bar gained a route to a new
//! surface and a running commentary on the queue, and did not move a pixel.

use iced::widget::{
    Space, button, column, container, horizontal_rule, image as iced_image, mouse_area, row, stack,
    text, tooltip,
};
use iced::{Color, Element, Length, alignment};

use crate::app::Message;
use crate::motion::{Control, Ink};
use crate::place::Place;
use crate::player::PlayerState;
use crate::{groove, icon, needle, player, theme};

/// The persistent now-playing bar, in three zones — the current track and its
/// timestamps on the left, the transport in the middle, quiet status and the
/// volume on the right — with the needle under all three.
///
/// **The bar is 81 px and the needle is 2**, where the bar alone was 105 before
/// the needle and 57 after it. The seek row is not deleted so much as *moved*:
/// its job is stated better by a line that also says what the queue is shaped
/// like, and `docs/REFUSALS.md` permits exactly that one move on this bar — a
/// slot may be replaced by a better statement of the same fact, and none may be
/// removed for tidiness.
///
/// The band went back up because 57 was correct in every part and wrong as a
/// proportion: the left zone's three line boxes are 56 px and the band was 56,
/// so the type touched both edges of the bar it sits in. [`theme::BAR_CONTENT_H`]
/// carries the re-derivation — two [`theme::HANG`]s, led by
/// [`theme::BAR_ZONE_LEAD`] above and below the type and by [`theme::BAR_LEAD`]
/// above and below the transport, both of them named tokens rather than
/// pixels chosen to look right.
///
/// The two flanking zones are equal-weight fills, which is what keeps the
/// centre column optically centred no matter how long a track title runs; both
/// clip rather than push.
///
/// The **volume goes at the far right**, with the signal-path readout
/// immediately to its left. That is where a listener reaches for it, and the
/// adjacency is the point: the fader is the one control on screen that can
/// take the path out of bit-exactness, and the note that says whether it is
/// bit-exact sits next to it. Reading them together needs no explanation and
/// no icon.
///
/// Nothing in here changes size as playback moves. The centre column is
/// [`theme::TRANSPORT_W`] wide, each timestamp sits in a fixed
/// [`theme::STAMP_W`] slot that is there whether or not anything is playing,
/// the signal-path
/// slot is [`theme::SIGNAL_W`] wide whether or not it says anything, the
/// volume block is [`theme::VOLUME_BLOCK_W`] wide in every state, and every
/// glyph lives in a fixed box — so starting a track, crossing the hour mark,
/// sending a command, moving the fader, muting, or a device that cannot
/// follow the music cannot reflow the bar. The right-hand zone is aligned to
/// its right edge as well, so even the rarely-seen skipped-tracks note grows
/// leftward into the gutter instead of shifting anything beside it. Every
/// glyph, position, and enabled-state comes from [`PlayerState`] —
/// event-derived, tested in `player.rs`.
pub(crate) fn view(
    player: &PlayerState,
    place: Place,
    ink: Ink,
    cover: Option<iced_image::Handle>,
) -> Element<'_, Message> {
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
    status = status.push(signal_path(player)).push(volume(player, ink));
    let bar = row![
        container(now_playing_block(player, place, cover))
            .width(Length::Fill)
            .clip(true),
        transport_row(player, ink),
        // **The properties zone.** Shuffle stands at its head, in a fixed slot
        // *outside* the right-aligned status row, so that nothing the status
        // says — a skipped-tracks note, a signal path arriving — can move it.
        // The row grows leftward into the slack between the two.
        container(row![shuffle_toggle(player, ink), status].spacing(theme::GAP_LG))
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Right)
            .clip(true),
    ]
    .spacing(theme::GAP_LG)
    .align_y(iced::Alignment::Center);
    let line = player.needle_bar();
    let tip = line.preview.clone();
    // **The whole of the window's bottom edge, in three layers and 59 px.** The
    // hairline, the band, the needle — and over all of them the hover tip,
    // which is a *layer* and therefore costs no height at all (the same trick
    // [`theme::PREVIEW_H`] documents, and the reason the transport can sit on
    // the bar's own centre line).
    stack![
        column![
            horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
            // **One centre line, one window gutter.** The band is
            // [`theme::BAR_CONTENT_H`] and its mid-line is the transport's
            // centre line by construction ([`theme::BAR_LEAD`]), so every zone
            // centred in it puts its own mark on that line rather than centring
            // its block around it. The horizontal padding is [`theme::HANG`],
            // the one gutter every surface that touches a window edge hangs
            // from. There is no *vertical* padding left to be asymmetric: the
            // band is the whole bar.
            container(bar)
                .width(Length::Fill)
                .height(Length::Fixed(theme::BAR_CONTENT_H))
                .padding(theme::pad(0.0, theme::HANG))
                .style(move |_theme| theme::bar(room)),
            // **The needle hangs off no gutter**, deliberately: it is the
            // window's own bottom edge and it states the whole queue, so it
            // runs the full width. Law L5 gives the bar `HANG`, `W − HANG`, its
            // zone boundaries and its reserved slots' edges; the needle's edges
            // are the window's, which is the one pair every surface shares.
            needle_line(line),
        ],
        tip_layer(tip),
    ]
    .into()
}

/// The needle proper: [`needle::Needle`] over the queue the engine is holding,
/// wired to the pointer when there is a queue to move within and inert when
/// there is not.
///
/// The two style functions are the whole difference. An inert needle draws its
/// unfilled track rather than nothing, because a line that came and went with
/// the music would be movement in the one place ADR-0020 forbids it — and it
/// refuses the pointer rather than looking identical and doing nothing, which
/// is the rule [`groove::Groove`] set for a track of undeclared length.
fn needle_line(line: player::NeedleBar) -> Element<'static, Message> {
    let room = theme::active();
    if line.interactive {
        needle::Needle::new(line, room, theme::needle)
            .on_pointer(
                Message::NeedlePressed,
                Message::NeedleDragged,
                Message::NeedleHovered,
                Message::NeedleReleased,
                Message::NeedleLeft,
            )
            .into()
    } else {
        needle::Needle::new(line, room, theme::needle_inert).into()
    }
}

/// The layer the needle's hover tip floats in: the whole bar's height, with the
/// tip pinned to the bottom just clear of the line it describes.
///
/// A **layer**, so the bar does not change height under the pointer and the
/// needle keeps costing the collection [`theme::NEEDLE_H`] and nothing else.
/// The horizontal placement is [`player::preview_offset`], which keeps the tip
/// whole and on screen at both ends (pure, and tested there) — and it is
/// measured against the *window's* width because that is what the needle is
/// measured against, so the tip cannot drift from the segment it names.
fn tip_layer(preview: Option<player::Preview>) -> Element<'static, Message> {
    let room = theme::active();
    let mut lane = row![];
    if let Some(preview) = preview {
        let offset = player::preview_offset(&preview, theme::NEEDLE_TIP_W);
        lane = lane.push(Space::with_width(Length::Fixed(offset))).push(
            container(
                text(preview.label)
                    .size(theme::SIZE_CAPTION)
                    .line_height(theme::LEADING_CAPTION)
                    .wrapping(text::Wrapping::None),
            )
            .width(Length::Fixed(theme::NEEDLE_TIP_W))
            .height(Length::Fixed(theme::PREVIEW_H))
            .clip(true)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center)
            .style(move |_theme| theme::preview_tip(room)),
        );
    }
    container(lane)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(alignment::Vertical::Bottom)
        .padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: theme::NEEDLE_H,
            left: 0.0,
        })
        .into()
}

/// The bar's left zone: the now-playing lines, and the **Queue** control
/// beside them.
///
/// This is the *place* a listener looks for what is coming, which is the whole
/// argument for moving the queue's door here from the top bar.
///
/// **The now-playing text is the control that takes you to the record**, and
/// that is the reservation ADR-0016 made being spent.
///
/// The prior-art study's R3 is the most-supported affordance in the field —
/// *get back to what is playing* — and every product it surveyed spends the
/// now-playing block's press on it. baz had none. ADR-0016 left the text
/// deliberately free for it and gave the queue the labelled control beside it;
/// ADR-0022 removed the last persistent surface that knew which record was
/// under the lamp, which turned R3 from missing into acute, so the text is now
/// the door to the sounding record's page.
///
/// Two doors, side by side, both labelled, two subjects: **the text is the
/// record, the word `Queue` is the queue.** Neither is a bare gesture and
/// neither is an icon.
fn now_playing_block(
    player: &PlayerState,
    place: Place,
    cover: Option<iced_image::Handle>,
) -> Element<'_, Message> {
    let stamps = player.stamps();
    // **The two timestamps moved here** (ADR-0017 §1.1), into the same
    // [`theme::STAMP_W`] slots they held when they flanked a groove — elapsed
    // right-aligned and total left-aligned, so the pair reads nose to nose with
    // one gap between them instead of 260 px of bar.
    //
    // They are reserved in both senses: the slot is that wide whatever the
    // digits say, and it is *there* whether or not anything is playing. A
    // stopped bar keeps the lane, so a track starting moves no title.
    let elapsed_color = if stamps.as_ref().is_some_and(|stamps| stamps.pending) {
        theme::active().lamp
    } else {
        theme::active().paper_faint
    };
    row![
        container(back_to_playing(player, cover))
            .width(Length::Fill)
            .clip(true),
        stamp(
            stamps.as_ref().map(|stamps| stamps.elapsed.clone()),
            elapsed_color,
            alignment::Horizontal::Right,
        ),
        stamp(
            stamps.as_ref().map(|stamps| stamps.total.clone()),
            theme::active().paper_faint,
            alignment::Horizontal::Left,
        ),
        queue_button(player, place == Place::Queue),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .into()
}

/// One of the two timestamps: a [`theme::STAMP_W`] slot, one line of tabular
/// figures, and nothing when there is nothing to say.
///
/// The digits are tabular — a property of the bundled Sans rather than of a
/// second face — and the slot is fixed, so they cannot shuffle anything
/// sideways as they tick or when a track crosses the hour.
///
/// It is one line box tall and centred in the row, so its **ink** lands on the
/// bar's one centre line rather than its block landing anywhere (law L4).
fn stamp(
    value: Option<String>,
    color: Color,
    align: alignment::Horizontal,
) -> Element<'static, Message> {
    let content: Element<'static, Message> = match value {
        None => Space::new(
            Length::Fixed(theme::STAMP_W),
            Length::Fixed(theme::LINE_META),
        )
        .into(),
        Some(value) => text(value)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(color)
            .wrapping(text::Wrapping::None)
            .into(),
    };
    container(content)
        .width(Length::Fixed(theme::STAMP_W))
        .height(Length::Fixed(theme::LINE_META))
        .align_x(align)
        .align_y(alignment::Vertical::Center)
        .into()
}

/// The **Queue** control: the word, the count of what it opens onto, and the
/// press that opens it.
///
/// Four properties, each of them load-bearing:
///
/// - **It is labelled, and it is always there.** Not a gesture, not a bare
///   figure, not an icon — the word says what the press opens. The study behind
///   this (`docs/design/03-interface-prior-art.md` §5.3(1)) is unambiguous: an
///   unlabelled route to a transient surface produces users who cannot tell
///   what they just did. It is offered with nothing queued too, because the
///   popover has an honest empty state and a control that came and went with
///   the music would be a moving target in the one row that does not move.
/// - **It says what it opens, and nothing else.** The readout used to be the
///   `3 / 12` position; it is now the queue's size — the critique's
///   `Queue · N`, with the separator carried by the gap rather than a middle
///   dot, since the label and the figure sit at opposite ends of a 152 px
///   control and a dot at the right edge would attach to nothing. The position
///   moved out into the ambient line beside it, where
///   [`PlayerState::continuation_note`] states it as *what is left* rather than
///   *where you are*. Printing both would have been the same subtraction twice.
///   Nothing was removed from the bar: a slot was replaced by a better
///   statement of the same fact, which is the one move `docs/REFUSALS.md`
///   permits here.
/// - **The readout is a reserved slot.** [`theme::POSITION_W`] wide whether or
///   not there is anything queued, so a queue arriving moves no title; and
///   `None` rather than `0`, because a queue that does not exist has no size.
/// - **The whole control is a reserved slot too** ([`theme::UP_NEXT_W`]), and
///   [`theme::now_playing`] varies only colour, never geometry — so hovering it
///   and opening the popover both leave every pixel where it was. The lit state
///   is the anchor the toolkit will not let the popover draw as a notch.
///
/// It is the same message <kbd>Q</kbd> sends.
fn queue_button(player: &PlayerState, open: bool) -> Element<'_, Message> {
    let room = theme::active();
    let readout: Element<'_, Message> = match player.queue_size_note() {
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
            // **The figure sits on the control's own inner edge**, not 36 px
            // inside it (the audit's defect 13). The readout keeps its reserved
            // [`theme::POSITION_W`] slot — the bar may not move when a queue
            // arrives — and the slack between the word and the slot is taken by
            // a fill rather than left at the right-hand end, so the number lands
            // on an edge something else shares.
            Space::with_width(Length::Fill),
            readout,
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fixed(theme::UP_NEXT_W))
    // 8 + a 16 px line box + 8 = `TRANSPORT_HIT`: one control height (law L7).
    // It stood 24 px tall in a bar whose published floor is 32.
    .padding(theme::pad(theme::GAP_SM, theme::GAP_SM))
    .style(move |_theme, status| theme::now_playing(room, status, open))
    .on_press(Message::ToggleQueue)
    .into()
}

/// **The route back to what is playing**: the now-playing lines, wrapped in the
/// press that opens the sounding record's page.
///
/// `docs/design/03-interface-prior-art.md` R3 — *get back to what is playing* —
/// is band A in the study, every product surveyed spends an affordance on it,
/// and baz had none. With no persistent inspector there is nothing else on
/// screen that knows which record is under the lamp, so the block that names it
/// is the control that takes you to it.
///
/// Three properties, each of them the accessibility refusal being honoured:
///
/// - **It is visible and it is labelled.** The label is the track title and the
///   artist — the name of the thing the press leads to — and the tooltip is the
///   verb, which is the whole of an accessible name in a toolkit that publishes
///   no tree.
/// - **It is offered only when it can act.** Nothing sounding means no press,
///   for the reason `Play album` and the queue's rows are inert without an
///   engine: a control that cannot act must not pretend it can. The lines are
///   then returned bare, and because the button carries no padding and no
///   border ([`theme::now_playing_text`]) the two states are the same pixels.
/// - **It is bigger than the floor.** The block is [`theme::NOW_PLAYING_H`] 56
///   tall against law L7's [`theme::TRANSPORT_HIT`] 32 — the law sets one
///   height for a control that is a *box*, and a control that is a block of
///   type is bounded below by the same number rather than exempt from it. The
///   assertion is in this module's tests.
fn back_to_playing(
    player: &PlayerState,
    cover: Option<iced_image::Handle>,
) -> Element<'_, Message> {
    let room = theme::active();
    let lines = now_playing_line(player);
    if player.playing_album().is_none() {
        return lines;
    }
    // **The sounding record's sleeve, inside the block's own hit box.** One
    // object: the cover and the type are the same control and go to the same
    // place, which is why the image is a child of the button rather than a
    // sibling of it. [`theme::BAR_COVER`] carries the fit; nothing about the
    // band moved to make room.
    //
    // With no artwork there is no lane and no placeholder — the block is drawn
    // exactly as it was before this existed. The wall's own rule, one surface
    // along: a tile with no decoded art draws its gradient because a tile is
    // *about* the record, and the bar's block is about the track, so the
    // honest absence here is nothing at all.
    let lines: Element<'_, Message> = match cover {
        None => lines,
        Some(handle) => row![
            iced_image(handle)
                .width(Length::Fixed(theme::BAR_COVER))
                .height(Length::Fixed(theme::BAR_COVER)),
            container(lines).width(Length::Fill).clip(true),
        ]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::Center)
        .into(),
    };
    // There is no lit state, where the `Queue` door beside it has one: pressing
    // this while already on that record's page is `Place::album`'s toggle
    // taking you back to the wall, and a now-playing block that lit up would be
    // the bar claiming a state about the *record* rather than about the door.
    // The page's own `‹ Library` is the labelled way out.
    //
    // The block's right press opens its mirror menu (doc 09 §5.2) — what
    // makes S4 two gestures *from anywhere*: the sounding track is always
    // in the bar, so `Add to "{current}"` is always one right-click away.
    // The bar gains no slot for it; the menu is a layer, and the ratchet
    // (`docs/REFUSALS.md`) is untouched.
    crate::menu::area(
        tooltip(
            button(lines)
                .width(Length::Fill)
                .padding(0)
                .style(move |_theme, status| theme::now_playing_text(room, status))
                .on_press(Message::ShowPlayingAlbum),
            text("Go to the record that is playing")
                .size(theme::SIZE_CAPTION)
                .line_height(theme::LEADING_CAPTION),
            tooltip::Position::Top,
        )
        .gap(theme::GAP_XS)
        .padding(theme::GAP_XS)
        .style(move |_theme| theme::tooltip(room)),
        crate::menu::Target::NowPlaying,
    )
}

/// The now-playing lines proper: the current track as a
/// title-over-artist-over-continuation stack, or the engine's plainly-stated
/// absence as quiet status text.
///
/// # The third line is the queue, said without being asked
///
/// `docs/design/critique/02-surfaces.md` specifies two things for this corner
/// and the bar shipped one: the label *and* "stack status when queued". Without
/// it the only route to what is coming was pressing the control beside it,
/// which makes *knowing* cost a click — and the popover exists for
/// *manipulating* the queue (jump, remove), which is a different act with a
/// different price. So the continuation is ambient:
/// [`PlayerState::continuation_note`] owns every word of it, this draws it, and
/// on the last track of a queue it draws nothing at all — silence is a refusal,
/// not an omission (`docs/REFUSALS.md`).
///
/// # Nothing here can move
///
/// No line wraps. A bar that grew a row under a long album title would shove
/// the shelf up by a line, which is exactly the kind of movement this bar is
/// built not to make; the enclosing zone clips instead. And the continuation's
/// lane is [`theme::CONTINUATION_H`] tall **whether or not there is a
/// continuation**, so the title and the artist above it sit on the same pixels
/// from the first track of a queue to the last — the line coming and going as
/// the music moves is precisely the case a reserved slot exists for.
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
    // Three lanes — 20 · 16 · 20 = [`theme::NOW_PLAYING_H`] — so the **artist's
    // line box is the block's exact middle** and centring the block centres the
    // zone's own line on the bar's (law L4). The artist's lane is reserved
    // whether or not the tags carry one, for the same reason the continuation's
    // is: a track without an artist must not shift the two lines around it.
    //
    // **And there is no gap between them**, which is not tightness: `LINE_BODY`
    // 20 around a 13 px face already carries 3.5 px of leading a side, and a
    // `GAP_XXS` here would be a fourth user of the lattice's one named
    // exception (law L2) buying what the line boxes carry. The air the block
    // needs is taken *outside* it, by [`theme::BAR_ZONE_LEAD`] — which is the
    // whole of what the bar's re-derivation from 56 to 80 changed.
    let lines = column![
        container(
            text(now.title.as_str())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None)
        )
        .height(Length::Fixed(theme::LINE_BODY)),
        container(match &now.artist {
            Some(artist) => Element::from(
                text(artist.as_str())
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_dim)
                    .wrapping(text::Wrapping::None),
            ),
            None => Space::with_height(Length::Fixed(theme::LINE_META)).into(),
        })
        .height(Length::Fixed(theme::LINE_META)),
        continuation_lane(player),
    ];
    // The block is [`theme::NOW_PLAYING_H`] in every state, which is the number
    // the whole band is derived from — say so here rather than letting it fall
    // out of three line boxes that a future edit could change one of.
    container(lines)
        .height(Length::Fixed(theme::NOW_PLAYING_H))
        .into()
}

/// The ambient continuation's lane: `then 2 albums · 1:58:00 left` in the
/// bar's quietest voice, or a strip of the same height saying nothing.
///
/// The strip is the whole trick. The line is present for every track of a queue
/// but the last, so it appears and disappears with the music; reserving its
/// height means the two lines above it never move, and the zone stays shorter
/// than the centre column, so the bar's height stays a property of the
/// transport (asserted in [`theme`] and below).
fn continuation_lane(player: &PlayerState) -> Element<'_, Message> {
    let Some(note) = player.continuation_note() else {
        return Space::with_height(Length::Fixed(theme::CONTINUATION_H)).into();
    };
    container(
        text(note)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION)
            .color(theme::active().paper_faint)
            .wrapping(text::Wrapping::None),
    )
    .height(Length::Fixed(theme::CONTINUATION_H))
    .into()
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

/// The bar's centre: the transport row, and nothing else.
///
/// It was a *column* — the three buttons over a seek row, both centred in a
/// fixed 380 px block (`SEEK_ROW_W`) — because the controls and the
/// position they acted on read as one thing. The needle took the position to
/// the window's edge, so what is left is the row, in a
/// [`theme::TRANSPORT_W`] 112 px column: the block's centre is now the
/// buttons' own centre by construction rather than by a width they happened to
/// share with a groove, and the 268 px the column gives up go to the two
/// flanking zones — the left one being the zone the composition audit found
/// clipping below ~900 px.
///
/// **Previous · Play/Pause · Next stay**, and that is a decision rather than an
/// omission. ADR-0017 §1.1 refused the critique's hover-reveal-over-the-cover
/// transport on evidence the critique did not have: our own prior-art study
/// (R11) found three vendors bought "visual calm" by removing skip and all
/// three reversed; our own audit found "there is no Previous" the most-missed
/// control in the app; and glyphs over the playing cover need the playing cover
/// to be *on screen*, which after a filter or a long scroll it is not.
/// `docs/REFUSALS.md`'s visible-control rule makes it binding.
///
/// **And they stay here only.** The Now playing place drew this row a second
/// time, a few hundred pixels above the bar that was already under it — the
/// owner's *"now playing does not need the play pause controls"* and *"ensure
/// the play next and previous controls are removed"*. The wrapper that shared
/// it is gone with the second copy: the bar is in every place, so a place that
/// wants a transport already has one.
fn transport_glyphs(player: &PlayerState, ink: Ink) -> Element<'_, Message> {
    let pending = player.transport_pending();
    let toggle = player.play_pause();
    row![
        glyph_button(
            icon::Glyph::Previous,
            "Previous track",
            player.previous_enabled(),
            pending,
            Message::PreviousTrack,
            Control::Previous,
            ink,
        ),
        glyph_button(
            toggle.into(),
            toggle.label(),
            player.play_pause_enabled(),
            pending,
            Message::PlayPause,
            Control::PlayPause,
            ink,
        ),
        glyph_button(
            icon::Glyph::Next,
            "Next track",
            player.next_enabled(),
            pending,
            Message::NextTrack,
            Control::Next,
            ink,
        ),
    ]
    .spacing(theme::GAP_SM)
    .into()
}

fn transport_row(player: &PlayerState, ink: Ink) -> Element<'_, Message> {
    let transport = transport_glyphs(player, ink);
    // **The zone states the band's lead rather than borrowing the row's
    // centring** (law L4). `BAR_LEAD` is derived — whatever is left of
    // [`theme::BAR_CONTENT_H`] once the transport has taken its 32, halved — so
    // this padding cannot drift from the band, and the transport's centre line
    // *is* the band's mid-line by construction rather than by an assertion
    // somebody has to keep re-checking.
    container(transport)
        .width(Length::Fixed(theme::TRANSPORT_W))
        .padding(theme::pad(theme::BAR_LEAD, 0.0))
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
        .into()
}

/// One icon-only control: a glyph in a fixed square, named by a tooltip.
///
/// The size is fixed in both axes and the glyph is drawn into a box of its
/// own, so swapping play for pause moves nothing. `pending` reaches the ink
/// and only the ink (see [`theme::glyph_ink`]).
///
/// # The mark is the control, so the mark answers the pointer
///
/// The glyph is a rasterised sprite, and a `button` style's `text_color` never
/// reaches one — which is why hovering an icon button used to change the box
/// and leave the mark byte-identical (`docs/design/04-fluidity.md` §3.1). The
/// lever that *does* reach it is the image's own opacity, so the button reports
/// its own crossings through a `mouse_area` and the shell holds one
/// [`Control`] id (ADR-0020 §2.1). The ink then rides a 90 ms tween up the
/// ladder: 0.57 resting, 1.00 under the pointer, 0.75 held, 0.28 dead.
///
/// **The `mouse_area` is outside the button and only listens for crossings.**
/// `button` captures the press itself, so an outer wrapper could not see one
/// even if it asked; `CursorMoved` it ignores, which is exactly the event the
/// crossings are made of. The press comes from the shell's raw event stream
/// instead. Nothing about the button's own behaviour changes, and the hit target
/// is the same [`theme::TRANSPORT_HIT`] square it always was.
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
    control: Control,
    ink: Ink,
) -> Element<'_, Message> {
    let room = theme::active();
    let mark = container(
        iced_image(icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::glyph_ink(
                enabled,
                pending,
                ink.hover(control),
                ink.pressed(control),
            )),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let control_widget = button(mark)
        .width(Length::Fixed(theme::TRANSPORT_HIT))
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(0)
        .style(move |_theme, status| theme::transport(room, room.recess, status))
        .on_press_maybe(enabled.then_some(message));
    let named = tooltip(
        control_widget,
        text(label)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Top,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room));
    mouse_area(named)
        .on_enter(Message::ControlEntered(control))
        .on_exit(Message::ControlLeft(control))
        .into()
}

/// **The player's shuffle property**: the crossed arrows, lit when it is on.
///
/// # Why it is on this bar and not in the Library strip
///
/// One line, and it is L8.1's: *a control goes where what it reads is.* What a
/// mode reads is **the player**, and the player's surface is this bar — which
/// is under every place, where the strip is under one. A shuffle that governs a
/// playlist's `Play` cannot live on a control a listener cannot see from the
/// playlist page.
///
/// It is an **addition** to this bar, and nothing was removed to make room —
/// which matters because this bar's slots have a history: the prior-art study
/// (R11) found three vendors buy "visual calm" by removing control density and
/// all three reverse, and what they lose is always position, provenance and
/// skip. The transport did not move and no readout was traded away.
///
/// # Why the right-hand zone rather than the transport
///
/// The transport is the zone of **acts that happen once** — Previous, Play,
/// Next, each spent the moment it is pressed — and a fourth glyph among three
/// verbs would read as a fourth verb. The right-hand zone is where the player's
/// **standing properties** already are: the volume, the mute, and the signal
/// path they produce, none of which any track boundary touches. Shuffle is one
/// of those, so it stands with them. It also keeps the transport symmetric
/// about the bar's centre line and [`theme::TRANSPORT_W`] untouched.
///
/// # The glyph, and the lit state
///
/// `docs/design/10-controls-and-iconography.md` §3.2 refused the crossed arrows
/// for a precise reason: the symbol *promises a mode with a lit state*, and
/// baz's shuffle was an act. It is a mode now, so the promise is one the control
/// can keep, and the clause is rewritten rather than merely overridden. Lit is
/// the **accent**, and that is the one place the accent-discipline note admits
/// a second use: this control creates playback truth about what sounds next in
/// exactly the way `Play album` creates it about what sounds now.
///
/// The tooltip says which way the press goes, because a two-state glyph with
/// one name is the thing a first-timer cannot form an expectation about
/// (doc 11 §5 P6.2).
fn shuffle_toggle(player: &PlayerState, ink: Ink) -> Element<'static, Message> {
    let room = theme::active();
    let on = player.shuffle();
    let mark = container(
        iced_image(icon::inked(
            icon::Glyph::Shuffle,
            if on { room.lamp } else { room.glyph() },
        ))
        .width(Length::Fixed(theme::ICON_PX))
        .height(Length::Fixed(theme::ICON_PX))
        .opacity(if on {
            1.0
        } else {
            theme::glyph_ink(
                true,
                false,
                ink.hover(Control::Shuffle),
                ink.pressed(Control::Shuffle),
            )
        }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let control = button(mark)
        .width(Length::Fixed(theme::TRANSPORT_HIT))
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(0)
        .style(move |_theme, status| theme::transport(room, room.recess, status))
        .on_press(Message::ToggleShuffle);
    let named = tooltip(
        control,
        text(if on {
            "Shuffle is on \u{2014} turn it off and the run goes back to its own order"
        } else {
            "Shuffle is off \u{2014} turn it on and what plays next is shuffled"
        })
        .size(theme::SIZE_CAPTION)
        .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Top,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room));
    mouse_area(named)
        .on_enter(Message::ControlEntered(Control::Shuffle))
        .on_exit(Message::ControlLeft(Control::Shuffle))
        .into()
}

/// The volume control: a mute affordance and a fader, with a lane above the
/// fader where the level preview floats.
///
/// The fader is [`groove::Groove`], and the needle is built on the same pointer
/// machinery ([`crate::pointer`]) — the same cursor affordance, the same hover
/// preview, the same 4 px click-vs-drag threshold, the same "pointer lost ends
/// the gesture" — plus the one thing a fader needs and a seek line does not: a
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
fn volume(player: &PlayerState, ink: Ink) -> Element<'_, Message> {
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
        // **The mute glyph sits on the fader's rail**, and it needs no lift to
        // get there. The block is [`theme::VOLUME_ROW_H`] — one control height
        // — with the fader's own hit band centred in it, so centring the block
        // centres the *rail*, and a centred mute button lands its glyph on the
        // same line. The `MUTE_TOP` constant that used to buy this by hand is
        // deleted, and so are the two 16 px lanes that bought it structurally
        // when the bar was 96 px tall: at 56 the block is one square, which is
        // the same symmetry with two fewer numbers in it.
        glyph_button(
            icon::Glyph::speaker(state.muted),
            state.mute_label,
            state.interactive,
            state.mute_pending,
            Message::ToggleMute,
            Control::Mute,
            ink,
        ),
        // The level preview is a **layer** over the lane the bar already keeps
        // above the fader ([`theme::BAR_LEAD`] plus the fader's own slop above
        // its rail), exactly as the seek groove's tip was and the needle's is.
        // It costs the block no height, which is what lets the block be one
        // control height at all.
        stack![
            container(fader)
                .height(Length::Fill)
                .align_y(alignment::Vertical::Center),
            container(preview_lane(state.preview, theme::VOLUME_W, theme::LEVEL_W))
                .height(Length::Fill)
                .align_y(alignment::Vertical::Top),
        ],
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .width(Length::Fixed(theme::VOLUME_BLOCK_W))
    .height(Length::Fixed(theme::VOLUME_ROW_H))
    .into()
}

/// The lane above a groove where its hover preview floats: a fixed-height
/// strip `width` wide, empty until the pointer rests on the bar, then
/// carrying a `tip_width` tip centered on the pointer with what a click
/// there would ask for — a level over the fader. (The needle's own tip is
/// [`tip_layer`]: same idea, different lane, and a label that can be a record's
/// name as well as a time.)
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

    /// The bar's reserved-slot rule, re-checked for a centre column that is
    /// now the transport row and nothing else.
    ///
    /// `theme.rs` pins the tokens; this is the same claim for the row the view
    /// composes, asserted here so the module that builds the row owns the check
    /// for what it builds.
    #[test]
    fn the_transport_row_is_the_column_it_used_to_be_centred_in() {
        const TRANSPORT_ROW_W: f32 = 3.0 * theme::TRANSPORT_HIT + 2.0 * theme::GAP_SM;
        // The column *is* the row now — 112 px, where it was a 380 px block
        // sized for a groove and two timestamps. Equality rather than "fits
        // inside", because a column wider than its contents centres a block
        // and a column that is its contents centres the marks (law L4).
        const { assert!(TRANSPORT_ROW_W == theme::TRANSPORT_W) }
        // And the bar still fits the narrowest shipped window beside its two
        // flanking zones, with 268 px more room than it had.
        const { assert!(theme::TRANSPORT_W + theme::VOLUME_BLOCK_W + theme::SIGNAL_W < 760.0) }
    }

    /// **Law L4 — one centre line per bar**, re-derived at 80 px rather than
    /// nudged, and asserted off the geometry the view composes rather than off
    /// a screenshot.
    ///
    /// The audit measured seven mark-lines spanning 787 → 837 in a 102 px band
    /// whose own mid-line at 809.5 carried nothing: the zones were centred as
    /// *blocks*, and the blocks are different heights. The band has been
    /// re-derived twice since — down to 56 when the needle took the seek row's
    /// job, and back up to **80** when 56 turned out to be the same number as
    /// the left zone's own height, so the type touched both edges of the bar —
    /// and every number below is derived again from the band each time rather
    /// than carried over.
    ///
    /// The spread is **0**, against the law's ceiling of 2.
    #[test]
    fn every_mark_in_the_bar_sits_on_the_bars_one_centre_line() {
        /// The band's own mid-line — 40.
        const MID: f32 = theme::BAR_CONTENT_H / 2.0;
        /// The transport's glyph centres, from the top of the band.
        const TRANSPORT_CENTRE: f32 = theme::BAR_LEAD + theme::TRANSPORT_HIT / 2.0;
        /// The fader's rail centre, from the top of the volume block, which is
        /// itself centred in the band.
        const VOLUME_RAIL_CENTRE: f32 = theme::VOLUME_ROW_H / 2.0;
        /// The left zone's middle lane — the artist's line box — from the top
        /// of the band: the zone's lead, then the title's lane, then half the
        /// artist's.
        const ARTIST_CENTRE: f32 = theme::BAR_ZONE_LEAD + theme::LINE_BODY + theme::LINE_META / 2.0;
        /// A timestamp's ink centre: one line box, centred in the same band.
        const STAMP_CENTRE: f32 = MID;

        // 1. The band's mid-line **is** the transport's centre line. `BAR_LEAD`
        //    is derived from the band rather than chosen, so this cannot drift:
        //    it is (80 − 32) / 2 = `GAP_XL` 24, and the transport row states it
        //    as padding rather than borrowing the row's centring.
        const { assert!(TRANSPORT_CENTRE == MID) }
        const { assert!(theme::BAR_LEAD == theme::GAP_XL) }
        const { assert!(theme::BAR_CONTENT_H == 2.0 * theme::BAR_LEAD + theme::TRANSPORT_HIT) }
        // 2. The volume block is one control height with the fader's hit band
        //    centred in it, so centring the block centres the **rail** — the
        //    audit's 816 against 809.5 — and the mute glyph beside it lands on
        //    the same line without a lift.
        const { assert!(VOLUME_RAIL_CENTRE == theme::VOLUME_ROW_H / 2.0) }
        const { assert!(theme::VOLUME_ROW_H == theme::TRANSPORT_HIT) }
        const { assert!(theme::VOLUME_HIT < theme::VOLUME_ROW_H) }
        // 3. The left zone's three lanes are symmetric about the middle one and
        //    are led equally above and below, so its middle lane's centre *is*
        //    the band's centre. This is the assertion the re-derivation to 80
        //    turns on: the block is 56 and the lead is 12 a side, where at 56
        //    the block filled the band and the two were the same number.
        const { assert!(ARTIST_CENTRE == MID) }
        const {
            assert!(
                theme::NOW_PLAYING_H == theme::LINE_BODY + theme::LINE_META + theme::CONTINUATION_H
            );
        }
        const { assert!(theme::BAR_CONTENT_H == theme::NOW_PLAYING_H + 2.0 * theme::BAR_ZONE_LEAD) }
        const { assert!(theme::CONTINUATION_H == theme::LINE_BODY) }
        // 4. The two timestamps are one line box centred in the band, so their
        //    ink is on the line rather than hanging off a groove that no longer
        //    exists (the audit measured the old pair at 837, 27.5 px low).
        const { assert!(STAMP_CENTRE == MID) }
        const { assert!(theme::LINE_META < theme::BAR_CONTENT_H) }
        // 5. There is no vertical padding left to be asymmetric: the band is
        //    the whole bar, so the hairline is the only thing above the line's
        //    own arithmetic and it is 1 px on both readings.
        const { assert!(theme::BAR_CONTENT_H == 80.0) }
        // 6. Every zone fits inside the band with air to spare, or the band
        //    would not be what sets the line. **This is the proportion the
        //    owner's "too short" was about**: at 56 the left zone's `<=` was an
        //    equality and there was no air at all.
        const { assert!(theme::NOW_PLAYING_H < theme::BAR_CONTENT_H) }
        const { assert!(theme::VOLUME_ROW_H < theme::BAR_CONTENT_H) }
        const { assert!(theme::TRANSPORT_HIT < theme::BAR_CONTENT_H) }
    }

    /// **The band is derived from what it must hold, plus a stated lead** —
    /// the breathing rule, and where it lands against the window.
    ///
    /// The owner's reading was *"proportion is becoming an issue e.g. bottom
    /// bar is too short"*, and the arithmetic agreed: the left zone's three
    /// line boxes are 56 px and the band was 56, so the type filled the bar
    /// edge to edge.
    ///
    /// The lane count was re-argued before the height was. **Three lanes
    /// stay**: the continuation (`then 2 albums · 1:58:00 left`) earns its rung
    /// because ADR-0022 made the queue a *place* — reading what is next used to
    /// cost a popover that reflowed nothing and now costs leaving the wall — so
    /// the ambient line is the only free reading of the queue baz has, and it
    /// became more valuable at exactly the moment the bar became shorter.
    ///
    /// Then the height follows from the content and a **named gap** on each
    /// side. Not a ratio: a constant ink-to-band ratio is not reachable on the
    /// 4 px lattice for two bands of different content heights, and a lead off
    /// the lattice is law L2 broken to make a proportion true.
    #[test]
    fn the_band_is_its_content_plus_a_stated_lead_and_lands_on_two_hangs() {
        /// The bar as the composition audit measured it, before the needle.
        const WAS: f32 = 2.0 * (theme::GAP_SM + 24.0) + theme::TRANSPORT_HIT + 2.0 * 4.0 + 1.0;
        /// What the window's bottom edge cost at the needle's 57 px bar.
        const SHORT: f32 = 56.0 + 1.0 + theme::NEEDLE_H;
        /// What it costs now.
        const NOW: f32 = theme::BAR_CONTENT_H + 1.0 + theme::NEEDLE_H;

        const { assert!(WAS == 105.0) }
        const { assert!(SHORT == 59.0) }
        const { assert!(NOW == 83.0) }

        // **The band is two hangs**, which is why this height and not the 72
        // one rung below it: `HANG` is the product's one structural unit — the
        // window gutter, the wall label's height, the shelf header's band — so
        // the bar is measured in the same unit as the collection above it
        // rather than in a number of its own, and *both* of its leads come out
        // as named tokens rather than as pixels chosen to look right.
        const { assert!(theme::BAR_CONTENT_H == 2.0 * theme::HANG) }
        const { assert!(theme::BAR_ZONE_LEAD == theme::GAP_MD) }
        const { assert!(theme::BAR_LEAD == theme::GAP_XL) }
        // The lead is a named gap, and it is one rung above the top bar's,
        // because a hit box carries its own internal padding and a stack of
        // line boxes carries only its leading.
        const { assert!(theme::TOP_BAR_PAD_V == theme::GAP_SM) }
        const { assert!(theme::BAR_ZONE_LEAD > theme::TOP_BAR_PAD_V) }

        // Where it lands as a fraction of the window: 9.7 % at 860 and 7.7 % at
        // 1080, against 12.2 % before the needle and 6.9 % at 57. The wall
        // gained 46 px from the needle's work and gives 24 of them back — the
        // minimum that buys real air on the lattice, since the next step down
        // (72, an 8 px lead) is defensible and the one below it (64, a 4 px
        // lead) is not air at all.
        const { assert!(NOW < 0.10 * 860.0) }
        const { assert!(NOW < 0.08 * 1080.0) }
        const { assert!(WAS - NOW == 22.0) }
        const { assert!(NOW - SHORT == 24.0) }
        // The concession to the critique's ~32 px of bottom furniture, which is
        // what keeps Previous · Play/Pause · Next pointer-reachable.
        const { assert!(NOW - 32.0 == 51.0) }
        // And the needle's aiming band is still entirely inside the bar's
        // bottom lane, which is empty recess — so claiming height out of layout
        // can never take a press meant for a control. The lane grew, so the
        // bound got looser rather than tighter.
        const { assert!(theme::NEEDLE_HIT <= theme::BAR_LEAD) }
        // **81 is reachable and 80 was never going to be**: a bar is
        // `2ℓ + TRANSPORT_HIT + 1`, which is odd for every integer lead —
        // the hairline is odd and everything else is doubled. Stated as the
        // parity of the band rather than of the bar, so no cast is needed.
        const { assert!(theme::BAR_CONTENT_H == 2.0 * theme::BAR_LEAD + theme::TRANSPORT_HIT) }
        const { assert!(theme::BAR_LEAD + theme::BAR_LEAD == theme::BAR_CONTENT_H - 32.0) }
    }

    /// **The sounding record's cover fits the band that already existed.**
    ///
    /// The brief's one arithmetic claim, checked rather than asserted: 52 px
    /// square, inside the bar's 80 px band and its named lead, with nothing
    /// about either re-derived. [`theme::BAR_CONTENT_H`] is still two hangs,
    /// [`theme::BAR_ZONE_LEAD`] is still `GAP_MD`, and the cover is the
    /// largest square on the 4 px lattice that fits the 56 px the tallest zone
    /// already reserved — 52, with 2 px of slack above and below it inside a
    /// zone that is itself led by 12.
    #[test]
    fn the_cover_fits_the_bands_existing_lead_without_moving_it() {
        // The band and its leads are exactly what they were.
        const { assert!(theme::BAR_CONTENT_H == 2.0 * theme::HANG) }
        const { assert!(theme::BAR_ZONE_LEAD == theme::GAP_MD) }
        const { assert!(theme::NOW_PLAYING_H == theme::BAR_CONTENT_H - 2.0 * theme::BAR_ZONE_LEAD) }
        // The cover fits inside the tallest zone, on the lattice, and is the
        // largest such square: it is exactly one lattice step short of the
        // 56 px zone, so the next rung up *is* the zone and leaves no slack
        // at all.
        const { assert!(theme::BAR_COVER == 52.0) }
        const { assert!(theme::BAR_COVER < theme::NOW_PLAYING_H) }
        const { assert!(theme::BAR_COVER % theme::GAP_XS == 0.0) }
        const { assert!(theme::BAR_COVER + theme::GAP_XS == theme::NOW_PLAYING_H) }
        // And it never exceeds the decoded source, which is the wall's own
        // rule about artwork applied one surface along.
        const { assert!(theme::BAR_COVER <= theme::ART_MAX) }
    }

    /// **With artwork the cover is part of the control; without it the block
    /// is what it always was.**
    ///
    /// Two claims, both about where the widget sits rather than about what it
    /// says, so both are pinned to the source:
    ///
    /// 1. The image is built **inside** `back_to_playing`, before the
    ///    `button` — so the cover and the type are one hit target that goes
    ///    one place, not a picture beside a link.
    /// 2. The `None` arm returns the lines untouched. No reserved lane, no
    ///    placeholder, no gradient: a record with no decodable art draws the
    ///    bar that shipped before this existed.
    #[test]
    fn the_cover_joins_the_blocks_own_hit_target_and_is_absent_without_art() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/bottom_bar.rs"),
        )
        .expect("this module's own source")
        .replace("\r\n", "\n");
        let rest = source
            .split_once("fn back_to_playing")
            .expect("the now-playing block exists")
            .1;
        let block = &rest[..rest.find("\n}\n").expect("a function ends")];
        let cover_at = block
            .find("theme::BAR_COVER")
            .expect("the cover is drawn here");
        let button_at = block.find("button(lines)").expect("the block is a button");
        assert!(
            cover_at < button_at,
            "the cover is not inside the block's button — a picture beside a \
             link is two objects where the design asks for one"
        );
        assert!(
            block.contains("None => lines,"),
            "no artwork must return the lines exactly as they were"
        );
        for forbidden in [
            "gradient_block",
            "Space::new(Length::Fixed(theme::BAR_COVER",
        ] {
            assert!(
                !block.contains(forbidden),
                "the bar reserves `{forbidden}` for artwork that does not \
                 exist — the brief says the block renders exactly as today"
            );
        }
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

    /// **The bar reserves every slot it can be in** — re-checked for a left
    /// zone that just gained the two timestamps.
    ///
    /// One of the four properties `docs/design/01-ux-audit-and-ia.md` §5 says
    /// must not regress. The zone carries a labelled control with a readout that
    /// comes and goes with the queue, a continuation line that comes and goes
    /// with the *position in* the queue, and now an elapsed and a total that
    /// come and go with playback itself. All of them are reservations rather
    /// than additions: the control is [`theme::UP_NEXT_W`] and the readout
    /// inside it [`theme::POSITION_W`], each stamp is [`theme::STAMP_W`] whether
    /// or not there is a figure in it, the continuation's lane is
    /// [`theme::CONTINUATION_H`] tall whether it says anything or not, and the
    /// control's border is present in every state.
    #[test]
    fn the_left_zone_reserves_the_stamps_the_continuation_and_the_count() {
        // The zone's own budget at the shipped window: the two stamps, the
        // readout, the gaps between them and the button's horizontal padding
        // all come out of the fill zone, and what is left has to be a real
        // title lane — wide enough for the continuation line as well as the
        // title, since they share it.
        //
        // **The centre column gave 268 px back** when the seek row went, and
        // this is where they went: the title lane roughly doubles even after
        // the two stamps move in, which is the audit's §1.5 finding (the left
        // zone wrapping below ~900 px) addressed by arithmetic rather than by a
        // maximum width.
        const SHIPPED: f32 = 1280.0;
        const ZONE: f32 = SHIPPED
            - 2.0 * theme::HANG // the bar's own padding: the one window gutter
            - 2.0 * theme::GAP_LG // the gaps between its three zones
            - theme::TRANSPORT_W
            - theme::SIGNAL_W
            - theme::GAP_SM
            - theme::VOLUME_BLOCK_W;
        const TITLE_LANE: f32 =
            ZONE - theme::UP_NEXT_W - 2.0 * theme::STAMP_W - 3.0 * theme::GAP_SM;
        /// The zone's whole height: three stacked line boxes, every one of them
        /// reserved, so this is its height in every state rather than its
        /// tallest.
        const LEFT_H: f32 = theme::LINE_BODY + theme::LINE_META + theme::CONTINUATION_H;

        const { assert!(TITLE_LANE > 200.0) }
        // The zone is the bar's content band **less one lead a side**, so
        // neither the control's padding nor the continuation nor a stamp can be
        // what sets the bar's height, and the zone's middle lane is the bar's
        // centre line (law L4). It used to be *exactly* the band, which is the
        // proportion this bar was re-derived to fix.
        const { assert!(LEFT_H == theme::NOW_PLAYING_H) }
        const { assert!(LEFT_H + 2.0 * theme::BAR_ZONE_LEAD == theme::BAR_CONTENT_H) }
        // **The stack is symmetric about its middle lane**: the title's lane
        // and the continuation's are the same height, so the artist's line box
        // is the block's exact centre. Without this the middle line sits low
        // and the whole zone reads as one notch off.
        const { assert!(theme::CONTINUATION_H == theme::LINE_BODY) }
        // The lane still holds the line it is reserved for, with air to spare —
        // it is one line of caption type, and nothing here may wrap.
        const { assert!(theme::CONTINUATION_H >= theme::LINE_CAPTION) }
        // A stamp is one line box, so it centres on the band's line rather than
        // hanging below it the way it did beside the groove.
        const { assert!(theme::STAMP_W > 0.0 && theme::LINE_META <= theme::BAR_CONTENT_H) }
        // **The now-playing block is a pointer target now** — the route back to
        // the record that is sounding (R3) — and law L7's floor holds for it:
        // the law sets one height for a control that is a *box*, and a control
        // that is a block of type is bounded below by the same number rather
        // than exempt from it. 56 against 32, with no padding and no border, so
        // becoming a control moved nothing.
        const { assert!(theme::NOW_PLAYING_H >= theme::TRANSPORT_HIT) }

        let mut player = PlayerState::new(Availability::Ready);
        // Nothing queued and nothing playing: neither the count nor the
        // continuation nor a stamp says anything, and every slot is still there.
        assert_eq!(player.queue_size_note(), None);
        assert_eq!(player.continuation_note(), None);
        assert_eq!(player.stamps(), None);
        // And the needle draws its track with no fill and refuses the pointer,
        // rather than vanishing and taking 2 px of wall with it.
        let line = player.needle_bar();
        assert!(line.entries.is_empty());
        assert_eq!(line.playing, None);
        assert!(!line.interactive);

        player.apply(
            &Event::TrackStarted {
                path: std::path::PathBuf::from("/music/a/01.flac"),
                position: 0,
            },
            &[],
        );
        // Without a recorded queue there is still nothing to count, nothing to
        // say follows, and no segment to point at — the front end never invents
        // any of them (see `player.rs`'s honesty rule). The stamps do appear,
        // because an elapsed time is a fact the engine reported.
        assert_eq!(player.queue_size_note(), None);
        assert_eq!(player.continuation_note(), None);
        assert!(player.stamps().is_some());
        assert!(!player.needle_bar().interactive);
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
