#!/usr/bin/env bash
# Render every place ADR-0022 ships, headless, on a private Xvfb, with all six
# XDG redirections from docs/DEVELOPMENT.md. Nothing touches the owner's
# session; the run's `[mpris] no session bus` line is the receipt that it did
# not, and this script prints it.
#
# It exercises the whole decision against the real binary at two window sizes:
# the wall with no scrollbar, a record's page reached by pressing a tile, the
# queue place reached by the bar's labelled door, and — the route R3 asked for —
# the bar's now-playing text pressed to land back on the record that is playing.
#
# Build the binary **inside the toolbox**: a release binary built on the host
# links a newer glibc than the container has, and the run dies before it draws.
#
#   toolbox run -c baz-dev cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-places
#   toolbox run -c baz-dev docs/design/impl/places/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=$REPO/target/release/baz
FIX=${FIX:-/tmp/baz-places}
OUT=${OUT:-$REPO/docs/design/impl/places}
W=${W:-1280}
H=${H:-860}
DISP=${DISP:-:191}
S=/tmp/baz-places-scratch

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
# Two independent guarantees that nothing is audible: the sink discards every
# sample, and the fixture's samples are all zero (docs/DEVELOPMENT.md).
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF

mkdir -p "$S/config/baz"
cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "artist"
EOF

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
XPID=$!
sleep 1

env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
    "$BIN" > "$S/app.log" 2>&1 &
APID=$!

WID=""
for _ in $(seq 1 40); do
  WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW"; cat "$S/app.log"; kill $APID $XPID 2>/dev/null; exit 1; fi
export DISPLAY=$DISP
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 4   # let the launch scan land

shot() { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
klick(){ xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.9; }
key()  { xdotool key "$@"; sleep 0.6; }

# **Parked clear of the needle**, not in the corner: the needle runs the full
# width of the window's bottom edge and shows a hover tip, so a pointer resting
# in the bottom-right corner puts a floating label into every frame — which the
# composition ruler then measures as a mark in the bar.
PARK_X=$((W - 6)); PARK_Y=200
park() { xdotool mousemove $PARK_X $PARK_Y; }

# 1. The wall, and the right-hand edge: the index rail with **nothing** beside
#    it. The pointer is parked in the corner so no tile carries a hover rule.
park; shot 01-wall-no-scrollbar

# 2. A tile press: one press, and the record's page has replaced the wall.
klick 160 250                                   # the first sleeve
park; shot 02-album-place

# 3. Play it from the page — the control that replaced the wall's double-click.
klick 200 437                                   # `Play album`, under the sleeve
sleep 2
park; shot 03-album-playing

# 4. Esc back to the wall: the scroll is where it was and the record carries the
#    2 px rule that says it is the one you came back from.
key Escape
park; shot 04-wall-after-esc

# 5. The queue place, by the bar's labelled door.
klick 450 818                                   # `Queue` in the bar's left zone
park; shot 05-queue-place

# 6. …and a row's ✕, which is offered on hover only.
xdotool mousemove 700 300; sleep 0.9; shot 06-queue-row-hovered

# 7. **The route R3 asked for**: back to the wall, scroll somewhere else, then
#    press the now-playing text in the bar and land on the sounding record.
key Escape
xdotool mousemove 640 400
xdotool click 5; xdotool click 5; xdotool click 5; xdotool click 5
sleep 1
park; shot 07-wall-scrolled-away
klick 120 812                                   # the now-playing text itself
park; shot 08-back-to-playing

# 8. The Settings place, so the three headers can be compared as one frame.
key Escape
key ctrl+comma
park; shot 09-settings-place

key Escape
sleep 1

# 9. Everything again at 1920 × 1080, where the page's list reaches its measure
#    and the block centres.
kill $APID 2>/dev/null; wait $APID 2>/dev/null
kill $XPID 2>/dev/null; wait $XPID 2>/dev/null
W2=1920; H2=1080
Xvfb "$DISP" -screen 0 "${W2}x${H2}x24" -nolisten tcp &
XPID=$!
sleep 1
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
    "$BIN" >> "$S/app.log" 2>&1 &
APID=$!
WID=""
for _ in $(seq 1 40); do
  WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW (wide)"; cat "$S/app.log"; kill $APID $XPID 2>/dev/null; exit 1; fi
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W2" "$H2"
xdotool windowfocus --sync "$WID"
sleep 4
PARK_X=$((W2 - 6)); PARK_Y=200

park; shot 10-wall-1920
klick 160 250                                   # the first sleeve
park; shot 11-album-place-1920
klick 200 437                                   # `Play album`
sleep 2
key Escape
klick 450 1038                                  # the bar's `Queue` door
park; shot 12-queue-place-1920
key Escape
key ctrl+comma
park; shot 13-settings-place-1920

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]|^\[startup\] room" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"

# Clean up **only what this script started**, by pid, never by name: the owner
# runs his own baz and it has been killed twice by an agent's `pkill`.
kill $APID 2>/dev/null; wait $APID 2>/dev/null
kill $XPID 2>/dev/null; wait $XPID 2>/dev/null
echo "done — $OUT"
