//! The canonical, resumable Manual/Vibe playlist-creation place.
//!
//! # The flow was reviewed on 2026-08-15 and rebuilt
//!
//! The owner: *"we need to examine the flow for the vibe playlist. the ux is
//! terrible and it makes no sense right now."* Six things were wrong with it,
//! and each is answered here rather than in a note:
//!
//! 1. **Four names for one act.** He picked `Vibe`, a rule said `Make a mix`,
//!    a button said `Create mix`, the save said `Save playlist`, and Home's
//!    door said `Make a vibe playlist`. One vocabulary now: the place makes a
//!    **playlist**; the two ways in are **Manual** and **Vibe**; the Vibe
//!    route **composes**, and what it composes is a playlist you name and
//!    save.
//! 2. **The order was inverted.** `Shape the journey` — the energy shape and
//!    the waypoints, which exist to *inform* the request — stood below the
//!    button that spends the request, and `Save playlist` stood above the
//!    name field it needs. The form reads top to bottom now: describe, shape,
//!    compose, review, name, save.
//! 3. **The consent gate stood in the middle of the flow.** A first run was
//!    prompt → `Create mix` → a paragraph → a second, differently named
//!    button. The engine never needed two presses: `Message::VibeCreate`
//!    already starts the analysis and composes when it lands
//!    (`App`'s `VibePrepared`/`VibeAnalyzed` arms honour `awaiting_create`).
//!    So the paragraph moved **above** the press, where consent belongs, and
//!    the second button is gone.
//! 4. **Two first screens.** Home's shortcut opens this place with Vibe
//!    already chosen; the Playlists wall's ghost tile opens the fork. Both are
//!    kept — a shortcut that skips a fork is a shortcut — but they now land on
//!    the same drawing, with the same way back to the fork.
//! 5. **Manual and Vibe were not the same act twice.** Manual's rows were
//!    bare `Up | Down | Remove` word buttons with no artwork while Vibe's were
//!    `page::track_row` with the shared slots. Both draw [`draft_row`] now,
//!    and both hold `QueueItemVm`s, which is what made that possible.
//! 6. **The composer lived in `views::home`.** It had exactly one caller and
//!    it was this place. It lives here; Home keeps the door.

use std::path::Path;

use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::playlists::{CreationMode, Playlists};
use crate::vm::QueueItemVm;
use crate::{theme, views};

pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    player: &'a PlayerState,
    width: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let draft = &playlists.creation;
    let header = views::place_header_with(
        "New playlist",
        Some("Manual and Vibe become the same ordinary playlist"),
    );
    let body: Element<'a, Message> = match draft.mode {
        None => column![
            text("How would you like to begin?")
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .color(room.paper),
            choice(
                "Manual",
                "Start with an empty list, name it, then add tracks.",
                Message::PlaylistCreationMode(CreationMode::Manual),
            ),
            choice(
                "Vibe",
                "Describe a journey through your music. Baz composes it here, on this device.",
                Message::PlaylistCreationMode(CreationMode::Vibe),
            ),
        ]
        .spacing(theme::GAP_MD)
        .into(),
        Some(CreationMode::Manual) => manual_form(shelf, playlists, width),
        Some(CreationMode::Vibe) => vibe_form(shelf, playlists, player, width),
    };
    column![
        header,
        scrollable(container(body).padding(views::place_pad()))
            .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()))
            .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
            .width(Length::Fill)
            .height(Length::Fill)
    ]
    .into()
}

/// **Manual**: name it, then add tracks to it from anywhere in the product.
fn manual_form<'a>(shelf: &'a Shelf, playlists: &'a Playlists, width: f32) -> Element<'a, Message> {
    let draft = &playlists.creation;
    let mut form = column![
        back_button(),
        caption("MANUAL"),
        named("PLAYLIST NAME", name_input(&draft.name)),
        hint("Use the app-bar search and choose Add to playlist. Nothing is written until Save."),
    ]
    .spacing(theme::GAP_SM);
    for (index, item) in draft.items.iter().enumerate() {
        form = form.push(draft_row(
            shelf,
            item,
            index,
            draft.items.len(),
            width,
            &|row, delta| Message::PlaylistCreationShift(row, delta),
            &Message::PlaylistCreationRemove,
        ));
    }
    if let Some(reason) = playlists.creation_refusal() {
        form = form.push(error(reason));
    }
    form.push(action_button(
        "Save playlist",
        playlists
            .creation_can_save(false)
            .then_some(Message::PlaylistCreationSave),
    ))
    .into()
}

/// **Vibe**: describe it, shape it, compose it, review it, name it, save it —
/// in that order, in three named blocks.
///
/// The owner, on the version before this one: *"the UX/layout seems
/// inconsistent e.g. header design different font and small. the layout and
/// ux isn't good in terms of just poor use of colour, contrast, layout,
/// iconography, and no explanation text."* All of that was true, and the
/// remedies are the product's own rather than new inventions:
///
/// - the place wears the **identity block** every subject page wears
///   ([`views::page::identity_block`]) instead of a caption in the caption
///   voice;
/// - its parts are named by the **section rule** the record page's `TRACKS`
///   and `DETAILS` use, so this reads as one of baz's pages;
/// - the one press is the **commitment** control `Play album` is, with a mark
///   of its own — nothing else on the page carries that weight, which is what
///   makes it findable;
/// - the turn controls are the **stepper** pair Settings already draws, so
///   `+`/`−` are marks rather than sentences;
/// - and every block opens with **one line saying what it is for**, because a
///   control nobody can explain is a control nobody uses.
#[expect(
    clippy::too_many_lines,
    reason = "one visible flow: every state of the request and its result, in the order they are read"
)]
fn vibe_form<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    _player: &'a PlayerState,
    width: f32,
) -> Element<'a, Message> {
    let room = theme::active();
    let state = &shelf.vibe;
    let draft = &playlists.creation;
    let mut form = column![back_button()].spacing(theme::GAP_MD);
    if !playlists.available() {
        return form
            .push(hint("Playlist storage is unavailable on this system."))
            .into();
    }
    if !cfg!(feature = "vibe-analysis") {
        return form
            .push(hint(
                "This is the light build. Install the full build to add local sonic analysis.",
            ))
            .into();
    }

    let busy = state.preparing || state.analyzing;
    let cold = !state.has_features() && !busy;
    form = form.push(views::page::identity_block(views::page::Identity {
        name: "Vibe".to_owned(),
        face: theme::SEMIBOLD,
        edit: None,
        byline: "A playlist composed from the music on this device".to_owned(),
        facts: if state.analyzed() == 0 {
            "Nothing analysed yet · your audio never leaves Baz".to_owned()
        } else {
            format!(
                "{} tracks analysed · your audio never leaves Baz",
                state.analyzed()
            )
        },
        beside_facts: None,
    }));

    // 0. **A mood to start from**, asked before anything is typed. A recipe
    //    fills the words, the shape and the length and changes nothing else,
    //    so the form it leaves behind is the one a listener would have filled
    //    in themselves — and every field stays theirs to change.
    form = form
        .push(views::section_rule("Start from"))
        .push(hint(
            "A common mood, filled in for you. Everything it sets can be changed.",
        ))
        .push(recipes_row(state));

    // 1. **The words** — what goes in.
    form = form
        .push(views::section_rule("The words"))
        .push(hint(
            "Describe the music you want. The words choose what is in the list.",
        ))
        .push(
            text_input("Try “dreamy shoegaze for a rainy evening”", &state.prompt)
                .on_input(Message::VibePrompt)
                .on_submit(Message::VibeCreate)
                .width(Length::Fill)
                .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .style(move |_theme, status| theme::input(room, status)),
        )
        .push(
            row![
                word_button(
                    "Late-night focus",
                    Message::PlaylistCreationExample("Late-night focus"),
                ),
                word_button(
                    "Warm Sunday morning",
                    Message::PlaylistCreationExample("Warm Sunday morning"),
                ),
                word_button(
                    "Restless then calm",
                    Message::PlaylistCreationExample("Restless then calm"),
                ),
            ]
            .spacing(theme::GAP_SM),
        );

    // 2. **The shape** — where in the library the music sits as the list goes
    //    on. It stands above the press that spends it.
    form = form
        .push(views::section_rule("The shape"))
        .push(hint(
            "Each line is one thing Baz can measure, drawn across the playlist: \
             left is the first track, right is the last, and height is where a \
             track sits against the rest of your library. Drag the points.",
        ))
        .push(shapes_row(state))
        .push(hint(contour_note(state)));
    for (index, lane) in state.contour.lanes.iter().enumerate() {
        form = form.push(lane_block(state, index, lane));
    }
    form = form.push(dimensions_row(state));

    // 3. **The list** — the one press, and everything it produces.
    form = form.push(views::section_rule("The list"));
    if cold {
        form = form.push(hint(&format!(
            "First run: Baz reads {} tracks once, keeps a disposable local index, \
             and never uploads audio. Composing starts as soon as it can, and you \
             can cancel or keep listening.",
            crate::vibe::library_paths(&shelf.albums, &shelf.edition_choice).len()
        )));
    }
    let can_create = !state.preparing
        && !state.prompt.trim().is_empty()
        && (!state.analyzing || state.has_features());
    form = form.push(
        row![
            column![caption("LENGTH"), length_picker(state.length)].spacing(theme::GAP_XS),
            Space::new().width(Length::Fill),
            container(views::page::commitment_marked(
                crate::icon::Glyph::Queue,
                if cold { "Analyse & compose" } else { "Compose" },
                can_create,
                Message::VibeCreate,
            ))
            .width(Length::Fixed(theme::COMMITMENT_W)),
        ]
        .spacing(theme::GAP_MD)
        .align_y(iced::Alignment::End),
    );

    // What is happening, while it happens.
    if state.preparing {
        form = form.push(hint("Checking the local analysis index…"));
    }
    if state.analyzing {
        let current = state
            .current
            .as_deref()
            .map_or("next track", crate::vibe::seed_name);
        form = form
            .push(hint(&format!(
                "Analysing {} of {} · {} · {} skipped",
                state.done.saturating_add(state.failed).saturating_add(1),
                state.total,
                current,
                state.failed
            )))
            .push(hint(&if state.has_features() {
                format!(
                    "A playlist can use the {} tracks analysed so far; the scan continues in the background.",
                    state.done.saturating_sub(state.failed)
                )
            } else {
                "Composing begins as soon as the first tracks are analysed.".to_owned()
            }))
            .push(word_button("Cancel analysis", Message::VibeAnalysisCancel));
    }
    if let Some(failure) = state.failure_note() {
        form = form.push(
            text(failure)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.alert)
                .width(Length::Fill)
                .wrapping(text::Wrapping::Word),
        );
    }

    // What you got, what to call it, what to do with it.
    if let Some(preview) = &state.preview {
        form = form
            .push(
                text(format!(
                    "“{}” · {} · {} tracks",
                    preview.request,
                    preview.duration_note(),
                    preview.items.len()
                ))
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .color(room.paper),
            )
            .push(hint(&preview.pool_note()));
        if state.request_changed() {
            form = form.push(hint("Request changed · Compose to update this list"));
        }
        // **Every row reports the pointer**, so the lines above can light that
        // track's own place on each of them — the owner's *"when we hover the
        // playlist items it is showing where on the curve it's meant to be…
        // so a person can see it really worked."*
        for (position, item) in preview.items.iter().enumerate() {
            form = form.push(
                iced::widget::mouse_area(draft_row(
                    shelf,
                    item,
                    position,
                    preview.items.len(),
                    width,
                    &|row, delta| Message::VibePreviewShift(row, delta),
                    &Message::VibePreviewRemove,
                ))
                .on_enter(Message::VibePreviewHovered(Some(position)))
                .on_exit(Message::VibePreviewHovered(None)),
            );
        }
        let can_act = !preview.items.is_empty();
        let save_enabled = playlists.creation_can_save(can_act);
        form = form.push(named("PLAYLIST NAME", name_input(&draft.name)));
        if let Some(reason) = playlists.creation_refusal() {
            form = form.push(error(reason));
        }
        form = form.push(
            row![
                word_button_maybe("Play", can_act.then_some(Message::VibePlay)),
                word_button_maybe(
                    "Save playlist",
                    (can_act && save_enabled).then_some(Message::VibeSubmit),
                ),
                word_button_maybe(
                    "Compose again",
                    (!state.request_changed()).then_some(Message::VibeAnother),
                ),
            ]
            .spacing(theme::GAP_SM),
        );
    } else if state.has_features() && !busy && state.open {
        form = form.push(hint(
            "No tracks could satisfy this request without breaking the diversity rules. Try a shorter playlist or a broader description.",
        ));
    }

    // Maintenance, at the foot and out of the flow: re-reading the library is
    // not a step in making a playlist.
    if state.has_features() && !busy {
        form = form
            .push(views::section_rule("Local analysis"))
            .push(hint(
                "Baz reads new music the next time it composes. Re-read everything only if a file changed under it.",
            ))
            .push(word_button("Re-analyse the library", Message::VibeAnalyze));
    }
    form.width(Length::Fill).into()
}

/// **One line of the contour**, with its name, what it measures, its axis
/// words, its turn controls and — once a list exists — where every track of
/// that list landed on it.
fn lane_block<'a>(
    state: &'a crate::vibe::State,
    index: usize,
    lane: &'a crate::vibe::Lane,
) -> Element<'a, Message> {
    let room = theme::active();
    let (low, high) = lane.dimension.ends();
    let head = row![
        text(lane.dimension.label())
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .font(theme::MEDIUM)
            .color(room.paper),
        text(lane.dimension.measured_from())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
        Space::new().width(Length::Fill),
        stepper(
            crate::icon::Glyph::Minus,
            "One fewer turn",
            state.can_remove_point(index),
            Message::ContourPointRemoved(index),
        ),
        stepper(
            crate::icon::Glyph::Plus,
            "Another turn",
            state.can_add_point(index),
            Message::ContourPointAdded(index),
        ),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center);
    let canvas = row![
        column![
            axis_word(high),
            Space::new().height(Length::Fill),
            axis_word(low),
        ]
        .height(Length::Fixed(theme::CONTOUR_H))
        .width(Length::Fixed(theme::CONTOUR_AXIS_W))
        .align_x(alignment::Horizontal::Right),
        crate::contour::Contour::new(&lane.points, room, theme::CONTOUR_H)
            .field(state.field_of(lane.dimension))
            .result(result_dots(state, index))
            .highlight(state.hovered_row)
            .on_drag(move |point, at, level| Message::ContourDragged(index, point, at, level))
            .on_release(Message::ContourReleased),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center);
    let foot = row![
        Space::new().width(Length::Fixed(theme::CONTOUR_AXIS_W + theme::GAP_SM)),
        axis_word("first track"),
        Space::new().width(Length::Fill),
        hovered_note(state, index).map_or_else(
            || axis_word("last track"),
            |note| text(note)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper)
                .wrapping(text::Wrapping::None)
                .into()
        ),
    ]
    .align_y(iced::Alignment::Center);
    column![head, canvas, foot].spacing(theme::GAP_XS).into()
}

/// **The dimensions on offer**, as a row of pressable words: the ones drawn
/// are lit, the rest are quiet, and pressing one adds or removes its line.
///
/// The owner: *"can we have more than one of these for different musical
/// dimensions — this obviously kinda rolls up several aspects of a song into
/// one value."* `Energy` is that roll-up and stays; the others are its parts,
/// each measurable on its own.
fn dimensions_row(state: &crate::vibe::State) -> Element<'_, Message> {
    let room = theme::active();
    let mut offered = row![]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center);
    for dimension in crate::vibe::Dimension::ALL {
        let drawn = state.contour.has(dimension);
        let live = drawn || state.can_add_lane();
        offered = offered.push(
            button(
                text(dimension.label())
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .font(theme::MEDIUM)
                    .color(if drawn {
                        room.paper
                    } else if live {
                        room.paper_dim
                    } else {
                        room.paper_muted
                    }),
            )
            .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
            .style(move |_theme, status| theme::tile(room, status, drawn))
            .on_press_maybe(live.then_some(Message::ContourDimension(dimension))),
        );
    }
    column![
        row![
            text("LINES")
                .size(theme::SIZE_CAPTION)
                .line_height(theme::LEADING_CAPTION)
                .font(theme::MEDIUM)
                .color(room.paper_faint),
            Space::new().width(theme::GAP_SM),
            offered,
        ]
        .align_y(iced::Alignment::Center),
        hint(
            "Up to three at once. Every line you draw is another thing your library \
             has to satisfy at the same time.",
        ),
    ]
    .spacing(theme::GAP_XS)
    .into()
}

/// A `+`/`−` mark in a control box — Settings' own stepper, so a control that
/// adds or removes one thing looks the same wherever it is.
fn stepper(
    glyph: crate::icon::Glyph,
    name: &'static str,
    enabled: bool,
    message: Message,
) -> Element<'static, Message> {
    let room = theme::active();
    let mark = container(
        iced::widget::image(crate::icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::glyph_opacity(enabled, false)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    iced::widget::tooltip(
        button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.wall, status))
            .on_press_maybe(enabled.then_some(message)),
        text(name)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        iced::widget::tooltip::Position::Top,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// **The moods on offer**, as a row of pressable words. The one the request
/// currently matches is lit, and it stops being lit the moment a word or a
/// point is changed — because from then on the request is the listener's
/// rather than the recipe's.
fn recipes_row(state: &crate::vibe::State) -> Element<'_, Message> {
    let room = theme::active();
    let current = state.recipe();
    let mut offered = row![].spacing(theme::GAP_SM);
    for (index, recipe) in crate::vibe::Recipe::ALL.iter().enumerate() {
        let lit = current == Some(index);
        offered = offered.push(
            button(
                text(recipe.label)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .font(theme::MEDIUM)
                    .color(if lit { room.paper } else { room.paper_dim }),
            )
            .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
            .style(move |_theme, status| theme::tile(room, status, lit))
            .on_press(Message::VibeRecipe(index)),
        );
    }
    offered.into()
}

/// **The shapes, as pictures** — each preset drawn by the same widget that
/// draws the line itself, at thumbnail height with its handles off.
///
/// A label under a picture rather than a label instead of one: the words
/// `Peak and fall` describe the shape, and the shape shows it. `Any` is the
/// first and is the honest way to say *the words alone*, which has to stay
/// reachable now that a line is the default.
fn shapes_row(state: &crate::vibe::State) -> Element<'_, Message> {
    let room = theme::active();
    let mut shapes = row![].spacing(theme::GAP_SM);
    for (index, shape) in crate::vibe::Shape::ALL.iter().enumerate() {
        let points = SHAPE_POINTS[index].as_slice();
        let current = state
            .contour
            .lane(0)
            .is_some_and(|lane| lane.points == points)
            || (points.is_empty() && state.contour.lanes.is_empty());
        shapes = shapes.push(
            button(
                column![
                    container(
                        crate::contour::Contour::<Message>::new(
                            points,
                            room,
                            theme::CONTOUR_THUMB_H,
                        )
                        .marks(false)
                    )
                    .width(Length::Fixed(theme::CONTOUR_THUMB_W)),
                    text(shape.label)
                        .size(theme::SIZE_CAPTION)
                        .line_height(theme::LEADING_CAPTION)
                        .font(theme::MEDIUM)
                        .color(if current {
                            room.paper
                        } else {
                            room.paper_faint
                        }),
                ]
                .spacing(theme::GAP_XS)
                .align_x(alignment::Horizontal::Center),
            )
            .padding(theme::GAP_XS)
            .style(move |_theme, status| theme::tile(room, status, current))
            .on_press(Message::ContourShape(index)),
        );
    }
    shapes.into()
}

/// The shapes' own points, materialised once per frame so the thumbnails can
/// borrow them and the current one can be recognised by its geometry rather
/// than by a name the state would otherwise have to carry.
static SHAPE_POINTS: std::sync::LazyLock<Vec<Vec<crate::vibe::ContourPoint>>> =
    std::sync::LazyLock::new(|| {
        crate::vibe::Shape::ALL
            .iter()
            .map(|shape| shape.points())
            .collect()
    });

/// What the line is asking for, in one line of words — because a picture of a
/// request should still be *sayable*, and because the two ends are what a
/// listener checks before spending a minute of analysis.
fn contour_note(state: &crate::vibe::State) -> &'static str {
    let Some(opening) = state.contour.level_at(0, 0.0) else {
        return "No shape — the words alone decide the order.";
    };
    let landing = state.contour.level_at(0, 1.0).unwrap_or(opening);
    let rise = landing - opening;
    if rise > 0.6 {
        "Opens at the calm end of your library and climbs."
    } else if rise < -0.6 {
        "Opens high and comes down."
    } else if state
        .contour
        .lane(0)
        .is_some_and(|lane| lane.points.len() > 2)
    {
        "Turns on the way through and ends where it started."
    } else {
        "Holds one level the whole way."
    }
}

/// **What one hovered track did**, in words: where it is in the list, what
/// the shape asked for at that position, and where it actually landed.
///
/// The picture already shows it — the lit dot on its guide — and this says
/// the same thing in a sentence, because *"a person can see it really
/// worked"* is easier to be sure of when the two agree. It never claims more
/// than it knows: the levels are the collection's own, and the words for them
/// are bands rather than numbers.
fn hovered_note(state: &crate::vibe::State, lane: usize) -> Option<String> {
    let preview = state.preview.as_ref()?;
    let row = state.hovered_row?;
    let item = preview.items.get(row)?;
    let landed = *preview.levels.get(lane)?.get(row)?;
    let last = preview.items.len().saturating_sub(1).max(1);
    let at = f32::from(u16::try_from(row).unwrap_or(u16::MAX))
        / f32::from(u16::try_from(last).unwrap_or(u16::MAX));
    let asked = state.contour.level_at(lane, at);
    let position = format!("{} of {}", row + 1, preview.items.len());
    Some(asked.map_or_else(
        || format!("{position} · {} · {}", item.title, band(landed)),
        |asked| {
            format!(
                "{position} · {} · asked for {}, landed {}",
                item.title,
                band(asked),
                band(landed)
            )
        },
    ))
}

/// One level, as a word. Five bands over the collection's own range: enough
/// to say what happened, too few to pretend the analysis measures moods.
fn band(level: f32) -> &'static str {
    match level {
        level if level <= -1.4 => "the calm end",
        level if level <= -0.5 => "quiet",
        level if level < 0.5 => "the middle",
        level if level < 1.4 => "lively",
        _ => "the loud end",
    }
}

/// Where the composed list landed, as `(position, level)` for the contour to
/// draw over its own line — the answer in the request's units.
fn result_dots(state: &crate::vibe::State, lane: usize) -> Vec<(f32, f32)> {
    let Some(levels) = state
        .preview
        .as_ref()
        .and_then(|preview| preview.levels.get(lane))
    else {
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

/// **One row of a draft list, drawn the same way in both routes.**
///
/// Manual's rows were bare `Up | Down | Remove` word buttons with no artwork
/// while Vibe's preview used the shared track row — two anatomies for one act,
/// in one place, three lines apart in the same file. Both hold
/// [`QueueItemVm`]s, so both draw this.
fn draft_row<'a>(
    shelf: &'a Shelf,
    item: &'a QueueItemVm,
    position: usize,
    len: usize,
    width: f32,
    shift: &dyn Fn(usize, i32) -> Message,
    remove: &dyn Fn(usize) -> Message,
) -> Element<'a, Message> {
    let room = theme::active();
    let marker: Element<'a, Message> = text(format!("{:02}", position + 1))
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.paper_faint)
        .into();
    let under = item
        .artist
        .as_deref()
        .or(item.album_artist.as_deref())
        .map(|artist| (artist.into(), room.paper_dim, None));
    let context = item
        .album
        .as_deref()
        .map(|album| (album.into(), None, width >= theme::PLAYLIST_BREAKPOINT));
    let track = views::page::track_row(views::page::TrackRow {
        marker,
        artwork: None,
        title: item.title.as_str().into(),
        ink: room.paper,
        under,
        context,
        duration: item
            .duration
            .map(crate::vm::format_duration)
            .unwrap_or_default()
            .into(),
        playing: false,
        selected: false,
        press: None,
    });
    row![
        track,
        views::page::favourite_slot(&item.path, is_favourite(shelf, &item.path)),
        views::page::icon_slot(
            crate::icon::Glyph::ArrowUp,
            "Move up",
            position > 0,
            true,
            shift(position, -1),
        ),
        views::page::icon_slot(
            crate::icon::Glyph::ArrowDown,
            "Move down",
            position + 1 < len,
            true,
            shift(position, 1),
        ),
        views::page::icon_slot(
            crate::icon::Glyph::Close,
            "Remove",
            true,
            true,
            remove(position),
        ),
    ]
    .spacing(theme::GAP_XS)
    .align_y(iced::Alignment::Center)
    .into()
}

fn is_favourite(shelf: &Shelf, path: &Path) -> bool {
    crate::app::is_favourite(shelf, path)
}

fn length_picker(current: crate::vibe::MixLength) -> Element<'static, Message> {
    let room = theme::active();
    pick_list(
        crate::vibe::MixLength::ALL,
        Some(current),
        Message::VibeLength,
    )
    .width(Length::Fixed(170.0))
    .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
    .text_size(theme::SIZE_BODY)
    .text_line_height(theme::LEADING_BODY)
    .style(move |_theme, status| theme::output_picker(room, status))
    .menu_style(move |_theme| theme::output_menu(room))
    .into()
}

fn choice<'a>(title: &'a str, detail: &'a str, message: Message) -> Element<'a, Message> {
    let room = theme::active();
    button(
        column![
            text(title)
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .font(theme::MEDIUM),
            text(detail)
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .color(room.paper_dim),
        ]
        .spacing(theme::GAP_XS),
    )
    .on_press(message)
    .padding(theme::HANG)
    .width(Length::Fill)
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .into()
}

/// A block's own small heading, in the caption voice both routes use.
fn caption(word: &str) -> Element<'static, Message> {
    let room = theme::active();
    text(word.to_owned())
        .size(theme::SIZE_CAPTION)
        .line_height(theme::LEADING_CAPTION)
        .font(theme::MEDIUM)
        .color(room.paper_faint)
        .into()
}

/// A caption over the field it names.
fn named<'a>(word: &str, field: Element<'a, Message>) -> Element<'a, Message> {
    column![caption(word), field].spacing(theme::GAP_XS).into()
}

/// One word naming an axis of the contour, in the quietest voice on the
/// control: the picture is the subject and these are its edges.
fn axis_word(word: &'static str) -> Element<'static, Message> {
    let room = theme::active();
    text(word)
        .size(theme::SIZE_CAPTION)
        .line_height(theme::LEADING_CAPTION)
        .color(room.paper_muted)
        .wrapping(text::Wrapping::None)
        .into()
}

/// One quiet line: a statement about the flow rather than a control.
fn hint(line: &str) -> Element<'static, Message> {
    let room = theme::active();
    text(line.to_owned())
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.paper_dim)
        .width(Length::Fill)
        .wrapping(text::Wrapping::Word)
        .into()
}

fn name_input(value: &str) -> Element<'_, Message> {
    let room = theme::active();
    text_input("Playlist name", value)
        .on_input(Message::PlaylistCreationName)
        .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .style(move |_theme, status| theme::input(room, status))
        .into()
}

fn back_button<'a>() -> Element<'a, Message> {
    action_button("Back to choices", Some(Message::PlaylistCreationBack)).into()
}

fn action_button(label: &str, message: Option<Message>) -> iced::widget::Button<'_, Message> {
    let room = theme::active();
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .padding(theme::pad(0.0, theme::GAP_SM))
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press_maybe(message)
}

fn word_button<'a>(label: &str, message: Message) -> iced::widget::Button<'a, Message> {
    word_button_maybe(label, Some(message))
}

fn word_button_maybe<'a>(
    label: &str,
    message: Option<Message>,
) -> iced::widget::Button<'a, Message> {
    let room = theme::active();
    button(
        container(
            text(label.to_owned())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .padding(theme::pad(0.0, theme::GAP_SM))
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press_maybe(message)
}

fn error(message: String) -> Element<'static, Message> {
    let room = theme::active();
    text(message)
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.alert)
        .into()
}
