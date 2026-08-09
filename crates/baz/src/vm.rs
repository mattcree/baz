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

use baz_core::history::{History, Recency};
use baz_core::index::{Album, AlbumArtist, Edition, GroupHeader, GroupKey, Initial, Library};
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

/// How many rows the **Songs** section shows
/// (`docs/design/09-implicit-playlists.md` §5): the ranked head of the match
/// set, not an exhaustive list — the filtered wall below is the exhaustive
/// answer, in covers. Eight echoes the room's other handful,
/// [`crate::shuffle::SLEEVES`].
pub const SONGS: usize = 8;

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
    /// Genre, **verbatim from the tags** — the first one any track declares,
    /// exactly as it is spelled (see [`baz_core::index::Album::genre`]). A
    /// library that carries `Post-Rock`, `post rock` and `Rock; Instrumental`
    /// shows three genres here, because it *has* three genre tags.
    pub genre: Option<String>,
    /// When the library first saw this album, in nanoseconds since the Unix
    /// epoch, or `None` when every track predates the schema that started
    /// recording it — permanently, because no later scan can discover when a
    /// file arrived.
    pub first_seen_ns: Option<i64>,
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
    /// Mean bitrate in kbit/s over the tracks that declare one — the
    /// `Bitrate` row of the inspector's **Details** block. `None` when no
    /// track declared one, which is the truth and not a zero.
    pub bitrate: Option<u32>,
    /// Bit depth, when every track in the edition agrees on one.
    pub bit_depth: Option<u8>,
    /// Sample rate in Hz, when every track in the edition agrees on one.
    pub sample_rate: Option<u32>,
    /// What the **files themselves** say about ReplayGain, summarised: how
    /// many of this edition's tracks carry an album gain, and how many carry a
    /// track gain.
    ///
    /// Read straight off the tags (ADR-0013's "what the file said"), never a
    /// figure baz measured, and never an average — a Details row is a
    /// statement about the files on disk.
    pub replay_gain: ReplayGainCoverage,
    /// This edition's tracks in disc/track/title order, for the side panel.
    pub tracks: Vec<TrackVm>,
}

/// How much of an edition carries ReplayGain figures in its tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReplayGainCoverage {
    /// Tracks carrying `REPLAYGAIN_ALBUM_GAIN`.
    pub album: usize,
    /// Tracks carrying `REPLAYGAIN_TRACK_GAIN`.
    pub track: usize,
    /// Tracks in the edition, so the two counts above can be read as
    /// "some of them" rather than as bare numbers.
    pub total: usize,
}

impl ReplayGainCoverage {
    /// The Details row's value: what the tags carry, in a sentence a listener
    /// can act on.
    ///
    /// Deliberately not a decibel figure. Which gain the engine will actually
    /// apply depends on the mode, the pre-amps and the clipping setting — all
    /// of which live in the Settings place and are stated there, by
    /// [`crate::replaygain`], in strings this module may not paraphrase
    /// (ADR-0013 §8). What the inspector can honestly say about a *record* is
    /// whether its files were ever scanned.
    #[must_use]
    pub fn label(self) -> Option<String> {
        if self.total == 0 {
            return None;
        }
        match (self.album, self.track) {
            (0, 0) => Some("not in the tags".to_owned()),
            (album, _) if album == self.total => Some("album and track gains".to_owned()),
            (0, track) if track == self.total => Some("track gains only".to_owned()),
            (album, track) => Some(format!("{} of {} tagged", album.max(track), self.total)),
        }
    }
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
    /// Which disc of the set this track is on, when the tags said.
    ///
    /// From tags only — folder layouts rarely encode it reliably, and the
    /// inspector's disc headers are drawn from this. **Never faked**: a
    /// single-disc rip and a two-disc rip whose tagger forgot the field both
    /// arrive here as `None`, and both are then drawn as one unbroken list
    /// rather than as an invented `DISC 1`.
    pub disc: Option<u32>,
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
    /// The file's size in bytes as of the last scan, when the filesystem
    /// reported one. Summed over an edition it is the `Size` row of the
    /// inspector's **Details** block.
    pub bytes: Option<u64>,
}

impl TrackVm {
    fn from_meta(meta: &TrackMeta) -> Self {
        Self {
            disc: meta.disc,
            number: meta.track,
            title: display_title(meta),
            artist: meta.artist.clone(),
            duration: meta.duration,
            path: meta.path.clone(),
            bytes: meta.stamp.map(|stamp| stamp.size),
        }
    }
}

/// Display title for a track: the tag/inferred title, else the file name —
/// the one fallback every row surface shares, so a search answer and the
/// album page it doors to cannot name one file two ways.
fn display_title(meta: &TrackMeta) -> String {
    meta.title.clone().unwrap_or_else(|| {
        meta.path
            .file_name()
            .map_or_else(|| String::from("?"), |n| n.to_string_lossy().into_owned())
    })
}

/// One row of the **Songs** section — a ranked track-level search answer
/// (`docs/design/09-implicit-playlists.md` §5), owned by the UI.
///
/// It carries exactly what the row shows (title, artist · album, duration)
/// plus what its two presses need: [`Self::album_id`] is the wall identity
/// the record-name door navigates to and the album `play_track` needle-drops
/// (ADR-0023 §2), and the path/number/disc triple is what [`song_row`]
/// resolves back into the record's *selected edition* — the file itself may
/// belong to an edition the listener has switched away from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongVm {
    /// Display title (the tag/inferred title, else the file name).
    pub title: String,
    /// The name beside the title: the track's own artist, else who the album
    /// is filed under — a search answer always names somebody.
    pub artist: String,
    /// The record's title as tagged; `None` for an unknown-album group.
    pub album: Option<String>,
    /// The record's wall identity ([`album_id`]) — where the door goes and
    /// what the press queues.
    pub album_id: u64,
    /// Track number within its disc, when known — [`song_row`]'s second key.
    pub number: Option<u32>,
    /// Which disc, when the tags said — [`song_row`]'s second key.
    pub disc: Option<u32>,
    /// Playing time, when the scan read one.
    pub duration: Option<Duration>,
    /// The matched file — [`song_row`]'s first key, and the honest object of
    /// the row's playing mark.
    pub path: PathBuf,
}

/// **The Songs section's rows**: the top `cap` ranked matching tracks for
/// `query` — [`Library::search`]'s answer (ADR-0021: fit, then field, then
/// library order), surfaced instead of thrown away at the album fold
/// (doc 09 §5's finding). Empty for a blank query and for a query nothing
/// matches; the section is then absent, not empty.
#[must_use]
pub fn song_hits(library: &Library, query: &str, cap: usize) -> Vec<SongVm> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    library
        .search(query, cap)
        .into_iter()
        .map(|track| {
            let filed_under = AlbumArtistVm::from_core(AlbumArtist::of(track));
            SongVm {
                title: display_title(track),
                artist: track
                    .artist
                    .clone()
                    .unwrap_or_else(|| filed_under.label().to_owned()),
                album: track.album.clone(),
                album_id: album_id(AlbumArtist::of(track), track.album.as_deref()),
                number: track.track,
                disc: track.disc,
                duration: track.duration,
                path: track.path.clone(),
            }
        })
        .collect()
}

/// Where `song` sits in `album`'s **selected edition** — the row a songs
/// press needle-drops on (ADR-0023 §2: the record whole, the cursor on the
/// song, exactly the record page's `play_track` path).
///
/// The search matched a *file*; the page and the queue play the selected
/// edition, which may be a different rip of the same record. So the
/// resolution runs from the strongest key to the weakest: the path itself,
/// then the same song by disc/track number and case-folded title, then by
/// title alone. `None` when the selected edition holds nothing by that name,
/// in which case the row asks for nothing rather than playing a track the
/// listener did not point at (ADR-0014's out-of-range rule).
#[must_use]
pub fn song_row(album: &AlbumVm, chosen: Option<EditionKey>, song: &SongVm) -> Option<usize> {
    let tracks = &selected_edition(album, chosen)?.tracks;
    if let Some(row) = tracks.iter().position(|track| track.path == song.path) {
        return Some(row);
    }
    let title = song.title.to_lowercase();
    let same_title = |track: &TrackVm| track.title.to_lowercase() == title;
    tracks
        .iter()
        .position(|track| {
            same_title(track) && track.number == song.number && track.disc == song.disc
        })
        .or_else(|| tracks.iter().position(same_title))
}

/// The owned, render-ready form of [`baz_core::index::GroupHeader`].
///
/// Owned for the reason every other type here is owned — the wall keeps it
/// across frames while a scan grows the library underneath — and **typed**
/// rather than reduced to its label, because the index rail needs to know what
/// kind of value it is looking at in order to say what is *missing* between
/// two of them (see [`crate::rail`]). A rail built from strings could draw the
/// shelves that exist and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupHeaderVm {
    /// [`GroupKey::Artist`] — an A–Z shelf, or one of the two anonymous ends.
    Initial(Initial),
    /// [`GroupKey::Year`] — a decade by its first year; `None` is "No year".
    Decade(Option<u32>),
    /// [`GroupKey::Genre`] — the genre verbatim; `None` is "No genre".
    Genre(Option<String>),
    /// [`GroupKey::Added`] / [`GroupKey::Played`] — a recency bucket.
    Recency(Recency),
}

impl GroupHeaderVm {
    fn from_core(header: &GroupHeader<'_>) -> Self {
        match header {
            GroupHeader::Initial(initial) => Self::Initial(*initial),
            GroupHeader::Decade(decade) => Self::Decade(*decade),
            GroupHeader::Genre(genre) => Self::Genre(genre.map(str::to_owned)),
            GroupHeader::Recency(recency) => Self::Recency(*recency),
            // `GroupHeader` is not `#[non_exhaustive]` today; the arm exists so
            // that a key added to `baz-core` fails to *draw* rather than fails
            // to compile the whole front end.
            #[expect(
                unreachable_patterns,
                reason = "a future GroupHeader variant must degrade, not break the build"
            )]
            other => Self::Genre(Some(other.label())),
        }
    }

    /// The header's text — [`baz_core::index::GroupHeader::label`]'s answer,
    /// unchanged. Typography (the caps, the tracking) is the view's business.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Initial(initial) => initial.label(),
            Self::Decade(Some(decade)) => format!("{decade}s"),
            Self::Decade(None) => "No year".to_owned(),
            Self::Genre(Some(genre)) => genre.clone(),
            Self::Genre(None) => "No genre".to_owned(),
            Self::Recency(recency) => recency.label(),
        }
    }
}

/// One shelf of the wall: a group header and the albums under it, owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShelfVm {
    /// What the shelf's header says, and what the rail projects.
    pub header: GroupHeaderVm,
    /// The albums on it, in library order. Never empty.
    pub albums: Vec<AlbumVm>,
}

/// Build the wall from the library, **arranged by `key`** —
/// [`Library::shelves_with_history`] projected into owned view models
/// (ADR-0019).
///
/// Called after each applied scan batch and whenever the key changes; owned
/// strings are cloned per rebuild, which is milliseconds for a 10k-album wall
/// and happens off the per-frame path.
///
/// `history` is consulted only for [`GroupKey::Played`], and `None` is a
/// correct answer rather than a degraded one: a library with no ledger has one
/// `Never played` shelf holding everything, which is a true statement about a
/// library nobody has played.
pub fn build_shelves(library: &Library, key: GroupKey, history: Option<&History>) -> Vec<ShelfVm> {
    library
        .shelves_with_history(key, history)
        .into_iter()
        .filter_map(|shelf| {
            let albums: Vec<AlbumVm> = shelf.albums.iter().filter_map(build_album).collect();
            (!albums.is_empty()).then(|| ShelfVm {
                header: GroupHeaderVm::from_core(&shelf.header),
                albums,
            })
        })
        .collect()
}

/// Project one core album into its owned, render-ready form. `None` for an
/// album with no readable first track, which cannot be shown or played.
fn build_album(album: &Album<'_>) -> Option<AlbumVm> {
    let first = album.default_edition()?.tracks.first()?;
    Some(AlbumVm {
        id: album_id(album.artist, album.title),
        title: album.title.map(str::to_owned),
        track_artists_vary: track_artists_vary(album),
        artist: AlbumArtistVm::from_core(album.artist),
        year: album.year,
        genre: album.genre.map(str::to_owned),
        first_seen_ns: album.first_seen_ns,
        first_track: first.path.clone(),
        editions: album.editions.iter().map(build_edition).collect(),
    })
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
        bitrate: edition.bitrate(),
        bit_depth: edition.bit_depth(),
        sample_rate: edition.sample_rate(),
        replay_gain: ReplayGainCoverage {
            album: edition
                .tracks
                .iter()
                .filter(|t| t.replay_gain.album_gain_centidb.is_some())
                .count(),
            track: edition
                .tracks
                .iter()
                .filter(|t| t.replay_gain.track_gain_centidb.is_some())
                .count(),
            total: edition.tracks.len(),
        },
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

/// **The best match for `query`** — the album <kbd>Enter</kbd> plays
/// (ADR-0017 §1.2, ADR-0021).
///
/// `None` when the query is blank or nothing matches. Blank is not "the first
/// album on the wall": <kbd>Enter</kbd> with no query is the *selection's*
/// press, and inventing a record to play out of an empty query is exactly the
/// "nothing begins that the user did not begin" refusal.
///
/// # Why this asks the library a second question
///
/// [`matching_album_ids`] asks *which* albums match and deliberately throws
/// the order away, because the wall is a place and not a ranking — a filtered
/// shelf keeps its shelves, its headers and its alphabet. This asks *which
/// matched best*, which is a different question with a different answer, and
/// [`Library::search_albums`] is ADR-0021's answer to it: ranked by how well
/// the query fits the field it landed in, then by which field that was, then
/// by library order, with an album taking its best track's rank and appearing
/// once.
///
/// It costs one more corpus scan per <kbd>Enter</kbd> — not per keystroke —
/// which is a scan a listener has already decided to spend by pressing a key
/// that starts music.
#[must_use]
pub fn top_match(library: &Library, query: &str) -> Option<u64> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    let best = library.search_albums(query, 1).into_iter().next()?;
    Some(album_id(best.artist, best.title))
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
    /// **Playing provenance** (`docs/design/09-implicit-playlists.md` §6,
    /// ADR-0023's amendment): the name of the playlist *file* this run was
    /// reified from, when a play gesture reified one — `None` for every other
    /// origin (a record, a shuffle draw, a stacked queue).
    ///
    /// A statement about **origin, never a live link** — Plexamp's
    /// `playQueueSourceURI`, adopted by name. It travels with the record
    /// through every edit (`crate::queue_edit` clones it, appends extend
    /// [`Self::items`] on the same value), survives `QueueEnded`, and is
    /// replaced only when the queue is replaced: a `SetQueue` from any other
    /// gesture carries a record built with `None` here. Nothing consults it
    /// for behaviour — it powers the Queue place's summary lead and the
    /// picker's hoisted *playing* row, both readings.
    pub provenance: Option<String>,
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
    /// The record this track was queued **as part of**, when it was queued as
    /// part of one — `None` for a loose track.
    ///
    /// Per item rather than per queue, because the queue is one list holding
    /// whole albums *and* loose songs and ADR-0017 §1.7 is explicit that
    /// "albums \[are\] listed as albums, never flattened". A queue-wide title
    /// could not say which of a mixture a given row belongs to, so the fact
    /// lives where the ambiguity is: consecutive items sharing a title are one
    /// album, and that is what
    /// [`PlayerState::continuation_note`](crate::player::PlayerState::continuation_note)
    /// counts rather than counting tracks.
    pub album: Option<String>,
    /// Who the record this track was queued as part of is **filed under**, as
    /// the shelf labels it ([`AlbumArtistVm::label`]).
    ///
    /// `None` means "as the queue's own header already says" — the ordinary
    /// case, where a queue is one album and [`QueueVm::artist`] covers every
    /// row of it. It is carried per item for the same reason [`Self::album`]
    /// is: a queue may hold several records (a shuffle draws eight of them,
    /// `crate::shuffle`), and the second one is by somebody else. Without it the
    /// popover could name the second record but not who made it, which is
    /// exactly the half-fact a catalogue must not print.
    pub album_artist: Option<String>,
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

    /// Whether this queue is **exactly** `tracks`: the same files, in the same
    /// order, and no others.
    ///
    /// The question the album inspector asks before it marks a row — *is the
    /// album I am showing the queue that is playing?* — and the one a click on
    /// a track row asks before it decides whether a re-queue is needed
    /// (ADR-0014's `JumpTo`-alone case). Both need the same answer, so it is
    /// one function.
    ///
    /// **Whole-list, in order, and nothing weaker.** The two near-misses are
    /// exactly the ones a listener can produce by accident, and each must
    /// answer *no*:
    ///
    /// - **The same album from a different edition.** A queue built from the
    ///   FLAC rip and an inspector showing the MP3 rip list the same titles in
    ///   the same order and share not one path. Marking a row there would put
    ///   the dot on a file that is not sounding, and jumping would address a
    ///   position in a queue the engine does not hold.
    /// - **A prefix or a superset.** A queue is a play order, so "these tracks
    ///   are among those" is not the same fact as "this is what is queued", and
    ///   only the second one licenses an index.
    ///
    /// A queue that lists one file twice is compared position by position like
    /// any other, so the repetition is preserved rather than collapsed — which
    /// is what lets [`Self::playing`] go on distinguishing the two occurrences.
    #[must_use]
    pub fn holds_exactly(&self, tracks: &[TrackVm]) -> bool {
        self.items.len() == tracks.len()
            && self
                .items
                .iter()
                .zip(tracks)
                .all(|(item, track)| item.path == track.path)
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
    QueueVm {
        album: album.title.clone(),
        artist: album.artist.label().to_owned(),
        items: album_items(album, chosen),
        // A record is not a playlist file: no provenance, and a run this
        // gesture replaces loses whatever provenance it had (09 §6).
        provenance: None,
    }
}

/// One record's tracks as queue items — the body [`album_queue`] and
/// [`stacked_queue`] share, so a record queued alone and the same record queued
/// third in a shuffle are byte-for-byte the same rows.
fn album_items(album: &AlbumVm, chosen: Option<EditionKey>) -> Vec<QueueItemVm> {
    let per_track_artists = album.track_artists_vary;
    selected_edition(album, chosen).map_or_else(Vec::new, |edition| {
        edition
            .tracks
            .iter()
            .map(|track| QueueItemVm {
                title: track.title.clone(),
                artist: track.artist.clone().filter(|_| per_track_artists),
                album: album.title.clone(),
                album_artist: Some(album.artist.label().to_owned()),
                duration: track.duration,
                path: track.path.clone(),
            })
            .collect()
    })
}

/// **The run a snapshot names**, rebuilt from the library
/// (`crate::session`, ADR-0023 §6).
///
/// The paths came off disk and the facts around them did not, so every row is
/// resolved back through the *same* records the wall holds — which is what
/// makes a restored run read identically to the run that was saved, rather
/// than as a list of filenames. A path the library no longer holds is
/// **dropped**: a rescan may have taken it, and a queue row pointing at
/// nothing is a row that cannot play.
///
/// The header names the first row's record, exactly as [`stacked_queue`]'s
/// does, because a restored run is the same kind of thing: a list that may
/// hold several records. Provenance travels in the snapshot and is handed back
/// unchanged — it is a statement about origin, and the origin did not change
/// because baz was closed.
///
/// Returns the queue **and where the cursor landed in it**, which is not
/// `cursor` when rows before it were dropped. Resolving the cursor here rather
/// than at the call site is what keeps "the track you were on" true across a
/// rescan: it is found by *path*, the same reconciliation every other reading
/// of a queue position uses.
#[must_use]
pub fn restored_queue(
    albums: &[AlbumVm],
    paths: &[std::path::PathBuf],
    cursor: usize,
    provenance: Option<String>,
) -> (QueueVm, Option<usize>) {
    let mut by_path: std::collections::HashMap<&Path, (&AlbumVm, &TrackVm)> =
        std::collections::HashMap::new();
    for album in albums {
        for edition in &album.editions {
            for track in &edition.tracks {
                by_path
                    .entry(track.path.as_path())
                    .or_insert((album, track));
            }
        }
    }
    let wanted = paths.get(cursor).map(std::path::PathBuf::as_path);
    let mut items = Vec::with_capacity(paths.len());
    let mut landed = None;
    for path in paths {
        let Some((album, track)) = by_path.get(path.as_path()) else {
            continue;
        };
        if Some(path.as_path()) == wanted && landed.is_none() {
            landed = Some(items.len());
        }
        items.push(QueueItemVm {
            title: track.title.clone(),
            artist: track.artist.clone().filter(|_| album.track_artists_vary),
            album: album.title.clone(),
            album_artist: Some(album.artist.label().to_owned()),
            duration: track.duration,
            path: track.path.clone(),
        });
    }
    let first = items.first();
    let queue = QueueVm {
        album: first.and_then(|item| item.album.clone()),
        artist: first
            .and_then(|item| item.album_artist.clone())
            .unwrap_or_default(),
        items,
        provenance,
    };
    (queue, landed)
}

/// **A queue of whole records**, in the order given — what a shuffle sends
/// (`crate::shuffle`).
///
/// The one thing this must not do is flatten. ADR-0014 §"albums are listed as
/// albums, never flattened" is a promise about the *queue*, not only about the
/// popover that draws it: each record's tracks arrive in the edition's own
/// disc/track order, contiguous, carrying the album title and the album artist
/// that say which record they belong to. Sorting the whole list, interleaving
/// two records, or dropping the album title would each turn eight sleeves into
/// forty loose songs, and no later surface could put them back together.
///
/// The queue's own header names the **first** record, because that is the one
/// it opens on; every record after it is named by its own run in the list
/// ([`QueueItemVm::album_artist`]).
///
/// An empty `picks`, or picks whose editions hold no tracks, gives an empty
/// queue — which the caller must not send. Silence is not started by accident.
#[must_use]
pub fn stacked_queue(picks: &[(&AlbumVm, Option<EditionKey>)]) -> QueueVm {
    let mut items = Vec::new();
    for (album, chosen) in picks {
        items.extend(album_items(album, *chosen));
    }
    let first = picks.first().map(|(album, _)| *album);
    QueueVm {
        album: first.and_then(|album| album.title.clone()),
        artist: first.map_or_else(String::new, |album| album.artist.label().to_owned()),
        items,
        // A draw is an implicit playlist, but not a *file*: no provenance
        // (09 §6 — playlist files only).
        provenance: None,
    }
}

/// The **Details** block: the condition report in full, one row per field the
/// scan actually read.
///
/// `docs/design/03-interface-prior-art.md` R6 is the whole argument: baz's
/// audience arrived from products that show ~20 fields for free, and the
/// inspector showed four. This is the back of the record's card, and you turn
/// it over by scrolling.
///
/// # Two rules, and they are the same rule
///
/// **A row exists only when the scan read one.** No placeholders, no em
/// dashes, no `Unknown` — an absent row says "the files did not say" more
/// honestly than a present row saying nothing, and it keeps the block as long
/// as the library is good rather than always thirteen lines of mostly nothing.
///
/// **Nothing here is inferred.** Every value is a field a tag or the
/// filesystem stated. The fields `.interface-design/system.md` §9 names that
/// baz's schema does not carry — Label, Catalogue, `MusicBrainz` — are simply
/// absent rather than approximated from a folder name, and they will appear
/// here the day the scanner reads them and not a day before.
///
/// Returned as `(label, value)` pairs rather than rendered, so the ordering
/// and the honesty are unit-testable without a window.
#[must_use]
pub fn details(album: &AlbumVm, edition: Option<&EditionVm>) -> Vec<(&'static str, String)> {
    let mut rows: Vec<(&'static str, String)> = Vec::new();
    let mut push = |label: &'static str, value: Option<String>| {
        if let Some(value) = value {
            rows.push((label, value));
        }
    };
    push(
        "Album artist",
        match &album.artist {
            AlbumArtistVm::Named(name) => Some(name.clone()),
            AlbumArtistVm::Various => Some(VARIOUS_ARTISTS.to_owned()),
            AlbumArtistVm::Unknown => None,
        },
    );
    push("Released", album.year.map(|year| year.to_string()));
    push("Genre", album.genre.clone());
    if let Some(edition) = edition {
        push(
            "Discs",
            discs(edition).filter(|n| *n > 1).map(|n| n.to_string()),
        );
        push("Tracks", Some(edition.tracks.len().to_string()));
        push(
            "Format",
            edition.key.0.map(|format| format.name().to_owned()),
        );
        push(
            "Depth",
            edition.bit_depth.map(|depth| format!("{depth}-bit")),
        );
        push("Sample rate", edition.sample_rate.map(format_sample_rate));
        push(
            "Bitrate",
            edition.bitrate.map(|kbps| format!("{kbps} kbps")),
        );
        push("Size", format_size(edition));
        push("ReplayGain", edition.replay_gain.label());
    }
    push("Added", album.first_seen_ns.and_then(format_date));
    push(
        "Folder",
        album
            .first_track
            .parent()
            .map(|dir| dir.to_string_lossy().into_owned()),
    );
    rows
}

/// How many discs an edition spans, when its tags say — the count of distinct
/// disc numbers, or `None` when no track declared one.
///
/// `None` and `Some(1)` are different answers and the inspector treats them
/// differently: a record whose tagger never wrote the field is not a record
/// baz knows to be a single disc.
#[must_use]
pub fn discs(edition: &EditionVm) -> Option<usize> {
    let numbers: HashSet<u32> = edition.tracks.iter().filter_map(|t| t.disc).collect();
    (!numbers.is_empty()).then_some(numbers.len())
}

/// An edition's total size on disk, in the unit that makes it readable.
///
/// `None` unless **every** track reported a size: a total over some of the
/// files is a smaller number than the truth presented as the truth, which is
/// the one thing a condition report may not do.
fn format_size(edition: &EditionVm) -> Option<String> {
    let total: u64 = edition
        .tracks
        .iter()
        .map(|t| t.bytes)
        .sum::<Option<u64>>()?;
    #[expect(
        clippy::cast_precision_loss,
        reason = "a display figure rounded to one decimal; f64 is exact to 2^53 bytes anyway"
    )]
    let mib = total as f64 / (1024.0 * 1024.0);
    Some(if mib >= 1024.0 {
        format!("{:.1} GiB", mib / 1024.0)
    } else {
        format!("{mib:.0} MiB")
    })
}

/// A Unix timestamp in nanoseconds as a plain `D Mon YYYY`.
///
/// Hand-rolled rather than a date crate, because a date crate is a dependency
/// and this is one row of one block (`docs/ENGINEERING.md`: every dependency
/// is argued). The conversion is Howard Hinnant's `civil_from_days`, which is
/// exact for the whole proleptic Gregorian range and is the algorithm every
/// date library uses underneath.
///
/// **UTC, and it says so nowhere** — deliberately. This row answers *roughly
/// when did this record arrive*, at a resolution of a day, and a listener who
/// imported an album at 00:30 local time does not need the interface to
/// litigate which day that was. Anything that needed the exact instant would
/// need a time zone database, which is a dependency for a row nobody reads
/// that closely.
///
/// `None` for a timestamp outside the range the algorithm is exact for, which
/// is a filesystem or a schema fault rather than a date.
#[must_use]
pub fn format_date(ns: i64) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    const NS_PER_DAY: i64 = 86_400 * 1_000_000_000;
    // Floor division: a negative timestamp is a pre-1970 file, and truncation
    // toward zero would put it on the wrong day.
    let days = ns.div_euclid(NS_PER_DAY);
    // Shift the epoch to 0000-03-01 so that the leap day is the last day of
    // the "year" and the month arithmetic below never has to know about it.
    let z = days.checked_add(719_468)?;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = era * 400 + yoe + i64::from(month <= 2);
    let index = usize::try_from(month - 1).ok()?;
    Some(format!("{day} {} {year}", MONTHS.get(index)?))
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
            genre: None,
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

    /// The ARTIST projection, flattened — the wall as a plain list.
    ///
    /// It was `vm::build_albums`, calling `Library::albums()`, and the
    /// application no longer asks the library for a flat list at all: it asks
    /// for [`build_shelves`] under the active key (ADR-0017 step 8). The swap
    /// is safe on **evidence rather than inspection** — ADR-0019's
    /// `the_artist_key_is_the_flat_shelf_with_its_breaks_named` compares
    /// `albums()` against `shelves(GroupKey::Artist)` element for element, so
    /// the two hold the same albums in the same order and the only difference
    /// is whether the A–Z breaks between them are stated.
    ///
    /// So it survives here, where the tests that were written against the flat
    /// list still want one, and `the_artist_key_flattens_to_the_wall_as_it_was`
    /// re-makes the ADR's comparison from this side of the boundary.
    fn build_albums(library: &Library) -> Vec<AlbumVm> {
        build_shelves(library, GroupKey::Artist, None)
            .into_iter()
            .flat_map(|shelf| shelf.albums)
            .collect()
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

    /// A library with enough variety that shelving is visible: several
    /// artists, three decades, messy genres, one record with nothing declared.
    fn varied_library() -> Library {
        let track = |artist: &str, album: &str, year: u32, genre: Option<&str>| TrackMeta {
            year: Some(year),
            genre: genre.map(str::to_owned),
            ..meta(artist, album, "One", 1)
        };
        library_with(vec![
            track("Boards of Canada", "Geogaddi", 2002, Some("Electronic")),
            track("Bark Psychosis", "Hex", 1994, Some("Post-Rock")),
            track("Talk Talk", "Laughing Stock", 1991, Some("post rock")),
            track("Aphex Twin", "Selected Ambient", 1992, Some("electronic")),
            track("10cc", "The Original Soundtrack", 1975, None),
            TrackMeta {
                year: None,
                ..meta("Ólafur Arnalds", "Found Songs", "One", 1)
            },
        ])
    }

    /// **The wall the application draws is the same wall it drew before, for
    /// ARTIST** — the swap from `Library::albums()` to `shelves(key)`, re-made
    /// from the front end's side.
    ///
    /// ADR-0019 asserts the two lists are equal in `baz-core`
    /// (`the_artist_key_is_the_flat_shelf_with_its_breaks_named`); this asserts
    /// that projecting them into view models does not disturb it, which is the
    /// half of the claim that lives here.
    #[test]
    fn the_artist_key_flattens_to_the_wall_as_it_was() {
        let library = varied_library();
        let shelved = build_shelves(&library, GroupKey::Artist, None);
        let flat: Vec<&AlbumVm> = shelved.iter().flat_map(|shelf| &shelf.albums).collect();
        let direct: Vec<AlbumVm> = library.albums().iter().filter_map(build_album).collect();
        assert_eq!(flat.len(), direct.len());
        for (shelved, direct) in flat.iter().zip(&direct) {
            assert_eq!(**shelved, *direct);
        }
        // And the breaks are now *stated*: `10cc` starts with a digit, so it
        // is the `#` shelf; the rest are their initials, `Ó` included, and the
        // two B's are one shelf holding two records.
        assert_eq!(
            shelved
                .iter()
                .map(|shelf| shelf.header.label())
                .collect::<Vec<_>>(),
            ["#", "A", "B", "T", "Ó"]
        );
        assert_eq!(
            shelved[2].albums.len(),
            2,
            "Bark Psychosis and Boards of Canada"
        );
    }

    /// **Every key is a projection, never a filter**: every album appears
    /// under every key, exactly once. Asserted in `baz-core` and re-asserted
    /// here, because the view model drops albums with no readable first track
    /// and a projection that lost one on its way through would be a wall that
    /// quietly hid records.
    #[test]
    fn every_key_shelves_every_album_exactly_once() {
        let library = varied_library();
        let expected: Vec<u64> = {
            let mut ids: Vec<u64> = build_albums(&library).iter().map(|a| a.id).collect();
            ids.sort_unstable();
            ids
        };
        assert_eq!(expected.len(), 6);
        for key in GroupKey::ALL {
            let shelves = build_shelves(&library, key, None);
            assert!(
                shelves.iter().all(|shelf| !shelf.albums.is_empty()),
                "{key:?} produced an empty shelf"
            );
            let mut ids: Vec<u64> = shelves
                .iter()
                .flat_map(|shelf| &shelf.albums)
                .map(|album| album.id)
                .collect();
            ids.sort_unstable();
            assert_eq!(ids, expected, "{key:?} did not hold every album once");
        }
    }

    /// The headers each key draws, on one library — the five arrangements, in
    /// the order the wall lays them out.
    #[test]
    fn each_key_draws_its_own_headers() {
        let library = varied_library();
        let labels = |key| {
            build_shelves(&library, key, None)
                .iter()
                .map(|shelf| shelf.header.label())
                .collect::<Vec<_>>()
        };
        // Undated at the front, then decades, oldest first.
        assert_eq!(
            labels(GroupKey::Year),
            ["No year", "1970s", "1990s", "2000s"]
        );
        // Genre verbatim, and case-folded into one shelf: `Post-Rock` and
        // `post rock` are **two** genres because the files say so, but
        // `Electronic` and `electronic` are one — headed by the first spelling
        // seen, which is Aphex Twin's lowercase one because the wall is in
        // album-artist order underneath. Shelf order is the case-folded name,
        // so `post rock` precedes `Post-Rock` (space sorts before hyphen).
        assert_eq!(
            labels(GroupKey::Genre),
            ["No genre", "electronic", "post rock", "Post-Rock"]
        );
        // Everything was inserted a moment ago, so ADDED is one shelf — and it
        // reads `This evening`, which ADR-0019 §7 states rather than discovers:
        // ADDED borrows the ledger's bands so the rail has one vocabulary, and
        // the ledger's first band is six hours.
        assert_eq!(labels(GroupKey::Added), ["This evening"]);
        // …and with no ledger, PLAYED is one honest shelf.
        assert_eq!(labels(GroupKey::Played), ["Never played"]);
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
            bitrate: None,
            bit_depth: None,
            sample_rate: None,
            replay_gain: ReplayGainCoverage::default(),
            tracks: Vec::new(),
        };
        assert_eq!(bare_format.encoding_line().as_deref(), Some("WAV"));
        // ...and an edition with nothing at all says nothing at all.
        let nothing = EditionVm {
            key: EditionKey(None),
            detail: None,
            bitrate: None,
            bit_depth: None,
            sample_rate: None,
            replay_gain: ReplayGainCoverage::default(),
            tracks: Vec::new(),
        };
        assert_eq!(nothing.encoding_line(), None);
    }

    /// **A Details row exists only when the scan read one.**
    ///
    /// The whole rule of the block, and the reason it is worth a test rather
    /// than a comment: the failure mode of a condition report is not a missing
    /// row, it is a present row saying `Unknown`. Thirteen fields of nothing is
    /// a form; the fields the files actually carry are a record's card.
    #[test]
    fn details_lists_what_the_scan_read_and_invents_nothing() {
        let bare = AlbumVm {
            id: 1,
            title: Some("Untitled".to_owned()),
            artist: AlbumArtistVm::Unknown,
            track_artists_vary: false,
            year: None,
            genre: None,
            first_seen_ns: None,
            first_track: PathBuf::from("/m/x/01.flac"),
            editions: Vec::new(),
        };
        let rows = details(&bare, None);
        let labels: Vec<&str> = rows.iter().map(|(label, _)| *label).collect();
        // An unknown album artist is *absent*, not "Unknown Artist": the
        // caption's fallback label is a thing to read on a wall, and a
        // condition report may not restate it as a fact about the file.
        assert!(!labels.contains(&"Album artist"), "{labels:?}");
        assert!(!labels.contains(&"Released"), "{labels:?}");
        assert!(!labels.contains(&"Genre"), "{labels:?}");
        // The folder is the one thing that is always known — the album exists
        // because a file does.
        assert_eq!(labels, vec!["Folder"]);

        // Now a record the scan read properly.
        let full = AlbumVm {
            artist: AlbumArtistVm::Named("Talk Talk".to_owned()),
            year: Some(1988),
            genre: Some("Post-Rock".to_owned()),
            first_seen_ns: Some(1_700_000_000_000_000_000),
            ..bare.clone()
        };
        let edition = EditionVm {
            key: EditionKey(Some(AudioFormat::Flac)),
            detail: None,
            bitrate: Some(910),
            bit_depth: Some(16),
            sample_rate: Some(44_100),
            replay_gain: ReplayGainCoverage {
                album: 2,
                track: 2,
                total: 2,
            },
            tracks: vec![
                TrackVm {
                    disc: Some(1),
                    number: Some(1),
                    title: "The Rainbow".to_owned(),
                    artist: None,
                    duration: Some(Duration::from_secs(560)),
                    path: PathBuf::from("/m/x/01.flac"),
                    bytes: Some(60 * 1024 * 1024),
                },
                TrackVm {
                    disc: Some(2),
                    number: Some(1),
                    title: "Desire".to_owned(),
                    artist: None,
                    duration: Some(Duration::from_secs(415)),
                    path: PathBuf::from("/m/x/02.flac"),
                    bytes: Some(40 * 1024 * 1024),
                },
            ],
        };
        let rows = details(&full, Some(&edition));
        let value = |label: &str| {
            rows.iter()
                .find(|(name, _)| *name == label)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(value("Album artist"), Some("Talk Talk"));
        assert_eq!(value("Released"), Some("1988"));
        // Verbatim: the genre is not title-cased, split or mapped.
        assert_eq!(value("Genre"), Some("Post-Rock"));
        assert_eq!(value("Discs"), Some("2"));
        assert_eq!(value("Format"), Some("FLAC"));
        assert_eq!(value("Depth"), Some("16-bit"));
        assert_eq!(value("Sample rate"), Some("44.1 kHz"));
        assert_eq!(value("Bitrate"), Some("910 kbps"));
        assert_eq!(value("Size"), Some("100 MiB"));
        assert_eq!(value("ReplayGain"), Some("album and track gains"));
        assert_eq!(value("Folder"), Some("/m/x"));
        // And the block is a genuine improvement on the four lines the header
        // carries, which is the whole reason it exists (prior art R6).
        assert!(rows.len() >= 10, "{rows:?}");

        // A single-disc record draws no `Discs` row: `1` is not a fact worth a
        // line, and a record whose tagger never wrote the field is not a record
        // baz knows to be single-disc either.
        let one_disc = EditionVm {
            tracks: vec![EditionVm::clone(&edition).tracks.remove(0)],
            ..edition.clone()
        };
        assert!(
            !details(&full, Some(&one_disc))
                .iter()
                .any(|(l, _)| *l == "Discs")
        );

        // A partly-sized edition reports no size at all: a total over some of
        // the files, presented as the total, is the one thing a condition
        // report may not do.
        let mut partial = edition.clone();
        partial.tracks[1].bytes = None;
        assert!(
            !details(&full, Some(&partial))
                .iter()
                .any(|(l, _)| *l == "Size")
        );
    }

    /// The `Added` row's date arithmetic, at the boundaries that break naive
    /// implementations: the epoch, a leap day, a century that is not a leap
    /// year, and a pre-epoch file (which truncating division would put on the
    /// wrong day).
    #[test]
    fn added_dates_are_exact_at_the_awkward_boundaries() {
        const DAY: i64 = 86_400 * 1_000_000_000;
        assert_eq!(format_date(0).as_deref(), Some("1 Jan 1970"));
        assert_eq!(format_date(DAY - 1).as_deref(), Some("1 Jan 1970"));
        assert_eq!(format_date(DAY).as_deref(), Some("2 Jan 1970"));
        // A moment before the epoch is the *previous* day, not the epoch: `/`
        // truncates toward zero and would answer 1 Jan 1970 here.
        assert_eq!(format_date(-1).as_deref(), Some("31 Dec 1969"));
        // 2000 is a leap year (divisible by 400); 1900 was not.
        assert_eq!(
            format_date(951_782_400 * 1_000_000_000).as_deref(),
            Some("29 Feb 2000")
        );
        assert_eq!(
            format_date(1_700_000_000 * 1_000_000_000).as_deref(),
            Some("14 Nov 2023")
        );
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

    /// **What <kbd>Enter</kbd> plays** — the top-*ranked* match, not the
    /// first one in library order (ADR-0017 §1.2 via ADR-0021).
    ///
    /// The corpus is the one ADR-0021 argues from, laid out so that library
    /// order and rank disagree: `Kids Everywhere` by the Aardvark Collective
    /// sorts first by album artist and would have been what the old
    /// corpus-ordered `search` returned, and `Kid A` is the record a listener
    /// typing `kid` means. The whole reason step 12 had to precede step 11 is
    /// that this assertion could not have been made before it.
    #[test]
    fn enter_plays_the_ranked_first_match_and_not_the_first_in_library_order() {
        let library = library_with(vec![
            meta("Aardvark Collective", "Kids Everywhere", "Opening", 1),
            meta("Radiohead", "Kid A", "Everything In Its Right Place", 1),
            meta("Skid Row", "Slave to the Grind", "Monkey Business", 1),
        ]);
        // Library order — album artist first — puts the Aardvarks at the top
        // of the wall, and the wall keeps that order under a filter.
        let albums = build_albums(&library);
        assert_eq!(
            albums.first().and_then(|album| album.artist.name()),
            Some("Aardvark Collective"),
            "the fixture is only interesting while the wall disagrees with the rank"
        );
        assert_eq!(visible_indices(&albums, &library, "kid").len(), 3);

        // …and Enter plays `Kid A`, because `kid` starts that field and ends
        // on a word boundary where it ends inside `Kids` and inside `Skid`.
        let played = top_match(&library, "kid").expect("a match");
        assert_eq!(
            played,
            album_id(AlbumArtist::Named("Radiohead"), Some("Kid A"))
        );

        // An empty or blank query plays nothing: Enter with no query is the
        // selection's press, and inventing a record would be the software
        // beginning something nobody began.
        assert_eq!(top_match(&library, ""), None);
        assert_eq!(top_match(&library, "   "), None);
        // A query nothing matches plays nothing either.
        assert_eq!(top_match(&library, "zzzz"), None);
        // Leading and trailing space is trimmed, exactly as the filter trims
        // it, so the two halves of one keystroke cannot disagree.
        assert_eq!(top_match(&library, " kid "), Some(played));
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

    /// **S1, the section itself** (doc 09 §4): given a non-empty query with
    /// matching tracks, the Songs answers are the ranked head of the match
    /// set — top-ranked first (ADR-0021), capped at [`SONGS`] — and each row
    /// carries title, artist, album, duration, and the wall identity its
    /// door and its press spend.
    #[test]
    fn a_query_fills_the_songs_section_ranked_and_capped() {
        // One exactly-titled song on one record, and nine prefix-titled songs
        // on another: the exact match must rank first however the corpus is
        // ordered, and the cap must trim the tail, never the head.
        let mut tracks = vec![meta("Zeta", "Zed", "Night", 1)];
        for t in 1..=9 {
            tracks.push(meta("Alpha", "Nine", &format!("Night Song {t}"), t));
        }
        let library = library_with(tracks);
        let albums = build_albums(&library);

        let songs = song_hits(&library, "night", SONGS);
        assert_eq!(songs.len(), SONGS, "the ranked head, capped at eight");
        let first = &songs[0];
        assert_eq!(first.title, "Night", "best fit first — ADR-0021's Exact");
        assert_eq!(first.artist, "Zeta");
        assert_eq!(first.album.as_deref(), Some("Zed"));
        assert_eq!(first.duration, Some(Duration::from_secs(200)));
        // The row's identity is the wall's identity: the door and the press
        // land on the record the shelf actually holds.
        assert_eq!(
            first.album_id,
            album_id(AlbumArtist::Named("Zeta"), Some("Zed"))
        );
        assert!(albums.iter().any(|album| album.id == first.album_id));
        // The tail is the other record's rows, in library order.
        assert!(songs[1..].iter().all(|song| song.artist == "Alpha"));

        // Trimmed exactly as the filter trims, so the section and the wall
        // answer one query.
        assert_eq!(song_hits(&library, " night ", SONGS), songs);
    }

    /// **S1's absence criterion**: no matching tracks — or no query at all —
    /// means no Songs rows, so the section is absent rather than empty, and
    /// a blank query builds no section for the resting wall.
    #[test]
    fn no_matching_tracks_means_no_songs_rows() {
        let library = library_with(vec![meta("Abel", "Alpha", "Sunrise", 1)]);
        assert!(song_hits(&library, "zzz", SONGS).is_empty());
        assert!(song_hits(&library, "", SONGS).is_empty());
        assert!(song_hits(&library, "   ", SONGS).is_empty());
    }

    /// **S1, the press** (ADR-0023 §2 extended to the songs section): a song
    /// row resolves to its record queued **whole, in order** with the cursor
    /// on the song — the `SetQueue` + `JumpTo` shape of the record page's
    /// `play_track` path, never a one-track queue and never the top of the
    /// record.
    #[test]
    fn a_song_rows_press_is_a_needle_drop_on_its_whole_record() {
        let library = library_with(vec![
            meta("Alpha", "Nine", "Night Song 1", 1),
            meta("Alpha", "Nine", "Night Song 2", 2),
            meta("Alpha", "Nine", "Night Song 3", 3),
        ]);
        let albums = build_albums(&library);
        let album = &albums[0];

        let songs = song_hits(&library, "song 2", SONGS);
        let song = songs.first().expect("the query matches one song");
        assert_eq!(song.title, "Night Song 2");

        let row = song_row(album, None, song).expect("the song is on its record");
        let queue = album_queue(album, None);
        // The whole record, in the edition's own order — not the song alone.
        assert_eq!(queue.len(), 3);
        assert_eq!(
            queue.paths(),
            vm_paths(album),
            "the record whole, in order — the queue a click on the record's \
             own page would send"
        );
        // …and the jump lands on the song that was pressed, with the earlier
        // tracks behind the cursor.
        assert_eq!(row, 1);
        assert_eq!(queue.items[row].path, song.path);
    }

    /// The selected edition's paths in row order — what `play_track` queues.
    fn vm_paths(album: &AlbumVm) -> Vec<PathBuf> {
        selected_edition(album, None)
            .expect("an edition")
            .tracks
            .iter()
            .map(|t| t.path.clone())
            .collect()
    }

    /// A song found in one rip resolves into the rip the page would play:
    /// the search matched a *file*, the press queues the **selected
    /// edition** (ADR-0023 §2's "selected edition, whole, in order"), and
    /// the same song is found in it by number and title when the paths
    /// differ.
    #[test]
    fn a_song_resolves_into_the_edition_the_page_would_play() {
        let library = library_with(vec![
            encoded("Northwest Passage", "Lies", 2, AudioFormat::Mp3),
            encoded("Northwest Passage", "Passage", 1, AudioFormat::Flac),
            encoded("Northwest Passage", "Lies", 2, AudioFormat::Flac),
            encoded("Northwest Passage", "Passage", 1, AudioFormat::Mp3),
        ]);
        let albums = build_albums(&library);
        let album = &albums[0];

        let songs = song_hits(&library, "lies", SONGS);
        // Both rips' files matched; take one whose file is the MP3.
        let song = songs
            .iter()
            .find(|song| song.path.to_string_lossy().contains("/mp3/"))
            .expect("the MP3 rip's file is among the answers");

        // With the FLAC edition selected, the press still lands on *Lies* —
        // resolved by number and title into the edition on screen.
        let chosen = Some(EditionKey(Some(AudioFormat::Flac)));
        let row = song_row(album, chosen, song).expect("the song is on this record");
        let flac = selected_edition(album, chosen).expect("the FLAC edition");
        assert_eq!(flac.tracks[row].title, "Lies");
        assert_ne!(
            flac.tracks[row].path, song.path,
            "a different rip of the same song — the path key alone could not \
             have resolved this row"
        );
        // A song the selected edition does not hold asks for nothing.
        let stray = SongVm {
            title: "Not Here".to_owned(),
            artist: "Stan Rogers".to_owned(),
            album: Some("Northwest Passage".to_owned()),
            album_id: album.id,
            number: Some(9),
            disc: None,
            duration: None,
            path: PathBuf::from("/nowhere.flac"),
        };
        assert_eq!(song_row(album, chosen, &stray), None);
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
            provenance: None,
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
            album: Some("Loop".to_owned()),
            album_artist: None,
            duration: Some(Duration::from_secs(60)),
            path: path.clone(),
        };
        let queue = QueueVm {
            album: Some("Loop".to_owned()),
            artist: "A".to_owned(),
            items: vec![item("once"), item("again")],
            provenance: None,
        };
        // The position is exact and its path agrees, so it is the answer.
        assert_eq!(queue.playing(1, &path), Some(1));
        // With no usable position, the first occurrence is the answer.
        assert_eq!(queue.playing(7, &path), Some(0));
        assert_eq!(queue.total_time(), Duration::from_secs(120));
    }

    /// The album inspector's question — *is what I am listing the queue that
    /// is playing?* — with the two near-misses that must answer `false`.
    #[test]
    fn a_queue_holds_an_edition_exactly_or_not_at_all() {
        let album = two_edition_album();
        let flac = selected_edition(&album, None).expect("the default edition");
        let mp3 = selected_edition(&album, Some(EditionKey(Some(AudioFormat::Mp3))))
            .expect("the MP3 edition");

        // The queue that was built from this edition holds it, exactly.
        let queued = album_queue(&album, None);
        assert!(queued.holds_exactly(&flac.tracks));

        // The same album, the same titles, the same order — and a different
        // set of files. The inspector must not mark a row from this queue.
        assert!(
            !queued.holds_exactly(&mp3.tracks),
            "a different edition is a different queue"
        );
        assert!(
            !album_queue(&album, Some(EditionKey(Some(AudioFormat::Mp3))))
                .holds_exactly(&flac.tracks)
        );

        // A prefix is not the queue: "these are among those" licenses no index.
        assert!(!queued.holds_exactly(&flac.tracks[..1]));
        // Nor is a superset.
        let mut longer = flac.tracks.clone();
        longer.push(mp3.tracks[0].clone());
        assert!(!queued.holds_exactly(&longer));
        // Nor the same files in a different order.
        let mut reversed = flac.tracks.clone();
        reversed.reverse();
        assert!(!queued.holds_exactly(&reversed));

        // An empty queue holds an empty list and nothing else.
        let empty = QueueVm {
            album: None,
            artist: UNKNOWN_ARTIST.to_owned(),
            items: Vec::new(),
            provenance: None,
        };
        assert!(empty.holds_exactly(&[]));
        assert!(!empty.holds_exactly(&flac.tracks));
        assert!(!queued.holds_exactly(&[]));
    }

    /// A file listed twice is two entries, and the comparison keeps them —
    /// which is what lets [`QueueVm::playing`] go on telling them apart.
    #[test]
    fn a_repeated_track_is_compared_position_by_position() {
        let track = |path: &str| TrackVm {
            disc: None,
            number: Some(1),
            title: "Loop".to_owned(),
            artist: None,
            duration: Some(Duration::from_secs(60)),
            path: PathBuf::from(path),
            bytes: None,
        };
        let listed = vec![track("/m/a/1.flac"), track("/m/a/1.flac")];
        let item = |path: &str| QueueItemVm {
            title: "Loop".to_owned(),
            artist: None,
            album: Some("Loop".to_owned()),
            album_artist: None,
            duration: Some(Duration::from_secs(60)),
            path: PathBuf::from(path),
        };
        let queue = QueueVm {
            album: Some("Loop".to_owned()),
            artist: "A".to_owned(),
            items: vec![item("/m/a/1.flac"), item("/m/a/1.flac")],
            provenance: None,
        };
        assert!(queue.holds_exactly(&listed));
        // The repetition is not collapsed: one entry is not two.
        assert!(!queue.holds_exactly(&listed[..1]));
        // And the queue still distinguishes the two occurrences by position.
        assert_eq!(queue.playing(1, &listed[1].path), Some(1));
        assert_eq!(queue.playing(0, &listed[0].path), Some(0));
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

    /// **A shuffle's queue is whole records, in the order drawn** — the promise
    /// ADR-0014 makes about the queue itself, not only about the popover that
    /// draws it.
    ///
    /// Three things are pinned, and each one is a way the queue could be
    /// flattened without anybody noticing until a listener opened it:
    ///
    /// - every record's tracks are **contiguous and in the edition's own
    ///   order** — no interleaving, no global sort;
    /// - every item names the record it belongs to *and* who that record is
    ///   filed under, so a second record can be headed by its own name;
    /// - the paths the engine is sent are exactly the paths the rows list, in
    ///   the same order, which is what [`QueueVm`] exists to guarantee.
    #[test]
    fn a_shuffle_queues_whole_records_and_never_flattens_them() {
        let library = library_with(vec![
            meta("Boards of Canada", "Geogaddi", "Music Is Math", 2),
            meta("Boards of Canada", "Geogaddi", "Ready Lets Go", 1),
            meta("Talk Talk", "Laughing Stock", "Myrrhman", 1),
            meta("Talk Talk", "Laughing Stock", "Ascension Day", 2),
        ]);
        let albums = build_albums(&library);
        assert_eq!(albums.len(), 2);
        // Drawn in the order the shuffle picked them: the *second* album first,
        // so a queue that quietly re-sorted would be visible here.
        let picks = [(&albums[1], None), (&albums[0], None)];
        let queue = stacked_queue(&picks);

        assert_eq!(queue.len(), 4);
        let titles: Vec<&str> = queue.items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Myrrhman",
                "Ascension Day",
                "Ready Lets Go",
                "Music Is Math"
            ],
            "each record arrives whole, in its own track order, in the drawn order"
        );
        let records: Vec<Option<&str>> = queue.items.iter().map(|i| i.album.as_deref()).collect();
        assert_eq!(
            records,
            [
                Some("Laughing Stock"),
                Some("Laughing Stock"),
                Some("Geogaddi"),
                Some("Geogaddi")
            ],
            "consecutive items sharing a title are one record — the run is unbroken"
        );
        let filed: Vec<Option<&str>> = queue
            .items
            .iter()
            .map(|i| i.album_artist.as_deref())
            .collect();
        assert_eq!(
            filed,
            [
                Some("Talk Talk"),
                Some("Talk Talk"),
                Some("Boards of Canada"),
                Some("Boards of Canada")
            ],
            "the second record can be headed by its own artist, not the first's"
        );
        // The queue's own header names the record it opens on.
        assert_eq!(queue.album.as_deref(), Some("Laughing Stock"));
        assert_eq!(queue.artist, "Talk Talk");
        // What is sent is what is listed.
        let paths = queue.paths();
        assert_eq!(paths.len(), queue.items.len());
        assert!(
            paths
                .iter()
                .zip(&queue.items)
                .all(|(path, item)| *path == item.path)
        );
        // One record stacked alone is byte-for-byte the ordinary album queue.
        let alone = stacked_queue(&[(&albums[0], None)]);
        assert_eq!(alone.items, album_queue(&albums[0], None).items);
        // Nothing drawn is an empty queue, which the caller must not send.
        assert!(stacked_queue(&[]).is_empty());
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0:00");
        assert_eq!(format_duration(Duration::from_secs(243)), "4:03");
        assert_eq!(format_duration(Duration::from_secs(3723)), "1:02:03");
    }
}
