#!/usr/bin/env bash
# A silent, varied fixture for step 8: enough artists, decades, genres and
# never-played records that shelving is actually visible.
set -euo pipefail
ROOT=${1:?usage: make-fixture.sh DIR}
rm -rf "$ROOT"
mkdir -p "$ROOT"

# artist|album|year|genre|tracks
CATALOGUE=$(cat <<'EOF'
Aphex Twin|Selected Ambient Works 85-92|1992|Electronic|4
Aphex Twin|Drukqs|2001|electronic|3
Bark Psychosis|Hex|1994|Post-Rock|3
Boards of Canada|Music Has the Right to Children|1998|Electronic|4
Boards of Canada|Geogaddi|2002|IDM|3
Broadcast|The Noise Made by People|2000|Dream Pop|3
Can|Tago Mago|1971|Krautrock|3
Cocteau Twins|Heaven or Las Vegas|1990|Dream Pop|3
Duke Ellington|Money Jungle|1963|Jazz|3
Eno|Ambient 1|1978|Ambient|3
Fela Kuti|Zombie|1976|Afrobeat|2
Gastr del Sol|Camoufleur|1998|Post-Rock|3
Harold Budd|The Pearl|1984|Ambient|3
Joni Mitchell|Blue|1971|Folk|4
King Tubby|Dub From the Roots|1974|Dub|3
Low|Things We Lost in the Fire|2001|Slowcore|3
Miles Davis|In a Silent Way|1969|Jazz|2
Nina Simone|Wild Is the Wind|1966|Jazz|3
Ólafur Arnalds|Found Songs|2009|Modern Classical|3
Portishead|Dummy|1994|Trip-Hop|4
Radiohead|Kid A|2000|Rock; Electronic|4
Slowdive|Souvlaki|1993|Shoegaze|3
Stan Rogers|Northwest Passage|1981|Folk|3
Talk Talk|Laughing Stock|1991|post rock|3
The Caretaker|An Empty Bliss|2011|Ambient|3
Tim Hecker|Ravedeath, 1972|2011|Ambient|3
Tortoise|TNT|1998|Post-Rock|4
Wendy Carlos|Sonic Seasonings|1972|Electronic|2
10cc|The Original Soundtrack|1975||3
!!!|Myth Takes|2007|Dance-Punk|3
曲人|Untitled|2019|Experimental|2
EOF
)

i=0
while IFS='|' read -r artist album year genre tracks; do
  [ -z "$artist" ] && continue
  i=$((i + 1))
  dir="$ROOT/$artist/$album"
  mkdir -p "$dir"
  # A deterministic flat cover so the wall reads as covers, not placeholders.
  hue=$(( (i * 37) % 360 ))
  magick -size 400x400 "xc:hsl($hue,35%,28%)" \
      -gravity center -fill '#d8d4cb' -pointsize 28 \
      -annotate 0 "${album:0:18}" "$dir/cover.jpg" 2>/dev/null
  for t in $(seq 1 "$tracks"); do
    args=(-hide_banner -loglevel error -f lavfi -i "anullsrc=r=44100:cl=stereo" -t 6
          -metadata "artist=$artist" -metadata "album_artist=$artist"
          -metadata "album=$album" -metadata "title=Track $t"
          -metadata "track=$t" -metadata "date=$year")
    [ -n "$genre" ] && args+=(-metadata "genre=$genre")
    ffmpeg "${args[@]}" -y "$dir/$(printf '%02d' "$t") Track $t.flac"
  done
done <<< "$CATALOGUE"

echo "fixture: $(find "$ROOT" -name '*.flac' | wc -l) tracks in $i albums"
