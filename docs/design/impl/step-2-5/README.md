# ADR-0017 steps 2 and 5 — rendered evidence

Pixel evidence for [the palette indirection](../../../adr/0017-design-direction.md#15-rooms--adopt-the-model-ship-two-defer-two)
and [the hang](../../../adr/0017-design-direction.md#7-the-build-plan). Every
image is the real binary, captured per
[`docs/DEVELOPMENT.md`](../../../DEVELOPMENT.md#headless-ui-verification), and
nothing was touched that this work did not start:

- a private `Xvfb :151` / `:152`, `env -u WAYLAND_DISPLAY
  -u DBUS_SESSION_BUS_ADDRESS`, `WINIT_UNIX_BACKEND=x11`; no window manager, so
  the window is exactly the size it is asked for;
- **all six** redirections — scratch `HOME`, `XDG_DATA_HOME`,
  `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_RUNTIME_DIR` and no session-bus
  address — so the maintainer's library, config, thumbnails and session bus
  were never opened. Every log carries the receipt:

  ```
  [mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
  ```

- a throwaway 24-album / 96-track fixture of generated covers and **digitally
  silent** WAV (every sample a zero), never `~/Music`; the build carries
  `device-output` so the transport is real, and the scratch `HOME` carries an
  `.asoundrc` routing ALSA's default PCM to `null`. `BAZ_DEVICE_TESTS` was
  never set;
- captures targeted at *this* process's window by pid, never "the active
  window".

## The hang

| Image | What it shows |
|---|---|
| [`01-shelf-1280-before.png`](01-shelf-1280-before.png) | **Before.** The 240 × 284 cell at 1280 px: five works of 208, 32 px between them and **56 px** at each edge. The wall has a frame around it, the works are smaller than the gaps deserve, and 32 px of the window belong to no work at all. |
| [`02-shelf-1280-after.png`](02-shelf-1280-after.png) | **After.** Four works of 270, and the one number: `40 \| 270 \| 40 \| 270 \| 40 \| 270 \| 40 \| 270 \| 40`. Work-to-work and work-to-wall are the same 40. |
| [`03-shelf-1920-after.png`](03-shelf-1920-after.png) | 1920 px: six works of 273, gutters 40, margins 40. |
| [`04-shelf-960-after.png`](04-shelf-960-after.png) | 960 px: three works of 266, gutters 40, margins 40. |
| [`05-shelf-1280-reading-room.png`](05-shelf-1280-reading-room.png) | The same 1280 px wall in **Reading Room**, rendered with `BAZ_ROOM=reading-room`. Defined, not selectable — this is what step 20 will be judging, and the frame exists so that judgement has something to look at. |

### The 0 px measurement

A horizontal scanline through the artwork of each capture, run lengths taken
against the wall colour (the sleeve's drop shadow, which step 14 deletes, is
below the 24/255 threshold and is not counted as a work):

```
before  1280 px   56 | 208 | 32 | 208 | 32 | 208 | 32 | 208 | 32 | 208 | 56
after   1280 px   40 | 270 | 40 | 270 | 40 | 270 | 40 | 270 | 40
after   1920 px   40 | 273 | 40 | 274 | 40 | 273 | 40 | 273 | 40 | 274 | 40 | 273 | 40
after    960 px   40 | 267 | 40 | 266 | 40 | 267 | 40
```

Unaccounted pixels in every row: **0**. In the "after" rows every gutter and
every margin is `HANG` exactly — that is the claim
`the_gutter_is_the_hang_wherever_the_art_is_uncapped` makes at every width from
300 to 2560 at 1 px, here on real pixels.

The 273/274 alternation at 1920 is the rasterizer: the art is 273.33 px and is
deliberately **not** rounded to a whole pixel, because rounding it is exactly
what would put the difference back into the gutter.

## The bar did not move

Steps 2 and 5 touch the bottom bar's colours (through the palette) and nothing
else, and the promise *nothing moves as the music moves* is not something to
assume across a change this size. The same state, the same 1280 × 102 crop,
from the binary built at `8830b94` and from the binary built at this branch's
tip:

| Image | What it shows |
|---|---|
| [`06-bar-before.png`](06-bar-before.png) | The bar before. |
| [`07-bar-after.png`](07-bar-after.png) | The bar after. |
| [`08-bar-diff.png`](08-bar-diff.png) | `magick compare -metric AE` between them. |

```
differing pixels                     0 (0)
geometry                             1280x102  ·  1280x102
transport-row ink bounding box       110x21+145+3  ·  110x21+145+3
```

**Zero differing pixels.** Not "nothing visible moved" — the two images are
byte-identical (4 633 B each). The reserved-slot tests in `theme.rs` and
`views/bottom_bar.rs` say the geometry cannot move; this says it did not.

## What is not here

The needle, the index rail, group keys, density-as-zoom and the tile's new
vocabulary are steps 6, 8, 9, 10 and 14. The wall in these frames still draws
the tile's old chrome — a sleeve backing and a drop shadow — because deleting
those is step 14's job and this pass deliberately did not reach for it.
