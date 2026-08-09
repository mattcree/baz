# The search well, in the returns lane

Rendered from the real binary under Xvfb with the six XDG redirections of
`docs/DEVELOPMENT.md`; `capture.sh` regenerates every frame here and prints the
`[mpris] no session bus` receipt that says the owner's session was not touched.
Nothing was audible: the sink discards every sample and the fixture's samples
are all zero.

The owner's brief, verbatim: *"the design does not match properly… the search
should really be in the sidebar"*.

---

## What was decided, and why it is a field and not a destination

Spotify — the reference the owner keeps naming — makes **Search** a destination
you navigate to. baz must not, and the reason is a feature baz has that Spotify
does not: **type-anywhere** (ADR-0017 §1.2). Any printable key filters the wall
from anywhere, so the query is already open before you have decided to search. A
destination row would say *go somewhere first*, which is the opposite of what
the product does, and it would leave the thing the keystroke actually fills —
the field — somewhere else on screen.

The well is also as much a **readout** as an input, and a readout of the frame's
own state belongs in the frame's own resident surface. It was the last piece of
the frame still in the strip. With it moved, the strip carries no identity at
all — it is the wall's arrangement and the wall's verbs — and the eye has one
place to start, which is what *"does not match properly"* was about.

So: **the well is the fourth row of the lane's head**, under `Home`, `Library`
and `Now playing`, above the hairline. `Home` stays at the top; the owner said
so twice.

---

## The frames

| Frame | What it shows |
|---|---|
| `01-lane-well-at-rest-1280` | The well in the head. Placeholder `Search`, `25 albums · 206 tracks` on the line under it. Its mark stands on the destinations' glyph vertical and its text on their word vertical. |
| `02-well-focused-by-slash-1280` | <kbd>/</kbd> puts the caret in it. The ring is the only focus state in the product. |
| `03-well-mid-query-1280` | `an`. The readout becomes `16 of 25 albums`; the `Songs` section takes the head of the wall's body (doc 09 §5). |
| `04-strip-after-the-move-1280` | The strip, cropped: five state words, three act words, the gear. No well, no doors. |
| `05-esc-blurs-the-well-first-1280` | The first <kbd>Esc</kbd> is **iced's**, not baz's — `text_input` handles it by unfocusing and capturing. The query survives. |
| `06-esc-then-peels-the-query-1280` | The second reaches `crate::keys` and peels. Placeholder and counts return. |
| `07-lane-collapsed-1280` | 96 px: the magnifier as the head's fourth mark, in the destinations' anatomy, tooltipped `Search`. |
| `08-typed-from-the-rail-opened-the-lane-1280` | One keystroke from the rail opens the lane and lands the caret. One frame, no tween. |
| `09-collapsed-mark-lit-by-a-live-query-1280` | Collapsed under a live query: the mark takes the lit ink. The one thing 96 px can say about the wall's state without a word. |
| `10-collapsed-magnifier-opened-the-lane-1280` | Pressing the mark opens the lane back onto the caret. |
| `11-home-before-typing-1280` | `Place::Home`, lane collapsed. |
| `12-typed-from-home-lands-in-the-lane-1280` | …and one letter brings the Library back **and** fills the well. Before the move this filled a field that was not on screen and narrowed a wall that was not either. |
| `13-well-back-in-the-strip-980` | Below `SIDEBAR_FLOOR` the lane cannot open, so the strip takes the well back in doc 10 §4.1's exact form — counts as the placeholder, match count in the reserved slot. Strip 884 ≥ 872: one line. |
| `14-strip-splits-with-the-well-900` | Strip 804 < 872: the two-line regime, unchanged. |
| `20`–`22` | The same three states at 1920 × 1080. |

---

## The two figures, re-homed — and the arithmetic that forced it

In the strip the counts were the **placeholder** and the match count sat in a
reserved `MATCH_W` 88 slot **inside** the field (doc 10 §4.1). Neither survives
the move, and it is arithmetic rather than taste:

```
the strip's well                    280
  − text inset (12 + 16 + 8)         36
  − reserved match slot (12 + 88)   100
  = the query's own lane            144

the lane's well  (SIDEBAR_MEASURE)  232
  − text inset (SIDEBAR_HEAD_TEXT_X) 44
  − reserved match slot (12 + 88)   100
  = the query's own lane             88     ← a third of what §4.1 sized for
```

So both figures come out of the field and onto one line under it, where each
gets the whole 176 px measure and the query gets the whole field:

- **at rest** — `25 albums · 206 tracks`. The corpus, under the glyph that says
  *search this*: L8.3's valve exactly as the placeholder was.
- **narrowing** — `16 of 25 albums`. The caption returns, because outside the
  control being typed into `16 / 25` is a figure with no subject. Doc 07 §3.1's
  own words in doc 10's position.

The line is **always drawn** and **left-aligned**, which is the reserved-slot
discipline in its cheaper form: the first character never moves, the tail
shortens, and no `RECENT` row is pushed down by a keystroke.

Where the strip still draws the well — below `SIDEBAR_FLOOR` — it keeps the
in-well anatomy unchanged, because a strip is one control tall and has no second
line to give. **Two forms, never two at once**, and the breakpoint is the lane's
own floor rather than a second one.

---

## What the head costs the list

The well's block is `SIDEBAR_WELL_H` = 32 + 4 + 16 = **52**, plus a `GAP_SM`
lead: 60 px, which is most of one `SIDEBAR_ROW_H`. Stated rather than hidden:

| Window | `RECENT` rows visible, before | after |
|---|---:|---:|
| 1280 × 860 | 7 | **6** |
| 1920 × 1080 | 11 | **10** |

`(H − bar 83 − 2 × GAP_XL 48 − head − rule 25 − heading 36 − marks 48) / 64`,
where `head` is `3 × 40` and now `+ GAP_SM + 52`.

---

## The strip's budget, re-derived

`theme.rs`'s `the_strip_holds_its_tenants_at_the_single_line_floor` holds all of
this as const arithmetic; the numbers are:

```
with the well (below SIDEBAR_FLOOR — the strip still draws it):
  40 + well 200 + 24 + keys 314 + 24 + acts 182 + 16 + gear 32 + 40  =  872
                                                       TOP_BAR_SPLIT = 872

without it (SIDEBAR_FLOOR and above — the lane draws it):
  40 +                keys 314 + 24 + acts 182 + 16 + gear 32 + 40  =  648
  narrowest strip that regime can produce: SIDEBAR_FLOOR − SIDEBAR_W = 720
                                                             648 < 720
```

Three findings fall out of it:

1. **`TOP_BAR_SPLIT` becomes 872 and becomes exact.** It was 960 for a line of
   958 — a rounded seam. The `Playlists` door's 64 px and its `GAP_XL` went with
   ADR-0030 §5, and what is left comes to 872 on the nose.
2. **The two-line split still earns its keep, and only just.** Not because of
   the frame line — the door and the well both stood on that, and it is now
   328 px — but because of the **library** line, whose two tenants are untouched
   at 600. Between 600 and 872 there is no single line that fits and a two-line
   pair that does. That band is exactly the band the well is still a tenant of.
3. **Above `SIDEBAR_FLOOR` the strip is one line at every width, in either lane
   state**, and that is asserted rather than assumed.

Two deletions the re-derivation forced:

- **`well_width`'s fluid range.** `clamp(W − 1000, 200, 280)` could only be
  climbed between strip widths 1200 and 1280, and the strip is never that wide
  while the well is in it. `WELL_W` is a flat 200, and **the split is now the
  whole of the collapse order** rather than its second step.
- **`top_bar_h(width)`.** It is `top_bar_h(window_w, lane_open)` now. The strip
  is drawn at `App::body_width` while this function was still being handed the
  *window*, so between a 1000 and a 1056 px window with the lane open the strip
  drew two lines and the virtualizer's viewport estimate assumed one — 40 px of
  mis-virtualized shelf, in a band nothing had looked at.

---

## Two defects found against `docs/design/impl/lane-and-home/`, and one not

Comparing the shipped frames against doc 13 and ADR-0030:

- **A lane row's sleeve was 40 px, not 48.** `lane_row` read `PANEL_SLEEVE` when
  the lane was open — the *playlist panel's* sleeve — so `SIDEBAR_ROW_H`'s own
  derivation (48 with one `GAP_SM` above and below) described a row nothing was
  drawing, and doc 13 §9.2's window drawing states 48. Measured off
  `01-lane-open-1280.png` at 40 × 40. Fixed; `01-lane-well-at-rest-1280.png`
  measures 48.
- **A playlist's quotations were never asked for.** A list's sleeve is a collage
  of the records it quotes, read out of the wall's thumbnail cache — and nothing
  put those records *into* it. `Shelf::offscreen_art` yields the lane's records
  and skips its lists. So a list drew the deterministic gradient until one of the
  records it quotes happened to scroll onto the wall: real artwork **by luck**,
  which is not what ADR-0030 §2 claims. The shell now names them, four per list,
  on the same guard.
- **The lane could not say which record was sounding.** Doc 13 §2.6: *"the
  playing record takes the lamp dot before its name and the halo around its
  sleeve."* Every lane row drew `theme::track_row` with `playing` hard-coded
  `false`, so the surface whose whole subject is *things you have touched* said
  nothing about which of them was on —
  `lane-and-home/03-lane-open-sounding-1280.png` shows `Ochre` sounding, in the
  bar and on the wall, and unmarked in the lane. It takes the **row's**
  vocabulary now: the dot before the name and the card the sounding row keeps
  whatever the pointer is doing, which is what the queue and a playlist's page
  already draw, and which is the one mark that survives the collapse. Not the
  tile's warmed halo — that wants the lamp's own clock in a surface ADR-0030 §4
  costs at zero idle CPU. Visible in `01` and `07` here.
- **The "empty sleeve squares" were not a defect.** In `01-lane-open-1280.png`
  the three lists wear a near-black tile with one faint bar, and all three are
  pixel-identical. That is real artwork: `mkfixture.sh`'s **`mono`** family,
  *"near-black monolith with one faint mark: the black-sleeve case"*, scaled from
  600 px to 48. All three lists were built with `find | sort | head -n`, so all
  three began at the same record — and a list with fewer than four distinct
  records draws that record's sleeve full-bleed rather than a collage. The
  fixture here strides the file list instead (`awk 'NR % 9 == offset'`), which is
  what makes the collage visible in these frames. **The one real absence in that
  frame — no records among the lists — is also the fixture**: it is the first
  launch, the ledger is empty, and nothing had been played yet.
  `03-lane-open-sounding-1280.png` from the same run shows the mix.
