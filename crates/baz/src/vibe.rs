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
const RECENTLY_OFFERED_CAP: usize = PLAYLIST_CAP * 2;

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

/// One dimension, and the shape asked of it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Lane {
    pub(crate) dimension: Dimension,
    pub(crate) points: Vec<ContourPoint>,
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
    /// The most points one line may carry. Six is four turns — more shape
    /// than a playlist of tens of tracks can express.
    const MAX_POINTS: usize = 6;
    /// **The most lines at once.** Every line is another thing the library
    /// must satisfy *simultaneously*, and a request nothing can answer is
    /// worse than a coarse one: three is enough to say something specific
    /// while leaving the collection room to answer.
    pub(crate) const MAX_LANES: usize = 3;

    /// One line over one dimension.
    pub(crate) fn of(dimension: Dimension, points: Vec<ContourPoint>) -> Self {
        Self {
            lanes: vec![Lane { dimension, points }],
        }
    }

    pub(crate) fn lane(&self, index: usize) -> Option<&Lane> {
        self.lanes.get(index)
    }

    pub(crate) fn has(&self, dimension: Dimension) -> bool {
        self.lanes.iter().any(|lane| lane.dimension == dimension)
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

    /// Add a turn where there is most room for one, at the level the line
    /// already stands at there — so the shape does not jump when it gains a
    /// handle.
    pub(crate) fn add_point(&mut self, lane: usize) {
        let Some(points) = self.lanes.get(lane).map(|lane| lane.points.clone()) else {
            return;
        };
        if points.len() >= Self::MAX_POINTS || points.len() < 2 {
            return;
        }
        let Some((index, at)) = points
            .windows(2)
            .enumerate()
            .max_by(|left, right| {
                (left.1[1].at - left.1[0].at).total_cmp(&(right.1[1].at - right.1[0].at))
            })
            .map(|(index, pair)| (index, f32::midpoint(pair[0].at, pair[1].at)))
        else {
            return;
        };
        let level = level_at(&points, at).unwrap_or(0.0);
        if let Some(lane) = self.lanes.get_mut(lane) {
            lane.points.insert(index + 1, ContourPoint { at, level });
        }
    }

    /// Take the last turn back out. The two ends are the line and never go.
    pub(crate) fn remove_point(&mut self, lane: usize) {
        if let Some(lane) = self.lanes.get_mut(lane)
            && lane.points.len() > 2
        {
            let index = lane.points.len() - 2;
            lane.points.remove(index);
        }
    }

    /// Give a dimension a line of its own, at the same shape the first lane
    /// carries — a second line that started flat would look like a mistake,
    /// and one that started at a random shape would be one.
    pub(crate) fn add_lane(&mut self, dimension: Dimension) {
        if self.has(dimension) || self.lanes.len() >= Self::MAX_LANES {
            return;
        }
        let points = self
            .lanes
            .first()
            .map_or_else(|| Shape::DEFAULT.points(), |lane| lane.points.clone());
        self.lanes.push(Lane { dimension, points });
    }

    /// Take a dimension's line away, leaving it unconstrained. The first lane
    /// stays: a contour with no lines at all is `Any`, which is a *shape*
    /// choice made in the shape row rather than by emptying the control.
    pub(crate) fn remove_lane(&mut self, dimension: Dimension) {
        if self.lanes.len() > 1 {
            self.lanes.retain(|lane| lane.dimension != dimension);
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

/// The preview before it becomes a normal playlist file.
#[derive(Debug, Clone)]
pub(crate) struct Generated {
    pub(crate) description: String,
    pub(crate) request: String,
    pub(crate) items: Vec<QueueItemVm>,
    /// Where each chosen track landed, **one row per drawn line** in lane
    /// order, each holding a level per track in listening order — the result
    /// in the request's own units, so each line can draw what it got over
    /// what it asked for.
    pub(crate) levels: Vec<Vec<f32>>,
    pub(crate) pool_tracks: usize,
    pub(crate) analyzed_tracks: usize,
    pub(crate) tempo_span: Option<(f32, f32)>,
    pub(crate) target_minutes: u64,
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
    recently_offered: Vec<PathBuf>,
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
    features: HashMap<PathBuf, SonicFeatures>,
    pending: VecDeque<PathBuf>,
    recently_offered: VecDeque<PathBuf>,
    run: u64,
    variation: u64,
    active_workers: usize,
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
            contour: Contour::of(Dimension::Energy, Shape::DEFAULT.points()),
            field: HashMap::new(),
            hovered_row: None,
            features: HashMap::new(),
            pending: VecDeque::new(),
            recently_offered: VecDeque::new(),
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

    /// How many tracks the local index holds features for.
    pub(crate) fn analyzed(&self) -> usize {
        self.features.len()
    }

    /// The pointer entered or left one row of the preview.
    pub(crate) fn hover_row(&mut self, row: Option<usize>) {
        self.hovered_row = row;
    }

    /// **Load a named shape onto every line.** A shape is a shape: asking for
    /// `Peak and fall` with tempo and brightness drawn means both of them
    /// peak and fall, which is what the picture then shows. Lines are shaped
    /// apart by dragging them apart.
    pub(crate) fn set_shape(&mut self, shape: Shape) {
        let points = shape.points();
        if points.is_empty() {
            self.contour.lanes.clear();
            return;
        }
        if self.contour.lanes.is_empty() {
            self.contour = Contour::of(Dimension::Energy, points);
            return;
        }
        for lane in &mut self.contour.lanes {
            lane.points.clone_from(&points);
        }
    }

    /// Move one point of one line, by the widget's raw geometry.
    pub(crate) fn drag_contour(&mut self, lane: usize, index: usize, at: f32, level: f32) {
        self.contour.drag(lane, index, at, level);
    }

    /// Whether a line can gain or lose a turn, so the two controls can be
    /// inert rather than absent at the ends of their range.
    pub(crate) fn can_add_point(&self, lane: usize) -> bool {
        self.contour
            .lane(lane)
            .is_some_and(|lane| (2..Contour::MAX_POINTS).contains(&lane.points.len()))
    }

    pub(crate) fn can_remove_point(&self, lane: usize) -> bool {
        self.contour
            .lane(lane)
            .is_some_and(|lane| lane.points.len() > 2)
    }

    pub(crate) fn add_contour_point(&mut self, lane: usize) {
        self.contour.add_point(lane);
    }

    pub(crate) fn remove_contour_point(&mut self, lane: usize) {
        self.contour.remove_point(lane);
    }

    /// Give a dimension a line of its own, or take its line away.
    pub(crate) fn toggle_dimension(&mut self, dimension: Dimension) {
        if self.contour.has(dimension) {
            self.contour.remove_lane(dimension);
        } else {
            self.contour.add_lane(dimension);
        }
    }

    /// Whether another line can be drawn at all.
    pub(crate) fn can_add_lane(&self) -> bool {
        self.contour.lanes.len() < Contour::MAX_LANES
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
            let engine = match dimension {
                Dimension::Energy => baz_vibe::Dimension::Energy,
                Dimension::Tempo => baz_vibe::Dimension::Tempo,
                Dimension::Brightness => baz_vibe::Dimension::Brightness,
                Dimension::Dynamics => baz_vibe::Dimension::Dynamics,
                Dimension::Texture => baz_vibe::Dimension::Texture,
            };
            let values: Vec<f32> = self
                .features
                .values()
                .map(|features| features.value(engine))
                .collect();
            // The same rank scale the engine scores against: a track's place
            // in the collection, not its fraction of the distance between the
            // two most extreme records.
            let mut sorted = values.clone();
            sorted.sort_by(f32::total_cmp);
            #[expect(
                clippy::cast_precision_loss,
                reason = "a library's track count is far below f32's exact-integer range"
            )]
            let level = |value: f32| {
                if sorted.is_empty() {
                    return 0.0;
                }
                let below = sorted.partition_point(|held| *held < value);
                let through = sorted.partition_point(|held| *held <= value);
                let rank = ((below + through) as f32 / 2.0) / sorted.len() as f32;
                rank.clamp(0.0, 1.0)
                    .mul_add(2.0 * LEVEL_LIMIT, -LEVEL_LIMIT)
            };
            let field = field_of(values.iter().map(|value| level(*value)));
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
                self.recently_offered = prepared.recently_offered.into();
                self.total = self.features.len() + self.pending.len();
                self.done = self.features.len();
                self.analyzing = !self.pending.is_empty();
                self.error = None;
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
        self.prompt = prompt.chars().take(240).collect();
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
        self.length = length;
    }

    pub(crate) fn create(
        &mut self,
        index: Option<&Path>,
        albums: &[AlbumVm],
        chosen: &HashMap<u64, EditionKey>,
    ) {
        self.open = true;
        self.awaiting_create = false;
        #[cfg(not(feature = "vibe-analysis"))]
        let _ = index;
        let recently_offered = self.recently_offered.iter().cloned().collect();
        let request = self.effective_request();
        let generated = generate(
            &request,
            &self.contour,
            self.length,
            self.variation,
            &recently_offered,
            &self.features,
            albums,
            chosen,
        );
        self.variation = self.variation.wrapping_add(1);
        let preview = match generated {
            Ok(preview) => {
                self.error = None;
                preview
            }
            Err(error) => {
                self.error = Some(error);
                None
            }
        };
        if let Some(preview) = &preview {
            for item in &preview.items {
                self.recently_offered.retain(|path| path != &item.path);
                self.recently_offered.push_back(item.path.clone());
            }
            while self.recently_offered.len() > RECENTLY_OFFERED_CAP {
                self.recently_offered.pop_front();
            }
            #[cfg(feature = "vibe-analysis")]
            {
                if let Some(index) = index
                    && let Err(error) = baz_vibe::remember_offered(
                        index,
                        &preview
                            .items
                            .iter()
                            .map(|item| item.path.clone())
                            .collect::<Vec<_>>(),
                    )
                {
                    self.error = Some(format!("Could not retain mix freshness: {error}"));
                }
            }
        }
        self.preview = preview;
    }

    pub(crate) fn another(
        &mut self,
        index: Option<&Path>,
        albums: &[AlbumVm],
        chosen: &HashMap<u64, EditionKey>,
    ) {
        self.create(index, albums, chosen);
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
            recently_offered: prepared.recently_offered,
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
                dimension: match lane.dimension {
                    Dimension::Energy => baz_vibe::Dimension::Energy,
                    Dimension::Tempo => baz_vibe::Dimension::Tempo,
                    Dimension::Brightness => baz_vibe::Dimension::Brightness,
                    Dimension::Dynamics => baz_vibe::Dimension::Dynamics,
                    Dimension::Texture => baz_vibe::Dimension::Texture,
                },
                points: lane
                    .points
                    .iter()
                    .map(|point| baz_vibe::ContourPoint {
                        at: point.at,
                        level: point.level,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(feature = "vibe-analysis")]
#[expect(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "candidate projection, duration convergence and result construction form one generation boundary"
)]
fn generate(
    prompt: &str,
    contour: &Contour,
    length: MixLength,
    variation: u64,
    recently_offered: &HashSet<PathBuf>,
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
    let mut best = None;
    for _ in 0..4 {
        // **The words choose the pool; the shape chooses the walk.** Either
        // may be absent, and `select_contour` is the one selector both the
        // older entry points are written in terms of — so a request with no
        // line still behaves exactly as the free-text one always did.
        let selection = baz_vibe::select_contour(
            prompt,
            &engine_contour(contour),
            &candidates,
            limit,
            variation,
            recently_offered,
        )
        .map_err(|error| error.to_string())?;
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
    let levels: Vec<Vec<f32>> = selection
        .levels
        .iter()
        .map(|lane| {
            kept.iter()
                .filter_map(|&index| lane.get(index).copied())
                .collect()
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
        pool_tracks,
        analyzed_tracks: selection.pool_tracks,
        tempo_span: selection.tempo_span,
        target_minutes: length.minutes(),
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
    _recently_offered: &HashSet<PathBuf>,
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
        state.add_contour_point(0);
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

    /// **A turn arrives where there is room for it, at the level the line
    /// already stands at** — so gaining a handle changes the shape by
    /// nothing, and the listener can drag it deliberately rather than
    /// recovering from a jump.
    #[test]
    fn a_new_turn_lands_on_the_line_it_joins() {
        let mut state = State::default();
        state.set_shape(Shape::DEFAULT);
        let before: Vec<f32> = (0..=10)
            .map(|step| {
                state
                    .contour
                    .level_at(0, f32::from(u8::try_from(step).unwrap_or(0)) / 10.0)
                    .expect("a line")
            })
            .collect();
        state.add_contour_point(0);
        for (step, level) in before.iter().enumerate() {
            let at = f32::from(u8::try_from(step).unwrap_or(0)) / 10.0;
            let after = state.contour.level_at(0, at).expect("still a line");
            assert!((after - level).abs() < 0.001, "the shape moved at {at}");
        }
        // The ends never go, however many times the control is pressed.
        for _ in 0..8 {
            state.remove_contour_point(0);
        }
        assert_eq!(points(&state).len(), 2);
        assert!(!state.can_remove_point(0));
        // …and the cap holds at the other end.
        for _ in 0..8 {
            state.add_contour_point(0);
        }
        assert_eq!(points(&state).len(), Contour::MAX_POINTS);
        assert!(!state.can_add_point(0));
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
            let drawn = Contour::of(Dimension::Energy, shape.points());
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
                        (ours - theirs).abs() < 0.0001,
                        "{} disagrees at {at}: drawn {ours}, scored {theirs}",
                        shape.label
                    ),
                    _ => panic!("{} is a line on one side only at {at}", shape.label),
                }
            }
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
            levels: Vec::new(),
            pool_tracks: 3,
            analyzed_tracks: 3,
            tempo_span: None,
            target_minutes: 90,
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
