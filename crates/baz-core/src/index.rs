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
//! substring match over each track's artist + album artist + album + title,
//! folded with [`str::to_lowercase`] (full case folding such as `ß`/`SS`
//! equivalence is out of scope for now). The album artist is included only
//! when it differs from the artist, so the ordinary album adds nothing to
//! the corpus a keystroke scans while the name on a soundtrack's tile stays
//! findable. Results come back in library order — album artist, album, disc,
//! track, title, path — so repeated queries are deterministic. An
//! **empty query returns nothing**: every haystack contains the empty
//! string, so the only honest answer would be the entire library truncated
//! at `limit`, which would misrepresent a 100k-track library as `limit`
//! tracks. "No query yet" is the shelf's state ([`Library::albums`]), not a
//! search result.
//!
//! # Schema versioning
//!
//! The schema version lives in SQLite's `PRAGMA user_version` and migrations
//! run stepwise at open (see `migrate`), so a schema change is one new
//! match arm, not a format break. A database from a *newer* baz is refused
//! rather than guessed at. Every database — including a brand-new one —
//! walks the same chain from 0, so "created fresh" and "upgraded" can never
//! drift into two different shapes.
//!
//! # What leaves the library
//!
//! [`Library::remove_tracks`] is the only path out, and it decides nothing:
//! it deletes the paths it is handed. Whether a file is *gone* — as opposed
//! to unseen, unreadable, or on a drive that is not plugged in today — is
//! answered against the filesystem before a path ever reaches here
//! ([`crate::library::is_confirmed_gone`], and the four gates in
//! `docs/adr/0010-incremental-scanning-and-removal.md`). Keeping the
//! judgement out of the store is what makes "baz deleted my library" a
//! thing that can be tested for in one place.
//!
//! [`Library::known_files`] is the other half of the same conversation: the
//! snapshot a scan needs to skip unchanged files, and the only list of rows
//! a scan worker can ever nominate for removal.
//!
//! # Albums and editions
//!
//! [`Library::albums`] groups tracks into albums by **album artist** +
//! title ([`AlbumArtist`], `docs/adr/0008-album-artist-grouping.md`), then
//! splits each album by [`AudioFormat`] into [`Edition`]s: one shelf tile,
//! one track list per format the collector actually owns. The ranking that
//! decides which edition is the default is documented on
//! [`Album::editions`] and in `docs/adr/0007-album-editions.md`.

use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, params};

use crate::library::{AudioFormat, FileStamp, KnownFiles, TrackMeta};
use crate::replaygain::{ComputedGains, ComputedReplayGain, ReplayGainTags};

/// The schema version this build reads and writes (`PRAGMA user_version`).
const SCHEMA_VERSION: i64 = 6;

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

/// Version 2: the per-track encoding columns that album editions are built
/// from (`docs/adr/0007-album-editions.md`). Added rather than rebuilt, so
/// an existing library keeps its rows, its `id`s, and its `path` index.
///
/// The transaction and the `user_version` bump are applied by
/// [`migrate_v1_to_v2`], which has Rust work to do in between.
const SCHEMA_V2_COLUMNS: &str = "
    ALTER TABLE tracks ADD COLUMN format      TEXT;    -- AudioFormat::code()
    ALTER TABLE tracks ADD COLUMN bit_depth   INTEGER; -- bits per sample
    ALTER TABLE tracks ADD COLUMN sample_rate INTEGER; -- Hz
    ALTER TABLE tracks ADD COLUMN bitrate     INTEGER; -- kbit/s, VBR-averaged
";

/// Version 3: the album-artist grouping key
/// (`docs/adr/0008-album-artist-grouping.md`). Two more nullable columns,
/// added rather than rebuilt, exactly as v2 was.
///
/// The transaction and the `user_version` bump are applied by
/// [`migrate_v2_to_v3`].
const SCHEMA_V3_COLUMNS: &str = "
    ALTER TABLE tracks ADD COLUMN album_artist TEXT;    -- ALBUMARTIST/TPE2/aART
    ALTER TABLE tracks ADD COLUMN compilation  INTEGER; -- 0/1 flag; NULL = unsaid
";

/// Version 4: the file stamp incremental scanning compares
/// (`docs/adr/0010-incremental-scanning-and-removal.md`). Two more nullable columns,
/// added rather than rebuilt, exactly as v2 and v3 were.
///
/// The transaction and the `user_version` bump are applied by
/// [`migrate_v3_to_v4`].
const SCHEMA_V4_COLUMNS: &str = "
    ALTER TABLE tracks ADD COLUMN mtime_ns  INTEGER; -- ns since the Unix epoch
    ALTER TABLE tracks ADD COLUMN file_size INTEGER; -- bytes
";

/// Version 5: the ReplayGain figures a file already carries
/// (`docs/adr/0013-replaygain.md`). Four more nullable columns, added rather
/// than rebuilt, exactly as v2, v3 and v4 were.
///
/// Integer columns, in the units [`crate::replaygain`] argues for: gains in
/// hundredths of a decibel, peaks in millionths of full scale. A `REAL` column
/// would have stored a value the tag never carried that much precision for,
/// and would have cost [`TrackMeta`] its `Eq`.
///
/// The transaction and the `user_version` bump are applied by
/// [`migrate_v4_to_v5`].
const SCHEMA_V5_COLUMNS: &str = "
    ALTER TABLE tracks ADD COLUMN rg_track_gain_centidb INTEGER; -- 0.01 dB
    ALTER TABLE tracks ADD COLUMN rg_track_peak_micro   INTEGER; -- 1e-6 FS
    ALTER TABLE tracks ADD COLUMN rg_album_gain_centidb INTEGER; -- 0.01 dB
    ALTER TABLE tracks ADD COLUMN rg_album_peak_micro   INTEGER; -- 1e-6 FS
";

/// Version 6: the ReplayGain figures **baz measured itself**
/// (`docs/adr/0015-replaygain-analysis.md`). Six more nullable columns, added
/// rather than rebuilt, exactly as v2 – v5 were.
///
/// # Why they are separate columns rather than the v5 ones
///
/// Because a computed figure and a tagged one are different claims and the
/// database is where the difference has to survive:
///
/// - **The scanner cannot destroy a measurement.** [`UPSERT_TRACK`] names the
///   v5 columns and not these, so a rescan — which knows only what a file's
///   tags say — rewrites the tag columns and leaves these untouched. That is
///   the "must not fight the incremental scanner" property, held by the shape
///   of the schema rather than by two writers agreeing to be careful.
/// - **"Where did this figure come from" has a true answer.** The selection
///   rule ([`ReplayGainSettings::resolve_with`](crate::replaygain::ReplayGainSettings::resolve_with))
///   can prefer the tag *field by field* and report which one it used, because
///   both are still there to choose between.
///
/// The last two columns are the [`FileStamp`] of the file **as measured**. A
/// loudness figure is a claim about a file's samples, so it stops being true
/// when the file changes; storing the stamp is what lets a stale measurement be
/// recognised and ignored rather than played
/// ([`ComputedReplayGain::is_fresh_for`]).
///
/// The transaction and the `user_version` bump are applied by
/// [`migrate_v5_to_v6`].
const SCHEMA_V6_COLUMNS: &str = "
    ALTER TABLE tracks ADD COLUMN rg_computed_track_gain_centidb INTEGER; -- 0.01 dB
    ALTER TABLE tracks ADD COLUMN rg_computed_track_peak_micro   INTEGER; -- 1e-6 FS
    ALTER TABLE tracks ADD COLUMN rg_computed_album_gain_centidb INTEGER; -- 0.01 dB
    ALTER TABLE tracks ADD COLUMN rg_computed_album_peak_micro   INTEGER; -- 1e-6 FS
    ALTER TABLE tracks ADD COLUMN rg_computed_mtime_ns           INTEGER; -- ns since epoch
    ALTER TABLE tracks ADD COLUMN rg_computed_file_size          INTEGER; -- bytes
";

/// Write one track's measured ReplayGain (schema v6).
///
/// An `UPDATE` rather than an upsert on purpose: a measurement belongs to a
/// track the library already holds, and a path the library does not hold is a
/// file that was removed while the pass was running — which updates nothing and
/// is exactly right.
const STORE_COMPUTED_REPLAY_GAIN: &str = "
    UPDATE tracks SET
        rg_computed_track_gain_centidb = ?2,
        rg_computed_track_peak_micro   = ?3,
        rg_computed_album_gain_centidb = ?4,
        rg_computed_album_peak_micro   = ?5,
        rg_computed_mtime_ns           = ?6,
        rg_computed_file_size          = ?7
    WHERE path = ?1
";

/// Insert-or-replace by path: a rescan of the same file updates its metadata
/// instead of failing the batch or duplicating the track.
///
/// **The `rg_computed_*` columns are deliberately absent** from both the insert
/// list and the update list (schema v6): a scan reads tags, and a measurement
/// is not a tag. A new row therefore gets `NULL` measurements, and an updated
/// row keeps whatever it had — which, if the file really changed, is a stamp
/// that no longer matches and so a measurement nothing will use.
const UPSERT_TRACK: &str = "
    INSERT INTO tracks
        (path, artist, album, title, track, disc, year, duration_ns,
         format, bit_depth, sample_rate, bitrate, album_artist, compilation,
         mtime_ns, file_size,
         rg_track_gain_centidb, rg_track_peak_micro,
         rg_album_gain_centidb, rg_album_peak_micro)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
            ?15, ?16, ?17, ?18, ?19, ?20)
    ON CONFLICT(path) DO UPDATE SET
        artist = excluded.artist,
        album = excluded.album,
        title = excluded.title,
        track = excluded.track,
        disc = excluded.disc,
        year = excluded.year,
        duration_ns = excluded.duration_ns,
        format = excluded.format,
        bit_depth = excluded.bit_depth,
        sample_rate = excluded.sample_rate,
        bitrate = excluded.bitrate,
        album_artist = excluded.album_artist,
        compilation = excluded.compilation,
        mtime_ns = excluded.mtime_ns,
        file_size = excluded.file_size,
        rg_track_gain_centidb = excluded.rg_track_gain_centidb,
        rg_track_peak_micro = excluded.rg_track_peak_micro,
        rg_album_gain_centidb = excluded.rg_album_gain_centidb,
        rg_album_peak_micro = excluded.rg_album_peak_micro
";

const SELECT_ALL_TRACKS: &str = "
    SELECT path, artist, album, title, track, disc, year, duration_ns,
           format, bit_depth, sample_rate, bitrate, album_artist, compilation,
           mtime_ns, file_size,
           rg_track_gain_centidb, rg_track_peak_micro,
           rg_album_gain_centidb, rg_album_peak_micro,
           rg_computed_track_gain_centidb, rg_computed_track_peak_micro,
           rg_computed_album_gain_centidb, rg_computed_album_peak_micro,
           rg_computed_mtime_ns, rg_computed_file_size
    FROM tracks
";

/// Delete one row by path. The `path` column is `UNIQUE`, so this removes at
/// most one row and reports whether it did.
const DELETE_TRACK: &str = "DELETE FROM tracks WHERE path = ?1";

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
        let rows = stmt.query_and_then([], |row| {
            Ok::<_, IndexError>((row_to_meta(row)?, row_to_computed(row)?))
        })?;
        for row in rows {
            let (meta, computed) = row?;
            self.index.put(meta, computed);
        }
        self.index.rebuild_order();
        Ok(())
    }

    /// Re-read everything from the database, replacing the in-RAM index.
    ///
    /// For a holder that knows **another connection** has written to the same
    /// file — which today is exactly one caller, [`crate::analysis`], whose
    /// worker opens the library a second time and plans each pass against what
    /// the scanner has since stored. SQLite in WAL mode makes the concurrency
    /// legal; this makes it visible.
    ///
    /// It is a full hydrate, so it costs what opening the library costs. That
    /// is the honest price of a snapshot: there is no way to learn what changed
    /// without asking.
    ///
    /// # Errors
    ///
    /// [`IndexError::Sqlite`] if the read fails and
    /// [`IndexError::CorruptStoredPath`] if a stored path cannot be decoded on
    /// this platform. The in-RAM index is left empty in that case rather than
    /// half-loaded.
    pub fn reload(&mut self) -> Result<(), IndexError> {
        self.hydrate()
    }

    /// What a ReplayGain analysis measured for `path`, if it measured anything
    /// that **still applies** to the file the index knows (ADR-0015).
    ///
    /// A measurement whose stamp no longer matches the row's is reported as no
    /// measurement: the figures describe samples, and the samples have moved.
    /// The file is then simply one that needs measuring again, which is the
    /// state it was in before it was ever measured.
    #[must_use]
    pub fn computed_replay_gain(&self, path: &Path) -> ReplayGainTags {
        self.index
            .by_path
            .get(path)
            .and_then(|&index| self.index.tracks.get(index))
            .map_or_else(ReplayGainTags::default, |track| {
                track.computed.figures_for(track.meta.stamp)
            })
    }

    /// The whole library's still-applying measurements, as the lookup the
    /// engine's seam takes ([`ComputedGains`]).
    ///
    /// A snapshot rather than a live view, and owned rather than borrowed:
    /// the engine consults it from its own thread at a track boundary, and a
    /// front end replaces it wholesale after an analysis pass finishes. Only
    /// tracks with something measured are included, so an unmeasured library
    /// costs an empty map.
    #[must_use]
    pub fn computed_gains(&self) -> ComputedGainMap {
        ComputedGainMap(
            self.index
                .tracks
                .iter()
                .filter_map(|track| {
                    let figures = track.computed.figures_for(track.meta.stamp);
                    (!figures.is_empty()).then(|| (track.meta.path.clone(), figures))
                })
                .collect(),
        )
    }

    /// Record what a ReplayGain analysis measured, for tracks the library
    /// holds (ADR-0015).
    ///
    /// One transaction for the whole batch, because the batch is an album
    /// edition and an edition's figures are a set: a crash must not leave an
    /// album whose tracks were measured against an album gain that was never
    /// written. Paths the library does not hold are ignored — a file removed
    /// while the pass was running is not an error, it is news.
    ///
    /// Returns the number of rows actually written.
    ///
    /// # Errors
    ///
    /// [`IndexError::Sqlite`] if the write fails; the whole batch is then
    /// rolled back and the in-RAM index is left matching the database.
    pub fn store_computed_replay_gain<I>(&mut self, measurements: I) -> Result<usize, IndexError>
    where
        I: IntoIterator<Item = (PathBuf, ComputedReplayGain)>,
    {
        let measurements: Vec<(PathBuf, ComputedReplayGain)> = measurements.into_iter().collect();
        let mut written = 0;
        {
            let tx = self.conn.transaction()?;
            {
                let mut stmt = tx.prepare_cached(STORE_COMPUTED_REPLAY_GAIN)?;
                for (path, computed) in &measurements {
                    written += stmt.execute(params![
                        path_to_blob(path),
                        computed.figures.track_gain_centidb,
                        computed.figures.track_peak_micro,
                        computed.figures.album_gain_centidb,
                        computed.figures.album_peak_micro,
                        computed.stamp.map(|stamp| stamp.mtime_ns),
                        computed
                            .stamp
                            .and_then(|stamp| i64::try_from(stamp.size).ok()),
                    ])?;
                }
            }
            tx.commit()?;
        }
        // Mirror into RAM only after the batch is durably committed, exactly as
        // `add_tracks` does. A measurement changes neither the sort key nor the
        // search corpus, so no re-sort is needed.
        for (path, computed) in measurements {
            if let Some(&index) = self.index.by_path.get(&path)
                && let Some(track) = self.index.tracks.get_mut(index)
            {
                track.computed = computed;
            }
        }
        Ok(written)
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
                        meta.format.map(AudioFormat::code),
                        meta.bit_depth,
                        meta.sample_rate,
                        meta.bitrate,
                        meta.album_artist,
                        meta.compilation,
                        meta.stamp.map(|stamp| stamp.mtime_ns),
                        meta.stamp.and_then(|stamp| i64::try_from(stamp.size).ok()),
                        meta.replay_gain.track_gain_centidb,
                        meta.replay_gain.track_peak_micro,
                        meta.replay_gain.album_gain_centidb,
                        meta.replay_gain.album_peak_micro,
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

    /// Remove tracks by path: delete their rows and drop them from the
    /// in-RAM index. Paths the library does not hold are ignored. Returns
    /// the number of rows actually deleted.
    ///
    /// This is the only way anything leaves the library, and it is
    /// deliberately dumb: it deletes exactly the paths it is handed and
    /// decides nothing. Whether a file is *gone* — as opposed to merely
    /// unseen, unreadable, or on a drive that is not plugged in today — is
    /// [`crate::library::is_confirmed_gone`]'s question, answered against
    /// the filesystem before a path ever reaches here.
    ///
    /// # Errors
    ///
    /// [`IndexError::Sqlite`] if a delete fails. Batches committed before
    /// the failure stay deleted; the failing batch is rolled back and the
    /// in-RAM index is left matching whatever the database now holds.
    pub fn remove_tracks<I, P>(&mut self, paths: I) -> Result<usize, IndexError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let paths: Vec<PathBuf> = paths
            .into_iter()
            .map(|path| path.as_ref().to_path_buf())
            .collect();
        let mut deleted = 0;
        let mut gone: HashSet<&Path> = HashSet::new();
        let result = self.delete_batches(&paths, &mut deleted, &mut gone);
        if !gone.is_empty() {
            self.index
                .tracks
                .retain(|track| !gone.contains(track.meta.path.as_path()));
            self.index.rebuild_order();
        }
        result.map(|()| deleted)
    }

    fn delete_batches<'p>(
        &mut self,
        paths: &'p [PathBuf],
        deleted: &mut usize,
        gone: &mut HashSet<&'p Path>,
    ) -> Result<(), IndexError> {
        for chunk in paths.chunks(TRANSACTION_BATCH) {
            let tx = self.conn.transaction()?;
            let mut removed_here: Vec<&Path> = Vec::new();
            {
                let mut stmt = tx.prepare_cached(DELETE_TRACK)?;
                for path in chunk {
                    if stmt.execute(params![path_to_blob(path)])? > 0 {
                        removed_here.push(path.as_path());
                    }
                }
            }
            tx.commit()?;
            // Mirror into RAM only after the batch is durably committed, so
            // a failed batch never drops a track the database still holds.
            *deleted += removed_here.len();
            gone.extend(removed_here);
        }
        Ok(())
    }

    /// Every path the library holds, with the [`FileStamp`] recorded for it
    /// — the input an incremental scan needs
    /// ([`crate::library::scan_incremental`]).
    ///
    /// A row written before schema v4, or one for a file whose filesystem
    /// could not report a usable timestamp, maps to `None` and is therefore
    /// always re-read. The map is a snapshot: it is handed to a scan worker
    /// that runs while the library keeps being written to, so it owns its
    /// paths rather than borrowing them.
    #[must_use]
    pub fn known_files(&self) -> KnownFiles {
        self.index
            .tracks
            .iter()
            .map(|track| (track.meta.path.clone(), track.meta.stamp))
            .collect()
    }

    /// Search the library: literal, case-insensitive substring match over
    /// artist + album artist + album + title, capped at `limit` results, in
    /// library order (album artist, album, disc, track, title — see the
    /// [module docs](self)).
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

    /// The shelf view: tracks grouped into albums, sorted by album artist
    /// then album title (case-insensitively), tracks within each album
    /// ordered by disc, then track number, then title.
    ///
    /// The grouping key is **album artist + album**, compared
    /// case-insensitively, so the same album title under different artists
    /// stays separate while a soundtrack whose every track names a different
    /// composer stays *together*. The album artist of a track is resolved by
    /// the fallback chain on [`AlbumArtist`]. Tracks with an unknown album
    /// artist or album group under that unknown key — all artist-less,
    /// album-less strays share one shelf entry — and unknowns sort before
    /// known values, so they surface at the front rather than hiding at the
    /// end of a long shelf; compilations without a named album artist sort
    /// after every named one, the other end of the same shelf.
    ///
    /// Each album is then split by codec into [`Album::editions`]: the same
    /// album ripped to FLAC *and* to MP3 is **one** entry with two editions,
    /// not two entries and not one entry with every track listed twice.
    #[must_use]
    pub fn albums(&self) -> Vec<Album<'_>> {
        let mut albums: Vec<Album<'_>> = Vec::new();
        let mut current_key: Option<(&ArtistKey, &Option<String>)> = None;
        // Library order sorts by folded (album artist, album, ...) first, so
        // each album is one consecutive run, already in in-album track order.
        for track in self.index.in_order() {
            let key = (&track.key.artist, &track.key.album);
            if current_key != Some(key) {
                current_key = Some(key);
                albums.push(Album {
                    artist: AlbumArtist::of(&track.meta),
                    title: track.meta.album.as_deref(),
                    year: None,
                    editions: Vec::new(),
                });
            }
            if let Some(album) = albums.last_mut() {
                if album.year.is_none() {
                    album.year = track.meta.year;
                }
                album.push_track(&track.meta);
            }
        }
        for album in &mut albums {
            album.editions.sort_by(rank_editions);
        }
        albums
    }
}

/// Who an album is filed under — the grouping key's artist half, and the
/// name the shelf tile and side-panel header show.
///
/// This is deliberately **not** an `Option<String>` carrying a
/// `"Various Artists"` sentinel. The owner's own library contains a file
/// whose `TPE2` frame literally reads `Various Artists`, so a magic string
/// could not tell "the tagger named this album's artist" from "baz gave up
/// and called it a compilation" — a distinction that decides whether the
/// name on screen came from the user's own curation or from us. The enum
/// makes the two unconfusable by construction.
///
/// # The fallback chain
///
/// [`AlbumArtist::of`] resolves one track, in this order
/// (`docs/adr/0008-album-artist-grouping.md`):
///
/// 1. **The album-artist tag** ([`TrackMeta::album_artist`]) →
///    [`AlbumArtist::Named`]. This is the tag that exists for exactly this
///    problem, and every serious library manager writes it.
/// 2. **The compilation flag** ([`TrackMeta::compilation`]) →
///    [`AlbumArtist::Various`]. A file that says "I am a compilation"
///    without naming an album artist has told us that its track artist is
///    *not* the album's artist; grouping by the track artist would be
///    following a value the file itself disclaimed.
/// 3. **The track artist** → [`AlbumArtist::Named`]. An album whose tracks
///    share one artist — the overwhelming majority — groups exactly as it
///    did before album artists existed.
/// 4. **Nothing** → [`AlbumArtist::Unknown`].
///
/// The chain is per *track* rather than per album on purpose: it is the
/// grouping key, so it has to be computable before the album exists. Step 3
/// is what makes "if the album's tracks share one artist, that artist" fall
/// out for free — tracks that share an artist share a key, and tracks that
/// do not are only merged when step 1 or 2 gave a reason to merge them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlbumArtist<'a> {
    /// A named album artist: the album-artist tag, or the track artist when
    /// the file declares no album artist.
    Named(&'a str),
    /// A compilation with no named album artist — tracks by different
    /// artists that the files themselves flag as belonging to one release.
    Various,
    /// Nothing is known: no album artist, no compilation flag, no artist.
    Unknown,
}

impl<'a> AlbumArtist<'a> {
    /// Resolve one track's album artist by the fallback chain documented on
    /// [`AlbumArtist`].
    #[must_use]
    pub fn of(meta: &'a TrackMeta) -> Self {
        if let Some(name) = meta.album_artist.as_deref() {
            return Self::Named(name);
        }
        if meta.compilation == Some(true) {
            return Self::Various;
        }
        match meta.artist.as_deref() {
            Some(name) => Self::Named(name),
            None => Self::Unknown,
        }
    }

    /// The name, when there is one. `None` for a compilation or an unknown
    /// — neither has a name, and inventing one is the caller's decision to
    /// make in the language of its own presentation layer.
    #[must_use]
    pub fn name(self) -> Option<&'a str> {
        match self {
            Self::Named(name) => Some(name),
            Self::Various | Self::Unknown => None,
        }
    }
}

/// One album on the shelf, as grouped by [`Library::albums`]. Borrows from
/// the library; a snapshot to render, not a place to mutate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Album<'a> {
    /// Who the album is filed under, resolved by [`AlbumArtist::of`] from
    /// the album's first track (every track in the album shares this key).
    pub artist: AlbumArtist<'a>,
    /// Album title as first seen; `None` for the unknown-album group.
    pub title: Option<&'a str>,
    /// Release year: the first year any track on the album declares.
    pub year: Option<u32>,
    /// The formats this album is owned in, **best first** — never empty (an
    /// album exists because it has tracks, and every track lands in exactly
    /// one edition).
    ///
    /// The ordering is total and deterministic, applied in this sequence:
    ///
    /// 1. **Lossless before lossy, unknown-codec last.** Fidelity is the
    ///    point of keeping a second copy; see [`AudioFormat::is_lossless`].
    /// 2. **More tracks first.** Within a tier this prefers the complete rip
    ///    over a partial one — playing 3 of 12 tracks by default would be
    ///    the worse failure.
    /// 3. **Higher mean bitrate first.** A 24/96 FLAC over a 16/44 one, a
    ///    320 kbit/s MP3 over a 128. Across *different* lossy codecs this is
    ///    a preference, not a fidelity claim (128 kbit/s Opus is not worse
    ///    than 192 kbit/s MP3) — it only ever breaks a tie, and some answer
    ///    must be given.
    /// 4. **Codec code, ascending.** Nothing but determinism rides on this.
    ///
    /// The user can always override the default in the UI; this decides only
    /// what is offered first. See `docs/adr/0007-album-editions.md`.
    pub editions: Vec<Edition<'a>>,
}

impl<'a> Album<'a> {
    /// The edition to show and play unless the user says otherwise: the
    /// best-ranked one (see [`Album::editions`]).
    #[must_use]
    pub fn default_edition(&self) -> Option<&Edition<'a>> {
        self.editions.first()
    }

    /// The edition in `format`, if this album has one.
    #[must_use]
    pub fn edition(&self, format: Option<AudioFormat>) -> Option<&Edition<'a>> {
        self.editions.iter().find(|e| e.format == format)
    }

    /// File this track under its codec's edition, creating that edition on
    /// first sight. Tracks arrive in library order, so appending preserves
    /// each edition's disc/track/title order without a second sort.
    fn push_track(&mut self, meta: &'a TrackMeta) {
        let format = meta.format;
        if let Some(edition) = self.editions.iter_mut().find(|e| e.format == format) {
            edition.tracks.push(meta);
        } else {
            self.editions.push(Edition {
                format,
                tracks: vec![meta],
            });
        }
    }
}

/// One album as owned in one codec: the FLAC rip, or the MP3 copy.
///
/// An album with a single edition is the ordinary case and behaves exactly
/// as an album did before editions existed — the UI shows no selector for
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edition<'a> {
    /// The codec every track in this edition is encoded with. `None` is the
    /// edition of tracks whose codec is not known (see
    /// [`TrackMeta::format`]) — an honest bucket, not a format.
    pub format: Option<AudioFormat>,
    /// This edition's tracks in disc/track-number/title order.
    pub tracks: Vec<&'a TrackMeta>,
}

impl Edition<'_> {
    /// Whether this edition's codec reconstructs its source bit-exactly. An
    /// unknown codec is not assumed lossless.
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        self.format.is_some_and(AudioFormat::is_lossless)
    }

    /// The bit depth the whole edition shares, or `None` if its tracks
    /// disagree (or never declared one).
    ///
    /// Uniform-or-nothing rather than an average: "16-bit" is a claim about
    /// every track, and an edition with one 24-bit outlier should decline to
    /// make it rather than round the outlier away.
    #[must_use]
    pub fn bit_depth(&self) -> Option<u8> {
        uniform(self.tracks.iter().map(|t| t.bit_depth))
    }

    /// The sample rate the whole edition shares, or `None` if its tracks
    /// disagree (or never declared one). Uniform-or-nothing, as
    /// [`Edition::bit_depth`].
    #[must_use]
    pub fn sample_rate(&self) -> Option<u32> {
        uniform(self.tracks.iter().map(|t| t.sample_rate))
    }

    /// Mean bitrate (kbit/s) over the tracks that declare one, rounded down;
    /// `None` when no track does. An average is the honest summary here —
    /// unlike depth and rate, bitrate legitimately varies track to track,
    /// and VBR means it varies within a track too.
    #[must_use]
    pub fn bitrate(&self) -> Option<u32> {
        let mut sum: u64 = 0;
        let mut count: u64 = 0;
        for rate in self.tracks.iter().filter_map(|t| t.bitrate) {
            sum += u64::from(rate);
            count += 1;
        }
        if count == 0 {
            return None;
        }
        u32::try_from(sum / count).ok()
    }

    /// Fidelity tier for the default-edition ranking: lossless, lossy,
    /// unknown. Lower is better.
    fn tier(&self) -> u8 {
        match self.format {
            Some(format) if format.is_lossless() => 0,
            Some(_) => 1,
            None => 2,
        }
    }
}

/// The best-first edition ordering documented on [`Album::editions`].
fn rank_editions(a: &Edition<'_>, b: &Edition<'_>) -> Ordering {
    a.tier()
        .cmp(&b.tier())
        .then_with(|| b.tracks.len().cmp(&a.tracks.len()))
        .then_with(|| b.bitrate().unwrap_or(0).cmp(&a.bitrate().unwrap_or(0)))
        // Formats are unique per album (they are the grouping key), so this
        // makes the order total.
        .then_with(|| {
            a.format
                .map(AudioFormat::code)
                .cmp(&b.format.map(AudioFormat::code))
        })
}

/// The single value every item declares, or `None` if any is missing or they
/// disagree. An empty sequence yields `None`.
fn uniform<T: Copy + PartialEq>(mut values: impl Iterator<Item = Option<T>>) -> Option<T> {
    let first = values.next()??;
    values.all(|value| value == Some(first)).then_some(first)
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
    /// Insert a track, replacing any existing entry for the same path and
    /// **keeping whatever measurement that entry carried**.
    ///
    /// The keeping is the point: this is the path a rescan takes, and a scan
    /// speaks only about tags. Dropping the measurement here would make the
    /// in-RAM index disagree with the database, which deliberately preserves
    /// the `rg_computed_*` columns across an upsert (see [`UPSERT_TRACK`]).
    /// A measurement of a file that really changed is not lost either — it is
    /// simply stale, which [`ComputedReplayGain::figures_for`] already answers.
    ///
    /// Callers must [`SearchIndex::rebuild_order`] afterwards (batched).
    fn insert(&mut self, meta: TrackMeta) {
        let computed = self
            .by_path
            .get(&meta.path)
            .and_then(|&index| self.tracks.get(index))
            .map_or_else(ComputedReplayGain::default, |track| track.computed);
        self.put(meta, computed);
    }

    /// Insert a track together with the measurement recorded for it.
    fn put(&mut self, meta: TrackMeta, computed: ComputedReplayGain) {
        let entry = IndexedTrack::new(meta, computed);
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

    /// Re-sort storage into library order — folded album artist, album,
    /// disc, track, title, with the (unique) path as the final tiebreak so
    /// the order is total and deterministic — and re-map paths to their new
    /// positions. Unknowns sort before known values, so they group at the
    /// front.
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
    /// What a ReplayGain analysis measured for this file, and which version of
    /// it (ADR-0015). Kept beside the metadata rather than inside
    /// [`TrackMeta`] because it is not something a scan produces: `TrackMeta`
    /// is what reading a file's tags yields, and nothing that builds one has a
    /// measurement to put in it.
    computed: ComputedReplayGain,
    /// Case-folded `artist\nalbum artist\nalbum\ntitle` (the separator keeps
    /// a query from matching across field boundaries). The album-artist
    /// slot is left empty when it would only repeat the artist, which is the
    /// overwhelming majority of tracks — so the corpus a search scans grows
    /// only for the albums that actually have a distinct album artist, and
    /// searching "RODIK" finds the album whose tile says RODIK.
    haystack: String,
    key: SortKey,
}

impl IndexedTrack {
    fn new(meta: TrackMeta, computed: ComputedReplayGain) -> Self {
        let artist = meta.artist.as_deref().map(str::to_lowercase);
        let album_artist = meta
            .album_artist
            .as_deref()
            .map(str::to_lowercase)
            .filter(|name| Some(name) != artist.as_ref());
        let album = meta.album.as_deref().map(str::to_lowercase);
        let title = meta.title.as_deref().map(str::to_lowercase);
        let mut haystack = String::new();
        for part in [&artist, &album_artist, &album, &title] {
            if let Some(text) = part {
                haystack.push_str(text);
            }
            haystack.push('\n');
        }
        let key = SortKey {
            artist: ArtistKey::of(&meta),
            album,
            disc: meta.disc,
            track: meta.track,
            title,
        };
        Self {
            meta,
            computed,
            haystack,
            key,
        }
    }
}

/// A snapshot of every measurement the library holds, as the engine's
/// [`ComputedGains`] seam consumes it (ADR-0015).
///
/// Built by [`Library::computed_gains`] and handed to
/// [`EngineHandle::set_computed_gains`](crate::engine::EngineHandle::set_computed_gains).
/// It is immutable and cheap to share: a front end that has just finished an
/// analysis pass builds a new one and replaces the old, rather than mutating a
/// map the engine is reading.
///
/// Only fresh figures are in it — [`Library::computed_gains`] applies the
/// staleness rule when it builds the map — so the lookup on the engine's side
/// is a hash and nothing else.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComputedGainMap(HashMap<PathBuf, ReplayGainTags>);

impl ComputedGainMap {
    /// How many tracks have a measurement in this snapshot.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing in the library has been measured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl ComputedGains for ComputedGainMap {
    fn computed(&self, path: &Path) -> ReplayGainTags {
        self.0.get(path).copied().unwrap_or_default()
    }
}

/// Library-order sort key: case-folded strings, unknowns first (see
/// [`SearchIndex::rebuild_order`]). Field order *is* the sort order.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SortKey {
    artist: ArtistKey,
    album: Option<String>,
    disc: Option<u32>,
    track: Option<u32>,
    title: Option<String>,
}

/// The sortable, case-folded form of [`AlbumArtist`] — the album grouping
/// key's artist half.
///
/// Variant order *is* shelf order: unknowns first (a stray with no metadata
/// should be visible at the front, not buried), then named artists
/// alphabetically, then unnamed compilations. Both anonymous buckets sit at
/// an end of the shelf rather than somewhere in the middle of the alphabet
/// where their names would have landed by accident.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum ArtistKey {
    /// Nothing known — [`AlbumArtist::Unknown`].
    Unknown,
    /// A case-folded name — [`AlbumArtist::Named`].
    Named(String),
    /// A compilation with no named album artist — [`AlbumArtist::Various`].
    Various,
}

impl ArtistKey {
    fn of(meta: &TrackMeta) -> Self {
        match AlbumArtist::of(meta) {
            AlbumArtist::Named(name) => Self::Named(name.to_lowercase()),
            AlbumArtist::Various => Self::Various,
            AlbumArtist::Unknown => Self::Unknown,
        }
    }
}

/// Run pending schema migrations, stepwise, up to [`SCHEMA_VERSION`].
///
/// Each arm migrates exactly one version and the loop re-reads
/// `user_version`, so versions chain automatically: a v3 adds a `2 => ...`
/// arm and bumps [`SCHEMA_VERSION`], nothing else.
///
/// A brand-new database walks the *whole* chain (0 → v1 → … → v5) rather
/// than being stamped with the current schema directly. That costs a few
/// statements once, and buys the guarantee that a freshly created database
/// and an upgraded one are byte-identical in shape — no class of "works on a
/// new install, breaks on an old library" bug can hide between the two
/// paths, and every release exercises its own migration code.
fn migrate(conn: &Connection) -> Result<(), IndexError> {
    loop {
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => conn.execute_batch(SCHEMA_V1)?,
            1 => migrate_v1_to_v2(conn)?,
            2 => migrate_v2_to_v3(conn)?,
            3 => migrate_v3_to_v4(conn)?,
            4 => migrate_v4_to_v5(conn)?,
            5 => migrate_v5_to_v6(conn)?,
            SCHEMA_VERSION => return Ok(()),
            found => return Err(IndexError::SchemaTooNew { found }),
        }
    }
}

/// v1 → v2: add the encoding columns and backfill what can be known without
/// touching the music files.
///
/// The columns, the backfill, and the `user_version` bump are one
/// transaction: an interrupted upgrade leaves a v1 database, which the next
/// open migrates again. SQLite's DDL is transactional, so this holds for the
/// `ALTER TABLE`s too.
fn migrate_v1_to_v2(conn: &Connection) -> Result<(), IndexError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(SCHEMA_V2_COLUMNS)?;
    backfill_formats(&tx)?;
    tx.pragma_update(None, "user_version", 2)?;
    tx.commit()?;
    Ok(())
}

/// v2 → v3: add the album-artist grouping columns.
///
/// Both columns are left `NULL` for existing rows, and deliberately so.
/// Neither can be derived from anything already in the database — an album
/// artist lives in the file's tags and nowhere else, and inventing one from
/// the stored track artist would be indistinguishable, forever after, from
/// a value the user's tagger actually wrote. The v2 backfill could read a
/// file *extension*; there is no equivalent here, and an upgrade must not
/// turn into a full library re-read at startup.
///
/// `NULL` is self-healing rather than permanent, exactly as v2's was: baz
/// rescans its music folder at every start and [`Library::add_tracks`]
/// upserts, so the first scan after the upgrade fills in every surviving
/// file's real album artist. (Since v4 that rescan is incremental — but a
/// migrated row carries no file stamp either, so it is re-read regardless;
/// see [`migrate_v3_to_v4`].) Until then, [`AlbumArtist::of`] falls through
/// to the track artist and grouping is precisely the pre-v3 behavior — the
/// upgrade cannot make the shelf worse, only later.
///
/// The `ALTER TABLE`s and the `user_version` bump are one transaction
/// (SQLite's DDL is transactional), so an interrupted upgrade leaves a v2
/// database that the next open migrates again.
fn migrate_v2_to_v3(conn: &Connection) -> Result<(), IndexError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(SCHEMA_V3_COLUMNS)?;
    tx.pragma_update(None, "user_version", 3)?;
    tx.commit()?;
    Ok(())
}

/// v3 → v4: add the file-stamp columns incremental scanning compares.
///
/// `NULL` for every existing row, and the *only* honest value. A stamp is a
/// pair of facts about a file on disk right now; filling it from anything
/// already in the database would be inventing a claim that the file is
/// unchanged — the one claim that, if wrong, makes baz show stale tags
/// forever. Stat'ing every file to fill it properly is exactly the startup
/// re-read this feature exists to remove, and would make the upgrade itself
/// the slow launch it is meant to prevent.
///
/// `NULL` is self-healing, as v2's and v3's backfill gaps were: an unstamped
/// row is always re-read (see [`crate::library::Scan`]), so the first scan
/// after the upgrade is a full one and stamps everything it touches. From
/// the second launch on, scanning is incremental. The upgrade therefore
/// costs one ordinary scan and no correctness at all.
///
/// The `ALTER TABLE`s and the `user_version` bump are one transaction
/// (SQLite's DDL is transactional), so an interrupted upgrade leaves a v3
/// database that the next open migrates again.
fn migrate_v3_to_v4(conn: &Connection) -> Result<(), IndexError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(SCHEMA_V4_COLUMNS)?;
    tx.pragma_update(None, "user_version", 4)?;
    tx.commit()?;
    Ok(())
}

/// v4 → v5: add the ReplayGain columns (ADR-0013).
///
/// `NULL` for every existing row, and the only honest value. A ReplayGain
/// figure lives in the file's tags and nowhere else — nothing already in the
/// database implies one, and *computing* one means an EBU R128 analysis pass
/// over every track, which is [`crate::analysis`]'s work (ADR-0015) and could
/// certainly not happen inside a migration. The v2 backfill had a file
/// extension to read; there is no equivalent here.
///
/// `NULL` is self-healing rather than permanent, exactly as v2's, v3's and
/// v4's gaps were: baz rescans its music folder at every start and
/// [`Library::add_tracks`] upserts, so the first scan after the upgrade fills
/// in every surviving file's real ReplayGain. That scan is incremental (v4), so
/// an unchanged file is *not* re-read and keeps its NULLs — which is correct
/// and not a bug: an unchanged file is one whose tags have not moved, and a
/// listener who runs a ReplayGain scanner over their library changes the files,
/// which moves their stamps, which is what makes baz re-read them. Until then
/// [`ReplayGainSource::NoTag`](crate::protocol::ReplayGainSource::NoTag) is the
/// honest reading and the no-ReplayGain pre-amp (zero by default) is what
/// applies, so the upgrade cannot change what anything sounds like.
///
/// The `ALTER TABLE`s and the `user_version` bump are one transaction
/// (SQLite's DDL is transactional), so an interrupted upgrade leaves a v4
/// database that the next open migrates again.
fn migrate_v4_to_v5(conn: &Connection) -> Result<(), IndexError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(SCHEMA_V5_COLUMNS)?;
    tx.pragma_update(None, "user_version", 5)?;
    tx.commit()?;
    Ok(())
}

/// v5 → v6: add the columns a ReplayGain **analysis** writes (ADR-0015).
///
/// `NULL` for every existing row, and the only honest value — for a stronger
/// reason than v5's. A computed loudness is not a fact that could be derived
/// from anything in the database at all: it is the output of decoding every
/// sample of every file, which is minutes to hours of work and is exactly what
/// the background pass exists to do somewhere other than inside a migration.
/// v2's backfill had a file extension to read; there is not even a tag to read
/// here.
///
/// `NULL` is self-healing on the same terms as v2 – v5, with a different
/// healer: the first
/// [`AnalysisCommand::StartReplayGainAnalysis`](crate::protocol::AnalysisCommand::StartReplayGainAnalysis)
/// a listener sends fills it, and until they send one nothing changes — an
/// upgraded library sounds exactly as it did, because
/// [`ReplayGainSource::NoTag`](crate::protocol::ReplayGainSource::NoTag) and
/// the no-ReplayGain pre-amp (zero by default) are what apply to a file with
/// no figure of either kind.
///
/// The `ALTER TABLE`s and the `user_version` bump are one transaction (SQLite's
/// DDL is transactional), so an interrupted upgrade leaves a v5 database that
/// the next open migrates again.
fn migrate_v5_to_v6(conn: &Connection) -> Result<(), IndexError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(SCHEMA_V6_COLUMNS)?;
    tx.pragma_update(None, "user_version", 6)?;
    tx.commit()?;
    Ok(())
}

/// Fill `format` for existing rows from the file extension, where the
/// extension settles the question by itself.
///
/// This is the honest half of a v1 upgrade. `bit_depth`, `sample_rate` and
/// `bitrate` stay NULL, and ambiguous *containers* keep a NULL `format`:
/// `.m4a`/`.mp4` may hold ALAC or AAC and `.ogg` may hold Vorbis, Opus or
/// FLAC, and only reading the file answers that. Nothing here reads a file —
/// an upgrade must not turn into a full library re-read at startup.
///
/// NULL is self-healing rather than permanent: baz rescans its music folder
/// at every start, and [`Library::add_tracks`] upserts, so each surviving
/// file gets its true codec and properties within the first scan after the
/// upgrade. (Since v4 that rescan is incremental — but a migrated row has no
/// file stamp either, so it is re-read regardless; see
/// [`migrate_v3_to_v4`].) Until then an unbackfilled album simply shows one
/// unnamed edition — exactly the pre-editions behavior.
fn backfill_formats(conn: &Connection) -> Result<(), IndexError> {
    let mut updates: Vec<(i64, &'static str)> = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT id, path FROM tracks")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            if let Some(format) = format_from_extension(&path_from_blob(blob)?) {
                updates.push((id, format.code()));
            }
        }
    }
    let mut stmt = conn.prepare("UPDATE tracks SET format = ?2 WHERE id = ?1")?;
    for (id, code) in updates {
        stmt.execute(params![id, code])?;
    }
    Ok(())
}

/// The codec a file *extension* pins down on its own, for the v1 → v2
/// backfill only. Container extensions that can hold several codecs are
/// deliberately absent — see [`backfill_formats`].
fn format_from_extension(path: &Path) -> Option<AudioFormat> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "flac" => Some(AudioFormat::Flac),
        "mp3" => Some(AudioFormat::Mp3),
        "wav" => Some(AudioFormat::Wav),
        "opus" => Some(AudioFormat::Opus),
        _ => None,
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
    let format: Option<String> = row.get(8)?;
    Ok(TrackMeta {
        path: path_from_blob(path_blob)?,
        artist: row.get(1)?,
        album: row.get(2)?,
        title: row.get(3)?,
        track: row.get(4)?,
        disc: row.get(5)?,
        year: row.get(6)?,
        duration,
        // An unreadable code degrades to "unknown format" rather than
        // failing the open (see `AudioFormat::from_code`).
        format: format.as_deref().and_then(AudioFormat::from_code),
        bit_depth: row.get(9)?,
        sample_rate: row.get(10)?,
        bitrate: row.get(11)?,
        album_artist: row.get(12)?,
        compilation: row.get(13)?,
        stamp: row_to_stamp(row)?,
        replay_gain: row_to_replay_gain(row)?,
    })
}

/// The [`ReplayGainTags`] a row carries (schema v5), `None` per field for a
/// pre-v5 row or a file that declared nothing.
///
/// A stored value outside the range its unit can hold degrades to `None` — the
/// same "this file did not say" a missing column gives — rather than failing
/// the open. Nothing baz writes can land outside the range; a value that has
/// is a corrupt database, and refusing to open a listener's whole library over
/// one bad integer would be the wrong trade (`AudioFormat::from_code` makes the
/// same call for the same reason).
fn row_to_replay_gain(row: &rusqlite::Row<'_>) -> Result<ReplayGainTags, IndexError> {
    figures_at(row, 16)
}

/// The [`ComputedReplayGain`] a row carries (schema v6) — the figures baz
/// measured and the stamp of the file it measured them from.
///
/// Out-of-range values degrade to `None` on exactly the terms
/// [`row_to_replay_gain`] states. A measurement with only half a stamp is a
/// measurement with no stamp, for [`row_to_stamp`]'s reason: a comparison needs
/// both, and an incomplete pair reported as a partial match is a stale figure
/// nobody would catch.
fn row_to_computed(row: &rusqlite::Row<'_>) -> Result<ComputedReplayGain, IndexError> {
    let mtime_ns: Option<i64> = row.get(24)?;
    let size: Option<i64> = row.get(25)?;
    let stamp = match (mtime_ns, size) {
        (Some(mtime_ns), Some(size)) => u64::try_from(size)
            .ok()
            .map(|size| FileStamp { mtime_ns, size }),
        _ => None,
    };
    Ok(ComputedReplayGain {
        figures: figures_at(row, 20)?,
        stamp,
    })
}

/// The four ReplayGain figures starting at column `first`, in the
/// gain/peak/gain/peak order both column groups use.
///
/// A stored value outside the range its unit can hold degrades to `None` — the
/// same "this file did not say" a missing column gives — rather than failing
/// the open. Nothing baz writes can land outside the range; a value that has is
/// a corrupt database, and refusing to open a listener's whole library over one
/// bad integer would be the wrong trade (`AudioFormat::from_code` makes the
/// same call for the same reason).
fn figures_at(row: &rusqlite::Row<'_>, first: usize) -> Result<ReplayGainTags, IndexError> {
    let gain = |column: usize| -> Result<Option<i16>, IndexError> {
        Ok(row
            .get::<_, Option<i64>>(column)?
            .and_then(|v| i16::try_from(v).ok()))
    };
    let peak = |column: usize| -> Result<Option<u32>, IndexError> {
        Ok(row
            .get::<_, Option<i64>>(column)?
            .and_then(|v| u32::try_from(v).ok()))
    };
    Ok(ReplayGainTags {
        track_gain_centidb: gain(first)?,
        track_peak_micro: peak(first + 1)?,
        album_gain_centidb: gain(first + 2)?,
        album_peak_micro: peak(first + 3)?,
    })
}

/// The [`FileStamp`] a row carries, or `None` when either half is missing —
/// a pre-v4 row, or a file whose filesystem declined to timestamp it. Half a
/// stamp is not a stamp: a comparison needs both, so an incomplete pair is
/// reported as no pair rather than as a partial match nobody can use.
fn row_to_stamp(row: &rusqlite::Row<'_>) -> Result<Option<FileStamp>, IndexError> {
    let mtime_ns: Option<i64> = row.get(14)?;
    let size: Option<i64> = row.get(15)?;
    let (Some(mtime_ns), Some(size)) = (mtime_ns, size) else {
        return Ok(None);
    };
    // A negative size is not something this code writes; treat it as the
    // corrupt value it is and fall back to "unstamped", i.e. always re-read.
    Ok(u64::try_from(size)
        .ok()
        .map(|size| FileStamp { mtime_ns, size }))
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

    /// A track with nothing but a path, for building fixtures from.
    fn bare_meta() -> TrackMeta {
        TrackMeta {
            path: PathBuf::from("/m/a.flac"),
            artist: None,
            album_artist: None,
            compilation: None,
            album: None,
            title: None,
            track: None,
            disc: None,
            year: None,
            duration: None,
            format: None,
            bit_depth: None,
            sample_rate: None,
            bitrate: None,
            stamp: None,
            replay_gain: ReplayGainTags::default(),
        }
    }

    #[test]
    fn haystack_separates_fields_and_folds_case() {
        let track = IndexedTrack::new(
            TrackMeta {
                artist: Some("Größenwahn".to_owned()),
                album: Some("LIVE".to_owned()),
                ..bare_meta()
            },
            ComputedReplayGain::default(),
        );
        assert_eq!(track.haystack, "größenwahn\n\nlive\n\n");
        // The separator keeps queries from matching across field boundaries.
        assert!(!track.haystack.contains("wahnlive"));
    }

    #[test]
    fn haystack_carries_a_distinct_album_artist_but_never_repeats_the_artist() {
        // A soundtrack: the album is filed under a name no track artist has.
        let soundtrack = IndexedTrack::new(
            TrackMeta {
                artist: Some("Kouhei Okamura".to_owned()),
                album_artist: Some("RODIK".to_owned()),
                album: Some("Cookie's Bustle OST (gamerip)".to_owned()),
                ..bare_meta()
            },
            ComputedReplayGain::default(),
        );
        assert!(
            soundtrack.haystack.contains("rodik"),
            "searching the name on the tile must find the album"
        );

        // The ordinary album: album artist == artist, so the slot stays
        // empty rather than doubling every record in the corpus.
        let ordinary = IndexedTrack::new(
            TrackMeta {
                artist: Some("Stan Rogers".to_owned()),
                album_artist: Some("STAN ROGERS".to_owned()),
                album: Some("Northwest Passage".to_owned()),
                ..bare_meta()
            },
            ComputedReplayGain::default(),
        );
        assert_eq!(ordinary.haystack, "stan rogers\n\nnorthwest passage\n\n");
    }

    #[test]
    fn album_artist_resolution_follows_the_documented_chain() {
        let tagged = TrackMeta {
            artist: Some("Kouhei Okamura".to_owned()),
            album_artist: Some("RODIK".to_owned()),
            compilation: Some(true),
            ..bare_meta()
        };
        assert_eq!(
            AlbumArtist::of(&tagged),
            AlbumArtist::Named("RODIK"),
            "a named album artist outranks every other signal"
        );

        let flagged = TrackMeta {
            artist: Some("Kouhei Okamura".to_owned()),
            compilation: Some(true),
            ..bare_meta()
        };
        assert_eq!(AlbumArtist::of(&flagged), AlbumArtist::Various);
        assert_eq!(AlbumArtist::of(&flagged).name(), None);

        let ordinary = TrackMeta {
            artist: Some("Stan Rogers".to_owned()),
            compilation: Some(false),
            ..bare_meta()
        };
        assert_eq!(
            AlbumArtist::of(&ordinary),
            AlbumArtist::Named("Stan Rogers")
        );

        assert_eq!(AlbumArtist::of(&bare_meta()), AlbumArtist::Unknown);
        assert_eq!(AlbumArtist::of(&bare_meta()).name(), None);

        // A tag that literally reads "Various Artists" is a *name* the user's
        // tagger wrote, not baz's compilation bucket. The owner's library has
        // one; the two must stay distinguishable.
        let literal = TrackMeta {
            album_artist: Some("Various Artists".to_owned()),
            ..bare_meta()
        };
        assert_eq!(
            AlbumArtist::of(&literal),
            AlbumArtist::Named("Various Artists")
        );
        assert_ne!(AlbumArtist::of(&literal), AlbumArtist::Various);
    }

    #[test]
    fn artist_keys_put_both_anonymous_buckets_at_the_ends_of_the_shelf() {
        let mut keys = [
            ArtistKey::Various,
            ArtistKey::Named("zeta".to_owned()),
            ArtistKey::Unknown,
            ArtistKey::Named("alpha".to_owned()),
        ];
        keys.sort();
        assert!(matches!(keys[0], ArtistKey::Unknown));
        assert!(matches!(&keys[1], ArtistKey::Named(n) if n == "alpha"));
        assert!(matches!(&keys[2], ArtistKey::Named(n) if n == "zeta"));
        assert!(matches!(keys[3], ArtistKey::Various));
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
            duration: Some(Duration::new(215, 123_456_789)),
            ..bare_meta()
        };
        let nanos = duration_to_nanos(&meta).expect("convert").expect("some");
        assert_eq!(nanos, 215_123_456_789);
    }

    #[test]
    fn backfill_only_trusts_unambiguous_extensions() {
        use std::path::Path;
        assert_eq!(
            format_from_extension(Path::new("/m/a/01.FLAC")),
            Some(AudioFormat::Flac)
        );
        assert_eq!(
            format_from_extension(Path::new("/m/a/01.mp3")),
            Some(AudioFormat::Mp3)
        );
        assert_eq!(
            format_from_extension(Path::new("/m/a/01.wav")),
            Some(AudioFormat::Wav)
        );
        assert_eq!(
            format_from_extension(Path::new("/m/a/01.opus")),
            Some(AudioFormat::Opus)
        );
        // Containers whose codec only the file itself knows stay unknown, so
        // the rescan decides rather than the backfill guessing.
        for ambiguous in ["/m/a/01.m4a", "/m/a/01.mp4", "/m/a/01.ogg", "/m/a/01"] {
            assert_eq!(
                format_from_extension(Path::new(ambiguous)),
                None,
                "{ambiguous} must not be guessed at"
            );
        }
    }

    #[test]
    fn uniform_reports_only_a_value_every_track_agrees_on() {
        assert_eq!(uniform([Some(16), Some(16)].into_iter()), Some(16));
        assert_eq!(uniform([Some(16), Some(24)].into_iter()), None);
        assert_eq!(uniform([Some(16), None].into_iter()), None);
        assert_eq!(uniform([None, Some(16)].into_iter()), None);
        assert_eq!(uniform(std::iter::empty::<Option<u8>>()), None);
    }
}
