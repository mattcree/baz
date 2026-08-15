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

use iced::widget::{Space, column, row, text};
use iced::{Element, Length};

use crate::app::Message;
use crate::views::compose::{Layout, chip, heading, wrap_chips};
use crate::{theme, views};

pub(crate) fn view(vibe: &crate::vibe::State, layout: Layout) -> Element<'_, Message> {
    let mut block = column![heading("How should it move?")].spacing(theme::GAP_SM);
    block = block.push(views::hint(sentence(vibe)));
    if layout.draw_curve {
        if vibe.expanded {
            for (index, lane) in vibe.contour.lanes.iter().enumerate() {
                block = block.push(line(vibe, index, &lane.points, Some(lane.dimension)));
            }
        } else if let Some(lane) = vibe.contour.lane(0) {
            block = block.push(line(vibe, 0, &lane.points, None));
        }
    }
    block = block.push(presets(vibe));
    block = block.push(expander(vibe));
    block.into()
}

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
        block = block
            .push(views::caption_word(&dimension.label().to_uppercase()))
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

/// **The shape, said out loud**, updating as it is dragged.
///
/// A picture of a request should still be *sayable*: it is what a listener
/// checks before spending a compose, it is the whole control under
/// [`theme::COMPOSE_SHORT_H`], and it is what somebody reading the screen aloud
/// has.
fn sentence(vibe: &crate::vibe::State) -> &'static str {
    let Some(opening) = vibe.contour.level_at(0, 0.0) else {
        return "No shape — your words alone decide the order.";
    };
    let landing = vibe.contour.level_at(0, 1.0).unwrap_or(opening);
    let peak = (0_u8..=10)
        .filter_map(|step| vibe.contour.level_at(0, f32::from(step) / 10.0_f32))
        .fold(f32::MIN, f32::max);
    let turns = vibe
        .contour
        .lane(0)
        .is_some_and(|lane| lane.points.len() > 2);
    let rise = landing - opening;
    if turns && peak > opening.max(landing) + 0.4 {
        "Starts quiet, climbs to a peak partway through, and comes down."
    } else if rise > 0.6 {
        "Starts quiet and climbs the whole way."
    } else if rise < -0.6 {
        "Starts loud and winds down."
    } else if turns {
        "Turns on the way through and ends where it started."
    } else {
        "Holds one level the whole way."
    }
}

/// **The presets, underneath rather than above** — because they are the
/// press-instead-of-drag route to the same outcome, and under
/// [`theme::COMPOSE_SHORT_H`] they are the only route. Chips rather than
/// thumbnails: a 104 px picture of a straight line teaches nothing the word
/// *steady* does not.
fn presets(vibe: &crate::vibe::State) -> Element<'_, Message> {
    let points: Vec<Vec<crate::vibe::ContourPoint>> = crate::vibe::Shape::ALL
        .iter()
        .map(|shape| shape.points())
        .collect();
    wrap_chips(
        crate::vibe::Shape::ALL
            .iter()
            .enumerate()
            .map(|(index, shape)| {
                let lit = vibe
                    .contour
                    .lane(0)
                    .is_some_and(|lane| lane.points == points[index])
                    || (points[index].is_empty() && vibe.contour.lanes.is_empty());
                chip(shape.label, lit, Message::ContourShape(index))
            })
            .collect(),
        3,
    )
}

/// **A labelled control, not a bare triangle.** Design 21 §5's seventh item:
/// the expander says what it opens, and what it opens is already holding this
/// line's own points.
fn expander(vibe: &crate::vibe::State) -> Element<'_, Message> {
    let room = theme::active();
    let open = !vibe.contour.is_one_line();
    let label = if open {
        "Back to one line"
    } else {
        "Tune each thing Baz listens for"
    };
    let mut block = column![views::word_button_maybe(
        label,
        Some(Message::ContourExpander),
    )]
    .spacing(theme::GAP_XS);
    if open {
        block = block.push(
            text(
                "Every line starts at the shape above. Each is a stated combination of \
                 measurements — never a mood.",
            )
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_dim)
            .width(Length::Fill)
            .wrapping(text::Wrapping::Word),
        );
    }
    block.into()
}
