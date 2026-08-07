//! The library index: durable SQLite store plus an in-RAM search structure.
//!
//! This is the back half of the "point it at a directory and search is
//! instant" pillar (ADR-0003, `docs/research/04-tech-stack.md`): the scanner
//! in [`crate::library`] produces [`TrackMeta`] values, and [`Library`] here
//! persists them and answers queries. Two stores cooperate:
//!
//! - **SQLite** (via `rusqlite`, `bundled` so no system library is needed) is
//!   the durable truth. It exists so a restart is a cheap hydrate, not a
//!   rescan; complex queries (and FTS5, if we ever want it) can grow here.
//! - **An in-RAM index**, hydrated in full at open, answers every search.
//!   The "instant" feel comes from matching case-folded haystacks in memory
//!   with zero I/O per keystroke — the Phase 1 spike put this at sub-ms p99
//!   over 100k tracks, and `benches/search.rs` keeps that honest.
//!
//! # Path storage: BLOB, platform-native bytes
//!
//! Track paths are the identity of everything and are **not guaranteed to be
//! valid UTF-8**, so they are stored as a `BLOB`, never `TEXT`:
//!
//! - On Unix, the blob is the raw `OsStr` bytes — lossless by definition.
//! - On Windows, the blob is the path's UTF-16 code units in little-endian
//!   byte order (including any unpaired surrogates) — also lossless.
//!
//! The trade-off is that the database file is **not portable across OS
//! families**. That is accepted: the database is a per-machine cache of a
//! local music folder, and the paths inside it would be meaningless on
//! another OS anyway.
//!
//! # Search semantics
//!
//! [`Library::search`] is a literal, Unicode-aware case-insensitive
//! substring match over each track's artist + album + title (folded with
//! [`str::to_lowercase`]; full case folding such as `ß`/`SS` equivalence is
//! out of scope for now). Results come back in library order — artist, album,
//! disc, track, title, path — so repeated queries are deterministic. An
//! **empty query returns nothing**: every haystack contains the empty
//! string, so the only honest answer would be the entire library truncated
//! at `limit`, which would misrepresent a 100k-track library as `limit`
//! tracks. "No query yet" is the shelf's state ([`Library::albums`]), not a
//! search result.
//!
//! # Schema versioning
//!
//! The schema version lives in SQLite's `PRAGMA user_version` and migrations
//! run stepwise at open (see `migrate`), so a v0.2 schema change is one new
//! match arm, not a format break. A database from a *newer* baz is refused
//! rather than guessed at.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, params};

use crate::library::TrackMeta;

/// The schema version this build reads and writes (`PRAGMA user_version`).
const SCHEMA_VERSION: i64 = 1;

/// Rows per write transaction in [`Library::add_tracks`]. A scan yields tens
/// of thousands of tracks; committing in batches keeps any single
/// transaction (and the in-flight chunk buffer) bounded while still amortizing
/// fsync cost across thousands of rows.
const TRANSACTION_BATCH: usize = 5_000;

/// Version 1 of the schema. Executed as one transaction together with the
/// `user_version` bump so a crash mid-migration cannot leave a half-made
/// schema behind.
const SCHEMA_V1: &str = "
    BEGIN;
    CREATE TABLE tracks (
        id          INTEGER PRIMARY KEY,
        path        BLOB NOT NULL UNIQUE,  -- OS-native bytes; see module docs
        artist      TEXT,
        album       TEXT,
        title       TEXT,
        track       INTEGER,
        disc        INTEGER,
        year        INTEGER,
        duration_ns INTEGER                -- whole track duration, nanoseconds
    ) STRICT;
    PRAGMA user_version = 1;
    COMMIT;
";

/// Insert-or-replace by path: a rescan of the same file updates its metadata
/// instead of failing the batch or duplicating the track.
const UPSERT_TRACK: &str = "
    INSERT INTO tracks (path, artist, album, title, track, disc, year, duration_ns)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
    ON CONFLICT(path) DO UPDATE SET
        artist = excluded.artist,
        album = excluded.album,
        title = excluded.title,
        track = excluded.track,
        disc = excluded.disc,
        year = excluded.year,
        duration_ns = excluded.duration_ns
";

const SELECT_ALL_TRACKS: &str =
    "SELECT path, artist, album, title, track, disc, year, duration_ns FROM tracks";

/// The library index could not be opened or updated.
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    /// The underlying SQLite operation failed.
    #[error("library database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The database was written by a newer baz than this one. Refusing to
    /// read or "migrate" it is what protects the newer install's data.
    #[error("library database schema version {found} is newer than this build supports")]
    SchemaTooNew {
        /// The `PRAGMA user_version` found in the database.
        found: i64,
    },
    /// A stored path blob is not decodable on this platform (for example an
    /// odd-length UTF-16 blob on Windows) — the database is corrupt or from
    /// another OS family (see the module docs on path storage).
    #[error("a stored track path is corrupt or from another platform")]
    CorruptStoredPath,
    /// A stored duration is negative, which this code never writes — the
    /// database is corrupt.
    #[error("a stored track duration is corrupt (negative)")]
    CorruptStoredDuration,
    /// A track's duration exceeds what fits in the database column
    /// (~292 years in nanoseconds) — only conceivable with corrupt tags.
    #[error("track duration for `{}` exceeds the storable range", path.display())]
    DurationOutOfRange {
        /// The track whose duration could not be stored.
        path: PathBuf,
    },
}

/// The track library: a durable SQLite store and an in-RAM search index that
/// always reflect each other. See the [module docs](self) for the design.
pub struct Library {
    conn: Connection,
    index: SearchIndex,
}

impl Library {
    /// Open the library database at `db_path`, creating and initializing it
    /// on first run, then hydrate the full in-RAM index from it.
    ///
    /// # Errors
    ///
    /// [`IndexError::Sqlite`] if the file cannot be opened (for example a
    /// missing parent directory) or read; [`IndexError::SchemaTooNew`] if the
    /// database was written by a newer baz; [`IndexError::CorruptStoredPath`]
    /// if a stored path cannot be decoded on this platform.
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self, IndexError> {
        let conn = Connection::open(db_path)?;
        // WAL keeps scan-time writers from blocking any future readers and
        // batches fsyncs; NORMAL sync is durable-enough for a rebuildable
        // cache of what is on disk anyway.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::from_connection(conn)
    }

    /// Open an ephemeral in-memory library: same behavior, no file. For
    /// tests, benchmarks, and any future "browse without a library" mode.
    ///
    /// # Errors
    ///
    /// [`IndexError::Sqlite`] if SQLite cannot create the in-memory database.
    pub fn open_in_memory() -> Result<Self, IndexError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self, IndexError> {
        migrate(&conn)?;
        let mut library = Self {
            conn,
            index: SearchIndex::default(),
        };
        library.hydrate()?;
        Ok(library)
    }

    /// Load every stored track into the in-RAM index, replacing its contents.
    fn hydrate(&mut self) -> Result<(), IndexError> {
        self.index = SearchIndex::default();
        let mut stmt = self.conn.prepare(SELECT_ALL_TRACKS)?;
        let rows = stmt.query_and_then([], row_to_meta)?;
        for meta in rows {
            self.index.insert(meta?);
        }
        self.index.rebuild_order();
        Ok(())
    }

    /// Add (or, for already-known paths, update) tracks, persisting them and
    /// making them searchable immediately. Designed to be called repeatedly
    /// with batches while a scan is still running; writes are committed in
    /// transactions of a few thousand rows (`TRANSACTION_BATCH`). Returns
    /// the number of tracks written.
    ///
    /// # Errors
    ///
    /// [`IndexError::Sqlite`] if a write fails and
    /// [`IndexError::DurationOutOfRange`] for an unstorable duration. Tracks
    /// from batches committed before the failure remain added; the failing
    /// batch is rolled back, and the in-RAM index stays consistent with
    /// whatever the database now holds.
    pub fn add_tracks<I>(&mut self, tracks: I) -> Result<usize, IndexError>
    where
        I: IntoIterator<Item = TrackMeta>,
    {
        let mut iter = tracks.into_iter().peekable();
        let mut added = 0;
        let result = self.insert_batches(&mut iter, &mut added);
        // Re-sort exactly once whether or not a batch failed, so the index
        // order always matches what actually landed.
        self.index.rebuild_order();
        result.map(|()| added)
    }

    fn insert_batches<I>(
        &mut self,
        iter: &mut Peekable<I>,
        added: &mut usize,
    ) -> Result<(), IndexError>
    where
        I: Iterator<Item = TrackMeta>,
    {
        while iter.peek().is_some() {
            let chunk: Vec<TrackMeta> = iter.by_ref().take(TRANSACTION_BATCH).collect();
            let tx = self.conn.transaction()?;
            {
                let mut stmt = tx.prepare_cached(UPSERT_TRACK)?;
                for meta in &chunk {
                    stmt.execute(params![
                        path_to_blob(&meta.path),
                        meta.artist,
                        meta.album,
                        meta.title,
                        meta.track,
                        meta.disc,
                        meta.year,
                        duration_to_nanos(meta)?,
                    ])?;
                }
            }
            tx.commit()?;
            // Mirror into RAM only after the batch is durably committed, so
            // a failed batch never leaves ghost tracks in the index.
            *added += chunk.len();
            for meta in chunk {
                self.index.insert(meta);
            }
        }
        Ok(())
    }

    /// Search the library: literal, case-insensitive substring match over
    /// artist + album + title, capped at `limit` results, in library order
    /// (artist, album, disc, track, title — see the [module docs](self)).
    ///
    /// An empty `query` returns no results, deliberately (module docs), and
    /// so does a query containing `\n` — that is the field/record separator
    /// inside the search corpus, so such a query could only ever ask for a
    /// cross-field match, which search does not offer.
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<&TrackMeta> {
        if query.is_empty() || query.contains('\n') || limit == 0 {
            return Vec::new();
        }
        let needle = query.to_lowercase();
        let mut results: Vec<&TrackMeta> = Vec::new();
        let mut last_index = usize::MAX;
        // One SIMD scan (memmem) over the whole corpus; byte-wise matching
        // is sound because UTF-8 is self-synchronizing — a valid-UTF-8
        // needle can only match at character boundaries. Positions come
        // back in ascending order, which is library order, so results need
        // no re-sorting. Multiple matches inside one track arrive
        // consecutively and are deduplicated against the previous hit.
        for position in memchr::memmem::find_iter(self.index.corpus.as_bytes(), needle.as_bytes()) {
            let index = self.index.track_index_at_offset(position);
            if index == last_index {
                continue;
            }
            last_index = index;
            if let Some(track) = self.index.tracks.get(index) {
                results.push(&track.meta);
                if results.len() == limit {
                    break;
                }
            }
        }
        results
    }

    /// All tracks in library order (artist, album, disc, track, title, path).
    pub fn tracks(&self) -> impl Iterator<Item = &TrackMeta> {
        self.index.in_order().map(|track| &track.meta)
    }

    /// Number of tracks in the library.
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.tracks.len()
    }

    /// Whether the library holds no tracks at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.tracks.is_empty()
    }

    /// The shelf view: tracks grouped into albums, sorted by artist then
    /// album title (case-insensitively), tracks within each album ordered by
    /// disc, then track number, then title.
    ///
    /// The grouping key is artist + album (an album-artist tag can refine
    /// this later), compared case-insensitively, so the same album title
    /// under different artists stays separate. Tracks with an unknown artist
    /// or album group under that unknown (`None`) key — all artist-less,
    /// album-less strays share one shelf entry — and unknowns sort before
    /// known values, so they surface at the front rather than hiding at the
    /// end of a long shelf.
    #[must_use]
    pub fn albums(&self) -> Vec<Album<'_>> {
        let mut albums: Vec<Album<'_>> = Vec::new();
        let mut current_key: Option<(&Option<String>, &Option<String>)> = None;
        // Library order sorts by folded (artist, album, ...) first, so each
        // album is one consecutive run, already in in-album track order.
        for track in self.index.in_order() {
            let key = (&track.key.artist, &track.key.album);
            if current_key != Some(key) {
                current_key = Some(key);
                albums.push(Album {
                    artist: track.meta.artist.as_deref(),
                    title: track.meta.album.as_deref(),
                    year: None,
                    tracks: Vec::new(),
                });
            }
            if let Some(album) = albums.last_mut() {
                if album.year.is_none() {
                    album.year = track.meta.year;
                }
                album.tracks.push(&track.meta);
            }
        }
        albums
    }
}

/// One album on the shelf, as grouped by [`Library::albums`]. Borrows from
/// the library; a snapshot to render, not a place to mutate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Album<'a> {
    /// Artist name as first seen on the album's tracks; `None` for the
    /// unknown-artist group.
    pub artist: Option<&'a str>,
    /// Album title as first seen; `None` for the unknown-album group.
    pub title: Option<&'a str>,
    /// Release year: the first year any track on the album declares.
    pub year: Option<u32>,
    /// The album's tracks in disc/track-number/title order; paths and
    /// per-track detail live on each [`TrackMeta`].
    pub tracks: Vec<&'a TrackMeta>,
}

/// The in-RAM half of [`Library`]: every track with a precomputed
/// case-folded haystack and sort key.
///
/// `tracks` is kept **physically sorted** in library order rather than
/// indirected through an index vector: search is the per-keystroke hot path
/// and scans every haystack on a miss, so it must walk contiguous memory,
/// not chase indices. The price is re-sorting and re-mapping after each
/// added batch — a scan-time cost, paid off the hot path.
#[derive(Default)]
struct SearchIndex {
    /// Track storage in library order (see [`SearchIndex::rebuild_order`]).
    tracks: Vec<IndexedTrack>,
    /// Path → position in `tracks`, for upsert semantics. Rebuilt together
    /// with the sort, so it is only valid between batches — which is the
    /// only time `insert` runs.
    by_path: HashMap<PathBuf, usize>,
    /// Every track's haystack concatenated in library order. Search runs
    /// *one* substring scan over this single buffer instead of one call per
    /// track — that difference is what turns ~2 ms per keystroke into
    /// sub-ms (see `benches/search.rs`). Records end with `\n` and queries
    /// containing `\n` are rejected, so a match can never span two tracks.
    corpus: String,
    /// Byte offset in `corpus` where each track's haystack starts; maps a
    /// match position back to a track via binary search. Always starts with
    /// 0 and is strictly increasing (every haystack is non-empty).
    starts: Vec<usize>,
}

impl SearchIndex {
    /// Insert a track, replacing any existing entry for the same path.
    /// Callers must [`SearchIndex::rebuild_order`] afterwards (batched).
    fn insert(&mut self, meta: TrackMeta) {
        let entry = IndexedTrack::new(meta);
        match self.by_path.entry(entry.meta.path.clone()) {
            Entry::Occupied(slot) => {
                let index = *slot.get();
                if let Some(existing) = self.tracks.get_mut(index) {
                    *existing = entry;
                }
            }
            Entry::Vacant(slot) => {
                slot.insert(self.tracks.len());
                self.tracks.push(entry);
            }
        }
    }

    /// Re-sort storage into library order — folded artist, album, disc,
    /// track, title, with the (unique) path as the final tiebreak so the
    /// order is total and deterministic — and re-map paths to their new
    /// positions. `None` sorts before `Some`, so unknown fields group at
    /// the front.
    fn rebuild_order(&mut self) {
        self.tracks.sort_unstable_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then_with(|| a.meta.path.cmp(&b.meta.path))
        });
        self.by_path.clear();
        self.corpus.clear();
        self.starts.clear();
        for (index, track) in self.tracks.iter().enumerate() {
            self.by_path.insert(track.meta.path.clone(), index);
            self.starts.push(self.corpus.len());
            self.corpus.push_str(&track.haystack);
        }
    }

    /// Position in `tracks` of the track containing byte offset `position`
    /// of the corpus.
    fn track_index_at_offset(&self, position: usize) -> usize {
        // `starts[0] == 0`, so partition_point is always >= 1 for any
        // position; saturation is belt-and-braces, not a reachable case.
        self.starts
            .partition_point(|&start| start <= position)
            .saturating_sub(1)
    }

    /// Tracks in library order.
    fn in_order(&self) -> impl Iterator<Item = &IndexedTrack> {
        self.tracks.iter()
    }
}

/// One track plus what search and ordering precompute at insert time, so a
/// keystroke costs a substring scan and nothing else.
struct IndexedTrack {
    meta: TrackMeta,
    /// Case-folded `artist\nalbum\ntitle` (the separator keeps a query from
    /// matching across field boundaries).
    haystack: String,
    key: SortKey,
}

impl IndexedTrack {
    fn new(meta: TrackMeta) -> Self {
        let artist = meta.artist.as_deref().map(str::to_lowercase);
        let album = meta.album.as_deref().map(str::to_lowercase);
        let title = meta.title.as_deref().map(str::to_lowercase);
        let mut haystack = String::new();
        for part in [&artist, &album, &title] {
            if let Some(text) = part {
                haystack.push_str(text);
            }
            haystack.push('\n');
        }
        let key = SortKey {
            artist,
            album,
            disc: meta.disc,
            track: meta.track,
            title,
        };
        Self {
            meta,
            haystack,
            key,
        }
    }
}

/// Library-order sort key: case-folded strings, `None` first (see
/// [`SearchIndex::rebuild_order`]). Field order *is* the sort order.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SortKey {
    artist: Option<String>,
    album: Option<String>,
    disc: Option<u32>,
    track: Option<u32>,
    title: Option<String>,
}

/// Run pending schema migrations, stepwise, up to [`SCHEMA_VERSION`].
///
/// Each arm migrates exactly one version and the loop re-reads
/// `user_version`, so future versions chain automatically: v0.2 adds a
/// `1 => ...` arm and bumps [`SCHEMA_VERSION`], nothing else.
fn migrate(conn: &Connection) -> Result<(), IndexError> {
    loop {
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            // A fresh database: create the current schema directly.
            0 => conn.execute_batch(SCHEMA_V1)?,
            SCHEMA_VERSION => return Ok(()),
            found => return Err(IndexError::SchemaTooNew { found }),
        }
    }
}

/// Map one `tracks` row back to a [`TrackMeta`]. Column order must match
/// [`SELECT_ALL_TRACKS`].
fn row_to_meta(row: &rusqlite::Row<'_>) -> Result<TrackMeta, IndexError> {
    let path_blob: Vec<u8> = row.get(0)?;
    let duration_ns: Option<i64> = row.get(7)?;
    let duration = duration_ns
        .map(|nanos| u64::try_from(nanos).map(Duration::from_nanos))
        .transpose()
        .map_err(|_| IndexError::CorruptStoredDuration)?;
    Ok(TrackMeta {
        path: path_from_blob(path_blob)?,
        artist: row.get(1)?,
        album: row.get(2)?,
        title: row.get(3)?,
        track: row.get(4)?,
        disc: row.get(5)?,
        year: row.get(6)?,
        duration,
    })
}

/// A track duration as the nanosecond count stored in `duration_ns`.
fn duration_to_nanos(meta: &TrackMeta) -> Result<Option<i64>, IndexError> {
    meta.duration
        .map(|duration| {
            i64::try_from(duration.as_nanos()).map_err(|_| IndexError::DurationOutOfRange {
                path: meta.path.clone(),
            })
        })
        .transpose()
}

/// Encode a path for the `path BLOB` column — the platform-native lossless
/// encoding described in the [module docs](self).
#[cfg(unix)]
fn path_to_blob(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

/// Encode a path for the `path BLOB` column — the platform-native lossless
/// encoding described in the [module docs](self).
#[cfg(windows)]
fn path_to_blob(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Decode a `path BLOB` back to a [`PathBuf`]; inverse of [`path_to_blob`].
#[cfg(unix)]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Unix OsString accepts any byte sequence, so decoding cannot fail here; \
              the signature matches the fallible Windows implementation"
)]
fn path_from_blob(blob: Vec<u8>) -> Result<PathBuf, IndexError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(blob)))
}

/// Decode a `path BLOB` back to a [`PathBuf`]; inverse of [`path_to_blob`].
#[cfg(windows)]
fn path_from_blob(blob: Vec<u8>) -> Result<PathBuf, IndexError> {
    use std::os::windows::ffi::OsStringExt;
    if blob.len() % 2 != 0 {
        return Err(IndexError::CorruptStoredPath);
    }
    let wide: Vec<u16> = blob
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_blob_roundtrips() {
        for path in ["/music/artist/album/01 track.flac", "/tmp/ünï çödé/曲.mp3"] {
            let path = PathBuf::from(path);
            let back = path_from_blob(path_to_blob(&path)).expect("decode");
            assert_eq!(back, path);
        }
    }

    #[cfg(unix)]
    #[test]
    fn path_blob_roundtrips_non_utf8_bytes() {
        use std::os::unix::ffi::OsStringExt;
        let raw = b"/music/\xFF\xFEbroken/tr\xF0ack.flac".to_vec();
        let path = PathBuf::from(std::ffi::OsString::from_vec(raw));
        assert!(path.to_str().is_none(), "fixture must not be valid UTF-8");
        let back = path_from_blob(path_to_blob(&path)).expect("decode");
        assert_eq!(back, path);
    }

    #[test]
    fn haystack_separates_fields_and_folds_case() {
        let track = IndexedTrack::new(TrackMeta {
            path: PathBuf::from("/m/a.flac"),
            artist: Some("Größenwahn".to_owned()),
            album: Some("LIVE".to_owned()),
            title: None,
            track: None,
            disc: None,
            year: None,
            duration: None,
        });
        assert_eq!(track.haystack, "größenwahn\nlive\n\n");
        // The separator keeps queries from matching across field boundaries.
        assert!(!track.haystack.contains("wahnlive"));
    }

    #[test]
    fn fresh_database_migrates_to_current_version() {
        let library = Library::open_in_memory().expect("open");
        let version: i64 = library
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("read user_version");
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn duration_nanos_conversion_is_exact() {
        let meta = TrackMeta {
            path: PathBuf::from("/m/a.flac"),
            artist: None,
            album: None,
            title: None,
            track: None,
            disc: None,
            year: None,
            duration: Some(Duration::new(215, 123_456_789)),
        };
        let nanos = duration_to_nanos(&meta).expect("convert").expect("some");
        assert_eq!(nanos, 215_123_456_789);
    }
}
