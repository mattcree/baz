# The index rail's fisheye — captures and the numbers they chose

The owner's ask, verbatim: *"the side bar thing is good but can we make sure
that it has a magnification style attempt to allow you to select things. like
mac OS dock. you move your mouse and it makes the hovered item bigger, and the
surrounding ones"*.

Shipped as `crates/baz/src/spine.rs` — the rail's own hand-built widget —
under the motion class ADR-0020's amendment names: **pointer-derived
deformation**, a pure function of the pointer's position with no clock, no
tween and no subscription. `docs/REFUSALS.md`'s motion entry carries the
pointer.

## The falloff function, and the values chosen

`theme::magnify(distance)` — a raised cosine over the rest distance from the
pointer to a slot's centre:

```
scale(d) = 1 + (MAGNIFY_MAX − 1) · (cos(π·|d| / MAGNIFY_REACH) + 1) / 2   for |d| < MAGNIFY_REACH
         = 1                                                              beyond
```

| token | value | why this value |
|---|---|---|
| `MAGNIFY_MAX` | **1.9** | A bound, not taste: `1.9 × SIZE_HEADING` = 19 px keeps the swollen letter's type inside one `RAIL_PITCH` (20), so the lens overdraws its neighbours' air but never their ink and no slot ever has to move. 2.0 — the dock's own ceiling — is the first value to break that. On the captures, 1.9 reads unmistakably dock-like (cap-height ink measures 13 px against 7 at rest, ≈1.86×). |
| `MAGNIFY_REACH` | **60** (3 slots) | Tried 50 (2.5 slots) first: the second neighbour barely moved (≈1.09×) and the lens read as one popped letter. At 60 the profile at slot distances is ≈1.9 · 1.68 · 1.23 · 1.0 — the hovered letter and two visibly-swollen neighbours each side, which is the dock's skirt. Wider, and half the alphabet stirs with every move. |

The falloff is unit-tested (`theme::the_fisheye_peaks_under_the_pointer_and_rests_beyond_its_reach`):
peak exactly `MAGNIFY_MAX` at zero, symmetric, monotone non-increasing, exactly
1 at the reach and beyond — no seam, no ripple.

## What the frames show

`before-*` is the button-column rail this replaced; `after-*` is the spine.
`*-rail.png` is the right-hand 130 px — the strip a reviewer actually judges.
Each scene ships at 1280×860 and 1920×1080.

| scene | what to look at |
|---|---|
| `01-rest` | The rest state. The one deliberate change is the pitch (below); ink, size, alignment and voice are as they were. |
| `02-hover-mid` | The lens over N: 1.9× under the pointer, two visible neighbours each side, `paper` ink on the letter a press would take. Nothing else in the frame differs from rest — the wall cannot reflow because the lane's width is a constant the widget never touches. |
| `03-hover-upper` | The lens near the strip's head, beside the *current* letter (A, medium face) — the two vocabularies compose. |
| `04-hover-edge` | The pointer at `x = W − 2`, in the `HANG` gutter: **the same lens**. The whole lane is the hit surface, so the screen edge — the easiest target a pointer has (Fitts) — belongs to the rail. Presses land there too (verified live: a gutter click at S's y jumped the wall to S). |
| `05-hover-lower` | The lens near the strip's foot. |

Measured off `01-rest` before/after with `magick compare`: every differing
pixel falls in the rail's own 9 px ink column (x ∈ [W−49, W−41] at both
sizes) — the wall, the headers and both bars are pixel-identical.

## The defect the before-frames measured

The rail was drawn at a **16 px pitch**: the view stacked its entries with
`GAP_XS` (4) while `theme::RAIL_PITCH` — its doc explicitly spending the
index's air on the gap — is `RAIL_LINE_H + GAP_SM` = 20, and the rail's
capacity arithmetic already budgeted 20 per entry. Token, doc and budget
against the render: the render was wrong. The spine lays the strip out from
`RAIL_PITCH` itself, so the drawn pitch, the hit-slot height and the elision
capacity are now one number (measured off the frames: 16.0 between every pair
of letters in `before-01`, 20.0 in `after-01`).

## Hit targets, before → after

| | before | after |
|---|---|---|
| a letter | its own glyph box, ~7 × 12 px | the nearest-slot band: **108 × 20 px** (clearance + ink lane + gutter, one `RAIL_PITCH` tall) |
| between letters | dead | the nearest slot's |
| the `HANG` gutter | dead | live — the window edge is a rail target |
| absent letters, `·` marks | inert | inert (drawn, never pressable — `docs/REFUSALS.md`) |

The slots are contiguous and **fixed**: the glyphs swell about centres that
never move, so the hit region never has to chase the visuals — the target
under a swollen letter is the same slot at every scale, and hit zones can
never lag the glyphs they serve.

## Reproducing

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-rail
toolbox run -c baz-dev env TAG=after docs/design/impl/index-magnification/capture.sh
```

Every run in this directory was made under the six-variable isolation recipe
(docs/DEVELOPMENT.md); the `[mpris] no session bus` receipt printed for each.
