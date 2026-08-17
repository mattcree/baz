#!/usr/bin/env bash
# **One press of Escape, with the caret in the search well.**
#
# The unit tests pin which message the key produces. They cannot tell you what
# a listener sees, because the thing that was broken lived in the toolkit: iced
# consumed the press to blur the field, and the query stayed on the wall. So
# this drives a real baz on a real X server and photographs the wall before the
# press and after it.
#
# It uses the fixture library rather than the owner's, because the question is
# whether the query goes away and any six records answer that. All six XDG
# variables are redirected into a scratch tree and the display is a private
# Xvfb, so nothing here touches the owner's library, config or session bus.
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
OUT="$ROOT/docs/design/impl/escape-in-the-well"
S=$(mktemp -d)
DISP=":${DISPLAY_NUM:-97}"
W=1400; H=900
BIN="$ROOT/target/release/baz"

FIX=${FIX:-$ROOT/target/tmp/escape-fixture}
if [[ ! -d $FIX ]]; then
  echo "no fixture at $FIX — run docs/design/impl/contour/mkfixture-varied.sh first" >&2
  exit 1
fi

mkdir -p "$S"/{home,data,config,cache,run} "$S/config/baz" "$OUT"
chmod 700 "$S/run"
cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "alphabet"
density = "compact"
sidebar_open = true
EOF

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
sleep 4

shot() { import -window "$WID" "$OUT/$1.png"; }

# Type-anywhere: the letters open the well and put the caret in it, which is
# the state the whole question is about.
xdotool type --clearmodifiers --delay 60 "kesh"
sleep 2
shot 01-query-typed

# **One press.**
xdotool key --clearmodifiers Escape
sleep 2
shot 02-after-one-escape

echo "wrote $OUT/01-query-typed.png and $OUT/02-after-one-escape.png"
