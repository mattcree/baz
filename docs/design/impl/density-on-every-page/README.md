# Density on every page — the fourth step, and the control where the works are

**The owner, 2026-08-10, in two messages:**

> *"we should ensure the density options are available on all pages..."*
>
> *"4 levels makes sense to me"*

Two asks, one change. `Compact` is the fourth step; the detent marks now stand
on **every place that hangs works** and on none that does not; and the three
places that hang works read **one grid**, which is what fixes a defect the ask
walked straight into.

Frames taken with [`capture.sh`](capture.sh) against the real binary, headless
on a private Xvfb, with all six XDG redirections from
`docs/DEVELOPMENT.md` §"Headless UI verification". The run's
`[mpris] no session bus` line is the isolation receipt and the script prints
it. The fixture is [`mkfixture.sh`](mkfixture.sh) — the composition fixture
plus one prolific artist, because a page with two records draws two tiles at
every step and would have said nothing.

**Two builds.** `before` is `ed282c8`, the commit this branch started from;
`after` is the branch. Frame `08` is taken from both.

---

## 1 · The fourth step goes *inside* the ladder, and that was measured

`Density::ALL` is walked at the width the wall really gets —
`window − sidebar − INDEX_LANE_W − WALL_SCROLLBAR_W` (`App::grid_width`), for
seven windows in both lane states. Columns, and the art edge each hangs:

| window | lane | grid | Spacious | Balanced | **Compact** | Dense |
|---|---|---|---|---|---|---|
| 1000 | open | 608 | 1 × 320 | 2 × 244 | 2 × 250 | 2 × 240 |
| 1000 | collapsed | 792 | 2 × 320 | 2 × 320 | 3 × 216 | 3 × 227 |
| 1280 | open | 888 | 2 × 320 | 3 × 243 | 3 × 253 | 4 × 187 |
| 1280 | collapsed | 1072 | 3 × 293 | 3 × 304 | **4 × 228** | 5 × 181 |
| 1440 | open | 1048 | 2 × 320 | 3 × 296 | **4 × 222** | 5 × 176 |
| 1440 | collapsed | 1232 | 3 × 320 | 4 × 258 | 5 × 208 | 5 × 213 |
| 1600 | open | 1208 | 3 × 320 | 4 × 252 | 4 × 262 | 5 × 208 |
| 1600 | collapsed | 1392 | 4 × 288 | 4 × 298 | **5 × 240** | 6 × 199 |
| 1920 | open | 1528 | 4 × 320 | 5 × 258 | **6 × 217** | 7 × 186 |
| 1920 | collapsed | 1712 | 4 × 320 | 5 × 294 | **7 × 208** | 8 × 182 |
| 2560 | open | 2168 | 6 × 305 | 7 × 264 | **8 × 235** | 10 × 186 |
| 2560 | collapsed | 2352 | 6 × 320 | 8 × 249 | **9 × 226** | 10 × 204 |
| 3840 | open | 3448 | 10 × 292 | 11 × 270 | **13 × 231** | 15 × 200 |
| 3840 | collapsed | 3632 | 10 × 310 | 12 × 259 | **14 × 225** | 16 × 197 |

Bold is a rung that stands strictly between its neighbours: **nine of
fourteen**. What the sweep found:

- **The widest gap in the ladder is `Balanced` → `Dense`.** It jumps two, three
  or four columns at every window from 1280 up (5 → 8 at the owner's own 1920
  with the lane collapsed). `Spacious` → `Balanced` jumps nought or one, and at
  1280 collapsed it jumps *nothing* — the two steps hang three columns each and
  differ only in 11 px of art. So the interior is where the ladder is uneven,
  and it is uneven on exactly the side a listener crosses, because `Balanced`
  is the default and `Dense` is its neighbour.
- **A step looser than `Spacious` is refused by the system, not by taste.**
  `Spacious.art_max()` is already `art::THUMB_PX` 320 — the edge the thumbnail
  cache holds — and the table shows it standing on that cap at seven of the
  fourteen widths, spending the slack on margins. A looser step could not draw
  a larger work. It could only add air, which is not a density step.
- **A step tighter than `Dense` was the other candidate and loses on the same
  measurement**: it would put an eleventh column on a 1920 wall and leave the
  `Balanced` → `Dense` chasm exactly where it is.

So `Compact`, and its numbers are that rung **halved** rather than tuned:
208 = (240 + 176)/2, 236 = (272 + 200)/2, 280 = (320 + 240)/2, and the hang's
own midpoint 34 taken down to **32**, the nearest value on the 4 px lattice
`theme.rs` holds every measure to.
`the_ladder_only_tightens_and_the_fourth_step_halves_its_widest_rung` asserts
each of those, so the row cannot be quietly re-tuned later.

**What the sweep also says against itself, and the frames confirm:** at narrow
windows the ladder has no room for a fourth rung. At 1172–1232 px of grid the
three original steps already hang consecutive integers, so `Compact` must
repeat a neighbour's column count there and differ only in art. That is a fact
of the arithmetic, not a flaw in the numbers, and it is why
`a_tighter_step_never_hangs_fewer_works` asserts *never fewer* rather than
*strictly more*. From about 1400 px of grid up, every rung is distinct.

**Frames `01` (the wall), `02` (Home), `03` (an artist's page)** — four steps,
three pages, two windows. **Frame `07`** crops the rail's foot so the four
marks and which one is lit can be read without hunting.

---

## 2 · What density means on a page with no tiles: **nothing, and the control
is absent there**

The wall, Home and an artist's page hang tiles, so density is a **column
count**. A record's page, a playlist's page, `Now playing` and `Settings` are
**rows**, and a column of rows has no column count. The decision is that
density does not apply there and the marks are not drawn there.

The argument is a measurement rather than a preference. **A track row's height
is `theme::TRANSPORT_HIT` 32** — that number is the pointer-target floor, the
mitigation ADR-0017 §4 owes a toolkit that publishes no accessibility tree, and
not a spacing choice. So:

- a step **tighter** than the default could not shrink a row without breaking
  the very floor that ADR-0028's visible-control argument exists to serve;
- a step **looser** could only pad text, which changes no fact on screen and is
  not what anyone asks a density control for.

The alternative — marks on the rows pages that scale their pitch — was
therefore refused, and the alternative *to that* — marks in the returns lane,
where they would be resident on all seven places — was refused for the reason
the owner's own ask gives: on four of those places they would be **present and
inert**, which is worse than absent.

**Frame `04`** is a record's page at both windows: rows, and no marks anywhere.
It is the *decided* absence, and a frame is how a reader checks the absence is
clean rather than a control that failed to draw. Since `views/page.rs` is now
the record's page and the playlist's page in one composition, that one frame is
evidence for both.

---

## 3 · Where the control lives, once it is not the rail's foot alone

**The marks stand at the trailing edge of the block of works they hang.**

- On the **Library**, that block is the whole place, and its trailing edge is
  the index rail's lane — so the marks close the lane, exactly where ADR-0028
  put them, at exactly the geometry ADR-0028 declared. Nothing about the wall
  moved (frames `01`, `07`).
- On **Home** and an **artist's page**, the block is a named section, so the
  marks stand on that section's rule — `RECENTLY ADDED` and `RECORDS`
  (`views::section_rule_hung`).

**Not the top bar.** The owner's standing complaint — *"just adding stuff into
that top bar isn't good"* — did not have to be argued past, because the ledger
already forbids it: doc 07 L8.1 makes density's subject **the viewport**, so
its home is the place's body or nowhere, and the strip is the frame.

**Not the returns lane** — for the same rule, and for §2's reason.

**The keyboard already worked, and that was half the defect.**
<kbd>Ctrl</kbd>+<kbd>=</kbd> / <kbd>Ctrl</kbd>+<kbd>-</kbd> are ungated by
place: `App::update_modified_input` steps `self.density` from anywhere. But
Home and the artist page named `Density::Balanced` in their own source, so off
the wall the keys changed the state and **nothing on screen moved**. The keys
are not touched by this branch; making the pages read the density is what
makes them work.

**Frames `05` and `06`** are the marks pressed *on Home* and *on the artist's
page* — before and after the press, at both windows — because a control only
ever driven from the Library is not evidence that it is live where it is drawn.

### The route these frames took, and why it matters

Density is **one piece of state for the whole product**. So frames `01`–`03`
set the step by **pressing a detent mark with the pointer on the wall**, and
then Home and the artist page are *walked to* and photographed without the
control being touched again. Set it once; every place that hangs works follows.
Before this branch, only the wall did.

Every gesture in `capture.sh` is one a listener makes: a press on a mark, a
press on a lane destination, a typed query, a press on a record's tile
**caption** (the sleeve carries four hover-revealed options that a press in the
middle would hit instead), a press on the record page's `Artist ›` breadcrumb,
a press on the lane's own `Collapse`. Two earlier runs of this script produced
frames that *looked* like passes and were not — a shelf header at a y that was
right for one build and wrong for the other photographed a filtered wall and
labelled it an artist's page — which is exactly the failure mode the route
discipline exists to catch. Both are fixed, and the reason is written at the
call sites.

---

## 4 · The defect the ask walked into: **frame `08`**

`views/artist.rs` resolved a grid of its own —
`Grid::new(width − 2 × HANG, Density::Balanced)` — and it was wrong three ways.
It named a step outright, so the page ignored the control entirely. Its width
was a hand-written guess at `place_pad`'s horizontals that **missed the
scrollbar lane**, so the block was resolved for 10 px the page does not have.
And it bore no relation to what the wall reserves.

Measured at the owner's own window — **1920, the returns lane collapsed,
`balanced`, the same sixteen records on both pages, one press apart**:

| | columns | art |
|---|---|---|
| the wall | 5 | 294.4 px |
| the artist page, before | **6** | **244.0 px** |
| the artist page, after | 5 | 294.4 px |

`08c-artist-over-wall-before-1920x1080.png` is the artist page over the wall,
and the two halves plainly disagree: six covers against five, 50 px of edge
between the same record drawn twice.
`08c-artist-over-wall-after-1920x1080.png` is the same pair after, and the two
halves are the same wall to the pixel.

The two widths straddled a boundary that 22 px of arithmetic decided —
`(1712 − 40)/280 = 5.97` against `(1744 − 40)/280 = 6.09` — which is how
fragile a second answer to *how wide is the grid* is. So there is one answer
now: the shell resolves `Shelf::grid` and hands it to every place that hangs
works. `every_place_that_hangs_works_hangs_them_on_one_grid` reads the sources
and fails if `home.rs`, `artist.rs`, `shelf.rs` or `page.rs` grows a
`Grid::new` again, or if `app.rs` stops handing `state.grid()` down.

It costs the artist page and Home **22 px**: the wall's width reserves the
rail's lane and the wall's 4 px bar (112 px) where those pages' own gutters
take 90. It is spent at the trailing edge, where nothing hangs from, and the
alternative is a cover that changes size when you walk to its artist.

Home was wrong the same way and is fixed the same way. It was the asymmetry in
its clearest form: `RECENTLY ADDED` showed *one row of the wall's own tiles at
the density's column count* — except that it named `Balanced`, so the row was
one width forever and the control that would have changed it was on another
page.

---

## What needs the owner's eye

**The `Dense` mark is now sixteen squares, and it is at the limit of 16 px.**
The detent glyphs are the wall at their own hang — 1, 4, 9 works — and there is
no whole number of columns between two and three, so the fourth step re-keyed
the family rather than joining it: `Compact` wears the 3 × 3 field `Dense` used
to, and `Dense` wears a new 4 × 4 whose cells minify to 2.25 px on a 1×
display. It reads as *many small works*, which is all this detent has to mean,
and the tooltip carries the name. Frame `07` at both windows is the thing to
look at. If it reads as mush rather than as a grid, the honest alternatives are
a larger sprite for this one mark or a different fourth glyph, and both are
small changes.

## Files

| | |
|---|---|
| `capture.sh` | every frame here, from the two binaries |
| `mkfixture.sh` | the fixture: 39 records, 16 of them one artist's |
| `01-wall-<step>-<W>x<H>` | the wall at each step, both windows |
| `02-home-<step>-<W>x<H>` | Home at each step, walked to |
| `03-artist-<step>-<W>x<H>` | an artist's page at each step, walked to |
| `04-record-page-<W>x<H>` | a page of rows: no marks, decided |
| `05-home-{before,after}-press` | the marks pressed on Home |
| `06-artist-{before,after}-press` | the marks pressed on an artist's page |
| `07-rail-foot-<step>-<W>x<H>` | the four marks, cropped, one lit |
| `08*-{before,after}` | the artist page against the wall, 1920, lane collapsed |
