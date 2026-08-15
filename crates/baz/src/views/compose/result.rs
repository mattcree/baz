//! **What you got** — design 21 §7's states 1, 2, 5, 6, 7 and 8, in the one
//! pane that carries all of them.
//!
//! The result is a **playlist, not a receipt** (the quorum's R8): ordinary
//! rows with reorder, remove and the same anatomy every other list in baz
//! draws, a name you can edit in place, and an `.m3u8` on disk at the end of
//! it exactly like every other list.
//!
//! Two readouts live here. The **diff** stands first, because it teaches the
//! most and is the cheapest thing in design 21 — one use and the whole model
//! is learnt without anybody being taught it. The **match ticks** stand on
//! each row: three of them, filled by strength, never a colour, because a
//! reading that rests on telling two hues apart is a reading some people
//! cannot make. Selecting a row explains it in three cues — the enlarged dot
//! on the line, the row's own tick, and the position number — and none of them
//! is a colour either.

use iced::widget::{Space, column, container, row, text};
use iced::{Element, Length};

use crate::app::{Message, Shelf};
use crate::playlists::Playlists;
use crate::views::compose::{Layout, Stage, heading};
use crate::{theme, views};

pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    stage: Stage,
    layout: Layout,
) -> Element<'a, Message> {
    let vibe = &shelf.vibe;
    match stage {
        Stage::Cold if vibe.preview.is_none() => cold(shelf),
        Stage::Listening if vibe.preview.is_none() => listening(vibe),
        _ => list(shelf, playlists, layout),
    }
}

/// **1 · Never listened**, which is where a new listener spends their entire
/// first session and which the shipping build did not design.
///
/// The ask pane beside this is fully drawn and fully pressable — set up the
/// request while it works — and the cost is stated rather than hidden, because
/// this is a consent decision and consent needs a number. The number is
/// measured: `docs/design/impl/vibe-memory/`, 4 490 tracks an hour at four
/// workers on a real library.
fn cold(shelf: &Shelf) -> Element<'_, Message> {
    let tracks = crate::vibe::library_paths(&shelf.albums, &shelf.edition_choice).len();
    column![
        heading("Baz has not listened to your music yet"),
        views::hint(&format!(
            "To compose from sound rather than from tags, Baz reads each track once — {tracks} \
             of them, {}. It keeps a disposable local index, nothing is uploaded, and you can \
             stop and pick up where it left off at any time.",
            crate::vibe::listening_estimate(tracks)
        )),
        Space::new().height(theme::GAP_SM),
        views::page::commitment_marked(
            crate::icon::Glyph::Queue,
            "Listen to my music".into(),
            true,
            Message::VibeAnalyze,
        ),
    ]
    .spacing(theme::GAP_SM)
    .into()
}

/// **2 · Listening** — a real reading rather than a spinner: how many, how
/// long is left, and a way to stop.
fn listening(vibe: &crate::vibe::State) -> Element<'_, Message> {
    let done = vibe.done.saturating_sub(vibe.failed);
    let left = vibe
        .total
        .saturating_sub(vibe.done.saturating_add(vibe.failed));
    let mut block = column![heading("Listening to your music")].spacing(theme::GAP_SM);
    if vibe.preparing {
        return block
            .push(views::hint("Checking what Baz has already heard…"))
            .into();
    }
    block = block
        .push(views::hint(&format!(
            "{done} of {} heard · about {} left · {} skipped",
            vibe.total,
            crate::vibe::listening_estimate(left),
            vibe.failed
        )))
        .push(views::hint(if vibe.has_features() {
            "You can compose from what it has heard so far; the rest keeps arriving."
        } else {
            "Composing begins as soon as the first songs are heard."
        }))
        .push(views::word_button_maybe(
            "Stop listening",
            Some(Message::VibeAnalysisCancel),
        ));
    if let Some(failure) = vibe.failure_note() {
        block = block.push(views::alert(&failure));
    }
    block.into()
}

/// **6 · A list**, and 7 and 8 with it: the result, its rows, what changed,
/// and what to call it.
#[expect(
    clippy::too_many_lines,
    reason = "one pane: the diff, the rows, the why-line and the save, in the order they are read"
)]
fn list<'a>(shelf: &'a Shelf, playlists: &'a Playlists, layout: Layout) -> Element<'a, Message> {
    let room = theme::active();
    let vibe = &shelf.vibe;
    let advanced = vibe.depth == crate::vibe::Depth::Advanced;
    let draft = &playlists.creation;
    let Some(preview) = &vibe.preview else {
        return column![
            heading("Ready when you are"),
            views::hint(
                "The shape on its own is a perfectly good request. Compose, and the songs \
                 appear here.",
            ),
        ]
        .spacing(theme::GAP_SM)
        .into();
    };

    let mut block = column![
        row![
            heading("Your list"),
            Space::new().width(Length::Fill),
            text(format!(
                "{} songs · {}",
                preview.items.len(),
                preview.duration_note()
            ))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint),
        ]
        .align_y(iced::Alignment::Center)
    ]
    // The heading and its count are one group — the count is *about* the
    // list rather than a second heading beside it — so it steps down a size
    // and an ink, and the rows below stand clear of both.
    .spacing(theme::GAP_MD);

    // **The diff first**, because it teaches the most. It is the query
    // builder explaining what it just did, so it belongs to the depth that
    // admits to being one.
    if let Some(diff) = preview.diff.as_ref().filter(|_| advanced) {
        block = block.push(
            container(
                column![
                    text(format!("{} new · {} kept", diff.fresh, diff.kept))
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .font(theme::MEDIUM)
                        .color(room.paper),
                    text(capitalised(&diff.cause))
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_dim)
                        .width(Length::Fill)
                        .wrapping(text::Wrapping::Word),
                ]
                .spacing(theme::GAP_XS),
            )
            .padding(theme::GAP_SM)
            .style(move |_theme| theme::segmented(room)),
        );
    }

    // **A request the library cannot fill says so, in numbers.** Nothing is
    // padded to reach the asked-for length — design 21 §12 — so the honest
    // move is to say what happened and offer the control that fixes it.
    if preview.items.len() < preview.asked_positions {
        block = block.push(views::alert(&format!(
            "Only {} of the {} songs asked for could be found: {} were eligible, and the \
             diversity rules will not take the same artist twice in a row. Lower the line, \
             widen the words, or ask for less.",
            preview.items.len(),
            preview.asked_positions,
            preview.eligible_tracks
        )));
    }

    if vibe.request_changed() {
        block = block.push(views::hint(
            "The request has changed — compose to bring this list up to date.",
        ));
    }

    for (position, item) in preview.items.iter().enumerate() {
        let selected = vibe.selected_row == Some(position);
        block = block.push(
            iced::widget::mouse_area(super::super::new_playlist::draft_row(
                shelf,
                item,
                position,
                preview.items.len(),
                layout.measure,
                vibe.hovered_row == Some(position) || selected,
                preview
                    .matches
                    .get(position)
                    .filter(|_| advanced)
                    .map(|found| found.ticks),
                &super::super::new_playlist::DraftEdits {
                    shift: &|row, delta| Message::VibePreviewShift(row, delta),
                    remove: &Message::VibePreviewRemove,
                },
            ))
            .on_press(Message::VibePreviewSelected(position))
            .on_enter(Message::VibePreviewEntered(position))
            .on_exit(Message::VibePreviewLeft(position)),
        );
        // **The why-line**, under the row it explains, as a rank and never a
        // score: *your words let it in; your line put it fourth.*
        if selected
            && advanced
            && let Some(why) = vibe.why(position)
        {
            block = block.push(
                container(
                    text(why)
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_dim)
                        .width(Length::Fill)
                        .wrapping(text::Wrapping::Word),
                )
                .padding(theme::pad(theme::GAP_XS, theme::GAP_SM)),
            );
        }
    }

    // **8 · Saved.** The name is proposed from the request and editable in
    // place; what lands on disk is the same `.m3u8` as every other list.
    let can_act = !preview.items.is_empty();
    let save_enabled = playlists.creation_can_save(can_act);
    block = block
        .push(Space::new().height(theme::GAP_SM))
        .push(views::caption_word("NAME"))
        .push(views::name_input(&draft.name));
    if let Some(reason) = playlists.creation_refusal() {
        block = block.push(views::alert(&reason));
    }
    block
        .push(
            row![
                views::word_button_maybe("Play", can_act.then_some(Message::VibePlay)),
                views::word_button_maybe(
                    "Save playlist",
                    (can_act && save_enabled).then_some(Message::VibeSubmit),
                ),
            ]
            .spacing(theme::GAP_SM),
        )
        .into()
}

/// The diff's sentence, which is assembled lower-case so it can be quoted
/// mid-line, given a capital where it starts one.
fn capitalised(sentence: &str) -> String {
    let mut characters = sentence.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + characters.as_str()
    })
}
