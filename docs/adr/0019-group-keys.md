# ADR-0019: Group keys — the shelf gets five arrangements, not one

**Status**: accepted (2026-08-08) · **amends ADR-0008** (its grouping stops
being the only grouping) · **§2 amended by
[ADR-0035](0035-the-wall-has-a-subject.md)** (2026-08-10) · implements step 4
of ADR-0017's build plan · builds on ADR-0018's play-history ledger

> **§2 amended by ADR-0035 (2026-08-10).** ARTIST breaks on the **artist**, not
> on their initial: one shelf per person, headed by their name, in the order
> `ArtistKey` already sorted in. Everything §2 says about that order and about
> the two anonymous ends is unchanged and now applies to whole names rather
> than to letters; `Initial` is unchanged too and became the *index rail's*
> vocabulary, which is where its non-ASCII rule was always earning its keep.
> **§1's promise is untouched and is what licenses the change**: grouping
> albums under their album artist shows every album exactly once, so
> `shelves(GroupKey::Artist)` is still `albums()` with its breaks named,
> element for element — only finer. The key's word went back to `Artist`, and
> its code never moved.

## Context

ADR-0008 decided **what one album is**: the case-folded (album artist, album
title) pair, with editions underneath by codec (ADR-0007). It answered that
question well and it answered no other. `Library::albums()` has exactly one
order — album artist, then album — and the shelf has exactly one arrangement.

Our own UX audit called that an information-architecture problem in its own
words: *"the shelf has one sort and no facets"*. The independent design
critique (`docs/design/critique/02-surfaces.md`) answered it concretely, and
ADR-0017 §1.7 adopted the answer:

> **Group keys [v1]: ARTIST / YEAR / GENRE / ADDED / PLAYED — one row of words,
> no menus. Genre verbatim from tags (messy tags show, honestly).**

and, with it, the index rail:

> a 36 px type-only rail, **a pure projection of the active group key** —
> ARTIST → A–Z, YEAR → decades, GENRE → genre names, ADDED/PLAYED → recency
> buckets. Re-derives on key change; no state of its own.

ADR-0017 §5 named this **the largest breach of ADR-0006 in the plan**, and it
is right to: a redesign was supposed to cost layer 3 and `theme.rs`, and this
one asks the library for a different *shape of answer* and asks the database to
persist two facts it never held. That is a product change wearing a redesign's
clothes, and ADR-0017's amendment to ADR-0006 already says so. This ADR is the
`baz-core` half.

## Decision

### 1. `GroupKey`, and a shelf that carries its own header

```rust
pub enum GroupKey { Artist, Year, Genre, Added, Played }

impl Library {
    pub fn shelves(&self, key: GroupKey) -> Vec<Shelf<'_>>;
    pub fn shelves_with_history(&self, key: GroupKey, history: Option<&History>)
        -> Vec<Shelf<'_>>;
}

pub struct Shelf<'a> { pub header: GroupHeader<'a>, pub albums: Vec<Album<'a>> }

pub enum GroupHeader<'a> {
    Initial(Initial),        // ARTIST — Unknown | # | a letter | Various
    Decade(Option<u32>),     // YEAR   — None is "No year"
    Genre(Option<&'a str>),  // GENRE  — verbatim; None is "No genre"
    Recency(Recency),        // ADDED / PLAYED — the ledger's buckets
}
```

**`albums()` keeps its signature and its meaning.** ADR-0017 writes the API as
`Library::albums(key)`; the shipped name is `shelves(key)`, for two reasons.
The honest name for a function returning shelves is `shelves`, and — the
load-bearing one — `albums()` is called from `crates/baz/src/vm.rs` and from
`analysis.rs`, and the wall does not consume group keys until step 8. Changing
the arity now would mean rewriting a view against a key nothing can yet
select. `albums()` *is* the ARTIST projection flattened, and that is asserted
rather than asserted-in-prose:
`the_artist_key_is_the_flat_shelf_with_its_breaks_named` compares the two album
lists element for element.

**Every key is a projection, never a filter.** Every album the library holds
appears under every key, exactly once, including the albums whose files declare
nothing — a wall that quietly dropped your untagged records would be a wall you
could not trust, and `every_key_shelves_every_album_exactly_once` is the
assertion. This is also what makes the index rail derivable: a rail is
`shelves(key).iter().map(|s| s.header.label())` and nothing else, so a future
key (CRATES, MOOD) costs the rail zero lines.

**Order within a shelf is library order.** `albums()` yields album artist then
album title, and shelving preserves it, so within a decade or a genre the wall
reads alphabetically — the order every other view of this library already uses.

### 2. ARTIST — ADR-0008's grouping, with its breaks named

*(Amended by ADR-0035: the breaks are the artists themselves and `Initial` is
now the rail's vocabulary. The order below is unchanged and is the order the
artist shelves go in.)*

The albums and their order are precisely ADR-0008's. What is new is that the
breaks between them are *stated*, as `Initial`:

```rust
pub enum Initial { Unknown, Other, Letter(char), Various }
```

Variant order *is* shelf order, and it is the order `ArtistKey` already sorts
in: unknowns first, then names that do not start with a letter (`10cc`, `!!!`)
on the design's `#` shelf, then the alphabet, then unnamed compilations. Both
anonymous buckets stay at an end of the shelf rather than landing in the middle
of the alphabet where a sentinel string's letters would have put them —
ADR-0008 §Consequences chose that and nothing here disturbs it.

`Letter` is **not restricted to ASCII**. `Ólafur Arnalds` gets `Ó` and `曲人`
gets `曲`. Folding every non-Latin script onto `#` would make the rail useless
for exactly the library that most needs one, and the price — a rail with more
than 27 entries — is a thing the rail already has to handle, because GENRE has
no bound at all.

### 3. YEAR — decades, undated at the front

The album's year (the first any track declares, unchanged from ADR-0007) floors
to its decade. Shelves run oldest to newest with `No year` at the front, which
is the "unknowns surface rather than hide" rule the whole index follows.

### 4. GENRE — verbatim, and this is the entire specification

Schema v7 adds `genre TEXT`, read from `ItemKey::Genre` — Vorbis `GENRE`, ID3v2
`TCON`, MP4 `©gen`, APE `Genre` — through the same `clean_str` hygiene every
other tag field gets, which decides *present or absent* and changes no
character of a value it keeps.

**No normalisation. No mapping table. No splitting on `;` or `/`. No
title-casing. There will not be one later.** A library that carries
`Post-Rock`, `post rock` and `Rock; Instrumental` shows three genres, because
it *has* three genre tags. This is not laziness; it is the point. Per
`docs/research/05-personas.md` principle 4 the library is a cache of what the
files say and not a place we improve them, and the GENRE key's actual value to
Marta is that it makes her tagging visible so she can fix it *in her tagger*,
where the fix survives. A mapping table would hide the mess, and would then be
a permanent, unwinnable argument about which spellings are the same genre.

**One thing is done to a genre and only one: case folding, for grouping.**
`Rock` and `rock` are one shelf, and its header is the first spelling seen. The
alternative is two shelves that read identically on screen, which is a bug
rather than honesty — and case folding is exactly what artist and album titles
have had since ADR-0003. It changes no displayed character.

**No inference.** A folder called `Jazz/` does not make a file jazz. A
directory name is evidence about who made a record and what it is called —
which is why ADR-0008 §4 lets it fill artist and album — and evidence about
nothing else.

An album's genre is the **first** its tracks declare, exactly as its year is.
Tracks on one record routinely disagree, and there is no answer to "which of
these is the album's genre" more honest than "the one its first track claims".
Refusing to answer when they differ would file most compilations under no genre
at all, which is the worse failure for a key whose job is to show you your
tags.

### 5. ADDED — a first-seen column a rescan structurally cannot move

Schema v7 adds `first_seen_ns INTEGER`, nanoseconds since the Unix epoch (the
unit `mtime_ns` already uses).

The whole difficulty of this column is that baz rescans at every launch, so
"when did this arrive" is destroyed the moment a second scan touches it. The
guarantee is therefore made **by the shape of the schema, not by a convention**:
`UPSERT_TRACK` names `first_seen_ns` in its `INSERT` list and omits it from its
`ON CONFLICT DO UPDATE` list. A row's first-seen is written once, when the row
is created, and there is no statement anywhere in baz that can move it. This is
the same structural trick the `rg_computed_*` columns use (ADR-0015 §v6), for
the same reason: a property held by the schema beats a property held by two
writers agreeing to be careful. `SearchIndex::insert` mirrors it in RAM, and
`first_seen_is_written_once_and_no_rescan_can_move_it` asserts both halves
agree across a restart.

**An album's first-seen is the earliest of its tracks'.** A rip whose second
disc landed a year after its first is a record you have had for a year. Dating
an album by its *newest* track would resurface a twenty-year-old album at the
top of the wall because one file was re-ripped — which is the behaviour ADDED
exists to provide, not to be fooled by.

**Existing rows get `NULL`, permanently, and this is the decision rather than
the gap.** Unlike every previous migration's NULLs, this one does not
self-heal, because no later scan can discover the fact. Three backfills were
considered and all three are lies:

| Candidate | Why not |
|---|---|
| The migration's own clock | Files a listener's twenty-year collection under TODAY on upgrade day, and is then indistinguishable forever from an import that really happened that day. |
| `mtime_ns` | Real evidence about the *file*, not about when it entered the library: a ReplayGain scanner or a tag fix moves it, so a retagged library would report itself as freshly imported. Also `NULL` for every pre-v4 row. |
| `id` order | Row ids are an insertion sequence, not a clock. |

So baz reports what it knows. `Recency::Unrecorded` — "Not recorded" — is the
shelf, and everything scanned *after* the upgrade gets a real first-seen and
appears at the top, which is the case ADDED exists for ("new rips appear under
ADDED"). A fabricated backfill would buy a prettier first screen at the cost of
the only property the column has.

### 6. PLAYED — wired to the ledger, correct without one

ADR-0018's ledger landed while this work was in flight, so PLAYED is wired to
it rather than to a placeholder:

```rust
library.shelves_with_history(GroupKey::Played, Some(&history))
```

`Album::played_recency` takes the **most recent** bucket over every track of
every edition — the FLAC rip and the MP3 copy are one record, and a listener
who played the phone copy last week has not gone a year without this album.
`Recency` is ordered most-recent-first, so "most recent" is `min`.

**The ledger is optional at runtime and PLAYED is correct without it.**
`History` writes nothing until a front end calls `EngineHandle::set_history`,
which `crates/baz` does not do yet. `shelves(GroupKey::Played)` — no ledger —
and an existing but empty ledger produce the identical answer: one shelf,
`NEVER PLAYED`, holding the whole library. That is not a degraded mode; "baz
has no record of playing this" is a true statement about a library nobody has
played. Both are asserted.

### 7. One recency vocabulary, extended rather than duplicated

ADDED and PLAYED are drawn by the same rail in the same lane, so they use the
**same** enum: ADR-0018's `history::Recency` (`ThisEvening`, `Today`,
`ThisWeek`, `ThisMonth`, `MonthsAgo(n)`, `YearsAgo(n)`, `Never`), through its
public `bucket()`. Two bucket vocabularies that had to agree would eventually
not.

ADDED needs exactly one bucket a play ledger can never produce, so the variant
was added to `history::Recency` rather than mapped onto a second type in
`index.rs`:

- **`Unrecorded`**, last in the order, for a row that predates first-seen. It is
  genuinely distinct from `Never`: `Never` is a *positive* statement the ledger
  makes ("this was not played"), `Unrecorded` says there is no timestamp at all.
  `History::recency` never returns it.

`Recency::label()` also lands there, beside the enum, because the rail's text
for a bucket belongs with the bucket. One consequence is stated rather than
discovered: ADDED's most recent shelf reads `This evening` (the ledger's first
band is six hours), which is a slightly odd word for an import and is worth
less than a second set of bands.

### 8. Schema v7

`SCHEMA_VERSION` moves 6 → 7, adding `genre TEXT` and `first_seen_ns INTEGER`
by `ALTER TABLE` inside one transaction with the `user_version` bump — the same
discipline as v2 – v6, so an interrupted upgrade leaves a v6 database the next
open migrates again.

Proof, not assurance: `a_v6_database_migrates_in_place_without_losing_anything`
builds a genuine v6 database from the **v6 schema and v6 `INSERT`s with no baz
code involved**, holding the owner's actual rows — the double rip, the
soundtrack, the file whose tagger really wrote `Various Artists`, Unicode
throughout — migrates it by opening it, and asserts every column survives
(including v6's own stamped measurement), that `user_version` really moved to
7, that both new columns are `NULL`, that ADDED therefore draws exactly one
`Not recorded` shelf, and that grouping is bit-for-bit the pre-v7 shelf. Paths
are written with `stored_path_bytes`, so the fixture is a real database on
Windows too rather than a Unix one.

`a_rescan_after_a_v6_upgrade_fills_the_genre_and_never_invents_a_first_seen`
holds the two columns' different healing behaviour apart.

## Consequences

- `Library` gains `shelves` / `shelves_with_history`; `albums()` is unchanged
  and is the ARTIST projection flattened, asserted.
- `Album` gains `genre: Option<&str>` and `first_seen_ns: Option<i64>`, and
  `played_recency`. `TrackMeta` gains `genre: Option<String>`. Additive, but
  struct-literal construction of `TrackMeta` breaks — one mechanical line in a
  `crates/baz` test fixture, no behaviour.
- The index gains two nullable columns and one migration arm. One of those
  columns is the first in `tracks` whose value a rescan is structurally unable
  to reach.
- `history::Recency` gains `Unrecorded` and `label()`. The enum is
  `#[non_exhaustive]`, so this is not a breaking change to a front end.
- **Zero new dependencies.** The recency bands are elapsed-time arithmetic on
  the ledger's own constants; no calendar, no timezone database, no date crate.
  ADR-0018 already argued that trade and it is inherited rather than re-taken.
- `ScanEntry` crossed clippy's `large_enum_variant` threshold when `TrackMeta`
  grew. Boxed nothing; the lint is `#[expect]`-ed with the reason — `Track` is
  the common variant, one per audio file, consumed immediately, and boxing it
  would put an allocation on the scan's per-file path to shrink the rare
  variants.

### What this ADR does *not* do

- **Search ranking** (ADR-0017 step 12) is untouched. `Library::search` still
  returns corpus order and still documents that it needs no re-sorting. It was
  looked at and left: ranking changes `search`'s documented contract and the
  tests that pin its determinism, which is a decision with its own shape and
  does not belong in the same commit as a schema migration.
- **No UI.** The group-key row, the sticky shelf headers and the index rail are
  step 8. Nothing in `crates/baz` selects a key yet, which is why `albums()`
  keeps its signature.

### Deliberately deferred

- **A CRATES key.** ADR-0017 defers crates; when they arrive a crate is a
  `GroupKey` variant, a `GroupHeader` variant and a `ShelfSort` variant, and the
  rail costs nothing. That is the point of the shape.
- **A persisted active key.** `GroupKey::code`/`from_code` exist so `config.rs`
  can hold one, as density will (ADR-0017 §1.3). Nothing writes it yet.
- **Per-shelf album ordering.** Within a decade the wall reads alphabetically.
  Ordering YEAR's shelves by year-within-decade, or GENRE's by anything but the
  name, is a view decision with no evidence behind it yet.
- **Multi-value genre tags.** A file with two `GENRE` comments contributes its
  first. Shelving one album under several genres is a different feature — it
  breaks "every album appears exactly once" — and needs its own argument.
- **Backfilling `first_seen_ns` from a listener's own evidence.** If someone
  wants their old library dated, importing dates from a file they supply is an
  explicit, visible act. Guessing inside a migration is not.
