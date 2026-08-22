//! **The ground the visualisation becomes when it is not what you are looking
//! at.**
//!
//! The owner, 2026-08-22: *"when we switch to other screens and the visualizer
//! stays in the background… it should be heavily blurred or opaque? … almost
//! that liquid glass look"*.
//!
//! # The mode stops mattering here, and that is the decision
//!
//! Four visualisations answer four different questions about the signal —
//! *what frequencies*, *how loud lately*, *both over time*, *what shape is the
//! air making* — and each is a reading somebody chose to look at. Away from
//! Now playing **nobody is reading it**. It is behind a wall of records, a
//! playlist, a settings form; it is the room's weather, not an instrument.
//!
//! So all four become **one** ground here, and that is not laziness: at the
//! blur the owner asked for, a spectrum and a spectrogram are the same
//! picture. Keeping four constructions to be indistinguishable would be four
//! things to maintain for one appearance, and it would put a mode switch
//! behind a surface where its effect cannot be seen.
//!
//! # How something soft is drawn with rectangles
//!
//! [`crate::response`]'s note is the standing one: iced's `canvas` would give
//! strokes and gradients directly and costs a tessellation stack this project
//! prices deliberately. There is no blur pass to reach for either — a blur
//! wants a texture, and quads do not make one.
//!
//! What quads *do* make is **accumulated alpha**, which is what a blur looks
//! like from the outside. Each band is drawn as [`RINGS`] nested capsules,
//! largest and faintest first, each corner-radius'd to half its own height so
//! it has no corners at all. Their union has no edge you can point at: the
//! alpha climbs towards the middle and falls away to nothing, which is a
//! radial falloff built out of the one primitive available. Sixty-three quads
//! for the whole field — cheaper than the spectrum it replaces, which draws
//! one per band per frame plus its spacing.
//!
//! # It is a ground, so nothing may be read off it
//!
//! Every contrast floor over this surface is the floor over the room's own
//! wall, unchanged — which is only true because the whole field is capped at
//! [`CEILING`] alpha and the frost pane sits over it. The blobs breathe with
//! the record; nothing else about the interface moves because of them.

use iced::advanced::renderer::Renderer as _;
use iced::advanced::widget::{Widget, tree};
use iced::advanced::{Layout, layout, mouse, renderer};
use iced::{Color, Element, Length, Rectangle, Size, Theme};

/// How many soft blobs the field is drawn as.
///
/// Seven, not the spectrum's own band count: the point of this surface is that
/// it has no detail, and a low-pass at the *source* is a cheaper and more
/// honest blur than drawing detail and then hiding it.
const BLOBS: usize = 7;

/// The nested capsules one blob is built from — see the module note.
///
/// **Twenty-four, and the number is a photograph's.** At nine the steps
/// between them are eleven per cent of the blob's size and the first capture
/// of this surface came back with visible contour rings — a topographic map of
/// a blur, which is the one thing it must not look like. At twenty-four each
/// step is four per cent and the falloff reads as continuous. They cost one
/// quad each: a hundred and sixty-eight for the whole field, against the
/// spectrum's own twenty-four plus its spacing.
const RINGS: usize = 24;

/// How wide a blob is, as a multiple of the spacing between their centres.
///
/// Above one, so neighbours overlap and their alphas add. That overlap is
/// what stops the field reading as seven separate lamps.
const SPREAD: f32 = 3.0;

/// The most alpha any single blob's centre may reach.
///
/// The ceiling is the whole reason the contrast floors over this ground are
/// the room's own unchanged floors: what a reader stands on is the wall, very
/// slightly tinted, whatever the record is doing.
const CEILING: f32 = 0.62;

/// The share of the height a silent band still occupies.
///
/// Not zero: a quiet passage should read as *calm*, not as the visualisation
/// having stopped. Something that vanished between tracks would look like a
/// fault on a surface nobody is watching closely enough to diagnose.
const REST: f32 = 0.22;

/// The rest of the height, spent on how loud the band is.
const SWELL: f32 = 0.78;

/// **How much of the height a band at `level` takes.**
///
/// Its own function so [`REST`]'s promise — that silence is calm rather than
/// absent — is a thing a test can call rather than a constant it can only
/// re-read.
fn share(level: f32) -> f32 {
    SWELL.mul_add(level.clamp(0.0, 1.0), REST)
}

/// **The soft ground**, one blob per band.
pub(crate) struct Glass {
    /// The bands, already reduced to [`BLOBS`].
    levels: [f32; BLOBS],
    /// The record's three inks, walked across the field.
    inks: [Color; 3],
}

impl Glass {
    /// Build the ground from a frame's bands.
    ///
    /// `bands` is however many the spectrum measures; they are averaged down
    /// to [`BLOBS`] in order, which is the low-pass the module note describes.
    ///
    /// **It takes no measure**, unlike [`crate::scope`]: this is the window's
    /// backdrop and nothing else, so it fills whatever it is given and the
    /// blob geometry is derived from the bounds at draw time.
    pub(crate) fn new(bands: &[f32], inks: [Color; 3]) -> Self {
        Self {
            levels: reduce(bands),
            inks,
        }
    }
}

/// **`bands` averaged down to [`BLOBS`], in order.**
///
/// Averaged rather than sampled: taking every *n*th band would let one loud
/// narrow peak carry a whole blob and leave the bands beside it unrepresented,
/// which is aliasing — and a blur that aliases is not a blur.
fn reduce(bands: &[f32]) -> [f32; BLOBS] {
    if bands.is_empty() {
        return [0.0; BLOBS];
    }
    std::array::from_fn(|blob| {
        let from = blob * bands.len() / BLOBS;
        let to = ((blob + 1) * bands.len() / BLOBS)
            .max(from + 1)
            .min(bands.len());
        let slice = &bands[from..to];
        #[expect(clippy::cast_precision_loss, reason = "a band count below 256")]
        let count = slice.len() as f32;
        slice.iter().sum::<f32>() / count
    })
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Glass {
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut iced::advanced::widget::Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, Length::Fill, Length::Fill)
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
        if bounds.width < 1.0 || bounds.height < 1.0 {
            return;
        }
        #[expect(clippy::cast_precision_loss, reason = "a blob count of seven")]
        let pitch = bounds.width / BLOBS as f32;
        let middle = bounds.y + bounds.height / 2.0;
        for (blob, level) in self.levels.iter().enumerate() {
            let level = level.clamp(0.0, 1.0);
            #[expect(clippy::cast_precision_loss, reason = "as above")]
            let centre = bounds.x + (blob as f32 + 0.5) * pitch;
            let ink = crate::visualizer::level_ink(
                level,
                crate::visualizer::across(blob, BLOBS),
                self.inks,
            );
            let full_w = pitch * SPREAD;
            let full_h = bounds.height * share(level);
            for ring in 0..RINGS {
                // **Largest and faintest first.** `share` runs 1 → 1/RINGS, so
                // the outermost capsule is the whole blob at the lowest alpha
                // and each one inside it is smaller and stronger. Their sum is
                // the falloff.
                #[expect(clippy::cast_precision_loss, reason = "a ring count of nine")]
                let share = (RINGS - ring) as f32 / RINGS as f32;
                let w = full_w * share;
                let h = full_h * share;
                if w < 1.0 || h < 1.0 {
                    continue;
                }
                #[expect(clippy::cast_precision_loss, reason = "as above")]
                let alpha = ink.a * CEILING / RINGS as f32;
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: centre - w / 2.0,
                            y: middle - h / 2.0,
                            width: w,
                            height: h,
                        },
                        border: iced::Border {
                            // No corners at all: half the shorter side is a
                            // capsule, which is the closest a rectangle comes
                            // to having no edge.
                            radius: (w.min(h) / 2.0).into(),
                            ..iced::Border::default()
                        },
                        ..renderer::Quad::default()
                    },
                    Color { a: alpha, ..ink },
                );
            }
        }
    }
}

impl<'a, Message: 'a> From<Glass> for Element<'a, Message, Theme, iced::Renderer> {
    fn from(glass: Glass) -> Self {
        Self::new(glass)
    }
}

#[cfg(test)]
mod tests {
    use super::{BLOBS, CEILING, RINGS, reduce, share};

    /// **Every band is represented, and none of them twice.**
    ///
    /// A blur that dropped bands would let a loud narrow peak vanish and a
    /// quiet one carry a whole blob — aliasing, which is the one thing a
    /// low-pass may not do.
    #[test]
    fn reducing_the_bands_averages_all_of_them_in_order() {
        let bands: Vec<f32> = (0..64_u16).map(|band| f32::from(band) / 63.0).collect();
        let blobs = reduce(&bands);
        for pair in blobs.windows(2) {
            assert!(pair[1] > pair[0], "the ramp did not survive: {blobs:?}");
        }
        assert!(blobs[0] < 0.2 && blobs[BLOBS - 1] > 0.8, "{blobs:?}");
    }

    /// **A field with nothing in it still answers**, rather than dividing by a
    /// band count it does not have.
    #[test]
    fn an_empty_frame_is_a_flat_field() {
        for (bands, want) in [(vec![], 0.0_f32), (vec![0.5], 0.5)] {
            for blob in reduce(&bands) {
                assert!((blob - want).abs() < 1e-6, "{blob} is not {want}");
            }
        }
    }

    /// **Silence is calm, not absence.** Something that vanished between
    /// tracks would read as a fault on a surface nobody is watching closely
    /// enough to diagnose — and a full band fills the field exactly, rather
    /// than overflowing it.
    #[test]
    fn a_silent_band_still_occupies_the_field() {
        assert!(share(0.0) > 0.1, "silence draws nothing");
        assert!(
            (share(1.0) - 1.0).abs() < 1e-6,
            "a loud band is not the height"
        );
        assert!(share(0.5) > share(0.0) && share(1.0) > share(0.5));
        // Out of range is not a taller blob than the field it is drawn in.
        assert!((share(4.0) - share(1.0)).abs() < f32::EPSILON);
        assert!((share(-1.0) - share(0.0)).abs() < f32::EPSILON);
    }

    /// **No record can make this ground loud**, which is why every contrast
    /// floor over it is the floor over the bare wall — the reason the rest of
    /// the product needed no re-measuring when this arrived.
    ///
    /// The rings of one blob sum to the ink's own alpha times [`CEILING`], so
    /// the check is over the sum rather than over any one of them.
    #[test]
    fn no_record_can_make_this_ground_loud() {
        for level in [0.0_f32, 0.5, 1.0] {
            let ink = crate::visualizer::level_ink(level, 0.5, [iced::Color::WHITE; 3]);
            #[expect(clippy::cast_precision_loss, reason = "a ring count of nine")]
            let stacked: f32 = (0..RINGS).map(|_| ink.a * CEILING / RINGS as f32).sum();
            assert!(
                stacked <= CEILING,
                "a band at {level} stacks to {stacked}, over the {CEILING} ceiling"
            );
        }
    }
}
