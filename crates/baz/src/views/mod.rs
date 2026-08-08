//! View composition — ADR-0006's layer 3, and the only disposable one.
//!
//! One module per surface of the interface:
//!
//! - [`setup`] — the first-run "Where's your music?" screen.
//! - [`top_bar`] — the search well, the group-key row, and the quiet counts.
//! - [`shelf`] — the wall: the shelved, virtualized album grid, its pinned
//!   group headers, the index rail, its tiles and its empty states.
//! - [`side_panel`] — the album inspector: header, edition selector, Play,
//!   track list.
//! - [`queue`] — the queue popover: what baz handed the engine, and where it
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
//! album is selected ([`crate::selection`]). [`queue`] is the **popover**,
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
pub(crate) mod queue;
pub(crate) mod settings;
pub(crate) mod setup;
pub(crate) mod shelf;
pub(crate) mod side_panel;
pub(crate) mod top_bar;

use iced::widget::{Space, button, container, image as iced_image, mouse_area, text, tooltip};
use iced::{Color, Element, Length, alignment};

use crate::app::Message;
use crate::motion::{Control, Ink};
use crate::{icon, theme, vm};

/// A `size`×`size` block filled with the album's deterministic two-color
/// gradient (hash → HSL, see [`vm::gradient_colors`]) — a stand-in sleeve,
/// square-cornered like the artwork it substitutes.
///
/// Shared rather than owned by one surface: the same placeholder stands in
/// for a missing sleeve on a tile and in the side panel, and a redesign that
/// changed one and not the other would be a bug.
///
/// # It is quieter than a real cover, on purpose
///
/// The stops are pulled back toward the sleeve's recess backing by
/// [`theme::Palette::placeholder_ink`], and that is the fix for something
/// plainly wrong in every wide screenshot: at full strength these gradients were
/// the *brightest* objects on a wall of mostly-dark real artwork, so the eye
/// went first to the records baz knows least about. An album with no cover
/// should be the quietest tile in its row.
///
/// The hues survive the mix, which is the whole reason the gradient exists:
/// two albums with no art must still look like two different albums.
///
/// # `shown`
///
/// How strongly the placeholder is drawn, 0…1 — the gradient's own answer to
/// the opacity a real thumbnail is composited at when its record is **outside a
/// running shuffle's pool** ([`theme::POOL_DIM`]). A gradient background is
/// painted rather than sampled, so there is nothing to set an opacity on; it is
/// mixed toward the wall instead, which is what compositing it at that opacity
/// against the wall would have produced. Ordinary tiles pass 1.0 and the mix is
/// the identity.
pub(crate) fn gradient_block(album_id: u64, size: f32, shown: f32) -> Element<'static, Message> {
    let room = theme::active();
    let (c1, c2) = vm::gradient_colors(album_id);
    let to_color = |c: [u8; 3]| {
        let ink = room.placeholder_ink(Color::from_rgb8(c[0], c[1], c[2]));
        theme::Palette::mix(room.wall, ink, shown.clamp(0.0, 1.0))
    };
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
/// and with the same chrome-free treatment, as the bottom bar's transport
/// buttons ([`theme::transport`] — at rest, the glyph and nothing else).
///
/// Shared by every surface that can be dismissed because a dismissal must look
/// and land the same wherever it is — a close control that moved or changed
/// size between the album inspector and the popover would be two controls, not
/// one. `label` names what is being closed ("Close queue"), which is the
/// tooltip and, iced 0.13 having no accessibility tree, the whole of the
/// control's accessible name.
///
/// `message` is the layer's own dismissal, because there is one rule *per
/// layer* rather than one rule: the inspector's ✕ closes the inspector, the
/// popover's closes the popover, and neither can reach the other.
///
/// The tooltip opens *below* the button rather than above it: these sit in a
/// surface's top row, where there is nothing above to open into.
///
/// `on` is the surface the ✕ stands on. An icon button's hover mark is an
/// **opaque** pre-composite now rather than an alpha the renderer blends
/// ([`theme::Palette::ink_over`]), so the one control that appears on two
/// different planes — the inspector's panel and the popover's float — has to be
/// told which one it is on.
/// `control` is which ✕ this is, so the shell can tell the two apart and fade
/// exactly the one under the pointer (ADR-0020 §2.1); `fade` is the layer's own
/// arrival, 1 for a layer that is simply there.
pub(crate) fn close_button(
    on: iced::Color,
    label: &'static str,
    message: Message,
    control: Control,
    ink: Ink,
    fade: f32,
) -> Element<'static, Message> {
    let room = theme::active();
    let mark = container(
        iced_image(icon::handle(icon::Glyph::Close))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(
                theme::glyph_ink(true, false, ink.hover(control), ink.pressed(control)) * fade,
            ),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let named = tooltip(
        button(mark)
            .width(Length::Fixed(theme::TRANSPORT_HIT))
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .padding(0)
            .style(move |_theme, status| {
                theme::fade_button(&theme::transport(room, on, status), fade)
            })
            .on_press(message),
        text(label)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room));
    mouse_area(named)
        .on_enter(Message::ControlEntered(control))
        .on_exit(Message::ControlLeft(control))
        .into()
}
