//! View composition — ADR-0006's layer 3, and the only disposable one.
//!
//! One module per surface of the interface:
//!
//! - [`setup`] — the first-run "Where's your music?" screen.
//! - [`top_bar`] — the search well and the quiet counts beside it.
//! - [`shelf`] — the virtualized album grid, its tiles, and its empty states.
//! - [`side_panel`] — the album inspector: header, edition selector, Play,
//!   track list.
//! - [`up_next`] — the queue popover: what baz handed the engine, and where it
//!   is in it.
//! - [`settings`] — the Settings place: the standing decisions, today
//!   ReplayGain.
//! - [`bottom_bar`] — now-playing, transport, seek row.
//!
//! They are the four kinds ADR-0016 names, and which kind a surface is decides
//! what it may cost. [`top_bar`] and [`shelf`] compose the Library **place**,
//! and [`settings`] is the other one — places fill the window and replace each
//! other. [`side_panel`] is the Library's **inspector**, the sole tenant of the
//! column beside the shelf at [`theme::PANEL_W`]; it is open exactly when an
//! album is selected ([`crate::selection`]). [`up_next`] is the **popover**,
//! anchored to the bar, [`theme::POPOVER_W`] wide, taking no width from the
//! shelf at all ([`crate::overlay`]). And [`bottom_bar`] is the **bar**, which
//! is in every place and never moves.
//!
//! Everything here is iced-specific and holds no state: each module exposes a
//! `view` function that reads [`crate::app`]'s state (and [`crate::player`]'s
//! render-ready readings) and returns an [`Element`]. Composition — which
//! surfaces are on screen and in what arrangement — stays in `app.rs` with
//! the state and the update loop; these modules only know how to draw one
//! surface each. A layout or visual redesign rewrites these files and nothing
//! else, which is the whole point of the split.
//!
//! Values, not layout, live in [`crate::theme`]: no view function here may
//! carry a hardcoded color, size, or padding (ADR-0006 calls that a
//! review-blocking defect). The few constants that *are* here are geometry a
//! single surface owns — a fixed field width, a panel's inset — and each sits
//! in the module that draws it.
//!
//! # `views::shelf` and `shelf`
//!
//! There are two shelves and they are different layers: [`crate::shelf`] is
//! the pure virtualization *math* (layer 1, unit-tested without a window),
//! [`views::shelf`](shelf) is the *composition* that spends it. The geometry
//! module keeps its place and its name; where a view file needs it, it is
//! imported as `geometry` so the two never read as the same thing.

pub(crate) mod bottom_bar;
pub(crate) mod settings;
pub(crate) mod setup;
pub(crate) mod shelf;
pub(crate) mod side_panel;
pub(crate) mod top_bar;
pub(crate) mod up_next;

use iced::widget::{Space, button, container, image as iced_image, text, tooltip};
use iced::{Color, Element, Length, alignment};

use crate::app::Message;
use crate::{icon, theme, vm};

/// A `size`×`size` block filled with the album's deterministic two-color
/// gradient (hash → HSL, see [`vm::gradient_colors`]) — a stand-in sleeve,
/// square-cornered like the artwork it substitutes.
///
/// Shared rather than owned by one surface: the same placeholder stands in
/// for a missing sleeve on a tile and in the side panel, and a redesign that
/// changed one and not the other would be a bug.
pub(crate) fn gradient_block(album_id: u64, size: f32) -> Element<'static, Message> {
    let (c1, c2) = vm::gradient_colors(album_id);
    let to_color = |c: [u8; 3]| Color::from_rgb8(c[0], c[1], c[2]);
    let gradient = iced::gradient::Linear::new(iced::Radians(2.4))
        .add_stop(0.0, to_color(c1))
        .add_stop(1.0, to_color(c2));
    container(Space::new(Length::Fixed(size), Length::Fixed(size)))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Gradient(gradient.into())),
            ..container::Style::default()
        })
        .into()
}

/// The ✕ that dismisses a layer: the close glyph in the same fixed square,
/// and with the same quiet card, as the bottom bar's transport buttons.
///
/// Shared by every surface that can be dismissed because a dismissal must look
/// and land the same wherever it is — a close control that moved or changed
/// size between the album inspector and the popover would be two controls, not
/// one. `label` names what is being closed ("Close up next"), which is the
/// tooltip and, iced 0.13 having no accessibility tree, the whole of the
/// control's accessible name.
///
/// `message` is the layer's own dismissal, because there is one rule *per
/// layer* rather than one rule: the inspector's ✕ closes the inspector, the
/// popover's closes the popover, and neither can reach the other.
///
/// The tooltip opens *below* the button rather than above it: these sit in a
/// surface's top row, where there is nothing above to open into.
pub(crate) fn close_button(label: &'static str, message: Message) -> Element<'static, Message> {
    let mark = container(
        iced_image(icon::handle(icon::Glyph::Close))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::GLYPH_OPACITY),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    tooltip(
        button(mark)
            .width(Length::Fixed(theme::TRANSPORT_HIT))
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .padding(0)
            .style(theme::transport)
            .on_press(message),
        text(label)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(theme::tooltip)
    .into()
}
