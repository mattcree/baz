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
#[expect(
    clippy::too_many_lines,
    reason = "retrieval, position-aware fit, diversity and sequencing form one auditable playlist policy"
)]
pub fn select_journey_avoiding<S: std::hash::BuildHasher>(
    profiles: &[Profile],
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
    recently_offered: &HashSet<PathBuf, S>,
) -> Selection {
    if profiles.is_empty()
        || profiles.iter().all(Profile::is_empty)
        || candidates.is_empty()
        || limit == 0
    {
        return Selection {
            pool_tracks: candidates.len(),
            ..Selection::default()
        };
    }
    let energy_range = range(
        candidates
            .iter()
            .map(|candidate| candidate.features.energy()),
    );
    let bright_range = range(
        candidates
            .iter()
            .map(|candidate| candidate.features.brightness()),
    );
    let seed = profiles
        .iter()
        .find_map(|profile| profile.seed.as_ref())
        .and_then(|path| {
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
    let mut scored: Vec<(usize, f32)> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let score = profiles
                .iter()
                .map(|profile| {
                    profile_fit(
                        candidate,
                        f32::from(profile.energy),
                        f32::from(profile.brightness),
                        energy_range,
                        bright_range,
                        seed,
                        max_seed_distance,
                    )
                })
                .fold(f32::INFINITY, f32::min);
            let freshness_cost = if recently_offered.contains(&candidate.path) {
                2.0
            } else {
                0.0
            };
            (index, score + freshness_cost)
        })
        .collect();
    scored.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    let shortlist_len = (limit.saturating_mul(5)).max(limit).min(scored.len());
    scored.truncate(shortlist_len);

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
        let (target_energy, target_brightness) = interpolate_targets(profiles, fraction);
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
                    let fit = profile_fit(
                        candidate,
                        target_energy,
                        target_brightness,
                        energy_range,
                        bright_range,
                        seed,
                        max_seed_distance,
                    );
                    let transition = previous.map_or(0.0, |previous| {
                        previous.distance(&candidate.features) / transition_scale
                    });
                    let variation = variation_noise(&candidate.path, variation);
                    let freshness = if recently_offered.contains(&candidate.path) {
                        2.0
                    } else {
                        0.0
                    };
                    0.67 * fit + 0.28 * transition + 0.05 * variation + freshness
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
        paths: chosen
            .into_iter()
            .map(|index| candidates[index].path.clone())
            .collect(),
        pool_tracks: candidates.len(),
        tempo_span,
    }
}

/// Retrieve and sequence tracks for an ordinary-language musical request.
/// Text and audio are embedded by the paired bundled CLAP towers; all ranking
/// remains local and the recent-preview policy applies across generations.
///
/// # Errors
///
/// Returns an inference error if the bundled model cannot embed the prompt.
#[expect(
    clippy::too_many_lines,
    reason = "semantic retrieval, diversity and continuity form one auditable playlist policy"
)]
pub fn select_semantic<S: std::hash::BuildHasher>(
    prompt: &str,
    candidates: &[Candidate],
    limit: usize,
    variation: u64,
    recently_offered: &HashSet<PathBuf, S>,
) -> Result<Selection, Error> {
    if prompt.trim().is_empty() || candidates.is_empty() || limit == 0 {
        return Ok(Selection {
            pool_tracks: candidates.len(),
            ..Selection::default()
        });
    }
    let target = semantic::embed_text(prompt.trim()).map_err(Error::Semantic)?;
    let mut scored: Vec<_> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let freshness = if recently_offered.contains(&candidate.path) {
                2.0
            } else {
                0.0
            };
            (
                index,
                candidate.features.semantic_distance(&target) + freshness,
            )
        })
        .collect();
    scored.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    scored.truncate((limit.saturating_mul(5)).max(limit).min(scored.len()));

    let mut chosen: Vec<usize> = Vec::with_capacity(limit.min(scored.len()));
    let mut chosen_paths = HashSet::new();
    let mut artist_counts: HashMap<&str, usize> = HashMap::new();
    let mut album_counts: HashMap<u64, usize> = HashMap::new();
    while chosen.len() < limit && !scored.is_empty() {
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
            .min_by(|(_, (left, _)), (_, (right, _))| {
                let cost = |index: usize| {
                    let candidate = &candidates[index];
                    let relevance = candidate.features.semantic_distance(&target);
                    let continuity = previous.map_or(0.0, |previous| {
                        previous.semantic_pair_distance(&candidate.features)
                    });
                    let freshness = if recently_offered.contains(&candidate.path) {
                        2.0
                    } else {
                        0.0
                    };
                    0.72 * relevance
                        + 0.23 * continuity
                        + 0.05 * variation_noise(&candidate.path, variation)
                        + freshness
                };
                cost(*left)
                    .total_cmp(&cost(*right))
                    .then_with(|| left.cmp(right))
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
    Ok(Selection {
        paths: chosen
            .into_iter()
            .map(|index| candidates[index].path.clone())
            .collect(),
        pool_tracks: candidates.len(),
        tempo_span,
    })
}

fn interpolate_targets(profiles: &[Profile], fraction: f32) -> (f32, f32) {
    if profiles.len() == 1 {
        return (
            f32::from(profiles[0].energy),
            f32::from(profiles[0].brightness),
        );
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a journey has only a handful of visible waypoints"
    )]
    let scaled = fraction.clamp(0.0, 1.0) * (profiles.len() - 1) as f32;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "scaled is clamped to the non-negative visible waypoint range"
    )]
    let left = (scaled.floor() as usize).min(profiles.len() - 1);
    let right = (left + 1).min(profiles.len() - 1);
    let mix = scaled - scaled.floor();
    let interpolate = |from: i8, to: i8| f32::from(from) + f32::from(to - from) * mix;
    (
        interpolate(profiles[left].energy, profiles[right].energy),
        interpolate(profiles[left].brightness, profiles[right].brightness),
    )
}

fn profile_fit(
    candidate: &Candidate,
    energy: f32,
    brightness: f32,
    energy_range: (f32, f32),
    bright_range: (f32, f32),
    seed: Option<&Features>,
    max_seed_distance: f32,
) -> f32 {
    let mut score = 0.0_f32;
    let mut weights = 0.0_f32;
    if energy.abs() > f32::EPSILON {
        score += axis_distance_target(candidate.features.energy(), energy_range, energy);
        weights += 1.0;
    }
    if brightness.abs() > f32::EPSILON {
        score += axis_distance_target(candidate.features.brightness(), bright_range, brightness);
        weights += 1.0;
    }
    if let Some(seed) = seed {
        score += 1.5 * seed.distance(&candidate.features) / max_seed_distance;
        weights += 1.5;
    }
    score / weights.max(1.0)
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

fn range(values: impl Iterator<Item = f32>) -> (f32, f32) {
    values.fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), value| {
        (low.min(value), high.max(value))
    })
}

fn axis_distance_target(value: f32, (low, high): (f32, f32), level: f32) -> f32 {
    let normalized = if (high - low).abs() <= f32::EPSILON {
        0.5
    } else {
        (value - low) / (high - low)
    };
    let target = (level.clamp(-2.0, 2.0) + 2.0) / 4.0;
    (normalized - target).abs()
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
    let semantic = semantic::embed_audio(&path).map_err(Error::Semantic)?;
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
