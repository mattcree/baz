# Shuffle as a property, and All songs — the render harness

Headless captures of the three things the owner asked for on 2026-08-09 and
2026-08-10:

> *"please can we remove pull since it doesn't make sense here."*
>
> *"can you make shuffle a property of the player i.e. toggle on/off."*
>
> *"the 'all songs' should be an implicit playlist."*

The direct descendant of [`../shuffle-and-pull/`](../shuffle-and-pull/), whose
subjects no longer exist: `Pull` was removed, and shuffle stopped being a draw
from the wall, so that harness's pool-dimming and ring frames photograph a
feature that is gone. This one photographs what replaced it.

Reproduce:

```sh
toolbox run -c baz-dev cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-shuffle-fixture
toolbox run -c baz-dev docs/design/impl/shuffle-and-all-songs/capture.sh
```

The fixture is the composition audit's own: 25 albums, 206 tracks, every sample
a zero. **Nothing is audible** — the samples are silence *and* the scratch
`.asoundrc` points the default PCM at `null`, which are two independent
guarantees rather than one. `BAZ_DEVICE_TESTS` is unset throughout.

**Nothing touches the owner's session.** The run takes all six XDG
redirections from `docs/DEVELOPMENT.md` on a private Xvfb, and the receipt that
it did is the line the script prints at the end:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

The `device-output` feature is needed for the bottom bar to be drawn at all —
without it `Availability::NotBuilt` hides the playback UI, and the shuffle
toggle is *on* that bar. That is why this harness runs in the toolbox where a
host build would do: an earlier attempt on the host photographed a window with
no bottom bar and no toggle in it.

## The frames

| | |
|---|---|
| `01-the-strip-without-pull` | The Library strip. The acts cluster is `Play all` and nothing else — `Pull` removed, `Shuffle` moved to the bar. `ACTS_W` 182 → 88 is this frame. |
| `02-shuffle-off-with-a-run` | The bar with a run and shuffle off: the crossed arrows at the head of the right-hand zone, in resting glyph ink beside the volume. |
| `03-the-run-before-shuffle` | The queue as `Play all` built it — the wall's own arrangement. **The control frame for the comparison below.** |
| `04-shuffle-on` | The same bar, toggled. Lit is the **accent**, the one place beside `Play album` the accent discipline admits. |
| `05-shuffle-on-tooltip` | The hover sentence — the second channel the state is carried on, so the lit state is not colour alone. |
| `06-the-run-shuffled` | The run re-ordered. |
| `07-shuffle-off-again` | Toggled back; the glyph returns to resting ink. |
| `08-the-run-restored` | The run after on-then-off. |
| `09-diff-restored-vs-before` | `03` vs `08`. |
| `10-diff-shuffled-vs-before` | `03` vs `06`. |
| `11-all-songs-in-the-panel` | **All songs** at the head of the playlist panel: collage sleeve, `25 records · 206 songs · 17:58:06`, above the Queue. |

## What the captures actually establish

**Turning shuffle off restores the unshuffled order**, and this is the frame
pair that says so. The script crops the rows column out of `03` and `08` and
compares them:

```
  the rows: restored vs. before  (must be 0):
0 (0)

  the rows: shuffled vs. before  (must not be 0):
6.51942e+08 (9948)
```

**Zero differing pixels.** The second comparison is there so the first cannot
be satisfied by a shuffle that never shuffled.

Two things about the method are worth stating, because both were wrong first:

- **Playback is paused before the three run frames are taken.** The fixture is
  silent against a null device, so the engine drains a track as fast as it can
  decode one and the cursor walks the queue while the camera is loading. An
  earlier version compared two frames taken eight seconds apart and measured
  the playhead moving. Paused, the only thing that can differ between `03` and
  `08` is what this feature does to the order.
- **The comparison is over a crop of the rows, not the whole frame.** The bar's
  elapsed figure and the needle move between any two shots; a whole-frame `AE`
  counts the clock as a difference and answers a question nobody asked.

**What is behind the needle does not re-order.** Read `03` and `06` side by
side at the head of the list:

| `03` before | `06` shuffled |
|---|---|
| *Ochre* — Undertow 1 | *Ochre* — Undertow 1 |
| **Marginalia 2** ● | **Marginalia 2** ● |
| Sixth Street 3 | *Meadowgrass* — Cassette Weather 2 |
| Blue Hour 4 | … |

The played row and the sounding row stand; the third row onward is a different
record. That is `arranged(queue, seed, keep)` with `keep` = the playing row + 1,
and the log line beside it says the same thing in words:

```
[all-songs] play — 25 records · 206 songs · 17:58:06
[shuffle] on — the run re-arranged from row 3
[shuffle] off — the run re-arranged from row 5
```

**The mode never stops the music.** Both toggles go out as `UpdateQueue`; the
`[playback] track started` lines continue across both without a gap, and the
bar in `04` and `07` is still playing.

**All songs is a list and not a destination.** `11` shows it drawn as a
playlist — sleeve, name, counts — at the head of the directory, above the
unnamed sounding list it is a sibling of. Its counts are the wall's:
`25 records · 206 songs`, and under a query they would read `7 of 25 records`,
because a list called *All songs* that held seven of twenty-five would be
lying. There is no `Add` beside it in any state and there cannot be: it has no
file. That negative is asserted where a picture cannot carry it — `menu.rs`
sweeps every target and every reachable `Facts`, and `all_songs.rs` closes the
upstream half by pinning that the list's run carries no provenance.

## What is *not* captured, and why

**The picker mid-pick.** Opening the panel as a picker needs a `+` press on a
row, and the panel's own rows are the only ones on screen at that moment; the
frame would show the refusal but not prove it, since a screenshot cannot show
that a control is absent *in every state*. The tests do that.

**All songs' own page.** There isn't one, deliberately — doc 09 §2 names the
wall itself as where this list is seen, and a second surface listing the same
music as text would be doc 07 L8.6's one fact drawn twice, drawn worse and
without the art. `01` is that page: the wall *is* the list.

## Coordinates

Every step is driven by **pointer**, not by keys, and that is not stylistic.
The search well holds focus from launch and now lives in the returns lane
(ADR-0030's search amendment), so iced's `text_input` consumes bare letters
wherever the pointer is — an earlier version pressed `q` for the Queue door
and photographed a search for `"qq"` instead. Every control here has a
visible, pointer-reachable form, which is the product's own rule, so the
harness uses those.

The coordinates in the script are read off `01` and `02` at exactly
1280 × 860 with the lane open. Change `W` or `H` and they must be re-read.
