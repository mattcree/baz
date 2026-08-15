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
    let room = theme::active();
    let vibe = &shelf.vibe;
    let mut page = column![heading("Start a smart playlist")]
        .spacing(theme::GAP_MD)
        .width(Length::Fill);
    page = page.push(views::hint(
        "Baz composes these from how your music actually sounds, and keeps them as \
         ordinary playlists you can edit.",
    ));

    // **The first step, first.** On a library Baz has never heard, no tile
    // below can do anything until this is done, so it stands above them with
    // its cost stated rather than waiting to be discovered.
    if stage == Stage::Cold {
        let tracks = crate::vibe::library_paths(&shelf.albums, &shelf.edition_choice).len();
        page = page.push(
            container(
                column![
                    text("First, Baz needs to listen")
                        .size(theme::SIZE_EMPHASIS)
                        .line_height(theme::LEADING_EMPHASIS)
                        .font(theme::MEDIUM)
                        .color(room.paper),
                    views::hint(&format!(
                        "It reads each track once — {tracks} of them, {}. Nothing is \
                         uploaded, the index is disposable, and you can stop and pick up \
                         where it left off.",
                        crate::vibe::listening_estimate(tracks)
                    )),
                    Space::new().height(theme::GAP_XS),
                    views::page::commitment_marked(
                        crate::icon::Glyph::Queue,
                        "Listen to my music".into(),
                        true,
                        Message::VibeAnalyze,
                    ),
                ]
                .spacing(theme::GAP_XS),
            )
            .padding(theme::GAP_MD)
            .max_width(STEP_W)
            .style(move |_theme| theme::segmented(room)),
        );
    } else if stage == Stage::Listening {
        let done = vibe.done.saturating_sub(vibe.failed);
        let left = vibe
            .total
            .saturating_sub(vibe.done.saturating_add(vibe.failed));
        page = page.push(views::hint(&format!(
            "Listening — {done} of {} heard, about {} left. Pick a mood whenever you like; \
             it will compose from what Baz has heard so far.",
            vibe.total,
            crate::vibe::listening_estimate(left)
        )));
    }

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
