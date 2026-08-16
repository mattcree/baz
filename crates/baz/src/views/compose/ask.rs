//! **Narrowing it down** — the filter column, and the readouts that say what
//! it caught.
//!
//! This band led the page until design note 25. The owner: *"if we treat
//! words as just a kind of filter… the curves make more sense up front."*
//! Everything here still writes into the one request; what changed is that it
//! no longer claims to *be* the request. The heading is a caption rather than
//! a question, the field says it is optional, and the page's one
//! emphasis-weight question is now the line's.
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
//!    as from the block above it. Within a group is [`TIGHT`]; between groups
//!    is the page's own [`theme::GAP_LG`], set where the column is assembled
//!    — which is what lets proximity do the grouping without a rule or a box.
//! 2. **Hierarchy by de-emphasis.** The field is the only thing here at body
//!    size, because it is the only thing here you type into — but the pane no
//!    longer opens with a heading, so nothing on it competes with the line's
//!    question for the page's primary voice.
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
//! Design 21 §4, as amended by note 25. The field has one sentence underneath
//! — *leave it empty and Baz draws from everything it has heard* — because
//! that is both what says there is no hidden state and what makes the
//! optionality true rather than merely tolerated. Beneath it, two ways of
//! writing it: **starting points** replace the line, **the vocabulary**
//! appends with a comma. Neither is a second input; there is one line, so
//! there is no way to have two of anything, and a starting point stops being
//! lit the moment the words change — not a mode switching off, a label
//! ceasing to be true.

use iced::widget::{Space, column, row, text, text_input};
use iced::{Element, Length};

use crate::app::{Message, Shelf};
use crate::views::compose::{Stage, chip, wrap_chips};
use crate::{theme, views};

/// Space **within** a group.
const TIGHT: f32 = theme::GAP_XS;

pub(crate) fn view(shelf: &Shelf) -> Element<'_, Message> {
    let room = theme::active();
    let vibe = &shelf.vibe;
    // **Folded away until asked for.** The words are the optional half, and
    // the untrustworthy half, and unfolded they are five rows — which is the
    // difference between `Compose` sitting above the fold and below it. A
    // request that arrived with words already in it opens itself.
    let open = vibe.words_open || !vibe.prompt.trim().is_empty();
    let mut band = column![
        row![
            chip("Only certain songs", open, Message::VibeWords(!open)),
            Space::new().width(Length::Fill),
            quiet(if open {
                "Optional"
            } else {
                "Baz will use every song it has heard"
            }),
        ]
        .align_y(iced::Alignment::Center)
    ]
    .spacing(TIGHT);
    if !open {
        return band.into();
    }

    band = band
        .push(
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
        )
        .push(matches_note(vibe));

    // The two ways of writing it, under one label rather than two.
    let current = vibe.recipe();
    // Two labels rather than one, because these are two different acts: a
    // mood **replaces** the words, a vocabulary word **adds** to them.
    let mut ways = column![views::caption_word("OR PICK A MOOD")].spacing(TIGHT);
    ways = ways.push(wrap_chips(
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
    ));
    ways = ways
        .push(Space::new().height(theme::GAP_SM))
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

    column![band, ways].spacing(theme::GAP_MD).into()
}

/// **How long**, as its own small block, so the length is not something you
/// find on a button after you have already decided to press it.
pub(crate) fn length(vibe: &crate::vibe::State) -> Element<'static, Message> {
    column![
        views::caption_word("HOW LONG"),
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
    ]
    .spacing(TIGHT)
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
        return quiet("Baz has not listened yet. The count appears as it does.");
    }
    let head = if live.prompt.is_empty() {
        format!("Using all {} of your songs", live.analysed)
    } else {
        // **Not "match".** The sweep in `docs/design/impl/vibe-eligibility/`
        // put two of six test requests at or below chance against their own
        // genre, so a sentence reading *211 of 5 076 songs match* claims a
        // precision the retrieval does not have — and design note 23 §4 calls
        // that the worst failing available here, because it is a dishonesty
        // rather than a limitation. *Drew … to choose from* is the same
        // arithmetic without the claim: it says what Baz did, and leaves
        // whether it was right to the three titles underneath.
        format!("Using {} of your {} songs", live.eligible, live.analysed)
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
            note = note.push(quiet(
                "Baz is guessing at your words, not filtering on them.",
            ));
        }
    } else {
        note = note.push(quiet(&format!(
            "Closest: {}. If these look wrong, try different words.",
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
pub(crate) fn commitment(shelf: &Shelf, stage: Stage) -> Element<'_, Message> {
    let vibe = &shelf.vibe;
    let ready = vibe.done.saturating_sub(vibe.failed);
    let (label, live) = match stage {
        // At most one commitment on screen: on a cold library the act on offer
        // is *Listen to my music*, in the result pane with its cost stated.
        Stage::Cold => ("Compose · Baz needs to listen first".to_owned(), false),
        Stage::Listening if vibe.has_features() => (format!("Compose from {ready} so far"), true),
        Stage::Listening => ("Compose · listening…".to_owned(), false),
        Stage::Ready => (
            format!("Compose · about {}", crate::vibe::spoken(vibe.length)),
            true,
        ),
    };
    column![
        views::page::commitment_marked(
            crate::icon::Glyph::Queue,
            label.into(),
            live,
            Message::VibeCreate,
        ),
        // What pressing it again will do, said rather than discovered.
        quiet("Press again for a different list."),
    ]
    .spacing(TIGHT)
    .into()
}
