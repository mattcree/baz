//! Optional, completely local musical analysis for baz.
//!
//! This crate is deliberately outside the player and GUI crates. It decodes a
//! file through baz-core's offline decoder, extracts conventional music-
//! information-retrieval features with bliss, and owns a replaceable SQLite
//! cache. Nothing here is reachable from the realtime playback thread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use baz_core::playback::{AudioSource, resample_interleaved};
use bliss_audio::{Analysis, AnalysisIndex, FeaturesVersion, Song};
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;

/// Sample rate required by bliss' feature extractors.
const ANALYSIS_RATE: u32 = 22_050;
/// Stereo channels produced by baz-core's offline decoder.
const CHANNELS: usize = 2;
/// Schema of the independent, disposable analysis cache.
const STORE_VERSION: i64 = 1;

/// A conventional local description of one track.
#[derive(Debug, Clone, PartialEq)]
pub struct Features {
    /// Normalized bliss feature vector. Its version travels beside it in the
    /// store, so incompatible analyzer upgrades are never mixed.
    values: Vec<f32>,
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

    fn at(&self, index: AnalysisIndex) -> f32 {
        self.values[index as usize]
    }
}

/// Select tracks near the requested collection-relative targets, then order
/// the shortlist by local sonic continuity while enforcing artist and album
/// diversity. Retrieval and sequencing are deliberately separate: nearest
/// neighbours alone make repetitive, poorly flowing playlists.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "retrieval, diversity and sequencing stay together as one auditable playlist policy"
)]
pub fn select(profile: &Profile, candidates: &[Candidate], limit: usize) -> Selection {
    if profile.is_empty() || candidates.is_empty() || limit == 0 {
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
    let seed = profile.seed.as_ref().and_then(|path| {
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
            let mut score = 0.0_f32;
            let mut weights = 0.0_f32;
            if profile.energy != 0 {
                score += axis_distance(candidate.features.energy(), energy_range, profile.energy);
                weights += 1.0;
            }
            if profile.brightness != 0 {
                score += axis_distance(
                    candidate.features.brightness(),
                    bright_range,
                    profile.brightness,
                );
                weights += 1.0;
            }
            if let Some(seed) = seed {
                score += 1.5 * seed.distance(&candidate.features) / max_seed_distance;
                weights += 1.5;
            }
            (index, score / weights.max(1.0))
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
                    && (!require_fresh_album || !album_counts.contains_key(&candidate.album))
            })
            .min_by(
                |(_, (left_index, left_fit)), (_, (right_index, right_fit))| {
                    let cost = |index: usize, fit: f32| {
                        let transition = previous.map_or(0.0, |previous| {
                            previous.distance(&candidates[index].features) / transition_scale
                        });
                        0.7 * fit + 0.3 * transition
                    };
                    cost(*left_index, *left_fit)
                        .total_cmp(&cost(*right_index, *right_fit))
                        .then_with(|| left_index.cmp(right_index))
                },
            )
            .map(|(position, _)| position);
        let Some(next) = next else {
            break;
        };
        let (index, _) = scored.remove(next);
        *artist_counts
            .entry(candidates[index].artist.as_str())
            .or_default() += 1;
        *album_counts.entry(candidates[index].album).or_default() += 1;
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

fn range(values: impl Iterator<Item = f32>) -> (f32, f32) {
    values.fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), value| {
        (low.min(value), high.max(value))
    })
}

fn axis_distance(value: f32, (low, high): (f32, f32), level: i8) -> f32 {
    let normalized = if (high - low).abs() <= f32::EPSILON {
        0.5
    } else {
        (value - low) / (high - low)
    };
    let target = (f32::from(level.clamp(-2, 2)) + 2.0) / 4.0;
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
    let after = Stamp::read(&path)?;
    if before != after {
        return Err(Error::Analyze {
            path,
            detail: "the file changed while it was being analysed".to_owned(),
        });
    }
    let features = Features {
        values: analysis.as_vec(),
    };
    Store::open(store_path)?.put(&path, after, &analysis)?;
    Ok(Analyzed { path, features })
}

struct Store {
    connection: Connection,
}

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
        connection.execute_batch(&format!(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS features (
                 path BLOB PRIMARY KEY NOT NULL,
                 bytes INTEGER NOT NULL,
                 modified_ns INTEGER NOT NULL,
                 feature_version INTEGER NOT NULL,
                 values_blob BLOB NOT NULL
             );
             PRAGMA user_version={STORE_VERSION};"
        ))?;
        Ok(Self { connection })
    }

    fn current(&self, path: &Path, stamp: Stamp) -> Result<Option<Features>, Error> {
        let row: Option<(i64, i64, i64, Vec<u8>)> = self
            .connection
            .query_row(
                "SELECT bytes, modified_ns, feature_version, values_blob
                   FROM features WHERE path = ?1",
                [path_bytes(path)],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let Some((bytes, modified_ns, version, blob)) = row else {
            return Ok(None);
        };
        if u64::try_from(bytes).ok() != Some(stamp.bytes)
            || modified_ns != stamp.modified_ns
            || version != i64::from(u16::from(FeaturesVersion::LATEST))
        {
            return Ok(None);
        }
        Ok(decode_values(&blob).ok().map(|values| Features { values }))
    }

    fn put(&self, path: &Path, stamp: Stamp, analysis: &Analysis) -> Result<(), Error> {
        let bytes = i64::try_from(stamp.bytes).unwrap_or(i64::MAX);
        let version = i64::from(u16::from(analysis.features_version));
        self.connection.execute(
            "INSERT INTO features(path, bytes, modified_ns, feature_version, values_blob)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
                bytes=excluded.bytes,
                modified_ns=excluded.modified_ns,
                feature_version=excluded.feature_version,
                values_blob=excluded.values_blob",
            params![
                path_bytes(path),
                bytes,
                stamp.modified_ns,
                version,
                encode_values(&analysis.as_vec())
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
            features: Features { values },
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
            .execute_batch("PRAGMA user_version=2")
            .expect("future version");
        drop(connection);

        assert!(matches!(
            prepare(&store, Vec::new()),
            Err(Error::UnsupportedStoreVersion {
                found: 2,
                supported: STORE_VERSION
            })
        ));
        let connection = Connection::open(store).expect("reopen");
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, 2);
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
