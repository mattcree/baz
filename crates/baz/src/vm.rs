//! View models: the owned, render-ready projection of the library.
//!
//! `baz_core::index::Library` hands out borrowed snapshots
//! ([`baz_core::index::Album`]); the GUI needs owned data it can keep across
//! frames while the library keeps growing under a live scan. This module maps
//! one to the other and holds every piece of shelf logic that does not need a
//! window to be tested: album identity, editions and their selection,
//! search-to-album filtering, gradient placeholder colors, and duration
//! formatting.
//!
//! # Editions
//!
//! An album the user owns in several codecs arrives here as one
//! [`baz_core::index::Album`] with several editions (ADR-0007), and leaves as
//! one [`AlbumVm`] with several [`EditionVm`]s, best first. Which one is on
//! screen is *not* stored here: the shelf keeps a per-album
//! [`EditionKey`] and passes it to [`selected_edition`] / [`album_queue`], so
//! the whole selection rule is a pure function of (album, choice) and is
//! tested as one.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use baz_core::index::{Edition, Library};
use baz_core::library::{AudioFormat, TrackMeta};

/// Cap on tracks fetched per search keystroke. Search feeds the shelf filter
/// through track→album mapping, so the cap bounds worst-case per-keystroke
/// work; 10 000 matched tracks is far beyond what a filtered shelf can
/// usefully show, and a query that broad is on its way to more keystrokes.
pub const SEARCH_LIMIT: usize = 10_000;

/// One album tile on the shelf, owned by the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumVm {
    /// Stable identity across view-model rebuilds: a hash of the album's
    /// case-folded grouping key (see [`album_id`]). Keys the thumbnail cache
    /// and the current selection, both of which must survive the shelf
    /// growing mid-scan.
    pub id: u64,
    /// Album title as first seen on its tracks; `None` = unknown-album group.
    pub title: Option<String>,
    /// Artist as first seen; `None` = unknown-artist group.
    pub artist: Option<String>,
    /// Release year, first one any track declares.
    pub year: Option<u32>,
    /// First track's path — the file art resolution reads for an embedded
    /// picture, and whose parent directory is searched for cover files.
    /// Taken from the default edition: the best copy is the one most likely
    /// to carry good artwork.
    pub first_track: PathBuf,
    /// The formats this album is owned in, best first (see
    /// [`baz_core::index::Album::editions`]). Never empty. Exactly one for
    /// the ordinary single-format album, and the UI shows no selector then.
    pub editions: Vec<EditionVm>,
}

impl AlbumVm {
    /// Every track of every edition.
    ///
    /// The lookup surface for resolving a *playing* path back to its album:
    /// that must succeed whichever edition was queued, including one the user
    /// has since switched away from.
    pub fn all_tracks(&self) -> impl Iterator<Item = &TrackVm> {
        self.editions
            .iter()
            .flat_map(|edition| edition.tracks.iter())
    }
}

/// Identifies one edition within its album, for remembering a choice.
///
/// A wrapper rather than a bare `Option<AudioFormat>` because `None` is
/// itself a legitimate edition — the one holding tracks whose codec is not
/// known (see [`baz_core::library::TrackMeta::format`]) — so "the unnamed
/// edition" and "no choice made" must not collide in the selection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EditionKey(pub Option<AudioFormat>);

impl EditionKey {
    /// The selector's label: the codec's name, or `Unknown` for tracks whose
    /// codec the scan could not name.
    #[must_use]
    pub fn label(self) -> &'static str {
        self.0.map_or("Unknown", AudioFormat::name)
    }
}

/// One selectable format of an album, owned by the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditionVm {
    /// Which edition this is; also its selector label ([`EditionKey::label`]).
    pub key: EditionKey,
    /// A quiet one-line encoding summary — `24-bit · 96 kHz` for a lossless
    /// edition, `320 kbps` for a lossy one — or `None` when the scan read no
    /// property worth stating. Never invented: a mixed-rate edition declines
    /// to claim a rate (see [`baz_core::index::Edition::bit_depth`]).
    pub detail: Option<String>,
    /// This edition's tracks in disc/track/title order, for the side panel.
    pub tracks: Vec<TrackVm>,
}

impl EditionVm {
    /// The side panel's encoding line: `FLAC · 16-bit · 44.1 kHz`, or as
    /// much of it as the scan actually established.
    ///
    /// `None` when the codec is unknown *and* no property was read — there
    /// would be nothing to say, and an empty line saying it is worse than no
    /// line at all.
    #[must_use]
    pub fn encoding_line(&self) -> Option<String> {
        match (self.key.0, self.detail.as_deref()) {
            (None, None) => None,
            (None, Some(detail)) => Some(detail.to_owned()),
            (Some(format), None) => Some(format.name().to_owned()),
            (Some(format), Some(detail)) => Some(format!("{} · {detail}", format.name())),
        }
    }
}

/// One row in the side panel's track list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackVm {
    /// Track number within its disc, when known.
    pub number: Option<u32>,
    /// Display title: the tag/inferred title, else the file name.
    pub title: String,
    /// Playing time, when the scan could read it cheaply.
    pub duration: Option<Duration>,
    /// The audio file — the future playback seam addresses tracks by path.
    pub path: PathBuf,
}

impl TrackVm {
    fn from_meta(meta: &TrackMeta) -> Self {
        let title = meta.title.clone().unwrap_or_else(|| {
            meta.path
                .file_name()
                .map_or_else(|| String::from("?"), |n| n.to_string_lossy().into_owned())
        });
        Self {
            number: meta.track,
            title,
            duration: meta.duration,
            path: meta.path.clone(),
        }
    }
}

/// Build the full shelf from the library, in [`Library::albums`] order
/// (artist, then album, case-insensitively; unknowns first). Called after
/// each applied scan batch — owned strings are cloned per rebuild, which is
/// milliseconds for a 10k-album shelf and happens off the per-frame path.
pub fn build_albums(library: &Library) -> Vec<AlbumVm> {
    library
        .albums()
        .into_iter()
        .filter_map(|album| {
            let first = album.default_edition()?.tracks.first()?;
            Some(AlbumVm {
                id: album_id(album.artist, album.title),
                title: album.title.map(str::to_owned),
                artist: album.artist.map(str::to_owned),
                year: album.year,
                first_track: first.path.clone(),
                editions: album.editions.iter().map(build_edition).collect(),
            })
        })
        .collect()
}

/// Project one core edition into its owned, render-ready form.
fn build_edition(edition: &Edition<'_>) -> EditionVm {
    EditionVm {
        key: EditionKey(edition.format),
        detail: edition_detail(edition),
        tracks: edition
            .tracks
            .iter()
            .map(|t| TrackVm::from_meta(t))
            .collect(),
    }
}

/// The encoding summary shown under an album's title.
///
/// A lossless edition is described by what it preserves — depth and rate; a
/// lossy one by what it spends — bitrate. Quoting a sample rate for an MP3
/// and calling it a quality statement would be the wrong number in the right
/// place, so each tier gets the figure that actually means something for it.
fn edition_detail(edition: &Edition<'_>) -> Option<String> {
    if edition.is_lossless() {
        let mut parts: Vec<String> = Vec::new();
        if let Some(depth) = edition.bit_depth() {
            parts.push(format!("{depth}-bit"));
        }
        if let Some(rate) = edition.sample_rate() {
            parts.push(format_sample_rate(rate));
        }
        if !parts.is_empty() {
            return Some(parts.join(" · "));
        }
    }
    edition.bitrate().map(|kbps| format!("{kbps} kbps"))
}

/// A sample rate in kHz, to one decimal and no trailing `.0`: `44.1 kHz`,
/// `48 kHz`, `96 kHz`.
fn format_sample_rate(hz: u32) -> String {
    let tenths = hz.saturating_add(50) / 100; // hz/100 kHz, rounded half-up
    let (whole, fraction) = (tenths / 10, tenths % 10);
    if fraction == 0 {
        format!("{whole} kHz")
    } else {
        format!("{whole}.{fraction} kHz")
    }
}

/// The edition to show and play for `album`: the one the user chose, when
/// the album still has it, else the default (best-ranked) edition.
///
/// The fallback is what keeps a stale choice harmless. Editions come and go
/// while a scan runs and after a rescan of a changed folder, and a remembered
/// format that has vanished must silently become "the best one available"
/// rather than an empty track list.
#[must_use]
pub fn selected_edition(album: &AlbumVm, chosen: Option<EditionKey>) -> Option<&EditionVm> {
    chosen
        .and_then(|key| album.editions.iter().find(|edition| edition.key == key))
        .or_else(|| album.editions.first())
}

/// The album ids matching `query`, or `None` when the query is blank (no
/// filter — show the whole shelf).
///
/// [`Library::search`] returns *tracks*; the shelf shows *albums*. Each
/// matched track is mapped to its album's identity and deduplicated into a
/// set; the caller filters the existing shelf against the set, so shelf
/// ordering is preserved (no relevance reordering — the shelf is a place,
/// not a ranking). Capped at [`SEARCH_LIMIT`] tracks per keystroke.
pub fn matching_album_ids(library: &Library, query: &str) -> Option<HashSet<u64>> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    let mut ids = HashSet::new();
    for track in library.search(query, SEARCH_LIMIT) {
        ids.insert(album_id(track.artist.as_deref(), track.album.as_deref()));
    }
    Some(ids)
}

/// The album's play queue: every path of the **selected edition** in the
/// side panel's disc/track/title order (the order [`EditionVm::tracks`]
/// already carries, straight from [`Library::albums`]), byte-for-byte
/// verbatim — this is exactly the `paths` payload for
/// [`Command::SetQueue`](baz_core::protocol::Command::SetQueue).
///
/// What the panel lists is what plays: `chosen` is the same value the track
/// list was rendered from, resolved by the same [`selected_edition`], so a
/// queue can never contain a format the user was not looking at.
#[must_use]
pub fn album_queue(album: &AlbumVm, chosen: Option<EditionKey>) -> Vec<PathBuf> {
    selected_edition(album, chosen).map_or_else(Vec::new, |edition| {
        edition
            .tracks
            .iter()
            .map(|track| track.path.clone())
            .collect()
    })
}

/// Indices into `albums` that survive the current query filter (all of them
/// for a blank query). This is the shelf's render list.
pub fn visible_indices(albums: &[AlbumVm], library: &Library, query: &str) -> Vec<usize> {
    match matching_album_ids(library, query) {
        None => (0..albums.len()).collect(),
        Some(ids) => albums
            .iter()
            .enumerate()
            .filter(|(_, album)| ids.contains(&album.id))
            .map(|(i, _)| i)
            .collect(),
    }
}

/// Deterministic album identity: FNV-1a 64 over the case-folded
/// artist/album pair, exactly mirroring the grouping key
/// [`Library::albums`] uses (`str::to_lowercase`), with `None` and `Some`
/// kept distinct. Stable across processes and rebuilds — it feeds the
/// thumbnail cache key and the gradient placeholder colors.
#[must_use]
pub fn album_id(artist: Option<&str>, album: Option<&str>) -> u64 {
    let mut hash = fnv1a(0xcbf2_9ce4_8422_2325, &[]);
    for field in [artist, album] {
        match field {
            // 0x01 marks "unknown", distinct from any real name's bytes.
            None => hash = fnv1a(hash, &[0x01]),
            Some(text) => hash = fnv1a(hash, text.to_lowercase().as_bytes()),
        }
        // Field separator: 0x00 never appears inside a Rust string's UTF-8.
        hash = fnv1a(hash, &[0x00]);
    }
    hash
}

/// One FNV-1a 64 round over `bytes`, continuing from `hash`.
fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Two deterministic RGB colors for an album's placeholder gradient
/// (hash → HSL → RGB, ported from the Phase 1 spike): saturation and
/// lightness are clamped to a mid range so white text stays readable on
/// every generated pair.
#[must_use]
pub fn gradient_colors(album_id: u64) -> ([u8; 3], [u8; 3]) {
    let color = |salt: u64| -> [u8; 3] {
        let v = splitmix64(album_id ^ salt.wrapping_mul(0x9E37));
        #[expect(
            clippy::cast_precision_loss,
            reason = "values are reduced modulo small ranges before the cast"
        )]
        let (h, s, l) = (
            (v % 360) as f32,
            0.35 + ((v >> 16) % 35) as f32 / 100.0,
            0.22 + ((v >> 32) % 28) as f32 / 100.0,
        );
        hsl_to_rgb(h, s, l)
    };
    (color(1), color(2))
}

/// splitmix64 — tiny, well-known PRNG step (same as the spike's).
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Standard HSL→RGB conversion (h in degrees, s/l in 0..=1).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "channel values are clamped to 0.0..=1.0 before scaling to u8"
)]
#[expect(
    clippy::many_single_char_names,
    reason = "h/s/l/c/x/m are the textbook variable names for this conversion"
)]
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [
        ((r1 + m).clamp(0.0, 1.0) * 255.0).round() as u8,
        ((g1 + m).clamp(0.0, 1.0) * 255.0).round() as u8,
        ((b1 + m).clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

/// `m:ss` (or `h:mm:ss`) for track durations.
#[must_use]
pub fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(artist: &str, album: &str, title: &str, track: u32) -> TrackMeta {
        TrackMeta {
            path: PathBuf::from(format!("/m/{artist}/{album}/{track:02} {title}.flac")),
            artist: Some(artist.to_owned()),
            album: Some(album.to_owned()),
            title: Some(title.to_owned()),
            track: Some(track),
            disc: None,
            year: Some(2020),
            duration: Some(Duration::from_secs(200)),
            format: None,
            bit_depth: None,
            sample_rate: None,
            bitrate: None,
        }
    }

    /// The same track, encoded: path under a per-format root, so the two
    /// editions of an album have genuinely different files.
    fn encoded(album: &str, title: &str, track: u32, format: AudioFormat) -> TrackMeta {
        let lossless = format.is_lossless();
        TrackMeta {
            path: PathBuf::from(format!(
                "/m/{}/Stan Rogers/{album}/{track:02} {title}.x",
                format.code()
            )),
            format: Some(format),
            bit_depth: lossless.then_some(16),
            sample_rate: Some(44_100),
            bitrate: Some(if lossless { 900 } else { 320 }),
            ..meta("Stan Rogers", album, title, track)
        }
    }

    /// The owner's case: one album, two rips.
    fn two_edition_album() -> AlbumVm {
        let library = library_with(vec![
            encoded("Northwest Passage", "Lies", 2, AudioFormat::Mp3),
            encoded("Northwest Passage", "Passage", 1, AudioFormat::Flac),
            encoded("Northwest Passage", "Lies", 2, AudioFormat::Flac),
            encoded("Northwest Passage", "Passage", 1, AudioFormat::Mp3),
        ]);
        let mut albums = build_albums(&library);
        assert_eq!(albums.len(), 1, "one tile per album, not per format");
        albums.remove(0)
    }

    fn library_with(tracks: Vec<TrackMeta>) -> Library {
        let mut library = Library::open_in_memory().expect("in-memory library");
        library.add_tracks(tracks).expect("add tracks");
        library
    }

    #[test]
    fn album_id_is_deterministic_and_case_folded() {
        let a = album_id(Some("Boards of Canada"), Some("Geogaddi"));
        assert_eq!(a, album_id(Some("boards of canada"), Some("GEOGADDI")));
        assert_ne!(a, album_id(Some("Boards of Canada"), Some("Other")));
        // None is distinct from Some(""), and field boundaries matter.
        assert_ne!(album_id(None, None), album_id(Some(""), Some("")));
        assert_ne!(
            album_id(Some("ab"), Some("c")),
            album_id(Some("a"), Some("bc"))
        );
    }

    #[test]
    fn build_albums_groups_and_orders() {
        let library = library_with(vec![
            meta("Zed", "Last", "One", 1),
            meta("Abel", "First", "Two", 2),
            meta("Abel", "First", "One", 1),
        ]);
        let albums = build_albums(&library);
        assert_eq!(albums.len(), 2);
        assert_eq!(albums[0].artist.as_deref(), Some("Abel"));
        // One format in, one edition out — nothing for a selector to show.
        assert_eq!(albums[0].editions.len(), 1);
        let tracks = &albums[0].editions[0].tracks;
        assert_eq!(tracks.len(), 2);
        // In-album order is by track number.
        assert_eq!(tracks[0].number, Some(1));
        assert_eq!(tracks[1].number, Some(2));
        assert_eq!(albums[1].artist.as_deref(), Some("Zed"));
        // Ids are unique per shelf entry.
        assert_ne!(albums[0].id, albums[1].id);
        // First track path feeds art resolution.
        assert_eq!(albums[0].first_track, tracks[0].path);
    }

    #[test]
    fn editions_default_to_the_ranked_best_and_list_only_their_own_tracks() {
        let album = two_edition_album();
        assert_eq!(album.editions.len(), 2);
        assert_eq!(
            album.editions.iter().map(|e| e.key).collect::<Vec<_>>(),
            [
                EditionKey(Some(AudioFormat::Flac)),
                EditionKey(Some(AudioFormat::Mp3)),
            ],
            "lossless first"
        );

        // No choice yet: the best edition, and only its tracks — not the
        // interleaved union that album grouping used to produce.
        let default = selected_edition(&album, None).expect("a default edition");
        assert_eq!(default.key.label(), "FLAC");
        assert_eq!(default.detail.as_deref(), Some("16-bit · 44.1 kHz"));
        assert_eq!(default.tracks.len(), 2);
        assert!(
            default
                .tracks
                .iter()
                .all(|t| t.path.to_string_lossy().contains("/flac/")),
            "the FLAC edition lists FLAC files only"
        );
        // Art still comes from the default edition's first track.
        assert_eq!(album.first_track, default.tracks[0].path);
    }

    #[test]
    fn choosing_an_edition_changes_both_the_track_list_and_the_queue() {
        let album = two_edition_album();
        let mp3 = EditionKey(Some(AudioFormat::Mp3));

        let chosen = selected_edition(&album, Some(mp3)).expect("the MP3 edition");
        assert_eq!(chosen.key, mp3);
        assert_eq!(chosen.key.label(), "MP3");
        assert_eq!(
            chosen.detail.as_deref(),
            Some("320 kbps"),
            "a lossy edition is described by its bitrate, not its sample rate"
        );

        // The queue is exactly the listed edition, in the listed order.
        let queue = album_queue(&album, Some(mp3));
        let listed: Vec<PathBuf> = chosen.tracks.iter().map(|t| t.path.clone()).collect();
        assert_eq!(queue, listed);
        assert!(
            queue.iter().all(|p| p.to_string_lossy().contains("/mp3/")),
            "playing the MP3 edition queues MP3 files only"
        );
        // And it differs from the default queue, or the selector is a lie.
        assert_ne!(queue, album_queue(&album, None));
        assert_eq!(album_queue(&album, None).len(), 2, "no duplicated tracks");
    }

    #[test]
    fn a_choice_the_album_no_longer_offers_falls_back_to_the_default() {
        let album = two_edition_album();
        // A rescan dropped the MP3 folder; the remembered key is now stale.
        let stale = EditionKey(Some(AudioFormat::Opus));
        let edition = selected_edition(&album, Some(stale)).expect("a fallback");
        assert_eq!(edition.key, EditionKey(Some(AudioFormat::Flac)));
        assert_eq!(album_queue(&album, Some(stale)), album_queue(&album, None));
    }

    #[test]
    fn an_unnamed_codec_is_a_selectable_edition_distinct_from_no_choice() {
        // Rows a v1 upgrade could not backfill sit alongside rescanned ones.
        let library = library_with(vec![
            encoded("Mixed", "One", 1, AudioFormat::Flac),
            meta("Stan Rogers", "Mixed", "One", 1),
        ]);
        let albums = build_albums(&library);
        assert_eq!(albums.len(), 1);
        let album = &albums[0];
        assert_eq!(album.editions.len(), 2);

        let unknown = EditionKey(None);
        assert_eq!(unknown.label(), "Unknown");
        assert_eq!(album.editions[1].key, unknown, "unnamed ranks last");
        // Selecting it is a real choice, not "no choice": it must not
        // collapse into the default.
        let chosen = selected_edition(album, Some(unknown)).expect("the unnamed edition");
        assert_eq!(chosen.key, unknown);
        assert_ne!(album_queue(album, Some(unknown)), album_queue(album, None));
    }

    #[test]
    fn all_tracks_spans_every_edition_so_playback_always_resolves() {
        let album = two_edition_album();
        assert_eq!(album.all_tracks().count(), 4);
        // A path from the *non*-selected edition still resolves — the user
        // may switch editions while that one is still playing.
        let playing = &album.editions[1].tracks[0].path;
        assert!(album.all_tracks().any(|t| &t.path == playing));
    }

    #[test]
    fn the_encoding_line_states_only_what_was_established() {
        let album = two_edition_album();
        assert_eq!(
            album.editions[0].encoding_line().as_deref(),
            Some("FLAC · 16-bit · 44.1 kHz")
        );
        assert_eq!(
            album.editions[1].encoding_line().as_deref(),
            Some("MP3 · 320 kbps")
        );
        // A codec with nothing read about it still names itself...
        let bare_format = EditionVm {
            key: EditionKey(Some(AudioFormat::Wav)),
            detail: None,
            tracks: Vec::new(),
        };
        assert_eq!(bare_format.encoding_line().as_deref(), Some("WAV"));
        // ...and an edition with nothing at all says nothing at all.
        let nothing = EditionVm {
            key: EditionKey(None),
            detail: None,
            tracks: Vec::new(),
        };
        assert_eq!(nothing.encoding_line(), None);
    }

    #[test]
    fn sample_rates_read_the_way_they_are_spoken() {
        assert_eq!(format_sample_rate(44_100), "44.1 kHz");
        assert_eq!(format_sample_rate(48_000), "48 kHz");
        assert_eq!(format_sample_rate(96_000), "96 kHz");
        assert_eq!(format_sample_rate(192_000), "192 kHz");
        assert_eq!(format_sample_rate(8_000), "8 kHz");
        assert_eq!(format_sample_rate(22_050), "22.1 kHz");
    }

    #[test]
    fn track_vm_title_falls_back_to_file_name() {
        let mut stray = meta("A", "B", "T", 1);
        stray.title = None;
        stray.path = PathBuf::from("/m/A/B/03 mystery.flac");
        let vm = TrackVm::from_meta(&stray);
        assert_eq!(vm.title, "03 mystery.flac");
    }

    #[test]
    fn search_filter_maps_tracks_to_albums_and_preserves_order() {
        let library = library_with(vec![
            meta("Abel", "Alpha", "Sunrise", 1),
            meta("Bea", "Beta", "Sunset", 1),
            meta("Bea", "Beta", "Moonrise", 2),
            meta("Cid", "Gamma", "Noon", 1),
        ]);
        let albums = build_albums(&library);

        // Blank query: everything visible, shelf order.
        assert_eq!(visible_indices(&albums, &library, ""), vec![0, 1, 2]);
        assert_eq!(visible_indices(&albums, &library, "   "), vec![0, 1, 2]);

        // "sun" matches tracks on two albums; both tracks of Beta dedupe to
        // one album; order stays shelf order (Alpha before Beta).
        let visible = visible_indices(&albums, &library, "sun");
        assert_eq!(visible.len(), 2);
        assert_eq!(albums[visible[0]].title.as_deref(), Some("Alpha"));
        assert_eq!(albums[visible[1]].title.as_deref(), Some("Beta"));

        // Case-insensitive, and artist matches count too.
        assert_eq!(visible_indices(&albums, &library, "CID").len(), 1);
        // No match: empty shelf, not "no filter".
        assert!(visible_indices(&albums, &library, "zzz").is_empty());
    }

    #[test]
    fn album_queue_orders_by_disc_then_track_with_verbatim_paths() {
        // Deliberately shuffled input across two discs, with a path that
        // exercises spaces and non-ASCII — queue paths must be the library's
        // paths byte-for-byte.
        let odd_path = PathBuf::from("/m/Ártist/Dühble Album/d2 01 — søng.flac");
        let mut d2t1 = meta("Artist", "Double", "Song", 1);
        d2t1.disc = Some(2);
        d2t1.path = odd_path.clone();
        let mut d1t2 = meta("Artist", "Double", "Later", 2);
        d1t2.disc = Some(1);
        let mut d1t1 = meta("Artist", "Double", "Early", 1);
        d1t1.disc = Some(1);

        let library = library_with(vec![d2t1, d1t2, d1t1.clone()]);
        let albums = build_albums(&library);
        assert_eq!(albums.len(), 1);
        let queue = album_queue(&albums[0], None);
        assert_eq!(
            queue,
            vec![
                d1t1.path.clone(),
                PathBuf::from("/m/Artist/Double/02 Later.flac"),
                odd_path,
            ],
            "disc 1 tracks 1..2, then disc 2 track 1; paths verbatim"
        );
        // Fidelity both ways: every queued path is a library track path.
        for path in &queue {
            assert!(
                albums[0].all_tracks().any(|t| &t.path == path),
                "queue path {path:?} must come from the album's tracks"
            );
        }
    }

    #[test]
    fn gradient_colors_are_deterministic_and_distinct() {
        let id_a = album_id(Some("a"), Some("x"));
        let id_b = album_id(Some("b"), Some("y"));
        assert_eq!(gradient_colors(id_a), gradient_colors(id_a));
        assert_ne!(gradient_colors(id_a), gradient_colors(id_b));
        let (c1, c2) = gradient_colors(id_a);
        assert_ne!(c1, c2, "the two gradient stops should differ");
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0:00");
        assert_eq!(format_duration(Duration::from_secs(243)), "4:03");
        assert_eq!(format_duration(Duration::from_secs(3723)), "1:02:03");
    }
}
