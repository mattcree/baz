#!/usr/bin/env python3
"""The header strip is the same height on both places the breadcrumb joins.

The Album place leads its strip with `Artist › Album`, whose first half is a
**button**, and the Artist place leads with the artist's name, which is a word.
A button is `TRANSPORT_HIT` 32 tall and a word's line box is `LINE_EMPHASIS` 20,
so without care the strip would be twelve pixels taller on one side of a press
than the other — and the press that crosses it is the one the whole feature is
about. `views::artist` gives its lead the same 32, and this checks it against
the shipped frames rather than against the source.

The strip's foot is its hairline: a full-width 1 px rule under the header, and
the first such rule down the body of each frame.

Run from this directory, after `capture.sh`:

    ./measure.py
"""

import subprocess
import sys

BODY_X0, BODY_X1 = 300, 1240
TOP, BOTTOM = 4, 120


def rule_y(path):
    """The header's hairline: the topmost near-continuous lift off the wall's
    ink running the body's whole width."""
    out = subprocess.run(
        ["magick", path, "-depth", "8", "txt:-"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    grid = {}
    for line in out.splitlines()[1:]:
        pos, rest = line.split(":", 1)
        x, y = (int(v) for v in pos.split(","))
        if not (BODY_X0 <= x < BODY_X1 and TOP <= y < BOTTOM):
            continue
        r, g, b = (int(v) for v in rest.split("(", 1)[1].split(")", 1)[0].split(",")[:3])
        grid[(x, y)] = (r, g, b)
    wall = grid[(BODY_X1 - 4, BOTTOM - 1)]
    for y in range(TOP, BOTTOM):
        lit = sum(
            1
            for x in range(BODY_X0, BODY_X1)
            if (x, y) in grid and sum(grid[(x, y)]) - sum(wall) > 10
        )
        if lit > (BODY_X1 - BODY_X0) * 0.95:
            return y
    return None


def main():
    frames = {
        "Album  (leads with the breadcrumb — a button)": "01-album-page-with-the-breadcrumb.png",
        "Artist (leads with the name — a word)       ": "03-the-artist-place.png",
        "Album  (a second record, from the artist)   ": "04-a-second-record-from-the-artist-page.png",
    }
    seen = {}
    for label, path in frames.items():
        y = rule_y(path)
        if y is None:
            print(f"  no header hairline found in {path}")
            return 1
        seen[label] = y
        print(f"  {label}: hairline at y = {y}")
    heights = set(seen.values())
    print()
    if len(heights) != 1:
        print(f"  FAIL: the strip changes height across the breadcrumb: {sorted(heights)}")
        return 1
    print(
        f"  the header strip is {heights.pop()} px on both places the breadcrumb "
        "joins — the press is not a jump"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
