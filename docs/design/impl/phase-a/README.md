# Phase A — rendered evidence

Pixel evidence for **A1–A6** of [`../../02-visual-language.md`](../../02-visual-language.md)
§11, the pure value changes of the gallery direction, against the design system
in [`../../../../.interface-design/system.md`](../../../../.interface-design/system.md).

Every image is the real binary. `*-before.png` is the merge base
(`076af68`, the IA restructure complete and no Phase A landed); `*-after.png`
is the same binary built from the sixth Phase A commit. Both were rendered in
the same session, on the same private display, against the same fixture, so the
only variable between the pair is the six commits.

## How they were made

The recipe is [`../../../DEVELOPMENT.md`](../../../DEVELOPMENT.md) §"Headless UI
verification", with **all six variables** redirected:

- a private `Xvfb` at 1400×1000, `env -u WAYLAND_DISPLAY -u
  DBUS_SESSION_BUS_ADDRESS`, `WINIT_UNIX_BACKEND=x11`; baz opens at its shipped
  1280×860 and there is no window manager to move it;
- scratch `HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`,
  `XDG_RUNTIME_DIR` (and `XDG_STATE_HOME`), one set per build, so the
  maintainer's library database, config, thumbnails and session bus were never
  opened;
- an 18-album / 187-track throwaway fixture of generated covers and
  **digitally silent** FLAC, never `~/Music`;
- a `--features device-output` release build — three of the five surfaces here
  are about the transport — so the scratch `HOME` carries an `.asoundrc`
  routing ALSA's default PCM to the `null` device. Two independent guarantees of
  silence: every sample is a zero, and the sink discards it.
  `BAZ_DEVICE_TESTS` was never set;
- captures targeted at *this* process's window by pid, never "the active
  window".

**The receipt that the isolation held** is the line both runs logged:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

One consequence of the null sink is visible in the frames: it accepts writes as
fast as they arrive, so playback free-wheels at tens of times real time. That is
why the elapsed stamps in the "playing" frames are further along than the few
seconds between captures, and why the fixture's tracks are two minutes rather
than ten seconds.

## The frames

| Image | What it shows |
|---|---|
| [`01-shelf-before.png`](01-shelf-before.png) · [`01-shelf-after.png`](01-shelf-after.png) | **The shelf.** The counts in the top bar go from typewritten to proportional (A1); the wall goes from warm charcoal to neutral-cool near-black and the paper from off-white to archival ivory (A3, A4). The tile geometry is deliberately untouched — the hang is Phase B. |
| [`02-inspector-before.png`](02-inspector-before.png) · [`02-inspector-after.png`](02-inspector-after.png) | **The album inspector.** The catalogue line (`1998 · 12 tracks · 33:35`) and the condition report (`FLAC · 16-bit · 44.1 kHz`) were the loudest monospace in the product and are now Sans; the durations still form a straight column, because Plex Sans's figures are tabular (A1). The column surface is `PLINTH` (A3). Play album is still a solid amber slab: revoking that is C1, and out of scope here. |
| [`03-playing-before.png`](03-playing-before.png) · [`03-playing-after.png`](03-playing-after.png) | **Playing.** Halo, lamp dot, playing row, `2 / 12` position readout, `bit-perfect`, and the seek groove — the accent discipline unchanged by Phase A, on the new surfaces. |
| [`04-up-next-before.png`](04-up-next-before.png) · [`04-up-next-after.png`](04-up-next-after.png) | **The Up next popover**, on `PLINTH_LIT`, with the row radius down from 4 to 3 (A5) and the durations in the Sans (A1). |
| [`05-settings-before.png`](05-settings-before.png) · [`05-settings-after.png`](05-settings-after.png) | **The Settings place.** The `0.00 dB` readouts and the segmented control, in the new inks and the tighter radii. |
| [`06-bar-states-before.png`](06-bar-states-before.png) · [`06-bar-states-after.png`](06-bar-states-after.png) | **The now-playing bar in four states** — stopped, playing, paused, seek-hovered — cropped to the same 1280×104 region of the window and stacked. |
| [`07-bar-playing-stacked.png`](07-bar-playing-stacked.png) | The same bar state, before over after, for reading the type change down one column. |

## The reserved slots still hold

The bar's promise is that nothing moves as the music moves, and A6 narrowed five
of the slots that promise rests on. Measured rather than asserted.

**The transport row occupies the same pixels in every state, in both builds.**
Thresholded bounding box of the row's 160 px column, in the same crop:

```
before  stopped      160x45+0+2      after  stopped      160x45+0+2
before  playing      160x47+0+0      after  playing      160x47+0+0
before  paused       160x47+0+0      after  paused       160x47+0+0
before  seek-hover   160x47+0+0      after  seek-hover   160x47+0+0
```

(The stopped row is two rows shorter because Previous and Next are inert and
their glyphs fall below the threshold, not because anything moved.)

**The bar's top edge does not move.** Sampled at x = 300, a column with no
content in any state, the hairline is at row 2 and the bar's own surface begins
at row 3 — in every state of both builds. What changed is only the colour, and
it changed to exactly the specified bytes:

```
before   row 1 #131110 (WALL)   row 2 #4D4946 (HAIRLINE)   row 3 #0D0B0A (RECESS)
after    row 1 #0C0D0E (WALL)   row 2 #454442 (HAIRLINE)   row 3 #060708 (RECESS)
```

**The differing-pixel counts between states are unchanged**, which is the
strongest available statement that the slots are doing the same job at their new
widths:

```
                              before      after
playing vs paused              1 619      1 606
playing vs seek-hovered        1 305      1 298
playing vs stopped             4 582      4 568
```

| Image | What it shows |
|---|---|
| [`08-bar-diff-playing-vs-paused-after.png`](08-bar-diff-playing-vs-paused-after.png) · [`08-bar-diff-playing-vs-paused-before.png`](08-bar-diff-playing-vs-paused-before.png) | Playing against paused. Everything that lights up is the toggle's glyph box and the seek fill. |
| [`09-bar-diff-playing-vs-stopped-after.png`](09-bar-diff-playing-vs-stopped-after.png) | Playing against stopped — the largest difference the bar can produce. Every changed pixel is *inside* a slot reserved for it: the title lane, the `POSITION_W` readout, the transport glyph box, the two `STAMP_W` timestamps, the groove, and the `SIGNAL_W` slot. Nothing beside them shifted. |

## The palette landed as specified

Read out of the rendered frames rather than out of `theme.rs`:

| Surface | before | after | `system.md` §2 |
|---|---|---|---|
| `WALL` (shelf background) | `#131110` | **`#0C0D0E`** | `#0C0D0E` |
| `RECESS` (the bar) | `#0D0B0A` | **`#060708`** | `#060708` |
| `PLINTH` (inspector column) | `#1B1916` | **`#141517`** | `#141517` |
| `PLINTH_LIT` (popover) | `#221F1C` | **`#1C1D20`** | `#1C1D20` |

## The one place a proportional face could still jiggle

| Image | What it shows |
|---|---|
| [`10-preamp-negative-before.png`](10-preamp-negative-before.png) · [`10-preamp-negative-after.png`](10-preamp-negative-after.png) | The ReplayGain pre-amp stepped one press below zero. Before: `-1.00 dB` with a hyphen-minus, in the mono. After: `−1.00 dB` with **U+2212**, in the Sans — the glyph that advances exactly as wide as the `+`. |
| [`11-preamp-positive-before.png`](11-preamp-positive-before.png) · [`11-preamp-positive-after.png`](11-preamp-positive-after.png) | The same control one press above zero. The right edge of the value is pinned in both; what A6 fixes is the *left* edge, which used to move 2.4 px as the sign changed. |

## What Phase A deliberately did not fix

Two things in these frames look unfinished, and both are the next phases' work
rather than an oversight:

- **The tile's hover and selection chrome is square-cornered.** `RADIUS_TILE` is
  deleted because the shelf has no rectangles that are not artwork; the
  *background and border* go in **B1**, which replaces both states with a rule
  under the label. Until then the chrome is there and hard-edged.
- **Play album is still a solid amber rectangle.** Revoking that exception —
  `LAMP`-outlined, `LAMP_WASH` on hover and press — is **C1**.

The shelf's proportions are also untouched: the hang, the density control and
the spine index are **B3**, **B6** and **B7**.
