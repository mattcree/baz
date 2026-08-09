//! **The Now playing place** — the sounding record at the size it deserves.
//!
//! The owner's extension of the returns lane's head: *"as an extension we will
//! want a Now playing page at the top with the Home and Library"*.
//!
//! # Why it is not `Place::Album`
//!
//! Its subject is **what is sounding**, which is the bottom bar's subject on a
//! page. `Place::Album`'s subject is *the record you pointed at*. The two are
//! the same record most of the time and that is exactly why they must be
//! different surfaces: a page that silently changed which record it was about
//! when a track ended would be an album page that navigates itself, and a page
//! that did not follow the music would be a now-playing screen that lies. This
//! one carries no id, for the same reason the bar carries none — the engine's
//! answer is the only one it may draw.
//!
//! # A first version, and what it is designed to become
//!
//! Deliberately simple: the artwork large, the identity under it, the needle
//! and the position, and the transport. No visualizer and no VU — those are
//! future work and are not allowed to constrain this.
//!
//! **The kiosk full-screen mode is this same surface at a larger size**, and
//! that is a property of the composition rather than a plan: every measure
//! here is derived from the viewport by [`art_edge`], so the place at 3840 px
//! is this place with a bigger number in it. `docs/design/12-now-playing-and-kiosk.md`
//! (unfinished, on `design/12-now-playing-kiosk`) argues the *reason* — the
//! surface is read at two distances that do not overlap, the far field wants
//! very few very large statements, and the near field already has the bar,
//! which is in every place. Nothing here forecloses it.
//!
//! # The serif stays on the Home placard
//!
//! The work's title here is set in the sans, not in `theme::WORK_TITLE`. The
//! serif italic is the *museum placard's* convention and there is one placard
//! in the product; a second consumer would be a display face arriving one
//! surface at a time, which is the thing `assets/fonts/README.md` records as
//! deleted and staying deleted. `the_serif_is_the_work_titles_and_nothing_else`
//! holds this to it.

use iced::widget::{Space, column, container, image as iced_image, row, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::theme;
use crate::views::gradient_block;

/// **The artwork's edge**, derived from the viewport and clamped.
///
/// The whole of what makes the kiosk mode this surface at a larger size: the
/// work takes the room it is given, bounded below so it never stops being the
/// subject and above so a 4 K panel does not become one cover and nothing
/// else. The height term is what stops a wide, short window pushing the
/// placard off the bottom — a now-playing screen that has scrolled away from
/// what is playing is not one.
#[must_use]
pub(crate) fn art_edge(width: f32, height: f32) -> f32 {
    // What the placard and the transport under the work need: three lines,
    // the needle, the transport's own hit box, and the gaps between them.
    let below = theme::LINE_HEADING
        + theme::LINE_HERO
        + theme::LINE_BODY
        + theme::NEEDLE_H
        + theme::TRANSPORT_HIT
        + 4.0 * theme::GAP_LG;
    let by_width = width - 2.0 * theme::HANG;
    let by_height = height - 2.0 * theme::HANG - below;
    by_width
        .min(by_height)
        .clamp(theme::ART_MIN, NOW_PLAYING_MAX)
}

/// The largest the artwork is ever drawn: **720**.
///
/// Past this a cover stops gaining anything — the decoded thumbnail is not
/// that large, and a bigger square would be upscaling — and the placard under
/// it would be pushed to the bottom of a very tall column. It is the one
/// number in this file that is chosen rather than derived, and it is chosen
/// against the decode's own ceiling.
pub(crate) const NOW_PLAYING_MAX: f32 = 720.0;

/// The Now playing place's body.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    ink: crate::motion::Ink,
    width: f32,
    height: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let Some(now) = player.now_playing() else {
        // **Nothing is sounding**, said once and plainly. Not an error, not an
        // empty frame with a grey square in it: silence is a feature
        // (`docs/REFUSALS.md`), and a place whose subject is absent says so.
        return container(
            text("Nothing playing.")
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .color(room.paper_faint),
        )
        .center(Length::Fill)
        .into();
    };
    let edge = art_edge(width, height);
    let work: Element<'a, Message> = match now.album_id.and_then(|id| shelf.thumbs.peek(&id)) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .into(),
        // The wall's own deterministic gradient, at this scale — the same
        // stand-in a tile shows, so a record with no cover is the same object
        // here as it is there.
        None => gradient_block(now.album_id.unwrap_or_default(), edge, 1.0),
    };

    // Owned: the two figures are `String`s the reading builds, and a borrow
    // of them cannot outlive this function.
    let stamps = player.stamps();
    let elapsed = player.elapsed_ms();
    let total = player.track_ms().unwrap_or(0);

    let mut placard = column![
        // The artist in letterspaced caps, over the work's title — the wall
        // label's own order, at the far field's scale.
        text(theme::tracked(
            &now.artist
                .clone()
                .or_else(|| now.track_artist.clone())
                .unwrap_or_default()
                .to_uppercase()
        ))
        .size(theme::SIZE_HEADING)
        .line_height(theme::LEADING_HEADING)
        .font(theme::MEDIUM)
        .color(room.paper_faint),
        text(now.title.clone())
            .size(theme::SIZE_HERO)
            .line_height(theme::LEADING_HERO)
            .font(theme::SEMIBOLD)
            .color(room.paper)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XS)
    .width(Length::Fixed(edge));
    if let Some(album) = &now.album {
        placard = placard.push(
            text(album.clone())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None),
        );
    }

    // **The needle, on the placard and at the work's own width** — the Home
    // page's rule, applied at this scale, and for its reason: nothing is drawn
    // on the artwork.
    placard = placard
        .push(Space::with_height(Length::Fixed(theme::GAP_LG)))
        .push(crate::views::home::needle(elapsed, total, edge))
        .push(figures(stamps, room));

    container(
        column![
            work,
            placard,
            crate::views::bottom_bar::transport(player, ink)
        ]
        .spacing(theme::GAP_XL)
        .align_x(alignment::Horizontal::Center),
    )
    .center(Length::Fill)
    .into()
}

/// `3:12` and `6:27`, at the two ends of the needle's own width.
///
/// The bar's own two timestamps, in the bar's own vocabulary: the position
/// being shown on the left and the track's length on the right, with the
/// pending mark the bar uses when the figure is a *request* rather than a
/// confirmed reading — a number must never be mistaken for playback truth it
/// has not earned.
fn figures(
    stamps: Option<crate::player::Stamps>,
    room: &'static theme::Palette,
) -> Element<'static, Message> {
    let Some(stamps) = stamps else {
        return Space::with_height(Length::Fixed(theme::LINE_META)).into();
    };
    let ink = if stamps.pending {
        room.paper_faint
    } else {
        room.paper_dim
    };
    row![
        text(stamps.elapsed)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(ink),
        Space::with_width(Length::Fill),
        text(stamps.total)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint),
    ]
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The kiosk is this surface at a larger size**, and it is a property of
    /// the arithmetic rather than a plan: the work's edge grows with the
    /// viewport, monotonically, and is bounded at both ends.
    #[test]
    fn the_work_grows_with_the_window_and_stops_at_both_ends() {
        let mut previous = 0.0_f32;
        for side in (400..=4000).step_by(7) {
            let side = f32::from(u16::try_from(side).expect("a window side fits u16"));
            let edge = art_edge(side, side);
            assert!(edge >= theme::ART_MIN, "{side}: {edge}");
            assert!(edge <= NOW_PLAYING_MAX, "{side}: {edge}");
            assert!(
                edge >= previous,
                "{side}: the work shrank as the window grew"
            );
            previous = edge;
        }
        // A 4 K panel is the surface at its ceiling, not one cover and
        // nothing else.
        assert!((art_edge(3840.0, 2160.0) - NOW_PLAYING_MAX).abs() < f32::EPSILON);
    }

    /// **A wide, short window is bounded by its height** — a now-playing
    /// screen whose placard has been pushed off the bottom is not one.
    #[test]
    fn a_short_window_is_bounded_by_its_height() {
        assert!(
            art_edge(2560.0, 600.0) < art_edge(2560.0, 1400.0),
            "the height has to be in the arithmetic"
        );
        // …and it never collapses below the floor, whatever the window does.
        for height in [0.0, 1.0, 120.0, 300.0] {
            assert!((art_edge(1280.0, height) - theme::ART_MIN).abs() < f32::EPSILON);
        }
    }
}
