# ADR-0021: Search ranking — what ranks first is what plays

**Status**: accepted (2026-08-08) · implements step 12 of ADR-0017's build plan
· closes the deferral ADR-0019 recorded in its *"What this ADR does not do"* ·
**amends the documented contract of `Library::search`** (results are no longer
in library order)

## Context

`Library::search` has always returned **corpus order** — the order tracks
happen to sit in the in-RAM index, which is library order — truncated at
`limit`. ADR-0019 looked at ranking, left it, and said why: *"ranking changes
`search`'s documented contract and the tests that pin its determinism, which is
a decision with its own shape"*. This is that decision.

Corpus order is a perfectly good answer for a **filter**. It stops being one
the moment a result is *chosen*, and the adopted design chooses one:

> **Find — type anywhere [v1]**: any bare keystroke filters the wall next
> frame; **Enter plays first match**; Esc clears.
> — `docs/design/critique/02-surfaces.md`, adopted by ADR-0017 §1.2

So the first result is not a position in a list, it is **the thing that comes
out of the speakers**. Under corpus order, typing `kid` and pressing Enter
plays whichever matching track is alphabetically earliest by album artist —
`Kids Everywhere` by the Aardvark Collective, not `Kid A`. ADR-0017 §5 states
the dependency in one line: *"navigation makes ranking a requirement"*.

The other half of the problem is at the seam. The wall draws **albums**;
`Library::search` returns **tracks**; `crates/baz/src/vm.rs` folds one onto the
other with a `HashSet` and says so plainly in its own doc comment — *"no
relevance reordering — the shelf is a place, not a ranking"*. That was honest
while there was no ranking to lose. With one, folding a *track* cap onto an
*album* question silently drops records: an album whose only matching track
fell outside the cap disappears from the wall, and which albums survive depends
on how many tracks the ones before them happened to match.

## Decision

### 1. Three signals, compared in order. No formula, no weights.

A result's rank is a lexicographic comparison of three small ordered values.
There is no score, no arithmetic and nothing to tune, which means any two
results can be explained by naming the first signal that separates them — the
property the whole exercise was for.

**1. How well the query fits the field it landed in.** Six tiers, best first:

| Tier | The query… | `kid` against |
|---|---|---|
| `Exact` | is the whole field | `Kid` |
| `PrefixWord` | starts the field, ends on a word boundary | `Kid A` |
| `Prefix` | starts the field, ends inside a word | `Kids` |
| `Word` | starts a later word, ends on a boundary | `The Kid` |
| `WordStart` | starts a later word, ends inside it | `The Kidz` |
| `Fragment` | starts inside a word | `Skid Row` |

**2. Which field it landed in**: artist (track or album artist), then album
title, then track title.

**3. Library order** — album artist, album, disc, track, title, path. The order
`Library::tracks` already yields.

A track matching in several places is ranked by its **best** match.

### 2. Why fit comes before field

This is the only ordering decision in the model that could plausibly have gone
the other way, and the case that settles it is concrete. Query `yesterday`:

- *Yesterdays New Quintet* — an **artist** match, `Prefix` (it starts the
  field, but inside a word).
- The Beatles' *Yesterday* — a **title** match, `Exact`.

Ranked by field first, Enter plays a Yesterdays New Quintet track and the
listener's entire library of Beatles records sits below a jazz outfit they were
not thinking of. The fit of a match is the *evidence about what was meant*; the
field is a fact about where the evidence was found. Evidence first.

The field still earns its place as the tiebreak, and it breaks upwards — artist,
then album, then title — because at **equal fit** the artist names a whole body
of work and the album names a record. Preferring them puts the broadest true
answer first and keeps a discography together at the top instead of interleaved
with songs that happen to share a word.

Within the first signal, **position beats completeness**: `Kids` (a record whose
name *starts* with what was typed) outranks `The Kid` (a whole word, but later
in the name). A listener types the beginning of the name they are thinking of,
and every incremental find in every other program — address bars, file pickers,
command palettes — behaves this way. `Exact` sits above both because it is
maximal on both axes at once.

**Word boundaries are read, never assumed.** A boundary is a neighbouring
character that is not alphanumeric, or the absence of one. Scripts that do not
space their words therefore reach `Fragment` for an interior substring and
`Exact` for a whole field, and nothing in between — which is the honest reading:
there is no boundary evidence in `東京事変`, so none is claimed. Both CJK cases
are still *found*, exactly as before; only their rank is affected, and it is
affected by a rule that made no claim it could not support.

### 3. Album coherence is grouping, not a count

Matching tracks are kept **together by album**, an album taking the rank of its
best-matching track. Ranked track by track, a query for `moon` would return
`Moon`, then `Moonchild` from a different record, then `Half Moonlight` from
the first one — a record read out in two pieces with someone else's in between.
The wall draws albums; scattering one across the results makes a strong hit look
like several weak ones.

**Nothing is scored by how many tracks matched.** That was considered and
rejected: a thirty-track compilation would outrank a four-track EP for a reason
the query never asked about, and "more of this record matched" is not evidence
that this record is the one meant. `more_matching_tracks_never_outrank_one_
better_match` asserts it.

### 4. Determinism is a contract, and it is held by construction

The order is **total**: the third signal is library order, which is already
total (the unique path is its final tiebreak, ADR-0003). Equal ranks are never
compared — they are left where the corpus scan put them, by *stable* sorting —
so library order is the tiebreak without ever being a comparison.

The ranking machinery is a **counting sort**, not a comparison sort: there are
eighteen possible ranks (six tiers × three fields) whatever the library's size,
so albums are bucketed in one linear pass. `Relevance::code` and `Relevance`'s
`Ord` have to agree for that to be correct, and
`relevance_codes_are_ordered_like_the_comparison` checks them against each other
over every value that exists rather than trusting the arithmetic.

`ranking_is_deterministic_and_independent_of_insertion_order` builds the same
five tracks three ways — in order, reversed, and one batch at a time as a scan
in progress would — and asserts one answer, repeatedly.

### 5. `Library::search_albums`, so the ranking survives the mapping

```rust
pub fn search_albums(&self, query: &str, limit: usize) -> Vec<Album<'_>>
```

Same query, same ranking, projected onto albums: an album's rank is its
best-matching track's, and it appears exactly once however many of its tracks
matched. `Library::search` keeps its signature and its meaning — the inspector,
a future track list and anything track-shaped still needs it — and both are
projections of *one* ranked answer, so a front end cannot get one order from one
call and a different order from the other.

The point is the cap. `search_albums(query, 10)` caps **the answer**;
`search(query, 500)` folded onto albums caps **the working set**.
`search_albums_does_not_lose_an_album_to_a_track_cap` builds the case where
those differ — one record with sixty matching tracks and one with a single
better one — and shows the second album vanishing from the folded track search
and surviving the album search.

This costs `crates/baz` nothing today (nothing here changes `vm.rs`), and it is
what step 8's wall should call.

**Implementation note.** `Library::albums` and `search_albums` now build their
`Album` values from the same album runs, recorded in the index at sort time
(`SearchIndex::album_starts` / `album_of`, two `usize` vectors — ~800 KB at
100k tracks). A search therefore builds only the albums it matched instead of
building the whole shelf and filtering it, and the shelf's notion of "one
album" and the ranking's cannot drift apart, because there is only one.

### 6. The candidate cap: `Library::RANKED_CANDIDATES = 4096`

Ranking has to **see** a match before it can call it first, so it cannot stop
early the way an unranked filter could. Measured over 100k tracks, ranking every
match cost:

| Query | Matches | Ranking every match |
|---|---|---|
| `silver` (a common word) | ~30 % of the library | **1.17 ms** |
| `e` (**the first keystroke of every search**) | ~all of it, several times each | **11.6 ms** |

The second row is the one that decides it. A type-anywhere find has no search
box to focus, so the *first* keystroke is always a one-character query — it is
not a pathological case, it is the case that happens every single time anyone
searches. 11.6 ms is most of a 60 Hz frame, against a friction budget that reads
*"keystroke → filtered wall = next frame"*, before the wall has laid out or
drawn anything.

So **`search` ranks the first 4096 matching tracks in library order, not every
match in the library.** That puts the worst case at **397 µs**.

**What it costs, stated plainly**: for a query matching more than 4096 tracks, a
better match beyond the cap is not seen.
`ranking_examines_a_capped_prefix_of_library_order_and_says_so` asserts exactly
that — including the failure, not only the success — because a documented limit
that no test can demonstrate is a hope.

Three things make it the right trade rather than a quiet reintroduction of the
bug this ADR exists to fix:

- The old code ranked nothing at all within a working set of `limit` (50, or the
  front end's 500). The new one ranks within 4096 — **eight to eighty times
  larger** — and *inside it corpus position decides nothing*.
- The cap binds only on queries that have **not narrowed anything yet**. A query
  matching 4 % of a 100k library is one or two characters in; the match set
  shrinks roughly geometrically per keystroke, and by the time a query is
  specific enough for Enter to mean something, every match fits inside the cap
  and the ranking is exact. Every worked example in this ADR is in that regime.
- It is **deterministic**. A prefix of library order is a stable, reproducible
  set, so "the same query always returns the same list" survives intact.

A cap tied to `limit` was rejected: `search` is capped in tracks and
`search_albums` in albums, and one album can hold any number of tracks, so a
limit-derived candidate set would mean the two projections disagreed about which
matches they had even looked at.

## Measurements

`crates/baz-core/benches/search.rs`, 100k synthetic tracks, criterion, 100
samples, same host and session for both columns; the median of the reported
interval. `first_keystroke` is new in this ADR — the old bench had no
one-character query, which is why nobody had noticed that the cheapest-looking
line in it was the most expensive one.

| Query | Before (unranked) | After — `search` | After — `search_albums` |
|---|---|---|---|
| `velvet sparrow` (selective) | 129 µs | 177 µs | 181 µs |
| `GRÖßENWAHN` (folded) | 61 µs | 159 µs | 162 µs |
| `東京` (CJK) | 151 µs | 176 µs | 182 µs |
| `silver` (common word) | 1.5 µs | 132 µs | 141 µs |
| `zyzzyva quartet` (total miss) | 133 µs | 133 µs | 133 µs |
| `e` (first keystroke) | 4.8 µs | **397 µs** | **407 µs** |

The "before" column is flattering and should be read carefully: the old search
stopped at the first `limit` matches, so its cheap lines are cheap *because it
did no ranking and looked at almost nothing*. `total_miss` — the only query
where both implementations must scan the entire corpus and neither has anything
to rank — is unchanged at 133 µs, which is the honest fixed cost of a
100k-track scan on this host. Everything else in the "after" column is that
scan plus ranking.

Two optimisations were needed to get there and both are in the code with their
reasons: the **candidate cap** (§6), and a **field cursor** — matches inside one
track arrive at ascending offsets, so the haystack's four fields are walked
forward once per *track* instead of being re-derived per *match*. The cursor
alone took the first keystroke from 500 µs to 397 µs and a common word from
151 µs to 132 µs, and `the_field_cursor_lands_where_a_walk_from_the_start_would`
proves it is a speed trick and only a speed trick, by checking every resume
point against a fresh walk for every byte of a real haystack.

**Zero new dependencies.** `memchr` was already there. A fuzzy-match crate was
never seriously in play: the requirement was a model that can be *explained*,
and an edit-distance score is a number nobody can argue with or predict.

## Consequences

- `Library::search` returns **ranked** results. Its documented contract changes
  from "library order" to "best match first, library order as the final
  tiebreak"; the module docs, the method docs and `benches/search.rs` all say so.
  Determinism, the limit, empty-query and separator behaviour are unchanged and
  their existing tests pass unmodified.
- `Library::search_albums` and `Library::RANKED_CANDIDATES` are new public API.
  Nothing is removed.
- `crates/baz` is untouched and its behaviour is unchanged in kind: it folds
  search results onto album ids in a `HashSet`, so it now folds a better-ordered
  list. Adopting `search_albums` is step 8's work.
- The index carries two more `usize` vectors (the album runs). They are built in
  the sort pass that already existed, off the per-keystroke path.
- The ranking types (`MatchTier`, `SearchField`, `Relevance`) are **private**.
  The model is a behaviour, and the tests assert it as one — the ordering of
  results — rather than reading the implementation back to itself
  (`docs/ENGINEERING.md`: *"tests are written to specification, not to
  implementation"*). Only the classifier's own unit tests, which are in-crate,
  name the tiers.

### What this ADR does *not* do

- **No UI.** Type-anywhere itself (ADR-0017 §1.2), the query's display and the
  match count are front-end work. This is the answer they will ask for.
- **No new matching.** The corpus, the folding, the field set and the separator
  rule are ADR-0003's and ADR-0008's, unchanged. Ranking reorders what search
  already found; it does not find anything new, and a query that matched nothing
  before still matches nothing.
- **No stemming, no synonyms, no transliteration.** `beetles` does not find the
  Beatles and `tokyo jihen` does not find `東京事変`. Each of those is a
  different feature with its own argument to make, and none of them can be made
  by a ranking.

### Deliberately deferred

- **Ranking by anything the listener has done.** Play counts, recency and skip
  history are all in the ledger (ADR-0018) and all tempting as a fourth signal.
  They are deferred because they make the answer depend on state the listener
  cannot see, and "why did that come first" would stop having an answer that
  fits in a sentence. If one arrives it goes *below* the three signals here, as a
  tiebreak among equals, not above them.
- **Multi-word queries as multiple terms.** `kid a` is one literal substring, so
  it does not match `A Kid`. Treating a query as a set of terms is a real
  improvement and a real change to what search *finds* — which makes it ADR-0003
  territory, not this ADR's.
- **Raising or removing the cap.** It is a measured number on one host, and the
  measurement is in this document so a future change to it has to beat the same
  bench rather than an opinion.
