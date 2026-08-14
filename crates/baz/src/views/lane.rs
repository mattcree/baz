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

use iced::widget::{
    Space, button, column, container, image as iced_image, row, rule, scrollable, text,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::lane::{Destination, Subject, Touched};
use crate::place::Place;
use crate::playlists::Playlists;
use crate::views::playlist_sleeve;
use crate::{icon, theme};

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

    // The rows fill the lane's content box, open and collapsed, so the column
    // has to offer them its whole width rather than shrinking to the widest
    // word.
    let mut head = column![].width(Length::Fill);
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
    // The rows still need their whole 216 px — sleeve 48, two `GAP_SM` seams,
    // the 146 px text lane, the lamp's six — so only the edge-side pad yields
    // those pixels. Keeping the leading pad at 8 preserves the head and row
    // alignment; shrinking both would merely move the clip to the other side.
    let list_pad = iced::Padding {
        top: 0.0,
        right: theme::SIDEBAR_PAD - theme::WALL_SCROLLBAR_W,
        bottom: 0.0,
        left: theme::SIDEBAR_PAD,
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
            .padding(theme::pad(0.0, theme::SIDEBAR_PAD))
    };
    let body = column![
        flanked(head.into()),
        // The head's one rule: four destinations above it, the things you
        // have touched below. **The lane still has exactly one seam**, and
        // that is the point of drawing the sections' headings rather than a
        // second rule: a heading names a section, a rule cuts the surface, and
        // there is one cut here because there are two parts — the frame's
        // concerns, and yours.
        container(rule::horizontal(1).style(move |_theme| theme::hairline(room, room.recess)))
            .padding(theme::pad(theme::GAP_MD, theme::SIDEBAR_PAD)),
        list,
        // **The footer reads the *resolved* state, not the persisted intent.**
        // It was handed the persisted intent, so a window narrowed below
        // [`theme::SIDEBAR_FLOOR`] with the lane remembered open drew a 64 px
        // rail whose foot still carried the open-state control — the
        // `LaneCollapsed` chevron and the word `Collapse`, live and pressable,
        // inside a rail with no measure for either. Every other part of the
        // lane already drew from `open`; this was the one that did not.
        //
        // The intent itself is deliberately *kept* — widening the window
        // restores the open lane rather than making the listener ask twice —
        // which is exactly why the footer has to read the resolved state:
        // the remembered value is a wish, and the foot of the lane states
        // what the lane *is*. Below the floor that is the inert `Expanded`
        // mark (ADR-0030 §3), the branch `lane_toggle` has always had and in
        // this state never reached.
        flanked(marks(open, theme::sidebar_can_expand(window_w))),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // The lane's one lead is `SIDEBAR_PAD` all round, and the collapse footer
    // spends its own `GAP_MD` above the control — the old 24 px outer gutter
    // combined with the collapse control's own 12 px padding into 36 px of
    // dead space at the bottom, paid for by the scrollable list above it.
    let lane = container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: theme::SIDEBAR_PAD,
            right: 0.0,
            bottom: theme::SIDEBAR_PAD,
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

/// One of the head's four destinations: the tile and, expanded, its word, in
/// **one row** — the control is the row, so the card (`theme::dest_row`)
/// highlights the icon and the word together, hovered and selected alike.
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
        .width(Length::Fixed(theme::SIDEBAR_GLYPH_PX))
        .height(Length::Fixed(theme::SIDEBAR_GLYPH_PX))
        .opacity(if here {
            theme::GLYPH_OPACITY_HOVER
        } else {
            theme::GLYPH_OPACITY
        });
    // **The tile holds the glyph centred** — [`theme::SIDEBAR_GLYPH_PX`] 32 in
    // the 48 square, a [`theme::GAP_SM`] of air all round. Open and collapsed
    // the tile starts on the same vertical; the collapse removes the word,
    // never the tile.
    let boxed = |content: Element<'static, Message>, x| {
        container(content)
            .width(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
            .height(Length::Fixed(theme::SIDEBAR_GLYPH_BOX))
            .align_x(x)
    };
    let tile =
        boxed(mark.into(), alignment::Horizontal::Center).align_y(alignment::Vertical::Center);
    // **The lamp dot, tucked against the *glyph's* top-right corner** — and it
    // survives the collapse, which is the whole reason it is stacked on the
    // tile rather than set after the word: collapsed there is no word to set
    // it after, and *is anything on?* is precisely the question a 64 px lane
    // has to keep answering.
    //
    // **The corner it tucks against is the mark's, not the box's**, and that
    // is a correction. The dot was pinned to the container's own top-right,
    // which was right while the box was the glyph's own 24 — then the box
    // became the 48 px tile with a 32 px glyph centred in it, and the same
    // line started putting the dot in the corner of an invisible square with
    // a whole [`theme::GAP_SM`] of air diagonally between it and any ink. The
    // owner read it as *"the pip when Now playing is active is in a strange
    // position"*, which is exactly what it is: an accent floating in a corner
    // that is not a corner of anything drawn.
    //
    // The inset is **derived from the tile and the glyph** rather than written
    // as 8, so the next change to either size carries the dot with it instead
    // of stranding it again.
    let tile_block: Element<'static, Message> = if to == Destination::NowPlaying && sounding {
        iced::widget::stack![
            tile,
            boxed(lamp_dot(), alignment::Horizontal::Right)
                .align_y(alignment::Vertical::Top)
                .padding(theme::SIDEBAR_GLYPH_INSET),
        ]
        .into()
    } else {
        tile.into()
    };
    // **The row is the control**: the tile and, expanded, the word on the
    // lane's one `GAP_SM` seam — the seam the `RECENT` rows' words stand on,
    // since the tile is the sleeve's size, so every word in the lane shares
    // one vertical ([`theme::SIDEBAR_HEAD_TEXT_X`]). One button holds both, so
    // `theme::dest_row`'s card is a single highlight across them — the owner's
    // *"the full row with icon and text should appear highlighted together"* —
    // and the row's vertical centre is the tile's, so the word reads as
    // standing level with the icon, never hung above or below it.
    let content: Element<'static, Message> = if open {
        row![
            tile_block,
            text(to.label())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(theme::GAP_SM)
        .align_y(alignment::Vertical::Center)
        .into()
    } else {
        tile_block
    };
    // The card is the **row**, so it spans the lane's content box rather than
    // shrinking to the word: at shrink width the four destinations wore four
    // different cards and the widest of them cut through its own last letter,
    // because a button sized to a `Wrapping::None` text has no room to spare.
    // Filling also makes the head's card the same object as a `RECENT` row's,
    // which is what "highlighted together" has to mean in a list.
    let dest = button(content)
        .width(Length::Fill)
        .height(Length::Fixed(theme::SIDEBAR_DEST_H))
        .padding(0)
        .style(move |_theme, status| theme::dest_row(room, here, status))
        .on_press(Message::GoTo(to));
    if !open {
        // Collapsed, the word is the tooltip — the icon-only law (doc 10
        // §3.1): a control with no visible label carries its name where a
        // pointer can find it.
        return iced::widget::tooltip(
            dest,
            text(to.label())
                .size(theme::SIZE_CAPTION)
                .line_height(theme::LEADING_CAPTION),
            iced::widget::tooltip::Position::Right,
        )
        .gap(theme::GAP_XS)
        .padding(theme::GAP_XS)
        .style(move |_theme| theme::tooltip(room))
        .into();
    }
    dest.into()
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

/// The lane's one mixed body of touched things, inside its one scroller.
///
/// Playlists and records use the same row and the same most-recent-touch order;
/// the sleeve and second line still say which kind a row is.
///
/// # There is no `RECENT` heading any more
///
/// ADR-0030 drew one because the lane has two parts — the frame's concerns and
/// yours — but **the head's one rule is what cuts them**, and it already does.
/// A heading over the lane's only section names nothing the eye can use: there
/// is no second section for it to distinguish this one from, so it was a word
/// spent saying *the rest of this surface*. The owner: *"can you remove
/// 'Recent' from the sidebar when it is not collapsed"* — and *not collapsed*
/// was the only state it ever appeared in, so the ask is to drop it outright.
/// Removing it also returns a whole heading line box to the scroller, which is
/// the surface in the lane that is always short of one.
fn sections<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    lane: &'a crate::lane::Lane,
    open: bool,
    sounding_row: Option<Subject>,
) -> iced::widget::Column<'a, Message> {
    // An empty history draws an empty column — there is no longer a heading
    // that would otherwise stand over nothing, so the emptiness needs no
    // special case to stay honest.
    let mut body = column![];
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

/// **The far trailing six pixels of every expanded `RECENT` row**, occupied by
/// the lamp when the row is sounding and reserved when it is not.
///
/// # The slot
///
/// The dot used to be conditionally inserted *before* the title, so starting a
/// run shifted the name right and switching its origin shifted two rows. A
/// permanent trailing slot makes playback a change of ink, not geometry — the
/// owner's *"we don't want reflowing text"*, and ADR-0030's 2026-08-12
/// amendment. That is unchanged and is why the reservation is drawn in the
/// quiet state too.
///
/// # Which line it stands on
///
/// What the amendment never settled was *which of the row's two text lines the
/// dot is level with*, and the answer it inherited was neither: the slot was
/// centred against a `Length::Fill` height inside a row whose other children
/// are centred, so it landed on the **two-line block's** centre, which is the
/// [`theme::GAP_XXS`] seam between the title and the metadata. The owner: *"the
/// now playing pip on the recent list is also in a strange position"*.
///
/// The line it belongs on is the one it is a fact about — the **name** of the
/// thing that is sounding. So the slot is built as the text column's own
/// shape: the dot centred in a [`theme::LINE_BODY`] box, and under it exactly
/// the seam and metadata line the title has under it. Both columns are then
/// the same height, and the row's `Center` alignment lands the dot on the
/// title's centre *by construction* — there is no offset for a later edit to
/// leave stale, which is precisely how it drifted the first time.
fn lamp_slot(playing: bool) -> Element<'static, Message> {
    let lamp: Element<'static, Message> = if playing {
        lamp_dot()
    } else {
        Space::new()
            .width(Length::Fixed(theme::SIDEBAR_LAMP_SLOT_W))
            .height(Length::Fixed(theme::DOT))
            .into()
    };
    column![
        container(lamp)
            .width(Length::Fixed(theme::SIDEBAR_LAMP_SLOT_W))
            .height(Length::Fixed(theme::LINE_BODY))
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Center),
        Space::new()
            .width(Length::Fixed(theme::SIDEBAR_LAMP_SLOT_W))
            .height(Length::Fixed(theme::LINE_META)),
    ]
    .spacing(theme::GAP_XXS)
    .width(Length::Fixed(theme::SIDEBAR_LAMP_SLOT_W))
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
    let lamp_slot = lamp_slot(playing);
    let body: Element<'a, Message> = if open {
        row![
            sleeve,
            container(
                column![
                    lane_line(&crate::views::Fitted {
                        content: &entry.name,
                        face: &crate::views::FIT_MEDIUM,
                        size: theme::SIZE_BODY,
                        leading: theme::LEADING_BODY,
                        line_height: theme::LINE_BODY,
                        font: theme::MEDIUM,
                        color: room.paper,
                        measure: theme::SIDEBAR_ROW_TEXT_W,
                    }),
                    lane_line(&crate::views::Fitted {
                        content: &entry.under,
                        face: &crate::views::FIT_REGULAR,
                        size: theme::SIZE_META,
                        leading: theme::LEADING_META,
                        line_height: theme::LINE_META,
                        font: theme::SANS,
                        color: room.paper_faint,
                        measure: theme::SIDEBAR_ROW_TEXT_W,
                    }),
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
            .align_x(alignment::Horizontal::Left)
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
    // The flank is the lane's own [`theme::SIDEBAR_PAD`] (`list_pad` in
    // `view`), so the row spends no padding of its own and the sleeve stands
    // on the same lead open and collapsed — the collapse removes the text
    // column, never the sleeve.
    .padding(0)
    // The card the sounding row keeps whatever the pointer is doing — and the
    // one mark that survives the collapse, where there is no name to set a dot
    // before and 64 px still has to answer *which of these is on?*
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

/// One fixed-height expanded-lane line, fitted with a visible end ellipsis.
///
/// The reading lives in [`crate::views::fitted_line`] now — it began here and
/// the bottom bar's sounding-track lines needed the same thing, so it moved
/// out rather than being copied. This is the lane's name for it, which keeps
/// the row's own composition readable.
fn lane_line(line: &crate::views::Fitted<'_>) -> Element<'static, Message> {
    crate::views::fitted_line(line)
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

    /// **Playlists and records are one list, in one scroller, under no
    /// heading at all.**
    ///
    /// The heading half of this is asserted as an **absence**, deliberately,
    /// and deleting the test instead would have been the wrong move: an
    /// unasserted absence is an invitation, and the next edit that felt the
    /// list wanted introducing would re-add a word over it unchallenged. The
    /// lane's one rule is what separates the frame's concerns from yours;
    /// there is no second section here for a heading to distinguish this one
    /// from, which is why the owner asked for it gone.
    #[test]
    fn playlists_and_records_share_one_unheaded_list_and_one_scroller() {
        let source = source();
        let shipped = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        let sections = body(&source, "fn sections<'a>(");
        assert!(
            !sections.contains("heading(") && !sections.contains("PLAYLISTS"),
            "the lane's one list has grown a heading over it again"
        );
        assert!(
            !shipped.contains("fn heading("),
            "`heading` is unused now that the lane's one list carries no word; \
             leaving it behind is a section waiting to be re-added"
        );
        assert!(
            !shipped.contains("\"RECENT\""),
            "the lane names a section again"
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
            view.contains("bottom: theme::SIDEBAR_PAD"),
            "the lane kept its old oversized bottom gutter"
        );
        assert!(
            marks.contains("top: theme::GAP_MD") && marks.contains("bottom: 0.0"),
            "the collapse footer still pads equally above and below itself"
        );
    }

    /// **The footer states what the lane *is*, not what it was asked to be.**
    ///
    /// Below [`theme::SIDEBAR_FLOOR`] the persisted intent is overruled — the
    /// lane draws its 64 px rail whatever the listener last chose (ADR-0030
    /// §3) — and every part of `view` resolves from that one answer except,
    /// until now, the footer, which was handed `shelf.lane_open` directly. The
    /// visible defect was the owner's: *"when I narrow the window, it force
    /// collapses the sidebar, but it still shows the collapse icon"* — a live,
    /// pressable `Collapse` control, word and all, inside a rail with no
    /// measure for it.
    ///
    /// The intent is still read exactly once, to compute `open`, so widening
    /// the window restores the lane the listener asked for. That is the
    /// behaviour, and it is *why* this assertion exists: as long as the wish
    /// is remembered, the foot of the lane must be drawn from the resolution
    /// of it rather than from the wish.
    #[test]
    fn the_footer_reads_the_resolved_lane_state_not_the_persisted_intent() {
        let source = source();
        let view = body(&source, "pub(crate) fn view<'a>(");
        assert!(
            view.contains("flanked(marks(open, theme::sidebar_can_expand(window_w)))"),
            "the collapse footer is drawn from the persisted intent again, so a \
             force-collapsed rail can carry the open state's control"
        );
        assert!(
            !view.contains("marks(shelf.lane_open"),
            "`marks` is reading `shelf.lane_open` directly"
        );
        assert_eq!(
            view.matches("shelf.lane_open").count(),
            2,
            "the persisted intent should be read only where `open` and `width` \
             are resolved from it — every other part of the lane takes `open`"
        );
    }

    /// **Both of the lane's lamps sit against ink rather than against a box.**
    ///
    /// They are one test because they are one mistake made twice, and both
    /// times by a container quietly changing meaning underneath an alignment
    /// that was correct when it was written.
    ///
    /// * The destination lamp pinned to its container's top-right, which was
    ///   the glyph's own corner while the box was 24 px. The box became the
    ///   48 px tile with a 32 px glyph centred in it, and the dot was left in
    ///   the corner of an invisible square. It is now inset by the tile's own
    ///   air, **derived** from the two sizes so a future glyph change carries
    ///   it along instead of stranding it a third time.
    /// * The `RECENT` lamp centred against a `Length::Fill` height, which put
    ///   it on the two-line block's centre — the [`theme::GAP_XXS`] gap
    ///   between the title and the metadata, level with neither. It now
    ///   carries the text column's own shape, so the row's `Center` alignment
    ///   lands it on the title's centre by construction.
    ///
    /// The trailing *slot* is untouched by either fix and must stay: a
    /// conditional dot before the name reflows the text, which is the whole
    /// reason ADR-0030's 2026-08-12 amendment put it at the far edge.
    #[test]
    fn the_lanes_two_lamps_stand_against_ink() {
        // The tile's air is what the destination dot is inset by, and the
        // text column's shape is what the `RECENT` dot borrows — both are
        // arithmetic on tokens rather than numbers written into the view.
        const {
            assert!(theme::SIDEBAR_GLYPH_BOX > theme::SIDEBAR_GLYPH_PX);
            assert!((theme::SIDEBAR_GLYPH_BOX - theme::SIDEBAR_GLYPH_PX) / 2.0 == theme::GAP_SM);
        }

        let source = source();
        let destination = body(&source, "fn destination_row(");
        assert!(
            destination.contains(".padding(theme::SIDEBAR_GLYPH_INSET)"),
            "the destination lamp is back in the corner of the tile rather than \
             the corner of the mark drawn in it"
        );

        let slot = body(&source, "fn lamp_slot(playing: bool)");
        assert!(
            slot.contains(".height(Length::Fixed(theme::LINE_BODY))")
                && slot.contains(".height(Length::Fixed(theme::LINE_META))")
                && slot.contains(".spacing(theme::GAP_XXS)"),
            "the `RECENT` lamp no longer carries the text column's own shape, so \
             it is free to drift off the title line it is a fact about"
        );
        assert!(
            !slot.contains("Length::Fill"),
            "the `RECENT` lamp is centred against the whole row again, which \
             puts it in the gap between the two text lines"
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
        let slot = body(&source, "fn lamp_slot(playing: bool)");
        assert!(
            row.contains("let lamp_slot = lamp_slot(playing);")
                && slot.contains("if playing")
                && slot.contains("lamp_dot()"),
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
            3,
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

    /// The edge scrollbar spends its lane inside the right pad, so the rows
    /// still get their whole 216 px of open geometry and the lamp's trailing
    /// edge never reaches the scrollbar's lane. In numbers: the padded content
    /// box is `SIDEBAR_W − SIDEBAR_PAD − (SIDEBAR_PAD − WALL_SCROLLBAR_W)`, and
    /// the row's far edge is the sleeve plus both seams, the text lane and the
    /// lamp slot.
    #[test]
    fn the_edge_scrollbar_cannot_clip_the_recents_trailing_lamp() {
        const {
            let content = theme::SIDEBAR_W
                - theme::SIDEBAR_PAD
                - (theme::SIDEBAR_PAD - theme::WALL_SCROLLBAR_W);
            let row = theme::SIDEBAR_SLEEVE
                + theme::GAP_SM
                + theme::SIDEBAR_ROW_TEXT_W
                + theme::GAP_SM
                + theme::SIDEBAR_LAMP_SLOT_W;
            assert!(row <= content);
        }
        let source = source();
        let head = source.split("#[cfg(test)]").next().expect("a head");
        assert!(
            head.contains("right: theme::SIDEBAR_PAD - theme::WALL_SCROLLBAR_W")
                && head.contains("left: theme::SIDEBAR_PAD"),
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

        let (album, album_truncated) = crate::views::fit(
            album,
            &*crate::views::FIT_MEDIUM,
            body_size,
            theme::SIDEBAR_ROW_TEXT_W,
        );
        let (playlist, playlist_truncated) = crate::views::fit(
            playlist,
            &*crate::views::FIT_REGULAR,
            meta_size,
            theme::SIDEBAR_ROW_TEXT_W,
        );
        assert!(album_truncated);
        assert!(playlist_truncated);
        let prefix_w = theme::SIDEBAR_ROW_TEXT_W - theme::ELLIPSIS_SLOT_W;
        assert!(
            crate::views::text_width(&*crate::views::FIT_MEDIUM, body_size, &album) <= prefix_w
        );
        assert!(
            crate::views::text_width(&*crate::views::FIT_REGULAR, meta_size, &playlist) <= prefix_w
        );
        assert_eq!(
            crate::views::fit(
                short,
                &*crate::views::FIT_MEDIUM,
                body_size,
                theme::SIDEBAR_ROW_TEXT_W
            ),
            (short.to_owned(), false)
        );
    }
}
