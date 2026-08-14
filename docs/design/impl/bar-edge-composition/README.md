# Resident bar edge composition

Implementation record for backlog item 19, 2026-08-14.

The owner asked for the top and bottom chrome to sit closer to the window, the
application mark to be larger and less inset, and the sounding sleeve's
horizontal and vertical padding to match. These are one edge-composition pass;
the Library wall, index rail and scrollbar retain their established 40 px
collection geometry.

| Rendered/derived edge | Before | After |
|---|---:|---:|
| App mark, left ink | 40 px | **16 px** |
| App trailing sprite, right ink | 40 px | **16 px** |
| Application mark | 16 px | **24 px** |
| Bottom sounding sleeve, left | 40 px | **14 px** |
| Bottom sounding sleeve, top within band | 14 px | **14 px** |

The top uses `APP_BAR_EDGE = GAP_LG = 16`: its trailing hit-box padding is 8,
then the ordinary 8 px sprite inset puts ink at 16. This leaves the six-pixel
borderless resize band outside the close target. The bottom uses
`BAR_EDGE_PAD = (BAR_CONTENT_H - BAR_COVER) / 2 = 14`, so equality is structural
rather than an image-specific nudge. The app-bar line grows by eight pixels for
the larger mark but saves 48 at its two edges; it is 600 px at the 696 px floor
and retains 96 px of draggable slack.

Verification used the repository's isolated Toolbox/Xvfb capture fixture at
1280×860 and 1920×1080, with a playing-state frame at 1280×860. The fixture's
silent audio and private XDG directories kept the owner's library, session and
audio untouched. Source assertions pin the two ink edges, resize clearance,
larger mark lane, minimum-width budget and sleeve's equal x/y inset; the full
790-test Baz suite and warnings-denied Clippy pass.
