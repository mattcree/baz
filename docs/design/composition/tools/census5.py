#!/usr/bin/env python3
"""Rhythm lattice test, information-hierarchy mass, and the remaining controls."""
import sys

sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from ruler import Img, hexs, dist, ink_box, ink_mass  # noqa: E402
from bands import xruns, yruns  # noqa: E402

SHOTS = "/tmp/baz-comp-shots"
W, H = (int(v) for v in sys.argv[1].split("x"))
TAG = f"{W}x{H}"
WALL = (0x0C, 0x0D, 0x0E)
RECESS = (0x06, 0x07, 0x08)
PLINTH = (0x14, 0x15, 0x17)
LIT = (0x1C, 0x1D, 0x20)
load = lambda n: Img(f"{SHOTS}/{n}-{TAG}.png")  # noqa: E731

im = load("wall-rest")
BAR = next(y + 1 for y in range(H - 1, H - 200, -1) if dist(im.rgb(5, y), RECESS) > 2)
RULE2 = BAR - 1
# The two chrome bands, found rather than assumed — their heights are tokens
# now, so the ruler reads them off the frame instead of remembering 53 and 102.
TOPB = next(y for y in range(12, 120) if dist(im.rgb(5, y), (0x0C, 0x0D, 0x0E)) > 2)
BODY = TOPB + 1
print(f"############ {TAG}   top bar h {TOPB + 1}   bar h {H - BAR + 1}")


def yedges(imx, x0, x1, y0, y1, g):
    out = set()
    for s, e in yruns(imx, x0, x1, y0, y1, thr=3, gap=0, ground=g)[1]:
        out.add(s)
        out.add(e)
    return sorted(out)


def lattice(vals, u):
    """Best phase, and the share of values within +-1 px of the lattice."""
    best = None
    for ph in range(u):
        hit = sum(1 for v in vals if min((v - ph) % u, u - ((v - ph) % u)) <= 1)
        if best is None or hit > best[0]:
            best = (hit, ph)
    return best[0] / len(vals), best[1]


imi = load("inspector")
panel_x = next(x for x in range(W - 1, 0, -1) if dist(imi.rgb(x, RULE2 - 6), PLINTH) > 2) + 1
imp = load("wall-playing")
ims = load("settings")
imq = load("queue-playing")
# The popover's surface, found over the whole body rather than at a row a
# fixed bar height used to put it at.
_qy = [y for y in range(BODY, RULE2) if dist(imq.rgb(W - 60, y), LIT) <= 3]
_qm = (min(_qy) + max(_qy)) // 2 if _qy else RULE2 - 40
xs = [x for x in range(W) if dist(imq.rgb(x, _qm), LIT) <= 3]

SETS = {
    "top bar": yedges(im, 0, W, 0, TOPB, WALL),
    "bottom bar (idle)": yedges(im, 0, W, BAR, H, RECESS),
    "bottom bar (playing)": yedges(imp, 0, W, BAR, H, RECESS),
    "inspector": yedges(imi, panel_x, W, BODY, RULE2, PLINTH),
    "settings": yedges(ims, 0, W, BODY, RULE2, WALL),
    "queue popover": yedges(imq, min(xs), max(xs) + 1, BODY, RULE2, LIT),
    "tile column": yedges(im, 40, 310 if W < 1600 else 313, BODY, RULE2, WALL),
}
pooled = sorted(set().union(*SETS.values()))

print("\n### RHYTHM — share of chrome y-edges within +-1 px of a u-lattice")
print("   surface                n   " + "  ".join(f"u={u:<2d}" for u in (4, 6, 8, 12, 16)))
for k, v in list(SETS.items()) + [("POOLED", pooled)]:
    cells = []
    for u in (4, 6, 8, 12, 16):
        s, ph = lattice(v, u)
        cells.append(f"{s:4.0%}")
    print(f"   {k:22s}{len(v):4d}   " + "  ".join(f"{c:<5s}" for c in cells))
print("\n   (a lattice of unit u catches a random set at about 3/u; u=4 -> 75%,")
print("    u=6 -> 50%, u=8 -> 38%, u=12 -> 25%, u=16 -> 19%)")

print("\n### RHYTHM — the y-edges themselves")
for k, v in SETS.items():
    print(f"   {k:22s} {v}")

print("\n### HIERARCHY — weighted ink mass per element (area x contrast)")


def mass_of(imx, box, g, thr=8):
    cov, cx, cy, m = ink_mass(imx, box[0], box[1], box[2], box[3], g, thr)
    return m, cov


print("   -- wall (rest) --")
tot = mass_of(im, (0, 53, W, RULE2), WALL)[0]
art_x1 = next(x for x in range(41, W) if dist(im.rgb(x, 200), WALL) <= 2)
one_art = mass_of(im, (40, 93, art_x1, 93 + (art_x1 - 40)), WALL)[0]
one_label = mass_of(im, (40, 93 + (art_x1 - 40) + 16, art_x1, 93 + (art_x1 - 40) + 16 + 37), WALL)[0]
print(f"      whole wall body mass {tot:.3e}")
print(f"      one sleeve  {one_art:.3e}  ({one_art / tot:6.2%} of the wall)")
print(f"      its label   {one_label:.3e}  ({one_label / tot:6.2%})  ratio sleeve:label "
      f"{one_art / max(one_label, 1):.1f} : 1")

print("   -- top bar --")
tb = mass_of(im, (0, 0, W, 52), WALL)[0]
for nm, a, b in (("search well", 10, 380), ("counts", 1000, W - 108), ("Settings", W - 100, W - 10)):
    m = mass_of(im, (a, 0, b, 52), WALL)[0]
    print(f"      {nm:14s} {m:.3e}  {m / tb:6.2%} of the bar")

print("   -- bottom bar (playing) --")
bb = mass_of(imp, (0, BAR, W, H), RECESS)[0]
for nm, a, b in (
    ("left zone", 8, 270),
    ("Queue control", 270, 460),
    ("transport", W // 2 - 130, W // 2 + 130),
    ("seek row", W // 2 - 220, W // 2 - 130),
    ("signal note", W - 260, W - 150),
    ("volume block", W - 150, W),
):
    m = mass_of(imp, (a, BAR, b, H), RECESS)[0]
    print(f"      {nm:14s} {m:.3e}  {m / bb:6.2%} of the bar")

print("   -- inspector --")
ip = mass_of(imi, (panel_x, 53, W, RULE2), PLINTH)[0]
for nm, y0, y1 in (
    ("close", 80, 112),
    ("sleeve", 121, 413),
    ("title", 420, 450),
    ("artist", 450, 474),
    ("catalogue+cond", 474, 515),
    ("Play album", 520, 560),
    ("track list", 560, 710),
    ("footnote", 712, 740),
):
    m = mass_of(imi, (panel_x, y0, W, y1), PLINTH)[0]
    print(f"      {nm:14s} {m:.3e}  {m / ip:6.2%} of the panel")

print("\n### SELECTED tile's rule (measured in the inspector frame)")
for y in range(400, 430):
    c = imi.rgb(100, y)
    if dist(c, WALL) > 2:
        print(f"      y={y}  {hexs(c)}")

print("\n### PLAY ALBUM — the primary control's internal composition")
band = [(s, e) for s, e in yruns(imi, panel_x, W, 500, 570, thr=3, gap=1, ground=PLINTH)[1]]
print("      bands:", band)
s, e = band[-1] if len(band) == 1 else band[0]
inner = ink_box(imi, panel_x + 26, s + 2, W - 26, e - 2, PLINTH, 20)
print(f"      button y[{s},{e}) h {e - s};  inner ink {inner}")
print(f"      button box x[{panel_x + 24},{W - 24}) w {W - 48 - panel_x}; "
      f"inner ink centred? ink cx {(inner[0] + inner[2]) / 2:.1f} vs box cx "
      f"{(panel_x + 24 + W - 24) / 2:.1f}")
print(f"      inner ink cy {(inner[1] + inner[3]) / 2:.1f} vs box cy {(s + e) / 2:.1f}")

print("\n### SETTINGS controls")
seg = ink_box(ims, 248, 120, 890, 155, WALL, 6)
print(f"      segmented control box {seg}")
for lab, a, b in (("Off", 250, 460), ("Track", 460, 670), ("Album", 670, 880)):
    bx = ink_box(ims, a, 126, b, 152, (0x1C, 0x1D, 0x20), 30)
    print(f"      segment {lab:6s} label ink {bx}")
cb = ink_box(ims, 245, 260, 268, 285, WALL, 6)
print(f"      checkbox box {cb}")
