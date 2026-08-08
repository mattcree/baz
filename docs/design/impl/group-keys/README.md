# Step 8 — group keys, shelf headers and the index rail

Pixel evidence for [ADR-0017](../../../adr/0017-design-direction.md) step 8,
consuming [ADR-0019](../../../adr/0019-group-keys.md)'s `baz-core` API. Every
image is the real binary; nothing here was drawn by hand or composited.

## How they were made

Per [`docs/DEVELOPMENT.md`](../../../DEVELOPMENT.md)'s headless section, with
**all six variables redirected**:

```sh
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS \
  DISPLAY=:191 WINIT_UNIX_BACKEND=x11 \
  HOME=$S/home XDG_DATA_HOME=$S/data XDG_CONFIG_HOME=$S/config \
  XDG_CACHE_HOME=$S/cache XDG_RUNTIME_DIR=/tmp/bz8run XDG_STATE_HOME=$S/state \
  target/release/baz $FIXTURE
```

- a private `Xvfb :191`, no window manager, the window sized by `xdotool` and
  captured **by this process's pid**, never "the active window";
- a scratch `HOME` carrying an `.asoundrc` that routes ALSA's default PCM to
  `null`, and a **digitally silent** fixture — two independent guarantees of
  silence, with `BAZ_DEVICE_TESTS` unset;
- the build carries `device-output`, so the bottom bar is the real one;
- the isolation receipt, from every run's log:

  ```
  [mpris] no session bus; desktop media controls unavailable
          (I/O error: No such file or directory (os error 2))
  ```

- the fixture is [`mkfixture.sh`](mkfixture.sh): **31 albums, 95 silent FLAC
  tracks, 30 artists over six decades, 22 genres spelled as people spell them**
  (`Post-Rock`, `post rock`, `Rock; Electronic`, `Electronic`, `electronic`),
  one album with no genre tag at all, one artist starting with a digit, one
  with punctuation, one non-Latin, and nothing ever played. That variety is the
  point: a fixture of one artist and one genre would let every key draw the
  same wall.

## The frames

| Image | What it shows |
|---|---|
| [`key-1-1280.png`](key-1-1280.png) · [`key-1-1920.png`](key-1-1920.png) | **ARTIST.** The key row with one word lit — full paper, Medium — and four quiet ones. Shelf headers `#` and `A`; the rail is `#` + A–Z with the letters the collection has nothing under drawn in the muted ink, plus `Ó` and a CJK initial past `Z`. |
| [`key-2-1280.png`](key-2-1280.png) · [`key-2-1920.png`](key-2-1920.png) | **YEAR.** Decade headers `1960S`, `1970S`; the rail is the decade run. |
| [`key-3-1280.png`](key-3-1280.png) · [`key-3-1920.png`](key-3-1920.png) | **GENRE.** Verbatim from the tags and nothing else: `No genre` first, then the case-folded run in which `Post-Rock` and `post rock` are two shelves and `Electronic`/`electronic` are one. |
| [`key-4-1280.png`](key-4-1280.png) · [`key-4-1920.png`](key-4-1920.png) | **ADDED.** One `THIS EVENING` shelf — the fixture was imported a moment before the capture, and ADR-0019 §7 states rather than discovers that ADDED borrows the ledger's six-hour first band. |
| [`key-5-1280.png`](key-5-1280.png) · [`key-5-1920.png`](key-5-1920.png) | **PLAYED.** One `NEVER PLAYED` shelf holding the library. Not a degraded mode: nothing in the fixture has been played, and the rail says so in one entry. |
| [`rail-jump-artist-1280.png`](rail-jump-artist-1280.png) | **The rail jumps.** A click on `T` in the rail; the wall is at the `T` shelf, landed on its *header band* rather than mid-shelf. |
| [`sticky-artist-1280.png`](sticky-artist-1280.png) · [`sticky-year-1280.png`](sticky-year-1280.png) · [`sticky-genre-1280.png`](sticky-genre-1280.png) · [`sticky-artist-1920.png`](sticky-artist-1920.png) · [`sticky-year-1920.png`](sticky-year-1920.png) · [`sticky-genre-1920.png`](sticky-genre-1920.png) | **The pinned header.** Scrolled walls: the header of the shelf on screen sits at the top of the viewport, its own covers passing under an opaque wall-coloured band, with the next in-flow header below. |

## Measured off the pixels, not eyeballed

[`rule.py`](rule.py) reads the cover columns straight out of a screenshot by
walking a scanline and calling anything that is not `#0C0D0E` a work.

**The hang survives the rail, at both widths.** The rail takes
`INDEX_LANE_W` = 60 off the wall before the grid is resolved, and the grid's
own arithmetic is untouched by it:

```
$ python3 rule.py key-5-1280.png 250
  cover 0: x   40.. 294  width 255
  cover 1: x  335.. 589  width 255
  cover 2: x  630.. 884  width 255
  cover 3: x  925..1179  width 255
  gutter 0->1: 40   gutter 1->2: 40   gutter 2->3: 40
  left margin: 40

$ python3 rule.py key-5-1920.png 300
  cover 0..5: width 264 each
  gutters: 39, 39, 40, 39, 39
  left margin: 40
```

**gutter == margin == `HANG` 40** at 1280, and at 1920 within the renderer's
own sub-pixel rounding of a 263.33 px sleeve. `crates/baz/src/shelf.rs`'s
`the_hang_holds_with_the_index_rail_taken_off_the_wall` makes the same claim as
algebra over every width from 300 to 2560.

**The header shares the first column's left edge.** Trimming the header band's
scanline gives its ink's bounding box:

```
$ magick key-1-1280.png -crop 1200x16+0+94 +repage -fuzz 6% -trim -format '%wx%h+%X+%Y' info:
  7x7+40+3
$ magick key-5-1920.png -crop 1800x16+0+94 +repage -fuzz 6% -trim -format '%wx%h+%X+%Y' info:
  84x7+40+3
```

x = **40** at both widths, which is the first cover's left edge at both widths.
The wall's headers introduce no x-position of their own.

**The rail's right edge is the top bar's right edge.**

```
$ magick key-1-1280.png -crop 100x600+1180+100 +repage -fuzz 6% -trim -format '%wx%h+%X+%Y' info:
  9x525+75+43        →  ink ends at x 1263
$ magick key-5-1920.png -crop 60x700+1860+100  +repage -fuzz 6% -trim -format '%wx%h+%X+%Y' info:
  27x7+17+372        →  ink ends at x 1903
```

1280 − 1263 = 17 and 1920 − 1903 = 17, which is `GAP_LG` 16 plus the glyph's
own right sidebearing — the same gutter the `Settings` word above it is set
against.

## The vertical unit, and where it comes from

One number, and everything else is arithmetic on it:

```
HANG                    40   the wall's top edge, and the gap between two rows
SHELF_HEADER_H = HANG   40   the header band
  HEADING_LINE_H        14   the header's line box, at the band's top
  band − line           26   clear wall, then the shelf's first row
```

Air above a header's ink is 40 and air below it is 26 — **20 : 13** — so a
header sits nearer the shelf it names than the shelf it follows. Because the
band and a row's trailing gap are the *same* number, the scroll offset at which
a shelf's last row leaves the top of the viewport is exactly the offset at which
the next shelf's band enters it, which is what lets the pinned lane hold exactly
one header at every offset with no overlap and no motion. `shelf.rs`'s
`exactly_one_header_is_in_the_pinned_lane_at_every_offset` sweeps that at 1 px.

## What these frames also caught

The first capture pass would not scroll past the first shelf. The cause was not
the geometry: the pinned header was a `stack` layer that *appeared* when a
header pinned, which put the `scrollable` one level deeper in the widget tree,
and iced 0.13 keys widget state by position — so its scroll offset was rebuilt
from nothing, the wall snapped to the top, the header un-pinned, the stack went
away and the offset came back. A two-frame oscillation, invisible in every unit
test and obvious the moment a real wheel event met a real wall. The layer is now
always in the tree and only its contents change.

## The one thing worth re-opening

`INDEX_W` is 36 px because ADR-0017 §1.7 fixed it there, and the scrollbar's own
lane bounds it from the other side. At the heading size that holds every letter,
`#`, `Various`, `No year` and every decade whole — the ARTIST and YEAR rails are
complete — but it clips `Unknown` (42.4 px), every recency bucket
(`Never played`, 59.1 px) and most genre names, as
[`key-3-1280.png`](key-3-1280.png) shows plainly. The full value is always set
in the shelf header one `HANG` to the left at the same moment, so nothing is
unreadable, and `crates/baz/src/font.rs`'s
`the_index_rail_holds_every_letter_and_decade_whole` keeps the trade measured
rather than discovered. Widening the lane is a decision for the ADR.
