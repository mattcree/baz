#!/usr/bin/env python3
"""What the band's presence costs the page, measured off the shipped frames.

The band is **absent, not empty**, so the only thing its disappearance may do
is let everything below it move up by exactly the room it was taking. This
finds the `RECENTLY ADDED` section rule — a full-width hairline in the body —
in the frame with the band and in the frame without it, and reports the
distance between them.

Run from this directory, after `capture.sh`:

    ./measure.py
"""

import subprocess
import sys

# The body, right of the returns lane and above the bottom bar. A hairline in
# the lane or in the bar is not a section rule.
BODY_X0, BODY_X1 = 320, 1226
BODY_Y0, BODY_Y1 = 20, 700


def rows(path):
    """Every pixel of the frame as (x, y, r, g, b), via ImageMagick's txt: form."""
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
        r, g, b = (int(v) for v in rest.split("(", 1)[1].split(")", 1)[0].split(",")[:3])
        grid[(x, y)] = (r, g, b)
    return grid


def rules(path):
    """Every section rule's row, top to bottom: a near-continuous lift off the
    wall's ink running the body's whole width. Adjacent rows collapse, so a
    hairline drawn 1 px tall and one drawn 2 px tall both count once."""
    grid = rows(path)
    wall = grid[(BODY_X1 - 4, BODY_Y1)]
    found = []
    for y in range(BODY_Y0, BODY_Y1):
        lit = sum(
            1
            for x in range(BODY_X0, BODY_X1)
            if sum(grid[(x, y)]) - sum(wall) > 12
        )
        if lit > (BODY_X1 - BODY_X0) * 0.95:
            if not found or y - found[-1] > 2:
                found.append(y)
    return found


def main():
    frames = {
        "band present (the launch snapshot)": "01-home-with-a-run-to-carry-on-with.png",
        "band absent  (something sounding) ": "03-home-while-it-is-sounding.png",
        "band back    (paused)             ": "04-home-after-a-pause.png",
        "band back    (a different run)    ": "06-the-band-follows-the-engine-not-the-snapshot.png",
    }
    seen = {}
    for label, path in frames.items():
        found = rules(path)
        seen[label] = found
        print(f"  {label}: section rules at y = {found}")

    present = [v for k, v in seen.items() if "absent" not in k]
    absent = seen["band absent  (something sounding) "]
    if len(absent) != 1:
        print(f"  FAIL: the band is gone, so there is one rule, not {len(absent)}")
        return 1
    if any(len(v) != 2 for v in present):
        print("  FAIL: with a run to carry on with there are two rules")
        return 1
    if len({tuple(v) for v in present}) != 1:
        print("  FAIL: the band came back at a different height")
        return 1
    top, second = present[0]
    print()
    print(f"  `CONTINUE` rule           : y = {top}")
    print(f"  `RECENTLY ADDED` rule     : y = {second}  (band present)")
    print(f"                              y = {absent[0]}  (band absent)")
    print(f"  the band's whole room     : {second - absent[0]} px, gap included")
    print("  and it comes back at exactly the same height, every time, to the pixel")
    return 0


if __name__ == "__main__":
    sys.exit(main())
