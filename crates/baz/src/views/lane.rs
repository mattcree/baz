//! **The returns lane** — the resident surface at the window's left edge
//! (ADR-0030, as the owner amended it).
//!
//! Three parts, top to bottom:
//!
//! 1. **The head** — `Home`, `Library`, `Playlists`, `Now playing`, always all
//!    four and always in that order. Search moved from this surface to the
//!    resident app bar in ADR-0040's 2026-08-12 amendment.
//!    The place you are in is drawn in full paper ink; the others rest at
//!    `paper_dim`. **`Now playing` carries the lamp dot when something is
//!    sounding** — the accent's one reserved meaning, spent so the lane can
//!    answer *is anything on?* without being read.
//! 2. **`RECENT`** — playlists and the last
//!    [`crate::lane::RECENT_ALBUMS`] records in one last-touched order
//!    ([`crate::lane`] owns the membership and ordering and tests it without a
//!    window), inside one scroller.
//! 3. **`Collapse`** at the foot — one control, a chevron and its word, the
//!    chevron pointing the way the lane will move. Collapsed it is the
//!    chevron alone under its tooltip, like the destinations above it.
//!
//! # Why search is still not a destination
//!
//! Spotify — the reference the owner keeps naming — makes `Search` a
//! destination you navigate to. baz must not, and the reason is a feature baz
//! has that Spotify does not: **type-anywhere** (ADR-0017 §1.2). Any printable
//! key reveals the chooser from anywhere in the product, so the query is already
//! open before you have decided to search; a destination row would say *go
//! somewhere first*, which is the opposite of what the product does, and it
//! would leave the thing the keystroke actually fills — the field — somewhere
//! else on screen.
//!
//! The well is now an app-wide frame control: it remains visible while the
//! listener moves between places, and its dropover covers rather than replaces
//! the place underneath.
//!
//! # The collapse is a hard cut
//!
//! No tween (ADR-0030 §3.1). A width tween would re-resolve `Grid::new` on
//! every frame of the slide and pop columns mid-flight; one frame is cheaper
//! *and* better. This is the one press in the product that re-hangs the
//! collection, and it is safe because it lands **outside the wall** — no wall
//! gesture can be in flight when it fires.
//!
//! # The ground
//!
//! The lane stands on [`theme::Palette::recess`] — one plane *below* the wall
//! — so it reads as cut into the room rather than stuck onto it, with a
//! hairline on its right edge and nothing else. That choice is also what makes
//! its rows answer the pointer correctly for free: a row steps one plane up
//! from whatever it stands on ([`theme::Palette::step_up`]), so on the recess
//! a hovered row lands on the wall's own colour.

use std::sync::LazyLock;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use iced::widget::{
    Space, button, column, container, image as iced_image, row, rule, scrollable, text,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::lane::{Destination, Subject, Touched};
use crate::place::Place;
use crate::playlists::Playlists;
use crate::views::playlist_sleeve;
use crate::{font, icon, theme};

static LANE_REGULAR: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(font::SANS_REGULAR).expect("the bundled regular face is valid")
});
static LANE_MEDIUM: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(font::SANS_MEDIUM).expect("the bundled medium face is valid")
});

/// The lane, at the width its state says.
///
/// `lane` is already resolved, split and ordered by [`crate::lane::resolve`] —
/// this function decides nothing about membership, which is what keeps the
/// ordering testable without a window.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    place: Place,
    lane: &'a crate::lane::Lane,
    sounding: bool,
    sounding_row: Option<Subject>,
    window_w: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let open = theme::sidebar_w(window_w, shelf.lane_open) >= theme::SIDEBAR_W;
    let width = theme::sidebar_w(window_w, shelf.lane_open);

    let mut head = column![];
    for to in Destination::ALL {
        head = head.push(destination_row(to, place, open, sounding));
    }

    let body_rows = sections(shelf, playlists, lane, open, sounding_row);
    // **The rows carry the lane's gutter, not the scrollable**, so the bar
    // rides the lane's own right edge — the owner's *"the scrollbar should be
    // at the edge of it"*. A scrollbar inset by the gutter reads as belonging
    // to the list rather than to the surface, and leaves a dead strip between
    // it and the seam. The rows keep the inset; only the bar reaches the edge.
    // `wall_scrollbar` consumes its 4 px from the scrollable's content box.
    // The rows still need their full 232 px measure, including the Recent
    // lamp's far-trailing slot, so only the edge-side gutter yields those
    // pixels. Keeping the leading gutter at 24 preserves the head and row
    // alignment; shrinking both would merely move the clip to the other side.
    let list_pad = iced::Padding {
        top: 0.0,
        right: theme::GAP_XL - theme::WALL_SCROLLBAR_W,
        bottom: 0.0,
        left: theme::GAP_XL,
    };
    let list = scrollable(body_rows.padding(list_pad))
        .on_scroll(Message::LaneScrolled)
        .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.recess, status))
        .width(Length::Fill)
        .height(Length::Fill);

    let flanked = |e: Element<'a, Message>| {
        container(e)
            .width(Length::Fill)
            .padding(theme::pad(0.0, theme::GAP_XL))
    };
    let body = column![
        flanked(head.into()),
        // The head's one rule: three destinations above it, the things you
        // have touched below. **The lane still has exactly one seam**, and
        // that is the point of drawing the sections' headings rather than a
        // second rule: a heading names a section, a rule cuts the surface, and
        // there is one cut here because there are two parts — the frame's
        // concerns, and yours.
        container(rule::horizontal(1).style(move |_theme| theme::hairline(room, room.recess)))
            .padding(theme::pad(theme::GAP_MD, theme::GAP_XL)),
        list,
        flanked(marks(shelf.lane_open, theme::sidebar_can_expand(window_w))),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // Keep the established `GAP_XL` lead at the top, but only `GAP_MD` under
    // the footer. The old symmetric 24 px outer padding combined with the
    // collapse control's own 12 px padding into 36 px of dead space at the
    // bottom, paid for by the scrollable list above it.
    let lane = container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: theme::GAP_XL,
            right: 0.0,
            bottom: theme::GAP_MD,
            left: 0.0,
        })
        .style(move |_theme| theme::lane_ground(room));

    row![
        lane,
        // The hairline on the right edge, in the lane's own width. Drawn as a
        // sibling rather than as a border because iced 0.13's `Border` is
        // four-sided, which is why every single line in the product is a rule.
        container(Space::new().width(Length::Fixed(1.0)).height(Length::Fill))
            .height(Length::Fill)
            .style(move |_theme| theme::lane_seam(room)),
    ]
    .width(Length::Fixed(width))
    .height(Length::Fill)
    .into()
}

/// One of the head's four destinations: the glyph, and — expanded — its word.
///
/// **Not a toggle.** [`Place::go`] argues it: pressing the destination you are
/// already at leaves you there, where `Queue` and `Settings` close themselves.
/// The row is still a live control in that state rather than an inert mark,
/// which is the one place this departs from the density detents' anatomy —
/// a destination that stopped answering the pointer when you arrived would
/// read as broken, and pressing it costs nothing.
fn destination_row(
    to: Destination,
    place: Place,
    open: bool,
    sounding: bool,
) -> Element<'static, Message> {
    let room = theme::active();
    let here = place.destination() == Some(to);
    let glyph = match to {
        Destination::Home => icon::Glyph::Home,
        Destination::Library => icon::Glyph::Library,
        Destination::Playlists => icon::Glyph::Queue,
        Destination::NowPlaying => icon::Glyph::NowPlaying,
    };
    let mark = iced_image(icon::handle(glyph))
        .width(Length::Fixed(theme::ICON_PX))
        .height(Length::Fixed(theme::ICON_PX))
        .opacity(if here {
            theme::GLYPH_OPACITY_HOVER
        } else {
            theme::GLYPH_OPACITY
        });
    // **The lamp dot, tucked against the glyph's corner** — and it survives
    // the collapse, which is the whole reason it is stacked on the glyph
    // rather than set after the word: collapsed there is no word to set it
    // after, and *is anything on?* is precisely the question a 96 px lane has
    // to keep answering.
    // Collapsed, the glyph is the only thing in the row, so it centres on the
    // lane's own axis — the same axis the sleeves below it centre on. Open, it
    // stands at the left of its box with the word after it.
    let glyph_x = if open {
        alignment::Horizontal::Left
    } else {
        alignment::Horizontal::Center
    };
    let boxed = |content: Element<'static, Message>, x| {
        container(content)
            .width(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
            .height(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
            .align_x(x)
    };
    let mut glyph_box = boxed(mark.into(), glyph_x).align_y(alignment::Vertical::Center);
    // Collapsed, the place you are in is marked by a card **the size of the
    // glyph**, not by a band across the rail — see [`theme::lane_current`] for
    // why the band was wrong and why it carries no border.
    if here && !open {
        glyph_box = glyph_box.style(move |_theme| theme::lane_current(room));
    }
    let glyph_block: Element<'static, Message> = if to == Destination::NowPlaying && sounding {
        iced::widget::stack![
            glyph_box,
            boxed(lamp_dot(), alignment::Horizontal::Right).align_y(alignment::Vertical::Top),
        ]
        .into()
    } else {
        glyph_box.into()
    };
    let mut line = row![glyph_block]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::Center);
    if open {
        line = line.push(
            text(to.label())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .color(if here { room.paper } else { room.paper_dim })
                .wrapping(text::Wrapping::None),
        );
    }
    let row_button = button(
        container(line)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(glyph_x)
            .align_y(alignment::Vertical::Center)
            .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::SIDEBAR_DEST_H))
    // Collapsed, the row's flanks go: the glyph centres on the lane's own
    // axis, which is the axis the sleeves below it centre on, and a rail
    // whose head and body stood on two different verticals would read as two
    // surfaces.
    .padding(theme::pad(0.0, if open { theme::GAP_SM } else { 0.0 }))
    // The row family, on the lane's own ground: one step up under the
    // pointer, the whole hit area painted (`theme::track_row`).
    // `here` only carries the row's card while there is a word in the row to
    // stand in it; collapsed the mark is the glyph's own box, above.
    .style(move |_theme, status| theme::track_row(room, room.recess, status, here && open))
    .on_press(Message::GoTo(to));
    if open {
        return row_button.into();
    }
    // Collapsed, the word is the tooltip — the icon-only law (doc 10 §3.1):
    // a control with no visible label carries its name where a pointer can
    // find it.
    iced::widget::tooltip(
        row_button,
        text(to.label())
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Right,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// The lamp dot on `Now playing`: [`theme::DOT`], the accent, and nothing
/// else.
///
/// the product's amber entry is what licenses it — the lamp states what
/// is true about playback *right now*, and "something is sounding" is exactly
/// that fact. It is not lit by hover, by the queue holding music, or by the
/// place being on screen.
fn lamp_dot() -> Element<'static, Message> {
    let room = theme::active();
    container(
        Space::new()
            .width(Length::Fixed(theme::DOT))
            .height(Length::Fixed(theme::DOT)),
    )
    .style(move |_theme| theme::lamp_dot(room))
    .into()
}

/// The lane's one mixed `RECENT` body, inside its one scroller.
///
/// Playlists and records use the same row and the same most-recent-touch order;
/// the sleeve and second line still say which kind a row is. With no rows the
/// heading is absent, so an empty history remains an honestly empty lane.
fn sections<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    lane: &'a crate::lane::Lane,
    open: bool,
    sounding_row: Option<Subject>,
) -> iced::widget::Column<'a, Message> {
    if lane.rows.is_empty() {
        return column![];
    }
    let mut body = column![heading("RECENT", open)];
    for entry in &lane.rows {
        // **Which row is sounding** — doc 13 §2.6's claim, delivered.
        //
        // A list is never "the sounding record" however many of its tracks
        // are in the run: the fact is about a record, and a list that lit
        // because one of its members was playing would be the
        // invisible-pool posture in a sleeve. **That argument is kept and
        // it is still true — but it is about a list lighting
        // *incidentally*, and it does not reach the case where the list is
        // what the listener put on.** The owner: *"I still see albums
        // specifically appearing as if they are playing rather than the
        // playlist … in a sense we need to track which playlist + track is
        // playing"*.
        //
        // So the mark follows the **run's origin**
        // ([`crate::lane::sounding_subject`], which is the same call the
        // recency ordering makes, so the two cannot disagree): a run
        // reified from a list marks the list and none of its records; every
        // other run marks the record, as before. Nothing lights
        // incidentally either way — a list only ever marked because it *is*
        // the run, which is the most direct fact available rather than a
        // guess from membership.
        //
        // One origin still marks one row.
        let sounds = sounding_row.is_some() && sounding_row == Some(entry.subject);
        body = body.push(lane_row(shelf, playlists, entry, open, sounds));
    }
    body
}

/// A section's word, in the room's quietest voice.
///
/// Absent when the lane is collapsed: at 96 px there is no measure for a
/// tracked word, and a heading over an unlabelled column of sleeves would be
/// naming nothing the eye can use. The tooltips name every mixed row while the
/// lane is collapsed, so no second kind mark is needed.
fn heading(word: &'static str, open: bool) -> Element<'static, Message> {
    let room = theme::active();
    if !open {
        return Space::new().height(Length::Fixed(theme::GAP_SM)).into();
    }
    container(
        text(theme::tracked(word))
            .size(theme::SIZE_HEADING)
            .line_height(theme::LEADING_HEADING)
            .font(theme::MEDIUM)
            .color(room.paper_faint),
    )
    .padding(theme::pad(0.0, theme::GAP_SM))
    .into()
}

/// One row of the lane: a sleeve, and — expanded — the name over one quiet
/// line.
///
/// **Nothing is drawn *on* a row to mark its kind** — no badge, no glyph, no
/// corner — because a mark you must learn is not a hierarchy, and because
/// nothing is ever drawn on top of a sleeve (ADR-0024 §A3.3). What marks it
/// is **the line under the name**: an artist's name for a found thing, and
/// `Playlist · 14 · 42:10` for a made one
/// ([`PanelRow::counts`](crate::playlists::PanelRow::counts), ADR-0024
/// §A3.1). Same widget, same size, same ink — a different string, which is
/// what lets a mixed list stay one list rather than becoming two lists
/// sharing a column.
///
/// **This corrects what stood here.** The comment used to say that nothing
/// marks the kind *because the sleeve already does*: a record wears its
/// cover, a playlist wears the 2 × 2 collage of the records it quotes
/// (ADR-0024 §A1, ADR-0030 §2). That premise is **false for every playlist of
/// one to three distinct records** — below four, `views::playlist_sleeve`
/// draws the first record's cover full-bleed through the same `sleeve_cell`
/// this row builds for a record, from the same cache at the same edge. It is
/// therefore false for every playlist `Save as playlist` makes from a CD (one
/// record, by construction) and for every list on its way to four. The
/// collage stays exactly as §A1 designed it and stays *a* signal; it is no
/// longer *the* signal (§A3.2).
fn lane_row<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    entry: &'a Touched,
    open: bool,
    playing: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    // **[`theme::SIDEBAR_SLEEVE`] 48 in both states**, and that is a
    // correction: this read `PANEL_SLEEVE` 40 when open, which drew the
    // *panel's* sleeve in the lane and left [`theme::SIDEBAR_ROW_H`]'s own
    // derivation — 48 with one `GAP_SM` above and below — describing a row
    // that was not being drawn. Doc 13 §9.2's window drawing states 48 in the
    // expanded lane; `docs/design/impl/lane-and-home/01-lane-open-1280.png`
    // measures 40.
    let edge = theme::SIDEBAR_SLEEVE;
    let sleeve: Element<'a, Message> = match entry.subject {
        Subject::Record(id) => match shelf.thumb(id) {
            Some(handle) => iced_image(handle.clone())
                .width(Length::Fixed(edge))
                .height(Length::Fixed(edge))
                .into(),
            None => crate::views::gradient_block(id, edge, 1.0),
        },
        // The list's sleeve is the panel's own — the same 2 × 2 collage of
        // the records it quotes, from the same cache (ADR-0024 §A1) — so a
        // list cannot look like two different objects in two surfaces.
        Subject::Playlist(id) => {
            let art = playlists.row(id).map_or(&[][..], |entry| &entry.art);
            playlist_sleeve(shelf, art, &entry.name, edge)
        }
    };
    let press = match entry.subject {
        Subject::Record(id) => Message::AlbumClicked(id),
        Subject::Playlist(id) => Message::OpenPlaylist(id),
    };
    // **The lamp owns the far trailing six pixels in every expanded row.** It
    // used to be conditionally inserted before the title, so starting a run
    // shifted the name right and switching its origin shifted two rows. A
    // permanent trailing slot makes playback a change of ink, not geometry.
    // Both text lines share the 146 px boundary in `SIDEBAR_ROW_TEXT_W` and
    // are fitted with the actual bundled face before clipping, so a long album
    // or playlist name yields with an ellipsis instead of wrapping under it.
    let lamp: Element<'static, Message> = if playing {
        lamp_dot()
    } else {
        Space::new()
            .width(Length::Fixed(theme::SIDEBAR_LAMP_SLOT_W))
            .height(Length::Fixed(theme::DOT))
            .into()
    };
    let lamp_slot = container(lamp)
        .width(Length::Fixed(theme::SIDEBAR_LAMP_SLOT_W))
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Right)
        .align_y(alignment::Vertical::Center);
    let body: Element<'a, Message> = if open {
        row![
            sleeve,
            container(
                column![
                    lane_line(
                        &entry.name,
                        &*LANE_MEDIUM,
                        theme::SIZE_BODY,
                        theme::LEADING_BODY,
                        theme::LINE_BODY,
                        theme::MEDIUM,
                        room.paper,
                    ),
                    lane_line(
                        &entry.under,
                        &*LANE_REGULAR,
                        theme::SIZE_META,
                        theme::LEADING_META,
                        theme::LINE_META,
                        theme::SANS,
                        room.paper_faint,
                    ),
                ]
                .spacing(theme::GAP_XXS)
            )
            .width(Length::Fixed(theme::SIDEBAR_ROW_TEXT_W))
            .clip(true),
            lamp_slot,
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        container(sleeve)
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .into()
    };
    let row_button = button(
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center)
            .clip(true),
    )
    .width(Length::Fill)
    .height(Length::Fixed(theme::SIDEBAR_ROW_H))
    .padding(theme::pad(0.0, if open { theme::GAP_SM } else { 0.0 }))
    // The card the sounding row keeps whatever the pointer is doing — and the
    // one mark that survives the collapse, where there is no name to set a dot
    // before and 96 px still has to answer *which of these is on?*
    .style(move |_theme, status| theme::track_row(room, room.recess, status, playing))
    .on_press(press);
    if open {
        return crate::menu::selection_cursor(row_button);
    }
    // Collapsed the sleeve is the only identification, so the name is the
    // tooltip — the same clause that names the head's glyphs.
    crate::menu::selection_cursor(
        iced::widget::tooltip(
            row_button,
            text(entry.name.clone())
                .size(theme::SIZE_CAPTION)
                .line_height(theme::LEADING_CAPTION),
            iced::widget::tooltip::Position::Right,
        )
        .gap(theme::GAP_XS)
        .padding(theme::GAP_XS)
        .style(move |_theme| theme::tooltip(room)),
    )
}

/// One fixed-height expanded-lane line. When it is long, the prefix and the
/// ellipsis occupy separate clipped subslots: Iced 0.13 can still break
/// `Wrapping::None` text, so relying on one text widget can put the ellipsis on
/// an invisible second line.
fn lane_line<'a>(
    content: &str,
    face: &impl Font,
    size: f32,
    leading: f32,
    line_height: f32,
    font: iced::Font,
    color: iced::Color,
) -> Element<'a, Message> {
    let (fitted, truncated) = fit_lane_line(content, face, size);
    let prefix = container(
        text(fitted)
            .size(size)
            .line_height(leading)
            .font(font)
            .color(color)
            .wrapping(text::Wrapping::None),
    )
    .width(if truncated {
        Length::Fixed(theme::SIDEBAR_ROW_TEXT_W - theme::SIDEBAR_ELLIPSIS_SLOT_W)
    } else {
        Length::Fill
    })
    .height(Length::Fixed(line_height))
    .clip(true);
    let ending: Element<'a, Message> = if truncated {
        container(
            text("…")
                .size(size)
                .line_height(leading)
                .font(font)
                .color(color)
                .wrapping(text::Wrapping::None),
        )
        .width(Length::Fixed(theme::SIDEBAR_ELLIPSIS_SLOT_W))
        .height(Length::Fixed(line_height))
        .align_x(alignment::Horizontal::Right)
        .clip(true)
        .into()
    } else {
        Space::new().width(Length::Fixed(0.0)).into()
    };

    container(row![prefix, ending])
        .width(Length::Fixed(theme::SIDEBAR_ROW_TEXT_W))
        .height(Length::Fixed(line_height))
        .clip(true)
        .into()
}

/// Fit the prefix of one expanded-lane line using the same face and size the
/// widget draws. The ellipsis itself owns a separate slot in [`lane_line`].
fn fit_lane_line(text: &str, face: &impl Font, size: f32) -> (String, bool) {
    if text_width(face, size, text) <= theme::SIDEBAR_ROW_TEXT_W {
        return (text.to_owned(), false);
    }

    let scaled = face.as_scaled(PxScale::from(size));
    let prefix_w = theme::SIDEBAR_ROW_TEXT_W - theme::SIDEBAR_ELLIPSIS_SLOT_W;
    let mut fitted = String::new();
    let mut width = 0.0;
    let mut previous = None;
    for character in text.chars() {
        let glyph = scaled.glyph_id(character);
        let next =
            width + previous.map_or(0.0, |was| scaled.kern(was, glyph)) + scaled.h_advance(glyph);
        if next > prefix_w {
            break;
        }
        fitted.push(character);
        width = next;
        previous = Some(glyph);
    }
    (fitted, true)
}

fn text_width(face: &impl Font, size: f32, text: &str) -> f32 {
    let scaled = face.as_scaled(PxScale::from(size));
    let mut width = 0.0;
    let mut previous = None;
    for character in text.chars() {
        let glyph = scaled.glyph_id(character);
        if let Some(was) = previous {
            width += scaled.kern(was, glyph);
        }
        width += scaled.h_advance(glyph);
        previous = Some(glyph);
    }
    width
}

/// **One control at the lane's foot**: `Collapse` when the lane is open,
/// the chevron alone when it is not.
///
/// It shipped briefly as *two* marks in the density detents' anatomy — one
/// per state, the current one inert — and the owner read the built thing and
/// said it was not the design. He is right, and the two cases are not alike:
/// density has **four** named steps, so marks that name each one are the
/// only way to say which you are on. The lane has two states, and the one you
/// are not in is fully described by the word for getting there. A pair of
/// marks for a binary is a radio group where a switch belongs — and it made
/// the lane's own state something you had to read two glyphs to learn.
///
/// So: a labelled word while there is room for a word, the chevron alone when
/// there is not, and the chevron always points **the way the lane will move**.
///
/// Below [`theme::SIDEBAR_FLOOR`] it is inert and drawn at the disabled ink:
/// expanding there would leave the collection one column of covers, and a
/// control that produces a state the window cannot hold is a trap.
fn marks(open: bool, can_expand: bool) -> Element<'static, Message> {
    container(lane_toggle(open, can_expand))
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        // Separation belongs above the footer. The lane's outer bottom lead
        // supplies the space below it, so symmetric padding here only reduced
        // the scrolling viewport.
        .padding(iced::Padding {
            top: theme::GAP_MD,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        })
        .into()
}

/// [`marks`]' one control.
fn lane_toggle(open: bool, can_expand: bool) -> Element<'static, Message> {
    let room = theme::active();
    // The chevron points the way the lane will move: closing, it points at
    // the edge it is heading for; opening, away from it.
    let glyph = if open {
        icon::Glyph::LaneCollapsed
    } else {
        icon::Glyph::LaneExpanded
    };
    let usable = open || can_expand;
    let opacity = if usable {
        theme::GLYPH_OPACITY
    } else {
        theme::GLYPH_OPACITY_DISABLED
    };
    let mark = iced_image(icon::handle(glyph))
        .width(Length::Fixed(theme::ICON_PX))
        .height(Length::Fixed(theme::ICON_PX))
        .opacity(opacity);
    // The word rides with the glyph while the lane is open; collapsed, the
    // lane is 96 px and the glyph stands alone under its tooltip, exactly as
    // the destinations above it do.
    let body: Element<'static, Message> = if open {
        row![
            mark,
            text("Collapse")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        mark.into()
    };
    let boxed = container(body)
        .height(Length::Fixed(theme::STEPPER_HIT))
        .align_y(alignment::Vertical::Center);
    if !usable {
        return boxed.into();
    }
    let pressable = button(boxed)
        .height(Length::Fixed(theme::STEPPER_HIT))
        .padding(theme::pad(0.0, theme::GAP_SM))
        .style(move |_theme, status| theme::transport(room, room.recess, status))
        .on_press(Message::ToggleLane);
    if open {
        return pressable.into();
    }
    iced::widget::tooltip(
        pressable,
        text("Expand")
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Right,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

#[cfg(test)]
mod tests {
    use crate::theme;

    /// This file's own source, for the pins below.
    fn source() -> String {
        include_str!("lane.rs").replace("\r\n", "\n")
    }

    /// One function's body, by name.
    fn body(source: &str, signature: &str) -> String {
        let rest = source
            .split_once(signature)
            .unwrap_or_else(|| panic!("`{signature}` exists"))
            .1;
        rest[..rest.find("\n}\n").expect("a function ends")].to_owned()
    }

    /// **A lane row's sleeve is [`theme::SIDEBAR_SLEEVE`] in both states.**
    ///
    /// It was [`theme::PANEL_SLEEVE`] 40 when open, which drew the *panel's*
    /// sleeve in the lane and left [`theme::SIDEBAR_ROW_H`]'s derivation — 48
    /// with one `GAP_SM` above and below — describing a row nothing was drawing.
    #[test]
    fn a_lane_rows_sleeve_is_the_lanes_own_size() {
        let source = source();
        let row = body(&source, "fn lane_row<'a>(");
        assert!(
            row.contains("let edge = theme::SIDEBAR_SLEEVE;"),
            "the lane's sleeve is resolved per state again"
        );
        assert!(
            !row.contains("theme::PANEL_SLEEVE"),
            "the lane draws the playlist panel's sleeve"
        );
    }

    /// **The lists have a section, it stands above `RECENT`, and both are
    /// inside one scroller.**
    ///
    /// Playlists and records are one recency list: one heading, one scroller,
    /// and no kind-specific block to push the other kind away.
    #[test]
    fn playlists_and_records_share_one_recent_section_and_one_scroller() {
        let source = source();
        let shipped = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        let sections = body(&source, "fn sections<'a>(");
        assert!(
            sections.contains("heading(\"RECENT\", open)") && !sections.contains("PLAYLISTS"),
            "the lane split playlists back into a separate area"
        );
        assert!(
            sections.contains("if lane.rows.is_empty()"),
            "an empty mixed history still draws a heading"
        );
        assert_eq!(
            shipped.matches("scrollable(").count(),
            1,
            "the mixed lane grew a second scroll position"
        );
        let view = body(&source, "pub(crate) fn view<'a>(");
        assert!(
            view.contains("scrollable(body_rows.padding("),
            "the mixed rows are no longer the scroller's content"
        );
    }

    #[test]
    fn the_collapse_footer_spends_its_space_above_the_control() {
        let source = source();
        let view = body(&source, "pub(crate) fn view<'a>(");
        let marks = body(&source, "fn marks(open: bool, can_expand: bool)");
        assert!(
            view.contains("bottom: theme::GAP_MD"),
            "the lane kept its old oversized bottom gutter"
        );
        assert!(
            marks.contains("top: theme::GAP_MD") && marks.contains("bottom: 0.0"),
            "the collapse footer still pads equally above and below itself"
        );
    }

    /// **The sounding record is marked in the lane** — doc 13 §2.6's claim,
    /// which the shipped lane did not keep: every row drew
    /// [`theme::track_row`] with `playing` hard-coded `false`, so the surface
    /// whose whole subject is *things you have touched* could not say which of
    /// them was on.
    ///
    /// The trailing dot and the row's card, which is the **row's** vocabulary
    /// — what the queue and a playlist's page already draw — rather
    /// than the tile's halo, which would want the lamp's clock in a surface
    /// ADR-0030 §4 costs at zero idle CPU.
    ///
    /// **Which row, though, is not asserted here**, and the split is the
    /// point. *A playlist's run marks the list and not the records it quotes*
    /// is a **behavioural** claim, and it is pinned as one, over real values,
    /// in `lane.rs`'s `the_sounding_row_is_the_list_when_a_list_is_what_was_put_on`
    /// and `only_one_row_is_ever_marked`. A source scan cannot tell a correct
    /// rewrite from a broken one, so it must not be what guards a rule.
    ///
    /// What a source scan *can* say, and what is load-bearing enough to keep:
    /// **the view does not re-derive the answer.** The rule lives in
    /// `lane::sounding_subject` — the same call the recency ordering makes —
    /// and a view that went back to matching on the sounding file's record
    /// would be correct-looking, would pass every behavioural test in
    /// `lane.rs`, and would put the dot and the order back to reading two
    /// separate answers to one question. That is a fact about *where the code
    /// is*, which is the only kind of fact this form is good for. The
    /// remaining assertions are the row's own drawing, which builds iced
    /// widgets and so has nothing else to be asserted against.
    #[test]
    fn the_sounding_record_is_the_marked_row() {
        let source = source();
        // The marking moved into `sections` when the lists got a section of
        // their own, so that is what this scans — a test still reading `view`
        // would pass by looking at a function the rule left.
        let sections = body(&source, "fn sections<'a>(");
        assert!(
            sections.contains("sounding_row == Some(entry.subject)"),
            "the lane no longer marks the row `lane::sounding_subject` named"
        );
        assert!(
            !sections.contains("Subject::Record(id) if Some(id) =="),
            "the lane derived the sounding row from the sounding file's record \
             again, instead of taking `lane::sounding_subject`'s answer — the \
             dot and the recency order are now free to disagree"
        );
        assert!(
            !sections.contains("Subject::Playlist(") && !sections.contains("played_list"),
            "the lane re-derived which row is sounding rather than being handed \
             it; the rule belongs in `lane::sounding_subject`"
        );
        let row = body(&source, "fn lane_row<'a>(");
        assert!(
            row.contains("let lamp: Element<'static, Message> = if playing")
                && row.contains("lamp_dot()")
                && row.contains("lamp_slot"),
            "the sounding row lost its stable trailing lamp slot"
        );
        assert!(
            row.contains("theme::track_row(room, room.recess, status, playing)"),
            "the sounding row lost the card that survives the collapse"
        );
    }

    /// The owner's no-reflow rule is geometry first: every open row reserves
    /// the same far-trailing slot, and both one-line strings stop at the same
    /// boundary whether this row is sounding or quiet.
    #[test]
    fn an_expanded_recent_row_reserves_one_trailing_lamp_slot() {
        const {
            assert!(theme::SIDEBAR_LAMP_SLOT_W == theme::DOT);
            assert!(theme::SIDEBAR_ROW_TEXT_W == 146.0);
        }

        let source = source();
        let row = body(&source, "fn lane_row<'a>(");
        assert_eq!(
            row.matches("lamp_slot").count(),
            2,
            "the slot should be built once and placed once"
        );
        assert!(
            row.contains("Length::Fixed(theme::SIDEBAR_ROW_TEXT_W)")
                && row.matches("lane_line(").count() == 2,
            "title and metadata no longer share the stable trailing boundary"
        );
        assert!(
            !row.contains("if playing && open"),
            "playback still changes the expanded row's child geometry"
        );
    }

    #[test]
    fn the_edge_scrollbar_cannot_clip_the_recents_trailing_lamp() {
        const {
            assert!(
                theme::SIDEBAR_W
                    - theme::WALL_SCROLLBAR_W
                    - theme::GAP_XL
                    - (theme::GAP_XL - theme::WALL_SCROLLBAR_W)
                    == theme::SIDEBAR_MEASURE
            );
        }
        let source = source();
        let head = source.split("#[cfg(test)]").next().expect("a head");
        assert!(
            head.contains("right: theme::GAP_XL - theme::WALL_SCROLLBAR_W")
                && head.contains("left: theme::GAP_XL"),
            "the lane's scrollbar has reclaimed the Recent lamp's measured slot"
        );
    }

    #[test]
    fn long_album_and_playlist_lines_end_in_a_measured_ellipsis() {
        let album = "A record title deliberately much longer than the returns lane can ever hold";
        let playlist =
            "Playlist · 12345 · a deliberately impossible amount of metadata for one row";
        let short = "Ochre";
        // These are measurement inputs; the corresponding drawn calls above
        // supply the leading named beside each size.
        let body_size = theme::SIZE_BODY; // drawn with theme::LEADING_BODY
        let meta_size = theme::SIZE_META; // drawn with theme::LEADING_META

        let (album, album_truncated) = super::fit_lane_line(album, &*super::LANE_MEDIUM, body_size);
        let (playlist, playlist_truncated) =
            super::fit_lane_line(playlist, &*super::LANE_REGULAR, meta_size);
        assert!(album_truncated);
        assert!(playlist_truncated);
        let prefix_w = theme::SIDEBAR_ROW_TEXT_W - theme::SIDEBAR_ELLIPSIS_SLOT_W;
        assert!(super::text_width(&*super::LANE_MEDIUM, body_size, &album) <= prefix_w);
        assert!(super::text_width(&*super::LANE_REGULAR, meta_size, &playlist) <= prefix_w);
        assert_eq!(
            super::fit_lane_line(short, &*super::LANE_MEDIUM, body_size),
            (short.to_owned(), false)
        );
    }
}
