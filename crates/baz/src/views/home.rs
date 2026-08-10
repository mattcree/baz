//! **The Home place** — the interrupted run, and what is new.
//!
//! ADR-0030 §3.2 recommended a home *band* at the head of the Library's body
//! and drew this page in §9.4 as the alternative it was being recommended
//! against. **The owner chose the page**, and the product's preamble
//! says his decision is sufficient on its own; the ADR carries the amendment.
//!
//! # Two sections, and an honest inventory behind them
//!
//! ADR-0030 §6 inventoried what a home surface could truthfully hold and
//! found exactly two facts worth the room. That survives the change from band
//! to place unchanged, because it was an argument about *facts*, not about
//! geometry:
//!
//! - **`CONTINUE`** — the run to carry on with (ADR-0023 §6's snapshot, built
//!   for this; see [`crate::session`]).
//! - **`RECENTLY ADDED`** — a row of records by first-seen, in the wall's own
//!   tile, carrying the wall's own hover options.
//!
//! # The band asks one question, and it is only asked in the silence
//!
//! > **`CONTINUE` stands whenever there is a run to carry on with and nothing
//! > is sounding.** Start anything, anywhere in the product, and it is gone;
//! > stop, and it is back, describing where you now are.
//!
//! The owner's rule, in his words: *"keep it simple with the continue part…
//! once you select resume, it just disappears"*, *"or takes you to now
//! playing"*, *"it just reappears when you stop the player"*. It replaces a
//! design in which the band gained a second reading and turned into a
//! `NOW PLAYING` placard once the music started, and it is better than that
//! design rather than merely smaller than it:
//!
//! - **It is one predicate, not a lifecycle.** [`standing`] is the whole of
//!   it, and there is no bookkeeping about a question having been *spent* that
//!   could get out of step with the engine.
//! - **There is no path where the band is wrongly absent.** Every state that
//!   is not *sounding* either has a run to offer or has nothing to offer, and
//!   both are drawn correctly by the same three lines.
//! - **It costs nothing at rest.** A live needle on this page would have
//!   wanted the position while the music ran; a band that is *absent* while
//!   the music runs wants nothing, so Home adds no subscription and no clock
//!   at all. That is the idle-cost problem deleted rather than budgeted for.
//! - **It is useful after every stop**, not only after a launch. Pause an
//!   album halfway, come to Home, and the way back in is right there.
//!
//! What is sounding is the bottom bar's job, in every place, and
//! [`Place::NowPlaying`](crate::place::Place::NowPlaying) is a place of its own
//! one row up in the returns lane. A Home band that described the sounding
//! track would be the same fact in three places at once.
//!
//! **`Resume` is the one play gesture in the product that navigates**, and it
//! goes to `Now playing` — see [`resume_line`].
//!
//! Refused from the page and still refused: **recently played** and
//! **playlists**, which are the returns lane's content one column to the left
//! — one fact drawn twice is doc 07 L8.6's test; **the pull**, which is an act
//! you press, and an unbidden offer is generation without a request; and every
//! engagement statistic, which is not close.
//!
//! **A section is absent, not empty.** `CONTINUE` is absent while something is
//! sounding, with no run to carry on with, and when the library no longer holds
//! the file the run is on; `RECENTLY ADDED` is absent when the library holds
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
use crate::player::{Phase, PlayerState};
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

/// **What the `CONTINUE` band is a placard for**: the track to carry on with
/// and how far into it to carry on from — or `None` when there is nothing to
/// carry on with, or something is already sounding.
///
/// **One function with one answer**, deliberately. The band draws from the
/// live run when there is one and from the persisted snapshot only before
/// anything has played, and those are not two code paths that could disagree
/// about which record the listener is looking at — they are two arms of the
/// same `match`, in priority order:
///
/// 1. **Something is sounding** ([`Phase::Playing`]) — no band. This is the
///    whole of the owner's rule and it is read off the engine, so it is true
///    of every route into playback: the wall's hover `Play`, a playlist,
///    `Play all`, the bar, a media key, MPRIS.
/// 2. **The engine holds a track and is not playing it** — *paused*, and the
///    band describes **what you paused**, at the engine's own confirmed
///    position. Never the launch snapshot, which by then is describing the
///    start of this same track rather than where you actually are.
/// 3. **Something has sounded and the engine holds nothing** — the run
///    *ended*. **No band.** This is the one case the word "stopped" does not
///    settle on its own, and it goes the other way from a pause: a run you
///    played to the end has no "where you stopped", the needle would sit at a
///    finish, and the product's standing rules is emphatic that the queue empties and
///    the silence at the end of a run is a feature. An offer to carry on with
///    something you completed is the interface remembering something that is
///    over.
/// 4. **Nothing has sounded** — the run baz launched with, at the position it
///    was interrupted at ([`crate::session`]). The only moment the snapshot is
///    read, and `crate::app`'s `next_snapshot` guarantees it is still the file
///    baz opened: nothing this process writes can move it while nothing has
///    sounded, so what the band shows cannot drift under it.
///
/// Pure, and it takes the two values it reads rather than the shell, so the
/// rule is unit-testable without a window — which is what the tests below walk.
#[must_use]
pub(crate) fn standing<'a>(
    player: &'a PlayerState,
    resume: &'a crate::session::Snapshot,
) -> Option<(&'a std::path::Path, u64)> {
    if player.phase() == Phase::Playing {
        return None;
    }
    if let Some(path) = player.now_playing_path() {
        return Some((path, player.elapsed_ms()));
    }
    if player.has_sounded() {
        return None;
    }
    resume.current().map(|path| (path, resume.position_ms))
}

/// **`CONTINUE`** — the run to carry on with, drawn only in the silence.
///
/// The sleeve at [`theme::CONTINUE_SLEEVE`] beside a placard: the artist in
/// letterspaced caps, the work's title in [`theme::WORK_TITLE`], the condition
/// line, then the needle and what it is a needle into.
///
/// **Absent, not empty** (ADR-0030 §6): nothing to carry on with ([`standing`]),
/// or a track whose file the library no longer holds, and there is no band at
/// all. Nothing here draws a placeholder, because a placard about no work is
/// worse than no placard.
///
/// **The needle is static by construction.** [`standing`] answers `None` while
/// anything is playing, so the position this draws is one the engine has
/// stopped moving — which is why Home needs no clock, no timer and no
/// subscription of its own to keep it honest. It is never extrapolated, and
/// there is nothing here that could extrapolate it.
fn continue_band<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    resume: &'a crate::session::Snapshot,
    width: f32,
) -> Option<Element<'a, Message>> {
    let room = theme::active();
    let (path, elapsed) = standing(player, resume)?;
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

    // **The length is the scan's reading, never the engine's**, even when the
    // band is describing a paused session that the engine could report one
    // for. Every other fact on this placard — the artist, the work, the
    // condition line — comes from the library's view model, and one figure
    // from the other side would let the length and the condition line
    // disagree about which file they are describing.
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
/// the product's standing rules forbids drawing on a work, and every product baz is
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
/// **`Resume` is the ordinary `Play`** (ADR-0030 §6), aimed at where the band
/// says you are: it is the one press that spends the position on the placard,
/// and it is the only thing on this page that starts audio.
///
/// **It is also the one play gesture in the product that navigates** — it
/// starts the run *and* goes to
/// [`Place::NowPlaying`](crate::place::Place::NowPlaying). Pressing `Play` on
/// the wall's hover options, on a record's page or in a playlist deliberately
/// moves you nowhere, and this one is not an inconsistency with them but the
/// difference between two verbs: those say *play this*, and answering them by
/// leaving the surface you are choosing from would be the interface taking the
/// wheel; `Resume` says *pick up where I left off*, and the place that
/// describes where you are is the answer to it rather than a side effect of
/// it. It is also what makes the band's disappearance coherent — you are not
/// left standing on Home watching a placard go, you are on the surface that
/// describes what is now sounding. See `crate::app`'s `App::resume_the_run`.
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
    use std::path::{Path, PathBuf};

    use baz_core::protocol::Event;

    use super::*;
    use crate::player::Availability;

    const FIRST: &str = "/m/Anhydrous 1.flac";
    const SECOND: &str = "/m/Anhydrous 2.flac";
    const ELSEWHERE: &str = "/m/Some Other Record.flac";

    /// The interrupted run baz launched with: three tracks, stopped 3:12 into
    /// the second.
    fn interrupted() -> crate::session::Snapshot {
        crate::session::Snapshot {
            paths: vec![
                PathBuf::from(FIRST),
                PathBuf::from(SECOND),
                PathBuf::from("/m/Anhydrous 3.flac"),
            ],
            cursor: 1,
            position_ms: 192_000,
            provenance: None,
        }
    }

    fn started(path: &str) -> Event {
        Event::TrackStarted {
            path: PathBuf::from(path),
            position: 0,
        }
    }

    /// A shell at launch: an engine, and nothing has sounded through it.
    fn at_launch() -> PlayerState {
        PlayerState::new(Availability::Ready)
    }

    /// **The band stands on the interrupted run until something sounds**, and
    /// the moment anything does, it is gone.
    ///
    /// The owner's rule from the near side: *"once you select resume, it just
    /// disappears"*. What the band shows before the press is the snapshot's
    /// own record at the snapshot's own position — the launch state, and the
    /// only state in which the file is read at all.
    #[test]
    fn the_band_stands_on_the_interrupted_run_until_something_sounds() {
        let resume = interrupted();
        let mut player = at_launch();
        assert_eq!(
            standing(&player, &resume),
            Some((Path::new(SECOND), 192_000)),
            "at launch the band is the interrupted run, at its stored position"
        );

        player.apply(&started(SECOND), &[]);
        assert_eq!(
            standing(&player, &resume),
            None,
            "something is sounding, so the question the band asks is not one \
             to ask"
        );
    }

    /// **Every route into playback takes the band away**, because the rule is
    /// read off the engine rather than off the gesture.
    ///
    /// The wall's hover `Play`, a playlist, `Play all`, the bar, a media key,
    /// MPRIS — none of them is named here and none of them has to be: they all
    /// end in one [`Event::TrackStarted`], and a track that has nothing to do
    /// with the snapshot takes the band away exactly as the snapshot's own
    /// does. Nothing in [`standing`] compares the two paths.
    #[test]
    fn playback_from_anywhere_at_all_takes_the_band_away() {
        let resume = interrupted();
        let mut player = at_launch();
        player.apply(&started(ELSEWHERE), &[]);
        assert_eq!(standing(&player, &resume), None);
    }

    /// **A pause brings the band back describing what you paused** — not the
    /// launch snapshot, which by then names the start of the track you are
    /// halfway through.
    ///
    /// The owner's rule from the far side: *"it just reappears when you stop
    /// the player"*. This is the case that makes the band useful after every
    /// stop rather than only after a launch, and it is why the content comes
    /// from the live run whenever there is one.
    #[test]
    fn a_pause_brings_the_band_back_describing_what_was_paused() {
        let resume = interrupted();
        let mut player = at_launch();
        player.apply(&started(ELSEWHERE), &[]);
        player.apply(
            &Event::Progress {
                elapsed_ms: 45_000,
                track_ms: Some(300_000),
            },
            &[],
        );
        player.apply(&Event::Paused, &[]);
        assert_eq!(
            standing(&player, &resume),
            Some((Path::new(ELSEWHERE), 45_000)),
            "the band describes the paused run at the engine's own position, \
             and the launch snapshot is not consulted once anything has sounded"
        );

        player.apply(&Event::Resumed, &[]);
        assert_eq!(standing(&player, &resume), None, "sounding again");

        // …and it comes back on the next pause, at the next position.
        player.apply(
            &Event::Progress {
                elapsed_ms: 61_000,
                track_ms: Some(300_000),
            },
            &[],
        );
        player.apply(&Event::Paused, &[]);
        assert_eq!(
            standing(&player, &resume),
            Some((Path::new(ELSEWHERE), 61_000))
        );
    }

    /// **A run that finished is not a run to carry on with.**
    ///
    /// The one case the word *stopped* does not settle on its own, and it goes
    /// the other way from a pause. A run played to its end has no "where you
    /// stopped"; the product's standing rules calls the silence at the end of a run a
    /// feature, and an offer to carry on with something you completed is the
    /// interface remembering something that is over. The snapshot is *not*
    /// fallen back on here — that is what [`PlayerState::has_sounded`] is for,
    /// since the phase, the queue and the playing row look the same in this
    /// state as they do at launch.
    #[test]
    fn a_run_that_finished_is_not_a_run_to_carry_on_with() {
        for ending in [Event::QueueEnded, Event::Stopped] {
            let resume = interrupted();
            let mut player = at_launch();
            player.apply(&started(SECOND), &[]);
            player.apply(&ending, &[]);
            assert_eq!(
                standing(&player, &resume),
                None,
                "{ending:?} left an offer to replay a run that is over"
            );
            assert!(
                !resume.is_empty() && resume.current().is_some(),
                "…and the snapshot is still perfectly readable, which is the \
                 point: only `has_sounded` tells this state from a launch"
            );
        }
    }

    /// **Nothing to carry on with is a state**, and the band is absent rather
    /// than empty (ADR-0030 §6). A fresh install, a snapshot whose cursor
    /// fell outside its run, and an engine that never started are all it.
    #[test]
    fn nothing_to_carry_on_with_is_a_state() {
        assert_eq!(
            standing(&at_launch(), &crate::session::Snapshot::default()),
            None,
            "a fresh install"
        );
        let mut past_the_end = interrupted();
        past_the_end.cursor = 9;
        assert_eq!(
            standing(&at_launch(), &past_the_end),
            None,
            "a cursor outside the run names no track"
        );
    }

    /// **Home has nothing to animate.** [`standing`] answers `None` for every
    /// state in which the engine is moving a position, so the needle this page
    /// draws is always one that has stopped — which is why Home adds no
    /// timer, no clock and no subscription of its own, and why the position it
    /// draws can never be an extrapolation.
    ///
    /// The claim is checked rather than asserted in prose: the only phase in
    /// which a position advances is [`Phase::Playing`], and there is no
    /// snapshot and no engine state that produces a band in it.
    #[test]
    fn the_band_is_never_on_screen_while_a_position_is_moving() {
        let resume = interrupted();
        let mut player = at_launch();
        for event in [
            started(SECOND),
            Event::Progress {
                elapsed_ms: 1_000,
                track_ms: Some(300_000),
            },
            Event::Paused,
            Event::Resumed,
            started(ELSEWHERE),
            Event::Paused,
            Event::Resumed,
            Event::QueueEnded,
        ] {
            player.apply(&event, &[]);
            if player.phase() == Phase::Playing {
                assert_eq!(
                    standing(&player, &resume),
                    None,
                    "{event:?} left a static needle on screen while the engine \
                     was moving the position behind it"
                );
            }
        }
    }

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
