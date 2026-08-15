//! **The smart playlist's own door**: six moods as tiles, and — on a library
//! Baz has never heard — the one step that has to come first, standing where
//! it can be seen.
//!
//! The owner, on the shipped page: *"the weird dependency on 'listening to my
//! music' seems to be a first step before using this feature rather than
//! buried."* It was buried: three screens in, in the corner of a result pane,
//! under a form asking for words that could not be used yet. It is the first
//! thing here, because it is the first thing.
//!
//! And: *"can we add a second New Playlist option specifically for these
//! Smart playlists? and again, can we have maybe 5-6 presets at that level as
//! tiles where when a user selects it, it creates a new one."* Six tiles.
//! Pressing one composes and lands on the result, with the form behind it
//! where it is useful rather than in front of it where it is a toll.
//!
//! This is a **door, not a wizard step**. There is no *next*, nothing to fill
//! in, and no way to be halfway through it: every tile is a finished request
//! and the seventh way in is to write your own words.

use iced::widget::{Space, column, container, row, text};
use iced::{Element, Length};

use crate::app::{Message, Shelf};
use crate::views::compose::{Layout, Stage, heading};
use crate::{theme, views};

/// A mood tile's measure. Smaller than a record's sleeve — these are
/// *requests*, not works, and a wall of half-metre moods would claim they were
/// the same kind of thing.
const TILE: f32 = 176.0;

/// …and its height, **fixed**, so the rows align. The words under each name
/// run to one, two or three lines depending on the mood, and a grid whose
/// tiles each stood at their own content's height would be a ragged edge
/// pretending to be a wall.
const TILE_H: f32 = 96.0;

/// The listening step's own measure. It is a paragraph and a press, not a
/// banner: at the full width of a wide window its button would be a metre of
/// accent for one act.
const STEP_W: f32 = 560.0;

pub(crate) fn view(shelf: &Shelf, stage: Stage, layout: Layout) -> Element<'_, Message> {
    let vibe = &shelf.vibe;
    let mut page = column![heading("Start a smart playlist")]
        .spacing(theme::GAP_MD)
        .width(Length::Fill);
    page = page.push(views::hint(
        "Baz composes these from how your music actually sounds, and keeps them as \
         ordinary playlists you can edit.",
    ));

    // **The listening step, in whatever state it is actually in.**
    //
    // It said its piece only while nothing at all had been heard, so pressing
    // *Listen to my music* made the block vanish and put nothing in its
    // place — no progress, no confirmation, nothing. And once a single track
    // was analysed the door never mentioned listening again, however much of
    // the library was still unheard. Both read as *the feature does not
    // work*, and the second is worse: it is a library that quietly stays half
    // heard.
    //
    // So the block is always here, and says which of the three things is
    // true.
    let heard = vibe.analysed();
    let library = crate::vibe::library_paths(&shelf.albums, &shelf.edition_choice).len();
    let unheard = library.saturating_sub(heard);
    page = page.push(step(vibe, stage, heard, library, unheard));

    page = page
        .push(Space::new().height(theme::GAP_SM))
        .push(views::caption_word("PICK ONE"));

    // The moods, as tiles. Pressing one *is* the act — it fills the request
    // and composes — which is what the owner asked for and what makes this a
    // door rather than a form with pictures on it.
    let per_row = if layout.side_by_side { 3 } else { 2 };
    let mut grid = column![].spacing(theme::GAP_MD);
    let mut line = row![].spacing(theme::GAP_MD);
    let mut in_line = 0;
    for (index, recipe) in crate::vibe::Recipe::ALL.iter().enumerate() {
        line = line.push(tile(
            recipe.label,
            recipe.prompt,
            Message::VibeRecipe(index),
        ));
        in_line += 1;
        if in_line == per_row {
            grid = grid.push(line);
            line = row![].spacing(theme::GAP_MD);
            in_line = 0;
        }
    }
    // The seventh way in, in the same anatomy so it is plainly one of the
    // options rather than an escape hatch beside them.
    line = line.push(tile(
        "Your own words",
        "Describe the music yourself, and shape how it moves.",
        Message::VibeStartBlank,
    ));
    grid = grid.push(line);

    page.push(grid).into()
}

/// **Where listening stands**, in one block that is always drawn.
///
/// Three states and a fourth that is really the first again: nothing heard,
/// hearing it now, all heard, and *some* heard with the rest waiting. The
/// last is the one that was missing, and it is the ordinary case — a library
/// grows, and a scan can be stopped.
fn step(
    vibe: &crate::vibe::State,
    stage: Stage,
    heard: usize,
    library: usize,
    unheard: usize,
) -> Element<'_, Message> {
    let room = theme::active();
    let (title, detail, offer) = match stage {
        Stage::Listening if vibe.preparing => (
            "Baz is looking at what it has already heard".to_owned(),
            "This takes a moment on a large library.".to_owned(),
            None,
        ),
        Stage::Listening => {
            let done = vibe.done.saturating_sub(vibe.failed);
            (
                format!("Listening — {done} of {} songs", vibe.total),
                format!(
                    "About {} left. Pick a mood whenever you like; it composes from what \
                     Baz has heard so far.",
                    crate::vibe::listening_estimate(
                        vibe.total
                            .saturating_sub(vibe.done.saturating_add(vibe.failed))
                    )
                ),
                Some(("Stop listening", Message::VibeAnalysisCancel)),
            )
        }
        // Nothing heard: the first step, and nothing below it can do anything
        // until it is taken.
        Stage::Cold => (
            "First, Baz needs to listen".to_owned(),
            format!(
                "It reads each track once — {library} of them, {}. Nothing is uploaded, the \
                 index is disposable, and you can stop and pick up where it left off.",
                crate::vibe::listening_estimate(library)
            ),
            Some(("Listen to my music", Message::VibeAnalyze)),
        ),
        // **Some heard, some not** — the state that was invisible. A library
        // grows and a scan can be stopped, so this is the ordinary case rather
        // than an edge one.
        Stage::Ready if unheard > 0 => (
            format!("Baz has heard {heard} of your {library} songs"),
            format!(
                "The {unheard} it has not heard yet cannot appear in a smart playlist. \
                 Reading them takes about {}.",
                crate::vibe::listening_estimate(unheard)
            ),
            Some(("Listen to the rest", Message::VibeAnalyze)),
        ),
        Stage::Ready => (
            format!("Baz has heard all {heard} of your songs"),
            "Pick a mood below, or write your own words.".to_owned(),
            None,
        ),
    };
    let mut block = column![
        text(title)
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .font(theme::MEDIUM)
            .color(room.paper),
        views::hint(&detail),
    ]
    .spacing(theme::GAP_XS);
    if let Some((label, message)) = offer {
        block =
            block
                .push(Space::new().height(theme::GAP_XS))
                .push(views::page::commitment_marked(
                    crate::icon::Glyph::Queue,
                    label.into(),
                    true,
                    message,
                ));
    }
    if let Some(failure) = vibe.failure_note() {
        block = block.push(views::alert(&failure));
    }
    container(block)
        .padding(theme::GAP_MD)
        .max_width(STEP_W)
        .style(move |_theme| theme::segmented(room))
        .into()
}

/// One mood, as a pressable card: its name, and the words it will actually
/// send — because a tile whose name is the whole of the information is a tile
/// you have to press to understand.
fn tile<'a>(name: &'a str, words: &'a str, press: Message) -> Element<'a, Message> {
    let room = theme::active();
    iced::widget::button(
        column![
            text(name)
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .color(room.paper),
            text(words)
                .size(theme::SIZE_CAPTION)
                .line_height(theme::LEADING_CAPTION)
                .color(room.paper_faint)
                .width(Length::Fill)
                .wrapping(text::Wrapping::Word),
        ]
        .spacing(theme::GAP_XS)
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true),
    )
    .width(Length::Fixed(TILE))
    .height(Length::Fixed(TILE_H))
    .padding(theme::GAP_MD)
    .style(move |_theme, status| theme::pill(room, room.wall, status, false))
    .on_press(press)
    .into()
}
