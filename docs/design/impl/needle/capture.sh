#!/usr/bin/env bash
# Render the needle and the 57 px bar headless, on a private Xvfb, with all six
# XDG redirections from docs/DEVELOPMENT.md. Nothing touches the owner's
# session; nothing is audible.
#
#   bash docs/design/composition/tools/mkfixture.sh /tmp/baz-comp-fixture
#   toolbox run -c baz-dev cargo build --release --all-features
#   toolbox run -c baz-dev bash docs/design/impl/needle/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=$REPO/target/release/baz
FIX=${FIX:-/tmp/baz-comp-fixture}
OUT=${OUT:-$REPO/docs/design/impl/needle}
W=${W:-1280}
H=${H:-860}
DISP=${DISP:-:181}
S=/tmp/baz-needle-scratch-$W

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
# Two independent guarantees of silence: every sample in the fixture is a zero,
# and the default PCM is a null sink.
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
XPID=$!
sleep 1

env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
    "$BIN" "$FIX" > "$S/app.log" 2>&1 &
APID=$!

WID=""
for _ in $(seq 1 80); do
  WID=$(DISPLAY=$DISP xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW"; cat "$S/app.log"; kill $APID $XPID 2>/dev/null; exit 1; fi
export DISPLAY=$DISP
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 3

shot() { sleep 0.8; magick import -window root "$OUT/$1-${W}x${H}.png"; echo "  shot $1"; }
mv()   { xdotool mousemove "$1" "$2"; sleep 0.5; }
key()  { xdotool key "$@"; sleep 0.5; }
typ()  { xdotool type --delay 40 "$1"; sleep 0.8; }

PARK_X=$((W - 6)); PARK_Y=$((H / 2))

# --- stopped -----------------------------------------------------------------
mv $PARK_X $PARK_Y;                          shot 01-bar-stopped

# --- playing: "Closing Time", whose first track is an hour of silence, so the
#     null sink cannot burn through the queue while the frames are taken.
typ "Closing Time"; sleep 0.6
xdotool mousemove $((40 + 60)) $((H / 3)); sleep 0.3
xdotool click --repeat 2 --delay 120 1; sleep 1.5
key Escape; sleep 0.6
key ctrl+b; sleep 0.6
mv $PARK_X $PARK_Y;                          shot 02-bar-playing

# --- the needle's aiming band: a segment other than the sounding one, so the
#     tip names the record a click would play.
mv $((W * 3 / 4)) $((H - 4));                shot 03-needle-segment-hovered
# --- and inside the sounding entry, where the tip is a timestamp.
mv 90 $((H - 8));                            shot 04-needle-playing-entry-hovered

# --- paused -------------------------------------------------------------------
key space; mv $PARK_X $PARK_Y;               shot 05-bar-paused

# --- the wall it all pays for. The inspector goes so the frame is the
#     collection and its two strips, which is what the 46 px is spent on.
key space; sleep 0.5
key ctrl+b; sleep 0.6
mv $PARK_X $PARK_Y;                          shot 06-wall-playing

grep -c "no session bus" "$S/app.log" > /dev/null && echo "RECEIPT OK: $(grep -m1 mpris "$S/app.log")"
kill $APID 2>/dev/null; sleep 0.6; kill -9 $APID 2>/dev/null
kill $XPID 2>/dev/null; sleep 0.3; kill -9 $XPID 2>/dev/null
wait 2>/dev/null
echo "done ${W}x${H}"
