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
        let generated = generate(
            &self.prompt,
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
            preview.request != self.prompt.trim() || preview.target_minutes != self.length.minutes()
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

#[cfg(feature = "vibe-analysis")]
#[expect(
    clippy::too_many_lines,
    reason = "candidate projection, duration convergence and result construction form one generation boundary"
)]
fn generate(
    prompt: &str,
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
        let selection =
            baz_vibe::select_semantic(prompt, &candidates, limit, variation, recently_offered)
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
    let selected = selection
        .paths
        .iter()
        .filter_map(|path| items.remove(path))
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
