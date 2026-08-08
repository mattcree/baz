//! Library scanning: walk a music folder, extract per-track metadata.
//!
//! This is the front half of the "point it at a directory and play within a
//! minute" pillar: [`scan`] walks a root with `walkdir`, picks out audio
//! files by extension ([`AUDIO_EXTENSIONS`]), reads their tags with `lofty`,
//! and fills any holes from the folder structure via [`inference`] (tags win
//! over inference, field by field). Persistence, watching, and search
//! indexing are deliberately *not* here — they consume this module's output
//! in later stages.
//!
//! # Why an iterator
//!
//! [`Scan`] is a pull-based [`Iterator`] rather than a callback API: the
//! future UI populates the shelf live during a scan, and an iterator lets the
//! consumer decide the pacing — drain it on a worker thread into a channel,
//! batch entries per frame, or stop early — without this module dictating a
//! threading model or inverting control. Each `next()` does the I/O for at
//! most one directory entry, so consumption is incremental by construction.
//!
//! # Resilience
//!
//! A scan of a real library will meet unreadable directories, permission
//! holes, and corrupt files. None of these abort the scan: per-file problems
//! are *data*, reported as [`ScanEntry::Failed`] so the UI can show them,
//! while the iterator keeps going. Only failure to start at all — a missing
//! or non-directory root — is a [`ScanError`].
//!
//! A file carrying a codec baz cannot decode is neither: it is dropped, and
//! [`Scan`] says why.
//!
//! # Incremental scanning
//!
//! Re-reading every file's tags on every launch is the honest thing to do
//! exactly once. [`scan_incremental`] takes the [`FileStamp`]s the index
//! already holds ([`crate::index::Library::known_files`]) and, for any file
//! whose size *and* modification time are unchanged, reports
//! [`ScanEntry::Unchanged`] without opening it: one `stat` instead of a tag
//! parse.
//!
//! Measured over a synthetic 10 000-file library (`benches/scan.rs`, which
//! carries the full table): the scan drops from **61.2 ms to 10.3 ms**
//! (5.9×), and a whole launch — scan plus the index writes it causes — from
//! **83.4 ms to 11.6 ms** (7.2×), because an unchanged file is also a row
//! nobody rewrites. Both are lower bounds: the fixtures have no embedded
//! cover art and fit in the page cache, and neither of those costs applies
//! to the `stat` side at all.
//!
//! The stamp is deliberately (mtime, size) and not a content hash: a hash
//! would have to read every byte of every file, which is the cost the whole
//! exercise exists to avoid. The failure mode is a file rewritten in place
//! to exactly the same length *and* with its mtime restored — which is
//! something only a deliberate tool does, and which a user can always force
//! past by touching the file.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lofty::file::{FileType, TaggedFile};
use lofty::prelude::*;
use lofty::tag::{ItemKey, Tag};

use crate::replaygain::{ReplayGainReader, ReplayGainTags};

pub mod inference;

/// File extensions (ASCII case-insensitive, without the dot) the scanner
/// treats as audio. This is *the* place to extend format support.
///
/// **The list is a promise.** Every extension here is one the playback engine
/// can decode, because a shelf that lists a track and then skips it is worse
/// than one that never listed it. `.opus` is absent for exactly that reason:
/// Symphonia has no Opus decoder (see [`AudioFormat::is_decodable`]).
/// `every_advertised_extension_decodes` in
/// `crates/baz-core/tests/playback.rs` enforces the promise against real
/// encoded fixtures, so adding an extension without a decoder fails the build.
///
/// Extension is a necessary but not sufficient filter — a container can hold
/// a codec we cannot play (`.ogg` carries Vorbis, FLAC *or* Opus). The second
/// half of the promise is kept in [`Scan`], which drops files whose actual
/// codec is not decodable.
pub const AUDIO_EXTENSIONS: &[&str] = &["flac", "mp3", "ogg", "m4a", "mp4", "wav"];

/// The codec a track's samples are stored in.
///
/// This is the axis along which a multi-format collection splits into
/// *editions* (`docs/adr/0007-album-editions.md`): a collector who keeps a
/// lossless archive and a lossy copy for the phone has one album in two
/// codecs, and the shelf must say so.
///
/// Deliberately the **codec**, never the container or the folder name. A
/// library may be filed as `FLAC/…` and `MP3/…`, or may not be filed by
/// format at all; only what is inside the file is trustworthy. Every variant
/// here is a codec that one of [`AUDIO_EXTENSIONS`] actually carries —
/// anything else reads back as `None` (see [`TrackMeta::format`]) rather
/// than being guessed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioFormat {
    /// Free Lossless Audio Codec — `.flac`, or FLAC in an Ogg/MP4 container.
    Flac,
    /// Apple Lossless — `.m4a` / `.mp4`.
    Alac,
    /// Linear PCM in a RIFF WAVE container — `.wav`.
    Wav,
    /// MPEG-1/2 Audio Layer III — `.mp3`.
    Mp3,
    /// Advanced Audio Coding — `.m4a` / `.mp4`.
    Aac,
    /// Ogg Vorbis — `.ogg`.
    Vorbis,
    /// Opus — `.opus`, or Opus in an Ogg container.
    ///
    /// Recognized but **not decodable** (see [`AudioFormat::is_decodable`]),
    /// so no scan produces it today. The variant stays because it is
    /// persisted on-disk data ([`AudioFormat::code`]) that older index rows
    /// may still hold, and because it is what a future Opus decoder would
    /// switch back on.
    Opus,
}

impl AudioFormat {
    /// Whether the codec reconstructs its source samples bit-exactly.
    ///
    /// This is the primary key of the default-edition ranking
    /// ([`crate::index::Album::editions`]): a lossless edition is preferred
    /// over a lossy one whatever their bitrates say, because "lossless" is a
    /// fact about the decoded samples and "bitrate" is a fact about the
    /// file.
    #[must_use]
    pub fn is_lossless(self) -> bool {
        matches!(self, Self::Flac | Self::Alac | Self::Wav)
    }

    /// Whether [`crate::playback`] can decode this codec — i.e. whether a
    /// track in it may be put on the shelf at all.
    ///
    /// This is the codec-level half of the promise
    /// [`AUDIO_EXTENSIONS`] makes at the extension level, and it exists
    /// because the two are not the same question: an `.ogg` may hold Vorbis,
    /// FLAC *or* Opus, and only the file's own bytes say which.
    ///
    /// **Opus is the one `false`.** Symphonia demuxes Ogg Opus correctly —
    /// it parses `OpusHead`, honours the pre-skip and derives packet
    /// durations from the TOC byte — but ships no Opus *decoder* in any
    /// released version (0.5's `symphonia-codec-opus` is an empty
    /// placeholder that was never published; 0.6.0 still has none). The
    /// alternatives all cost either a bundled C library or an unproven
    /// crate on the decode path, so baz declines to list what it cannot
    /// play. `docs/BACKLOG.md` records the decision and what would reverse
    /// it.
    #[must_use]
    pub fn is_decodable(self) -> bool {
        !matches!(self, Self::Opus)
    }

    /// The stable lowercase code used to persist this format in the index
    /// (`tracks.format`, schema v2). Never change an existing code: it is
    /// on-disk data, and [`AudioFormat::from_code`] is its only reader.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Flac => "flac",
            Self::Alac => "alac",
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::Aac => "aac",
            Self::Vorbis => "vorbis",
            Self::Opus => "opus",
        }
    }

    /// Parse a [`AudioFormat::code`] back. An unrecognized code yields
    /// `None` — the same "unknown format" a never-scanned row carries — so a
    /// database holding a code this build does not know degrades to an
    /// unnamed edition instead of failing the open.
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "flac" => Some(Self::Flac),
            "alac" => Some(Self::Alac),
            "wav" => Some(Self::Wav),
            "mp3" => Some(Self::Mp3),
            "aac" => Some(Self::Aac),
            "vorbis" => Some(Self::Vorbis),
            "opus" => Some(Self::Opus),
            _ => None,
        }
    }

    /// The name a listener would recognize, for edition labels.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Flac => "FLAC",
            Self::Alac => "ALAC",
            Self::Wav => "WAV",
            Self::Mp3 => "MP3",
            Self::Aac => "AAC",
            Self::Vorbis => "Vorbis",
            Self::Opus => "Opus",
        }
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What the scanner compares to decide whether a file needs re-reading:
/// its last-modification time and its size, as the filesystem reports them.
///
/// Both halves matter. Size alone misses an edit that keeps the length;
/// mtime alone misses a filesystem with coarse timestamp granularity where
/// two writes in the same second are indistinguishable. Together they are
/// the same pair `make`, `rsync` and every backup tool in existence trust,
/// and they cost one `stat` — which the directory walk is doing anyway.
///
/// This is **not** a content hash, deliberately: hashing means reading every
/// byte of every file, which is precisely the cost incremental scanning
/// exists to avoid. A file rewritten in place to exactly its old length with
/// its old mtime restored will be missed; that is a thing only a deliberate
/// tool does, and `touch` is the user's escape hatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileStamp {
    /// Modification time in nanoseconds relative to the Unix epoch —
    /// negative for the (theoretical) pre-1970 file.
    pub mtime_ns: i64,
    /// File size in bytes.
    pub size: u64,
}

impl FileStamp {
    /// The stamp of a file from its metadata, or `None` when the platform
    /// cannot report a modification time or the time does not fit in an
    /// `i64` of nanoseconds (before 1678 or after 2262).
    ///
    /// `None` is never a claim that the file is unchanged: an unstamped file
    /// is always re-read, so an exotic filesystem degrades to the old
    /// full-rescan behaviour rather than to a stale library.
    #[must_use]
    pub fn of(metadata: &std::fs::Metadata) -> Option<Self> {
        let mtime_ns = match metadata.modified().ok()?.duration_since(UNIX_EPOCH) {
            Ok(since) => i64::try_from(since.as_nanos()).ok()?,
            Err(before) => i64::try_from(before.duration().as_nanos())
                .ok()?
                .checked_neg()?,
        };
        Some(Self {
            mtime_ns,
            size: metadata.len(),
        })
    }

    /// The stamp of the file at `path`, or `None` if it cannot be stat'ed or
    /// its timestamp is unrepresentable (see [`FileStamp::of`]). Symlinks are
    /// followed, matching what reading the file would do.
    #[must_use]
    pub fn of_path(path: &Path) -> Option<Self> {
        Self::of(&std::fs::metadata(path).ok()?)
    }

    /// The `SystemTime` this stamp's `mtime_ns` denotes, for tests and for
    /// any caller that wants to restore a timestamp.
    #[must_use]
    pub fn modified(self) -> SystemTime {
        let magnitude = Duration::from_nanos(self.mtime_ns.unsigned_abs());
        if self.mtime_ns < 0 {
            UNIX_EPOCH - magnitude
        } else {
            UNIX_EPOCH + magnitude
        }
    }
}

/// What the index holds about one file it already knows: the stamp a scan
/// compares, and the **library root the row was recorded under**.
///
/// The two are read by the two halves of a scan and neither is guessed at:
///
/// - `stamp` is [`scan_incremental`]'s input. `None` — a row written before
///   stamps existed (schema v4), or a file whose filesystem could not report a
///   usable timestamp — is never a claim of freshness; such a file is always
///   re-read.
/// - `root` is the removal pass's. It is the root whose walk last *read* this
///   file, recorded in the index (schema v8), and it replaces the
///   `starts_with(root_being_scanned)` test the multi-root gate used to make
///   (`docs/adr/0022-library-roots-and-refresh.md`). `None` — a row written
///   before roots existed that no rescan has refreshed, or one added by a
///   caller that named no root — belongs to no root, so no root's scan may
///   ever prune it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownFile {
    /// The size and modification time recorded for the file.
    pub stamp: Option<FileStamp>,
    /// The library root the row is recorded under. Shared rather than copied:
    /// a library has a handful of roots and a hundred thousand rows.
    pub root: Option<Arc<Path>>,
}

impl KnownFile {
    /// A file known by its stamp alone, belonging to no recorded root — what
    /// every row in a pre-v8 index looks like, and what a caller that names no
    /// root produces.
    #[must_use]
    pub fn stamped(stamp: Option<FileStamp>) -> Self {
        Self { stamp, root: None }
    }

    /// A file known by its stamp and the root it was found under.
    #[must_use]
    pub fn new(stamp: Option<FileStamp>, root: Option<Arc<Path>>) -> Self {
        Self { stamp, root }
    }
}

/// Every path the index already knows, with the [`KnownFile`] recorded for it.
///
/// This is what [`scan_incremental`] consults to skip unchanged files, and
/// what a removal pass uses to enumerate the rows a scan did not see — and, in
/// the same lookup, to check that the root doing the pruning is the root that
/// put the row there.
pub type KnownFiles = HashMap<PathBuf, KnownFile>;

/// Metadata for one audio file, as the indexer and shelf UI will consume it.
///
/// Every descriptive field is optional: files are the source of truth, and
/// when neither tags nor folder structure provide a value we report "unknown"
/// rather than invent one. Presentation choices ("Unknown Artist", filename
/// fallbacks) belong to the consumer.
///
/// The last four fields describe the *encoding* rather than the work. They
/// all come from the audio-property header lofty parses during the same read
/// as the tags — no extra I/O, no decoding — and they exist for one purpose:
/// splitting an album into editions and ranking them (see [`AudioFormat`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMeta {
    /// Absolute path of the audio file (the file's identity; never inferred).
    pub path: PathBuf,
    /// Track artist, from tags or else the grandparent directory.
    pub artist: Option<String>,
    /// Album artist — who the *album* is filed under, as distinct from who
    /// performs an individual track. This is the tag that keeps a
    /// soundtrack or a compilation from shattering into one shelf entry per
    /// composer (`docs/adr/0008-album-artist-grouping.md`), read from
    /// whichever spelling the container uses: Vorbis `ALBUMARTIST` (and the
    /// non-standard `ALBUM ARTIST`), `ID3v2` `TPE2`, MP4 `aART`, APE
    /// `Album Artist`.
    ///
    /// `None` means the file did not say. It is **not** a claim that the
    /// album artist equals the track artist — that fallback is a grouping
    /// decision ([`crate::index::AlbumArtist`]), made where the album's
    /// whole track list is visible, not guessed at here.
    ///
    /// Folder inference fills this exactly when it fills
    /// [`TrackMeta::artist`] — from the grandparent directory of an
    /// `Artist/Album/track` layout — so a folder-organised library with no
    /// tags at all still groups correctly.
    pub album_artist: Option<String>,
    /// The file's compilation flag: `ID3v2` `TCMP`, MP4 `cpil`, Vorbis
    /// `COMPILATION`, APE `Compilation`.
    ///
    /// It earns its place by making one grouping case decidable that
    /// nothing else can decide: tracks by *different* artists that belong
    /// to one album and carry no album-artist tag. Without a signal, two
    /// artists who each released a "Greatest Hits" would merge into one
    /// shelf entry; with it, a flagged compilation groups under
    /// [`crate::index::AlbumArtist::Various`] instead.
    ///
    /// `None` is "the file said nothing", distinct from `Some(false)`.
    pub compilation: Option<bool>,
    /// Album title, from tags or else the parent directory.
    pub album: Option<String>,
    /// Genre, **exactly as the file spells it** — Vorbis `GENRE`, ID3v2
    /// `TCON`, MP4 `©gen`, APE `Genre`.
    ///
    /// Verbatim is the whole specification (`docs/adr/0018-group-keys.md`,
    /// `docs/design/critique/02-surfaces.md`): no normalisation, no mapping
    /// table, no splitting on `;` or `/`, no title-casing. A library that
    /// carries `Post-Rock`, `post rock` and `Rock; Instrumental` shows three
    /// genres, because it *has* three genre tags, and the GENRE group key
    /// exists to let a listener see that and fix it in their tagger — the
    /// library is a cache of what the files say, not a place we improve them
    /// (`docs/research/05-personas.md`, principle 4).
    ///
    /// Never inferred from the folder structure: a directory name is evidence
    /// about artist and album (people file by those) and evidence about
    /// nothing else.
    ///
    /// `None` means the file did not say.
    pub genre: Option<String>,
    /// Track title, from tags or else the filename.
    pub title: Option<String>,
    /// Track number within the disc, from tags or else the filename.
    pub track: Option<u32>,
    /// Disc number, from tags only (folder layouts rarely encode it reliably).
    pub disc: Option<u32>,
    /// Release year, from tags only.
    pub year: Option<u32>,
    /// Playing time, when the format headers make it cheaply available
    /// during the tag read (no decoding is performed).
    pub duration: Option<Duration>,
    /// Codec the samples are stored in — the edition axis.
    ///
    /// `None` means "not known": a codec [`AudioFormat`] does not name, or an
    /// index row written before schema v2 that no rescan has refreshed yet.
    /// It never means "no format".
    pub format: Option<AudioFormat>,
    /// Bits per sample. Lossless codecs declare one; MP3/AAC/Vorbis/Opus do
    /// not — their internal precision is not a sample width — so `None` there
    /// is the truth, not a gap.
    pub bit_depth: Option<u8>,
    /// Sample rate in Hz.
    pub sample_rate: Option<u32>,
    /// Audio bitrate in kbit/s as declared or derived by the container; an
    /// average for VBR encodings.
    pub bitrate: Option<u32>,
    /// The file's size and modification time as of this read — what the next
    /// scan compares to decide whether the tags above are still current (see
    /// [`FileStamp`] and [`scan_incremental`]).
    ///
    /// `None` means "not known": a filesystem that could not report a usable
    /// timestamp, or an index row written before schema v4. Such a file is
    /// always re-read, which is exactly the pre-v4 behaviour.
    ///
    /// It describes the *file*, not the work — like the four encoding fields
    /// above, and unlike them it is not read from the file's contents at all.
    pub stamp: Option<FileStamp>,
    /// The ReplayGain figures the file already carries, if any (ADR-0013).
    ///
    /// **What the file said**, and only that: baz honours
    /// `REPLAYGAIN_TRACK_GAIN` and its siblings where a scanner has written
    /// them, from Vorbis comments, ID3v2 `TXXX` frames, MP4 freeform atoms and
    /// APE items alike — plus the Opus-style `R128_*` integer form where that
    /// is all a file has.
    ///
    /// Figures **baz measured itself** (ADR-0015) are deliberately *not* here.
    /// This struct is what reading a file's tags yields, a scan is the only
    /// thing that builds one, and a scan cannot measure loudness; a measurement
    /// lives beside the row in the index
    /// ([`Library::computed_replay_gain`](crate::index::Library::computed_replay_gain)),
    /// which is also what makes a rescan structurally unable to destroy one.
    ///
    /// All-`None` ([`ReplayGainTags::is_empty`]) is the ordinary state of a
    /// library nothing has ever scanned, and it means "the file did not say" —
    /// never "this track needs no gain". What the engine does with that is
    /// [`ReplayGainSettings::resolve_with`](crate::replaygain::ReplayGainSettings::resolve_with)'s
    /// no-figure rule.
    ///
    /// Integers rather than floats, for the three reasons
    /// [`crate::replaygain`] gives — one of which is that this struct keeps its
    /// `Eq`.
    pub replay_gain: ReplayGainTags,
}

/// One result from a running scan: a successfully read track, or a per-file
/// failure reported as data (see the module docs on resilience).
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::large_enum_variant,
    reason = "Track is the overwhelmingly common variant — one per audio file, \
              where Failed and Unchanged are the exceptions — and the value is \
              yielded, matched and consumed immediately rather than stored in \
              bulk. Boxing it to shrink the rare variants would put a heap \
              allocation on the scan's per-file path to save nothing that is \
              ever kept."
)]
pub enum ScanEntry {
    /// An audio file that was read successfully.
    Track(TrackMeta),
    /// An audio file the index already holds whose [`FileStamp`] is
    /// unchanged, so its tags were **not** re-read (see
    /// [`scan_incremental`]).
    ///
    /// It is reported rather than silently dropped because "we looked at
    /// this file and it is still there" is exactly the fact a removal pass
    /// needs: a path that produced no entry at all is a path the scan may
    /// simply never have reached.
    Unchanged {
        /// The file that was skipped.
        path: PathBuf,
    },
    /// A file or directory the scanner could not process. The scan continues
    /// past it; the consumer decides whether and how to surface it.
    Failed {
        /// The file or directory that failed.
        path: PathBuf,
        /// Human-readable cause (I/O error, unparsable container, …). Kept as
        /// a string because it exists to be shown and logged, not matched on.
        reason: String,
    },
}

/// A scan could not start. Per-file problems are never this — they are
/// [`ScanEntry::Failed`] items in the stream.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// The scan root does not exist.
    #[error("scan root `{}` does not exist", path.display())]
    RootNotFound {
        /// The root that was requested.
        path: PathBuf,
    },
    /// The scan root exists but is not a directory.
    #[error("scan root `{}` is not a directory", path.display())]
    RootNotDirectory {
        /// The root that was requested.
        path: PathBuf,
    },
}

/// Start scanning `root` for audio files.
///
/// Returns an iterator over [`ScanEntry`] — see the [module docs](self) for
/// why the API is shaped this way. Directory traversal order is unspecified.
///
/// # Errors
///
/// [`ScanError::RootNotFound`] if `root` does not exist, and
/// [`ScanError::RootNotDirectory`] if it is not a directory. Everything after
/// that is reported in-stream as [`ScanEntry::Failed`].
pub fn scan(root: impl AsRef<Path>) -> Result<Scan<'static>, ScanError> {
    start(root.as_ref(), None)
}

/// Start scanning `root`, reusing what the index already knows.
///
/// Identical to [`scan`] except that a file whose [`FileStamp`] matches the
/// one recorded in `known` is reported as [`ScanEntry::Unchanged`] instead
/// of being opened and re-tagged. Everything else — new files, files whose
/// size or mtime moved, files `known` has no stamp for — is read exactly as
/// [`scan`] reads it, so the two produce the same library from the same
/// disk; only the work differs.
///
/// # Errors
///
/// The same two as [`scan`]: [`ScanError::RootNotFound`] and
/// [`ScanError::RootNotDirectory`].
pub fn scan_incremental(root: impl AsRef<Path>, known: &KnownFiles) -> Result<Scan<'_>, ScanError> {
    start(root.as_ref(), Some(known))
}

/// The body [`scan`] and [`scan_incremental`] share.
fn start<'a>(root: &Path, known: Option<&'a KnownFiles>) -> Result<Scan<'a>, ScanError> {
    let root = root.to_path_buf();
    if !root.exists() {
        return Err(ScanError::RootNotFound { path: root });
    }
    if !root.is_dir() {
        return Err(ScanError::RootNotDirectory { path: root });
    }
    let walker = walkdir::WalkDir::new(&root).into_iter();
    Ok(Scan {
        root,
        walker,
        known,
    })
}

/// A running scan; see [`scan`].
///
/// # What a scan leaves out
///
/// Two filters, in this order, and both are about the promise
/// [`AUDIO_EXTENSIONS`] makes:
///
/// 1. **Extension** — anything not in [`AUDIO_EXTENSIONS`] is not audio as
///    far as baz is concerned, and never touched.
/// 2. **Codec** — a file whose extension passed but whose *bytes* turn out to
///    carry a codec the engine cannot decode
///    ([`AudioFormat::is_decodable`]) is dropped rather than listed. Today
///    that is exactly Ogg Opus arriving as `.ogg` — the one case where an
///    accepted container can hold an unplayable codec. It is not a
///    [`ScanEntry::Failed`]: nothing failed, and a listener with a folder of
///    Opus files does not want a wall of red.
///
/// The codec filter costs no extra I/O — the codec comes from the same lofty
/// read that produced the tags.
///
/// A third filter applies only to [`scan_incremental`]: a file whose
/// [`FileStamp`] matches the index's is reported as
/// [`ScanEntry::Unchanged`] and never opened.
#[derive(Debug)]
pub struct Scan<'a> {
    root: PathBuf,
    walker: walkdir::IntoIter,
    /// What the index already holds, when this is an incremental scan.
    known: Option<&'a KnownFiles>,
}

impl Iterator for Scan<'_> {
    type Item = ScanEntry;

    fn next(&mut self) -> Option<ScanEntry> {
        loop {
            let entry = match self.walker.next()? {
                Ok(entry) => entry,
                Err(err) => {
                    // Unreadable directory or similar: report it and keep
                    // walking the rest of the tree.
                    let path = err
                        .path()
                        .map_or_else(|| self.root.clone(), Path::to_path_buf);
                    return Some(ScanEntry::Failed {
                        path,
                        reason: err.to_string(),
                    });
                }
            };
            if !entry.file_type().is_file() || !has_audio_extension(entry.path()) {
                continue;
            }
            // One `stat` — the whole cost of the incremental check, and the
            // value the row carries forward so the *next* scan can make it.
            // A file we cannot stat has no stamp and is simply read.
            let stamp = entry.metadata().ok().and_then(|m| FileStamp::of(&m));
            let path = entry.into_path();
            if self.is_unchanged(&path, stamp) {
                return Some(ScanEntry::Unchanged { path });
            }
            // `None` is a playable-container-with-unplayable-codec (Ogg
            // Opus in a `.ogg`): not an entry, not a failure. See the type
            // docs.
            if let Some(entry) = read_track(&self.root, path, stamp) {
                return Some(entry);
            }
        }
    }
}

impl Scan<'_> {
    /// Whether this file's tags can be taken from the index unread: an
    /// incremental scan, a path the index knows, a stamp recorded for it,
    /// and a stamp that still matches what is on disk. Any missing link
    /// means a full read.
    fn is_unchanged(&self, path: &Path, stamp: Option<FileStamp>) -> bool {
        let (Some(known), Some(stamp)) = (self.known, stamp) else {
            return false;
        };
        known.get(path).and_then(|known| known.stamp) == Some(stamp)
    }
}

/// Whether `path` is **positively confirmed** to no longer exist — the one
/// question a caller may delete an index row on.
///
/// Two conditions, both required:
///
/// 1. **The parent directory is present and is a directory.** An absent
///    parent is not evidence: an unmounted NAS, an unplugged drive and a
///    deleted folder are indistinguishable from below, and `stat` on a file
///    under a missing directory returns `NotFound` for all three. Requiring
///    the parent is what makes "the mount is not here today" cost nothing.
/// 2. **`symlink_metadata` on the file itself fails with `NotFound`.** Any
///    other outcome keeps the row: the file existing obviously does, and so
///    does a permission error or an I/O error, because those say the
///    filesystem would not answer — not that the answer is "gone".
///
/// `symlink_metadata` rather than `metadata` on purpose: a *broken symlink*
/// is a file that exists (the link does), and deleting the row for one would
/// be deleting a row for something still on disk.
///
/// The residual cost of rule 1 is stated plainly: deleting an entire album
/// *folder* leaves its rows in the index, because nothing on the filesystem
/// distinguishes that from the folder being a mount point that is not
/// mounted right now. Keeping a stale row is a cosmetic bug; deleting a
/// present listener's library is not, and `docs/BACKLOG.md` carries the gap.
#[must_use]
pub fn is_confirmed_gone(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if !parent.is_dir() {
        return false;
    }
    matches!(
        std::fs::symlink_metadata(path),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound
    )
}

/// Does the path carry one of [`AUDIO_EXTENSIONS`] (ASCII case-insensitive)?
fn has_audio_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(ext))
        })
}

/// Read one audio file: tags via lofty, holes filled by path inference.
///
/// `None` means "this file is not for us": the container was one we scan but
/// the codec inside is one the engine cannot decode
/// ([`AudioFormat::is_decodable`]). The caller drops it silently — see the
/// [`Scan`] docs for why that is not a [`ScanEntry::Failed`]. A file whose
/// format lofty could not identify at all is *kept*: an honest "unknown
/// format" row for something we can very likely still play beats dropping a
/// track on a guess.
fn read_track(root: &Path, path: PathBuf, stamp: Option<FileStamp>) -> Option<ScanEntry> {
    match read_tagged_file(&path) {
        Ok(file) => {
            let meta = build_meta(root, path, &file, stamp);
            match meta.format {
                Some(format) if !format.is_decodable() => None,
                _ => Some(ScanEntry::Track(meta)),
            }
        }
        Err(err) => Some(ScanEntry::Failed { path, reason: err }),
    }
}

/// Read one file's tags and audio properties, identifying it **by content**.
///
/// Deliberately not `lofty::read_from_path`, which — as its own documentation
/// says — picks a parser from the file *extension* alone. That is wrong twice
/// over for a music library:
///
/// - `.ogg` is a container, not a codec. lofty maps the extension to Vorbis,
///   so an Ogg Opus file named `.ogg` (which encoders do emit) came back as
///   "Vorbis: File missing magic signature" — a scan failure blaming the file
///   for baz's mis-guess, and, worse, one that hid the fact the file is Opus
///   and therefore unplayable.
/// - A mislabelled file — a FLAC named `.mp3`, which real libraries contain —
///   failed to parse at all instead of simply being read.
///
/// `guess_file_type` inspects the first 36 bytes and overrides the extension
/// when the content is conclusive; when it is not, the extension guess stands,
/// so nothing that used to work stops working. The cost is 36 bytes of a read
/// that was going to happen anyway.
fn read_tagged_file(path: &Path) -> Result<TaggedFile, String> {
    lofty::probe::Probe::open(path)
        .map_err(|e| e.to_string())?
        .guess_file_type()
        .map_err(|e| e.to_string())?
        .read()
        .map_err(|e| e.to_string())
}

/// Merge tag data with folder-structure inference. Tags win field by field;
/// inference only fills what tags leave blank.
fn build_meta(
    root: &Path,
    path: PathBuf,
    file: &TaggedFile,
    stamp: Option<FileStamp>,
) -> TrackMeta {
    // Inference must only ever see the part of the path below the scan root,
    // so directories the user did not choose to scan cannot leak in.
    let relative = path.strip_prefix(root).unwrap_or(&path);
    let inferred = inference::infer_from_relative_path(relative);

    let tag = file.primary_tag().or_else(|| file.first_tag());
    let (artist, album_artist, compilation, album, genre, title, track, disc, year) = match tag {
        Some(tag) => (
            tag.artist().as_deref().and_then(clean_str),
            album_artist(tag),
            compilation_flag(tag),
            tag.album().as_deref().and_then(clean_str),
            // Verbatim: `clean_str` only decides present-or-absent (a blank
            // tag is not a genre), and changes no character of a value it
            // keeps beyond the surrounding whitespace. See
            // [`TrackMeta::genre`].
            tag.genre().as_deref().and_then(clean_str),
            tag.title().as_deref().and_then(clean_str),
            nonzero(tag.track()),
            nonzero(tag.disk()),
            nonzero(tag.year()),
        ),
        None => (None, None, None, None, None, None, None, None, None),
    };

    let properties = file.properties();
    let duration = properties.duration();

    // Inference fills the album artist on exactly the terms it fills the
    // track artist: only when the tags left *both* blank. A file that names
    // its artist but not its album artist is trusted over its folder — the
    // "Beatles/" directory must not overrule an `ARTIST=The Beatles` tag —
    // and the album-artist fallback chain (`crate::index::AlbumArtist`)
    // reaches that tag anyway.
    let inferred_artist = inferred.artist;
    let album_artist = album_artist.or_else(|| {
        if artist.is_none() {
            inferred_artist.clone()
        } else {
            None
        }
    });

    TrackMeta {
        artist: artist.or(inferred_artist),
        album_artist,
        compilation,
        album: album.or(inferred.album),
        // No inference: a folder name is evidence about who made a record and
        // what it is called, and evidence about nothing else.
        genre,
        title: title.or(inferred.title),
        track: track.or(inferred.track),
        disc,
        year,
        duration: (!duration.is_zero()).then_some(duration),
        format: detect_format(file),
        bit_depth: properties.bit_depth(),
        sample_rate: nonzero(properties.sample_rate()),
        bitrate: nonzero(properties.audio_bitrate()),
        stamp,
        replay_gain: tag.map_or_else(ReplayGainTags::default, replay_gain),
        path,
    }
}

/// The ReplayGain figures a tag carries, whatever the container calls them.
///
/// Two passes, because lofty splits the keys into two worlds and both matter:
///
/// 1. **The mapped keys.** lofty already understands the four standard
///    spellings across every container baz reads — Vorbis
///    `REPLAYGAIN_TRACK_GAIN`, ID3v2 `TXXX:REPLAYGAIN_TRACK_GAIN`, MP4
///    `----:com.apple.iTunes:replaygain_track_gain`, APE
///    `REPLAYGAIN_TRACK_GAIN` — and folds them onto one [`ItemKey`]. Asking for
///    the `ItemKey` is therefore the whole of the container-specific work, and
///    the canonical name is handed to the parser alongside the value.
/// 2. **The unmapped keys**, for the one form no standard blesses and lofty
///    therefore leaves as [`ItemKey::Unknown`]: the Opus-style `R128_TRACK_GAIN`
///    integer, which turns up in Vorbis comments on FLAC and Ogg files written
///    by R128-era tools even though `.opus` itself is not scanned.
///
/// Recognition, parsing and precedence all live in [`crate::replaygain`], so a
/// file means the same thing to the scanner as it does to the playback path —
/// which reads the same tags through Symphonia rather than lofty.
fn replay_gain(tag: &Tag) -> ReplayGainTags {
    const MAPPED: [(&str, ItemKey); 4] = [
        ("REPLAYGAIN_TRACK_GAIN", ItemKey::ReplayGainTrackGain),
        ("REPLAYGAIN_TRACK_PEAK", ItemKey::ReplayGainTrackPeak),
        ("REPLAYGAIN_ALBUM_GAIN", ItemKey::ReplayGainAlbumGain),
        ("REPLAYGAIN_ALBUM_PEAK", ItemKey::ReplayGainAlbumPeak),
    ];
    let mut reader = ReplayGainReader::default();
    for (name, key) in MAPPED {
        if let Some(value) = tag.get_string(&key) {
            reader.absorb(name, value);
        }
    }
    for item in tag.items() {
        if let ItemKey::Unknown(key) = item.key()
            && let Some(value) = item.value().text()
        {
            reader.absorb(key, value);
        }
    }
    reader.finish()
}

/// Which codec a file carries, from what lofty already parsed — which is a
/// fact about the file's *bytes*, because [`read_tagged_file`] identifies it
/// by content rather than by extension.
///
/// Most containers answer this outright. MP4 is the one ambiguous case:
/// `.m4a`/`.mp4` may hold ALAC or AAC, and the format-agnostic
/// `lofty::properties::FileProperties` that a `TaggedFile` exposes has
/// dropped the container-specific `Mp4Codec` by the time we see it. The
/// surviving discriminator is the declared bit depth: lofty fills it from
/// the `alac` sample-description atom and leaves it unset for AAC, which has
/// no sample width to declare. The one known false positive is FLAC-in-MP4
/// (which also declares a depth); it lands on [`AudioFormat::Alac`] — a
/// wrong *name* but the right fidelity tier, and a combination essentially
/// absent from real libraries.
///
/// Anything else — AIFF, APE, `WavPack`, Musepack, Speex, a codec lofty grows
/// later — is reported as `None`. The scanner only walks
/// [`AUDIO_EXTENSIONS`], so those arrive only from a mislabeled file, and an
/// honest "unknown" beats a guess. `None` is also what keeps such a file *on*
/// the shelf: only a positively-identified undecodable codec is dropped
/// ([`AudioFormat::is_decodable`]).
fn detect_format(file: &TaggedFile) -> Option<AudioFormat> {
    match file.file_type() {
        FileType::Flac => Some(AudioFormat::Flac),
        FileType::Wav => Some(AudioFormat::Wav),
        FileType::Mpeg => Some(AudioFormat::Mp3),
        FileType::Aac => Some(AudioFormat::Aac),
        FileType::Vorbis => Some(AudioFormat::Vorbis),
        FileType::Opus => Some(AudioFormat::Opus),
        FileType::Mp4 => Some(if file.properties().bit_depth().is_some() {
            AudioFormat::Alac
        } else {
            AudioFormat::Aac
        }),
        _ => None,
    }
}

/// The album-artist tag, whatever the container calls it.
///
/// lofty's [`ItemKey::AlbumArtist`] already covers the mappings that matter
/// — Vorbis `ALBUMARTIST`, `ID3v2` `TPE2`, MP4 `aART`, APE `Album Artist` —
/// so the common case is one lookup. The second pass exists for the one
/// spelling real files use that no standard blesses and lofty therefore
/// leaves as an unmapped key: Vorbis comments written `ALBUM ARTIST` (or
/// `Album_Artist`) by older taggers. Punctuation-insensitive matching is
/// confined to that fallback, so a mapped tag is never second-guessed.
fn album_artist(tag: &Tag) -> Option<String> {
    if let Some(name) = tag.get_string(&ItemKey::AlbumArtist).and_then(clean_str) {
        return Some(name);
    }
    tag.items()
        .filter(|item| matches!(item.key(), ItemKey::Unknown(key) if is_album_artist_key(key)))
        .find_map(|item| item.value().text().and_then(clean_str))
}

/// Whether an unmapped tag key is a spelling of "album artist" — spaces and
/// underscores ignored, ASCII case ignored.
fn is_album_artist_key(key: &str) -> bool {
    let squashed: String = key.chars().filter(|c| !matches!(c, ' ' | '_')).collect();
    squashed.eq_ignore_ascii_case("albumartist")
}

/// The compilation flag, if the file sets one. See
/// [`TrackMeta::compilation`] for why it is read at all.
fn compilation_flag(tag: &Tag) -> Option<bool> {
    tag.get_string(&ItemKey::FlagCompilation)
        .and_then(parse_flag)
}

/// Parse a boolean tag value. Containers disagree on the spelling — MP4
/// stores a real boolean that lofty surfaces as `"1"`/`"0"`, `ID3v2` `TCMP`
/// and Vorbis `COMPILATION` are free text — so the spellings actually found
/// in the wild are accepted and anything else is `None` rather than
/// silently false.
fn parse_flag(value: &str) -> Option<bool> {
    let value = value.trim();
    if value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes") {
        return Some(true);
    }
    if value == "0" || value.eq_ignore_ascii_case("false") || value.eq_ignore_ascii_case("no") {
        return Some(false);
    }
    None
}

/// Messy-library hygiene: a tag field that is present but blank (or
/// whitespace) counts as absent, so inference can fill it.
fn clean_str(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Numeric tag hygiene: `0` conventionally means "unset" for track/disc/year.
fn nonzero(value: Option<u32>) -> Option<u32> {
    value.filter(|&n| n != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_extension_matching_is_case_insensitive() {
        assert!(has_audio_extension(Path::new("a/b/track.FLAC")));
        assert!(has_audio_extension(Path::new("track.Mp3")));
        assert!(has_audio_extension(Path::new("track.wav")));
        assert!(!has_audio_extension(Path::new("cover.jpg")));
        assert!(!has_audio_extension(Path::new("no_extension")));
        assert!(!has_audio_extension(Path::new(".flac"))); // dotfile, no ext
    }

    #[test]
    fn format_codes_roundtrip_and_classify_fidelity() {
        let all = [
            AudioFormat::Flac,
            AudioFormat::Alac,
            AudioFormat::Wav,
            AudioFormat::Mp3,
            AudioFormat::Aac,
            AudioFormat::Vorbis,
            AudioFormat::Opus,
        ];
        for format in all {
            assert_eq!(
                AudioFormat::from_code(format.code()),
                Some(format),
                "{format} must survive a round trip through its stored code"
            );
        }
        // Codes are unique, or two formats would collide in the database.
        let mut codes: Vec<&str> = all.iter().map(|f| f.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), all.len());

        assert!(AudioFormat::Flac.is_lossless());
        assert!(AudioFormat::Alac.is_lossless());
        assert!(AudioFormat::Wav.is_lossless());
        assert!(!AudioFormat::Mp3.is_lossless());
        assert!(!AudioFormat::Aac.is_lossless());
        assert!(!AudioFormat::Vorbis.is_lossless());
        assert!(!AudioFormat::Opus.is_lossless());

        // A code from a future baz degrades to "unknown", never an error.
        assert_eq!(AudioFormat::from_code("wavpack"), None);
        assert_eq!(AudioFormat::from_code(""), None);
        // Codes are case-sensitive by construction; we only ever write ours.
        assert_eq!(AudioFormat::from_code("FLAC"), None);
        assert_eq!(AudioFormat::Flac.to_string(), "FLAC");
    }

    #[test]
    fn blank_and_zero_tag_values_count_as_absent() {
        assert_eq!(clean_str("  "), None);
        assert_eq!(clean_str(" x "), Some("x".to_owned()));
        assert_eq!(clean_str(""), None);
        assert_eq!(nonzero(Some(0)), None);
        assert_eq!(nonzero(Some(7)), Some(7));
    }

    #[test]
    fn non_standard_album_artist_spellings_are_recognized() {
        for spelling in [
            "ALBUM ARTIST",
            "Album Artist",
            "album artist",
            "ALBUM_ARTIST",
            "AlbumArtist",
        ] {
            assert!(
                is_album_artist_key(spelling),
                "{spelling} is an album-artist key in the wild"
            );
        }
        // Neighbouring keys must not be swallowed.
        for other in ["ALBUMARTISTSORT", "ALBUM ARTISTS", "ARTIST", "ALBUM", ""] {
            assert!(!is_album_artist_key(other), "{other} is a different tag");
        }
    }

    #[test]
    fn compilation_flag_spellings() {
        assert_eq!(parse_flag("1"), Some(true));
        assert_eq!(parse_flag(" true "), Some(true));
        assert_eq!(parse_flag("Yes"), Some(true));
        assert_eq!(parse_flag("0"), Some(false));
        assert_eq!(parse_flag("FALSE"), Some(false));
        assert_eq!(parse_flag("no"), Some(false));
        // Anything else is "the file did not say", never a silent false.
        assert_eq!(parse_flag(""), None);
        assert_eq!(parse_flag("2"), None);
        assert_eq!(parse_flag("maybe"), None);
    }
}
