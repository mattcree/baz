//! **The stereo image** — the one reading in the set that is not about the
//! music, but about the *record*.
//!
//! The fifth visualisation, and it exists because the other four cannot see
//! what it sees. Spectrum, the rolling waveform, the spectrogram and the
//! oscilloscope all read [`baz_core::engine::VisualizationFrame::samples`],
//! which is the **mono fold**: the two channels added together. A spectrum of
//! the left channel and a spectrum of the right are two pictures of the same
//! music, so folding costs those four nothing at all.
//!
//! It costs this one everything, so the engine now publishes
//! [`side`](baz_core::engine::VisualizationFrame::side) beside it — the
//! half-difference, from which `left = mid + side` and `right = mid - side`
//! exactly.
//!
//! # What it answers
//!
//! **How wide is this record, and do its two channels agree?** A goniometer,
//! the tool every mastering desk has had for sixty years, and its readings are
//! learned in one sitting because they are *shapes* rather than numbers:
//!
//! - a **vertical line** is mono — a single mixed-down channel, or a record
//!   that simply is not wide;
//! - a **fat vertical cloud** is an ordinary stereo mix: mostly centred, with
//!   width around it;
//! - a **horizontal** spread is the interesting one. The channels are
//!   *opposed*, and a record that looks like this loses its middle the moment
//!   anybody plays it on a phone speaker or in a room where the two speakers
//!   sum. It is the one fault in a mastering chain that is invisible until you
//!   look at exactly this.
//!
//! The axes are drawn because the shape means nothing without them, and they
//! are the two readings above: the upright is mono, the crossbar is
//! cancellation.
//!
//! # Why it is drawn in quads
//!
//! [`crate::response`]'s standing note: iced's `canvas` would give points and
//! strokes directly and costs a tessellation stack this project prices
//! deliberately. A cloud of points is the one shape quads make *well* — two
//! hundred and fifty-six small squares, which is a quarter of what the
//! oscilloscope draws.
//!
//! # Nothing here rests on colour
//!
//! The reading is the **shape**, and the shape is the same in greyscale. Ink
//! walks the record's three colours across the width exactly as every other
//! field does, and carries no fact of its own.

use iced::advanced::renderer::Renderer as _;
use iced::advanced::widget::{Widget, tree};
use iced::advanced::{Layout, layout, mouse, renderer};
use iced::{Color, Element, Length, Rectangle, Size, Theme};

use crate::theme;

/// How much of the half-height a full-scale mono signal uses.
///
/// Short of the edge for [`crate::scope`]'s reason: a master that touches the
/// top and bottom would read as *cut off* rather than as *loud*, and the cloud
/// would collide with the title drawn over this field.
const REACH: f32 = 0.80;

/// One point's side, in pixels.
///
/// Small: the picture is a *density*, and a cloud made of large dots is a
/// scatter of dots rather than a cloud. Two hundred and fifty-six of these at
/// this size is a shape you read at a glance and cannot read a number off,
/// which is the correct amount of precision for a background.
const DOT: f32 = 2.5;

/// How dark the axes are against the field's own ink.
const AXIS: f32 = 0.34;

/// **The live cloud of the delivered stereo signal.**
pub(crate) struct Stereo {
    /// `(side, mid)` per sampled point — across, then up.
    points: Vec<(f32, f32)>,
    /// The record's three inks, walked across the field.
    inks: [Color; 3],
    /// The axes' ink.
    datum: Color,
    width: f32,
    height: f32,
}

impl Stereo {
    /// Build the image over one delivered frame.
    pub(crate) fn new(
        mid: &[f32],
        side: &[f32],
        inks: [Color; 3],
        datum: Color,
        field: Size,
    ) -> Self {
        Self {
            points: mid
                .iter()
                .zip(side)
                .map(|(mid, side)| (*side, *mid))
                .collect(),
            inks,
            datum,
            width: field.width,
            height: field.height,
        }
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Stereo {
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.width), Length::Fixed(self.height))
    }

    fn layout(
        &mut self,
        _tree: &mut iced::advanced::widget::Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(
            limits,
            Length::Fixed(self.width),
            Length::Fixed(self.height),
        )
    }

    fn draw(
        &self,
        _tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if bounds.width < 2.0 || bounds.height < 2.0 {
            return;
        }
        let mut quad = |x: f32, y: f32, w: f32, h: f32, colour: Color| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x,
                        y,
                        width: w,
                        height: h,
                    },
                    ..renderer::Quad::default()
                },
                colour,
            );
        };
        let middle_x = bounds.x + bounds.width / 2.0;
        let middle_y = bounds.y + bounds.height / 2.0;
        // **A square field, whatever the window is.** The reading is an angle,
        // and an angle read off axes with different scales is a different
        // angle — a wide window would make every record look wide.
        let reach = bounds.width.min(bounds.height) / 2.0 * REACH;

        // The upright is mono and the crossbar is cancellation; both are
        // stated, because the cloud's shape means nothing without them.
        let axis = theme::alpha(self.datum, AXIS);
        quad(middle_x - 0.5, middle_y - reach, 1.0, reach * 2.0, axis);
        quad(middle_x - reach, middle_y - 0.5, reach * 2.0, 1.0, axis);

        for (side, mid) in &self.points {
            let x = middle_x + side.clamp(-1.0, 1.0) * reach;
            let y = middle_y - mid.clamp(-1.0, 1.0) * reach;
            // How far out this point is, which is how loud that instant was.
            let level = side.hypot(*mid).min(1.0);
            let ink =
                crate::visualizer::level_ink(level, (side * 0.5 + 0.5).clamp(0.0, 1.0), self.inks);
            quad(x - DOT / 2.0, y - DOT / 2.0, DOT, DOT, ink);
        }
    }
}

impl<'a, Message: 'a> From<Stereo> for Element<'a, Message, Theme, iced::Renderer> {
    fn from(stereo: Stereo) -> Self {
        Self::new(stereo)
    }
}

#[cfg(test)]
mod tests {
    use super::Stereo;

    fn cloud(left: &[f32], right: &[f32]) -> Vec<(f32, f32)> {
        let mid: Vec<f32> = left.iter().zip(right).map(|(l, r)| (l + r) * 0.5).collect();
        let side: Vec<f32> = left.iter().zip(right).map(|(l, r)| (l - r) * 0.5).collect();
        Stereo::new(
            &mid,
            &side,
            [iced::Color::WHITE; 3],
            iced::Color::WHITE,
            iced::Size::new(100.0, 100.0),
        )
        .points
    }

    /// **A mono record is a vertical line**, which is the reading a listener
    /// learns first and the one that must never be wrong: a file with one
    /// channel in it has no stereo image, and drawing width where there is
    /// none would be inventing a fact about somebody's record.
    #[test]
    fn a_mono_record_stands_upright() {
        let signal: Vec<f32> = (0..64_u16).map(|at| f32::from(at) / 64.0 - 0.5).collect();
        for (across, _) in cloud(&signal, &signal) {
            assert!(across.abs() < 1e-6, "a mono record was drawn {across} wide");
        }
    }

    /// **An out-of-phase record lies flat**, which is the fault this
    /// visualisation exists to make visible: two channels that cancel have no
    /// middle left when anything sums them.
    #[test]
    fn an_inverted_channel_lies_on_its_side() {
        let signal: Vec<f32> = (0..64_u16).map(|at| f32::from(at) / 64.0 - 0.5).collect();
        let inverted: Vec<f32> = signal.iter().map(|sample| -sample).collect();
        for (across, up) in cloud(&signal, &inverted) {
            assert!(up.abs() < 1e-6, "an inverted pair kept {up} of its middle");
            assert!(across.abs() > 0.0 || up.abs() < 1e-6);
        }
    }

    /// **An ordinary wide mix is neither**, so the two readings above are
    /// distinguishable rather than the only two shapes the field can make.
    #[test]
    fn a_wide_mix_opens_out() {
        let left: Vec<f32> = (0..64_u16).map(|at| f32::from(at) / 64.0 - 0.5).collect();
        let right: Vec<f32> = left.iter().map(|sample| sample * 0.4).collect();
        let points = cloud(&left, &right);
        assert!(
            points.iter().any(|(across, _)| across.abs() > 0.05),
            "a wide mix was drawn as mono"
        );
        assert!(
            points.iter().any(|(_, up)| up.abs() > 0.05),
            "a wide mix lost its middle"
        );
    }
}
