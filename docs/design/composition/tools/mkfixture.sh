#!/usr/bin/env bash
# Build a silent FLAC fixture with generated covers for the composition audit
# and for the store screenshots.
#
# Every sample is a zero — one of the two independent guarantees that a
# headless run is inaudible — and every cover is drawn by ImageMagick. The
# metadata is fictional and deliberately so: `docs/screenshots/` is published
# on Flathub and in the README, and a store page is not the place to publish
# somebody's record collection or somebody else's cover art.
set -euo pipefail

FIX=${1:-/tmp/baz-comp-fixture}
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

rm -rf "$FIX"; mkdir -p "$FIX"

raw_silence() { # seconds -> stdout raw s16le stereo 44.1k
  local secs=$1
  head -c $((44100 * 4 * secs)) /dev/zero
}

mkflac() { # seconds outfile
  local secs=$1 out=$2
  raw_silence "$secs" | flac --totally-silent -0 --force-raw-format \
    --endian=little --sign=signed --channels=2 --bps=16 --sample-rate=44100 \
    -o "$out" -
}

# One canonical file per distinct duration; albums copy + retag.
echo "building duration pool..."
DURS=(97 134 168 203 241 274 312 349 387 424 461 508)
for d in "${DURS[@]}"; do mkflac "$d" "$TMP/d$d.flac"; done
mkflac 3600 "$TMP/hour.flac"
echo "pool done"

# cover FAMILY HUE OUT  — six visual families, hue-varied so the lamp differs
# **Sleeves carry words.** The six families were abstract marks on flat fields,
# which is a picture of a colour scheme rather than of a record: almost every
# real sleeve has the artist or the title printed on it somewhere, and a wall
# of untitled shapes is the strongest single tell that a screenshot is a
# fixture. Every family sets its own type now — different weight, position and
# scale per family, so the wall still reads as twenty-five different designers
# and not one template with the text swapped.
#
# Grain over the lot: a flat gradient is a rendering, and a little noise is the
# difference between "generated" and "photographed".
cover() {
  local fam=$1 hue=$2 out=$3 artist=$4 title=$5
  local S=600
  case $fam in
    mono)   # near-black monolith, the title small and low
      magick -size ${S}x${S} "xc:hsl(${hue},18%,7%)" \
        -fill "hsl(${hue},22%,17%)" -draw "rectangle 60,60 540,420" \
        -fill "hsl(${hue},12%,72%)" -pointsize 26 -gravity southwest \
        -annotate +62+92 "$artist" \
        -fill "hsl(${hue},10%,52%)" -pointsize 20 -gravity southwest \
        -annotate +62+62 "$title" "$out" ;;
    pale)   # paper-pale sleeve, centred serif-ish block
      magick -size ${S}x${S} "xc:hsl(${hue},14%,88%)" \
        -fill "hsl(${hue},30%,42%)" -draw "circle 300,250 300,110" \
        -fill "hsl(${hue},40%,18%)" -pointsize 34 -gravity south \
        -annotate +0+96 "$title" \
        -fill "hsl(${hue},20%,38%)" -pointsize 22 -gravity south \
        -annotate +0+58 "$artist" "$out" ;;
    chroma) # saturated field, title reversed out of the bar
      magick -size ${S}x${S} "xc:hsl(${hue},72%,46%)" \
        -fill "hsl($(( (hue+180) % 360 )),72%,22%)" -draw "rectangle 0,390 600,510" \
        -fill "hsl(${hue},20%,96%)" -pointsize 40 -gravity west \
        -annotate +40+50 "$title" \
        -fill "hsl($(( (hue+180) % 360 )),40%,90%)" -pointsize 22 -gravity west \
        -annotate +40+96 "$artist" "$out" ;;
    split)  # two-tone diagonal, type across the top
      magick -size ${S}x${S} "xc:hsl(${hue},40%,30%)" \
        -fill "hsl($(( (hue+40) % 360 )),55%,62%)" \
        -draw "polygon 0,600 600,0 600,600" \
        -fill "hsl(${hue},10%,95%)" -pointsize 30 -gravity northwest \
        -annotate +44+44 "$artist" \
        -fill "hsl(${hue},14%,80%)" -pointsize 24 -gravity northwest \
        -annotate +44+86 "$title" "$out" ;;
    rings)  # concentric geometry, the name on the rim
      magick -size ${S}x${S} "xc:hsl(${hue},30%,14%)" \
        -fill none -stroke "hsl(${hue},60%,58%)" -strokewidth 8 \
        -draw "circle 300,270 300,80" -draw "circle 300,270 300,150" \
        -draw "circle 300,270 300,220" -stroke none \
        -fill "hsl(${hue},40%,88%)" -pointsize 32 -gravity south \
        -annotate +0+74 "$artist" \
        -fill "hsl(${hue},30%,60%)" -pointsize 20 -gravity south \
        -annotate +0+42 "$title" "$out" ;;
    type)   # typographic sleeve: the title is the whole design
      magick -size ${S}x${S} "xc:hsl(${hue},25%,20%)" \
        -fill "hsl(${hue},18%,86%)" -pointsize 54 -gravity center \
        -size 480x -background none -fill "hsl(${hue},18%,86%)" \
        label:"$title" -gravity center -composite \
        -fill "hsl(${hue},14%,58%)" -pointsize 22 -gravity south \
        -annotate +0+46 "$artist" "$out" ;;
  esac
  # A little noise, so a sleeve reads as a printed thing rather than a fill.
  magick "$out" -attenuate 0.28 +noise Gaussian -quality 92 "$out"
}

# album: DIR_INDEX FAMILY HUE NTRACKS "Album Title" "Artist" YEAR GENRE
album() {
  local i=$1 fam=$2 hue=$3 n=$4 title=$5 artist=$6 year=$7 genre=$8
  local dir="$FIX/$(printf '%02d' "$i") - $artist - $title"
  mkdir -p "$dir"
  cover "$fam" "$hue" "$dir/cover.jpg" "$artist" "$title"
  local t
  for ((t = 1; t <= n; t++)); do
    local d=${DURS[$(( (i * 7 + t * 5) % ${#DURS[@]} ))]}
    local src="$TMP/d$d.flac"
    [[ $i -eq 1 && $t -eq 1 ]] && src="$TMP/hour.flac"
    local f
    f="$dir/$(printf '%02d' "$t") $(track_title "$i" "$t").flac"
    cp "$src" "$f"
    metaflac --remove-all-tags \
      --set-tag="ALBUM=$title" --set-tag="ARTIST=$artist" \
      --set-tag="ALBUMARTIST=$artist" --set-tag="DATE=$year" \
      --set-tag="GENRE=$genre" --set-tag="TRACKNUMBER=$t" \
      --set-tag="TITLE=$(track_title "$i" "$t")" "$f"
  done
}

# **Track titles that read as titles.** They were a pool of fifteen with the
# track number stuck on the end — `Blue Hour 3` — which is the one detail that
# gave the fixture away as generated in every frame it appeared in. Forty-eight
# now, and the stride is coprime with the pool so an album walks it without
# repeating inside itself.
TITLES=("Slow Return" "Field Recording" "Anhydrous" "Nightwatch" "The Long Lie Down"
        "Cassette Weather" "Pilot Light" "Undertow" "Marginalia" "Sixth Street"
        "Blue Hour" "Ledger" "Attic Tape" "Ferrous" "Quiet Part Loud"
        "Winter Ferry" "Halogen" "A Careful Distance" "Saltmarsh" "Every Little Light"
        "The Wire Fence" "Nine Bells" "Low Tide" "Signal Hill" "Middle Distance"
        "Terminal Velocity" "Paper Anniversary" "Grain" "Fathoms" "The Slow Parade"
        "Bell Foundry" "Overcast" "Stray Current" "Coastal Path" "Lantern"
        "Two Weeks Notice" "Hinterland" "The Quiet Coach" "Aftermath" "Sea Fret"
        "Meridian" "Rushlight" "The Turning Year" "Copperplate" "Understory"
        "Northerly" "Gasworks" "The Last Post")
track_title() { local i=$1 t=$2; echo "${TITLES[$(( (i * 11 + t * 7) % ${#TITLES[@]} ))]}"; }

echo "building albums..."
album  1 mono    28  9  "Closing Time"                          "Halvard Sten"        1997 "Ambient"
album  2 pale    42 11  "Paper Mill"                            "The Ardent"          2003 "Folk"
album  3 chroma 210  7  "Cyan Handbook"                         "Nils Odden"          2011 "Electronic"
# **The one title that exists to be too long**, so the audit has a caption that
# clips. `$LONG_TITLE` lets a caller ask for an ordinary one instead — the
# store capture does, because a store page wants a wall of records and not a
# demonstration of an ellipsis. Overriding it here rather than retagging
# afterwards is what keeps the sleeve and the caption saying the same thing.
album  4 split   12 13  "${LONG_TITLE:-A Rather Considerably Overlong Album Title That Will Clip}" "Marguerite Vance-Lindqvist" 1984 "Jazz"
album  5 rings  340  6  "Orbits"                                "Kesh"                2019 "Electronic"
album  6 type    68 10  "Werkbund"                              "Studio Hain"         1978 "Krautrock"
album  7 mono   200  8  "Basalt"                                "Ini Kovac"           2005 "Drone"
album  8 pale   100 12  "Chalk Downs"                           "Edith Rowan Quartet" 1992 "Classical"
album  9 chroma  35  5  "Amber Room"                            "Sotto"               2021 "Ambient"
album 10 split  270  9  "Violet Ledger"                         "Anne-Marie Puig"     1988 "Jazz"
album 11 rings   15  7  "Red Shift"                             "Corvin"              2014 "Electronic"
album 12 type   180 11  "Hydrograph"                            "Peel & Marsh"        2001 "Post-rock"
album 13 mono   300  6  "Nocturne For Nobody"                   "Halvard Sten"        2009 "Ambient"
album 14 pale    55  8  "Wheatfield"                            "The Ardent"          2016 "Folk"
album 15 chroma 120  4  "Green Line"                            "Nils Odden"          1996 "Electronic"
album 16 split   88 10  "Meadowgrass"                           "Sonja Aalto"         1981 "Folk"
album 17 rings  250  7  "Indigo Machines"                       "Kesh"                2023 "Electronic"
album 18 type   320  9  "Magenta Press"                         "Studio Hain"         1986 "Krautrock"
album 19 mono    10  5  "Ferric"                                "Ini Kovac"           2012 "Drone"
album 20 pale   160  6  "Seagrass"                              "Edith Rowan Quartet" 1974 "Classical"
album 21 chroma 280  8  "Ultraviolet Notes"                     "Sotto"               2018 "Ambient"
album 22 split   45 12  "Ochre"                                 "Anne-Marie Puig"     1999 "Jazz"
album 23 rings  195  4  "Teal"                                  "Corvin"              2007 "Electronic"
album 24 type     0 10  "Zero Degrees Of Separation"            "Peel & Marsh"        1993 "Post-rock"
album 25 mono   140  9  "Verdigris"                             "Sonja Aalto"         2022 "Folk"

echo "tracks: $(find "$FIX" -name '*.flac' | wc -l)  albums: $(find "$FIX" -mindepth 1 -maxdepth 1 -type d | wc -l)"
du -sh "$FIX"
