# The index rail's fisheye — captures and the numbers they chose

The owner's asks, verbatim, in order:

1. *"the side bar thing is good but can we make sure that it has a
   magnification style attempt to allow you to select things. like mac OS
   dock. you move your mouse and it makes the hovered item bigger, and the
   surrounding ones"*
2. *"make sure the magnification is more dramatic and make sure the cursor
   changes appropriately. we also are not showing which we are about to select
   by hover causing a highlight"*
3. *"I dunno if the rail needs to be there for all types of grouping, since it
   goes off the edge of the screen"*

Shipped as `crates/baz/src/spine.rs` — the rail's own hand-built widget —
under the motion class ADR-0020's amendment names: **pointer-derived
deformation**, a pure function of the pointer's position with no clock, no
tween and no subscription. The per-key vocabulary decision that answers ask 3
is the amendment trail on ADR-0017 §1.7.

## The deformation: scale × displacement

`theme::magnify(d)` — a raised cosine over the rest distance from the pointer
to a slot's centre:

```
scale(d) = 1 + (MAGNIFY_MAX − 1) · (cos(π·|d| / MAGNIFY_REACH) + 1) / 2   for |d| < MAGNIFY_REACH
         = 1                                                              beyond
```

`theme::magnify_shift(d)` — its integral, the dock's own mechanism: each gap
between two entries stretches by the mean magnification across it, so the
swollen letters sit in room their neighbours vacated. Odd, monotone
(`d + shift(d)` has derivative `magnify(d) ≥ 1`, so entries never cross), and
saturating at `MAGNIFY_SPREAD` — the far field moves as one piece and keeps
its rest spacing.

| token | value | why this value |
|---|---|---|
| `MAGNIFY_MAX` | **2.5** | Shipped at 1.9 first, bounded so no glyph left its fixed 20 px slot; the owner's desktop verdict was "too subtle" (ask 2). 2.5 is the dock's territory, and it is affordable exactly because the strip now displaces: the peak letter's room is vacated by its neighbours, not stolen from them. The width bound holds — the widest letter at peak measures ~26 px against the 60 px ink lane (`font.rs` asserts it). |
| `MAGNIFY_REACH` | **60** (3 slots) | The falloff profile at slot distances is 2.5 · 2.1 · 1.4 · 1.0 — a peak with two visibly-swollen neighbours each side. Narrower popped single letters; wider stirs half the alphabet. |
| `MAGNIFY_SPREAD` | **45** = (2.5−1)·60/2 | The area under the falloff's hump: what the far field displaces by, and therefore the air the widget's elision capacity reserves at each strip end so the spread can never push a letter out of the lane. Where a non-elided strip has less air than this, the shift is capped at the air that exists and the spread degrades before anything clips. |

Both functions are unit-tested (`theme.rs`): peak/symmetry/monotonicity/exact
rest for the falloff; the integral relation, order preservation, saturation
and — the property everything hangs on — **hit-order preservation** for the
shift: `|d + shift(d)|` grows with `|d|`, so the glyph the lens holds biggest,
the slot a press fires, and the chip the hover draws are provably one letter.

## Cursor and hover highlight (ask 2)

- **Cursor**: `mouse_interaction` answers the hand (`Interaction::Pointer`)
  over any slot a press would jump — the letter, the gap between letters, the
  clearance, and the `HANG` gutter out to the window edge — and nothing
  (`Interaction::None`) over absent letters, `·` marks, and the strip's empty
  head and foot. Proven in-frame: the hover captures are taken with `maim`,
  which draws the real cursor (`import` does not), and the hand is visible on
  the letters and in the gutter; an arrow over absent `#`.
- **Highlight**: the winning slot — the hit test's answer, never a different
  letter — carries the rail's press vocabulary back from its button days: the
  `ink_wash` chip behind the glyph's box and ink lifted to full paper, the
  same family the top bar's group words use (`theme::group_key`). Never the
  accent; never on an absent letter. Asserted off a recording renderer in
  `spine.rs` (chip position, wash colour, one chip only, none over gaps).

## The capacity bug (ask 3, first half)

The owner's "goes off the edge of the screen" reproduces at short windows
(`bug-500-*.png`, the merged build at 1280×500: the strip runs under the
bottom bar and clips): the view fitted the rail against `Shelf::grid_size`,
whose height between scroll events is an **estimate that ignores the bottom
bar** (`size.height − TOP_BAR_H`), so at launch and after every resize the
capacity admitted ~5 slots more than the lane holds. The old view also drew a
16 px pitch against a 20 px budget, which under-filled by exactly enough to
mask the overstatement — the pitch correction exposed it.

Fix: **the widget elides, not the view.** `Spine` receives the whole rail and
fits it inside `layout`'s real bounds per frame (`rail::elide` against
`capacity(bounds.height)`), so the admitted strip cannot disagree with the
height that exists. Pinned by `the_rail_never_outgrows_the_lane_it_is_given`
(every `GroupKey` with 60–70 adversarial groups, heights swept 0–1400) and
`a_short_lane_shows_the_elided_strip_it_can_hold` (widget-level, presses
included). `fixed-500-*.png` is the same window on this branch: `# A–I · Z`,
fitted, with the lens's travel reserved.

## The vocabulary decision (ask 3, second half)

Recorded on ADR-0017 §1.7's amendment trail. Short form — the test is *can
the reader guess the vocabulary and its order without reading it?*:

| key | vocabulary | verdict |
|---|---|---|
| ARTIST | `#` + A–Z | keep |
| YEAR | decades between extremes | keep (bounded by construction) |
| ADDED / PLAYED | the recency scale | keep (ordered, interpolable when elided) |
| GENRE | ~~names~~ → **initials** on the A–Z frame | keep, vocabulary changed — names are unguessable and elide into unaimable dots; spellings are an alphabet, and a letter jumps to the first genre spelled with it (`rail::genre`; `genre-initials-*.png`) |

Rejected: dropping the rail for any key (it is the wall's only scroll
affordance since ADR-0022 deleted the scrollbar), and keeping genre names
(the failed vocabulary was the problem, not the rail). ADR-0019 §4's refusal
to invent a taxonomy stands: an absent `B` states a fact about spellings.

## What the frames show

`before-*` is the button-column rail all of this replaced; `after-*` is the
spine as of this branch. `*-rail.png` is the right-hand 170 px. Scenes at
1280×860 and 1920×1080:

| scene | what to look at |
|---|---|
| `01-rest` | The rest state: pitch 20 (the token's number — `before` drew 16), ink, alignment and voice otherwise as they were. No cursor in frame by design, so rest diffs cleanly. |
| `02-hover-mid` | The lens over N at 2.5×: chip + paper ink + hand cursor on N, M/O swollen and pushed apart, the far field shifted as one piece into the reserved air. |
| `03-hover-upper` | The lens beside the *current* letter (A, medium face): the two vocabularies compose. |
| `04-hover-edge` | The pointer at `x = W − 2`: the same lens, same winner — the whole lane is the hit surface, so the screen edge belongs to the rail (Fitts). Live-verified: a gutter click at S's y jumps the wall to S; a press on absent `#` changes nothing. |
| `05-hover-lower` | The lens near the strip's foot. |
| `bug-500` / `fixed-500` | The owner's overflow, reproduced on his build and gone on this one (above). |
| `genre-initials` | The GENRE rail speaking initials. |

Measured off `01-rest` before/after with `magick compare`: every differing
pixel falls in the rail's own 9 px ink column (x ∈ [W−49, W−41] at both
sizes) — the wall, the headers and both bars are pixel-identical.

## Reproducing

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-rail
toolbox run -c baz-dev env TAG=after docs/design/impl/index-magnification/capture.sh
```

Every run in this directory was made under the six-variable isolation recipe
(docs/DEVELOPMENT.md); the `[mpris] no session bus` receipt printed for each.
The hover shots need `maim` in the toolbox (installed for this work) — it is
the grabber that draws the cursor.
