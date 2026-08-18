//! **The shortcuts card** — what you can press, on screen, while baz is
//! running.
//!
//! The backlog, for a long time: *"the bindings are in the README and nowhere
//! the user can see them while running — no `?` overlay, no menu."* baz has a
//! keyboard grammar it teaches nowhere, which makes it a grammar for people
//! who have read the repository.
//!
//! # It cannot lie
//!
//! Every row comes from [`crate::keys::SHORTCUTS`], and every row of that
//! table is walked back through `keys::binding_for` by a test. A discovery
//! surface that named a key which did nothing would be worse than no surface
//! at all — it is the one kind of documentation a listener will trust
//! immediately and blame themselves for.
//!
//! The modifier is named for the platform ([`crate::keys::COMMAND_LABEL`]),
//! because `Ctrl` on a Mac is a key that exists and is not the one meant.
//!
//! # It is a card, not a place
//!
//! No navigation, no state, nothing to configure. It opens over whatever you
//! were doing, `Esc` peels it like every other layer, and pressing outside it
//! puts it away — the context menu's own manners, for the same reason: a
//! listener who opened it by accident must be able to dismiss it without
//! learning anything.

use iced::widget::{Space, column, container, mouse_area, row, scrollable, text};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::{keys, theme};

/// The card's own measure — wide enough for the longest description at the
/// metadata size without the key column having to wrap, and narrow enough to
/// read as an overlay rather than a page.
const CARD_W: f32 = 460.0;

/// The key column, fixed so every description starts on one line.
const KEY_W: f32 = 128.0;

/// Draw the card over the place, centred.
pub(crate) fn layer(window: iced::Size) -> Element<'static, Message> {
    let room = theme::active();
    let mut body = column![].spacing(theme::GAP_LG);
    for (heading, rows) in keys::SHORTCUTS {
        let mut group = column![
            text((*heading).to_owned())
                .size(theme::SIZE_HEADING)
                .line_height(theme::LEADING_HEADING)
                .font(theme::MEDIUM)
                .color(room.paper_faint),
        ]
        .spacing(theme::GAP_XS);
        for (key, does) in *rows {
            group = group.push(
                row![
                    container(
                        text(keys::shortcut_key(key))
                            .size(theme::SIZE_META)
                            .line_height(theme::LEADING_META)
                            .font(theme::MEDIUM)
                            .color(room.paper),
                    )
                    .width(Length::Fixed(KEY_W)),
                    text((*does).to_owned())
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper_dim),
                ]
                .spacing(theme::GAP_SM),
            );
        }
        body = body.push(group);
    }

    let card = container(
        scrollable(container(body).padding(theme::GAP_LG))
            .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
            .style(move |_theme, status| theme::scrollbar(room, room.plinth, status)),
    )
    .width(Length::Fixed(CARD_W))
    // Bounded rather than fixed: the card is as tall as it needs to be, and
    // scrolls only on a window too short to hold it whole.
    .max_height((window.height - 4.0 * theme::HANG).max(theme::WINDOW_FLOOR_H * 0.5))
    .style(move |_theme| theme::menu(room));

    // **Pressing outside puts it away**, which is the manner every floating
    // layer in the product has. The catcher is the whole window and the card
    // sits over it, so a press that reaches the catcher is by construction a
    // press that missed the card.
    iced::widget::stack![
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
        )
        .on_press(Message::ToggleShortcuts),
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center),
    ]
    .into()
}
