//! **The oscilloscope** — the delivered waveform itself, drawn as a trace.
//!
//! The fourth visualisation, and the first that is not a bar chart. Spectrum
//! answers *what frequencies*, the rolling waveform answers *how loud, over the
//! last second*, the spectrogram answers *both, over time* — and all three are
//! rectangles whose heights change. A scope answers a different question
//! entirely: **what shape is the air making right now**. A sine is a sine, a
//! square wave is visibly square, a heavily limited master is a slab, and a
//! quiet passage is a thin wandering line. It is the one reading in the set
//! that is about the signal rather than about a measurement of it.
//!
//! # The trigger is the whole difference between this and noise
//!
//! [`baz_core::engine::VISUAL_SAMPLE_COUNT`] points arrive per frame, sampled
//! from wherever the last delivered block happened to start. Drawn as they
//! come, a steady 200 Hz tone slides sideways a random distance every frame and
//! reads as static — the trace is correct and completely unreadable.
//!
//! Every oscilloscope ever built solves this the same way and so does this one:
//! **start drawing at a rising zero crossing.** The waveform is then pinned to
//! its own period rather than to the block boundary, so a steady tone stands
//! still and a changing one moves because *it* changed. The search is bounded
//! to the first half of the buffer so there is always a whole half left to
//! draw, and a buffer with no crossing at all — silence, or DC — draws from the
//! start, because a flat line in the wrong phase is still a flat line.
//!
//! # Why it is drawn in quads
//!
//! [`crate::response`]'s note is the one to read: iced's `canvas` would give
//! strokes directly and costs a tessellation stack this project prices
//! deliberately. A trace is a column per pixel spanning from the previous
//! sample's height to this one, which is the same construction the equaliser's
//! curve uses — and it is what makes a steep edge a line rather than a ladder.

use iced::advanced::renderer::Renderer as _;
use iced::advanced::widget::{Widget, tree};
use iced::advanced::{Layout, layout, mouse, renderer};
use iced::{Color, Element, Length, Rectangle, Size, Theme};

use crate::theme;

/// How far into the buffer a rising zero crossing is looked for.
///
/// Half, so the drawn half is always whole: a trigger found at 60 % would
/// leave 40 % of a screen's worth of trace and a blank right-hand edge that
/// moved about, which is the jitter this exists to remove, relocated.
const TRIGGER_SEARCH: f32 = 0.5;

/// The trace's thickness, and the floor on a column's height so a silent
/// passage is a line rather than nothing.
const LINE_W: f32 = 1.5;

/// How much of the half-height a full-scale sample uses.
///
/// Not 1.0: a master that clips would touch the top and bottom edges and read
/// as *cut off* rather than as *loud*, and the trace would collide with the
/// title drawn over this field.
const REACH: f32 = 0.86;

/// The live trace of the delivered signal.
pub(crate) struct Scope {
    /// The frame's samples, already triggered — see [`triggered`].
    points: Vec<f32>,
    /// The record's three inks, walked across the trace.
    inks: [Color; 3],
    /// The datum's ink.
    datum: Color,
    width: f32,
    height: f32,
}

impl Scope {
    /// Build a scope over one delivered frame.
    pub(crate) fn new(samples: &[f32], inks: [Color; 3], datum: Color, size: Size) -> Self {
        Self {
            points: triggered(samples).to_vec(),
            inks,
            datum,
            width: size.width,
            height: size.height,
        }
    }
}

/// **The samples from the first rising zero crossing onward**, or all of them
/// when there is none.
///
/// Rising rather than either direction, because a falling crossing an octave
/// away in phase would let a symmetric wave alternate between two positions —
/// half the jitter, which looks like a fault rather than like a decision.
fn triggered(samples: &[f32]) -> &[f32] {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a fixed 256-sample buffer, into an index inside it"
    )]
    let limit = ((samples.len() as f32) * TRIGGER_SEARCH) as usize;
    for at in 1..limit {
        if samples[at - 1] <= 0.0 && samples[at] > 0.0 {
            return &samples[at..];
        }
    }
    samples
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Scope {
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
        if bounds.width < 1.0 || self.points.len() < 2 {
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

        let middle = bounds.y + bounds.height / 2.0;
        let reach = bounds.height / 2.0 * REACH;
        // **The datum, unbroken.** A scope without one is a wandering line
        // with no stated rest position, so a quiet passage reads as drifting
        // rather than as quiet.
        quad(
            bounds.x,
            middle - 0.5,
            bounds.width,
            1.0,
            theme::alpha(self.datum, 0.5),
        );

        // **A column per pixel, interpolated — not a column per sample.**
        //
        // 256 points across a window is five pixels each, and a rectangle per
        // sample draws the trace as a five-pixel staircase: correct, and
        // visibly quantised in a way the signal is not. So the walk is over
        // the *pixels* and the sample is read between the two points that
        // straddle each one, which is what makes a rising edge a slope rather
        // than a step. The same construction [`crate::response`] uses for the
        // equaliser's curve, and the reason both read as lines.
        let y_of = |sample: f32| middle - sample.clamp(-1.0, 1.0) * reach;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a window's width in pixels"
        )]
        let columns = bounds.width.max(1.0) as usize;
        #[expect(clippy::cast_precision_loss, reason = "as above")]
        let span = (self.points.len() - 1) as f32;
        let at_of = |column: usize| {
            #[expect(clippy::cast_precision_loss, reason = "as above")]
            let across = column as f32 / (columns.max(2) - 1) as f32;
            let exact = across * span;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "as above"
            )]
            let index = exact as usize;
            let next = (index + 1).min(self.points.len() - 1);
            let t = exact - exact.floor();
            (self.points[next] - self.points[index]).mul_add(t, self.points[index])
        };
        let mut previous = y_of(at_of(0));
        for column in 1..columns {
            #[expect(clippy::cast_precision_loss, reason = "as above")]
            let left = bounds.x + column as f32 - 1.0;
            let sample = at_of(column);
            let y = y_of(sample);
            // **From the last height to this one**, so a steep edge is drawn
            // as the line it is instead of as a stack of disconnected marks.
            let top = previous.min(y) - LINE_W / 2.0;
            let height = (previous - y).abs() + LINE_W;
            let ink = crate::visualizer::level_ink(
                sample.abs(),
                crate::visualizer::across(column, columns),
                self.inks,
            );
            quad(left, top, LINE_W.max(1.0), height, ink);
            previous = y;
        }
    }
}

impl<'a, Message: 'a> From<Scope> for Element<'a, Message, Theme, iced::Renderer> {
    fn from(scope: Scope) -> Self {
        Self::new(scope)
    }
}

#[cfg(test)]
mod tests {
    use super::{TRIGGER_SEARCH, triggered};

    /// **A steady tone starts at the same phase every frame**, which is the
    /// whole of what the trigger is for.
    ///
    /// Drawn untriggered, a 256-point window of a periodic signal starts
    /// wherever the delivered block happened to start, so the trace slides a
    /// random distance sideways every frame and a listener sees static. Here
    /// the same wave is offered at four different starting phases and asked
    /// where the trace begins: the same place, every time, to within a sample.
    #[test]
    fn the_same_tone_is_triggered_at_the_same_phase_whatever_the_block_offset() {
        let period = 32.0_f32;
        let wave = |phase: usize| -> Vec<f32> {
            (0..256)
                .map(|at| {
                    #[expect(clippy::cast_precision_loss, reason = "a 256-point test buffer")]
                    let t = (at + phase) as f32;
                    (t / period * std::f32::consts::TAU).sin()
                })
                .collect()
        };
        let mut firsts = Vec::new();
        for phase in [0, 7, 16, 23] {
            let samples = wave(phase);
            let trace = triggered(&samples);
            // The first point after a rising crossing is just above zero, and
            // the next is rising further. That is the phase, stated without
            // relying on an index the offset changes.
            assert!(
                trace[0] > 0.0 && trace[1] > trace[0],
                "phase {phase} did not trigger on a rising crossing"
            );
            firsts.push(trace[0]);
        }
        let spread = firsts.iter().fold(f32::MIN, |a, b| a.max(*b))
            - firsts.iter().fold(f32::MAX, |a, b| a.min(*b));
        assert!(
            spread < 0.25,
            "the trigger lands at four different phases (spread {spread})"
        );
    }

    /// **Silence draws from the start rather than not at all.**
    ///
    /// A flat line has no rising crossing, and the honest answer is the flat
    /// line: a scope that showed nothing during a quiet passage would look
    /// broken at exactly the moment a listener is most likely to check it.
    #[test]
    fn a_signal_with_no_crossing_is_drawn_from_its_start() {
        for flat in [vec![0.0_f32; 256], vec![-0.4; 256], vec![0.7; 256]] {
            assert_eq!(
                triggered(&flat).len(),
                flat.len(),
                "a signal with no rising crossing lost samples"
            );
        }
    }

    /// **Half the buffer is always left to draw.**
    ///
    /// A trigger found late would leave a short trace and a blank right-hand
    /// edge whose width moved from frame to frame — the jitter the trigger
    /// exists to remove, relocated to the other end of the trace.
    #[test]
    fn the_drawn_trace_is_never_shorter_than_half_the_buffer() {
        // A wave whose first rising crossing is as late as the search allows.
        let mut samples = vec![-1.0_f32; 256];
        for sample in samples.iter_mut().skip(200) {
            *sample = 1.0;
        }
        let trace = triggered(&samples);
        #[expect(clippy::cast_precision_loss, reason = "a 256-point test buffer")]
        let drawn = trace.len() as f32;
        #[expect(clippy::cast_precision_loss, reason = "as above")]
        let floor = samples.len() as f32 * TRIGGER_SEARCH;
        assert!(
            drawn >= floor,
            "a late crossing left {} of 256 points",
            trace.len()
        );
    }
}
