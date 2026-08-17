#!/usr/bin/env bash
# **Repairing a missing playlist entry, end to end.**
#
# A playlist is written by hand with one entry pointing at a path that does not
# exist — `/gone/drive/Kesh/Signal Hill/03 - Gasworks.flac` — while a file of
# that exact name sits in the fixture library under a different prefix. That is
# the situation ADR-0024 §3 describes: a drive that moved.
#
# The run then does what a listener would: open the playlist, hover the broken
# row, press `Locate…`, and press the candidate. Frames either side.
#
# Private Xvfb, all six XDG variables redirected into a scratch tree.
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
OUT="$ROOT/docs/design/impl/locate-missing-entry"
S=$(mktemp -d)
DISP=":${DISPLAY_NUM:-95}"
W=1400; H=900
BIN="$ROOT/target/release/baz"
FIX=${FIX:-$ROOT/target/tmp/escape-fixture}
# Measured off 01-the-page.
ROW_X=${ROW_X:-900}
ROW_Y=${ROW_Y:-355}
LOCATE_X=${LOCATE_X:-1190}
CANDIDATE_X=${CANDIDATE_X:-1075}
CANDIDATE_Y=${CANDIDATE_Y:-381}

mkdir -p "$S"/{home,data,config,cache,run} "$S/config/baz" "$S/data/baz/playlists" "$OUT"
chmod 700 "$S/run"
cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "alphabet"
density = "compact"
sidebar_open = true
EOF

# **The playlist.** Three entries: two that resolve, and one whose drive is
# gone but whose filename is still in the library.
REAL_A=$(find "$FIX" -name '*.flac' | sort | sed -n '1p')
REAL_B=$(find "$FIX" -name '*.flac' | sort | sed -n '2p')
LOST=$(find "$FIX/06 - Kesh - Signal Hill" -name '*.flac' | sort | sed -n '3p')
LOST_NAME=$(basename "$LOST")
{
  echo "#EXTM3U"
  echo "$REAL_A"
  echo "/gone/drive/Kesh/Signal Hill/$LOST_NAME"
  echo "$REAL_B"
} > "$S/data/baz/playlists/Road Trip.m3u8"
echo "the broken entry is /gone/drive/Kesh/Signal Hill/$LOST_NAME"
echo "the file itself is   $LOST"

Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp >/dev/null 2>&1 &
XPID=$!
sleep 2
export DISPLAY=$DISP
cleanup() {
  [[ -n ${APID:-} ]] && kill "$APID" 2>/dev/null
  local pid
  pid=$(pgrep -x -f "$BIN" || true)
  [[ -n $pid ]] && kill $pid 2>/dev/null
  kill "$XPID" 2>/dev/null
  return 0
}
trap cleanup EXIT INT TERM

env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
    "$BIN" >> "$S/app.log" 2>&1 &
APID=$!

WID=""
for _ in $(seq 1 80); do
  WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.5
done
[[ -z $WID ]] && { echo "baz never opened a window; see $S/app.log" >&2; exit 1; }
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 5

shot() { import -window "$WID" "$OUT/$1.png"; }
park() { xdotool mousemove "$1" "$2"; sleep 0.4; xdotool mousemove $(($1 + 1)) $(($2 + 1)); sleep 1.2; }
click() { park "$1" "$2"; xdotool click 1; sleep 2; }

# Playlists in the lane, then the list itself.
click 89 185
shot 00-playlists
park 376 626
click 321 571
shot 01-the-page

# Hover the broken row so its slots are offered, then press `Locate…`.
park "$ROW_X" "$ROW_Y"
shot 02-row-hovered
click "$LOCATE_X" "$ROW_Y"
shot 03-the-card

# Confirm the candidate.
click "$CANDIDATE_X" "$CANDIDATE_Y"
shot 04-repaired

echo "--- the file on disk afterwards:"
cat "$S/data/baz/playlists/Road Trip.m3u8"
