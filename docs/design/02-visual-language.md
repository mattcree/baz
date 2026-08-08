# baz — Visual Language

> **Partly superseded by [ADR-0017](../adr/0017-design-direction.md).** The
> direction, the palette, the type system, the hang arithmetic, the accent law
> and the reserved slots all still govern, and Phase A of §11 has shipped. Five
> things no longer do:
>
> | No longer governs | Now |
> |---|---|
> | §2.7 — density placed in **Settings → Appearance** | **Superseded.** The three named steps survive as data; the control is a zoom gesture (`Ctrl+-` / `Ctrl+=` / `Ctrl+scroll`) with the step persisted as state. Settings is never the answer to a view question. ADR-0017 §1.3 |
> | §2.8 — the `#`+A–Z spine index at `INDEX_W` 20 | **Superseded** by the critique's index rail: a pure projection of the active group key (ARTIST → A–Z, YEAR → decades, GENRE → names, ADDED/PLAYED → recency buckets), `INDEX_W` 36. ADR-0017 §1.7 |
> | §6.5 — the bar's 102 px geometry, seek row and preview lane | **Superseded.** The needle takes the seek row's job; the bar drops to 58 px keeping Previous · Play/Pause · Next, the wall label, `3 / 12`, the signal note and volume. §6.5's *invariants* survive intact — nothing in the bar is sized to its content, slots exist whether or not they have anything to say, state changes touch ink not geometry, and the slots are a ratchet. ADR-0017 §1.1 |
> | §10 — *"a light variant: still defer"* | **Superseded.** The four-room model is adopted; the `Palette` indirection lands early (step 2) because every per-surface style below is written against it. Closing Time ships; Reading Room ships only with an answer to the pale-sleeve question that is not a border on artwork; Stone and Plaster are deferred. ADR-0017 §1.5 |
> | §11 — Phases A / B / C | **Superseded** by ADR-0017 §7's single sequence. Phase A is done; B and C are folded into it. |
>
> §5.2's contrast work is **upheld and extended**, not superseded: the WCAG
> floors stay and govern ink-on-surface, the critique's ≥ 0.03 oklch-L step law
> is adopted alongside them and governs surface-on-surface, the ink ramp is
> composited before it is measured, and the exemptions are named rather than
> waived. The argument is in ADR-0017 §1.6.

> The definitive specification for how baz looks. **Revision 2, 2026-08-08**,
> under the **gallery / archive** direction, superseding the "listening room"
> of revision 1. Written against `crates/baz/src/theme.rs`,
> `crates/baz/src/font.rs`, `crates/baz/src/art.rs`, the information
> architecture in `01-ux-audit-and-ia.md`, the vision's fifth pillar
> ("presentation that honours the artwork"), and the personas in
> `docs/research/05-personas.md`.
>
> **This document is a specification, not code.** Every number is either a
> token that already exists — kept deliberately, so the spec and the code share
> one vocabulary — or a change, marked **CHANGE** with the reason and the
> evidence. The condensed version future work reads first is
> `.interface-design/system.md`. The pictures are in `visual/gallery/`.

---

## 0. What changed, and why

The owner reviewed the shipped UI: *"our current design has some slightly off
stuff in terms of proportion, fonts (some weird monospace looking fonts which
are lame)"*, and chose **gallery / archive** — near-black, generous negative
space, album art treated as artwork with room to breathe, chrome almost absent
— over the shipped "listening room" and a hi-fi-faceplate option. He also chose
**no monospace at all**.

Revision 1's *method* stands and most of its *findings* stand. Its **direction**
does not. What survives, what is superseded, and what is new:

| Revision 1 said | Status | Why |
|---|---|---|
| The listening room: warm charcoal walls, amber lamp | **SUPERSEDED** (§1) | Direction change. The room becomes a neutral-cool near-black hang; the ink becomes archival mount board. |
| baz has no typeface; bundle IBM Plex | **SHIPPED, and now reduced** (§3) | Correct and done. Five faces become three: `MONO` and `SERIF` are deleted. |
| Every figure that changes in place is set in `MONO` | **SUPERSEDED and disproved** (§3) | Plex Sans's digits are tabular by default — *measured*. The mono was never needed. |
| The covers float; give them a contact shadow | **SUPERSEDED and disproved** (§4.3) | *Measured*: on near-black a black shadow is a 1.04 : 1 rounding error. All shadows are deleted; the halo is the only one left. |
| A fixed 240 px grid in a variable window is the proportion bug | **UPHELD, and re-solved** (§2) | Still the bug. Revision 1's `floor`-based fix produced too few columns at narrow widths and too-small art at wide ones; §2 replaces it. |
| `PAPER_FAINT` / `PAPER_MUTED` were below their contrast floors | **UPHELD; values re-derived** (§5.2) | Re-computed against the new surfaces. They land within two bytes of revision 1's — and the one pairing revision 1 had to excuse as a rounding case now passes outright. |
| One accent, reserved for playback truth | **UPHELD** (§5.3) | Unchanged in meaning. One use is tightened: Play album stops being a solid amber slab. |
| The lamp takes its hue from the playing sleeve | **UPHELD and promoted** (§1.2) | Still the signature, now half of a larger one. |
| The accent cuts: focus ring, scanning note | **SHIPPED** | Already in `theme.rs`. |
| `SIZE_TITLE` 22, `SIZE_HERO` 32 | **UPHELD** (§3.2) | The reasons still hold; no reason to churn the numbers again. |
| The serif, in exactly two places | **SUPERSEDED** (§3.3) | Revision 1 nominated it as the first thing to cut if the design needed disciplining. This is that moment. |
| `THUMB_PX` stays 256; revisit later | **SUPERSEDED** (§4.2) | The new grid draws art up to 320 px, so 256 would upscale. 320, with the cache arithmetic re-done. |
| The now-playing bar is the best thing in the product | **UPHELD** (§6.5) | Its geometry is untouched. Four reserved slots shrink to their measured worst case. |
| Motion is 0 ms | **UPHELD** (§7) | |

### 0.1 And what the prior-art study changed

`03-interface-prior-art.md` landed while this revision was being drawn — 16+
products, three peers installed and rendered. It contradicts one thing in this
document and adds two.

| Finding | Effect here |
|---|---|
| **R7 — there is no density control.** Making the cell a function of *viewport* width fixes dead gutters but is still a designer's constant from the user's side. Density control is universal outside music; Steam and Google Photos both took durable damage for removing one. | **§2.7 is new.** Three named steps parameterise all four hang numbers. The viewport-derived width is kept and *parameterised*, not replaced. |
| **R8 — no shelf index.** Losing jump-to-letter was Sonos users' loudest concrete complaint. Invisible at 29 albums, fatal at Marta's 40 000 — and a gallery direction makes scrolling *longer*. | **§2.8 is new.** A 20 px spine index, type not chrome, so §1.3's claim survives. |
| **R6 — the inspector shows four lines where fooyin shows twenty.** A regression for the cataloguer personas. | **§6.2 gains `Details`**: the condition report in full, no disclosure, below the fold. |
| **R9 — keep a strip of shelf below 940 px.** | **§6.9 is new** (the visual half; the structural decision is `01` §4.3's). |
| **No evidence for "Plexamp is the UX bar."** Its own axiom is player-first with the library one level down — the inverse of baz's bet. | Revision 1 leaned on that premise; §1.3 replaces it with the measured argument. |
| **Content share at rest: the tradition gives the collection 0–26 % of the window; baz gives it 73–100 %.** | §1.3. This is the strongest available answer to "sameness is failure", and it is about **proportion**, which is exactly what this revision is for. |

---

## 1. Direction

**A record archive after closing time. The works are lit; the room is not.**

baz is a hang, not a dashboard. The wall is near-black and *neutral-cool* — the
matte paint of a black-cube gallery. Not the warm charcoal of a listening room,
which was the previous direction, and not the blue-grey of a stock dark theme.
The type is warm ivory, the colour of archival mount board. **The room is cold
and the paper is warm**, which is what a gallery actually looks like at night,
and it is the single decision that keeps a near-black grid from reading as
every other media app.

There is one light in the room and it is pointed at one thing: the record that
is playing. Everything else — every control, count, setting, state — is made of
surface, edge and ink. The room is quiet so the records are loud.

**Who it is for.** Marta, whose 40 000 tracks should look like a collection
rather than a spreadsheet; Devon, who plays albums front to back and wants the
sleeve at a size worth looking at; Karl, who wants to be told the truth about
the signal path in the quietest possible voice. Nobody here is being sold
anything, discovered to, or recommended at.

### 1.1 The domain this came out of

Not "dark music app". The world of a print archive and its reading room:

- **the hang** — the arrangement of works on a wall; the distance between them
  is a curatorial decision, never leftover space;
- **the wall label** — the small card beside a work: title, maker, date,
  medium. Small, quiet, always in the same place, never *on* the work;
- **the picture light** — the one warm lamp aimed at a piece; the only light in
  the room that is *pointed* at something;
- **the reading room** — where you are given one item at a time, at a table,
  and everything else stays in the stacks;
- **the condition report** — the archivist's honest note about what the object
  actually is (`FLAC · 16-bit · 44.1 kHz`, `bit-perfect`). Never a sales pitch;
- **the shelfmark** — figures, quietly set, in a fixed column, proving the
  thing is catalogued and findable.

**The colour world**, walked rather than named: matte hanging-wall black; the
shadow gap where the wall meets the floor; **archival mount board**, a warm
ivory that is a *material* rather than "white text"; **graphite**, the pencil
the accession number is written in; the picture light's narrow tungsten pool;
and the sleeves, which supply every other colour in the room.

### 1.2 The signature — the picture light and its label

Two halves of one component.

**The light.** One accent, reserved for playback truth, whose **hue is read
from the playing sleeve** — lightness and chroma fixed at the coordinates of
amber, so a white sleeve cannot produce a white lamp and a fluorescent one
cannot produce a fluorescent lamp. It is always recognisably the same lamp with
a different record in front of it. Amber is the default and the fallback. Only
the hue is data. (Revision 1's §3.3, carried forward unchanged; the extraction
method is §4.4 here.)

**The label.** Every album in baz is captioned by a **wall label**: a
fixed-height, left-aligned, two-line block set beneath the work and never on
it. The same object at four scales. When the work is playing, the label's first
line carries the lamp dot and the work carries the halo.

Five concrete places the signature appears, each locatable in a screenshot:

1. **the shelf tile** — label under every sleeve; playing gains dot + halo
   (`visual/gallery/01-shelf-1280.png`, column 2);
2. **the album inspector header** — the same label, larger, plus the catalogue
   line and the condition report (`03-album-inspector.png`);
3. **the now-playing bar's left zone** — the same label at bar scale, plus the
   `3 / 12` position slot. *The bar is a wall label for what is sounding*
   (`04-now-playing-bar.png`);
4. **Up-next popover rows** — one-line labels; the playing row's dot replaces
   its number in a column that never changes width;
5. **the seek groove** — the light laid flat: elapsed fill and knob take the
   lamp hue, and the elapsed stamp warms while a seek is in flight.

Plus **Play album**, the only control in baz outlined in the accent — the
switch that turns the picture light on.

### 1.3 Why this is not the near-black grid every media app converges on

Gallery/archive is the least inherently distinctive of the three directions
offered: a dark grid of covers is what everyone builds. The palette does not
make it specific; **what is in it** does. Three defaults rejected, and what
replaces each:

| The default | What baz does instead |
|---|---|
| A uniform card grid with a fixed cell and a play button appearing over the art on hover | **No cell.** The art absorbs all spare width (§2). Nothing is ever drawn on top of a sleeve. Hover is a rule under the *label*. |
| Chrome washed in a colour sampled from the current cover | One hue, from one cover, for one 6 px dot / 4 px rail / halo. **If the wall ever changes colour, this feature has been implemented wrongly.** |
| A monospaced "data" face for the technical bits — the thing the owner actually complained about | Proportional everywhere, with alignment *measured* rather than substituted for (§3). |

**And the measured argument, which is better than any of the above.**
`03-interface-prior-art.md` §2.3 rendered the peers and measured the fraction of
the window each gives the user's own collection at rest:

| Product | Collection's share of the window |
|---|---|
| fooyin "Vision" | **0 %** — the library is a collapsed vertical tab |
| Strawberry | 17 % |
| fooyin "Simple" | 19 % |
| fooyin "Obsidian" | 26 % |
| Lollypop | 87 % |
| **baz** | **73 %** with the inspector open, **100 %** without |

Only three of sixteen surveyed players open on the user's covers at all, and two
of those are server clients. **The album-shelf home is unoccupied territory.**
That is where "a near-black grid is what every media app converges on" is
answered: not by the palette, which cannot win that argument, but by
*proportion* — which is what this entire revision is about, and why §2 is the
longest section in the document.

One correction to a premise revision 1 carried: the study found **no evidence
for "Plexamp is the UX bar."** Plexamp's own stated axiom is that *the player is
the primary interface element and sits on top of everything else* — player-first
with a library attached, with its cover wall one level down. That is the inverse
of baz's bet. Plexamp is worth citing for its transport's polish and its hover
preview, and for nothing else.

And one structural claim that no other player makes:

> **The shelf contains exactly two kinds of thing: artwork and type.**

No cards, no borders, no radii, no shadows, no badges, no overlays, no
separators, no scrim. Squint at `02-shelf-1920.png`: you read a rhythm of small
bright label blocks in regular columns, with images floating above them, and
exactly one warm glow. **The grid's structure is carried by the labels, not by
the sleeves' edges** — which is what lets a near-black cover merge into a
near-black wall without the wall falling apart. That merge is visible in
`02-shelf-1920.png` (*In Rainbows*, row 1) and it is accepted, deliberately:
it is what a hang looks like at night, and the covers that vanish are the ones
whose designers chose black.

---

## 2. Proportion — the owner's first complaint, and a real bug

### 2.1 The bug

`shelf.rs` lays out a **fixed 240 px cell** (`CELL_W`), containing a **fixed
208 px sleeve** (`ART_PX`), in a block centred inside a variable window with
`GRID_PADDING` 24. The art therefore never grows, and everything the window
does not divide evenly pools as dead gutter at the edges:

| Window | Columns | Art | Block | Dead gutter |
|---|---|---|---|---|
| 1280, no inspector | 5 | 208 | 1200 | 32 px |
| **922** (1280 with the inspector open) | 3 | 208 | 720 | **154 px** — 77 px of nothing at each edge |
| 1920, no inspector | 7 | 208 | 1680 | 192 px |
| 2560 | 10 | 208 | 2400 | 112 px |

Under a gallery direction — fewer, larger works with air between them — this is
exactly backwards: it produces *more* covers, each *smaller*, with the air
swept into a pile at the window's edge.

### 2.2 The hang

**One number drives the grid.** `HANG` = 40 px is the distance from a work to
its neighbour **and** from a work to the edge of the wall. The art absorbs all
remaining width up to `ART_MAX`; only past that do the margins absorb it.

```
HANG        = 40            # the grid's one spacing token
ART_MIN     = 240           # a sleeve is never drawn smaller
ART_MAX     = 320           # ≤ art::THUMB_PX — and nothing ever upscales
ART_TARGET  = 272           # the size the wall wants
                            # all four are functions of the density step — §2.7

grid_w(W)   = W - inspector_w(W) - SCROLLBAR_LANE - INDEX_W

columns(w)  = clamp( floor((w + HANG) / (ART_TARGET + HANG) + 0.5),
                     1,
                     max(1, floor((w - HANG) / (ART_MIN + HANG))) )

art(w)      = min(ART_MAX, (w - (columns + 1) * HANG) / columns)

gutter(w)   = columns > 1 ? (w - 2*HANG - columns*art) / (columns - 1) : 0
              # clamped to 2*HANG; any surplus goes to the margins, block centred

row_h(w)    = art(w) + GAP_LG + LABEL_H + HANG          # = art(w) + 92.4
```

**The grid width is not the window width.** The shelf keeps two lanes clear on
its right — the scrollbar's `SCROLLBAR_LANE` 10 (already reserved whether or not
the list scrolls) and the spine index's `INDEX_W` 20 (§2.8). At a 1280 px window
with no inspector the hang lays out in **1250 px**, not 1280. The table in §2.3
is keyed on grid width for that reason.

**`floor(x + 0.5)`, never a language's `round`.** Rust's `f32::round` is
half-away-from-zero; Python's is banker's. A grid whose column count depends on
which language expressed it is not a specification, and 5.5 columns is a case
this arithmetic actually reaches.

Two properties worth naming, because they are what makes this a system rather
than a formula:

1. **Whenever the art is not capped, `gutter == HANG` exactly.** The
   arithmetic puts every spare pixel into the artwork, so **dead gutter is
   0 px at every width**. That is the whole of the proportion fix, in one
   number.
2. **`ART_MAX = 4/3 × ART_MIN`, deliberately.** At every column-count change
   the art hands off from its largest to its smallest — 320 → 240 — at exactly
   one width per transition, with no width at which the grid is ambiguous.

**Rounding, not flooring**, is the second half of the fix. Revision 1 chose the
column count by `floor` against a minimum, which pins the art near its floor
and gets *worse* on bigger screens (7 columns at 208 px on a 1920 window).
Rounding against a *target* keeps the art in a narrow band around 272 at every
width — 240 to 320 across a 640 → 2560 range.

### 2.3 Worked, so the change is checkable

At the `Balanced` step, keyed on **grid width** (the window, less the inspector,
less the two lanes):

| Window | Grid width | Columns | Art | Gutter | Margin | Row pitch | Today | Today's dead gutter |
|---|---|---|---|---|---|---|---|---|
| 670 | 640 | 2 | 260 | 40 | 40 | 352.4 | 2 × 208 | 112 px |
| **1280** + inspector 358 | **892** | 3 | **244** | 40 | 40 | 336.4 | 3 × 208 | **154 px** |
| 1120 | 1090 | 3 | 310 | 40 | 40 | 402.4 | 4 × 208 | 112 px |
| **1280** (no inspector) | **1250** | **4** | **262.5** | 40 | 40 | 354.9 | 5 × 208 | 32 px |
| 1920 + inspector 420 | 1470 | 5 | 246 | 40 | 40 | 338.4 | 6 × 208 | 12 px |
| **1920** (no inspector) | **1890** | **6** | **268.3** | 40 | 40 | 360.7 | 7 × 208 | 192 px |
| 2560 | 2530 | 8 | 271.2 | 40 | 40 | 363.6 | 10 × 208 | 112 px |

Drawn at 1280 in `visual/gallery/01-shelf-1280.png` and at 1920 in
`02-shelf-1920.png`, with the computed figures printed into each picture.

**The one discontinuity, stated rather than hidden.** Between 1120 and 1160 px
the grid goes from three 320 px sleeves to four 240 px ones. Every column grid
jumps somewhere; these numbers make the jump exactly the 4/3 ratio the tokens
were chosen for, and put it at one width rather than smeared across a band.

### 2.4 What "generous negative space" means, in numbers

Honestly, and it is not what it sounds like. Art coverage per cell is
essentially unchanged — 64.9 % today (208² in 240 × 284), 64.2 % at a 1280
window under the new grid (262.5² in 302.5 × 354.9). **The generosity is not
proportional, it is distributional:**

- **Dead gutter goes from 32–192 px to 0 px at every width.** Every spare pixel
  is between two works instead of pooled at the window's edge.
- **The works get larger, everywhere.** At a 1280 window: 208 → 262.5 px,
  **+26 % on the edge and +59 % in area**. With the inspector open: 208 → 244,
  +17 %. At no width in the table does a sleeve get smaller than it is today.
- **Fewer of them.** 5 → 4 columns at 1280, 7 → 6 at 1920. This is the
  direction's request taken literally.
- **The vertical rhythm becomes the horizontal one.** The gap between rows is
  `HANG`, the same 40 px as the gap between columns, so the wall reads as a
  field rather than as stacked strips. Today the row gap is implicit in
  `CELL_H` and does not match anything.

### 2.5 The caption block, and what it costs

The label is **two independent one-line lanes**, not one two-line box —
revision 1's `CAPTION_LINE_H` reasoning, kept verbatim, because it is right:
`Wrapping::None` does not stop iced 0.13 breaking a long paragraph, so inside a
single box a long title would push the artist line out of the very slot
reserved to keep it still.

```
LABEL_LINE_H = SIZE_BODY × 1.40 = 18.2
LABEL_H      = 2 × LABEL_LINE_H = 36.4
```

**CHANGE** from `SIZE_BODY × LINE_HEIGHT` (16.9 / 33.8): the line height is now
set per type token (§3.2) rather than taking iced's 1.3 default everywhere.

Gap from work to label: `GAP_LG` 16 (**CHANGE**, was `GAP_MD` 12) — a label
hangs clear of its work.

**The year is dropped from the shelf label** (upheld from revision 1). At rest
the wall answers *what do I own*; `which pressing` is the inspector's catalogue
line. Two facts per label is a wall of records; three is a table.

### 2.6 `THUMB_PX`, and what it costs the cache

`ART_MAX` is 320, so `art::THUMB_PX` must become **320** (**CHANGE**, was 256)
or every sleeve above 256 px would upscale and soften — which the new grid
produces at almost every width.

**Invariant, and it should be asserted in code: `ART_MAX == THUMB_PX`. No
artwork in baz is ever drawn larger than its source.**

Recomputed from `art.rs`'s own derivation, at the same 150 MiB budget:

| `THUMB_PX` | Worst-case entry | Entries at 150 MiB |
|---|---|---|
| 256 | 256 × 256 × 4 = 262 144 B = 256 KiB | **600** |
| **320** | 320 × 320 × 4 = 409 600 B = 400 KiB | **384** |

**−36.0 % capacity, +56 % bytes per entry.** (Revision 1 quoted "+37 %,
600 → 375" for this change; 375 was wrong — 150 MiB ÷ 400 KiB is exactly 384.)

**The budget stays at 150 MiB.** 384 entries is roughly **8× the live widget
count**: at 1920 the shelf shows 6 columns over `ceil(922 / 360.7) + 1` = 4
visible rows plus 2 × `OVERSCAN_ROWS`, i.e. ~48 tiles. The LRU exists to make a
fling meet decoded art, and 384 entries covers 64 rows — about 23 000 px of
scroll — before it recycles.

**What this does not fix, stated plainly.** The cache is DPI-blind. On a 2×
display a 320 logical px tile still wants 640 device px and gets 320. 320 is
chosen because it is the largest *logical* size the layout can produce, so at
1× nothing upscales; the honest fix is a DPI-aware cache and it is not this
document's.

### 2.7 Density is a user control, not a designer's constant

**This section exists because `03-interface-prior-art.md` R7 contradicted an
earlier draft of §2.2**, and the contradiction was correct. Making the cell a
function of viewport width fixes the dead gutter but leaves the user with a
constant they cannot touch. The study's evidence:

- Density control is **universal outside music** — Lightroom (thumbnail slider,
  `-`/`+`, `J` to cycle cell modes), Calibre (cover size *and* grid
  background), Steam, Plex (per-view poster slider), Feishin (`itemSize`,
  `itemGap`, `itemsPerRow`), Google Photos, Apple Photos.
- **Nobody in music does it**, which is an opportunity rather than a precedent.
- **Two products that removed a density level took durable damage**: Steam's
  grid-size slider (users demanded *"a slider to adjust icon size in GRID
  MODE"*; Valve later restored a small library view) and Google Photos' year
  view.

Under a gallery direction this matters **more, not less**. A direction whose
whole point is *fewer, larger* covers is a direction that has made a strong
choice on the user's behalf, and Marta's 40 000 albums and Devon's 300 are not
served by the same wall.

**Three named steps. Not a free zoom.** A slider makes every screenshot of baz
different, every layout report unreproducible, and every reserved-slot argument
in this document conditional. Steps also mean the grid's properties hold
everywhere rather than at one setting.

| Step | `HANG` | `ART_MIN` | `ART_TARGET` | `ART_MAX` | at 1280 | at 1920 |
|---|---|---|---|---|---|---|
| **Spacious** | 48 | 288 | 320 | 320 | 3 × 320 | 5 × 320 |
| **Balanced** (default) | 40 | 240 | 272 | 320 | 4 × 262 | 6 × 268 |
| **Dense** | 28 | 176 | 200 | 240 | 5 × 216 | 8 × 205 |

Drawn at one window width in `visual/gallery/06-density.png`.

Four things this design gets right that a naive size slider would not:

1. **The step parameterises the hang; it does not override it.** §2.2's
   properties — `gutter == HANG` wherever the art is uncapped, `art` always
   within `[ART_MIN, ART_MAX]`, dead gutter zero — hold at all three steps.
2. **`ART_MAX ≤ THUMB_PX` at every step**, so §2.6's *nothing upscales*
   invariant is a property of the system rather than of the default.
3. **`Dense` is roughly what baz ships today** (5 × 216 at 1280 against today's
   5 × 208). Nobody who likes the current shelf loses it; they gain a name for
   it.
4. **`Spacious` pins the art at `ART_MAX` and lets the margins take the
   slack** — which is exactly the `gutter ≤ 2 × HANG` rule from §2.2 finally
   earning its keep.

**The cost, stated.** At `Dense` the cache holds 320² thumbnails for ~200 px
tiles — 2.5× the pixels needed. A density-aware decode size is the obvious
optimisation and is deliberately not taken here: it would make the LRU's
contents depend on a setting, which means invalidating the whole cache when the
setting changes, which is a bigger decision than this document should make.
Live-widget headroom at the worst case (`Dense`, 2560 px, ~110 live tiles
against 384 entries) is 3.5×, which is thin but sufficient.

Placement: **Settings → Appearance**, which `01-ux-audit-and-ia.md` §4.5 already
reserves. The steps are named **plainly rather than in the room's vocabulary**,
deliberately and consistently with §3.3: a setting is where the software talks
about *itself*, and everything baz says about itself is plain.

### 2.8 The spine index

**`03-interface-prior-art.md` R8.** The single most concrete regression Sonos
users named was losing alphabetical jump — *"if you want something beginning
with a 'T', you have to scroll through hundreds of screens"*. baz's fixture is
29 albums and Marta's library is 40 000, so this gap is invisible in every
screenshot anyone has taken of this product.

**And a gallery direction makes it worse.** Fewer, larger covers with more air
between them means a longer scroll for the same collection: at `Balanced` a
40 000-album library is ~10 000 rows and roughly 3.5 million pixels of scroll.
Air is a luxury the collection pays for in distance, and an index is what buys
it back.

| Part | Spec |
|---|---|
| lane | `INDEX_W` **20 px**, on the shelf's right, outside `SCROLLBAR_LANE`; taken off the grid width before `columns()` (§2.2) |
| keys | `#` then `A`–`Z`, `SIZE_CAPTION` 11 at 1.45, centred in the lane |
| present in the collection | `PAPER_FAINT` |
| absent from the collection | `PAPER_MUTED` — **drawn, not hidden**: an index that hides its gaps lies about the collection |
| current (pointer, or the scroll position's initial) | `PAPER` Medium |
| accent | **never** — an index is navigation, not playback truth |
| when 27 keys do not fit | the run subsamples, the pattern every phone contact list uses |

**It is type, not chrome**, so §1.3's claim survives intact: the shelf still
contains exactly two kinds of thing, artwork and type. It is also the archive's
own device — the run of letters down the edge of a card-catalogue drawer — which
is why it belongs to this direction rather than being bolted onto it.

Type-to-jump is **`/`-scoped, never type-anywhere**: the audit (§4.8) already
resolved that bare letters belong to the transport, and that resolution stands.

---

## 3. Type — proportional everywhere

### 3.1 The monospace is deleted, and it was never needed

Revision 1's rule was: *iced 0.13 exposes no OpenType feature control, so there
is no `tnum`; therefore every figure that changes in place is set in `MONO`.*
The premise is true. The conclusion does not follow, and the difference is
worth measuring rather than assuming.

**Many grotesques ship tabular figures by default.** IBM Plex Sans is one.

Measured two ways — first by reading `hmtx` directly (the same tables the
hand-written TrueType reader in `crates/baz/src/font.rs` parses), then by
shaping through HarfBuzz with default features on (`calt`, `liga`, `kern`
applied), which is what cosmic-text does behind `iced::widget::text`:

| Face | Digit advances `0`–`9` | Distinct widths | Tabular by default |
|---|---|---|---|
| IBM Plex Sans **Regular** | 600 / 1000 em, every digit | 1 | **yes** |
| IBM Plex Sans **Medium** | 600 / 1000 em, every digit | 1 | **yes** |
| IBM Plex Sans **SemiBold** | 600 / 1000 em, every digit | 1 | **yes** |
| IBM Plex Serif SemiBold | 600 / 1000 em, every digit | 1 | yes |
| IBM Plex **Mono** Regular | 600 / 1000 em (every glyph) | 1 | yes |

The Sans's digit advance is **identical to the Mono's**, in all three weights.
There is no kerning between digits and no default-on substitution that touches
them. Shaped end to end:

| String | String | Sans Regular | Δ |
|---|---|---|---|
| `0:00:00` | `9:59:59` | 43.008 px each at `SIZE_META` | **0.000 px** |
| `1:23:45` | `8:07:02` | 43.008 px each | 0.000 px |
| `999` | `111` | 21.600 px each | 0.000 px |
| `-18.1 dB` | `-60.0 dB` | 47.280 px each | 0.000 px |
| `12 / 32 albums` | `11 / 11 albums` | 81.660 px each | 0.000 px |

**So `MONO` is deleted.** The alignment it was standing in for is a property of
the Sans. Nothing in baz needs a second face to keep a column straight. The
specimen is `visual/gallery/05-figures-specimen.png`.

Three consequences beyond taste:

1. **A latent clip is fixed.** `STAMP_W` is 52 px. `10:00:00` measures 57.60 px
   in Plex Mono — the shipped build **cannot** hold a ten-hour track — and
   50.21 px in Plex Sans, which it can.
2. **Every reserved slot gets narrower**, because Sans is proportional
   everywhere the string is not a digit (§3.4).
3. **`theme.rs`'s `MONO_EM` const survives as `DIGIT_EM`**, still 0.6, and is
   now a property of the face baz actually sets its figures in rather than of a
   second face it also had to ship.

### 3.2 The scale

Sizes are `theme.rs`'s. Line heights are `LineHeight::Relative`, which iced
0.13's `text` accepts; baz currently takes the toolkit default of 1.3
everywhere.

| Token | px | line-height | weight | Used for |
|---|---|---|---|---|
| `SIZE_CAPTION` | 11 | 1.45 | Regular | tooltips, hover tips, footnotes |
| `SIZE_META` | 12 | 1.35 | Regular | label line 2, durations, counts, notes, control labels |
| `SIZE_BODY` | 13 | 1.40 | Regular / Medium | label line 1, track titles, button labels |
| `SIZE_EMPHASIS` | 15 | 1.35 | Regular / Medium | section headings, empty-state lines, inspector artist |
| `SIZE_TITLE` | **22** (revision 1's change, upheld) | 1.20 | SemiBold | the album's title |
| `SIZE_HERO` | **32** (revision 1's change, upheld) | 1.15 | SemiBold | the first-run question |

**Emphasis comes from weight, ink and size only.** iced 0.13 exposes no
letter-spacing, no small caps and no OpenType features, so nothing in this
system may be specified in terms of them. A wall label would normally be
letterspaced; baz's cannot be, and pretending otherwise in a mockup would be
specifying something the toolkit cannot draw.

### 3.3 The serif is deleted too

Revision 1 gave Plex Serif SemiBold exactly two jobs — the album title and the
first-run question — and said: *"this is the one deliberate accessory in the
design, and if one thing has to be cut to keep the design disciplined, it is
this."*

This is that moment, for a reason the new direction supplies. The gallery's
whole thesis is that **the room supplies nothing and the work supplies
everything**. A display face is the room supplying personality. The album title
becomes Plex Sans SemiBold at 22.

**The bundle, restated.** Three faces instead of five:

| Face | Bytes |
|---|---|
| `IBMPlexSans-Regular.ttf` | 200 500 |
| `IBMPlexSans-Medium.ttf` | 202 460 |
| `IBMPlexSans-SemiBold.ttf` | 202 632 |
| **Total** | **605 592** |
| *Removed:* `IBMPlexMono-Regular.ttf` | −173 052 |
| *Removed:* `IBMPlexSerif-SemiBold.ttf` | −222 876 |
| **Saved** | **395 928 bytes (39.5 %)** |

Licensing is unchanged and already cleared: OFL-1.1, verbatim upstream, hashes
and provenance in `crates/baz/assets/fonts/README.md`, `OFL.txt` committed.
Nothing here is a *new* face, so nothing here needs a new licence review —
which is the argument for solving the figure problem inside the family baz
already ships rather than auditioning a replacement.

**If Plex Sans had lacked tabular digits**, the fallback would have been stated
here rather than wished away: right-aligned duration and timestamp columns
(ragged-left reads fine editorially — it is how a bibliography sets page
numbers), fixed-width reserved slots for every ticking figure, and an explicit
list of where a width jiggle is tolerable. Two of those three are specified
anyway (§3.4, §3.5), because they are good practice independent of the face.

### 3.4 Reserved slots, re-derived from the real advances

Revision 1 found `theme.rs`'s slot assertions had guessed 0.5 em against an
actual 0.6 em. The same discipline applies here: every slot is re-measured in
the face that will draw it, at the size it uses.

| Token | Today | Sans px | Mono px | **New** | Worst case | Slack |
|---|---|---|---|---|---|---|
| `STAMP_W` | 52 | 50.21 | 57.60 | **52** | `10:00:00` | 1.79 |
| `SIGNAL_W` | 120 | 92.38 | 108.00 | **96** | `192 → 176.4 kHz` | 3.62 |
| `LEVEL_W` | 62 | 43.34 | 52.80 | **48** | `-18.1 dB` | 4.66 |
| `PREVIEW_W` | 58 | 39.42 + 2 × `GAP_XS` | 46.20 | **48** | `0:00:00` + padding | 0.58 |
| `SETTING_VALUE_W` | 68 | 56.89 | 64.80 | **60** | `+20.00 dB` | 3.11 |
| `TRACK_NO_W` | 24 | 21.60 | 21.60 | **24** | `999` | 2.40 |
| `POSITION_W` | — | 53.46 | 64.80 | **56** | `199 / 240` | 2.54 |

`POSITION_W` is new: the `3 / 12` readout the IA adds to the bar's left zone
(`01-ux-audit-and-ia.md` §3.4). It is sized for a three-figure queue; a
four-figure one (`9999 / 9999`, 67.86 px) would clip, which is a deliberate
bound — no album has 1000 tracks and a whole-library shuffle queue is a
different surface's problem.

**The test in `font.rs` does not change shape, only its inputs.**
`every_reserved_slot_holds_its_worst_case_in_the_bundled_face` already parses
these very bytes and measures these very strings; it switches from `mono()` to
`sans()`, gains `10:00:00` and `199 / 240`, and keeps its 1 px `SLACK`. Do not
ship the face reduction without it.

### 3.5 The one place a proportional face can still jiggle

Measured, at `SIZE_META`:

| Glyph | Advance |
|---|---|
| `-` hyphen-minus U+002D | 0.399 em (4.79 px) |
| `+` plus U+002B | 0.600 em (7.20 px) |
| `−` minus U+2212 | 0.600 em (7.20 px) |

So `-20.00 dB` measures **54.48 px** and `+20.00 dB` **56.89 px** — a 2.4 px
shift in a right-aligned slot's *left* edge as the ReplayGain pre-amp steps
through zero.

- **The fix.** `replaygain::format_centidb` emits **U+2212** for negatives.
  `−20.00 dB` and `+20.00 dB` then measure **56.89 px each**, exactly.
  (`theme.rs` already specifies U+2212 for the stepper's own `−` glyph, "matches
  `+` in width and height" — this makes the *formatter* agree with the
  control.) Its unit tests assert the ASCII form and must be updated together.
- **The residual, accepted.** Unsigned `0.00 dB` is 7.2 px narrower than a
  signed value. One value, at one point in the travel, changing only when a
  human presses a stepper, in a slot whose right edge is pinned. Padding a
  phantom sign column to hide it would be worse than the thing it hides.
- **Never acceptable, anywhere:** a figure that ticks with playback. Elapsed,
  remaining, seek preview, level tip, queue position, track duration. All are
  fixed-digit-count strings and Plex Sans makes them exact to 0.000 px.
- **The standing rule.** **Figure columns are right-aligned**, in fixed slots.
  Ragged-left reads fine editorially, and the pinned edge is the one the eye
  follows down a column.

---

## 4. Album art

Art is the product. Everything else exists to get out of its way.

### 4.1 Sizes

| Surface | Edge (logical px) | Source |
|---|---|---|
| shelf tile | **240 – 320, computed per shelf width** (§2.2) | 320² LRU thumbnail |
| album inspector | `min(column − 2 × GAP_XL, 320)`, **left-aligned** in the column | same |
| now-playing bar | **none** | — |

**The inspector's sleeve is left-aligned, not centred**, flush with the label
beneath it and with the track list below that — a flush-left hang. It is capped
at `ART_MAX` so the *no artwork upscales* invariant holds on every surface, not
just the shelf.

**The bar carries no artwork.** Revision 1 argued this from pixel budget; the
better argument is the direction's: **in a gallery the label does not reproduce
the work.** What is playing is said in words in the bar and shown, haloed, in
the shelf.

**Nothing is ever drawn on top of a sleeve.** No play overlay on hover, no
badge, no duration chip, no gradient scrim, no selection tint. Even the playing
mark sits beside the label, off the art. The only thing that touches artwork is
light around it.

### 4.2 Depth, and why the contact shadow is deleted

Revision 1 diagnosed the sleeves as "floating" and prescribed a tighter, lower,
darker contact shadow so a sleeve would stand on the shelf rather than hover
above it. Under a near-black wall that prescription does not work, and the
measurement says so:

| Shadow | Composited over `WALL` `#0C0D0E` | Contrast |
|---|---|---|
| black @ 45 % | `#070708` | **1.04 : 1** |
| black @ 55 % | `#050606` | 1.04 : 1 |
| black @ 65 % | `#040505` | 1.05 : 1 |

**On near-black, a drop shadow is not a design tool; it is a rounding error.**
There is no luminance left to remove. Making it darker, larger or softer moves
the number in the third decimal place.

So: **`SHADOW` is deleted, and nothing in the shelf has any shadow at all.**
The single shadow primitive left in the product is the playing halo, and it is
not elevation — it is light.

**Depth strategy, committed to: surface-step elevation.** Four planes, whisper
quiet in bytes (8 apart) and plainly felt in linear light:

| Surface | Hex | Linear L | Step | Role |
|---|---|---|---|---|
| `RECESS` | `#060708` | 0.00208 | — | the shadow gap: bar, input wells, groove troughs |
| `WALL` | `#0C0D0E` | 0.00398 | ×1.91 | the hanging wall |
| `PLINTH` ⟵ was `CARD` | `#141517` | 0.00747 | ×1.88 | inspector column, popover, resting control |
| `PLINTH_LIT` ⟵ was `CARD_HIGH` | `#1C1D20` | 0.01230 | ×1.65 | selected segment, playing row, hovered control |

**Two tokens are renamed** (**CHANGE**): `CARD` → `PLINTH`, `CARD_HIGH` →
`PLINTH_LIT`. "Card" is web-app vocabulary and, under this direction, a lie —
there are no cards. A plinth is the thing a work stands on. Every other token
name (`WALL`, `RECESS`, `PAPER*`, `LAMP*`, `HAIRLINE*`) already belongs to a
place rather than to a scale and is kept, so this is a two-line rename rather
than a vocabulary rewrite that would collide with the IA work in flight.

**Hairlines** appear in exactly three roles — the rule under the top bar, the
rule above the now-playing bar, and the rule dividing the inspector from the
shelf — plus the tile's own hover/selection rule (§6.1) and control borders.
`HAIRLINE` = `PAPER` @ **7 %** and `HAIRLINE_STRONG` = `PAPER` @ **15 %**
(**CHANGE**, was 8 % / 17 %): the same alpha over a darker ground is a larger
step, so holding the *perceived* weight means lowering the number. iced's
`Border` is four-sided, so every single line is a `rule` widget — already how
the bars are built.

### 4.3 When art is missing

Keep the deterministic gradient (`vm::gradient_colors`, hash → HSL), with
revision 1's two changes, one of them re-tuned for the darker wall:

- **Quieten it.** Today it samples S ∈ [0.35, 0.70], L ∈ [0.22, 0.50], which
  out-shouts real covers. **CHANGE** to **S ∈ [0.10, 0.28], L ∈ [0.16, 0.30]**
  — revision 1's range with the floor lifted from 0.14, because against
  `#0C0D0E` a placeholder at L 0.14 barely clears the wall and a missing sleeve
  should read as absence, not as a hole.
- **Give it a letterform.** The album title's first character at **0.28 × the
  art edge**, `PAPER` at 12 %, optically centred — now in **Sans SemiBold**,
  the serif having been deleted. One `text` widget.

Deterministic per album id, so the same missing album is the same colour every
launch; that consistency is what lets Marta recognise a hole in her collection
by sight.

### 4.4 Colour from art: yes, in exactly one place

Unchanged from revision 1 and restated because it is the signature.

- **Source.** The already-decoded ≤ 320² RGBA thumbnail in the LRU. No new
  decode, no new I/O, no new dependency.
- **Method.** A 4 × 4 × 4 RGB histogram (64 bins) over every fourth pixel
  (~6 400 samples at 320²). Convert bin centroids to a perceptual space;
  discard bins below 0.04 chroma, below 0.25 lightness, or above 0.85
  lightness. Take the most populous survivor; if none survives, amber.
- **The constraint that makes it a design.** **Only the hue survives.**
  Lightness is forced to 0.72 and chroma to 0.13 — the coordinates of `LAMP`.
- **Where it lands.** The halo, the playing dot, the seek fill and knob, and
  Play album's border and triangle. Nothing else.
- **When.** Once per *track change*. Sub-millisecond.
- **What it must never do.** Tint a surface, a border, body text, a control, or
  the artwork. **If the wall changes colour, this has been implemented
  wrongly.**
- **Shipping.** `LAMP` becomes a function of the playing album defaulting to
  `#E3A14E`. Ship the function returning the constant first; the extraction is
  then a one-file change that redesigns nothing. A setting turns it off; amber
  is the off state.

---

## 5. Tokens

### 5.1 The palette

| Token | Hex | Role | May **not** be used for |
|---|---|---|---|
| `RECESS` | **`#060708`** ⟵ CHANGE | shadow gap: bar, input wells, groove troughs, sleeve backing | text, raised surfaces |
| `WALL` | **`#0C0D0E`** ⟵ CHANGE | the hanging wall | anything raised |
| `PLINTH` | **`#141517`** ⟵ CHANGE + RENAME | inspector column, popover, resting control | the shelf |
| `PLINTH_LIT` | **`#1C1D20`** ⟵ CHANGE + RENAME | selected segment, playing row, hovered control | anything at rest |
| `HAIRLINE` | **`PAPER` @ 7 %** ⟵ CHANGE | the structural rules, a hovered label's rule, resting control borders | decoration, tile edges |
| `HAIRLINE_STRONG` | **`PAPER` @ 15 %** ⟵ CHANGE | selected control edges, the playing row's edge | a resting border |
| `PAPER` | **`#E8E4DB`** ⟵ CHANGE | primary text | large fills |
| `PAPER_DIM` | **`#ABA8A1`** ⟵ CHANGE | secondary text | figures that tick |
| `PAPER_FAINT` | **`#888680`** ⟵ CHANGE | tertiary text, **the selected tile's rule** | primary labels |
| `PAPER_MUTED` | **`#6C6A66`** ⟵ CHANGE | set but not sounding | text a user must read |
| `PAPER_RING` | `PAPER` @ 45 % | keyboard focus, `text_input` only | anything else |
| `SELECT_WASH` | `PAPER` @ 18 % | `text_input` selection | backgrounds |
| `LAMP` | `#E3A14E` | **the accent** — §5.3 | see §5.3 |
| `LAMP_BRIGHT` | `#F1B362` | the accent, hovered | a resting state |
| `LAMP_DEEP` | `#C7883D` | the accent, held | a resting state |
| `LAMP_GLOW` | `LAMP` @ 45 %, **blur 24** ⟵ CHANGE | the playing sleeve's halo | fills, borders, text |
| `LAMP_WASH` | **`LAMP` @ 10 % / 20 %** ⟵ NEW | Play album, hovered / pressed | any resting state |
| `ALERT` | `#D9776B` | problems, stated quietly | anything merely unusual |
| `SUCCESS` | `#86A97C` | *nothing yet — keep the slot* | decoration |

**Deleted:** `SHADOW` (§4.2), `RADIUS_TILE` (§5.4), `LAMP_INK` (§5.3 — nothing
sits *on* the accent any more), `MONO` and `SERIF` (§3).

**One board at four levels of light.** `PAPER_DIM`, `PAPER_FAINT` and
`PAPER_MUTED` are the same r : g : b ratios as `PAPER`, scaled down, so the ink
family is one material rather than four greys that drifted apart. (The shipped
ramp drifts warmer as it darkens; against a *cool* wall that reads yellowish.)
Each value is the **smallest** point on that ramp clearing its floor on every
surface it can land on, with 0.1 of margin.

### 5.2 Contrast, re-derived against the gallery surfaces

WCAG 2.1, computed rather than estimated. Floors are 4.5 : 1 for anything a
user has to read and 3 : 1 for a non-text mark whose job is to be locatable.

| Ink | on `RECESS` | on `WALL` | on `PLINTH` | on `PLINTH_LIT` | Floor | |
|---|---|---|---|---|---|---|
| `PAPER` `#E8E4DB` | 15.89 | 15.33 | 14.40 | 13.28 | 4.5 | pass |
| `PAPER_DIM` `#ABA8A1` | 8.49 | 8.20 | 7.70 | 7.10 | 4.5 | pass |
| `PAPER_FAINT` `#888680` | 5.54 | 5.34 | 5.02 | **4.63** | 4.5 | pass |
| `PAPER_MUTED` `#6C6A66` | 3.74 | 3.60 | 3.39 | 3.12 | 3.0 | pass |
| `LAMP` `#E3A14E` | 9.10 | 8.78 | 8.24 | 7.60 | 3.0 | pass |
| `ALERT` `#D9776B` | 6.53 | 6.30 | 5.92 | 5.46 | 4.5 | pass |

**The interesting result, reported honestly.** Re-derived independently on the
new ink ramp against the new surfaces, `PAPER_FAINT` and `PAPER_MUTED` land
within two bytes of revision 1's values (`#888680` vs `#8A857C`; `#6C6A66` vs
`#6E6A62`). The near-black wall does not demand different inks. What it does
change is the margin at the top of the range: revision 1's `PAPER_FAINT` on
`CARD_HIGH` computed to **4.483**, which `theme.rs`'s contrast test has to
excuse by comparing at the one-decimal precision WCAG publishes (its `ROUNDING`
constant, with a comment naming the pairing). On `PLINTH_LIT` the same ink
measures **4.63**.

> **The rounding excuse in `every_ink_clears_its_contrast_floor_on_every_surface_it_lands_on` can be deleted.** No pairing in the gallery palette needs it.

The invariants `theme.rs` already asserts are kept:
`PAPER_MUTED.r < PAPER_FAINT.r` (0.424 < 0.533) and
`PAPER_MUTED.r > RECESS.r × 2.0` (0.424 > 0.047). In `f32`:
`PAPER = (0.910, 0.894, 0.859)`, `PAPER_DIM = (0.671, 0.659, 0.631)`,
`PAPER_FAINT = (0.533, 0.525, 0.502)`, `PAPER_MUTED = (0.424, 0.416, 0.400)`,
`WALL = (0.047, 0.051, 0.055)`, `RECESS = (0.024, 0.027, 0.031)`,
`PLINTH = (0.078, 0.082, 0.090)`, `PLINTH_LIT = (0.110, 0.114, 0.125)`.

The two corrections revision 1 pinned as *corrections* stay pinned: the v0.1
values still fail, and the test that says so still passes.

iced 0.13 publishes no accessibility tree, so contrast and hit-target size are
the only accessibility guarantees baz can make. That is a reason to honour them
exactly, not a reason to shrug.

### 5.3 The accent discipline

**Playback truth** is a fact about the audio the engine is producing *right
now*: which album is sounding, which track within it, and where the playhead
is. Nothing else qualifies — not what is queued, not what is selected, not what
has focus, not what the scanner is doing, not how a gain stage is configured.

`LAMP` and its relatives may appear in exactly these places:

1. **the playing album's halo** — `LAMP_GLOW`, on artwork at any size;
2. **the playing dot** — `DOT` 6 px, in a wall label's first line or in a
   row's number column;
3. **the seek groove** — the elapsed fill and its knob;
4. **a seek in flight** — the elapsed stamp warms to `LAMP` while a position
   has been asked for and not yet confirmed;
5. **Play album** — its 1 px border and its play triangle.

It may **not** appear on: input focus, text selection, the scanning note, tile
hover or selection, the queue popover's header or its opening affordance's
active state, the Settings nav's current section, the Previous button, panel
toggles, the edition or ReplayGain selectors, the volume fader, the unity
detent, hover previews, tooltips, scrollbars, checkboxes, steppers, the
wordmark, or any readout whatsoever.

#### The exception is revoked, and replaced by a rule

Revision 1 permitted one solid amber rectangle — Play album — and argued it,
with an escape clause: *"If that ever stops being true, the exception is revoked
and Play becomes a `PAPER`-outlined button."*

Drawing it under the gallery direction is what revoked it. With the tile cards,
the radii, the shadows, the mono and the serif all gone, a full-width amber slab
became the loudest thing on screen and it was **not** the artwork. The mockup
made that unarguable.

**CHANGE:** Play album is `LAMP`-outlined, not `LAMP`-filled. 1 px `LAMP`
border, `LAMP` play triangle, `PAPER` SemiBold label, no fill at rest;
`LAMP_WASH` at 10 % hovered and 20 % pressed, with the border moving to
`LAMP_BRIGHT` / `LAMP_DEEP`. Height `TRANSPORT_HIT` 32, full column width.

This buys a rule with no exceptions, which is worth more than an exception with
a good argument:

> **Amber is never an opaque fill.** It appears only as a ≤ 6 px mark, a 4 px
> rail, a 1 px line, or light.

`LAMP_INK` had exactly one use — the label on the amber fill — so it is
deleted, and with it the `LAMP_INK on LAMP` row of the contrast test.
`the_lamp_is_spent_only_on_playback_truth`'s `PERMITTED` list keeps all four
names; `primary` still paints the accent, as a border rather than a background,
and the test's `button_colors` sweep already inspects `border.color`.

### 5.4 Spacing and radii

Base unit 4. The shipped ladder, plus one name for the grid's number.

| Token | px | Used for |
|---|---|---|
| `GAP_XXS` | 2 | lines within one block |
| `GAP_XS` | 4 | dot to label, row padding, chip padding |
| `GAP_SM` | 8 | siblings within a group |
| `GAP_MD` | 12 | groups within a surface |
| `GAP_LG` | 16 | surface padding, bar gutters, **work → label** |
| `GAP_XL` | 24 | panel padding, settings sections, bar outer gutters |
| `HANG` | **40** ⟵ NEW | work-to-work **and** work-to-wall-edge (§2.2) |

Radii come down, because an archive is rectilinear:

| Token | px | Applies to |
|---|---|---|
| — | **0** | **artwork, always**, and every rule |
| `RADIUS_SEGMENT` | **3** ⟵ CHANGE (was 4) | a segment inside its well, a checkbox, a queue/track row |
| `RADIUS_CHIP` | **3** ⟵ CHANGE (was 4) | hover tips, tooltips |
| `RADIUS_CTRL` | **4** ⟵ CHANGE (was 6) | buttons, inputs, wells, steppers, the popover |
| `DOT / 2` | 3 | the playing dot |
| `RADIUS_TILE` | **deleted** | the shelf has no rectangles that are not artwork |

The nesting rule holds: an inner shape is one step tighter than the well
containing it (segment 3 inside well 4).

---

## 6. Components

Each spec gives the states, the measurements, and **what the component needs
from whatever contains it** — the IA restructure is in flight, so nothing here
assumes today's layout.

### 6.1 Album tile — the wall label

**Needs from its container:** a column of the width the hang computes. Nothing
else.

| Part | Spec |
|---|---|
| art | `art(w)` square, radius 0, **no shadow, no card, nothing behind it** |
| gap art → label | `GAP_LG` (16) |
| title | `SIZE_BODY` / 1.40 Medium `PAPER`, `Wrapping::None`, clipped at one lane |
| gap | `GAP_XXS` (2) |
| artist | `SIZE_META` / 1.35 Regular `PAPER_FAINT`, `Wrapping::None`, clipped |
| label block | `LABEL_H` 36.4, left-aligned to the art's left edge |
| hit target | the whole cell |

**States — the shelf's entire state vocabulary is a rule under the label:**

| State | Mark |
|---|---|
| rest | none |
| hover | artist lifts `PAPER_FAINT` → `PAPER_DIM`, **plus a 1 px `HAIRLINE_STRONG` rule under the label**, art-width, `GAP_XS` below |
| pressed | identical to hover |
| selected (the inspector is showing this album) | **a 2 px `PAPER_FAINT` rule under the label**, art-width |
| playing | composes with any of the above: `LAMP_GLOW` halo, blur 24, offset 0 + `DOT` before the title, `GAP_XS` |

This directly answers the audit's §1.2 finding that *"in a screenshot you cannot
tell which tile is selected and which is merely under the pointer"*. Hover and
selection are now 1 px hairline versus 2 px paper — a 2× thickness and roughly
4× ink jump apart, and neither is a card, a border, a radius or the accent. It
supersedes `SELECTION_EDGE` (the 2 px tile border a parallel pass added), which
is deleted along with the tile's background entirely: `theme::tile` collapses to
"no background, no border, ever".

Pressed is deliberately identical to hover: a distinct press state on a control
whose click lasts ~100 ms is a flicker, and the feedback the user wants is the
inspector opening.

**`LAMP_GLOW`'s blur rises 16 → 24** (**CHANGE**). It is now the only shadow in
the product, so it can afford the room, and at 240–320 px sleeves a 16 px blur
is a thin rim rather than a light.

### 6.2 Album inspector

**Needs from its container:** the width band the IA specifies —
`clamp(0.28 × W, 340, 420)` — a scroll region, and below 700 px of window
height the whole panel scrolls rather than only the list.

Background `PLINTH`, `GAP_XL` padding, a 1 px `HAIRLINE` vertical rule against
the shelf.

| Part | Spec |
|---|---|
| sleeve | `min(column − 2 × GAP_XL, ART_MAX)`, **left-aligned**, halo only when *this* album is playing |
| title | `SIZE_TITLE` 22 / 1.20 SemiBold `PAPER`, capped at two lines |
| artist | `SIZE_EMPHASIS` / 1.35 `PAPER_DIM` |
| catalogue line | `SIZE_META` `PAPER_FAINT` — `1992 · 13 tracks · 45:35`, describing the **selected edition** |
| condition report | `SIZE_META` `PAPER_FAINT` — `FLAC · 16-bit · 44.1 kHz`, only when the scan read one |
| edition selector | §6.6, only when `editions.len() > 1` |
| Play album | §5.3 |
| track list | §6.3, reading width capped at 600 px |
| **Details** | below, always present — see below |
| gaps | `GAP_MD` between blocks |

Drawn in `visual/gallery/03-album-inspector.png` — deliberately showing an
album that is **selected but not playing**, with a *different* album playing in
the shelf beside it, because selection and playback are different facts and the
inspector has to be able to show one without the other. The whole column,
unscrolled, is `07-inspector-full.png`.

#### Details — the condition report in full

**`03-interface-prior-art.md` R6.** baz's audience came from a product that
showed roughly twenty fields for free; four lines is a regression for Marta and
Karl, and it is one this document was about to ship.

| Part | Spec |
|---|---|
| separator | a `HAIRLINE` rule, full content width |
| heading | `Details`, `SIZE_META` `PAPER_MUTED` Medium |
| field label | right-aligned in `FIELD_LABEL_W` **96**, `SIZE_META` `PAPER_MUTED` — wide enough for `Album artist`, the longest label in the set |
| field value | left-aligned after `GAP_MD`, `SIZE_META` `PAPER_DIM` |
| pitch | 17 px per row |
| presence | a row exists **only when the scan read one** — no `—`, no "Unknown" |

Fields, in this order: Album artist · Released · Label · Catalogue · Genre ·
Discs · Format · Bitrate · Size · ReplayGain · MusicBrainz · Added · Path.

**No disclosure triangle, no click, no "more" affordance.** It sits below the
track list, so on any real window it is below the fold: the wall label carries
the essentials and the condition report is on the back of the card, which you
turn over by scrolling. Devon never sees it. Marta never has to ask for it.
That is what progressive disclosure is supposed to mean, and it costs one rule
and a two-column list.

The label/value pair is a **right-aligned label column**, which is the same
figure-column discipline as §3.5 applied to text: the pinned edge is the one
the eye runs down.

### 6.9 The shelf strip, below 940 px

**`03-interface-prior-art.md` R9**, and the structural half of this belongs to
`01-ux-audit-and-ia.md` §4.3 rather than here. Below 940 px the IA has the
inspector take the content area and the shelf vanish entirely. Both
cataloguer-grade peers keep the collection on screen in detail view —
Lightroom's Filmstrip, Calibre's shared model — and since that regime is also
where the eventual full-window Album *place* is prototyped, the decision
propagates.

The visual spec, for whatever the IA decides to call it:

| Part | Spec |
|---|---|
| height | `ART_MIN(density)` + 2 × `GAP_MD` — 264 at `Balanced` |
| sleeves | `ART_MIN(density)`, gutter `HANG`, scrolling horizontally |
| labels | **none** — at strip scale a two-line label is noise, and the inspector already says which work you are on |
| selected | the 2 px `PAPER_FAINT` rule beneath it, exactly as §6.1 |
| playing | the halo, exactly as §6.1 |
| position | directly above the now-playing bar |

Still artwork and type only — in fact artwork only, which is the strongest
possible form of §1.3's claim.

### 6.3 Track and queue rows

One component, two uses (the inspector's list and the Up-next popover's).
**Needs from its container:** ≥ 300 px of width and a scroll region.

| Part | Spec |
|---|---|
| number column | `TRACK_NO_W` 24, **right-aligned**, `SIZE_META` `PAPER_FAINT` |
| title | `SIZE_BODY` / 1.40, `Wrapping::None` |
| artist (when track artists vary) | `SIZE_META` `PAPER_DIM` beneath, `GAP_XXS` |
| duration | `SIZE_META` `PAPER_FAINT`, **right-aligned** |
| row padding | `pad(GAP_XS, GAP_XS)`, `RADIUS_SEGMENT` 3 |
| list gutter | `scroll_gutter()` — `SCROLLBAR_LANE` 10, reserved whether or not the list scrolls |

| State | Number column | Title ink | Row |
|---|---|---|---|
| upcoming | position | `PAPER` | none |
| played | position | `PAPER_FAINT` | none |
| hovered | position | `PAPER` | `PLINTH` |
| playing | **`DOT` lamp dot**, replacing the number | `PAPER` Medium | `PLINTH_LIT` + `HAIRLINE_STRONG` |

The dot replaces the number rather than joining it, in a column that is
`TRACK_NO_W` wide either way, so a track starting moves no text.

### 6.4 Up next popover

**Needs from its container:** anchoring above the now-playing bar. Per the IA:
`POPOVER_W` 360, max height 0.6 × window, `GAP_LG` above the bar and from the
right edge.

Surface `PLINTH_LIT`, 1 px `HAIRLINE_STRONG`, `RADIUS_CTRL` 4, **no shadow**
(§4.2 deleted them; the surface step and the edge are the separation) and **no
scrim** — dimming ten thousand covers to show twelve rows is the exact mistake
this palette exists to avoid. No arrow or notch: iced's borders are four-sided.

Header `Up next` at `SIZE_EMPHASIS` with a ✕ right; summary `3 of 12 · 51:20`
at `SIZE_META` `PAPER_FAINT`; then §6.3's rows with a per-row ✕ on hover.
**The header and the affordance's active state are not amber** (§5.3).

### 6.5 The now-playing bar

**Unchanged geometry. Height exactly 102 px in every state.**

```
  1  rule (HAIRLINE)
 12  GAP_MD padding
 32  TRANSPORT_HIT
  8  GAP_SM
 15  PREVIEW_H          — reserved whether or not anything is hovering
 22  RAIL_HIT           — RAIL (4) + 2 × HIT_SLOP (9)
 12  GAP_MD padding
---
102
```

Three zones on one centre line, drawn at 2× in
`visual/gallery/04-now-playing-bar.png` with every slot called out.

**Left.** The wall label at bar scale — title `SIZE_BODY` Medium `PAPER` over
artist `SIZE_META` `PAPER_DIM`, `GAP_XXS` — plus the new `POSITION_W` 56 slot
carrying `3 / 12`, the block optically centred in the bar's height. The zone
takes a **maximum** width rather than pure `Fill` (the IA's §4.3 fix), so at
760 px the title clips as designed instead of breaking to three lines.

**Centre.** Previous · Play/Pause · Next over the seek groove, in a fixed
`SEEK_ROW_W` 380 column. Previous is new (the IA's step 5); it costs 40 px in a
column with 268 px spare.

**Right.** `SIGNAL_W` 96 then `VOLUME_BLOCK_W` 136, right-aligned, `GAP_XL`
from the window edge (**CHANGE**, was `GAP_LG`, and the detent sat one pixel
from the glass).

**What keeps it pixel-stable**, and the invariants this spec is accountable
for, unchanged from revision 1 except in their numbers:

1. **Nothing in the bar is sized to its content.** Every slot is a token wide
   enough for its worst case, measured (§3.4).
2. **Slots exist whether or not they have anything to say.** The seek row is
   reserved with no track loaded; `SIGNAL_W` is reserved when the chain is
   ordinary; the preview lane is reserved when the pointer is elsewhere.
3. **State changes touch ink, never geometry.** Pending is an opacity. Mute is
   a glyph swap in a fixed box. The seek knob is the only thing permitted to
   change size (5 → 7); the volume knob may not, because it would drag the
   unity detent and a detent that moves is not a detent.
4. **The face change is the risk, and the test is the answer.** Four slots
   shrink; `font.rs`'s advance-width test measures each against the face that
   will draw it. Do not ship the change without it.
5. **The slots are a ratchet** (`03-interface-prior-art.md` R11). Three vendors
   bought "visual calm" by removing control density inside two years and all
   three reversed; the information lost was always position, provenance and
   skip. **A slot may be added to this bar. None may be removed for
   tidiness** — and §3.4 shrinking four of them is a change to their *width*,
   which is the opposite move: the same facts, stated in less room.

### 6.6 Controls

**Grooves.** Both are `groove::Groove`. `RAIL` 4 in a `RECESS` trough, radius
`RAIL / 2`, **no border** (**CHANGE** — one fewer hairline; the trough's own
surface step is the edge). Seek: `LAMP` → `LAMP_BRIGHT` hovered → `LAMP_DEEP`
dragged, knob `KNOB` 5 → `KNOB_ACTIVE` 7; unfilled and knobless at undeclared
length. Volume: `PAPER_FAINT` → `PAPER_DIM`, `PAPER_MUTED` muted, `RECESS` with
no engine, knob never changes size. Unity detent `DETENT_W` 2 × `DETENT_H` 5
lifted `DETENT_GAP` 2 clear of the knob, `HAIRLINE` at rest and **`PAPER` when
engaged** — a five-fold ink jump on a 2 px mark. Never amber: unity is a
property of the control, not a claim about the music.

**Segmented control** (edition selector, ReplayGain mode). `RECESS` well,
`RADIUS_CTRL` 4, `HAIRLINE` 1 px, `SEGMENT_INSET` 2; segments `RADIUS_SEGMENT`
3, `Length::Fill`, `SIZE_META` Medium. Selected `PLINTH_LIT` +
`HAIRLINE_STRONG` + `PAPER`; unselected no background + `PAPER_DIM`; hovered
`PLINTH` + `PAPER`. Never amber.

**Transport.** `ICON_PX` 16 glyph in a `TRANSPORT_HIT` 32 square. `PLINTH` +
`HAIRLINE` at rest, `PLINTH_LIT` + `HAIRLINE_STRONG` hovered, `RECESS` pressed,
`GLYPH_OPACITY_DISABLED` 0.45 disabled, `GLYPH_OPACITY_PENDING` 0.55 pending —
**ink only**; no size, weight, colour or shape may vary with `pending`. A
tooltip is the accessible name, because iced publishes no accessibility tree.

**Stepper.** `STEPPER_HIT` 24 square, `RADIUS_CTRL` 4, `PLINTH` + `HAIRLINE`;
glyph `SIZE_BODY`, `−` is U+2212, `PAPER` live / `PAPER_MUTED` at the end of
travel; value in `SETTING_VALUE_W` **60**, right-aligned.

**Checkbox.** `SIZE_BODY` 13 box at `RADIUS_SEGMENT` 3, `RECESS` unchecked /
`PLINTH_LIT` checked, `HAIRLINE_STRONG` border, tick in `PAPER`. Never amber.

**Search field.** `RECESS` well, `RADIUS_CTRL` 4, `pad(GAP_SM, GAP_MD)`,
`SIZE_BODY`, 360 px in the top bar, plus the inline ✕ the IA specifies.
Placeholder `PAPER_FAINT`, value `PAPER`, selection `SELECT_WASH`. Border
`HAIRLINE` at rest, `HAIRLINE_STRONG` hovered, `PAPER_RING` focused.

### 6.7 Status readouts

Every readout obeys four rules: **no icon, no background, no border, never the
accent.** One that can appear and disappear lives in a fixed-width slot so its
arrival moves nothing. All of them are now Sans (§3.1); the *only* change from
revision 1's table is that "Mono" becomes "Sans" throughout.

| Readout | Ink | Notes |
|---|---|---|
| counts (`24 albums · 287 tracks`) | `PAPER_FAINT` | right-aligned in the top bar |
| filtered counts (`1 / 24 albums`) | `PAPER_FAINT` | |
| scanning | `PAPER_DIM` | a scan is the library working, not the music |
| files skipped | `PAPER_FAINT` | |
| problem | `ALERT` | quietly; no klaxon, no icon |
| signal path (`48 → 44.1 kHz`, `bit-perfect`) | `PAPER_FAINT` | `SIGNAL_W` 96, right-aligned, tooltip carries one plain sentence |
| queue position (`3 / 12`) | `PAPER_FAINT` | `POSITION_W` 56 |
| ReplayGain readout (`−7.24 dB`) | `PAPER` over a `PAPER_FAINT` detail line | `GAP_XXS` between |

**The conversion and bit-perfect notes get identical treatment** — same size,
weight, ink and slot — so neither can read as the other's verdict. Karl can
find them; nobody else will ever notice them arrive. This is the single most
important tone decision in the product and it must not be "improved" with a
colour, a badge or a lock icon.

### 6.8 Empty, loading and first-run

**baz has no spinner and no progress bar, anywhere.** During a scan the shelf
filling with covers *is* the progress indicator. Empty states are quiet centred
text, never an illustration and never a call to action.

| Surface | Line (`SIZE_EMPHASIS` `PAPER_DIM`) | Hint (`SIZE_META` `PAPER_FAINT`) |
|---|---|---|
| scanning, nothing yet | The shelf fills as the scan finds your music… | — |
| scanned, genuinely empty | No albums here yet | baz rescans this folder each time it starts |
| search, no match | Nothing matches “…” | Esc clears the search |
| queue, nothing queued | Nothing queued | Play an album and it appears here |

**First run.** One question, one field, one footnote, centred on an otherwise
empty `WALL`. Wordmark `baz` at `SIZE_EMPHASIS` `PAPER_FAINT` — deliberately
**unlit**, because nothing is playing, which is the product teaching what lit
means before it ever means anything. Question `Where's your music?` at
`SIZE_HERO` 32 / 1.15 **SemiBold Sans** (**CHANGE** — the serif is deleted).
Sub-line `SIZE_EMPHASIS` `PAPER_DIM`; field 460 px; error `ALERT`; footnote
`SIZE_CAPTION` `PAPER_FAINT`; `GAP_SM` within the heading block and `GAP_XL`
between blocks. Plus the folder picker and drop target the IA's step 12 adds.

---

## 7. Motion

**Every state change in baz takes 0 ms.** iced 0.13 ships no animation runtime;
producing a transition means driving state from a `window::frames()`
subscription, which redraws whether or not anything is moving. baz measures its
startup in hundreds of milliseconds and its memory in a 150 MiB thumbnail
budget.

**Permitted movement**, neither of which is animation: the seek fill and the
elapsed stamp advancing with playback (data arriving), and scrolling.

**Never animated, at any version:** the bar's geometry; the shelf grid — no
stagger, no pop-in, no fade as thumbnails decode, a thumbnail replacing its
placeholder is an instant swap; album art; anything requiring a redraw while
the window is idle.

**If iced gains a runtime**, exactly three things may animate and all three must
degrade to instant: the tile's hover rule 90 ms ease-out; a panel or popover
140 ms ease-out; the lamp's hue on track change 200 ms linear — a lamp warming.
No spring, no bounce, no overshoot.

---

## 8. What iced 0.13 forces

| The design wants | iced 0.13 | Fallback taken here |
|---|---|---|
| rounded or clipped artwork | `image` cannot be clipped or rounded | square sleeves, embraced — records are square |
| **tabular figures** | no OpenType feature control, no `tnum` | **not needed** — Plex Sans's digits are tabular by default (§3.1) |
| letter-spacing, small caps | neither is exposed | emphasis is weight, ink and size only |
| a hover zoom on a cover | no per-widget transform | the rule under the label is the affordance |
| an icon set | none ships | closed polygons in a unit square (`icon.rs`); the IA adds exactly one glyph (Previous, a mirror of Next) |
| a single-sided border | `Border` is four-sided | `rule` widgets — already how the bars are built |
| a focus ring on buttons | buttons take no keyboard focus | `PAPER_RING` on `text_input` only; tooltips name icon-only controls |
| transitions | no runtime; `frames()` redraws while idle | 0 ms everywhere (§7) |
| pointer capture during a drag | none | end the gesture on `CursorLeft` / `Unfocused` and commit (`groove.rs`) |
| text ellipsis | no ellipsis mode | `Wrapping::None` clips; every clipping slot has a fixed width |
| radial gradients / blur / backdrop | linear gradients on containers only | the placeholder's gradient; nothing else needs one |
| shadow spread | `Shadow` has colour, offset, blur only | tuned via blur — and there is exactly one shadow left (`LAMP_GLOW`) |
| an accessibility tree | none | contrast floors (§5.2) and hit targets are the guarantees baz *can* make |

---

## 9. Performance

| Change | Per-frame cost | Other cost |
|---|---|---|
| three faces instead of five | none | **−395 928 bytes** of binary; one font load at startup |
| the hang (fluid cell width) | none — arithmetic per layout pass, not per tile; widget count per frame unchanged | `shelf.rs`'s `CELL_W` / `CELL_H` / `ART_PX` become functions of shelf width |
| `THUMB_PX` 256 → 320 | none | LRU **600 → 384 entries** at the same 150 MiB (§2.6) |
| no shadow on tiles | **one fewer quad per tile** | none |
| the label's hover/selection rule | one `rule` widget on at most two tiles at a time | none |
| the density control | none — the step is read once per layout pass | at `Dense` the LRU holds 320² entries for ~200 px tiles; headroom falls to 3.5× the live widget count at 2560 px (§2.7) |
| the spine index | ≤ 27 `text` widgets, never virtualized because it is bounded | 20 px off the grid width |
| `Details` in the inspector | ≤ 13 `text` pairs, on one surface, below the fold | none |
| quieter placeholder + letterform | one extra `text` per art-less tile | none |
| art-derived lamp | none | one histogram per **track change**; sub-millisecond |

**Forbidden by this specification, on performance grounds:** blur or backdrop
effects of any kind; any per-frame animation or idle redraw; artwork above
`THUMB_PX`; per-tile gradients; shadows on anything that is not the playing
halo.

---

## 10. A light variant

**Recommendation: still defer.** Dark-first is right for this audience, and the
gallery direction makes the case stronger, not weaker — the whole design rests
on the works being the only lit things in the room.

Two of revision 1's three judgement calls survive verbatim and one gets harder:

1. **The halo stops working.** Amber glow on a paper ground has almost no
   contrast; the "this one" signal would have to become a different mark
   between themes, not a recoloured one.
2. **The sleeves need an edge.** On paper, dark covers punch and pale covers
   disappear, so every sleeve needs a hairline the dark variant deliberately
   does not have — which breaks §1.3's "artwork and type, nothing else".
3. **New, and worse than revision 1 knew.** The *inverse* of §4.2 is also true:
   on a paper ground a shadow works and a surface step barely does. A light
   variant would have to switch depth strategies, not just values — which is
   the definition of a second design.

Mechanically it is still half a day: every `pub const Color` becomes a field on
a `Palette` resolved at startup. The design work is the three above. Do not ship
a light theme that is the dark one with the numbers flipped.

---

## 11. Adoption order

The IA restructure (`01-ux-audit-and-ia.md` §5) is in flight in a parallel
agent's hands. **This spec lands on top of that work, and must not fight it.**
The ordering below is chosen so that every step either touches files the IA
work has already finished with, or touches only `theme.rs` values that the IA
work reads but never writes.

**Rule of engagement:** the IA owns *structure* — which surfaces exist, what
they contain, how they are dismissed. This document owns *values* — colour,
size, spacing, face. Where a step needs both, it is sequenced after the IA step
that creates the surface.

### Phase A — pure value changes, safe at any time

Nothing here moves a widget; all of it is `theme.rs` constants and one asset
directory. Each is independently shippable and revertible.

| # | Change | Touches | Why here |
|---|---|---|---|
| **A1** | **Delete `MONO`.** Retarget `font.rs`'s slot test from `mono()` to `sans()`; add `10:00:00`; rename `MONO_EM` → `DIGIT_EM`. Remove `IBMPlexMono-Regular.ttf` and its README row. | `font.rs`, `theme.rs`, views naming `theme::MONO` | The owner's actual complaint, and the change with the most visible effect per line. Proven by a test before anything else moves. |
| **A2** | **Delete `SERIF`.** It is `#[expect(dead_code)]` today, so this is a deletion with no call sites. | `font.rs`, `theme.rs` | Free, and it halves the remaining asset question. |
| **A3** | **The surfaces**: `WALL`, `RECESS`, four new values; `CARD` → `PLINTH`, `CARD_HIGH` → `PLINTH_LIT` (a mechanical rename). | `theme.rs` + a `sed` over views | The direction change, in eight constants. **Sequence after the IA's step 8**, which is the last one to move view code between files. |
| **A4** | **The inks and hairlines**: `PAPER`, `PAPER_DIM`, `PAPER_FAINT`, `PAPER_MUTED`, `HAIRLINE`, `HAIRLINE_STRONG`. Delete the `ROUNDING` excuse from the contrast test. | `theme.rs` | Must follow A3 — the ratios are computed against A3's surfaces. |
| **A5** | **Radii and line heights**: `RADIUS_CTRL` 4, `RADIUS_SEGMENT` 3, `RADIUS_CHIP` 3; per-token line heights; delete `RADIUS_TILE`. | `theme.rs`, views setting `.line_height(…)` | Independent of everything above. |
| **A6** | **The reserved slots**: `SIGNAL_W` 96, `LEVEL_W` 48, `PREVIEW_W` 48, `SETTING_VALUE_W` 60; add `POSITION_W` 56. `format_centidb` emits U+2212. | `theme.rs`, `replaygain.rs` + its tests | Must follow A1 (the measurements are in Sans). **Sequence after the IA's step 10**, which is the last one to touch bar geometry. |

### Phase B — the shelf, which is where the direction becomes visible

| # | Change | Touches | Why here |
|---|---|---|---|
| **B1** | **Delete the tile's chrome.** `theme::tile` → no background, no border, ever. Delete `SHADOW` and `SELECTION_EDGE`; `sleeve()` loses its shadow. | `theme.rs`, `views/shelf.rs` | Strictly a deletion, and it must land before B2 or the mock and the build disagree about what a tile is. |
| **B2** | **The label's rule** — 1 px `HAIRLINE_STRONG` on hover, 2 px `PAPER_FAINT` on selection; the artist's hover ink lift; `LABEL_H` 36.4; `GAP_LG` from art to label. | `views/shelf.rs`, `theme.rs` | Restores the two states B1 removed, in the new vocabulary. **Sequence after the IA's step 2**, which owns the caption block's height. |
| **B3** | **The hang** — `HANG`, `ART_MIN`, `ART_MAX`, `ART_TARGET`; `grid_width`; `columns` / `art` / `gutter` / `row_h` as functions of grid width, with `floor(x + 0.5)`. Extend `the_shelf_virtualizes_at_both_of_the_rails_two_widths` to the width *band*. | `shelf.rs` (pure, tested), `views/shelf.rs` | The only change that touches geometry code. **Sequence after the IA's step 9**, which sets the inspector's width band — the hang is a function of what that leaves. |
| **B4** | **`THUMB_PX` 320** and the `THUMB_CACHE_ENTRIES` re-derivation to 384. Assert `max(ART_MAX) == THUMB_PX` over every density step. | `art.rs` | Must follow B3, or the cache grows for art that has not arrived yet. |
| **B5** | **`LAMP_GLOW` blur 24**; the shelf's scrollbar takes `theme::scrollbar`. | `theme.rs`, `views/shelf.rs` | Two numbers. |
| **B6** | **The density control** (§2.7) — the three steps as pure data; the hang's four numbers read from the active step; the setting itself in Settings → Appearance. | `shelf.rs`, `theme.rs`, config, `views/settings.rs` | Must follow B3 (it parameterises B3's arithmetic) **and the IA's step 8**, which creates the Settings place the control lives in. Ship the steps as data with `Balanced` hard-coded first, then the setting: the layout half is testable without a UI. |
| **B7** | **The spine index** (§2.8) — `INDEX_W`, the lane taken off `grid_width`, the key run, jump-on-click, `/`-scoped type-to-jump. | `shelf.rs`, `views/shelf.rs`, `keys.rs` | Must follow B3 (it changes the grid width) and B6 (its lane must not be re-tuned twice). **This is the one item here that is a feature rather than a restyle**, and it is the one the study says a 40 000-album library cannot ship without. |

### Phase C — the surfaces the IA is still building

Each of these is a *style* for a surface the IA creates. They land **with** the
IA step that creates the surface, not before it.

| # | Change | Lands with |
|---|---|---|
| **C1** | Play album: `LAMP`-outlined, `LAMP_WASH` hover/press; delete `LAMP_INK` and the `LAMP_INK on LAMP` contrast row. | any time after A3 — it is one style function |
| **C2** | The inspector's flush-left sleeve capped at `ART_MAX`, the catalogue and condition lines, the two-line title cap. | IA step 9 |
| **C2b** | **Details** (§6.2) — the field list. Near-term it renders only the fields the scanner already has; the rest arrive as the scanner does, and the block is designed so a new field is one row rather than a layout decision. | IA step 9, immediately after C2 |
| **C2c** | The shelf strip below 940 px (§6.9). | IA step 9 — it is the same breakpoint |
| **C3** | The popover's `PLINTH_LIT` surface, `RADIUS_CTRL` 4, no shadow, no scrim. | IA step 6 |
| **C4** | The Settings place's section list and 640 px content cap in the new inks. | IA step 8 |
| **C5** | First run: Sans SemiBold hero, unlit wordmark. | IA step 12 |
| **C6** | The placeholder's quieter gradient and its letterform. | any time after A2 |
| **C7** | The derived lamp (§4.4) — **last**, because it is the only item that is a feature rather than a restyle, and everything above must be true before it means anything. | after all of the above |

### What must not regress, and how to know

- `every_ink_clears_its_contrast_floor_on_every_surface_it_lands_on` — extend
  the surface list to the renamed tokens; **delete `ROUNDING`**.
- `the_lamp_is_spent_only_on_playback_truth` — `PERMITTED` is unchanged; check
  it still sees `primary` painting the accent now that it is a border.
- `every_reserved_slot_holds_its_worst_case_in_the_bundled_face` — retarget to
  the Sans, add `10:00:00` and `199 / 240`, keep the 1 px slack.
- `the_shelf_virtualizes_at_both_of_the_rails_two_widths` — becomes a sweep
  over the width band, asserting `gutter == HANG` wherever art is uncapped and
  `ART_MIN ≤ art ≤ ART_MAX` everywhere.
- `a_setting_note_still_wraps_inside_its_two_reserved_lines` — unchanged in
  shape; the Sans is already what it measures.

---

## 12. The pictures

In `visual/gallery/`. Drawn to this document's own tokens by `render.py`, which
computes the hang with §2.2's arithmetic verbatim — so the column counts and
art sizes in the pictures are derived, not eyeballed, and a picture that
disagrees with the shipped app means one of the two is wrong.

| File | What it is |
|---|---|
| `01-shelf-1280.svg` / `.png` | the shelf at a 1280 window (grid 1250) — 4 × 262 px, gutter 40, dead gutter 0. Four tile states named in place: rest, playing (halo + dot), hovered (rule + ink lift), and a paper-pale sleeve for the other extreme |
| `02-shelf-1920.svg` / `.png` | the shelf at a 1920 window (grid 1890) — 6 × 268 px. The wide-window proportion, the squint test, and a near-black sleeve merging into the wall as designed |
| `03-album-inspector.svg` / `.png` | 1280 with the inspector open (grid 892) — 3 × 244 px. The inspector's album is *selected but not playing* while a different one plays in the shelf |
| `04-now-playing-bar.svg` / `.png` | the bar at 2×, with every reserved slot, its worst-case string and its measured width |
| `05-figures-specimen.svg` / `.png` | the proof that deleting the mono costs nothing: digit advance boxes in all three weights, a real duration column, stacked timestamps, and the one jiggle named |
| `06-density.svg` / `.png` | the three density steps at one window width (§2.7) |
| `07-inspector-full.svg` / `.png` | the inspector's whole column unscrolled, including the `Details` block (§6.2) |
| `render.py` | the generator; `python3 docs/design/visual/gallery/render.py` |

The PNGs are rendered with the **real bundled faces** installed into a scratch
`HOME`, so the type in them is IBM Plex Sans and not a host fallback. (The
revision-1 mockups in `visual/` could not claim this and said so; treat them as
the previous iteration.)

The revision-1 pictures stay in `visual/` — the `*-current-*.png` files are
still the only photographs of the shipped binary, and the `mock-*` files are
the superseded direction, kept so the change can be judged rather than
asserted.
