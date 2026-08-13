//! Home's opt-in, local sonic-playlist state.
//!
//! The full build delegates decoding, MIR extraction, persistence and ranking
//! to the optional `baz-vibe` crate. A light build retains the same Home seam
//! but contains no analyzer dependency or model/runtime payload.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use crate::vm::{self, AlbumVm, EditionKey, QueueItemVm};

/// An ordinary generated playlist is deliberately bounded and editable.
#[cfg(feature = "vibe-analysis")]
pub(crate) const PLAYLIST_LEN: usize = 24;

#[cfg(feature = "vibe-analysis")]
type SonicFeatures = baz_vibe::Features;

#[cfg(not(feature = "vibe-analysis"))]
type SonicFeatures = u8;

/// The listener-facing controlled vocabulary for the first sonic build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Preset {
    Calm,
    Warm,
    #[default]
    Focus,
    Bright,
    Drive,
}

impl Preset {
    pub(crate) const ALL: [Self; 5] = [
        Self::Calm,
        Self::Warm,
        Self::Focus,
        Self::Bright,
        Self::Drive,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Calm => "Calm",
            Self::Warm => "Warm",
            Self::Focus => "Focus",
            Self::Bright => "Bright",
            Self::Drive => "Drive",
        }
    }

    #[cfg(feature = "vibe-analysis")]
    fn profile(self, seed: Option<PathBuf>) -> baz_vibe::Profile {
        let (energy, brightness) = match self {
            Self::Calm => (-2, -1),
            Self::Warm => (-1, -2),
            Self::Focus => (-1, 0),
            Self::Bright => (0, 2),
            Self::Drive => (2, 1),
        };
        baz_vibe::Profile {
            energy,
            brightness,
            seed,
        }
    }
}

/// The preview before it becomes a normal playlist file.
#[derive(Debug, Clone)]
pub(crate) struct Generated {
    pub(crate) description: String,
    pub(crate) items: Vec<QueueItemVm>,
    pub(crate) pool_tracks: usize,
    pub(crate) analyzed_tracks: usize,
    pub(crate) tempo_span: Option<(f32, f32)>,
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
#[derive(Debug, Default)]
pub(crate) struct State {
    pub(crate) open: bool,
    pub(crate) preset: Preset,
    pub(crate) seed: Option<PathBuf>,
    pub(crate) preparing: bool,
    pub(crate) analyzing: bool,
    pub(crate) total: usize,
    pub(crate) done: usize,
    pub(crate) failed: usize,
    pub(crate) current: Option<PathBuf>,
    pub(crate) error: Option<String>,
    pub(crate) preview: Option<Generated>,
    features: HashMap<PathBuf, SonicFeatures>,
    pending: VecDeque<PathBuf>,
    run: u64,
}

impl State {
    pub(crate) fn begin(&mut self) {
        self.open = true;
        self.error = None;
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
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
        self.error = None;
        self.preview = None;
        self.pending.clear();
    }

    pub(crate) fn accept_preparation(&mut self, result: Result<Preparation, String>) {
        self.preparing = false;
        match result {
            Ok(prepared) => {
                self.features = prepared.ready;
                self.pending = prepared.pending.into();
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

    pub(crate) fn next_job(&mut self) -> Option<(u64, PathBuf)> {
        if !self.analyzing || self.current.is_some() {
            return None;
        }
        let path = self.pending.pop_front()?;
        self.current = Some(path.clone());
        Some((self.run, path))
    }

    pub(crate) fn accept_analysis(&mut self, result: AnalysisResult) {
        if result.run != self.run || !self.analyzing {
            return;
        }
        self.current = None;
        match result.features {
            Ok(features) => {
                self.features.insert(result.path, features);
                self.done = self.done.saturating_add(1);
            }
            Err(error) => {
                self.failed = self.failed.saturating_add(1);
                self.error = Some(error);
            }
        }
        if self.pending.is_empty() {
            self.analyzing = false;
        }
    }

    pub(crate) fn cancel_analysis(&mut self) {
        self.run = self.run.wrapping_add(1);
        self.preparing = false;
        self.analyzing = false;
        self.current = None;
        self.pending.clear();
        self.error = None;
    }

    pub(crate) fn choose(
        &mut self,
        preset: Preset,
        albums: &[AlbumVm],
        chosen: &HashMap<u64, EditionKey>,
    ) {
        self.preset = preset;
        self.rebuild(albums, chosen);
    }

    pub(crate) fn use_seed(
        &mut self,
        path: Option<PathBuf>,
        albums: &[AlbumVm],
        chosen: &HashMap<u64, EditionKey>,
    ) {
        self.seed = path.filter(|path| self.features.contains_key(path));
        self.rebuild(albums, chosen);
    }

    pub(crate) fn rebuild(&mut self, albums: &[AlbumVm], chosen: &HashMap<u64, EditionKey>) {
        self.preview = generate(
            self.preset,
            self.seed.clone(),
            &self.features,
            albums,
            chosen,
        );
    }

    pub(crate) fn has_features(&self) -> bool {
        !self.features.is_empty()
    }

    pub(crate) fn can_seed(&self, path: &Path) -> bool {
        self.features.contains_key(path)
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
                    eprintln!("[vibe] skipped {}: {error}", path.display());
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

#[cfg(feature = "vibe-analysis")]
fn generate(
    preset: Preset,
    seed: Option<PathBuf>,
    features: &HashMap<PathBuf, SonicFeatures>,
    albums: &[AlbumVm],
    chosen: &HashMap<u64, EditionKey>,
) -> Option<Generated> {
    let mut candidates = Vec::new();
    let mut items = HashMap::new();
    let pool_tracks = library_paths(albums, chosen).len();
    for album in albums {
        let Some(edition) = vm::selected_edition(album, chosen.get(&album.id).copied()) else {
            continue;
        };
        for track in &edition.tracks {
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
        return None;
    }
    let profile = preset.profile(seed.clone());
    let selection = baz_vibe::select(&profile, &candidates, PLAYLIST_LEN);
    let selected = selection
        .paths
        .iter()
        .filter_map(|path| items.remove(path))
        .collect();
    let description = seed.map_or_else(
        || format!("{} · local sonic features", preset.label()),
        |path| {
            format!(
                "{} · shaped around {} · local sonic features",
                preset.label(),
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("the sounding track")
            )
        },
    );
    Some(Generated {
        description,
        items: selected,
        pool_tracks,
        analyzed_tracks: selection.pool_tracks,
        tempo_span: selection.tempo_span,
    })
}

#[cfg(not(feature = "vibe-analysis"))]
fn generate(
    _preset: Preset,
    _seed: Option<PathBuf>,
    _features: &HashMap<PathBuf, SonicFeatures>,
    _albums: &[AlbumVm],
    _chosen: &HashMap<u64, EditionKey>,
) -> Option<Generated> {
    None
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

    #[test]
    fn analysis_scope_is_the_selected_library_edition() {
        assert_eq!(
            library_paths(&[album()], &HashMap::new()),
            [PathBuf::from("/m/one.flac")]
        );
    }

    #[test]
    fn cancel_invalidates_a_late_worker_result() {
        let mut state = State::default();
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
