#!/usr/bin/env bash
# Render doc 09 §13 steps 5–7 — queue-place edit parity, `Play all`, and
# shift-click — headless, on a private Xvfb, with all six XDG redirections
# from docs/DEVELOPMENT.md. Nothing touches the owner's session; the run's
# `[mpris] no session bus` line is the receipt that it did not, and this
# script prints it.
#
# What it shows against the real binary, at 1280×860 and 1920×1080:
#
#   1. **`Play all` in the Library strip**, leading `Shuffle` and `Pull` —
#      the wall's three acts, one cluster (07-control-placement L8.1).
#   2. **One press reifies the wall**: every visible record, whole, in the
#      arrangement's order, playing from the top — the queue place opens on
#      a 25-record, 200-plus-track run with an ordinary summary. (Playback
#      is paused right after the press so the cursor holds still for the
#      stills; the null sink otherwise races through silent tracks.)
#   3. **Edit parity on the queue's rows**: hover reveals ▲▼ ✕ + in the
#      playlist page's reserved slots; a ▼ press swaps two rows under the
#      paused run; the `+` opens the panel as the picker holding the row's
#      track, Queue row first.
#   4. **Shift-click a sleeve appends the record** — the run is not
#      replaced, and the record joins the tail as its own headed group,
#      reached by scrolling the (virtualized) place to its end.
#
# Build the binary **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-qp-fix
#   toolbox run -c baz-dev docs/design/impl/queue-parity/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-qp-fix}
OUT=${OUT:-$REPO/docs/design/impl/queue-parity}
DISP=${DISP:-:198}
S=/tmp/baz-qp-scratch

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

launch() { # W H
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
  if [[ -z $WID ]]; then echo "NO WINDOW"; cat "$S/app.log"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$1" "$2"
  xdotool windowfocus --sync "$WID"
  sleep 4   # let the launch scan land
}

shot()  { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
key()   { xdotool key "$@"; sleep 0.5; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.5; }
park()  { xdotool mousemove $((W - 6)) 300; }

# ---- 1280 × 860 -----------------------------------------------------------
W=1280; H=860
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp &
XPID=$!
sleep 1
launch $W $H

# The strip's coordinates at 1280 (measured off the rendered frame): the
# wall-acts cluster after the group keys, and the queue place's slot lanes
# on the 880 px list centred in the window.
PA_X=785;  PA_Y=24          # `Play all`, first of the three acts
ROW_X=600                    # a queue row's body (list left edge ≈ 195)
Y1=204; Y2=236; Y3=268       # first three one-line rows' centres
STEP_UP_X=979; STEP_DN_X=1007; REMOVE_X=1035; PLUS_X=1063
TILE2_X=444; TILE2_Y=250     # second sleeve on the wall (Violet Ledger)

# 1. The wall at rest: `Play all` leads Shuffle and Pull in the strip.
park; shot 01-strip-play-all-1280x860
# 2. One press: the wall becomes the queue, the first track sounds — then
#    Space pauses so the run holds still for the stills.
click $PA_X $PA_Y
sleep 0.8
key space
key ctrl+u
park; shot 02-play-all-queue-1280x860
# 3. Hover a row: the playlist page's slots — ↑ ↓ ✕ + — arrive in their
#    reserved lanes, and nothing shifts sideways.
xdotool mousemove $ROW_X $Y2; sleep 0.6
shot 03-queue-row-parity-1280x860
# 4. ↓ on that row: the entry swaps with its neighbour, the run keeps
#    playing (here: stays paused), the numbers renumber.
click $STEP_DN_X $Y2
xdotool mousemove $ROW_X $Y3; sleep 0.6
shot 04-queue-reordered-1280x860
# 5. The row's `+`: the panel opens as the picker holding this row's track,
#    the Queue row first among the destinations.
click $PLUS_X $Y3
park; shot 05-transfer-picker-1280x860
# 6. Shift-click a sleeve: back on the wall, the record is appended to the
#    run — not played, not resumed — and the bar's continuation line counts
#    one more album.
key Escape          # put the pick down
key Escape          # close the panel
key Escape          # leave the queue place for the wall
xdotool keydown shift; sleep 0.2
click $TILE2_X $TILE2_Y
xdotool keyup shift; sleep 0.3
park; shot 06-wall-after-shift-click-1280x860
# 7. The tail: the appended record is its own headed group at the end of
#    the run — reached by scrolling the virtualized place to its bottom.
key ctrl+u
xdotool mousemove 600 400
xdotool click --repeat 240 --delay 8 5   # wheel to the tail of the run
park; shot 07-queue-tail-appended-1280x860

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

# ---- 1920 × 1080 ----------------------------------------------------------
W=1920; H=1080
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp &
XPID=$!
sleep 1
launch $W $H

ROW_X=900                    # list left edge ≈ 515 at 1920
STEP_DN_X=1327; PLUS_X=1383

park; shot 08-strip-play-all-1920x1080
click $PA_X $PA_Y
sleep 0.8
key space
key ctrl+u
park; shot 09-play-all-queue-1920x1080
xdotool mousemove $ROW_X $Y2; sleep 0.6
shot 10-queue-row-parity-1920x1080

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]|^\[startup\] room|^\[play-all\]" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
