"""Hold a ruler up to a screenshot: read the cover columns and the rail's edge
straight off the pixels."""
import subprocess
import sys

path, row = sys.argv[1], int(sys.argv[2])
w, h = map(int, subprocess.check_output(
    ["magick", "identify", "-format", "%w %h", path]).decode().split())
raw = subprocess.check_output(
    ["magick", path, "-depth", "8", "RGB:-"])


def px(x, y):
    i = (y * w + x) * 3
    return raw[i], raw[i + 1], raw[i + 2]


WALL = (12, 13, 14)


def is_wall(p):
    return all(abs(a - b) <= 3 for a, b in zip(p, WALL))


runs = []
start = None
for x in range(w):
    art = not is_wall(px(x, row))
    if art and start is None:
        start = x
    elif not art and start is not None:
        runs.append((start, x - 1))
        start = None
if start is not None:
    runs.append((start, w - 1))

print(f"{path} row {row}: image {w}x{h}")
covers = [r for r in runs if r[1] - r[0] > 50]
for i, (a, b) in enumerate(covers):
    print(f"  cover {i}: x {a}..{b}  width {b - a + 1}")
for i in range(1, len(covers)):
    print(f"  gutter {i - 1}->{i}: {covers[i][0] - covers[i - 1][1] - 1}")
if covers:
    print(f"  left margin: {covers[0][0]}")
