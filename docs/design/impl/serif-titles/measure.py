#!/usr/bin/env python3
"""Measure the two page heroes out of the captured frames.

The claim of doc 14 tier 2 is a claim about *type*, and type is the one thing
an arithmetic test in `theme.rs` cannot see: the tokens say `SIZE_HERO` on both
pages either way. So the numbers below come out of the pixels.

Three readings, per window size:

  1. **The identity blocks are still the same shape.** Each block's three ink
     bands — hero, byline, facts — with their tops, the pitch between them,
     and the block's ink extent. Tier 1 made the two blocks one composition;
     tier 2 must not have moved them by a pixel, because its whole claim is
     that *only the face changed*.
  2. **The hero band's ink height**, which is a fact about the string rather
     than about the size: `Ochre` has neither an ascender above cap height nor
     a descender, `Road Trip` has both, so these differ at the same
     `SIZE_HERO` 28 and are printed so the difference is not mistaken for one.
  3. **A lean reading on the first letter** — the x of its leftmost ink near
     the top of the band against the x near its foot. It is corroboration and
     not proof, and it is deliberately modest: `Ochre` opens on a round `O`
     whose leftmost point is at mid-height, so an italic `O` registers only a
     few pixels. The *proof* that the bundled serif italic is what rendered —
     rather than a host serif iced silently fell back to — is mechanical and
     lives in `crates/baz/src/font.rs`:
     `the_family_names_baz_asks_for_are_the_names_the_faces_spell` compares
     `theme::WORK_TITLE`'s family string against the name the bundled bytes
     spell for themselves and checks the face declares the italic style, and
     `the_serif_face_carries_every_letter_an_album_title_arrives_with` closes
     the per-glyph half of the same hole.

Reads PNGs through ImageMagick's `txt:` dump, so it needs no Python imaging
dependency — the same posture as `capture.sh`, which shells out to `magick`.

    python3 docs/design/impl/serif-titles/measure.py
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


def bands(ink: dict[tuple[int, int], int]) -> list[tuple[int, int]]:
    """The (top, height) of every run of consecutive rows holding ink."""
    rows = sorted({y for (_, y), value in ink.items() if value >= INK})
    if not rows:
        return []
    out: list[tuple[int, int]] = []
    start = previous = rows[0]
    for row in rows[1:]:
        if row > previous + 1:
            out.append((start, previous - start + 1))
            start = row
        previous = row
    out.append((start, previous - start + 1))
    return out


def lean(ink: dict[tuple[int, int], int], top: int, height: int) -> int:
    """How far the first letter's left edge leans, in px, top against foot.

    Positive means the stroke is further right at the top of the letter than
    at its foot — which is what an italic does and what a roman does not.

    Only rows whose leftmost ink is near the band's own leftmost are counted.
    A line's ink band runs from the tallest ascender to the deepest descender,
    and the rows below the baseline hold ink from *some other* letter (`Road
    Trip`'s `p`), whose x has nothing to say about the first letter's slope.
    """

    def leftmost(row: int) -> int | None:
        xs = [x for (x, y), value in ink.items() if y == row and value >= INK]
        return min(xs) if xs else None

    edges = [(row, leftmost(row)) for row in range(top, top + height)]
    edges = [(row, x) for row, x in edges if x is not None]
    if not edges:
        return 0
    near = min(x for _, x in edges) + 24
    first = [(row, x) for row, x in edges if x <= near]
    # One row in from each end: the extreme rows of a band are antialiasing.
    return first[1][1] - first[-2][1] if len(first) >= 4 else 0


def report(size: str, found: pathlib.Path, made: pathlib.Path) -> None:
    print(f"\n=== {size} ===")
    for label, path in (("a record", found), ("a playlist", made)):
        if not path.exists():
            print(f"  {label}: MISSING {path.name}")
            continue
        ink = pixels(path)
        found_bands = bands(ink)
        print(f"  {label} — {path.name}")
        for index, (top, height) in enumerate(found_bands):
            line = ("hero", "byline", "facts")[index] if index < 3 else f"line {index}"
            note = ""
            if line == "hero":
                note = f"   first letter leans {lean(ink, top, height):+d} px"
            print(f"      {line:7} top y={top:3}  ink height={height:2}{note}")
        if len(found_bands) >= 3:
            block = found_bands[2][0] + found_bands[2][1] - found_bands[0][0]
            pitch = [
                found_bands[1][0] - found_bands[0][0],
                found_bands[2][0] - found_bands[1][0],
            ]
            print(
                f"      block  first ink to last = {block} px"
                f"   pitch hero→byline={pitch[0]}  byline→facts={pitch[1]}"
            )


def main() -> int:
    for size, prefix in (("1280 x 860", "0"), ("1920 x 1080", "1")):
        window = size.replace(" ", "")
        report(
            size,
            HERE / f"{prefix}3-hero-found-{window}.png",
            HERE / f"{prefix}4-hero-made-{window}.png",
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
