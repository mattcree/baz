# ADR-0008: Album artist — what makes one album one album

**Status**: accepted (2026-08-07) · **amended by
[ADR-0019](0019-group-keys.md)** (2026-08-08)

> **Amended by ADR-0019 (2026-08-08).** Everything below still decides **what
> one album is**, and nothing in it changed: the (album artist, album title)
> key, the fallback chain, the `AlbumArtist` enum, the compilation flag, and
> the shelf order that keeps both anonymous buckets at the ends. What changed
> is that this grouping is no longer the *only* grouping. `Library::shelves`
> arranges the same albums under one of five group keys — ARTIST, YEAR, GENRE,
> ADDED, PLAYED — and ARTIST is this ADR's shelf with its breaks named: one
> per artist since [ADR-0035](0035-the-wall-has-a-subject.md), one per initial
> before it. Either way the albums and their order are exactly this ADR's, and
> `Library::albums()` keeps its signature and is the ARTIST projection
> flattened, asserted rather than claimed. See
> `docs/adr/0019-group-keys.md`; §5's schema is now v7 of 7.

## Context

ADR-0007 answered "one album, several formats". It left the prior question
alone: **what is one album?** Grouping keyed on case-folded (track artist,
album title), and the owner's own library shows what that costs. Four shelf
entries, from real output over `~/.local/share/baz/library.db`:

    Kouhei Okamura, Masashi Matsumoto, Katsuhiko Nakamichi — Cookie's Bustle : 1 edition
    Miki Nagamatsu — Cookie's Bustle OST : 1 edition
    Miki Nagamatsu, Kouhei Okamura, Masashi Matsumoto, Katsuhiko Nakamichi — Cookie's Bustle OST (gamerip) : 1 edition
    Kouhei Okamura, Masashi Matsumoto, Katsuhiko Nakamichi — Cookie's Bustle OST (gamerip) : 2 editions

Five entries, in a 40-track library, for what a person sees as three albums.
The cause is not messy tagging: the `ARTIST` tag is *supposed* to differ per
track on a soundtrack or a compilation — it is the per-track credit. The tag
that says which album a track belongs to is `ALBUMARTIST`, and every serious
library manager (Picard, beets, MusicBee, foobar2000, Navidrome) keys on it.
baz was not reading it.

This is Marta's case (`docs/research/05-personas.md`): forty thousand tracks
tagged deliberately, and a player that treats her per-composer credits as a
reason to fragment her shelf is a player that punishes the curation. It is
also Devon's — the shelf is the interface, and a shelf that shows a soundtrack
four times is not showing a collection.

## Decision

### 1. The grouping key becomes (album artist, album title)

`Library::albums` keys on case-folded (album-artist-or-fallback, album
title). The fallback chain, resolved **per track** by `AlbumArtist::of`:

1. **The album-artist tag** → `Named(name)`. Read from whichever spelling the
   container uses — Vorbis `ALBUMARTIST` (and the non-standard `ALBUM ARTIST`
   older taggers write), ID3v2 `TPE2`, MP4 `aART`, APE `Album Artist`.
2. **The compilation flag** → `Various`. See §3.
3. **The track artist** → `Named(name)`.
4. **Nothing** → `Unknown`.

The chain is per track, not per album, because it *is* the grouping key: it
has to be computable before the album exists. Step 3 is what makes the
often-stated rule "if the album's tracks share one artist, use that artist"
fall out for free — tracks that share an artist share a key, and tracks that
do not are merged only when step 1 or 2 gave a reason to merge them. An
album with no album-artist tag and one consistent artist groups **exactly**
as it did before this ADR; that is the overwhelming majority of every library
and it must not move.

### 2. `AlbumArtist` is an enum, not a string with a sentinel

    enum AlbumArtist<'a> { Named(&'a str), Various, Unknown }

The tempting shortcut is `Option<String>` with `"Various Artists"` written in
when we give up. The owner's library refutes it directly: one of his MP3s
carries `TPE2 = Various Artists` as a genuine, deliberate tag. Under a magic
string, "the tagger named this album's artist" and "baz could not name it"
become the same value — and the shelf could no longer distinguish a name the
user curated from a word we invented. They also must not share an album *id*,
or two different albums collide in the thumbnail cache. So the three states
are three values, and `"Various Artists"` exists only as a UI label
(`vm::VARIOUS_ARTISTS`) that nothing ever matches on.

### 3. The compilation flag, and the case we deliberately do not guess

The remaining case is an album whose tracks name different artists and which
carries no album-artist tag. Grouping it by title alone is not available:
two artists who each released a *Greatest Hits* is far commoner in a real
library than an untagged compilation, and merging them would be a regression
with no signal behind it (`same_album_title_by_different_artists_stays_separate`
has guarded this since ADR-0003's index work).

So we need a signal, and one already exists in the files: the compilation
flag — ID3v2 `TCMP`, MP4 `cpil`, Vorbis `COMPILATION`, APE `Compilation`.
iTunes sets it, Picard sets it on Various-Artists releases. A file that flags
itself a compilation has told us its track artist is *not* the album's
artist; grouping by that artist would be following a value the file itself
disclaimed. That is the flag's whole justification for existing on
`TrackMeta` — it makes step 2 of the chain reachable, and nothing else in baz
reads it. Without it the field would not have earned its place.

Where no signal exists at all, **baz declines to merge and this is
documented, not accidental**: same title, different artists, no album artist,
no flag → separate shelf entries. Inventing a compilation out of a title
collision would be a guess, and per `docs/research/05-personas.md` principle
4 the library is a cache of what the files say, not a place we improve them.

### 4. Folder inference fills the album artist — but only where it fills the artist

An untagged `Artist/Album/track.ext` tree must still group correctly, so
inference populates `album_artist` from the same grandparent directory it
already takes `artist` from — and **only when the tags left both blank**. A
file that names its artist but not its album artist is trusted over its
folder: a directory called `Beatles/` must not overrule `ARTIST=The Beatles`,
because the album artist is now the shelf caption *and* the grouping key, and
a folder name is the weaker evidence. The chain reaches the artist tag at
step 3 anyway, so nothing is lost by the restraint.

### 5. Schema v3

`SCHEMA_VERSION` moves 2 → 3, adding `album_artist TEXT` and
`compilation INTEGER` by `ALTER TABLE` inside one transaction with the
`user_version` bump — the same discipline as v2, so an interrupted upgrade
leaves a v2 database the next open migrates again.

Existing rows get **`NULL`, with no backfill**. Unlike v2's `format`, which a
file extension could sometimes settle, nothing already in the database can
produce an album artist: it lives in the file's tags and nowhere else, and
copying the stored track artist into it would be indistinguishable, forever
after, from a value the user's tagger actually wrote. An upgrade must not
become a full library re-read at startup either.

`NULL` is self-healing: baz rescans at every launch and `add_tracks` upserts,
so the first scan fills every surviving file's real album artist. (ADR-0010
made that rescan incremental; a migrated row carries no file stamp, and an
unstamped row is always re-read, so the first scan after an upgrade is still
a full one.) Until then
`AlbumArtist::of` falls through to the track artist and grouping is precisely
the pre-v3 behaviour — the upgrade cannot make the shelf worse, only later
better. Both halves are asserted in `crates/baz-core/tests/index.rs` against a
database built with **v2 schema and v2 `INSERT`s and no baz code**, holding
the owner's actual rows.

### 6. UI (ADR-0006 layering)

- **Layer 1 (`vm.rs`, iced-free, unit-tested):** `AlbumArtistVm`, the owned
  mirror of the core enum, owns the labels (`label()` — the name,
  `Various Artists`, or `Unknown Artist`) so a tile caption can never come
  out blank. `AlbumVm::track_artists_vary` decides once per album whether the
  side panel lists per-track artists: true exactly when some track names an
  artist (case-folded) that the album's header does not already state. A
  track with no artist never triggers it; an album with no *named* artist is
  covered by no name, so any track that names one differs.
- **Layer 2 (`theme.rs`):** untouched. The per-track artist reuses
  `SIZE_META` / `PAPER_DIM`, the existing quiet-secondary-line pair.
- **Layer 3 (`app.rs`):** the tile caption and the panel header read
  `album.artist.label()`; `track_row` takes a `show_artist` flag and stacks
  the track's own artist under its title — the same title-over-artist shape
  the now-playing bar already uses. An ordinary album gains no extra line at
  all.

Grouping a soundtrack into one tile must not cost the information that made
it shatter. The per-composer credits are exactly what a collector-curator
keeps a soundtrack for, so they move from the *tile* (where they were noise
and a bug) to the *track list* (where they are the point).

Two consequences follow for free and are asserted: the album artist joins the
search corpus — the name on the tile has to be a name the search box finds,
or the filtered shelf contradicts the unfiltered one — but only when it
differs from the track artist, so the common case adds nothing to the corpus
the per-keystroke scan walks. And `now playing` shows the album's artist only
when it *has* a name (`AlbumArtistVm::name`), never the words `Various
Artists` as if the engine were playing a band by that name.

## Consequences

- The owner's library goes from **7 shelf entries to 5**, and the Cookie's
  Bustle cluster from **5 entries to 3**: `RODIK — Cookie's Bustle OST
  (gamerip)` (FLAC + AAC editions, 8 tracks, per-composer credits listed),
  `Miki Nagamatsu — Cookie's Bustle OST` (7 tracks), and `Various Artists —
  Cookie's Bustle` (2 tracks, from the album-artist tag the files carry).
  Verified by migrating a copy of the live `library.db` and rescanning
  `~/Music`.
- `Album::artist` changes type from `Option<&str>` to `AlbumArtist<'_>`, and
  `vm::album_id` takes it. Breaking changes to pre-1.0 internal APIs, made
  deliberately: an accessor returning `Option<&str>` would have quietly
  reintroduced the two-state model the enum exists to prevent.
- Shelf order changes at the edges: unnamed compilations sort after every
  named artist, unknowns still sort first. Both anonymous buckets sit at an
  end of the shelf rather than in the middle of the alphabet where a
  sentinel string's letters would have landed them by accident.
- The index gains two nullable columns and one migration arm.

### Deliberately deferred

- **Merging on directory.** A flagless, album-artist-less compilation whose
  files share one folder is arguably one album, and the folder is real
  evidence. It is also the same evidence that would merge two `Greatest Hits`
  albums filed in one flat directory. Wanting a second signal before
  overriding the tags is the whole spirit of §3; revisit with a real library
  that needs it.
- **`ALBUMARTISTSORT` / sort names.** "Beatles, The" belongs to a browsing
  facet, not to identity.
- **MusicBrainz release IDs** as the grouping key. Strictly better than any
  string when present, and strictly unavailable in the untagged libraries
  baz promises to be excellent with. Would layer above this chain, not
  replace it.
- **Artist-level browsing.** The album artist is now first-class data; an
  artist facet is a separate feature with its own design, not a side effect
  of fixing grouping.
- **Per-track artists on the shelf tile.** They belong to the track list. The
  tile can least afford chrome (ADR-0007 reached the same conclusion about
  the multi-format hint).
