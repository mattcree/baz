//! The canonical, resumable Manual/Vibe playlist-creation place.

use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::PlayerState;
use crate::playlists::{CreationMode, EnergyShape, Playlists};
use crate::{theme, views};

#[expect(
    clippy::too_many_lines,
    reason = "the three shallow creation states are one visible flow and share the draft they render"
)]
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
                "Describe a journey through your music. Baz composes locally.",
                Message::PlaylistCreationMode(CreationMode::Vibe),
            ),
        ]
        .spacing(theme::GAP_MD)
        .into(),
        Some(CreationMode::Manual) => {
            let refusal = playlists.creation_refusal();
            let mut form = column![
                back_button(),
                text("MANUAL")
                    .size(theme::SIZE_CAPTION)
                    .line_height(theme::LEADING_CAPTION)
                    .font(theme::MEDIUM)
                    .color(room.paper_faint),
                name_input(&draft.name),
                text("Use the app-bar search and choose Add to playlist. Nothing is written until Save.")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_dim),
            ]
            .spacing(theme::GAP_SM);
            for (index, item) in draft.items.iter().enumerate() {
                form = form.push(
                    row![
                        column![
                            text(item.title.as_str())
                                .size(theme::SIZE_BODY)
                                .line_height(theme::LEADING_BODY)
                                .color(room.paper),
                            text(
                                item.artist
                                    .as_deref()
                                    .or(item.album_artist.as_deref())
                                    .unwrap_or("Unknown artist"),
                            )
                            .size(theme::SIZE_META)
                            .line_height(theme::LEADING_META)
                            .color(room.paper_dim),
                        ],
                        iced::widget::Space::new().width(Length::Fill),
                        views::page::favourite_slot(
                            &item.path,
                            crate::app::is_favourite(shelf, &item.path),
                        ),
                        action_button(
                            "Up",
                            (index > 0).then_some(Message::PlaylistCreationShift(index, -1)),
                        ),
                        action_button(
                            "Down",
                            (index + 1 < draft.items.len())
                                .then_some(Message::PlaylistCreationShift(index, 1)),
                        ),
                        action_button("Remove", Some(Message::PlaylistCreationRemove(index)),),
                    ]
                    .spacing(theme::GAP_SM)
                    .align_y(iced::Alignment::Center),
                );
            }
            if let Some(reason) = refusal {
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
        Some(CreationMode::Vibe) => {
            let mut form = column![back_button()].spacing(theme::GAP_SM);
            form = form.push(crate::views::home::vibe_creator(
                shelf,
                player,
                playlists.available(),
                width,
                playlists.creation_can_save(
                    shelf
                        .vibe
                        .preview
                        .as_ref()
                        .is_some_and(|preview| !preview.items.is_empty()),
                ),
            ));
            if draft.shape_open {
                let mut shapes = row![].spacing(theme::GAP_SM);
                for shape in EnergyShape::ALL {
                    shapes = shapes.push(action_button(
                        shape.label(),
                        Some(Message::PlaylistCreationEnergy(shape)),
                    ));
                }
                let waypoints = column![
                    text("SEMANTIC WAYPOINTS")
                        .size(theme::SIZE_CAPTION)
                        .line_height(theme::LEADING_CAPTION)
                        .font(theme::MEDIUM)
                        .color(room.paper_faint),
                    waypoint(0, "Start", &draft.waypoints[0]),
                    waypoint(1, "Middle (optional)", &draft.waypoints[1]),
                    waypoint(2, "Finish (optional)", &draft.waypoints[2]),
                ]
                .spacing(theme::GAP_SM);
                form = form.push(shapes).push(waypoints);
            } else {
                form = form.push(action_button(
                    "Shape the journey",
                    Some(Message::PlaylistCreationToggleShape),
                ));
            }
            if shelf.vibe.preview.is_some() {
                form = form.push(
                    column![
                        text("PLAYLIST NAME")
                            .size(theme::SIZE_CAPTION)
                            .line_height(theme::LEADING_CAPTION)
                            .font(theme::MEDIUM)
                            .color(room.paper_faint),
                        name_input(&draft.name),
                    ]
                    .spacing(theme::GAP_XS),
                );
                if let Some(reason) = playlists.creation_refusal() {
                    form = form.push(error(reason));
                }
            }
            form.into()
        }
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

fn waypoint<'a>(index: usize, label: &'a str, value: &'a str) -> Element<'a, Message> {
    let room = theme::active();
    text_input(label, value)
        .on_input(move |text| Message::PlaylistCreationWaypoint(index, text))
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

fn error(message: String) -> Element<'static, Message> {
    let room = theme::active();
    text(message)
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.alert)
        .into()
}
