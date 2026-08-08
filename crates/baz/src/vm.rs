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
//! # Album artist
//!
//! Who an album is filed under is a three-state enum all the way through
//! ([`AlbumArtistVm`], mirroring [`baz_core::index::AlbumArtist`]) — named,
//! an unnamed compilation, or unknown. The display strings for the latter
//! two live on [`AlbumArtistVm::label`] and nowhere else, so no code path
//! can confuse them with a tag that happens to read the same words
//! (ADR-0008).
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
use std::path::{Path, PathBuf};
use std::time::Duration;

use baz_core::index::{Album, AlbumArtist, Edition, Library};
use baz_core::library::{AudioFormat, TrackMeta};

/// What the shelf calls an album whose artist is not known at all.
pub const UNKNOWN_ARTIST: &str = "Unknown Artist";

/// What the shelf calls an album that is a compilation with no named album
/// artist ([`AlbumArtist::Various`]). Chosen because it is the phrase every
/// tagger, every CD sleeve and every other player already uses — but it is
/// *only* a label: nothing in baz ever matches on this string, so a file
/// whose tag genuinely reads "Various Artists" stays a
/// [`AlbumArtistVm::Named`] album and is never confused with this one.
pub const VARIOUS_ARTISTS: &str = "Various Artists";

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
    /// Who the album is filed under — the owned mirror of
    /// [`baz_core::index::AlbumArtist`].
    pub artist: AlbumArtistVm,
    /// Whether the album's *track* artists say something its header does
    /// not, in which case the side panel lists each track's own artist.
    ///
    /// True exactly when some track names an artist that is not the album's
    /// artist: a soundtrack filed under one label with a different composer
    /// per cue, or a compilation. False for the ordinary album, where a
    /// per-track artist column would repeat the header on every row.
    /// Marta's per-composer credits are the reason this exists — grouping a
    /// soundtrack into one tile must not cost the information that made it
    /// shatter in the first place.
    pub track_artists_vary: bool,
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

/// The owned, render-ready form of [`baz_core::index::AlbumArtist`].
///
/// A three-state enum rather than an `Option<String>` plus a sentinel, for
/// the reason the core type gives: "the tagger wrote *Various Artists*" and
/// "baz could not name this album's artist" must not be the same value. The
/// display strings live on [`AlbumArtistVm::label`] and nowhere else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlbumArtistVm {
    /// A named album artist.
    Named(String),
    /// A compilation with no named album artist.
    Various,
    /// Nothing known.
    Unknown,
}

impl AlbumArtistVm {
    fn from_core(artist: AlbumArtist<'_>) -> Self {
        match artist {
            AlbumArtist::Named(name) => Self::Named(name.to_owned()),
            AlbumArtist::Various => Self::Various,
            AlbumArtist::Unknown => Self::Unknown,
        }
    }

    /// What the tile caption and the panel header say. Always something —
    /// a shelf tile with a blank second line reads as a rendering bug.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Named(name) => name,
            Self::Various => VARIOUS_ARTISTS,
            Self::Unknown => UNKNOWN_ARTIST,
        }
    }

    /// The name, when the album has one — `None` for a compilation or an
    /// unknown, whose labels are baz's words rather than the library's.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Named(name) => Some(name),
            Self::Various | Self::Unknown => None,
        }
    }
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
    /// This track's *own* artist, verbatim from its tags. Kept even when it
    /// equals the album's, so the decision to show it stays one place
    /// ([`AlbumVm::track_artists_vary`]).
    pub artist: Option<String>,
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
            artist: meta.artist.clone(),
            duration: meta.duration,
            path: meta.path.clone(),
        }
    }
}

/// Build the full shelf from the library, in [`Library::albums`] order
/// (album artist, then album, case-insensitively; unknowns first). Called
/// after each applied scan batch — owned strings are cloned per rebuild,
/// which is milliseconds for a 10k-album shelf and happens off the
/// per-frame path.
pub fn build_albums(library: &Library) -> Vec<AlbumVm> {
    library
        .albums()
        .into_iter()
        .filter_map(|album| {
            let first = album.default_edition()?.tracks.first()?;
            Some(AlbumVm {
                id: album_id(album.artist, album.title),
                title: album.title.map(str::to_owned),
                track_artists_vary: track_artists_vary(&album),
                artist: AlbumArtistVm::from_core(album.artist),
                year: album.year,
                first_track: first.path.clone(),
                editions: album.editions.iter().map(build_edition).collect(),
            })
        })
        .collect()
}

/// Whether any track names an artist the album's header does not already
/// state — the condition for listing per-track artists in the side panel.
///
/// A track with no artist of its own never triggers it: it adds nothing.
/// An album with no *named* artist (a compilation, or an unknown) is
/// covered by no name at all, so any track that names one differs.
/// Comparison is case-folded, matching the grouping key, so "RODIK" and
/// "Rodik" do not read as a difference worth a whole extra line per row.
fn track_artists_vary(album: &Album<'_>) -> bool {
    let header = album.artist.name().map(str::to_lowercase);
    album
        .editions
        .iter()
        .flat_map(|edition| edition.tracks.iter())
        .filter_map(|track| track.artist.as_deref())
        .any(|artist| Some(artist.to_lowercase()) != header)
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
///
/// Shared so that every rate in the interface is spelled the same way — the
/// side panel's encoding line and the bottom bar's signal-path readout
/// ([`crate::player::PlayerState::signal_note`]) name the same 44 100 Hz
/// identically.
#[must_use]
pub fn format_sample_rate(hz: u32) -> String {
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
        // The same per-track resolution the grouping key uses, so a matched
        // track always maps onto the shelf entry it is actually filed under.
        ids.insert(album_id(AlbumArtist::of(track), track.album.as_deref()));
    }
    Some(ids)
}

/// The queue baz handed the engine: what was sent, in the order it was sent,
/// with the catalogue facts needed to *show* it.
///
/// This is deliberately one value rather than two parallel ones. The paths
/// are the [`Command::SetQueue`](baz_core::protocol::Command::SetQueue)
/// payload and the rows are what the queue panel lists, and they are built in
/// the same pass from the same edition — so the list on screen cannot drift
/// from the list the engine is playing. [`Self::paths`] is the only way to get
/// the payload, which is what makes that structural rather than a convention.
///
/// It carries no notion of *where* playback is: that is engine truth, arrives
/// as [`Event::TrackStarted`](baz_core::protocol::Event::TrackStarted), and is
/// reconciled against this record by [`Self::playing`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueVm {
    /// Title of the album the queue was built from, when it has one.
    pub album: Option<String>,
    /// Who that album is filed under, as the shelf labels it
    /// ([`AlbumArtistVm::label`]) — always something.
    pub artist: String,
    /// The tracks, in play order. Indices are queue positions, which is the
    /// unit [`Event::TrackStarted`](baz_core::protocol::Event::TrackStarted)
    /// reports in.
    pub items: Vec<QueueItemVm>,
}

/// One track in the queue, as much of it as the panel shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItemVm {
    /// Display title (the tag/inferred title, else the file name).
    pub title: String,
    /// The track's own artist, carried only when the album's header does not
    /// already cover it — the same rule, and the same field, the side panel's
    /// track list follows ([`AlbumVm::track_artists_vary`]).
    pub artist: Option<String>,
    /// Playing time, when the scan read one.
    pub duration: Option<Duration>,
    /// The file. The identity the engine addresses this track by, and what
    /// [`QueueVm::playing`] reconciles a `TrackStarted` against.
    pub path: PathBuf,
}

impl QueueVm {
    /// The `paths` payload for
    /// [`Command::SetQueue`](baz_core::protocol::Command::SetQueue): every
    /// item's path, in order, byte-for-byte verbatim from the library.
    #[must_use]
    pub fn paths(&self) -> Vec<PathBuf> {
        self.items.iter().map(|item| item.path.clone()).collect()
    }

    /// How many tracks were queued.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether nothing was queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Total playing time of the queue, over the tracks that declared one.
    #[must_use]
    pub fn total_time(&self) -> Duration {
        self.items.iter().filter_map(|item| item.duration).sum()
    }

    /// Which row the engine's last
    /// [`TrackStarted`](baz_core::protocol::Event::TrackStarted) names —
    /// `None` when this record cannot honestly claim to hold that track.
    ///
    /// The engine reports a queue *position* and a *path*, and this record is
    /// the app's own memory of what it sent. Those can disagree: a queue
    /// replaced while the previous one's last event was still in flight is the
    /// ordinary way it happens. So the position is taken as the answer **only
    /// when the path at it matches**; otherwise the path is searched for
    /// (a queue that repeats a file answers with its first occurrence), and if
    /// it is not in this queue at all the answer is nothing at all. The panel
    /// then marks no row rather than marking the wrong one — the honesty rule
    /// [`crate::player`] states, applied to a list.
    #[must_use]
    pub fn playing(&self, position: usize, path: &Path) -> Option<usize> {
        if self
            .items
            .get(position)
            .is_some_and(|item| item.path == path)
        {
            return Some(position);
        }
        self.items.iter().position(|item| item.path == path)
    }
}

/// The album's play queue: the **selected edition**'s tracks in the side
/// panel's disc/track/title order (the order [`EditionVm::tracks`] already
/// carries, straight from [`Library::albums`]).
///
/// What the panel lists is what plays: `chosen` is the same value the track
/// list was rendered from, resolved by the same [`selected_edition`], so a
/// queue can never contain a format the user was not looking at.
#[must_use]
pub fn album_queue(album: &AlbumVm, chosen: Option<EditionKey>) -> QueueVm {
    let per_track_artists = album.track_artists_vary;
    let items = selected_edition(album, chosen).map_or_else(Vec::new, |edition| {
        edition
            .tracks
            .iter()
            .map(|track| QueueItemVm {
                title: track.title.clone(),
                artist: track.artist.clone().filter(|_| per_track_artists),
                duration: track.duration,
                path: track.path.clone(),
            })
            .collect()
    });
    QueueVm {
        album: album.title.clone(),
        artist: album.artist.label().to_owned(),
        items,
    }
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
/// (album artist, album title) pair, exactly mirroring the grouping key
/// [`Library::albums`] uses (`str::to_lowercase`). Each of the three
/// [`AlbumArtist`] states gets its own marker byte, so an album filed under
/// a literal "Various Artists" tag and a nameless compilation never collide
/// on one id. Stable across processes and rebuilds — it feeds the thumbnail
/// cache key and the gradient placeholder colors.
#[must_use]
pub fn album_id(artist: AlbumArtist<'_>, album: Option<&str>) -> u64 {
    let mut hash = fnv1a(0xcbf2_9ce4_8422_2325, &[]);
    match artist {
        // 0x01 marks "unknown", distinct from any real name's bytes; 0x02
        // marks the nameless compilation.
        AlbumArtist::Unknown => hash = fnv1a(hash, &[0x01]),
        AlbumArtist::Various => hash = fnv1a(hash, &[0x02]),
        AlbumArtist::Named(name) => hash = fnv1a(hash, name.to_lowercase().as_bytes()),
    }
    // Field separator: 0x00 never appears inside a Rust string's UTF-8.
    hash = fnv1a(hash, &[0x00]);
    match album {
        None => hash = fnv1a(hash, &[0x01]),
        Some(text) => hash = fnv1a(hash, text.to_lowercase().as_bytes()),
    }
    fnv1a(hash, &[0x00])
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
    use baz_core::replaygain::ReplayGainTags;

    use super::*;

    fn meta(artist: &str, album: &str, title: &str, track: u32) -> TrackMeta {
        TrackMeta {
            path: PathBuf::from(format!("/m/{artist}/{album}/{track:02} {title}.flac")),
            artist: Some(artist.to_owned()),
            album_artist: None,
            compilation: None,
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
            stamp: None,
            replay_gain: ReplayGainTags::default(),
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
        let named = |name| album_id(AlbumArtist::Named(name), Some("Geogaddi"));
        let a = named("Boards of Canada");
        assert_eq!(a, named("boards of canada"));
        assert_ne!(
            a,
            album_id(AlbumArtist::Named("Boards of Canada"), Some("Other"))
        );
        // Unknown is distinct from Some(""), and field boundaries matter.
        assert_ne!(
            album_id(AlbumArtist::Unknown, None),
            album_id(AlbumArtist::Named(""), Some(""))
        );
        assert_ne!(
            album_id(AlbumArtist::Named("ab"), Some("c")),
            album_id(AlbumArtist::Named("a"), Some("bc"))
        );
        // The three artist states are three identities. A tag that literally
        // reads "Various Artists" is a *named* album, and must not land on
        // the same shelf entry as a nameless compilation.
        let title = Some("Cookie's Bustle");
        let states = [
            album_id(AlbumArtist::Named(VARIOUS_ARTISTS), title),
            album_id(AlbumArtist::Various, title),
            album_id(AlbumArtist::Unknown, title),
        ];
        assert_ne!(states[0], states[1]);
        assert_ne!(states[1], states[2]);
        assert_ne!(states[0], states[2]);
    }

    #[test]
    fn album_artist_labels_never_leave_a_caption_blank() {
        assert_eq!(AlbumArtistVm::Named("RODIK".into()).label(), "RODIK");
        assert_eq!(AlbumArtistVm::Various.label(), VARIOUS_ARTISTS);
        assert_eq!(AlbumArtistVm::Unknown.label(), UNKNOWN_ARTIST);
        // `name` is the library's word for it; `label` is ours.
        assert_eq!(AlbumArtistVm::Named("RODIK".into()).name(), Some("RODIK"));
        assert_eq!(AlbumArtistVm::Various.name(), None);
        assert_eq!(AlbumArtistVm::Unknown.name(), None);
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
        assert_eq!(albums[0].artist, AlbumArtistVm::Named("Abel".into()));
        // One format in, one edition out — nothing for a selector to show.
        assert_eq!(albums[0].editions.len(), 1);
        let tracks = &albums[0].editions[0].tracks;
        assert_eq!(tracks.len(), 2);
        // In-album order is by track number.
        assert_eq!(tracks[0].number, Some(1));
        assert_eq!(tracks[1].number, Some(2));
        assert_eq!(albums[1].artist, AlbumArtistVm::Named("Zed".into()));
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
        let queue = album_queue(&album, Some(mp3)).paths();
        let listed: Vec<PathBuf> = chosen.tracks.iter().map(|t| t.path.clone()).collect();
        assert_eq!(queue, listed);
        assert!(
            queue.iter().all(|p| p.to_string_lossy().contains("/mp3/")),
            "playing the MP3 edition queues MP3 files only"
        );
        // And it differs from the default queue, or the selector is a lie.
        assert_ne!(queue, album_queue(&album, None).paths());
        assert_eq!(album_queue(&album, None).len(), 2, "no duplicated tracks");
    }

    #[test]
    fn a_choice_the_album_no_longer_offers_falls_back_to_the_default() {
        let album = two_edition_album();
        // A rescan dropped the MP3 folder; the remembered key is now stale.
        let stale = EditionKey(Some(AudioFormat::Opus));
        let edition = selected_edition(&album, Some(stale)).expect("a fallback");
        assert_eq!(edition.key, EditionKey(Some(AudioFormat::Flac)));
        assert_eq!(
            album_queue(&album, Some(stale)).paths(),
            album_queue(&album, None).paths()
        );
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
        assert_ne!(
            album_queue(album, Some(unknown)).paths(),
            album_queue(album, None).paths()
        );
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

    /// The owner's soundtrack: one album artist, a different composer on
    /// every cue.
    fn soundtrack() -> Library {
        library_with(
            ["Kouhei Okamura", "Katsuhiko Nakamichi", "Miki Nagamatsu"]
                .into_iter()
                .enumerate()
                .map(|(index, composer)| {
                    let number = u32::try_from(index).expect("small") + 1;
                    TrackMeta {
                        album_artist: Some("RODIK".to_owned()),
                        ..meta(composer, "Cookie's Bustle OST (gamerip)", "Cue", number)
                    }
                })
                .collect(),
        )
    }

    #[test]
    fn a_soundtrack_is_one_tile_captioned_by_its_album_artist() {
        let albums = build_albums(&soundtrack());
        assert_eq!(albums.len(), 1, "one tile, not one per composer");
        let album = &albums[0];
        assert_eq!(album.artist, AlbumArtistVm::Named("RODIK".into()));
        assert_eq!(album.artist.label(), "RODIK");
        // The header names the album artist; the rows keep the composers.
        assert!(
            album.track_artists_vary,
            "the per-cue credits say something the header does not"
        );
        let credits: Vec<Option<&str>> = album.editions[0]
            .tracks
            .iter()
            .map(|t| t.artist.as_deref())
            .collect();
        assert_eq!(
            credits,
            [
                Some("Kouhei Okamura"),
                Some("Katsuhiko Nakamichi"),
                Some("Miki Nagamatsu"),
            ]
        );
    }

    #[test]
    fn an_ordinary_album_does_not_repeat_its_artist_on_every_row() {
        let albums = build_albums(&library_with(vec![
            meta("Stan Rogers", "Northwest Passage", "Lies", 2),
            meta("Stan Rogers", "Northwest Passage", "Passage", 1),
        ]));
        assert_eq!(albums.len(), 1);
        assert!(
            !albums[0].track_artists_vary,
            "a per-track artist column would just repeat the header"
        );

        // Case alone is not a difference worth a line per row.
        let folded = build_albums(&library_with(vec![
            TrackMeta {
                album_artist: Some("STAN ROGERS".to_owned()),
                ..meta("Stan Rogers", "Northwest Passage", "Lies", 2)
            },
            TrackMeta {
                album_artist: Some("STAN ROGERS".to_owned()),
                ..meta("stan rogers", "Northwest Passage", "Passage", 1)
            },
        ]));
        assert_eq!(folded.len(), 1);
        assert!(!folded[0].track_artists_vary);
    }

    #[test]
    fn an_album_nothing_is_known_about_shows_no_track_artists() {
        let mut stray = meta("x", "y", "z", 1);
        stray.artist = None;
        stray.album = None;
        stray.album_artist = None;
        let albums = build_albums(&library_with(vec![stray]));
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].artist, AlbumArtistVm::Unknown);
        assert_eq!(albums[0].artist.label(), UNKNOWN_ARTIST);
        assert!(
            !albums[0].track_artists_vary,
            "no track names an artist, so there is nothing to add"
        );
    }

    #[test]
    fn an_unnamed_compilation_is_labelled_and_lists_its_artists() {
        let albums = build_albums(&library_with(
            ["Alpha", "Beta"]
                .into_iter()
                .enumerate()
                .map(|(index, artist)| {
                    let number = u32::try_from(index).expect("small") + 1;
                    TrackMeta {
                        compilation: Some(true),
                        ..meta(artist, "Now That's What I Call 42", "Song", number)
                    }
                })
                .collect(),
        ));
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].artist, AlbumArtistVm::Various);
        assert_eq!(albums[0].artist.label(), VARIOUS_ARTISTS);
        assert_eq!(albums[0].artist.name(), None);
        assert!(
            albums[0].track_artists_vary,
            "the header names nobody, so every row must name someone"
        );
    }

    #[test]
    fn searching_the_album_artist_filters_the_shelf_to_that_album() {
        let library = soundtrack();
        let albums = build_albums(&library);
        // The name shown on the tile has to be a name the search box finds,
        // or the filtered shelf contradicts the unfiltered one.
        assert_eq!(visible_indices(&albums, &library, "rodik"), vec![0]);
        // A composer finds it too, through their own track.
        assert_eq!(visible_indices(&albums, &library, "Katsuhiko"), vec![0]);
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
        let queue = album_queue(&albums[0], None).paths();
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

    /// The queue record and the `SetQueue` payload are one construction, so
    /// what the panel lists is exactly what the engine was handed — including
    /// the ordering and the verbatim paths the test above pins.
    #[test]
    fn the_queue_record_carries_the_rows_and_the_payload_together() {
        let album = two_edition_album();
        let queue = album_queue(&album, None);

        assert_eq!(queue.album.as_deref(), Some("Northwest Passage"));
        assert_eq!(queue.artist, "Stan Rogers");
        assert_eq!(queue.len(), 2);
        assert!(!queue.is_empty());
        // Row order is item order is payload order.
        let titles: Vec<&str> = queue.items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["Passage", "Lies"]);
        assert_eq!(
            queue.paths(),
            queue
                .items
                .iter()
                .map(|item| item.path.clone())
                .collect::<Vec<_>>()
        );
        // The durations the scan read add up.
        assert_eq!(queue.total_time(), Duration::from_secs(400));
    }

    /// A per-track artist appears on a queue row exactly when the side panel
    /// would show it — one rule for both lists, read off the album.
    #[test]
    fn queue_rows_name_a_track_artist_only_when_the_album_header_does_not() {
        let mut ordinary = meta("Rodik", "Solo", "Alone", 1);
        ordinary.artist = Some("Rodik".to_owned());
        let library = library_with(vec![ordinary]);
        let album = &build_albums(&library)[0];
        assert!(!album.track_artists_vary);
        assert_eq!(album_queue(album, None).items[0].artist, None);

        let mut cue = meta("Various Composers", "Score", "Main Title", 1);
        cue.album_artist = Some("Various Composers".to_owned());
        cue.artist = Some("Jóhann Jóhannsson".to_owned());
        let library = library_with(vec![cue]);
        let album = &build_albums(&library)[0];
        assert!(album.track_artists_vary);
        assert_eq!(
            album_queue(album, None).items[0].artist.as_deref(),
            Some("Jóhann Jóhannsson")
        );
    }

    /// The marking rule: the engine's position is believed when the path at it
    /// agrees, the path wins when it does not, and a track this queue never
    /// held marks nothing at all.
    #[test]
    fn the_playing_row_is_resolved_by_position_then_by_path() {
        let album = two_edition_album();
        let queue = album_queue(&album, None);
        let first = queue.items[0].path.clone();
        let second = queue.items[1].path.clone();

        assert_eq!(queue.playing(0, &first), Some(0));
        assert_eq!(queue.playing(1, &second), Some(1));
        // Position and path disagree (a queue replaced under an in-flight
        // event): the path is the identity, so it wins.
        assert_eq!(queue.playing(0, &second), Some(1));
        // Position past the end, path still known.
        assert_eq!(queue.playing(99, &first), Some(0));
        // A file this queue never held marks nothing — not row 0, not row 99.
        assert_eq!(queue.playing(0, Path::new("/m/elsewhere/x.flac")), None);
        assert_eq!(queue.playing(1, Path::new("/m/elsewhere/x.flac")), None);
        // An empty queue can mark nothing whatever it is told.
        let empty = QueueVm {
            album: None,
            artist: UNKNOWN_ARTIST.to_owned(),
            items: Vec::new(),
        };
        assert!(empty.is_empty());
        assert_eq!(empty.playing(0, &first), None);
    }

    /// A queue that repeats a file answers with its first occurrence when the
    /// position cannot settle it — the only choice that is not a guess.
    #[test]
    fn a_repeated_path_resolves_by_position_first() {
        let path = PathBuf::from("/m/a/1.flac");
        let item = |title: &str| QueueItemVm {
            title: title.to_owned(),
            artist: None,
            duration: Some(Duration::from_secs(60)),
            path: path.clone(),
        };
        let queue = QueueVm {
            album: Some("Loop".to_owned()),
            artist: "A".to_owned(),
            items: vec![item("once"), item("again")],
        };
        // The position is exact and its path agrees, so it is the answer.
        assert_eq!(queue.playing(1, &path), Some(1));
        // With no usable position, the first occurrence is the answer.
        assert_eq!(queue.playing(7, &path), Some(0));
        assert_eq!(queue.total_time(), Duration::from_secs(120));
    }

    #[test]
    fn gradient_colors_are_deterministic_and_distinct() {
        let id_a = album_id(AlbumArtist::Named("a"), Some("x"));
        let id_b = album_id(AlbumArtist::Named("b"), Some("y"));
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
