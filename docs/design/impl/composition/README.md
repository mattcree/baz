# Applying the composition audit — measured, before and after

[`docs/design/06-composition-audit.md`](../../06-composition-audit.md) measured
fifteen ranked defects and proposed seven composition laws. This directory is
what happened when they were applied: the same rulers, over the same fixture, on
two builds — the merge this branch started from and the merge it produced.

**Nothing here is asserted.** Every number is read off a committed PNG by
[`composition/tools/`](../../composition/tools/), and both columns come from the
same run of the same script, so the comparison is like for like.

## How it was measured

The real release build, `--features device-output`, on a private `Xvfb`, with
all six redirections from [`DEVELOPMENT.md`](../../../DEVELOPMENT.md). Every run
carries the receipt:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

The fixture is the audit's own: 25 albums / 206 tracks of **digitally silent**
FLAC with generated covers, a scratch `HOME` whose `.asoundrc` routes ALSA's
default PCM to `null`, and `BAZ_DEVICE_TESTS` never set. 32 frames — 16 states ×
1280 × 860 and 1920 × 1080 — in [`shots/`](shots/); the pre-change frames the
"before" column is read from are in [`before/`](before/); the raw ruler output
for both is in [`census/`](census/).

### Three corrections to the rulers themselves

The audit's tools carried the *old* geometry as constants, so they had to be
taught to read what they were measuring. All three are in the tools' own diffs:

- the top strip's height and the body's top were hardcoded at 53; they are found
  from the strip's own hairline now;
- the panel's left edge, the popover's surface and the search well's extent were
  probed at rows a fixed bar height put them at;
- **the capture script pressed `q` to open the queue and clicked a hardcoded
  pixel to start playback.** iced 0.13's `text_input` keeps focus until a click
  lands elsewhere and the search well takes focus at launch, so `q` went into the
  *field*; and the click coordinate was a fixture of a layout that had since
  moved. Both failed silently — the "playing" frames were idle bars and the
  "queue" frames were search results. The script blurs the field first and finds
  the album by name now.

---

## 1. Alignment edges — the window gutters

The audit's highest-yield measurement: **three window gutters in one
application**, so nothing in either bar aligned with anything on the wall.

| surface | before, L / R | after, L / R |
|---|---:|---:|
| top bar | 16 / 1264 | **40 / 1240** |
| bottom bar | 16 / 1264 | **40 / 1240** |
| the wall (works) | 40 / 1240 | 40 / 1240 |
| the Settings place | 24 / 1264 | **40 / 1240** |
| the index rail's right edge | 1264 | **1240** |
| **distinct window gutters** | **3** (16 · 24 · 40) | **1** (`HANG` 40) |

At 1920 the same, offset the same way: 16 / 24 / 40 · 1904 / 1904 / 1880 before,
40 · 1880 after.

The wall's top-bar edge set, at 1280:

| | before | after |
|---|---|---|
| edges | 16 · 376 · 404…705 · 1048 · 1172 · 1200 · 1245 · 1264 | 40 · 400 · 428…729 · 1024 · 1148 · 1180 · 1221 · 1240 |
| shared with the collection | **0** | **2** (40 and 1240) |

## 2. The bar's centre line

Ink centres of each zone's primary mark, against the bar's own mid-line. The
seek row is listed but excluded from the spread: it hangs below the line by
design, and ADR-0017 step 10 deletes it.

| mark | before (1280) | after (1280) | before (1920) | after (1920) |
|---|---:|---:|---:|---:|
| transport glyph centres | **−22.5** | **0.0** | **−22.5** | **0.0** |
| volume rail centre | **+6.5** | **0.0** | **+6.5** | **0.0** |
| mute glyph centre | +0.5 | 0.0 | +0.5 | 0.0 |
| now-playing line 2 | +1.0 | +0.5 | +1.0 | +0.5 |
| `Queue` label | +1.5 | +2.0 | — | — |
| signal note | +1.0 | +1.5 | +1.0 | +1.5 |
| **spread, excluding the groove** | **29.0 px** | **2.0 px** | **29.0 px** | **2.0 px** |
| seek groove (hangs below) | +27.5 | +36.0 | +27.5 | +36.0 |

The bar's own height: **102 → 105** (a 104 px band and its hairline), and the
top strip **53 → 49**. The body's share of the window is 0.8198 → **0.8209**.

## 3. Vertical rhythm

Share of drawn chrome y-edges within ±1 px of a lattice of unit *u*. A random
set scores about 3/*u*, which is the null row.

| surface (1280) | n before → after | u=4 before | u=4 after |
|---|---:|---:|---:|
| top bar | 2 → 2 | 100 % | 100 % |
| bottom bar, idle | 4 → 4 | 100 % | 75 % |
| bottom bar, playing | 6 → 8 | 83 % | **100 %** |
| album inspector | 26 → 28 | 88 % | **93 %** |
| Settings place | 12 → 12 | 92 % | **100 %** |
| queue popover | 30 → 30 | 87 % | 83 % |
| tile column | 12 → 12 | 92 % | 92 % |
| **pooled** | **87 → 89** | **78 %** | **80 %** |
| *chance* | | *75 %* | *75 %* |

At 1920 the pooled 4-lattice reads **89 %** (n = 71) after; the before column at
that width is missing because the ruler's popover probe failed on the pre-change
frame, and it is not reconstructed rather than guessed.

**This is the correction with the least measured yield, and the reason is worth
stating.** The ruler counts *ink* y-edges — the top and bottom of each glyph run
— and an ink edge is a property of the typeface's ascender and descender inside
the line box, not of the layout. Roughly half the pooled sample is therefore not
a layout decision at all, which caps how far the pooled figure can move. What
did move is every surface whose sample is mostly *slots*: the playing bar to
100 %, the Settings place to 100 %, the inspector to 93 %. The queue popover,
which is almost entirely rows of type, moved the other way by 4 points.

The cause the audit named is closed at the token level, and that is the part a
test can hold: the six line boxes were 15.95 · 16.20 · 18.20 · 20.25 · 22.80 ·
32.20 and are now 16 · 16 · 20 · 20 · 24 · 32, with the leading derived from the
box rather than the box from the leading (law L2).

### The offenders the audit listed

| element | before | after |
|---|---|---|
| inspector caption pitches | 26 · 24 · 20 (three, one measure intended) | 28 · 23 · 20 |
| inspector track rows | pitch 28.6, ±1.4 accumulating | pitch **32**, exact |
| queue rows | pitch 28.14 | pitch **32**, exact |
| search well | h **30**, specified 32 | h **32** |
| `Play album` box | h 33 | h **32** |
| wall label block `LABEL_H` | 36.4 | **40 = `HANG`**; tile pitch `art + 96` |
| shelf-break band | 40 = 14 ink + 26 air | 40 = **12** ink + **28** air |

## 4. Optical centring

| element | before Δ | after Δ |
|---|---:|---:|
| `Settings` label, y, in its 32 px box | **−6.4** | **+1.5** |
| `Settings` against the counts line it shares a row with | **6.0 px apart** | **1.0 px apart** |
| `Play album` label + triangle, x, in a 292 px button | **−83.3** | **+3.5** |
| `Play album` label + triangle, y, in a 32 px button | **−6.4** | **−1.0** |
| Previous / Play / Next glyph centroids | 0.0 / −0.7 / 0.0 | 0.0 / −0.7 / 0.0 |
| first-run block centre, as a share of H | **0.501** | **0.423** |
| first-run ink centroid against the block's centre, x | **−92.7** | **−48.9** |
| first-run block width against its longest line | 460 vs 363 (**97 px of slack**) | 360 vs 332 (**28 px**) |

Every glyph in a hit box was already centred to a pixel, before and after —
which is what made the two failures a locatable mistake rather than a habit.

## 5. Information hierarchy — the album inspector

Contrast-weighted ink mass (area × contrast) over the panel's named regions.

| element | before | after (1280) | after (1920) |
|---|---:|---:|---:|
| the sleeve | **95.00 %** | **79.48 %** | **71.90 %** |
| the track list | 1.74 % | **9.68 %** | 9.30 % |
| `Play album` | 1.11 % | 2.10 % | 1.96 % |
| catalogue + condition | 0.44 % | 1.81 % | 1.64 % |
| **the title** | **0.87 %** | **1.34 %** | 1.21 % |
| the artist | 0.44 % | 1.15 % | 1.04 % |
| the footnote | 0.34 % | 1.44 % | 2.38 % |
| the close ✕ | 0.07 % | 0.28 % | 0.25 % |
| **sleeve : title** | **110 : 1** | **59 : 1** | **59 : 1** |
| **title + artist + track list** | **3.05 %** | **12.17 %** | **11.55 %** |

The sleeve is capped at 120 px (`INSPECTOR_SLEEVE`) and its share falls as the
audit's arithmetic said it would. **The declared order is still not the measured
order**: the title is sixth of eight rather than first, because a bright 120 px
cover is still four times the ink of a 19 px line of type. Stated rather than
smoothed over — see §8 below.

## 6. Density — ink against ground

| surface | before 1280 | after 1280 | before 1920 | after 1920 |
|---|---:|---:|---:|---:|
| the wall (body) | 28.26 % | 26.68 % | — | 16.88 % |
| album inspector panel | **38.70 %** | **10.66 %** | — | 10.32 % |
| queue popover | 4.99 % | 5.35 % | — | 7.43 % |
| top bar | 4.85 % | 6.81 % | — | 5.18 % |
| bottom bar, playing | 2.74 % | 2.65 % | — | 1.78 % |
| Settings place body | **1.89 %** | **2.00 %** | — | **1.15 %** |
| bottom bar, idle | 1.06 % | 1.03 % | — | 0.68 % |
| first run | 0.58 % | 0.53 % | — | 0.28 % |
| empty library | 0.20 % | 0.20 % | — | 0.10 % |

The wall's figure is lower than the audit's 65 % for a reason that is about the
*fixture* and not the layout: the audit's frames predate the group keys, so its
wall was a flat grid with every cell filled, and this one is shelved by ARTIST
over 25 albums, where most shelves hold two works. The right comparison is
before-against-after in the same column, and it is 28.26 → 26.68 — the 1.6 points
the wider index rail costs.

## 7. Proportion

| division | before 1280 | after 1280 |
|---|---:|---:|
| top bar / window height | 0.0616 | 0.0570 |
| bottom bar / window height | 0.1186 | 0.1221 |
| body / window height | 0.8198 | **0.8209** |
| inspector / window width | 0.2656 | 0.2656 |
| inspector sleeve / panel width | 0.8588 | **0.3529** |
| tile: art / (art + gap + label) | 0.838 (audit) | 0.813 |
| Settings content right edge / W | 0.6859 | 0.6984 |
| Settings content right edge / W, at 1920 | **0.4573** | **0.5344** |

## 8. What did not close

**The density inversion did not close, and it should not be forced.** The
Settings place reads 2.00 % at 1280 and **1.15 %** at 1920, against a wall at
26.7 % and 16.9 %. Making the content responsive (defect 9) moved its right edge
from 0.457 W to 0.534 W at 1920 and bought 0.01 points of ink; the one-gutter and
rhythm work bought nothing here, because a place with one section and five
controls is empty for a reason that is not composition. **The remedy is content,
not layout** — the output chain, the library roots, the enrichment toggles — and
until they exist the honest answer is a measure cap, which is what
`SETTINGS_CONTENT_MAX` now is. Filling the room by stretching five controls
across 1 800 px would make it emptier, not fuller.

**The inspector's declared order is still not its measured order.** The title is
sixth of eight. Capping the sleeve at the audit's alternative — 84 px, share
54.8 % — would close more of it, and it is not taken here because 120 is the
number the audit ranked and because a cover below ~100 px stops being a cover.
If the declaration is to be met rather than approached, the next move is the
title's *size*, not the sleeve's.

**The queue popover's 4-lattice fit fell** 87 % → 83 %. Its sample is almost
entirely rows of type, and its row pitch is exactly 32 now where it was 28.14, so
what moved is which ink edges the ruler sees rather than which slots the layout
reserves.

## Reproducing this

```sh
bash docs/design/composition/tools/mkfixture.sh /tmp/baz-comp-fixture
SCEN=A W=1280 H=860 DISP=:191 bash docs/design/composition/tools/capture.sh
python3 docs/design/composition/tools/census2.py 1280x860   # element edges, the bar's marks
python3 docs/design/composition/tools/census3.py 1280x860   # optical centring
python3 docs/design/composition/tools/census4.py 1280x860   # rhythm, proportion, density
python3 docs/design/composition/tools/census5.py 1280x860   # lattice test, hierarchy mass
python3 docs/design/composition/tools/overlay.py docs/design/impl/composition
```
