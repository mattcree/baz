#!/usr/bin/env python3
"""Measure the wide `Now playing` out of the captured frames.

The claim of the width half of this study is a **position**, so the number that
matters is where the two columns actually land — not what the arithmetic says
they should. Four readings per window and per build, all in window coordinates:

  1. **the sleeve's edges** — the work is a solid block of artwork, so it is
     the one thing on this surface that can be found without knowing anything
     about the layout: the longest contiguous run of columns that are opaque
     over the sleeve's own height;
  2. **the run column's first ink** — the leftmost mark to the right of the
     sleeve, which is the run's number lane;
  3. **the gap between them** — the reading the owner made by eye
     (*"the playlist hugs right and the art hugs left"*), and the one doc 12
     §5.5a's own note put at *"~700 px"*;
  4. **the air outside the pair** — how much field is left of the sleeve and
     right of the run. Before, these were `HANG` 40 on both sides and every
     spare pixel was in the middle; after, they carry the slack and the middle
     is one `GAP_XL` 24.

Reads PNGs through ImageMagick's `txt:` dump, so it needs no Python imaging
dependency — the same posture as `capture.sh`, which shells out to `magick`.

    python3 docs/design/impl/one-list-drawn-once/measure.py
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
PIXEL = re.compile(r"^(\d+),(\d+): \((\d+),(\d+),(\d+)")

# The field is dark everywhere (`wall` is L 0.158 and the brightest ambient
# stop is L 0.220), so 60/255 sits well above the ground and well below both
# `paper_faint` and any artwork this fixture draws.
INK = 60
# The returns lane is a fixed 280 px and is not part of the body.
LANE = 280
# A column is "solid" — artwork rather than type — when nearly all of it is
# ink. Type never manages this over a 200 px band; a sleeve always does.
SOLID = 0.9


def pixels(png: pathlib.Path, x: int, y: int, w: int, h: int):
    """Every pixel of a crop, as (x, y, brightest channel), window-relative."""
    out = subprocess.run(
        ["magick", str(png), "-crop", f"{w}x{h}+{x}+{y}", "+repage", "txt:-"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    for line in out.splitlines():
        m = PIXEL.match(line)
        if m:
            cx, cy, r, g, b = (int(v) for v in m.groups())
            yield cx + x, cy + y, max(r, g, b)


def read(png: pathlib.Path, width: int, band: tuple[int, int]) -> dict | None:
    """The four readings, from one frame."""
    y0, y1 = band
    body_w = width - LANE
    ink: dict[int, int] = {}
    for x, _y, v in pixels(png, LANE, y0, body_w, y1 - y0):
        if v > INK:
            ink[x] = ink.get(x, 0) + 1
    if not ink:
        return None
    tall = y1 - y0

    # 1 · the sleeve: the longest contiguous run of solid columns.
    solid = sorted(x for x, n in ink.items() if n >= SOLID * tall)
    if not solid:
        return None
    best = run = (solid[0], solid[0])
    for x in solid[1:]:
        run = (run[0], x) if x == run[1] + 1 else (x, x)
        if run[1] - run[0] > best[1] - best[0]:
            best = run
    sleeve_l, sleeve_r = best

    # 2 · the run column's first ink, to the right of the sleeve.
    right = [x for x in ink if x > sleeve_r + 2]
    if not right:
        return None
    run_l, run_r = min(right), max(right)
    return {
        "sleeve": (sleeve_l, sleeve_r),
        "run": (run_l, run_r),
        "gap": run_l - sleeve_r,
        "air_left": sleeve_l - LANE,
        "air_right": width - run_r,
    }


# window → (the y band that crosses both the sleeve and the run's rows)
SIZES = {
    "1280x860": (0, 1280, (300, 500)),
    "1920x1080": (1, 1920, (400, 600)),
    "2560x1440": (2, 2560, (400, 600)),
}


def main() -> int:
    bad = 0
    for size, (prefix, width, band) in SIZES.items():
        print(f"\n=== {size} ===")
        seen = {}
        for build in ("before", "after"):
            png = HERE / f"{prefix}1-now-playing-{build}-{size}.png"
            if not png.exists():
                print(f"  {build}: no frame")
                continue
            r = read(png, width, band)
            if r is None:
                print(f"  {build}: could not find the sleeve")
                bad += 1
                continue
            seen[build] = r
            print(
                f"  {build:6s} sleeve {r['sleeve'][0]}–{r['sleeve'][1]}"
                f" ({r['sleeve'][1] - r['sleeve'][0]} px)"
                f" · run from {r['run'][0]}"
                f" · GAP {r['gap']}"
                f" · air {r['air_left']} | {r['air_right']}"
            )
        if len(seen) == 2:
            b, a = seen["before"], seen["after"]
            # The work must not shrink: the run takes width the record cannot
            # use, which is `the_run_costs_the_record_nothing…`'s claim, here
            # in pixels.
            wb = b["sleeve"][1] - b["sleeve"][0]
            wa = a["sleeve"][1] - a["sleeve"][0]
            verdict = "ok" if wa >= wb else "REGRESSED"
            print(f"  → the work {wb} → {wa} px  [{verdict}]")
            if wa < wb:
                bad += 1
            # …and the gap must collapse to the seam.
            verdict = "ok" if a["gap"] <= b["gap"] else "REGRESSED"
            print(f"  → the gap  {b['gap']} → {a['gap']} px  [{verdict}]")
            if a["gap"] > b["gap"]:
                bad += 1
    print()
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
