#!/usr/bin/env python3
"""Read the merged surface's two load-bearing measures off the committed PNGs.

Nothing here estimates. Both figures are read from pixels:

- **the sleeve's edge**, which is what ADR-0029's unspent 32 px was costing;
- **the bar's title lane**, which is what the `Queue` door's 152 px bought —
  doc 12 §6.4.1 computed 288 → 448 from the tokens and flagged it unverified.

Method for the lane: the left zone's two timestamps sit in fixed `STAMP_W` 52
slots, the elapsed one **right**-aligned and the total one **left**-aligned
(`views/bottom_bar.rs`). So the total's leftmost ink *is* its slot's left edge,
and everything else in the zone follows from it by arithmetic the frame does not
have to be trusted for. The title lane is the block that is left after the two
slots and the two `GAP_SM` between them.

    docs/design/impl/queue-merged/measure.py
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "composition", "tools"))
from ruler import Img  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))

HANG = 40
GAP_SM = 8
GAP_LG = 16
STAMP_W = 52
TRANSPORT_W = 112
BAR_H = 81


def ink_columns(im, y0, y1, x0, x1, floor=70):
    """Columns in the band that carry ink brighter than the room's wall."""
    out = []
    for x in range(x0, x1):
        for y in range(y0, y1):
            i = 3 * (y * im.w + x)
            if max(im.px[i], im.px[i + 1], im.px[i + 2]) >= floor:
                out.append(x)
                break
    return out


def runs(columns, gap=6):
    """Group columns into contiguous-ish runs, so a word reads as one span."""
    spans = []
    for x in columns:
        if spans and x - spans[-1][1] <= gap:
            spans[-1][1] = x
        else:
            spans.append([x, x])
    return [tuple(span) for span in spans]


def square(im, x0, x1, y0, y1, floor=60):
    """The largest solid block of colour in the region — the sleeve."""
    rows = [
        y
        for y in range(y0, y1)
        if sum(
            1
            for x in range(x0, x1, 4)
            if max(
                im.px[3 * (y * im.w + x)],
                im.px[3 * (y * im.w + x) + 1],
                im.px[3 * (y * im.w + x) + 2],
            )
            >= floor
        )
        > 40
    ]
    if not rows:
        return None
    top, bottom = rows[0], rows[-1]
    mid = (top + bottom) // 2
    cols = [
        x
        for x in range(x0, x1)
        if max(
            im.px[3 * (mid * im.w + x)],
            im.px[3 * (mid * im.w + x) + 1],
            im.px[3 * (mid * im.w + x) + 2],
        )
        >= floor
    ]
    return cols[0], cols[-1], top, bottom


def report(name, window_w, window_h, work_right=None):
    im = Img(os.path.join(HERE, name))
    print(f"\n{name} — {im.w} × {im.h}")

    # --- the sleeve -------------------------------------------------------
    # The sleeve is scanned inside the record column only: with the run
    # standing, its rows are ink too, and a detector that swept the whole body
    # would measure the two columns together.
    found = square(im, 0, work_right or im.w, 0, im.h - BAR_H - 20)
    if found:
        x0, x1, y0, y1 = found
        print(f"  sleeve   x {x0}–{x1}  ({x1 - x0 + 1} px)   y {y0}–{y1}  ({y1 - y0 + 1} px)")

    # --- the bar's left zone ---------------------------------------------
    band_top = im.h - BAR_H + 10
    band_bottom = im.h - 20
    zone = (window_w - 2 * HANG - 2 * GAP_LG - TRANSPORT_W) / 2
    spans = runs(ink_columns(im, band_top, band_bottom, HANG, int(HANG + zone) + 2))
    # The two timestamps are the last two spans in the zone.
    if len(spans) >= 2:
        elapsed, total = spans[-2], spans[-1]
        block_right = total[0] + STAMP_W
        title_lane = block_right - 2 * STAMP_W - 2 * GAP_SM - HANG
        print(f"  zone     {HANG} … {HANG + zone:.0f}   ({zone:.0f} px, one of two equal fills)")
        print(f"  elapsed  ink {elapsed[0]}–{elapsed[1]}   (right-aligned, slot ends {elapsed[1] + 1})")
        print(f"  total    ink {total[0]}–{total[1]}   (left-aligned, slot starts {total[0]})")
        print(f"  title lane  {title_lane} px   (block {HANG}…{block_right} less two stamps and two gaps)")
        print(f"  …with the door it was  {title_lane - 152 - GAP_SM} px")


if __name__ == "__main__":
    report("01d-sleeve-collapsed-1280x860.png", 1280, 860)
    report("01b-run-off-1280x860.png", 1280, 860)
    report("02b-run-off-1920x1080.png", 1920, 1080)
    report("01a-run-on-1280x860.png", 1280, 860, work_right=1280 - 480)
    report("02a-run-on-1920x1080.png", 1920, 1080, work_right=1920 - 480)
