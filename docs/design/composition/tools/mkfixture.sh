#!/usr/bin/env bash
# Build a silent FLAC fixture with generated covers for the composition audit.
# Every sample is a zero; every cover is drawn by ImageMagick.
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
cover() {
  local fam=$1 hue=$2 out=$3 txt=$4
  local S=600
  case $fam in
    mono)   # near-black monolith with one faint mark: the black-sleeve case
      magick -size ${S}x${S} "xc:hsl(${hue},18%,7%)" \
        -fill "hsl(${hue},22%,17%)" -draw "rectangle 250,470 350,500" "$out" ;;
    pale)   # paper-pale sleeve
      magick -size ${S}x${S} "xc:hsl(${hue},14%,88%)" \
        -fill "hsl(${hue},30%,42%)" -draw "circle 300,300 300,120" "$out" ;;
    chroma) # saturated flat field, one bar
      magick -size ${S}x${S} "xc:hsl(${hue},72%,46%)" \
        -fill "hsl($(( (hue+180) % 360 )),72%,22%)" -draw "rectangle 0,430 600,470" "$out" ;;
    split)  # two-tone diagonal
      magick -size ${S}x${S} "xc:hsl(${hue},40%,30%)" \
        -fill "hsl($(( (hue+40) % 360 )),55%,62%)" \
        -draw "polygon 0,600 600,0 600,600" "$out" ;;
    rings)  # concentric geometry
      magick -size ${S}x${S} "xc:hsl(${hue},30%,14%)" \
        -fill none -stroke "hsl(${hue},60%,58%)" -strokewidth 8 \
        -draw "circle 300,300 300,90" -draw "circle 300,300 300,160" \
        -draw "circle 300,300 300,230" "$out" ;;
    type)   # typographic sleeve
      magick -size ${S}x${S} "xc:hsl(${hue},25%,20%)" \
        -fill "hsl(${hue},18%,86%)" -pointsize 92 -gravity center \
        -annotate +0+0 "$txt" "$out" ;;
  esac
}

# album: DIR_INDEX FAMILY HUE NTRACKS "Album Title" "Artist" YEAR GENRE
album() {
  local i=$1 fam=$2 hue=$3 n=$4 title=$5 artist=$6 year=$7 genre=$8
  local dir="$FIX/$(printf '%02d' "$i") - $artist - $title"
  mkdir -p "$dir"
  cover "$fam" "$hue" "$dir/cover.jpg" "${title:0:2}"
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

TITLES=("Slow Return" "Field Recording" "Anhydrous" "Nightwatch" "The Long Lie Down"
        "Cassette Weather" "Pilot Light" "Undertow" "Marginalia" "Sixth Street"
        "Blue Hour" "Ledger" "Attic Tape" "Ferrous" "Quiet Part Loud")
track_title() { local i=$1 t=$2; echo "${TITLES[$(( (i * 3 + t) % ${#TITLES[@]} ))]} $t"; }

echo "building albums..."
album  1 mono    28  9  "Closing Time"                          "Halvard Sten"        1997 "Ambient"
album  2 pale    42 11  "Paper Mill"                            "The Ardent"          2003 "Folk"
album  3 chroma 210  7  "Cyan Handbook"                         "Nils Odden"          2011 "Electronic"
album  4 split   12 13  "A Rather Considerably Overlong Album Title That Will Clip" "Marguerite Vance-Lindqvist" 1984 "Jazz"
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
