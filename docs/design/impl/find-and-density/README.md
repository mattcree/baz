# Steps 11 and 6 — type anywhere, and density as zoom

Pixel evidence for ADR-0017 build-plan **step 11** (type-anywhere and the
modifier layer) and **step 6** (density as a zoom gesture).

Every frame is the real release binary, captured by
[`../../composition/tools/capture.sh`](../../composition/tools/capture.sh)
scenarios `E` and `F` — the fixed harness, with the six-variable isolation of
`docs/DEVELOPMENT.md`: private `Xvfb`, scratch `HOME` / `XDG_DATA_HOME` /
`XDG_CONFIG_HOME` / `XDG_CACHE_HOME` / `XDG_RUNTIME_DIR`, no
`DBUS_SESSION_BUS_ADDRESS`. Each run printed the receipt:

```
[mpris] no session bus; desktop media controls unavailable (…)
```

The fixture is the generated, digitally silent 25-album / 206-track set from
`mkfixture.sh`, never `~/Music`.

**Every density frame was reached by the real gesture**, not by a seeded
config: `Ctrl+-`, `Ctrl+=`, `Ctrl+wheel`. So the frames are evidence about the
bindings as much as about the geometry.

One harness note, because it cost an hour: the host's glibc is newer than the
toolbox's, so a host-built `target/release/baz` dies in the container with a
`GLIBC_… not found` link error and the script sits in its wait-for-a-window
loop. `capture.sh` now takes `BIN` and reports `APP EXITED` instead of hanging.

## The hang, measured off the pixels

`bands.py x` over a row of covers, both window sizes. Grid width is the window
less `INDEX_LANE_W` 108 — 1172 and 1812.

| Step | window | measured cover runs | art | gutter | margin | spec |
|---|---|---|---|---|---|---|
| Spacious | 1280 | `48..368`, `426..746` | 320 | **58** | **48** | 3 × 320 |
| Balanced | 1280 | `40..283`, `323..566` | 243 | **40** | **40** | 4 × 243 |
| Dense | 1280 | `28..229`, `257..458` | 201 | **28** | **28** | 5 × 200.8 |
| Spacious | 1920 | `48..353`, `401..706` | 305 | **48** | **48** | 5 × 304.8 |
| Balanced | 1920 | `40..295`, `335..591` | 255/256 | **40** | **40** | 6 × 255.3 |
| Dense | 1920 | `28..223`, `251..446` | 195 | **28** | **28** | 8 × 195 |

`gutter == margin == HANG` at every step, on real pixels, with the index rail's
lane off the wall. The alternating 255/256 at Balanced/1920 is the rasterizer
splitting a 255.33 px cover across the row, which is deliberate: rounding the
art is exactly what would put the difference back into the gutter
(`.interface-design/system.md` §7).

## The frames

| Image | What it shows |
|---|---|
| [`density-spacious-1280x860.png`](density-spacious-1280x860.png) | **Spacious at 1280.** Art pinned at `ART_MAX` 320, so the margins take the slack — the one case `gutter > hang`, at 58. |
| [`density-balanced-1280x860.png`](density-balanced-1280x860.png) | **Balanced**, the default and `theme.rs`'s tokens exactly. 40 · 243 · 40 · 243 · 40. |
| [`density-dense-1280x860.png`](density-dense-1280x860.png) | **Dense** — today's shelf. Five columns of 200.8 where baz drew 5 × 208 before the hang landed, so nobody loses what they had. |
| [`density-spacious-again-1280x860.png`](density-spacious-again-1280x860.png) | A fourth `Ctrl+=` at the top of the ladder. `magick compare -metric AE` against the frame before it: **0**. The zoom saturates rather than wrapping. |
| [`density-wheeled-1280x860.png`](density-wheeled-1280x860.png) | Two `Ctrl`+wheel-down notches from Spacious: the pointer half of the same gesture lands on Dense, and the wall is re-anchored rather than left where the `scrollable` also scrolled it. |
| the same five at `1920x1080` | the second width, measured above. |
| [`find-typed-1280x860.png`](find-typed-1280x860.png) | **Type anywhere.** `co` typed from a cold wall — no click, no `/`, nothing focused. The well fills, takes the focus ring, and the count reads `18 of 25 albums`. |
| [`find-typed-then-zoomed-1280x860.png`](find-typed-then-zoomed-1280x860.png) | `Ctrl+-` *while the well has focus*. The query is still `co` and still 18 of 25 — see below for what this frame used to show. |
| [`find-cleared-1280x860.png`](find-cleared-1280x860.png) | `Esc`, `Esc`: the field blurs, then the query clears **and the well is left blurred**, so the next `Space` is the transport rather than a space. |

## Two defects the frames caught, which the arithmetic did not

**1. Every tile clipped its artist line at Dense.** `views/shelf.rs` sized the
tile's hit box as `row_h − theme::HANG + RULE_LANE_H`. At the default that is
right; at Dense, where the hang is 28, it is 12 px shorter than the label it
holds, so the title survived and the artist line did not. The number was in a
different file from the token it was wrong about. Fixed to the grid's own hang
and pinned by `views::shelf::a_tiles_box_holds_its_work_and_its_whole_label`,
which sweeps all three steps at every width.

**2. `Ctrl+-` typed a hyphen into the query.** iced 0.13's `text_input` inserts
whatever character a press produced and consults the command modifier for its
own four clipboard chords only. With the well focused the frame read `co-` in
the well and *Nothing matches "co-"* on the wall. The same was already true of
`Ctrl+,` and had shipped. One rule now governs both paths into the query — *a
keystroke made with the command modifier is never query text*
(`keys::field_edit_is_query`) — so the chord does nothing there instead of
corrupting the search. It is recorded with iced's other hard limits in
`.interface-design/system.md` §12.

Neither would have been found by reading the diff.
