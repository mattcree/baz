//! **The Home place** — the interrupted run, and what is new.
//!
//! ADR-0030 §3.2 recommended a home *band* at the head of the Library's body
//! and drew this page in §9.4 as the alternative it was being recommended
//! against. **The owner chose the page**, and `docs/REFUSALS.md`'s preamble
//! says his decision is sufficient on its own; the ADR carries the amendment.
//!
//! # Two sections, and an honest inventory behind them
//!
//! ADR-0030 §6 inventoried what a home surface could truthfully hold and
//! found exactly two facts worth the room. That survives the change from band
//! to place unchanged, because it was an argument about *facts*, not about
//! geometry:
//!
//! - **`CONTINUE`** — the interrupted run (ADR-0023 §6's snapshot, built for
//!   this; see [`crate::session`]).
//! - **`RECENTLY ADDED`** — a row of records by first-seen, in the wall's own
//!   tile, carrying the wall's own hover options.
//!
//! Refused from the page and still refused: **recently played** and
//! **playlists**, which are the returns lane's content one column to the left
//! — one fact drawn twice is doc 07 L8.6's test; **the pull**, which is an act
//! you press, and an unbidden offer is generation without a request; and every
//! engagement statistic, which is not close.
//!
//! **A section is absent, not empty.** `CONTINUE` is absent with no snapshot
//! or unresolvable files; `RECENTLY ADDED` is absent when the library holds
//! fewer than a row of records. A page with neither says so in one line rather
//! than drawing two empty headings.
//!
//! # The signature of the whole design
//!
//! The placard carries the needle, and **nothing is drawn on the artwork**.
//! Every product this one is measured against puts a progress bar across the
//! bottom of the cover; baz puts it under the wall label, at exactly the
//! sleeve's width, where a gallery puts the caption. That is the one drawing
//! in the mockup the owner approved that is not a rearrangement of something
//! already shipped, and it is what [`needle`] is.

use iced::widget::{Space, button, column, container, image as iced_image, row, scrollable, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::shelf::Grid;
use crate::views::{gradient_block, section_rule};
use crate::{icon, theme, vm};

/// How many records `RECENTLY ADDED` shows: **one row of the wall's own
/// tiles**, at whatever the density's column count is.
///
/// A row rather than a number, because the wall's arithmetic already decides
/// how many works fit across a width and a second answer to that question
/// would be a second grid.
fn recent_columns(width: f32) -> usize {
    Grid::new(
        (width - 2.0 * theme::HANG).max(0.0),
        crate::shelf::Density::Balanced,
    )
    .columns
}

/// **The needle's arithmetic**, alone: how much of the line is amber and how
/// much is muted, given a position, a length and the sleeve's width.
///
/// Split out from the drawing so the numbers are testable without a window —
/// the one thing on this page that is arithmetic rather than composition, and
/// the one thing that would be wrong in a way a screenshot could not show.
///
/// Three properties hold at every input, and the tests state them: the two
/// runs and the tick fill the sleeve's width exactly; a track with no declared
/// length reads as unstarted rather than as finished; and a position past the
/// end clamps rather than overrunning.
#[must_use]
fn needle_runs(elapsed_ms: u64, total_ms: u64, width: f32) -> (f32, f32) {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a track's length in milliseconds is far below f32's \
                  exact-integer range; the quotient is a fraction of one"
    )]
    let fraction = if total_ms > 0 {
        (elapsed_ms as f32 / total_ms as f32).clamp(0.0, 1.0)
    } else {
        // No declared length is *unstarted*, never finished: an undeclared
        // duration is the scan not having read one, and a full amber line
        // would be the interface inventing a fact about a track it has not
        // measured.
        0.0
    };
    // The tick takes 1 px out of the line, so the two runs and it add to the
    // width exactly — the rule that keeps the needle the sleeve's own measure
    // at every position, which is what makes the band read as one object.
    let usable = (width - theme::NEEDLE_TICK_W).max(0.0);
    let filled = (usable * fraction).round();
    (filled, usable - filled)
}

/// The Home place's body.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    resume: &'a crate::session::Snapshot,
    width: f32,
    collecting: crate::playlists::Collecting,
) -> Element<'a, Message> {
    let room = theme::active();
    let mut body = column![].spacing(theme::HANG);

    let continuing = continue_band(shelf, player, resume, width);
    let added = recently_added(shelf, player, width, collecting);
    let nothing = continuing.is_none() && added.is_none();
    if let Some(band) = continuing {
        body = body.push(band);
    }
    if let Some(band) = added {
        body = body.push(band);
    }
    if nothing {
        // A page with neither fact says so once, plainly, and offers the one
        // thing there is to do: the collection.
        return container(
            text("Nothing to pick up yet. The Library is where everything is.")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        )
        .center(Length::Fill)
        .into();
    }
    scrollable(container(body).padding(crate::views::place_pad()))
        .direction(iced::widget::scrollable::Direction::Vertical(
            theme::wall_scrollbar(),
        ))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// **`CONTINUE`** — the run baz was interrupted in the middle of.
///
/// The sleeve at [`theme::CONTINUE_SLEEVE`] beside a placard: the artist in
/// letterspaced caps, the work's title in [`theme::WORK_TITLE`], the condition
/// line, then the needle and what it is a needle into.
///
/// **Absent, not empty** (ADR-0030 §6): no snapshot, or a snapshot whose files
/// the library no longer holds, and there is no band at all. Nothing here draws
/// a placeholder, because a placard about no work is worse than no placard.
fn continue_band<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    resume: &'a crate::session::Snapshot,
    width: f32,
) -> Option<Element<'a, Message>> {
    let room = theme::active();
    let path = resume.current()?;
    // The record the interrupted track belongs to, found by path — the same
    // reconciliation every other reading of a queue position uses, and what
    // keeps the band true across a rescan that renumbered the run.
    let (album, track) = shelf.albums.iter().find_map(|album| {
        album
            .editions
            .iter()
            .flat_map(|edition| edition.tracks.iter())
            .find(|track| track.path == path)
            .map(|track| (album, track))
    })?;
    let edge = theme::CONTINUE_SLEEVE;
    let sleeve: Element<'a, Message> = match shelf.thumbs.peek(&album.id) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .into(),
        None => gradient_block(album.id, edge, 1.0),
    };

    // The condition line: what the record is, in the album page's own
    // vocabulary and from the same view model — `1988 · FLAC · 16-bit ·
    // 44.1 kHz`, and each part absent when the scan did not read it.
    let edition = album.editions.first();
    let mut condition: Vec<String> = Vec::new();
    if let Some(year) = album.year {
        condition.push(year.to_string());
    }
    if let Some(edition) = edition {
        if let Some(format) = edition.key.0 {
            condition.push(format.name().to_owned());
        }
        if let Some(depth) = edition.bit_depth {
            condition.push(format!("{depth}-bit"));
        }
        if let Some(rate) = edition.sample_rate {
            condition.push(vm::format_sample_rate(rate));
        }
    }

    let elapsed = resume.position_ms;
    let total = track
        .duration
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0);

    let mut placard = column![
        // The artist in letterspaced caps — the section rule's own voice, in
        // the placard's top line, which is where a wall label puts it.
        text(theme::tracked(&album.artist.label().to_uppercase()))
            .size(theme::SIZE_HEADING)
            .line_height(theme::LEADING_HEADING)
            .font(theme::MEDIUM)
            .color(room.paper_faint),
        // **The work's own title, in serif italic.** The one string in the
        // product that takes it; see [`theme::WORK_TITLE`].
        text(
            album
                .title
                .clone()
                .unwrap_or_else(|| "Unknown Album".into())
        )
        .size(theme::SIZE_TITLE)
        .line_height(theme::LEADING_TITLE)
        .font(theme::WORK_TITLE)
        .color(room.paper)
        .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XS);
    if !condition.is_empty() {
        placard = placard.push(
            text(condition.join(" · "))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        );
    }
    placard = placard
        .push(Space::with_height(Length::Fixed(theme::GAP_SM)))
        .push(needle(elapsed, total, edge))
        .push(resume_line(player, track, elapsed, total));

    Some(
        column![
            section_rule("Continue"),
            container(
                row![sleeve, placard]
                    .spacing(theme::GAP_XL)
                    .align_y(iced::Alignment::Start)
            )
            .width(Length::Fixed(width.min(theme::ALBUM_BREAKPOINT))),
        ]
        .spacing(theme::GAP_LG)
        .into(),
    )
}

/// **The needle**: a 2 px hairline exactly the sleeve's width, amber up to the
/// elapsed fraction with a 1 px tick at the position, muted after.
///
/// It is drawn on the *placard*, at the sleeve's measure — not on the artwork.
/// That is the design's signature and it is a rule rather than a preference:
/// `docs/REFUSALS.md` forbids drawing on a work, and every product baz is
/// measured against puts this line across the bottom of the cover.
///
/// The amber is licensed: this is playback truth, which is the accent's one
/// meaning. The tick is what turns a proportion into a *position* — a bar
/// alone reads as "how much", and a mark on it reads as "where".
pub(crate) fn needle(elapsed_ms: u64, total_ms: u64, width: f32) -> Element<'static, Message> {
    let room = theme::active();
    let (filled, rest) = needle_runs(elapsed_ms, total_ms, width);
    let lane = |w: f32, colour: iced::Color, h: f32| {
        container(Space::new(Length::Fixed(w), Length::Fixed(h)))
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(colour)),
                ..container::Style::default()
            })
            .into()
    };
    let mut parts: Vec<Element<'static, Message>> = Vec::new();
    if filled > 0.0 {
        parts.push(lane(filled, room.lamp, theme::NEEDLE_H));
    }
    // The tick: 1 px at the position, at the full accent, drawn taller than
    // the line so it reads as a mark *on* it rather than as a longer run of it.
    parts.push(lane(
        theme::NEEDLE_TICK_W,
        room.lamp_bright,
        theme::NEEDLE_H + theme::GAP_XS,
    ));
    if rest > 0.0 {
        parts.push(lane(rest, room.hairline_strong(room.wall), theme::NEEDLE_H));
    }
    container(
        iced::widget::Row::with_children(parts)
            .align_y(iced::Alignment::Center)
            .height(Length::Fixed(theme::NEEDLE_H + theme::GAP_XS)),
    )
    .width(Length::Fixed(width))
    .clip(true)
    .into()
}

/// `Resume · Anhydrous 2 · 3:12 of 6:27` — the verb, the track, and the
/// position in figures.
///
/// **`Resume` is the ordinary `Play`** (ADR-0030 §6), aimed at the snapshot's
/// cursor: it is the one press that spends the interrupted position, and it is
/// the only thing on this page that starts audio.
fn resume_line<'a>(
    player: &'a PlayerState,
    track: &'a vm::TrackVm,
    elapsed: u64,
    total: u64,
) -> Element<'a, Message> {
    let room = theme::active();
    let figures = format!(
        "{} of {}",
        vm::format_duration(std::time::Duration::from_millis(elapsed)),
        vm::format_duration(std::time::Duration::from_millis(total)),
    );
    let verb = button(
        container(
            row![
                iced_image(icon::handle(icon::Glyph::Play))
                    .width(Length::Fixed(theme::ICON_PX))
                    .height(Length::Fixed(theme::ICON_PX)),
                text("Resume")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .font(theme::MEDIUM)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press_maybe(player.engine_ready().then_some(Message::ResumeRun));
    row![
        verb,
        text(format!("{} · {figures}", track.title))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .into()
}

/// **`RECENTLY ADDED`** — one row of records by `first_seen_ns`, newest first,
/// in **the wall's own tile**.
///
/// Not a second tile design: `views::shelf::tile` is called with the wall's
/// own [`Grid`], so the sleeve, the caption, the playing mark and the hover
/// options are the wall's, to the pixel. A record behaves the same wherever it
/// is drawn, which is what makes a second surface showing records affordable
/// at all.
///
/// **Absent, not empty**: a library with fewer records than a row has columns
/// has nothing to say that the wall one press away does not say better.
/// **The row `RECENTLY ADDED` draws**, resolved: newest `first_seen_ns` first,
/// ties by title, one row's worth — or empty when the library has fewer
/// records than a row has columns.
///
/// Shared with the shell, which asks for the same ids to decode their art:
/// the wall's prefetch is a range over the wall, and a record drawn *beside*
/// the wall is not in it (`Shelf::request_thumbs_for`). Two answers to "which
/// records does Home show" could disagree, and the one that disagreed would be
/// the one whose covers never arrived.
///
/// The tie-break is the returns lane's own total-order rule, for the reason it
/// has there: two launches over the same library must draw the same row.
pub(crate) fn newest(shelf: &Shelf, width: f32) -> Vec<&vm::AlbumVm> {
    let columns = recent_columns(width);
    let mut newest: Vec<&vm::AlbumVm> = shelf
        .albums
        .iter()
        .filter(|album| album.first_seen_ns.is_some())
        .collect();
    if newest.len() < columns {
        return Vec::new();
    }
    newest.sort_by(|a, b| {
        b.first_seen_ns
            .cmp(&a.first_seen_ns)
            .then_with(|| a.title.cmp(&b.title))
    });
    newest.truncate(columns);
    newest
}

fn recently_added<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    width: f32,
    collecting: crate::playlists::Collecting,
) -> Option<Element<'a, Message>> {
    let newest = newest(shelf, width);
    if newest.is_empty() {
        return None;
    }
    let hang = Grid::new(
        (width - 2.0 * theme::HANG).max(0.0),
        crate::shelf::Density::Balanced,
    );
    let mut tiles = row![].spacing(hang.gutter);
    for album in newest {
        tiles = tiles.push(crate::views::shelf::tile(
            shelf,
            player,
            hang,
            album,
            0.0,
            (false, false),
            collecting,
        ));
    }
    Some(
        column![section_rule("Recently added"), tiles]
            .spacing(theme::GAP_LG)
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The needle is exactly the sleeve's width, at every position.**
    ///
    /// That is the rule the whole band's composition rests on: the placard's
    /// line and the artwork beside it are one measure, so a needle that
    /// rounded its way to 131 or 133 px would break the alignment the eye
    /// actually reads. Swept at 1 ms over a real track's length.
    #[test]
    fn the_needle_is_the_sleeves_measure_at_every_position() {
        let width = theme::CONTINUE_SLEEVE;
        let total = 387_000_u64;
        for elapsed in (0..=total).step_by(97) {
            let (filled, rest) = needle_runs(elapsed, total, width);
            assert!(filled >= 0.0 && rest >= 0.0, "{elapsed} ms");
            assert!(
                (filled + rest + theme::NEEDLE_TICK_W - width).abs() < 0.001,
                "{elapsed} ms: {filled} + {rest} + tick != {width}"
            );
        }
    }

    /// The two ends, and they are the ends: nothing amber at the start, and
    /// nothing muted at the finish.
    #[test]
    fn the_needle_starts_empty_and_finishes_full() {
        let width = theme::CONTINUE_SLEEVE;
        let (filled, rest) = needle_runs(0, 100_000, width);
        assert!((filled - 0.0).abs() < f32::EPSILON);
        assert!((rest - (width - theme::NEEDLE_TICK_W)).abs() < 0.001);

        let (filled, rest) = needle_runs(100_000, 100_000, width);
        assert!((filled - (width - theme::NEEDLE_TICK_W)).abs() < 0.001);
        assert!((rest - 0.0).abs() < 0.001);
    }

    /// **A track with no declared length reads as unstarted**, never as
    /// finished — an undeclared duration is the scan not having read one, and
    /// a full amber line would be the interface inventing a fact about a track
    /// it has not measured. A position past the end clamps rather than
    /// overrunning, which is the same rule from the other side.
    #[test]
    fn the_needle_invents_nothing_and_overruns_nothing() {
        let width = theme::CONTINUE_SLEEVE;
        for elapsed in [0, 1, 10_000, u64::MAX] {
            let (filled, rest) = needle_runs(elapsed, 0, width);
            assert!(
                (filled - 0.0).abs() < f32::EPSILON,
                "{elapsed} ms of nothing"
            );
            assert!((rest - (width - theme::NEEDLE_TICK_W)).abs() < 0.001);
        }
        let (filled, rest) = needle_runs(999_999, 1_000, width);
        assert!((filled - (width - theme::NEEDLE_TICK_W)).abs() < 0.001);
        assert!(rest >= 0.0, "the muted run never goes negative");
    }
}
