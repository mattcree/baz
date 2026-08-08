#!/usr/bin/env bash
# Render the index rail, headless, at rest and under the fisheye — on a private
# Xvfb with all six XDG redirections from docs/DEVELOPMENT.md. Nothing touches
# the owner's session; the run's `[mpris] no session bus` line is the receipt
# that it did not, and this script prints it.
#
# What it captures, at 1280×860 and again at 1920×1080:
#
#   <TAG>-01-rest            pointer parked clear of the rail
#   <TAG>-02-hover-mid       pointer on the rail, middle of the strip
#   <TAG>-03-hover-upper     pointer on the rail, upper quarter
#   <TAG>-04-hover-edge      pointer at the window's right edge, same y — the
#                            gutter is part of the hit lane (Fitts: the edge
#                            is the easiest target a pointer has)
#   <TAG>-05-hover-lower     pointer on the rail, lower quarter
#
# …plus a `-rail` crop of each (the right-hand 130 px), which is the strip a
# reviewer actually judges, and `TAG=before` against `TAG=after` is the whole
# argument.
#
# Build the binary **inside the toolbox** (docs/DEVELOPMENT.md: a host build
# links a newer glibc and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-rail
#   toolbox run -c baz-dev env TAG=after docs/design/impl/index-magnification/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-rail}
OUT=${OUT:-$REPO/docs/design/impl/index-magnification}
TAG=${TAG:-after}
DISP=${DISP:-:193}
S=/tmp/baz-rail-scratch

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

run_at() {
  local W=$1 H=$2
  Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
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
  if [[ -z $WID ]]; then echo "NO WINDOW at ${W}"; cat "$S/app.log"; kill $APID $XPID 2>/dev/null; exit 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$W" "$H"
  xdotool windowfocus --sync "$WID"
  sleep 4   # let the launch scan land

  # The rail's ink hangs from x = W − HANG (40); a single letter is ~7 px wide,
  # so x = W − 44 rests on the letters themselves.
  local RAIL_X=$((W - 44)) EDGE_X=$((W - 2)) MID=$((H / 2))
  crop() { magick "$OUT/$TAG-$1-${W}.png" -crop "170x$((H))+$((W - 170))+0" "$OUT/$TAG-$1-${W}-rail.png"
           echo "  shot $TAG-$1-${W}"; }
  # The rest frame carries no cursor (`import`), so before/after rest frames
  # diff on the surface alone; every hover frame carries the real cursor
  # (`maim` draws it), because the shape of the hand over a jump is part of
  # what these frames exist to prove.
  shot()  { sleep 0.9; magick import -window root "$OUT/$TAG-$1-${W}.png"; crop "$1"; }
  cshot() { sleep 0.9; maim "$OUT/$TAG-$1-${W}.png"; crop "$1"; }
  # Parked clear of the rail *and* the needle (see places/capture.sh).
  xdotool mousemove $((W / 2)) 120;                       shot  01-rest
  xdotool mousemove "$RAIL_X" "$MID";                     cshot 02-hover-mid
  xdotool mousemove "$RAIL_X" $((MID - H / 4));           cshot 03-hover-upper
  xdotool mousemove "$EDGE_X" "$MID";                     cshot 04-hover-edge
  xdotool mousemove "$RAIL_X" $((MID + H / 4));           cshot 05-hover-lower

  # Clean up **only what this script started**, by pid, never by name: the
  # owner runs his own baz.
  kill $APID 2>/dev/null; wait $APID 2>/dev/null
  kill $XPID 2>/dev/null; wait $XPID 2>/dev/null
}

run_at 1280 860
run_at 1920 1080

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]|^\[startup\] room" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
