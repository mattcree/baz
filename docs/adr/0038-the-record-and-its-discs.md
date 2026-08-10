# ADR-0038: The record and its discs — a multi-CD album is one item

**Status**: accepted (2026-08-10)

## Context

The owner, 2026-08-10: *"it would be good if multi CD albums were a single
item"*.

Before assuming that was broken, all four shapes a two-disc rip actually
arrives in were built as real tagged FLAC files
(`docs/design/impl/multi-disc/mkfixture.sh`) and scanned. What the shelf did
with them:

| # | shape | before |
|---|---|---|
| 1 | one `ALBUM` tag, `DISCNUMBER` 1 and 2, one folder | **one record**, ordered (disc, track) |
| 2 | the same, split across `Disc 1/` and `Disc 2/` | **one record**, ordered (disc, track) |
| 3 | `… (Disc 1)`/`(Disc 2)`, `… CD1`/`CD2`, `… [Disc 1]`/`[Disc 2]` | **two records each** — six tiles for three albums |
| 4 | no disc tag at all, two folders, track numbers colliding | **one record**, tracks interleaved `1, 1, 2, 2, 3, 3` |

Two findings, and the first is the one that matters most:

**Ordering was already right.** `disc` is the third field of the library's
`SortKey` — folded album artist, album, **disc**, track, title, path — and has
been since the key existed, with
`albums_order_tracks_by_disc_then_number_then_title` holding it. A merged
two-disc set has never played its two track-ones interleaved. The correctness
bug this ADR was most on guard against was not there.

**Shape 3 is the whole of the ask.** The grouping key is (album artist, album
title), which reads no path at all — so shape 2's folder split is not a fact
the shelf can even see, and shapes 1 and 2 are the same case. Shape 3 is
different in kind: the two discs genuinely carry *different album titles*,
because the ripper put the disc number in the `ALBUM` tag. And this is how a
great many rips arrive.

Shape 4 already merged and already had nothing to order by.

## Decision

### 1. Discs are an axis of one record, not a second grouping

A record is still (album artist, album title) — ADR-0008 is unamended. A disc
is a *position within* that record: it orders the track list and it breaks the
page. It is deliberately **not** a new entity, so nothing downstream had to
learn a new noun.

This composes with editions ([ADR-0007](0007-album-editions.md)) rather than
colliding, because they are perpendicular: **album artist + title** says what
one record is, **codec** says how many editions it has, **disc** says how each
edition's track list is ordered and broken. A two-disc set owned in FLAC and in
MP3 is one record, two editions, two discs each —
`a_two_disc_set_in_two_codecs_is_one_record_with_two_editions_of_two_discs`.

### 2. The disc marker: a closed list, at the end of the title, or nothing

`split_disc_marker` takes a trailing **disc marker** off an album title.
`"Sandinista! [Disc 2]"` → `("Sandinista!", Some(2))`. Five conditions, all
required:

1. At the very **end**, ignoring trailing whitespace.
2. One of exactly three words — `disc`, `disk`, `cd` — ASCII
   case-insensitively. No `part`, no `volume`, no `side`, no roman numerals,
   no abbreviations this list does not name.
3. Followed by **one or two ASCII digits** naming a disc 1–99. `"Compact
   Disc"` is a title; a marker with no number is not a marker.
4. Either wrapped in one **matched bracket pair** — `(…)`, `[…]`, `{…}` — or
   beginning on a **whitespace boundary**. `"…soundtrackcd2"` is left alone.
5. Something is **left**. A record called exactly `"CD 1"` keeps its name,
   because the alternative is a record with no name.

A separator the ripper left behind — whitespace, `-`, `–`, `—`, `,`, `:`, `_`
— comes off with the marker. `.` does not (it ends `Vol.`), nor do `!` and `?`
(they end titles).

**There is no distance, no similarity and no fuzziness anywhere in this**, and
the function's type is what enforces that: it returns a *subslice* of the
title it was given, so it cannot invent a character it was not handed.
`the_disc_marker_rule_is_narrow_and_says_where_it_stops` is the whole table,
accepts and refusals side by side.

### 3. A marker is not enough. There must be a sibling

This is where the ADR spends its honesty, because this is a **guess**, and
[ADR-0008](0008-album-artist-grouping.md) declines to merge album artists
where no signal exists rather than inventing one. Stripping `(Disc 2)` off a
title is exactly such an invention — and it is also what every listener
expects. Both of those are true and the second one wins, narrowly:

**A marker is acted on only when a sibling exists** — two distinct spellings
sharing one base title under one album artist. Concretely:

- `Bitches Brew CD1` **+** `Bitches Brew CD2` → one record, `Bitches Brew`.
- `Spirit of Eden` **+** `Spirit of Eden - Disc 2` → one record, `Spirit of
  Eden`. The unmarked spelling counts as a sibling; a tagger that marked the
  second disc and left the first alone is common.
- `Bitches Brew CD1` **alone** → the record stays called `Bitches Brew CD1`. A
  listener who owns only disc 1 gets no rename, because the rename would buy
  nothing: there is nothing to merge it with, and the tag is what their files
  actually say.
- `Bitches Brew CD1` + `Bitches Brew Live` → two records. The base must match
  to the character; "contains" is not a relation this rule knows.
- `Bitches Brew CD1` by Miles Davis + `Bitches Brew` by somebody else → two
  records. The album artist is half the key and stays half the key.

So the rule can never rename a record it did not merge. That is the precise
statement of what the guess costs and of what it cannot cost.

**What it does cost**, stated plainly: an artist who really released two
records called `Live` and `Live CD1` will see them merged into one called
`Live`, with two discs. Nothing distinguishes that from the case this rule
exists for, and no amount of extra cleverness would — the tags are identical.
It is reversible in the only sense that matters: **no file is written**. The
titles on disk are untouched, the index keeps them verbatim (they are still
searchable — `a_merged_record_is_searchable_by_its_name_and_by_its_tag`), and
deleting the rule restores the old shelf on the next scan.

Because "is there a sibling" is a fact about the *library* and not about the
track, the decision is made once per rebuild over the whole index
(`SearchIndex::merge_discs`) and never per track. `Library::record_title` is
how a front end asks it of a loose `TrackMeta` — a search hit, a playlist
entry — so a Songs row and the tile it doors to cannot compute two different
identities for one record.

### 4. The disc number: tags win, the marker fills the hole, nothing is faked

`disc_of(meta)` is the `DISCNUMBER` tag, else the number the marker in the
track's *own* album title carries. Tag first, always — the marker is a
fallback that fills a hole, exactly as folder inference is for artist and album
(`crate::library`). `a_disc_tag_outranks_a_marker_in_the_title`.

This is the correctness half. A `CD1`/`CD2` rip that never wrote `DISCNUMBER`
— shape 3b, and real — would otherwise merge into a record whose two track-ones
interleave. With it, the merged list plays 1·1, 1·2, 2·1, 2·2.

Three things are deliberately **not** inferred:

- **The folder.** `Disc 1/` and `Disc 2/` are not read. A title is a tag
  somebody wrote about the music; a folder is a place, and shape 4 shows the
  same layout meaning nothing at all. Shape 4 therefore still merges with
  tracks interleaved, and its page draws no disc breaks — the honest rendering
  of a rip that did not say.
- **Disc 1 for an unmarked sibling.** In `Spirit of Eden` + `Spirit of Eden -
  Disc 2`, the unmarked half is not told it is disc 1. Nothing wrote that. It
  sorts first — an unknown disc sorts before a known one, which is exactly
  where an unnumbered disc belongs — and it is drawn with no header above it.
- **Anything, ever, into the files.** baz does not write tags.

### 5. The page breaks, and it already knew how

`views::album` has drawn `DISC n` headers since the run column existed, from
`TrackVm::disc` and `vm::discs`. Two discs' worth of tracks in one flat
`1, 2, 3 … 1, 2, 3` would be worse than not merging, so the merged record had
to reach that machinery, and it does: `TrackVm::disc` is now `disc_of`, so a
`CD1`/`CD2` set that never wrote `DISCNUMBER` gets its breaks.

`vm::discs` gains one clause: a run of tracks naming **no** disc counts as a
disc, when some other track names one. That is the asymmetric rip — half of
`Spirit of Eden` names a disc and half does not — and counting only the named
half would answer "1 disc" for a record that visibly has two, leaving the page
flat. The unnamed run is *counted*, never *numbered*: no header is drawn above
it, because no header is earned. `None` still means "no track named a disc at
all", so a single-disc rip and an untagged one stay distinguishable.

### 6. What counts a record now counts it once

Everything that counts records reads the merged shelf, so all of it followed
for free and was checked rather than assumed: the collection footer
(`Collection::count` over the drawn albums), the artist page's `n records ·
m tracks`, `All songs` and the shuffle bag (`implicit.rs`, which works on
`AlbumVm`s), and search — `search_albums` returns the merged record **once**,
and `matching_album_ids` / `top_match` / the Songs rows resolve to its id.
Three call sites that derived a record id from a loose `TrackMeta` were moved
onto `Library::record_title` so they could not drift: `vm::song_hits`,
`vm::matching_album_ids`, and the playlist page's run headers and sleeve
collage.

## Consequences

- Shape 3 is fixed: three records instead of six, in each of the three
  spellings, ordered and broken by disc.
- Shapes 1 and 2 are unchanged, which is the point — they were already right,
  and the tests now say so rather than leaving it to be re-discovered.
- Shape 4 is unchanged and deliberately unmerged-looking: one record, no disc
  breaks, tracks interleaved. Folder names remain evidence about nothing.
- A lone marked disc keeps its name. This is the declined guess, and it is the
  one a listener with an incomplete rip will see.
- Two genuinely different records whose titles differ only by a disc marker
  will merge. Unfixable without inventing a signal; stated above.
- Cost on the scan path: one suffix parse per track per rebuild, and no
  allocation at all in a library whose titles carry no markers —
  `SearchIndex::merged_records` keeps such a library out of the second pass
  entirely.
- `TrackMeta` is untouched. It is still exactly what reading a file's tags
  yields; every derived answer is computed above it.
