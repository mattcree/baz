# The 2026-09-02 interface pass — what the frames said

`docs/WORK.md` items 86–95. Everything here came out of the real binary at
**1600 × 900** through `docs/screenshots/capture.sh` in the isolated harness,
against the maintainer's own library — with the `[mpris] no session bus`
receipt on every run.

**The method is the finding.** The 2026-08-23 audit read 161k lines with five
reviewers and put UI/UX explicitly out of scope. Nine defects were sitting in
the frames, and **three of them had a doc comment or a passing test asserting
they were already correct**:

| The claim | Where | The frame |
|---|---|---|
| "the band's own frequency under it and its gain above" | `views/equalizer.rs` module doc | no band label was drawn, in any build |
| "they hang from the aside's own lane like everything else in it (law L5)" | `views::page::act` | `Add to playlist…` at x 305, the lane at 293 |
| the prefix "measured in the real face at the real size" | `views::fitted_line` | measured at 0.769 of the drawn width |

That is the audit's own theme — *prose standing in for a guard* — reaching the
area the audit could not cover, because a claim about what is **drawn** has no
return value and a source scan cannot see a pixel.

## The measurements

**Type.** `views::text_width` scaled with `PxScale::from(size)`. `ab_glyph`
inherited rusttype's `Scale`, where that number is the face's *height* — ascent
to descent — and not its em square, which is what `text.size(n)` means. Plex
carries `unitsPerEm` 1000 against a height of 1300, so every advance came back
at **0.769** of its drawn width.

```text
eight tabular digits at SIZE_META   measured 44.31   truth 57.60   ratio 0.76923
"10:00:00" at SIZE_META             measured 38.62   truth 50.21   ratio 0.76923
                                                     (system.md §8.1, HarfBuzz)
1000 / 1300                                                        = 0.76923
```

The truth column is not a second measurement of the same thing: §8 of the
design system states that every Plex digit advances exactly 600/1000 em in all
three weights, and §8.1's `STAMP_W` was measured through HarfBuzz. The check
that would have caught this was already published.

**The wall's origin**, at the same window, with the same 1 190 px block:

```text
Library     374 records, bar visible      block hangs from x 266
Playlists     4 tiles,  no bar            block hangs from x 321
                                                            ── 55 px ──
```

Not two surfaces disagreeing. One surface disagreeing with itself: iced spends
a scrollbar's `spacing` only while the bar is on screen, so the columns moved
by half the reservation depending on how full the wall currently was.

**The alert ink**, in greyscale, which is the reading that has to survive:

```text
"3 folders are not reachable"  (alert)         peak 139
"11 tracks skipped"            (paper_faint)   peak 134
```

Five values apart. Recorded as a **withdrawn** finding rather than a fixed one:
the sentence says what is wrong in its own words, so nothing here rests on
telling two hues apart. What it does say is that the alert ink is buying almost
no emphasis, which is worth knowing before somebody spends it again.

## The frames

| | |
|---|---|
| `01-rooms-before.png` | sixteen rooms as sixteen words; the one swatch strip is below all of them, off the bottom of the page |
| `02-rooms-after.png` | every room wearing its own six planes, in the elevation order, so the ramp reads before the hue does |
| `03-equaliser-before.png` | ten unnamed faders, and the pre-amp one label lane above the zero line it is supposed to share |
| `04-equaliser-after.png` | `32 · 63 · 125 · 250 · 500 · 1k · 2k · 4k · 8k · 16k`, eleven handles on one line |
| `05-title-clipped-before.png` | `Now That I've Found You: A Collecti` — cut mid-glyph at the tile's edge, no ellipsis. `fitted_line`'s own doc comment names this album as the failure it exists to prevent |
| `06-title-cut-after.png` | `Now That I've Found You: A Coll…` |
| `07-playlists-before.png` | the lead run's bare 1 190 px hairline naming nothing, and the block hung 55 px right of the Library's |
| `08-playlists-after.png` | both gone |
| `09-nothing-playing-before.png` | two words in 1 370 × 760, and the same two words in the bar below |
| `10-nothing-playing-after.png` | the wall's own empty-state shape, naming two resident controls rather than growing a third |

## The receipt for item 95

`crate::collection` took 2 888 lines out of `app.rs`, and the claim that
nothing changed on the way across is measured rather than asserted:

```text
library  home  album  playlist  search  themes  now-playing  equalizer  smart-playlist
      0     0      0         0       0       0            0          0               0
                                                             differing pixels
```

`magick compare -metric AE`, every frame, against the build immediately before
the move.
