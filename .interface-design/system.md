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

Hex is the exact sRGB of the `f32` constants, rounded to the nearest byte.

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

`INDEX_W` **20 px**, a lane the shelf keeps clear on its right, outside the
scrollbar's. A–Z plus `#`, `SIZE_CAPTION` at 1.45. Letters the collection has an
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

| Token | px | line-height | weight | Used for |
|---|---|---|---|---|
| `SIZE_CAPTION` | 11 | 1.45 | Regular | tooltips, hover tips, footnotes |
| `SIZE_META` | 12 | 1.35 | Regular | label line 2, durations, counts, notes, control labels |
| `SIZE_BODY` | 13 | 1.40 | Regular / Medium | label line 1, track titles, button labels |
| `SIZE_EMPHASIS` | 15 | 1.35 | Regular / Medium | section headings, empty-state lines, inspector artist |
| `SIZE_TITLE` | 22 | 1.20 | SemiBold | the album's title |
| `SIZE_HERO` | 32 | 1.15 | SemiBold | the first-run question |

`LABEL_LINE_H` = `SIZE_BODY × 1.40` = 18.2; `LABEL_H` = 2 × that = **36.4**.
Two independent one-line lanes, not one two-line box, so a long title clips at
one line instead of pushing the artist line out of the slot reserved to keep it
still.

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
