//! **The drag's ghost**: the lifted row's title following the pointer
//! (doc 09 §13 step 8; `crate::drag`'s module docs carry the gesture).
//!
//! The float mechanics are the context menu's — a full-window layer, the
//! card placed by padding at a point [`crate::menu::anchor`] has already
//! kept inside the window — minus everything a menu has that a ghost must
//! not: **no backdrop, no `opaque`, no control.** Text in a container
//! captures nothing, so every event falls through this layer to the rows
//! measuring the pointer and to the release that commits; the ghost is a
//! statement of what is in the hand, never a surface.
//!
//! One card anatomy ([`theme::menu`] — surface step plus hairline, no
//! shadow), one line in the room's dimmed ink ([`theme::Palette::paper_dim`]
//! — "reduced opacity" said with an ink the room already owns rather than
//! an alpha, for the palette's own ink-over reason), clipped at the menu's
//! width. No new colours, no new anatomy.

use iced::widget::{container, text};
use iced::{Element, Length, Point, Size};

use crate::app::Message;
use crate::drag::DragState;
use crate::theme;

/// The ghost card's height: one meta line with the card's own air.
const GHOST_H: f32 = theme::LINE_META + 2.0 * theme::GAP_XS;

/// The whole pass-through layer: the card, a hand's offset from the
/// pointer so the row under the pointer stays legible, held inside the
/// window by the menu's own `anchor`.
pub(crate) fn layer(drag: &DragState, window: Size) -> Element<'static, Message> {
    let room = theme::active();
    let size = Size::new(theme::MENU_W, GHOST_H);
    let at = crate::menu::anchor(
        Point::new(drag.at.x + theme::GAP_MD, drag.at.y + theme::GAP_MD),
        size,
        window,
    );
    container(
        container(
            text(drag.title.clone())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None),
        )
        .width(Length::Shrink)
        .max_width(size.width)
        .padding(theme::pad(theme::GAP_XS, theme::GAP_MD))
        .clip(true)
        .style(move |_theme| theme::menu(room)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(iced::Padding {
        top: at.y,
        left: at.x,
        right: 0.0,
        bottom: 0.0,
    })
    .into()
}
