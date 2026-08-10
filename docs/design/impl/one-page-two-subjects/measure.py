#!/usr/bin/env python3
"""Measure the two pages against each other, out of the captured frames.

The claim of *one page, two subjects* is that a record's page and a playlist's
page are **one composition** — so the interesting number is not any single
measurement but whether the two pages produce the *same* one. A unit test can
assert that the tokens are shared; only the pixels can say that the shared
tokens land in the same place on both.

Three readings, for each window size and each build:

  1. **The identity block**, at tier 1's own crop and threshold: the three ink
     bands (hero, byline, facts), their tops, the pitch between them, and the
     block's ink extent. The two pages must agree — and *before*, they did
     not: the whole block sat 12 px higher on a playlist's page, because the
     strip above it was 12 px shorter (see reading 2's note). Tiers 1 and 2
     could not see it, because each cropped its two blocks out of its own
     page and compared their *shapes*.
  2. **The aside's slot**, from a crop that starts below the sleeve: the
     `Play album` / `Play` band and the acts band under it, with each band's
     top, height and **leftmost ink**. This is the reading the change is
     actually for, and it caught two things. The **left** disagreed because a
     record's single quiet act was a centred, full-width box and a playlist's
     three were natural-width words on the aside's own lane. The **top**
     disagreed by 12 px for a reason no source sweep could see: `TOP_BAR_H` is
     `2 · TOP_BAR_PAD_V + TRANSPORT_HIT + 1`, but the shared strip lays out
     whatever lead it is handed, and a record's breadcrumb is a *control* that
     declares 32 while a playlist's name was a bare 20. The composition boxes
     its lead at the control height now. The rest of the product still has
     that 12 px — Queue, Settings and the Artist place all lead with a bare
     name — and moving every place is a change to the frame, logged rather
     than taken here.
  3. **The page's x-edges**, from the two-column crop: the leftmost ink in the
     main column on each page. The aside is `ALBUM_ASIDE_W` 320 and the seam
     is `GAP_XL` 24, so both pages must start their text at the same x — law
     L5, measured rather than asserted.

Every reading prints `agree` or `DIFFER` beside it, so the file can be read as
a verdict rather than as a table to compare by hand.

Reads PNGs through ImageMagick's `txt:` dump, so it needs no Python imaging
dependency — the same posture as `capture.sh`, which shells out to `magick`.

    python3 docs/design/impl/one-page-two-subjects/measure.py
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
PIXEL = re.compile(r"^(\d+),(\d+): \((\d+),(\d+),(\d+)")
# The crops are on `plinth`, which is dark; ink is anything clearly above it.
# 60/255 sits well above the ground's own value and well below `paper_faint`.
INK = 60


def pixels(path: pathlib.Path) -> dict[tuple[int, int], int]:
    """Every pixel of `path` as (x, y) -> luminance, 0-255."""
    dump = subprocess.run(
        ["magick", str(path), "-depth", "8", "txt:-"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    out: dict[tuple[int, int], int] = {}
    for line in dump.splitlines():
        found = PIXEL.match(line)
        if not found:
            continue
        x, y, r, g, b = (int(part) for part in found.groups())
        out[(x, y)] = (r * 299 + g * 587 + b * 114) // 1000
    return out


def bands(ink: dict[tuple[int, int], int], gap: int = 1) -> list[tuple[int, int]]:
    """The (top, height) of every run of rows holding ink.

    `gap` is how many blank rows still count as one band — 1 keeps every line
    separate, which is what the identity block wants; the aside's controls want
    a larger one, because a button's border and its label are two ink runs of
    one object.
    """
    rows = sorted({y for (_, y), value in ink.items() if value >= INK})
    if not rows:
        return []
    out: list[tuple[int, int]] = []
    start = previous = rows[0]
    for row in rows[1:]:
        if row > previous + gap:
            out.append((start, previous - start + 1))
            start = row
        previous = row
    out.append((start, previous - start + 1))
    return out


def leftmost(ink: dict[tuple[int, int], int], top: int, height: int) -> int:
    """The leftmost inked x anywhere in the band."""
    xs = [x for (x, y), value in ink.items() if top <= y < top + height and value >= INK]
    return min(xs) if xs else -1


def verdict(found, made) -> str:
    return "agree" if found == made else f"DIFFER  ({found} vs {made})"


def identity(size: str, build: str, prefix: str) -> None:
    window = size.replace(" ", "")
    reading = {}
    for label, name in (
        ("a record", f"{prefix}7-identity-found-{build}-{window}.png"),
        ("a playlist", f"{prefix}8-identity-made-{build}-{window}.png"),
    ):
        path = HERE / name
        if not path.exists():
            print(f"    {label}: MISSING {name}")
            return
        found = bands(pixels(path))
        tops = [top for top, _ in found[:3]]
        block = found[2][0] + found[2][1] - found[0][0] if len(found) >= 3 else 0
        pitch = [tops[1] - tops[0], tops[2] - tops[1]] if len(tops) >= 3 else []
        reading[label] = (tops, pitch, block)
        lines = ", ".join(
            f"{name} y={top} h={height}"
            for name, (top, height) in zip(("hero", "byline", "facts"), found[:3])
        )
        print(f"    {label:11} {lines}")
    (ft, fp, fb), (mt, mp, mb) = reading["a record"], reading["a playlist"]
    print(f"      tops     {verdict(ft, mt)}")
    print(f"      pitch    {verdict(fp, mp)}")
    print(f"      block    {verdict(fb, mb)}   ({fb} px of ink, 80 px of box)")
    # The band *heights* are not expected to agree and are not compared: an ink
    # band runs from the tallest ascender to the deepest descender, so `Ochre`
    # in a serif italic and `Road Trip` in the sans differ at the same
    # `SIZE_HERO` 28. That is a fact about the strings (design 14 §5.2), and
    # `../serif-titles/measure.py` is where it is read.


def aside(size: str, build: str, prefix: str) -> None:
    window = size.replace(" ", "")
    reading = {}
    for label, name in (
        ("a record", f"{prefix}5-aside-record-{build}-{window}.png"),
        ("a playlist", f"{prefix}6-aside-playlist-{build}-{window}.png"),
    ):
        path = HERE / name
        if not path.exists():
            print(f"    {label}: MISSING {name}")
            return
        ink = pixels(path)
        # gap 3: a control is a border and a label, and they are two runs.
        found = [band for band in bands(ink, gap=3) if band[1] > 3][:2]
        reading[label] = [(top, height, leftmost(ink, top, height)) for top, height in found]
        for slot, (top, height) in zip(("commitment", "the acts"), found):
            print(
                f"    {label:11} {slot:11} y={top:3} h={height:2}"
                f"  left x={leftmost(ink, top, height)}"
            )
    found, made = reading["a record"], reading["a playlist"]
    for index, slot in enumerate(("commitment", "the acts")):
        if index >= len(found) or index >= len(made):
            continue
        print(f"      {slot:11} top    {verdict(found[index][0], made[index][0])}")
        print(f"      {slot:11} left   {verdict(found[index][2], made[index][2])}")


def columns(size: str, build: str, prefix: str) -> None:
    window = size.replace(" ", "")
    reading = {}
    for label, name in (
        ("a record", f"{prefix}9-page-record-{build}-{window}.png"),
        ("a playlist", f"{prefix}a-page-playlist-{build}-{window}.png"),
    ):
        path = HERE / name
        if not path.exists():
            print(f"    {label}: MISSING {name}")
            return
        ink = pixels(path)
        # The main column starts one GAP_XL right of the 320 px aside; anything
        # inked at or past 344 is the column's own content.
        xs = [x for (x, _), value in ink.items() if value >= INK and x >= 344]
        reading[label] = min(xs) if xs else -1
        print(f"    {label:11} main column's first ink at x={reading[label]}")
    print(f"      x-edge   {verdict(reading['a record'], reading['a playlist'])}")


def main() -> int:
    for size, prefix in (("1280 x 860", "0"), ("1920 x 1080", "1")):
        for build in ("before", "after"):
            print(f"\n=== {size} · {build} ===")
            print("  the identity block")
            identity(size, build, prefix)
            print("  the aside's slot")
            aside(size, build, prefix)
            print("  the two columns")
            columns(size, build, prefix)
    return 0


if __name__ == "__main__":
    sys.exit(main())
