# ADR-0010: Incremental scanning, and what proves a file is gone

**Status**: accepted (2026-08-07)

## Context

Two entries in `docs/BACKLOG.md` describe the same missing half of library
hygiene, from opposite ends:

- **"A full rescan runs on every launch."** baz re-opened and re-tag-parsed
  every file in the music folder at every start. On the owner's 37 tracks
  that is free. On the 40 000 Marta keeps (`docs/research/05-personas.md`),
  or the 100 000 the search index is built for, it is a startup tax paid
  forever for a folder that usually has not changed at all.
- **"Deleted files linger in the index."** `add_tracks` is upsert-only, so
  nothing has ever removed a row. A file deleted on disk stays on the shelf
  until the database is deleted by hand — which is exactly what happened
  when an agent's test fixtures were once upserted into the maintainer's
  live library.

They share a mechanism: both need the scan to know what the index already
holds, rather than treating every launch as the first one.

The removal half is the dangerous one, and it is dangerous asymmetrically.
A stale row is a cosmetic wrong that a rescan fixes. A wrongly deleted row
is *the user's library*, gone, with the file still on disk — and the obvious
implementation ("delete everything this scan did not see") destroys a
library every time a NAS is not mounted, a drive is unplugged, a permission
changes, or the user points baz at a different folder for a moment.

## Decision

### 1. The stamp: (mtime, size), not a hash

`TrackMeta` gains `stamp: Option<FileStamp>`, a pair of `i64` nanoseconds
since the Unix epoch and a `u64` byte count, taken from the `std::fs::Metadata`
the directory walk already had to fetch. `scan_incremental(root, known)`
compares it against what the index recorded and, on a match, emits
`ScanEntry::Unchanged { path }` without opening the file.

Both halves are kept. Size alone misses an edit that preserves length;
mtime alone misses two writes inside one tick of a coarse-granularity
filesystem. Together they are the pair `make`, `rsync` and every backup tool
in existence trust.

A content hash would be strictly more accurate and strictly self-defeating:
hashing means reading every byte of every file, which is the cost the whole
exercise exists to avoid. The residual failure mode is a file rewritten in
place to exactly its old length with its old mtime restored — something only
a deliberate tool does, and `touch` is the escape hatch.

`None` is never a claim of freshness. A file whose timestamp the platform
cannot report, or cannot represent in `i64` nanoseconds (before 1678, after
2262), is simply always read. So is a row from before schema v4.

### 2. What it costs, measured

`crates/baz-core/benches/scan.rs` builds 10 000 tagged WAVs and runs both
passes over them. Development host, warm page cache, release build:

| measurement | 10 000 files | per file |
|---|---|---|
| `scan/cold_10k` | 61.2 ms | 6.1 µs |
| `scan/warm_10k` | 10.3 ms | 1.0 µs |
| `scan/launch_cold_10k` (scan + index writes) | 83.4 ms | 8.3 µs |
| `scan/launch_warm_10k` (scan + index writes) | 11.6 ms | 1.2 µs |

**5.9× on the scan, 7.2× on the launch.** The launch ratio is larger than
the scan ratio because an unchanged file is also a row nobody rewrites: the
warm pass performs zero `add_tracks`, zero SQLite transactions and zero
view-model rebuilds, where the cold pass upserted every track in the library
on every start.

Both numbers are reported as **lower bounds**, deliberately. The fixtures
carry no embedded cover art and fit entirely in the page cache; real FLACs
and MP4s hold a JPEG inside the tag block that lofty parses, and a
100k-track library fits in nobody's cache. Neither cost applies to the
`stat` side. How much wider the gap is on a real collection is a question
only a real collection answers, and this ADR does not guess at it.

### 3. Removal: positive confirmation, four gates

Rows are **never** deleted for being unseen. A path must clear all four of
these, and each exists because of a specific way "I did not see it" is not
"it is not there":

1. **The walk saw something.** A scan that produced no entry whatsoever
   prunes nothing. This is what an unmounted share looks like when the mount
   point survives as an empty directory — the single case where every other
   check would pass for every row at once.
2. **The path is under the root just scanned.** The index may hold several
   roots: the user re-pointed baz at another folder, or (as happened) an
   agent's fixtures landed rows outside the music tree. Scanning one root is
   silence about every other.
3. **No ancestor the walk failed on.** `walkdir` already reports an
   unreadable directory as `ScanEntry::Failed`; everything below such a
   directory was never looked at, so a scan that failed partway deletes
   nothing it could not reach.
4. **The filesystem confirms it** — `library::is_confirmed_gone`: the file's
   parent directory is present *and* `symlink_metadata` on the file itself
   fails with `NotFound`. `symlink_metadata`, not `metadata`, so a broken
   symlink counts as a file that exists — the link does. Any other error
   keeps the row: a permission error says the filesystem would not answer,
   not that the answer is "gone".

Requiring the *parent directory* (gate 4) is what makes an absent mount cost
nothing, because a missing directory answers `NotFound` for every path below
it whether those files were deleted or merely unplugged.

**The price is stated rather than hidden**: deleting an entire album
*folder* leaves its rows in the index, because from the filesystem's side
that is indistinguishable from the folder being a mount point that is not
mounted right now. Deleting individual files works; deleting directories
does not, yet. That asymmetry is the deliberate choice — a stale row is
cosmetic, a deleted library is not — and `docs/BACKLOG.md` carries the
remaining case together with what would settle it (a signal that separates
"deleted" from "not mounted": remembered mount points, or an explicit,
user-initiated prune that shows what it is about to remove).

The mechanics keep policy and effect apart. `Library::remove_tracks` is
deliberately dumb: it deletes exactly the paths it is handed and decides
nothing. Every judgement lives in `scan::vanished`, tested against real
temporary directories, and the worker can only ever nominate paths the
caller handed it in `Library::known_files`.

### 4. Schema v4

`SCHEMA_VERSION` moves 3 → 4, adding `mtime_ns INTEGER` and
`file_size INTEGER` by `ALTER TABLE` inside one transaction with the
`user_version` bump — the same discipline as v2 and v3, so an interrupted
upgrade leaves a v3 database the next open migrates again.

Existing rows get **`NULL`, with no backfill**, and here `NULL` is the only
honest value available. v2 could read a file extension; there is no
equivalent for a stamp, because a stamp is a pair of facts about a file *on
disk right now*. Deriving one from anything already in the database would be
inventing the single claim that, if wrong, makes baz show stale tags
forever. Stat'ing the whole library during the upgrade is exactly the
startup re-read the feature exists to remove.

`NULL` is self-healing, as v2's and v3's gaps were: an unstamped row is
always re-read, so the first launch after the upgrade is an ordinary full
scan that stamps everything it touches, and every launch after that is
incremental. The upgrade costs one normal scan and no correctness.

Proof, not assurance: `crates/baz-core/tests/index.rs` builds a genuine v3
database from the v3 schema and v3 `INSERT`s with no baz code involved —
the double rip, the RODIK soundtrack, a real `Various Artists` tag, a real
compilation flag, and non-ASCII paths and titles — migrates it by opening
it, and asserts every column survives byte for byte, that `user_version`
really moved to 4, that the new columns are `NULL`, that `known_files`
therefore asks for a full first scan, and that grouping is *identical* to
before the upgrade.

## Consequences

- Launch cost stops scaling with library size for the unchanged case. The
  status line's counts become honest about it: `Done` now reports added,
  updated, unchanged, removed and failed separately rather than one "tracks
  read" total.
- Deleting a file from disk finally removes it from the shelf. Deleting a
  directory does not (§3), by choice.
- `Scan` gains a lifetime (`Scan<'a>`) because an incremental scan borrows
  the index's snapshot; `scan()` still returns `Scan<'static>` and behaves
  exactly as before. `ScanEntry` gains an `Unchanged` variant. Both are
  breaking changes to pre-1.0 internal APIs.
- The index gains two nullable columns, one migration arm, and its first
  delete path.

### Deliberately deferred

- **Removing rows under a directory that itself vanished.** §3. Needs a
  signal the filesystem does not offer.
- **A user-facing prune.** "These 412 rows point at files I cannot find —
  remove them?" is the honest answer to everything §3 declines to do
  automatically, and it belongs to a library-maintenance surface that does
  not exist yet.
- **Watching the folder** (inotify/FSEvents/`ReadDirectoryChangesW`). A
  cheap rescan makes watch folders *less* urgent, not more; it is its own
  chapter in `VISION.md`.
- **A per-root record.** Gate 2 uses the root currently being scanned. An
  index that remembered which root each row came from could be stricter
  still, and would want a `roots` table — worth it when baz supports more
  than one music folder, not before.
- **Content hashing for move detection.** A file moved rather than deleted
  is currently a removal plus an addition, which loses nothing baz stores
  today. It would matter once per-track state (play counts, ratings) exists.
