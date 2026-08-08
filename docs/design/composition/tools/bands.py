#!/usr/bin/env python3
"""Element edges inside a named band: where each thing starts and stops."""
import sys

sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from ruler import Img, hexs, modal, dist, runs  # noqa: E402


def xruns(im, y0, y1, x0=0, x1=None, thr=6, gap=6, ground=None):
    x1 = im.w if x1 is None else x1
    g = ground or modal(im, x0, y0, x1, y1)
    flags = [
        any(dist(im.rgb(x, y), g) > thr for y in range(y0, y1)) for x in range(x0, x1)
    ]
    return g, runs(flags, origin=x0, gap=gap)


def yruns(im, x0, x1, y0=0, y1=None, thr=6, gap=2, ground=None):
    y1 = im.h if y1 is None else y1
    g = ground or modal(im, x0, y0, x1, y1)
    flags = [
        any(dist(im.rgb(x, y), g) > thr for x in range(x0, x1)) for y in range(y0, y1)
    ]
    return g, runs(flags, origin=y0, gap=gap)


if __name__ == "__main__":
    im = Img(sys.argv[1])
    mode = sys.argv[2]
    a, b, c, d = (int(v) for v in sys.argv[3:7])
    thr = int(sys.argv[7]) if len(sys.argv) > 7 else 6
    gap = int(sys.argv[8]) if len(sys.argv) > 8 else 6
    if mode == "x":
        g, r = xruns(im, a, b, c, d, thr=thr, gap=gap)
        print(f"ground {hexs(g)}   x-runs in rows [{a},{b})")
        for s, e in r:
            print(f"  {s:5d} .. {e:5d}   w={e - s}")
    else:
        g, r = yruns(im, a, b, c, d, thr=thr, gap=gap)
        print(f"ground {hexs(g)}   y-runs in cols [{a},{b})")
        for s, e in r:
            print(f"  {s:5d} .. {e:5d}   h={e - s}")
