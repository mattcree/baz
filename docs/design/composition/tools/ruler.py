#!/usr/bin/env python3
"""A ruler for baz's screenshots.

Reads PNGs with the standard library only (zlib + struct) so every pixel is
addressable, then measures: alignment edges, vertical rhythm, optical versus
mathematical centring, proportion, symmetry and ink density.

Nothing here estimates. Every number is read off a committed PNG.
"""

import struct
import sys
import zlib
from collections import Counter

# ---------------------------------------------------------------- PNG decode


class Img:
    def __init__(self, path):
        with open(path, "rb") as fh:
            data = fh.read()
        assert data[:8] == b"\x89PNG\r\n\x1a\n", path
        pos, idat, pal, trns = 8, bytearray(), None, None
        while pos < len(data):
            (ln,) = struct.unpack(">I", data[pos : pos + 4])
            typ = data[pos + 4 : pos + 8]
            body = data[pos + 8 : pos + 8 + ln]
            if typ == b"IHDR":
                (self.w, self.h, depth, colour, _, _, interlace) = struct.unpack(
                    ">IIBBBBB", body
                )
                assert depth == 8 and interlace == 0, (depth, interlace)
                self.colour = colour
            elif typ == b"PLTE":
                pal = body
            elif typ == b"tRNS":
                trns = body
            elif typ == b"IDAT":
                idat += body
            elif typ == b"IEND":
                break
            pos += 12 + ln
        raw = zlib.decompress(bytes(idat))
        nch = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[self.colour]
        stride = self.w * nch
        out = bytearray(stride * self.h)
        prev = bytearray(stride)
        p = 0
        for y in range(self.h):
            ft = raw[p]
            p += 1
            line = bytearray(raw[p : p + stride])
            p += stride
            if ft == 1:
                for i in range(nch, stride):
                    line[i] = (line[i] + line[i - nch]) & 0xFF
            elif ft == 2:
                for i in range(stride):
                    line[i] = (line[i] + prev[i]) & 0xFF
            elif ft == 3:
                for i in range(stride):
                    a = line[i - nch] if i >= nch else 0
                    line[i] = (line[i] + ((a + prev[i]) >> 1)) & 0xFF
            elif ft == 4:
                for i in range(stride):
                    a = line[i - nch] if i >= nch else 0
                    b = prev[i]
                    c = prev[i - nch] if i >= nch else 0
                    pp = a + b - c
                    pa, pb, pc = abs(pp - a), abs(pp - b), abs(pp - c)
                    pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                    line[i] = (line[i] + pr) & 0xFF
            out[y * stride : (y + 1) * stride] = line
            prev = line
        # normalise to RGB triples
        self.px = bytearray(self.w * self.h * 3)
        if self.colour == 2:
            self.px = out
        elif self.colour == 6:
            for i in range(self.w * self.h):
                self.px[3 * i : 3 * i + 3] = out[4 * i : 4 * i + 3]
        elif self.colour == 0:
            for i in range(self.w * self.h):
                v = out[i]
                self.px[3 * i : 3 * i + 3] = bytes((v, v, v))
        elif self.colour == 3:
            for i in range(self.w * self.h):
                j = out[i] * 3
                self.px[3 * i : 3 * i + 3] = pal[j : j + 3]
        del trns

    def rgb(self, x, y):
        i = 3 * (y * self.w + x)
        return self.px[i], self.px[i + 1], self.px[i + 2]

    def lum(self, x, y):
        r, g, b = self.rgb(x, y)
        return 0.2126 * r + 0.7152 * g + 0.0722 * b


def hexs(c):
    return "#%02X%02X%02X" % c


# ------------------------------------------------------------------ measures


def modal(img, x0, y0, x1, y1, step=1):
    """The most common colour in a box — a region's ground."""
    c = Counter()
    for y in range(y0, y1, step):
        for x in range(x0, x1, step):
            c[img.rgb(x, y)] += 1
    return c.most_common(1)[0][0]


def dist(a, b):
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]))


def col_ink(img, y0, y1, x0, x1, ground, thr):
    """Per-column: does any row in [y0,y1) differ from `ground` by > thr?"""
    return [
        any(dist(img.rgb(x, y), ground) > thr for y in range(y0, y1))
        for x in range(x0, x1)
    ]


def row_ink(img, x0, x1, y0, y1, ground, thr):
    return [
        any(dist(img.rgb(x, y), ground) > thr for x in range(x0, x1))
        for y in range(y0, y1)
    ]


def runs(flags, origin=0, gap=0):
    """Contiguous True runs as (start, end_exclusive), merging gaps <= `gap`."""
    out = []
    s = None
    for i, f in enumerate(flags):
        if f and s is None:
            s = i
        elif not f and s is not None:
            out.append((s + origin, i + origin))
            s = None
    if s is not None:
        out.append((s + origin, len(flags) + origin))
    if gap:
        merged = [out[0]] if out else []
        for a, b in out[1:]:
            if a - merged[-1][1] <= gap:
                merged[-1] = (merged[-1][0], b)
            else:
                merged.append((a, b))
        out = merged
    return out


def ink_box(img, x0, y0, x1, y1, ground, thr):
    """Bounding box of everything in the region that is not the ground."""
    lo_x, hi_x, lo_y, hi_y = None, None, None, None
    for y in range(y0, y1):
        for x in range(x0, x1):
            if dist(img.rgb(x, y), ground) > thr:
                lo_x = x if lo_x is None else min(lo_x, x)
                hi_x = x if hi_x is None else max(hi_x, x)
                lo_y = y if lo_y is None else min(lo_y, y)
                hi_y = y if hi_y is None else max(hi_y, y)
    return None if lo_x is None else (lo_x, lo_y, hi_x + 1, hi_y + 1)


def ink_mass(img, x0, y0, x1, y1, ground, thr=8):
    """Ink coverage and ink centroid, weighted by contrast against the ground.

    Returns (coverage_fraction, cx, cy, weighted_mass).
    """
    n = 0
    tot = 0.0
    sx = sy = 0.0
    gl = 0.2126 * ground[0] + 0.7152 * ground[1] + 0.0722 * ground[2]
    for y in range(y0, y1):
        for x in range(x0, x1):
            c = img.rgb(x, y)
            if dist(c, ground) > thr:
                n += 1
                wgt = abs(0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2] - gl)
                tot += wgt
                sx += wgt * (x + 0.5)
                sy += wgt * (y + 0.5)
    area = (x1 - x0) * (y1 - y0)
    if tot == 0:
        return 0.0, None, None, 0.0
    return n / area, sx / tot, sy / tot, tot


def best_unit(values, lo=2, hi=32):
    """The vertical unit that best explains a set of positions."""
    out = []
    for u in range(lo, hi + 1):
        for phase in range(u):
            resid = sum(min((v - phase) % u, u - ((v - phase) % u)) for v in values)
            out.append((resid / len(values), u, phase))
    out.sort()
    return out


if __name__ == "__main__":
    im = Img(sys.argv[1])
    print(im.w, im.h, hexs(im.rgb(int(sys.argv[2]), int(sys.argv[3]))))
