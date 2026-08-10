#!/usr/bin/env bash
# Build the shapes a multi-CD album actually arrives in, as silent FLAC (and
# MP3) files with generated covers — so "which of these is one record" is
# answered by scanning real tagged files rather than by reading the grouping
# code. This is the fixture ADR-0038's table was measured on.
#
# Every sample is a zero, every cover is drawn by ImageMagick, and nothing here
# is copied from anybody's collection. ~/Music is never touched.
#
#   ./mkfixture.sh [DIR]     (default /tmp/baz-multidisc-fixture)
#
# The shapes, one album artist each so a mis-merge cannot hide behind a
# neighbour:
#
#   1  Prince        one ALBUM tag, DISCNUMBER 1/2, ONE folder
#   2  The Clash     one ALBUM tag, DISCNUMBER 1/2, TWO folders
#   3a Miles Davis   "… (Disc 1)" / "… (Disc 2)", DISCNUMBER present
#   3b Fleetwood Mac "… CD1" / "… CD2", NO DISCNUMBER — and in FLAC *and* MP3,
#                    which is the discs × editions interaction (ADR-0007)
#   3c The Beatles   "… [Disc 1]" / "… [Disc 2]", DISCNUMBER present
#   3d Wu-Tang Clan  "… CD1" ALONE — the declined guess: no sibling, no rename
#   3e Talk Talk     "…" + "… - Disc 2" — the asymmetric rip
#   4  Genesis       NO disc signal at all, two folders, colliding track numbers
#
# Requires ffmpeg (with libmp3lame) and ImageMagick. Verify what was written:
#   ffprobe -show_entries format_tags -of default=nk=0 <file>
set -euo pipefail

FIX=${1:-/tmp/baz-multidisc-fixture}
rm -rf "$FIX"
mkdir -p "$FIX"

# cover HUE OUT TEXT — one flat field per record, hue-varied so the wall's
# tiles are distinguishable at a glance.
cover() {
  mkdir -p "$(dirname "$2")"
  magick -size 600x600 "xc:hsl($1,42%,26%)" -fill "hsl($1,20%,88%)" \
    -pointsize 96 -gravity center -annotate +0+0 "$3" "$2"
}

# track DIR ARTIST ALBUM TITLE TRACK DISC EXT
# DISC may be empty, meaning the file carries no DISCNUMBER at all.
track() {
  local dir=$1 artist=$2 album=$3 title=$4 number=$5 disc=$6 ext=${7:-flac}
  mkdir -p "$dir"
  local codec=(-c:a flac)
  [[ $ext == mp3 ]] && codec=(-c:a libmp3lame -b:a 320k)
  local args=(-hide_banner -loglevel error -y
    -f lavfi -i "anullsrc=r=44100:cl=stereo" -t 2 "${codec[@]}"
    -metadata "ARTIST=$artist" -metadata "ALBUMARTIST=$artist"
    -metadata "ALBUM=$album" -metadata "TITLE=$title"
    -metadata "TRACKNUMBER=$number" -metadata "DATE=1979")
  [[ -n $disc ]] && args+=(-metadata "DISCNUMBER=$disc")
  ffmpeg "${args[@]}" "$dir/$(printf '%02d' "$number") $title.$ext"
}

# side DIR ARTIST ALBUM MARK DISC N EXT — N tracks of one disc.
side() {
  local dir=$1 artist=$2 album=$3 mark=$4 disc=$5 n=$6 ext=${7:-flac}
  local t
  for ((t = 1; t <= n; t++)); do
    track "$dir" "$artist" "$album" "$mark $t" "$t" "$disc" "$ext"
  done
}

echo "1  Prince — one tag, two discs, one folder"
D="$FIX/1-one-tag-one-folder/Prince/Sign o' the Times"
cover 265 "$D/cover.jpg" "SotT"
side "$D" "Prince" "Sign o' the Times" "Sign One" 1 4
side "$D" "Prince" "Sign o' the Times" "Sign Two" 2 4

echo "2  The Clash — one tag, two discs, two folders"
D="$FIX/2-one-tag-two-folders/The Clash/Sandinista!"
for d in 1 2; do
  cover 8 "$D/Disc $d/cover.jpg" "Sand"
  side "$D/Disc $d" "The Clash" "Sandinista!" "Sandinista $d" "$d" 4
done

echo "3a Miles Davis — the disc is in the title, (Disc n)"
D="$FIX/3-in-the-title/Miles Davis"
for d in 1 2; do
  cover 190 "$D/Bitches Brew (Disc $d)/cover.jpg" "BB"
  side "$D/Bitches Brew (Disc $d)" "Miles Davis" "Bitches Brew (Disc $d)" \
    "Brew $d" "$d" 4
done

echo "3b Fleetwood Mac — CDn, no DISCNUMBER, in two codecs"
for ext in flac mp3; do
  for d in 1 2; do
    D="$FIX/3-in-the-title/${ext^^}/Fleetwood Mac/Tusk CD$d"
    cover 40 "$D/cover.jpg" "Tusk"
    side "$D" "Fleetwood Mac" "Tusk CD$d" "Tusk $d" "" 4 "$ext"
  done
done

echo "3c The Beatles — [Disc n]"
D="$FIX/3-in-the-title/The Beatles"
for d in 1 2; do
  cover 0 "$D/The Beatles [Disc $d]/cover.jpg" "WHITE"
  side "$D/The Beatles [Disc $d]" "The Beatles" "The Beatles [Disc $d]" \
    "White $d" "$d" 4
done

echo "3d Wu-Tang Clan — a marked disc with no sibling (the declined guess)"
D="$FIX/3-in-the-title/Wu-Tang Clan/Wu-Tang Forever CD1"
cover 55 "$D/cover.jpg" "WTF"
side "$D" "Wu-Tang Clan" "Wu-Tang Forever CD1" "Forever" 1 4

echo "3e Talk Talk — the asymmetric rip: only the second disc is marked"
D="$FIX/3-in-the-title/Talk Talk"
cover 210 "$D/Spirit of Eden/cover.jpg" "SoE"
side "$D/Spirit of Eden" "Talk Talk" "Spirit of Eden" "Eden" "" 4
cover 210 "$D/Spirit of Eden - Disc 2/cover.jpg" "SoE"
side "$D/Spirit of Eden - Disc 2" "Talk Talk" "Spirit of Eden - Disc 2" \
  "Eden Two" "" 4

echo "4  Genesis — no disc signal anywhere, two folders, colliding numbers"
D="$FIX/4-no-disc-signal/Genesis/The Lamb Lies Down on Broadway"
for d in 1 2; do
  cover 100 "$D/Disc $d/cover.jpg" "Lamb"
  side "$D/Disc $d" "Genesis" "The Lamb Lies Down on Broadway" "Lamb $d" "" 4
done

echo "fixture at $FIX ($(find "$FIX" -type f -name '*.flac' -o -name '*.mp3' | wc -l) files)"
