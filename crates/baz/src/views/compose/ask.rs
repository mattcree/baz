//! **What do you want to hear?** — one band, one request, and the readouts
//! that say what it means.
//!
//! Design 21 §4. The **field is at the head of it**, with one sentence
//! underneath — *this is exactly what Baz searches for* — because that
//! sentence is what says there is no hidden state, nothing accumulating out of
//! sight, and that what you can read is what will happen. The engine now earns
//! that claim rather than merely making it: compose is deterministic, and the
//! freshness store that used to bias every list invisibly is gone.
//!
//! Beneath the field, two ways of writing it. **Starting points** replace the
//! line, visibly, and are editable at once. **The vocabulary** appends with a
//! comma. Neither is a second input; there is one line, so there is no way to
//! have two of anything, and a starting point stops being lit the moment the
//! words change — not a mode switching off, a label ceasing to be true.

use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Element, Length};

use crate::app::{Message, Shelf};
use crate::views::compose::{Layout, Stage, chip, heading, wrap_chips};
use crate::{theme, views};

pub(crate) fn view(shelf: &Shelf, stage: Stage, layout: Layout) -> Element<'_, Message> {
    let room = theme::active();
    let vibe = &shelf.vibe;
    let mut band = column![
        heading("What do you want to hear?"),
        text_input("warm analogue soul, unhurried", &vibe.prompt)
            .on_input(Message::VibePrompt)
            .on_submit(Message::VibeCreate)
            .width(Length::Fill)
            .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .style(move |_theme, status| theme::input(room, status)),
        views::hint("This is exactly what Baz searches for."),
    ]
    .spacing(theme::GAP_SM);

    // **The live readouts.** A count says how many; the three titles say how
    // well, which is the question a listener actually has. Together they are
    // the difference between a text box and a control somebody can learn.
    band = band.push(matches_note(vibe));

    // **Starting points.** Six named moods; pressing one replaces the line
    // with its words. A starting point also sets the shape and the length —
    // but only while the listener has not set them themselves. Drag a point
    // once and they are yours; from then on a mood changes the words and
    // nothing else, which is the sort of effect that is invisible when it is
    // right.
    let current = vibe.recipe();
    band = band
        .push(views::caption_word("A PLACE TO START"))
        .push(wrap_chips(
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
            2,
        ));

    // **The vocabulary.** Twelve words in two rows, each chosen by measurement
    // — `docs/design/impl/vibe-eligibility/`, finding 6 — because there is no
    // language model here and the text tower answers descriptive phrases about
    // sound. Telling somebody to describe the sound and not the story without
    // giving them the words is a scold.
    for name in crate::vibe::Chip::ROWS {
        band = band
            .push(views::caption_word(&name.to_uppercase()))
            .push(wrap_chips(
                crate::vibe::Chip::ALL
                    .iter()
                    .enumerate()
                    .filter(|(_, chip)| chip.row == name)
                    .map(|(index, held)| chip(held.word, false, Message::VibeWord(index)))
                    .collect(),
                3,
            ));
    }

    band = band
        .push(Space::new().height(theme::GAP_SM))
        .push(commitment(shelf, stage, layout));
    container(band).width(Length::Fill).into()
}

/// **The live match count, and the three nearest** — design 21 §6's first
/// readout with this plan's addition beside it.
///
/// The count is the eligible set the words draw, by the same rule a compose
/// will apply, so *"matches 340 songs"* is a promise rather than a
/// description. The three titles are the cheapest possible answer to *does baz
/// understand my phrase*: type *slow sparse piano*, see a death-metal track
/// first, and you know before spending a compose.
fn matches_note(vibe: &crate::vibe::State) -> Element<'_, Message> {
    let room = theme::active();
    if vibe.counting {
        return views::hint("Counting…");
    }
    let Some(live) = &vibe.live else {
        // Nothing to count against yet, and a prompt to type is not a
        // readout. It goes rather than standing under the sentence that has
        // already said what the field is for.
        return Space::new().into();
    };
    // **A count of nothing is not a count.** On a library baz has never heard
    // the arithmetic is honest and the sentence is nonsense — *matches 0 songs
    // of the 0 Baz has heard* — so the readout says what is actually true
    // instead, which is that there is nothing to count against yet.
    let head = if live.analysed == 0 {
        "Baz has not heard anything yet — the count arrives as it listens.".to_owned()
    } else if live.prompt.is_empty() {
        format!(
            "No words yet — all {} songs Baz has heard are eligible.",
            live.analysed
        )
    } else {
        format!(
            "Matches {} songs of the {} Baz has heard.",
            live.eligible, live.analysed
        )
    };
    let mut note = column![
        text(head)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper)
            .width(Length::Fill)
            .wrapping(text::Wrapping::Word),
    ]
    .spacing(theme::GAP_XS);
    if !live.closest.is_empty() {
        note = note.push(views::hint("Closest so far:"));
        for title in &live.closest {
            note = note.push(
                text(format!("· {title}"))
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_dim)
                    .width(Length::Fill)
                    .wrapping(text::Wrapping::None),
            );
        }
    }
    note.into()
}

/// **The one press, and the length it commits to** — in minutes on the
/// commitment itself, which is the quorum's R6: *how long* is part of what you
/// are about to spend, not a setting somewhere above it.
///
/// Its words change with what it can actually do. On a library baz has never
/// heard it says so and offers to listen; while it is listening it says how
/// much it can already compose from, which is true at every point of the bar.
fn commitment(shelf: &Shelf, stage: Stage, _layout: Layout) -> Element<'_, Message> {
    let vibe = &shelf.vibe;
    let ready = vibe.done.saturating_sub(vibe.failed);
    let (label, message) = match stage {
        // **At most one commitment on screen.** On a library baz has never
        // heard, the act on offer is *Listen to my music* — it is in the
        // result pane with its cost stated — so this one says what it needs
        // and waits rather than competing with it for the accent.
        Stage::Cold => ("Compose · needs listening first".to_owned(), None),
        Stage::Listening if vibe.has_features() => (
            format!("Compose from {ready} so far"),
            Some(Message::VibeCreate),
        ),
        Stage::Listening => ("Compose · listening…".to_owned(), None),
        Stage::Ready => (
            format!("Compose · about {}", crate::vibe::spoken(vibe.length)),
            Some(Message::VibeCreate),
        ),
    };
    let lengths = wrap_chips(
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
    );
    let mut block = column![views::caption_word("HOW LONG"), lengths].spacing(theme::GAP_XS);
    block = block
        .push(Space::new().height(theme::GAP_XS))
        .push(views::page::commitment_marked(
            crate::icon::Glyph::Queue,
            label.into(),
            message.is_some(),
            Message::VibeCreate,
        ));
    if let Some(preview) = &vibe.preview {
        // *Compose again* states what it will replace, and only once there is
        // something to lose. **Another version** stands beside it as a
        // distinct, visible press — the one that carries the variation the
        // engine used to take invisibly on every compose.
        block = block
            .push(views::hint(&format!(
                "Composing again replaces the {} songs on the right.",
                preview.items.len()
            )))
            .push(
                row![
                    views::word_button_maybe("Another version", Some(Message::VibeAnother)),
                    Space::new().width(Length::Fill),
                ]
                .spacing(theme::GAP_SM),
            );
    }
    // **The other way to make a playlist**, as a quiet act rather than as a
    // fork asked before anything is shown. One press either way, and nobody
    // has to classify themselves to see the page.
    block
        .push(Space::new().height(theme::GAP_SM))
        .push(views::word_button_maybe(
            "…or start with an empty list",
            Some(Message::PlaylistCreationMode(
                crate::playlists::CreationMode::Manual,
            )),
        ))
        .into()
}
