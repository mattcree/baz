//! Playlist storage: one `.m3u8` file per playlist, in a folder the user
//! owns (ADR-0024 §1–§3).
//!
//! A playlist is **a named, ordered list of track references, made by a
//! person, stored in a file that person owns**. This module is the storage
//! layer only — the file format, the folder, and nothing else. It knows no
//! engine, no index and no pixels; a second front end reads playlists through
//! this module for the same reason it reads history through [`crate::history`].
//!
//! # The honesty clause, as an API property
//!
//! ADR-0024's second honesty clause — *nothing edits a playlist but the
//! user* — is enforced here by shape, not by discipline: **there is no
//! function in this module that writes a playlist file except
//! [`Playlist::save`]**, which exists solely to persist an explicit user
//! edit, and [`Folder::create`], [`Folder::rename`] and [`Folder::delete`],
//! which are the user's own acts on the shelf. Reading never writes back.
//! There is no auto-repair, no dedup, no normalisation-on-read that touches
//! the disk, and no code path by which playback, scanning, or anything else
//! can reach a playlist file. A missing entry stays in the file; deciding
//! what is playable is the caller's job ([`Playlist::partition`] merely
//! splits by the caller's verdict).
//!
//! # The format
//!
//! **Read liberally** (ADR-0024 §2): the `#EXTM3U` header is optional; a
//! bare list of paths is a playlist; CRLF and a UTF-8 BOM are tolerated;
//! blank lines are skipped; relative paths resolve against the file's own
//! directory and `~/` against the user's home; unknown `#EXT*` directives
//! and comments are preserved byte-verbatim, in order, so a rewrite never
//! strips a line it did not understand. Duplicate paths are allowed and
//! survive — a list that plays its theme twice is its maker's business.
//!
//! **Write the strict common subset**: `#EXTM3U` first, one
//! `#EXTINF:seconds,Artist - Title` line per entry when the metadata is
//! known, one absolute path per line, UTF-8, LF. Preserved foreign lines are
//! written back in their original positions relative to the entries. Every
//! save is an atomic whole-file rewrite — a temp file in the same directory,
//! then a rename, so a crash mid-save costs at most the edit and never the
//! playlist (the same-filesystem rename is what makes the swap atomic).
//!
//! **The round-trip law**: `read(write(read(f)))` equals `read(f)` for any
//! file — parse-then-rewrite is idempotent, and lossless for everything the
//! liberal reader is documented to preserve. The fuzz target
//! (`fuzz/fuzz_targets/playlist_m3u.rs`) holds [`parse`] → [`render`] →
//! [`parse`] to exactly this on arbitrary bytes.
//!
//! # Paths that are not UTF-8
//!
//! A Unix path is bytes, and a rare one is not valid UTF-8. Such a path is
//! read byte-verbatim ([`PathBuf`] from `OsStr` bytes) and written
//! byte-verbatim beneath a warning comment — the file honestly mirrors the
//! filesystem that produced it, and a baz-private escape dialect would
//! forfeit the interop that is the whole point of M3U (ADR-0024 §2). The
//! warning comment is baz's own: it is regenerated on write and skipped on
//! read rather than preserved, so it cannot multiply. On Windows, paths are
//! UTF-16 and never yield non-UTF-8 lines; foreign bytes in a file read
//! there degrade to the replacement character — the same trade
//! [`crate::history`] and [`crate::protocol`] already make.
//!
//! # The folder
//!
//! `$XDG_DATA_HOME/baz/playlists/` and its platform equivalents, through the
//! same `dirs` seam as the history ledger: [`Folder::open`] takes any
//! directory (tests hand it a tempdir), [`Folder::open_default`] asks the
//! platform. One file per playlist, filename = playlist name, `.m3u8`
//! written; plain `.m3u` is listed and read but never minted. External edits
//! are honoured by fingerprint, not by watcher: [`Playlist::fingerprint`]
//! captures the file's mtime and size at read, and the caller compares
//! ([`Playlist::externally_edited`]) before trusting a cached copy — last
//! writer wins per file.

use std::borrow::Cow;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::library::FileStamp;

mod format;

/// The directory under baz's data dir that holds the playlist files —
/// `<data_dir>/baz/playlists/`, beside `library.db` and `history.tsv`.
pub const PLAYLISTS_DIR: &str = "playlists";

/// The extension baz writes: `.m3u8`, whose UTF-8 mandate is the reason it
/// is the extension — plain `.m3u`'s locale-dependent encoding is the
/// documented ambiguity of the format.
pub const EXTENSION: &str = "m3u8";

/// The extension baz tolerates read-only: `.m3u` files dropped into the
/// folder are listed and read, but baz never creates one.
pub const LEGACY_EXTENSION: &str = "m3u";

/// One line of a playlist, as a value.
///
/// A playlist file is a sequence of these, in file order. Entries are the
/// tracks; notes are everything else the file carried — comments, provenance
/// lines, `#EXT*` directives this module does not interpret — preserved so
/// that a rewrite never strips what it did not understand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A track reference, with whatever `#EXTINF` metadata preceded it.
    Entry(Entry),
    /// A comment or directive preserved byte-verbatim, in its position.
    Note(Note),
}

impl Item {
    /// This item as an entry, or `None` if it is a note.
    #[must_use]
    pub fn as_entry(&self) -> Option<&Entry> {
        match self {
            Self::Entry(entry) => Some(entry),
            Self::Note(_) => None,
        }
    }
}

/// One track reference: a path, and the display metadata the file gave it.
///
/// The path is stored as read (resolved to absolute) and **nothing here has
/// checked whether it exists** — resolution against the filesystem and the
/// library is the caller's job, decided at the moment it matters
/// ([`Playlist::partition`] applies the caller's verdict). An entry whose
/// path no longer resolves stays in the file (ADR-0024 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The referenced file. Absolute after a read: relative paths were
    /// resolved against the playlist file's own directory, `~/` against
    /// the user's home.
    pub path: PathBuf,
    /// The `#EXTINF` metadata that preceded this path, when there was any.
    pub extinf: Option<ExtInf>,
}

impl Entry {
    /// An entry for `path` with no metadata.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            extinf: None,
        }
    }
}

/// The `#EXTINF` metadata of one entry: `#EXTINF:seconds,display title`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtInf {
    /// The declared length in whole seconds, or `None` where the file said
    /// `-1` (the format's convention for "unknown"). Fractional lengths in
    /// the wild are read rounded down.
    pub seconds: Option<u64>,
    /// The display title, conventionally `Artist - Title`, verbatim from
    /// after the first comma. May be empty.
    pub title: String,
}

/// A comment or directive line preserved byte-verbatim.
///
/// Bytes rather than a `String` because the liberal reader preserves what it
/// found, and what it found is not guaranteed to be UTF-8. [`Note::text`] is
/// the lossy view for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note(pub(crate) Vec<u8>);

impl Note {
    /// A note carrying `text`, made safe for the line-oriented file: line
    /// breaks become spaces, and a line that would not read back as a
    /// comment gains a `# ` prefix.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let flat: String = text
            .chars()
            .map(|ch| if ch == '\n' || ch == '\r' { ' ' } else { ch })
            .collect();
        if flat.trim_start().starts_with('#') {
            Self(flat.into_bytes())
        } else {
            Self(format!("# {flat}").into_bytes())
        }
    }

    /// The line, exactly as it sits in the file (no trailing newline).
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    /// The line as text, replacement characters standing in for any bytes
    /// that were never UTF-8.
    #[must_use]
    pub fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }
}

/// Parse M3U bytes into items — the liberal read, documented on
/// [the module](self).
///
/// `directory` is what relative paths resolve against; pass the playlist
/// file's own parent, absolute. (With a relative base the resolved paths
/// stay relative and the round-trip law does not hold; every caller in baz
/// has an absolute one.) `~/` resolves against the platform's home
/// directory. Never touches the filesystem and never panics, whatever the
/// bytes — this is the surface the fuzz target drives.
#[must_use]
pub fn parse(bytes: &[u8], directory: &Path) -> Vec<Item> {
    format::parse(bytes, directory, dirs::home_dir().as_deref())
}

/// Render items to M3U bytes — the strict write, documented on
/// [the module](self): `#EXTM3U`, `#EXTINF` where known, one path per line,
/// LF, notes verbatim in their positions.
///
/// Infallible by construction over anything [`parse`] produced.
/// [`Playlist::save`] is the guarded door for caller-built entries: it
/// refuses what M3U cannot carry (a relative path, a line break, a path
/// with leading or trailing whitespace) before rendering.
#[must_use]
pub fn render(items: &[Item]) -> Vec<u8> {
    format::render(items)
}

/// Something went wrong with a playlist file or its folder.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum PlaylistError {
    /// A file or directory could not be read, written, or created.
    #[error("playlist {}: {source}", path.display())]
    Io {
        /// The file or directory in question.
        path: PathBuf,
        /// What the operating system said.
        #[source]
        source: std::io::Error,
    },
    /// The platform has no data directory to put the folder in, so
    /// [`Folder::open_default`] has nowhere to look.
    #[error("no user data directory on this platform; open the folder by path instead")]
    NoDataDirectory,
    /// No playlist file of this name is in the folder.
    #[error("no playlist named {name:?}")]
    NotFound {
        /// The name that was asked for.
        name: String,
    },
    /// A playlist of this name already exists, and nothing here overwrites.
    #[error("a playlist named {name:?} already exists")]
    AlreadyExists {
        /// The name that collided.
        name: String,
    },
    /// The name cannot be a filename; [`validate_name`] states the rule.
    #[error("{name:?} cannot be a playlist name: {why}")]
    InvalidName {
        /// The rejected name.
        name: String,
        /// Which part of the rule it broke.
        why: &'static str,
    },
    /// The entry cannot be expressed in an M3U line, so [`Playlist::save`]
    /// refused rather than writing a file that would read back differently.
    #[error("entry {} cannot be written: {why}", path.display())]
    UnwritableEntry {
        /// The entry's path.
        path: PathBuf,
        /// What M3U cannot carry about it.
        why: &'static str,
    },
}

/// Windows' reserved device names, which no portable filename may use as
/// its first dot-separated part.
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// The name rule, in one place: a playlist name must survive as a filename
/// on every filesystem baz runs on, because the filename *is* the name
/// (ADR-0024 §2) and playlists are meant to be synced and copied.
///
/// Rejected: the empty and the all-whitespace name; `.` and `..`; path
/// separators (`/`, `\`) and NUL; the characters Windows filesystems refuse
/// (`< > : " | ? *`); control characters; a leading or trailing space and a
/// trailing dot (silently stripped on Windows, so the file would come back
/// under a different name); Windows' reserved device names (`CON`, `NUL`,
/// `COM1`… — checked against the part before the first dot); and names
/// longer than 200 bytes, which with the extension would brush filesystem
/// filename limits.
///
/// This validates names baz **mints** ([`Folder::create`],
/// [`Folder::rename`]). Files already on disk are read whatever they are
/// called — the folder is the user's, and a liberal reader does not refuse
/// what a filesystem allowed.
///
/// # Errors
///
/// [`PlaylistError::InvalidName`], naming the part of the rule broken.
pub fn validate_name(name: &str) -> Result<(), PlaylistError> {
    let fail = |why: &'static str| {
        Err(PlaylistError::InvalidName {
            name: name.to_string(),
            why,
        })
    };
    if name.trim().is_empty() {
        return fail("it is empty");
    }
    if name == "." || name == ".." {
        return fail("it names a directory, not a playlist");
    }
    if name.contains(['/', '\\']) {
        return fail("it contains a path separator");
    }
    if name.contains(['\0', '<', '>', ':', '"', '|', '?', '*']) {
        return fail("it contains a character some filesystems refuse");
    }
    if name.chars().any(char::is_control) {
        return fail("it contains a control character");
    }
    if name.starts_with(' ') || name.ends_with(' ') || name.ends_with('.') {
        return fail("it begins or ends with a space or ends with a dot, which Windows strips");
    }
    let stem = name.split('.').next().unwrap_or(name);
    if RESERVED_NAMES
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return fail("it is a reserved device name on Windows");
    }
    if name.len() > 200 {
        return fail("it is longer than a filename can safely be");
    }
    Ok(())
}

/// One playlist, as a value: its name, its file, and its lines in order.
///
/// Obtained from [`Playlist::read`] or [`Folder::create`]; edited in memory
/// through [`Playlist::items_mut`]; persisted — and only ever persisted — by
/// the explicit [`Playlist::save`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    name: String,
    path: PathBuf,
    items: Vec<Item>,
    stamp: Option<FileStamp>,
}

/// [`Playlist::partition`]'s verdict: the entries the caller judged
/// playable, and the ones it did not, both in playlist order.
///
/// "Play sends the playable subset" (ADR-0024 §3) is built on this — and on
/// nothing here having an opinion of its own about what plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition<'a> {
    /// Entries the caller's predicate accepted, in order.
    pub playable: Vec<&'a Entry>,
    /// Entries it declined — missing, unindexed, whatever the caller's
    /// standard was — in order, still in the file.
    pub missing: Vec<&'a Entry>,
}

impl Playlist {
    /// Read the playlist file at `path` — any `.m3u8` or `.m3u`, from the
    /// folder or anywhere else (an import is just a read).
    ///
    /// The name is the file's stem; relative entries resolve against the
    /// file's own directory; the file is not modified, whatever shape it is
    /// in. The fingerprint is captured before the bytes are read, so an
    /// edit racing this read is caught by the next
    /// [`externally_edited`](Self::externally_edited) check rather than
    /// missed.
    ///
    /// # Errors
    ///
    /// [`PlaylistError::Io`] if the file cannot be read. A file full of
    /// lines this module does not understand is not an error — the liberal
    /// reader keeps them as [`Note`]s.
    pub fn read(path: impl Into<PathBuf>) -> Result<Self, PlaylistError> {
        let path = path.into();
        let stamp = FileStamp::of_path(&path);
        let bytes = std::fs::read(&path).map_err(|source| PlaylistError::Io {
            path: path.clone(),
            source,
        })?;
        let directory = path.parent().unwrap_or_else(|| Path::new(""));
        let items = parse(&bytes, directory);
        Ok(Self {
            name: path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            items,
            stamp,
        })
    }

    /// The playlist's name — the file's stem, exactly (ADR-0024 §2:
    /// filename = playlist name).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The file this playlist was read from and saves to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every line of the playlist, entries and notes, in file order.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The lines, editable. This edits the value in memory; nothing reaches
    /// the file until [`save`](Self::save).
    pub fn items_mut(&mut self) -> &mut Vec<Item> {
        &mut self.items
    }

    /// The entries alone, in order, duplicates and all.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.items.iter().filter_map(Item::as_entry)
    }

    /// Split the entries by the caller's judgement of what is playable.
    ///
    /// The predicate is the caller's whole authority: this module has not
    /// stat'ed anything and holds no opinion. Duplicates are judged each
    /// time they appear, so a list that repeats a track partitions as many
    /// times as it plays it.
    #[must_use]
    pub fn partition(&self, mut is_playable: impl FnMut(&Path) -> bool) -> Partition<'_> {
        let mut partition = Partition {
            playable: Vec::new(),
            missing: Vec::new(),
        };
        for entry in self.entries() {
            if is_playable(&entry.path) {
                partition.playable.push(entry);
            } else {
                partition.missing.push(entry);
            }
        }
        partition
    }

    /// The file's fingerprint — mtime and size — as it was when this value
    /// was read, or `None` on a filesystem that cannot report one.
    ///
    /// The external-edit mechanism, whole (ADR-0024 §2: *external edits
    /// honoured via mtime; last writer wins per file*). Compare with
    /// [`FileStamp::of_path`] now, or use
    /// [`externally_edited`](Self::externally_edited).
    #[must_use]
    pub fn fingerprint(&self) -> Option<FileStamp> {
        self.stamp
    }

    /// Whether the file has changed since this value was read — an external
    /// editor, a sync tool, another baz. One `stat`, no read, no write.
    ///
    /// A filesystem that reports no usable stamp at all makes this `false`
    /// rather than crying wolf on every check; a caller that must be sure
    /// re-reads. A deleted file reads as edited, which it is.
    #[must_use]
    pub fn externally_edited(&self) -> bool {
        FileStamp::of_path(&self.path) != self.stamp
    }

    /// Persist the user's edit: the whole file, rewritten atomically.
    ///
    /// **This is the only function in this module that writes an existing
    /// playlist file**, and it exists to be called at exactly one moment —
    /// when a person edited this playlist and meant it (the module docs
    /// carry the honesty clause this enforces). The write is a temp file in
    /// the playlist's own directory followed by a rename, so the old file
    /// is intact until the new one is complete, on the same filesystem, and
    /// the swap is atomic. Foreign notes are written back in their
    /// positions; nothing is deduplicated, sorted, or repaired.
    ///
    /// # Errors
    ///
    /// [`PlaylistError::UnwritableEntry`] before a byte is written if an
    /// entry cannot be expressed in M3U — a relative path (baz writes
    /// absolute, ADR-0024 §2), a path containing a line break (the format
    /// is line-oriented and has no escapes), or one with leading or
    /// trailing whitespace (the liberal reader would trim it back
    /// differently). [`PlaylistError::Io`] if the write itself fails; the
    /// previous file contents survive any failure.
    pub fn save(&mut self) -> Result<(), PlaylistError> {
        for entry in self.entries() {
            let bytes = format::path_bytes(&entry.path);
            let why = if !entry.path.is_absolute() {
                Some("playlists carry absolute paths, and this one is relative")
            } else if bytes.contains(&b'\n') {
                Some("its path contains a line break, which M3U cannot carry")
            } else if bytes.trim_ascii() != bytes.as_ref() {
                Some("its path begins or ends with whitespace, which a reader would trim away")
            } else {
                None
            };
            if let Some(why) = why {
                return Err(PlaylistError::UnwritableEntry {
                    path: entry.path.clone(),
                    why,
                });
            }
        }
        write_atomic(&self.path, &render(&self.items))?;
        self.stamp = FileStamp::of_path(&self.path);
        Ok(())
    }
}

/// Which dialect a listed file is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// `.m3u8` — UTF-8 by mandate; the one baz writes.
    M3u8,
    /// `.m3u` — read-only tolerated; baz never creates one, and a rename
    /// keeps the extension rather than silently converting the file.
    M3u,
}

/// One playlist as the folder lists it: a name, a file, a dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistFile {
    /// The playlist's name — the file's stem (lossily decoded for a stem
    /// that is not UTF-8; the path below stays exact).
    pub name: String,
    /// The file itself.
    pub path: PathBuf,
    /// Which extension it carries.
    pub dialect: Dialect,
}

impl PlaylistFile {
    /// Read this playlist. Sugar for [`Playlist::read`] on
    /// [`path`](Self::path).
    ///
    /// # Errors
    ///
    /// Whatever [`Playlist::read`] reports.
    pub fn read(&self) -> Result<Playlist, PlaylistError> {
        Playlist::read(&self.path)
    }
}

/// The playlists folder: enumeration, and the user's acts on the shelf —
/// create, rename, delete.
///
/// Opened on any directory for tests and tools, or on the platform's data
/// directory via [`Folder::open_default`] — the same seam as
/// [`HistoryLedger::open`](crate::history::HistoryLedger::open) /
/// [`open_default`](crate::history::HistoryLedger::open_default), for the
/// same reason: nothing in here needs to know where it is.
#[derive(Debug, Clone)]
pub struct Folder {
    dir: PathBuf,
}

impl Folder {
    /// Open (or create) the playlists folder at `dir`.
    ///
    /// Creating the directory is not editing a playlist; no playlist file
    /// is touched.
    ///
    /// # Errors
    ///
    /// [`PlaylistError::Io`] if the directory cannot be created.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, PlaylistError> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).map_err(|source| PlaylistError::Io {
            path: dir.clone(),
            source,
        })?;
        Ok(Self { dir })
    }

    /// Open the folder at [`Self::default_path`] — in the user's own data
    /// directory, beside the library and the history ledger.
    ///
    /// # Errors
    ///
    /// [`PlaylistError::NoDataDirectory`] if the platform has no data
    /// directory, or whatever [`Folder::open`] reports.
    pub fn open_default() -> Result<Self, PlaylistError> {
        Self::open(Self::default_path().ok_or(PlaylistError::NoDataDirectory)?)
    }

    /// `$XDG_DATA_HOME/baz/playlists/` and its platform equivalents.
    #[must_use]
    pub fn default_path() -> Option<PathBuf> {
        Some(dirs::data_dir()?.join("baz").join(PLAYLISTS_DIR))
    }

    /// The directory this folder lists.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Every playlist in the folder — `*.m3u8`, plus read-only `*.m3u` —
    /// sorted by name, case-insensitively.
    ///
    /// Names are the file stems, whatever they are: enumeration is as
    /// liberal as reading, and [`validate_name`] binds only what baz mints.
    ///
    /// # Errors
    ///
    /// [`PlaylistError::Io`] if the directory cannot be read.
    pub fn list(&self) -> Result<Vec<PlaylistFile>, PlaylistError> {
        let entries = std::fs::read_dir(&self.dir).map_err(|source| PlaylistError::Io {
            path: self.dir.clone(),
            source,
        })?;
        let mut found = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }
            let Some(extension) = path.extension() else {
                continue;
            };
            let dialect = if extension.eq_ignore_ascii_case(EXTENSION) {
                Dialect::M3u8
            } else if extension.eq_ignore_ascii_case(LEGACY_EXTENSION) {
                Dialect::M3u
            } else {
                continue;
            };
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            found.push(PlaylistFile {
                name,
                path,
                dialect,
            });
        }
        found.sort_by(|a, b| {
            (a.name.to_lowercase(), &a.name).cmp(&(b.name.to_lowercase(), &b.name))
        });
        Ok(found)
    }

    /// Create a new, empty playlist named `name` — the user's act.
    ///
    /// Writes `<dir>/<name>.m3u8` holding the header and a provenance
    /// comment (`# made with baz on …` — inert, legible, never consulted
    /// for behaviour; ADR-0024 §2), atomically. Refuses a name that already
    /// exists in either dialect: creating is never overwriting.
    ///
    /// # Errors
    ///
    /// [`PlaylistError::InvalidName`] per [`validate_name`];
    /// [`PlaylistError::AlreadyExists`] if the name is taken;
    /// [`PlaylistError::Io`] if the write fails.
    pub fn create(&self, name: &str) -> Result<Playlist, PlaylistError> {
        validate_name(name)?;
        if self.locate(name).is_ok() {
            return Err(PlaylistError::AlreadyExists {
                name: name.to_string(),
            });
        }
        let path = self.dir.join(format!("{name}.{EXTENSION}"));
        let date = crate::history::format::format_timestamp(crate::history::now_unix_s());
        let content = format!(
            "{}\n# made with baz on {}\n",
            format::HEADER_LINE,
            &date[..10]
        );
        write_atomic(&path, content.as_bytes())?;
        Playlist::read(path)
    }

    /// Rename the playlist named `from` to `to` — a filesystem rename of
    /// the file, keeping its extension. Refuses an existing target in
    /// either dialect; nothing here overwrites. The check precedes the
    /// rename rather than being atomic with it, which for a single user's
    /// own data directory is the honest trade.
    ///
    /// # Errors
    ///
    /// [`PlaylistError::NotFound`] if `from` is not in the folder;
    /// [`PlaylistError::InvalidName`] per [`validate_name`];
    /// [`PlaylistError::AlreadyExists`] if `to` is taken;
    /// [`PlaylistError::Io`] if the rename fails.
    pub fn rename(&self, from: &str, to: &str) -> Result<PlaylistFile, PlaylistError> {
        validate_name(to)?;
        let source = self.locate(from)?;
        if self.locate(to).is_ok() {
            return Err(PlaylistError::AlreadyExists {
                name: to.to_string(),
            });
        }
        let extension = match source.dialect {
            Dialect::M3u8 => EXTENSION,
            Dialect::M3u => LEGACY_EXTENSION,
        };
        let target = self.dir.join(format!("{to}.{extension}"));
        std::fs::rename(&source.path, &target).map_err(|error| PlaylistError::Io {
            path: source.path.clone(),
            source: error,
        })?;
        Ok(PlaylistFile {
            name: to.to_string(),
            path: target,
            dialect: source.dialect,
        })
    }

    /// Delete the playlist named `name` — the file goes; the music stays.
    ///
    /// Unlinks outright. The product's own `Delete` does not spend this —
    /// it spends [`Self::delete_to_trash`], the reversible form (doc 11 §5
    /// P2) — but tools and tests that mean *remove, now, here* keep the
    /// plain verb.
    ///
    /// # Errors
    ///
    /// [`PlaylistError::NotFound`] if it is not in the folder;
    /// [`PlaylistError::Io`] if the file cannot be removed.
    pub fn delete(&self, name: &str) -> Result<(), PlaylistError> {
        let found = self.locate(name)?;
        std::fs::remove_file(&found.path).map_err(|source| PlaylistError::Io {
            path: found.path,
            source,
        })
    }

    /// Move the playlist named `name` to the **platform trash** — the file
    /// goes where the desktop's own restore can reach it; the music stays.
    ///
    /// The forgiveness form of [`Self::delete`] (doc 11 §5 P2): deletion of
    /// a small file is the textbook reversible act, so the interface spends
    /// this and retires its confirm dialog — the trash *is* the safety net,
    /// so the warning's job is done by the mechanism instead. On Linux this
    /// is the freedesktop trash spec (`$XDG_DATA_HOME/Trash`, or the
    /// mount's own `.Trash-$uid`), which every desktop file manager lists
    /// and restores from; on macOS and Windows it is the Trash and the
    /// Recycle Bin.
    ///
    /// A failure leaves the file exactly where it was: refusing to delete
    /// is the honest answer when the reversible route is not available, and
    /// nothing falls back to unlinking behind the listener's back.
    ///
    /// # Errors
    ///
    /// [`PlaylistError::NotFound`] if it is not in the folder;
    /// [`PlaylistError::Io`] if the platform refuses the move (carrying the
    /// trash layer's own words).
    pub fn delete_to_trash(&self, name: &str) -> Result<(), PlaylistError> {
        let found = self.locate(name)?;
        trash::delete(&found.path).map_err(|error| PlaylistError::Io {
            path: found.path,
            source: std::io::Error::other(error),
        })
    }

    /// The file currently answering to `name`, `.m3u8` before `.m3u`.
    fn locate(&self, name: &str) -> Result<PlaylistFile, PlaylistError> {
        for (extension, dialect) in [(EXTENSION, Dialect::M3u8), (LEGACY_EXTENSION, Dialect::M3u)] {
            let path = self.dir.join(format!("{name}.{extension}"));
            if path.is_file() {
                return Ok(PlaylistFile {
                    name: name.to_string(),
                    path,
                    dialect,
                });
            }
        }
        Err(PlaylistError::NotFound {
            name: name.to_string(),
        })
    }
}

/// Write `bytes` to `path` through a temp file in the same directory and a
/// rename — the same-filesystem guarantee that makes the swap atomic. The
/// data is synced before the rename, so a crash leaves either the old file
/// or the complete new one, never a torn one.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), PlaylistError> {
    let fail = |source: std::io::Error| PlaylistError::Io {
        path: path.to_path_buf(),
        source,
    };
    let directory = path.parent().unwrap_or_else(|| Path::new(""));
    let pid = std::process::id();
    for attempt in 0u32..1024 {
        // `.tmp`, not `.m3u8`: enumeration must never list a half-written
        // file, and a leftover from a crash is visibly debris, not a list.
        let candidate = directory.join(format!(".baz-playlist-{pid}-{attempt}.tmp"));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(fail(error)),
        };
        let written = file
            .write_all(bytes)
            .and_then(|()| file.sync_data())
            .and_then(|()| {
                drop(file);
                std::fs::rename(&candidate, path)
            });
        return match written {
            Ok(()) => Ok(()),
            Err(error) => {
                // Best effort: the temp file is debris either way, and the
                // error worth reporting is the write's.
                let _ = std::fs::remove_file(&candidate);
                Err(fail(error))
            }
        };
    }
    Err(fail(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not find a free temp-file name",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> (tempfile::TempDir, Folder) {
        let dir = tempfile::tempdir().expect("tempdir");
        let folder = Folder::open(dir.path().join("playlists")).expect("open");
        (dir, folder)
    }

    /// An absolute fixture path *by the platform's own rule*. `/music/a.flac`
    /// is absolute on unix but drive-less — and therefore relative — on
    /// Windows, where `save` rightly refuses it; the same fixture gains `C:`
    /// there. The third Windows-fixture lesson in this repo, after the
    /// UTF-16LE stored paths and the FILETIME stamps: a fixture must satisfy
    /// the property it claims on every platform CI runs, not just the one it
    /// was written on.
    fn track(path: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:{path}"))
        } else {
            PathBuf::from(path)
        }
    }

    // ----- the folder ------------------------------------------------------

    #[test]
    fn the_default_path_sits_beside_the_library() {
        if let Some(path) = Folder::default_path() {
            assert!(path.ends_with("baz/playlists") || path.ends_with("baz\\playlists"));
        }
    }

    #[test]
    fn create_list_rename_delete_round_the_shelf() {
        let (_keep, folder) = folder();
        folder.create("Driving").expect("create");
        folder.create("ambient").expect("create");
        let names: Vec<String> = folder
            .list()
            .expect("list")
            .into_iter()
            .map(|file| file.name)
            .collect();
        // Sorted case-insensitively: 'a' before 'D' would be wrong reading
        // case-sensitively, right reading like a person.
        assert_eq!(names, ["ambient", "Driving"]);

        folder.rename("ambient", "Ambient late").expect("rename");
        let names: Vec<String> = folder
            .list()
            .expect("list")
            .into_iter()
            .map(|file| file.name)
            .collect();
        assert_eq!(names, ["Ambient late", "Driving"]);

        folder.delete("Driving").expect("delete");
        assert_eq!(folder.list().expect("list").len(), 1);
        assert!(matches!(
            folder.delete("Driving"),
            Err(PlaylistError::NotFound { .. })
        ));
    }

    #[test]
    fn a_new_playlist_documents_its_own_provenance() {
        let (_keep, folder) = folder();
        let playlist = folder.create("Quiet").expect("create");
        assert_eq!(playlist.name(), "Quiet");
        assert_eq!(playlist.entries().count(), 0);
        // The provenance comment is an ordinary preserved note.
        let notes: Vec<String> = playlist
            .items()
            .iter()
            .filter_map(|item| match item {
                Item::Note(note) => Some(note.text().into_owned()),
                Item::Entry(_) => None,
            })
            .collect();
        assert_eq!(notes.len(), 1);
        assert!(notes[0].starts_with("# made with baz on "), "{notes:?}");
        let text = std::fs::read_to_string(playlist.path()).expect("read");
        assert!(text.starts_with("#EXTM3U\n"), "{text:?}");
    }

    #[test]
    fn create_refuses_an_existing_name_in_either_dialect() {
        let (_keep, folder) = folder();
        folder.create("Mix").expect("create");
        assert!(matches!(
            folder.create("Mix"),
            Err(PlaylistError::AlreadyExists { .. })
        ));
        // A legacy .m3u also holds its name: creating would shadow it.
        std::fs::write(folder.dir().join("Road.m3u"), "/a.flac\n").expect("write");
        assert!(matches!(
            folder.create("Road"),
            Err(PlaylistError::AlreadyExists { .. })
        ));
    }

    #[test]
    fn rename_refuses_an_existing_target_and_does_not_overwrite() {
        let (_keep, folder) = folder();
        folder.create("A").expect("create");
        let mut b = folder.create("B").expect("create");
        b.items_mut()
            .push(Item::Entry(Entry::new(track("/music/b.flac"))));
        b.save().expect("save");
        let before = std::fs::read(b.path()).expect("read");
        assert!(matches!(
            folder.rename("A", "B"),
            Err(PlaylistError::AlreadyExists { .. })
        ));
        // B's file is untouched by the refusal.
        assert_eq!(std::fs::read(b.path()).expect("read"), before);
        assert!(matches!(
            folder.rename("missing", "C"),
            Err(PlaylistError::NotFound { .. })
        ));
    }

    #[test]
    fn a_legacy_m3u_is_listed_read_and_kept_legacy_on_rename() {
        let (_keep, folder) = folder();
        std::fs::write(folder.dir().join("From foobar.m3u"), "/music/a.flac\n").expect("write");
        let listed = folder.list().expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].dialect, Dialect::M3u);
        let playlist = listed[0].read().expect("read");
        assert_eq!(playlist.entries().count(), 1);
        // A rename keeps the extension: renaming is not converting.
        let renamed = folder.rename("From foobar", "Imported").expect("rename");
        assert_eq!(renamed.dialect, Dialect::M3u);
        assert!(renamed.path.ends_with("Imported.m3u"));
    }

    #[test]
    fn enumeration_skips_what_is_not_a_playlist() {
        let (_keep, folder) = folder();
        folder.create("Real").expect("create");
        std::fs::write(folder.dir().join("notes.txt"), "not a playlist").expect("write");
        std::fs::write(folder.dir().join(".baz-playlist-1-0.tmp"), "debris").expect("write");
        std::fs::create_dir(folder.dir().join("subdir.m3u8")).expect("mkdir");
        let names: Vec<String> = folder
            .list()
            .expect("list")
            .into_iter()
            .map(|file| file.name)
            .collect();
        assert_eq!(names, ["Real"]);
    }

    // ----- the name rule ---------------------------------------------------

    #[test]
    fn the_name_rule_rejects_what_a_filesystem_would_mangle() {
        for bad in [
            "",
            "  ",
            ".",
            "..",
            "a/b",
            "a\\b",
            "a\0b",
            "a:b",
            "a\"b",
            "a|b",
            "a?b",
            "a*b",
            "a<b",
            "a>b",
            "a\tb",
            " leading",
            "trailing ",
            "trailing.",
            "CON",
            "con",
            "Con.backup",
            "COM7",
            "lpt3.old",
        ] {
            assert!(
                matches!(validate_name(bad), Err(PlaylistError::InvalidName { .. })),
                "{bad:?} should have been rejected"
            );
        }
        assert!(
            matches!(
                validate_name(&"x".repeat(201)),
                Err(PlaylistError::InvalidName { .. })
            ),
            "an over-long name should have been rejected"
        );
        for good in [
            "Driving",
            "quiet, late",
            "1977–82 singles",
            "音楽",
            "mix.v2",
            "console", // contains 'con' but is not the device name
            ".hidden", // a dot-file is odd but survives everywhere
        ] {
            assert!(validate_name(good).is_ok(), "{good:?} should have passed");
        }
    }

    // ----- reading and saving ---------------------------------------------

    #[test]
    fn an_edit_reaches_the_file_only_at_save() {
        let (_keep, folder) = folder();
        let mut playlist = folder.create("Mix").expect("create");
        let before = std::fs::read(playlist.path()).expect("read");
        playlist
            .items_mut()
            .push(Item::Entry(Entry::new(track("/music/a.flac"))));
        assert_eq!(
            std::fs::read(playlist.path()).expect("read"),
            before,
            "an in-memory edit touched the file"
        );
        playlist.save().expect("save");
        let text = std::fs::read_to_string(playlist.path()).expect("read");
        assert!(text.contains("/music/a.flac\n"), "{text:?}");
        // The provenance note survived the rewrite, in its place.
        assert!(text.contains("# made with baz on "), "{text:?}");
    }

    #[test]
    fn reading_never_writes() {
        let (_keep, folder) = folder();
        // A messy imported file: headerless, CRLF, foreign directive.
        let path = folder.dir().join("Imported.m3u8");
        let source = b"#EXTGRP:oddball\r\n/music/a.flac\r\n\r\n/music/a.flac\r\n";
        std::fs::write(&path, source).expect("write");
        let playlist = Playlist::read(&path).expect("read");
        assert_eq!(playlist.entries().count(), 2, "duplicates must survive");
        let _ = folder.list().expect("list");
        assert_eq!(
            std::fs::read(&path).expect("read"),
            source.to_vec(),
            "a read or a listing rewrote the file"
        );
    }

    #[test]
    fn save_is_atomic_and_leaves_no_debris() {
        let (_keep, folder) = folder();
        let mut playlist = folder.create("Mix").expect("create");
        for path in ["/music/a.flac", "/music/b.flac"] {
            playlist
                .items_mut()
                .push(Item::Entry(Entry::new(track(path))));
        }
        playlist.save().expect("save");
        let leftovers: Vec<_> = std::fs::read_dir(folder.dir())
            .expect("read_dir")
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp files were left behind");
        let reread = Playlist::read(playlist.path()).expect("reread");
        assert_eq!(reread.items(), playlist.items());
    }

    #[test]
    fn save_refuses_what_m3u_cannot_carry() {
        let (_keep, folder) = folder();
        let mut playlist = folder.create("Mix").expect("create");
        playlist
            .items_mut()
            .push(Item::Entry(Entry::new("relative/path.flac")));
        assert!(matches!(
            playlist.save(),
            Err(PlaylistError::UnwritableEntry { .. })
        ));
        playlist.items_mut().clear();
        playlist
            .items_mut()
            .push(Item::Entry(Entry::new(track("/music/line\nbreak.flac"))));
        assert!(matches!(
            playlist.save(),
            Err(PlaylistError::UnwritableEntry { .. })
        ));
        playlist.items_mut().clear();
        playlist.items_mut().push(Item::Entry(Entry::new(track(
            "/music/trailing space.flac ",
        ))));
        assert!(matches!(
            playlist.save(),
            Err(PlaylistError::UnwritableEntry { .. })
        ));
        // The refusals wrote nothing: the file still parses to its created
        // state.
        let reread = Playlist::read(playlist.path()).expect("reread");
        assert_eq!(reread.entries().count(), 0);
    }

    // ----- the fingerprint -------------------------------------------------

    #[test]
    fn an_external_edit_shows_in_the_fingerprint() {
        let (_keep, folder) = folder();
        let mut playlist = folder.create("Mix").expect("create");
        playlist
            .items_mut()
            .push(Item::Entry(Entry::new(track("/music/a.flac"))));
        playlist.save().expect("save");
        let playlist = Playlist::read(playlist.path()).expect("read");
        assert!(playlist.fingerprint().is_some());
        assert!(!playlist.externally_edited());

        // The user opens it in vim and adds a track: size changes, so the
        // stamp changes even inside one mtime granule.
        let mut bytes = std::fs::read(playlist.path()).expect("read");
        bytes.extend_from_slice(b"/music/added by hand.flac\n");
        std::fs::write(playlist.path(), &bytes).expect("write");
        assert!(playlist.externally_edited());

        // Re-reading takes the new fingerprint and the new entry.
        let reread = Playlist::read(playlist.path()).expect("reread");
        assert!(!reread.externally_edited());
        assert_eq!(reread.entries().count(), 2);

        // A deleted file is an external edit, not a panic.
        std::fs::remove_file(reread.path()).expect("remove");
        assert!(reread.externally_edited());
    }

    // ----- the partition helper -------------------------------------------

    #[test]
    fn partition_applies_the_callers_verdict_and_keeps_order() {
        let (_keep, folder) = folder();
        let mut playlist = folder.create("Mix").expect("create");
        for track in [
            "/music/a.flac",
            "/gone/b.flac",
            "/music/c.flac",
            "/gone/b.flac", // a duplicate is judged each time it appears
        ] {
            playlist.items_mut().push(Item::Entry(Entry::new(track)));
        }
        let verdict = playlist.partition(|path| path.starts_with("/music"));
        let playable: Vec<&Path> = verdict.playable.iter().map(|e| e.path.as_path()).collect();
        let missing: Vec<&Path> = verdict.missing.iter().map(|e| e.path.as_path()).collect();
        assert_eq!(
            playable,
            [Path::new("/music/a.flac"), Path::new("/music/c.flac")]
        );
        assert_eq!(
            missing,
            [Path::new("/gone/b.flac"), Path::new("/gone/b.flac")]
        );
        // `38 of 40 · 2 missing` is these two numbers.
        assert_eq!(verdict.playable.len() + verdict.missing.len(), 4);
    }
}
