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
//! findable. An **empty query returns nothing**: every haystack contains the
//! empty string, so the only honest answer would be the entire library
//! truncated at `limit`, which would misrepresent a 100k-track library as
//! `limit` tracks. "No query yet" is the shelf's state
//! ([`Library::albums`]), not a search result.
//!
//! Results come back **ranked, best match first**
//! (`docs/adr/0021-search-ranking.md`) rather than in library order, because
//! the design's type-anywhere find plays the first match on `Enter`
//! (`docs/design/critique/02-surfaces.md`) — so what ranks first is what
//! plays. The ranking is three signals compared in order: how well the query
//! fits the field it matched, which field that was, and library order as the
//! final tiebreak. It is a *total* order, so the same query over the same
//! library always returns the same list. [`Library::search_albums`] is the
//! same ranking projected onto albums, which is the unit the wall draws.
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
//!
//! # Group keys
//!
//! [`Library::shelves`] arranges those albums into the shelves the wall draws,
//! under one [`GroupKey`] — ARTIST, YEAR, GENRE, ADDED or PLAYED
//! (`docs/adr/0019-group-keys.md`, which amends ADR-0008). Each key is a
//! *projection* of the same albums and never a filter: every album appears
//! under every key, once, including the albums whose files declare nothing.
//! Each [`Shelf`] carries the [`GroupHeader`] it draws, which is also the
//! whole of what the index rail shows — so the rail is
//! `shelves(key).iter().map(|s| s.header.label())` and never needs
//! re-specifying when a key is added.
//!
//! Two of the keys read facts the schema had to grow (v7): `genre`, verbatim
//! from the tags, and `first_seen_ns`, written once when a row is created and
//! structurally unreachable by any later rescan. PLAYED reads the play-history
//! ledger (`docs/adr/0018-play-history-ledger.md`) through
//! [`Library::shelves_with_history`], and answers `NEVER PLAYED` for the whole
//! library when there is no ledger — which is the true answer, not a
//! placeholder. ADDED and PLAYED share the ledger's [`Recency`] vocabulary
//! rather than defining a second one.

use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

use crate::history::{History, Recency, bucket};
use crate::library::{AudioFormat, FileStamp, KnownFile, KnownFiles, TrackMeta};
use crate::replaygain::{ComputedGains, ComputedReplayGain, ReplayGainTags};

/// The schema version this build reads and writes (`PRAGMA user_version`).
const SCHEMA_VERSION: i64 = 8;

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

/// Version 7: the two facts the GENRE and ADDED group keys are made of
/// (`docs/adr/0018-group-keys.md`). Two more nullable columns, added rather
/// than rebuilt, exactly as v2 – v6 were.
///
/// # Why `first_seen_ns` is not just another scanned column
///
/// It is the one column in `tracks` that a rescan must **never** rewrite.
/// "When did this album arrive in my collection" is destroyed the moment a
/// second scan touches it, and a scan runs at every launch — so the guarantee
/// is made by the shape of [`UPSERT_TRACK`], which names `first_seen_ns` in
/// its `INSERT` list and omits it from its `ON CONFLICT DO UPDATE` list. A
/// row's first-seen is written once, when the row is created, and no later
/// code path can move it. That is the same structural trick the `rg_computed_*`
/// columns use (schema v6), for the same reason: a property held by the schema
/// beats a property held by two writers agreeing to be careful.
///
/// It is nanoseconds since the Unix epoch, matching `mtime_ns` — the only
/// other timestamp in the table — so the two are comparable without a
/// conversion nobody would remember to write.
///
/// The transaction and the `user_version` bump are applied by
/// [`migrate_v6_to_v7`].
const SCHEMA_V7_COLUMNS: &str = "
    ALTER TABLE tracks ADD COLUMN genre         TEXT;    -- verbatim from tags
    ALTER TABLE tracks ADD COLUMN first_seen_ns INTEGER; -- ns since the epoch
";

/// Version 8: **which library root each row came from**, and the roots
/// themselves (`docs/adr/0022-library-roots-and-refresh.md`).
///
/// One nullable column added rather than rebuilt, exactly as v2 – v7 were, plus
/// the first table `tracks` has ever had a companion.
///
/// # Why the column exists
///
/// It replaces a *guess* with a *record*. ADR-0010's removal policy protects a
/// multi-root index with four gates, and the second of them was "the path is
/// under the root just scanned" — a `starts_with` on the path. That test is
/// correct only while roots cannot nest and no file is reachable from two of
/// them, which is precisely the assumption supporting several music folders
/// destroys: `~/Music` and `~/Music/Live` both claim the same file, and so do
/// a folder and a symlink into it. The column answers the question the prefix
/// was approximating — *which root's walk actually read this file* — so a
/// scan of one root can only ever nominate rows that root itself put there.
///
/// `NULL` means "no root recorded", and it is a **safe** value rather than a
/// gap: no root's scan may prune a row that belongs to none. Every pre-v8 row
/// starts there and is adopted by [`Library::adopt_root`] at the next launch,
/// which is a backfill that is *knowable* — a pre-v8 baz held exactly one
/// music folder, so every row it wrote came from that folder — unlike the
/// three ADR-0019 refused for `first_seen_ns`, each of which would have had to
/// invent a fact no one recorded.
///
/// The blob is the same platform-native path encoding as `tracks.path` (module
/// docs), for the same reason: a root is a path, and paths are not UTF-8.
///
/// # Why there is also a table
///
/// `roots` records what baz *knows about* a root — when a scan of it last
/// completed — as distinct from which roots the listener has chosen, which is
/// `config.toml`'s business and stays there. Two facts, two homes, and the
/// index never has an opinion about which folders a listener wants.
///
/// `last_scan_ns` is nanoseconds since the Unix epoch, matching `mtime_ns` and
/// `first_seen_ns` — the table's only timestamp vocabulary — so nothing needs a
/// conversion nobody would remember to write.
///
/// The `ALTER TABLE`, the `CREATE TABLE` and the `user_version` bump are
/// applied by [`migrate_v7_to_v8`].
const SCHEMA_V8: &str = "
    ALTER TABLE tracks ADD COLUMN root BLOB; -- OS-native bytes; see module docs
    CREATE TABLE roots (
        path         BLOB PRIMARY KEY,  -- OS-native bytes; see module docs
        last_scan_ns INTEGER            -- ns since the epoch; NULL = never finished
    ) STRICT;
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
///
/// **`first_seen_ns` is in the insert list and absent from the update list**
/// (schema v7), which is the whole of how ADDED survives a rescan: the value
/// is written when the row is created and there is no statement anywhere that
/// can move it afterwards. See [`SCHEMA_V7_COLUMNS`].
///
/// **`root` is in both lists** (schema v8), which is the opposite decision and
/// the right one for the opposite reason: where a first-seen is a fact about
/// the past that a rescan must not disturb, a root is a fact about *now* —
/// which folder baz is currently finding this file under. A listener who
/// removes one folder and adds another containing the same tree has re-homed
/// those tracks, and the row should say so the moment a walk reads it again.
/// The update is `COALESCE`d so that a caller which names **no** root (a bare
/// [`Library::add_tracks`]) leaves whatever root a scan recorded alone: saying
/// nothing about a row's root must not be the same as clearing it.
const UPSERT_TRACK: &str = "
    INSERT INTO tracks
        (path, artist, album, title, track, disc, year, duration_ns,
         format, bit_depth, sample_rate, bitrate, album_artist, compilation,
         mtime_ns, file_size,
         rg_track_gain_centidb, rg_track_peak_micro,
         rg_album_gain_centidb, rg_album_peak_micro,
         genre, first_seen_ns, root)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
            ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)
    ON CONFLICT(path) DO UPDATE SET
        root = COALESCE(excluded.root, tracks.root),
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
        rg_album_peak_micro = excluded.rg_album_peak_micro,
        genre = excluded.genre
";

const SELECT_ALL_TRACKS: &str = "
    SELECT path, artist, album, title, track, disc, year, duration_ns,
           format, bit_depth, sample_rate, bitrate, album_artist, compilation,
           mtime_ns, file_size,
           rg_track_gain_centidb, rg_track_peak_micro,
           rg_album_gain_centidb, rg_album_peak_micro,
           rg_computed_track_gain_centidb, rg_computed_track_peak_micro,
           rg_computed_album_gain_centidb, rg_computed_album_peak_micro,
           rg_computed_mtime_ns, rg_computed_file_size,
           genre, first_seen_ns, root
    FROM tracks
";

/// Delete one row by path. The `path` column is `UNIQUE`, so this removes at
/// most one row and reports whether it did.
const DELETE_TRACK: &str = "DELETE FROM tracks WHERE path = ?1";

/// Claim one unrooted row for a root (schema v8's backfill, one path at a
/// time). `root IS NULL` in the `WHERE` clause is what makes it unable to
/// re-home a row that already names a root — adoption fills gaps and never
/// overrules a scan.
const ADOPT_TRACK: &str = "UPDATE tracks SET root = ?2 WHERE path = ?1 AND root IS NULL";

/// Every root the index has a record for, with the moment a scan of it last
/// finished.
const SELECT_ROOTS: &str = "SELECT path, last_scan_ns FROM roots";

/// Record that a scan of a root finished at a moment.
const RECORD_ROOT_SCAN: &str = "
    INSERT INTO roots (path, last_scan_ns) VALUES (?1, ?2)
    ON CONFLICT(path) DO UPDATE SET last_scan_ns = excluded.last_scan_ns
";

/// Forget a root: its `roots` row.
const DELETE_ROOT: &str = "DELETE FROM roots WHERE path = ?1";

/// Forget a root: every track row recorded under it.
///
/// Keyed on the recorded `root` column and **not** on a path prefix, for the
/// reason [`SCHEMA_V8`] gives: a prefix would take rows out of a nested root
/// the listener did not remove.
const DELETE_TRACKS_UNDER_ROOT: &str = "DELETE FROM tracks WHERE root = ?1";

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
    /// The `roots` table, in RAM: root → when a scan of it last finished
    /// (schema v8). A library has a handful of roots, so it is hydrated whole
    /// at open and kept in step by [`Library::record_scan`] and
    /// [`Library::forget_root`] — the Settings surface asks for it once per
    /// frame, and a per-frame query for four rows would be four queries too
    /// many.
    roots: HashMap<PathBuf, Option<i64>>,
}

impl Library {
    /// How many matching tracks [`Library::search`] ranks: the first this many
    /// in library order, not every match in the library.
    ///
    /// Ranking has to see a match before it can call it first, so it cannot
    /// stop early the way the old unranked filter could. Over a 100k-track
    /// library that is measured, not guessed: ranking every match of a
    /// one-character query — which is what the *first* keystroke of a
    /// type-anywhere find always is — cost **11.6 ms**, most of a 60 Hz frame,
    /// against a friction budget of "keystroke → filtered wall = next frame"
    /// (`docs/design/critique/02-surfaces.md`). Capping the candidate set puts
    /// the worst case back under half a millisecond
    /// (`docs/adr/0021-search-ranking.md` records both).
    ///
    /// **What it costs, stated plainly**: for a query matching more than this
    /// many tracks, an excellent match beyond the cap is not seen. Three things
    /// make that the right trade rather than a reintroduction of the bug
    /// ranking exists to fix:
    ///
    /// - It is **eight to eighty times** the working set the old code ranked
    ///   nothing within (it took the first `limit` matches in corpus order and
    ///   called the first one the answer), and far more than any wall shows.
    /// - It binds only on queries that have **not narrowed anything yet**. A
    ///   query matching 4 % of a library is one or two characters in; the match
    ///   set shrinks roughly geometrically per keystroke, and by the time a
    ///   query is specific enough for `Enter` to mean something, every match
    ///   fits inside the cap and the ranking is exact.
    /// - It is **deterministic**: a prefix of library order is a stable,
    ///   reproducible set, so the same query still always returns the same
    ///   list.
    pub const RANKED_CANDIDATES: usize = 4096;

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
            roots: HashMap::new(),
        };
        library.hydrate()?;
        Ok(library)
    }

    /// Load every stored track into the in-RAM index, replacing its contents.
    fn hydrate(&mut self) -> Result<(), IndexError> {
        self.index = SearchIndex::default();
        // One `Arc<Path>` per distinct root for the whole library, not one per
        // row: a hundred thousand tracks come from a handful of folders.
        let mut roots: HashMap<PathBuf, Arc<Path>> = HashMap::new();
        {
            let mut stmt = self.conn.prepare(SELECT_ALL_TRACKS)?;
            let rows = stmt.query_and_then([], |row| {
                Ok::<_, IndexError>((
                    row_to_meta(row)?,
                    row_to_computed(row)?,
                    row_to_first_seen(row)?,
                    row_to_root(row)?,
                ))
            })?;
            for row in rows {
                let (meta, computed, first_seen, root) = row?;
                let root = root.map(|root| Arc::clone(shared_root(&mut roots, &root)));
                self.index.put(meta, computed, first_seen, root);
            }
        }
        self.index.rebuild_order();
        self.hydrate_roots()?;
        Ok(())
    }

    /// Load the `roots` table into RAM, replacing its contents.
    fn hydrate_roots(&mut self) -> Result<(), IndexError> {
        self.roots.clear();
        let mut stmt = self.conn.prepare(SELECT_ROOTS)?;
        let rows = stmt.query_and_then([], |row| {
            let path: Vec<u8> = row.get(0)?;
            Ok::<_, IndexError>((path_from_blob(path)?, row.get::<_, Option<i64>>(1)?))
        })?;
        for row in rows {
            let (path, last_scan_ns) = row?;
            self.roots.insert(path, last_scan_ns);
        }
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
        self.add_tracks_under(None, tracks)
    }

    /// [`Library::add_tracks`], recording which **library root** the batch was
    /// found under (schema v8).
    ///
    /// The root belongs to the batch rather than to each [`TrackMeta`], and
    /// that is not a shortcut: a scan walks one root at a time, so every entry
    /// it emits came from the root it is currently walking, and a per-track
    /// field would be the same path repeated a hundred thousand times for a
    /// fact the caller already knows once. It is also the field a scan is
    /// *entitled* to set — `TrackMeta` is what reading a file's tags yields,
    /// and no file carries the name of the folder somebody pointed baz at.
    ///
    /// `None` records no root, which is what [`Library::add_tracks`] does and
    /// what every pre-v8 row holds. Such a row is unprunable by any root's scan
    /// (see [`KnownFile`]), which is the safe direction.
    ///
    /// # Errors
    ///
    /// Exactly [`Library::add_tracks`]'s.
    pub fn add_tracks_under<I>(
        &mut self,
        root: Option<&Path>,
        tracks: I,
    ) -> Result<usize, IndexError>
    where
        I: IntoIterator<Item = TrackMeta>,
    {
        let root: Option<Arc<Path>> = root.map(Arc::from);
        let mut iter = tracks.into_iter().peekable();
        let mut added = 0;
        // One clock reading for the whole call, not one per row: the tracks in
        // a scan batch arrived together, and giving them timestamps that differ
        // by the microseconds an insert takes would be precision the fact does
        // not have. Only rows the database has never held will use it (see
        // [`UPSERT_TRACK`]).
        let now_ns = now_ns();
        let result = self.insert_batches(&mut iter, &mut added, now_ns, root.as_ref());
        // Re-sort exactly once whether or not a batch failed, so the index
        // order always matches what actually landed.
        self.index.rebuild_order();
        result.map(|()| added)
    }

    fn insert_batches<I>(
        &mut self,
        iter: &mut Peekable<I>,
        added: &mut usize,
        now_ns: i64,
        root: Option<&Arc<Path>>,
    ) -> Result<(), IndexError>
    where
        I: Iterator<Item = TrackMeta>,
    {
        let root_blob = root.map(|root| path_to_blob(root));
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
                        meta.genre,
                        now_ns,
                        root_blob,
                    ])?;
                }
            }
            tx.commit()?;
            // Mirror into RAM only after the batch is durably committed, so
            // a failed batch never leaves ghost tracks in the index.
            *added += chunk.len();
            for meta in chunk {
                self.index.insert(meta, now_ns, root.map(Arc::clone));
            }
        }
        Ok(())
    }

    /// Claim every rootless row under `root` for it — schema v8's backfill,
    /// run by the front end at launch for each folder it is configured to hold
    /// (`docs/adr/0022-library-roots-and-refresh.md`).
    ///
    /// Returns how many rows were adopted.
    ///
    /// # Why this backfill is honest where ADR-0019's three were not
    ///
    /// It states a fact somebody recorded, not one nobody did. A pre-v8 baz
    /// held exactly **one** music folder and scanned exactly that folder, so
    /// every row in a pre-v8 index came from it — the config file still says
    /// which folder, and the row's own path still says whether it is under it.
    /// Both halves are checked here: a row is claimed only if it names no root
    /// *and* lies under this one. The alternative candidates ADR-0019 rejected
    /// for `first_seen_ns` all had to invent an unrecorded fact; this one reads
    /// two recorded ones.
    ///
    /// A row under **no** configured root stays unrooted and is therefore
    /// permanently unprunable by any scan — the honest answer for a file baz
    /// was pointed at once and is not pointed at now, and the state a listener
    /// clears by adding that folder back or by leaving it forgotten.
    ///
    /// Nested roots resolve by whoever asks first: adoption never overrules an
    /// existing root, so a caller adopting in configuration order gives a file
    /// under both `~/Music` and `~/Music/Live` to whichever the listener listed
    /// first. The next full read of that file re-homes it to the root that read
    /// it, which is the same rule every other row follows.
    ///
    /// # Errors
    ///
    /// [`IndexError::Sqlite`] if the write fails; the in-RAM index is then left
    /// matching whatever the database holds.
    pub fn adopt_root(&mut self, root: &Path) -> Result<usize, IndexError> {
        let orphans: Vec<usize> = self
            .index
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.root.is_none() && track.meta.path.starts_with(root))
            .map(|(index, _)| index)
            .collect();
        if orphans.is_empty() {
            return Ok(0);
        }
        let shared: Arc<Path> = Arc::from(root);
        let root_blob = path_to_blob(root);
        let mut adopted = 0;
        {
            let tx = self.conn.transaction()?;
            {
                let mut stmt = tx.prepare_cached(ADOPT_TRACK)?;
                for &index in &orphans {
                    if let Some(track) = self.index.tracks.get(index) {
                        adopted +=
                            stmt.execute(params![path_to_blob(&track.meta.path), root_blob])?;
                    }
                }
            }
            tx.commit()?;
        }
        // Mirror into RAM only after the batch is durably committed, exactly as
        // every other writer here does. A root changes neither the sort key nor
        // the search corpus, so no re-sort is needed.
        for index in orphans {
            if let Some(track) = self.index.tracks.get_mut(index) {
                track.root = Some(Arc::clone(&shared));
            }
        }
        Ok(adopted)
    }

    /// Forget a library root: **delete every row recorded under it**, and the
    /// root's own record.
    ///
    /// Returns how many track rows went.
    ///
    /// This is what removing a folder in the Settings place does, and the
    /// choice is argued in `docs/adr/0022-library-roots-and-refresh.md`: a
    /// folder baz no longer holds is a folder baz can no longer refresh, so
    /// leaving its albums on the wall would leave a listener with rows that no
    /// scan can ever correct or remove. Nothing on disk is touched — baz has
    /// never deleted a music file and this does not start.
    ///
    /// It is keyed on the **recorded root**, never on a path prefix, so a
    /// nested root the listener kept does not lose the tracks it holds.
    ///
    /// # Errors
    ///
    /// [`IndexError::Sqlite`] if the delete fails; the in-RAM index is then
    /// left matching whatever the database holds.
    pub fn forget_root(&mut self, root: &Path) -> Result<usize, IndexError> {
        let root_blob = path_to_blob(root);
        let deleted = {
            let tx = self.conn.transaction()?;
            let deleted = tx.execute(DELETE_TRACKS_UNDER_ROOT, params![root_blob])?;
            tx.execute(DELETE_ROOT, params![root_blob])?;
            tx.commit()?;
            deleted
        };
        self.roots.remove(root);
        if deleted > 0 {
            self.index
                .tracks
                .retain(|track| track.root.as_deref() != Some(root));
            self.index.rebuild_order();
        }
        Ok(deleted)
    }

    /// Record that a scan of `root` finished at `at_ns` (nanoseconds since the
    /// Unix epoch) — the `roots` table's only writer.
    ///
    /// Called when a scan **completes**, never when one starts: the fact the
    /// Settings place reports is "baz has looked at this folder and this is
    /// when it finished", and a scan that was interrupted looked at part of it.
    ///
    /// # Errors
    ///
    /// [`IndexError::Sqlite`] if the write fails.
    pub fn record_scan(&mut self, root: &Path, at_ns: i64) -> Result<(), IndexError> {
        self.conn
            .execute(RECORD_ROOT_SCAN, params![path_to_blob(root), at_ns])?;
        self.roots.insert(root.to_path_buf(), Some(at_ns));
        Ok(())
    }

    /// What the index holds for one library root: how many tracks are recorded
    /// under it, and when a scan of it last finished.
    ///
    /// A root the index has never seen answers `RootStats::default()` — no
    /// tracks, never scanned — which is the true statement about a folder a
    /// listener added a moment ago, not a placeholder.
    #[must_use]
    pub fn root_stats(&self, root: &Path) -> RootStats {
        RootStats {
            tracks: self
                .index
                .tracks
                .iter()
                .filter(|track| track.root.as_deref() == Some(root))
                .count(),
            last_scan_ns: self.roots.get(root).copied().flatten(),
        }
    }

    /// How many rows belong to **no** recorded root: pre-v8 rows no launch has
    /// adopted, and rows added by a caller that named none.
    ///
    /// Reported rather than hidden because it is exactly the population no
    /// scan can ever prune — see [`Library::adopt_root`].
    #[must_use]
    pub fn unrooted_tracks(&self) -> usize {
        self.index
            .tracks
            .iter()
            .filter(|track| track.root.is_none())
            .count()
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

    /// Every path the library holds, with the [`FileStamp`] and the **library
    /// root** recorded for it — the input an incremental scan needs
    /// ([`crate::library::scan_incremental`]) and the only rows a scan may ever
    /// nominate for removal.
    ///
    /// A row written before schema v4, or one for a file whose filesystem
    /// could not report a usable timestamp, carries no stamp and is therefore
    /// always re-read; a row written before schema v8 that no launch has
    /// adopted carries no root and can therefore never be pruned. The map is a
    /// snapshot: it is handed to a scan worker that runs while the library
    /// keeps being written to, so it owns its paths rather than borrowing them.
    #[must_use]
    pub fn known_files(&self) -> KnownFiles {
        self.index
            .tracks
            .iter()
            .map(|track| {
                (
                    track.meta.path.clone(),
                    KnownFile::new(track.meta.stamp, track.root.clone()),
                )
            })
            .collect()
    }

    /// Search the library: literal, case-insensitive substring match over
    /// artist + album artist + album + title, **ranked best match first** and
    /// capped at `limit` results.
    ///
    /// # The ranking
    ///
    /// Three signals, compared in that order and no others
    /// (`docs/adr/0021-search-ranking.md`). There is no scoring formula and no
    /// weights: the comparison is lexicographic over three small ordered
    /// values, so any two results can be explained by naming the first signal
    /// that separates them.
    ///
    /// 1. **How well the query fits the field it landed in.** In order: the
    ///    query *is* the whole field; it starts the field and ends on a word
    ///    boundary; it starts the field mid-word; it starts a later word and
    ///    ends on a word boundary; it starts a later word mid-word; it starts
    ///    mid-word. Position dominates completeness because a listener types
    ///    the *beginning* of the name they are thinking of.
    /// 2. **Which field it landed in**: artist (track or album artist), then
    ///    album title, then track title. Only ever a tiebreak between matches
    ///    that fit equally well — which is why an exactly-matching song title
    ///    still beats an artist whose name merely contains the query.
    /// 3. **Library order** — album artist, album, disc, track, title, path
    ///    ([`Library::tracks`]). Total and stable, so the same query over the
    ///    same library always returns the same list in the same order.
    ///
    /// A track matching in several places is ranked by its **best** match.
    /// Matching tracks are kept **together by album**, an album taking the rank
    /// of its best-matching track: the wall draws albums, and a record whose
    /// tracks were scattered through the results would read as several weak
    /// hits instead of one strong one. Nothing is scored by *how many* tracks
    /// matched — that would rank a long compilation above a short record for a
    /// reason the query never asked about.
    ///
    /// # What it costs
    ///
    /// Ranking needs every match, so this cannot stop early the way an
    /// unranked filter could; a query matching a third of the library scans and
    /// scores all of it. That is measured rather than assumed —
    /// `benches/search.rs`, and ADR-0021 records the numbers.
    ///
    /// An empty `query` returns no results, deliberately (module docs), and
    /// so does a query containing `\n` — that is the field/record separator
    /// inside the search corpus, so such a query could only ever ask for a
    /// cross-field match, which search does not offer.
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<&TrackMeta> {
        if limit == 0 {
            return Vec::new();
        }
        let ranked = self.ranked(query);
        ranked
            .tracks()
            .take(limit)
            .filter_map(|index| self.index.tracks.get(index))
            .map(|track| &track.meta)
            .collect()
    }

    /// The same search and the same ranking as [`Library::search`], projected
    /// onto **albums** — the unit the wall actually draws — capped at `limit`
    /// albums.
    ///
    /// An album's rank is its best-matching track's, and it appears exactly
    /// once however many of its tracks matched.
    ///
    /// This exists so the ranking survives the mapping. A front end that calls
    /// `search(query, n)` and folds the resulting tracks onto their albums
    /// applies a *track* cap to an *album* question: an album whose only
    /// matching track fell outside the cap disappears from the wall, and which
    /// albums survive depends on how many tracks the ones before them happened
    /// to match. Here the cap is applied to the answer, not to the working set.
    ///
    /// Empty and separator-bearing queries return nothing, exactly as
    /// [`Library::search`] does.
    #[must_use]
    pub fn search_albums(&self, query: &str, limit: usize) -> Vec<Album<'_>> {
        if limit == 0 {
            return Vec::new();
        }
        let ranked = self.ranked(query);
        ranked
            .albums()
            .take(limit)
            .filter_map(|album| self.album_at(album))
            .collect()
    }

    /// Every match for `query`, ranked — the one implementation both
    /// [`Library::search`] and [`Library::search_albums`] project.
    ///
    /// One SIMD scan (`memmem`) over the whole corpus; byte-wise matching is
    /// sound because UTF-8 is self-synchronizing — a valid-UTF-8 needle can
    /// only match at character boundaries. Positions come back in ascending
    /// order, which is library order, which is why the track cursor can walk
    /// forward instead of binary-searching, why the several matches inside one
    /// track arrive consecutively, and why an album's matching tracks are
    /// already contiguous by the time they are grouped.
    fn ranked(&self, query: &str) -> RankedHits {
        let mut ranked = RankedHits::default();
        if query.is_empty() || query.contains('\n') {
            return ranked;
        }
        let needle = query.to_lowercase();
        let corpus = self.index.corpus.as_bytes();
        let mut track = 0usize;
        let mut cursor: Option<Field> = None;
        for position in memchr::memmem::find_iter(corpus, needle.as_bytes()) {
            while self
                .index
                .starts
                .get(track + 1)
                .is_some_and(|&start| start <= position)
            {
                track += 1;
            }
            let known = ranked.hits.last().is_some_and(|last| last.track == track);
            if !known {
                if ranked.hits.len() >= Self::RANKED_CANDIDATES {
                    break;
                }
                // A new track is a new haystack, so the field walk restarts.
                cursor = None;
            }
            let Some(haystack) = self.index.haystack_at(track) else {
                continue;
            };
            let Some(&start) = self.index.starts.get(track) else {
                continue;
            };
            let (relevance, field) = classify(haystack, position - start, needle.len(), cursor);
            cursor = Some(field);
            match ranked.hits.last_mut() {
                Some(last) if last.track == track => last.relevance = last.relevance.min(relevance),
                _ => ranked.hits.push(Hit { track, relevance }),
            }
        }
        ranked.group(&self.index.album_of);
        ranked
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
    ///
    /// This is the flat shelf, which is also exactly what
    /// [`GroupKey::Artist`] arranges: `albums()` and
    /// `shelves(GroupKey::Artist)` contain the same albums in the same order,
    /// and the difference is only whether the breaks between them — one per
    /// artist — are stated
    /// (`the_artist_key_is_the_flat_shelf_with_its_breaks_named`).
    #[must_use]
    pub fn albums(&self) -> Vec<Album<'_>> {
        (0..self.index.album_starts.len())
            .filter_map(|album| self.album_at(album))
            .collect()
    }

    /// Build one album from its run of tracks.
    ///
    /// Library order sorts by folded (album artist, album, ...) first, so each
    /// album is one consecutive run of `tracks`, already in in-album track
    /// order — [`SearchIndex::album_starts`] records where each run begins.
    /// That is what lets a *search* build only the albums it matched instead of
    /// building the whole shelf and filtering it.
    ///
    /// `None` for a run index the library does not have.
    fn album_at(&self, album: usize) -> Option<Album<'_>> {
        let start = *self.index.album_starts.get(album)?;
        let end = self
            .index
            .album_starts
            .get(album + 1)
            .copied()
            .unwrap_or(self.index.tracks.len());
        let tracks = self.index.tracks.get(start..end)?;
        let first = tracks.first()?;
        let mut built = Album {
            artist: AlbumArtist::of(&first.meta),
            title: first.meta.album.as_deref(),
            year: None,
            genre: None,
            first_seen_ns: None,
            editions: Vec::new(),
        };
        for track in tracks {
            if built.year.is_none() {
                built.year = track.meta.year;
            }
            if built.genre.is_none() {
                built.genre = track.meta.genre.as_deref();
            }
            built.first_seen_ns = match (built.first_seen_ns, track.first_seen) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (seen, None) | (None, seen) => seen,
            };
            built.push_track(&track.meta);
        }
        built.editions.sort_by(rank_editions);
        Some(built)
    }

    /// The shelf view **arranged into the shelves the wall draws**, under one
    /// [`GroupKey`] (ADR-0018, `docs/adr/0017-design-direction.md` step 4).
    ///
    /// Every album in the library appears in exactly one shelf, whatever the
    /// key: a value nothing declares gets a shelf of its own rather than being
    /// dropped, because a wall that silently omits your untagged records is a
    /// wall you cannot trust. [`Shelf::header`] is the group header the shelf
    /// draws and the index rail projects; it is derived from the key and the
    /// data and holds no state of its own.
    ///
    /// [`GroupKey::Played`] answers from no ledger here — every album lands in
    /// [`Recency::Never`] — because a [`Library`] holds none. Pass the play
    /// history to [`Library::shelves_with_history`] to get the real answer.
    #[must_use]
    pub fn shelves(&self, key: GroupKey) -> Vec<Shelf<'_>> {
        self.shelves_with_history(key, None)
    }

    /// [`Library::shelves`], reading [`GroupKey::Played`] from the play
    /// history (ADR-0018).
    ///
    /// `history` is consulted **only** for [`GroupKey::Played`]; every other
    /// key ignores it entirely, so a caller with no ledger loses nothing.
    ///
    /// `None` and an empty [`History`] are the same answer and both are
    /// correct rather than degraded: the ledger is optional at runtime — it
    /// writes nothing until a front end calls
    /// [`EngineHandle::set_history`](crate::engine::EngineHandle::set_history)
    /// — and "baz has no record of playing this" is a true statement about a
    /// library nobody has played. PLAYED still draws one shelf, `NEVER
    /// PLAYED`, holding everything.
    ///
    /// An album's last-played moment is the **most recent** of its tracks',
    /// across every edition: playing one track off a record is a thing you did
    /// with that record, and a listener looking for "what have I not touched in
    /// a year" means the album, not each track separately.
    #[must_use]
    pub fn shelves_with_history(&self, key: GroupKey, history: Option<&History>) -> Vec<Shelf<'_>> {
        let now = SystemTime::now();
        let mut shelves: Vec<Shelf<'_>> = Vec::new();
        let mut sorts: Vec<ShelfSort> = Vec::new();
        let mut placed: HashMap<ShelfSort, usize> = HashMap::new();
        for album in self.albums() {
            let sort = ShelfSort::of(key, &album, now, history);
            let index = match placed.entry(sort.clone()) {
                Entry::Occupied(slot) => *slot.get(),
                Entry::Vacant(slot) => {
                    slot.insert(shelves.len());
                    shelves.push(Shelf {
                        header: GroupHeader::of(key, &album, now, history),
                        albums: Vec::new(),
                    });
                    sorts.push(sort);
                    shelves.len() - 1
                }
            };
            shelves[index].albums.push(album);
        }
        // **The artist header is the spelling that sorts first**, not the
        // first one found. Identity is case-folded, so `Alpha` and `alpha` are
        // one artist with two spellings on disk and the shelf's *first* album
        // is an order — album title's — that a retag can change. Taking the
        // minimum makes the name a property of the set rather than of the
        // walk, which is the same rule, and therefore the same answer, as the
        // front end's `views::artist::label`; it also happens to prefer the
        // capitalised form a tagger meant, since upper case sorts ahead.
        for shelf in &mut shelves {
            if let GroupHeader::Artist(named @ AlbumArtist::Named(_)) = shelf.header {
                shelf.header = GroupHeader::Artist(
                    shelf
                        .albums
                        .iter()
                        .map(|album| album.artist)
                        .filter(|artist| matches!(artist, AlbumArtist::Named(_)))
                        .min_by_key(|artist| artist.name())
                        .unwrap_or(named),
                );
            }
        }
        // Sort the shelves, carrying their albums with them. `albums()` yields
        // library order, so each shelf's contents are already in it and stay
        // there — within a decade, a genre or an artist the wall reads
        // alphabetically, which is the order every other view of this library
        // uses.
        let mut order: Vec<usize> = (0..shelves.len()).collect();
        order.sort_by(|&a, &b| sorts[a].cmp(&sorts[b]));
        let mut sorted: Vec<Option<Shelf<'_>>> = shelves.into_iter().map(Some).collect();
        order
            .into_iter()
            .filter_map(|index| sorted[index].take())
            .collect()
    }
}

/// What the index holds for one library root — [`Library::root_stats`].
///
/// Two facts and no judgement: how many rows name this root, and when a scan of
/// it last finished. Whether the folder is *currently* reachable is a question
/// about the filesystem right now, which a scan answers and an index cannot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RootStats {
    /// Tracks recorded under this root.
    pub tracks: usize,
    /// When a scan of it last **finished**, in nanoseconds since the Unix
    /// epoch. `None` means no scan of it has ever completed — including for a
    /// folder added a moment ago, which is the true answer rather than a
    /// placeholder.
    pub last_scan_ns: Option<i64>,
}

/// The axis the wall's shelves break on: one row of words, no menus
/// (`docs/design/critique/02-surfaces.md`, ADR-0018).
///
/// Each key is a *projection* of the same albums, never a filter: every album
/// the library holds appears under every key, in a different arrangement with
/// different headers. That is what lets the index rail be a pure projection of
/// the active key with no state of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GroupKey {
    /// **The album artist** — the grouping ADR-0008 decided, now one key among
    /// several. One shelf per artist, headed by their name (see
    /// [`GroupHeader::Artist`]), unknowns first and unnamed compilations last.
    ///
    /// It broke records on the artist's *initial* until ADR-0035: `A`, `C`,
    /// `S`, with everyone whose name begins with an S sharing a shelf. That
    /// was a key called `Artist` that grouped by something else, which is what
    /// made its word collide with the front end's Artist **place** — the
    /// owner's finding, *"artists should be grouping stuff by artist not just
    /// alphabetically"*. Grouping by the artist makes the word true, and the
    /// alphabet survives where it was always the useful thing: the index rail
    /// (see [`Initial`]).
    Artist,
    /// Release year, shelved by decade (see [`GroupHeader::Decade`]).
    Year,
    /// Genre, **verbatim from the tags** (see [`GroupHeader::Genre`]).
    Genre,
    /// When the library first saw the album, in recency buckets (see
    /// [`Recency`]).
    Added,
    /// When the album was last played, in recency buckets ending in
    /// [`Recency::Never`].
    Played,
}

impl GroupKey {
    /// Every key, in the order the wall's row of words states them.
    pub const ALL: [Self; 5] = [
        Self::Artist,
        Self::Year,
        Self::Genre,
        Self::Added,
        Self::Played,
    ];

    /// The word the wall's group-key row shows. Typography — the design draws
    /// this row in caps — is the view's business, not this module's.
    ///
    /// **A label may be renamed; a [`code`](GroupKey::code) may not.** The word
    /// is copy on a screen and answers to the design; the code is on-disk
    /// config. [`Self::Artist`] is the case that made the difference matter:
    /// its word was briefly `A–Z`, for the release in which the key grouped by
    /// initial while wearing an artist's name, and came back to `Artist` when
    /// the key started grouping by artist (ADR-0035). Its code was `"artist"`
    /// throughout, so every `config.toml` baz has ever written still resolves
    /// — and now resolves to the arrangement its word always claimed.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Artist => "Artist",
            Self::Year => "Year",
            Self::Genre => "Genre",
            Self::Added => "Added",
            Self::Played => "Played",
        }
    }

    /// The stable lowercase code for persisting which key is active. Never
    /// change an existing code: it is on-disk data (config), and
    /// [`GroupKey::from_code`] is its only reader.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Artist => "artist",
            Self::Year => "year",
            Self::Genre => "genre",
            Self::Added => "added",
            Self::Played => "played",
        }
    }

    /// Parse a [`GroupKey::code`] back. An unrecognized code yields `None`, so
    /// a config written by a newer baz falls back to a default rather than
    /// failing a launch.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.code() == code)
    }
}

/// One shelf on the wall: a group header and the albums under it.
///
/// Produced by [`Library::shelves`]. Borrows from the library; a snapshot to
/// render, not a place to mutate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shelf<'a> {
    /// The header this shelf draws, and the value the index rail projects.
    pub header: GroupHeader<'a>,
    /// The albums under it, in library order (album artist, then album).
    /// Never empty — a shelf exists because an album landed on it.
    pub albums: Vec<Album<'a>>,
}

/// The header a [`Shelf`] draws — one value per [`GroupKey`], derived from the
/// data and holding no state of its own.
///
/// This is the whole of what the index rail shows, which is why the rail never
/// needs re-specifying when a key is added: a rail is
/// `shelves(key).iter().map(|s| s.header.label())`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupHeader<'a> {
    /// [`GroupKey::Artist`] — **the artist the shelf holds the records of**.
    ///
    /// [`AlbumArtist::Named`] carries the spelling that sorts first among the
    /// shelf's records, not the first one found: identity is case-folded, so
    /// `Alpha` and `alpha` are one artist with two spellings on disk, and a
    /// minimum is a property of the set where *first found* is a property of
    /// the walk. The two anonymous states keep the words the initial buckets
    /// used, `Unknown` and `Various`, and stay at the two ends of the wall.
    Artist(AlbumArtist<'a>),
    /// [`GroupKey::Year`] — the decade a release year falls in, as its first
    /// year (`1994` shelves under `Some(1990)`). `None` is the shelf for
    /// albums whose files declare no year.
    Decade(Option<u32>),
    /// [`GroupKey::Genre`] — the genre **exactly as the album's first track
    /// spells it**, or `None` for albums whose files declare no genre.
    ///
    /// Shelves are keyed on the case-folded spelling, so `Rock` and `rock` are
    /// one shelf — the same treatment artist and album titles have always had,
    /// and the alternative is two shelves that read identically on screen.
    /// Nothing else is touched: `Post-Rock`, `post rock` and
    /// `Rock; Instrumental` are three genres, because the files say so. There
    /// is no mapping table and there will not be one — see [`TrackMeta::genre`].
    Genre(Option<&'a str>),
    /// [`GroupKey::Added`] and [`GroupKey::Played`] — a recency bucket.
    Recency(Recency),
}

impl<'a> GroupHeader<'a> {
    /// The header's text.
    ///
    /// A [`String`] rather than a borrow because three of the four variants
    /// have no stored string to lend. Typography — the design draws headers at
    /// 9–10 px in caps — is the view's business; this is the value.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Artist(AlbumArtist::Named(name)) => (*name).to_owned(),
            Self::Artist(AlbumArtist::Various) => "Various".to_owned(),
            Self::Artist(AlbumArtist::Unknown) => "Unknown".to_owned(),
            Self::Decade(Some(decade)) => format!("{decade}s"),
            Self::Decade(None) => "No year".to_owned(),
            Self::Genre(Some(genre)) => (*genre).to_owned(),
            Self::Genre(None) => "No genre".to_owned(),
            Self::Recency(recency) => recency.label(),
        }
    }

    /// The header one album lands under, for `key`.
    fn of(key: GroupKey, album: &Album<'a>, now: SystemTime, history: Option<&History>) -> Self {
        match key {
            GroupKey::Artist => Self::Artist(album.artist),
            GroupKey::Year => Self::Decade(album.year.map(|year| year - year % 10)),
            GroupKey::Genre => Self::Genre(album.genre),
            GroupKey::Added => Self::Recency(added_recency(album.first_seen_ns, now)),
            GroupKey::Played => Self::Recency(album.played_recency(history, now)),
        }
    }
}

/// **The index rail's letter for an album artist** — the alphabet, plus the
/// two ends of it that are not letters.
///
/// It was the wall's own header until ADR-0035, when [`GroupKey::Artist`]
/// started breaking on the artist rather than on their initial. The alphabet
/// did not stop being useful when it stopped being a header: it is what the
/// index rail speaks, and a rail is the one place a coarse bucket earns its
/// keep, because you aim at a letter and land on the first artist under it.
/// So this type stayed exactly as it was and only its consumer moved.
///
/// Variant order *is* wall order, and it is the order [`Library::albums`]
/// already sorts in (see `ArtistKey`): the unknowns first, then everything
/// whose name starts with something that is not a letter, then the alphabet,
/// then unnamed compilations. Both anonymous buckets sit at an end rather than
/// in the middle of the alphabet where a sentinel string's letters would have
/// landed them by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Initial {
    /// Albums with no artist of any kind — [`AlbumArtist::Unknown`].
    Unknown,
    /// A name that does not start with a letter: `10cc`, `!!!`, `¡Forward,
    /// Russia!`. The design's `#` shelf.
    Other,
    /// The upper-cased first letter of a named album artist. Not restricted to
    /// ASCII: `Ø` and `曲` get their own shelves, because a rail that folded
    /// every script onto `#` would be unusable for the library that needs it
    /// most.
    Letter(char),
    /// A compilation the files flagged but did not name —
    /// [`AlbumArtist::Various`].
    Various,
}

impl Initial {
    /// The rail entry a resolved album artist files under.
    #[must_use]
    pub fn of(artist: AlbumArtist<'_>) -> Self {
        match artist {
            AlbumArtist::Unknown => Self::Unknown,
            AlbumArtist::Various => Self::Various,
            AlbumArtist::Named(name) => match name.chars().next() {
                // `to_uppercase` can yield several characters (`ß` → `SS`);
                // the first is the shelf, which is the one a reader looks for.
                Some(first) if first.is_alphabetic() => {
                    Self::Letter(first.to_uppercase().next().unwrap_or(first))
                }
                // An empty name cannot occur — `clean_str` drops blank tags —
                // but `#` is the right answer if one ever does.
                _ => Self::Other,
            },
        }
    }

    /// The header's text.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Unknown => "Unknown".to_owned(),
            Self::Other => "#".to_owned(),
            Self::Letter(letter) => letter.to_string(),
            Self::Various => "Various".to_owned(),
        }
    }
}

/// The moment now, in nanoseconds since the Unix epoch — what a new row's
/// `first_seen_ns` is stamped with.
///
/// A clock before 1678 or after 2262 saturates rather than panicking: the
/// consequence is a wrong recency bucket on a machine whose clock is absurd,
/// which is strictly better than refusing to store a track.
fn now_ns() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => {
            i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, i64::saturating_neg)
        }
    }
}

/// The [`GroupKey::Added`] bucket for a first-seen timestamp, as of `now`.
///
/// The buckets are the ledger's ([`crate::history::Recency`]) rather than a
/// second set of bands defined here. ADDED and PLAYED are drawn by the same
/// rail, in the same lane, and two vocabularies that had to agree would
/// eventually not.
///
/// `None` — a row from before schema v7 — is [`Recency::Unrecorded`], not
/// [`Recency::Never`]: baz has no date for those files and declines to invent
/// one (see [`migrate_v6_to_v7`]).
fn added_recency(first_seen_ns: Option<i64>, now: SystemTime) -> Recency {
    let Some(first_seen_ns) = first_seen_ns else {
        return Recency::Unrecorded;
    };
    let then_s = first_seen_ns.max(0) / NANOS_PER_SECOND;
    let now_s = i64::try_from(
        now.duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
    )
    .unwrap_or(i64::MAX);
    // A first-seen in the future — a clock corrected backwards since the scan
    // — saturates to zero elapsed, which reads as the most recent bucket.
    // That is the same rule `History::recency` follows for a play.
    bucket(u64::try_from(now_s.saturating_sub(then_s)).unwrap_or(0))
}

/// Nanoseconds in a second, for turning `first_seen_ns` into the whole
/// seconds the ledger's buckets are defined in.
const NANOS_PER_SECOND: i64 = 1_000_000_000;

/// The sort key of a shelf: what decides the order shelves appear in, kept
/// beside the [`Shelf`] rather than derived from its header so that GENRE can
/// sort case-folded while its header keeps the tag's own spelling.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ShelfSort {
    /// [`GroupKey::Artist`]: the album artist, case-folded — which is
    /// `ArtistKey`, the key [`Library::albums`] already sorts by. Reused
    /// rather than restated, because "the order the artist shelves go in" and
    /// "the order the library's albums go in" are the same sentence, and that
    /// is exactly why this key is `albums()` with its breaks named.
    Artist(ArtistKey),
    /// [`GroupKey::Year`]: `None` (no year declared) first, then ascending
    /// decades. Unknowns at the front is the rule the whole index follows.
    Decade(Option<u32>),
    /// [`GroupKey::Genre`]: `None` (no genre declared) first, then the
    /// case-folded name.
    Genre(Option<String>),
    /// [`GroupKey::Added`] / [`GroupKey::Played`]: variant order is shelf
    /// order, newest first.
    Recency(Recency),
}

impl ShelfSort {
    fn of(key: GroupKey, album: &Album<'_>, now: SystemTime, history: Option<&History>) -> Self {
        match key {
            GroupKey::Artist => Self::Artist(ArtistKey::of_album_artist(album.artist)),
            GroupKey::Year => Self::Decade(album.year.map(|year| year - year % 10)),
            GroupKey::Genre => Self::Genre(album.genre.map(str::to_lowercase)),
            GroupKey::Added => Self::Recency(added_recency(album.first_seen_ns, now)),
            GroupKey::Played => Self::Recency(album.played_recency(history, now)),
        }
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
    /// Genre: the first genre any track on the album declares, **verbatim**
    /// (see [`TrackMeta::genre`]), or `None` when no track declares one.
    ///
    /// First-declared rather than a consensus, exactly as `year` is. Tracks on
    /// one album routinely disagree about genre — a compilation genuinely
    /// spans several — and there is no answer to "which of these is the
    /// album's genre" that is more honest than "the one the record's first
    /// track claims". Refusing to answer when they differ would file most
    /// compilations under no genre at all, which is the worse failure: the
    /// GENRE key exists to show a listener what their tags actually say.
    pub genre: Option<&'a str>,
    /// When the library first saw this album, in nanoseconds since the Unix
    /// epoch — the **earliest** first-seen among its tracks.
    ///
    /// Earliest rather than latest because the question ADDED asks is when the
    /// record arrived, and a rip whose second disc landed a year after its
    /// first is a record you have had for a year. The alternative — dating an
    /// album by its newest track — would resurface a twenty-year-old album at
    /// the top of the wall because one file was re-ripped, which is the
    /// behaviour ADDED exists to *provide*, not to be fooled by.
    ///
    /// `None` when every track predates schema v7 — permanently, because no
    /// later scan can discover when a file arrived. `docs/adr/0019-group-keys.md`
    /// §5 records the three backfills that were considered and why each is a
    /// lie.
    pub first_seen_ns: Option<i64>,
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

    /// How long ago this album was last played, as of `now` — the
    /// [`GroupKey::Played`] shelf it lands on.
    ///
    /// The **most recent** bucket over **every** track of **every** edition,
    /// because the FLAC rip and the MP3 copy are one record: a listener who
    /// played the phone copy last week has not gone a year without this album.
    /// [`Recency`] is ordered most-recent-first, so "most recent" is `min`.
    ///
    /// No ledger, or a ledger with nothing about this album, is
    /// [`Recency::Never`] — the true answer, not a fallback.
    #[must_use]
    pub fn played_recency(&self, history: Option<&History>, now: SystemTime) -> Recency {
        let Some(history) = history else {
            return Recency::Never;
        };
        self.editions
            .iter()
            .flat_map(|edition| edition.tracks.iter())
            .map(|track| history.recency(&track.path, now))
            .min()
            .unwrap_or(Recency::Never)
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

/// How well a query fits the field it matched — the **first** signal in the
/// search ranking (`docs/adr/0021-search-ranking.md`).
///
/// Variant order *is* rank order, and the whole model is in it: **position
/// first, completeness second**, under one exact-match tier that is both.
/// Position dominates because a listener types the beginning of the name they
/// are thinking of — typing `kid` and being shown `Kids`, a record whose name
/// starts with what was typed, is the behaviour every incremental find in every
/// other program has; being shown `The Kid` instead is not.
///
/// "Word" means a boundary in the case-folded field: the neighbouring character
/// is not alphanumeric, or there is no neighbouring character. Scripts that do
/// not space their words — the CJK case the corpus tests pin — therefore reach
/// [`MatchTier::Fragment`] for an interior substring and [`MatchTier::Exact`]
/// for a whole field, which is the honest reading: there is no word boundary
/// evidence to use, so none is claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MatchTier {
    /// The query is the entire field: `kid a` in `kid a`.
    Exact,
    /// The query starts the field and ends on a word boundary: `kid` in
    /// `kid a`.
    PrefixWord,
    /// The query starts the field but ends inside a word: `kid` in `kids`.
    Prefix,
    /// The query starts a later word and ends on a word boundary: `road` in
    /// `abbey road`.
    Word,
    /// The query starts a later word but ends inside it: `roa` in `abbey road`.
    WordStart,
    /// The query starts inside a word: `bbey` in `abbey road`.
    Fragment,
}

impl MatchTier {
    /// How many tiers there are — the height of the ranking's first signal.
    const COUNT: usize = 6;

    /// Position in the variant order, which *is* the rank.
    fn rank(self) -> usize {
        match self {
            Self::Exact => 0,
            Self::PrefixWord => 1,
            Self::Prefix => 2,
            Self::Word => 3,
            Self::WordStart => 4,
            Self::Fragment => 5,
        }
    }
}

/// Which field a query matched — the **second** signal in the search ranking,
/// consulted only when two matches fit their fields equally well.
///
/// Variant order *is* rank order: who made it, then what record it is on, then
/// which song. It is second rather than first deliberately. Ranking by field
/// first would put every track by *Yesterdays New Quintet* above the Beatles'
/// `Yesterday` for the query `yesterday`, because an artist match would outrank
/// an exact title match — the fit of the match is the evidence about what the
/// listener meant, and the field is only a way of breaking a tie between two
/// equally good fits.
///
/// The tie it breaks it breaks upwards: at equal fit, the artist names a whole
/// body of work and the album names a record, so preferring them puts the
/// broadest true answer first and keeps a discography together at the top
/// rather than interleaved with songs that happen to share a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SearchField {
    /// The track artist or the album artist — both answer "who made this",
    /// and the haystack only carries the second when it differs from the
    /// first.
    Artist,
    /// The album title.
    Album,
    /// The track title.
    Title,
}

impl SearchField {
    /// How many fields there are — the height of the ranking's second signal.
    const COUNT: usize = 3;

    /// The field a haystack slot belongs to. The slots are artist, album
    /// artist, album, title (see [`IndexedTrack::haystack`]); the first two are
    /// one field for ranking.
    fn of_slot(slot: usize) -> Self {
        match slot {
            0 | 1 => Self::Artist,
            2 => Self::Album,
            _ => Self::Title,
        }
    }

    /// Position in the variant order, which *is* the rank.
    fn rank(self) -> usize {
        match self {
            Self::Artist => 0,
            Self::Album => 1,
            Self::Title => 2,
        }
    }
}

/// One match's rank: the ranking's first two signals, compared in order.
///
/// The derived [`Ord`] is lexicographic in declaration order — tier, then field
/// — which is precisely the model. The third signal (library order) is not in
/// here because it is not a property of the match: it is the position the
/// matching track already sits at, and it is applied by *stable* sorting rather
/// than by comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Relevance {
    tier: MatchTier,
    field: SearchField,
}

impl Relevance {
    /// How many distinct ranks exist: six tiers times three fields.
    ///
    /// This being a small constant is what makes ranking a **counting sort**
    /// rather than a comparison sort — the difference between O(n) and
    /// O(n log n) on the per-keystroke path, for a query that matches a large
    /// part of the library.
    const COUNT: usize = MatchTier::COUNT * SearchField::COUNT;

    /// This rank as an index in `0..COUNT`, ordered identically to [`Ord`]
    /// (tier major, field minor). The counting sort's correctness rests on
    /// that agreement, and `relevance_codes_are_ordered_like_the_comparison`
    /// asserts it over every value rather than trusting the arithmetic.
    fn code(self) -> usize {
        self.tier.rank() * SearchField::COUNT + self.field.rank()
    }
}

/// Which field of a track's haystack a match landed in: its byte range and its
/// slot number.
///
/// Carried between matches as a **cursor**. A haystack is four
/// `\n`-terminated fields and `memmem` reports matches at ascending offsets,
/// so the fields of one track can be walked forward once in total instead of
/// being re-derived from the start of the haystack for every match — which is
/// the difference between one pass per *track* and one pass per *match* on the
/// per-keystroke path. A one-character query matches every field of every
/// track several times over, so that difference is the whole cost of the first
/// keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Field {
    /// Byte where the field starts in the haystack.
    start: usize,
    /// Byte where it ends — the position of its terminating `\n`.
    end: usize,
    /// Which of the four slots it is (see [`IndexedTrack::haystack`]).
    slot: usize,
}

impl Field {
    /// The field of `haystack` containing byte `offset`, resuming from `from`
    /// when that cursor belongs to the same haystack and starts at or before
    /// the offset.
    ///
    /// A query cannot contain `\n` (it is refused before it reaches here), so a
    /// match lies wholly inside the field its first byte is in and the offset
    /// alone settles it.
    fn containing(haystack: &str, offset: usize, from: Option<Self>) -> Self {
        let bytes = haystack.as_bytes();
        let mut field = from.filter(|field| field.start <= offset).unwrap_or(Self {
            start: 0,
            end: 0,
            slot: 0,
        });
        loop {
            field.end = bytes
                .get(field.start..)
                .and_then(|rest| memchr::memchr(b'\n', rest))
                .map_or(bytes.len(), |newline| field.start + newline);
            if offset < field.end || field.end >= bytes.len() {
                return field;
            }
            field.start = field.end + 1;
            field.slot += 1;
        }
    }
}

/// How well a match at `start..start + needle_len` fits `field` — the ranking's
/// first signal, and the whole of the word-boundary rule.
fn tier_of(field: &str, start: usize, needle_len: usize) -> MatchTier {
    let end = start + needle_len;
    let at_start = start == 0;
    let at_end = end == field.len();
    let opens_word = at_start
        || !field
            .get(..start)
            .and_then(|before| before.chars().next_back())
            .is_some_and(char::is_alphanumeric);
    let closes_word = at_end
        || !field
            .get(end..)
            .and_then(|after| after.chars().next())
            .is_some_and(char::is_alphanumeric);
    match (at_start, at_end, opens_word, closes_word) {
        (true, true, _, _) => MatchTier::Exact,
        (true, _, _, true) => MatchTier::PrefixWord,
        (true, _, _, false) => MatchTier::Prefix,
        (_, _, true, true) => MatchTier::Word,
        (_, _, true, false) => MatchTier::WordStart,
        (_, _, false, _) => MatchTier::Fragment,
    }
}

/// Classify one match inside one track's `haystack`, at byte `offset` for
/// `needle_len` bytes, resuming the field walk from `from`.
///
/// Returns the rank and the cursor to hand back for this track's next match.
fn classify(
    haystack: &str,
    offset: usize,
    needle_len: usize,
    from: Option<Field>,
) -> (Relevance, Field) {
    let field = Field::containing(haystack, offset, from);
    let tier = haystack
        .get(field.start..field.end)
        .map_or(MatchTier::Fragment, |text| {
            tier_of(text, offset - field.start, needle_len)
        });
    (
        Relevance {
            tier,
            field: SearchField::of_slot(field.slot),
        },
        field,
    )
}

/// One matching track and how well it matched (its **best** match, when it
/// matched in several places).
#[derive(Debug, Clone, Copy)]
struct Hit {
    /// Position in [`SearchIndex::tracks`], which is library order.
    track: usize,
    relevance: Relevance,
}

/// One album's share of a result set: a contiguous slice of [`RankedHits::hits`]
/// and the best rank anything in it achieved.
#[derive(Debug, Clone, Copy)]
struct AlbumHits {
    /// Position in [`SearchIndex::album_starts`].
    album: usize,
    /// Where this album's hits start in [`RankedHits::hits`].
    first: usize,
    /// How many of them there are — never zero.
    len: usize,
    /// The best rank among them, which is the album's own rank.
    best: Relevance,
}

/// Every match for one query, ranked.
///
/// Built by [`Library::ranked`] and read two ways: as tracks
/// ([`Library::search`]) and as albums ([`Library::search_albums`]). Both are
/// projections of the *same* order, so a front end cannot get one ranking from
/// one call and a different one from the other.
#[derive(Debug, Default)]
struct RankedHits {
    /// Matching tracks. Filled in library order by the corpus scan, then
    /// re-ordered within each album by [`RankedHits::group`].
    hits: Vec<Hit>,
    /// The albums those hits belong to, in library order.
    albums: Vec<AlbumHits>,
    /// Positions in `albums`, in rank order.
    order: Vec<usize>,
}

impl RankedHits {
    /// Group the hits by album and rank them — the whole of the ranking's
    /// third signal and its album coherence rule.
    ///
    /// Hits arrive in library order and an album is one contiguous run of
    /// library order, so an album's hits are *already* adjacent: grouping is
    /// one linear pass and needs no map. Within an album the hits are then
    /// **stably** sorted by rank, and the albums are counting-sorted by their
    /// best rank. Stability is what makes library order the final tiebreak
    /// without ever being compared: equal ranks keep the order the scan
    /// produced, which is library order.
    fn group(&mut self, album_of: &[usize]) {
        for (position, hit) in self.hits.iter().enumerate() {
            let Some(&album) = album_of.get(hit.track) else {
                continue;
            };
            match self.albums.last_mut() {
                Some(last) if last.album == album => {
                    last.len += 1;
                    last.best = last.best.min(hit.relevance);
                }
                _ => self.albums.push(AlbumHits {
                    album,
                    first: position,
                    len: 1,
                    best: hit.relevance,
                }),
            }
        }
        for album in &self.albums {
            if let Some(slice) = self.hits.get_mut(album.first..album.first + album.len) {
                slice.sort_by_key(|hit| hit.relevance);
            }
        }
        self.order = counting_sort(&self.albums);
    }

    /// The matching tracks, best first — positions in [`SearchIndex::tracks`].
    fn tracks(&self) -> impl Iterator<Item = usize> + '_ {
        self.order
            .iter()
            .filter_map(|&album| self.albums.get(album))
            .flat_map(|album| {
                self.hits
                    .get(album.first..album.first + album.len)
                    .unwrap_or_default()
            })
            .map(|hit| hit.track)
    }

    /// The matching albums, best first, each once — positions in
    /// [`SearchIndex::album_starts`].
    fn albums(&self) -> impl Iterator<Item = usize> + '_ {
        self.order
            .iter()
            .filter_map(|&album| self.albums.get(album))
            .map(|album| album.album)
    }
}

/// Order `albums` by rank, best first, as a permutation of their positions.
///
/// A counting sort over [`Relevance::COUNT`] buckets rather than a comparison
/// sort: there are eighteen possible ranks whatever the library's size, so this
/// is linear in the number of matching albums and does no comparisons at all.
/// It is stable — each bucket is filled in the order it is walked — which is
/// how library order survives as the final tiebreak.
fn counting_sort(albums: &[AlbumHits]) -> Vec<usize> {
    let mut slots = [0usize; Relevance::COUNT];
    for album in albums {
        if let Some(count) = slots.get_mut(album.best.code()) {
            *count += 1;
        }
    }
    let mut offset = 0;
    for count in &mut slots {
        let start = offset;
        offset += *count;
        *count = start;
    }
    let mut order = vec![0usize; albums.len()];
    for (position, album) in albums.iter().enumerate() {
        if let Some(slot) = slots.get_mut(album.best.code())
            && let Some(target) = order.get_mut(*slot)
        {
            *target = position;
            *slot += 1;
        }
    }
    order
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
    /// Which album run each track belongs to — an index into `album_starts`,
    /// one entry per track. Search needs a matching track's album before it
    /// can rank it, on the per-keystroke path; recomputing the grouping there
    /// would mean re-deriving the whole shelf per keystroke.
    album_of: Vec<usize>,
    /// Position in `tracks` where each album's run of tracks begins. Library
    /// order groups an album into one consecutive run, so run `a` is
    /// `tracks[album_starts[a]..album_starts[a + 1]]` — this vector *is* the
    /// shelf, and both [`Library::albums`] and [`Library::search_albums`] build
    /// their [`Album`]s from it.
    album_starts: Vec<usize>,
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
    /// It also keeps whatever **first-seen** timestamp that entry carried, and
    /// takes `now_ns` only for a path it has never held. This mirrors, in RAM,
    /// the property [`UPSERT_TRACK`] holds in the database by naming
    /// `first_seen_ns` in its `INSERT` list and omitting it from its update
    /// list: a row's first-seen is written once and a rescan cannot move it.
    /// The two halves have to agree, or the shelf would disagree with itself
    /// across a restart.
    ///
    /// Callers must [`SearchIndex::rebuild_order`] afterwards (batched).
    fn insert(&mut self, meta: TrackMeta, now_ns: i64, root: Option<Arc<Path>>) {
        let existing = self
            .by_path
            .get(&meta.path)
            .and_then(|&index| self.tracks.get(index));
        let computed = existing.map_or_else(ComputedReplayGain::default, |track| track.computed);
        let first_seen = existing.map_or(Some(now_ns), |track| track.first_seen);
        // A caller that names no root leaves the row where it was, rather than
        // orphaning it — which mirrors the database, where `add_tracks` binds
        // NULL and the upsert's `root = excluded.root` would otherwise clear a
        // root a scan had recorded.
        let root = root.or_else(|| existing.and_then(|track| track.root.clone()));
        self.put(meta, computed, first_seen, root);
    }

    /// Insert a track together with the measurement recorded for it, the
    /// moment the library first saw it, and the root it was found under.
    fn put(
        &mut self,
        meta: TrackMeta,
        computed: ComputedReplayGain,
        first_seen: Option<i64>,
        root: Option<Arc<Path>>,
    ) {
        let entry = IndexedTrack::new(meta, computed, first_seen, root);
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
    ///
    /// The album runs are derived in the same pass, on the same key
    /// [`Library::albums`] has always grouped on, so the shelf and the search's
    /// notion of "one album" cannot drift apart.
    fn rebuild_order(&mut self) {
        self.tracks.sort_unstable_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then_with(|| a.meta.path.cmp(&b.meta.path))
        });
        self.by_path.clear();
        self.corpus.clear();
        self.starts.clear();
        self.album_of.clear();
        self.album_starts.clear();
        let mut current: Option<(&ArtistKey, &Option<String>)> = None;
        for (index, track) in self.tracks.iter().enumerate() {
            self.by_path.insert(track.meta.path.clone(), index);
            self.starts.push(self.corpus.len());
            self.corpus.push_str(&track.haystack);
            let key = (&track.key.artist, &track.key.album);
            if current != Some(key) {
                current = Some(key);
                self.album_starts.push(index);
            }
            self.album_of
                .push(self.album_starts.len().saturating_sub(1));
        }
    }

    /// One track's haystack, as a slice of the corpus.
    fn haystack_at(&self, track: usize) -> Option<&str> {
        let start = *self.starts.get(track)?;
        let end = self
            .starts
            .get(track + 1)
            .copied()
            .unwrap_or(self.corpus.len());
        self.corpus.get(start..end)
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
    /// When the library first stored this path, in nanoseconds since the Unix
    /// epoch — the ADDED group key's whole input (ADR-0018).
    ///
    /// Kept here rather than inside [`TrackMeta`] for [`IndexedTrack::computed`]'s
    /// reason, which applies more sharply: `TrackMeta` is what reading a file's
    /// tags yields, and no file carries the date it entered somebody's
    /// collection. Putting it on `TrackMeta` would also hand every rescan a
    /// value to overwrite, which is precisely what must not be possible.
    ///
    /// `None` is a row written before schema v7 — permanently, and honestly:
    /// see [`migrate_v6_to_v7`].
    first_seen: Option<i64>,
    /// The library root this file was found under (schema v8) — the fact
    /// removal's second gate keys on, in place of the path prefix it used to
    /// approximate with (`docs/adr/0022-library-roots-and-refresh.md`).
    ///
    /// Kept here rather than inside [`TrackMeta`] for [`IndexedTrack::computed`]'s
    /// reason: `TrackMeta` is what reading a file's *tags* yields, and no file
    /// carries the name of a folder somebody pointed baz at.
    ///
    /// Shared rather than owned: a hundred thousand rows come from a handful of
    /// folders, so the whole library holds one allocation per distinct root.
    ///
    /// `None` is a row from before v8 that no launch has adopted, or one added
    /// by a caller naming no root. It belongs to no root, so no root's scan can
    /// prune it — see [`Library::adopt_root`].
    root: Option<Arc<Path>>,
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
    fn new(
        meta: TrackMeta,
        computed: ComputedReplayGain,
        first_seen: Option<i64>,
        root: Option<Arc<Path>>,
    ) -> Self {
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
            first_seen,
            root,
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
///
/// It is also `ShelfSort::Artist`, and deliberately the same type: the order
/// the [`GroupKey::Artist`] shelves go in *is* the order the library's albums
/// go in, which is what makes that key `albums()` with its breaks named.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
        Self::of_album_artist(AlbumArtist::of(meta))
    }

    /// The same key from an already-resolved album artist — what the shelves
    /// sort on, where the fallback chain has run once for the whole album.
    fn of_album_artist(artist: AlbumArtist<'_>) -> Self {
        match artist {
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
            6 => migrate_v6_to_v7(conn)?,
            7 => migrate_v7_to_v8(conn)?,
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

/// v6 → v7: add the genre and first-seen columns the GENRE and ADDED group
/// keys are made of (ADR-0018).
///
/// `NULL` for every existing row, and for the two columns it means two
/// different things — which is the interesting half of this migration.
///
/// **`genre` is `NULL` and self-healing**, exactly as v3's `album_artist` was
/// and for the identical reason: a genre lives in the file's tags and nowhere
/// else, nothing already in the database implies one, and an upgrade must not
/// become a full library re-read at startup. baz rescans at every launch and
/// [`Library::add_tracks`] upserts, so the first scan after the upgrade fills
/// in every surviving file's real genre. That scan is incremental (v4), but a
/// migrated row's stamp is not disturbed here, so — unlike v3, whose migrated
/// rows had no stamp at all — an unchanged file is *not* re-read and keeps its
/// `NULL` genre until something touches it. Until then GENRE files it under
/// the untagged shelf, which is the honest answer to "what genre did this file
/// declare" for a row nobody has read the genre of.
///
/// **`first_seen_ns` is `NULL` and stays `NULL` forever**, and that is
/// deliberate rather than a gap. Three candidate backfills were considered and
/// all three are lies:
///
/// - *Now.* Stamping the migration's own clock would file a listener's entire
///   twenty-year collection under TODAY on the day they upgrade, and would then
///   be indistinguishable, forever after, from an import that really did happen
///   that day.
/// - *`mtime_ns`.* A file's modification time is real evidence, but it is
///   evidence about the *file*, not about when it entered the library: a
///   ReplayGain scanner or a tag fix rewrites it, so a library that has been
///   retagged would report itself as freshly imported. It is also `NULL` for
///   every pre-v4 row.
/// - *`id` order.* Row ids are an insertion sequence, not a clock, and they
///   would order a library that was imported in one pass by nothing more
///   meaningful than the directory walk.
///
/// So baz reports what it knows: it did not record when those files arrived,
/// and [`Recency::Unrecorded`] says exactly that on the shelf. Everything
/// scanned *after* the upgrade gets a real first-seen and appears under TODAY,
/// which is the case ADDED exists for — "new rips appear under ADDED"
/// (`docs/design/critique/02-surfaces.md`). The one thing a fabricated
/// backfill would buy is a prettier first screen, at the cost of the only
/// property the column has.
///
/// The `ALTER TABLE`s and the `user_version` bump are one transaction (SQLite's
/// DDL is transactional), so an interrupted upgrade leaves a v6 database that
/// the next open migrates again.
fn migrate_v6_to_v7(conn: &Connection) -> Result<(), IndexError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(SCHEMA_V7_COLUMNS)?;
    tx.pragma_update(None, "user_version", 7)?;
    tx.commit()?;
    Ok(())
}

/// v7 → v8: add `tracks.root` and the `roots` table
/// (`docs/adr/0022-library-roots-and-refresh.md`).
///
/// `root` is `NULL` for every existing row *here*, and unlike v3 – v7 that
/// `NULL` is neither permanent nor self-healed by an ordinary rescan. It is
/// filled by [`Library::adopt_root`], which the front end calls at launch for
/// each folder it is configured to hold.
///
/// **Why the backfill is not in the migration.** The fact is real and knowable
/// — a pre-v8 baz held exactly one music folder, so every row it wrote came
/// from that folder — but the migration is the one place in baz that cannot
/// know *which* folder: the name lives in `config.toml`, which is the front
/// end's file, and `baz-core` has never read it. So the migration adds the
/// column and the caller who holds the fact states it, one call, at the moment
/// it also states which roots to scan. That is the difference between this
/// backfill and the three ADR-0019 refused for `first_seen_ns`: those had no
/// holder anywhere, because nobody had ever recorded when a track arrived.
///
/// **Why an unadopted `NULL` is safe.** The removal gate reads the recorded
/// root, and `NULL` matches no root, so an unadopted row is one no scan can
/// delete. A migration that got the direction wrong here would delete
/// libraries; this one can only decline to prune.
///
/// The `roots` table starts **empty**, which says exactly the truth: no scan
/// has finished under this schema yet. The first completed scan of each folder
/// writes its row.
///
/// The `ALTER TABLE`, the `CREATE TABLE` and the `user_version` bump are one
/// transaction (SQLite's DDL is transactional), so an interrupted upgrade
/// leaves a v7 database that the next open migrates again.
fn migrate_v7_to_v8(conn: &Connection) -> Result<(), IndexError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(SCHEMA_V8)?;
    tx.pragma_update(None, "user_version", 8)?;
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
        genre: row.get(26)?,
        stamp: row_to_stamp(row)?,
        replay_gain: row_to_replay_gain(row)?,
    })
}

/// The first-seen timestamp a row carries (schema v7), or `None` for a row
/// written before the column existed — see [`migrate_v6_to_v7`] for why that
/// `None` is permanent and honest rather than a gap waiting to be filled.
fn row_to_first_seen(row: &rusqlite::Row<'_>) -> Result<Option<i64>, IndexError> {
    Ok(row.get(27)?)
}

/// The library root a row was recorded under (schema v8), or `None` for a row
/// written before the column existed that no launch has adopted — see
/// [`Library::adopt_root`].
fn row_to_root(row: &rusqlite::Row<'_>) -> Result<Option<PathBuf>, IndexError> {
    row.get::<_, Option<Vec<u8>>>(28)?
        .map(path_from_blob)
        .transpose()
}

/// The one [`Arc<Path>`] this library uses for `root`, made on first sight.
///
/// Hydrating a hundred-thousand-row library must not allocate a hundred
/// thousand copies of four folder names.
fn shared_root<'a>(roots: &'a mut HashMap<PathBuf, Arc<Path>>, root: &Path) -> &'a Arc<Path> {
    roots
        .entry(root.to_path_buf())
        .or_insert_with(|| Arc::from(root))
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
            genre: None,
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
            None,
            None,
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
            None,
            None,
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
            None,
            None,
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

    /// Every rank there is, in the order the model states them.
    fn every_relevance() -> Vec<Relevance> {
        let tiers = [
            MatchTier::Exact,
            MatchTier::PrefixWord,
            MatchTier::Prefix,
            MatchTier::Word,
            MatchTier::WordStart,
            MatchTier::Fragment,
        ];
        let fields = [SearchField::Artist, SearchField::Album, SearchField::Title];
        assert_eq!(tiers.len(), MatchTier::COUNT, "a tier was added unlisted");
        assert_eq!(
            fields.len(),
            SearchField::COUNT,
            "a field was added unlisted"
        );
        tiers
            .into_iter()
            .flat_map(|tier| {
                fields
                    .into_iter()
                    .map(move |field| Relevance { tier, field })
            })
            .collect()
    }

    #[test]
    fn relevance_codes_are_ordered_like_the_comparison() {
        // The counting sort ranks by `code()` and the rest of the model
        // compares by `Ord`. If those two ever disagreed the ranking would be
        // silently wrong rather than loudly broken, so they are checked
        // against each other over every value that exists.
        let all = every_relevance();
        assert_eq!(all.len(), Relevance::COUNT);
        for (position, relevance) in all.iter().enumerate() {
            assert_eq!(relevance.code(), position, "{relevance:?} is out of place");
        }
        for pair in all.windows(2) {
            let [earlier, later] = pair else { continue };
            assert!(earlier < later, "{earlier:?} must outrank {later:?}");
            assert!(earlier.code() < later.code());
        }
    }

    #[test]
    fn match_tiers_read_the_word_boundaries_they_claim_to() {
        // The model, field by field, at the level the doc comment states it.
        let cases = [
            ("kid a", "kid a", MatchTier::Exact),
            ("kid a", "kid", MatchTier::PrefixWord),
            ("kids", "kid", MatchTier::Prefix),
            ("abbey road", "road", MatchTier::Word),
            ("abbey road", "roa", MatchTier::WordStart),
            ("abbey road", "bbey", MatchTier::Fragment),
            // Punctuation is a boundary, so a hyphenated name reads as words.
            ("post-rock", "rock", MatchTier::Word),
            ("post-rock", "roc", MatchTier::WordStart),
            // A name that is all punctuation still has both its ends.
            ("!!! live", "!!!", MatchTier::PrefixWord),
            // No word boundaries to read: an interior CJK substring claims
            // nothing it cannot show, and a whole field is still exact.
            ("東京事変", "東京事変", MatchTier::Exact),
            ("東京事変", "京事", MatchTier::Fragment),
            ("東京事変", "東京", MatchTier::Prefix),
            // Multi-byte characters at the boundaries, so the char walk is
            // exercised rather than a byte one.
            ("größenwahn sinn", "größenwahn", MatchTier::PrefixWord),
            ("ein größenwahn", "größenwahn", MatchTier::Word),
        ];
        for (field, needle, expected) in cases {
            let start = field.find(needle).expect("fixture must contain its needle");
            assert_eq!(
                tier_of(field, start, needle.len()),
                expected,
                "{needle:?} in {field:?}"
            );
        }
    }

    #[test]
    fn the_field_cursor_lands_where_a_walk_from_the_start_would() {
        // The cursor is a speed trick on the per-keystroke path; it is only
        // allowed to be one. Resuming from *any* earlier field must give the
        // answer a fresh walk gives, for every byte of a real haystack.
        let haystack = "größenwahn\nrodik\n東京事変\nkid a\n";
        for offset in 0..haystack.len() {
            if !haystack.is_char_boundary(offset) {
                continue;
            }
            let fresh = Field::containing(haystack, offset, None);
            for resume in 0..=offset {
                if !haystack.is_char_boundary(resume) {
                    continue;
                }
                let from = Field::containing(haystack, resume, None);
                assert_eq!(
                    Field::containing(haystack, offset, Some(from)),
                    fresh,
                    "resuming at {resume} broke the field at {offset}"
                );
            }
        }
    }

    #[test]
    fn the_album_runs_are_the_shelf() {
        // Search ranks by album, and it does so from `album_starts` rather
        // than from `albums()`. The two must be the same grouping or the wall
        // and the ranking would disagree about what one record is.
        let mut library = Library::open_in_memory().expect("open");
        let track = |path: &str, artist: &str, album: &str| TrackMeta {
            path: PathBuf::from(path),
            artist: Some(artist.to_owned()),
            album: Some(album.to_owned()),
            ..bare_meta()
        };
        library
            .add_tracks(vec![
                track("/m/1.flac", "Aa", "One"),
                track("/m/2.flac", "Aa", "One"),
                track("/m/3.flac", "Aa", "Two"),
                track("/m/4.flac", "Bb", "One"),
                TrackMeta {
                    path: PathBuf::from("/m/5.flac"),
                    ..bare_meta()
                },
            ])
            .expect("add");

        assert_eq!(library.index.album_starts.len(), library.albums().len());
        assert_eq!(library.index.album_of.len(), library.len());
        for (position, &album) in library.index.album_of.iter().enumerate() {
            let start = library.index.album_starts[album];
            assert!(start <= position, "a track precedes its own album's run");
            let end = library
                .index
                .album_starts
                .get(album + 1)
                .copied()
                .unwrap_or(library.len());
            assert!(position < end, "a track falls outside its own album's run");
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
