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
//! # The queue is not a second place — it is this surface's other half
//!
//! The owner, 2026-08-10: *"the queue and the now playing need integrated in
//! some way so we can remove the queue option from the bottom bar"*. So
//! `Place::Queue` is deleted and this place absorbs it whole, and the argument
//! is stronger than adjacency (`docs/design/12-now-playing-and-kiosk.md`
//! §3.4.1):
//!
//! > **A run is a list and a cursor. Now playing is the cursor. The queue is
//! > the list. They are two readings of one object.**
//!
//! What was on screen before the merge read as a defect rather than a layout:
//! *a surface about what is playing that could not say what it was playing
//! **in**, beside a surface about the list that did not show what was sounding
//! **from** it.* Each held the half the other was missing. The list half is
//! [`crate::views::queue::run_column`], drawn here at [`theme::RUN_MEASURE`],
//! with every one of the fifteen gestures the queue place had
//! (`every_queue_affordance_survives_the_merge`).
//!
//! # Two densities, and `F11` is not one of them
//!
//! The `Run` word in the top-right decides whether the list is on this screen,
//! and it is **remembered** ([`crate::config::Config::run_column`]) rather than
//! bound to full-screen. That is a toolkit fact rather than a preference: iced
//! 0.13 publishes no monitor enumeration at all, so baz cannot tell
//! *full-screen on the second monitor* from *full-screen on the only monitor*,
//! and a single-display listener pressing `F11` would lose the run editor with
//! no way back that is not un-full-screening. `F11` stays a **window** act that
//! works in every place; what genuinely changes with size is arithmetic —
//! which axis the two columns sit on ([`theme::SPLIT_FLOOR`]) — and not mode.
//!
//! # A first version, and what it is designed to become
//!
//! Deliberately simple: the artwork large, the identity under it, the needle
//! and the position — and the run beside it. No transport — the bar under this
//! place carries it, and drawing it twice was a defect. No visualizer and no
//! VU — those are future work and are not allowed to constrain this.
//!
//! **The kiosk full-screen mode is this same surface at a larger size**, and
//! that is a property of the composition rather than a plan: every measure
//! here is derived from the viewport by [`art_edge`], so the place at 3840 px
//! is this place with a bigger number in it. `docs/design/12-now-playing-and-kiosk.md`
//! argues the *reason* — the surface is read at two distances that do not
//! overlap, the far field wants very few very large statements, and the near
//! field already has the bar, which is in every place. Nothing here forecloses
//! it.
//!
//! # The serif stays on the Home placard
//!
//! The work's title here is set in the sans, not in `theme::WORK_TITLE`. The
//! serif italic is the *museum placard's* convention and there is one placard
//! in the product; a second consumer would be a display face arriving one
//! surface at a time, which is the thing `assets/fonts/README.md` records as
//! deleted and staying deleted. `the_serif_is_the_work_titles_and_nothing_else`
//! holds this to it.

use iced::widget::{Space, button, column, container, image as iced_image, row, stack, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::playlists::{Collecting, NameEntry};
use crate::theme;
use crate::views::{gradient_block, queue};

/// **The run column's width in a body `width` px wide**, or `0` when the run is
/// not standing beside the record.
///
/// Two conditions, both of them arithmetic rather than mode (doc 12 §3.4.2 —
/// the density is a *stated control*, and full-screen changes nothing about
/// it): the listener has turned the run off, or the body is below
/// [`theme::SPLIT_FLOOR`] and the two columns have re-stacked into one, where
/// the run takes the whole measure and the record becomes its head.
#[must_use]
pub(crate) fn run_w(width: f32, run: bool) -> f32 {
    if run && width >= theme::SPLIT_FLOOR {
        theme::RUN_MEASURE
    } else {
        0.0
    }
}

/// What the placard under the work needs: three lines, the needle, and the
/// gaps between them — **130**.
///
/// It summed [`theme::TRANSPORT_HIT`] as well until the merge, which was the
/// unspent half of ADR-0029's first step: the duplicated transport widget came
/// off this surface (the bar carries it, in this place as in every other) and
/// the 32 px it had reserved stayed in the arithmetic, so the sleeve was 32 px
/// short at every height-bound size. Removing it is the whole of the fix.
///
/// It grows again when the surface does: doc 12 §5.5's figure of 190 is this
/// number plus the momentary meter's 24, the feed's 20 and one
/// [`theme::GAP_LG`] — none of which are built yet (they are steps A5 and A9),
/// and none of which may reserve height before they exist.
const BELOW: f32 = theme::LINE_HEADING
    + theme::LINE_HERO
    + theme::LINE_BODY
    + theme::NEEDLE_H
    + 4.0 * theme::GAP_LG;

/// **The edge the record is actually drawn at**, whichever composition the
/// body's width has put it in.
///
/// Above [`theme::SPLIT_FLOOR`] it is [`art_edge`]'s answer, with the run's
/// column taken off the width when the run is standing. Below it the columns
/// have re-stacked and the record is the run's **head block** at
/// [`theme::ART_MIN`] — the size at which a cover stops being a subject is the
/// size the head gives it, because what is left worth doing at that width is
/// the list (doc 12 §5.5a).
///
/// Stating it in one function is what keeps the surface monotonic across the
/// floor: the record does not lurch when the two columns become one.
#[must_use]
pub(crate) fn record_edge(width: f32, height: f32, run: bool) -> f32 {
    if run && width < theme::SPLIT_FLOOR {
        theme::ART_MIN
    } else {
        art_edge(width, height, run_w(width, run))
    }
}

/// **The artwork's edge**, derived from the viewport and clamped.
///
/// The whole of what makes the kiosk mode this surface at a larger size: the
/// work takes the room it is given, bounded below so it never stops being the
/// subject and above so a 4 K panel does not become one cover and nothing
/// else. The height term is what stops a wide, short window pushing the
/// placard off the bottom — a now-playing screen that has scrolled away from
/// what is playing is not one.
///
/// `run_w` is [`run_w`]'s answer, and it is subtracted from the *width* term
/// alone: the run's head sits in the run's own column, so it costs the record
/// no height (doc 12 §5.5a). **The run costs the record nothing wherever the
/// record is height-bound**, which is every window above the narrowest one this
/// product draws — `the_run_costs_the_record_nothing_where_it_is_height_bound`
/// is that claim swept.
#[must_use]
pub(crate) fn art_edge(width: f32, height: f32, run_w: f32) -> f32 {
    let beside = if run_w > 0.0 {
        run_w + theme::GAP_XL
    } else {
        0.0
    };
    let by_width = width - 2.0 * theme::HANG - beside;
    let by_height = height - 2.0 * theme::HANG - BELOW;
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

/// The air the run column leaves above its summary for the place's own
/// top-right controls, which are drawn as a **layer** and therefore cost the
/// record no height (doc 12 §5.5a).
const CONTROLS_CLEARANCE: f32 = theme::TRANSPORT_HIT + theme::GAP_LG;

/// **The merged Now playing place**: the record, and the run it is a position
/// in (doc 12 §3.4, `Place::Queue`'s whole inheritance).
///
/// `run` is the listener's standing answer to *is the list on this screen* —
/// the `Run` word in the top-right, remembered across launches, and
/// **deliberately not bound to full-screen**: iced 0.13 publishes no monitor
/// enumeration, so baz cannot tell a second-display full-screen from an
/// only-display one, and a single-display listener pressing `F11` would lose
/// the editor with no way back (§3.4.2). Everything else on this surface is
/// arithmetic — which axis the two columns sit on, and nothing more.
#[expect(
    clippy::too_many_arguments,
    reason = "the surface is two halves and each half's readings arrive \
              whole: the record's from the shelf and the engine, the run's \
              from the four independent studies `views::queue` already names"
)]
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    width: f32,
    height: f32,
    run: bool,
    hovered: Option<usize>,
    saving: Option<&'a NameEntry>,
    collecting: Collecting,
    scroll: f32,
    drag: Option<&'a crate::drag::DragState>,
    can_undo: bool,
) -> Element<'a, Message> {
    let now = player.now_playing();
    // The run is on this screen when the listener has left it on *and* there
    // is a run to draw. A `Run` word standing over an empty column would be a
    // density with nothing in it.
    let showing_run = run && player.queue_list().is_some();
    if now.is_none() && !showing_run {
        // **A start in flight is not silence.** `Resume` on the Home place
        // navigates here in the same press that asks the engine to begin
        // (`App::resume_the_run`), and the engine's `TrackStarted` is a frame
        // or two behind the press — so for those frames there is a record on
        // its way and no record yet to draw.
        //
        // The place stays bare rather than announcing silence it is about to
        // contradict. A sentence that appears and vanishes is *read*, and a
        // statement of silence is the one thing this surface must never make
        // while something is starting; a blank that fills is not read at all.
        // The condition is the engine's own — a transport command awaiting its
        // confirming event — so nothing here has to know which press sent it.
        if player.transport_pending() {
            return Space::new(Length::Fill, Length::Fill).into();
        }
        // **The two empty states became one** (doc 12 §6.4.4). This surface
        // would otherwise carry both *"Nothing playing."* and *"Nothing
        // queued"*; the run's wins, because it is strictly more informative —
        // it names the gestures that fill the list, and it carries the
        // silence-is-a-feature sentence the product wants said at exactly this
        // moment.
        return stack![
            container(queue::empty_state()).center(Length::Fill),
            controls(run),
        ]
        .into();
    }
    let run_w = run_w(width, showing_run);
    let edge = record_edge(width, height, showing_run);
    let stacked = showing_run && run_w <= 0.0;
    let measure =
        (width - 2.0 * theme::HANG - theme::SCROLLBAR_LANE).clamp(0.0, theme::LIST_MEASURE);
    let record = now.map(|now| {
        if stacked {
            head_block(shelf, player, now, edge, measure)
        } else {
            record_column(shelf, player, now, edge)
        }
    });
    let body: Element<'a, Message> = match (record, showing_run) {
        // **The record alone**, centred in the body: the composition exactly as
        // it stood before the merge, at the size this window gives it.
        (Some(record), false) => container(record).center(Length::Fill).into(),
        // **Two columns.** The record column is *left-hung*, not centred: with
        // the run taking the right edge, centring the work in what remains
        // would leave the placard's left alignment pointing at nothing. The
        // work and its placard share a left edge with each other and hang from
        // the body's own `HANG`.
        (Some(record), true) if run_w > 0.0 => row![
            container(record)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(iced::Padding {
                    top: theme::HANG,
                    right: 0.0,
                    bottom: theme::HANG,
                    left: theme::HANG,
                })
                .align_y(alignment::Vertical::Center),
            Space::with_width(Length::Fixed(theme::GAP_XL)),
            container(run_scroll(
                player,
                queue::Frame {
                    measure: run_w - theme::SCROLLBAR_LANE,
                    viewport_h: height,
                    scroll,
                    clearance: CONTROLS_CLEARANCE,
                    pad: iced::Padding {
                        top: theme::HANG,
                        right: theme::SCROLLBAR_LANE,
                        bottom: theme::HANG,
                        left: 0.0,
                    },
                },
                None,
                hovered,
                saving,
                collecting,
                drag,
                can_undo,
            ))
            .width(Length::Fixed(run_w))
            .height(Length::Fill),
            Space::with_width(Length::Fixed(theme::HANG)),
        ]
        .into(),
        // **One column, below `SPLIT_FLOOR`**: the run wins and the record
        // becomes its head. The record cannot be the size it deserves at this
        // width in any case, and what is left worth doing is the list — so the
        // same four objects are re-hung rather than a second layout drawn.
        (record, _) => {
            let head = record.map(|record| (record, edge + theme::GAP_LG + BELOW));
            run_scroll(
                player,
                queue::Frame {
                    measure,
                    viewport_h: height,
                    scroll,
                    clearance: CONTROLS_CLEARANCE,
                    pad: crate::views::place_pad(),
                },
                head,
                hovered,
                saving,
                collecting,
                drag,
                can_undo,
            )
        }
    };
    stack![body, controls(run)].into()
}

/// [`queue::run_column`], named here so the two call sites above read as one
/// thing rather than as eight arguments twice.
#[expect(
    clippy::too_many_arguments,
    reason = "a pass-through: every argument is `queue::run_column`'s own, and \
              naming them again in a struct would name this call site and \
              nothing else"
)]
fn run_scroll<'a>(
    player: &'a PlayerState,
    frame: queue::Frame,
    head: Option<(Element<'a, Message>, f32)>,
    hovered: Option<usize>,
    saving: Option<&'a NameEntry>,
    collecting: Collecting,
    drag: Option<&'a crate::drag::DragState>,
    can_undo: bool,
) -> Element<'a, Message> {
    queue::run_column(
        player, frame, head, hovered, saving, collecting, drag, can_undo,
    )
}

/// **The place's top-right controls** — today the `Run` word alone.
///
/// A *layer* over the body rather than a strip in it, which is what keeps the
/// record's arithmetic untouched by the word that governs the column beside it
/// (doc 12 §5.5a: *the run's head is in the run's column, so it costs the
/// record no height*). The run column leaves [`CONTROLS_CLEARANCE`] of air
/// above its summary so the two never overlap.
///
/// **Visible at rest, and pointer-reachable**, which the product's standing
/// rule requires; the alternative — revealing it on mouse-move, as most kiosks
/// do — is refused outright. It is a **peer** of the `Ambient` word-door that
/// arrives with the ambient field, not a row inside it: those toggles govern
/// how the surface *looks*, and this one governs what it *is* (§3.4.3).
fn controls(run: bool) -> Element<'static, Message> {
    let room = theme::active();
    container(
        button(
            container(
                text("Run")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .font(theme::MEDIUM)
                    .wrapping(text::Wrapping::None),
            )
            .height(Length::Fill)
            .align_y(alignment::Vertical::Center),
        )
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(theme::pad(0.0, theme::GAP_SM))
        .style(move |_theme, status| theme::now_playing(room, status, run))
        .on_press(Message::ToggleRun),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(theme::pad(theme::HANG, theme::HANG))
    .align_x(alignment::Horizontal::Right)
    .align_y(alignment::Vertical::Top)
    .into()
}

/// **The record as the run's head block** — the composition below
/// [`theme::SPLIT_FLOOR`], where the two columns have re-stacked into one.
///
/// The cover at [`theme::ART_MIN`] on the left with the artist, the title and
/// the record beside it, and the needle under the pair at the head's own width.
/// **It scrolls away with the list**, which would be wrong above the floor and
/// is right here: at this width the surface has become the editor, and an
/// editor whose first 300 px are a fixed hero is an editor you scroll past to
/// use.
fn head_block<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    now: &'a crate::player::NowPlaying,
    edge: f32,
    measure: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let stamps = player.stamps();
    let elapsed = player.elapsed_ms();
    let total = player.track_ms().unwrap_or(0);
    let mut identity = column![
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
            .size(theme::SIZE_TITLE)
            .line_height(theme::LEADING_TITLE)
            .font(theme::SEMIBOLD)
            .color(room.paper)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XS)
    .width(Length::Fill);
    if let Some(album) = &now.album {
        identity = identity.push(
            text(album.clone())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None),
        );
    }
    column![
        row![
            sleeve(shelf, now, edge),
            container(identity)
                .width(Length::Fill)
                .height(Length::Fixed(edge))
                .align_y(alignment::Vertical::Bottom),
        ]
        .spacing(theme::GAP_LG),
        Space::with_height(Length::Fixed(theme::GAP_LG)),
        crate::views::home::needle(elapsed, total, measure),
        figures(stamps, room),
    ]
    .into()
}

/// The work itself at `edge` — the decoded cover, or the wall's own
/// deterministic gradient where a record has none.
fn sleeve<'a>(
    shelf: &'a Shelf,
    now: &'a crate::player::NowPlaying,
    edge: f32,
) -> Element<'a, Message> {
    match now.album_id.and_then(|id| shelf.thumbs.peek(&id)) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .into(),
        // The wall's own deterministic gradient, at this scale — the same
        // stand-in a tile shows, so a record with no cover is the same object
        // here as it is there.
        None => gradient_block(now.album_id.unwrap_or_default(), edge, 1.0),
    }
}

/// The record column: the work at `edge`, and the placard under it.
fn record_column<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    now: &'a crate::player::NowPlaying,
    edge: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let work = sleeve(shelf, now, edge);

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

    // **No transport here.** The bar is under every place, this one included,
    // and it already carries play/pause and the two skips — so the page drew
    // the *same function* a second time, a few hundred pixels above the first
    // (`bottom_bar::transport`, called from here). The owner: *"now playing
    // does not need the play pause controls"*, and *"ensure the play next and
    // previous controls are removed"*. It was a duplicate, not a choice.
    //
    // What this surface owes is a reading, not a control: the work at the size
    // it deserves, who made it, where the needle stands. The one place in the
    // product where the same fact appears twice on purpose is the lamp — and
    // that is a mark, not a button.
    column![work, placard]
        .spacing(theme::GAP_XL)
        .align_x(alignment::Horizontal::Center)
        .width(Length::Shrink)
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

    /// Every window side the sweeps below walk, in the shape `art_edge`'s
    /// original test walks them.
    fn sides() -> impl Iterator<Item = f32> {
        (400..=4000)
            .step_by(7)
            .map(|side| f32::from(u16::try_from(side).expect("a window side fits u16")))
    }

    /// **The kiosk is this surface at a larger size**, and it is a property of
    /// the arithmetic rather than a plan: the work's edge grows with the
    /// viewport, monotonically, and is bounded at both ends — **with the run
    /// standing beside it and without**.
    #[test]
    fn the_work_grows_with_the_window_and_stops_at_both_ends() {
        for run in [false, true] {
            let mut previous = 0.0_f32;
            for side in sides() {
                let edge = record_edge(side, side, run);
                assert!(edge >= theme::ART_MIN, "{side}: {edge}");
                assert!(edge <= NOW_PLAYING_MAX, "{side}: {edge}");
                assert!(
                    edge >= previous,
                    "{side}: the work shrank as the window grew (run {run})"
                );
                previous = edge;
            }
        }
        // A 4 K panel is the surface at its ceiling, not one cover and
        // nothing else — and the run does not take it off the ceiling.
        for run in [false, true] {
            let edge = record_edge(3840.0, 2160.0, run);
            assert!((edge - NOW_PLAYING_MAX).abs() < f32::EPSILON, "run {run}");
        }
    }

    /// **A wide, short window is bounded by its height** — a now-playing
    /// screen whose placard has been pushed off the bottom is not one.
    #[test]
    fn a_short_window_is_bounded_by_its_height() {
        assert!(
            art_edge(2560.0, 600.0, 0.0) < art_edge(2560.0, 1400.0, 0.0),
            "the height has to be in the arithmetic"
        );
        // …and it never collapses below the floor, whatever the window does.
        for height in [0.0, 1.0, 120.0, 300.0] {
            for run in [false, true] {
                let edge = art_edge(1280.0, height, run_w(1280.0, run));
                assert!((edge - theme::ART_MIN).abs() < f32::EPSILON, "run {run}");
            }
        }
    }

    /// **ADR-0029's first step, finished**: the duplicated transport widget
    /// came off this surface and the 32 px it had reserved stayed in the
    /// arithmetic, so the sleeve was 32 px short at every height-bound size.
    ///
    /// The number is asserted rather than the delta, because the delta is what
    /// a future step's own additions will change and the *terms* are what must
    /// not silently grow a transport again.
    #[test]
    fn the_placard_reserves_no_transport_it_does_not_draw() {
        const { assert!(BELOW == 130.0) }
        const { assert!(BELOW + theme::TRANSPORT_HIT == 162.0) }
        // 1280 × 860 with the returns lane collapsed: 1184 × 779 of body,
        // height-bound, and the sleeve is the height less the gutter and the
        // placard — with no transport in the subtraction.
        assert!((art_edge(1184.0, 779.0, 0.0) - (779.0 - 80.0 - BELOW)).abs() < f32::EPSILON);
        assert!((art_edge(1184.0, 779.0, 0.0) - 569.0).abs() < f32::EPSILON);
    }

    /// **The run costs the record nothing wherever the record is
    /// height-bound** (doc 12 §5.5a's table, stated as the property it is
    /// rather than as six rows).
    ///
    /// The run takes width the record structurally cannot use: above the
    /// tightest window this product draws, `below` is short and a 16 : 9 body
    /// is short before it is narrow, so the record's edge is set by the height
    /// and the column beside it changes nothing. Where the record *is*
    /// width-bound the cost is real, and the sweep pins that it is exactly the
    /// width the run took — recorded as a cost paid rather than hidden.
    #[test]
    fn the_run_costs_the_record_nothing_where_it_is_height_bound() {
        for width in sides() {
            for height in sides() {
                let with = art_edge(width, height, run_w(width, true));
                let without = art_edge(width, height, 0.0);
                let beside = run_w(width, true);
                if beside <= 0.0 {
                    assert!((with - without).abs() < f32::EPSILON, "{width}×{height}");
                    continue;
                }
                let by_height = height - 2.0 * theme::HANG - BELOW;
                if width - 2.0 * theme::HANG - (beside + theme::GAP_XL) >= by_height {
                    assert!(
                        (with - without).abs() < f32::EPSILON,
                        "{width}×{height}: the run cost the record {} px it was not using",
                        without - with
                    );
                }
                assert!(with <= without, "{width}×{height}");
            }
        }
        // The one row of §5.5a's table where the cost is real: 1280 × 860 with
        // the returns lane open is 1000 px of body, the tightest case this
        // product has, and the record is width-bound there. The remedy is
        // already on screen and already keyed — Ctrl+B collapses the lane and
        // the record comes back — which is why this is a cost paid rather than
        // a cost hidden.
        let (body_w, body_h) = (1000.0, 779.0);
        assert!(art_edge(body_w, body_h, run_w(body_w, true)) < art_edge(body_w, body_h, 0.0));
        // …and with the lane collapsed at the same window, it is free again.
        let body_w = 1184.0;
        assert!(
            (art_edge(body_w, body_h, run_w(body_w, true)) - art_edge(body_w, body_h, 0.0)).abs()
                < f32::EPSILON
        );
    }

    /// **The two columns re-stack below the split floor**, swept 400–4000 the
    /// way `art_edge`'s own tests are.
    ///
    /// Below [`theme::SPLIT_FLOOR`] the record cannot be the size it deserves
    /// in any case, so the run takes the measure and the record becomes its
    /// head — **one composition degrading, not a second layout**. Above it the
    /// two stand side by side at every size, with the run always exactly
    /// [`theme::RUN_MEASURE`].
    #[test]
    fn the_two_columns_restack_below_the_split_floor() {
        for width in sides() {
            let split = run_w(width, true);
            assert_eq!(
                split > 0.0,
                width >= theme::SPLIT_FLOOR,
                "{width}: the split floor is the only condition"
            );
            if split > 0.0 {
                assert!((split - theme::RUN_MEASURE).abs() < f32::EPSILON, "{width}");
                // At the floor itself the record still clears its own floor,
                // which is what the floor was derived from.
                assert!(
                    width - 2.0 * theme::HANG - (split + theme::GAP_XL) >= theme::ART_MIN,
                    "{width}: the record fell below ART_MIN inside the split"
                );
            }
            // The word turned off is the whole body, at every width.
            assert!((run_w(width, false)).abs() < f32::EPSILON, "{width}");
        }
        // The floor bites at a 1064 px window with the lane open and an 880 px
        // window with it collapsed — both below the 1280 the composition
        // audits are taken at, so the regime is real rather than theoretical.
        assert!((run_w(theme::SPLIT_FLOOR - 1.0, true)).abs() < f32::EPSILON);
        assert!(run_w(theme::SPLIT_FLOOR, true) > 0.0);
    }

    /// **Every queue affordance survives the merge** — doc 12 §6.4.4's table
    /// of fifteen, as a source assertion over the two modules that now hold
    /// them between them.
    ///
    /// Pinned over the source the way `views/queue.rs` and `views/shelf.rs`
    /// pin their own rulers: the property is about which widgets and which
    /// messages these files build, there is no `PlayerState` to construct
    /// without an engine, and the literals below are exactly what a reviewer
    /// would have to delete to break them. **A merge that quietly dropped a
    /// gesture is the failure this exists to catch**, and it is the one
    /// failure a screenshot cannot show.
    #[test]
    fn every_queue_affordance_survives_the_merge() {
        let read = |name: &str| {
            std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("src/views")
                    .join(name),
            )
            .expect("a view module's own source")
            .replace("\r\n", "\n")
        };
        let run = read("queue.rs");
        let place = read("now_playing.rs");

        // 1 · row click → jump. 2 · the per-row ✕. 3 · the ▲▼ steppers.
        // 4 · the transfer `+`. 5 · drag-to-reorder, and its observation wire.
        // 6 · `Save as playlist` and its field. 7 · `Undo`. 12 · the column's
        // own scroll.
        for spent in [
            "Message::JumpToQueued(index)",
            "Message::RemoveQueued(index)",
            "Message::ShiftQueued(index, -1)",
            "Message::ShiftQueued(index, 1)",
            "Message::AddQueuedToPlaylist(index)",
            "Message::DragLift(crate::drag::List::Queue, index, at)",
            "Message::DragOverRow(crate::drag::List::Queue, index, before)",
            "Message::SaveQueueStart",
            "Message::SaveQueueInput",
            "Message::SaveQueueSubmit",
            "Message::Undo",
            "Message::QueueScrolled",
        ] {
            assert!(
                run.contains(spent),
                "the run column no longer spends `{spent}`"
            );
        }
        // 9 · the right-press mirror menu. 10 · row hover tracking.
        for wired in [
            "crate::menu::Target::QueueRow { row: index }",
            "Message::QueueRowEntered(index)",
            "Message::QueueRowLeft(index)",
        ] {
            assert!(
                run.contains(wired),
                "the run column no longer wires `{wired}`"
            );
        }
        // 11 · the virtual window, with both spacers — the load-bearing one,
        // because `Play all` can reify a whole library into this run.
        assert!(
            run.contains("queue_window::window(&shapes, scroll - rows_top, viewport_h)")
                && run.contains("for index in win.first..win.end")
                && run.contains("Space::with_height(Length::Fixed(win.top))")
                && run.contains("Space::with_height(Length::Fixed(win.bottom))"),
            "the run is no longer virtual"
        );
        // 13 · album group headers — albums are listed as albums, never
        // flattened (ADR-0014).
        assert!(run.contains("fn album_group("), "the record headers went");
        // 8 · the provenance-led summary, promoted to the surface's head.
        assert!(run.contains("text(list.summary)"), "the run's head went");
        // 15 · the empty state — and it is **the one this surface now uses**,
        // which is the merge decision §6.4.4 records: the queue's wins,
        // because it names the gestures that fill the list.
        assert!(
            place.contains("queue::empty_state()"),
            "the merged surface draws its own empty state again"
        );
        // 14 · the header strip and the second empty state are the two that
        // **go**: the merged place wears no header (the lane is the route, and
        // the head states the list), and it says nothing about silence that
        // the run's own empty state does not say better.
        //
        // Both needles are spelled in halves so that this assertion is not its
        // own counter-example — `implicit.rs`'s rule for a test that searches
        // the file it lives in.
        for (head, tail) in [("text", "(\"Nothing"), ("place", "_header")] {
            let gone = format!("{head}{tail}");
            assert!(
                !place.contains(&gone),
                "`{gone}` came back to the merged place"
            );
        }
        // …and the branch above the empty state stays: a start in flight is
        // not silence.
        assert!(place.contains("player.transport_pending()"));

        // And the surface reaches all of it through the one column, rather
        // than by copying a row anatomy that would then drift.
        assert!(
            place.contains("queue::run_column("),
            "the merged place stopped drawing the run column"
        );
    }

    /// **The run is virtual at kiosk scale** (doc 12 §12 M1's gate).
    ///
    /// `Play all` can reify a whole library into this run, so the column the
    /// merged surface draws must cost the frame what a twelve-track record
    /// does — at 3840 × 2160 as much as at 1280 × 860. The arithmetic is
    /// [`crate::queue_window`]'s and this asserts the surface's own inputs
    /// reach it: a five-figure run, at the kiosk's viewport, builds a bounded
    /// slice.
    #[test]
    fn the_run_is_virtual_at_kiosk_scale() {
        use crate::queue_window::{self, RowShape};
        let rows: Vec<RowShape> = (0..40_000)
            .map(|index| RowShape {
                head: (index % 12 == 0 && index > 0).then_some(true),
                two_line: index % 3 == 0,
            })
            .collect();
        for viewport in [779.0_f32, 999.0, 2079.0] {
            let span = viewport + 2.0 * queue_window::MARGIN;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a positive row count in the tens; the assert below pins it"
            )]
            let bound = (span / queue_window::row_pitch(false)).ceil() as usize + 2;
            assert!(bound < 200, "the bound stays small: {bound}");
            for scroll in [0.0, 12_345.0, 987_654.0] {
                let win = queue_window::window(&rows, scroll, viewport);
                assert!(
                    win.end - win.first <= bound,
                    "{} rows at {viewport} px / offset {scroll} — the run is not virtual",
                    win.end - win.first
                );
            }
        }
    }
}
