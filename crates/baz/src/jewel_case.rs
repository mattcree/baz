//! The interactive jewel case used by Now Playing.
//!
//! This is deliberately a canvas inside Iced's existing renderer, not a
//! second 3D stack. A cosine foreshortens the front or rear insert through a
//! full turn; the spine, clear plastic edges, highlight and shadow preserve
//! the case at the edge-on points where an image alone would disappear.

use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use iced::mouse;
use iced::widget::canvas::{self, Canvas};
use iced::{Color, Element, Font, Length, Pixels, Point, Rectangle, Size, Vector, alignment};

use crate::app::Message;
use crate::{field, theme, vm};

/// Twenty frames per second: enough for this deliberately slow object, while
/// costing a third of a conventional 60 Hz decorative loop.
pub(crate) const TICK: Duration = Duration::from_millis(50);

/// One complete unattended turn.
const TURN: Duration = Duration::from_secs(32);
const DRAG_YAW_PER_PX: f32 = 0.012;
const DRAG_PITCH_PER_PX: f32 = 0.006;
const PITCH_LIMIT: f32 = 0.32;

/// The one animated reading the application owns.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Rotation {
    yaw: f32,
    pitch: f32,
    held: Option<Point>,
    last_tick: Instant,
}

impl Rotation {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            // Begin with enough angle to announce that this is an object, not
            // a flat replacement for the sleeve that stood here before it.
            yaw: 0.18,
            pitch: -0.04,
            held: None,
            last_tick: now,
        }
    }

    pub(crate) fn tick(&mut self, now: Instant) {
        let elapsed = now
            .saturating_duration_since(self.last_tick)
            .min(Duration::from_millis(100));
        self.last_tick = now;
        if self.held.is_some() {
            return;
        }
        self.yaw = wrap(self.yaw + TAU * elapsed.as_secs_f32() / TURN.as_secs_f32());
        // A released vertical tilt settles slowly back to the case's quiet
        // resting cant while the horizontal turn continues.
        let settle = 1.0 - (-2.4 * elapsed.as_secs_f32()).exp();
        self.pitch += (-0.04 - self.pitch) * settle;
    }

    pub(crate) fn press(&mut self, at: Point) {
        self.held = Some(at);
    }

    pub(crate) fn drag(&mut self, at: Point) {
        let Some(was) = self.held.replace(at) else {
            return;
        };
        self.yaw = wrap(self.yaw + (at.x - was.x) * DRAG_YAW_PER_PX);
        self.pitch =
            (self.pitch - (at.y - was.y) * DRAG_PITCH_PER_PX).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    pub(crate) fn release(&mut self) {
        self.held = None;
    }

    pub(crate) fn dragging(self) -> bool {
        self.held.is_some()
    }
}

fn wrap(angle: f32) -> f32 {
    angle.rem_euclid(TAU)
}

/// Owned copy for the generated rear insert.
#[derive(Debug, Clone)]
pub(crate) struct Insert {
    pub(crate) album_id: u64,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) tracks: Vec<String>,
}

/// The canvas program. Every field is render-ready and owned so the element
/// does not borrow the library while Iced retains its widget tree.
#[derive(Debug, Clone)]
struct Case {
    rotation: Rotation,
    art: Art,
    insert: Insert,
}

/// Render-ready pictures for the case.
#[derive(Debug, Clone)]
pub(crate) struct Art {
    pub(crate) front: Option<iced::widget::image::Handle>,
    pub(crate) from: Option<iced::widget::image::Handle>,
    pub(crate) front_opacity: f32,
    pub(crate) back: Option<iced::widget::image::Handle>,
    pub(crate) field: Option<field::Field>,
}

/// Build the case at the same square measure the old sleeve occupied.
pub(crate) fn view(
    edge: f32,
    rotation: Rotation,
    art: Art,
    insert: Insert,
) -> Element<'static, Message> {
    Canvas::new(Case {
        rotation,
        art,
        insert,
    })
    .width(Length::Fixed(edge))
    .height(Length::Fixed(edge))
    .into()
}

impl canvas::Program<Message> for Case {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        use canvas::event::Status;
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if cursor.is_over(bounds) =>
            {
                let Some(at) = cursor.position() else {
                    return (Status::Ignored, None);
                };
                (Status::Captured, Some(Message::CasePressed(at)))
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position })
                if self.rotation.dragging() =>
            {
                (Status::Captured, Some(Message::CaseDragged(position)))
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if self.rotation.dragging() =>
            {
                (Status::Captured, Some(Message::CaseReleased))
            }
            canvas::Event::Touch(iced::touch::Event::FingerPressed { position, .. })
                if bounds.contains(position) =>
            {
                (Status::Captured, Some(Message::CasePressed(position)))
            }
            canvas::Event::Touch(iced::touch::Event::FingerMoved { position, .. })
                if self.rotation.dragging() =>
            {
                (Status::Captured, Some(Message::CaseDragged(position)))
            }
            canvas::Event::Touch(
                iced::touch::Event::FingerLifted { .. } | iced::touch::Event::FingerLost { .. },
            ) if self.rotation.dragging() => (Status::Captured, Some(Message::CaseReleased)),
            _ => (Status::Ignored, None),
        }
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.rotation.dragging() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let room = theme::active();
        let edge = bounds.width.min(bounds.height) * 0.91;
        let centre = frame.center();
        let cosine = self.rotation.yaw.cos();
        let sine = self.rotation.yaw.sin();
        let face_width = cosine.abs().max(0.035);
        let pitch_scale = self.rotation.pitch.cos().max(0.9);

        // A low, soft stand-in shadow. It moves with depth but not with the
        // face transform, which keeps the object grounded at edge-on angles.
        let shadow_w = edge * (0.35 + 0.45 * cosine.abs());
        let shadow = canvas::Path::rounded_rectangle(
            Point::new(centre.x - shadow_w / 2.0, centre.y + edge * 0.46),
            Size::new(shadow_w, edge * 0.035),
            (edge * 0.018).into(),
        );
        frame.fill(&shadow, alpha(room.shadow, 0.34));

        // The face shifts by half the apparent case thickness as it turns.
        let thickness = edge * 0.055;
        let face_shift = sine * thickness * 0.42;
        frame.with_save(|frame| {
            frame.translate(Vector::new(centre.x + face_shift, centre.y));
            frame.rotate(self.rotation.pitch * 0.075);
            frame.scale_nonuniform(Vector::new(face_width, pitch_scale));
            frame.translate(Vector::new(-edge / 2.0, -edge / 2.0));

            draw_face(frame, self, edge, cosine >= 0.0, room);
        });

        // The clear spine remains physically present where the face tends to
        // zero. At broad angles it becomes only a highlight at the turning
        // edge; edge-on it carries the whole silhouette.
        let spine_w = thickness * (0.7 + 0.3 * sine.abs());
        let spine_x = centre.x - spine_w / 2.0 - sine.signum() * edge * face_width * 0.48;
        let case_spine = canvas::Path::rounded_rectangle(
            Point::new(spine_x, centre.y - edge * pitch_scale / 2.0),
            Size::new(spine_w, edge * pitch_scale),
            (edge * 0.009).into(),
        );
        frame.fill(
            &case_spine,
            alpha(room.paper, 0.12 + 0.22 * (1.0 - face_width)),
        );
        frame.stroke(
            &case_spine,
            canvas::Stroke::default()
                .with_color(alpha(room.paper, 0.38))
                .with_width(1.0),
        );

        vec![frame.into_geometry()]
    }
}

fn draw_face(
    frame: &mut canvas::Frame,
    case: &Case,
    edge: f32,
    front: bool,
    room: &'static theme::Palette,
) {
    let radius = edge * 0.018;
    let shell =
        canvas::Path::rounded_rectangle(Point::ORIGIN, Size::new(edge, edge), radius.into());
    frame.fill(&shell, alpha(room.paper, 0.10));
    frame.stroke(
        &shell,
        canvas::Stroke::default()
            .with_color(alpha(room.paper, 0.48))
            .with_width((edge * 0.004).max(1.0)),
    );

    let lip = edge * 0.032;
    let insert = Rectangle::new(
        Point::new(lip, lip),
        Size::new(edge - 2.0 * lip, edge - 2.0 * lip),
    );
    if front {
        if let Some(handle) = &case.art.front {
            if let Some(from) = &case.art.from
                && case.art.front_opacity < 1.0
            {
                frame.draw_image(insert, canvas::Image::new(from.clone()));
            }
            frame.draw_image(
                insert,
                canvas::Image::new(handle.clone()).opacity(case.art.front_opacity),
            );
        } else {
            generated_ground(frame, insert, case.insert.album_id, room);
        }
    } else if let Some(handle) = &case.art.back {
        frame.draw_image(insert, canvas::Image::new(handle.clone()));
    } else {
        generated_back(frame, insert, case, room);
    }

    // The lid's quiet reflection and the hinge rails are what make a cover
    // read as being *inside* clear plastic rather than merely transformed.
    let reflection = canvas::Path::new(|path| {
        path.move_to(Point::new(lip, lip));
        path.line_to(Point::new(edge * 0.38, lip));
        path.line_to(Point::new(edge * 0.18, edge - lip));
        path.line_to(Point::new(lip, edge - lip));
        path.close();
    });
    frame.fill(&reflection, alpha(Color::WHITE, 0.055));
    for y in [edge * 0.16, edge * 0.84] {
        let hinge = canvas::Path::rounded_rectangle(
            Point::new(edge * 0.006, y - edge * 0.035),
            Size::new(edge * 0.022, edge * 0.07),
            (edge * 0.006).into(),
        );
        frame.fill(&hinge, alpha(room.paper, 0.26));
    }
}

fn generated_ground(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    album_id: u64,
    room: &'static theme::Palette,
) {
    let (a, b) = vm::gradient_colors(album_id);
    let color = |rgb: [u8; 3]| room.placeholder_ink(Color::from_rgb8(rgb[0], rgb[1], rgb[2]));
    let gradient = canvas::gradient::Linear::new(
        bounds.position(),
        Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
    )
    .add_stop(0.0, color(a))
    .add_stop(1.0, color(b));
    frame.fill(
        &canvas::Path::rectangle(bounds.position(), bounds.size()),
        gradient,
    );
}

fn generated_back(
    frame: &mut canvas::Frame,
    bounds: Rectangle,
    case: &Case,
    room: &'static theme::Palette,
) {
    generated_ground(frame, bounds, case.insert.album_id, room);
    if let Some(field) = case.art.field {
        let colors = field.colors(room);
        frame.fill(
            &canvas::Path::rectangle(bounds.position(), bounds.size()),
            alpha(colors[1], 0.26),
        );
    }

    let pad = bounds.width * 0.075;
    let left = bounds.x + pad;
    let mut y = bounds.y + pad;
    let title_size = (bounds.width * 0.047).clamp(13.0, 30.0);
    let track_size = (bounds.width * 0.026).clamp(8.0, 16.0);
    draw_text(
        frame,
        &case.insert.artist.to_uppercase(),
        left,
        y,
        track_size,
        theme::MEDIUM,
        room.paper_faint,
    );
    y += track_size * 1.65;
    draw_text(
        frame,
        &case.insert.title,
        left,
        y,
        title_size,
        theme::SEMIBOLD,
        room.paper,
    );
    y += title_size * 1.75;

    let line_h = track_size * 1.5;
    let available = (bounds.y + bounds.height - pad - y).max(0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "positive insert geometry is capped to the small track vector"
    )]
    let rows = (available / line_h).floor() as usize;
    let shown = rows.min(case.insert.tracks.len());
    for (index, track) in case.insert.tracks.iter().take(shown).enumerate() {
        let line = format!("{:02}  {}", index + 1, clipped(track, 42));
        draw_text(
            frame,
            &line,
            left,
            y,
            track_size,
            theme::SANS,
            room.paper_dim,
        );
        y += line_h;
    }
    if shown < case.insert.tracks.len() && rows > 0 {
        let more = case.insert.tracks.len() - shown;
        draw_text(
            frame,
            &format!("+{more} more"),
            left,
            bounds.y + bounds.height - pad - track_size,
            track_size,
            theme::MEDIUM,
            room.paper_faint,
        );
    }
}

fn draw_text(
    frame: &mut canvas::Frame,
    content: &str,
    x: f32,
    y: f32,
    size: f32,
    font: Font,
    color: Color,
) {
    frame.fill_text(canvas::Text {
        content: content.to_owned(),
        position: Point::new(x, y),
        color,
        size: Pixels(size),
        line_height: iced::widget::text::LineHeight::Relative(1.0),
        font,
        horizontal_alignment: alignment::Horizontal::Left,
        vertical_alignment: alignment::Vertical::Top,
        shaping: iced::widget::text::Shaping::Basic,
    });
}

fn clipped(value: &str, max: usize) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn alpha(mut color: Color, opacity: f32) -> Color {
    color.a = opacity.clamp(0.0, 1.0);
    color
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_rotation_advances_and_a_held_case_does_not() {
        let start = Instant::now();
        let mut rotation = Rotation::new(start);
        let before = rotation.yaw;
        rotation.tick(start + TICK);
        assert!(rotation.yaw > before);

        rotation.press(Point::new(10.0, 10.0));
        let held = rotation.yaw;
        rotation.tick(start + 2 * TICK);
        assert!((rotation.yaw - held).abs() < f32::EPSILON);
    }

    #[test]
    fn dragging_turns_and_tilts_with_bounded_pitch() {
        let mut rotation = Rotation::new(Instant::now());
        rotation.press(Point::ORIGIN);
        let yaw = rotation.yaw;
        rotation.drag(Point::new(100.0, 10_000.0));
        assert!((rotation.yaw - yaw).abs() > f32::EPSILON);
        assert!((rotation.pitch + PITCH_LIMIT).abs() < f32::EPSILON);
        rotation.release();
        assert!(!rotation.dragging());
    }

    #[test]
    fn long_rear_labels_are_clipped_on_character_boundaries() {
        assert_eq!(clipped("Sigur Rós", 7), "Sigur R…");
        assert_eq!(clipped("Short", 42), "Short");
    }

    #[test]
    fn angles_are_kept_in_one_turn() {
        let pi = std::f32::consts::PI;
        assert!((wrap(-pi / 2.0) - 3.0 * pi / 2.0).abs() < f32::EPSILON);
        assert!((wrap(9.0 * pi) - pi).abs() < 0.000_01);
    }
}
