//! **How should it move?** — the second and last question the page asks.
//!
//! Design 21 §5. One line by default, and it is a **blend**: five lanes
//! holding one curve between them, weighted with energy dominant. Weighted
//! rather than averaged because every dimension here is a rank within the
//! collection, so a plain mean puts loud-and-slow in the same place as
//! quiet-and-fast, and a line through the middle would be satisfied by tracks
//! that sound nothing alike — the *"the dots aren't following my line"*
//! failure, back wearing a different hat.
//!
//! Because the default really is five lanes at one curve, *tune each thing
//! Baz listens for* **reveals** the per-dimension lines rather than seeding
//! them: they were already the blend, and stay it until one is dragged away
//! from its neighbours.
//!
//! The control carries what §5 asks of it: the axis labelled in words with no
//! legend and no key, the eligible songs drawn behind it, a sentence stating
//! the shape, presets as chips underneath, and — under
//! [`theme::COMPOSE_SHORT_H`] of window height — the sentence and the presets
//! alone, which is the control's accessible form anyway.

use iced::widget::{Space, column, row};
use iced::{Element, Length};

use crate::app::Message;
use crate::views::compose::{Layout, chip, heading, wrap_chips};
use crate::{theme, views};

pub(crate) fn view(vibe: &crate::vibe::State, layout: Layout) -> Element<'_, Message> {
    let mut block = column![heading("How should it move?")].spacing(theme::GAP_SM);
    block = block.push(views::hint(crate::vibe::shape_words(&vibe.contour)));
    if layout.draw_curve {
        if vibe.expanded {
            for (index, lane) in vibe.contour.lanes.iter().enumerate() {
                block = block.push(line(vibe, index, &lane.points, Some(lane.dimension)));
            }
        } else if let Some(lane) = vibe.contour.lane(0) {
            block = block.push(line(vibe, 0, &lane.points, None));
        }
    }
    // **One shape row, not two.** The named presets and the point count were
    // the same control twice — a preset *is* a shape plus a number of points,
    // so `Slow build` and `Straight` lit at once and said the same thing in
    // two vocabularies. The count wins because it is the finer-grained of the
    // two, which is what the owner asked for: any shape a preset could make
    // is a count plus dragging, and the sentence above names what you drew in
    // the words the presets used to supply.
    block = block.push(points(vibe));
    block = block.push(expander(vibe));
    block.into()
}

/// **How many points the line carries** — the control that was missing.
///
/// The owner: *"the graph/curve does not allow any users to adjust the curve?
/// maybe add a point count?"* The points were always draggable; what could
/// not be changed was **how many there are**, so a two-point preset could be
/// tilted and nothing else, and the only route to a turn was picking a preset
/// that happened to have one.
///
/// A count rather than the `−`/`+` stepper design 21 §5 deleted, and it is
/// deleted for a good reason: two marks that add and remove say nothing about
/// where you are in a range or that a range exists. These are the page's own
/// pills, lit at the current number, so the whole range is visible and one
/// press reaches any of it.
///
/// A new turn arrives **on the line it joins** — at the level the line
/// already stands at, in the widest gap — so gaining a handle changes the
/// shape by nothing, and it is dragged deliberately rather than recovered
/// from.
fn points(vibe: &crate::vibe::State) -> Element<'_, Message> {
    let current = vibe.contour.points();
    column![
        views::caption_word("POINTS"),
        wrap_chips(
            (crate::vibe::Contour::MIN_POINTS..=crate::vibe::Contour::MAX_POINTS)
                .map(|count| {
                    chip(
                        POINT_LABELS[count - crate::vibe::Contour::MIN_POINTS],
                        count == current,
                        Message::ContourPoints(count),
                    )
                })
                .collect(),
            5,
        ),
    ]
    .spacing(theme::GAP_XS)
    .into()
}

/// The counts, as words rather than as bare digits: a row reading `2 3 4 5 6`
/// beside a row reading `Any Steady Slow build` would be two kinds of thing
/// in one anatomy.
const POINT_LABELS: [&str; 5] = ["Straight", "1 turn", "2 turns", "3 turns", "4 turns"];

/// **One drawn line**, with the cloud behind it, the words at its ends and the
/// result over it.
///
/// `named` is `None` while this is the blend — the listener is not asked to
/// know the word *energy*, let alone *spectral flatness* — and carries the
/// dimension's own name once the lines have been pulled apart, at which point
/// naming them is the entire reason they are open.
fn line<'a>(
    vibe: &'a crate::vibe::State,
    index: usize,
    points: &'a [crate::vibe::ContourPoint],
    named: Option<crate::vibe::Dimension>,
) -> Element<'a, Message> {
    let room = theme::active();
    // **The axis in words, not in a legend.** Three words at each end while
    // this is the blend, because the blend is several things at once and a
    // single word for it would be a claim about which one dominates.
    let (low, high) = named.map_or(("quiet, slow, sparse", "loud, fast, busy"), |dimension| {
        dimension.ends()
    });
    let mut block = column![].spacing(theme::GAP_XS);
    if let Some(dimension) = named {
        // Opened, each line says what it *measures*. That is the whole reason
        // to open them: a listener who asks for `Brightness` is asking for
        // spectral centroid, rolloff and zero crossings, and is entitled to
        // know it — none of these is a mood and none of them pretends to be.
        // **What it measures, and how much it counts.** The five lines do not
        // influence a result equally — the blend is weighted with energy
        // dominant — so a line that says only what it measures leaves the
        // listener to discover by experiment that dragging texture moves the
        // list a quarter as far as dragging energy does.
        block = block
            .push(views::caption_word(&format!(
                "{} · {}% OF THE BLEND",
                dimension.label().to_uppercase(),
                dimension.share()
            )))
            .push(views::hint(dimension.measured_from()));
    }
    // **The axis words sit above and below the line, not in a gutter beside
    // it.** Three words do not fit a 48 px lane, and the first attempt put
    // *quiet, slow, sparse* straight through *first song*. Above and below
    // there is the whole measure for them, and no legend anywhere.
    let canvas = crate::contour::Contour::new(points, room, theme::CONTOUR_H)
        .field(cloud(vibe, named.map(|_| index)))
        .result(dots(vibe, named.map(|_| index)))
        .highlight(vibe.selected_row.or(vibe.hovered_row))
        .on_drag(move |point, at, level| Message::ContourDragged(index, point, at, level))
        .on_release(Message::ContourReleased);
    let foot = row![
        views::axis_word("first song"),
        Space::new().width(Length::Fill),
        views::axis_word("last song"),
    ]
    .align_y(iced::Alignment::Center);
    block
        .push(views::axis_word(high))
        .push(canvas)
        .push(views::axis_word(low))
        .push(foot)
        .into()
}

/// **The eligible songs, drawn behind the line** — design 21 §6's second
/// readout, and the clearest picture of cause and effect in the feature.
///
/// It is the eligible set and not the library: narrow the phrase and watch it
/// thin out under the curve; draw the line where the cloud is not and you know
/// what will happen before pressing anything. Live, from the same debounced
/// count under the field, so it describes the phrase on screen rather than the
/// one that was last composed.
fn cloud(vibe: &crate::vibe::State, lane: Option<usize>) -> &[f32] {
    // Opened, each line draws its own dimension's cloud, from the last
    // compose: there is no live per-dimension count, and drawing the blend
    // behind five different axes would put the same picture behind five
    // different questions.
    if let Some(lane) = lane {
        return vibe
            .preview
            .as_ref()
            .and_then(|preview| preview.lane_clouds.get(lane))
            .map_or(&[], Vec::as_slice);
    }
    vibe.live
        .as_ref()
        .map(|live| live.cloud.as_slice())
        // Before the words have settled once, the last compose's eligible set
        // is a truer picture than the whole library — and the whole library is
        // the honest picture only when neither exists.
        .or_else(|| {
            vibe.preview
                .as_ref()
                .map(|preview| preview.cloud.as_slice())
        })
        .filter(|cloud| !cloud.is_empty())
        .unwrap_or_else(|| vibe.field_of(crate::vibe::Dimension::Energy))
}

/// Where the composed list landed, as `(position, level)` for the line to draw
/// over itself — the answer in the request's own units, on the blended axis
/// the line is drawn on.
fn dots(vibe: &crate::vibe::State, lane: Option<usize>) -> Vec<(f32, f32)> {
    let Some(levels) = vibe.preview.as_ref().and_then(|preview| match lane {
        Some(lane) => preview.levels.get(lane),
        None => Some(&preview.blended),
    }) else {
        return Vec::new();
    };
    let last = levels.len().saturating_sub(1).max(1);
    levels
        .iter()
        .enumerate()
        .map(|(index, level)| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a playlist is bounded at PLAYLIST_CAP, far below f32's exact range"
            )]
            let at = index as f32 / last as f32;
            (at, *level)
        })
        .collect()
}

/// **A labelled control, not a bare triangle.** Design 21 §5's seventh item:
/// the expander says what it opens, and what it opens is already holding this
/// line's own points.
fn expander(vibe: &crate::vibe::State) -> Element<'_, Message> {
    let open = vibe.expanded;
    let label = if open {
        "Back to one line"
    } else {
        "Shape each thing Baz listens for separately"
    };
    // A chip rather than a bare word, and a sentence saying what it *does*.
    // The owner had to ask for per-dimension curves after they shipped —
    // which is what a control nobody can find looks like — and then said the
    // control *"isn't clear how it influences things"*, which the old copy
    // earned by describing itself instead of its effect.
    column![
        chip(label, open, Message::ContourExpander),
        views::hint(if open {
            "One line each. Drag energy and the list gets louder or quieter where you \
             drew it; drag brightness and it gets darker or crisper there instead. They \
             do not count equally — the share beside each name is how much it moves the \
             result."
        } else {
            "One line moves all five things Baz listens for together. Open this to make \
             the list climb in energy while it steadies in tempo, or any other \
             combination."
        }),
    ]
    .spacing(theme::GAP_XS)
    .into()
}
