//! **Peak against average** — what is left of this master's dynamics.
//!
//! The sixth visualisation, and the last of the pair that ask about the
//! *record* rather than about the music. [`crate::stereo`] asks how wide it
//! is; this one asks how much of it is still moving.
//!
//! # The reading is the gap, not either line
//!
//! Two figures per frame: the **loudest instant** in it, and the **average**
//! of the whole of it. Neither is interesting alone — the rolling waveform
//! already draws the second, and the first on its own is a number every master
//! since 1995 pins to the same place. What is interesting is the **distance
//! between them**, which is the crest factor, which is the thing thirty years
//! of louder-is-better mastering has been spending.
//!
//! - A **thick band** is a record with its transients intact: the drum hits
//!   stand well above what is around them.
//! - A **thin ribbon riding the top** is a record squashed flat. Everything is
//!   as loud as everything else, and the loudest instant is barely louder than
//!   the average.
//!
//! So it is drawn as a **filled band between the two**, not as two lines: the
//! thing to look at is the space, and a picture that made you measure the gap
//! between two strokes would be asking a reader to do the arithmetic the
//! picture exists to do for them.
//!
//! # Nothing here rests on colour
//!
//! The reading is a **thickness**, which the owner's own standing rule
//! requires: a fact nobody can read in greyscale is not a reading. Ink walks
//! the record's three colours across time as every other field does, and
//! carries nothing of its own.
//!
//! # What it does not claim
//!
//! It is not a loudness measurement. `crate::replaygain` owns that, in EBU
//! R128, over whole tracks, and it is what baz plays at. This is a picture of
//! the last second of delivered audio and its vertical scale is the rolling
//! waveform's own dB map — good enough to see a shape change between records,
//! and not a number anybody should quote.

use iced::advanced::renderer::Renderer as _;
use iced::advanced::widget::{Widget, tree};
use iced::advanced::{Layout, layout, mouse, renderer};
use iced::{Color, Element, Length, Rectangle, Size, Theme};

use crate::theme;

/// How much of the height a full-scale peak uses.
const REACH: f32 = 0.86;

/// The floor on the band's thickness, so a perfectly flat frame is still a
/// line rather than nothing. A gap of zero is a real reading — it means the
/// peak *is* the average — and it has to look like one.
const FLOOR: f32 = 1.5;

/// How dark the datum is against the field's own ink.
const DATUM: f32 = 0.4;

/// **The band between the loudest instant and the average**, over time.
pub(crate) struct Headroom {
    /// `(average, peak)` per frame, oldest first, each 0…1 of the height.
    frames: Vec<(f32, f32)>,
    /// The record's three inks, walked across time.
    inks: [Color; 3],
    /// The datum's ink.
    datum: Color,
    width: f32,
    height: f32,
}

impl Headroom {
    /// Build the band over the frames the history holds.
    pub(crate) fn new(
        history: &crate::visualizer::History,
        inks: [Color; 3],
        datum: Color,
        size: Size,
    ) -> Self {
        Self {
            frames: (0..history.len())
                .map(|at| (history.average(at), history.peak(at)))
                .collect(),
            inks,
            datum,
            width: size.width,
            height: size.height,
        }
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Headroom {
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
        if bounds.width < 2.0 || bounds.height < 2.0 || self.frames.is_empty() {
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
        // **The floor of the scale, drawn**, because a band floating over
        // nothing gives a reader no idea how far it could fall.
        let floor = bounds.y + bounds.height;
        quad(
            bounds.x,
            floor - 1.0,
            bounds.width,
            1.0,
            theme::alpha(self.datum, DATUM),
        );

        #[expect(clippy::cast_precision_loss, reason = "a fixed history of 32 frames")]
        let pitch = bounds.width / self.frames.len() as f32;
        let reach = bounds.height * REACH;
        for (at, (average, peak)) in self.frames.iter().enumerate() {
            #[expect(clippy::cast_precision_loss, reason = "as above")]
            let left = bounds.x + at as f32 * pitch;
            let average = average.clamp(0.0, 1.0);
            // The peak can only be at or above the average — a loudest instant
            // quieter than the mean is arithmetically impossible, and clamping
            // says so rather than drawing an upside-down band if it ever is.
            let peak = peak.clamp(average, 1.0);
            let top = floor - peak * reach;
            let bottom = floor - average * reach;
            let ink = crate::visualizer::level_ink(
                peak,
                crate::visualizer::across(at, self.frames.len()),
                self.inks,
            );
            quad(left, top, pitch, (bottom - top).max(FLOOR), ink);
        }
    }
}

impl<'a, Message: 'a> From<Headroom> for Element<'a, Message, Theme, iced::Renderer> {
    fn from(headroom: Headroom) -> Self {
        Self::new(headroom)
    }
}
