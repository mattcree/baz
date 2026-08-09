# Controls and iconography — rendered evidence

Pixel evidence for [doc 10 §7](../../10-controls-and-iconography.md)'s eight
steps ([ADR-0026](../../../adr/0026-iconography-and-the-strip-budget.md),
accepted). Every image is the real binary — release, `device-output` — run
headless the way `docs/DEVELOPMENT.md` §"Headless UI verification"
prescribes: a private `Xvfb :199`, all six XDG redirections, an `.asoundrc`
routing ALSA's default PCM to `null`, and the 25-album / 206-track silent
fixture. The run's `[mpris] no session bus` line is the isolation receipt,
and [`capture.sh`](capture.sh) prints it. Repro is three commands, in the
script's own header.

## The frames

| Image | What it shows |
|---|---|
| [`01-strip-single-line-1280x860.png`](01-strip-single-line-1280x860.png) | **Steps 1–4 at the shipped window.** The well wears the magnifier and carries `25 albums · 206 tracks` as its placeholder (the counts moved in from ~1 100 px away); `Play all` leads its cluster with the triangle at the ordinary glyph ink — no accent; the corner is the gear, 32 px where the 84 px word stood. Ten controls, three clusters, three vocabularies. |
| [`02-gear-hover-tooltip-1280x860.png`](02-gear-hover-tooltip-1280x860.png) | **The gear's accessible name.** Hovered: the ink rides to 1.0 on the transport's own tween, and the tooltip says the word — L8.4's amendment licenses the symbol, ADR-0017 §4c requires the name. |
| [`03-well-match-count-1280x860.png`](03-well-match-count-1280x860.png) | **Step 2, filtering.** `low` typed from the wall (type-anywhere); the match count `12 / 25` lands in the well's reserved right-hand slot, beside the caret producing it — doc 07 §3.1's move, delivered. |
| [`04-strip-single-line-floor-960x860.png`](04-strip-single-line-floor-960x860.png) | **The single-line regime at its declared floor.** 960 is the seam the L9 budget sums to exactly: every tenant on one line, the well at its 200 px floor, the gear on screen. (The query from frame 03 is still standing — Esc's first press blurs the well and keeps the query, so the floor is shown *filtering*, which is the harder case.) |
| [`05-strip-split-760x860.png`](05-strip-split-760x860.png) | **Step 5: the split, not a sweep.** Below 960 the strip is two lines — the frame line (well · doors) on the window line, the library line (states · acts) beneath — 89 px against 49. Nothing hides, nothing overflows, no menu appears. |
| [`06-strip-split-600x860.png`](06-strip-split-600x860.png) | **The strip's floor, and the window's declared minimum.** At 600 the two lines still hold every tenant (the library line's budget is 600 to the pixel). The bottom bar degrades below its own documented ~760 floor exactly as it did before this work — the strip's floor is not the bar's, and the study leaves the bar alone. |
| [`07-queue-row-glyphs-1280x860.png`](07-queue-row-glyphs-1280x860.png) | **Step 6: one mark technology.** A hovered queue row's reserved slots — ↑ ↓ ✕ + — all drawn glyphs at one stroke weight, where the ✕ used to stand beside three borrowed font characters. Same slots, same widths, same messages. |
| [`08-settings-steppers-1280x860.png`](08-settings-steppers-1280x860.png) | **Steps 6 and 8 in the Settings place.** The pre-amp steppers wear the drawn − / + pair (the minus is the plus's own bar), and the header is `place_header` — `‹ Library · Settings ·` the note — the frame as one function in five places. |
| [`09-bottom-bar-before.png`](09-bottom-bar-before.png) | The bottom bar (83 px: band, hairline, needle) cropped from a build of the **branch base** (`5a719f2`), nothing playing, same fixture. |
| [`10-bottom-bar-after.png`](10-bottom-bar-after.png) | The same crop from this branch. `magick compare -metric AE` reports **0** differing pixels — doc 10 §4.4's "examined and deliberately untouched", verified rather than asserted. |

## The receipt

```
[startup] room: Closing Time
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Printed by both launches (the branch binary and the baseline). The null
sink accepts writes faster than real time, so the paused elapsed stamp in
frames 07–08 reads further in than the seconds between captures — nothing
about the marks depends on it.
