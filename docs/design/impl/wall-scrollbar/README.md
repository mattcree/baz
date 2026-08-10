# The wall's scrollbar moves to the window's edge

> *"scroll bar is in a strange location… it seems to have padding on the
> right"* — the owner, 2026-08-09.

The bar the owner asked for the day before shipped at the right edge of the
**wall's scrollable**, which is the structurally honest place for it and the
wrong place to look. The index rail's `INDEX_LANE_W` 108 stood *outboard* of
it, so a 4 px mark floated in the middle-right with a wide strip of window to
its right holding nothing but twenty-seven single letters. That is what he
described, and a ruler on a rendered frame agrees with him.

## Measured, before and after

`capture.sh` renders the same scenario with the binary from the base commit and
with the binary from this branch, and `measure.py` reads the two x-ranges that
matter off each frame. The full output:

| frame | the bar | the rail's ink | window |
|---|---|---|---|
| `01-lane-open-before-1280` | **x 1168–1171** | x 1226–1239 | 1280 |
| `02-lane-open-after-1280` | **x 1276–1279** | x 1226–1239 | 1280 |
| `01-lane-shut-before-1280` | **x 1168–1171** | x 1226–1239 | 1280 |
| `02-lane-shut-after-1280` | **x 1276–1279** | x 1226–1239 | 1280 |
| `05-lane-open-before-1920` | **x 1808–1811** | x 1866–1879 | 1920 |
| `06-lane-open-after-1920` | **x 1916–1919** | x 1866–1879 | 1920 |
| `05-lane-shut-before-1920` | **x 1808–1811** | x 1866–1879 | 1920 |
| `06-lane-shut-after-1920` | **x 1916–1919** | x 1866–1879 | 1920 |

Read the rail's column: **it does not move**. Neither does anything else. The
bounding box of *every* pixel that differs between `01` and `02` is
`(1168, 49) – (1280, 156)` — the old bar's column and the new one, and nothing
else in a 1280 × 860 frame. At 1920 it is `(1808, 49) – (1920, 224)`. That is
the whole change: not a cover, not a letter, not a density detent moved.

`08-diff-1280.png` is that difference, auto-levelled.

## The frames

| file | what it shows |
|---|---|
| `01-lane-open-before-1280.png` | the defect: the bar at the wall's edge, 108 px of window outboard of it |
| `01-lane-shut-before-1280.png` | the same with the returns lane collapsed — the strip is the window's, not the lane's |
| `02-lane-open-after-1280.png` | the bar on the window's edge, the rail inboard |
| `02-lane-shut-after-1280.png` | …collapsed |
| `03-edge-strip-1280.png` | the right-hand 160 px of before and after, side by side |
| `04-edge-strip-lane-shut-1280.png` | the same strip with the lane collapsed |
| `05` / `06` | the four 1920 × 1080 frames |
| `07-edge-strip-1920.png` | the 1920 strip, before beside after |
| `08-diff-1280.png` | every pixel that changed |
| `09-at-the-top-of-the-wall.png` | the starting state for the two driven checks |
| `10-the-rail-jumped-from-one-px-inboard.png` | a rail jump pressed at x = W − 5, one pixel inboard of the bar — the narrowest surviving part of the Fitts band |
| `11-the-bar-dragged-to-the-end.png` | the bar grabbed at x = W − 3 and dragged to the foot — the gesture it exists for, in the 4 px it was given |

The last two are the half a still frame cannot show, and both are proved by a
difference rather than by eye: `09 → 10` and `10 → 11` each change the whole
wall. A press at x = W − 5 reaches the rail; a press at x = W − 3 reaches the
bar. That is the 4 px boundary, driven.

## Why the edge, and what it cost

The returns lane had already answered this question in the owner's own words —
*"the scrollbar should be at the edge of it"* — and the answer there was: **the
rows carry the gutter so the bar can ride the surface's own edge. The content
keeps its inset; only the bar reaches the edge.** The wall's surface is the
window. The same move puts the bar on the window's edge and moves nothing else.

Two alternatives were weighed and both fail on evidence rather than on taste:

- **Narrow `INDEX_LANE_W` and close the gap instead.** There is nothing in the
  108 to cut. It is `INDEX_CLEARANCE` 8 + `INDEX_W` 60 + `HANG` 40; `INDEX_W`
  was raised from 36 to 60 precisely because 36 clipped `Unknown`, every
  recency bucket and most genre names (`crate::font`'s
  `the_index_rail_holds_the_labels_its_keys_produce` measures the whole set),
  and the 40 is law L1's one window gutter. The strip does not read as empty
  because the lane is too wide; it reads as empty because a rail is *sparse* —
  the ink in it is fourteen columns of single letters.
- **Overlay the bar on the rail's lane rather than reserving for it.** This
  moves the bar 4 px and fixes nothing, and `theme::wall_scrollbar` already
  records why overlaying was refused: the wall's block is centred and is not
  guaranteed to leave 4 px of slack at every width, so a cover's right edge
  would sometimes be under the bar.

**The cost is Fitts, and it is 4 px.** The rail's press band deliberately ran
to the window's edge, so flinging the pointer at the edge without aiming always
hit the rail. It now stops 4 px short, and those 4 px belong to the bar. What
the edge hits is still an affordance for scrolling the same wall — the one
whose entire reason for existing is the gesture the rail cannot do — and the
band that names its destinations is still 104 px wide. That is the trade. It is
taken because a scrollbar that is not on the container's outer edge is not
where anyone looks for one, which is the defect as reported.

## Running it

Both binaries must be built **inside the toolbox** — a host-built release
binary links a newer glibc than the container has and dies before it draws, and
a capture script waiting for a window simply hangs.

```sh
git worktree add --detach /tmp/baz-scrollbar-before <the base commit>
toolbox run -c baz-dev env CARGO_TARGET_DIR=$PWD/target/tb bash -lc \
  'cd /tmp/baz-scrollbar-before && cargo build --release -p baz --features device-output'
cp target/tb/release/baz /tmp/baz-bin-before
toolbox run -c baz-dev env CARGO_TARGET_DIR=$PWD/target/tb \
  cargo build --release -p baz --features device-output
cp target/tb/release/baz /tmp/baz-bin-after

toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-scrollbar-fix
toolbox run -c baz-dev env FIX=/tmp/baz-scrollbar-fix \
  docs/design/impl/wall-scrollbar/capture.sh

# Pillow is not in the toolbox; the ruler runs on the host.
./docs/design/impl/wall-scrollbar/measure.py docs/design/impl/wall-scrollbar
```

The run is isolated by all six XDG redirections from `docs/DEVELOPMENT.md` and
prints its own receipt:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Nothing is audible: `BAZ_DEVICE_TESTS` is unset, the scratch `HOME` routes
ALSA's default PCM to `null`, and every fixture sample is a zero.

## What the ruler keys on

A rail column and the bar's column both carry a lot of ink over the height of
the strip — twenty-seven letters add up. What separates them is the **longest
contiguous inked run**: the scroller is one unbroken block of a hundred-odd
pixels, and a letter is never more than a line. Counting inked rows was the
first attempt and cannot tell them apart at all.
