# The pinned heading's ground, 2026-09-03

The owner, for the third time: *"the background colour of the section headers
in sticky mode is also still not the same as the background."*

Both frames: the real binary at 1600 × 900 in the isolated harness, the
owner's own library, Amethyst room, *The Fall Of Math* sounding, the Library
scrolled until the `D` heading pinned. Cropped to the band and the rows under
it (window y 60–260).

| frame | build | band, x 245 | ground beside it, x 245 y 152 | band, x 1480 | ground, x 1480 y 152 |
|---|---|---|---|---|---|
| `01-before` | `1b69223` | `srgb(14,12,17)` | `srgb(21,22,16)` | `srgb(14,12,17)` | `srgb(13,16,12)` |
| `02-after` | this change | `srgb(21,22,16)` | `srgb(21,22,17)` | `srgb(13,17,13)` | `srgb(13,16,12)` |

Before, the band was the room's wall at one colour across the whole width — a
purple slab over a green wash. After, it reads within one level of the ground
at every x, and the one level is the wash's own gradient across the strip's
height (`x 1480` goes `13,17,13 → 13,16,12 → 12,16,12` from y 116 to y 240
whether or not a band is there).

**How:** the band draws a copy of the place's ground under the heading — the
bare wall, the record's wash, the veiled weather and the frost, at the
window's rectangle and clipped to the strip (`views::now_playing::band_ground`,
`glass::Glass::framed`) — instead of a colour that could only approximate it.
`WORK.md` item 107.
