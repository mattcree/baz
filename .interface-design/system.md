# baz — design system

> **Read [ADR-0017](../docs/adr/0017-design-direction.md) first.** It resolves
> this system against an independent design critique
> (`docs/design/critique/`) and supersedes five things below: density moves
> from a Settings row to a zoom gesture; the `#`+A–Z spine index becomes an
> index rail derived from the active group key at `INDEX_W` 36; the bar's seek
> row becomes a 2 px segmented needle and the bar drops 102 → 58 px (Previous ·
> Play/Pause · Next stay); the four-room model is adopted with the `Palette`
> indirection landing early; and the WCAG contrast floors are joined — not
> replaced — by a ≥ 0.03 oklch-L step law over surfaces. Everything else here
> stands.

> The file future work reads first. Written 2026-08-08 for the **gallery /
> archive** direction the owner chose over the previous "listening room".
> Every number here is either a token that already exists in
> `crates/baz/src/theme.rs` or a change carried with its reason.
> The long argument is `docs/design/02-visual-language.md`; the pictures are
> `docs/design/visual/gallery/`.

---

## 1. Direction and feel

**A record archive after closing time. The works are lit; the room is not.**

baz is a hang, not a dashboard. The wall is near-black and *neutral-cool* — the
matte paint of a black-cube gallery, never the warm charcoal of a listening
room and never the blue-grey of a stock dark theme. The type is warm ivory, the
colour of archival mount board, so the room is cold and the paper is warm, which
is what a gallery actually looks like at night. There is one light in the room
and it is pointed at one thing: the record that is playing. Everything else —
every control, count, setting and state — is made of surface, edge and ink.

**Who it is for.** People who own their music. Marta, whose 40 000 tracks should
look like a collection; Devon, who wants the sleeve at a size worth looking at;
Karl, who wants the truth about the signal path stated in the quietest possible
voice. Nobody here is being sold anything.

**What it deliberately is not.** Not a streaming client (no blue, no rounded
artwork, no colour-washed chrome, no play button hovering over a cover). Not a
skeuomorphic hi-fi (no wood, no VU needles, no drawn shelf). Not a card grid.

### 1.1 The signature — the picture light and its label

Two halves of one component, and they are the whole of baz's identity.

**The light.** One accent, reserved for playback truth, whose **hue is read from
the playing sleeve** — lightness fixed at the coordinates of amber, chroma
fixed, so it is always recognisably the same lamp with a different record in
front of it. Amber is the default and the fallback.

**The label.** Every album in baz is captioned by a **wall label**: a
fixed-height, left-aligned, two-line block set beneath the work, never on it.
Same object at four scales. When the work is playing, the label's first line
carries the lamp dot and the work carries the halo.

Where the signature appears — five concrete places, and it is locatable in each:

1. **The shelf tile** — label under every sleeve; playing gains dot + halo.
2. **The album inspector header** — the same label, larger, plus the catalogue
   line and the condition report.
3. **The now-playing bar's left zone** — the same label at bar scale. *The bar
   is a wall label for what is sounding.*
4. **Up-next popover rows** — one-line labels; the playing row's dot replaces
   its number in a column that never changes width.
5. **The seek groove** — the light laid flat: elapsed fill and knob take the
   lamp hue; the elapsed stamp warms while a seek is in flight.

Plus the primary **Play album** button — the switch that turns the picture
light on, and the only control in baz outlined in the accent.

### 1.2 The three rules the direction reduces to

1. **The shelf contains exactly two kinds of thing: artwork and type.** No
   cards, no borders, no radii, no shadows, no badges, no overlays, no
   separators. Checkable in one glance at a screenshot.
2. **Nothing is ever drawn on top of a sleeve.** The only thing that touches
   artwork is light around it.
3. **No artwork is ever drawn larger than its source.** `ART_MAX == THUMB_PX`,
   asserted.

---

## 2. Depth strategy

**Surface-step elevation. One committed approach, and the other two are
explicitly out.**

- **Not borders-only.** A gallery wall has no lines drawn on it. Hairlines
  survive in exactly three roles (below) and nowhere else.
- **Not shadows.** *Measured*: black at 55 % composited over the wall
  (`#0C0D0E`) yields `#050606` — a contrast ratio of **1.04 : 1**. On
  near-black a drop shadow is not a design tool, it is a rounding error. The
  previous direction's contact shadow is **deleted**, not tuned.

Elevation is a step in surface lightness and nothing else:

| Surface | Hex | Linear L | Step | Role |
|---|---|---|---|---|
| `RECESS` | `#060708` | 0.00208 | — | the shadow gap: inset chrome, *below* the wall |
| `WALL` | `#0C0D0E` | 0.00398 | ×1.91 | the hanging wall — app background |
| `PLINTH` | `#141517` | 0.00747 | ×1.88 | one step up: inspector column, popover, resting control |
| `PLINTH_LIT` | `#1C1D20` | 0.01230 | ×1.65 | one step above that: selected segment, playing row, hovered control |

Whisper-quiet in bytes (8 apart), plainly felt in linear light (nearly 2×
per step, which is what the eye actually uses at these levels). Squint and you
perceive four planes and no edges.

**Hairlines** exist in three roles only: the rule under the top bar, the rule
above the now-playing bar, and the rule dividing the inspector from the shelf.
`HAIRLINE` = `PAPER` @ **7 %**; `HAIRLINE_STRONG` = `PAPER` @ **15 %** (down
from 8 % / 17 %, because the same alpha over a darker ground is a larger step).
iced 0.13's `Border` is four-sided, so every single line is a `rule` widget.

**The one shadow primitive in the product is the playing halo**, and it is not
elevation — it is light.

---

## 3. Spacing

Base unit **4**. The existing ladder, plus two names the gallery needs.

| Token | px | Used for |
|---|---|---|
| `GAP_XXS` | 2 | lines within one block |
| `GAP_XS` | 4 | dot to label, row padding, chip padding |
| `GAP_SM` | 8 | siblings within a group |
| `GAP_MD` | 12 | groups within a surface |
| `GAP_LG` | 16 | surface padding, bar gutters, **work → label** |
| `GAP_XL` | 24 | panel padding, settings sections |
| `HANG` | **40** ⟵ NEW | the grid's one number: work-to-work *and* work-to-wall-edge |

Padding is symmetric unless a token says otherwise. The two asymmetric paddings
in the product are `scroll_gutter()` (right only, reserving the scrollbar lane)
and the grid's margin when the art is at `ART_MAX` (§7).

---

## 4. Palette

**There is no longer one palette; there is a room.** ADR-0017 §1.5 adopts the
four-room model, and build-plan step 2 has landed the mechanism: every value
below is a field on `theme::Palette` rather than a `pub const Color`, and ~30
style functions take a `&Palette`. Two rooms are defined.

| Room | Wall | Ink | Accent | Status |
|---|---|---|---|---|
| **Closing Time** | `#0C0D0E` cool near-black | `#E8E4DB` warm ivory | amber `#E3A14E` | the room baz is |
| **Reading Room** | `#EEEBE4` warm ivory | `#1E2226` cool near-black | oxblood `#A33E25` = `oklch(0.50 0.14 35)` | **defined, not selectable** |

Reading Room's surfaces descend as they rise (`recess` `#FAF6EF` is the
*lightest* plane, `plinth` `#E3DFD8` and `plinth_lit` `#D7D4CD` the raised
ones), its ink ramp is `#1E2226` / `#393E42` / `#575B60` / `#70757B`, and its
focus ring is the room's ink at **55 %** rather than 45 % — the one alpha a
room sets for itself, because the same opacity over a lighter ground is a
smaller step. It ships when §1.5's pale-sleeve-on-paper question has an answer
that is not a border on artwork (step 20); until then `theme::follow` returns
Closing Time for every desktop and `theme::READING_ROOM_SHIPS` is the whole of
the gate. `BAZ_ROOM=reading-room` renders it for review and is a development
hatch, not a product surface.

The table below is **Closing Time**. Hex is the exact sRGB of the `f32`
values, rounded to the nearest byte.

| Token | Hex | Role | May **not** be used for |
|---|---|---|---|
| `RECESS` | `#060708` | shadow gap: bar, input wells, groove troughs, sleeve backing | text, raised surfaces |
| `WALL` | `#0C0D0E` | the hanging wall | anything raised |
| `PLINTH` ⟵ was `CARD` | `#141517` | inspector column, popover, resting control | the shelf |
| `PLINTH_LIT` ⟵ was `CARD_HIGH` | `#1C1D20` | selected segment, playing row, hovered control | anything at rest |
| `HAIRLINE` | `PAPER` @ 7 % | the three structural rules | decoration, tile edges |
| `HAIRLINE_STRONG` | `PAPER` @ 15 % | a hovered tile's label rule, a selected control's edge | a resting border |
| `PAPER` | `#E8E4DB` | primary text | large fills |
| `PAPER_DIM` | `#ABA8A1` | secondary text | figures that tick |
| `PAPER_FAINT` | `#888680` | tertiary text, **the selected tile's rule** | primary labels |
| `PAPER_MUTED` | `#6C6A66` | set but not sounding | text a user must read |
| `PAPER_RING` | `PAPER` @ 45 % | keyboard focus, `text_input` only | anything else |
| `SELECT_WASH` | `PAPER` @ 18 % | `text_input` selection | backgrounds |
| `LAMP` | `#E3A14E` | **the accent** — §5 | see §5 |
| `LAMP_BRIGHT` | `#F1B362` | the accent, hovered | a resting state |
| `LAMP_DEEP` | `#C7883D` | the accent, held | a resting state |
| `LAMP_GLOW` | `LAMP` @ 45 %, blur **24** | the playing sleeve's halo | fills, borders, text |
| `LAMP_WASH` | `LAMP` @ 10 % / 20 % | Play album, hovered / pressed | any resting state |
| `ALERT` | `#D9776B` | problems, stated quietly | anything merely unusual |
| `SUCCESS` | `#86A97C` | *nothing yet — keep the slot* | decoration |

**Deleted:** `SHADOW` (§2), `RADIUS_TILE` (§6), `LAMP_INK` (§5 — nothing sits
*on* the accent any more).

**One board at four levels of light.** `PAPER_DIM`, `PAPER_FAINT` and
`PAPER_MUTED` are the same r:g:b ratios as `PAPER`, scaled — so the ink family
is one material, not four greys that drifted. Every value is the *smallest*
point on that ramp clearing its floor on every surface it can land on, with
0.1 of margin.

### 4.1 Contrast (WCAG 2.1, computed against the gallery surfaces)

| Ink | on `RECESS` | on `WALL` | on `PLINTH` | on `PLINTH_LIT` | Floor | |
|---|---|---|---|---|---|---|
| `PAPER` `#E8E4DB` | 15.89 | 15.33 | 14.40 | 13.28 | 4.5 | pass |
| `PAPER_DIM` `#ABA8A1` | 8.49 | 8.20 | 7.70 | 7.10 | 4.5 | pass |
| `PAPER_FAINT` `#888680` | 5.54 | 5.34 | 5.02 | **4.63** | 4.5 | pass |
| `PAPER_MUTED` `#6C6A66` | 3.74 | 3.60 | 3.39 | 3.12 | 3.0 | pass |
| `LAMP` `#E3A14E` | 9.10 | 8.78 | 8.24 | 7.60 | 3.0 | pass |
| `ALERT` `#D9776B` | 6.53 | 6.30 | 5.92 | 5.46 | 4.5 | pass |

`PAPER_FAINT` on the top surface is **4.63**, not the 4.483-rounded-to-4.5 the
previous pass had to name as an exception — so `theme.rs`'s `ROUNDING` excuse in
the contrast test is deleted.

**Two laws, over disjoint domains** (ADR-0017 §1.6), both asserted by
`every_ink_and_every_surface_clears_its_floor`:

1. **Surface against surface** is measured in **oklch L**, never in WCAG: the
   wall on the plinth is 1.30 : 1 and that number carries no information.
   Adjacent levels differ by **≥ 0.03 L** and no room's surfaces sit in the
   **dead zone L .45–.58**. Closing Time steps +0.0311 / +0.0367 / +0.0360;
   Reading Room −0.0338 / −0.0356 / −0.0345. (The dead zone is a rule about
   rooms. An ink or an accent may live there, and Reading Room's oxblood, at
   L 0.4997, does.)
2. **Ink against surface** is measured in WCAG 2.1, and **opacity is
   composited before the ratio is taken** — an alpha is a colour once it is
   drawn, so a hierarchy expressed in opacity cannot hide an unreadable value
   from a test that sees only opaque tokens.

**The exemption list, by name**: `hairline`, `hairline_strong` and `lamp_glow`
are non-text marks that exist only to be locatable and are never read, so the
3 : 1 mark floor is the wrong instrument and the L-step law governs them
instead. The needle's unfilled track and the index rail's absent letters join
the list when steps 8 and 9 build them. `select_wash` is exempt as a *mark* and
measured as a *ground*: what a user reads is `PAPER` on the composited wash
(10.60 : 1 in Closing Time, 10.38 : 1 in Reading Room). Everything else keeps
its floor. An exemption list you must add a name to is a rule; "WCAG is
meaningless here" is not.

---

## 5. The accent discipline

**Playback truth** is a fact about the audio the engine is producing *right
now*: which album is sounding, which track, and where the playhead is. Nothing
else qualifies.

`LAMP` and its relatives may appear in exactly these places:

1. the playing album's **halo** (`LAMP_GLOW`, artwork at any size);
2. the playing **dot** (`DOT` 6 px, in a label's first line or a row's number
   column);
3. the **seek groove**'s elapsed fill and knob;
4. the **elapsed stamp** while a seek is in flight;
5. the **Play album** button's 1 px border and its play triangle — it is the
   only control that *creates* playback truth.

**Amber is never an opaque fill.** It appears only as a ≤ 6 px mark, a 4 px
rail, a 1 px line, or light. The previous direction made Play album a solid
lamp rectangle as an argued exception; under a room this quiet that slab became
the loudest thing on screen and was *not* the artwork, so the exception is
revoked in favour of a rule with no exceptions. Play album is `LAMP` outlined
with a `LAMP` triangle and a `PAPER` label, over `LAMP_WASH` at 10 % hovered
and 20 % pressed. `LAMP_INK` therefore has no remaining use and is deleted.

It may **not** appear on: input focus, text selection, the scanning note, tile
hover or selection, the queue popover's header or active affordance, the
Settings nav's current section, the Previous button, panel toggles, the edition
or ReplayGain selectors, the volume fader, the unity detent, hover previews,
tooltips, scrollbars, checkboxes, steppers, the wordmark, or any readout.

---

## 6. Radii

| Token | px | Applies to |
|---|---|---|
| — | **0** | **artwork, always**, and every rule |
| `RADIUS_SEGMENT` | **3** ⟵ was 4 | a segment inside its well, a checkbox, a queue/track row |
| `RADIUS_CHIP` | **3** ⟵ was 4 | hover tips, tooltips |
| `RADIUS_CTRL` | **4** ⟵ was 6 | buttons, inputs, wells, steppers, the popover |
| `DOT / 2` | 3 | the playing dot |
| `RADIUS_TILE` | **deleted** | the shelf has no rectangles that are not artwork |

Sharper than the listening room because an archive is rectilinear and a sleeve
has square corners. Nesting holds: 3 inside 4.

---

## 7. The hang — the shelf grid

**One number drives the grid.** `HANG` = 40 is the distance from a work to its
neighbour *and* from a work to the edge of the wall. The art absorbs all
remaining width up to `ART_MAX`; past that the margins absorb it.

```
grid_w(W)   = W - inspector_w(W) - SCROLLBAR_LANE - INDEX_W     # NOT the window

columns(w)  = clamp(floor((w + HANG) / (ART_TARGET + HANG) + 0.5),
                    1,
                    max(1, floor((w - HANG) / (ART_MIN + HANG))))
art(w)      = min(ART_MAX, (w - (columns + 1) * HANG) / columns)
gutter(w)   = columns > 1 ? (w - 2*HANG - columns*art) / (columns - 1) : 0
              # clamped to 2*HANG; surplus goes to the margins, block centred
row_h(w)    = art(w) + GAP_LG + LABEL_H + HANG
```

`floor(x + 0.5)`, never a language's `round`: Rust's `f32::round` is
half-away-from-zero and Python's is banker's, and a grid whose column count
depends on which language expressed it is not a specification.

**`HANG`, `ART_MIN`, `ART_TARGET` and `ART_MAX` are all functions of the
density step** (§7.1). The values below are `Balanced`, the default.

| | `HANG` | `ART_MIN` | `ART_TARGET` | `ART_MAX` |
|---|---|---|---|---|
| Spacious | 48 | 288 | 320 | 320 |
| **Balanced** | **40** | **240** | **272** | **320** |
| Dense | 28 | 176 | 200 | 240 |

| Shelf width | Columns | Art | Gutter | Margin | Row pitch | Today | Today's dead gutter |
|---|---|---|---|---|---|---|---|
| 640 | 2 | 260 | 40 | 40 | 352 | 2 × 208 | 112 px |
| 760 | 2 | 320 | 40 | 40 | 412 | 2 × 208 | 232 px |
| 860 | 2 | 320 | 80 | 70 | 412 | 3 × 208 | 92 px |
| **922** (1280 − inspector) | 3 | **254** | 40 | 40 | 346 | 3 × 208 | **154 px** |
| 1120 | 3 | 320 | 40 | 40 | 412 | 4 × 208 | 112 px |
| **1280** (no inspector) | **4** | **270** | 40 | 40 | 362 | 5 × 208 | 32 px |
| 1500 (1920 − inspector) | 5 | 252 | 40 | 40 | 344 | 6 × 208 | 12 px |
| **1920** (no inspector) | **6** | **273** | 40 | 40 | 365 | 7 × 208 | 192 px |
| 2560 | 8 | 275 | 40 | 40 | 367 | 10 × 208 | 112 px |

**Whenever the art is not capped, the gutter is exactly `HANG`** — the formula
puts every spare pixel into the artwork, so dead gutter is **0 px at every
width**. That is the proportion fix, in one number.

**Built** (ADR-0017 step 5). `crates/baz/src/shelf.rs`'s `Grid` is the
arithmetic above; `theme.rs` carries `HANG` / `ART_MIN` / `ART_TARGET` /
`ART_MAX`; the tile is sized by the grid it is handed and computes none of it.
`the_hang_reproduces_the_specifications_table` asserts all nine rows of the
table, and `the_gutter_is_the_hang_wherever_the_art_is_uncapped` asserts the
0 px claim at **every width from 300 to 2560 at 1 px** — the transitions are
single-pixel events, so a coarse sweep can step over one. The virtualization
test moved to the same 1 px band. Measured on real pixels at 1280 the wall
reads `40 | 270 | 40 | 270 | 40 | 270 | 40 | 270 | 40`, with **0 px
unaccounted**. `ART_MAX` now equals `THUMB_PX`, which went 256 → 320 with the
LRU re-derived to **384 entries** at the same ~150 MiB budget.

Two things the shipped grid does that the table does not say. The art is
**not** rounded to a whole pixel — at 1920 it is 273.33 and the rasterizer
alternates 273/274 across the row — because rounding it is exactly what would
put the difference back into the gutter. And the scrollbar now overlays the
right *margin* rather than taking width from the block, which it can do
without clipping anything: the margin is 40 and the bar's lane is 10.

`ART_MAX = 4/3 × ART_MIN` deliberately, so at every column-count change the art
hands off from its largest to its smallest with no ambiguity: 320 → 240 at
exactly one width per transition.

**Generous negative space, in numbers.** Not more space proportionally — the
same ~64 % art coverage as today — but *redistributed*: every spare pixel goes
between the works instead of pooling at the window's edge, and the works are
**30 % larger in linear terms and 68 % larger in area** at 1280 px.

### 7.1 Density is a user control

`03-interface-prior-art.md` R7 contradicted an earlier draft of this section:
making the cell a function of the viewport fixes dead gutters but still leaves
the user with a designer's constant. Density control is universal outside music
(Lightroom, Calibre, Steam, Plex, Google Photos) and **two products that removed
one took durable damage**. Under a direction that deliberately shows *fewer,
larger* covers this matters more, not less: 300 albums and 40 000 albums do not
want the same wall.

**Three named steps, in Settings → Appearance. Not a free zoom** — a slider
makes every screenshot of baz different and every layout bug unreproducible.
Steps parameterise the hang rather than overriding it, so §7's properties
(`gutter == HANG` when uncapped, `art ∈ [ART_MIN, ART_MAX]`) hold at every step.

| Step | at a 1280 window | at 1920 | Note |
|---|---|---|---|
| **Spacious** | 3 × 320 | 5 × 320 | art pinned at `ART_MAX`; the margins take the slack |
| **Balanced** | 4 × 262 | 6 × 268 | the default |
| **Dense** | 5 × 216 | 8 × 205 | roughly what baz ships today |

Named plainly rather than in the room's vocabulary, deliberately: a setting is
where the software talks about *itself*, and it is the one place baz uses plain
UI language. Drawn in `docs/design/visual/gallery/06-density.png`.

`ART_MAX` never exceeds `THUMB_PX` at any step, so the *nothing upscales*
invariant is a property of the system, not of the default. At `Dense` the cache
holds 320² thumbnails for ~200 px tiles; a density-aware decode size is the
obvious optimisation and is deliberately not taken here.

### 7.2 The spine index

`03-interface-prior-art.md` R8: losing jump-to-letter was the single most
concrete regression Sonos users named. **A wall of covers is beautiful at 200
albums and unusable at 5 000 without an index** — and a gallery direction makes
scrolling *longer*, so this gets worse under this design, not better.

`INDEX_W` **60 px**, a lane the shelf keeps clear on its right, outside the
scrollbar's, with `HANG` between the lane and the window's edge (§13, L1). It
was 20 in this document and 36 in ADR-0017 §1.7, and both were measured wrong:
at 36 the lane clips `Unknown`, every recency bucket and most genre names, so it
worked for one of the five group keys and failed for three. 60 holds every label
the keys can *produce* — the widest, `Never played`, measures 59.14 px — and only
arbitrary genre names elide, with the full value one gutter to the left in the
shelf header. A–Z plus `#`, `SIZE_HEADING` at its own 12 px line box. Letters the collection has an
album under are `PAPER_FAINT`; letters it does not are `PAPER_MUTED` — an index
that hides its gaps lies about the collection. The key under the pointer or at
the current scroll position is `PAPER` Medium. **Never the accent**: an index is
navigation, not playback truth. When 27 keys do not fit the height the run
subsamples, the pattern every phone contact list uses.

It is **type, not chrome**, so §1.2's claim survives intact: the shelf still
contains exactly two kinds of thing.

Type-to-jump is `/`-scoped, never type-anywhere — the audit (§4.8) already
resolved that bare letters belong to the transport.

---

## 8. Type — proportional everywhere

**No monospace anywhere in baz.** `MONO` is deleted. So is `SERIF`.

**The measurement that made it possible.** IBM Plex Sans ships **tabular
figures by default**: every digit `0`–`9` advances exactly **600/1000 em** in
Regular, Medium *and* SemiBold — the same advance as Plex Mono. Confirmed
through HarfBuzz with default features on (`calt`/`liga`/`kern` applied), so it
is what cosmic-text will actually draw. `0:00:00` and `9:59:59` measure
**43.008 px** each at `SIZE_META`; the delta is 0.0000 px. Alignment is not
approximated, it is exact.

Bundle: **three faces** (Sans Regular / Medium / SemiBold), 605 592 bytes —
down 395 928 bytes from the five that ship today. OFL-1.1, verbatim upstream,
hashes in `crates/baz/assets/fonts/README.md`.

**The line box is the token; the leading is derived.** This table used to give
the leading and let the box fall out, and the six boxes came to 15.95, 16.20,
18.20, 20.25, 22.80 and 32.20 — **not one of them a multiple of the spacing
unit**. `06-composition-audit.md` §2 measured the consequence: pooled over the
whole application a 4 px lattice caught 77–80 % of the drawn chrome edges
against a 75 % null, which is chance. There was no vertical rhythm and there
could not be one, because the type was not in it. Quantising the boxes cost at
most 1.8 px on one token (§13, L2).

| Token | px | line box | leading | weight | Used for |
|---|---|---|---|---|---|
| `SIZE_HEADING` | 10 | **12** | 1.200 | Medium | shelf breaks, the index rail, group keys |
| `SIZE_CAPTION` | 11 | **16** | 1.455 | Regular | tooltips, hover tips, footnotes |
| `SIZE_META` | 12 | **16** | 1.333 | Regular | label line 2, durations, counts, notes, control labels |
| `SIZE_BODY` | 13 | **20** | 1.538 | Regular / Medium | label line 1, track titles, button labels |
| `SIZE_EMPHASIS` | 15 | **20** | 1.333 | Regular / Medium | section headings, empty-state lines, inspector artist |
| `SIZE_TITLE` | 19 | **24** | 1.263 | SemiBold | the album's title |
| `SIZE_HERO` | 28 | **32** | 1.143 | SemiBold | the first-run question |

`LABEL_LINE_H` = `LINE_BODY` = 20; `LABEL_H` = 2 × that = **40 = `HANG`** — a
wall label is exactly one hang tall, and the tile's row pitch is therefore
`art + 96`. Two independent one-line lanes, not one two-line box, so a long
title clips at one line instead of pushing the artist line out of the slot
reserved to keep it still.

Two further numbers fall out rather than being chosen: caption and meta collapse
onto one 16 px lane, so the bar's left zone is stacked out of two heights instead
of five; and a text well's vertical padding is one number, `(32 − 20 − 2) / 2`,
because both wells baz draws take the same 20 px box (§13, L7).

Emphasis comes from **weight, ink and size only** — iced 0.13 exposes no
letter-spacing, no small caps and no OpenType features, so nothing in this
system may depend on them.

### 8.1 Reserved slots, re-derived from the real advances

Measured in Plex Sans Regular at the size the slot uses. The mono column is
what the same string costs today.

| Token | Today | Sans px | Mono px | New | Worst case |
|---|---|---|---|---|---|
| `STAMP_W` | 52 | 50.21 | 57.60 | **52** | `10:00:00` |
| `SIGNAL_W` | 120 | 92.38 | 108.00 | **96** | `192 → 176.4 kHz` |
| `LEVEL_W` | 62 | 43.34 | 52.80 | **48** | `-18.1 dB` |
| `PREVIEW_W` | 58 | 39.42 | 46.20 | **48** | `0:00:00` + padding |
| `SETTING_VALUE_W` | 68 | 56.89 | 64.80 | **60** | `+20.00 dB` |
| `TRACK_NO_W` | 24 | 21.60 | 21.60 | **24** | `999` |
| `POSITION_W` | — | 53.46 | 64.80 | **56** | `199 / 240` |

`STAMP_W` keeps its number and gains a capability: at 52 px the *mono* face
could not hold `10:00:00` (57.60 px — the current build clips a ten-hour
track); Sans holds it at 50.21 with 1.79 px to spare.

### 8.2 The one place a proportional face can still jiggle

Hyphen-minus advances 0.399 em; `+` and U+2212 both advance 0.600 em. So
`-20.00 dB` measures 54.48 px and `+20.00 dB` 56.89 px — a 2.4 px shift in a
right-aligned slot's *left* edge as the ReplayGain pre-amp steps through zero.

- **Fix**: `replaygain::format_centidb` emits **U+2212** for negatives. Then
  `−20.00 dB` and `+20.00 dB` measure 56.89 px each, exactly.
- **Residual, accepted**: unsigned `0.00 dB` is 7.2 px narrower, at one point
  in the travel, changing only when a human presses a stepper.
- **Never acceptable**: anything that ticks with playback — elapsed, remaining,
  seek preview, level tip, queue position. All are fixed-digit-count strings
  and Plex Sans makes them exact to 0.000 px.
- **Rule**: figure columns are **right-aligned**. Ragged-left reads fine
  editorially and pins the edge the eye follows.

---

## 9. Component patterns

### Album tile
`art × art` square, radius 0, no shadow, no card, nothing behind it. Then
`GAP_LG` (16), then the wall label: title `SIZE_BODY`/1.40 Medium `PAPER`
(`Wrapping::None`), `GAP_XXS`, artist `SIZE_META`/1.35 Regular `PAPER_FAINT`.
The label block is `LABEL_H` = 36.4, left-aligned to the art's left edge.

| State | Mark |
|---|---|
| rest | none |
| hover | artist lifts to `PAPER_DIM` **+ a 1 px `HAIRLINE_STRONG` rule under the label**, art-width |
| pressed | identical to hover |
| selected | **a 2 px `PAPER_FAINT` rule under the label**, art-width |
| playing | composes with any of the above: `LAMP_GLOW` halo (blur **24**, offset 0) + `DOT` before the title |

The rule under the label is the shelf's only state vocabulary — no card, no
border, no radius, no accent. Hover and selection are 1 px hairline versus 2 px
paper, a 2× thickness and ~4× ink jump apart, which is what the audit's
"hover and selection are nearly the same mark" was asking for.

### Album inspector
Column background `PLINTH`, `GAP_XL` padding, a 1 px `HAIRLINE` rule against
the shelf. Order: sleeve (`min(column − 2·GAP_XL, ART_MAX)`, **left-aligned**,
halo when playing) → title `SIZE_TITLE`/1.20 SemiBold `PAPER`, two lines max →
artist `SIZE_EMPHASIS` `PAPER_DIM` → catalogue line `SIZE_META` `PAPER_FAINT`
(`1992 · 13 tracks · 45:35`) → condition report `SIZE_META` `PAPER_FAINT`
(`FLAC · 16-bit · 44.1 kHz`) → edition selector when there is a choice →
`Play album` → track list, reading width capped at 600 px → **Details**.

**Details** is the condition report in full: a `HAIRLINE` rule, the word
`Details` in `SIZE_META` `PAPER_MUTED`, then one row per field — label
right-aligned in `FIELD_LABEL_W` 96 in `PAPER_MUTED`, value left-aligned in
`PAPER_DIM`, 17 px pitch, a row only when the scan read one. Album artist,
Released, Label, Catalogue, Genre, Discs, Format, Bitrate, Size, ReplayGain,
MusicBrainz, Added, Path.

**No disclosure, no click.** `03-interface-prior-art.md` R6: baz's audience came
from a product that showed ~20 fields for free, and four lines is a regression
for Marta and Karl. It sits below the track list, so it is below the fold — the
wall label carries the essentials and the condition report is on the back of the
card, which you turn over by scrolling. Devon never sees it; Marta never has to
ask for it. Drawn in `07-inspector-full.png`.

### Track / queue row
`TRACK_NO_W` 24 right-aligned number in `SIZE_META` `PAPER_FAINT` (the lamp dot
replaces it when playing, in a column that never changes width) · title
`SIZE_BODY` · duration `SIZE_META` `PAPER_FAINT`, **right-aligned**. Row pad
`pad(GAP_XS, GAP_XS)`, `RADIUS_SEGMENT` 3. Playing: `PLINTH_LIT` +
`HAIRLINE_STRONG`. Hover: `PLINTH`. List keeps `scroll_gutter()`.

### Now-playing bar
**102 px in every state**, three zones on one centre line. Left: the wall label
at bar scale plus the `POSITION_W` 56 slot (`3 / 12`), max width capped so it
clips rather than wraps. Centre: Previous · Play/Pause · Next over the seek
groove in a fixed `SEEK_ROW_W` 380 column. Right: `SIGNAL_W` 96 then the volume
block, right-aligned.

**The bar carries no artwork.** In a gallery the label does not reproduce the
work.

### Grooves
`RAIL` 4 in a `RECESS` trough, radius `RAIL/2`, **no border** (one fewer
hairline). Seek: `LAMP` → `LAMP_BRIGHT` hovered → `LAMP_DEEP` dragged, knob 5 →
7. Volume: `PAPER_FAINT` → `PAPER_DIM`, knob never changes size (it would drag
the unity detent). Detent `HAIRLINE` at rest, `PAPER` engaged. Never amber.

### Controls
Segmented control: `RECESS` well, `RADIUS_CTRL` 4, `SEGMENT_INSET` 2; segment
`RADIUS_SEGMENT` 3, selected = `PLINTH_LIT` + `HAIRLINE_STRONG` + `PAPER`,
unselected = no background + `PAPER_DIM`, hovered = `PLINTH` + `PAPER`.
Transport: `TRANSPORT_HIT` 32 square, `PLINTH` + `HAIRLINE`, hover
`PLINTH_LIT` + `HAIRLINE_STRONG`, press `RECESS`, pending = opacity only.
Primary (Play album): `TRANSPORT_HIT` 32 tall, full column width, 1 px `LAMP`
border, `LAMP` triangle, `PAPER` SemiBold label, no fill at rest. At most one
per screen.

### States
Every interactive element carries default / hover / pressed / disabled, and
`text_input` additionally focused (`PAPER_RING`). Data carries scanning, empty,
no-match and error. **baz has no spinner and no progress bar anywhere** — the
shelf filling *is* the scan indicator.

---

## 10. Motion

**Every state change takes 0 ms.** iced 0.13 ships no animation runtime;
driving one from `window::frames()` redraws while idle, and baz measures its
startup in hundreds of milliseconds. Permitted movement: the seek fill and the
elapsed stamp advancing with playback (that is data arriving), and scrolling.

Never animated at any version: the bar's geometry, the grid (no stagger, no
fade as thumbnails decode — a thumbnail replacing its placeholder is an instant
swap), artwork. If a runtime arrives: the hover rule 90 ms, a panel 140 ms, the
lamp's hue 200 ms linear. No spring, no bounce, no overshoot.

---

## 11. Performance budget

| Change | Per-frame | Other |
|---|---|---|
| three faces instead of five | none | −395 928 bytes of binary |
| fluid cell width | none — arithmetic per layout pass, not per tile | `shelf.rs` constants become functions of width |
| `THUMB_PX` 256 → 320 | none | LRU **600 → 384 entries** at the same 150 MiB (400 KiB/entry, 36 % fewer, ~8× the live widget count) |
| no shadow on tiles | one fewer quad per tile | none |
| art-derived lamp | none | one 4 k-sample histogram per **track change**, sub-millisecond |

**Forbidden on performance grounds:** blur or backdrop of any kind; any
per-frame animation or idle redraw; artwork above `THUMB_PX`; per-tile
gradients; shadows on anything that is not the playing halo.

---

## 12. iced 0.13's hard limits

| The design wants | Fallback taken |
|---|---|
| rounded / clipped artwork | square sleeves, embraced — records are square |
| tabular figures via `tnum` | **not needed**: Plex Sans's digits are tabular by default (§8) |
| letter-spacing, small caps | emphasis is weight, ink and size only |
| a single-sided border | `rule` widgets — already how the bars are built |
| an icon set | closed polygons in a unit square (`icon.rs`); no strokes, no caps, no true arcs |
| a focus ring on buttons | `PAPER_RING` on `text_input` only; tooltips name icon-only controls |
| transitions | 0 ms everywhere |
| pointer capture | end the gesture on `CursorLeft`/`Unfocused` and commit (`groove.rs`) |
| text ellipsis | `Wrapping::None` clips; every clipping slot has a fixed width |
| shadow spread | tuned via blur (`LAMP_GLOW` only) |
| an accessibility tree | contrast floors and hit targets are the guarantees baz *can* make, so honour them exactly |

---

## 13. Composition laws

Ten tokens can all be right and the frame still read as subtly off, because
**tokens are not composition**. `docs/design/06-composition-audit.md` measured
where, off real pixels, with the rulers committed at
`docs/design/composition/tools/`: three window gutters in one application, a bar
whose seven mark-lines spanned 50 px inside a 102 px band, and a type scale
whose line boxes were not multiples of its own spacing unit.

The seven laws below close that. Each one is **assertable**, and each carries
the test that pins it — because the project's own history is that a rule which
is not pinned drifts, and the accent discipline (§5) and the contrast floors
(§4.1) are the two that prove the habit works.

| Law | Pinned by |
|---|---|
| L1 one gutter per window edge | `theme::one_gutter_touches_every_window_edge` |
| L2 the unit is 4, and the type is in it | `theme::the_vertical_unit_is_four_and_the_type_is_in_it` |
| L3 optical centring | `theme::a_fixed_box_states_how_its_content_is_centred` |
| L4 one centre line per bar | `theme::the_bar_has_one_centre_line_and_every_mark_is_on_it`, `views::bottom_bar::every_mark_in_the_bar_sits_on_the_bars_one_centre_line` |
| L5 the permitted alignment edges, per surface | `theme::every_surface_declares_the_edges_it_permits` |
| L6 hierarchy is declared and then measured | `theme::the_declared_hierarchy_is_the_geometry_that_produces_it`, plus the render pass |
| L7 one control height | `theme::the_product_stands_at_one_control_height` |

### L1 — One gutter per window edge

> Every surface that touches a window edge hangs from the same two lines:
> `x = HANG` and `x = W − HANG`, and `y = HANG` where a surface has a free top.
> `GAP_LG` is a gap *between* things and `GAP_XL` is padding *inside* a panel;
> **neither is ever a window margin**. A panel's own content keeps `GAP_XL` from
> the panel's edge, which is a different edge.

baz drew its chrome on 16, its panels on 24 and its collection on 40, so nothing
in either bar was aligned with anything on the wall, at either width, by exactly
24 px. Six of the wall's sixteen x-edges were singletons because of it. There
are four window-edge surfaces — the top strip (in both places), the now-playing
bar, the Settings place and the index rail — and the test names all four by the
literal a reviewer would have to change to break it.

### L2 — The vertical unit is 4, and the type is inside it

> Base unit **4**. Every gap, every reserved slot height, every control height
> **and every line box** is an exact multiple of 4. A leading is chosen so that
> `size × leading` is a multiple of 4, not the other way round.

The one named exception is `GAP_XXS` 2 — an intra-block line gap, never a slot —
and the test asserts both that it is 2 *and* that it is off the lattice, so the
exception cannot quietly become a habit. Compile-time: it fails the build rather
than the review.

### L3 — Optical centring: the box centres the ink, not the line box

> Content shorter than the box that holds it is centred in **both** axes by the
> box. A `button` with a fixed height always states its content's vertical
> alignment; a `container` with a fixed width always states its horizontal one.
> Where a mark's optical centre differs from its bounding box — the play
> triangle is the only one in the product — the *mass centroid* is what is
> centred.

Every glyph in a hit box was already centred to a pixel, which is what made the
two failures a locatable mistake rather than a habit: `Settings` sat 6.4 px above
its own centre and `Play album` 6.0 px above and **86.5 px left** of its, both
because iced 0.13 lays unaligned content out at the top-left of a fixed box.

### L4 — One centre line per bar

> A bar has one horizontal centre line and every *mark* in it sits on that line:
> glyph centres, rail centres, and the baseline of any single-line label. A zone
> taller than one line hangs its extra lines **symmetrically** about that line.
> Zones are centred by their marks, never by their blocks.

Structural rather than nudged. The bar's band is
`2 × BAR_LEAD + TRANSPORT_HIT`, so its mid-line *is* the transport's centre;
the volume block reserves an empty lane below its fader equal to the preview
lane above it, so centring the block centres the **rail** (and the `MUTE_TOP`
offset that used to buy the same alignment by hand is deleted); and the left
zone's stack is 20 · 16 · 20, so its middle lane is the block's exact centre.
The seek row hangs below the line rather than pushing it up. Measured spread
before: **50 px**. Ceiling: 2 px.

### L5 — The permitted alignment edges, per surface

> Each surface declares its alignment edges. An element that introduces an edge
> outside the list is a defect, and adding an edge means arguing for it in the
> list — the same rule the contrast exemption list already uses.

| surface | permitted x-edges |
|---|---|
| the wall | `HANG` and the hang's derived column edges; nothing else |
| the top bar | `HANG`, `W − HANG`, and the search well's right edge |
| the bottom bar | `HANG`, `W − HANG`, the zone boundaries, and the reserved slots' own edges |
| the album inspector | panel edge, panel + `GAP_XL`, panel width − `GAP_XL` — **one content lane**, less the declared `SCROLLBAR_LANE` |
| the queue popover | popover + `GAP_LG` and one indent lane for rows |
| the Settings place | `HANG`, nav right edge, content left edge, content right edge |

In both lists the extra edges came from one thing — a *row's own horizontal
padding*, applied inside a surface that had already stated its lane — so that is
what the test forbids. The full edge census is the render pass
(`composition/tools/census2.py`).

### L6 — Hierarchy is declared and then measured

> Each surface declares what a listener should notice first, second and third.
> The measured order — contrast-weighted ink mass over the named regions — must
> agree. Where it cannot (the wall, where one sleeve is ~135× its label, and
> deliberately so), the declaration says by how much.

| surface | declared order |
|---|---|
| the wall | the works ≫ their labels ≫ the playing mark ≫ the counts |
| the top bar | the counts → the well → `Settings` |
| the bottom bar | what is sounding → the transport → the position → what is next |
| the album inspector | the title → `Play album` → the track list → the sleeve → the condition |
| the Settings place | the section → its controls → their current values |
| first run | the question → the field → the hint |

The inspector was the inversion: its sleeve was *the panel minus its two
paddings*, 93.6 % of the panel's ink, and the album's own **name came fifth of
eight** at 1/164th of the weight of a picture already on the wall 24 px away.
`INSPECTOR_SLEEVE` is a cap now — 120, share 71.2 % — and the unit test holds the
one number the whole ranking is a function of. The ranking itself is measured by
`ink_mass` over a rendered frame, which is a slow test and belongs behind the
render harness's gate.

### L7 — One control height

> Every pointer target is `TRANSPORT_HIT` **32** tall. The only exceptions are
> `STEPPER_HIT` **24** and `NEEDLE_HIT` **12**, and both are named. A control
> that is none of the three is a defect, including a checkbox, a text well and
> the first-run input.

The product drew **five** heights — 40, 32, 30, 24, 13 — while publishing a
floor of 32, and asserted `TRANSPORT_HIT >= 32` and `STEPPER_HIT <
TRANSPORT_HIT` and nothing about the other three. The groove's own hit band
`RAIL_HIT` is 24, i.e. the named secondary square, so the one control in baz
that is a rail rather than a box is still one of the two heights and not a
third with a rail-shaped excuse.

**`NEEDLE_HIT` 12 is the third, and it is the one exception this law grants a
size it does not otherwise permit.** ADR-0017 §1.1's needle is 2 px flush on the
window's bottom edge and its whole bargain is that it costs the collection 2 px
rather than the 45 the seek row spent; reserving 24 or 32 of *layout* would undo
that, and claiming 24 of *aim* would reach into the transport row's boxes and
take presses meant for Next. So the needle reserves `NEEDLE_H` and claims its
band upward, out of layout, bounded by the empty lane the bar keeps under its
transport: `NEEDLE_HIT == GAP_MD` and `NEEDLE_HIT <= BAR_LEAD`, both asserted.
The exception is a *size with a proof attached* rather than a rail-shaped
excuse — the bound is what makes it safe, and the bound is the test.
