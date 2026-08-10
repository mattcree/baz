# The app bar — one band, seven places, before and after

Frames for [ADR-0040](../../../adr/0040-the-app-bar.md), taken by
[`capture.sh`](capture.sh) against two release binaries: `before` is `main` at
`3b9d32b`, `after` is this branch. Headless on a private Xvfb, with all six XDG
redirections; the run's `[mpris] no session bus` line is printed at the end of
every capture and is the receipt that nothing touched the owner's session.

Both widths, every frame: **1280 × 860** and **1920 × 1080**.

## What the owner asked for

> *"please remove the 'Play all' button at the top of the library"*
>
> *"and please put the display options at the top bar"*
>
> *"we should have replaced the top window chrome with an app bar which has
> this + settings + the window controls, the same on all screens"*

## The frames

| | |
|---|---|
| `01-library-*` | The Library. **Before**: `Play all` beside the arrangement row, the gear at the strip's right corner, the density marks down the index rail's foot at the bottom right. **After**: none of those three — the strip is the arrangement row alone, and the bar above carries the marks, the gear and the window buttons. |
| `02-home-*` | Home. Wears no strip in either build; **after**, the app bar is over it, with the marks present (Home hangs works) and `RECENTLY ADDED` back to a plain section rule. |
| `03-record-*` | A record's page, reached by pressing the first shelf's first tile **caption**. Marks absent — a page of rows hangs no columns. |
| `04-playlist-*` | A playlist's page, reached from the lane. Marks absent. |
| `05-artist-*` | An artist's page, reached from the record page's `Artist ›` breadcrumb. **Marks present** — it hangs works — and `RECORDS` is a plain section rule again. |
| `06-nowplaying-*` | Now playing. No strip in either build; the bar is over it. Marks absent. |
| `07-settings-*` | Settings. **After**, reached by pressing the bar's own gear, which is the claim: the gear is in every place now, not only the Library. |
| **`10-every-band-*`** | **The frame that answers the ask.** The top band cropped from all seven places and stacked. Read it as a column: the gear and the three window buttons must be in register down all seven rows, and the display options must appear on exactly three (Library, Home, artist). |
| `08-density-before-press-*` / `08-density-after-press-*` | The marks are **live from the bar**: `Dense` pressed in the band, and the wall re-hangs beneath it. |
| `12-marks-1x-*` / `12-marks-4x-*` | The four marks at the bar's real size, and magnified 4× with a point filter. See the note below. |
| `09-buttons-before-*` / `09-buttons-after-*` / `09-maximised-*` | The window controls, and a press on maximise. See the note below. |
| `11-borderless-*` | The same window under `BAZ_BORDERLESS=1`. |

## Three things these frames cannot show, said plainly

A frame that cannot show what it claims is worse than no frame, so:

1. **Xvfb has no window manager.** So there is no platform title bar above baz
   in *either* build, and `11-borderless-*` looks like the ordinary frames. It
   proves the setting is wired and the layout survives it; it does not show
   what GNOME does with `decorations: false`. On the owner's machine the
   `before` build has a ~37–46 px title bar above everything in these pictures,
   and the `after` build has one **as well** until that field is flipped —
   which is the debt ADR-0040 §6 takes deliberately.
2. **Maximise is a request nobody services here**, for the same reason. In
   `09-buttons-after-*` the press landed — the button wears its own wash — and
   the glyph correctly did **not** change to the restore mark, because the
   window did not maximise. That is the design working: the state is read back
   from the window (`window::get_maximized`) rather than flipped
   optimistically, so a control cannot claim a window state the compositor
   refused. The restore drawing itself is pinned in
   `icon.rs::the_window_controls_are_three_marks_on_the_sets_stroke`.
3. **Idle CPU is not measured here.** Xvfb has no GPU, iced falls back to
   `tiny-skia`, and both builds sit at ~99.8 % regardless.

## The `Dense` mark

`12-marks-4x-*` puts the four side by side, magnified with `-filter point` so
the question is not answered by blurring it. The first three read cleanly at
the bar's size; **`Dense` is a 4 × 4 whose cells minify to 2.25 px at 1× and is
visibly softer than its neighbours.** The owner said *"the way they appear for
the library is nice"* and the marks moved unchanged on the strength of it —
but that sentence was said about the set, and this one mark is the one worth
looking at twice. A larger sprite for it alone is small work.

(A separate change is tightening the `Dense` *step*'s ladder arithmetic. That
does not change this sprite; it does mean the mark's claim about what the step
hangs is worth re-reading once both have landed.)

## How the route was made honest

Five false frames have been produced on this project, and this study produced
three more before it produced these — every one from a coordinate, and every
one caught by *looking at the picture* rather than by counting files:

- **the park position, twice.** Parked over the wall, a tile opened its four
  hover options and the Library frame was a frame of the pointer. Moved to the
  app bar's middle — dead on the branch, but the *base* build has the Library
  strip there and `PLAYED` came up lit. It parks in the returns lane's empty
  lower half now, which is inert in both builds and every place.
- **the breadcrumb x.** 365 is the `›` separator, not the `Artist` door; the
  press stayed on the record's page and the frame was labelled the artist's.
  It is 340 now — and the fix was found by reading the frame, which said
  `Red Shift` in 40 px type.
- **the bar's own control centres.** `APP_BAR_MARKS_W`/`APP_BAR_BUTTONS_W`'s
  companions name each slot's *trailing* edge; read as centres they put the
  density press on the minimise button and the gear press inside the window
  controls. The script now lays the boxes out right-to-left the way the row
  does, and shows the arithmetic.
- **config leaked between builds.** baz persists the arrangement and the
  density on exit, so a stray press in the base walk arrived in the branch's
  `config.toml` and the two builds were photographed with different walls. The
  config is rewritten before *every* launch now. A comparison whose two halves
  are not in the same state is not a comparison.

Everything else follows the standing rules: one walk function driving both
builds, every navigation a pointer press on a control visible in the frame
before it, the window moved to 0,0 and sized explicitly so both builds are
photographed in one coordinate system, and every y below the bar expressed
through `y()` so the branch's extra 41 px is added exactly once.
