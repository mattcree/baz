//! Borderless-window edge and corner resizing.
//!
//! iced 0.14 exposes the compositor-owned resize action. This wrapper gives
//! Baz's undecorated window the ordinary eight resize targets without
//! reimplementing resize motion or stealing input outside the narrow frame.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Renderer as _, Shell, Widget, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Theme, Vector, mouse, window};

use crate::app::Message;

/// Logical pixels claimed inside each undecorated window edge.
pub(crate) const RESIZE_BAND: f32 = 6.0;

/// Add native compositor resize gestures to `content`.
pub(crate) fn resize_frame<'a>(
    content: impl Into<Element<'a, Message>>,
    enabled: bool,
) -> Element<'a, Message> {
    Element::new(Frame {
        content: content.into(),
        enabled,
    })
}

/// Make the place/lane body a hard paint and input viewport between Baz's two
/// resident bars.
///
/// This is intentionally outside every individual scrollable. iced 0.14's
/// scrollable only opens a renderer layer while its cached scrollbar state is
/// active; its inactive branch passes a viewport rectangle that image widgets
/// ignore. One composition boundary remains correct through stale layout and
/// widget-tree transitions and covers every current/future sleeve consumer.
pub(crate) fn body_clip<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    Element::new(Clip {
        content: content.into(),
    })
}

struct Clip<'a> {
    content: Element<'a, Message>,
}

fn clipped_bounds(bounds: Rectangle, viewport: &Rectangle) -> Option<Rectangle> {
    bounds.intersection(viewport)
}

impl Widget<Message, Theme, iced::Renderer> for Clip<'_> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let Some(clipped) = clipped_bounds(layout.bounds(), viewport) else {
            return;
        };
        let cursor = if cursor.is_over(clipped) || cursor.is_levitating() {
            cursor
        } else {
            mouse::Cursor::Unavailable
        };
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &clipped,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let Some(clipped) = clipped_bounds(layout.bounds(), viewport) else {
            return;
        };
        renderer.with_layer(clipped, |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                &clipped,
            );
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let Some(clipped) = clipped_bounds(layout.bounds(), viewport) else {
            return mouse::Interaction::default();
        };
        if !cursor.is_over(clipped) {
            return mouse::Interaction::default();
        }
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            &clipped,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, iced::Renderer>> {
        let clipped = clipped_bounds(layout.bounds(), viewport)?;
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            &clipped,
            translation,
        )
    }
}

struct Frame<'a> {
    content: Element<'a, Message>,
    enabled: bool,
}

fn direction_at(bounds: Rectangle, position: iced::Point) -> Option<window::Direction> {
    if !bounds.contains(position) {
        return None;
    }
    let north = position.y < bounds.y + RESIZE_BAND;
    let south = position.y >= bounds.y + bounds.height - RESIZE_BAND;
    let west = position.x < bounds.x + RESIZE_BAND;
    let east = position.x >= bounds.x + bounds.width - RESIZE_BAND;
    match (north, south, west, east) {
        (true, _, true, _) => Some(window::Direction::NorthWest),
        (true, _, _, true) => Some(window::Direction::NorthEast),
        (_, true, true, _) => Some(window::Direction::SouthWest),
        (_, true, _, true) => Some(window::Direction::SouthEast),
        (true, _, _, _) => Some(window::Direction::North),
        (_, true, _, _) => Some(window::Direction::South),
        (_, _, true, _) => Some(window::Direction::West),
        (_, _, _, true) => Some(window::Direction::East),
        _ => None,
    }
}

fn interaction(direction: window::Direction) -> mouse::Interaction {
    match direction {
        window::Direction::North | window::Direction::South => {
            mouse::Interaction::ResizingVertically
        }
        window::Direction::East | window::Direction::West => {
            mouse::Interaction::ResizingHorizontally
        }
        window::Direction::NorthEast | window::Direction::SouthWest => {
            mouse::Interaction::ResizingDiagonallyUp
        }
        window::Direction::NorthWest | window::Direction::SouthEast => {
            mouse::Interaction::ResizingDiagonallyDown
        }
    }
}

impl Widget<Message, Theme, iced::Renderer> for Frame<'_> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.enabled
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            )
            && let Some(position) = cursor.position()
            && let Some(direction) = direction_at(layout.bounds(), position)
        {
            shell.publish(Message::WindowResize(direction));
            shell.capture_event();
            return;
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if self.enabled
            && let Some(position) = cursor.position()
            && let Some(direction) = direction_at(layout.bounds(), position)
        {
            return interaction(direction);
        }
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(direction: window::Direction) -> u8 {
        match direction {
            window::Direction::North => 0,
            window::Direction::South => 1,
            window::Direction::East => 2,
            window::Direction::West => 3,
            window::Direction::NorthEast => 4,
            window::Direction::NorthWest => 5,
            window::Direction::SouthEast => 6,
            window::Direction::SouthWest => 7,
        }
    }

    #[test]
    fn all_eight_resize_targets_and_the_interior_are_distinct() {
        let bounds = Rectangle::new(iced::Point::ORIGIN, Size::new(200.0, 120.0));
        let cases = [
            ((0.0, 0.0), Some(window::Direction::NorthWest)),
            ((100.0, 0.0), Some(window::Direction::North)),
            ((199.0, 0.0), Some(window::Direction::NorthEast)),
            ((199.0, 60.0), Some(window::Direction::East)),
            ((199.0, 119.0), Some(window::Direction::SouthEast)),
            ((100.0, 119.0), Some(window::Direction::South)),
            ((0.0, 119.0), Some(window::Direction::SouthWest)),
            ((0.0, 60.0), Some(window::Direction::West)),
            ((100.0, 60.0), None),
        ];
        for ((x, y), expected) in cases {
            assert_eq!(
                direction_at(bounds, iced::Point::new(x, y)).map(code),
                expected.map(code)
            );
        }
    }

    #[test]
    fn resize_band_stays_inside_the_window() {
        let bounds = Rectangle::new(iced::Point::new(10.0, 20.0), Size::new(100.0, 80.0));
        assert_eq!(
            direction_at(bounds, iced::Point::new(9.0, 20.0)).map(code),
            None
        );
        assert_eq!(
            direction_at(bounds, iced::Point::new(16.0, 60.0)).map(code),
            None
        );
        assert_eq!(
            direction_at(bounds, iced::Point::new(15.9, 60.0)).map(code),
            Some(code(window::Direction::West))
        );
    }

    #[test]
    fn the_body_clip_is_the_intersection_not_the_whole_window() {
        let body = Rectangle::new(iced::Point::new(0.0, 41.0), Size::new(1280.0, 736.0));
        let window = Rectangle::new(iced::Point::ORIGIN, Size::new(1280.0, 860.0));
        assert_eq!(clipped_bounds(body, &window), Some(body));

        let damaged = Rectangle::new(iced::Point::new(0.0, 0.0), Size::new(1280.0, 100.0));
        assert_eq!(
            clipped_bounds(body, &damaged),
            Some(Rectangle::new(
                iced::Point::new(0.0, 41.0),
                Size::new(1280.0, 59.0)
            ))
        );
    }

    #[test]
    fn every_place_crosses_one_physical_body_scissor_before_the_bars() {
        let frame = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/window_frame.rs"),
        )
        .expect("frame source");
        assert!(
            frame.contains("renderer.with_layer(clipped"),
            "a viewport rectangle alone cannot clip iced images"
        );
        assert!(
            frame.contains("mouse::Cursor::Unavailable"),
            "off-body content can still claim pointer input through chrome"
        );

        let app = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("app source");
        let clip = app
            .find("let screen = crate::window_frame::body_clip(screen);")
            .expect("shared body clip");
        let app_bar = app[clip..]
            .find("views::app_bar::view(")
            .expect("resident app bar")
            + clip;
        let bottom = app[app_bar..]
            .find("views::bottom_bar::view(")
            .expect("resident bottom bar")
            + app_bar;
        assert!(clip < app_bar && app_bar < bottom);
    }
}
