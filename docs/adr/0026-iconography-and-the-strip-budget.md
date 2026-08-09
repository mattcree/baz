# ADR-0026: Iconography by rule, and the strip budget law

**Status**: proposed (2026-08-09) · extracts the decisions of
[`docs/design/10-controls-and-iconography.md`](../design/10-controls-and-iconography.md) ·
changes no engine command, no protocol message, no control's surface, and
no gesture — every change is a control's *form* or the strip's
*arrangement* · amends `.interface-design/system.md` §13 (one clause of
L8.4; adds L9) · extends `crates/baz/src/icon.rs`'s glyph set within its
shipped mechanism · sibling of doc 07's L8, at the next scale down: L8
decides which surface a control lives on; this record decides what it
wears and how many fit · the owner's brief, verbatim: *"just adding stuff
into that top bar isn't good. I just find we need to have a proper think
about how we lay out controls and what is intuitive. we are also using
quite poor HCI due to almost no icons etc."*

## Context

The Library strip holds ten controls and three readouts, all words in one
line. Measured off the shipped render at the default 1280 px window
(`docs/design/impl/queue-parity/01-strip-play-all-1280x860.png`), content
and gutters claim ≈ 1 249 px — 97.6 % of the strip — leaving ≈ 31 px of
slack; the transient scan notes need ≈ 140 px more, so **the strip
already overflows at the shipped window whenever a scan with skipped
files is running**, pushing `Settings` past the window's edge. The
strip's view takes no window width (`views/top_bar.rs:39`) and nothing in
it clips, so no responsive behaviour exists even in principle.

The cause is structural, and it is the rail's disease at a smaller
scale: doc 07's L8 admits controls to the strip by *subject*, each
admission locally argued, and no law has ever bounded a surface's
population or governed a control's form. Meanwhile the product's
iconography is inconsistent rather than absent: seven drawn glyphs carry
the transport, the speaker and the ✕ (`crates/baz/src/icon.rs:97–112`),
while the two most conventionally iconic controls in software — search
and settings — are a borderless box and an 84 px word, and the row-edit
slots mix drawn glyphs with borrowed font characters at different stroke
weights in one row (`views/queue.rs:480–550`).

## Decision

### 1. The form rule

> A control is drawn as an **icon alone** only when all three hold:
> (1) the symbol is universal across the products and platforms this
> audience arrives from; (2) baz's semantics are the convention's
> semantics exactly — a symbol whose convention promises a mode may not
> label an act; (3) the control's meaning is stable in every state.
> Icon-only controls carry a tooltip (the accessible name, ADR-0017 §4c)
> and the L7 hit floor. Where the convention is close but not exact, the
> **word stays and may carry the glyph as its leading mark**. Where no
> convention exists, the control is a **word** — an invented icon is a
> private code; a word never needs a legend.

Applied (the full table is doc 10 §3.2): transport, speaker, ✕, `+`,
▲▼ — icon-only (the first three shipped; the last two promoted from
font characters, §4). Search — the magnifier marks the well; the well
itself stays (ADR-0017 §1.2). Settings — the gear, icon-only (§2).
`Play all` — glyph + word, `Play album`'s shipped anatomy without the
accent. `Shuffle` — **word**, with the sharpened reason recorded: the
crossed-arrows convention is a mode toggle with a lit state, and baz's
shuffle is a bounded draw that ends in silence (`REFUSALS.md:19–34`);
wearing the mode's symbol would claim a mode the product refuses.
`Pull`, the group keys, every door (`Queue`, `Playlists`, `‹ Library`,
`Back`), the page word-acts, and every readout — words or bare type,
unchanged; readouts keep `02` §6.7's *no icon, no background, never the
accent* verbatim.

### 2. One named exception to L8.4

L8.4(1) — *a door is labelled with the name of the place, in words* — is
amended, not weakened:

> A door is labelled with the name of what it opens — in words, **or by
> its universal symbol where one exists. The symbols that count as
> labels are enumerated, and the list is two: the gear (Settings) and
> the magnifier (search).** A door whose symbol is merely familiar
> keeps its word.

The clause's evidence (Sonos, Plexamp — `03` §4.5, §5.2e) concerns
unlabelled and unconventional routes; the gear in the top-right corner
is the most standardized door in software in both symbol and position.
`Queue` and `Playlists` were considered for the list and refused: no
universal symbol separates queue from playlist from menu, and a door
that can be misread is worse than a door that must be read. The list is
closed the way the contrast exemption list is (ADR-0017 §1.6): a new
name requires an ADR that argues it.

### 3. The strip's charter, and L9 — the budget law

> **L9 — A strip declares its tenants and holds them at its floor.**
> Every strip enumerates its tenants' reserved widths; their sum plus
> the frame's gutters must fit the strip's declared single-line floor,
> and the sum is asserted in code. When the window is narrower than the
> floor, the strip **splits at its declared seam** into enumerated
> lines, each with its own asserted floor. A strip never hides a
> tenant, never sweeps one into a menu, and never overflows. A proposed
> tenant that does not fit the budget does not enter — it re-homes by
> subject (L8) or displaces an argued incumbent.

The Library strip's charter under it: **the frame line** (the well with
the magnifier and the counts, the `Playlists` door, the gear) and **the
library line** (the five arrangement words; `Play all` with the
triangle, `Shuffle`, `Pull`) — one line at ≥ 960 px, two lines below,
floor 600. Numbers, lane widths and the mockups are doc 10 §4; the
assertion takes the const-arithmetic shape the bar already uses
(`views/bottom_bar.rs:874–885`).

### 4. The counts fold into the well; one mark technology

- **The collection counts become the search well's placeholder** — the
  one lane that is empty exactly when the query is, describing exactly
  the corpus the well searches — and the **match count takes a reserved
  slot inside the well** (`7 / 1 284`), discharging doc 07 §3.1's
  prescribed move. The status row keeps only the transient scan /
  skipped / problem notes, beside the gear. L8.6 holds: each fact is
  stated once.
- **A control slot carries a drawn glyph or a word, never a borrowed
  font character.** `icon.rs` grows by five — `Magnifier`, `Gear`,
  `Plus`, `ArrowUp`, `ArrowDown` — in the shipped mechanism (closed
  polygons, supersampled once, theme-inked, no dependency; rings drawn
  as keyhole outlines under the existing even-odd test,
  `icon.rs:372–395`). The reorder and transfer slots and the settings
  steppers swap U+2191/U+2193/`+`/`−` for the drawn set; font
  characters remain legitimate inside *labels* (`‹ Library`, U+2212 in
  a figure), because a label is type. New glyphs keep the shipped
  stroke band (0.14–0.15 of the unit square), the one opacity ladder
  (`theme.rs:1404–1459`), and per-glyph coverage tests.

### 5. What is deliberately unchanged

The now-playing bar, in full (the ratchet, `REFUSALS.md:87–92`, and the
form rule passing it as shipped); every door that is a word; the group
keys; every gesture, message, key binding and context menu; the accent
discipline (no new glyph touches the lamp); the panel, the pages, and
every row's slot geometry.

## Consequences

- **The strip's arithmetic reverses.** At 1280: the counts' ≈ 120 px,
  `Settings`' 52 px and the well's 80 px return; `Play all`'s triangle
  spends 26. Slack goes from ≈ 31 px to ≈ 225 px, the scan-note
  overflow becomes impossible above the floor, and `top_bar::view`
  gains the `window_width` parameter every regime needs.
- **Five glyphs, two of them rings**, land in `icon.rs` with tests; the
  sprite sheet grows 7 → 12 with no new crate, asset or licence.
- **Three tests pin the new laws**:
  `theme::the_strip_holds_its_tenants_at_the_single_line_floor` (L9 as
  const arithmetic), `every_icon_only_control_carries_a_tooltip`
  (source-pinned, the `queue.rs:647–705` shape), and stroke-band
  assertions per glyph. The composited-ink contrast sweep (ADR-0017
  §1.6) governs the new marks by construction — they are the existing
  glyph ink.
- **`.interface-design/system.md` §13** gains L9 and the L8.4
  amendment; doc 07's `placement.rs` table is untouched, because no
  control changed homes.
- **Two tokens change, one dies, one pair arrives**: `SEARCH_W` 360 →
  fluid `WELL_W` 200–280; `SETTINGS_TOGGLE_W` deleted
  (`theme.rs:2877`); `TOP_BAR_H` 49 joined by `TOP_BAR_2LINE_H` 89
  with the 960 / 600 breakpoints.
- Implementation is doc 10 §7's eight steps, each whole; steps 1–2
  (the gear, the magnifier, the counts fold) are the relief and ship
  first.

## Considered and rejected

Recorded so re-proposal meets the argument (full list, doc 10 §6): an
overflow / hamburger menu (no overflow exists to sweep into, doc 07
§0.1, and a menu holding acts breaks doc 09 §5.2's mirror rule); a
customize-the-toolbar preference (`03` §4.3's tradition, and a hidden
control violates the visible-control rule for whoever hid it); hiding
controls behind hover (refused, `REFUSALS.md:147–149`); a group-key
dropdown (refused by name, `REFUSALS.md:70–72`); crossed-arrows for
`Shuffle` and a die for `Pull` (§1's semantics clause; the
recommendation costume); icons for the `Queue`/`Playlists` doors (§2);
icon + word everywhere (a hedge, not a statement); an icon font or SVG
pipeline (`icon.rs:12–50`'s survey stands); a permanent two-line strip
(40 px of the collection at rest, forever, to avoid one breakpoint).
