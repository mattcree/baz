//! Home's opt-in, local sonic-playlist state.
//!
//! The full build delegates decoding, MIR extraction, persistence and ranking
//! to the optional `baz-vibe` crate. A light build retains the same Home seam
//! but contains no analyzer dependency or model/runtime payload.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::vm::{self, AlbumVm, EditionKey, QueueItemVm};

/// An ordinary generated mix is deliberately bounded and editable.
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

    /// What it is measured from, plainly enough to put on screen.
    pub(crate) const fn measured_from(self) -> &'static str {
        match self {
            Self::Energy => "tempo, loudness, and how much the loudness moves",
            Self::Tempo => "beats per minute",
            Self::Brightness => "spectral centroid, rolloff and zero crossings",
            Self::Dynamics => "how much the loudness moves within a track",
            Self::Texture => "spectral flatness — tonal against noisy",
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

    pub(crate) fn lane(&self, index: usize) -> Option<&Lane> {
        self.lanes.get(index)
    }

    /// The level one lane asks for at `fraction`.
    pub(crate) fn level_at(&self, lane: usize, fraction: f32) -> Option<f32> {
        level_at(&self.lanes.get(lane)?.points, fraction)
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

/// **One word of the vocabulary** — a way of writing the request, never a
/// second input beside it. Pressing one appends it to the line with a comma.
///
/// There is no language model here. The text tower answers *descriptive
/// phrases about sound*: "slow sparse piano, melancholy" retrieves, "songs
/// about my ex" retrieves noise. The vocabulary is the answer to that, and it
/// is a route rather than a rule — telling somebody to describe the sound and
/// not the story, without giving them the words, is a scold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Chip {
    /// Which row it sits in.
    pub(crate) row: &'static str,
    /// The word itself, exactly as it reaches the model.
    pub(crate) word: &'static str,
}

impl Chip {
    /// **The twelve, in two rows, each chosen by measurement** —
    /// `docs/design/impl/vibe-eligibility/`, finding 6. Twenty-seven
    /// candidates were scored on how far appending them moves a real request's
    /// pool *towards the chip's own meaning*, over five ordinary starting
    /// phrases. These twelve are the ones that did.
    ///
    /// **Design 21 §4 asked for three rows and the numbers refused one.** A
    /// *moves like* row was measured too — `slow`, `driving`, `hypnotic`,
    /// `sparse`, `danceable` and four more. Every one of them displaced the
    /// pool heavily and pulled it almost nowhere: its best chip scored 0.046
    /// against the *made of* row's 0.142, and two of its nine were at or below
    /// zero. Appending an adjective to a five-word request scrambles the
    /// embedding rather than steering it, and instrumentation words survive
    /// that because they name something the audio tower can hear. The row also
    /// duplicated the question the curve asks directly beneath it.
    ///
    /// If it is ever wanted back, the thing to change is not this list: it is
    /// that movement words should steer the **curve** — press *driving*, get a
    /// shape — rather than be appended to a sentence that then means something
    /// else.
    pub(crate) const ALL: [Self; 12] = [
        Self {
            row: "made of",
            word: "acoustic guitar",
        },
        Self {
            row: "made of",
            word: "synthesizers",
        },
        Self {
            row: "made of",
            word: "piano",
        },
        Self {
            row: "made of",
            word: "strings",
        },
        Self {
            row: "made of",
            word: "electric guitars",
        },
        Self {
            row: "made of",
            word: "female vocals",
        },
        Self {
            row: "feels like",
            word: "hopeful",
        },
        Self {
            row: "feels like",
            word: "warm",
        },
        Self {
            row: "feels like",
            word: "dark",
        },
        Self {
            row: "feels like",
            word: "melancholy",
        },
        Self {
            row: "feels like",
            word: "dreamy",
        },
        Self {
            row: "feels like",
            word: "tense",
        },
    ];

    /// The rows, in the order the band draws them.
    pub(crate) const ROWS: [&'static str; 2] = ["made of", "feels like"];
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
    /// When the words have been still long enough to be worth embedding.
    /// `None` means there is nothing waiting.
    count_due: Option<std::time::Instant>,
    /// **Whether the per-dimension lines are open.** Kept rather than derived
    /// from whether the curves differ, because *open and identical* is a real
    /// state: it is what the expander shows the moment it is pressed, and it
    /// is the whole of design 21 §5's claim that the lines were already the
    /// blend.
    pub(crate) expanded: bool,
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
            contour: Contour::blended(&Shape::DEFAULT.points()),
            field: HashMap::new(),
            hovered_row: None,
            selected_row: None,
            live: None,
            counting: false,
            varied: false,
            count_due: None,
            expanded: false,
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
        let placed = format!("{} of {}", row + 1, preview.items.len());
        // With no line drawn there is no height to report, and the honest
        // sentence is the shorter one rather than an invented percentile.
        let where_it_went = preview.blended.get(row).copied().map_or_else(
            || format!("You drew no line, so the order is continuity alone; it is {placed}."),
            |level| {
                // A level is a rank stretched onto −2…+2, so this is the rank
                // back.
                let percentile = ((level + 2.0) / 4.0 * 100.0).clamp(0.0, 100.0).round();
                format!(
                    "Your line put it {placed} — louder, faster and busier than \
                     {percentile:.0}% of this request's songs."
                )
            },
        );
        let Some(found) = preview.matches.get(row) else {
            return Some(format!(
                "You asked for no words, so every song Baz has heard was eligible. \
                 {where_it_went}"
            ));
        };
        let strength = match found.ticks {
            3 => "one of the strongest matches",
            2 => "a fair match",
            _ => "a weak match — your line asked for something your words did not have much of",
        };
        Some(format!(
            "Your words let it in: {strength} of the {} eligible. {where_it_went}",
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

    /// **Open or close the per-dimension lines.**
    ///
    /// Closing puts every line back on the first one's curve, which is the
    /// only way back to one line once they have been pulled apart — and it is
    /// lossy, which is why it says *back to one line* rather than *close*.
    pub(crate) fn toggle_expander(&mut self) {
        self.expanded = !self.expanded;
        if !self.expanded {
            let Some(points) = self.contour.lane(0).map(|lane| lane.points.clone()) else {
                return;
            };
            for lane in &mut self.contour.lanes {
                lane.points.clone_from(&points);
            }
        }
    }

    /// Which recipe the request currently matches, if any — so the row can
    /// light the one you started from and stop lighting it the moment you
    /// change the words.
    pub(crate) fn recipe(&self) -> Option<usize> {
        Recipe::ALL.iter().position(|recipe| {
            self.prompt == recipe.prompt
                && self.length == recipe.length
                && self
                    .contour
                    .lane(0)
                    .is_some_and(|lane| lane.points == recipe.shape().points())
        })
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
        if self.expanded {
            self.contour.drag(lane, index, at, level);
            return;
        }
        // While it is one line, it is five lanes holding one curve, and
        // dragging it drags all of them — otherwise the blend would silently
        // stop being a blend at the first gesture.
        for held in 0..self.contour.lanes.len() {
            self.contour.drag(held, index, at, level);
        }
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
        self.prompt = prompt;
    }

    /// **Appending a word from the vocabulary**, with a comma — design 21 §4's
    /// rule, and the one thing a chip does.
    pub(crate) fn append_word(&mut self, word: &str) {
        let existing = self.prompt.trim_end().trim_end_matches(',').to_owned();
        let joined = if existing.is_empty() {
            word.to_owned()
        } else {
            format!("{existing}, {word}")
        };
        self.set_prompt(&joined);
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

    #[cfg(not(feature = "vibe-analysis"))]
    fn cloud_of<'a>(&self, _members: impl Iterator<Item = &'a Path>) -> Vec<f32> {
        Vec::new()
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
        self.prompt.trim().to_owned()
    }

    pub(crate) fn set_length(&mut self, length: MixLength) {
        self.length_touched = true;
        self.length = length;
    }

    /// **Compose.**
    ///
    /// Deterministic: the same request composed twice returns the identical
    /// list. It did not used to be — the seed advanced on every press and a
    /// freshness penalty pushed recently offered tracks away — and that is why
    /// the diff below can now state a cause and always be right. *"Identical,
    /// because nothing changed"* has to be true before it is worth saying.
    pub(crate) fn create(&mut self, albums: &[AlbumVm], chosen: &HashMap<u64, EditionKey>) {
        self.open = true;
        self.awaiting_create = false;
        let request = self.effective_request();
        let generated = generate(
            &request,
            &self.contour,
            self.length,
            self.variation,
            &self.features,
            albums,
            chosen,
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

    /// **Another version**: the same request, a different draw.
    ///
    /// The visible press that carries the power the old auto-incrementing seed
    /// took invisibly. It is a distinct act, and the diff names it as the
    /// cause, so variation is something the listener asked for rather than
    /// something that happened to them.
    pub(crate) fn another(&mut self, albums: &[AlbumVm], chosen: &HashMap<u64, EditionKey>) {
        self.variation = self.variation.wrapping_add(1);
        self.varied = true;
        self.create(albums, chosen);
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
        changed("another version: the same request, a different draw")
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
    features: &HashMap<PathBuf, SonicFeatures>,
    albums: &[AlbumVm],
    chosen: &HashMap<u64, EditionKey>,
) -> Result<Option<Generated>, String> {
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
    _features: &HashMap<PathBuf, SonicFeatures>,
    _albums: &[AlbumVm],
    _chosen: &HashMap<u64, EditionKey>,
) -> Result<Option<Generated>, String> {
    Ok(None)
}

/// Name shown for the current seed without exposing a full path in Home.
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
        let mut state = State::default();
        state.set_shape(Shape::DEFAULT);
        // The ends hold their positions however far the pointer wanders.
        state.drag_contour(0, 0, 0.9, 0.5);
        assert!((points(&state)[0].at - 0.0).abs() < f32::EPSILON);
        assert!((points(&state)[0].level - 0.5).abs() < f32::EPSILON);
        let last = points(&state).len() - 1;
        state.drag_contour(0, last, 0.1, -0.5);
        assert!((points(&state)[last].at - 1.0).abs() < f32::EPSILON);

        // Levels clamp to the collection's own ends rather than running off
        // the top of the box.
        state.drag_contour(0, 0, 0.0, 9.0);
        assert!((points(&state)[0].level - LEVEL_LIMIT).abs() < f32::EPSILON);
        state.drag_contour(0, 0, 0.0, -9.0);
        assert!((points(&state)[0].level + LEVEL_LIMIT).abs() < f32::EPSILON);

        // An interior turn stays between its neighbours, with a gap either
        // side: two points at one position would ask for two levels at once.
        // The turn comes from a preset now rather than from a stepper.
        state.set_shape(Shape::ALL[3]);
        assert_eq!(points(&state).len(), 3);
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
            assert_eq!(drawn, shape.points(), "{} did not load", shape.label);
            if index == 0 {
                assert!(drawn.is_empty(), "Any is no line at all");
                continue;
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
                let ours = drawn.level_at(0, at);
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

    /// **A recipe fills the form and leaves it editable**, which is the whole
    /// of what makes it a starting point rather than a mode.
    #[test]
    fn a_recipe_fills_the_request_and_stops_claiming_it_the_moment_it_changes() {
        let mut state = State::default();
        assert_eq!(
            state.recipe(),
            None,
            "nothing is a recipe until one is pressed"
        );
        for (index, recipe) in Recipe::ALL.iter().enumerate() {
            state.start_from(*recipe);
            assert_eq!(state.prompt, recipe.prompt);
            assert_eq!(state.length, recipe.length);
            assert_eq!(points(&state), recipe.shape().points());
            assert_eq!(
                state.recipe(),
                Some(index),
                "{} does not recognise itself",
                recipe.label
            );
        }
        // Change any one of the three and it is the listener's request.
        state.start_from(Recipe::ALL[0]);
        state.set_prompt("something else entirely");
        assert_eq!(state.recipe(), None);
        state.start_from(Recipe::ALL[0]);
        state.drag_contour(0, 0, 0.0, 1.9);
        assert_eq!(state.recipe(), None);
        state.start_from(Recipe::ALL[0]);
        state.set_length(MixLength::TwoHours);
        assert_eq!(state.recipe(), None);

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
            contour: Contour::blended(&Shape::DEFAULT.points()),
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

        // …and the visible press names itself rather than hiding behind the
        // request, which is exactly what the old auto-incrementing seed did.
        let drawn = generated("warm brass", &["c", "d"], 300, 60);
        let sentence = diff(&same, &drawn, true).cause;
        assert!(sentence.contains("another version"), "{sentence}");
    }

    /// **A row explains itself as a rank, never a score** — the quorum's R9.
    #[test]
    fn a_selected_row_explains_itself_without_a_number_nobody_asked_for() {
        let mut state = State {
            preview: Some(Generated {
                items: vec![QueueItemVm {
                    title: "One".to_owned(),
                    artist: None,
                    album: None,
                    album_artist: None,
                    duration: None,
                    path: PathBuf::from("/one.flac"),
                }],
                blended: vec![1.12],
                matches: vec![Match {
                    similarity: 0.41,
                    ticks: 3,
                }],
                eligible_tracks: 260,
                ..Generated::default()
            }),
            ..State::default()
        };
        let why = state.why(0).expect("a selected row explains itself");
        assert!(why.contains("Your words let it in"), "{why}");
        assert!(why.contains("260 eligible"), "{why}");
        assert!(why.contains("78%"), "{why}");
        assert!(!why.contains("0.41"), "a rank, never a score: {why}");

        // Selecting the same row again puts the explanation away.
        state.select_row(0);
        assert_eq!(state.selected_row, Some(0));
        state.select_row(0);
        assert_eq!(state.selected_row, None);
    }

    /// **A chip appends; it never replaces.** Design 21 §4's rules table.
    #[test]
    fn the_vocabulary_appends_to_the_one_request() {
        let mut state = State::default();
        state.append_word("piano");
        assert_eq!(state.prompt, "piano");
        state.append_word("melancholy");
        assert_eq!(state.prompt, "piano, melancholy");
        state.set_prompt("warm brass,");
        state.append_word("dark");
        assert_eq!(state.prompt, "warm brass, dark");
        // Every chip is in a row the band actually draws.
        for chip in Chip::ALL {
            assert!(Chip::ROWS.contains(&chip.row), "{}", chip.word);
        }
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
