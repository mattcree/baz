#!/usr/bin/env bash
# **The owner's own `O Brother, Where Art Thou?`, before and after.**
#
# He pointed at a photograph of his Home and said some albums are not grouped
# properly. This runs the real binary against a **copy** of his real
# `library.db` and searches for the record, so the count on screen is his data
# and not a fixture's.
#
# The copy matters: the database is copied into a scratch XDG tree and every
# one of the six variables is redirected, so nothing here can write to his
# library, his config, his session bus or his playlists. `music_dirs` is left
# empty on purpose — a scan would find the share unmounted and mark rows
# unavailable, and the question is about grouping rows the index already holds.
#
# Pass a baz binary as $1 to photograph a different build (the before frame is
# taken with a build from before the fix).
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
OUT="$ROOT/docs/design/impl/untagged-compilation"
BIN=${1:-$ROOT/target/release/baz}
LABEL=${2:-after}
S=$(mktemp -d)
DISP=":${DISPLAY_NUM:-94}"
W=1400; H=900

mkdir -p "$S"/{home,data,config,cache,run} "$S/config/baz" "$S/data/baz" "$OUT"
chmod 700 "$S/run"
cp ~/.local/share/baz/library.db "$S/data/baz/library.db"
# **A configured root that is not there.** An empty `music_dirs` lands on the
# first-run screen and shows no library at all; the real roots would send a
# scan across the owner's NAS to answer a question about rows the index
# already holds. A root that does not exist gives neither: baz is configured,
# the scan finds nothing to walk, and the scanner's positive-evidence gate
# means an unwalkable root prunes nothing (`crate::reach`). Every row survives
# and the wall is drawn from them, which is exactly the question.
printf 'music_dirs = ["/nonexistent-baz-grouping-proof"]\ngroup_key = "alphabet"\ndensity = "compact"\nsidebar_open = true\n' \
  > "$S/config/baz/config.toml"

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
for _ in $(seq 1 90); do
  WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.5
done
[[ -z $WID ]] && { echo "baz never opened a window; see $S/app.log" >&2; exit 1; }
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 8

xdotool type --clearmodifiers --delay 45 "o brother"
sleep 4
import -window "$WID" "$OUT/$LABEL-tracks.png"
# **Down to the ALBUMS section**, which is the whole question — the tracks
# above it are the same 57 either way, and what changed is how many records
# they are gathered into.
xdotool mousemove 400 400
for _ in $(seq 1 70); do xdotool click 5; done
sleep 3
import -window "$WID" "$OUT/$LABEL.png"
echo "wrote $OUT/$LABEL.png"
