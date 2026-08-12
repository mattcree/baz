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

echo "rendered:"
ls -l hicolor/*/apps/io.github.mattcree.baz.png | awk '{print "  " $5 "\t" $9}'
