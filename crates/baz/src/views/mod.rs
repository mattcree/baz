//! View composition — ADR-0006's layer 3, and the only disposable one.
//!
//! One module per surface of the interface:
//!
//! - [`setup`] — the first-run "Where's your music?" screen.
//! - [`top_bar`] — the search well and the quiet counts beside it.
//! - [`shelf`] — the virtualized album grid, its tiles, and its empty states.
//! - [`side_panel`] — the selected album: header, edition selector, Play,
//!   track list.
//! - [`bottom_bar`] — now-playing, transport, seek row.
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
pub(crate) mod setup;
pub(crate) mod shelf;
pub(crate) mod side_panel;
pub(crate) mod top_bar;

use iced::widget::{Space, container};
use iced::{Color, Element, Length};

use crate::app::Message;
use crate::vm;

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
