//! Optional, completely local musical analysis for baz.
//!
//! This crate is deliberately outside the player and GUI crates. It decodes a
//! file through baz-core's offline decoder, extracts conventional music-
//! information-retrieval features with bliss, and owns a replaceable SQLite
//! cache. Nothing here is reachable from the realtime playback thread.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use baz_core::playback::{AudioSource, resample_interleaved};
use bliss_audio::{Analysis, AnalysisIndex, FeaturesVersion, Song};
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;

mod semantic;

/// Sample rate required by bliss' feature extractors.
const ANALYSIS_RATE: u32 = 22_050;
/// Stereo channels produced by baz-core's offline decoder.
const CHANNELS: usize = 2;
/// Schema of the independent, disposable analysis cache.
///
/// Version 3 is version 2 minus a promise. `recent_offers` held a freshness
/// history that biased every generated playlist away from tracks recently
/// offered — invisibly, against weights summing to under one, at a penalty of
/// 2.0, which made it a ban rather than a tiebreak. Design 21 §4 says *"no
/// hidden state, nothing accumulating out of sight"*, and a diff sentence
/// reading *"you changed nothing"* over a changed list would have been the
/// first thing a listener saw. The table is no longer created and no longer
/// read; an existing one is left where it is, inert, because a disposable
/// cache does not need a migration to stop using a column.
const STORE_VERSION: i64 = 3;

/// A conventional local description of one track.
#[derive(Debug, Clone, PartialEq)]
pub struct Features {
    /// Normalized bliss feature vector. Its version travels beside it in the
    /// store, so incompatible analyzer upgrades are never mixed.
    values: Vec<f32>,
    semantic: Vec<f32>,
}

/// Listener steering applied to a sonic candidate pool. Zero means that an
/// axis is unconstrained; the two signs select opposite ends of the
/// collection-relative range.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    /// -2 calm … +2 energetic.
    pub energy: i8,
    /// -2 warm/dark … +2 bright/crisp.
    pub brightness: i8,
    /// Optional sonic anchor from the same analyzed library.
    pub seed: Option<PathBuf>,
}

impl Profile {
    /// Whether the listener has supplied any meaningful steering.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.energy == 0 && self.brightness == 0 && self.seed.is_none()
    }
}

/// **One musical dimension a contour can be drawn over.**
///
/// Each is a *stated combination* of the analyser's own features, never a
/// learned label: baz can measure how fast, how loud, how bright and how
/// varied a recording is, and it cannot measure how a record feels. Naming
/// the combination is what keeps the control honest — a listener who asks for
/// `Brightness` is asking for spectral centroid, rolloff and zero crossings,
/// and is entitled to know it.
///
/// The owner: *"can we have more than one of these for different musical
/// dimensions — this obviously kinda rolls up several aspects of a song into
/// one value."* [`Dimension::Energy`] is that roll-up, kept because it is
/// what most people mean by *a mix that builds*; the others are its parts and
/// its neighbours, each on its own line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    /// Tempo, loudness and how much the loudness moves — the composite.
    Energy,
    /// Tempo alone. The one dimension with an absolute reading a listener
    /// already has a word for: beats per minute.
    Tempo,
    /// Zero crossings, spectral centroid and rolloff: dark and warm at one
    /// end, bright and crisp at the other.
    Brightness,
    /// How much the loudness moves within the track — steady at one end,
    /// swinging at the other.
    Dynamics,
    /// Spectral flatness: tonal at one end, noisy at the other.
    Texture,
}

impl Dimension {
    /// Every dimension, in the order a surface should offer them.
    pub const ALL: [Self; 5] = [
        Self::Energy,
        Self::Tempo,
        Self::Brightness,
        Self::Dynamics,
        Self::Texture,
    ];

    /// The dimension's own name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Energy => "Energy",
            Self::Tempo => "Tempo",
            Self::Brightness => "Brightness",
            Self::Dynamics => "Dynamics",
            Self::Texture => "Texture",
        }
    }

    /// What the two ends of its axis are, low first — the words a surface
    /// puts beside the line.
    #[must_use]
    pub const fn ends(self) -> (&'static str, &'static str) {
        match self {
            Self::Energy => ("calmer", "louder"),
            Self::Tempo => ("slower", "faster"),
            Self::Brightness => ("darker", "brighter"),
            Self::Dynamics => ("steadier", "swingier"),
            Self::Texture => ("cleaner", "noisier"),
        }
    }

    /// What it is measured from, said plainly enough to put in an interface.
    #[must_use]
    pub const fn measured_from(self) -> &'static str {
        match self {
            Self::Energy => "tempo, loudness and how much the loudness moves",
            Self::Tempo => "beats per minute",
            Self::Brightness => "spectral centroid, rolloff and zero crossings",
            Self::Dynamics => "how much the loudness moves within a track",
            Self::Texture => "spectral flatness — tonal against noisy",
        }
    }
}

/// **One point on a contour**: how far through the playlist, and the level the
/// music should be at when it gets there.
///
/// `at` is a fraction of the finished list, `0.0` for the opening track and
/// `1.0` for the last. `level` is the same collection-relative −2…+2 scale
/// [`Profile`] uses, so a contour and a profile mean the same thing by the
/// same number: −2 is the calm (or dark) end of *this* library, +2 the
/// energetic (or bright) end, and the middle is the middle of what the
/// listener actually owns rather than an absolute the analysis cannot know.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourPoint {
    /// Position through the playlist, 0…1.
    pub at: f32,
    /// Target level at that position, −2…+2.
    pub level: f32,
}

/// **One line of a contour**: a dimension, the shape asked of it, and how much
/// it counts against the others.
#[derive(Debug, Clone, PartialEq)]
pub struct Lane {
    /// What this line is about.
    pub dimension: Dimension,
    /// Its points, in order of position through the playlist.
    pub points: Vec<ContourPoint>,
    /// **How much this line counts**, against the other lines of the same
    /// request.
    ///
    /// One line drawn alone can leave this at 1.0 and nothing changes. It
    /// exists for the blend: design 21 §5 asks for one default line standing
    /// for every dimension at once, and says it must be a **weighted** mean
    /// with energy dominant, because every dimension here is a rank within
    /// the collection — so an unweighted mean puts loud-and-slow in the same
    /// place as quiet-and-fast, and a line drawn through the middle would be
    /// satisfied by tracks that sound nothing alike.
    pub weight: f32,
}

impl Lane {
    /// One line, counting for one.
    #[must_use]
    pub fn new(dimension: Dimension, points: Vec<ContourPoint>) -> Self {
        Self {
            dimension,
            points,
            weight: 1.0,
        }
    }
}

/// **The shape a generated playlist is asked to follow** — a line per
/// dimension, read at every position in the list.
///
/// A dimension with no lane is *unconstrained*, which is not the same as a
/// lane pinned to the middle: unconstrained means it does not enter the cost
/// at all, so a contour drawn over energy alone lets brightness fall wherever
/// the music does. That distinction is why this is a list of lanes rather
/// than a fixed set of levels.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contour {
    /// The lines, in the order a surface draws them.
    pub lanes: Vec<Lane>,
}

impl Contour {
    /// **The weights of the blended line**, in [`Dimension::ALL`] order,
    /// energy dominant.
    ///
    /// They sum to one, which is what makes [`Contour::blended`] consistent:
    /// give every dimension the same curve and the weighted mean *is* that
    /// curve, whatever the weights are. Design 21 §5 predicted that somebody
    /// would later simplify this away without knowing why it held, so it is
    /// pinned by `a_blend_of_one_curve_is_that_curve` rather than only
    /// written down.
    pub const BLEND: [f32; 5] = [0.40, 0.20, 0.15, 0.15, 0.10];

    /// A contour of one line, counting for one.
    #[must_use]
    pub fn of(dimension: Dimension, points: Vec<ContourPoint>) -> Self {
        Self {
            lanes: vec![Lane::new(dimension, points)],
        }
    }

    /// **The default request's line**: every dimension asked for the same
    /// shape, weighted with energy dominant.
    ///
    /// The listener sees one line. It is five, holding one curve between
    /// them, which is why opening design 21 §5's expander changes nothing
    /// until a line is dragged apart from its neighbours: the expander does
    /// not *seed* the per-dimension curves from the blend, it reveals that
    /// they were already the blend.
    #[must_use]
    pub fn blended(points: &[ContourPoint]) -> Self {
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

    /// A contour that steers nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lanes.iter().all(|lane| lane.points.is_empty())
    }

    /// The level a line asks for at `fraction`, or `None` where the line is
    /// unconstrained.
    ///
    /// Points are read in the order given and the line is straight between
    /// them; before the first point and after the last it holds that point's
    /// level, so a contour is defined everywhere rather than only between its
    /// ends.
    #[must_use]
    pub fn level_at(points: &[ContourPoint], fraction: f32) -> Option<f32> {
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

    /// Every lane's target at one position, in lane order, with the weight it
    /// counts for.
    fn targets_at(&self, fraction: f32) -> Vec<(Dimension, Option<f32>, f32)> {
        self.lanes
            .iter()
            .map(|lane| {
                (
                    lane.dimension,
                    Self::level_at(&lane.points, fraction),
                    lane.weight,
                )
            })
            .collect()
    }
}

/// A library track decorated only with the identity needed for diversity.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Exact path and stable join back to Baz's library projection.
    pub path: PathBuf,
    /// Album identity; generated lists take one per album before repeats.
    pub album: u64,
    /// Human artist identity used to prevent adjacent/repeated domination.
    pub artist: String,
    /// Cached sonic features.
    pub features: Features,
}

/// **How well one chosen track answered the words**, in the two forms a
/// surface needs: the number, and the bucket it is drawn as.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Match {
    /// Cosine against the request embedding.
    pub similarity: f32,
    /// Which third of the eligible pool it sits in.
    pub strength: Strength,
}

/// **Three buckets of match strength**, never more.
///
/// Three, because the underlying cosines drift with the phrase and with the
/// library, and a picture that changes when the numbers drift is a picture
/// that cannot be read. The boundaries are the *pool's own terciles*, decided
/// by measurement in `docs/design/impl/vibe-eligibility/`: absolute cosine
/// boundaries would show three ticks on every row of one request and one tick
/// on every row of another, which is drift wearing a hat.
///
/// A weak tick is not a failure to hide. It says the line asked for something
/// the words did not have much of, which is true and useful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    /// The bottom third of the eligible pool.
    Weak,
    /// The middle third.
    Fair,
    /// The top third.
    Strong,
}

impl Strength {
    /// How many of three ticks to fill.
    #[must_use]
    pub const fn ticks(self) -> u8 {
        match self {
            Self::Weak => 1,
            Self::Fair => 2,
            Self::Strong => 3,
        }
    }
}

/// Ranked and sequenced sonic result.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Paths in listening order.
    pub paths: Vec<PathBuf>,
    /// Every analysed track the request was offered.
    pub analysed_tracks: usize,
    /// **How many of them the words let in** — the eligible set, and the
    /// number design 21 §6's live count states. With no words this is every
    /// analysed track, which is the truth rather than a special case.
    pub eligible_tracks: usize,
    /// Tempo span of the selected tracks, rounded only by the UI.
    pub tempo_span: Option<(f32, f32)>,
    /// Where each chosen track sits on the −2…+2 axes the request was made
    /// on: one row per [`Lane`], in lane order, each holding a level per
    /// chosen track in listening order.
    ///
    /// It is the *result* in the request's own units, which is what lets a
    /// surface draw what it got over what it asked for instead of asking the
    /// listener to take the answer on faith.
    pub levels: Vec<Vec<f32>>,
    /// [`Self::levels`] collapsed by the lanes' own weights — one level per
    /// chosen track, which is what the blended line draws its dots at.
    pub blended: Vec<f32>,
    /// **The eligible songs, on the same axes** — one row per lane, each
    /// holding a level for every track the words let in.
    ///
    /// This is design 21 §6's cloud: narrow the phrase and watch it thin out
    /// under the curve. It is the eligible set rather than the library, which
    /// is the whole of what makes it a picture of cause and effect.
    pub cloud: Vec<Vec<f32>>,
    /// [`Self::cloud`] collapsed by the lanes' own weights.
    pub blended_cloud: Vec<f32>,
    /// How well each chosen track answered the words, in listening order.
    /// Empty for a request with no words, because a shape-only request has no
    /// match strength and drawing one would be an invention.
    pub matches: Vec<Match>,
}

impl Features {
    /// Estimated beats per minute, derived from bliss' documented 0–206 BPM
    /// normalization.
    #[must_use]
    pub fn tempo_bpm(&self) -> f32 {
        (self.values[AnalysisIndex::Tempo as usize] + 1.0) * 103.0
    }

    /// A collection-relative input for energy ranking. This is intentionally
    /// a transparent combination, not a learned emotion label.
    #[must_use]
    pub fn energy(&self) -> f32 {
        mean(&[
            self.at(AnalysisIndex::Tempo),
            self.at(AnalysisIndex::MeanLoudness),
            self.at(AnalysisIndex::StdDeviationLoudness),
        ])
    }

    /// A spectral brightness input: high-frequency crossings, centroid and
    /// rolloff. The collection normalizes it before listener controls use it.
    #[must_use]
    pub fn brightness(&self) -> f32 {
        mean(&[
            self.at(AnalysisIndex::Zcr),
            self.at(AnalysisIndex::MeanSpectralCentroid),
            self.at(AnalysisIndex::MeanSpectralRolloff),
        ])
    }

    /// **One dimension's reading for this track**, before the collection
    /// ranks it. Every one is a stated combination of the analyser's own
    /// features — see [`Dimension`] — and none of them is a mood.
    #[must_use]
    pub fn value(&self, dimension: Dimension) -> f32 {
        match dimension {
            Dimension::Energy => self.energy(),
            Dimension::Tempo => self.at(AnalysisIndex::Tempo),
            Dimension::Brightness => self.brightness(),
            Dimension::Dynamics => self.at(AnalysisIndex::StdDeviationLoudness),
            Dimension::Texture => self.at(AnalysisIndex::MeanSpectralFlatness),
        }
    }

    /// Euclidean distance in the complete normalized feature space.
    #[must_use]
    pub fn distance(&self, other: &Self) -> f32 {
        self.values
            .iter()
            .zip(&other.values)
            .map(|(left, right)| (left - right).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// **How well this track answers an embedded request** — the cosine of the
    /// two unit vectors, −1…1, higher being closer.
    ///
    /// It is the one number the eligible set is drawn from, so it is public:
    /// a surface that wants to say *matches 340 songs* while the listener is
    /// still typing must be able to ask the same question the selector asks,
    /// and get the same answer.
    #[must_use]
    pub fn similarity(&self, request: &[f32]) -> f32 {
        self.semantic
            .iter()
            .zip(request)
            .map(|(left, right)| left * right)
            .sum::<f32>()
    }

    fn semantic_distance(&self, other: &[f32]) -> f32 {
        1.0 - self.similarity(other)
    }

    fn semantic_pair_distance(&self, other: &Self) -> f32 {
        self.semantic_distance(&other.semantic)
    }

    fn at(&self, index: AnalysisIndex) -> f32 {
        self.values[index as usize]
    }
}

/// **Embed an ordinary-language request** with the bundled local text tower.
///
/// The one embedding the whole feature is built on. It is separated from
/// selection because a surface needs it *before* a request is committed — to
/// count what matches while the listener is still typing — and because paying
/// for the tower once per keystroke-settled phrase, rather than once per
/// compose, is what makes that count affordable.
///
/// # Errors
///
/// Returns an inference error if the bundled model cannot embed the prompt.
pub fn embed_request(prompt: &str) -> Result<Vec<f32>, Error> {
    semantic::embed_text(prompt.trim()).map_err(Error::Semantic)
}

/// **The songs the words let in** — the eligible set, and the first of the two
/// stages selection now has.
///
/// Design 21 §3 says the words decide *which* songs are eligible and the line
/// decides *where* each one goes. Until this existed that was a metaphor: the
/// walk scored one blended cost over every analysed track, so a poor
/// word-match could win a slot by sitting at the right height, and moving the
/// line changed which tracks were even in the room. A pool is what makes the
/// two sentences true — *the words let it in; the line put it fourth* — and it
/// is what the match count, the cloud and the ticks are all readings of.
///
/// The policy is the **knee** of the ranked similarity curve, chosen by the
/// sweep in `docs/design/impl/vibe-eligibility/`: a fixed cosine floor is
/// unusable because the distribution moves wholesale with the phrase (a floor
/// that keeps 3 749 tracks for one request keeps one for another), and a
/// top-K-per-cent cut answers the same number for every phrase anybody types,
/// which makes the count a decoration. The knee's relevance matches
/// top-K-per-cent's at matched pool size and its size responds to the words,
/// which is the whole point of drawing it.
#[derive(Debug, Clone, Default)]
pub struct Pool {
    /// Indices into the candidates, best match first.
    ranked: Vec<usize>,
    /// Each ranked track's cosine, in the same order.
    similarities: Vec<f32>,
    /// Whether words drew this pool at all.
    from_words: bool,
}

/// The knee is searched only within this share of the ranking: past it, a bend
/// is the tail's noise rather than the end of the answer.
const KNEE_HORIZON: f32 = 0.25;
/// …and never cuts above this many tracks, so a pool always has room for a
/// playlist and its diversity rules.
const KNEE_FLOOR: usize = 24;
/// Below this many analysed tracks a ranking has no distribution to find a
/// knee in, and the honest pool is everything there is.
const KNEE_MINIMUM_LIBRARY: usize = KNEE_FLOOR * 4;

impl Pool {
    /// How many songs the words let in.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ranked.len()
    }

    /// Whether nothing at all is eligible.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranked.is_empty()
    }

    /// The eligible candidates, best match first.
    #[must_use]
    pub fn ranked(&self) -> &[usize] {
        &self.ranked
    }

    /// The best matches first, as `(candidate index, cosine)` — what design
    /// 21 §6's *closest three* is read off, because a count says how many and
    /// never how well.
    pub fn closest(&self, count: usize) -> impl Iterator<Item = (usize, f32)> + '_ {
        self.ranked
            .iter()
            .copied()
            .zip(self.similarities.iter().copied())
            .take(count)
    }

    /// Which third of the pool a candidate sits in, or `None` where it is not
    /// eligible — or where there were no words, in which case there is no
    /// match strength to report and inventing one would be a lie.
    #[must_use]
    pub fn strength(&self, candidate: usize) -> Option<Strength> {
        if !self.from_words {
            return None;
        }
        let place = self.ranked.iter().position(|index| *index == candidate)?;
        let third = self.ranked.len().div_ceil(3).max(1);
        Some(match place / third {
            0 => Strength::Strong,
            1 => Strength::Fair,
            _ => Strength::Weak,
        })
    }

    /// A candidate's cosine against the request, where it is eligible.
    #[must_use]
    pub fn similarity(&self, candidate: usize) -> Option<f32> {
        let place = self.ranked.iter().position(|index| *index == candidate)?;
        self.similarities.get(place).copied()
    }
}

/// **Draw the eligible set** for an embedded request over an analysed pool.
///
/// With no words every analysed track is eligible, which is the truth about a
/// shape-only request rather than a special case: nothing was asked of the
/// words, so the words exclude nothing.
#[must_use]
pub fn eligible(request: Option<&[f32]>, candidates: &[Candidate]) -> Pool {
    let Some(request) = request else {
        return Pool {
            ranked: (0..candidates.len()).collect(),
            similarities: vec![0.0; candidates.len()],
            from_words: false,
        };
    };
    let mut ranked: Vec<usize> = (0..candidates.len()).collect();
    let similarities: Vec<f32> = candidates
        .iter()
        .map(|candidate| candidate.features.similarity(request))
        .collect();
    ranked.sort_by(|left, right| {
        similarities[*right]
            .total_cmp(&similarities[*left])
            .then_with(|| left.cmp(right))
    });
    let sorted: Vec<f32> = ranked.iter().map(|index| similarities[*index]).collect();
    let kept = knee(&sorted);
    ranked.truncate(kept);
    Pool {
        similarities: ranked.iter().map(|index| similarities[*index]).collect(),
        ranked,
        from_words: true,
    }
}

/// **How many of a ranked similarity list the words let in** — the same
/// policy [`eligible`] applies, over nothing but the numbers.
///
/// Separated so a live count can be paid for in a few million multiply-adds
/// against vectors already in memory, rather than by projecting the whole
/// library into candidates once per settled phrase. The count on screen and
/// the pool a compose walks are then the same rule reading the same ranking,
/// which is the only way *"matches 340 songs"* can be a promise.
///
/// `sorted_descending` must be sorted, highest first; nothing else is assumed.
#[must_use]
pub fn eligible_count(sorted_descending: &[f32]) -> usize {
    knee(sorted_descending)
}

/// **Where the answer stops falling steeply**: the ranked curve's furthest
/// point below the chord joining the two ends of the search window.
///
/// Not the largest single gap. A decaying curve's biggest step is almost
/// always at its head, so a largest-gap rule reliably answers *"the top"*
/// whatever the phrase — measured, and it pinned all eighteen swept prompts to
/// the smallest pool they were allowed. The chord distance asks the question
/// that was meant, and is invariant to how steep the head happens to be.
fn knee(sorted_descending: &[f32]) -> usize {
    let count = sorted_descending.len();
    if count < KNEE_MINIMUM_LIBRARY {
        return count;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a bounded share of a library count"
    )]
    let horizon = ((count as f32 * KNEE_HORIZON) as usize)
        .max(KNEE_FLOOR + 1)
        .min(count - 1);
    let fall = sorted_descending[0] - sorted_descending[horizon];
    if fall <= f32::EPSILON {
        return count;
    }
    let mut best = (KNEE_FLOOR, f32::MIN);
    for index in KNEE_FLOOR..horizon {
        #[expect(clippy::cast_precision_loss, reason = "bounded library counts")]
        let chord = fall.mul_add(-(index as f32 / horizon as f32), sorted_descending[0]);
        let below = chord - sorted_descending[index];
        if below > best.1 {
            best = (index, below);
        }
    }
    best.0
}

/// Select tracks near the requested collection-relative targets, then order
/// the shortlist by local sonic continuity while enforcing artist and album
/// diversity. Retrieval and sequencing are deliberately separate: nearest
/// neighbours alone make repetitive, poorly flowing playlists.
#[must_use]
pub fn select(profile: &Profile, candidates: &[Candidate], limit: usize) -> Selection {
    select_journey(std::slice::from_ref(profile), candidates, limit, 0)
}

/// Select and sequence a position-aware journey through two or more sonic
/// profiles. Targets are linearly interpolated over playlist position; the
/// optional variation seed perturbs only close choices, preserving the named
/// journey and diversity rules while allowing an explicit “another version”.
#[must_use]
pub fn select_journey(
    profiles: &[Profile],
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
) -> Selection {
    if profiles.is_empty() || profiles.iter().all(Profile::is_empty) {
        return Selection {
            analysed_tracks: candidates.len(),
            eligible_tracks: candidates.len(),
            ..Selection::default()
        };
    }
    walk(
        &Request {
            semantic: None,
            contour: &profiles_as_contour(profiles),
            seed: profiles.iter().find_map(|profile| profile.seed.as_ref()),
        },
        &eligible(None, candidates),
        candidates,
        limit,
        variation,
    )
}

/// Retrieve and sequence tracks for an ordinary-language musical request.
/// Text and audio are embedded by the paired bundled CLAP towers, and all
/// ranking remains local.
///
/// # Errors
///
/// Returns an inference error if the bundled model cannot embed the prompt.
pub fn select_semantic(
    prompt: &str,
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
) -> Result<Selection, Error> {
    select_contour(prompt, &Contour::default(), candidates, limit, variation)
}

/// **The words choose the pool; the shape chooses the walk** — the one
/// selector the other two are written in terms of.
///
/// Two stages, and the order of them is the design. `prompt` draws the
/// eligible set through the bundled CLAP towers ([`eligible`]); `contour`
/// then walks **within that set only**, asking each position in the finished
/// list for a level on the pool's own axes. Either may be absent: with no
/// contour this is retrieval ordered by continuity, and with no prompt the
/// pool is the whole analysed library and the shape does all the choosing.
///
/// What changed when this stopped being one blended cost, and why it had to:
/// relevance and fit used to *trade against each other*, at 0.45 against
/// 0.30, so a track could win a slot by sitting at the right height however
/// poorly it answered the words — which is how a lullaby became eligible for
/// a workout. Relevance no longer buys position. It decides membership, and
/// inside the pool it is only a tiebreak.
///
/// The diversity rules — an artist twice at most, never twice in a row, a
/// fresh album while one is available — are the same in all three, because
/// they are properties of *a playlist* rather than of how it was asked for.
///
/// # Errors
///
/// Returns an inference error if the bundled model cannot embed the prompt.
pub fn select_contour(
    prompt: &str,
    contour: &Contour,
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
) -> Result<Selection, Error> {
    let prompt = prompt.trim();
    if (prompt.is_empty() && contour.is_empty()) || candidates.is_empty() || limit == 0 {
        return Ok(Selection {
            analysed_tracks: candidates.len(),
            eligible_tracks: candidates.len(),
            ..Selection::default()
        });
    }
    let semantic = if prompt.is_empty() {
        None
    } else {
        Some(embed_request(prompt)?)
    };
    let pool = eligible(semantic.as_deref(), candidates);
    Ok(compose(
        semantic.as_deref(),
        &pool,
        contour,
        candidates,
        limit,
        variation,
    ))
}

/// **The second stage on its own**: walk a shape through an eligible set that
/// has already been drawn.
///
/// Separated from [`select_contour`] because a caller converging on a target
/// listening time walks the same pool several times with different lengths,
/// and re-embedding the prompt once per attempt would pay for the text tower
/// four times to answer one press. The pool is what is expensive to think
/// about; the walk is cheap.
#[must_use]
pub fn compose(
    request: Option<&[f32]>,
    pool: &Pool,
    contour: &Contour,
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
) -> Selection {
    walk(
        &Request {
            semantic: request,
            contour,
            seed: None,
        },
        pool,
        candidates,
        limit,
        variation,
    )
}

/// **Where every candidate sits on one dimension's axis** — −2…+2, in the
/// order given.
///
/// The scale is pool-relative by construction: the given pool's own ranking
/// stretched onto −2…+2, which is the same mapping the fit scores against. A
/// surface can therefore draw the eligible songs behind a lane, and the chosen
/// tracks over it, without holding a second opinion about what *energetic*
/// means.
#[must_use]
pub fn levels(candidates: &[Candidate], dimension: Dimension) -> Vec<f32> {
    let axis = Axis::of(
        candidates
            .iter()
            .map(|candidate| candidate.features.value(dimension)),
    );
    candidates
        .iter()
        .map(|candidate| axis.level(candidate.features.value(dimension)))
        .collect()
}

/// **How much of the pool the walk may hold in the room at once**, per track
/// it will place.
///
/// With words this never fires: an eligible set is a few hundred tracks and
/// the walk simply chooses from all of it, which is what makes *"moving the
/// line reorders rather than re-selects"* literally true rather than nearly
/// true. It exists for the shape-only request, whose pool is the whole
/// analysed library and which cannot afford a scoring pass over tens of
/// thousands of tracks per position.
const SHORTLIST_PER_TRACK: usize = 32;

/// Where a shaped request retrieves from: evenly along the list, one sample
/// per track up to eight. Eight is more turns than a contour can have (the
/// interface caps it at six points) so no part of a shape goes
/// unrepresented, and it bounds the shortlist pass at eight scorings of the
/// pool.
fn shortlist_samples(limit: usize) -> Vec<f32> {
    let count = limit.clamp(1, 8);
    #[expect(
        clippy::cast_precision_loss,
        reason = "the sample count is at most eight"
    )]
    (0..count)
        .map(|step| {
            if count == 1 {
                0.0
            } else {
                step as f32 / (count - 1) as f32
            }
        })
        .collect()
}

/// What one request asks of the pool: words, a shape, and an optional anchor.
struct Request<'a> {
    semantic: Option<&'a [f32]>,
    contour: &'a Contour,
    seed: Option<&'a PathBuf>,
}

/// **How the cost inside the pool is split.**
///
/// Note what is *not* here any more: a three-way trade in which relevance and
/// fit bid against each other. That trade is what made a lullaby eligible for
/// a workout — at 0.45 relevance against 0.30 fit, a track that answered the
/// words poorly could still take a slot by standing at the right height — and
/// it is exactly the thing design 21 §3 promised could not happen.
///
/// Membership is now the words' job and position is the line's, so relevance
/// does not buy position. It stays in the cost at a weight small enough to be
/// only what it should be: a tiebreak between two tracks the line likes
/// equally, both of which the words already let in.
struct Weights {
    relevance: f32,
    fit: f32,
    continuity: f32,
}

impl Weights {
    const VARIATION: f32 = 0.05;

    const fn for_request(shape: bool) -> Self {
        if shape {
            // The line decides where in the pool to be; relevance breaks ties
            // and continuity keeps the walk from lurching between neighbours.
            Self {
                relevance: 0.05,
                fit: 0.70,
                continuity: 0.25,
            }
        } else {
            // No line: nothing asks for a position, so the pool's own order is
            // the order, softened by continuity.
            Self {
                relevance: 0.72,
                fit: 0.0,
                continuity: 0.28,
            }
        }
    }
}

/// Retrieval, position-aware fit, diversity and sequencing — one auditable
/// policy, walked once per generated playlist.
#[expect(
    clippy::too_many_lines,
    reason = "one pass over one policy: shortlist, then a diversity-constrained walk"
)]
fn walk(
    request: &Request<'_>,
    pool: &Pool,
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
) -> Selection {
    if pool.is_empty() || limit == 0 {
        return Selection {
            analysed_tracks: candidates.len(),
            eligible_tracks: pool.len(),
            ..Selection::default()
        };
    }
    // **One rank axis per dimension the request mentions, over the eligible
    // set** — not over the library.
    //
    // Plan 22 §1.3, decision 4. *How should it move* means *how should the
    // music you asked for move*: the axis words stay true within the request,
    // the line is always fillable, and a phrase whose whole answer is quiet
    // still has a top and a bottom to climb between. The cost is that the same
    // drawn line means a different absolute loudness for different phrases —
    // but the axis was already collection-relative and never absolute, so this
    // narrows what it is relative *to* rather than changing its kind.
    let members = pool.ranked();
    let axes = Axes::over(
        request.contour.lanes.iter().map(|lane| lane.dimension),
        members,
        candidates,
    );
    let seed = request.seed.and_then(|path| {
        candidates
            .iter()
            .find(|candidate| &candidate.path == path)
            .map(|candidate| &candidate.features)
    });
    let max_seed_distance = seed.map_or(1.0, |seed| {
        members
            .iter()
            .map(|index| seed.distance(&candidates[*index].features))
            .fold(0.0_f32, f32::max)
            .max(f32::EPSILON)
    });
    let transition_scale = seed.map_or_else(
        || {
            candidates.first().map_or(1.0, |candidate| {
                let dimensions = u16::try_from(candidate.features.values.len()).unwrap_or(u16::MAX);
                2.0 * f32::from(dimensions).sqrt()
            })
        },
        |_| max_seed_distance,
    );
    let weights = Weights::for_request(!request.contour.is_empty() || seed.is_some());
    let relevance = |candidate: &Candidate| {
        request
            .semantic
            .map_or(0.0, |target| candidate.features.semantic_distance(target))
    };
    let fit_at = |candidate: &Candidate, fraction: f32| {
        target_fit(
            candidate,
            &request.contour.targets_at(fraction),
            &axes,
            seed,
            max_seed_distance,
        )
    };
    // **Ordinarily the room *is* the pool.**
    //
    // An eligible set is a few hundred tracks, so the walk chooses from all of
    // it and moving the line cannot change which tracks are present — which is
    // what design 21 §3's answer to *"why did my change do nothing?"* asserts
    // and what invariant I3 checks.
    //
    // The narrowing below fires only for a request with no words at all, whose
    // pool is the whole analysed library. There, one global ranking would not
    // do: if the best few thousand all sit at one height no walk over them can
    // climb, which is exactly what the owner saw — *"the little dots seem to
    // all be more or less in a line and not following my line."* So a shaped
    // request narrows **per position**: the curve is sampled, each sample
    // takes its own best few, and the union is the room, so every height the
    // line asks for has candidates in it.
    let shortlist_len = limit.saturating_mul(SHORTLIST_PER_TRACK).max(limit);
    let mut scored: Vec<(usize, f32)> = if shortlist_len >= members.len() {
        members.iter().map(|index| (*index, 0.0)).collect()
    } else if request.contour.is_empty() {
        let mut scored: Vec<(usize, f32)> = members
            .iter()
            .map(|index| {
                (
                    *index,
                    weights
                        .relevance
                        .mul_add(relevance(&candidates[*index]), 0.0),
                )
            })
            .collect();
        scored.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        scored.truncate(shortlist_len);
        scored
    } else {
        let samples = shortlist_samples(limit);
        let per_sample = shortlist_len.div_ceil(samples.len().max(1));
        let mut taken: HashSet<usize> = HashSet::new();
        let mut shortlist: Vec<(usize, f32)> = Vec::with_capacity(shortlist_len);
        for fraction in samples {
            let mut at_position: Vec<(usize, f32)> = members
                .iter()
                .filter(|index| !taken.contains(*index))
                .map(|index| {
                    let candidate = &candidates[*index];
                    (
                        *index,
                        weights.relevance.mul_add(
                            relevance(candidate),
                            weights.fit * fit_at(candidate, fraction),
                        ),
                    )
                })
                .collect();
            at_position.sort_by(|left, right| {
                left.1
                    .total_cmp(&right.1)
                    .then_with(|| left.0.cmp(&right.0))
            });
            for entry in at_position.into_iter().take(per_sample) {
                taken.insert(entry.0);
                shortlist.push(entry);
            }
        }
        shortlist
    };

    let mut chosen: Vec<usize> = Vec::with_capacity(limit.min(scored.len()));
    let mut chosen_paths = HashSet::new();
    let mut artist_counts: HashMap<&str, usize> = HashMap::new();
    let mut album_counts: HashMap<u64, usize> = HashMap::new();
    while chosen.len() < limit && !scored.is_empty() {
        #[expect(
            clippy::cast_precision_loss,
            reason = "playlist positions are bounded to tens of tracks"
        )]
        let fraction = chosen.len() as f32 / limit.saturating_sub(1).max(1) as f32;
        let previous = chosen.last().map(|&index| &candidates[index].features);
        let last_artist = chosen
            .last()
            .map(|&index| candidates[index].artist.as_str());
        let require_fresh_album = scored
            .iter()
            .any(|(index, _)| !album_counts.contains_key(&candidates[*index].album));
        let next = scored
            .iter()
            .enumerate()
            .filter(|(_, (index, _))| {
                let candidate = &candidates[*index];
                artist_counts
                    .get(candidate.artist.as_str())
                    .copied()
                    .unwrap_or(0)
                    < 2
                    && last_artist != Some(candidate.artist.as_str())
                    && !chosen_paths.contains(&candidate.path)
                    && (!require_fresh_album || !album_counts.contains_key(&candidate.album))
            })
            .min_by(|(_, (left_index, _)), (_, (right_index, _))| {
                let cost = |index: usize| {
                    let candidate = &candidates[index];
                    // Continuity is measured in the same space the request is
                    // made in: two tracks a text request considers alike, or
                    // two tracks whose whole feature vectors are close.
                    let continuity = previous.map_or(0.0, |previous| {
                        if request.semantic.is_some() {
                            previous.semantic_pair_distance(&candidate.features)
                        } else {
                            previous.distance(&candidate.features) / transition_scale
                        }
                    });
                    weights.relevance * relevance(candidate)
                        + weights.fit * fit_at(candidate, fraction)
                        + weights.continuity * continuity
                        + Weights::VARIATION * variation_noise(&candidate.path, variation)
                };
                cost(*left_index)
                    .total_cmp(&cost(*right_index))
                    .then_with(|| left_index.cmp(right_index))
            })
            .map(|(position, _)| position);
        let Some(next) = next else {
            break;
        };
        let (index, _) = scored.remove(next);
        *artist_counts
            .entry(candidates[index].artist.as_str())
            .or_default() += 1;
        *album_counts.entry(candidates[index].album).or_default() += 1;
        chosen_paths.insert(candidates[index].path.clone());
        chosen.push(index);
    }
    let mut tempos = chosen
        .iter()
        .map(|&index| candidates[index].features.tempo_bpm());
    let tempo_span = tempos.next().map(|first| {
        tempos.fold((first, first), |(low, high), tempo| {
            (low.min(tempo), high.max(tempo))
        })
    });
    // One row per lane of the request, each holding that lane's level for
    // every chosen track — the result in the request's own units, lane by
    // lane, so a surface can draw each line's dots over its own line.
    let level_of = |index: usize, lane: &Lane| {
        axes.level(
            lane.dimension,
            candidates[index].features.value(lane.dimension),
        )
    };
    let levels: Vec<Vec<f32>> = request
        .contour
        .lanes
        .iter()
        .map(|lane| chosen.iter().map(|&index| level_of(index, lane)).collect())
        .collect();
    let cloud: Vec<Vec<f32>> = request
        .contour
        .lanes
        .iter()
        .map(|lane| members.iter().map(|&index| level_of(index, lane)).collect())
        .collect();
    let blend = |rows: &[Vec<f32>], count: usize| blended(&request.contour.lanes, rows, count);
    Selection {
        blended: blend(&levels, chosen.len()),
        blended_cloud: blend(&cloud, members.len()),
        // The engine says how well each chosen track answered the words, in
        // both the forms a surface needs, so that a tick on a row is a reading
        // of what selection did rather than a second opinion computed in the
        // view.
        matches: chosen
            .iter()
            .filter_map(|&index| {
                Some(Match {
                    similarity: pool.similarity(index)?,
                    strength: pool.strength(index)?,
                })
            })
            .collect(),
        levels,
        cloud,
        paths: chosen
            .into_iter()
            .map(|index| candidates[index].path.clone())
            .collect(),
        analysed_tracks: candidates.len(),
        eligible_tracks: members.len(),
        tempo_span,
    }
}

/// **The lanes collapsed by their own weights** — one level per column of
/// `rows`, which is what the single blended line draws.
///
/// The arithmetic lives here rather than in a view because design 21 §5's
/// consistency claim is a property of the engine's own numbers: give every
/// lane the same curve and this returns that curve, whatever the weights are.
fn blended(lanes: &[Lane], rows: &[Vec<f32>], count: usize) -> Vec<f32> {
    let total: f32 = lanes.iter().map(|lane| lane.weight).sum();
    if rows.is_empty() || total <= f32::EPSILON {
        return Vec::new();
    }
    (0..count)
        .map(|column| {
            lanes
                .iter()
                .zip(rows)
                .map(|(lane, row)| lane.weight * row.get(column).copied().unwrap_or(0.0))
                .sum::<f32>()
                / total
        })
        .collect()
}

/// The profiles a journey names, as the contour they always described: one
/// point per profile, evenly spaced, on the axes the profile constrains.
///
/// A profile's `0` means *unconstrained* — its own doc says so — so a zero
/// contributes no point rather than a point at the middle. That is the one
/// place the two vocabularies differ, and it is resolved here rather than in
/// the walk.
fn profiles_as_contour(profiles: &[Profile]) -> Contour {
    let mut energy = Vec::new();
    let mut brightness = Vec::new();
    #[expect(
        clippy::cast_precision_loss,
        reason = "a journey has only a handful of visible waypoints"
    )]
    let at = |index: usize| {
        if profiles.len() <= 1 {
            0.0
        } else {
            index as f32 / (profiles.len() - 1) as f32
        }
    };
    for (index, profile) in profiles.iter().enumerate() {
        if profile.energy != 0 {
            energy.push(ContourPoint {
                at: at(index),
                level: f32::from(profile.energy),
            });
        }
        if profile.brightness != 0 {
            brightness.push(ContourPoint {
                at: at(index),
                level: f32::from(profile.brightness),
            });
        }
    }
    let mut lanes = Vec::new();
    if !energy.is_empty() {
        lanes.push(Lane::new(Dimension::Energy, energy));
    }
    if !brightness.is_empty() {
        lanes.push(Lane::new(Dimension::Brightness, brightness));
    }
    Contour { lanes }
}

/// How badly a candidate misses what this position asked for.
///
/// An axis with no target does not enter the average, which is what makes an
/// unconstrained line cost nothing rather than pull everything to the middle.
/// Each axis counts for its lane's own weight, which is what makes the blended
/// default line a *weighted* mean with energy dominant rather than the plain
/// average design 21 §5 rejects. A seed, where one is given, is worth one and
/// a half axes: it is a whole feature vector rather than a single number.
fn target_fit(
    candidate: &Candidate,
    targets: &[(Dimension, Option<f32>, f32)],
    axes: &Axes,
    seed: Option<&Features>,
    max_seed_distance: f32,
) -> f32 {
    let mut score = 0.0_f32;
    let mut weights = 0.0_f32;
    for (dimension, target, weight) in targets {
        if let Some(target) = target {
            score +=
                weight * axes.distance(*dimension, candidate.features.value(*dimension), *target);
            weights += weight;
        }
    }
    if let Some(seed) = seed {
        score += 1.5 * seed.distance(&candidate.features) / max_seed_distance;
        weights += 1.5;
    }
    score / weights.max(1.0)
}

/// One rank axis per dimension a request mentions, built once per walk.
struct Axes {
    axes: HashMap<Dimension, Axis>,
}

impl Axes {
    /// Build the axes the request needs, over the eligible set it will choose
    /// from — `members` indexes `candidates`.
    fn over(
        dimensions: impl Iterator<Item = Dimension>,
        members: &[usize],
        candidates: &[Candidate],
    ) -> Self {
        let mut axes = HashMap::new();
        for dimension in dimensions {
            axes.entry(dimension).or_insert_with(|| {
                Axis::of(
                    members
                        .iter()
                        .map(|index| candidates[*index].features.value(dimension)),
                )
            });
        }
        Self { axes }
    }

    fn distance(&self, dimension: Dimension, value: f32, level: f32) -> f32 {
        self.axes
            .get(&dimension)
            .map_or(0.0, |axis| axis.distance(value, level))
    }

    fn level(&self, dimension: Dimension, value: f32) -> f32 {
        self.axes
            .get(&dimension)
            .map_or(0.0, |axis| axis.level(value))
    }
}

/// Where a raw feature value sits on the −2…+2 scale a [`Contour`] is drawn
/// on — the inverse of the mapping `Axis::distance` compares against,
/// so a level drawn and a level scored are the same number.
/// **One axis of the collection, as places rather than as a span.**
///
/// A level is the *rank* of a value within the analysed pool — the median
/// track sits at 0, the quietest at −2, the loudest at +2 — and not its
/// fraction of the distance between the two most extreme records.
///
/// The difference is the whole feature working or not. Loudness and tempo
/// cluster hard: a real library has a handful of outliers at each end and
/// everything else packed in the middle, so a min–max axis maps almost every
/// track to within a whisker of the centre. Ask such an axis for a rising
/// line and the tracks that answer it are *all near the middle*, which is
/// exactly what the owner saw: *"the little dots seem to all be more or less
/// in a line and not following my line."* On a rank axis the collection is
/// spread across the scale by construction, so `+1` means "in the livelier
/// quarter of what you own" and there is always something there to pick.
struct Axis {
    sorted: Vec<f32>,
}

impl Axis {
    fn of(values: impl Iterator<Item = f32>) -> Self {
        let mut sorted: Vec<f32> = values.collect();
        sorted.sort_by(f32::total_cmp);
        Self { sorted }
    }

    /// Where `value` stands in the pool, `0.0..=1.0`. Ties take the middle of
    /// the run they belong to, so a collection whose values are all identical
    /// reads as the middle rather than as one end.
    fn rank(&self, value: f32) -> f32 {
        if self.sorted.is_empty() {
            return 0.5;
        }
        let below = self.sorted.partition_point(|held| *held < value);
        let through = self.sorted.partition_point(|held| *held <= value);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a library's track count is far below f32's exact-integer range"
        )]
        let position = (below + through) as f32 / 2.0;
        #[expect(
            clippy::cast_precision_loss,
            reason = "a library's track count is far below f32's exact-integer range"
        )]
        let total = self.sorted.len() as f32;
        (position / total).clamp(0.0, 1.0)
    }

    /// The −2…+2 level `value` sits at.
    fn level(&self, value: f32) -> f32 {
        self.rank(value).mul_add(4.0, -2.0)
    }

    /// How far `value` is from a requested level, in ranks.
    fn distance(&self, value: f32, level: f32) -> f32 {
        let target = (level.clamp(-2.0, 2.0) + 2.0) / 4.0;
        (self.rank(value) - target).abs()
    }
}

fn variation_noise(path: &Path, seed: u64) -> f32 {
    if seed == 0 {
        return 0.0;
    }
    let hash = path_bytes(path)
        .iter()
        .fold(seed ^ 0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    #[expect(
        clippy::cast_precision_loss,
        reason = "the low 24 bits intentionally become a stable unit interval"
    )]
    {
        (hash & 0x00ff_ffff) as f32 / 0x00ff_ffff_u64 as f32
    }
}

fn mean(values: &[f32]) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "this averages exactly three feature axes, far below f32's integer precision"
    )]
    let count = values.len() as f32;
    values.iter().sum::<f32>() / count
}

/// One analyzed path returned by a worker task.
#[derive(Debug, Clone, PartialEq)]
pub struct Analyzed {
    /// Exact library path.
    pub path: PathBuf,
    /// Extracted local features.
    pub features: Features,
}

/// Existing usable rows and paths that still require analysis.
#[derive(Debug, Clone, Default)]
pub struct Prepared {
    /// Current cached features, keyed by exact path.
    pub ready: HashMap<PathBuf, Features>,
    /// Missing or stale paths, in the caller's library order.
    pub pending: Vec<PathBuf>,
}

/// Analysis/cache failure. A single unreadable track is recoverable by the
/// caller and must not invalidate features already extracted for other files.
#[derive(Debug, Error)]
pub enum Error {
    /// The source file could not be inspected.
    #[error("could not inspect {path}: {source}")]
    Inspect {
        /// Source path.
        path: PathBuf,
        /// Filesystem error.
        source: std::io::Error,
    },
    /// Baz's hardened offline decoder refused the track.
    #[error("could not decode {path}: {detail}")]
    Decode {
        /// Source path.
        path: PathBuf,
        /// Decoder diagnostic.
        detail: String,
    },
    /// Feature extraction refused the decoded samples.
    #[error("could not analyse {path}: {detail}")]
    Analyze {
        /// Source path.
        path: PathBuf,
        /// Analyzer diagnostic.
        detail: String,
    },
    /// The bundled semantic model or its local inference failed.
    #[error("semantic analysis: {0}")]
    Semantic(String),
    /// The independent analysis cache could not be read or written.
    #[error("analysis cache: {0}")]
    Store(#[from] rusqlite::Error),
    /// A newer Baz wrote this disposable cache; an older build must not stamp
    /// it backwards or guess at its layout.
    #[error("analysis cache schema {found} is newer than supported schema {supported}")]
    UnsupportedStoreVersion {
        /// Version read without modifying the database.
        found: i64,
        /// Latest version understood by this build.
        supported: i64,
    },
    /// The cache row was structurally invalid and is treated as absent.
    #[error("analysis cache contains an invalid feature row")]
    InvalidRow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    bytes: u64,
    modified_ns: i64,
}

impl Stamp {
    fn read(path: &Path) -> Result<Self, Error> {
        let metadata = std::fs::metadata(path).map_err(|source| Error::Inspect {
            path: path.to_owned(),
            source,
        })?;
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .and_then(|duration| i64::try_from(duration.as_nanos()).ok())
            .unwrap_or(-1);
        Ok(Self {
            bytes: metadata.len(),
            modified_ns,
        })
    }
}

/// Inspect the requested library paths against the versioned local cache.
///
/// # Errors
///
/// Returns a cache error if the independent database cannot be opened.
pub fn prepare(store_path: &Path, paths: Vec<PathBuf>) -> Result<Prepared, Error> {
    let store = Store::open(store_path)?;
    let mut prepared = Prepared::default();
    for path in paths {
        let Ok(stamp) = Stamp::read(&path) else {
            prepared.pending.push(path);
            continue;
        };
        match store.current(&path, stamp)? {
            Some(features) => {
                prepared.ready.insert(path, features);
            }
            None => prepared.pending.push(path),
        }
    }
    Ok(prepared)
}

/// Decode, analyze and persist one track. Intended for a cancellable sequence
/// of background tasks rather than a monolithic uninterruptible library pass.
///
/// # Errors
///
/// Returns an inspection, decode, analysis or cache error for this path.
pub fn analyze_and_store(store_path: &Path, path: PathBuf) -> Result<Analyzed, Error> {
    let before = Stamp::read(&path)?;
    let decoded = AudioSource::decode_all(&path).map_err(|error| Error::Decode {
        path: path.clone(),
        detail: error.to_string(),
    })?;
    let resampled = resample_interleaved(&decoded.samples, decoded.sample_rate, ANALYSIS_RATE)
        .map_err(|error| Error::Decode {
            path: path.clone(),
            detail: error.to_string(),
        })?;
    let mono: Vec<f32> = resampled
        .chunks_exact(CHANNELS)
        .map(|frame| (frame[0] + frame[1]) * std::f32::consts::FRAC_1_SQRT_2)
        .collect();
    let analysis = Song::analyze(&mono).map_err(|error| Error::Analyze {
        path: path.clone(),
        detail: error.to_string(),
    })?;
    let semantic = semantic::embed_audio(&decoded).map_err(Error::Semantic)?;
    let after = Stamp::read(&path)?;
    if before != after {
        return Err(Error::Analyze {
            path,
            detail: "the file changed while it was being analysed".to_owned(),
        });
    }
    let features = Features {
        values: analysis.as_vec(),
        semantic: semantic.clone(),
    };
    Store::open(store_path)?.put(&path, after, &analysis, &semantic)?;
    Ok(Analyzed { path, features })
}

struct Store {
    connection: Connection,
}

type StoredFeatureRow = (i64, i64, i64, Vec<u8>, Option<Vec<u8>>);

impl Store {
    fn open(path: &Path) -> Result<Self, Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Inspect {
                path: parent.to_owned(),
                source,
            })?;
        }
        let connection = Connection::open(path)?;
        let found: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if found > STORE_VERSION {
            return Err(Error::UnsupportedStoreVersion {
                found,
                supported: STORE_VERSION,
            });
        }
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS features (
                 path BLOB PRIMARY KEY NOT NULL,
                 bytes INTEGER NOT NULL,
                 modified_ns INTEGER NOT NULL,
                 feature_version INTEGER NOT NULL,
                 values_blob BLOB NOT NULL,
                 semantic_blob BLOB
             );
             ",
        )?;
        if found < 2 {
            let has_semantic: bool = connection
                .prepare("PRAGMA table_info(features)")?
                .query_map([], |row| row.get::<_, String>(1))?
                .filter_map(Result::ok)
                .any(|name| name == "semantic_blob");
            if !has_semantic {
                connection.execute("ALTER TABLE features ADD COLUMN semantic_blob BLOB", [])?;
            }
        }
        connection.execute_batch(&format!("PRAGMA user_version={STORE_VERSION};"))?;
        Ok(Self { connection })
    }

    fn current(&self, path: &Path, stamp: Stamp) -> Result<Option<Features>, Error> {
        let row: Option<StoredFeatureRow> = self
            .connection
            .query_row(
                "SELECT bytes, modified_ns, feature_version, values_blob, semantic_blob
                   FROM features WHERE path = ?1",
                [path_bytes(path)],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((bytes, modified_ns, version, blob, semantic_blob)) = row else {
            return Ok(None);
        };
        if u64::try_from(bytes).ok() != Some(stamp.bytes)
            || modified_ns != stamp.modified_ns
            || version != i64::from(u16::from(FeaturesVersion::LATEST))
        {
            return Ok(None);
        }
        let Some(semantic) = semantic_blob
            .as_deref()
            .and_then(|blob| decode_semantic(blob).ok())
        else {
            return Ok(None);
        };
        Ok(decode_values(&blob)
            .ok()
            .map(|values| Features { values, semantic }))
    }

    fn put(
        &self,
        path: &Path,
        stamp: Stamp,
        analysis: &Analysis,
        semantic: &[f32],
    ) -> Result<(), Error> {
        let bytes = i64::try_from(stamp.bytes).unwrap_or(i64::MAX);
        let version = i64::from(u16::from(analysis.features_version));
        self.connection.execute(
            "INSERT INTO features(path, bytes, modified_ns, feature_version, values_blob, semantic_blob)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(path) DO UPDATE SET
                bytes=excluded.bytes,
                modified_ns=excluded.modified_ns,
                feature_version=excluded.feature_version,
                values_blob=excluded.values_blob,
                semantic_blob=excluded.semantic_blob",
            params![
                path_bytes(path),
                bytes,
                stamp.modified_ns,
                version,
                encode_values(&analysis.as_vec()),
                encode_values(semantic),
            ],
        )?;
        Ok(())
    }
}

fn encode_values(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn decode_values(blob: &[u8]) -> Result<Vec<f32>, Error> {
    if !blob.len().is_multiple_of(size_of::<f32>()) {
        return Err(Error::InvalidRow);
    }
    let values: Vec<f32> = blob
        .chunks_exact(size_of::<f32>())
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect();
    Analysis::new(values.clone(), FeaturesVersion::LATEST).map_err(|_| Error::InvalidRow)?;
    Ok(values)
}

fn decode_semantic(blob: &[u8]) -> Result<Vec<f32>, Error> {
    let values = decode_floats(blob)?;
    if values.len() != 512 || values.iter().any(|value| !value.is_finite()) {
        return Err(Error::InvalidRow);
    }
    Ok(values)
}

fn decode_floats(blob: &[u8]) -> Result<Vec<f32>, Error> {
    if !blob.len().is_multiple_of(size_of::<f32>()) {
        return Err(Error::InvalidRow);
    }
    Ok(blob
        .chunks_exact(size_of::<f32>())
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect())
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(path: &str, album: u64, artist: &str, energy: f32, bright: f32) -> Candidate {
        let mut values = vec![0.0; FeaturesVersion::LATEST.feature_count()];
        values[AnalysisIndex::Tempo as usize] = energy;
        values[AnalysisIndex::MeanLoudness as usize] = energy;
        values[AnalysisIndex::StdDeviationLoudness as usize] = energy;
        values[AnalysisIndex::Zcr as usize] = bright;
        values[AnalysisIndex::MeanSpectralCentroid as usize] = bright;
        values[AnalysisIndex::MeanSpectralRolloff as usize] = bright;
        Candidate {
            path: PathBuf::from(path),
            album,
            artist: artist.to_owned(),
            features: Features {
                values,
                semantic: vec![0.0; 512],
            },
        }
    }

    /// A candidate whose *words* answer at a stated strength: the semantic
    /// vector is placed at angle `angle` in one plane, so its cosine against
    /// the request below is exactly `angle.cos()` and a library can be given a
    /// distribution to find a knee in.
    fn worded(path: &str, album: u64, energy: f32, angle: f32) -> Candidate {
        let mut candidate = synthetic(path, album, &format!("Artist {album}"), energy, energy);
        candidate.features.semantic = {
            let mut semantic = vec![0.0; 512];
            semantic[0] = angle.cos();
            semantic[1] = angle.sin();
            semantic
        };
        candidate
    }

    /// The request `worded` is answered at: the first axis of that plane.
    fn request() -> Vec<f32> {
        let mut request = vec![0.0; 512];
        request[0] = 1.0;
        request
    }

    /// A library with a real similarity distribution and a real spread of
    /// energies — enough of both for the knee to have something to find.
    fn library(count: u64) -> Vec<Candidate> {
        #[expect(clippy::cast_precision_loss, reason = "a bounded fixture count")]
        (0..count)
            .map(|index| {
                let through = index as f32 / count as f32;
                worded(
                    &format!("/track-{index}"),
                    index,
                    (through - 0.5) * 1.8,
                    // A steep head and a long shoulder, which is the shape a
                    // real retrieval curve has.
                    through.powi(2) * 1.4,
                )
            })
            .collect()
    }

    /// A rising line, drawn as the blend.
    fn rising() -> Contour {
        Contour::blended(&[
            ContourPoint {
                at: 0.0,
                level: -1.6,
            },
            ContourPoint {
                at: 1.0,
                level: 1.6,
            },
        ])
    }

    fn falling() -> Contour {
        Contour::blended(&[
            ContourPoint {
                at: 0.0,
                level: 1.6,
            },
            ContourPoint {
                at: 1.0,
                level: -1.6,
            },
        ])
    }

    fn composed(
        contour: &Contour,
        pool: &Pool,
        candidates: &[Candidate],
        limit: usize,
    ) -> Selection {
        walk(
            &Request {
                semantic: Some(&request()),
                contour,
                seed: None,
            },
            pool,
            candidates,
            limit,
            0,
        )
    }

    /// **I1 — the words let it in.** Every chosen track is in the eligible set,
    /// which is design 21 §3's first sentence and was not true before: the old
    /// walk scored one blended cost over the whole library, so relevance and
    /// fit traded, and a track could win a slot by standing at the right
    /// height however poorly it answered the words.
    #[test]
    fn every_chosen_track_is_one_the_words_let_in() {
        let candidates = library(400);
        let pool = eligible(Some(&request()), &candidates);
        assert!(pool.len() < candidates.len(), "the words excluded nothing");
        assert!(pool.len() >= KNEE_FLOOR);
        let eligible_paths: HashSet<&PathBuf> = pool
            .ranked()
            .iter()
            .map(|index| &candidates[*index].path)
            .collect();
        for contour in [rising(), falling()] {
            let selection = composed(&contour, &pool, &candidates, 18);
            assert!(!selection.paths.is_empty());
            for path in &selection.paths {
                assert!(
                    eligible_paths.contains(path),
                    "{} was chosen without being eligible",
                    path.display()
                );
            }
        }
    }

    /// **I3 — the line does not re-select the pool.** Move the curve with the
    /// words held still and the eligible set is identical, count and cloud
    /// alike. This is what makes design 21 §3's answer to *"why did my change
    /// do nothing?"* — *"you moved the line, which reorders rather than
    /// re-selects"* — a description rather than a claim.
    #[test]
    fn moving_the_line_does_not_change_what_is_eligible() {
        let candidates = library(400);
        let pool = eligible(Some(&request()), &candidates);
        let up = composed(&rising(), &pool, &candidates, 18);
        let down = composed(&falling(), &pool, &candidates, 18);
        assert_eq!(up.eligible_tracks, down.eligible_tracks);
        assert_eq!(up.cloud[0].len(), down.cloud[0].len());
        let sorted = |cloud: &[f32]| {
            let mut values = cloud.to_vec();
            values.sort_by(f32::total_cmp);
            values
        };
        assert_eq!(sorted(&up.cloud[0]), sorted(&down.cloud[0]));
        // …and the result really did move, or the invariant is vacuous.
        assert_ne!(up.paths, down.paths);
    }

    /// **I4 — a small pool is honest.** When the eligible set is no larger
    /// than the list being built, moving the line changes the order and never
    /// the membership. This is the one case where *"reorders rather than
    /// re-selects"* must be literally, exactly true.
    #[test]
    fn a_pool_no_bigger_than_the_list_only_ever_reorders() {
        let candidates = library(10);
        let pool = eligible(Some(&request()), &candidates);
        assert_eq!(pool.len(), candidates.len(), "a small library is all pool");
        let up = composed(&rising(), &pool, &candidates, 10);
        let down = composed(&falling(), &pool, &candidates, 10);
        let members = |selection: &Selection| {
            let mut paths = selection.paths.clone();
            paths.sort();
            paths
        };
        assert_eq!(members(&up), members(&down), "membership moved");
        assert_ne!(up.paths, down.paths, "and nothing was reordered");
    }

    /// **I5 — no padding.** A result is never longer than the pool supports,
    /// however many positions were asked for. Design 21 §12: *a request the
    /// library cannot fill returns fewer songs and says why.*
    #[test]
    fn a_result_never_outgrows_the_pool_it_came_from() {
        let candidates = library(10);
        let pool = eligible(Some(&request()), &candidates);
        let selection = composed(&rising(), &pool, &candidates, 40);
        assert!(selection.paths.len() <= pool.len());
        assert!(selection.paths.len() <= 10);
        assert_eq!(selection.eligible_tracks, pool.len());
        assert_eq!(selection.matches.len(), selection.paths.len());
    }

    /// **The blend is consistent under weighting.** Set every dimension to one
    /// curve and the weighted mean is that curve, whatever the weights are —
    /// which is what lets design 21 §5's expander *reveal* the per-dimension
    /// lines rather than seed them, and what makes "one line" and "five lines
    /// holding one curve" the same request.
    ///
    /// Design 21 §5 predicted this would be simplified away later by somebody
    /// who did not know why it held. This is the pin.
    #[test]
    fn a_blend_of_one_curve_is_that_curve() {
        let candidates = library(200);
        let pool = eligible(Some(&request()), &candidates);
        let contour = rising();
        assert_eq!(contour.lanes.len(), Dimension::ALL.len());
        let selection = composed(&contour, &pool, &candidates, 12);
        // Every lane holds the same curve, so the blend of the lanes' results
        // is the mean of identical inputs — per track, exactly.
        for (position, blended) in selection.blended.iter().enumerate() {
            let lane_levels: Vec<f32> = selection.levels.iter().map(|row| row[position]).collect();
            let weighted: f32 = Contour::BLEND
                .iter()
                .zip(&lane_levels)
                .map(|(weight, level)| weight * level)
                .sum::<f32>()
                / Contour::BLEND.iter().sum::<f32>();
            assert!(
                (blended - weighted).abs() < 0.0001,
                "position {position}: {blended} against {weighted}"
            );
        }
        // …and the weights are what design 21 §5 asked for: energy dominant,
        // summing to one, so the identity above holds for any of them.
        assert!((Contour::BLEND.iter().sum::<f32>() - 1.0).abs() < 0.0001);
        assert!(
            Contour::BLEND
                .iter()
                .skip(1)
                .all(|weight| *weight < Contour::BLEND[0])
        );
    }

    /// **The pool's own terciles**, and nothing absolute. The strongest third
    /// of what the words let in reads as three ticks whatever the phrase's
    /// cosines happen to be, which is what keeps the picture stable while the
    /// underlying numbers drift.
    #[test]
    fn match_strength_is_a_place_in_the_pool_rather_than_a_score() {
        let candidates = library(400);
        let pool = eligible(Some(&request()), &candidates);
        let ranked = pool.ranked().to_vec();
        assert_eq!(pool.strength(ranked[0]), Some(Strength::Strong));
        assert_eq!(
            pool.strength(ranked[ranked.len() - 1]),
            Some(Strength::Weak)
        );
        assert_eq!(Strength::Strong.ticks(), 3);
        assert_eq!(Strength::Weak.ticks(), 1);
        // Every third is populated rather than one bucket taking everything.
        let mut seen = [0_usize; 3];
        for index in &ranked {
            seen[usize::from(pool.strength(*index).expect("eligible").ticks()) - 1] += 1;
        }
        assert!(seen.iter().all(|count| *count > 0), "{seen:?}");
        // A request with no words has no match strength to report.
        let shapeless = eligible(None, &candidates);
        assert_eq!(shapeless.len(), candidates.len());
        assert_eq!(shapeless.strength(ranked[0]), None);
    }

    #[test]
    fn controls_rank_real_features_then_diversify_the_sequence() {
        let candidates = vec![
            synthetic("/quiet-a", 1, "One", -0.9, -0.7),
            synthetic("/quiet-b", 1, "One", -0.8, -0.6),
            synthetic("/quiet-c", 2, "Two", -0.7, -0.5),
            synthetic("/quiet-d", 3, "Three", -0.6, -0.4),
            synthetic("/loud", 4, "Four", 0.9, 0.8),
        ];
        let selection = select(
            &Profile {
                energy: -2,
                brightness: -1,
                seed: None,
            },
            &candidates,
            3,
        );
        assert_eq!(selection.paths.len(), 3);
        assert!(!selection.paths.contains(&PathBuf::from("/loud")));
        let selected: Vec<_> = selection
            .paths
            .iter()
            .map(|path| {
                candidates
                    .iter()
                    .find(|candidate| &candidate.path == path)
                    .expect("selected path belongs to the pool")
            })
            .collect();
        assert!(
            selected
                .windows(2)
                .all(|pair| pair[0].artist != pair[1].artist)
        );
        assert_eq!(
            selected
                .iter()
                .map(|candidate| candidate.album)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
        assert_eq!(selection.analysed_tracks, 5);
    }

    #[test]
    fn a_seed_uses_the_complete_sonic_vector() {
        let candidates = vec![
            synthetic("/seed", 1, "One", -0.8, 0.7),
            synthetic("/near", 2, "Two", -0.7, 0.6),
            synthetic("/far", 3, "Three", 0.8, -0.7),
        ];
        let selection = select(
            &Profile {
                seed: Some(PathBuf::from("/seed")),
                ..Profile::default()
            },
            &candidates,
            2,
        );
        assert_eq!(selection.paths, ["/seed", "/near"].map(PathBuf::from));
    }

    #[test]
    fn sequencing_never_falls_back_through_its_diversity_rules() {
        let candidates = vec![
            synthetic("/a-1", 1, "A", -0.9, 0.0),
            synthetic("/a-2", 2, "A", -0.8, 0.0),
            synthetic("/a-3", 3, "A", -0.7, 0.0),
            synthetic("/b", 4, "B", -0.6, 0.0),
            synthetic("/c", 5, "C", -0.5, 0.0),
        ];
        let selection = select(
            &Profile {
                energy: -2,
                ..Profile::default()
            },
            &candidates,
            5,
        );
        let artists: Vec<_> = selection
            .paths
            .iter()
            .map(|path| {
                candidates
                    .iter()
                    .find(|candidate| &candidate.path == path)
                    .expect("selected path")
                    .artist
                    .as_str()
            })
            .collect();
        assert!(artists.windows(2).all(|pair| pair[0] != pair[1]));
        assert!(artists.iter().filter(|artist| **artist == "A").count() <= 2);
    }

    #[test]
    fn a_journey_changes_the_target_across_the_list() {
        let candidates: Vec<_> = (-9_i8..=9)
            .map(|step| {
                let level = f32::from(step) / 10.0;
                synthetic(
                    &format!("/{step}"),
                    u64::try_from(i16::from(step) + 10).expect("positive album"),
                    &format!("Artist {step}"),
                    level,
                    0.0,
                )
            })
            .collect();
        let selection = select_journey(
            &[
                Profile {
                    energy: -2,
                    ..Profile::default()
                },
                Profile {
                    energy: 2,
                    ..Profile::default()
                },
                Profile {
                    energy: -1,
                    ..Profile::default()
                },
            ],
            &candidates,
            9,
            0,
        );
        let energy = |path: &PathBuf| {
            candidates
                .iter()
                .find(|candidate| &candidate.path == path)
                .expect("selected candidate")
                .features
                .energy()
        };
        let first = energy(selection.paths.first().expect("opening"));
        let turn = energy(&selection.paths[selection.paths.len() / 2]);
        let last = energy(selection.paths.last().expect("landing"));
        assert!(turn > first, "the turn must rise above the opening");
        assert!(turn > last, "the landing must come down from the turn");
    }

    /// **A contour is a line, and a line has a value everywhere.**
    ///
    /// Including outside its own ends: before the first point and after the
    /// last it holds that point's level, so the walk never asks a question the
    /// shape cannot answer — which would otherwise happen on the first and
    /// last track of every list, where the fraction lands exactly on 0 and 1.
    #[test]
    fn a_contour_reads_a_level_at_every_position() {
        let points = [
            ContourPoint {
                at: 0.0,
                level: -2.0,
            },
            ContourPoint {
                at: 0.5,
                level: 2.0,
            },
            ContourPoint {
                at: 1.0,
                level: 0.0,
            },
        ];
        assert_eq!(Contour::level_at(&points, 0.0), Some(-2.0));
        assert_eq!(Contour::level_at(&points, 0.25), Some(0.0));
        assert_eq!(Contour::level_at(&points, 0.5), Some(2.0));
        assert_eq!(Contour::level_at(&points, 0.75), Some(1.0));
        assert_eq!(Contour::level_at(&points, 1.0), Some(0.0));
        // Outside the declared range, and outside the unit interval.
        assert_eq!(Contour::level_at(&points, -1.0), Some(-2.0));
        assert_eq!(Contour::level_at(&points, 9.0), Some(0.0));
        // One point is a flat line; none is unconstrained, which is a
        // different thing from a line at zero.
        assert_eq!(
            Contour::level_at(
                &[ContourPoint {
                    at: 0.4,
                    level: 1.5
                }],
                0.9
            ),
            Some(1.5)
        );
        assert_eq!(Contour::level_at(&[], 0.5), None);
    }

    /// **The scale a contour is drawn on is a place in the collection**, not
    /// a fraction of the distance between its two most extreme records.
    ///
    /// That distinction is the feature working or not. Loudness and tempo
    /// cluster: a real library has a few outliers at each end and everything
    /// else packed in the middle, so a min–max axis put almost every track
    /// within a whisker of the centre — and a rising line then drew tracks
    /// that were all *at* the centre. The owner saw it immediately: *"the
    /// little dots seem to all be more or less in a line and not following my
    /// line."*
    #[test]
    fn levels_are_a_place_in_the_collection_rather_than_a_span() {
        let candidates = vec![
            synthetic("/calm", 1, "One", -0.9, -0.9),
            synthetic("/middle", 2, "Two", 0.0, 0.0),
            synthetic("/loud", 3, "Three", 0.9, 0.9),
        ];
        let spread = levels(&candidates, Dimension::Energy);
        // Three tracks are three thirds of the collection, each read at its
        // own middle: 1/6, 3/6, 5/6 of the way up.
        assert!((spread[0] - -4.0 / 3.0).abs() < 0.001, "{spread:?}");
        assert!(spread[1].abs() < 0.001, "{spread:?}");
        assert!((spread[2] - 4.0 / 3.0).abs() < 0.001, "{spread:?}");

        // **A clustered collection is spread across the axis anyway**, which
        // is the whole point: thirty tracks within a whisker of each other
        // still occupy the whole scale, so a line has something to follow at
        // every height it asks for.
        let mut clustered: Vec<Candidate> = (0..30)
            .map(|step| {
                let energy = 0.001 * f32::from(i8::try_from(step).unwrap_or(0));
                synthetic(&format!("/tight-{step}"), step, "Cluster", energy, 0.0)
            })
            .collect();
        clustered.push(synthetic("/outlier-low", 90, "Low", -0.95, 0.0));
        clustered.push(synthetic("/outlier-high", 91, "High", 0.95, 0.0));
        let spread = levels(&clustered, Dimension::Energy);
        let inside: Vec<f32> = spread.iter().take(30).copied().collect();
        let low = inside.iter().copied().fold(f32::MAX, f32::min);
        let high = inside.iter().copied().fold(f32::MIN, f32::max);
        assert!(
            high - low > 3.0,
            "the cluster occupies {low}…{high} of the axis, which is the \
             min-max behaviour this replaced"
        );

        // A collection with no spread at all says the middle rather than
        // dividing by zero.
        let flat = vec![synthetic("/a", 1, "One", 0.2, 0.2)];
        assert!(levels(&flat, Dimension::Energy)[0].abs() < 0.001);
    }

    /// **The shape is what the walk follows** — the whole point of the
    /// contour, asserted on the result rather than on the request.
    ///
    /// A pool spread evenly across the energy axis, asked for a line that
    /// climbs from the calm end to the loud one: the list it produces must
    /// climb too, and the levels it reports must be the ones it climbed
    /// through.
    #[test]
    fn a_rising_contour_produces_a_rising_list() {
        let candidates: Vec<_> = (-9_i8..=9)
            .map(|step| {
                let level = f32::from(step) / 10.0;
                synthetic(
                    &format!("/{step}"),
                    u64::try_from(i16::from(step) + 10).expect("positive album"),
                    &format!("Artist {step}"),
                    level,
                    0.0,
                )
            })
            .collect();
        let contour = Contour::of(
            Dimension::Energy,
            vec![
                ContourPoint {
                    at: 0.0,
                    level: -2.0,
                },
                ContourPoint {
                    at: 1.0,
                    level: 2.0,
                },
            ],
        );
        let selection =
            select_contour("", &contour, &candidates, 8, 0).expect("no prompt needs no model");
        assert_eq!(selection.paths.len(), 8);
        let energy = selection.levels.first().expect("one lane, one row");
        assert_eq!(energy.len(), selection.paths.len());
        let opening = *energy.first().expect("an opening");
        let landing = *energy.last().expect("a landing");
        assert!(
            landing > opening + 2.0,
            "the list did not climb: {opening} → {landing}"
        );
        // …and the reported levels are the chosen tracks', not a copy of the
        // request: each one is that track's own place in the collection.
        let by_path = |path: &PathBuf| {
            let index = candidates
                .iter()
                .position(|candidate| &candidate.path == path)
                .expect("a chosen path belongs to the pool");
            levels(&candidates, Dimension::Energy)[index]
        };
        for (path, level) in selection.paths.iter().zip(energy) {
            let expected = by_path(path);
            assert!((expected - level).abs() < 0.001, "{path:?}");
        }
    }

    /// **A clustered library still follows the line** — the owner's own
    /// report, as a test.
    ///
    /// *"after moving my line around on the chart, when I compose, the little
    /// dots seem to all be more or less in a line and not following my
    /// line."* Two causes, both here: a min–max axis mapped a packed
    /// collection onto a whisker of the scale, and a single global shortlist
    /// took the `5 × limit` best-scoring tracks — which, with words carrying
    /// the larger weight, could all sit at one height, leaving the walk
    /// nothing to climb with.
    ///
    /// The pool below is the shape a real library has: a dense middle, thin
    /// tails. A rising line over it must produce a rising list.
    #[test]
    fn a_clustered_library_still_follows_the_line() {
        let mut candidates: Vec<Candidate> = (0..60)
            .map(|step| {
                // A tight middle: sixty tracks inside a tenth of the range.
                let energy = 0.05 * (f32::from(i8::try_from(step % 4).unwrap_or(0)) - 1.5);
                synthetic(
                    &format!("/middle-{step}"),
                    u64::try_from(step).unwrap_or(0),
                    &format!("Middle {step}"),
                    energy,
                    0.0,
                )
            })
            .collect();
        for step in 0_u8..6 {
            candidates.push(synthetic(
                &format!("/quiet-{step}"),
                100 + u64::from(step),
                &format!("Quiet {step}"),
                -0.9 + 0.01 * f32::from(step),
                0.0,
            ));
            candidates.push(synthetic(
                &format!("/loud-{step}"),
                200 + u64::from(step),
                &format!("Loud {step}"),
                0.9 - 0.01 * f32::from(step),
                0.0,
            ));
        }
        let contour = Contour::of(
            Dimension::Energy,
            vec![
                ContourPoint {
                    at: 0.0,
                    level: -2.0,
                },
                ContourPoint {
                    at: 1.0,
                    level: 2.0,
                },
            ],
        );
        let selection =
            select_contour("", &contour, &candidates, 10, 0).expect("no prompt needs no model");
        let energy = selection.levels.first().expect("one lane, one row");
        let opening = *energy.first().expect("an opening");
        let landing = *energy.last().expect("a landing");
        assert!(
            landing > opening + 2.0,
            "the list did not climb through a clustered collection: \
             {opening} → {landing} ({energy:?})"
        );
        // **…and it climbed through the middle rather than jumping between
        // the tails**, which is the assertion the old behaviour could not
        // pass: with a min–max axis the sixty middle tracks all read as the
        // same level, so the quarters of the list were indistinguishable and
        // only the twelve outliers could move the line.
        let quarters = [
            energy[energy.len() / 4],
            energy[energy.len() / 2],
            energy[3 * energy.len() / 4],
        ];
        assert!(
            quarters[0] < quarters[1] && quarters[1] < quarters[2],
            "the walk skipped the middle of its own line: {energy:?}"
        );
    }

    /// **An unconstrained axis costs nothing**, which is what lets the
    /// energy line be drawn alone: a contour that says nothing about
    /// brightness must not quietly pull the list to the middle of it.
    #[test]
    fn an_undrawn_axis_does_not_steer() {
        let candidates = vec![
            synthetic("/dark-loud", 1, "One", 0.9, -0.9),
            synthetic("/bright-loud", 2, "Two", 0.9, 0.9),
            synthetic("/dark-calm", 3, "Three", -0.9, -0.9),
        ];
        let loud = Contour::of(
            Dimension::Energy,
            vec![ContourPoint {
                at: 0.0,
                level: 2.0,
            }],
        );
        let selection = select_contour("", &loud, &candidates, 2, 0).expect("no prompt");
        assert!(!selection.paths.contains(&PathBuf::from("/dark-calm")));
        assert_eq!(selection.paths.len(), 2, "both loud tracks, either order");
    }

    #[test]
    fn another_version_changes_close_choices_without_changing_the_request() {
        let candidates: Vec<_> = (0_u64..12)
            .map(|index| {
                synthetic(
                    &format!("/{index}"),
                    index,
                    &format!("Artist {index}"),
                    0.1,
                    -0.1,
                )
            })
            .collect();
        let profile = Profile {
            energy: 1,
            brightness: -1,
            seed: None,
        };
        let first = select_journey(std::slice::from_ref(&profile), &candidates, 6, 1);
        let another = select_journey(&[profile], &candidates, 6, 2);
        assert_ne!(first.paths, another.paths);
    }

    /// **I2 — the same request twice is the same list.**
    ///
    /// This replaces `recent_previews_do_not_keep_winning_new_mixes`, and the
    /// swap is the point. That test pinned a +2.0 penalty on recently offered
    /// tracks — against weights summing to under one, a ban rather than a
    /// tiebreak — applied invisibly on every compose. Design 21 §4 promises
    /// *"no hidden state, nothing accumulating out of sight"*, and §6's diff
    /// sentence has to be able to say *"identical, because nothing changed"*
    /// and be right. Variation is a visible press now, and it is the seed that
    /// carries it.
    #[test]
    fn an_unchanged_request_returns_an_identical_list() {
        let candidates: Vec<_> = (0_u64..12)
            .map(|index| {
                synthetic(
                    &format!("/{index}"),
                    index,
                    &format!("Artist {index}"),
                    0.1,
                    -0.1,
                )
            })
            .collect();
        let profile = Profile {
            energy: 1,
            brightness: -1,
            seed: None,
        };
        let first = select_journey(std::slice::from_ref(&profile), &candidates, 6, 0);
        let again = select_journey(std::slice::from_ref(&profile), &candidates, 6, 0);
        assert_eq!(first.paths, again.paths, "the same request twice");
        // …and the visible press is what changes it.
        let another = select_journey(&[profile], &candidates, 6, 1);
        assert_ne!(first.paths, another.paths, "another version");
    }

    #[test]
    fn duplicate_library_projections_never_duplicate_an_exact_track() {
        let mut candidates: Vec<_> = (0_u64..6)
            .map(|index| {
                synthetic(
                    &format!("/{index}"),
                    index,
                    &format!("Artist {index}"),
                    0.0,
                    0.0,
                )
            })
            .collect();
        candidates.push(candidates[0].clone());
        let selection = select(
            &Profile {
                energy: 1,
                ..Profile::default()
            },
            &candidates,
            candidates.len(),
        );
        let unique: HashSet<_> = selection.paths.iter().collect();
        assert_eq!(selection.paths.len(), unique.len());
    }

    #[test]
    #[cfg_attr(
        target_os = "windows",
        ignore = "the bundled ONNX audio inference stalls on the hosted Windows runner"
    )]
    fn a_real_wave_is_analyzed_cached_and_invalidated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("pulse.wav");
        let store = dir.path().join("vibe.db");
        write_wave(&audio, 44_100);

        let first = prepare(&store, vec![audio.clone()]).expect("prepare");
        assert_eq!(first.pending, std::slice::from_ref(&audio));
        let analyzed = analyze_and_store(&store, audio.clone()).expect("analysis");
        assert!(analyzed.features.tempo_bpm().is_finite());

        let cached = prepare(&store, vec![audio.clone()]).expect("prepare cached");
        assert!(cached.pending.is_empty());
        assert_eq!(cached.ready.get(&audio), Some(&analyzed.features));

        write_wave(&audio, 48_000);
        let stale = prepare(&store, vec![audio.clone()]).expect("prepare stale");
        assert_eq!(stale.pending, [audio]);
    }

    #[test]
    fn a_newer_store_is_refused_without_stamping_it_backwards() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = dir.path().join("vibe.db");
        let connection = Connection::open(&store).expect("store");
        connection
            .execute_batch("PRAGMA user_version=4")
            .expect("future version");
        drop(connection);

        assert!(matches!(
            prepare(&store, Vec::new()),
            Err(Error::UnsupportedStoreVersion {
                found: 4,
                supported: STORE_VERSION
            })
        ));
        let connection = Connection::open(store).expect("reopen");
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, 4);
    }

    fn write_wave(path: &Path, frames: usize) {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("wave");
        for frame in 0..frames {
            let pulse = if frame % 22_050 < 180 {
                i16::MAX / 3
            } else {
                0
            };
            writer.write_sample(pulse).expect("left");
            writer.write_sample(pulse).expect("right");
        }
        writer.finalize().expect("finalize");
    }
}
