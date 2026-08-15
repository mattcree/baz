#!/usr/bin/env bash
# **The sweep** — every place in baz, at three widths, in one run.
#
# The owner, 2026-08-15: *"can we do some more passes on the UI"*, after a
# review of the Vibe page whose three loudest complaints generalise: the wide
# window is not designed for, the narrow one is where things break, and
# controls that do not look like controls are everywhere once you look.
#
# **No click navigation.** Every earlier capture in this repository walked the
# interface by coordinate and three of them broke silently when the layout
# moved. `last_place` in `config.toml` restores any place on launch — including
# an album, an artist and a playlist, whose ids are FNV-1a folds this script
# computes the same way `vm::album_id` does — so a frame is one clean process
# per (place × width) and nothing depends on where a control happens to be.
#
# Widths, and why these three:
#
#   1000 × 700   the narrow window, above the 864 px floor and below every
#                two-column breakpoint the product has
#   1280 × 860   the window the design documents are argued at
#   1600 × 900   a desktop window, where a single Fill column stretches
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev env FIX=/tmp/baz-review-fix \
#     docs/design/impl/ui-sweep/capture.sh
#
# `ONLY=library,settings` shoots a subset; `WIDTHS="1600x900"` one width.
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-review-fix}
OUT=${OUT:-$REPO/docs/design/impl/ui-sweep/frames}
DISP=${DISP:-:210}
S=/tmp/baz-sweep-scratch
WIDTHS=${WIDTHS:-"1000x700 1280x860 1600x900"}

# The three id-bearing places, folded exactly as `vm::album_id`,
# `vm::named_artist_id` and `playlists::playlist_id` fold them.
ids() {
python3 - "$1" "$2" <<'PY'
import sys
def fnv1a(h, data):
    for b in data:
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h
B = 0xcbf29ce484222325
artist, album = sys.argv[1], sys.argv[2]
h = fnv1a(fnv1a(B, artist.lower().encode()), b"\x00")
print("artist:%d" % h)
print("album:%d" % fnv1a(fnv1a(h, album.lower().encode()), b"\x00"))
print("playlist:%d" % fnv1a(B, b"Road Trip"))
PY
}

mapfile -t PLACE_IDS < <(ids "Halvard Sten" "Closing Time")
ARTIST=${PLACE_IDS[0]}
ALBUM=${PLACE_IDS[1]}
PLAYLIST=${PLACE_IDS[2]}

PLACES=${ONLY:-"home library $ALBUM $ARTIST playlists $PLAYLIST favourites queue now-playing new-playlist settings"}

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF

mkdir -p "$S/data/baz/playlists"
{ echo "#EXTM3U"; find "$FIX" -name '*.flac' | sort | head -9; } \
  > "$S/data/baz/playlists/Road Trip.m3u8"
{ echo "#EXTM3U"; find "$FIX" -name '*.flac' | sort | tail -4; } \
  > "$S/data/baz/playlists/Sunday Morning.m3u8"

shoot() { # place  W  H
  local place=$1 W=$2 H=$3
  local name
  case "$place" in
    album:*)    name=album ;;
    artist:*)   name=artist ;;
    playlist:*) name=playlist ;;
    *)          name=$place ;;
  esac

  cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "alphabet"
last_place = "$place"
EOF

  Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
  local XPID=$!
  sleep 1
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$BIN" >> "$S/app.log" 2>&1 &
  local APID=$!
  local WID=""
  for _ in $(seq 1 40); do
    WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then
    echo "  NO WINDOW for $name at ${W}x${H}"
    kill "$APID" "$XPID" 2>/dev/null
    return 1
  fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$W" "$H"
  xdotool windowfocus --sync "$WID"
  # The scan, the sleeve decodes, and the art requests the first frame makes.
  sleep 7
  # Park where no tile, row or control is: the strip's own empty right end.
  # A picture of the composition must not be a picture of the pointer.
  xdotool mousemove $((W - 60)) 78
  sleep 0.4
  xdotool mousemove $((W - 58)) 80
  sleep 1.0
  magick import -window root "$OUT/${name}-${W}x${H}.png"
  echo "  shot ${name}-${W}x${H}"

  kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
}

mkdir -p "$S/config/baz"
for size in $WIDTHS; do
  W=${size%x*}
  H=${size#*x}
  for place in $PLACES; do
    shoot "$place" "$W" "$H"
  done
done

echo "--- isolation receipt ---"
grep -m1 mpris "$S/app.log" || echo "NO MPRIS LINE — the isolation is unproven"
echo "frames: $(find "$OUT" -name '*.png' | wc -l) in $OUT"
