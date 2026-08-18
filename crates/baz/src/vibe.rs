//! Home's opt-in, local sonic-playlist state.
//!
//! The full build delegates decoding, MIR extraction, persistence and ranking
//! to the optional `baz-vibe` crate. A light build retains the same Home seam
//! but contains no analyzer dependency or model/runtime payload.

use std::collections::{HashMap, HashSet, VecDeque};
#[cfg(feature = "vibe-analysis")]
use std::path::Path;
use std::path::PathBuf;

use crate::vm::{self, AlbumVm, EditionKey, QueueItemVm};

/// An ordinary generated mix is deliberately bounded and editable.
///
/// Behind the feature because the light build has no generator to bound: it
/// keeps the same Home seam and contains no analyser at all, and a constant
/// nothing can reach is a constant nobody maintains.
#[cfg(feature = "vibe-analysis")]
pub(crate) const PLAYLIST_CAP: usize = 64;

#[cfg(feature = "vibe-analysis")]
type SonicFeatures = baz_vibe::Features;

#[cfg(not(feature = "vibe-analysis"))]
type SonicFeatures = u8;

/// **One musical dimension a contour line can be drawn over.**
///
/// Mirrors `baz_vibe::Dimension` — the engine is an optional dependency, so
/// the vocabulary the interface is written in cannot live behind that
/// feature — and the conversion happens at the one gated call site.
///
/// The owner: *"can we have more than one of these for different musical
/// dimensions — this obviously kinda rolls up several aspects of a song into
/// one value."* He is describing [`Dimension::Energy`], which is exactly that
/// roll-up. It stays, because it is what most people mean by *a mix that
/// builds*; the others are its parts and its neighbours, each on a line of
/// its own, and **each is a stated combination of measurements rather than a
/// mood**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Dimension {
    Energy,
    Tempo,
    Brightness,
    Dynamics,
    Texture,
}

impl Dimension {
    /// Every dimension, in the order the interface offers them.
    pub(crate) const ALL: [Self; 5] = [
        Self::Energy,
        Self::Tempo,
        Self::Brightness,
        Self::Dynamics,
        Self::Texture,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Energy => "Energy",
            Self::Tempo => "Tempo",
            Self::Brightness => "Brightness",
            Self::Dynamics => "Dynamics",
            Self::Texture => "Texture",
        }
    }

    /// The two ends of its axis, low first — the words beside the line.
    pub(crate) const fn ends(self) -> (&'static str, &'static str) {
        match self {
            Self::Energy => ("calmer", "louder"),
            Self::Tempo => ("slower", "faster"),
            Self::Brightness => ("darker", "brighter"),
            Self::Dynamics => ("steadier", "swingier"),
            Self::Texture => ("cleaner", "noisier"),
        }
    }

    /// **What a song at one end of this axis simply *is*** — plain adjectives
    /// rather than the comparatives [`Self::ends`] labels the axis with.
    ///
    /// The owner, on what the whole feature has to make visible: *"one track
    /// represents a combination of the different points on that curve. e.g.
    /// loud, fast, dynamic? or quiet, slow, compressed."* These are those
    /// words. `ends` says which way the line goes — *louder* — and this says
    /// what the song there is — *loud* — because a row is describing itself,
    /// not comparing itself to the row above.
    pub(crate) const fn plain_ends(self) -> (&'static str, &'static str) {
        match self {
            Self::Energy => ("quiet", "loud"),
            Self::Tempo => ("slow", "fast"),
            Self::Brightness => ("dark", "bright"),
            Self::Dynamics => ("steady", "swinging"),
            Self::Texture => ("clean", "noisy"),
        }
    }

    /// **How much this line counts against the others**, as a percentage.
    ///
    /// The blend is weighted with energy dominant, so the five lines do not
    /// influence a result equally — dragging texture moves a list a quarter
    /// as far as dragging energy does. That is a surprise worth removing
    /// rather than a detail: it is exactly the *"isn't clear how it
    /// influences things"* the owner met.
    pub(crate) fn share(self) -> u8 {
        let weight = Self::ALL
            .iter()
            .position(|held| *held == self)
            .and_then(|index| Contour::BLEND.get(index).copied())
            .unwrap_or(0.0);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a weight in the unit interval times one hundred"
        )]
        {
            (weight * 100.0).round() as u8
        }
    }

    /// What it is measured from, plainly enough to put on screen.
    pub(crate) const fn measured_from(self) -> &'static str {
        match self {
            Self::Energy => "How loud and how fast a song is.",
            Self::Tempo => "How fast the beat is.",
            Self::Brightness => "How bright or dark a song sounds.",
            Self::Dynamics => "How much the loudness moves inside a song.",
            Self::Texture => "How clean or noisy a song sounds.",
        }
    }
}

/// **One point on a line** — how far through the finished playlist, and the
/// level the music should be at when it gets there.
///
/// `at` is `0.0` for the opening track and `1.0` for the last; `level` is the
/// collection-relative −2…+2 scale the engine scores against, where −2 is the
/// low end of *this* library on that dimension and +2 its high end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ContourPoint {
    pub(crate) at: f32,
    pub(crate) level: f32,
}

/// One dimension, the shape asked of it, and how much it counts against the
/// others.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Lane {
    pub(crate) dimension: Dimension,
    pub(crate) points: Vec<ContourPoint>,
    /// **How much this line counts.** Mirrors `baz_vibe::Lane::weight`; see
    /// [`Contour::BLEND`] for why one default line is five weighted ones.
    pub(crate) weight: f32,
}

/// **The shape the next generated playlist is asked to follow** — a line per
/// dimension.
///
/// A dimension with no lane is *unconstrained*: it does not enter the cost at
/// all, so a contour over energy alone lets everything else fall where the
/// music does.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Contour {
    pub(crate) lanes: Vec<Lane>,
}

impl Contour {
    /// Smallest gap between neighbouring points, as a fraction of the list.
    /// Two points at one position would ask for two levels at once.
    const MIN_GAP: f32 = 0.06;

    /// **How many points a line may carry**, and the range the count control
    /// offers.
    ///
    /// Two is a straight line between the first song and the last — the
    /// fewest a line can have and still be one.
    ///
    /// Ten is the owner's number, and it is deliberately past the point where
    /// every segment holds a song: an hour is around eighteen tracks, so ten
    /// points is a handle every two songs, and half an hour has fewer songs
    /// than handles. **That is a real limit and it is his to spend** — the
    /// cost of a line finer than the list is that the last of the detail
    /// cannot be expressed, not that anything breaks. The gain is that the
    /// control the page is now built around can actually be drawn on rather
    /// than only tilted.
    pub(crate) const MIN_POINTS: usize = 2;
    pub(crate) const MAX_POINTS: usize = 10;

    /// **What a fresh line carries.** Two points is a control you can tilt;
    /// ten is one you can draw with, which is what the line being the page's
    /// first question asks of it (design note 25).
    pub(crate) const DEFAULT_POINTS: usize = 10;

    /// The opening line: the given arc, at the default resolution, **evenly
    /// spaced**.
    ///
    /// Not [`Self::set_points`], which grows a line by halving its widest gap
    /// — the rule that lets a listener add a handle without their drawing
    /// moving, and the rule that turns two points into 1/8ths and a stray
    /// 1/16th on the way to ten. Nothing here is anybody's drawing yet, so
    /// the line can be sampled instead, and a row of evenly spaced handles is
    /// the only honest picture of *you may drag any part of this*.
    pub(crate) fn opening(points: &[ContourPoint]) -> Self {
        Self::blended(&resampled(points, Self::DEFAULT_POINTS))
    }

    /// **The weights of the blended line**, in [`Dimension::ALL`] order,
    /// energy dominant, summing to one. Mirrors `baz_vibe::Contour::BLEND`;
    /// the two are pinned together by `the_drawn_line_is_the_scored_line`.
    ///
    /// Design 21 §5: an *unweighted* mean of rank axes puts loud-and-slow in
    /// the same place as quiet-and-fast, so a line through the middle would be
    /// satisfied by tracks that sound nothing alike — the *"dots aren't
    /// following my line"* failure, back in a different hat.
    pub(crate) const BLEND: [f32; 5] = [0.40, 0.20, 0.15, 0.15, 0.10];

    /// **The default request's line**: every dimension asked for the same
    /// shape, weighted with energy dominant.
    ///
    /// The listener sees one line. It is five, holding one curve between them,
    /// which is why design 21 §5's expander *reveals* the per-dimension curves
    /// rather than seeding them — they were already the blend, and stay it
    /// until one is dragged away from its neighbours.
    pub(crate) fn blended(points: &[ContourPoint]) -> Self {
        Self {
            lanes: Dimension::ALL
                .into_iter()
                .zip(Self::BLEND)
                .map(|(dimension, weight)| Lane {
                    dimension,
                    points: points.to_vec(),
                    weight,
                })
                .collect(),
        }
    }

    /// **Whether every line still holds the same curve** — whether, in other
    /// words, this is still one line as far as the listener is concerned.
    ///
    /// The page draws one line while this is true and the five while it is
    /// not, which is how "tune each thing baz listens for" can be a disclosure
    /// rather than a mode.
    pub(crate) fn is_one_line(&self) -> bool {
        let mut lanes = self.lanes.iter();
        let Some(first) = lanes.next() else {
            return true;
        };
        lanes.all(|lane| lane.points == first.points)
    }

    /// **Give every line exactly `count` points**, adding turns where there is
    /// most room for one and taking them off the end.
    ///
    /// A new turn lands **on the line it joins** — at the level the line
    /// already stands at, in the widest gap — so gaining a handle changes the
    /// shape by nothing and the listener drags it deliberately rather than
    /// recovering from a jump.
    ///
    /// The `−`/`+` stepper this replaces was deleted with design 21 §5, and
    /// it should have been: it was two marks that said nothing about where
    /// you were in a range. What it took with it was the *capability* — with
    /// a two-point preset you could tilt a line and nothing else, which is
    /// the owner's *"the graph/curve does not allow any users to adjust the
    /// curve? maybe add a point count?"* A count says how many there are and
    /// how many there could be, in the same pills as everything else on the
    /// page.
    pub(crate) fn set_points(&mut self, count: usize) {
        let count = count.clamp(Self::MIN_POINTS, Self::MAX_POINTS);
        for lane in &mut self.lanes {
            while lane.points.len() > count {
                let index = lane.points.len() - 2;
                lane.points.remove(index);
            }
            while lane.points.len() < count {
                let Some((index, at)) = lane
                    .points
                    .windows(2)
                    .enumerate()
                    .max_by(|left, right| {
                        (left.1[1].at - left.1[0].at).total_cmp(&(right.1[1].at - right.1[0].at))
                    })
                    .map(|(index, pair)| (index, f32::midpoint(pair[0].at, pair[1].at)))
                else {
                    break;
                };
                let level = level_at(&lane.points, at).unwrap_or(0.0);
                lane.points.insert(index + 1, ContourPoint { at, level });
            }
        }
    }

    /// How many points the drawn line carries.
    pub(crate) fn points(&self) -> usize {
        self.lane(0).map_or(0, |lane| lane.points.len())
    }

    pub(crate) fn lane(&self, index: usize) -> Option<&Lane> {
        self.lanes.get(index)
    }

    /// **Move one point of one line**, within what a line may be: the ends
    /// stay at the ends — a playlist has a first track and a last — and an
    /// interior point stays between its neighbours.
    pub(crate) fn drag(&mut self, lane: usize, index: usize, at: f32, level: f32) {
        let Some(lane) = self.lanes.get_mut(lane) else {
            return;
        };
        let last = lane.points.len().saturating_sub(1);
        let Some(point) = lane.points.get_mut(index) else {
            return;
        };
        point.level = level.clamp(-LEVEL_LIMIT, LEVEL_LIMIT);
        if index == 0 || index == last {
            return;
        }
        let low = lane.points[index - 1].at + Self::MIN_GAP;
        let high = lane.points[index + 1].at - Self::MIN_GAP;
        if low <= high {
            lane.points[index].at = at.clamp(low, high);
        }
    }
}

/// The line's level over `fraction`, from its points alone.
///
/// A free function so the drawing can ask it per column without owning a
/// contour — the widget reads a borrowed slice at 4 px steps and an
/// allocation per column would be a per-frame cost for arithmetic.
pub(crate) fn level_at(points: &[ContourPoint], fraction: f32) -> Option<f32> {
    let fraction = fraction.clamp(0.0, 1.0);
    let first = points.first()?;
    if points.len() == 1 || fraction <= first.at {
        return Some(first.level);
    }
    let last = points.last()?;
    if fraction >= last.at {
        return Some(last.level);
    }
    let pair = points
        .windows(2)
        .find(|pair| fraction >= pair[0].at && fraction <= pair[1].at)?;
    let span = pair[1].at - pair[0].at;
    if span.abs() <= f32::EPSILON {
        return Some(pair[1].level);
    }
    let mix = (fraction - pair[0].at) / span;
    Some(pair[0].level + (pair[1].level - pair[0].level) * mix)
}

/// **How far in from each end a named extreme is taken**: a hundredth.
///
/// Not the end itself, because the end is where a misdetection lands — see
/// [`State::rebuild_profile`]. Not far in either: over a five-thousand-track
/// library this steps past about fifty, which still leaves the record named
/// inside the top one per cent, and on a library too small for a hundredth to
/// be anything it steps past none and names the true end.
#[cfg(feature = "vibe-analysis")]
const EXTREME_MARGIN: usize = 100;

/// **Below this p05–p95 span, an axis has nothing to say about a
/// collection.**
///
/// **Measured, not guessed** — `cargo run -p baz-vibe --bin vibe-spread` over
/// a real 5 076-track library, 2026-08-16:
///
/// ```text
///                p05      p50      p95     span
/// Energy       0.260    0.505    0.674    0.413
/// Tempo       -0.118    0.235    0.625    0.743
/// Brightness  -0.900   -0.730   -0.508    0.392
/// Dynamics     0.285    0.641    0.785    0.499
/// Texture     -0.938   -0.558   -0.168    0.771
/// ```
///
/// The narrowest axis a varied collection produces spans 0.39, so 0.12 is a
/// third of that and is reached only by a library that genuinely does not
/// move — a set of takes of one piece, a DJ set at one tempo. Deliberately
/// conservative: a false *"this line will not do much"* is worse than a
/// missing one, because it would talk somebody out of a control that works.
#[cfg(feature = "vibe-analysis")]
const FLAT_AXIS: f32 = 0.12;

/// The furthest a contour may reach on either axis: the collection's own
/// extremes, which is what the engine clamps its targets to.
const LEVEL_LIMIT: f32 = 2.0;

/// **A named shape, offered as a picture rather than as a word.**
///
/// The owner asked for *"a few defaults"* beside free text, and got four
/// buttons whose labels were the whole of the information. These are drawn:
/// each is the contour widget at thumbnail size, so what a preset does is
/// visible before it is pressed.
///
/// `Any` is first and is not a shape at all — it is the honest way to say
/// *the words alone*, which has to remain reachable now that a shape is the
/// default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Shape {
    pub(crate) label: &'static str,
    points: &'static [(f32, f32)],
}

impl Shape {
    /// Words alone: no line, nothing steered by position.
    pub(crate) const ANY: Self = Self {
        label: "Any",
        points: &[],
    };

    /// What a fresh Vibe request starts from. A rise is the commonest arc a
    /// listener means by *a mix*, and starting from it teaches the control in
    /// one glance — a flat line would draw a shape that says nothing about
    /// what dragging it would do.
    pub(crate) const DEFAULT: Self = Self {
        label: "Slow build",
        points: &[(0.0, -1.6), (1.0, 1.6)],
    };

    /// Every shape the wizard offers, in the order it offers them.
    pub(crate) const ALL: [Self; 6] = [
        Self::ANY,
        Self {
            label: "Steady",
            points: &[(0.0, 0.0), (1.0, 0.0)],
        },
        Self::DEFAULT,
        Self {
            label: "Peak and fall",
            points: &[(0.0, -1.2), (0.55, 1.8), (1.0, -0.8)],
        },
        Self {
            label: "Wind down",
            points: &[(0.0, 1.4), (1.0, -1.8)],
        },
        Self {
            label: "Waves",
            points: &[
                (0.0, -1.0),
                (0.25, 1.2),
                (0.5, -0.6),
                (0.75, 1.4),
                (1.0, -1.0),
            ],
        },
    ];

    /// The points this shape stands for.
    pub(crate) fn points(self) -> Vec<ContourPoint> {
        self.points
            .iter()
            .map(|&(at, level)| ContourPoint { at, level })
            .collect()
    }
}

/// **A recipe: a mood you can start from.**
///
/// The owner: *"we should have like 5-6 standard recipes — as part of the
/// wizard we should be asking users if they want to make a preset one. as
/// long as the presets are some really common moods and themes."*
///
/// A recipe is **words + a shape + a length**, which is the whole of a
/// request, so pressing one fills the form and changes nothing else: every
/// field stays editable, and a listener can take the words and redraw the
/// line, or keep the line and retype the words. It is a starting point and
/// never a mode.
///
/// The six are the moods people actually ask for rather than the ones that
/// demonstrate the machinery. Each one's words are ordinary language — they
/// go to the same text tower a typed request does — and each one's shape is
/// one of the drawn presets, so a recipe explains itself in the picture as
/// well as in its name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Recipe {
    pub(crate) label: &'static str,
    pub(crate) prompt: &'static str,
    shape: usize,
    pub(crate) length: MixLength,
}

impl Recipe {
    /// Every recipe the wizard offers, in the order it offers them.
    pub(crate) const ALL: [Self; 6] = [
        Self {
            label: "Late-night drive",
            prompt: "warm hypnotic music for driving at night",
            shape: 1,
            length: MixLength::Hour,
        },
        Self {
            label: "Sunday morning",
            prompt: "gentle unhurried music for a slow morning",
            shape: 2,
            length: MixLength::Hour,
        },
        Self {
            label: "Focus",
            prompt: "calm instrumental music without vocals for concentrating",
            shape: 1,
            length: MixLength::NinetyMinutes,
        },
        Self {
            label: "Workout",
            prompt: "fast loud driving music with a hard pulse",
            shape: 2,
            length: MixLength::HalfHour,
        },
        Self {
            label: "Wind down",
            prompt: "quiet soft slow music for the end of the day",
            shape: 4,
            length: MixLength::HalfHour,
        },
        Self {
            label: "Party",
            prompt: "upbeat energetic danceable music",
            shape: 3,
            length: MixLength::TwoHours,
        },
    ];

    /// The drawn shape this recipe starts from.
    pub(crate) fn shape(self) -> Shape {
        Shape::ALL[self.shape]
    }
}

/// How many bands the library's own distribution is drawn in behind the line.
///
/// Only the full build has a library to describe — a light one has no
/// analyser at all — so the histogram and its bucket count belong to it. The
/// contour itself is drawn either way: a shape is a request, and a request
/// does not need an analyser to be made.
#[cfg(feature = "vibe-analysis")]
/// Sixteen over four levels is a band per quarter-level: enough to show where
/// a collection sits, coarse enough that one loud record is not a spike.
pub(crate) const FIELD_BUCKETS: usize = 16;

/// Bucket a set of levels into the field the contour draws behind itself,
/// lowest band first, each normalised against the fullest.
///
/// Pure, so the shape of a collection's own histogram is testable without an
/// analyser, a window or a library.
#[cfg(feature = "vibe-analysis")]
pub(crate) fn field_of(levels: impl Iterator<Item = f32>) -> Vec<f32> {
    let mut buckets = vec![0.0_f32; FIELD_BUCKETS];
    let mut seen = 0.0_f32;
    for level in levels {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "the fraction is clamped into the unit interval before the cast, over a \
                      bucket count that is a small literal"
        )]
        let index = (((level + LEVEL_LIMIT) / (2.0 * LEVEL_LIMIT)).clamp(0.0, 1.0)
            * (FIELD_BUCKETS - 1) as f32)
            .round() as usize;
        if let Some(bucket) = buckets.get_mut(index.min(FIELD_BUCKETS - 1)) {
            *bucket += 1.0;
        }
        seen += 1.0;
    }
    if seen == 0.0 {
        return Vec::new();
    }
    let fullest = buckets.iter().copied().fold(0.0_f32, f32::max).max(1.0);
    for bucket in &mut buckets {
        *bucket /= fullest;
    }
    buckets
}

/// **Where one value stands on a sorted axis**, as the −2…+2 level the engine
/// scores against: a track's *place* in the pool, not its fraction of the
/// distance between the two most extreme records.
///
/// Loudness and tempo cluster hard — a real library has a handful of outliers
/// at each end and everything else packed in the middle — so a min–max axis
/// maps almost every track to within a whisker of the centre, and a rising
/// line drawn over it is answered by tracks that are all near the middle. That
/// is exactly the *"dots aren't following my line"* failure, and the rank is
/// what fixes it.
#[cfg(feature = "vibe-analysis")]
fn rank_level(sorted: &[f32], value: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let below = sorted.partition_point(|held| *held < value);
    let through = sorted.partition_point(|held| *held <= value);
    #[expect(
        clippy::cast_precision_loss,
        reason = "a library's track count is far below f32's exact-integer range"
    )]
    let rank = ((below + through) as f32 / 2.0) / sorted.len() as f32;
    rank.clamp(0.0, 1.0)
        .mul_add(2.0 * LEVEL_LIMIT, -LEVEL_LIMIT)
}

/// One dimension in the engine's own vocabulary.
#[cfg(feature = "vibe-analysis")]
const fn engine_dimension(dimension: Dimension) -> baz_vibe::Dimension {
    match dimension {
        Dimension::Energy => baz_vibe::Dimension::Energy,
        Dimension::Tempo => baz_vibe::Dimension::Tempo,
        Dimension::Brightness => baz_vibe::Dimension::Brightness,
        Dimension::Dynamics => baz_vibe::Dimension::Dynamics,
        Dimension::Texture => baz_vibe::Dimension::Texture,
    }
}

/// **The shape, said out loud** — a picture of a request should still be
/// *sayable*.
///
/// It is what a listener checks before spending a compose, it is the whole of
/// the control on a short window, and it is what somebody reading the screen
/// aloud has.
pub(crate) fn shape_words(contour: &Contour) -> &'static str {
    let Some(points) = contour.lane(0).map(|lane| lane.points.as_slice()) else {
        return "in no particular shape";
    };
    let Some(opening) = level_at(points, 0.0) else {
        return "in no particular shape";
    };
    let landing = level_at(points, 1.0).unwrap_or(opening);
    let peak = (0_u8..=10)
        .filter_map(|step| level_at(points, f32::from(step) / 10.0))
        .fold(f32::MIN, f32::max);
    let turns = points.len() > 2;
    let rise = landing - opening;
    if turns && peak > opening.max(landing) + 0.4 {
        "starting quiet, climbing to a peak partway through, then coming down"
    } else if rise > 0.6 {
        "starting quiet and climbing the whole way"
    } else if rise < -0.6 {
        "starting loud and winding down"
    } else if turns {
        "turning on the way through and ending where it started"
    } else {
        "holding one level the whole way"
    }
}

/// **One line, given `count` handles, without changing the line.**
///
/// **Every original point is kept**, and the new ones are shared out among
/// the gaps in proportion to how wide they are. That is what makes this
/// exact rather than approximate: an inserted point lands on a straight
/// segment, so reading the level anywhere gives the same answer it did
/// before. Sampling on a plain even grid was the first attempt and it is
/// wrong — `Waves` turns at 0.25 and 0.5, an even tenth grid misses both, and
/// the preset arrived visibly flattened.
///
/// Even spacing falls out anyway for the case that matters most: a two-point
/// line has one gap, so ten handles land on the ninths.
fn resampled(points: &[ContourPoint], count: usize) -> Vec<ContourPoint> {
    if points.len() < 2 || count <= points.len() {
        return points.to_vec();
    }
    let spare = count - points.len();
    let widths: Vec<f32> = points
        .windows(2)
        .map(|pair| (pair[1].at - pair[0].at).max(0.0))
        .collect();
    let total: f32 = widths.iter().sum();
    if total <= f32::EPSILON {
        return points.to_vec();
    }
    // Largest-remainder, so the handles add up to exactly what was asked for
    // rather than to whatever the rounding happened to leave.
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a handful of points over a unit interval"
    )]
    let mut shares: Vec<usize> = widths
        .iter()
        .map(|width| (width / total * spare as f32) as usize)
        .collect();
    let mut left = spare - shares.iter().sum::<usize>();
    while left > 0 {
        let Some((index, _)) = widths
            .iter()
            .enumerate()
            .max_by(|left, right| {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "a handful of points over a unit interval"
                )]
                let per = |(index, width): (usize, &f32)| *width / (shares[index] + 1) as f32;
                per(*left).total_cmp(&per(*right))
            })
            .map(|(index, width)| (index, *width))
        else {
            break;
        };
        shares[index] += 1;
        left -= 1;
    }
    let mut drawn = Vec::with_capacity(count);
    for (index, pair) in points.windows(2).enumerate() {
        drawn.push(pair[0]);
        let steps = shares[index] + 1;
        for step in 1..steps {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a handful of points over a unit interval"
            )]
            let fraction = step as f32 / steps as f32;
            let at = pair[0].at + (pair[1].at - pair[0].at) * fraction;
            drawn.push(ContourPoint {
                at,
                level: level_at(points, at).unwrap_or(pair[0].level),
            });
        }
    }
    drawn.push(*points.last().expect("two points at least"));
    drawn
}

/// A sentence's first letter, where the sentence was written as a clause.
///
/// The shape phrases are all lower-case gerunds — *starting quiet and
/// climbing the whole way* — because they were written to sit in the middle
/// of a line. Now that one of them opens it, one of them has to be a capital,
/// and rewriting five constants to be capitals would break the four other
/// places they are quoted mid-sentence.
fn capitalised(clause: &str) -> String {
    let mut characters = clause.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + characters.as_str()
    })
}

/// **What listening actually learned about this collection** — the reading the
/// door shows once a library has been heard.
///
/// Everything here is **measurement**, deliberately: tempo in BPM, the
/// loudest and quietest records by name, how many songs have never been
/// played. Nothing is inference, so nothing on it can be quietly wrong in the
/// way `docs/design/23-the-three-dimensions.md` describes the semantic half
/// as being.
///
/// Its job is **audit, not admiration** — *here is what I heard, check me* —
/// which is why the useful items are named records rather than summaries. If
/// baz calls an ambient piece the loudest thing in a library, its owner knows
/// in one second that something is wrong; an aggregate cannot be graded that
/// way.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Profile {
    /// How many tracks the reading is over.
    pub(crate) heard: usize,
    /// The 5th and 95th percentile tempo, in beats per minute — the one
    /// measurement with a unit a listener already owns.
    pub(crate) tempo_range: Option<(u32, u32)>,
    /// The median tempo, likewise.
    pub(crate) tempo_median: Option<u32>,
    /// **Named records at the extremes**: the label a listener can grade, one
    /// per axis, as `(what it is an extreme of, title, artist)`.
    pub(crate) extremes: Vec<(&'static str, String, String)>,
    /// **Axes this collection barely varies on**, where drawing a line will
    /// not do much — because a rank axis spreads whatever it is given across
    /// the whole scale and would otherwise look equally responsive.
    pub(crate) flat_axes: Vec<Dimension>,
    /// **An example phrase built from music they own**, for the field's
    /// placeholder — see [`State::rebuild_example`]. Survives a
    /// [`State::rebuild_profile`], because it comes from the library rather
    /// than from the analysis and is known long before anything is heard.
    pub(crate) example: Option<String>,
}

/// **The library's own commonest genre**, phrased as a request.
///
/// Separated from [`State`] so it can be tested without one, and because the
/// judgement in it is all in the details: a tag holding several genres is
/// read as its first, casing is the listener's own lowered (a placeholder is
/// not a title), and a tag too long to read as an example is declined rather
/// than truncated into nonsense. `None` where the library has no genre tags
/// at all, and the caller keeps its constant.
fn library_example(albums: &[AlbumVm]) -> Option<String> {
    /// Beyond this a "genre" is somebody's freeform note and would read as
    /// gibberish in a field that is meant to be teaching by example.
    const READABLE: usize = 24;
    let mut counts: HashMap<String, usize> = HashMap::new();
    for album in albums {
        let Some(genre) = album.genre.as_deref() else {
            continue;
        };
        // A multi-valued tag — `Rock; Alternative`, `Jazz / Funk` — is read
        // as its first value, which is the one the tagger led with.
        let word = genre
            .split([';', '/', ','])
            .next()
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if word.is_empty() || word.chars().count() > READABLE {
            continue;
        }
        *counts.entry(word).or_default() += 1;
    }
    // Ties break on the word itself rather than on hash order, so the same
    // library always offers the same example.
    let (word, _) = counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))?;
    Some(format!("warm {word}, slow and sparse"))
}

/// **What one level says about a song on one axis**, or `None` where it sits
/// too near the middle to say anything.
///
/// The threshold is the point of it. A song in the middle of the collection's
/// range on brightness is not *dark* and not *bright*, and calling it either
/// would be the interface inventing a fact — so it says nothing about that
/// axis and the reading is shorter. Which is also why a row's words are worth
/// reading: they are only ever the things that are actually true of it.
pub(crate) fn axis_reading(dimension: Dimension, level: f32) -> Option<&'static str> {
    /// How far from the middle a song must sit before the axis is worth
    /// naming: rather more than a third of the way to an end.
    const NOTABLE: f32 = 0.7;
    let (low, high) = dimension.plain_ends();
    if level >= NOTABLE {
        Some(high)
    } else if level <= -NOTABLE {
        Some(low)
    } else {
        None
    }
}

/// **One line's shape as a single verb**, for a sentence that has to name
/// several at once. [`shape_words`] is the long form for one line.
pub(crate) fn shape_verb(points: &[ContourPoint]) -> &'static str {
    let Some(opening) = level_at(points, 0.0) else {
        return "is unconstrained";
    };
    let landing = level_at(points, 1.0).unwrap_or(opening);
    let peak = (0_u8..=10)
        .filter_map(|step| level_at(points, f32::from(step) / 10.0))
        .fold(f32::MIN, f32::max);
    let rise = landing - opening;
    if points.len() > 2 && peak > opening.max(landing) + 0.4 {
        "peaks partway"
    } else if rise > 0.6 {
        "climbs"
    } else if rise < -0.6 {
        "winds down"
    } else if points.len() > 2 {
        "turns and returns"
    } else {
        "holds level"
    }
}

/// **A length, in the words a listener uses** — design 21 §9: never *tracks*
/// as a unit of length, and never a bare number of minutes on a control that
/// is about how long you are going to be listening for.
pub(crate) const fn spoken(length: MixLength) -> &'static str {
    match length {
        MixLength::HalfHour => "half an hour",
        MixLength::Hour => "an hour",
        MixLength::NinetyMinutes => "an hour and a half",
        MixLength::TwoHours => "two hours",
    }
}

/// **How long listening to this many tracks will take**, from the measured
/// rate rather than from a guess.
///
/// 4 490 tracks an hour at the shipping four workers, measured on a real
/// 5 076-track library across a network mount —
/// `docs/design/impl/vibe-memory/`. Every duration this page states comes
/// through here, so there is exactly one place to correct when the number is
/// re-measured, and no copy can drift away from it.
pub(crate) fn listening_estimate(tracks: usize) -> String {
    /// Tracks an hour, four workers. See `docs/design/impl/vibe-memory/`.
    const PER_HOUR: usize = 4_490;
    if tracks == 0 {
        return "no time at all".to_owned();
    }
    let minutes = (tracks * 60).div_ceil(PER_HOUR).max(1);
    match minutes {
        0..=3 => "a minute or two".to_owned(),
        // Rounded to five minutes: the per-track spread is four-fold, so a
        // figure to the minute would be a precision the measurement does not
        // have.
        4..=75 => format!("{} minutes", minutes.div_ceil(5) * 5),
        // …and to the half hour above that, rounded rather than always up:
        // 126 minutes is "2 hours", not "2 and a half".
        _ => match ((minutes + 15) / 30, (minutes + 15) / 30 % 2) {
            (halves, 0) => format!("{} hours", halves / 2),
            (halves, _) => format!("{} and a half hours", halves / 2),
        },
    }
}

/// Listening-time targets offered beside the ordinary-language request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MixLength {
    HalfHour,
    #[default]
    Hour,
    NinetyMinutes,
    TwoHours,
}

impl MixLength {
    pub(crate) const ALL: [Self; 4] = [
        Self::HalfHour,
        Self::Hour,
        Self::NinetyMinutes,
        Self::TwoHours,
    ];

    pub(crate) const fn minutes(self) -> u64 {
        match self {
            Self::HalfHour => 30,
            Self::Hour => 60,
            Self::NinetyMinutes => 90,
            Self::TwoHours => 120,
        }
    }
}

impl std::fmt::Display for MixLength {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} minutes", self.minutes())
    }
}

/// **How well one chosen track answered the words.** Mirrors
/// `baz_vibe::Match`, converted at the one gated call site.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Match {
    pub(crate) similarity: f32,
    /// One, two or three ticks. Never a colour — the standing rule.
    pub(crate) ticks: u8,
}

/// **What changed since the last compose, and why** — design 21 §6's fourth
/// readout, the cheapest thing in that document and the most valuable.
///
/// One use teaches the whole model: which songs are eligible is the words'
/// doing, where they go is the line's, and nothing else is moving in the dark.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Diff {
    /// Tracks in both the old list and the new.
    pub(crate) kept: usize,
    /// Tracks the new list has and the old did not.
    pub(crate) fresh: usize,
    /// How long the old list was.
    pub(crate) previous: usize,
    /// The sentence naming the cause, already assembled.
    pub(crate) cause: String,
}

/// The preview before it becomes a normal playlist file.
#[derive(Debug, Clone, Default)]
pub(crate) struct Generated {
    pub(crate) description: String,
    pub(crate) request: String,
    pub(crate) items: Vec<QueueItemVm>,
    /// Where each chosen track landed, **one row per drawn line** in lane
    /// order, each holding a level per track in listening order — the result
    /// in the request's own units, so each line can draw what it got over
    /// what it asked for.
    pub(crate) levels: Vec<Vec<f32>>,
    /// Those lines collapsed by their weights — where each track's dot sits on
    /// the one blended line.
    pub(crate) blended: Vec<f32>,
    /// **The eligible songs on the blended axis**, bucketed: design 21 §6's
    /// cloud, which is the eligible set rather than the library and is the
    /// clearest picture of cause and effect in the feature. Bucketed here
    /// rather than in the view, so the picture behind the line is a reading of
    /// what selection did and not a second opinion about it.
    pub(crate) cloud: Vec<f32>,
    /// The same, per drawn line, for when the expander is open.
    pub(crate) lane_clouds: Vec<Vec<f32>>,
    /// How well each chosen track answered the words, in listening order.
    /// Empty when there were no words: a shape-only request has no match
    /// strength, and drawing one would be an invention.
    pub(crate) matches: Vec<Match>,
    /// Every track in the library's selected editions.
    pub(crate) pool_tracks: usize,
    /// Every track baz has heard.
    pub(crate) analyzed_tracks: usize,
    /// **How many of them the words let in.**
    pub(crate) eligible_tracks: usize,
    pub(crate) tempo_span: Option<(f32, f32)>,
    pub(crate) target_minutes: u64,
    /// **How many positions were asked for** — so a request the library
    /// cannot fill can say so in numbers rather than quietly returning a short
    /// list. Nothing is ever padded to reach it.
    pub(crate) asked_positions: usize,
    /// The shape this was composed with, kept so the next compose can say
    /// whether the line moved.
    pub(crate) contour: Contour,
    /// What changed against the compose before it, where there was one.
    pub(crate) diff: Option<Diff>,
}

impl Generated {
    #[must_use]
    pub(crate) fn pool_note(&self) -> String {
        let coverage = format!(
            "{} of {} library tracks analysed",
            self.analyzed_tracks, self.pool_tracks
        );
        self.tempo_span.map_or(coverage.clone(), |(low, high)| {
            format!("{coverage} · selected tempo {low:.0}–{high:.0} BPM")
        })
    }

    #[must_use]
    pub(crate) fn duration_note(&self) -> String {
        let known: Vec<_> = self.items.iter().filter_map(|item| item.duration).collect();
        let unknown = self.items.len().saturating_sub(known.len());
        let known_total: std::time::Duration = known.iter().copied().sum();
        let average = if known.is_empty() {
            std::time::Duration::from_secs(4 * 60)
        } else {
            known_total / u32::try_from(known.len()).unwrap_or(u32::MAX)
        };
        let estimated = known_total + average * u32::try_from(unknown).unwrap_or(u32::MAX);
        let prefix = if unknown > 0 { "about " } else { "" };
        let actual = format!("{prefix}{}", crate::vm::format_duration(estimated));
        let target = std::time::Duration::from_secs(self.target_minutes * 60);
        if unknown == 0 && estimated < target {
            format!(
                "{actual} of requested {}",
                crate::vm::format_duration(target)
            )
        } else {
            actual
        }
    }
}

/// Result of checking the persistent cache away from the UI thread.
#[derive(Debug, Clone)]
pub(crate) struct Preparation {
    ready: HashMap<PathBuf, SonicFeatures>,
    pending: Vec<PathBuf>,
}

/// Result of one bounded worker task. `run` makes a late completion from a
/// cancelled pass harmless.
#[derive(Debug, Clone)]
pub(crate) struct AnalysisResult {
    pub(crate) run: u64,
    path: PathBuf,
    features: Result<SonicFeatures, String>,
}

/// Home's transient controller plus the session copy of the persistent index.
#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "open/awaiting/preparing/analyzing are independent visible facts: consent, requested generation, cache inspection and worker scheduling"
)]
pub(crate) struct State {
    pub(crate) open: bool,
    pub(crate) prompt: String,
    pub(crate) length: MixLength,
    pub(crate) awaiting_create: bool,
    pub(crate) preparing: bool,
    pub(crate) analyzing: bool,
    pub(crate) total: usize,
    pub(crate) done: usize,
    pub(crate) failed: usize,
    pub(crate) current: Option<PathBuf>,
    pub(crate) error: Option<String>,
    pub(crate) preview: Option<Generated>,
    /// **The shape the next list is asked to follow**, in the same
    /// collection-relative units `baz_vibe` scores against. Empty means the
    /// words alone decide, which is a first-class choice rather than a
    /// missing one ([`Shape::ANY`]).
    pub(crate) contour: Contour,
    /// How much of the analysed library sits at each height of a dimension's
    /// axis, lowest bucket first, normalised against the fullest — what each
    /// line draws behind itself, so a request the collection cannot fill is
    /// visible before it is spent. One entry per dimension that has ever been
    /// drawn.
    field: HashMap<Dimension, Vec<f32>>,
    /// **Which row of the preview the pointer is on**, so the contour can
    /// light that track's own dot. Session state about a pointer and nothing
    /// else: it is cleared by leaving the row, and no decision reads it.
    pub(crate) hovered_row: Option<usize>,
    /// **Which row of the result is selected**, so it can explain itself:
    /// design 21 §7 state 6's why-line, and the dot, tick and position that
    /// go with it.
    pub(crate) selected_row: Option<usize>,
    /// **What the words match right now**, before anything is composed —
    /// design 21 §6's live count and this plan's closest-three beside it.
    /// `None` until the first phrase settles.
    pub(crate) live: Option<Live>,
    /// Whether a live count is in flight, so the page can say *counting…*
    /// rather than show a stale number as though it were current.
    pub(crate) counting: bool,
    /// Whether *another version* is what produced the compose being run, so
    /// the diff can name the right cause.
    varied: bool,
    /// **What listening learned about this collection** — the door's reading,
    /// rebuilt whenever a scan settles.
    pub(crate) profile: Profile,
    /// When the words have been still long enough to be worth embedding.
    /// `None` means there is nothing waiting.
    count_due: Option<std::time::Instant>,
    /// **Whether the page is standing at its door** — the moods, rather than
    /// the form behind them.
    ///
    /// The owner: *"can we have maybe 5-6 presets at that level as tiles
    /// where when a user selects it, it creates a new one."* So the smart
    /// door lands here; pressing a tile composes, and the form appears behind
    /// the result where it is useful. It is not a wizard step — there is no
    /// *next* — it is the difference between being asked to fill something in
    /// and being offered six things to press.
    pub(crate) choosing: bool,
    /// **Which of the two depths the page is showing.**
    ///
    /// The owner: *"we should have a simple and advanced mode I think."* The
    /// split is not a feature gate — every control in advanced was in the
    /// page before it existed — it is an answer to the complaint beside it,
    /// that *"there are a ton of options which are just query builders"*.
    ///
    /// **Simple** is the four things a listener needs to get a playlist: what
    /// they want to hear, a mood to start from, how long, and the press. It
    /// states the query it has built and shows the list.
    ///
    /// **Advanced** adds the query builder proper — the vocabulary, the drawn
    /// line and its per-dimension curves, and the readouts that explain what
    /// the engine did.
    /// **Whether the request is narrowed by words at all** — `All songs`
    /// against `Matching songs`.
    ///
    /// Not a view flag: [`Self::effective_request`] reads it, so `All songs`
    /// genuinely means all of them. The phrase is kept while it is off, so
    /// changing your mind twice costs nothing.
    ///
    /// They are the optional half and the untrustworthy half, so they are
    /// folded away until asked for — which is what buys the line, the length
    /// and the press room to sit above the fold together. A request that
    /// arrived with words already in it opens itself: hiding what somebody
    /// just chose would be a worse economy than the one it bought.
    pub(crate) words_open: bool,
    /// **Whether the per-dimension lines are open.** Kept rather than derived
    /// from whether the curves differ, because *open and identical* is a real
    /// state: it is what the expander shows the moment it is pressed, and it
    /// is the whole of design 21 §5's claim that the lines were already the
    /// blend.
    /// **Which of the five lines is being edited**, or `None` for all of them
    /// at once.
    ///
    /// The owner: *"I like the idea of all lines being on the same graph and
    /// a way to kinda toggle between all and individual… then selecting each
    /// individually to be able to configure that line."* So there is one
    /// canvas and a row of tabs, rather than one canvas per dimension stacked
    /// down the page. `None` is the tab that says *all five*: every line
    /// holds the shape and a drag moves them together.
    pub(crate) shown: Option<usize>,
    /// Whether the listener has set the shape themselves. A mood sets the
    /// shape only until this is true.
    shape_touched: bool,
    /// The same, for the length.
    length_touched: bool,
    features: HashMap<PathBuf, SonicFeatures>,
    pending: VecDeque<PathBuf>,
    run: u64,
    variation: u64,
    active_workers: usize,
}

/// **What the words match, live** — one debounced text embedding against
/// vectors already in memory.
///
/// The count is design 21 §6's first readout: *"matches 340 songs of the 9 412
/// baz has heard"*. The three titles beneath it are this plan's one addition
/// to that section, and they earn their place because **a count says how many
/// and never how well**: type *slow sparse piano*, see a death-metal track
/// first, and you know before spending a compose.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Live {
    /// The phrase this describes. Held so a settled count that arrives after
    /// the words moved on can be discarded rather than shown as current.
    pub(crate) prompt: String,
    /// How many songs the words let in.
    pub(crate) eligible: usize,
    /// How many songs baz has heard at all.
    pub(crate) analysed: usize,
    /// The three best matches, nearest first, as *title — artist*.
    pub(crate) closest: Vec<String>,
    /// **The eligible songs' own distribution** on the blended axis, bucketed
    /// — design 21 §6's cloud, drawn behind the line and live.
    ///
    /// Live rather than left over from the last compose, because the sentence
    /// it has to earn is *"narrow the phrase and watch the cloud thin out
    /// under your curve"*, and a picture of the previous request cannot say
    /// that. It is affordable because the expensive half is already done: the
    /// count has ranked the library, and ranking a few hundred survivors on
    /// five axes costs nothing beside it.
    pub(crate) cloud: Vec<f32>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            prompt: String::new(),
            length: MixLength::Hour,
            awaiting_create: false,
            preparing: false,
            analyzing: false,
            total: 0,
            done: 0,
            failed: 0,
            current: None,
            error: None,
            preview: None,
            contour: Contour::opening(&Shape::DEFAULT.points()),
            field: HashMap::new(),
            hovered_row: None,
            selected_row: None,
            live: None,
            counting: false,
            varied: false,
            profile: Profile::default(),
            count_due: None,
            choosing: false,
            words_open: false,
            shown: None,
            shape_touched: false,
            length_touched: false,
            features: HashMap::new(),
            pending: VecDeque::new(),
            run: 0,
            variation: 0,
            active_workers: 0,
        }
    }
}

impl State {
    /// The library's own distribution over one dimension, for the line that
    /// draws it. Empty until something has been analysed.
    pub(crate) fn field_of(&self, dimension: Dimension) -> &[f32] {
        self.field.get(&dimension).map_or(&[], Vec::as_slice)
    }

    /// **How many tracks Baz has actually heard.**
    ///
    /// Deleted once as dead code and wanted again the moment the door had to
    /// say *heard 1 240 of your 5 076* — which is the state that decides
    /// whether it still offers to listen.
    pub(crate) fn analysed(&self) -> usize {
        self.features.len()
    }

    /// The pointer entered or left one row of the preview.
    pub(crate) fn hover_row(&mut self, row: Option<usize>) {
        self.hovered_row = row;
    }

    /// **A row explains itself**, or stops. Selecting the selected row puts
    /// the explanation away, which is the only way back out of it.
    pub(crate) fn select_row(&mut self, row: usize) {
        self.selected_row = if self.selected_row == Some(row) {
            None
        } else {
            Some(row)
        };
    }

    /// **The whole request, in one readable line** — what all these controls
    /// are actually building.
    ///
    /// The owner: *"the fact that there are a ton of options which are just
    /// query builders… seems like we should make that more clear."* They are,
    /// and the page had no single place that said so: the words were in one
    /// band, the shape in another, the length on the commitment, and nothing
    /// anywhere stated the query they add up to.
    ///
    /// This is that statement. It is assembled from the controls rather than
    /// stored, so it cannot drift from them, and it is the same three clauses
    /// design 21 §3's table names: **where** each song goes, **how many**
    /// there are, and **which** songs they are drawn from.
    ///
    /// **The shape leads the sentence** (design note 25). It used to open
    /// with the words — *songs like “warm brass”, starting quiet and climbing
    /// the whole way, for about an hour* — which put the half that is
    /// sometimes no better than chance at the head of the one line stating
    /// what the page is doing. The clauses are the same; the order now says
    /// which of them the request is actually built on.
    pub(crate) fn query(&self) -> String {
        let shape = if self.contour.lanes.is_empty() {
            "In no particular shape".to_owned()
        } else if !self.contour.is_one_line() {
            // **Name what each line asks for**, rather than counting them.
            // The owner: *"the 'shape each thing' bit isn't clear how it
            // influences things."* Saying *shaped separately across 5 of the
            // things Baz listens for* described the control; this describes
            // the request, which is what the listener is trying to read.
            let each: Vec<String> = self
                .contour
                .lanes
                .iter()
                .map(|lane| {
                    format!(
                        "{} {}",
                        lane.dimension.label().to_lowercase(),
                        shape_verb(&lane.points)
                    )
                })
                .collect();
            capitalised(&each.join(", "))
        } else {
            capitalised(shape_words(&self.contour))
        };
        let words = self.effective_request();
        let drawn_from = if words.is_empty() {
            "any song Baz has heard".to_owned()
        } else {
            format!("songs like “{words}”")
        };
        format!(
            "{shape}, for about {}, drawn from {drawn_from}.",
            spoken(self.length)
        )
    }

    /// **What the song at `row` actually is**, in the axis words — *loud,
    /// fast, swinging*.
    ///
    /// This is the feature proving itself. A listener who draws a rising line
    /// and then reads the list from top to bottom should watch these words
    /// travel from *quiet, slow, clean* to *loud, fast, noisy*, and needs to
    /// understand nothing about embeddings to see that it worked. It is read
    /// straight off the levels the engine returned for each drawn line, so it
    /// is the result speaking rather than a second opinion about it.
    pub(crate) fn row_is(&self, row: usize) -> Vec<&'static str> {
        self.readings(row)
            .into_iter()
            .map(|(_, word)| word)
            .collect()
    }

    /// **The same reading, cut to the `most` strongest axes**, in axis order.
    ///
    /// A row's own lane has room for three words, and three is also as many
    /// as anybody reads at a glance while scrolling. The ones kept are the
    /// axes the song is *furthest* from the middle on — the things most worth
    /// saying about it — and they stay in [`Dimension::ALL`] order so the
    /// column reads down consistently rather than reshuffling per row.
    pub(crate) fn row_is_briefly(&self, row: usize, most: usize) -> Vec<&'static str> {
        let mut readings = self.readings(row);
        readings.sort_by(|left, right| right.0.abs().total_cmp(&left.0.abs()));
        readings.truncate(most);
        let order: Vec<&'static str> = self.readings(row).into_iter().map(|(_, w)| w).collect();
        let kept: Vec<&'static str> = readings.into_iter().map(|(_, word)| word).collect();
        order
            .into_iter()
            .filter(|word| kept.contains(word))
            .collect()
    }

    /// Every axis this song is notable on, as `(level, word)`.
    fn readings(&self, row: usize) -> Vec<(f32, &'static str)> {
        let Some(preview) = self.preview.as_ref() else {
            return Vec::new();
        };
        self.contour
            .lanes
            .iter()
            .enumerate()
            .filter_map(|(lane, held)| {
                let level = preview.levels.get(lane)?.get(row).copied()?;
                Some((level, axis_reading(held.dimension, level)?))
            })
            .collect()
    }

    /// **What the line asked for at `row`'s position**, in the same words.
    ///
    /// The other half of the proof: *asked for loud, fast and swinging* beside
    /// *this song is loud, fast and steady* is a claim anybody can check, and
    /// it says plainly where the collection could not answer.
    pub(crate) fn row_asked(&self, row: usize) -> Vec<&'static str> {
        let Some(preview) = self.preview.as_ref() else {
            return Vec::new();
        };
        let last = preview.items.len().saturating_sub(1).max(1);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a playlist is bounded at PLAYLIST_CAP"
        )]
        let at = row as f32 / last as f32;
        self.contour
            .lanes
            .iter()
            .filter_map(|lane| {
                let level = level_at(&lane.points, at)?;
                axis_reading(lane.dimension, level)
            })
            .collect()
    }

    /// **Why this song is here**, in the two halves design 21 §3 promises an
    /// answer in: the words let it in, and the line put it where it is.
    ///
    /// Every number in it is a fact the engine returned — the match strength,
    /// the level, the pool size — rather than a second opinion computed here.
    /// A rank, never a score: the quorum's R9, and the reason the sentence
    /// says *louder than 78% of this request's songs* instead of *0.63*.
    pub(crate) fn why(&self, row: usize) -> Option<String> {
        let preview = self.preview.as_ref()?;
        if row >= preview.items.len() {
            return None;
        }
        let list = |words: &[&str]| match words {
            [] => "nothing in particular".to_owned(),
            [one] => (*one).to_owned(),
            [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        };
        let asked = self.row_asked(row);
        let is = self.row_is(row);
        let placed = format!("{} of {}", row + 1, preview.items.len());

        // **Asked against got, in the same words** — the claim a listener can
        // check by ear. Where they differ the collection could not answer,
        // and saying so is worth more than a number: it is the difference
        // between *this is what you asked for* and *this is the nearest your
        // music has*.
        let shape = if self.contour.lanes.is_empty() {
            format!("You drew no line, so nothing asked for a particular sound at {placed}.")
        } else if asked == is {
            format!(
                "At {placed} your line asked for {} — and this song is.",
                list(&asked)
            )
        } else {
            format!(
                "At {placed} your line asked for {}. This song is {}.",
                list(&asked),
                list(&is)
            )
        };
        let Some(found) = preview.matches.get(row) else {
            return Some(format!(
                "You asked for no words, so every song Baz has heard was eligible. {shape}"
            ));
        };
        let strength = match found.ticks {
            3 => "one of the strongest matches",
            2 => "a fair match",
            _ => "a weak match — the line asked for something your words did not have much of",
        };
        Some(format!(
            "Your words let it in: {strength} of the {} eligible. {shape}",
            preview.eligible_tracks
        ))
    }

    /// **Start from a mood**: its words always, and its shape and its length
    /// **only while the listener has not set them themselves**.
    ///
    /// Design 21 §4 bounds the effect that used to be silent. Drag a point
    /// once and the shape is yours; from then on a mood changes the words and
    /// nothing else. Invisible when it is right, and the alternative — a mood
    /// that throws away a line somebody drew — is the kind of thing that
    /// teaches people not to press anything.
    pub(crate) fn start_from(&mut self, recipe: Recipe) {
        self.choosing = false;
        self.set_prompt(recipe.prompt);
        if !self.shape_touched {
            let touched = self.shape_touched;
            self.set_shape(recipe.shape());
            self.shape_touched = touched;
        }
        if !self.length_touched {
            self.length = recipe.length;
        }
    }

    /// Enter by the smart playlist's own door: stand at the moods.
    pub(crate) fn begin_choosing(&mut self) {
        self.choosing = true;
        self.open = true;
    }

    /// **Show one line, or put every line back on one shape.**
    ///
    /// The owner, three times, each time more plainly: *"if I've edited any
    /// of the individual lines and then go back to all five, it does not snap
    /// the previously edited lines to the 'all five' line."*
    ///
    /// It does now, and the reason it did not is that this was built as a
    /// **view** — press a tab, change what you can drag, change nothing about
    /// the request. That is a defensible thing to build and it is not what
    /// *all five* means to somebody using it. **All five is one shape**, and
    /// choosing it is choosing to have one, so every line returns to it.
    ///
    /// Which shape? The one the tab was already drawing — the first line's,
    /// which is what a listener last drew there. Lossy by construction, and
    /// deliberately so: the alternative is what he kept finding, where a tab
    /// called *all five* quietly presided over five different shapes. The
    /// chip says what it will do before it is pressed.
    pub(crate) fn show_line(&mut self, lane: Option<usize>) {
        self.shown = lane.filter(|lane| *lane < self.contour.lanes.len());
        if self.shown.is_none() {
            self.gather_lines();
        }
    }

    /// **Put every line back on the first one's curve.** Private to
    /// [`Self::show_line`]: bringing the lines together is what choosing
    /// *all five together* means, and there is no second control for it.
    fn gather_lines(&mut self) {
        if self.contour.is_one_line() {
            return;
        }
        let Some(points) = self.contour.lane(0).map(|lane| lane.points.clone()) else {
            return;
        };
        for lane in &mut self.contour.lanes {
            lane.points.clone_from(&points);
        }
        self.shape_touched = true;
    }

    /// **Load a named shape onto every line.** A shape is a shape: asking for
    /// `Peak and fall` with tempo and brightness drawn means both of them
    /// peak and fall, which is what the picture then shows. Lines are shaped
    /// apart by dragging them apart.
    pub(crate) fn set_shape(&mut self, shape: Shape) {
        self.shape_touched = true;
        let points = shape.points();
        if points.is_empty() {
            self.contour.lanes.clear();
            return;
        }
        // **A preset arrives at the working resolution, not its own.** The
        // shapes are written as two or three points because that is the
        // fewest that states the arc; handing those straight to the listener
        // would take away eight of the ten handles they had a moment ago.
        // Sampling the arc keeps it and only changes the grip.
        let points = resampled(&points, Contour::DEFAULT_POINTS);
        if self.contour.lanes.is_empty() {
            self.contour = Contour::blended(&points);
            return;
        }
        for lane in &mut self.contour.lanes {
            lane.points.clone_from(&points);
        }
    }

    /// Move one point of one line, by the widget's raw geometry.
    ///
    /// **A drag is what makes the shape the listener's.** From here on a mood
    /// press changes the words and leaves the line alone.
    pub(crate) fn drag_contour(&mut self, lane: usize, index: usize, at: f32, level: f32) {
        self.shape_touched = true;
        if let Some(shown) = self.shown {
            self.contour.drag(shown, index, at, level);
            return;
        }
        // **All five, by the same amount.** On the *all five* tab the handles
        // belong to every line at once, so a drag moves each of them by what
        // the pointer moved rather than setting them all to where it landed.
        //
        // The difference only shows once the lines have been pulled apart —
        // which is exactly when setting them all to one place would silently
        // throw that work away. While they sit together, which is nearly
        // always, this is the same gesture it has always been.
        let Some(from) = self
            .contour
            .lane(lane)
            .and_then(|held| held.points.get(index).copied())
        else {
            return;
        };
        let (moved_at, moved_level) = (at - from.at, level - from.level);
        for held in 0..self.contour.lanes.len() {
            let Some(point) = self
                .contour
                .lane(held)
                .and_then(|line| line.points.get(index).copied())
            else {
                continue;
            };
            self.contour
                .drag(held, index, point.at + moved_at, point.level + moved_level);
        }
    }

    /// **An example made of music they own.**
    ///
    /// The field's placeholder was `warm hypnotic music for driving at night`
    /// — a good sentence about nobody's library. Design note 24 §7: *no
    /// generic content anywhere; every example drawn from what the listener
    /// actually owns*, and this is the cheapest instance of it. The frame is
    /// fixed and only the noun is theirs, which is the shape that note argues
    /// for: a surface that differs per library is one nobody can screenshot
    /// or support, so the rows stay put and their contents are the
    /// listener's.
    ///
    /// It teaches two things at once — that a genre word is a fine start, and
    /// that qualities are what sharpen it — and it does so with a word the
    /// library can actually answer.
    ///
    /// Costs one pass over the albums, on the schedule the shelves are
    /// rebuilt on. Needs no analysis, so it is right in the light build and
    /// right before a single track has been heard.
    pub(crate) fn rebuild_example(&mut self, albums: &[AlbumVm]) {
        self.profile.example = library_example(albums);
    }

    /// **Read what listening learned**, once, when a scan settles.
    ///
    /// Pure measurement over features already in memory, so it costs a pass
    /// over a few thousand floats and needs no model, no network and no
    /// second analysis. Recomputed rather than accumulated for the same
    /// reason the field is: every arrival can move the collection's extremes.
    #[cfg(feature = "vibe-analysis")]
    pub(crate) fn rebuild_profile(
        &mut self,
        albums: &[AlbumVm],
        chosen: &HashMap<u64, EditionKey>,
    ) {
        if self.features.is_empty() {
            self.profile = Profile {
                example: self.profile.example.take(),
                ..Profile::default()
            };
            return;
        }
        let named = |wanted: &Path| -> Option<(String, String)> {
            albums.iter().find_map(|album| {
                let edition = vm::selected_edition(album, chosen.get(&album.id).copied())?;
                let track = edition.tracks.iter().find(|track| track.path == wanted)?;
                Some((track.title.clone(), album.artist.label().to_owned()))
            })
        };
        let mut profile = Profile {
            heard: self.features.len(),
            // Not measured here and not lost here: it is about the library,
            // not about the analysis.
            example: self.profile.example.clone(),
            ..Profile::default()
        };

        // Tempo, in the one unit a listener already has.
        let mut tempos: Vec<f32> = self
            .features
            .values()
            .map(baz_vibe::Features::tempo_bpm)
            .collect();
        tempos.sort_by(f32::total_cmp);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a bounded quantile of a library count, into whole BPM"
        )]
        let at = |fraction: f32| {
            // Rounded rather than truncated: truncation is lopsided on a
            // short list — over three tracks the 95th percentile would land
            // on the middle one while the 5th landed on the first — and this
            // reading is a sentence about the ends, so it should reach them.
            let index = ((tempos.len() - 1) as f32).mul_add(fraction, 0.5) as usize;
            tempos[index.min(tempos.len() - 1)].round().max(0.0) as u32
        };
        profile.tempo_range = Some((at(0.05), at(0.95)));
        profile.tempo_median = Some(at(0.50));

        // **The named extremes**, which are the part a listener can grade.
        for (dimension, low_word, high_word) in [
            (Dimension::Energy, "Quietest", "Loudest"),
            (Dimension::Tempo, "Slowest", "Fastest"),
        ] {
            let engine = engine_dimension(dimension);
            let mut ranked: Vec<(f32, &PathBuf)> = self
                .features
                .iter()
                .map(|(path, features)| (features.value(engine), path))
                .collect();
            ranked.sort_by(|left, right| left.0.total_cmp(&right.0));
            // **A step in from each end.** The owner, on the shipped block:
            // *"the 'what baz heard' classified Day & Night by thundercat as
            // the fastest… it really isn't."* He was right, and the cause is
            // that a single reading got a sentence to itself: the five
            // fastest tracks in his library are led by a Renaissance madrigal
            // and a solo piano miniature at 190 BPM, which are **octave
            // errors** — the standard failure of beat tracking, and exactly
            // the thing that collects at the top of an argmax.
            //
            // The tempo *range* two lines below has always been p05–p95 for
            // this reason. Naming the ends with an argmin and an argmax was
            // the one place in the same block where one bad reading could
            // describe a whole library, which is an inconsistency rather than
            // a judgement.
            let step = ranked.len() / EXTREME_MARGIN;
            let ends = [
                (low_word, ranked.get(step)),
                (high_word, ranked.get(ranked.len().saturating_sub(step + 1))),
            ];
            for (word, end) in ends {
                if let Some((_, path)) = end
                    && let Some((title, artist)) = named(path)
                {
                    profile.extremes.push((word, title, artist));
                }
            }
        }

        // **Axes this collection has nothing to say on.** A rank axis spreads
        // whatever it is given across the whole scale, so a line over a
        // dimension with no real variation tracks perfectly while the music
        // does not change — and nothing else on screen would say so.
        for dimension in Dimension::ALL {
            let engine = engine_dimension(dimension);
            let mut values: Vec<f32> = self
                .features
                .values()
                .map(|features| features.value(engine))
                .collect();
            values.sort_by(f32::total_cmp);
            let span = values[values.len() * 95 / 100] - values[values.len() * 5 / 100];
            if span < FLAT_AXIS {
                profile.flat_axes.push(dimension);
            }
        }
        self.profile = profile;
    }

    /// The light build has no analyser and so nothing to read.
    #[cfg(not(feature = "vibe-analysis"))]
    pub(crate) fn rebuild_profile(
        &mut self,
        _albums: &[AlbumVm],
        _chosen: &HashMap<u64, EditionKey>,
    ) {
    }

    /// **Rebuild the library's own distribution** behind the line, from
    /// whatever has been analysed so far.
    ///
    /// It is derived rather than kept: analysis lands a track at a time, and
    /// each arrival can move the collection's extremes — which moves every
    /// other track's place on a collection-relative axis. Recomputing on
    /// arrival is a pass over a few thousand floats and keeps the picture
    /// honest at every moment of a scan.
    #[cfg(feature = "vibe-analysis")]
    pub(crate) fn rebuild_field(&mut self) {
        for dimension in Dimension::ALL {
            let engine = engine_dimension(dimension);
            let values: Vec<f32> = self
                .features
                .values()
                .map(|features| features.value(engine))
                .collect();
            let mut sorted = values.clone();
            sorted.sort_by(f32::total_cmp);
            let field = field_of(values.iter().map(|value| rank_level(&sorted, *value)));
            if field.is_empty() {
                self.field.remove(&dimension);
            } else {
                self.field.insert(dimension, field);
            }
        }
    }

    /// The light build has no analyser, so it has no collection to draw.
    #[cfg(not(feature = "vibe-analysis"))]
    pub(crate) fn rebuild_field(&mut self) {}

    pub(crate) fn begin_request(&mut self) {
        self.open = true;
        self.awaiting_create = true;
        self.error = None;
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
        self.awaiting_create = false;
        self.error = None;
        self.preview = None;
    }

    /// **Leaving the page puts the page away.**
    ///
    /// The owner: *"there are issues when navigating away, the page state is
    /// not cleaned up."* All of this state is *about the page* — a result on
    /// screen, a row explaining itself, a count describing a phrase, a
    /// debounce clock ticking — and every bit of it was surviving a
    /// navigation. Coming back landed on somebody else's screen: a list from
    /// a request you had stopped making, a selected row you never selected,
    /// and the door standing open behind a page that had already been used.
    ///
    /// **What is deliberately kept** is the *request*: the words, the shape,
    /// the length and the depth. Those are what you were asking for, and
    /// walking to the Library to check something is not a reason to lose
    /// them — the same rule `cancel_analysis` has always followed, and its
    /// test says so. What goes is everything that was only true while the
    /// page was on screen.
    pub(crate) fn leave_page(&mut self) {
        self.open = false;
        self.awaiting_create = false;
        self.choosing = false;
        self.preview = None;
        self.selected_row = None;
        self.hovered_row = None;
        self.varied = false;
        self.error = None;
        // The debounce clock is the one with a cost attached: it keeps a
        // 120 ms subscription alive, and it was doing that off-page for as
        // long as a phrase stayed unsettled.
        self.count_due = None;
        self.counting = false;
        self.live = None;
    }

    pub(crate) fn start_preparing(&mut self) {
        self.run = self.run.wrapping_add(1);
        self.preparing = true;
        self.analyzing = false;
        self.total = 0;
        self.done = 0;
        self.failed = 0;
        self.current = None;
        self.active_workers = 0;
        self.error = None;
        self.pending.clear();
    }

    pub(crate) fn accept_preparation(&mut self, result: Result<Preparation, String>) {
        self.preparing = false;
        match result {
            Ok(prepared) => {
                self.features = prepared.ready;
                self.rebuild_field();
                self.pending = prepared.pending.into();
                self.total = self.features.len() + self.pending.len();
                self.done = self.features.len();
                self.analyzing = !self.pending.is_empty();
                self.error = None;
                // Same reason: what is eligible has just been established.
                self.words_changed();
            }
            Err(error) => {
                self.error = Some(error);
                self.analyzing = false;
            }
        }
    }

    pub(crate) fn next_jobs(&mut self, limit: usize) -> Vec<(u64, PathBuf)> {
        if !self.analyzing {
            return Vec::new();
        }
        let count = limit.saturating_sub(self.active_workers);
        let mut jobs = Vec::with_capacity(count.min(self.pending.len()));
        for _ in 0..count {
            let Some(path) = self.pending.pop_front() else {
                break;
            };
            self.active_workers += 1;
            self.current = Some(path.clone());
            jobs.push((self.run, path));
        }
        jobs
    }

    pub(crate) fn accept_analysis(&mut self, result: AnalysisResult) {
        if result.run != self.run || !self.analyzing {
            return;
        }
        self.active_workers = self.active_workers.saturating_sub(1);
        self.current = None;
        match result.features {
            Ok(features) => {
                self.features.insert(result.path, features);
                self.rebuild_field();
                self.done = self.done.saturating_add(1);
            }
            Err(error) => {
                self.failed = self.failed.saturating_add(1);
                self.error = Some(error);
            }
        }
        if self.pending.is_empty() && self.active_workers == 0 {
            self.analyzing = false;
            // **The count is against a pool that just changed.** A live
            // readout that describes the library as it was when the phrase
            // settled is worse than no readout: it says *Baz has not heard
            // anything yet* over a library it has now heard. Recount.
            self.words_changed();
        }
    }

    pub(crate) fn cancel_analysis(&mut self) {
        self.run = self.run.wrapping_add(1);
        self.open = false;
        self.preparing = false;
        self.analyzing = false;
        self.current = None;
        self.active_workers = 0;
        self.pending.clear();
        self.error = None;
        self.awaiting_create = false;
    }

    pub(crate) fn set_prompt(&mut self, prompt: &str) {
        let prompt: String = prompt.chars().take(240).collect();
        if prompt.trim() != self.prompt.trim() {
            self.words_changed();
        }
        // Words arriving from anywhere — a mood on the door, a preset — are
        // words somebody asked for, so they turn the switch that uses them.
        if !prompt.trim().is_empty() {
            self.words_open = true;
        }
        self.prompt = prompt;
    }

    /// **All songs, or only the ones the words match.**
    ///
    /// Recounts, because everything on screen describing the eligible set has
    /// just stopped being true of it — the same path the words themselves
    /// take when they change.
    pub(crate) fn set_words(&mut self, open: bool) {
        if open == self.words_open {
            return;
        }
        self.words_open = open;
        self.words_changed();
    }

    /// How long the words must be still before they are worth a text
    /// embedding. Design 21 §6: *debounced ~400 ms after typing stops*.
    const COUNT_SETTLE: std::time::Duration = std::time::Duration::from_millis(400);

    /// The words moved: whatever is on screen describes the old ones.
    fn words_changed(&mut self) {
        self.count_due = Some(std::time::Instant::now() + Self::COUNT_SETTLE);
        self.counting = true;
    }

    /// Whether a count is waiting on the clock, so the shell can run its tick
    /// only while there is something to wait for.
    pub(crate) fn awaiting_count(&self) -> bool {
        self.count_due.is_some()
    }

    /// **Have the words been still long enough?** Returns the phrase to embed
    /// exactly once per settling.
    pub(crate) fn settled_prompt(&mut self) -> Option<String> {
        let due = self.count_due?;
        if std::time::Instant::now() < due {
            return None;
        }
        self.count_due = None;
        let prompt = self.effective_request();
        if prompt.is_empty() {
            // No words is not a narrow request, it is no request: everything
            // baz has heard is eligible, and it can be said without a model.
            self.counting = false;
            self.live = Some(Live {
                prompt,
                eligible: self.features.len(),
                analysed: self.features.len(),
                closest: Vec::new(),
                // With no words the eligible set is the library, and the
                // library's own shape is already kept per dimension.
                cloud: self.field_of(Dimension::Energy).to_vec(),
            });
            return None;
        }
        Some(prompt)
    }

    /// The embedded phrase came back. Count against the vectors already in
    /// memory — a few million multiply-adds, which is why this readout is
    /// affordable at all — and name the three nearest.
    #[cfg(feature = "vibe-analysis")]
    pub(crate) fn accept_embedding(
        &mut self,
        prompt: &str,
        embedding: &Result<Vec<f32>, String>,
        albums: &[AlbumVm],
        chosen: &HashMap<u64, EditionKey>,
    ) {
        if prompt != self.effective_request() {
            // The words moved on while the tower was thinking. A stale count
            // shown as current is worse than no count.
            return;
        }
        self.counting = false;
        let Ok(embedding) = embedding else {
            self.live = None;
            return;
        };
        // Borrowed, never cloned: this runs once per settled phrase and the
        // whole point of the readout is that it costs a scan of vectors that
        // are already resident.
        let mut scored: Vec<(f32, &Path)> = Vec::with_capacity(self.features.len());
        for (path, features) in &self.features {
            scored.push((features.similarity(embedding), path.as_path()));
        }
        scored.sort_by(|left, right| right.0.total_cmp(&left.0));
        let ranked: Vec<f32> = scored.iter().map(|(value, _)| *value).collect();
        let eligible = baz_vibe::eligible_count(&ranked);
        let named = |wanted: &Path| {
            albums.iter().find_map(|album| {
                let edition = vm::selected_edition(album, chosen.get(&album.id).copied())?;
                let track = edition.tracks.iter().find(|track| track.path == wanted)?;
                Some(format!("{} — {}", track.title, album.artist.label()))
            })
        };
        self.live = Some(Live {
            prompt: prompt.to_owned(),
            eligible,
            analysed: scored.len(),
            closest: scored
                .iter()
                .take(3)
                .filter_map(|(_, path)| named(path))
                .collect(),
            cloud: self.cloud_of(scored.iter().take(eligible).map(|(_, path)| *path)),
        });
    }

    /// **The eligible set's own shape on the blended axis**, bucketed for the
    /// picture behind the line.
    ///
    /// Ranked *within the eligible set* rather than within the library, which
    /// is the same pool-relative choice the engine's axes make: the cloud and
    /// the dots have to be measured against the same thing or the picture
    /// lies about where a track landed.
    #[cfg(feature = "vibe-analysis")]
    fn cloud_of<'a>(&self, members: impl Iterator<Item = &'a Path>) -> Vec<f32> {
        let members: Vec<&SonicFeatures> =
            members.filter_map(|path| self.features.get(path)).collect();
        if members.is_empty() {
            return Vec::new();
        }
        let mut blended = vec![0.0_f32; members.len()];
        let total: f32 = Contour::BLEND.iter().sum();
        for (dimension, weight) in Dimension::ALL.into_iter().zip(Contour::BLEND) {
            let engine = engine_dimension(dimension);
            let values: Vec<f32> = members
                .iter()
                .map(|features| features.value(engine))
                .collect();
            let mut sorted = values.clone();
            sorted.sort_by(f32::total_cmp);
            for (level, value) in blended.iter_mut().zip(&values) {
                *level += weight * rank_level(&sorted, *value);
            }
        }
        field_of(blended.into_iter().map(|level| level / total))
    }

    /// The light build has no tower to ask.
    #[cfg(not(feature = "vibe-analysis"))]
    pub(crate) fn accept_embedding(
        &mut self,
        _prompt: &str,
        _embedding: &Result<Vec<f32>, String>,
        _albums: &[AlbumVm],
        _chosen: &HashMap<u64, EditionKey>,
    ) {
        self.counting = false;
    }

    /// **The words, and only the words.**
    ///
    /// A `journey: String` used to be appended here — *"energy shape: Slow
    /// build; journey: X then Y"* — so that a shape reached the engine as
    /// *text*, embedded by a model that was being asked to match audio. That
    /// was the whole of what the old shaping controls did, and it is why
    /// they could not move a track by a position. The shape travels as a
    /// contour now, on its own axis, and the prompt says what it always
    /// meant.
    fn effective_request(&self) -> String {
        // **`All songs` means all songs.** The choice is a request-level one,
        // so it is answered here rather than by the view hiding a field that
        // still filtered — which is what folding the words away used to do,
        // under a line promising Baz would use everything it had heard.
        //
        // The words are kept rather than cleared: switching back should not
        // cost somebody the phrase they wrote.
        if !self.words_open {
            return String::new();
        }
        self.prompt.trim().to_owned()
    }

    pub(crate) fn set_length(&mut self, length: MixLength) {
        self.length_touched = true;
        self.length = length;
    }

    /// **Give the line a number of points**, and mark the shape as the
    /// listener's — the same rule dragging one follows.
    pub(crate) fn set_points(&mut self, count: usize) {
        self.shape_touched = true;
        self.contour.set_points(count);
    }

    /// **Bring the list back into step with a request that changed**, at the
    /// seed it already stands at.
    ///
    /// The difference from [`Self::compose`] is the whole of what makes an
    /// always-live list bearable: this does **not** advance the seed, so a
    /// changed length or a moved line changes only what that change implies,
    /// and the diff's sentence names it. Pressing *Compose* is what draws a
    /// different one.
    pub(crate) fn recompose(&mut self, albums: &[AlbumVm], chosen: &HashMap<u64, EditionKey>) {
        self.create(albums, chosen);
    }

    /// **Compose: a new list every press.**
    ///
    /// The owner: *"we should instead make the button compose generate a new
    /// playlist each time it's clicked."* So it does, and the separate
    /// *another version* press that used to carry that is gone — one control,
    /// one act, and the help text under it says what pressing again will do.
    ///
    /// **The engine underneath is still exactly deterministic**, which is what
    /// makes this honest rather than random: `compose(request, seed)` is a
    /// pure function and invariant I2 says so in CI. What advances is the
    /// seed, here, in the one place a press arrives — so the diff can always
    /// name the cause, and *a new draw of the same request* is a cause a
    /// listener performed rather than something that happened to them.
    pub(crate) fn compose(&mut self, albums: &[AlbumVm], chosen: &HashMap<u64, EditionKey>) {
        self.variation = self.variation.wrapping_add(1);
        self.varied = true;
        self.create(albums, chosen);
    }

    /// One compose at the seed the request currently stands at.
    fn create(&mut self, albums: &[AlbumVm], chosen: &HashMap<u64, EditionKey>) {
        self.open = true;
        self.awaiting_create = false;
        let request = self.effective_request();
        let generated = generate(
            &request,
            &self.contour,
            self.length,
            self.variation,
            Drawn {
                features: &self.features,
                albums,
                chosen,
            },
        );
        let mut preview = match generated {
            Ok(preview) => {
                self.error = None;
                preview
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        };
        if let Some(preview) = &mut preview {
            preview.diff = self
                .preview
                .as_ref()
                .map(|previous| diff(previous, preview, self.varied));
        }
        self.varied = false;
        self.preview = preview;
    }

    pub(crate) fn remove_preview(&mut self, row: usize) {
        if let Some(preview) = &mut self.preview
            && row < preview.items.len()
        {
            preview.items.remove(row);
        }
    }

    pub(crate) fn shift_preview(&mut self, row: usize, delta: i32) {
        let Some(preview) = &mut self.preview else {
            return;
        };
        let neighbour = match delta {
            value if value < 0 => row.checked_sub(1),
            value if value > 0 => row.checked_add(1),
            _ => None,
        };
        if let Some(neighbour) = neighbour.filter(|neighbour| *neighbour < preview.items.len()) {
            preview.items.swap(row, neighbour);
        }
    }

    pub(crate) fn has_features(&self) -> bool {
        !self.features.is_empty()
    }

    pub(crate) fn request_changed(&self) -> bool {
        self.preview.as_ref().is_some_and(|preview| {
            preview.request != self.effective_request()
                || preview.target_minutes != self.length.minutes()
        })
    }

    /// One bounded, listener-facing summary; raw paths and decoder internals
    /// remain diagnostics rather than becoming Home's most prominent copy.
    pub(crate) fn failure_note(&self) -> Option<String> {
        let error = self.error.as_deref()?;
        if self.failed == 0 {
            return Some(error.to_owned());
        }
        let tracks = if self.failed == 1 { "track" } else { "tracks" };
        Some(format!(
            "{} {tracks} skipped. Last issue: {error}",
            self.failed
        ))
    }
}

/// **What changed, and the sentence naming why** — design 21 §6's fourth
/// readout.
///
/// Every cause here is one the listener performed, and each is read off a fact
/// the engine returned rather than guessed at in the view: the words show up
/// as a changed eligible count, the line as a changed contour, the length as a
/// changed target, and *another version* as the press that was made. Nothing
/// else can move a list, which is exactly what the sentence is for.
fn diff(previous: &Generated, current: &Generated, varied: bool) -> Diff {
    let before: HashSet<&PathBuf> = previous.items.iter().map(|item| &item.path).collect();
    let kept = current
        .items
        .iter()
        .filter(|item| before.contains(&item.path))
        .count();
    let fresh = current.items.len().saturating_sub(kept);
    let changed = |what: &str| {
        if fresh == 0 && kept == current.items.len() && previous.items.len() == current.items.len()
        {
            format!("{what}, and the list is the same")
        } else {
            format!("{what} — changed {fresh} of {}", current.items.len())
        }
    };
    let words_moved = previous.request != current.request;
    let line_moved = previous.contour != current.contour;
    let length_moved = previous.target_minutes != current.target_minutes;
    let cause = if words_moved {
        match current.eligible_tracks.cmp(&previous.eligible_tracks) {
            std::cmp::Ordering::Equal => changed(&format!(
                "your words changed, and the same {} songs are still eligible",
                current.eligible_tracks
            )),
            ordering => {
                let moved = if ordering == std::cmp::Ordering::Less {
                    "narrowed"
                } else {
                    "widened"
                };
                changed(&format!(
                    "your words {moved} what is eligible, from {} to {}",
                    previous.eligible_tracks, current.eligible_tracks
                ))
            }
        }
    } else if varied {
        changed("a new draw of the same request")
    } else if line_moved && length_moved {
        changed("you moved the line and changed the length")
    } else if line_moved {
        // The claim design 21 §3 makes, and Phase 1's invariant I3 backs: the
        // pool is the words' doing, so a moved line reorders the same songs.
        changed(&format!(
            "you moved the line, which reorders the same {} eligible songs",
            current.eligible_tracks
        ))
    } else if length_moved {
        changed("you changed the length")
    } else {
        "identical, because nothing changed".to_owned()
    };
    Diff {
        kept,
        fresh,
        previous: previous.items.len(),
        cause,
    }
}

/// Paths in the selected editions are the complete, visible analysis scope.
pub(crate) fn library_paths(albums: &[AlbumVm], chosen: &HashMap<u64, EditionKey>) -> Vec<PathBuf> {
    albums
        .iter()
        .filter_map(|album| vm::selected_edition(album, chosen.get(&album.id).copied()))
        .flat_map(|edition| edition.tracks.iter().map(|track| track.path.clone()))
        .collect()
}

#[cfg(feature = "vibe-analysis")]
pub(crate) async fn prepare(index: PathBuf, paths: Vec<PathBuf>) -> Result<Preparation, String> {
    tokio::task::spawn_blocking(move || baz_vibe::prepare(&index, paths))
        .await
        .map_err(|error| format!("local analysis worker stopped: {error}"))?
        .map(|prepared| Preparation {
            ready: prepared.ready,
            pending: prepared.pending,
        })
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "vibe-analysis"))]
pub(crate) fn prepare(
    _index: PathBuf,
    _paths: Vec<PathBuf>,
) -> impl Future<Output = Result<Preparation, String>> {
    std::future::ready(Err(
        "This is the light build; local sonic analysis is not included.".to_owned(),
    ))
}

/// **Embed one settled phrase**, off the interface thread.
///
/// This is the cost design 21 §10 names: the text tower is roughly 350 MiB and
/// the first call is what pays for it. The phrase comes back beside the vector
/// so a result that arrives after the words moved on can be discarded rather
/// than shown as current.
#[cfg(feature = "vibe-analysis")]
pub(crate) async fn embed(prompt: String) -> (String, Result<Vec<f32>, String>) {
    let asked = prompt.clone();
    let result = tokio::task::spawn_blocking(move || baz_vibe::embed_request(&prompt))
        .await
        .map_err(|error| format!("local model worker stopped: {error}"))
        .and_then(|result| result.map_err(|error| error.to_string()));
    (asked, result)
}

/// **Give the text tower back**, off the interface thread.
///
/// Dropping an ONNX Runtime session tears down its arena, which is not
/// instant and has no business happening between two frames. Called when the
/// composing place is left — see `baz_vibe::release_text_model` for why that
/// is the moment and not a timer.
#[cfg(feature = "vibe-analysis")]
pub(crate) async fn release_text() {
    // If the pool is gone the process is going with it, and the memory this
    // was called to return is about to be returned by exit(2).
    let before = resident_mib();
    drop(tokio::task::spawn_blocking(baz_vibe::release_text_model).await);
    let after = resident_mib();
    if let (Some(before), Some(after)) = (before, after) {
        crate::baz_log!("[vibe] released the text tower: {before} MiB -> {after} MiB");
    } else {
        crate::baz_log!("[vibe] released the text tower");
    }

    // **A second trim, later, was tried and did not earn its keep.** The
    // analysis workers are tokio blocking threads that retire on their own
    // keep-alive and take their audio sessions with them, so a trim fifteen
    // seconds after this one ought to have collected their pages. Measured:
    // `762 MiB -> 762 MiB`. Whatever ONNX Runtime's per-session arena is, it
    // is not on glibc's free lists for `malloc_trim` to walk, and a delayed
    // task that reliably returns nothing is worse than no task at all.
}

/// This process's own resident set, in MiB — Linux only, and `None` anywhere
/// else rather than a guess.
#[cfg(feature = "vibe-analysis")]
fn resident_mib() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
        let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kib / 1024)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(not(feature = "vibe-analysis"))]
pub(crate) async fn release_text() {}

#[cfg(not(feature = "vibe-analysis"))]
pub(crate) fn embed(prompt: String) -> impl Future<Output = (String, Result<Vec<f32>, String>)> {
    std::future::ready((
        prompt,
        Err("This is the light build; local sonic analysis is not included.".to_owned()),
    ))
}

#[cfg(feature = "vibe-analysis")]
pub(crate) async fn analyze(index: PathBuf, run: u64, path: PathBuf) -> AnalysisResult {
    let analyzed_path = path.clone();
    let result =
        tokio::task::spawn_blocking(move || baz_vibe::analyze_and_store(&index, analyzed_path))
            .await
            .map_err(|error| format!("local analysis worker stopped: {error}"))
            .and_then(|result| {
                result.map_err(|error| {
                    crate::baz_log!("[vibe] skipped {}: {error}", path.display());
                    friendly_analysis_error(&path, &error)
                })
            })
            .map(|analyzed| analyzed.features);
    AnalysisResult {
        run,
        path,
        features: result,
    }
}

#[cfg(feature = "vibe-analysis")]
fn friendly_analysis_error(path: &Path, error: &baz_vibe::Error) -> String {
    let track = compact_track_name(path);
    let reason = match error {
        baz_vibe::Error::Inspect { .. } => {
            "Baz could not read the file. Check that it is still available and readable."
        }
        baz_vibe::Error::Decode { .. } => {
            "Baz could not read its audio data. The file may be damaged or use an unsupported encoding."
        }
        baz_vibe::Error::Analyze { .. } => "Local audio feature extraction failed for this file.",
        baz_vibe::Error::Semantic(_) => {
            "The bundled local semantic model could not analyse this file."
        }
        baz_vibe::Error::Store(_)
        | baz_vibe::Error::UnsupportedStoreVersion { .. }
        | baz_vibe::Error::InvalidRow => {
            "Baz could not update the disposable local analysis index."
        }
    };
    format!("Could not analyse “{track}”. {reason} Baz will retry it next time.")
}

#[cfg(feature = "vibe-analysis")]
fn compact_track_name(path: &Path) -> String {
    const MAX_CHARS: usize = 72;
    let name = seed_name(path);
    if name.chars().count() <= MAX_CHARS {
        return name.to_owned();
    }
    let mut compact: String = name.chars().take(MAX_CHARS - 1).collect();
    compact.push('…');
    compact
}

#[cfg(not(feature = "vibe-analysis"))]
pub(crate) fn analyze(
    _index: PathBuf,
    run: u64,
    path: PathBuf,
) -> impl Future<Output = AnalysisResult> {
    std::future::ready(AnalysisResult {
        run,
        path,
        features: Err("This is the light build; local sonic analysis is not included.".to_owned()),
    })
}

/// baz's contour in the engine's own vocabulary — one lane per lane, one
/// dimension per dimension.
#[cfg(feature = "vibe-analysis")]
fn engine_contour(contour: &Contour) -> baz_vibe::Contour {
    baz_vibe::Contour {
        lanes: contour
            .lanes
            .iter()
            .filter(|lane| !lane.points.is_empty())
            .map(|lane| baz_vibe::Lane {
                dimension: engine_dimension(lane.dimension),
                points: lane
                    .points
                    .iter()
                    .map(|point| baz_vibe::ContourPoint {
                        at: point.at,
                        level: point.level,
                    })
                    .collect(),
                weight: lane.weight,
            })
            .collect(),
    }
}

#[cfg(feature = "vibe-analysis")]
#[expect(
    clippy::too_many_lines,
    reason = "candidate projection, duration convergence and result construction form one generation boundary"
)]
fn generate(
    prompt: &str,
    contour: &Contour,
    length: MixLength,
    variation: u64,
    from: Drawn<'_>,
) -> Result<Option<Generated>, String> {
    let Drawn {
        features,
        albums,
        chosen,
    } = from;
    let mut candidates = Vec::new();
    let mut items = HashMap::new();
    let mut seen_paths = HashSet::new();
    let pool_tracks = library_paths(albums, chosen).len();
    for album in albums {
        let Some(edition) = vm::selected_edition(album, chosen.get(&album.id).copied()) else {
            continue;
        };
        for track in &edition.tracks {
            if !seen_paths.insert(track.path.clone()) {
                continue;
            }
            let Some(feature) = features.get(&track.path) else {
                continue;
            };
            candidates.push(baz_vibe::Candidate {
                path: track.path.clone(),
                album: album.id,
                artist: album.artist.label().to_owned(),
                features: feature.clone(),
            });
            items.insert(
                track.path.clone(),
                QueueItemVm {
                    title: track.title.clone(),
                    artist: track.artist.clone().filter(|_| album.track_artists_vary),
                    album: album.title.clone(),
                    album_artist: Some(album.artist.label().to_owned()),
                    duration: track.duration,
                    path: track.path.clone(),
                },
            );
        }
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    let known: Vec<_> = items.values().filter_map(|item| item.duration).collect();
    let average_seconds = if known.is_empty() {
        240
    } else {
        known.iter().map(std::time::Duration::as_secs).sum::<u64>()
            / u64::try_from(known.len()).unwrap_or(1)
    }
    .max(1);
    let target_seconds = length.minutes() * 60;
    let mut limit = usize::try_from(target_seconds.div_ceil(average_seconds))
        .unwrap_or(PLAYLIST_CAP)
        .clamp(1, PLAYLIST_CAP.min(candidates.len()));
    // **The words choose the pool; the shape chooses the walk.**
    //
    // The pool is drawn once, here, and the convergence below walks it four
    // times at different lengths — so the text tower is paid for once per
    // press rather than once per attempt, and so every attempt is choosing
    // from the same eligible set the readouts are describing.
    let request = if prompt.trim().is_empty() {
        None
    } else {
        Some(baz_vibe::embed_request(prompt).map_err(|error| error.to_string())?)
    };
    let engine_contour = engine_contour(contour);
    let pool = baz_vibe::eligible(request.as_deref(), &candidates);
    let mut best = None;
    for _ in 0..4 {
        let selection = baz_vibe::compose(
            request.as_deref(),
            &pool,
            &engine_contour,
            &candidates,
            limit,
            variation,
        );
        let selected_seconds = selection
            .paths
            .iter()
            .filter_map(|path| items.get(path))
            .map(|item| {
                item.duration
                    .map_or(average_seconds, |value| value.as_secs())
            })
            .sum::<u64>()
            .max(1);
        let difference = selected_seconds.abs_diff(target_seconds);
        if best
            .as_ref()
            .is_none_or(|(_, best_difference)| difference < *best_difference)
        {
            best = Some((selection, difference));
        }
        let scaled = u64::try_from(limit)
            .unwrap_or(u64::MAX)
            .saturating_mul(target_seconds)
            .div_ceil(selected_seconds);
        let adjusted = usize::try_from(scaled)
            .unwrap_or(PLAYLIST_CAP)
            .clamp(1, PLAYLIST_CAP.min(candidates.len()));
        if adjusted == limit {
            break;
        }
        limit = adjusted;
    }
    let Some((selection, _)) = best else {
        return Ok(None);
    };
    // The chosen tracks and their levels stay in step: a path the projection
    // no longer holds drops it from every lane's row too, so a dot on a line
    // is always the track beside it rather than the one that used to be
    // there.
    let kept: Vec<usize> = selection
        .paths
        .iter()
        .enumerate()
        .filter(|(_, path)| items.contains_key(*path))
        .map(|(index, _)| index)
        .collect();
    let selected: Vec<QueueItemVm> = kept
        .iter()
        .filter_map(|&index| items.remove(&selection.paths[index]))
        .collect();
    let in_step = |row: &[f32]| -> Vec<f32> {
        kept.iter()
            .filter_map(|&index| row.get(index).copied())
            .collect()
    };
    let levels: Vec<Vec<f32>> = selection.levels.iter().map(|lane| in_step(lane)).collect();
    let blended = in_step(&selection.blended);
    let matches: Vec<Match> = kept
        .iter()
        .filter_map(|&index| selection.matches.get(index))
        .map(|found| Match {
            similarity: found.similarity,
            ticks: found.strength.ticks(),
        })
        .collect();
    let description = format!(
        "{} · {} minutes · local semantic model",
        prompt.trim(),
        length.minutes()
    );
    Ok(Some(Generated {
        description,
        request: prompt.trim().to_owned(),
        items: selected,
        levels,
        blended,
        cloud: field_of(selection.blended_cloud.into_iter()),
        lane_clouds: selection
            .cloud
            .into_iter()
            .map(|lane| field_of(lane.into_iter()))
            .collect(),
        matches,
        pool_tracks,
        analyzed_tracks: selection.analysed_tracks,
        eligible_tracks: selection.eligible_tracks,
        tempo_span: selection.tempo_span,
        target_minutes: length.minutes(),
        asked_positions: limit,
        contour: contour.clone(),
        diff: None,
    }))
}

#[cfg(not(feature = "vibe-analysis"))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the light-build seam retains the full build's generation result contract"
)]
fn generate(
    _prompt: &str,
    _contour: &Contour,
    _length: MixLength,
    _variation: u64,
    _from: Drawn<'_>,
) -> Result<Option<Generated>, String> {
    Ok(None)
}

/// **Everything a compose draws from**, so the request and the library it is
/// asked of are two arguments rather than five.
///
/// The grouping is not cosmetic: these three travel together everywhere and
/// have to agree with each other — `features` is indexed by paths that only
/// mean anything against `albums` under `chosen`, so a caller that assembled
/// two of them from one place and the third from another would be composing a
/// list about two libraries.
#[derive(Clone, Copy)]
// The light build's `generate` is a stub that reads none of these. The struct
// still has to exist, because the call site that builds it does.
#[cfg_attr(
    not(feature = "vibe-analysis"),
    expect(
        dead_code,
        reason = "assembled by the caller, read only by the full build"
    )
)]
struct Drawn<'a> {
    /// What listening learned, per path.
    features: &'a HashMap<PathBuf, SonicFeatures>,
    /// The library as the shelf currently arranges it.
    albums: &'a [AlbumVm],
    /// Which edition of each record is the one that plays.
    chosen: &'a HashMap<u64, EditionKey>,
}

/// Name shown for the current seed without exposing a full path in Home.
///
/// Its one caller is `compact_track_name`, which shortens an analysis failure
/// for a listener — so it belongs to the build that can fail an analysis.
#[cfg(feature = "vibe-analysis")]
pub(crate) fn seed_name(path: &Path) -> &str {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("sounding track")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::vm::{AlbumArtistVm, EditionVm, ReplayGainCoverage, TrackVm};

    fn album() -> AlbumVm {
        AlbumVm {
            id: 1,
            title: Some("Record".to_owned()),
            artist: AlbumArtistVm::Named("Artist".to_owned()),
            track_artists_vary: false,
            year: None,
            genre: None,
            first_seen_ns: None,
            first_track: PathBuf::from("/m/one.flac"),
            editions: vec![EditionVm {
                key: EditionKey(None),
                detail: None,
                bitrate: None,
                bit_depth: None,
                sample_rate: None,
                replay_gain: ReplayGainCoverage::default(),
                tracks: vec![TrackVm {
                    disc: None,
                    number: Some(1),
                    title: "One".to_owned(),
                    artist: None,
                    duration: None,
                    path: PathBuf::from("/m/one.flac"),
                    bytes: None,
                }],
            }],
        }
    }

    /// The first lane's points, which is what these tests are about.
    fn points(state: &State) -> Vec<ContourPoint> {
        state
            .contour
            .lane(0)
            .map(|lane| lane.points.clone())
            .unwrap_or_default()
    }

    /// **The line is a line**: its ends are the playlist's ends, an interior
    /// turn stays between its neighbours, and no point may be dragged past
    /// the collection's own extremes.
    ///
    /// This is the whole of what a drag may do, and it lives here rather than
    /// in the widget because it is a rule about *a request*, not about a
    /// pointer — `crate::contour` reports raw geometry and this decides what
    /// it means.
    #[test]
    fn a_drag_cannot_make_a_shape_a_playlist_could_not_have() {
        /// Near enough for a level that has been through an addition.
        const STEP: f32 = 1e-4;
        let mut state = State::default();
        state.set_shape(Shape::DEFAULT);
        // The ends hold their positions however far the pointer wanders.
        //
        // Levels are compared to a rounding step rather than to
        // `f32::EPSILON`: a drag on the *all five* tab moves every line by
        // what the pointer moved, so where a point lands is the sum of where
        // it was and how far it went, and −1.6 + 2.1 is 0.49999988.
        state.drag_contour(0, 0, 0.9, 0.5);
        assert!((points(&state)[0].at - 0.0).abs() < f32::EPSILON);
        assert!((points(&state)[0].level - 0.5).abs() < STEP);
        let last = points(&state).len() - 1;
        state.drag_contour(0, last, 0.1, -0.5);
        assert!((points(&state)[last].at - 1.0).abs() < f32::EPSILON);

        // Levels clamp to the collection's own ends rather than running off
        // the top of the box.
        state.drag_contour(0, 0, 0.0, 9.0);
        assert!((points(&state)[0].level - LEVEL_LIMIT).abs() < STEP);
        state.drag_contour(0, 0, 0.0, -9.0);
        assert!((points(&state)[0].level + LEVEL_LIMIT).abs() < STEP);

        // An interior turn stays between its neighbours, with a gap either
        // side: two points at one position would ask for two levels at once.
        // The turn comes from a preset now rather than from a stepper, and a
        // preset arrives at the working resolution rather than its own.
        state.set_shape(Shape::ALL[3]);
        assert_eq!(points(&state).len(), Contour::DEFAULT_POINTS);
        state.drag_contour(0, 1, 5.0, 0.0);
        assert!(points(&state)[1].at <= 1.0 - Contour::MIN_GAP);
        state.drag_contour(0, 1, -5.0, 0.0);
        assert!(points(&state)[1].at >= Contour::MIN_GAP);
        assert!(
            points(&state)
                .windows(2)
                .all(|pair| pair[1].at > pair[0].at)
        );
    }

    /// **Turns come from the presets now**, not from a stepper.
    ///
    /// Design 21 §5 deletes the `−`/`+` pair that minted a curve — *"nothing
    /// else in baz creates a control with a stepper"* — so the shapes on
    /// offer are where a multi-turn line comes from, and every one of them is
    /// a line a playlist could actually have.
    #[test]
    fn every_offered_shape_is_a_line_a_playlist_could_have() {
        let mut state = State::default();
        for (index, shape) in Shape::ALL.iter().enumerate() {
            state.set_shape(*shape);
            let drawn = points(&state);
            if index == 0 {
                assert!(drawn.is_empty(), "Any is no line at all");
                continue;
            }
            // **The arc loads, at the working resolution.** A preset is
            // written as the fewest points that state it and arrives
            // resampled to `DEFAULT_POINTS`, so what has to hold is that the
            // line is the same line — its ends, and its level wherever you
            // read it — not that the points are the ones in the constant.
            assert_eq!(
                drawn.len(),
                Contour::DEFAULT_POINTS,
                "{} did not load at the working resolution",
                shape.label
            );
            for step in 0_u8..=20 {
                let at = f32::from(step) / 20.0;
                let (was, is) = (
                    level_at(&shape.points(), at).expect("a preset level"),
                    level_at(&drawn, at).expect("a drawn level"),
                );
                assert!(
                    (was - is).abs() < 0.001,
                    "{} changed shape at {at}: {was} became {is}",
                    shape.label
                );
            }
            assert!(drawn.len() >= 2, "{} needs two ends", shape.label);
            assert!(
                drawn.windows(2).all(|pair| pair[1].at > pair[0].at),
                "{} runs backwards",
                shape.label
            );
            // Every line of the blend gets it, because one drawn line is five
            // lanes holding one curve.
            assert_eq!(state.contour.lanes.len(), Dimension::ALL.len());
            assert!(state.contour.is_one_line());
        }
    }

    /// **The line the interface draws is the line the engine scores.**
    ///
    /// The arithmetic exists twice on purpose — `baz-vibe` is an optional
    /// dependency and the light build has no engine to ask — so the two are
    /// pinned together here, in the build that has both. Sampled rather than
    /// reasoned about: a lerp is easy to get subtly wrong at the ends, which
    /// is exactly where a playlist's first and last track live.
    #[cfg(feature = "vibe-analysis")]
    #[test]
    fn the_drawn_line_is_the_scored_line() {
        for shape in Shape::ALL {
            let drawn = Contour::blended(&shape.points());
            let scored = engine_contour(&drawn);
            for step in 0_u8..=20 {
                let at = f32::from(step) / 20.0;
                let ours = drawn.lane(0).and_then(|lane| level_at(&lane.points, at));
                let theirs = scored
                    .lanes
                    .first()
                    .and_then(|lane| baz_vibe::Contour::level_at(&lane.points, at));
                match (ours, theirs) {
                    (None, None) => {}
                    (Some(ours), Some(theirs)) => assert!(
                        (ours - theirs).abs() < 0.0001_f32,
                        "{} disagrees at {at}: drawn {ours}, scored {theirs}",
                        shape.label
                    ),
                    _ => panic!("{} is a line on one side only at {at}", shape.label),
                }
            }
        }
    }

    /// **A mood fills the form and leaves it editable**, which is the whole
    /// of what makes it a starting point rather than a mode.
    ///
    /// It used to also have to *recognise itself* afterwards, so a chip on
    /// this page could light. The chips are gone — the moods are on the door
    /// you come through, and the page teaches by example instead — so what is
    /// left to hold is that pressing one sets all three parts of a request
    /// and none of them stick.
    #[test]
    fn a_recipe_fills_the_request_and_leaves_every_part_of_it_editable() {
        let mut state = State::default();
        for recipe in Recipe::ALL {
            state.start_from(recipe);
            assert_eq!(state.prompt, recipe.prompt);
            assert_eq!(state.length, recipe.length);
            assert!(
                state.words_open,
                "{} filled the words without turning them on",
                recipe.label
            );
            assert_eq!(points(&state).len(), Contour::DEFAULT_POINTS);
            assert_eq!(
                points(&state).first().map(|point| point.level),
                recipe.shape().points().first().map(|point| point.level),
                "{} does not open where its shape does",
                recipe.label
            );
        }
        // Change any one of the three and it stays changed.
        state.start_from(Recipe::ALL[0]);
        state.set_prompt("something else entirely");
        assert_eq!(state.prompt, "something else entirely");
        state.set_length(MixLength::TwoHours);
        assert_eq!(state.length, MixLength::TwoHours);

        // Every recipe says something to the model and draws a real line.
        for recipe in Recipe::ALL {
            assert!(
                recipe.prompt.split_whitespace().count() >= 4,
                "{} asks the model for too little",
                recipe.label
            );
            assert!(!recipe.shape().points().is_empty(), "{}", recipe.label);
        }
    }

    /// **`Any` is a shape in the offered set and no line at all**, which is
    /// how the words alone stay reachable now that a line is the default.
    #[test]
    fn the_offered_shapes_include_no_shape() {
        assert_eq!(Shape::ALL[0].label, "Any");
        assert!(Shape::ANY.points().is_empty());
        assert!(level_at(&Shape::ANY.points(), 0.5).is_none());
        for shape in Shape::ALL.iter().skip(1) {
            let contour = shape.points();
            assert!(
                contour.len() >= 2,
                "{} is drawn as a line and needs two ends",
                shape.label
            );
            assert!(
                contour.windows(2).all(|pair| pair[1].at > pair[0].at),
                "{}'s points run backwards",
                shape.label
            );
            assert!(
                contour.iter().all(|point| point.level.abs() <= LEVEL_LIMIT),
                "{} reaches past the collection's own ends",
                shape.label
            );
        }
    }

    /// **The library's own distribution**, bucketed: the picture behind the
    /// line is a count of what there is, normalised against its fullest band
    /// so an enormous collection and a small one read the same.
    #[cfg(feature = "vibe-analysis")]
    #[test]
    fn the_field_is_the_collections_own_shape() {
        let field = field_of([-2.0, -2.0, -2.0, 0.0, 2.0].into_iter());
        assert_eq!(field.len(), FIELD_BUCKETS);
        assert!((field[0] - 1.0).abs() < f32::EPSILON, "{field:?}");
        assert!(field[FIELD_BUCKETS / 2] > 0.0);
        assert!(
            (field[FIELD_BUCKETS - 1] - 1.0 / 3.0).abs() < 0.001,
            "{field:?}"
        );
        // Nothing analysed is nothing drawn, rather than a flat band that
        // would claim the collection is evenly spread.
        assert!(field_of(std::iter::empty()).is_empty());
    }

    /// **What Baz heard, from features it did hear.**
    ///
    /// Design note 24 §2: the valuable items are falsifiable claims about
    /// specific records, so the test that matters is that the named extremes
    /// are the right records. If baz calls the ambient piece the loudest
    /// thing in a library, its owner knows in one second that something is
    /// broken — and so does this.
    #[cfg(feature = "vibe-analysis")]
    #[test]
    fn the_reading_names_the_records_a_listener_can_grade() {
        /// One hand-built track: tempo, loudness and loudness variance are
        /// what energy is made of, and the rest is held still so the axes
        /// that are supposed to be flat are unambiguously flat.
        fn heard(tempo: f32, loudness: f32) -> SonicFeatures {
            let mut values = vec![0.0_f32; 30];
            values[bliss_index::TEMPO] = tempo;
            values[bliss_index::MEAN_LOUDNESS] = loudness;
            values[bliss_index::STD_LOUDNESS] = loudness;
            SonicFeatures::from_values(values, vec![0.0; 512])
        }
        let named = |number: u32, title: &str| TrackVm {
            disc: None,
            number: Some(number),
            title: title.to_owned(),
            artist: None,
            duration: None,
            path: PathBuf::from(format!("/m/{title}.flac")),
            bytes: None,
        };
        let mut album = album();
        album.editions[0].tracks =
            vec![named(1, "Still"), named(2, "Middling"), named(3, "Racing")];

        let mut state = State {
            features: [
                (PathBuf::from("/m/Still.flac"), heard(-0.9, -0.9)),
                (PathBuf::from("/m/Middling.flac"), heard(0.0, 0.0)),
                (PathBuf::from("/m/Racing.flac"), heard(0.9, 0.9)),
            ]
            .into_iter()
            .collect(),
            ..State::default()
        };
        state.rebuild_profile(&[album], &HashMap::new());
        let profile = &state.profile;

        assert_eq!(profile.heard, 3);
        // The four named ends, each a claim about a record rather than a
        // summary — and each the *right* record.
        let ends: Vec<(&str, &str)> = profile
            .extremes
            .iter()
            .map(|(label, title, _)| (*label, title.as_str()))
            .collect();
        assert_eq!(
            ends,
            [
                ("Quietest", "Still"),
                ("Loudest", "Racing"),
                ("Slowest", "Still"),
                ("Fastest", "Racing"),
            ]
        );
        assert_eq!(
            profile.extremes[0].2, "Artist",
            "an extreme names who made it, or it cannot be recognised"
        );

        // Tempo in the one unit a listener already owns, and in order.
        let (low, high) = profile.tempo_range.expect("a tempo range");
        let middle = profile.tempo_median.expect("a median tempo");
        assert!(low <= middle && middle <= high, "{low} {middle} {high}");
        // bliss normalizes tempo over 0–206 BPM, so the middle track's 0.0 is
        // 103 — the conversion, checked in the unit a listener reads.
        assert_eq!(middle, 103);
        // The ends are the 5th and 95th percentile rather than the minimum
        // and maximum, so a single mastering artefact cannot set the range a
        // whole library is described by — which over three tracks is the same
        // as the ends: 0.9 normalized is 196 BPM, and −0.9 is 10.
        assert_eq!((low, high), (10, 196));

        // **The axes this collection cannot answer**, and only those. Tempo,
        // loudness and its variance were given a spread, so the three
        // dimensions made of them move; brightness and texture were handed
        // one value each and would otherwise draw a line the dots followed
        // perfectly while the music did not change.
        for moving in [Dimension::Tempo, Dimension::Energy, Dimension::Dynamics] {
            assert!(!profile.flat_axes.contains(&moving), "{moving:?}");
        }
        for still in [Dimension::Brightness, Dimension::Texture] {
            assert!(profile.flat_axes.contains(&still), "{still:?}");
        }

        // And a library that has been heard about, then forgotten, keeps
        // nothing it can no longer support.
        state.features.clear();
        state.rebuild_profile(&[], &HashMap::new());
        assert_eq!(state.profile, Profile::default());
    }

    /// The three feature slots the test above actually sets, spelled out
    /// rather than reached through `bliss_audio` — this crate does not depend
    /// on it, and the numbers are part of the stored format anyway.
    #[cfg(feature = "vibe-analysis")]
    mod bliss_index {
        pub(super) const TEMPO: usize = 0;
        pub(super) const MEAN_LOUDNESS: usize = 8;
        pub(super) const STD_LOUDNESS: usize = 9;
    }

    /// **All five together is one shape**, and choosing it makes it one.
    ///
    /// The owner had to say this three times, which is three more than it
    /// should have taken: shaping a line on its own and then returning to
    /// *all five* must leave five lines holding one curve, not a tab
    /// presiding over five different ones.
    #[test]
    fn the_all_five_tab_moves_every_line_and_keeps_what_was_shaped_apart() {
        let mut state = State::default();
        state.set_shape(Shape::DEFAULT);
        let level = |state: &State, lane: usize, point: usize| {
            state.contour.lane(lane).expect("a lane").points[point].level
        };

        // Shape one line on its own: brightness, dragged well clear.
        state.show_line(Some(2));
        state.drag_contour(2, 0, 0.0, 1.2);
        let apart = level(&state, 2, 0) - level(&state, 0, 0);
        assert!(apart > 2.0, "brightness did not move clear: {apart}");
        for other in [1_usize, 3, 4] {
            assert!(
                (level(&state, other, 0) - level(&state, 0, 0)).abs() < 1e-4,
                "line {other} moved when only brightness was held"
            );
        }

        // **Back to all five, and every line is back on one shape.** Not a
        // view of five shapes: one shape, which is what the tab says.
        state.show_line(None);
        assert!(
            state.contour.is_one_line(),
            "all five did not put the lines back on one shape"
        );
        let shared = level(&state, 0, 0);
        for lane in 1..5 {
            assert!(
                (level(&state, lane, 0) - shared).abs() < 1e-4,
                "line {lane}"
            );
        }

        // …and a drag there moves every one of them.
        state.drag_contour(0, 0, 0.0, shared + 0.5);
        for lane in 0..5 {
            let moved = level(&state, lane, 0) - shared;
            assert!(
                (moved - 0.5).abs() < 1e-4,
                "line {lane} moved by {moved}, not by what the pointer moved"
            );
        }
    }

    /// **The shape opens the sentence, and the words close it.**
    ///
    /// Design note 25 reordered the one line that states the whole request,
    /// because the order was a claim about which control it rests on. Worth a
    /// test rather than a glance: the clauses are assembled from three
    /// different controls and nothing else asserts they arrive in the right
    /// order.
    #[test]
    fn the_stated_request_leads_with_the_shape_and_ends_with_the_words() {
        let mut state = State::default();
        state.set_shape(Shape::DEFAULT);
        assert!(
            state
                .query()
                .ends_with("drawn from any song Baz has heard."),
            "{}",
            state.query()
        );
        state.set_prompt("warm brass");
        assert!(
            state
                .query()
                .ends_with("drawn from songs like “warm brass”."),
            "{}",
            state.query()
        );
        assert!(
            state
                .query()
                .starts_with("Starting quiet and climbing the whole way"),
            "{}",
            state.query()
        );
    }

    /// **A named end is a step in from the end.**
    ///
    /// Enough tracks and the step is real; too few and it names the true end
    /// rather than nothing. Both halves matter: the first is what stops one
    /// misdetection describing a library, and the second is what keeps the
    /// reading working on a collection of two dozen.
    #[cfg(feature = "vibe-analysis")]
    #[test]
    fn a_named_end_steps_past_the_very_end_once_there_is_room_to() {
        let heard = |tempo: f32| {
            let mut values = vec![0.0_f32; 30];
            values[0] = tempo;
            SonicFeatures::from_values(values, vec![0.0; 512])
        };
        let named = |number: usize| TrackVm {
            disc: None,
            number: u32::try_from(number).ok(),
            title: format!("{number:03}"),
            artist: None,
            duration: None,
            path: PathBuf::from(format!("/m/{number:03}.flac")),
            bytes: None,
        };
        let fastest = |state: &mut State, count: usize| {
            let mut album = album();
            album.editions[0].tracks = (0..count).map(named).collect();
            state.features = (0..count)
                .map(|index| {
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "a bounded track index into a normalized tempo"
                    )]
                    let tempo = index as f32 / count as f32;
                    (PathBuf::from(format!("/m/{index:03}.flac")), heard(tempo))
                })
                .collect();
            state.rebuild_profile(&[album], &HashMap::new());
            state
                .profile
                .extremes
                .iter()
                .find(|(word, _, _)| *word == "Fastest")
                .map(|(_, title, _)| title.clone())
                .expect("a fastest record")
        };

        // Two hundred tracks: a hundredth is two, so the second-fastest is
        // named and the outlier at the very top is not.
        let mut state = State::default();
        assert_eq!(fastest(&mut state, 200), "197");
        // Twenty-four: a hundredth is nothing, so the true end is named.
        let mut state = State::default();
        assert_eq!(fastest(&mut state, 24), "023");
    }

    /// **The field's example is made of their music**, and declines rather
    /// than embarrasses itself when it cannot be.
    #[test]
    fn the_placeholder_is_built_from_the_commonest_genre_they_own() {
        let tagged = |id: u64, genre: Option<&str>| AlbumVm {
            id,
            genre: genre.map(str::to_owned),
            ..album()
        };
        assert_eq!(
            library_example(&[
                tagged(1, Some("Shoegaze")),
                tagged(2, Some("shoegaze")),
                tagged(3, Some("Doom Metal")),
            ]),
            Some("warm shoegaze, slow and sparse".to_owned()),
            "the listener's own spelling, lowered, and the one they own most of"
        );
        // A multi-valued tag is read as the value the tagger led with.
        assert_eq!(
            library_example(&[tagged(1, Some("Jazz / Funk"))]),
            Some("warm jazz, slow and sparse".to_owned())
        );
        // Freeform notes in a genre field are declined, not truncated into
        // nonsense — and so is a library with no genres at all.
        assert_eq!(
            library_example(&[tagged(
                1,
                Some("recorded live at the Barrowlands, second night")
            )]),
            None
        );
        assert_eq!(library_example(&[tagged(1, None)]), None);
        assert_eq!(library_example(&[]), None);
    }

    #[test]
    fn analysis_scope_is_the_selected_library_edition() {
        assert_eq!(
            library_paths(&[album()], &HashMap::new()),
            [PathBuf::from("/m/one.flac")]
        );
    }

    #[test]
    fn analysis_scheduler_keeps_four_workers_in_flight() {
        let mut state = State {
            analyzing: true,
            pending: (0..6)
                .map(|index| PathBuf::from(format!("/m/{index}.flac")))
                .collect(),
            ..State::default()
        };

        let first = state.next_jobs(4);
        assert_eq!(first.len(), 4);
        assert!(state.next_jobs(4).is_empty());

        let (run, path) = first[0].clone();
        state.accept_analysis(AnalysisResult {
            run,
            path,
            features: Err("test".to_owned()),
        });
        assert_eq!(state.next_jobs(4).len(), 1);
    }

    /// **Leaving the page puts the page away, and keeps the request.**
    ///
    /// The owner: *"there are issues when navigating away, the page state is
    /// not cleaned up."* Every field below was surviving a navigation, so
    /// coming back landed on somebody else's screen — a list from a request
    /// you had stopped making, a row explaining itself that you never
    /// selected, a count describing a phrase that was no longer on screen, and
    /// a debounce clock still holding a 120 ms subscription open off-page.
    ///
    /// The second half is the half that is easy to lose later: walking to the
    /// Library to check something is not a reason to forget what you were
    /// asking for.
    #[test]
    fn leaving_the_page_clears_what_was_only_true_on_it() {
        let mut state = State {
            open: true,
            awaiting_create: true,
            choosing: true,
            counting: true,
            selected_row: Some(2),
            hovered_row: Some(3),
            error: Some("something".to_owned()),
            live: Some(Live {
                prompt: "warm brass".to_owned(),
                eligible: 40,
                analysed: 100,
                closest: Vec::new(),
                cloud: Vec::new(),
            }),
            preview: Some(Generated::default()),
            ..State::default()
        };
        state.set_prompt("warm brass after midnight");
        state.set_length(MixLength::TwoHours);
        state.set_shape(Shape::ALL[4]);
        let shape = state.contour.clone();

        state.leave_page();

        // Gone: everything that was about the page being on screen.
        assert!(state.preview.is_none(), "a stale result");
        assert_eq!(state.selected_row, None);
        assert_eq!(state.hovered_row, None);
        assert!(state.live.is_none(), "a count of a phrase you have left");
        assert!(!state.counting);
        assert!(!state.awaiting_count(), "a clock with nothing to answer");
        assert!(!state.choosing, "the door standing open behind a used page");
        assert!(!state.open && !state.awaiting_create);
        assert!(state.error.is_none());

        // Kept: the request itself.
        assert_eq!(state.prompt, "warm brass after midnight");
        assert_eq!(state.length, MixLength::TwoHours);
        assert_eq!(state.contour, shape);
    }

    #[test]
    fn cancel_dismisses_consent_without_losing_the_request_and_invalidates_late_work() {
        let mut state = State::default();
        state.set_prompt("warm brass after midnight");
        state.begin_request();
        state.start_preparing();
        let old = state.run;
        state.cancel_analysis();
        state.accept_analysis(AnalysisResult {
            run: old,
            path: PathBuf::from("/m/one.flac"),
            features: Err("late".to_owned()),
        });
        assert_eq!(state.failed, 0);
        assert!(state.error.is_none());
        assert!(!state.open);
        assert_eq!(state.prompt, "warm brass after midnight");
    }

    #[test]
    fn skipped_tracks_are_summarised_without_a_raw_path() {
        let mut state = State {
            failed: 17,
            error: Some("Could not analyse “Theme”. Baz will retry it next time.".to_owned()),
            ..State::default()
        };
        let note = state.failure_note().expect("failure note");
        assert!(note.starts_with("17 tracks skipped."));
        assert!(!note.contains("/music/"));

        state.failed = 1;
        assert!(
            state
                .failure_note()
                .expect("failure note")
                .starts_with("1 track skipped.")
        );
    }

    /// **The diff names the cause, and the cause is always something the
    /// listener did.** Design 21 §6: one use teaches the whole model, so the
    /// sentence has to be right in every case rather than plausible in most.
    #[test]
    fn the_diff_names_what_actually_changed() {
        let list = |titles: &[&str]| -> Vec<QueueItemVm> {
            titles
                .iter()
                .map(|title| QueueItemVm {
                    title: (*title).to_owned(),
                    artist: None,
                    album: None,
                    album_artist: None,
                    duration: None,
                    path: PathBuf::from(format!("/{title}.flac")),
                })
                .collect()
        };
        let generated = |request: &str, titles: &[&str], eligible: usize, minutes: u64| Generated {
            request: request.to_owned(),
            items: list(titles),
            eligible_tracks: eligible,
            target_minutes: minutes,
            contour: Contour::opening(&Shape::DEFAULT.points()),
            ..Generated::default()
        };

        // Nothing moved, so the sentence must say so — which it can only do
        // because compose is deterministic now.
        let same = generated("warm brass", &["a", "b"], 300, 60);
        assert_eq!(
            diff(&same, &same.clone(), false).cause,
            "identical, because nothing changed"
        );

        // The words narrowed the pool, with both counts named.
        let narrowed = generated("warm brass, strings", &["a", "c"], 240, 60);
        let sentence = diff(&same, &narrowed, false).cause;
        assert!(sentence.contains("narrowed"), "{sentence}");
        assert!(
            sentence.contains("300") && sentence.contains("240"),
            "{sentence}"
        );
        assert_eq!(diff(&same, &narrowed, false).kept, 1);
        assert_eq!(diff(&same, &narrowed, false).fresh, 1);

        // The line moved: the same eligible songs, in another order. This is
        // the sentence design 21 §3 promises and Phase 1's I3 backs.
        let mut moved = generated("warm brass", &["b", "a"], 300, 60);
        moved.contour = Contour::blended(&Shape::ALL[4].points());
        let sentence = diff(&same, &moved, false).cause;
        assert!(sentence.contains("reorders the same 300"), "{sentence}");

        // …and a re-press names itself rather than hiding behind the request,
        // which is exactly what the old auto-incrementing seed did not.
        let drawn = generated("warm brass", &["c", "d"], 300, 60);
        let sentence = diff(&same, &drawn, true).cause;
        assert!(sentence.contains("a new draw"), "{sentence}");
    }

    /// **The result proves itself in the axis words** — which is the whole of
    /// what the owner asked the feature to be able to do: *"one track
    /// represents a combination of the different points on that curve. e.g.
    /// loud, fast, dynamic? or quiet, slow, compressed… we have to be able to
    /// prove that this system is working."*
    ///
    /// A rising line over two songs: the first should read as the quiet, slow
    /// end and the last as the loud, fast end, in words a listener can check
    /// by ear without understanding anything underneath.
    #[test]
    fn a_row_says_what_it_is_in_the_words_the_line_is_drawn_in() {
        let song = |title: &str| QueueItemVm {
            title: title.to_owned(),
            artist: None,
            album: None,
            album_artist: None,
            duration: None,
            path: PathBuf::from(format!("/{title}.flac")),
        };
        let mut state = State {
            preview: Some(Generated {
                items: vec![song("first"), song("last")],
                // One row per lane in `Dimension::ALL` order, each holding a
                // level per song: the opener at the low end of every axis, the
                // closer at the high end.
                levels: vec![vec![-1.8, 1.8]; Dimension::ALL.len()],
                blended: vec![-1.8, 1.8],
                matches: vec![
                    Match {
                        similarity: 0.4,
                        ticks: 3,
                    },
                    Match {
                        similarity: 0.3,
                        ticks: 2,
                    },
                ],
                eligible_tracks: 260,
                ..Generated::default()
            }),
            ..State::default()
        };
        state.set_shape(Shape::DEFAULT);

        assert_eq!(
            state.row_is(0),
            ["quiet", "slow", "dark", "steady", "clean"],
            "the opening song reads as the low end of every axis"
        );
        assert_eq!(
            state.row_is(1),
            ["loud", "fast", "bright", "swinging", "noisy"],
            "and the closing song as the high end"
        );

        // The line was drawn rising, so what it *asked* for travels the same
        // way — and that is the claim a listener can check.
        assert_eq!(
            state.row_asked(0),
            ["quiet", "slow", "dark", "steady", "clean"]
        );
        assert_eq!(
            state.row_asked(1),
            ["loud", "fast", "bright", "swinging", "noisy"]
        );

        let why = state.why(0).expect("a selected row explains itself");
        assert!(why.contains("Your words let it in"), "{why}");
        assert!(why.contains("260 eligible"), "{why}");
        assert!(
            why.contains("asked for quiet, slow, dark, steady and clean"),
            "{why}"
        );
        // Asked and got agree here, so it says so in one clause rather than
        // repeating the same five words twice.
        assert!(why.contains("and this song is."), "{why}");
        assert!(!why.contains("0.4"), "a reading, never a score: {why}");

        // **A song in the middle of every axis says nothing**, rather than
        // being called dark for sitting a hair below the middle.
        let middling = State {
            preview: Some(Generated {
                items: vec![song("one")],
                levels: vec![vec![0.2]; Dimension::ALL.len()],
                blended: vec![0.2],
                ..Generated::default()
            }),
            ..State::default()
        };
        assert!(middling.row_is(0).is_empty(), "{:?}", middling.row_is(0));
    }

    #[test]
    fn the_request_survives_consent_and_preview_edits_are_in_memory() {
        let mut state = State::default();
        state.set_prompt("warm brass becoming urgent, then calm");
        state.set_length(MixLength::NinetyMinutes);
        state.begin_request();
        assert!(state.open && state.awaiting_create);
        assert_eq!(state.prompt, "warm brass becoming urgent, then calm");
        assert_eq!(state.length, MixLength::NinetyMinutes);

        let item = |title: &str| QueueItemVm {
            title: title.to_owned(),
            artist: None,
            album: None,
            album_artist: None,
            duration: Some(std::time::Duration::from_secs(180)),
            path: PathBuf::from(format!("/{title}.flac")),
        };
        state.preview = Some(Generated {
            description: "request".to_owned(),
            request: "warm brass becoming urgent, then calm".to_owned(),
            items: vec![item("one"), item("two"), item("three")],
            pool_tracks: 3,
            analyzed_tracks: 3,
            eligible_tracks: 3,
            target_minutes: 90,
            ..Generated::default()
        });
        assert!(!state.request_changed());
        state.set_length(MixLength::Hour);
        assert!(state.request_changed());
        state.shift_preview(2, -1);
        state.remove_preview(0);
        let preview = state.preview.expect("preview remains local");
        assert_eq!(
            preview
                .items
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            ["three", "two"]
        );
    }

    #[cfg(feature = "vibe-analysis")]
    #[test]
    fn decoder_internals_stay_out_of_the_listener_facing_failure() {
        let path = PathBuf::from("/mount/a/very/private/06 Theme Libre.mp3");
        let error = baz_vibe::Error::Decode {
            path: path.clone(),
            detail: "malformed stream: mpa: invalid main_data_offset".to_owned(),
        };
        let message = friendly_analysis_error(&path, &error);
        assert!(message.contains("06 Theme Libre"));
        assert!(!message.contains("/mount/"));
        assert!(!message.contains("main_data_offset"));
        assert!(message.contains("retry"));
    }
}
