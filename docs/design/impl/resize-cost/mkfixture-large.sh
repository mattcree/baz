#!/usr/bin/env bash
# A larger fixture than the 25-album composition one: N albums of silent FLAC
# with generated covers, so the wall has a real number of groups on it.
set -euo pipefail
FIX=${1:?outdir}
N=${2:-400}
TRACKS=${3:-8}
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
rm -rf "$FIX"; mkdir -p "$FIX"
head -c $((44100 * 4 * 3)) /dev/zero | flac --totally-silent -0 --force-raw-format \
  --endian=little --sign=signed --channels=2 --bps=16 --sample-rate=44100 -o "$TMP/s.flac" -
# Six covers, reused round-robin: the decode cost per album is what matters,
# not that every sleeve is unique.
for h in 20 80 140 200 260 320; do
  magick -size 600x600 "xc:hsl(${h},40%,35%)" -fill "hsl(${h},20%,88%)" \
    -pointsize 92 -gravity center -annotate +0+0 "$h" "$TMP/c$h.jpg"
done
COVERS=("$TMP"/c*.jpg)
for ((i = 1; i <= N; i++)); do
  artist=$(printf 'Artist %03d' $(( (i % 120) + 1 )))
  title=$(printf 'Record %04d' "$i")
  dir="$FIX/$(printf '%04d' "$i") - $artist - $title"
  mkdir -p "$dir"
  cp "${COVERS[$((i % 6))]}" "$dir/cover.jpg"
  for ((t = 1; t <= TRACKS; t++)); do
    f="$dir/$(printf '%02d' "$t") Track $t.flac"
    cp "$TMP/s.flac" "$f"
    metaflac --remove-all-tags --set-tag="ALBUM=$title" --set-tag="ARTIST=$artist" \
      --set-tag="ALBUMARTIST=$artist" --set-tag="DATE=$((1970 + i % 55))" \
      --set-tag="GENRE=Test" --set-tag="TRACKNUMBER=$t" --set-tag="TITLE=Track $t" "$f"
  done
done
echo "albums: $(find "$FIX" -mindepth 1 -maxdepth 1 -type d | wc -l)  tracks: $(find "$FIX" -name '*.flac' | wc -l)"
