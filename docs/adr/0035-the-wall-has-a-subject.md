# ADR-0035: One word called artist — the key groups by artist, and the sixth word stops existing

**Status**: accepted (2026-08-10) · **amends [ADR-0019](0019-group-keys.md) §2**
(the first key breaks on the artist, not on their initial; §1's projection
promise is untouched and is the reason this is a key at all) · builds on the
Artist place from [ADR-0022](0022-places-and-nothing-else.md)'s line of work ·
closes the costed proposal at `docs/BACKLOG.md` · the frames are
[`docs/design/impl/artists-grouped/`](../design/impl/artists-grouped/)

> **This ADR was accepted twice on one day.** The first form is described
> under *What was deleted* below and its frames are kept at
> [`docs/design/impl/artists-wall/`](../design/impl/artists-wall/); it added a
> sixth word to the strip and a second wall behind it. The owner looked at what
> shipped and named the real defect, which was smaller and one level down. What
> follows is the decision as it stands, not the first decision plus a
> correction. The inventory at the end is there because the first form is on
> `main`'s history and a reader who finds it deserves to know what happened to
> it — and because what a change *removes* is the part of it worth recording.

## Context

baz had **two things called artist**, and they were different things.

- The wall's **`ARTIST` group key** broke records on their album artist's
  *initial*. Its shelves read `Unknown`, `#`, `A`, `C`, `Various`; its index
  rail was the alphabet. It was a *sort*.
- The **Artist place** is a page about a person: their name, their records,
  reached from a record page's `Artist › Album` breadcrumb. It is a *subject*.

One word, one product, two meanings — and they are adjacent on screen, because
the key's word sits in the strip above a wall you can press a tile in to reach
the place.

The owner, in one line, on the first answer to that:

> *"artists should be grouping stuff by artist not just alphabetically"*

Which locates the defect exactly. The word was never the problem. **A key
called `ARTIST` that groups by a letter is a key whose word is false**, and
every way of fixing the *word* leaves that standing.

## Decision

### 1. `GroupKey::Artist` groups by the artist

One shelf per artist, headed by their name. Scrolling the wall reads
*Anne-Marie Puig* and her two records, then *Corvin* and his two, and so on.

**The order of the shelves** is `ArtistKey`'s: unknowns first, then named
artists case-folded alphabetically, then unnamed compilations. That is not a
new rule — it is the order `Library::albums()` has sorted in since ADR-0008,
and the order `Initial`'s variants were already in
(ADR-0019 §2: *"variant order is shelf order"*). The two anonymous ends stay at
the two ends for ADR-0008 §Consequences' reason: an unnamed compilation is not
a `V` and an unreadable artist is not a `U`, so neither may land in the middle
of the alphabet where a sentinel string's letters would have put it. The type
is *literally* reused — `ShelfSort::Artist(ArtistKey)` — because "the order the
artist shelves go in" and "the order the library's albums go in" are the same
sentence, and two copies of one sentence eventually disagree.

**The order within a shelf** is library order, which is album title,
case-folded. Not release year. ADR-0019 §1 set that rule for every key and
ADR-0019's *Deliberately deferred* explicitly held per-shelf ordering back as
"a view decision with no evidence behind it yet"; nothing here is new evidence.
A second ordering *within* a shelf would also be a second arrangement control
that nothing on screen explains.

**The header is the spelling that sorts first**, not the first one found.
Identity is case-folded, so `Alpha` and `alpha` are one artist with two
spellings on disk, and the shelf's first album is an order — album title's — a
retag can change. A minimum is a property of the set. This is the same rule and
therefore the same answer as `views::artist::label`, which is what stops a
header and the page it opens from naming one artist two ways; the front end
re-takes the minimum over the albums it actually draws, because `build_album`
drops albums with no readable track and a header must name the records under
it.

### 2. It is an ordinary group key, and that is the whole argument

ADR-0019 §1 promises that every key is a *projection* in which **every album
appears exactly once**, and `baz-core` sweeps `GroupKey::ALL` asserting it.
Grouping albums by their album artist satisfies that promise exactly — every
album has exactly one album artist, including the ones whose files declare
none. So there is no new machinery: no second projection, no subject held
beside the key, no parallel search, no fork in the virtualizer.

It is stronger than "satisfies": `shelves(GroupKey::Artist)` is **`albums()`
with its breaks named**, element for element, which is the property ADR-0019 §1
already asserted for this key and which survives unchanged
(`the_artist_key_is_the_flat_shelf_with_its_breaks_named`). The finer headers
name breaks that were already in the list.

### 3. `A–Z` does not survive, because it never had anything of its own

The obvious shape was to keep the letter grouping beside the artist grouping —
six words, `A–Z` and `ARTIST`. It is not what shipped, and the reason is the
identity above.

`A–Z` grouped albums by their artist's initial. `ARTIST` groups them by their
artist and sorts those groups alphabetically. **Both are `albums()`**; they
differ only in where the headers fall. So the two words would put the same
records on the wall in the same order under two sets of headers, one of which
is strictly coarser. That is not a second arrangement, it is a second *caption*
for the first — and a strip with two words producing one traversal is the
"two things called artist" defect wearing new clothes.

What `A–Z` was actually good for was **jumping to a letter**, which
`03-interface-prior-art.md` R8 named as the single most concrete regression
Sonos users reported. That is not in the headers and never was: it is the index
rail, and the rail still speaks the alphabet (§5). Losing the letter headers
loses no navigation.

The one thing that genuinely changes is **density**: a letter shelf packs many
artists' records into one flowing grid, and a shelf per artist is taller, with
more headers and more short last rows. That is a real difference and it is the
one the owner asked for by name. It is also a difference in *packing* rather
than in arrangement, and the product's answer to packing is the density detents
(ADR-0028), which are one press away at the foot of the rail.

### 4. Nothing was retired, so nothing needs migrating

`GroupKey::code()` is on-disk config data and the type's own doc says a code is
never repurposed. Nothing here repurposes one. **The variant is the same
variant and its code is still `"artist"`** — it names the same key, which
arranges the same albums in the same order, and now says so. Every
`config.toml` baz has ever written resolves, and resolves to the arrangement
its word always claimed.

The *label* moves back: `GroupKey::Artist.label()` is `"Artist"` again, so the
strip reads `ARTIST · YEAR · GENRE · ADDED · PLAYED`. That word is now true —
the key's shelves are artists, and pressing one opens the artist. The collision
ADR-0035 was first written to solve has stopped existing rather than been
renamed around.

The one key that *did* exist and does not now is the config's `wall_subject`
(see *What was deleted*). Its migration is the shape of `config.rs`: every value is read by name,
so a key nothing reads is not read, the arrangement beside it still resolves,
and the next save writes the document without it. Asserted in
`a_wall_subject_from_the_older_release_is_ignored_and_costs_nothing`.

### 5. The header is the door, and the rail is still the alphabet

**The header is a door to `Place::Artist`.** The place stays — the record
page's `Artist › Album` breadcrumb still needs it — and this is now how you
reach it from the wall, which is the job the artist tiles were doing. Same
`vm::artist_id`, so the two doors cannot land on different pages.

The type does not change: same face, same size, same tracking, same
`paper_faint` ink, same line box, in the pinned copy as in the in-flow one —
which is what keeps pinning a *position* rather than a *state* and is why it
needs no transition. What it gains is `theme::word_button`'s ground under the
pointer, the paint the breadcrumb already wears, on **the word's own box** and
not the shelf's width: a band-wide ground would light the whole wall on a
mouse-over, and a padded one would inset the header's ink off the block's left
edge, which is law L1's line. Every other key's header stays inert text,
because a decade is not a place.

**The rail is the alphabet**, still, and this needed checking rather than
assuming. `rail::entries` is a pure function of the shelf headers, and with a
shelf per artist there are far more headers than letters — a rail of one entry
per header would be a list of every artist in a 36 px lane, which is the wall
again. §7.2's premise for an index is that the reader can guess the vocabulary
and aim at it without reading it, and the alphabet is that; four hundred names
are not. So a letter is **the first artist filed under it**: press `C`, land on
Corvin.

This is exactly the shape `rail::genre` arrived at from the other direction and
for the same reason, and it costs the rail no new vocabulary: `Initial::of` is
still the whole mapping. It is asked of `baz-core`'s type rather than taken
from the header's text, which is what keeps `Various Artists` and `Unknown
Artist` at the two ends instead of filed under `V` and `U` in the middle of the
letters.

`Initial` itself is unchanged. It stopped being the wall's header and became
the rail's letter, which is the one place a coarse bucket earns its keep.

### 6. What one query buys, and what the wall's own guard was for

Two things the first form got right are kept, and one is deleted along with
what needed it.

**Kept: one query, spent once.** `Shelf::refilter` spends the search once and
the wall is that answer. There is no second projection to keep in step, because
there is no second wall.

**Kept: the min-spelling rule, agreeing with `views::artist::label`** (§1).

**Deleted: the artists' own art prefetch.** It existed because an artist tile
was a *collage* quoting records that were not on the wall, and the wall's
visible range is the whole of the thumbnail guard — so without it a collage
drew the deterministic gradient until one of the records it quoted happened to
scroll past, which is real artwork by luck and was verbatim the defect the
playlist collages and then the artist page had. There are no artist tiles now:
every tile on the wall is a record, inside the range guard, and the guard is
what it always was. `views::SLEEVE_CELLS` went with it; `views::playlist_sleeve`
stays, because playlists still have sleeves.

## What was deleted, and what the first form taught

The first form, accepted the same day and shipped as `feat/artists-wall`:
`GroupKey::Artist.label()` became `"A–Z"`; `ARTISTS` became a **sixth word** in
the strip's row, holding a `vm::WallSubject` beside `group_key` and persisted
beside it; pressing it put up a wall of **artist tiles**, one per person,
shelved by `Initial`, each a `views::playlist_sleeve` collage, each opening
`Place::Artist`.

Its central argument was that a wall of artists *could not* be a key, because a
wall showing no albums falsifies ADR-0019 §1's sweep. That argument was
correct, and it was an argument about the wrong wall. **Grouping the records
under their artist shows every album exactly once**, so the thing the owner
actually wanted was a key all along, and everything the subject existed to
carry could go:

`vm::WallSubject`, `vm::ArtistVm`, `vm::ArtistShelfVm`, `vm::build_artists`,
`vm::visible_artists`, the four parallel `artist_*` fields on `Shelf`,
`Shelf::show_subject`, the five `wall_*` accessors that let one virtualizer
serve two subjects, `views::shelf::artist_tile`, `views::SLEEVE_CELLS`,
`views::top_bar::subject_word`, the `wall_counts` / `wall_noun` readout split in
both wells, `Message::WallSubjectSelected`, the `6` accelerator, the
`wall_subject` config key, and the artists' art prefetch. Net **−700 lines**
across `crates/`, tests included.

Two of its findings survive it and are worth stating as findings rather than as
history. The **art-prefetch fix was correct and hard-won** — that collage-by-luck
defect has now bitten three surfaces — and the reason it belonged on the wall's
own range guard rather than beside `App::request_offscreen_art` still holds for
anything that quotes off-screen artwork from a scrolling surface: only the
wall's range guard re-fires on a scroll. And the **strip's budget is
arithmetic in both directions**, which the sixth word proved by costing 54 px
and this proves by giving all 54 back.

## The strip's budget, back where it was

Six words came to 366.50 px and `KEYS_W` was the next 4 px step, 368. Five
words come to 312.99 and `KEYS_W` is 314, which is the number it was before
this ADR's first form and is asserted as const arithmetic in `theme.rs`.

| | with the sixth word | now |
|---|---:|---:|
| `KEYS_W` | 368 | **314** |
| `LIBRARY_LINE` | 560 | **506** |
| `SINGLE_LINE` = `TOP_BAR_SPLIT` | 832 | **778** |
| `SINGLE_LINE_NO_WELL` | 608 | **554** |
| `WIDEST_LANE_STRIP` | 720 | 720 (unmoved) |
| `TOP_BAR_FLOOR`, and the window's own minimum | 600 / 696 | 600 / 696 (unmoved) |

`KEYS_SPENT` is kept at **0** rather than deleted, and stated as a difference
from the historical 314 rather than as a literal: what a word costs the strip is
the number the next one is argued against, and a constant that is zero says the
row is where it started. The library line sits 94 px under the floor and the
well-less regime has 166 px of headroom against the narrowest strip it can be
handed — both asserted as differences of the movements that produced them, so
neither can be right by coincidence.

The single-line-with-well band is **778…904**, which is wider than it has ever
been. The costing in `docs/BACKLOG.md` predicted this band would cease to exist
under six words; `Pull` and `Shuffle` left the acts cluster in between and paid
for the word twice over, and then the word left too.

## Consequences

- One word means one thing, because the two things became one thing. `ARTIST`
  arranges the wall by artist and an artist's name on that wall opens their
  place.
- The wall has one subject again: records. Every tile is a record, so every
  readout counts records and the noun is a literal rather than a call.
- **A shelf is now often one short row.** A library of many one-record artists
  is a taller wall with more headers than it was. That is the arrangement the
  owner asked for, and the density detents are the control for how tightly it
  hangs.
- Artist **search** is still not built, unchanged from the first form: ADR-0021
  ranks by which field a query landed in and throws that away at the album fold.
  What narrows now is records, and an artist's shelf survives when one of their
  records does — which is the same honest reading, arrived at with no
  projection to maintain.
- An artist is still not admitted to the returns lane, and the rule in
  `docs/BACKLOG.md` still says why: opening is not touching, and admitting one
  would mean a third store — *places I visited* — which `place.rs` refuses by
  name.
