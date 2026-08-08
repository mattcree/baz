# baz — Visual Language

> The definitive specification for how baz looks. Written 2026-08-08 against
> `crates/baz/src/theme.rs` at `1919193`, the vision's fifth pillar
> ("presentation that honors the artwork"), and the personas in
> `docs/research/05-personas.md`.
>
> **This document is a specification, not code.** Every number in it is either
> a token that already exists in `theme.rs` — kept deliberately, so the spec
> and the code share one vocabulary — or a change, marked **CHANGE** with the
> reason and the evidence. Screenshots of the current build and mockups of the
> target are in `docs/design/visual/`.

---

## 0. How this document was checked

Every claim about how baz looks today was made against the real binary, not
from reading the source. `docs/design/visual/00-` … `08-current-*.png` are the
shipped app rendered on a private headless display against a throwaway library
of 32 albums and 285 tracks with generated cover art in six different visual
idioms, built **without** `device-output` so nothing could make a sound.

That build has one consequence worth stating up front: with
`Availability::NotBuilt`, `app.rs` hides the now-playing bar entirely, so
**there is no screenshot of the bottom bar**. It is specified here and drawn in
`mock-now-playing.svg` instead.

---

## 1. Direction

**The listening room, after dark, with the lamp on.**

baz is a dim room with a wall of records in it. The walls are warm charcoal —
never the blue-grey of a stock dark theme — because the covers are the only
thing in the room allowed to have colour, and ten thousand of them supply more
than enough. The text is warm off-white, the weight of liner-note paper. There
is exactly one accent, an amplifier's lamp amber, and it is spent only on what
is true about the music *right now*: which record is playing, and where in it
the needle is. Everything else — every control, every setting, every count,
every state — is made of surface, edge and ink. The room is quiet so that the
records are loud.

**Who it is for.** People who own their music: Marta, who has spent years
tagging 40,000 tracks and wants the work to look like a collection rather than
a spreadsheet; Devon, who plays albums front to back and wants the sleeve at a
size worth looking at; Karl, who wants to be told the truth about the signal
path in the quietest possible voice. Not Spotify users. Nobody here is being
sold anything, discovered to, or recommended at.

**What it deliberately is not.**

- **Not a streaming client.** No blue. No algorithmic rows, no "made for you",
  no rounded-square artwork, no colour-washed backgrounds sampled from the
  cover, no play button hovering over the art.
- **Not a skeuomorphic hi-fi.** No wood, no brushed metal, no VU needles, no
  knurled knobs, no drawn shelf. This is the answer to the open question in
  `VISION.md` ("how far to lean into shelf skeuomorphism"): **not at all.** A
  drawn shelf is a texture competing with ten thousand textures the user
  already owns. The covers are the only material in the room; the tactility
  comes from how they are lit and where they cast their shadow, not from
  furniture drawn behind them.
- **Not a broadsheet.** No hairline editorial grid, no numbered eyebrows, no
  rules dividing everything from everything. This is a room, not a page.

### 1.1 What is kept, and what is sharpened

The "listening room" is right and it stays. It was a good first pass and the
palette rationale in `theme.rs` is sound. Three things are wrong with the
result, and they are the whole of this redesign.

**1. baz has no typeface.** It borrows one. `Font::DEFAULT` in iced 0.13 is
`Family::SansSerif` — a *generic* family that each platform resolves for
itself — and baz then asks that unknown family for `Weight::Medium` and
`Weight::Semibold`. When the resolved family has no such face, the fallback
lands somewhere else entirely. On the test machine it lands on a **monospace**:
in `01-current-shelf.png` every tile title is monospaced while every artist
line beneath it is proportional, and in `00-current-first-run.png` the product's
one line of copy — *Where's your music?* — is set in a typewriter face. This is
not a taste problem, it is the reason baz reads as alpha-tier, and it is also a
correctness problem: baz will look like a different product on every machine.
`icon.rs` already rejected system glyphs on exactly this ground ("a player
should look the same everywhere"). §2.2 fixes it.

**2. The covers float.** Every sleeve carries a card shadow — offset 3, blur 8,
45% black — which is the shadow of something hovering above a surface, not of
something standing on one. Combined with a fixed 240 px grid centred in the
window, leaving dead gutters at both edges, the shelf reads as a table of
thumbnails. §3 and §4.2 fix it: a contact shadow, and a grid that fills the
window.

**3. The lamp is on when nothing is playing.** The accent is documented as
"reserved for playback truth" and is then spent on input focus, on the scanning
note, and on the primary action. The search field takes focus at launch, so
**the first frame baz ever draws is an amber-ringed box with no music playing**
(`01-current-shelf.png`). A reserved signal that appears before there is
anything to signal is not reserved. §2.1 cuts it back to four uses — all of
them the playhead — and one argued exception.

### 1.2 The signature

**The lamp takes the colour of the record that is on.**

There is exactly one accent on screen, it means one thing, and its *hue* is
read from the playing album's cover. Lightness and chroma are fixed, so it is
always recognisably the same lamp — a white sleeve cannot produce a white lamp
and a fluorescent sleeve cannot produce a fluorescent one. Only the hue is
data. Amber is the default and the fallback.

This is the answer to "should baz do palette extraction" (§3.3). It costs the
single-accent discipline nothing — there is still one accent, reserved to the
same one thing — and it makes the room respond to the collection instead of
imposing on it. It is also the only place in this document where anything is
allowed to be derived from artwork.

---

## 2. Tokens

The token sheet is drawn in `visual/tokens.svg`.

### 2.1 Palette

Hex values are the exact sRGB of the `f32` constants in `theme.rs` (rounded to
the nearest byte), so this table and that file are one decision.

| Token | Hex | Role | May be used for | May **not** be used for |
|---|---|---|---|---|
| `WALL` | `#131110` | the room behind the covers | app background, top bar | anything raised |
| `RECESS` | `#0D0B0A` | inset chrome, *below* the wall | now-playing bar, input wells, groove troughs, the sleeve's backing plate | text, raised surfaces |
| `CARD` | `#1B1816` | one step above the wall | hovered tile, resting control, panel/settings surface | selection |
| `CARD_HIGH` | `#221F1C` | one step above `CARD` | selected tile, selected segment, playing row, hovered control | anything at rest |
| `HAIRLINE` | `#EDE3D9` @ 8% | findable when you look, invisible when you don't | every resting border, every rule, the resting scrollbar, the resting detent | text |
| `HAIRLINE_STRONG` | `#EDE3D9` @ 17% | the same edge, firmer | selection edges, hovered controls, hovered scrollbar | a decorative outline |
| `PAPER` | `#EAE6E0` | primary text | titles, track names, live control ink, the engaged detent | large fills |
| `PAPER_DIM` | `#A8A29A` | secondary text | artists, one-sentence explanations, hovered fader ink | numbers that change in place |
| `PAPER_FAINT` | **`#8A857C`** ⟵ CHANGE | tertiary text | counts, durations, hints, signal notes, resting fader ink | primary labels |
| `PAPER_MUTED` | **`#6E6A62`** ⟵ CHANGE | set but not sounding | the muted fader, a stepper at the end of its travel | text a user must read |
| `PAPER_RING` | **`#EAE6E0` @ 45%** ⟵ NEW | keyboard focus | the focused `text_input`'s border | anything else |
| `SELECT_WASH` | **`#EAE6E0` @ 18%** ⟵ NEW | selected text | `text_input` selection | backgrounds |
| `LAMP` | `#E3A14E` | **the accent** | see §2.1.1 — and nothing else | see §2.1.1 |
| `LAMP_BRIGHT` | `#F1B362` | the accent, hovered | the seek fill and knob under the pointer; the Play button hovered | a resting state |
| `LAMP_DEEP` | `#C7883D` | the accent, held | the seek fill while dragged; the Play button pressed | a resting state |
| `LAMP_GLOW` | **`#E3A14E` @ 45%** ⟵ CHANGE (was 30%) | the halo | the playing sleeve's glow | fills, borders, text |
| `LAMP_INK` | `#1B140B` | ink on the lamp | the Play button's label and triangle | anything on a dark ground |
| `ALERT` | `#D9776B` | problems, stated quietly | the top bar's problem note, first-run errors | anything that is merely unusual |
| `SUCCESS` | `#86A97C` | success | *nothing yet — keep the slot, do not invent a use* | decoration |
| `SHELF_SHADOW` | **`#000000` @ 55%, offset (0, 5), blur 14** ⟵ CHANGE | the contact shadow | artwork, at every size | any other widget |

`LAMP_SOFT` (`LAMP` @ 55%) is **deleted**: its only use was the focus ring,
which becomes `PAPER_RING`.

#### 2.1.1 The accent discipline

**Playback truth** is a fact about the audio the engine is producing *right
now*: which album is sounding, which track within it, and where the playhead
is in that track. Nothing else qualifies — not what is queued, not what is
selected, not what has focus, not what the scanner is doing, not how a gain
stage is configured, not whether a device can follow the sample rate.

`LAMP` (and its `BRIGHT`/`DEEP`/`GLOW`/`INK` relatives) may appear in exactly
these places, and nowhere else:

1. **The playing album's halo** — `LAMP_GLOW`, on artwork at any size.
2. **The playing dot** — a `DOT` (6 px) circle, beside the playing album's
   title on a tile and in the number column of the playing queue or track row.
3. **The seek groove** — the elapsed fill and its knob.
4. **A seek in flight** — the elapsed timestamp warms to `LAMP` while a
   position has been asked for and not yet confirmed. A position being asked
   for is a claim about the playhead.
5. **The primary Play action** — the one exception, argued below.

It may **not** appear on: input focus, text selection, the scanning note, tile
or row selection, panel toggles, the edition selector, the ReplayGain mode
selector, the volume fader, the unity detent, hover previews, tooltips,
scrollbars, checkboxes, steppers, the wordmark, or any readout whatsoever.

**The exception, argued.** Play is the control that *creates* playback truth;
it is the only control in the product that does. It appears at most once per
screen, and it is the only lamp-*filled* rectangle anywhere in baz — every
other amber is a 6 px dot, a 4 px rail, or a glow. If that ever stops being
true, the exception is revoked and Play becomes a `PAPER`-outlined button.

**Two cuts from the current build**, both fixing a lamp that is on when nothing
is playing:

- **Focus ring**: `LAMP_SOFT` → `PAPER_RING`. Where the keyboard is has nothing
  to do with where the music is, and the search field is focused on launch.
- **Scanning note**: `LAMP` → `PAPER_DIM`, and it loses the mono face it shares
  with the counts so it reads as a sentence fragment rather than a figure. A
  scan is the library working, not the music.

#### 2.1.2 Contrast

WCAG 2.1 contrast ratio against each of the four surfaces a token can land on,
computed rather than estimated. Two tokens fail today and are changed above;
the numbers are here so the change is checkable.

| Foreground | on `WALL` | on `CARD` | on `RECESS` | on `CARD_HIGH` | Floor | Verdict |
|---|---|---|---|---|---|---|
| `PAPER` | 15.1 | 14.2 | 15.8 | 13.2 | 4.5 | pass |
| `PAPER_DIM` | 7.4 | 7.0 | 7.8 | 6.5 | 4.5 | pass |
| `PAPER_FAINT` **old** `#726D66` | **3.7** | **3.4** | **3.8** | **3.2** | 4.5 | **fail** |
| `PAPER_FAINT` **new** `#8A857C` | 5.1 | 4.8 | 5.4 | 4.5 | 4.5 | pass |
| `PAPER_MUTED` **old** `#4A4743` | **2.0** | **1.9** | **2.1** | **1.8** | 3.0 (non-text) | **fail** |
| `PAPER_MUTED` **new** `#6E6A62` | 3.5 | 3.3 | 3.7 | 3.1 | 3.0 | pass |
| `LAMP` | 8.5 | 8.0 | 8.9 | 7.4 | 3.0 | pass |
| `ALERT` | 6.1 | 5.7 | 6.4 | 5.3 | 4.5 | pass |
| `LAMP_INK` on `LAMP` | — | — | — | — (8.2) | 4.5 | pass |

`PAPER_FAINT` carries durations, counts, the signal note and every hint in the
product — the whole of Karl's readout vocabulary — and at 3.4 : 1 on the panel
it is below AA. The new value is the same hue, lightened until it clears 4.5 : 1
on every surface it can sit on.

`PAPER_MUTED` is not text a user must read (it is the muted fader and a stepper
at the end of its travel), so the 3 : 1 non-text floor applies — but 1.9 : 1 is
below even that, which means the position the listener chose is effectively
invisible while muted, and restoring it is the entire reason mute keeps the
fader where it is. The new value clears 3 : 1 everywhere while staying plainly
quieter than a live control.

Both changes keep the invariants `theme.rs` already asserts:
`PAPER_MUTED.r < PAPER_FAINT.r` (0.431 < 0.541) and
`PAPER_MUTED.r > RECESS.r * 2.0` (0.431 > 0.102). In `f32` terms the new
constants are `PAPER_FAINT = (0.541, 0.522, 0.486)` and
`PAPER_MUTED = (0.431, 0.416, 0.384)`.

iced 0.13 publishes no accessibility tree, so contrast and hit-target size are
the only accessibility guarantees baz can currently make. That is a reason to
honour them exactly, not a reason to shrug.

### 2.2 Type

#### 2.2.1 baz must bundle its typeface

**Decision: embed the faces and set them as the application default.**

The evidence is §1.1(1) and the screenshots. The mechanism is already in
iced 0.13 and costs no new crate: `include_bytes!` each face, hand the bytes to
`iced::application(…).font(…)` (verified present in `iced` 0.13.1
`src/application.rs:208`; `Settings::fonts` is the same thing by another
route), and name the family with `.default_font(Font::with_name("IBM Plex
Sans"))`. `theme::MEDIUM` / `SEMIBOLD` / `MONO` then become that family at the
weight asked for, with a real face behind each. No generic families, no weight
fallback roulette.

Line heights in §2.2.2 are `text(…).line_height(LineHeight::Relative(f))`,
which iced 0.13 exposes (`iced_core` 0.13.2 `src/widget/text.rs:107`); baz
currently takes the toolkit default of `Relative(1.3)` everywhere, which
`theme::LINE_HEIGHT` already names.

**The family: the IBM Plex superfamily** — Plex Sans, Plex Mono, Plex Serif.
Three reasons, none of them fashion:

1. **The Sans and the Mono are drawn together.** baz sets every figure that
   changes in place in a monospace (see below). Today that means Liberation
   Mono digits sitting inside a sentence set in the system sans — two
   unrelated voices in one line, visible in every screenshot's `2022 · 8
   tracks · 47:21`. Plex Mono shares Plex Sans's x-height, stem weight and
   terminal treatment, so the same line reads as one typeface with two
   settings.
2. **It has the weights baz asks for.** Regular, Medium and SemiBold all exist
   as real faces, so nothing synthesises and nothing falls back.
3. **It comes from the right world.** Plex was drawn for technical
   documentation and machine interfaces; its squared terminals and open
   apertures read as instrument panel rather than web app, which is the room
   this product is in.

**Licence and size.** OFL-1.1: redistributable, GPL-compatible, requires the
licence text be shipped and the Reserved Font Name not be reused for modified
copies. Five faces are needed (Sans Regular / Medium / SemiBold, Mono Regular,
Serif SemiBold): ≈ 800 KB unsubsetted, ≈ 250 KB subset to Latin plus the
punctuation baz actually uses (`·` `—` `→` `−` `…` `“” ‘’`).

**The objection, answered.** `icon.rs` rejected an icon font partly because it
"adds a binary asset with its own license to vet and subset". That argument was
right for three glyphs and is wrong here: the whole interface's voice is at
stake, and the alternative is not "no asset" but "a different product on every
machine" — which the same module rejected for the same glyphs.

**Fallback if the asset is refused.** Name concrete families per platform
rather than the generic (`Segoe UI` / `SF Pro Text` / `Cantarell`), and
**never ask for a weight above Regular** — take emphasis from ink and size
instead. baz then looks different on each platform but at least looks
deliberate on each. This is strictly worse and should not be chosen quietly.

#### 2.2.2 The scale

Sizes are `theme.rs`'s, unchanged except where marked. Line heights are given
as `LineHeight::Relative`, which iced 0.13's `text` accepts.

| Token | px | line-height | weight | face | Used for |
|---|---|---|---|---|---|
| `SIZE_CAPTION` | 11 | 1.45 (≈16) | Regular | Sans | tooltips, hover tips, footnotes |
| `SIZE_META` | 12 | 1.35 (≈16) | Regular | Sans, or **Mono for figures** | captions, durations, counts, notes, control labels |
| `SIZE_BODY` | 13 | 1.40 (≈18) | Regular / Medium | Sans | tile titles, track titles, button labels |
| `SIZE_EMPHASIS` | 15 | 1.35 (≈20) | Regular / Medium | Sans | section headings, empty-state lines, panel artist |
| `SIZE_TITLE` | **22** ⟵ CHANGE (was 19) | 1.20 (≈26) | SemiBold | **Serif** | the album's title |
| `SIZE_HERO` | **32** ⟵ CHANGE (was 28) | 1.15 (≈37) | SemiBold | **Serif** | the first-run question |

`SIZE_TITLE` moves 19 → 22 because it names the subject of the whole surface
and 19 px is a heading, not a title. `SIZE_HERO` moves 28 → 32 for the same
reason on a screen that contains nothing else.

#### 2.2.3 The serif, and where it is allowed

The serif appears in **exactly two places**: the album title on the album
surface, and the first-run question. Both are "the thing itself" rather than
chrome — a record's name, and the product's single line of copy. Sleeve
typography is overwhelmingly serif or display; UI sans is what a settings
dialog looks like. This is the one deliberate accessory in the design, and if
one thing has to be cut to keep the design disciplined, it is this.

Everything the software says *about itself* — Settings, Queue, ReplayGain, Play
album, every label, every note — is Sans. No exceptions.

#### 2.2.4 Figures

iced 0.13 exposes no OpenType feature control, so there is no `tnum` and no way
to ask a proportional face for tabular figures.

**The rule: every figure that changes in place is set in `MONO`.** That is
already baz's practice and it stays. It covers timestamps, durations, track
numbers, queue positions, dB values, sample rates, and the counts line —
anything whose digits tick or whose value is driven by a control.

Figures that do **not** change in place — a year in a tile caption, a count
inside a sentence — may be Sans. The distinction is whether a digit changing
would move a neighbour.

What bundling the family buys here is that the mono no longer looks borrowed.

### 2.3 Spacing

Base unit 4. `theme.rs`'s scale, plus one name for a number the shelf already
uses.

| Token | px | Used for |
|---|---|---|
| `GAP_XXS` | 2 | lines within one block (title over artist) |
| `GAP_XS` | 4 | caption to title, dot to label, row padding |
| `GAP_SM` | 8 | siblings within a group |
| `GAP_MD` | 12 | groups within a surface |
| `GAP_LG` | 16 | surface padding, bar gutters |
| `GAP_XL` | 24 | screen-level breathing room, panel padding, settings sections |
| `GAP_XXL` | **32** ⟵ NEW name for an existing number | the art-to-art gutter, the shelf's outer padding |

Padding is symmetric unless a token says otherwise. The two asymmetric
paddings in the product are both deliberate and both already exist:
`scroll_gutter()` (right only, reserving the scrollbar lane) and the tile's
horizontal pad (centring art in its cell).

### 2.4 Radii

| Token | px | Applies to |
|---|---|---|
| — | **0** | **artwork, always** |
| `RADIUS_SEGMENT` | 4 | a segment inside its well, the playing queue row, a checkbox |
| `RADIUS_CHIP` | 4 | hover tips, tooltips |
| `RADIUS_CTRL` | 6 | buttons, inputs, segmented wells, steppers |
| `RADIUS_TILE` | 10 | the tile's hover/selection card |
| `DOT / 2` | 3 | the playing dot |

Artwork is square-cornered because iced 0.13 cannot round or clip an `image`,
and because a record sleeve has square corners. The constraint and the truth
agree; design *with* it rather than apologising for it. Nothing in this
document asks for a rounded cover.

The nesting rule holds throughout: an inner shape is one step tighter than the
well containing it (segment 4 inside well 6), so the inner shape nests rather
than straining against the edge.

### 2.5 Elevation and borders

**One depth strategy, committed to:** hairline borders plus whisper-quiet
surface steps, and **exactly one shadow in the entire product**.

- Surface order, darkest to lightest: `RECESS` < `WALL` < `CARD` < `CARD_HIGH`.
  Each step is 2–3 points of luminance. You should feel the hierarchy without
  being able to point at the edge.
- Borders are 1 px, `HAIRLINE` at rest and `HAIRLINE_STRONG` when a thing is
  selected or hovered. iced's `Border` is four-sided only, so any single line
  (the rule under the top bar, the rule above the now-playing bar) is a `rule`
  widget, not a border. This is already how it is built.
- **The shadow is reserved for artwork.** No shadows on cards, buttons,
  panels, tooltips, popovers or rows. A shadow in baz means "this is a physical
  object"; only the sleeves are.

### 2.6 Motion

**Every state change in baz takes 0 ms.**

This is a decision, not an omission. iced 0.13 ships no animation runtime;
producing a transition means driving state from a `window::frames()`
subscription, which redraws continuously whether or not anything is moving.
baz measures its startup in hundreds of milliseconds and its memory in a
150 MiB thumbnail budget; spending idle frames on a fade would be spending the
thing the product is *for*.

**Permitted movement** — two things, and neither is animation:

1. The seek fill and the elapsed timestamp advancing with playback. That is
   data arriving.
2. Scrolling, which the toolkit drives from the input device.

**Never animated, at any iced version:**

- the now-playing bar's geometry (§4.6);
- the shelf grid — no stagger, no pop-in, no fade as thumbnails decode; a
  thumbnail replacing its placeholder is an instant swap;
- album art — no crossfade, no Ken Burns, no zoom;
- anything that would require a redraw while the window is idle.

**If iced gains an animation runtime**, exactly three things may animate, and
all three must degrade to instant:

| What | Duration | Easing |
|---|---|---|
| the tile hover card's opacity | 90 ms | ease-out |
| a panel's open/close | 140 ms | ease-out |
| the lamp's hue when the playing album changes | 200 ms | linear — a lamp warming |

No spring, no bounce, no overshoot anywhere. This is a room, not a toy.

---

## 3. Album art

Art is the product. Everything in §2 exists to get out of its way.

### 3.1 Sizes

| Surface | Edge (logical px) | Source |
|---|---|---|
| shelf tile | **200 – 256, computed per window width** (§4.2) | 256² LRU thumbnail |
| album surface, two-column | **min(420, 40% of surface width)**, never below 240 | same |
| album surface, one-column | min(container − 2·`GAP_XL`, 480) | same |
| now-playing bar | **none** | — |

Three notes.

**The bar carries no artwork.** It is 102 px tall and pixel-stable; a 78 px
thumbnail in it would be too small to honour anything and would put a second
copy of the playing cover on a screen that already has the real one, haloed, in
the shelf. What is playing is said in words there, and shown in the shelf.

**The upper bound is the cache, not taste.** `art::THUMB_PX` is 256, so art
above 256 logical px upscales and softens. On a 2× display even a 256 px tile
is drawn from a 256 px source and is already soft. Raising `THUMB_PX` to 320
would cost 37% of the LRU's capacity (600 → 375 entries at the same 150 MiB
budget). **Recommendation: keep 256 for now**, and revisit when the cache is
DPI-aware rather than trading capacity for sharpness blindly.

**Nothing is ever drawn on top of a sleeve.** No play overlay on hover, no
badge, no duration chip, no gradient scrim, no selection tint. Even the playing
mark sits beside the caption, off the art. The only thing that touches the art
is light around it.

### 3.2 When art is missing

Keep the deterministic gradient (`vm::gradient_colors`, hash → HSL). Two
changes.

**Quieten it.** Today the placeholder samples S ∈ [0.35, 0.70] and L ∈ [0.22,
0.50], which produces blocks that out-shout real covers — a wall where the
*missing* art is the loudest thing on it is backwards. **CHANGE** to
S ∈ [0.10, 0.28], L ∈ [0.14, 0.28]. A missing sleeve should read as absence,
not as an abstract cover.

**Give it a letterform.** The album title's first character, set in the Serif
at **0.28 × the art edge**, `PAPER` at **12%** opacity, optically centred. One
`text` widget, no new glyph work. A blank gradient says nothing; a letter says
"this is a sleeve with no picture" and gives the eye something to sort by while
scrolling.

The gradient stays deterministic per album id, so the same missing album is the
same colour every launch — that consistency is what lets Marta recognise a hole
in her collection by sight.

### 3.3 Colour from art: yes, in exactly one place

The research is right that palette extraction is cheap and proven. It is also
how every streaming client ends up washing its chrome in whatever hue the
current cover happens to have, which destroys the neutral room the covers need.

**baz extracts one colour, from one cover, for one purpose: the lamp.**

- **Source.** The already-decoded ≤ 256² RGBA thumbnail sitting in the LRU. No
  new decode, no new I/O, no new dependency.
- **Method.** A 4 × 4 × 4 RGB histogram (64 bins) over every fourth pixel
  (≈ 4,000 samples). Convert bin centroids to a perceptual space; discard bins
  below 0.04 chroma, below 0.25 lightness, or above 0.85 lightness. Take the
  most populous survivor. If none survives, use amber.
- **The constraint that makes it a design.** **Only the hue survives.**
  Lightness is forced to 0.72 and chroma to 0.13 — the coordinates of `LAMP`
  itself. It is the same lamp with a different record in front of it, never a
  different lamp.
- **Where it lands.** The halo, the playing dot, the seek fill and knob, and
  the Play button's fill; `LAMP_INK` is recomputed as the same hue at
  lightness 0.12. Nothing else.
- **When.** Once per *track change*, not per frame. Sub-millisecond.
- **What it costs the single-accent discipline.** Nothing. There is still
  exactly one accent on screen and it still means exactly one thing. The rule
  is unchanged; only its hue is data.
- **What it must never do.** Tint a surface, a border, body text, a control, or
  the artwork itself. If the wall changes colour, this feature has been
  implemented wrongly.
- **Shipping.** `LAMP` becomes a function of the playing album rather than a
  constant, defaulting to `#E3A14E`. Ship the function returning the constant
  first; the extraction is then a one-file change that redesigns nothing. A
  setting turns it off; amber is the off state.

### 3.4 Making a wall of covers feel like a collection

Five moves, in order of how much they matter.

1. **Fill the window.** A fixed 240 px grid centred in a variable window leaves
   dead gutters — 220 px of nothing at 940 px wide with a panel open. A record
   wall goes to the edges. §4.2 makes the cell width a function of the
   viewport.
2. **Ground the covers.** `SHELF_SHADOW`: tighter, lower, darker than the
   current card shadow, so a sleeve stands on the shelf instead of hovering
   above it. This is where the tactility comes from, and it is the whole of the
   answer to "how much skeuomorphism".
3. **Quieten the captions.** Title in `PAPER` Medium, artist in `PAPER_FAINT`
   — and **drop the year** (**CHANGE**). At rest the shelf answers "what do I
   own"; the year answers "which pressing", which is a question the album
   surface already answers. Fifteen captions each carrying three facts is a
   table; fifteen carrying two is a wall of records with labels.
4. **Give the art more room.** Minimum 200 px, up to 256 (§4.2), against 208
   fixed today.
5. **Put nothing between the covers.** No borders, no cards at rest, no
   separators, no badges, no hover overlays. The gutter is empty wall.

Judge these against `visual/01-current-shelf.png` (before) and
`visual/mock-shelf.svg` (after).

---

## 4. Components

Each spec gives the states, the measurements, and — because the information
architecture is being revisited in parallel and the right-hand rail may not
survive — **what the component needs from whatever contains it**, rather than
assuming today's layout.

Every component is drawn in `visual/mock-states.svg`.

### 4.1 Album tile

**Needs from its container:** a cell of the width the grid computes; nothing
else. It is self-contained.

| Part | Spec |
|---|---|
| art | `ART` × `ART` square, radius 0, `RECESS` backing plate, `SHELF_SHADOW` |
| gap art → caption | `GAP_MD` (12) |
| title | `SIZE_BODY` / 1.40 Medium `PAPER`, `Wrapping::None`, clipped |
| gap | `GAP_XXS` (2) |
| artist | `SIZE_META` / 1.35 Regular `PAPER_FAINT`, `Wrapping::None`, clipped |
| cell padding | `GAP_MD` vertical; horizontal = (cell − art) / 2 |
| hit target | the whole cell |

**States** — the card is drawn behind the whole cell, inset `GAP_MD` around the
art:

| State | Card | Border | Art | Caption |
|---|---|---|---|---|
| rest | none | none | `SHELF_SHADOW` | as above |
| hover | `CARD`, `RADIUS_TILE` | none | unchanged | unchanged |
| pressed | *identical to hover* | | | |
| selected (its detail surface is showing this album) | `CARD_HIGH`, `RADIUS_TILE` | `HAIRLINE_STRONG` 1 px | unchanged | unchanged |
| playing | composes with any of the above | | `LAMP_GLOW` halo **instead of** `SHELF_SHADOW`, blur 16, offset 0 | `DOT` lamp dot + `GAP_XS` before the title |

Pressed is deliberately identical to hover: a distinct press state on a control
whose click lasts ~100 ms is a flicker, and the feedback the user wants is the
panel opening.

`LAMP_GLOW` rises from 30% to 45% (**CHANGE**) because at 200–256 px the 30%
halo is not visible against `WALL` — the mark that says "this one" has to
actually be a mark.

### 4.2 Shelf grid

**CHANGE: the cell width becomes a function of the viewport.**

```
GRID_PAD  = GAP_XXL (32)          # was 24
GUTTER    = GAP_XXL (32)          # the art-to-art gap, unchanged in value
ART_MIN   = 200
ART_MAX   = 256                   # = art::THUMB_PX; above this the art upscales

columns(w) = max(1, floor((w - 2*GRID_PAD + GUTTER) / (ART_MIN + GUTTER)))
art(w)     = min(ART_MAX, (w - 2*GRID_PAD - (columns(w)-1)*GUTTER) / columns(w))
gutter(w)  = columns(w) > 1
             ? (w - 2*GRID_PAD - columns(w)*art(w)) / (columns(w) - 1)
             : GUTTER
cell_h(w)  = art(w) + GAP_MD + 36 + GAP_XL     # art + gap + caption + row gap
```

Worked, so the change is checkable:

| Viewport | Columns | Art | Gutter | Row height | Today |
|---|---|---|---|---|---|
| 1280 (no panel) | 5 | 217 | 33 | 289 | 5 × 208, 284 |
| 940 (panel open) | 3 | 256 | 54 | 328 | 3 × 208, 284 |
| 640 (minimum) | 2 | 256 | 64 | 328 | 2 × 208 |
| 2560 | 10 | 220 | 33 | 292 | 10 × 208 |

The art never shrinks below today's 208 at any width that mattered, the grid
always reaches both edges, and the column counts are unchanged — so the shelf
gains sharpness and loses dead space without showing fewer records.

**Cost to the code**, stated honestly: `crates/baz/src/shelf.rs`'s `CELL_W` /
`CELL_H` / `ART_PX` constants become functions of the viewport width, and
`spacer_height` and `visible_rows` must take the derived row height. The
virtualization is otherwise untouched — same widget count per frame, same
overscan, arithmetic per layout pass rather than per tile. The existing test
`the_shelf_virtualizes_at_both_of_the_rails_two_widths` should keep its shape
and gain the new expected column counts.

**Scrollbar.** Apply `theme::scrollbar` and `theme::list_scrollbar` to the
shelf as well as the rail lists (**CHANGE**). Today the shelf uses iced's
default, and in `01-current-shelf.png` it is the brightest object on screen —
a stock blue-grey bar in a room that has no blue in it.

**Empty and loading:** §4.10.

### 4.3 Album surface

**Needs from its container:** ≥ 720 px total width with ≥ 360 px beside the
art for the two-column form; a scroll region for the track list. Below 720 px
it reflows to one column — art, then header, then list — rather than shrinking
the art. **The art never goes below 240 px on any surface.** Nothing here
depends on the surface being a right-hand rail.

Drawn full-width in `visual/mock-album-detail.svg`, precisely because it must
not assume the rail.

| Part | Spec |
|---|---|
| art | §3.1; `LAMP_GLOW` halo when this album is playing, `SHELF_SHADOW` otherwise |
| playing line (only when playing) | `DOT` + `SIZE_META` Mono `PAPER_DIM`, e.g. `Playing · track 3 of 8`, below the art |
| title | `SIZE_TITLE` (22) / 1.20 SemiBold **Serif** `PAPER` — wraps freely, this is the one thing on the surface allowed to |
| artist | `SIZE_EMPHASIS` / 1.35 `PAPER_DIM` |
| meta | `SIZE_META` Mono `PAPER_FAINT` — `year · n tracks · total`, describing the **selected edition**, not the album |
| encoding | `SIZE_META` Mono `PAPER_FAINT` — `FLAC · 16-bit · 44.1 kHz`, only when the scan read one |
| edition selector | §4.7, only when `editions.len() > 1` |
| Play album | §4.7, the only lamp-filled control |
| track list | §4.4, capped at **600 px** of reading width |
| gaps | `GAP_MD` between blocks, `GAP_XL` around the surface |

**The reading column is capped, not stretched.** A track list run out to a
1280 px window's edge puts half a screen of nothing between a title and its
duration, which is exactly the spreadsheet the shelf exists to avoid.

### 4.4 Track and queue rows

One component, two uses. **Needs from its container:** ≥ 300 px of width (24
number + 8 + title ≥ 180 + 8 + duration 44 + 10 scrollbar lane) and a scroll
region. It works in a rail, a drawer, a sheet or a page.

| Part | Spec |
|---|---|
| number column | `TRACK_NO_W` (24), right-aligned, `SIZE_META` Mono `PAPER_FAINT` |
| title | `SIZE_BODY` / 1.40, `Wrapping::None` |
| artist (when the album's track artists vary, or the queue row has one) | `SIZE_META` `PAPER_DIM` beneath the title, `GAP_XXS` |
| duration | `SIZE_META` Mono `PAPER_FAINT`, right |
| row padding | `pad(GAP_XS, GAP_XS)` |
| list gutter | `scroll_gutter()` — `SCROLLBAR_LANE` (10) on the right, reserved whether or not the list scrolls |

**States:**

| State | Number column | Title ink | Title weight | Row |
|---|---|---|---|---|
| upcoming / plain | position | `PAPER` | Regular | none |
| played | position | `PAPER_FAINT` | Regular | none |
| playing | **`DOT` lamp dot**, replacing the number | `PAPER` | Medium | `CARD_HIGH`, `RADIUS_SEGMENT`, `HAIRLINE_STRONG` 1 px |
| not interactive (v0.1) | — | — | — | **no hover affordance, no pointer cursor** |

The dot replaces the number rather than joining it, in a column that is
`TRACK_NO_W` wide either way, so a track starting moves no text. An affordance
that does nothing is a lie: rows gain hover and a pointer cursor on the day the
engine gains a "jump to queue position" command, and not before.

### 4.5 Transport controls

| Part | Spec |
|---|---|
| glyph box | `ICON_PX` (16) square, rasterised from polygons (`icon.rs`) |
| hit target | `TRANSPORT_HIT` (32) square, fixed in both axes |
| chrome | `CARD` + `HAIRLINE` 1 px, `RADIUS_CTRL` |
| hover | `CARD_HIGH` + `HAIRLINE_STRONG` |
| pressed | `RECESS` + `HAIRLINE_STRONG` |
| disabled | `CARD` + `HAIRLINE`, glyph at `GLYPH_OPACITY_DISABLED` (0.45) |
| pending (command sent, not yet confirmed) | **ink only**: glyph at `GLYPH_OPACITY_PENDING` (0.55). No size, weight, colour or shape may vary with `pending` |
| name | a tooltip — iced 0.13 publishes no accessibility tree, so this *is* the accessible name |

**No new icons are required by this document.** Anything added later must be
expressible as closed polygons in a unit square (`icon.rs` can only rasterise
that), which rules out strokes, arcs of significant radius, and anything with a
line cap.

### 4.6 The now-playing bar

**Three zones on one vertical centre line**: what is playing (left, fill,
clipped), transport over the groove (centre, fixed column), signal note and
volume (right, fill, right-aligned, clipped).

**Height: exactly 102 px in every state.**

```
  1  rule (HAIRLINE)
 12  GAP_MD padding
 32  TRANSPORT_HIT
  8  GAP_SM
 15  PREVIEW_H          — the hover-preview lane, reserved whether or not
                          anything is hovering
 22  RAIL_HIT           — RAIL (4) + 2 × HIT_SLOP (9)
 12  GAP_MD padding
---
102
```

**How the design preserves pixel stability** — the invariants this spec is
accountable for:

1. **Nothing in the bar is sized to its content.** `STAMP_W` 52, `SEEK_W` 260,
   `SEEK_ROW_W` 380, `SIGNAL_W` 120, `VOLUME_W` 96, `VOLUME_BLOCK_W` 136,
   `PREVIEW_W` 58, `LEVEL_W` 62 — every one a token, every one wide enough for
   its worst case (`h:mm:ss`, `192 → 176.4 kHz`, `-18.1 dB`).
2. **Slots exist whether or not they have anything to say.** The seek row is
   reserved with no track loaded; the signal note's 120 px is reserved when the
   chain is ordinary; the preview lane is reserved when the pointer is
   elsewhere. A note *appearing* moves nothing.
3. **State changes touch ink, never geometry.** Pending is an opacity. Mute is
   a glyph swap inside a fixed box plus an ink change. The seek knob is the
   only thing in the bar permitted to change size (5 → 7 px), and only because
   nothing is drawn beside it; the volume knob may not, because it would drag
   the unity detent with it and a detent that moves is not a detent.
4. **Nothing this document adds goes in the bar.** No artwork, no extra
   controls, no readouts.
5. **The one new risk is the bundled typeface** (§2.2): a different face has
   different figure widths, and every fixed slot in this bar was sized against
   the old one. Mitigation: keep the existing arithmetic assertions in
   `theme.rs`, and add one that measures the rendered advance width of
   `"0:00:00"` and `"-18.1 dB"` in the bundled Mono at `SIZE_META` /
   `SIZE_CAPTION` against `STAMP_W` and `LEVEL_W`. Do not ship the font change
   without it.

**Left zone.** Title `SIZE_BODY` Medium `PAPER` over artist `SIZE_META`
`PAPER_DIM`, `GAP_XXS` between, neither wrapping, the zone clipping. With no
engine or nothing playing, one line of `SIZE_META` `PAPER_FAINT` stating the
fact plainly.

**Right zone.** The signal note (§4.9) then the volume block, right-aligned so
even the rarely-seen skipped-files note grows leftward into the gutter. The
fader sits beside the note that says whether the path is bit-exact because the
fader is the one control that can take it out of bit-exactness; the adjacency
is the explanation.

### 4.7 The two grooves

Both are `groove::Groove`. Rail `RAIL` (4) tall, `RECESS` trough, `HAIRLINE`
1 px border, radius `RAIL / 2`. Cursor: `Pointer` when live, `Grabbing` while
held, `None` when inert.

**Seek — the accent applies, because position is playback truth.**

| State | Fill | Knob radius |
|---|---|---|
| rest | `LAMP` | `KNOB` (5) |
| hover | `LAMP_BRIGHT` | `KNOB_ACTIVE` (7) |
| dragged | `LAMP_DEEP` | `KNOB_ACTIVE` (7) |
| undeclared length | trough only, no fill | 0 — and the widget refuses the pointer and leaves the cursor alone |

The elapsed timestamp warms to `LAMP` while a seek is in flight and cools the
moment the engine confirms.

**Volume — the accent does not apply, because a setting is not the music.**

| State | Fill and knob | Knob radius |
|---|---|---|
| rest | `PAPER_FAINT` | `KNOB` (5) |
| hover / dragged | `PAPER_DIM` | `KNOB` (5) — **never changes** |
| muted | `PAPER_MUTED` — the chosen position stays visible | `KNOB` |
| no engine | `RECESS` — the groove and its detent keep their place | `KNOB` |

**The unity detent.** `DETENT_W` (2) × `DETENT_H` (5), at the top of the
travel, lifted `DETENT_GAP` (2) above the rail so it clears the knob rather
than hiding under it. Ink: `HAIRLINE` at rest, **`PAPER` when the handle is on
it** — a five-fold jump in weight on a 2 px mark, which is what makes "at
unity" and "one pixel below unity" different on sight. Deliberately not amber:
unity is a property of the control, not a claim about the music.

**Hover preview.** A `PREVIEW_W`/`LEVEL_W`-wide tip in `CARD_HIGH` with a
`HAIRLINE_STRONG` edge and `RADIUS_CHIP` corners, `SIZE_CAPTION` Mono
`PAPER_DIM`, floating in the reserved lane above the groove, centred on the
pointer and clamped to stay whole. Not amber: a preview is a position being
*considered*, which is neither truth nor a request.

### 4.8 Controls

**Segmented control** (edition selector, ReplayGain mode — the same control,
because it answers the same question: *which one of these few*).

| Part | Spec |
|---|---|
| well | `RECESS`, `RADIUS_CTRL`, `HAIRLINE` 1 px, `SEGMENT_INSET` (2) padding |
| segment | `RADIUS_SEGMENT` (4), `pad(GAP_XS, GAP_SM)`, `Length::Fill` — equal widths |
| label | `SIZE_META` Medium |
| selected | `CARD_HIGH` + `HAIRLINE_STRONG` + `PAPER` |
| unselected | no background + `PAPER_DIM` |
| hovered (unselected) | `CARD` + `PAPER` |
| disabled | as unselected, no press |

Never amber: choosing a format or a gain mode is a view, not a claim about what
is playing.

**Primary action** (Play album). `LAMP` fill, `RADIUS_CTRL`, `LAMP_INK` label
at `SIZE_BODY` SemiBold with the play triangle at `ICON_PX` beside it,
`pad(GAP_SM, 0)` and `Length::Fill` within its column. Hover `LAMP_BRIGHT`,
press `LAMP_DEEP`, disabled `CARD` + `PAPER_FAINT`. **At most one per screen.**

**Panel/view toggle** (Queue, Settings). Identical to a segment: label-only at
rest in `PAPER_DIM`, `CARD_HIGH` + `HAIRLINE_STRONG` when its surface is open.
Fixed width (`QUEUE_TOGGLE_W` 92) so gaining a count in the label — `Queue`
→ `Queue · 12` — moves nothing beside it.

**Stepper** (`−` / `+` beside a numeric setting).

| Part | Spec |
|---|---|
| button | `STEPPER_HIT` (24) square, `RADIUS_CTRL`, `CARD` + `HAIRLINE` — the transport's chrome in a smaller square |
| glyph | `SIZE_BODY` Mono, `−` is U+2212 (matches `+` in width and height), `PAPER` live / `PAPER_MUTED` at the end of travel |
| value | `SETTING_VALUE_W` (68), right-aligned, `SIZE_META` Mono `PAPER` |
| row | label (`SIZE_META` `PAPER_DIM`) — fill — value — `−` — `+`, `GAP_SM` |

Smaller than `TRANSPORT_HIT` on purpose: a setting is adjusted deliberately and
rarely; play and pause are hit in a hurry. A stepper at the end of its travel
renders disabled rather than absorbing the press, and the fixed value slot is
what stops a repeated press moving the button out from under the pointer.

**Checkbox.** `SIZE_BODY` (13) box at `RADIUS_SEGMENT`, `RECESS` unchecked /
`CARD_HIGH` checked, `HAIRLINE_STRONG` border, tick in `PAPER`, label
`SIZE_META` `PAPER`, `GAP_SM` between. Disabled: `PAPER_MUTED` throughout.
Never amber.

### 4.9 Search field and status readouts

**Search field.** `RECESS` well, `RADIUS_CTRL`, `pad(GAP_SM, GAP_MD)`,
`SIZE_BODY`, width 360 in the top bar. Placeholder `PAPER_FAINT`, value
`PAPER`, selection **`SELECT_WASH`** (**CHANGE**, was `LAMP_GLOW`).

| State | Border |
|---|---|
| rest | `HAIRLINE` |
| hover | `HAIRLINE_STRONG` |
| focused | **`PAPER_RING`** (**CHANGE**, was `LAMP_SOFT`) |

**Status readouts.** Every readout in baz obeys four rules: no icon, no
background, no border, and never the accent. A readout that can appear and
disappear lives in a fixed-width slot so its arrival moves nothing.

| Readout | Face | Ink | Notes |
|---|---|---|---|
| counts (`32 albums · 285 tracks`) | `SIZE_META` Mono | `PAPER_FAINT` | |
| filtered counts (`12 / 32 albums`) | `SIZE_META` Mono | `PAPER_FAINT` | |
| scanning | `SIZE_META` **Sans** | **`PAPER_DIM`** | **CHANGE**, was Mono `LAMP` |
| files skipped | `SIZE_META` Mono | `PAPER_FAINT` | |
| problem | `SIZE_META` Sans | `ALERT` | quietly; no klaxon, no icon |
| signal path (`48 → 44.1 kHz`, `bit-perfect`) | `SIZE_META` Mono | `PAPER_FAINT` | in `SIGNAL_W` (120), right-aligned, tooltip carries one plain sentence |
| ReplayGain readout (`-7.24 dB`) | `SIZE_META` Mono | `PAPER` over a `PAPER_FAINT` detail line | `GAP_XXS` between |

**The conversion and bit-perfect notes are deliberately quiet, and the two get
identical treatment.** `48 → 44.1 kHz` and `bit-perfect` are the same size, the
same weight, the same ink and the same slot, so neither can read as the other's
verdict. Karl can find them; nobody else will ever notice them arrive. This is
the single most important tone decision in the product and it must not be
"improved" with a colour, a badge or a lock icon.

### 4.10 Settings surface

The section shape is the template for every setting baz will ever have.

```
heading    SIZE_EMPHASIS / 1.35 Medium  PAPER
sentence   SIZE_META     / 1.35         PAPER_DIM     one sentence, what it is for
controls   GAP_SM between them
note       SIZE_META     / 1.35         PAPER_FAINT   in a slot of SETTING_NOTE_H
readout    value SIZE_META Mono PAPER   over   detail SIZE_META PAPER_FAINT
```

- Sections are separated by `GAP_XL` (24) and live in one scroll with
  `scroll_gutter()`.
- **The note's slot is reserved, not fitted** (`SETTING_NOTE_H` = 2 ×
  `SIZE_META` × `LINE_HEIGHT` = 31.2). The sentence changes with the setting;
  a slot that grew would push the controls below it down by a line the moment
  somebody pressed a segment — taking a control out from under the pointer
  that had just chosen it.
- **A section may not** use the accent, use an icon, use colour to encode a
  value, or change height when its value changes.
- **A readout appears only when there is something true to report.** ReplayGain
  set to Off states no figure at all, because the engine performs no ReplayGain
  arithmetic in that mode and `0.00 dB` would describe arithmetic that is not
  happening.
- Controls that cannot act render disabled rather than pretending.

**Needs from its container:** ≥ 320 px of content width (the segmented control
with three labels is the widest thing in it) and a scroll region. Nothing about
the section assumes a rail.

### 4.11 Empty, loading and first-run

**baz has no spinner and no progress bar, anywhere.** During a scan the shelf
filling with covers *is* the progress indicator — that is the first-run promise
in `05-personas.md` §4, and a bar counting to 100% next to it would be
admitting the shelf is not enough.

Empty states are quiet centred text, never an illustration and never a call to
action: an empty queue is the ordinary state of a player nobody has pressed
play on, not a problem to solve.

| Surface | Line (`SIZE_EMPHASIS` `PAPER_DIM`) | Hint (`SIZE_META` `PAPER_FAINT`) |
|---|---|---|
| scanning, nothing yet | The shelf fills as the scan finds your music… | — |
| scanned, genuinely empty | No albums here yet | baz rescans this folder each time it starts |
| search, no match | Nothing matches “…” | Esc clears the search |
| queue, nothing queued | Nothing queued | Play an album and it appears here |

Both lines are centred, `GAP_SM` apart, in the surface's full area.

**First run.** One question, one field, one footnote, centred in an otherwise
empty `WALL`.

| Part | Spec |
|---|---|
| wordmark | `baz`, `SIZE_EMPHASIS` Mono **`PAPER_FAINT`** (**CHANGE**, was `LAMP` — nothing is playing, so the lamp is off) |
| question | `Where's your music?`, `SIZE_HERO` (32) / 1.15 SemiBold **Serif** `PAPER` |
| sub | `Point baz at a folder — the shelf fills as it scans.`, `SIZE_EMPHASIS` `PAPER_DIM` |
| field | 460 wide, `pad(GAP_SM + 2, GAP_MD)`, `SIZE_EMPHASIS`, §4.9 |
| error | `SIZE_META` `ALERT` |
| footnote | `SIZE_CAPTION` `PAPER_FAINT` |
| gaps | `GAP_SM` within the heading block, `GAP_XL` between blocks |

The serif question is the first thing anyone ever sees of baz, and it is the
only place other than an album title that the serif is allowed. That is the
whole of baz's branding: a well-set question in a dark room.

---

## 5. What iced 0.13 forces, and the fallback in each case

| The design wants | iced 0.13 | Fallback taken here |
|---|---|---|
| rounded or clipped artwork | `image` cannot be clipped or rounded | **square sleeves, embraced** — records are square; nothing in this document asks otherwise |
| a hover zoom on a cover | no per-widget transform in the tree; `Transformation` is translate + uniform scale only | the hover card *behind* the art is the affordance |
| an icon set | none ships | polygons in `icon.rs`; **this document adds no new icon**, and any future one must be closed polygons in a unit square (no strokes, no caps, no true arcs) |
| a single-sided border | `Border` is four-sided | `rule` widgets for single lines — already how the top and bottom bars are built |
| a focus ring on buttons | buttons take no keyboard focus | `PAPER_RING` applies to `text_input` only; tooltips carry every icon-only control's name |
| tabular figures | no OpenType feature control, no `tnum` | Mono for every figure that changes in place (§2.2.4) |
| transitions | no animation runtime; a `frames()` subscription redraws while idle | **0 ms everywhere** (§2.6), with the three permitted animations specified for whenever the runtime arrives |
| pointer capture during a drag | none — `Grab`/`Grabbing` are cursor pictures | already solved in `groove.rs`: end the gesture on `CursorLeft` / `Unfocused`, and commit rather than cancel |
| text ellipsis | `text` has no ellipsis mode | `Wrapping::None` clips; every clipping slot has a fixed width so the clip is predictable |
| radial gradients / blur / backdrop | linear gradients on `container` backgrounds only | the placeholder's linear gradient; nothing else needs one |
| shadow spread | `Shadow` has colour, offset, blur — no spread | tuned via blur (`SHELF_SHADOW`, `LAMP_GLOW`) |
| an accessibility tree | none | contrast floors (§2.1.2) and hit targets are the guarantees baz *can* make, so they are honoured exactly |

---

## 6. Performance

Performance is a design constraint here, not an afterthought. What this
document costs:

| Change | Per-frame cost | Other cost |
|---|---|---|
| bundled typeface | none | ≈ 250 KB (Latin subset) or ≈ 800 KB binary; one font load at startup |
| flexible cell width | none — arithmetic per layout pass, not per tile; the widget count per frame is unchanged (~40 live tiles) | `shelf.rs` constants become functions |
| contact shadow, brighter halo | none — the same `Shadow` primitive, drawn in the same quad pass | none |
| art-derived lamp | none | one 4k-sample histogram per **track change**; sub-millisecond; no new decode, no new I/O, no new crate |
| quieter placeholder + letterform | one extra `text` widget per art-less tile | none |
| shelf scrollbar styling | none | none |

**Forbidden by this specification, on performance grounds:** blur or backdrop
effects of any kind; any per-frame animation or idle redraw; artwork above
`THUMB_PX` in the shelf; per-tile gradients; shadows on anything that is not
artwork.

---

## 7. A light variant

Dark-first is correct for this audience and this room. What a light variant
would actually take, so the cost is known rather than guessed:

**Mechanical (about half a day).** Every `pub const Color` in `theme.rs`
becomes a field on a `Palette` struct resolved once at startup, and the ~20
style functions read from it instead of from module constants. `theme.rs` is
already the single source of every value, so nothing outside it changes.

**Not mechanical — three judgement calls that are real design work:**

1. **The halo stops working.** Amber glow on a paper ground has almost no
   contrast. The playing mark would have to become something else — a filled
   amber bar down the sleeve's left edge, or a heavier dot — which means the
   "this one" signal is *different* between themes, not merely recoloured.
2. **The sleeves need an edge.** On `WALL` a dark cover is separated from the
   room by its own shadow. On paper, dark covers punch and pale covers
   disappear, so every sleeve needs a 1 px `HAIRLINE` the dark variant
   deliberately does not have — which changes the "nothing between the covers"
   rule the shelf is built on.
3. **The inks are new values, not inversions.** Roughly: ink `#1A1714` on paper
   `#F4F1EC`, dim `#5E574E`, faint `#7C746A`, and `LAMP` must darken to about
   `#A9670F` to clear 4.5 : 1 on paper — at which point it is no longer an
   amplifier lamp, it is amber ink, and §1.2's derived-hue lamp needs a second
   lightness target.

**Recommendation: defer.** Do it when someone asks for it, with the three
decisions above made explicitly. Do not ship a light theme that is the dark one
with the numbers flipped.

---

## 8. Adoption order

Each step is independently shippable and independently revertible.

1. **The typeface** (§2.2) — the single biggest step from alpha-tier to
   designed, and the one whose absence is visible in every screenshot. Ship it
   with the bottom-bar slot-width test (§4.6.5).
2. **The accent cuts** (§2.1.1) — focus ring, scanning note. Two lines.
3. **The contrast fixes** (§2.1.2) — two constants.
4. **The shadow and halo** (§2.1, §4.1) — three numbers.
5. **The shelf grid** (§4.2) and the shelf scrollbar — the only change that
   touches geometry code.
6. **Captions and the placeholder** (§3.2, §3.4.3).
7. **`SIZE_TITLE` / `SIZE_HERO` and the serif** (§2.2.2–3).
8. **The derived lamp** (§3.3) — last, because it is the only one that is a
   feature rather than a restyle, and because everything above must be true
   before it means anything.

---

## 9. The pictures

In `docs/design/visual/`. The `*-current-*.png` files are the shipped binary;
the `mock-*` files are this specification drawn to its own tokens.

| File | What it is |
|---|---|
| `00-current-first-run.png` | before — the first-run screen, hero line in a fallback monospace |
| `01-current-shelf.png` | before — the shelf; note the fallback faces, the dead gutters, the stock scrollbar, and the amber focus ring with nothing playing |
| `02-current-shelf-hover.png` | before — tile hover |
| `03-current-album-panel.png` | before — the album panel in the 340 px rail |
| `04-current-queue-empty.png` | before — the queue's empty state |
| `05-current-settings-panel.png` | before — the settings panel and the ReplayGain section |
| `06-current-search-results.png` | before — a narrowing search |
| `07-current-search-empty.png` | before — no matches |
| `08-current-shelf-scrolled.png` | before — deeper into the collection |
| `tokens.svg` / `.png` | the palette, the type scale, the spacing and radii |
| `mock-shelf.svg` / `.png` | after — the shelf, with one album playing and one hovered |
| `mock-album-detail.svg` / `.png` | after — the album surface, drawn full-width so it assumes no container |
| `mock-now-playing.svg` / `.png` | after — the bar at 2×, with its reserved slots called out |
| `mock-states.svg` / `.png` | after — every component in every state |

The mockups embed the real generated fixture cover art, so the wall of covers
can be judged rather than imagined. Their font stack names IBM Plex first; the
committed PNG renders fall back to what the build host had installed (Adwaita
Sans, Source Code Pro, Liberation Serif), so treat the PNGs as correct for
layout, colour and size, and the SVGs as correct for type.
