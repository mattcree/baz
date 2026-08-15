#!/usr/bin/env bash
# Render the hicolor PNG ladder from baz's committed red circle source.
#
# The PNGs are committed, because a packager should not need a working
# rasterizer to install baz and a Flatpak build has no network. This script is
# how they are regenerated when either SVG changes — run it and commit what it
# writes.
#
#     packaging/icons/render.sh
#
# Needs ImageMagick built against librsvg. ImageMagick's own MSVG renderer
# draws none of this file's gradients or filters and would silently produce a
# flat rectangle, so the check below is not paranoia: `magick -list format`
# must report SVG as "RSVG" (or rsvg-convert must be on PATH as the delegate).
set -euo pipefail

cd "$(dirname "$0")"

if ! command -v magick >/dev/null 2>&1; then
  echo "render.sh: ImageMagick (magick) is not installed" >&2
  exit 1
fi
if ! magick -list format | grep -qE '^ *SVG\*? +SVG +rw\+ +(Librsvg|Scalable).*RSVG'; then
  echo "render.sh: ImageMagick is not using librsvg for SVG." >&2
  echo "           Install librsvg2 (Fedora: librsvg2-tools) and re-check" >&2
  echo "           with: magick -list format | grep SVG" >&2
  exit 1
fi

source=../../crates/baz/assets/icons/logo-transparent-circle-red.svg
master=hicolor/scalable/apps/io.github.mattcree.baz.svg

# Keep the installable scalable icon byte-for-byte aligned with the artwork
# the app embeds. The SVG remains committed at the hicolor path because
# Flatpak installs that standard layout directly.
cmp -s "$source" "$master" || cp "$source" "$master"

# The supplied circle is legible at every supported hicolor rung, so every
# size comes from the one canonical source.
for size in 16 24 32 48 64 128 256 512; do
  magick -background none "$master" -resize "${size}x${size}" \
    "hicolor/${size}x${size}/apps/io.github.mattcree.baz.png"
done

# 8-bit, no colour profile, no timestamp chunk: small, and byte-stable across
# reruns so a regeneration that changed nothing shows up as no diff.
for png in hicolor/*/apps/io.github.mattcree.baz.png; do
  magick "$png" -strip -depth 8 PNG32:"$png"
done

# --------------------------------------------------------------------------
# The macOS icon, from the same master.
#
# `.icns` is committed for the same reason the PNG ladder is: the release
# runner should not need a rasterizer, and Apple's own `iconutil` only exists
# on macOS — generating it there would mean the artwork was rendered by a tool
# nobody can run while reviewing the change. This writes the container
# directly, which is a documented and very small format: an `icns` magic, a
# big-endian total length, then typed chunks whose payloads are ordinary PNGs.
#
# The ten types are the set `iconutil` emits from a complete `.iconset`, so a
# Mac reads exactly what it would have read from Apple's own tool.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
for size in 16 32 64 128 256 512 1024; do
  magick -background none "$master" -resize "${size}x${size}" \
    -strip -depth 8 PNG32:"$tmp/${size}.png"
done

python3 - "$tmp" io.github.mattcree.baz.icns <<'EOF'
import struct
import sys

source, target = sys.argv[1], sys.argv[2]

# type -> pixel edge. The names are Apple's; the pairs are the retina ladder,
# so `icp4` is 16pt at 1x and `ic11` is the same 16pt at 2x.
TYPES = [
    ("icp4", 16),    # 16pt @1x
    ("ic11", 32),    # 16pt @2x
    ("icp5", 32),    # 32pt @1x
    ("ic12", 64),    # 32pt @2x
    ("ic07", 128),   # 128pt @1x
    ("ic13", 256),   # 128pt @2x
    ("ic08", 256),   # 256pt @1x
    ("ic14", 512),   # 256pt @2x
    ("ic09", 512),   # 512pt @1x
    ("ic10", 1024),  # 512pt @2x
]

chunks = []
for name, edge in TYPES:
    with open(f"{source}/{edge}.png", "rb") as handle:
        payload = handle.read()
    if payload[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{edge}.png is not a PNG")
    chunks.append(name.encode("ascii") + struct.pack(">I", len(payload) + 8) + payload)

body = b"".join(chunks)
with open(target, "wb") as handle:
    handle.write(b"icns" + struct.pack(">I", len(body) + 8) + body)

# Read it back rather than trusting the write: a malformed icns fails silently
# on macOS by drawing the generic application mark, which is exactly the
# defect this file exists to fix.
with open(target, "rb") as handle:
    written = handle.read()
magic, total = written[:4], struct.unpack(">I", written[4:8])[0]
if magic != b"icns" or total != len(written):
    raise SystemExit("the icns header does not describe the file it is in")
offset, seen = 8, []
while offset < len(written):
    kind = written[offset : offset + 4].decode("ascii")
    length = struct.unpack(">I", written[offset + 4 : offset + 8])[0]
    if length < 8 or offset + length > len(written):
        raise SystemExit(f"chunk {kind} runs past the end of the file")
    # **Read the pixels the payload actually has**, not the size it was asked
    # for. A chunk whose type says 512 and whose PNG is 128 is the failure
    # this whole file exists to avoid, and it is invisible until a Mac draws
    # a blurry icon — the IHDR is thirteen bytes in and says so plainly.
    payload = written[offset + 8 : offset + length]
    width, height = struct.unpack(">II", payload[16:24])
    expected = dict(TYPES)[kind]
    if (width, height) != (expected, expected):
        raise SystemExit(f"{kind} claims {expected}px and holds {width}x{height}")
    seen.append(kind)
    offset += length
if seen != [name for name, _ in TYPES]:
    raise SystemExit(f"round trip lost chunks: {seen}")
print(f"  icns: {len(written)} bytes, {len(seen)} sizes, every payload at its stated edge")
EOF

echo "rendered:"
ls -l hicolor/*/apps/io.github.mattcree.baz.png | awk '{print "  " $5 "\t" $9}'
ls -l io.github.mattcree.baz.icns | awk '{print "  " $5 "\t" $9}'
