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
# **Two artefacts, and the release ships the one Apple's own tool makes.**
#
# `baz.iconset/` is the input: the ten PNGs `iconutil` expects, at Apple's
# exact filenames. It is committed for the same reason the hicolor ladder is —
# a release runner should not need a rasterizer — and `bundle.sh` runs
# `iconutil -c icns` over it on macOS, so what a person downloads is a
# container produced by Apple.
#
# `io.github.mattcree.baz.icns` is a fallback, written here directly, and is
# used only where `iconutil` does not exist: CI assembles a bundle on Linux to
# check its *shape*, and that check should not need a Mac. **A hand-written
# icns was what shipped first and macOS drew the generic icon over it** — the
# container parses perfectly in an independent reader, so the fault is
# something IconServices wants that a general parser does not, and the most
# likely candidate is the `TOC ` chunk every `iconutil` file opens with. It is
# written now. That is a hypothesis rather than a finding, which is exactly
# why the shipping path no longer depends on it being right.
iconset=baz.iconset
rm -rf "$iconset"
mkdir -p "$iconset"
render_rung() { # edge  name
  magick -background none "$master" -resize "${1}x${1}" \
    -strip -depth 8 PNG32:"$iconset/$2"
}
render_rung 16   icon_16x16.png
render_rung 32   icon_16x16@2x.png
render_rung 32   icon_32x32.png
render_rung 64   icon_32x32@2x.png
render_rung 128  icon_128x128.png
render_rung 256  icon_128x128@2x.png
render_rung 256  icon_256x256.png
render_rung 512  icon_256x256@2x.png
render_rung 512  icon_512x512.png
render_rung 1024 icon_512x512@2x.png

python3 - "$iconset" io.github.mattcree.baz.icns <<'EOF'
import struct
import sys

source, target = sys.argv[1], sys.argv[2]

# type -> (iconset filename, pixel edge). The names are Apple's, and this is
# the set `iconutil` emits from a complete `.iconset`.
TYPES = [
    ("icp4", "icon_16x16.png", 16),
    ("ic11", "icon_16x16@2x.png", 32),
    ("icp5", "icon_32x32.png", 32),
    ("ic12", "icon_32x32@2x.png", 64),
    ("ic07", "icon_128x128.png", 128),
    ("ic13", "icon_128x128@2x.png", 256),
    ("ic08", "icon_256x256.png", 256),
    ("ic14", "icon_256x256@2x.png", 512),
    ("ic09", "icon_512x512.png", 512),
    ("ic10", "icon_512x512@2x.png", 1024),
]

chunks = []
for name, filename, edge in TYPES:
    with open(f"{source}/{filename}", "rb") as handle:
        payload = handle.read()
    if payload[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{filename} is not a PNG")
    width, height = struct.unpack(">II", payload[16:24])
    if (width, height) != (edge, edge):
        raise SystemExit(f"{filename} is {width}x{height}, expected {edge}")
    chunks.append(name.encode("ascii") + struct.pack(">I", len(payload) + 8) + payload)

# **The table of contents Apple's own tool always writes**: one entry per
# following chunk, its type and its total length, in order. A reader can find
# any size without walking the file — and its absence is the one concrete
# difference between what this wrote first and what `iconutil` produces.
toc_body = b"".join(chunk[:8] for chunk in chunks)
toc = b"TOC " + struct.pack(">I", len(toc_body) + 8) + toc_body

body = toc + b"".join(chunks)
with open(target, "wb") as handle:
    handle.write(b"icns" + struct.pack(">I", len(body) + 8) + body)

# Read it back rather than trusting the write: a malformed icns fails silently
# on macOS by drawing the generic application mark.
with open(target, "rb") as handle:
    written = handle.read()
if written[:4] != b"icns" or struct.unpack(">I", written[4:8])[0] != len(written):
    raise SystemExit("the icns header does not describe the file it is in")
offset, seen = 8, []
while offset < len(written):
    kind = written[offset : offset + 4].decode("ascii")
    length = struct.unpack(">I", written[offset + 4 : offset + 8])[0]
    if length < 8 or offset + length > len(written):
        raise SystemExit(f"chunk {kind} runs past the end of the file")
    if kind != "TOC ":
        # Read the pixels the payload actually has, not the size it was asked
        # for: a chunk whose type says 512 and whose PNG is 128 opens, passes
        # by name, and draws blurry.
        payload = written[offset + 8 : offset + length]
        width, height = struct.unpack(">II", payload[16:24])
        expected = {name: edge for name, _, edge in TYPES}[kind]
        if (width, height) != (expected, expected):
            raise SystemExit(f"{kind} claims {expected}px and holds {width}x{height}")
    seen.append(kind)
    offset += length
if seen != ["TOC "] + [name for name, _, _ in TYPES]:
    raise SystemExit(f"round trip lost chunks: {seen}")
print(f"  icns fallback: {len(written)} bytes, TOC + {len(seen) - 1} sizes")
EOF

echo "rendered:"
ls -l hicolor/*/apps/io.github.mattcree.baz.png | awk '{print "  " $5 "\t" $9}'
ls -l io.github.mattcree.baz.icns | awk '{print "  " $5 "\t" $9}'
echo "iconset (what iconutil builds the shipping icns from):"
ls -l baz.iconset | awk 'NR>1 {print "  " $5 "\t" $9}'
