#!/usr/bin/env bash
# The density fixture: the composition fixture, plus **one prolific artist**.
#
# `docs/design/composition/tools/mkfixture.sh` builds 25 records over 15
# artists, at most two each. That is enough wall to fill four rows at the
# tightest step and it is *not* enough artist: a page with two records draws
# two tiles at every density, which would make the artist frames say nothing.
#
# So this adds fourteen more records under `Halvard Sten`, cloned from one of
# his and retagged. Sixteen records is four rows at Spacious and two at Dense
# on a 1920 window, which is a page whose shape visibly answers the control.
#
# Every sample is still a zero and every cover is still drawn by ImageMagick;
# nothing here reaches the network or the owner's collection.
set -euo pipefail

FIX=${1:-/tmp/baz-density-fix}
REPO=${REPO:-$(git rev-parse --show-toplevel)}

"$REPO/docs/design/composition/tools/mkfixture.sh" "$FIX"

SRC="$FIX/01 - Halvard Sten - Closing Time"
[[ -d $SRC ]] || { echo "the base fixture did not build $SRC"; exit 1; }

# Fourteen more, hue-stepped so the wall does not read as one record repeated —
# the covers have to differ or a column-count frame is unreadable.
TITLES=("Second Shift" "Grain" "The Undercroft" "Halfmast" "Sleeper Wave"
        "Cold Open" "Threadbare" "Lowlands" "Bell Tower" "Offcut"
        "Winter Count" "Dry Dock" "Small Hours" "Last Orders")
YEARS=(1999 2001 2002 2004 2006 2008 2010 2013 2015 2017 2018 2020 2021 2024)

for i in "${!TITLES[@]}"; do
  title=${TITLES[$i]}
  year=${YEARS[$i]}
  hue=$(( (i * 47 + 20) % 360 ))
  dir="$FIX/$(printf '%02d' $((26 + i))) - Halvard Sten - $title"
  rm -rf "$dir"; mkdir -p "$dir"
  magick -size 600x600 "xc:hsl(${hue},34%,26%)" \
    -fill "hsl($(( (hue + 150) % 360 )),58%,64%)" \
    -draw "rectangle 90,$((120 + i * 8)) 510,$((300 + i * 8))" \
    "$dir/cover.jpg"
  t=0
  for f in "$SRC"/*.flac; do
    t=$((t + 1))
    [[ $t -gt 6 ]] && break
    out="$dir/$(printf '%02d' "$t") ${title} ${t}.flac"
    cp "$f" "$out"
    metaflac --remove-all-tags \
      --set-tag="ALBUM=$title" --set-tag="ARTIST=Halvard Sten" \
      --set-tag="ALBUMARTIST=Halvard Sten" --set-tag="DATE=$year" \
      --set-tag="GENRE=Ambient" --set-tag="TRACKNUMBER=$t" \
      --set-tag="TITLE=${title} ${t}" "$out"
  done
done

echo "albums: $(find "$FIX" -mindepth 1 -maxdepth 1 -type d | wc -l)  \
Halvard Sten: $(find "$FIX" -mindepth 1 -maxdepth 1 -type d -name '*Halvard Sten*' | wc -l)"
