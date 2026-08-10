# ADR-0035: The wall has a subject — `A–Z`, `ARTISTS`, and one word that was two things

**Status**: accepted (2026-08-10) · **amends [ADR-0019](0019-group-keys.md)**
(the first key's *label*; §1's projection promise is untouched and is the
reason the sixth word is not a key) · **amends
[ADR-0026](0026-iconography-and-the-strip-budget.md) §3** (the strip's budget
is re-derived: `KEYS_W` 314 → 368 and everything downstream) · builds on the
Artist place from [ADR-0022](0022-places-and-nothing-else.md)'s line of work and
reuses [ADR-0024](0024-playlists.md) §A1's collage verbatim · closes the costed
proposal at `docs/BACKLOG.md` · the frames are
[`docs/design/impl/artists-wall/`](../design/impl/artists-wall/)

## Context

baz had **two things called artist**, and they were different things.

- The wall's **`ARTIST` group key** breaks records on their album artist's
  initial. Its shelves read `Unknown`, `#`, `A`, `C`, `Various`; its index rail
  is the alphabet. It is a *sort*.
- The **Artist place** is a page about a person: their name, their records,
  reached from a record page's `Artist › Album` breadcrumb. It is a *subject*.

One word, one product, two meanings — and they are adjacent on screen, because
the key's word sits in the strip above a wall you can press a tile in to reach
the place.

A costed proposal for fixing it was carried in `docs/BACKLOG.md`: an agent had
built the whole thing before its branch was discarded as a duplicate, and its
measurements were kept precisely so they would not need re-deriving. This ADR
records what shipped, which is that proposal with its numbers re-measured
against a main that had moved underneath them.

## Decision

### 1. The key's word becomes `A–Z`; its code does not move

`GroupKey::Artist.label()` is `"A–Z"`. The variant is unchanged and
`GroupKey::code()` is still `"artist"`.

**Why `A–Z` and not `NAME`.** The key produces an alphabet: its headers are
letters and its rail *is* the alphabet, so `A–Z` names the thing on screen.
`NAME` would still read as a subject — the name *of what?* — and would collide
with the Artist place again one release later.

**Why the code cannot follow.** It is on-disk data. Every `config.toml` baz has
ever written carries `group_key = "artist"`, and the type's own doc says never
change an existing code. The rename is therefore a *label* change and the
distinction between the two is now stated on `label()` itself, with the round
trip pinned in `baz-core`'s tests: the label moved, the code did not, and no
key's word is a subject the product has a place for.

### 2. `ARTISTS` is a sixth **word**, holding a `WallSubject` beside the key

The strip's row is now `A–Z · YEAR · GENRE · ADDED · PLAYED · ARTISTS` — six
words, one of them current, drawn identically. Pressing `ARTISTS` puts a wall
of **artists** up: one tile per person the collection is filed under, shelved
by initial.

**It is not a sixth `GroupKey`.** ADR-0019 §1 promises that every key is a
*projection* in which every album appears exactly once, and `baz-core` sweeps
`GroupKey::ALL` to assert it. A key that shelved artists would **falsify that
sweep rather than extend it**: it does not re-arrange the albums, it changes
what a tile *is*. So the subject is a separate `vm::WallSubject` held beside
`group_key` on the shelf and beside it in the config.

**It is not a lens.** The lens switcher is fixed at two words by the product's
standing rules (`WALL` · `MARQUEE`) and both are spoken for.

**What holding it separately buys, and it is visible.** The arrangement
survives a trip through the artists and back. Leave the records on `YEAR`,
press `ARTISTS`, press `YEAR`: the decades are exactly where they were — the
frames are pixel-identical, which `capture.sh` diffs and reports. A sixth key
would have had to forget the arrangement to show the artists and guess one to
come back.

Pressing one of the five *does* return the wall to records, and only in that
direction: the five words are how records are arranged, so pressing one is
asking for records. A `YEAR` that left the artists standing would be a word
that did nothing.

Its accelerator is `6` — the digit the number row had already kept out of the
query for exactly this, and `crate::keys` says so where it says the row is
spent as a row.

### 3. One query, projected twice

`Shelf::refilter` spends the search **once**, for the records, and then puts
that same answer through `vm::visible_artists`. Two searches would give the two
walls two chances to disagree about what a query means and would cost every
keystroke double.

The property, swept over every subset of a six-record collection rather than
demonstrated on three examples: **an artist survives exactly when one of their
records does.**

### 4. The artists wall costs the wall's machinery nothing

The virtualizer, the sticky headers, the grid arithmetic and the index rail are
untouched. The view asks *the wall* for its headers, its survivors and its
per-shelf counts — `wall_groups`, `wall_visible`, `wall_visible_counts` — and
only what a cell *is* forks. `rail::entries` is a pure function of the shelf
headers, and this wall's headers are `Initial`s, so `rail::artist` indexes it
verbatim: no branch, no new vocabulary, no state.

### 5. The tile is the album tile's anatomy and the playlist's collage

Same art edge inside the same mat, same two reserved caption lanes, same rule
lane, same hit box — the two subjects share a wall and must share its
arithmetic. The sleeve is `views::playlist_sleeve`: the 2 × 2 of the first
four, the full-bleed single below four, the designed rest tile at zero, out of
the same thumbnail cache with the same gradient while a decode is in flight. A
second collage would be two renderings of one idea that could drift apart.

The caption's second line is the record count, where an album tile carries the
artist and the year — the one fact about an artist the collection can state
without going to a network, which is the same line `views::artist` already
draws on the page.

What the tile deliberately lacks, each for a reason: **no hover options** (play,
queue and add are answers about a *record*, and an artist has no equivalent
verb yet), **no right-press menu** (`menu::Target` names records and lists),
and **no opened rule** (`Shelf::opened` is *the record the wall was last left
for* and it is one value; spending it on an artist would take the mark off the
record it was built for).

**The display spelling is the one that sorts first.** Identity is case-folded,
so `Alpha` and `alpha` are one artist with two spellings on disk, and *first
found* is an order a rescan can change. This is the same rule and the same
answer as `views::artist::label`, which is what stops a tile and the page it
opens from naming one artist two ways.

An artist's quotations are their records in the **records' own alphabetical
order**, not the wall's: the wall's order would make a collage depend on the
active group key, so pressing `YEAR` would reshuffle every artist's artwork.

### 6. Artist tiles have their own art prefetch

An artist's collage quotes records that are **not on the wall**, and the wall's
visible range is the whole of the thumbnail guard. Without this an artist's
collage draws the deterministic gradient until one of the records it quotes
happens to scroll past on the *records* wall — real artwork by luck, which is
verbatim the defect the playlist collages had and then the artist page had.
This is the third surface to need the guard extended, and the last one that
needs it.

It lives in `Shelf::request_visible_thumbs` rather than beside the artist
page's own line in `App::request_offscreen_art`, and the reason is a
correction to the plan: `request_offscreen_art` is keyed on the lane's stamps
and the place, neither of which moves when the wall slides under the pointer.
Only the wall's own range guard re-fires on a scroll, and the artists wall's
range *is* that guard. `SLEEVE_CELLS` is named in `views` so the prefetch and
the collage cannot ask for different numbers of cells.

### 7. The readouts follow the subject

Both wells — the lane's, and the strip's at the widths the lane cannot hold it
— count whatever the wall is a wall of, through `Shelf::wall_counts` and
`wall_noun`. `10 / 11` artists, and `11 artists · 206 tracks` as the strip
well's placeholder. A figure counting albums beside a wall of people would be a
readout describing a surface that is not on screen.

**Home's `COLLECTION` footer is deliberately not touched.** Its four figures
are a statistic about the whole collection, unnarrowed by any query, on a
different place from the wall — and it already states records and artists side
by side. There is no narrowed figure there to be wrong.

The **Songs** section is drawn only over a wall of records: it ends in an
`Albums` rule naming the wall beneath it, and its rows are tracks. Two subjects
stacked over a third would be the wall answering one query three ways.

## The strip's budget, re-derived

Measured in the bundled face at the metadata size, with `theme::tracked`
applied, and asserted as const arithmetic. Six words come to **366.50 px**;
`KEYS_W` is the next 4 px step, **368**.

| | before | after |
|---|---:|---:|
| `KEYS_W` | 314 | **368** |
| `LIBRARY_LINE` | 506 | **560** |
| `SINGLE_LINE` = `TOP_BAR_SPLIT` | 778 | **832** |
| `SINGLE_LINE_NO_WELL` | 554 | **608** |
| `WIDEST_LANE_STRIP` | 720 | 720 (unmoved) |
| `TOP_BAR_FLOOR`, and the window's own minimum | 600 / 696 | 600 / 696 (unmoved) |

Two things are worth stating plainly.

**The backlog's costing said the single-line-with-well band would cease to
exist**, and it was right about the arithmetic and wrong about the world. It
measured six words against a strip whose acts cluster was still 182 px wide and
got a split of 926 — above the widest strip that can hold the well at all
(`SIDEBAR_FLOOR − SIDEBAR_RAIL_W` = 904), which would have made the strip two
lines at every width below the lane's floor. `Pull` was removed and `Shuffle`
moved to the now-playing bar in between, taking `ACTS_W` from 182 to 88 and
paying for the sixth word twice over. The split is 832, the band is 832…904,
and it is now asserted **because it was predicted not to be there**.

**The headroom is stated as a difference, not a number.** The acts cluster gave
94 px back and the arrangement row has spent 54 of it, so the library line sits
40 px under the floor — and the assertion is written that way, so it cannot be
right by coincidence. Against the narrowest strip the well-less regime can be
handed there are 112 px. Those two figures, and the 1.50 px between the six
measured words and their declaration, are what a seventh word would come out
of.

## Consequences

- One word means one thing. `A–Z` is a sort and `Artist` is a subject, and the
  strip no longer says the second when it means the first.
- The wall has a subject as well as an arrangement, and they are independent
  state that persists independently.
- Artist **search** is still not built. ADR-0021 already ranks by which field a
  query landed in and throws that information away at the album fold; the
  artists wall narrows by *whose records matched*, which is a projection of the
  record search rather than a search of its own. That is the honest reading of
  what the query means today, and a real artist search is a larger change than
  this one.
- An artist is still not admitted to the returns lane, and the rule in
  `docs/BACKLOG.md` still says why: opening is not touching, and admitting one
  would mean a third store — *places I visited* — which `place.rs` refuses by
  name.
