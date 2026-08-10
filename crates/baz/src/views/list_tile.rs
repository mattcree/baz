//! The wall-sized tile shared by implicit playlists.
//!
//! Home's library-wide `All songs` and an artist's `All songs` differ in
//! scope, not anatomy: both wear the playlist collage, two caption lanes, and
//! the wall's own hover veil. Keeping that composition here makes the visual
//! promise structural rather than two large functions agreeing by inspection.

use iced::widget::{button, column, container, mouse_area, stack, text};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::implicit::ImplicitList;
use crate::player::PlayerState;
use crate::shelf::Grid;
use crate::{icon, theme};

/// The gestures exposed by one implicit-list tile.
pub(crate) struct Actions {
    pub(crate) play: Message,
    pub(crate) open: Option<Message>,
    pub(crate) enter: Message,
    pub(crate) exit: Message,
}

/// Draw `list` as one tile in the wall's own grid anatomy.
///
/// The tile itself plays the list. The veil repeats that visible action and
/// may also carry an `Open` road when the list has a distinct place to open.
/// `enter` and `exit` feed the one hover bit held by [`Shelf`]; only one
/// implicit-list tile is ever on screen at once.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &PlayerState,
    hang: Grid,
    list: &ImplicitList,
    hovered: bool,
    actions: Actions,
) -> Option<Element<'a, Message>> {
    if list.is_empty() {
        return None;
    }
    let room = theme::active();
    let edge = hang.art;
    let work = (edge - 2.0 * theme::SLEEVE_MAT).max(0.0);
    let art = crate::views::playlist_sleeve(shelf, &list.art, list.name(), work);
    let art: Element<'_, Message> = if hovered {
        let mut options = Vec::new();
        if player.engine_ready() {
            options.push(crate::views::shelf::VeilOption::accented(
                icon::Glyph::Play,
                "Play",
                actions.play.clone(),
            ));
        }
        if let Some(open) = actions.open {
            options.push(crate::views::shelf::VeilOption::new(
                icon::Glyph::Open,
                "Open",
                open,
            ));
        }
        stack![art, crate::views::shelf::veil(work, options)].into()
    } else {
        art
    };
    let sleeve = container(
        container(art)
            .width(Length::Fixed(work))
            .height(Length::Fixed(work))
            .style(move |_theme| theme::sleeve(room, 0.0)),
    )
    .width(Length::Fixed(edge))
    .height(Length::Fixed(edge))
    .padding(theme::SLEEVE_MAT)
    .style(move |_theme| theme::sleeve_mat(room));
    let caption_lane = |content: Element<'a, Message>| {
        container(content)
            .width(Length::Fixed(edge))
            .height(Length::Fixed(theme::CAPTION_LINE_H))
            .align_y(alignment::Vertical::Top)
            .clip(true)
    };
    let hover = if hovered { 1.0 } else { 0.0 };
    let caption_block = column![
        caption_lane(
            text(list.name().to_owned())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .color(room.paper)
                .wrapping(text::Wrapping::None)
                .into(),
        ),
        caption_lane(
            text(list.counts())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(theme::caption_ink(room, hover))
                .wrapping(text::Wrapping::None)
                .into(),
        ),
    ]
    .width(Length::Fixed(edge))
    .height(Length::Fixed(theme::CAPTION_H));
    let tile = column![
        sleeve,
        caption_block,
        crate::views::shelf::state_rule(hover, false, edge)
    ]
    .spacing(theme::GAP_XS)
    .width(Length::Fixed(edge));
    let pressable = button(tile)
        .padding(0)
        .style(move |_theme, status| theme::tile(room, status, false))
        .on_press(actions.play);
    Some(
        mouse_area(pressable)
            .on_enter(actions.enter)
            .on_exit(actions.exit)
            .into(),
    )
}
