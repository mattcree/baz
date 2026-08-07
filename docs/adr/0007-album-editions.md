# ADR-0007: Album editions — one album, several formats

**Status**: accepted (2026-08-07)

## Context

The owner's library holds `~/Music/FLAC/Stan Rogers/Northwest Passage/` **and**
`~/Music/MP3/Stan Rogers/Northwest Passage/`: one album, ripped twice. Album
grouping keys on case-folded (artist, album), so both folders merged into a
single shelf entry whose track list contained every track twice, interleaved —
and whose Play queued all 24 of them.

This is not a corner case. Marta, the collector-curator persona
(`docs/research/05-personas.md`), and Karl, the audiophile, both routinely keep
a lossless archive plus a lossy copy for portable devices; r/musichoarder
treats the practice as normal hygiene. Deduplicating one copy away would
destroy information the user deliberately created. The multi-format album is a
**concept the player must know about**, not a mess to be cleaned up.

Two properties had to hold together: the shelf shows albums (Devon's "the
library is the interface"), and the collector's second copy stays visible and
playable.

## Decision

### 1. The model: `Album { …, editions: Vec<Edition> }`

`Library::albums()` groups tracks into albums, then splits each album by
**codec** into editions.

> **Amended by ADR-0008 (2026-08-07).** As written, this decision grouped
> "exactly as before" — on case-folded (track artist, album title). It now
> groups on (**album artist**, album title), and `Album::artist` is an
> `AlbumArtist` enum rather than an `Option<&str>`; see
> `docs/adr/0008-album-artist-grouping.md`. Nothing about editions changed:
> the two axes are independent — album artist decides what one album *is*,
> codec decides how many editions it has — and
> `a_grouped_soundtrack_still_splits_into_editions_by_codec` holds them
> apart. §3 below is likewise v2-of-3; the v3 migration is ADR-0008 §5. An `Edition` is a format plus that
format's own track list; every track belongs to exactly one edition, and
nothing is paired across editions by position — so a partial rip is simply a
shorter edition, never a mis-alignment.

Editions are distinguished by `TrackMeta::format`, an `AudioFormat` read from
the file's audio headers by lofty during the same read as the tags. **Never by
folder name**: a library may be filed `FLAC/…` / `MP3/…`, filed by decade, or
not filed at all, and a folder called `MP3` may hold anything. `AudioFormat`
covers the codecs the scanner's extensions actually carry — FLAC, ALAC, WAV,
MP3, AAC, Vorbis, Opus — and reports anything else as `None`: an honest
unnamed edition rather than a guess.

The one ambiguity is MP4. `.m4a`/`.mp4` may hold ALAC or AAC, and the
format-agnostic `FileProperties` a lofty `TaggedFile` exposes has already
dropped the container's `Mp4Codec`. The discriminator we use is the declared
bit depth, which lofty fills from the `alac` sample-description atom and leaves
unset for AAC. Known false positive: FLAC-in-MP4, which also declares a depth
and would be labelled ALAC — a wrong name in the right fidelity tier, on a
combination essentially absent from real libraries.

Three coarse quality signals ride along on `TrackMeta` — `bit_depth`,
`sample_rate`, `bitrate` — because the same header read already has them and
all three are used: two to describe an edition on screen, all three to rank
them. Nothing speculative was added.

An `Edition` summarises depth and rate **uniform-or-nothing**: "16-bit" is a
claim about every track, and an edition with one 24-bit outlier declines to
make it rather than rounding the outlier away. Bitrate is averaged, because
unlike depth and rate it legitimately varies track to track (and within a
track, under VBR).

### 2. Default edition: the fidelity ranking

`Album::editions` is sorted best-first; the default is `editions[0]`. The
comparison, in order:

1. **Lossless before lossy, unknown codec last.** "Lossless" is a fact about
   the decoded samples; bitrate is a fact about the file. A codec we could not
   name is never *assumed* lossless.
2. **More tracks first.** Within a tier, the complete rip beats the partial
   one — defaulting to 3 of 12 tracks is the worse failure to hand a listener.
3. **Higher mean bitrate first.** 24/96 FLAC over 16/44; 320 kbit/s MP3 over
   128. Across *different* lossy codecs this is explicitly a preference and
   not a fidelity claim (128 kbit/s Opus is not worse than 192 kbit/s MP3) — it
   only ever breaks a tie, and some answer must be given.
4. **Codec code, ascending.** Determinism only. Formats are unique within an
   album (they are the grouping key), so the ordering is total.

Rule 1 is deliberately above rule 2, per the owner's directive that highest
fidelity wins. The consequence is that a partial lossless rip outranks a
complete lossy one; the selector is one click away, and rule 2 already covers
the more common within-tier case. Revisit if it bites.

### 3. Schema v2 — the first real migration

This is the first use of the `PRAGMA user_version` skeleton laid down in
ADR-0003's index work. `SCHEMA_VERSION` moves 1 → 2, adding four nullable
columns (`format`, `bit_depth`, `sample_rate`, `bitrate`) by `ALTER TABLE`
inside one transaction with the version bump, so an interrupted upgrade leaves
a v1 database that the next open migrates again.

**A fresh database now walks the whole chain** (0 → v1 → v2) rather than being
stamped with the current schema directly. That costs a few statements once per
install and buys the guarantee that "created fresh" and "upgraded" cannot drift
into two different shapes — no "works on a new install, breaks on an old
library" bug can hide between two code paths, and every release exercises its
own migration code.

Backfill policy for existing rows: **format from the file extension where the
extension settles it** (`.flac`, `.mp3`, `.wav`, `.opus`), and `NULL` where it
does not (`.m4a`, `.mp4`, `.ogg` are containers; only the file knows). Nothing
is read from disk — an upgrade must not become a full library re-read at
startup. `bit_depth`/`sample_rate`/`bitrate` stay `NULL` for the same reason.

`NULL` is self-healing rather than permanent: baz rescans its music folder at
every start and `add_tracks` upserts, so each surviving file gets its true
codec within the first scan after the upgrade. Until then an unbackfilled
album shows one unnamed edition — precisely the pre-editions behaviour.

> **Amended by ADR-0010 (2026-08-07).** That rescan is now *incremental*
> (schema v4), so it no longer re-reads every file on every launch. The
> self-healing claim above still holds unchanged: a migrated row carries no
> file stamp either, and an unstamped row is always re-read — so the first
> scan after any upgrade is a full one, exactly as this section assumes.

Proof, not assurance: `crates/baz-core/tests/index.rs` builds a genuine v1
database using the v1 schema and v1 `INSERT`s with no baz code involved,
migrates it by opening it, and asserts every row survives with its Unicode
metadata, durations, disc/track numbers and non-ASCII paths intact; that
`user_version` really moved to 2; that unambiguous extensions were backfilled
and ambiguous ones were not; that a second open is a no-op; and that the
owner's double rip comes back as one album with two editions.

### 4. UI (ADR-0006 layering)

- **Layer 1 (`vm.rs`, iced-free, unit-tested):** `EditionVm`, the `EditionKey`
  newtype, `selected_edition()`, and an `album_queue()` that takes the choice.
  Selection is *not* stored in `vm`; it is a pure function of (album, choice),
  so "what is listed" and "what plays" are the same call and cannot diverge.
  `EditionKey` wraps `Option<AudioFormat>` because `None` is itself a real
  edition — "the unnamed edition" and "no choice made" must not collide.
- **Layer 2 (`theme.rs`):** `segmented` (an inset well, like a text input) and
  `segment` (raised card when chosen, label-only otherwise), plus
  `RADIUS_SEGMENT` and `SEGMENT_INSET`. Deliberately **not** lamp amber: the
  accent means playback truth, and a format choice is a view, not a claim
  about what is playing.
- **Layer 3 (`app.rs`):** `edition_selector()` renders the control **only when
  the album has more than one edition**, so the ordinary single-format album
  gains no chrome whatsoever. The side panel's track list, its track/duration
  counts, and the queue Play sends all come from the selected edition; a quiet
  `FLAC · 16-bit · 44.1 kHz` line under the header states the encoding.

Switching editions changes what is listed and what the *next* Play queues. It
never interrupts what is already playing — and `resolve_now_playing` searches
every edition, so a track keeps its name on the bottom bar even after the panel
has been switched away from its format.

Selection is **session-scoped** (`HashMap<u64, EditionKey>` on the shelf). It
is not persisted because the config file is a hand-rolled single-key TOML
document (`config.rs` documents that adopting the `toml` crate is the plan when
it grows), so persisting a per-album map means taking a dependency for a
preference whose proper home is a column in the library database anyway.

## Consequences

- One tile per album, whatever the folder layout. The duplicated,
  interleaved track list is gone.
- `Album::tracks` is replaced by `Album::editions`; `album_queue` takes the
  chosen edition. Both are breaking changes to pre-1.0 internal APIs, made
  deliberately rather than papered over with a "default tracks" accessor that
  would quietly hide the other editions from every future caller.
- The index gains four nullable columns and one migration arm. A v1 database
  upgrades in place with no data loss and no startup re-read.

### Deliberately deferred

- **Persisted per-album preference.** Session-scoped for now; the honest home
  is an index column, landing with whatever else needs per-album state.
- **A library-wide format preference** ("always prefer MP3 on this laptop").
  The ranking is a default, not a policy engine.
- **"Merge all formats" view.** Nobody has asked for the old behaviour back,
  and it is one `all_tracks()` call away if they do.
- **Deduplicating identical rips.** Out of scope by design: baz never modifies
  or hides files it was not told to (`docs/research/05-personas.md`, principle
  4). A duplicate-review flow is a curation feature, opt-in and undoable, not
  a side effect of grouping.
- **A multi-format hint on the shelf tile.** The information is one click
  away, and tile chrome is the thing the shelf can least afford.
- **Editions as anything but codec** — remasters, deluxe editions, different
  masterings at the same codec. That needs tags baz does not read yet, and
  conflating it with the format axis now would make both harder later.
