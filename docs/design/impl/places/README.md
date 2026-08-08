# ADR-0022 — places, and nothing else

Every frame here is the **real binary**, rendered headless by
[`capture.sh`](capture.sh) with all six XDG redirections from
`docs/DEVELOPMENT.md`. The run's receipt that it did not touch the owner's
desktop:

```
[startup] room: Closing Time
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Silent fixtures (`composition/tools/mkfixture.sh` writes zeros) and an
`.asoundrc` routing ALSA's default PCM to `null` — two independent guarantees
that nothing was audible. The script cleans up **only the pids it started**,
never by name.

## The frames

| | |
|---|---|
| [`01-wall-no-scrollbar`](01-wall-no-scrollbar.png) | the wall, and the right-hand edge: the index rail with nothing beside it |
| [`02-album-place`](02-album-place.png) | one press on a tile, and the record's page has replaced the wall |
| [`03-album-playing`](03-album-playing.png) | `Play album` — the control that replaced the wall's double-click |
| [`04-wall-after-esc`](04-wall-after-esc.png) | `Esc` back: the scroll is where it was, and the record carries the 2 px rule |
| [`05-queue-place`](05-queue-place.png) | the queue at the width of the window, by the bar's labelled door |
| [`06-queue-row-hovered`](06-queue-row-hovered.png) | a row's ✕, offered on hover, in a slot reserved either way |
| [`07-wall-scrolled-away`](07-wall-scrolled-away.png) | the wall, scrolled off the sounding record |
| [`08-back-to-playing`](08-back-to-playing.png) | **R3**: the bar's now-playing text pressed, landing on the playing record's page |
| [`09-settings-place`](09-settings-place.png) | the Settings place, for comparing the three headers as one frame |
| [`10`–`13`](10-wall-1920.png) | all four places again at 1920 × 1080 |

## The composition tables

Measured by [`measure.py`](measure.py), which spends the rulers committed at
`docs/design/composition/tools/`. Reproduce with:

```sh
toolbox run -c baz-dev docs/design/impl/places/capture.sh
toolbox run -c baz-dev python3 docs/design/impl/places/measure.py
```

### L1 — one gutter per window edge

`HANG` is 40, so a 1280 px window wants ink between **40** and **1240**.

| surface | top strip | body | bottom bar |
|---|---|---|---|
| the wall (1280) | 40 … 1221 | **40 … 1240** | 41 … 1240 |
| the record's page (1280) | 48 … 1240 | **40 … 1230** | 41 … 1240 |
| the queue place (1280) | 48 … 1240 | 195 … 1047 | 40 … 1240 |
| the record's page (1920) | 48 … 1880 | 343 … 1567 | 41 … 1880 |

Three readings need their honest gloss:

- **48, not 40, on a place's top strip.** The `‹ Library` word button's *box*
  hangs from 40; its ink starts 8 px in, because a word button carries `GAP_SM`
  of padding — the same 8 px the top bar's `Settings` has always had. The edge
  the law is about is the box's.
- **1230, not 1240, on the record's page.** That is `SCROLLBAR_LANE` 10, the one
  inset law L5 permits on a right edge, and it is *declared* rather than
  absorbed: reserved whether or not the page overflows, so a fourteenth track
  arriving shunts no duration sideways.
- **195 … 1047 on the queue, and 343 … 1567 at 1920.** The capped-and-centred
  rule: a place's body grows with the window until its list reaches
  `LIST_MEASURE` 880 and then stops and centres. At 1280 the queue's list is
  already at the cap (1190 available), so it centres; the record's page is under
  it and hangs from both gutters.

The bottom furniture is **83** in every place — 80 band + 1 hairline + 2 needle.

### L4 — one centre line per bar

The band is 80 px and its mid-line is **818.5**.

| mark | centre | lines | Δ |
|---|---:|---:|---:|
| the now-playing block | 817.75 | 3 | −0.75 |
| elapsed / total | 819.00 | 1 | +0.50 |
| the `Queue` door | 818.00 | 1 | −0.50 |
| the transport | 818.00 | 1 | −0.50 |
| the signal note | 819.50 | 1 | +1.00 |
| the volume rail | 818.00 | 2 | −0.50 |

**Spread 1.75 px**, against the law's ceiling of 2 — re-derived at 80 px rather
than carried over from 56, and from 102 before that, where the audit measured
**50**.

Two measurement notes, because both were wrong before they were right:

- The volume's mark is its **rail**, and the unity detent is a deliberate 5 px
  mark 2 px above it. A run-joiner that tolerates a 2 px gap merges the two into
  one block whose centre is neither, and reads 815.00.
- The pointer must be parked **clear of the needle**. The needle runs the full
  width of the window's bottom edge and shows a hover tip, so a pointer resting
  in the bottom-right corner puts a floating label into every frame, which the
  ruler then measures as a seventh mark 31 px low.

### L5 — the permitted alignment edges

| surface | x-edges | which |
|---|---:|---|
| the record's page, the aside | **2** | 40, 384 |
| the queue place, the rows | **1** | 195 |

Against the audit's **8 distinct x-edges in the inspector's 340 px column, 5 of
them singletons**, and **four left edges in the popover's 358 px**. The rows in
both places carry no horizontal inset of their own, which is the literal the
unit test pins.

### L6 — declared, then measured

Declared: **the work ≫ `Play album` → the title → the artist → the catalogue
line → the track list → the condition**, and among *type* the title is first.

Contrast-weighted ink mass, two ways, because they answer different questions.
*Mass* is what §13 says and it ranks a twelve-row list above a one-word title
for the trivial reason that it is twelve lines; *mass per line* is loudness, and
loudness is what the audit's defect 5 was about.

| | mass | share | per line |
|---|---:|---:|---:|
| the work (sleeve) | 13 322 759 | 88.5 % | 13 322 759 |
| the track list | 780 417 | 5.2 % | 65 035 |
| the condition (`Details`) | 464 420 | 3.1 % | 35 725 |
| `Play album` | 178 767 | 1.2 % | 178 767 |
| **the title** | 135 544 | 0.9 % | **135 544** |
| the artist | 90 063 | 0.6 % | 90 063 |
| the catalogue line | 77 335 | 0.5 % | 77 335 |

By loudness the measured order is **work → `Play album` → title → artist →
catalogue → track row → `Details` row**, which is the declaration exactly.

`Play album` outranking the title is not an inversion: it is a 1 px amber border
around a 320 × 32 box — 704 px of full-contrast accent — against five glyphs of
28 px type, and it is the one commitment the page makes.

**The album's own name went from fifth of eight in the 340 px column to first
among type on its own page.** That is the whole of what the audit's defect 5
asked for, and it is why the sleeve being 88.5 % here is not the same finding:
in the column the sleeve was a second, larger copy of a work already on the wall
24 px to the left. Here there is no other copy.
