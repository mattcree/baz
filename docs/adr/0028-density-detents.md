# ADR-0028: Three visible detents for the wall's density

**Status**: accepted (2026-08-09) · ships doc 11 §5 P8, the owner choosing
option **(a)** · **overturns one clause of a standing rule under the
ledger's editing rule** — "no grid-size picker" as applied to three quiet
detent marks in the place's own body; the entry otherwise stands and is
narrowed, not deleted · amends doc 07 L8.1's density row · adds no state, no
message, no token and no dependency.

## Context

The product's own laws contradicted each other, and the contradiction was
being resolved silently in favour of invisibility.

- **The visible-control rule** (the product's standing rule, Accessibility): *"Every
  action in baz has a visible, pointer-reachable control. No action is
  keyboard-only, and no control's only affordance is hover."* Doc 09 §5.2
  applied it to gestures in so many words when it admitted the context menu:
  *"a right-click is a gesture, and no action may be gesture-only."* This
  entry is not taste — it is **the mitigation for a toolkit that publishes
  no accessibility tree** (ADR-0017 §4), which is why it has survived every
  design that wanted a control gone.
- **The view-options refusal** (the product's standing rule, The interface): *"No
  view-options menus. No grid-size picker, no list-mode toggle, no column
  chooser, no sort dropdown… density is a zoom gesture."*

Density's only routes were <kbd>Ctrl</kbd>+<kbd>-</kbd> /
<kbd>Ctrl</kbd>+<kbd>=</kbd> and <kbd>Ctrl</kbd>+scroll — a keyboard chord
and a modified gesture, no visible control, no readout, no Settings row. An
action whose every route is a gesture is exactly what the first entry
forbids; the second entry is what kept any visible route from existing. The
Jobs-era critique named the deadlock (doc 11 §4's scorecard row: *"breach of
their own rule"*) and presented the resolution both ways (§5 P8): give the
zoom a visible handle, or amend the visible-control rule to exempt
view-position acts. **The owner chose (a), the visible handle
(2026-08-09).**

## The entry's argument, engaged rather than snuck past

The editing rule: removing a refusal needs an ADR that beats its argument.
The clause's real argument, steelmanned from `02` §2.7 and ADR-0017 §1.3:

1. **A free zoom destroys reproducibility.** A slider makes every screenshot
   different, every layout report unreproducible, and every reserved-slot
   argument conditional.
2. **Settings must never be the answer to a view question** (ADR-0017 §1.3).
3. **View-options menus are the junk drawer** — a chooser row grows tenants,
   and a surface that enumerates view state invites more view state.

All three arguments **survive this decision untouched**:

1. The control is the three named steps and nothing between them — detents,
   not a slider. Every screenshot is still one of three walls per width.
   `Density` gains no variant, no fourth number, no interpolation.
2. There is still no Settings row, no Appearance section, no preference.
   The step persists exactly as it did — as state in `config.toml`, the way
   the group key does.
3. There is still no menu, no dropdown, no chooser, no readout row. Three
   marks of glyph ink stand in the place's body the way the group keys stand
   in the strip: words-or-marks on the surface itself, which is the shape
   the ledger's own gloss endorses (*"group keys are a row of words; the
   lens switcher is two words"*).

What does **not** survive is the clause's application to *any* visible
density control. It is beaten on two grounds the ledger itself supplies:

- **Its own corpus outranks it.** The accessibility entry protects listeners
  a toolkit already fails; the view-options entry protects the frame from
  chrome. Where the two collide — and for density they collide exactly —
  the ledger cannot coherently prefer the aesthetic entry: the
  visible-control rule is the stated reason the transport buttons, the
  search field and the labelled Queue door exist at all. A ledger that
  waives its accessibility mitigation to keep a wall quiet has inverted its
  own order of precedence.
- **The evidence base said CONTRADICTS from the start.** `03` R7 (Steam,
  Google Photos: durable damage for removing a density level; iPhoto's and
  iTunes grid view's size slider both Jobs-era) is what forced §2.7 into
  existence. The refusal never argued against a *visible* three-step
  control; it argued against menus and sliders, and the gesture-only
  outcome was inherited from ADR-0017 §1.3's (correct) rejection of the
  Settings placement, not decided on its own merits.

## Decision

### 1. The home: the foot of the index rail's lane

Doc 07 L8.1: density reads **the viewport, and nothing else** — subject:
view — so its home is the place's body, or nowhere. P8 names the two
candidate homes in the body; the rail's lane wins over the wall's empty
leading band on every axis that is not taste:

- **Subject.** The lane is the body's one resident view-subject surface —
  the rail reads *the arrangement and the viewport* (L8.1's own row). The
  two view controls share one strip; no new surface exists (L8.2(3): a
  cluster that already exists).
- **The leading band fails three ways.** It scrolls away with the wall — a
  control that leaves is a control back to undiscoverable; the pinned
  header claims the same band the moment the wall moves, so the lane would
  hold two tenants in alternation; and the band's height *is* the step's
  hang (28/40/48), so the control would resize itself as its own effect.
- **The wall's algebra is untouched.** `INDEX_LANE_W` 108 is constant at
  every step and every window; the grid is still resolved for
  `width − INDEX_LANE_W` and no width test changes by a character. The
  marks stand *below* the spine's strip, so the fisheye — a pure function
  of the pointer inside the spine's own bounds — never sees them, and the
  spine's per-frame elision absorbs the shorter lane exactly as it absorbs
  a short window (that arithmetic was already a function of the widget's
  real bounds; ADR-0020's amendment).

The marks are right-aligned so their glyph boxes stand on `W − HANG` — the
lane's one declared ink edge (law L1/L5), the same line the letters hang
from. The foot keeps one `HANG` of air above the bar. No new alignment edge,
no new height: each mark is a [`STEPPER_HIT`] box, L7's named secondary.

### 2. The form: three marks, spacious / balanced / dense

Three sprite glyphs in the existing sheet — one square, four squares, nine
squares: **the wall itself at its three densities**, which is as close to
self-depicting as a density mark can be (the convention every grid-size
control from Finder to Lightroom draws some variant of). Words were
considered and refused by geometry rather than principle: the lane's ink cap
is 68 px and the group-key anatomy at the meta size does not fit it without
clipping, and a control label that clips is worse than a glyph with a name.

- **Active mark**: full glyph ink (`GLYPH_OPACITY_HOVER` 1.0) against the
  resting `GLYPH_OPACITY` 0.57 of the other two — the group-key row's
  active treatment translated to sprite ink (paper against paper-faint),
  and **never the accent**: density is not playback truth. The wall itself
  is the primary readout — the covers' own size states the step — so the
  mark's lift is confirmation, not the sole carrier of the state.
- **The active mark is inert** — a container, not a button. Pressing the
  step you are on would do nothing, and a control that does nothing when
  pressed is the lie the rail's absent letters already refuse. This is
  L8.3's split in miniature: the active mark is the *fact*; the other two
  are the *controls*.
- **Hover** is the wash chip (`theme::transport`'s own background), the
  lane's established hover vocabulary — the spine's winner chip, the group
  keys' wash. No new tween: the 90 ms ink fade is the transport cluster's
  vocabulary, and `add_slot` set the precedent for a static-ink sprite
  button outside it.
- **Tooltips** per the icon-only law (doc 10 §3.1's accessibility clause,
  pinned by `theme::every_icon_only_control_carries_a_tooltip`): the
  step's name — `Spacious`, `Balanced`, `Dense` — is the accessible name.

### 3. The message: the gesture's own, exactly

A mark sends **`Message::DensityStep(current.steps_to(target))`** — the
same message, the same saturating `Density::step` walk, computed as the
signed number of gesture notches between here and the pressed mark.
`steps_to` is pure and pinned by
`a_marks_delta_is_the_gestures_own_notches`: applying `step(±1)` |delta|
times lands where one `DensityStep(delta)` lands, for every pair of steps.
The keys and <kbd>Ctrl</kbd>+scroll remain, **as accelerators of a visible
control** — the mirror rule's ordinary state (L8.7: the keyboard is the
same decision, made twice), where before they were the whole control. No
new message, no `DensitySet`, no second grammar.

### 4. The ledger, amended

the product's interface entry is narrowed under the editing rule and
carries the amendment note. What stands: no view-options menus, no
list-mode toggle, no column chooser, no sort dropdown, no free zoom, no
Settings row for a view question. What falls: "no grid-size picker" *as
applied to* three quiet detents in the place's own body. The entry's gloss
now reads "density is three detent marks on the rail's lane, and the zoom
gesture accelerates them".

## Deliberately not done

- **No Settings → Appearance row** — ADR-0017 §1.3 stands whole.
- **No readout beside the marks** — the wall is the readout; the active
  mark confirms.
- **No fourth step, no slider** — the three-step design is what makes the
  reserved-slot arguments unconditional, and this ADR leans on that.
  *(**Half overturned 2026-08-10** — see the amendment. There is a fourth
  step; there is still no slider. The clause's real argument was about
  reproducibility, and four named detents keep it whole.)*
- **No whole-lane-wide hit targets for the marks.** The spine's
  nearest-slot press owns the lane's band; giving the marks the full 108 px
  would put two press grammars on one x-range with nothing visible dividing
  them. The marks take the named 24 px square.

## Consequences

- `crates/baz/src/shelf.rs`: `Density::steps_to`, tested.
- `crates/baz/src/icon.rs`: three glyphs (one/four/nine squares), swept by
  the existing sheet tests plus their own run-count assertions.
- `crates/baz/src/views/shelf.rs`: the lane becomes spine over marks;
  placement and mirror tests
  (`the_density_marks_mirror_the_gestures_exact_messages`,
  `the_density_marks_stand_in_the_lanes_own_geometry`).
- `crates/baz/src/app.rs`: the keyboard-mirror table's `DensityStep` row now
  names the marks instead of declaring the gesture its own control.
- Captures under `docs/design/impl/density-control/` — all three detents,
  the active mark moving, taken by the real press on the real binary.


---

## Amendment (2026-08-10) — a fourth step, and the marks stand where the works do

**Status**: accepted · the owner, in two messages: *"we should ensure the
density options are available on all pages..."* and *"4 levels makes sense to
me"* · **overturns one bullet of this entry's own "Deliberately not done"**
(the fourth step) and **generalises §1** (the home) · adds no message, no
token, no dependency and no state.

Evidence, measurements and every frame quoted below:
`docs/design/impl/density-on-every-page/`.

### 1 · The fourth step is `Compact`, and it goes *inside* the ladder

This entry refused a fourth step on one argument — *the three-step design is
what makes the reserved-slot arguments unconditional* — and that argument was
about **reproducibility**, which is the same argument that refused the slider:
every screenshot of baz must be one of a small, named set of walls per width.
Four named detents keep that whole. What it was never an argument for is the
number three.

**Where it goes was measured, not chosen.** `Density::ALL` was swept at the
width the wall really gets (`App::grid_width`), for seven windows in both lane
states — fourteen walls, the full table in the impl README:

- **`Balanced` → `Dense` is the ladder's widest rung**, jumping two to four
  columns at every window from 1280 up — 5 → 8 at 1920 with the lane collapsed.
  `Spacious` → `Balanced` jumps nought or one, and at 1280 collapsed it jumps
  *nothing at all* (three columns each, 11 px of art between them).
- **Looser than `Spacious` is refused by the system.** `Spacious.art_max()` is
  already `art::THUMB_PX` 320, and the sweep shows it standing on that cap at
  half the widths, spending the slack on margins. A looser step cannot draw a
  larger work; it can only add air, and air is not a density step. This is the
  same invariant `the_wall_never_draws_art_larger_than_its_source` has always
  asserted — it now also closes one end of the ladder as a matter of design.
- **Tighter than `Dense`** would add an eleventh column to a 1920 wall and
  leave the widest rung exactly where it is — and that rung is the one a
  listener actually crosses, `Balanced` being the default and `Dense` its
  neighbour.

So the fourth step is the widest rung **halved**, and every number is arithmetic
rather than taste: `art_min` 208 = (240 + 176)/2, `art_target` 236 =
(272 + 200)/2, `art_max` 280 = (320 + 240)/2, and the hang's own midpoint 34
taken down to **32**, the nearest value on the 4 px lattice `theme.rs` holds
every measure to.
`the_ladder_only_tightens_and_the_fourth_step_halves_its_widest_rung` asserts
each of those and the ladder's monotonicity, so a later hand cannot re-tune the
row quietly or insert a step at the wrong index.

**Recorded against itself:** at narrow windows the ladder has no room for a
fourth rung. Below about 1400 px of grid the three original steps already hang
consecutive integers, so `Compact` repeats a neighbour's column count there and
differs only in art. That is a property of the arithmetic and it is why
`a_tighter_step_never_hangs_fewer_works` asserts *never fewer* rather than
*strictly more*.

**The three shipped words keep their exact spellings.** `spacious`,
`balanced`, `dense` are untouched and `Balanced` is still the default, so no
config document in the world re-hangs a wall because of this — `code`'s own
rule, and the lesson ADR-0035 learned the hard way when `"artist"` was
repurposed. The new word is `compact`, and it is asserted by name.

**The marks re-key rather than grow.** The detent glyphs are the wall at their
own hang — one work, four, nine — and there is no whole number of columns
between two and three. So `Compact` wears the 3 × 3 field `Dense` used to, and
`Dense` gains a 4 × 4. The set stays *self-depicting*, which is the one thing
it may not stop being; the cost is that `Dense`'s cells minify to 2.25 px on a
1× display, which is at the limit of 16 px and is flagged for the owner's eye
in the impl README.

### 2 · Density does not apply to a page of rows, and the marks are absent there

The ask's real question. The wall, Home and an artist's page hang tiles, so
density is a **column count**. A record's page, a playlist's page, `Now
playing` and `Settings` are **rows**, and a column of rows has no column count.

Both answers were available and this one is taken on a measurement: **a track
row's height is `theme::TRANSPORT_HIT` 32**, and that number is the
pointer-target floor — the mitigation ADR-0017 §4 owes a toolkit with no
accessibility tree — rather than a spacing choice. A tighter step could not
shrink a row without breaking the floor that *this entry's own argument* exists
to serve; a looser step could only pad text, which changes no fact on screen.
Density's declared subject is the viewport (doc 07 L8.1); its declared unit is
the column.

So the marks are drawn on three places and absent from four. **Absent, not
disabled**: a control that is present and inert is the lie this entry's §2
already refuses in the active mark, and the owner's ask rules it out in so many
words. `docs/design/impl/density-on-every-page/04-record-page-*.png` is the
absence, photographed, so a reader can check that it is clean rather than a
control that failed to draw.

### 3 · §1's home, generalised: the trailing edge of the block of works

§1 chose *the foot of the index rail's lane* and argued it on three grounds —
the lane is the body's resident view-subject strip, the wall's leading band
fails three ways, and the wall's algebra is untouched. All three still hold and
**the Library's control has not moved by a pixel**. What §1 could not say,
because the wall was then the only place that hung works, is what the rule
*behind* that choice was.

It is this: **the marks stand at the trailing edge of the block of works they
hang.** On the Library the block is the whole place, so its trailing edge is
the rail's lane. On Home and an artist's page the block is a *named section*,
so its trailing edge is that section's rule — `RECENTLY ADDED` and `RECORDS`,
via `views::section_rule_hung`, in the anatomy `section_rule_noted` already
established for a fact at a rule's right edge.

Two placements were considered and refused:

- **The top bar.** Refused without needing the owner's standing complaint
  (*"just adding stuff into that top bar isn't good"*), because doc 07 L8.1
  already settles it: density's subject is the viewport, so its home is the
  place's body or nowhere, and the strip is the frame.
- **The returns lane.** Resident on all seven places, which is superficially
  what *"available on all pages"* asks for — and refused for §2's reason: on
  the four places that hang no works it would be present and inert.

`density_marks` is **one function in two axes** (`DetentAxis::Column` down the
lane, `DetentAxis::Row` along a rule), so the three places cannot drift into
three controls that look alike. The press, the inert active mark, the tooltip
name, the hover wash and the `steps_to` delta are all §2 and §3 of this entry,
unchanged.

### 4 · One grid for every place that hangs works

Not asked for, and the thing the ask walked into. `views/home.rs` and
`views/artist.rs` each resolved a grid of their own —
`Grid::new(width − 2 × HANG, Density::Balanced)` — which named a step outright
(so neither page answered the control *or* the keys, which were never gated by
place) and guessed at `place_pad`'s horizontals, missing the scrollbar lane.

Measured at 1920 with the lane collapsed, same records, one press apart: **the
artist page drew six columns of 244 px art where the wall drew five of 294.**
The two widths straddled a boundary that 22 px of arithmetic decided.

So the shell resolves `Shelf::grid` once and hands it down, and a view file may
not resolve a grid at all —
`every_place_that_hangs_works_hangs_them_on_one_grid` reads the sources and
fails if `home.rs`, `artist.rs`, `views/shelf.rs` or `views/page.rs` grows a
`Grid::new`, or if `app.rs` stops passing `state.grid()`. A record is now the
same size wherever it is drawn **by construction**, which is what
`views/artist.rs`'s own docs claimed before this and could not deliver.

It costs Home and an artist's page 22 px of block width, at the trailing edge,
where nothing hangs from.

### Deliberately not done, still

- **No slider, no readout row, no Settings → Appearance row.** ADR-0017 §1.3
  stands whole and so does this entry's §4.
- **No fifth step, and no room for one at either end** — §1 above closes the
  loose end on the thumbnail cap and the tight end on the caption block.
- **No density control on `Now playing`, a record's page, a playlist's page or
  Settings** — §2.
- **No change to the keyboard.** <kbd>Ctrl</kbd>+<kbd>±</kbd> and
  <kbd>Ctrl</kbd>+scroll were never gated by place; they stepped the state
  everywhere and only the wall redrew. Making the pages read the density is
  what fixes them, and not one line of `keys.rs`'s table moved.

### Consequences

- `crates/baz/src/shelf.rs`: `Density::Compact`, its four numbers, and
  `the_ladder_only_tightens_and_the_fourth_step_halves_its_widest_rung`.
  `ALL` is `[Self; 4]`; nothing else in the type changed.
- `crates/baz/src/icon.rs`: `Glyph::DensityCompact`; the 3 × 3 field renamed to
  `DENSITY_COMPACT` and a 4 × 4 `DENSITY_DENSE` written. The family sweep now
  asserts its own length against `Density::ALL`.
- `crates/baz/src/views/mod.rs`: `density_marks`, `DetentAxis`, `MARK_INSET`
  and `density_mark` move here from `views/shelf.rs`; `section_rule_hung`
  joins `section_rule` and `section_rule_noted`.
- `crates/baz/src/views/shelf.rs`: `density_control` is now the lane's
  *placement* and nothing else. Both ADR-0028 tests survive, re-pointed.
- `crates/baz/src/views/home.rs`, `views/artist.rs`: no `Grid::new`; both take
  the shell's grid; both hang the marks on their block's rule.
- `crates/baz/src/app.rs`: `state.grid()` handed to both; `offscreen_art` loses
  its width parameter; the keyboard-mirror table's `DensityStep` row names the
  three homes.
- `.interface-design/system.md` §7.1: the fourth row, the fourth wall, and the
  placement rule.
- Captures under `docs/design/impl/density-on-every-page/` — every step on
  three pages at two windows, the absence on a page of rows, the marks pressed
  on each page, and the artist-versus-wall defect before and after.

---

## Second amendment (2026-08-10) — the ladder only tightens, and it is a proof now

**Status**: accepted · the owner, looking at the running app: *"why is balanced
smaller than compact... I think the dense should be a bit smaller"* ·
**overturns nothing in this entry's decisions** and corrects an arithmetic
defect underneath all of them · **retunes the tight end of the ladder on the
owner's taste** · adds no message, no token, no dependency and no state.

Evidence, sweeps and every frame quoted below:
`docs/design/impl/the-ladder-only-tightens/`.

### 1 · The defect: a density ladder that was not ordered

Two things in one sentence, and they are kept apart because one is a bug and
one is a preference.

The bug. Each step brings its own `hang`, and the wall's art is

```text
art = (w − (columns + 1) · hang) / columns
```

which **rises as the hang falls**. So wherever two steps land on the same
column count — which they must at any window narrow enough that the counts are
already consecutive integers, a fact this entry's first amendment already
recorded — the *tighter* step drew the *larger* work, because its gutters were
smaller. At 880 px of grid, `Balanced` hung 3 × 240.0 and `Compact` hung
3 × 250.7. At the shipped 1280 px window, 4 × 243 against 4 × 253. **That is
the ask, verbatim: balanced was smaller than compact.**

Swept 300 … 2560 px at every whole pixel, **30 of the 96 widths on the ask's
own 20 px grid inverted**. The three-step ladder that preceded `Compact`
inverted at 11 of them — 720 … 780, 1060 … 1140, 1400 … 1420, where `Spacious`
drew smaller works than `Balanced`. **`Compact` exposed the defect; it did not
introduce it.** The `git log` on the step table says so and so does the sweep:
this has been true since `b935a4e` gave the wall a zoom.

**Why the tests did not catch it, which is the part worth keeping.**
`a_tighter_step_never_hangs_fewer_works` asserted **column count**, and the
column count was correct the whole time — it is monotone by construction,
because a tighter step has both a smaller target and a smaller floor. Nothing
in the file asserted the quantity a listener actually sees. Worse, the file had
*noticed*: the same test's doc comment read *"the art is deliberately not
asserted to be monotone with it, and that is not an omission: at 1120 px
Spacious hangs 3 × 309.3 while Balanced hangs 3 × 320."* That is the inversion,
written down as a property and waved through.

### 2 · The fix: the steps partition the art range

Three fixes were available and each costs something.

- **One `hang` for all four steps.** Monotone immediately, and it throws away
  the thing the ladder was built for: a looser step is supposed to *breathe*
  more, and `Grid::header_h` — the shelf's header band — **is** the step's
  hang, so a shared hang means a shelf header that no longer zooms with the
  works. Refused.
- **Resolve the four steps together and clamp each to the one above.** The
  smallest behavioural footprint of the three (324 widths move for `Balanced`
  against 744 below) and trivially total. Refused on two grounds: it leaves
  the *cause* — overlapping ranges — in the table and guards it at resolution
  time; and it makes `Grid::new(w, Compact)` a function of Spacious and
  Balanced too, so §7.1's published table stops being computable from the row
  it publishes. This entry's reproducibility argument is that every screenshot
  is one of four **named, derivable** walls per width, and a non-local
  derivation weakens it.
- **Per-step ranges that cannot overlap.** Taken.

`Density::art_max` **stops being a tuned row and becomes derived**: it *is* the
next-looser step's `art_min`, and the loosest step's is `art::THUMB_PX`. The
four intervals abut and do not overlap:

| step | `hang` | art |
|---|---|---|
| `Spacious` | 48 | 288 … 320 |
| `Balanced` | 40 | 240 … 288 |
| `Compact` | 32 | 200 … 240 |
| `Dense` | 28 | 160 … 200 |

That is the whole proof. `Grid::new` clamps art to at most the step's cap, and
the column ceiling holds it to at least the step's floor, so a tighter step's
largest work **is** a looser step's smallest: they can meet, and they cannot
cross. It is checkable by reading four rows rather than by running a sweep.

One more rule closes the degenerate tail. Below about 416 px of grid every step
has collapsed to one column and `art` is `w − 2 · hang`, which rises as the
step tightens; so `Grid::art_cap` also caps art at **`w − 2 × WIDEST_HANG`**,
one column at the ladder's loosest hang. It binds nowhere a real window
reaches, and it is what finally makes `ART_FLOOR`'s own promise true — that
comment has said *a degenerate width yields a small wall instead of an inverted
one* since before there was a ladder to invert. The pairwise form (cap each
step against its own neighbour's hang) does not compose; one number, the
ladder's widest, closes every pair at once.

Swept at quarter-pixel resolution from 0 to 4000 px and at whole pixels to
20 000: **no inversion at any width.**

### 3 · What it costs, which is not nothing and is not hidden

**The default wall moves.** `Balanced`'s cap falls from 320 to 288, because
`Spacious` floors at 288, and **744 of the band's 2261 widths draw smaller art
than they did** — the tops of each column band, where `Balanced` used to run up
to the source's own edge. §7's published table changes in three rows: at 760,
860 and 1120 px of grid the default wall drew 320 px works and now draws 288.

It is not collateral, it is the fix seen from the other side: at those three
widths `Spacious` itself drew 308, 320 and 302.7, so the default step was
drawing a Spacious-sized cover. A ladder whose rungs name overlapping sizes has
as many rungs as it has numbers and no more. About 132 of the 744 were not
inversions — widths where `Balanced` was legitimately below `Spacious` and is
capped anyway — and those are the honest price of the ranges being disjoint
rather than merely ordered.

Two bounds hold and are asserted: every width that moves moves **down**, and
none moves below `Balanced`'s own floor 240, so no listener's wall crosses into
another step's range. `theme::ART_MAX` keeps its meaning (*no artwork is drawn
larger than its source*) and its other consumers — the album page's sleeve is
still 320.

**The rungs are shorter where they used to be backwards.** Where two steps tie
on column count the tighter one now flattens against the looser one's floor
instead of overtaking it: at 1172 px, `Balanced` 243 and `Compact` 240. A short
rung is a wall that barely changes; a backwards one is a wall that changes the
wrong way, and only one of those is a defect. **This is the thing to look at in
the frames**, and the narrow-window flatness the first amendment already
recorded is now visible in art as well as in columns.

**More widths sit in the gutters-take-the-slack regime.** Capped widths roughly
double for `Balanced` (360 → 748) and `Compact` (288 → 564). The module docs
call that the one asymmetric padding in the product; it is now less rare.

### 4 · The preference: `Dense`, and where the floor is

The second half of the ask, kept separate because it is taste rather than a
defect. `Dense` was 176 … 240 art at `hang` 28 — *today's shelf*, the 208 px
cell baz drew before density existed, and this entry's first amendment leant on
that equivalence (*"nobody loses what they have by the default moving"*). The
owner overturned it: the step exists to put the most works on screen and it was
hanging the wall baz drew when there was no ladder at all.

`Dense` is now **160 … 200**, target 184. At 1280 it hangs 6 × 162.7 where it
hung 5 × 200.8; at 1920, 9 × 170.2 where it hung 8 × 195. `Compact` is
**re-derived, not re-tuned** — it is still exactly the `Balanced`-to-`Dense`
rung halved, which is now 200 = (240 + 160)/2 and 228 = (272 + 184)/2, with the
hang's midpoint 34 still taken down to 32 on the 4 px lattice. The property
survives the retune, which is what it was written to do.

**The floor, and it is principled rather than the smallest number that fits.**
`ART_FLOOR` 1.0 is not a candidate — it is the backstop that keeps the geometry
total, and its own comment says so. Two of the product's own numbers are:

- **`art::THUMB_PX` halved.** The cache decodes to 320 px per edge and the wall
  is its largest consumer. Below half that edge the wall is discarding three
  quarters of the pixels it paid to decode, and the step stops being a density
  and starts being waste.
- **`theme::CONTINUE_SLEEVE` 132**, the smallest sleeve in the product that
  carries a record's *identity* — its own token says *"large enough that the
  record is identified by its cover rather than by its name."*
  `theme::PANEL_SLEEVE` 40 is below it and is an identifier beside a name, not
  a cover.

160 satisfies both, on the 4 px lattice, and clears the second by exactly one
`Dense` hang. It is a floor the wall really *reaches* — the tight end arrives
at 160 inside the band — so the claim is about something.

**A fifth step is still refused at both ends**, and the tight end's argument is
now this number rather than the caption block's.

### 5 · The test that was missing

`the_ladder_only_tightens_the_work_it_draws` sweeps every whole pixel of the
band **and** every quarter pixel below 420, and asserts on `art`. A single
width proves nothing here — the inversion appears and disappears with the
window, 880 inverted and 920 did not — which is why it is a sweep and why the
old test's single-width doc example was able to look like a property.

`the_steps_partition_the_art_range` pins the construction rather than the
consequence, so a later hand re-tuning a row fails on the rule rather than on a
width. `a_tighter_step_never_hangs_fewer_works` stays, and its doc now says
what it does and does not cover.

### Deliberately not done

- **No change to the marks, their placement, their glyphs or their messages.**
  This is the step table and `Grid`'s arithmetic; the control is untouched.
- **No change to `hang` at any step**, `Dense` included. A tighter hang gives
  *more* art, not less, so it is the wrong lever for the ask.
- **No fifth step and no slider.** §1 and this entry's first amendment.
- **No re-tuning to make the cost smaller.** Raising `Spacious`'s floor would
  buy `Balanced` its 320 back and turn `Spacious` into a fixed-size wall with
  pooled gutters — the design this file's module docs open by condemning.

### Consequences

- `crates/baz/src/shelf.rs`: `Density::art_max` derived; `Density::WIDEST_HANG`;
  `Grid::art_cap`; `Compact` and `Dense`'s art numbers; the monotonicity proof
  in the module docs; `the_ladder_only_tightens_the_work_it_draws`,
  `the_steps_partition_the_art_range` and
  `the_wall_hangs_no_work_below_the_size_a_cover_identifies_a_record_at`.
  Five existing tests re-pointed, three of them because the default wall moved.
- `.interface-design/system.md` §7.1: the art column, and the three §7 rows.
- Captures under `docs/design/impl/the-ladder-only-tightens/`.
