//! **What do you want to hear?** — one band, one request, and the readouts
//! that say what it means.
//!
//! # Hierarchy, after the owner looked at it
//!
//! *"I mean just look at this. there is just no information hierarchy."* He
//! was right, and the defect was specific: eleven blocks at one size, one
//! weight and **one spacing**, so nothing grouped and nothing led. The
//! remedies are *Refactoring UI*'s (Wathan and Schoger), applied rather than
//! alluded to:
//!
//! 1. **Space between groups must beat space within them.** Everything here
//!    was [`theme::GAP_SM`] apart, so `MADE OF` sat as far from its own chips
//!    as from the block above it. Within a group is [`TIGHT`] and between
//!    groups is [`APART`] — a six-fold step, which is what lets proximity do
//!    the grouping without a single rule or box.
//! 2. **Hierarchy by de-emphasis.** There is one primary control on this pane
//!    — the field — and it is the only thing at body size. Everything else
//!    steps *down* rather than the field stepping up.
//! 3. **Labels are usually optional.** Four all-caps labels — `A PLACE TO
//!    START`, `MADE OF`, `FEELS LIKE`, `HOW LONG` — competed with the content
//!    they named and with each other. Two survive: the length's is carried by
//!    the commitment, which already says *about an hour*, and the vocabulary's
//!    two rows are one group under one label.
//! 4. **Secondary information is one line, not four.** The count and its three
//!    closest titles were four body lines of equal weight. They are a count
//!    and a caption now, because they are something you glance at.
//!
//! # The model
//!
//! Design 21 §4. The **field is at the head**, with one sentence underneath —
//! *this is exactly what Baz searches for* — because that sentence is what
//! says there is no hidden state and that what you can read is what will
//! happen. Beneath it, two ways of writing it: **starting points** replace the
//! line, **the vocabulary** appends with a comma. Neither is a second input;
//! there is one line, so there is no way to have two of anything, and a
//! starting point stops being lit the moment the words change — not a mode
//! switching off, a label ceasing to be true.

use iced::widget::{Space, column, container, text, text_input};
use iced::{Element, Length};

use crate::app::{Message, Shelf};
use crate::views::compose::{Layout, Stage, chip, depth_tabs, heading, wrap_chips};
use crate::{theme, views};

/// Space **within** a group.
const TIGHT: f32 = theme::GAP_XS;
/// Space **between** groups — six times [`TIGHT`], which is the grouping.
const APART: f32 = theme::GAP_XL;

pub(crate) fn view(shelf: &Shelf, stage: Stage, layout: Layout) -> Element<'_, Message> {
    let room = theme::active();
    let vibe = &shelf.vibe;
    let advanced = vibe.depth == crate::vibe::Depth::Advanced;

    // 1. The one primary control, and the sentence saying what it is.
    let mut asked = column![
        heading("What do you want to hear?"),
        text_input(
            vibe.profile
                .example
                .as_deref()
                .unwrap_or("warm hypnotic music for driving at night"),
            &vibe.prompt,
        )
        .on_input(Message::VibePrompt)
        .on_submit(Message::VibeCreate)
        .width(Length::Fill)
        .padding(theme::pad(theme::GAP_SM, theme::GAP_MD))
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .style(move |_theme, status| theme::input(room, status)),
        quiet("This is exactly what Baz searches for."),
    ]
    .spacing(TIGHT);
    // **The shelf you forgot about, as a filter on the request.** It sits
    // with the field rather than under the moods because it narrows *which
    // songs*, which is the field's own question — and it composes with every
    // mood rather than being a seventh one, which is what makes it worth
    // having at all.
    //
    // Absent until the ledger can answer it: a toggle that silently does
    // nothing is worse than a control that is not there.
    if !vibe.never_played.is_empty() {
        asked = asked.push(Space::new().height(theme::GAP_XS)).push(chip(
            &format!("Only the {} I've never played", vibe.never_played.len()),
            vibe.unplayed_only,
            Message::VibeUnplayedOnly(!vibe.unplayed_only),
        ));
    }
    if advanced {
        asked = asked.push(matches_note(vibe));
    }

    // 2. The two ways of writing it.
    let current = vibe.recipe();
    let mut ways = column![
        views::caption_word("START FROM"),
        wrap_chips(
            crate::vibe::Recipe::ALL
                .iter()
                .enumerate()
                .map(|(index, recipe)| {
                    chip(
                        recipe.label,
                        current == Some(index),
                        Message::VibeRecipe(index),
                    )
                })
                .collect(),
            3,
        ),
    ]
    .spacing(TIGHT);
    // The vocabulary is the query builder's own, so it belongs to the depth
    // that admits to being one — and it is **one** group under one label,
    // rather than two labels competing over six words each.
    if advanced {
        ways = ways
            .push(Space::new().height(theme::GAP_MD))
            .push(views::caption_word("OR ADD A WORD"));
        for name in crate::vibe::Chip::ROWS {
            ways = ways.push(wrap_chips(
                crate::vibe::Chip::ALL
                    .iter()
                    .enumerate()
                    .filter(|(_, chip)| chip.row == name)
                    .map(|(index, held)| chip(held.word, false, Message::VibeWord(index)))
                    .collect(),
                3,
            ));
        }
    }

    container(
        column![
            depth_tabs(vibe.depth),
            asked,
            ways,
            commitment(shelf, stage, layout),
        ]
        .spacing(APART),
    )
    .width(Length::Fill)
    .into()
}

/// The quietest voice on the pane: a caption, for something you glance at
/// rather than read.
fn quiet(line: &str) -> Element<'static, Message> {
    let room = theme::active();
    text(line.to_owned())
        .size(theme::SIZE_CAPTION)
        .line_height(theme::LEADING_CAPTION)
        .color(room.paper_faint)
        .width(Length::Fill)
        .wrapping(text::Wrapping::Word)
        .into()
}

/// **The live match count, and the three nearest** — design 21 §6's first
/// readout with this plan's addition beside it.
///
/// The count is the eligible set the words draw, by the same rule a compose
/// will apply, so it is a promise rather than a description — **of what Baz
/// will choose from, and of nothing else**. It does not say those songs are
/// what was asked for, because two of the six requests measured in
/// `docs/design/impl/vibe-eligibility/` were at or below chance and a
/// readout that said *match* would be asserting otherwise.
///
/// The three titles are the cheapest possible answer to *does Baz understand
/// my phrase*: type *slow sparse piano*, see a death-metal track first, and
/// you know before spending a compose. They are the only part of this readout
/// that can be graded, which is why they carry the instruction to grade
/// them.
///
/// It was four body lines of equal weight, which is a good part of what made
/// this pane a wall. It is a count in the pane's own voice with its evidence
/// in the caption voice under it: one thing to glance at, not four to read.
fn matches_note(vibe: &crate::vibe::State) -> Element<'_, Message> {
    let room = theme::active();
    if vibe.counting {
        return quiet("Counting…");
    }
    let Some(live) = &vibe.live else {
        return Space::new().into();
    };
    // **A count of nothing is not a count.** On a library Baz has never heard
    // the arithmetic is honest and the sentence is nonsense — *matches 0 songs
    // of the 0 Baz has heard* — so the readout says what is actually true.
    if live.analysed == 0 {
        return quiet("Baz has not heard anything yet — the count arrives as it listens.");
    }
    let head = if live.prompt.is_empty() {
        format!("All {} songs Baz has heard", live.analysed)
    } else {
        // **Not "match".** The sweep in `docs/design/impl/vibe-eligibility/`
        // put two of six test requests at or below chance against their own
        // genre, so a sentence reading *211 of 5 076 songs match* claims a
        // precision the retrieval does not have — and design note 23 §4 calls
        // that the worst failing available here, because it is a dishonesty
        // rather than a limitation. *Drew … to choose from* is the same
        // arithmetic without the claim: it says what Baz did, and leaves
        // whether it was right to the three titles underneath.
        format!(
            "Baz drew {} of {} to choose from",
            live.eligible, live.analysed
        )
    };
    let mut note = column![
        text(head)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .font(theme::MEDIUM)
            .color(room.paper_dim),
    ]
    .spacing(0.0);
    // And the evidence, with the grading asked for out loud. These three are
    // the cheapest way to find out that a phrase is not landing, and they are
    // only useful if somebody looks at them.
    if live.closest.is_empty() {
        if !live.prompt.is_empty() {
            note = note.push(quiet("Its best reading of your words, not a filter."));
        }
    } else {
        note = note.push(quiet(&format!(
            "Nearest your words: {} — if these are not what you meant, it has not \
             understood the phrase.",
            live.closest.join(" · ")
        )));
    }
    note.into()
}

/// **The one press, the length it commits to, and what pressing it again
/// does.**
///
/// Length in words on the commitment itself — the quorum's R6: *how long* is
/// part of what you are about to spend, not a setting somewhere above it. The
/// pills above it carry no label of their own for the same reason: the button
/// beneath them already says *about an hour*.
///
/// Its words change with what it can actually do. On a library Baz has never
/// heard it says so, and the offer to listen is the one accent-weight control
/// on screen, in the other pane; while it is listening it says how much it can
/// already compose from, which is true at every point of the bar.
fn commitment(shelf: &Shelf, stage: Stage, _layout: Layout) -> Element<'_, Message> {
    let vibe = &shelf.vibe;
    let ready = vibe.done.saturating_sub(vibe.failed);
    let (label, live) = match stage {
        // At most one commitment on screen: on a cold library the act on offer
        // is *Listen to my music*, in the result pane with its cost stated.
        Stage::Cold => ("Compose · needs listening first".to_owned(), false),
        Stage::Listening if vibe.has_features() => (format!("Compose from {ready} so far"), true),
        Stage::Listening => ("Compose · listening…".to_owned(), false),
        Stage::Ready => (
            format!("Compose · about {}", crate::vibe::spoken(vibe.length)),
            true,
        ),
    };
    column![
        wrap_chips(
            crate::vibe::MixLength::ALL
                .iter()
                .map(|length| {
                    chip(
                        crate::vibe::spoken(*length),
                        vibe.length == *length,
                        Message::VibeLength(*length),
                    )
                })
                .collect(),
            4,
        ),
        Space::new().height(theme::GAP_SM),
        views::page::commitment_marked(
            crate::icon::Glyph::Queue,
            label.into(),
            live,
            Message::VibeCreate,
        ),
        // What pressing it again will do, said rather than discovered.
        quiet(if vibe.preview.is_some() {
            "Press again for a different list from the same request."
        } else {
            "Every press composes a different list from the same request."
        }),
    ]
    .spacing(TIGHT)
    .into()
}
