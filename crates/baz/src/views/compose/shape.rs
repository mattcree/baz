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
//!
//! **It is the first thing on the page now** (design note 25). It used to
//! stand in the middle of the answer column and only at the advanced depth,
//! so the default page had no curve on it at all — the one control here whose
//! effect a listener can check by ear, filed behind a tab, under a text field
//! whose retrieval was no better than chance for two of six tested requests.

use iced::widget::{Space, column, row};
use iced::{Element, Length};

use crate::app::Message;
use crate::views::compose::{Layout, chip, heading, wrap_chips};
use crate::{theme, views};

pub(crate) fn view(vibe: &crate::vibe::State, layout: Layout) -> Element<'_, Message> {
    let mut block = column![heading("How should it move?")].spacing(theme::GAP_SM);
    block = block.push(views::hint(crate::vibe::shape_words(&vibe.contour)));
    // **What the line is, said once, at the top of it.**
    //
    // The owner: *"the concept of the 'blend' of the curves isn't that
    // clear."* It was not said anywhere — the word appeared beside each
    // percentage as though it were a thing the reader already had, and the
    // single line said nothing about being five.
    //
    // The expanded sentence is the one that matters, and it is the same
    // answer as *"the stuff is not conforming to each"*: the walk satisfies
    // the shares, not each line, because it cannot satisfy each line. Saying
    // so turns a control that looks broken into one that is behaving
    // visibly.
    block = block.push(views::hint(if vibe.shown.is_some() {
        "A song cannot be in five places at once, so where the lines disagree the \
         shares settle it. That is why the dots track a 40% line closely and a 10% \
         line loosely."
    } else {
        "One shape, asked of five things at once: energy counts most, then tempo, then \
         brightness and dynamics, and texture least. Pick one below to shape it on its \
         own."
    }));
    // **One graph, and a row of tabs over it.** The owner: *"I like the idea
    // of all lines being on the same graph and a way to kinda toggle between
    // all and individual… then selecting each individually to be able to
    // configure that line."*
    //
    // It replaces five stacked canvases — 1 100 px of them — with one, and it
    // is the only arrangement in which the **disagreement between the lines**
    // is a thing you can see rather than a thing a sentence has to describe.
    if layout.draw_curve {
        block = block.push(tabs(vibe));
        block = block.push(line(vibe));
    }
    // **One shape row, not two.** The named presets and the point count were
    // the same control twice — a preset *is* a shape plus a number of points,
    // so `Slow build` and `Straight` lit at once and said the same thing in
    // two vocabularies. The count wins because it is the finer-grained of the
    // two, which is what the owner asked for: any shape a preset could make
    // is a count plus dragging, and the sentence above names what you drew in
    // the words the presets used to supply.
    block = block.push(points(vibe));
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
/// A new point arrives **on the line it joins** — at the level the line
/// already stands at, in the widest gap — so gaining a handle changes the
/// shape by nothing, and it is dragged deliberately rather than recovered
/// from.
///
/// They read `Straight`, `1 turn`, `2 turns`… until the owner said what was
/// wrong with that: *"the term 'turns' is not correct in this case I think?
/// more like just points on a curve."* Ten points on a straight line has no
/// turns in it at all, so the label was a claim about shape from a control
/// that only sets a number of handles. `POINTS` above supplies the noun —
/// which is the engine's own word ([`crate::vibe::ContourPoint`]) and every
/// envelope editor's word for the same thing.
fn points(vibe: &crate::vibe::State) -> Element<'_, Message> {
    let current = vibe.contour.points();
    column![
        views::caption_word("POINTS"),
        wrap_chips(
            (crate::vibe::Contour::MIN_POINTS..=crate::vibe::Contour::MAX_POINTS)
                .map(|count| {
                    chip(
                        &count.to_string(),
                        count == current,
                        Message::ContourPoints(count),
                    )
                })
                .collect(),
            9,
        ),
    ]
    .spacing(theme::GAP_XS)
    .into()
}

/// **The tabs over the graph** — all five, or one of them.
///
/// The share rides on the tab because it is a property of the line, and the
/// order is the order of the shares, so the row itself says which of them
/// moves the result most without anybody reading a number. Nothing here is a
/// mode: pressing a tab changes what you can drag and nothing about the
/// request, which is what makes them free to press.
fn tabs(vibe: &crate::vibe::State) -> Element<'_, Message> {
    let mut all = vec![chip(
        "All five",
        vibe.shown.is_none(),
        Message::VibeLine(None),
    )];
    for (index, lane) in vibe.contour.lanes.iter().enumerate() {
        all.push(chip(
            &format!("{} · {}%", lane.dimension.label(), lane.dimension.share()),
            vibe.shown == Some(index),
            Message::VibeLine(Some(index)),
        ));
    }
    wrap_chips(all, 6)
}

/// **The one graph**, with the cloud behind it, the words at its ends, the
/// result over it and the lines you are not editing faint underneath.
fn line(vibe: &crate::vibe::State) -> Element<'_, Message> {
    let room = theme::active();
    let shown = vibe.shown;
    let named = shown
        .and_then(|lane| vibe.contour.lane(lane))
        .map(|lane| lane.dimension);
    let index = shown.unwrap_or(0);
    let points = vibe
        .contour
        .lane(index)
        .map_or(&[][..], |lane| lane.points.as_slice());
    // **The axis in words, not in a legend.** Three words at each end while
    // all five are shown, because that is several things at once and a single
    // word for it would be a claim about which one dominates.
    let (low, high) = named.map_or(("quiet, slow, sparse", "loud, fast, busy"), |dimension| {
        dimension.ends()
    });
    // The four you are not editing. On *all five* there is nothing to ghost:
    // every line is the line, and drawing four copies of it under itself
    // would only thicken it.
    let ghosts: Vec<&[crate::vibe::ContourPoint]> = shown.map_or_else(Vec::new, |lane| {
        vibe.contour
            .lanes
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != lane)
            .map(|(_, other)| other.points.as_slice())
            .collect()
    });
    // **The axis words sit above and below the line, not in a gutter beside
    // it.** Three words do not fit a 48 px lane, and the first attempt put
    // *quiet, slow, sparse* straight through *first song*. Above and below
    // there is the whole measure for them, and no legend anywhere.
    let canvas = crate::contour::Contour::new(points, room, theme::CONTOUR_H)
        .ghosts(ghosts)
        .field(cloud(vibe, shown))
        .result(dots(vibe, shown))
        .highlight(vibe.selected_row.or(vibe.hovered_row))
        .on_drag(move |point, at, level| Message::ContourDragged(index, point, at, level))
        .on_release(Message::ContourReleased);
    let foot = row![
        views::axis_word("first song"),
        Space::new().width(Length::Fill),
        views::axis_word("last song"),
    ]
    .align_y(iced::Alignment::Center);
    let mut block = column![views::axis_word(high), canvas, views::axis_word(low), foot,]
        .spacing(theme::GAP_XS);
    if let Some(dimension) = named {
        // **What this line measures**, which is the whole reason to look at
        // one on its own: somebody who asks for `Brightness` is asking for
        // spectral centroid, rolloff and zero crossings, and is entitled to
        // know it. None of these is a mood and none of them pretends to be.
        block = block
            .push(Space::new().height(theme::GAP_XS))
            .push(views::hint(dimension.measured_from()));
        // **And where the collection has nothing to say on it, say so.** A
        // rank axis spreads whatever it is given across the whole scale by
        // construction, so a line drawn over a dimension this library barely
        // varies in will be followed perfectly by dots while nothing about
        // the music changes — a control that looks like it is working and is
        // not. Design note 24 §4.
        if vibe.profile.flat_axes.contains(&dimension) {
            block = block.push(views::alert(&format!(
                "Your music barely varies in {} — this line will move the list very \
                 little.",
                dimension.label().to_lowercase()
            )));
        }
        if !vibe.contour.is_one_line() {
            block = block.push(chip(
                "Put every line back on one shape",
                false,
                Message::VibeGatherLines,
            ));
        }
    }
    block.into()
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
