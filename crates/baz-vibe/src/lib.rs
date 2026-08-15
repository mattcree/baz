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
const STORE_VERSION: i64 = 2;

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

/// **One line of a contour**: a dimension, and the shape asked of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Lane {
    /// What this line is about.
    pub dimension: Dimension,
    /// Its points, in order of position through the playlist.
    pub points: Vec<ContourPoint>,
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
    /// A contour of one line.
    #[must_use]
    pub fn of(dimension: Dimension, points: Vec<ContourPoint>) -> Self {
        Self {
            lanes: vec![Lane { dimension, points }],
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

    /// Every lane's target at one position, in lane order.
    fn targets_at(&self, fraction: f32) -> Vec<(Dimension, Option<f32>)> {
        self.lanes
            .iter()
            .map(|lane| (lane.dimension, Self::level_at(&lane.points, fraction)))
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

/// Ranked and sequenced sonic result.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Paths in listening order.
    pub paths: Vec<PathBuf>,
    /// Complete analyzed candidate pool.
    pub pool_tracks: usize,
    /// Tempo span of the selected tracks, rounded only by the UI.
    pub tempo_span: Option<(f32, f32)>,
    /// Where each chosen track sits on the −2…+2 collection-relative axes the
    /// request was made on: one row per [`Lane`], in lane order, each holding
    /// a level per chosen track in listening order.
    ///
    /// It is the *result* in the request's own units, which is what lets a
    /// surface draw what it got over what it asked for instead of asking the
    /// listener to take the answer on faith.
    pub levels: Vec<Vec<f32>>,
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

    fn semantic_distance(&self, other: &[f32]) -> f32 {
        1.0 - self
            .semantic
            .iter()
            .zip(other)
            .map(|(left, right)| left * right)
            .sum::<f32>()
    }

    fn semantic_pair_distance(&self, other: &Self) -> f32 {
        self.semantic_distance(&other.semantic)
    }

    fn at(&self, index: AnalysisIndex) -> f32 {
        self.values[index as usize]
    }
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
    select_journey_avoiding(profiles, candidates, limit, variation, &HashSet::new())
}

/// Select a journey while strongly preferring tracks that have not appeared
/// in the listener's recent generated previews. Recent tracks remain a
/// fallback for very small libraries or tightly constrained requests.
#[must_use]
pub fn select_journey_avoiding<S: std::hash::BuildHasher>(
    profiles: &[Profile],
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
    recently_offered: &HashSet<PathBuf, S>,
) -> Selection {
    if profiles.is_empty() || profiles.iter().all(Profile::is_empty) {
        return Selection {
            pool_tracks: candidates.len(),
            ..Selection::default()
        };
    }
    walk(
        &Request {
            semantic: None,
            contour: &profiles_as_contour(profiles),
            seed: profiles.iter().find_map(|profile| profile.seed.as_ref()),
        },
        candidates,
        limit,
        variation,
        recently_offered,
    )
}

/// Retrieve and sequence tracks for an ordinary-language musical request.
/// Text and audio are embedded by the paired bundled CLAP towers; all ranking
/// remains local and the recent-preview policy applies across generations.
///
/// # Errors
///
/// Returns an inference error if the bundled model cannot embed the prompt.
pub fn select_semantic<S: std::hash::BuildHasher>(
    prompt: &str,
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
    recently_offered: &HashSet<PathBuf, S>,
) -> Result<Selection, Error> {
    select_contour(
        prompt,
        &Contour::default(),
        candidates,
        limit,
        variation,
        recently_offered,
    )
}

/// **Retrieve by words, sequence by shape** — the one selector the other two
/// are written in terms of.
///
/// `prompt` chooses *what* the pool is, in ordinary language, through the
/// bundled CLAP towers; `contour` chooses *when* it happens, by asking each
/// position in the finished list for a level on the collection's own energy
/// and brightness axes. Either may be absent: with no contour this is exactly
/// the semantic retrieval that shipped before it, and with no prompt it is
/// exactly the profile journey — the cost weights below say so per case, so
/// neither shipped behaviour moved to make room for the third.
///
/// The diversity rules (an artist twice at most, never twice in a row, a
/// fresh album while one is available) and the recent-preview policy are the
/// same in all three, because they are properties of *a playlist* rather than
/// of how it was asked for.
///
/// # Errors
///
/// Returns an inference error if the bundled model cannot embed the prompt.
pub fn select_contour<S: std::hash::BuildHasher>(
    prompt: &str,
    contour: &Contour,
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
    recently_offered: &HashSet<PathBuf, S>,
) -> Result<Selection, Error> {
    let prompt = prompt.trim();
    if (prompt.is_empty() && contour.is_empty()) || candidates.is_empty() || limit == 0 {
        return Ok(Selection {
            pool_tracks: candidates.len(),
            ..Selection::default()
        });
    }
    let semantic = if prompt.is_empty() {
        None
    } else {
        Some(semantic::embed_text(prompt).map_err(Error::Semantic)?)
    };
    Ok(walk(
        &Request {
            semantic: semantic.as_deref(),
            contour,
            seed: None,
        },
        candidates,
        limit,
        variation,
        recently_offered,
    ))
}

/// **Where every candidate sits on one dimension's axis** — −2…+2, in the
/// order given.
///
/// The scale is collection-relative by construction: the pool's own ranking
/// stretched onto −2…+2, which is the same mapping the fit scores against. A
/// surface can therefore draw the library behind a lane, and the chosen
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

/// How many candidates the walk gets to choose from, per track it will
/// place. Five is enough room for the diversity rules to have somewhere to go
/// and tight enough that the walk stays a walk rather than a search.
const SHORTLIST_PER_TRACK: usize = 5;

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

/// How the cost is split between wanting the *right* music and wanting it in
/// the *right order*. One row per kind of request, so adding the hybrid could
/// not move the two that shipped before it.
struct Weights {
    relevance: f32,
    fit: f32,
    continuity: f32,
}

impl Weights {
    const VARIATION: f32 = 0.05;

    const fn for_request(words: bool, shape: bool) -> Self {
        match (words, shape) {
            // Words alone: retrieval dominates and continuity keeps the walk
            // from lurching between neighbours.
            (true, false) => Self {
                relevance: 0.72,
                fit: 0.0,
                continuity: 0.23,
            },
            // A shape alone: the position's target *is* the retrieval.
            (false, true) => Self {
                relevance: 0.0,
                fit: 0.67,
                continuity: 0.28,
            },
            // Both: the words say what the pool is and the shape says where
            // in it to be, so neither may drown the other.
            _ => Self {
                relevance: 0.45,
                fit: 0.30,
                continuity: 0.20,
            },
        }
    }
}

/// Retrieval, position-aware fit, diversity and sequencing — one auditable
/// policy, walked once per generated playlist.
#[expect(
    clippy::too_many_lines,
    reason = "one pass over one policy: shortlist, then a diversity-constrained walk"
)]
fn walk<S: std::hash::BuildHasher>(
    request: &Request<'_>,
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
    recently_offered: &HashSet<PathBuf, S>,
) -> Selection {
    if candidates.is_empty() || limit == 0 {
        return Selection {
            pool_tracks: candidates.len(),
            ..Selection::default()
        };
    }
    // One rank axis per dimension the request mentions, over this pool.
    let axes = Axes::over(
        request.contour.lanes.iter().map(|lane| lane.dimension),
        candidates,
    );
    let seed = request.seed.and_then(|path| {
        candidates
            .iter()
            .find(|candidate| &candidate.path == path)
            .map(|candidate| &candidate.features)
    });
    let max_seed_distance = seed.map_or(1.0, |seed| {
        candidates
            .iter()
            .map(|candidate| seed.distance(&candidate.features))
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
    let weights = Weights::for_request(
        request.semantic.is_some(),
        !request.contour.is_empty() || seed.is_some(),
    );
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
    let freshness = |candidate: &Candidate| {
        if recently_offered.contains(&candidate.path) {
            2.0
        } else {
            0.0
        }
    };
    let shortlist_len = (limit.saturating_mul(SHORTLIST_PER_TRACK))
        .max(limit)
        .min(candidates.len());
    // **The shortlist has to be able to answer the whole shape.**
    //
    // One global ranking cannot: with words *and* a line, relevance carries
    // the larger weight, so the best `5 × limit` tracks are the most relevant
    // ones — and if those all sit at one height, no walk over them can climb.
    // That is precisely what the owner saw: *"the little dots seem to all be
    // more or less in a line and not following my line."*
    //
    // So a shaped request retrieves **per position**: the curve is sampled,
    // each sample takes its own best few, and the union is what the walk
    // chooses from. Every height the line asks for therefore has candidates
    // in the room, and the walk's job goes back to being what it is — order
    // and diversity — rather than making bricks without straw.
    //
    // A request with no shape keeps the single global ranking it always had.
    let mut scored: Vec<(usize, f32)> = if request.contour.is_empty() {
        let mut scored: Vec<(usize, f32)> = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| {
                (
                    index,
                    weights.relevance * relevance(candidate)
                        + weights.fit * fit_at(candidate, 0.0)
                        + freshness(candidate),
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
            let mut at_position: Vec<(usize, f32)> = candidates
                .iter()
                .enumerate()
                .filter(|(index, _)| !taken.contains(index))
                .map(|(index, candidate)| {
                    (
                        index,
                        weights.relevance * relevance(candidate)
                            + weights.fit * fit_at(candidate, fraction)
                            + freshness(candidate),
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
                    let freshness = if recently_offered.contains(&candidate.path) {
                        2.0
                    } else {
                        0.0
                    };
                    weights.relevance * relevance(candidate)
                        + weights.fit * fit_at(candidate, fraction)
                        + weights.continuity * continuity
                        + Weights::VARIATION * variation_noise(&candidate.path, variation)
                        + freshness
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
    Selection {
        // One row per lane of the request, each holding that lane's level for
        // every chosen track — the result in the request's own units, lane by
        // lane, so a surface can draw each line's dots over its own line.
        levels: request
            .contour
            .lanes
            .iter()
            .map(|lane| {
                chosen
                    .iter()
                    .map(|&index| {
                        axes.level(
                            lane.dimension,
                            candidates[index].features.value(lane.dimension),
                        )
                    })
                    .collect()
            })
            .collect(),
        paths: chosen
            .into_iter()
            .map(|index| candidates[index].path.clone())
            .collect(),
        pool_tracks: candidates.len(),
        tempo_span,
    }
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
        lanes.push(Lane {
            dimension: Dimension::Energy,
            points: energy,
        });
    }
    if !brightness.is_empty() {
        lanes.push(Lane {
            dimension: Dimension::Brightness,
            points: brightness,
        });
    }
    Contour { lanes }
}

/// How badly a candidate misses what this position asked for.
///
/// An axis with no target does not enter the average, which is what makes an
/// unconstrained line cost nothing rather than pull everything to the middle.
/// A seed, where one is given, is worth one and a half axes: it is a whole
/// feature vector rather than a single number.
fn target_fit(
    candidate: &Candidate,
    targets: &[(Dimension, Option<f32>)],
    axes: &Axes,
    seed: Option<&Features>,
    max_seed_distance: f32,
) -> f32 {
    let mut score = 0.0_f32;
    let mut weights = 0.0_f32;
    for (dimension, target) in targets {
        if let Some(target) = target {
            score += axes.distance(*dimension, candidate.features.value(*dimension), *target);
            weights += 1.0;
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
    /// Build the axes the request needs, over the pool it will choose from.
    fn over(dimensions: impl Iterator<Item = Dimension>, candidates: &[Candidate]) -> Self {
        let mut axes = HashMap::new();
        for dimension in dimensions {
            axes.entry(dimension).or_insert_with(|| {
                Axis::of(
                    candidates
                        .iter()
                        .map(|candidate| candidate.features.value(dimension)),
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
    /// Most recently offered generated tracks, newest last.
    pub recently_offered: Vec<PathBuf>,
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
        if store.was_recently_offered(&path)? {
            prepared.recently_offered.push(path.clone());
        }
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

/// Persist a bounded freshness history so a conspicuously strong match does
/// not dominate generated playlists after Baz restarts.
///
/// # Errors
///
/// Returns a cache error if the disposable local index cannot be updated.
pub fn remember_offered(store_path: &Path, paths: &[PathBuf]) -> Result<(), Error> {
    let store = Store::open(store_path)?;
    for path in paths {
        store.connection.execute(
            "INSERT INTO recent_offers(path, offered_order)
             VALUES (?1, (SELECT COALESCE(MAX(offered_order), 0) + 1 FROM recent_offers))
             ON CONFLICT(path) DO UPDATE SET offered_order=excluded.offered_order",
            [path_bytes(path)],
        )?;
    }
    store.connection.execute(
        "DELETE FROM recent_offers WHERE path NOT IN (
             SELECT path FROM recent_offers ORDER BY offered_order DESC LIMIT 128
         )",
        [],
    )?;
    Ok(())
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
             CREATE TABLE IF NOT EXISTS recent_offers (
                 path BLOB PRIMARY KEY NOT NULL,
                 offered_order INTEGER NOT NULL
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

    fn was_recently_offered(&self, path: &Path) -> Result<bool, Error> {
        self.connection
            .query_row(
                "SELECT 1 FROM recent_offers WHERE path = ?1",
                [path_bytes(path)],
                |_| Ok(true),
            )
            .optional()
            .map(Option::unwrap_or_default)
            .map_err(Error::Store)
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
        assert_eq!(selection.pool_tracks, 5);
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
        let selection = select_contour("", &contour, &candidates, 8, 0, &HashSet::new())
            .expect("no prompt needs no model");
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
        let selection = select_contour("", &contour, &candidates, 10, 0, &HashSet::new())
            .expect("no prompt needs no model");
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
        let selection =
            select_contour("", &loud, &candidates, 2, 0, &HashSet::new()).expect("no prompt");
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

    #[test]
    fn recent_previews_do_not_keep_winning_new_mixes() {
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
        let recent: HashSet<PathBuf> = first.paths.iter().cloned().collect();
        let next = select_journey_avoiding(&[profile], &candidates, 6, 1, &recent);
        assert!(next.paths.iter().all(|path| !recent.contains(path)));
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
    fn generated_track_freshness_survives_reopening_the_index() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("track.wav");
        let store = dir.path().join("vibe.db");
        write_wave(&audio, 44_100);
        remember_offered(&store, std::slice::from_ref(&audio)).expect("remember");
        let prepared = prepare(&store, vec![audio.clone()]).expect("reopen");
        assert_eq!(prepared.recently_offered, [audio]);
    }

    #[test]
    fn a_newer_store_is_refused_without_stamping_it_backwards() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = dir.path().join("vibe.db");
        let connection = Connection::open(&store).expect("store");
        connection
            .execute_batch("PRAGMA user_version=3")
            .expect("future version");
        drop(connection);

        assert!(matches!(
            prepare(&store, Vec::new()),
            Err(Error::UnsupportedStoreVersion {
                found: 3,
                supported: STORE_VERSION
            })
        ));
        let connection = Connection::open(store).expect("reopen");
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, 3);
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
