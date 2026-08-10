# The ladder only tightens

**2026-08-10.** The owner, looking at the running app:

> *"why is balanced smaller than compact... I think the dense should be a bit
> smaller"*

Two things in one sentence — **a defect and a preference** — and they are kept
apart here because one is arithmetic and one is taste.

The decision and its argument: **ADR-0028's second amendment**. This directory
is the measurement and the frames.

---

## 1 · The defect, verified

Each density step brings its own `hang`, and the wall's art is

```text
art = (w − (columns + 1) · hang) / columns
```

which **rises as the hang falls**. So wherever two steps land on the same
column count — which they must at any window narrow enough that the counts are
already consecutive integers — the *tighter* step drew the *larger* work,
because its gutters were smaller.

Replicating `Grid::new`'s arithmetic exactly (`floor(x + 0.5)`, the `ceiling`
clamp, the `art_max` clamp) and sweeping 700 … 2600 px in 20 px steps:

| grid width | Spacious | Balanced | Compact | Dense |
|---|---|---|---|---|
| 720 | **288.0** | **300.0** | 280.0 | 202.7 |
| 880 | 320.0 | **240.0** | **250.7** | 185.0 |
| 900 | 320.0 | **246.7** | **257.3** | 190.0 |
| 960 | 320.0 | **266.7** | **277.3** | 205.0 |

**30 of the 96 widths inverted.** Bold is a pair where the looser step drew the
smaller work.

### It is older than the fourth step

The same sweep against the **three-step** ladder that shipped before `Compact`
(`b935a4e` … `15e2aca`) inverts at **11 of the 96** — at 720, 740, 760, 780,
1060 … 1140, 1400 and 1420, all of them `Spacious` **<** `Balanced`. The 720 px
row above is one of them, and it is the row with no `Compact` in it.

So **`Compact` exposed the defect and did not cause it.** It has been true
since the day the wall gained a zoom; the fourth step nearly tripled the number
of widths at which a listener could see it, and put it at the shipped window.

### Why the tests did not catch it

`a_tighter_step_never_hangs_fewer_works` asserts **column count**, and the
column count was right the whole time — it is monotone by construction, because
a tighter step has both a smaller target and a smaller floor. Nothing asserted
the quantity a listener actually sees.

The file had even *noticed*. That test's doc comment read:

> *"The art is deliberately **not** asserted to be monotone with it, and that is
> not an omission: at 1120 px Spacious hangs 3 × 309.3 while Balanced hangs
> 3 × 320, because Balanced's art is capped there and Spacious's is not."*

That is the inversion, written down as a property and waved through.

---

## 2 · The fix, and what it cost

`Density::art_max` **stops being a tuned row and becomes derived**: it *is* the
next-looser step's `art_min`, and the loosest step's is `art::THUMB_PX`. The
four intervals abut and cannot overlap.

| step | `hang` | art | was |
|---|---|---|---|
| `Spacious` | 48 | 288 … 320 | 288 … 320 |
| `Balanced` | 40 | 240 … **288** | 240 … 320 |
| `Compact` | 32 | **200** … **240** | 208 … 280 |
| `Dense` | 28 | **160** … **200** | 176 … 240 |

`Grid::new` clamps art to at most the step's cap and the column ceiling holds it
to at least the step's floor, so a tighter step's largest work **is** a looser
step's smallest: they meet, and cannot cross. One further rule closes the
degenerate tail — `Grid::art_cap` also caps art at `w − 2 × WIDEST_HANG`, one
column at the ladder's loosest hang, which binds only below ~416 px of grid.

**Swept at quarter-pixel resolution from 0 to 4000 px and at whole pixels to
20 000: no inversion at any width.**

### The cost, stated

- **The default wall moves.** `Balanced` caps at 288 rather than 320, so **744
  of the band's 2261 widths draw smaller art** — the tops of each column band.
  About 132 of those were not inversions; they are the price of the ranges
  being *disjoint* rather than merely ordered. Every width that moves moves
  **down**, and none below `Balanced`'s own 240 px floor.
- **The rungs are shorter where they used to be backwards.** At 888 px of grid
  `Balanced` draws 242.7 and `Compact` 240 — a 3 px rung. A short rung is a
  wall that barely changes; a backwards one is a wall that changes the wrong
  way.
- **More widths sit in the gutters-take-the-slack regime**: capped widths
  roughly double for `Balanced` (360 → 748) and `Compact` (288 → 564).

Two alternatives were priced and refused — a shared hang (which costs the
zooming shelf header, since `Grid::header_h` *is* the step's hang) and
resolving the four steps together (smaller footprint, but it leaves the
overlapping ranges in the table and makes `Grid::new(w, Compact)` a function of
the two steps above it). ADR-0028's second amendment §2 has the argument.

---

## 3 · The preference, and where the floor is

`Dense` was 176 … 240 — *today's shelf*, the 208 px cell baz drew before density
existed. It is now **160 … 200**, target 184. `Compact` is **re-derived, not
re-tuned**: still exactly the `Balanced`-to-`Dense` rung halved.

**The floor is 160 and it is principled.** `ART_FLOOR` 1.0 is not a candidate —
it is the backstop that keeps the geometry total. Two of the product's own
numbers are:

- **`art::THUMB_PX` halved.** The cache decodes to 320 px per edge. Below half
  that, the wall discards three quarters of the pixels it paid to decode.
- **`theme::CONTINUE_SLEEVE` 132**, the smallest sleeve in the product that
  carries a record's identity — its own token says *"large enough that the
  record is identified by its cover rather than by its name"*.
  `theme::PANEL_SLEEVE` 40 is below it and is an identifier beside a name.

160 satisfies both, sits on the 4 px lattice, and clears the second by exactly
one `Dense` hang. It is a floor the wall really **reaches** — the tight end
arrives at 160 inside the band, so the claim is about something.

---

## 4 · The frames

Two builds — `BIN0` is the commit the branch started from, `BIN` is the branch
— photographed at the same three windows, moved to the same screen coordinates,
from the same fixture, with the same gestures. The step is set by **pressing
that step's own detent mark with the pointer**, never by a config key, so no
frame can be of a wall the control never reached.

The grid is **the window less 392** (`SIDEBAR_W` 280 for the open lane,
`INDEX_LANE_W` 108, and the scrollbar's lane). That number was **measured off
the frames**, not assumed — the first pass at `capture.sh` named three widths
that were 8 px out, and the measurement is what caught it.

### The three windows, and why each one

| window | grid | the pair it shows |
|---|---|---|
| 1120 × 860 | 728 | `Spacious` 292 **<** `Balanced` 304 — the inversion **that predates `Compact`** |
| 1280 × 860 | 888 | `Balanced` 242.7 **<** `Compact` 253.3 — **the owner's sentence** |
| 1600 × 1000 | 1208 | `Balanced` 252 **<** `Compact` 262, and where `Dense` moves most |

### Read these first

- `04-before-beside-after-1600x1000.png` — the whole change in one frame.
  Down the left column: 320, 252, **262**, 208 — the third rung is *bigger*
  than the second. Down the right: 320, 252, 203, 169.
- `04-before-beside-after-1120x860.png` — the same break, one rung higher and
  with no `Compact` involved: `Spacious` 292 above `Balanced` 304.
- `04-before-beside-after-1280x860.png` — the shipped window.

### Everything, measured

Every number below is **read off the frame**, by a pixel scan across the first
row of covers, and every one is `art − 4`: a sleeve is drawn inside a
`theme::SLEEVE_MAT` 2 mat, so the scan sees the picture and not the tile. The
bias is a constant at every step and every width, which is itself the check —
a frame that is off by anything other than 4 is a frame of a wall the script
did not mean to take.

| window | step | before (frame) | before (arithmetic) | after (frame) | after (arithmetic) |
|---|---|---|---|---|---|
| 1120 × 860 | Spacious | 288 | 2 × 292.0 | 288 | 2 × 292.0 |
| | Balanced | **300** | 2 × **304.0** | 284 | 2 × 288.0 |
| | Compact | 276 | 2 × 280.0 | 196 | 3 × 200.0 |
| | Dense | 201 | 3 × 205.3 | 196 | 3 × 200.0 |
| 1280 × 860 | Spacious | 316 | 2 × 320.0 | 316 | 2 × 320.0 |
| | Balanced | 239 | 3 × 242.7 | 239 | 3 × 242.7 |
| | Compact | **249** | 3 × **253.3** | 236 | 3 × 240.0 |
| | Dense | 183 | 4 × 187.0 | 183 | 4 × 187.0 |
| 1600 × 1000 | Spacious | 316 | 3 × 320.0 | 316 | 3 × 320.0 |
| | Balanced | 248 | 4 × 252.0 | 248 | 4 × 252.0 |
| | Compact | **258** | 4 × **262.0** | 199 | 5 × 203.2 |
| | Dense | 204 | 5 × 208.0 | 165 | 6 × 168.7 |

Bold is the inverted step. **Every frame matches the arithmetic to the mat**,
before and after, which is what makes the frames evidence rather than
decoration.

### The file names

- `01-wall-<step>-<build>-<W>x<H>.png` — the whole window, twenty-four of them.
- `02-band-<step>-<build>-<W>x<H>.png` — the first cover row, cropped at the
  same y in both builds and captioned with its measured work width.
- `03-ladder-<build>-<W>x<H>.png` — the four steps down one column, loosest
  first.
- `04-before-beside-after-<W>x<H>.png` — the two ladders side by side.

### Reproducing

The wall is grouped by **genre**, so the first shelf is `AMBIENT` — eighteen
records, which fills every row at every step. The default artist grouping puts
a two-record shelf first, and two covers cannot show a column count.

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=/tmp/tb-after \
  cargo build --release -p baz --features device-output
git worktree add /tmp/before 3b9d32b && cd /tmp/before
toolbox run -c baz-dev env CARGO_TARGET_DIR=/tmp/tb-before \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/impl/density-on-every-page/mkfixture.sh /tmp/baz-ladder-fix
toolbox run -c baz-dev env BIN0=/tmp/tb-before/release/baz BIN=/tmp/tb-after/release/baz \
  FIX=/tmp/baz-ladder-fix docs/design/impl/the-ladder-only-tightens/capture.sh
```

Headless, on a private Xvfb, with all six XDG redirections
(`docs/DEVELOPMENT.md`). Nothing is audible: the scratch `HOME` routes ALSA's
default PCM to null and every fixture sample is a zero. The receipt that the
isolation held, printed by the run:

```text
[mpris] no session bus; desktop media controls unavailable
```

---

## 5 · What is pinned, so it cannot come back

- `the_ladder_only_tightens_the_work_it_draws` — sweeps every whole pixel of
  the band **and** every quarter pixel below 420, and asserts on `art`. A
  single width proves nothing here: 880 inverted and 920 did not.
- `the_steps_partition_the_art_range` — the construction rather than the
  consequence, so a later hand re-tuning a row fails on the rule.
- `the_wall_hangs_no_work_below_the_size_a_cover_identifies_a_record_at` — both
  readings of the floor, and that the wall reaches it.
- `balanced_is_the_hang_the_tokens_publish` — now asserts the **cost**: the
  744 widths, that they only ever move down, and that none leaves `Balanced`'s
  own range.
