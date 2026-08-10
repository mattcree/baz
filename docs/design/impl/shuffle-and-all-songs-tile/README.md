# Shuffle that does not mutate the run, and `All songs` as a tile

Headless captures of the two things the owner asked for on 2026-08-10, both of
them *after* looking at what that morning's work had shipped as:

> *"I think shuffle as a concept is more about going to an unknown next track
> rather than actually mutating the track list if that makes sense."*
>
> *"again I wanted the Play all, to be more like a tile on the home screen, a
> special 'playlist'."*

The direct descendant of [`../shuffle-and-all-songs/`](../shuffle-and-all-songs/),
whose central experiment no longer describes the product. That harness
photographed the run **before** shuffle, the run **shuffled**, and the run
**restored**, and compared the first against the third at zero differing pixels.
There is nothing to restore now, because nothing is changed. **The experiment
inverts, and the inversion is the whole claim**: the rows must be identical off,
on, and off again; what moves is which row carries which mark.

Reproduce:

```sh
toolbox run -c baz-dev cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-tile-fixture
toolbox run -c baz-dev docs/design/impl/shuffle-and-all-songs-tile/capture.sh
```

The fixture is the composition audit's own: 25 albums, 206 tracks, every sample
a zero. **Nothing is audible** — the samples are silence *and* the scratch
`.asoundrc` points the default PCM at `null`, which are two independent
guarantees rather than one. `BAZ_DEVICE_TESTS` is unset throughout.

**Nothing touched the owner's session.** The run takes all six XDG redirections
from `docs/DEVELOPMENT.md` §"Headless UI verification" and prints its own
receipt:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

The other three lines the run printed, which are the two features narrating
themselves:

```
[all-songs] play everything — 25 records · 206 songs · 17:58:06
[shuffle] on — the run keeps its own order; the walk changed
[shuffle] off — the run keeps its own order; the walk changed
```

## The frames

| | |
|---|---|
| [`01-home-with-the-all-songs-tile`](01-home-with-the-all-songs-tile.png) | **Home, with the tile.** Nothing has played, so `CONTINUE` is absent and the tile is the first thing on the page — the ordinary state of Home, and the case the placement argument turns on. It is one column of the wall's own grid at the wall's own density, so it stands on the same lattice as `RECENTLY ADDED` below rather than on a second measure. The caption states the scope: `All songs · 25 records · 206 songs · 17:58:06`. |
| [`02-the-tiles-hover-options`](02-the-tiles-hover-options.png) | **The wall's own hover veil**, drawn by the wall's own function. Two options where a record has four, and the two that are missing are the two an implicit list cannot answer: `Add to…` is refused by construction (`Origin::file()` is `None`, so there is no file to append to) and `Queue` would append a library to a run. `Play` wears the accent; `Open` goes to the wall, which is where this list is looked at. The state rule under the caption lights, exactly as a record tile's does. |
| [`03-the-run-shuffle-off`](03-the-run-shuffle-off.png) | **The run the tile started, shuffle off.** 206 tracks, `2 of 206 · 17:53:38 left`. Row 2 carries the filled lamp dot — *sounding* — and row 3 carries the **open ring** — *next*. The ring is drawn in both modes because the fact is true in both; with shuffle off it is simply the row below, which costs the ordinary case nothing. |
| [`04-shuffle-on-tooltip`](04-shuffle-on-tooltip.png) | The bar's crossed arrows, pressed and lit in the accent, with the tooltip that says which way the next press goes: *"Shuffle is on — turn it off to play the run in its own order."* Neither reading mentions the list, because the list is not what this control touches. |
| [`05-the-run-shuffle-on`](05-the-run-shuffle-on.png) | **Shuffle on. The rows have not moved.** `Undertow 1`, `Marginalia 2`, `Sixth Street 3`, `Blue Hour 4` … in the order `Play all` built them, with their durations unchanged. What changed is the marking — the entries the pass is already past are dimmed — and the reading: `11 of 206 · 17:01:08 left` is how far through *the pass* the run is and what is left of *the bag*, not a row number and not the list's tail. The next entry is deeper in the bag than this window draws, which is honest and is why frames 11–12 exist. |
| [`06-the-run-shuffle-off-again`](06-the-run-shuffle-off-again.png) | **Off again**, and the run is in its own order because it never left it. `2 of 206` again, the ring back on row 3. Nothing was restored; there was nothing to restore. |
| [`07-diff-durations-on-vs-off`](07-diff-durations-on-vs-off.png) · [`08-diff-durations-off-again-vs-off`](08-diff-durations-off-again-vs-off.png) | The two zero-difference images (see the measurement below). |
| [`09-diff-column-on-vs-off`](09-diff-column-on-vs-off.png) | The opposite claim: the whole column *does* differ, so the first two are not satisfied by a mode that did nothing. |
| [`10-the-strip-keeps-play-all`](10-the-strip-keeps-play-all.png) | **`Play all` is still in the strip**, beside the six words that arrange the wall. It is not the tile relocated: it plays exactly what the wall shows, which is the only way to play a handful of search results. `ACTS_W` is untouched at 88. |
| [`11-a-record-shuffle-off`](11-a-record-shuffle-off.png) | **The mark at record scale**, where the whole run is nine rows and nothing can be off screen. Shuffle off: dot on `Anhydrous 2`, ring on `Nightwatch 3`. `2 of 9 · 41:05 left`; the bar reads `then 7 more · 41:05 left`. |
| [`12-a-record-shuffle-on`](12-a-record-shuffle-on.png) | **The money shot.** Shuffle on, same nine rows in the same order with the same durations — and the ring is now on `Marginalia 8`. Rows 1, 3, 5, 7 and 9 are dimmed: the pass has been past them. `5 of 9 · 24:34 left`, and the bar reads `then 4 more · 24:34 left` — the bag's remainder, not the list's tail. **This is the whole feature in one frame**: the list is untouched, the walk is different, and baz says where it is going. |
| [`13-diff-durations-one-record`](13-diff-durations-one-record.png) | Zero, over the nine-row run. |

## The measurement, and why it is the duration lane

The claim is *the rows do not move*, and a naive pixel diff of the run column
cannot express it: the marks legitimately change, and they are pixels in the
same column. So the comparison is over a **crop of the duration lane**.

`views::queue`'s row draws the duration in `paper_faint` **unconditionally**,
where the number lane carries the dot and the ring and the title lane dims
behind the pass. The durations are therefore invariant to every row state and
can change only if a row *moves*. A zero there is "the list was not permuted",
and nothing else.

| what | crop | measured | expected |
|---|---|---|---|
| the durations, shuffle on vs. off | `50x580+1075+195` | **0** | 0 — no row moved |
| the durations, off again vs. off | `50x580+1075+195` | **0** | 0 |
| the whole run column, on vs. off | `340x580+790+195` | **2 309** | not 0 — the marks moved |
| the durations, one record, on vs. off | `50x580+1075+195` | **0** | 0 |

The third row is the control. Without it the first two would be satisfied by a
shuffle that never shuffled — which is exactly the trap the predecessor harness
guarded against from the other direction.

Both crops are of the run column only. The bar's elapsed figure and the needle
move between any two shots taken seconds apart, so a whole-frame `AE` would
count the clock as a difference and answer a question nobody asked.

## What the frames do not show, and where it is proved instead

**Gaplessness with shuffle on.** A screenshot cannot photograph a splice. The
acceptance test is
`baz-core/tests/engine.rs::a_shuffled_run_is_gapless_and_bit_identical`: a
two-track queue under a traversal that walks it `[1, 0]`, with the delivered
stream compared **sample for sample** against the reference decodes concatenated
in the bag's order. A shuffle that chose its next track when the current one
ended could not have decoded it in time, and the silence, the click or the
repeated block that followed would be a length or a value mismatch there. Every
pre-existing gapless test in that file passes **unchanged**, which is the other
half of the claim.

**That the tile refuses to be a destination.** `implicit.rs` pins that the list
carries no provenance and `menu.rs` sweeps every target and every reachable set
of facts for `Add to "All songs"`. It is closed at its source rather than
photographed.
