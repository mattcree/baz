//! **The context menu's float** — the card of mirrored verbs at the pointer
//! (doc 09 §5.2), and the backdrop that makes a press outside mean *put it
//! down*.
//!
//! Composition only: which items are on the card was decided when the menu
//! opened ([`crate::menu::items`], captured on `App`), and what a press
//! sends is the item's own recorded messages. This module draws the card at
//! the anchored point and nothing else — the geometry ([`crate::menu::extent`],
//! [`crate::menu::anchor`]) is pure and tested where it lives.
//!
//! # The mechanics are the panel's
//!
//! ADR-0016's verified float mechanics, at float scale: stacked over the
//! window (in `app.rs`), the card wrapped in `opaque` so a press inside it —
//! any button — cannot fall through to a tile underneath, **no scrim**
//! (refused; the wall stays fully legible), and the separation strategy is
//! the panel's exact pair, a surface step plus a 1 px hairline
//! ([`theme::menu`]) — no shadow, which the product's standing rules reserves for the
//! playing halo. The items are ordinary [`theme::track_row`] word buttons at
//! [`theme::TRANSPORT_HIT`] (law L7: a control is one height everywhere), so
//! the card introduces no colour and no control anatomy the room does not
//! already have.

use iced::widget::{Space, button, column, container, mouse_area, stack, text};
use iced::{Element, Length, Size, alignment};

use crate::app::Message;
use crate::menu::Menu;
use crate::theme;

/// The whole overlay layer: the close-on-press backdrop, and the card at
/// its anchored point — at the pointer, flipped inside the window at the
/// edges (§5.2).
pub(crate) fn layer(menu: &Menu, window: Size) -> Element<'static, Message> {
    let room = theme::active();
    let size = crate::menu::extent(menu.items.len());
    let at = crate::menu::anchor(menu.at, size, window);
    let mut listed = column![];
    for (index, item) in menu.items.iter().enumerate() {
        // The verb, and — where one exists — the gesture that accelerates
        // it, quietly at the row's right edge (doc 11 §5 P6.1: the era
        // printed `⌘Q` beside Quit; the mirror layer prints `⇧‑click`
        // beside the queueing verb). Readout ink, never the verb's: the
        // hint is a fact about the gesture, not a second control.
        let mut inks = iced::widget::row![
            text(item.label.clone())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center);
        if let Some(accelerator) = item.accelerator {
            inks = inks.push(Space::new().width(Length::Fill).height(Length::Fixed(0.0)));
            inks = inks.push(
                text(accelerator)
                    .size(theme::SIZE_CAPTION)
                    .line_height(theme::LEADING_CAPTION)
                    .color(room.paper_faint)
                    .wrapping(text::Wrapping::None),
            );
        }
        listed = listed.push(
            button(
                container(inks)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_y(alignment::Vertical::Center)
                    .clip(true),
            )
            .width(Length::Fill)
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .padding(theme::pad(0.0, theme::GAP_MD))
            .style(move |_theme, status| theme::track_row(room, room.plinth, status, false))
            .on_press(Message::MenuItemPressed(index)),
        );
    }
    let card = container(listed)
        .width(Length::Fixed(theme::MENU_W))
        .padding(theme::pad(theme::GAP_XS, 0.0))
        .style(move |_theme| theme::menu(room));
    stack![
        // The backdrop: a left press anywhere outside the card closes the
        // menu and reaches nothing underneath — the press is spent on the
        // closing. It carries no right handler on purpose: a right press
        // falls through to the rows below, whose own `menu::area` replaces
        // this menu (one at a time, by construction). Wheel travel is not
        // the backdrop's either; it passes through to the place.
        mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
            .on_press(Message::CloseMenu),
        // The card, opaque, at the anchored point. The padding places it;
        // `anchor` has already kept the whole card inside the window, so
        // the fill container never squeezes it.
        container(iced::widget::opaque(card))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top: at.y,
                left: at.x,
                right: 0.0,
                bottom: 0.0,
            }),
    ]
    .into()
}
