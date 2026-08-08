#!/usr/bin/env python3
"""Structural edge map: the long vertical and horizontal discontinuities.

A composed frame has few. This counts them.
"""
import sys

sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from ruler import Img, hexs  # noqa: E402


def vedges(im, thr=6, minrun=0.0, y0=0, y1=None, x0=1, x1=None):
    """Per-column count of rows where the pixel differs from its left neighbour."""
    y1 = im.h if y1 is None else y1
    x1 = im.w if x1 is None else x1
    px, w = im.px, im.w
    counts = [0] * im.w
    for y in range(y0, y1):
        base = 3 * y * w
        for x in range(x0, x1):
            i = base + 3 * x
            j = i - 3
            if (
                abs(px[i] - px[j]) > thr
                or abs(px[i + 1] - px[j + 1]) > thr
                or abs(px[i + 2] - px[j + 2]) > thr
            ):
                counts[x] += 1
    span = y1 - y0
    return [(x, c, c / span) for x, c in enumerate(counts) if c / span >= minrun]


def hedges(im, thr=6, minrun=0.0, x0=0, x1=None, y0=1, y1=None):
    x1 = im.w if x1 is None else x1
    y1 = im.h if y1 is None else y1
    px, w = im.px, im.w
    counts = [0] * im.h
    for y in range(y0, y1):
        base = 3 * y * w
        prev = 3 * (y - 1) * w
        for x in range(x0, x1):
            i = base + 3 * x
            j = prev + 3 * x
            if (
                abs(px[i] - px[j]) > thr
                or abs(px[i + 1] - px[j + 1]) > thr
                or abs(px[i + 2] - px[j + 2]) > thr
            ):
                counts[y] += 1
    span = x1 - x0
    return [(y, c, c / span) for y, c in enumerate(counts) if c / span >= minrun]


if __name__ == "__main__":
    path = sys.argv[1]
    frac = float(sys.argv[2]) if len(sys.argv) > 2 else 0.10
    im = Img(path)
    print(f"# {path}  {im.w}x{im.h}   threshold: run >= {frac:.0%} of the span")
    print("\n## vertical edges (x, rows, share)")
    for x, c, f in vedges(im, minrun=frac):
        print(f"  x={x:5d}  {c:5d}  {f:6.1%}")
    print("\n## horizontal edges (y, cols, share)")
    for y, c, f in hedges(im, minrun=frac):
        print(f"  y={y:5d}  {c:5d}  {f:6.1%}")
