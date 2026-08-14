# Sticky section-header alignment

Item 22 removes the extra left space that appeared when a Library section
heading became sticky.

## The 56 px jump

The ordinary heading is part of the virtualized grid inside the wall
scrollable. That scrollable spans the window so its 4 px bar can reach the
edge, but reserves `WALL_RESERVE` on the right before laying out content:

```text
WALL_RESERVE = INDEX_LANE_W + WALL_SCROLLBAR_W = 108 + 4 = 112 px
```

The pinned copy is a sibling layered over that scrollable. Its field must span
the full outer wall to hide sleeves passing beneath it, but it also centered
the heading block in that outer width. The ordinary and sticky left edges were
therefore separated by `112 / 2 = 56 px`; the apparent padding was real and
did not come from glyph bearings or the tile mat.

## Shared correction

The pinned container still fills and paints the complete wall. It now applies
the same 112 px right reservation before centering `Grid::block_width`, making
its inner measure identical to the scrollable's. `header_band` remains the one
ordinary/sticky line implementation, so type, artist-link hit box, height and
vertical hand-over do not fork. No group key or page receives an offset.

## Verification

The geometry regression constructs every density at outer widths 696, 900,
1280, 1920 and 2560 px and proves:

```text
ordinary left = Grid::margin
sticky left   = (outer - WALL_RESERVE - Grid::block_width) / 2
ordinary left = sticky left
```

Real tiny-skia/Xvfb renders at 900, 1280 and 1920 px exercised all six group
keys—A–Z, Artist, Year, Genre, Added and Played—in the sticky state. At 1280 px
the ordinary heading and tile/caption edge measured about x = 308–309; after
scrolling, the pinned heading remained at x = 308 rather than moving to x =
364. The narrow and wide captures retained the same relationship, and the
opaque field continued across the wall.
