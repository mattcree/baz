#!/usr/bin/env bash
# Render the wall's scrollbar where it was and where it is now — headless, on a
# private Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md. Nothing
# touches the owner's session; the run's `[mpris] no session bus` line is the
# receipt that it did not, and this script prints it.
#
# The defect, in the owner's words: *"scroll bar is in a strange location… it
# seems to have padding on the right"*. The bar was drawn at the right edge of
# the **wall's scrollable**, with the index rail's `INDEX_LANE_W` 108 standing
# outboard of it, so 108 px of window sat to the right of a bar that read as
# floating in the middle.
#
# What this captures, at 1280 × 860 and 1920 × 1080, with the lane open and
# again collapsed:
#
#   1. **before** — the bar at the wall's edge, the rail outboard of it;
#   2. **after** — the bar on the window's edge, the rail inboard, every other
#      x unchanged;
#   3. the **right-hand strip of each**, cropped and stacked, which is where
#      the whole change is;
#   4. the **difference**, so that "nothing else moved" is a picture and not a
#      claim.
#
# `measure.py` then reports the bar's and the rail's x-ranges off the frames —
# the numbers that go in the commit message.
#
# Build **both** binaries inside the toolbox (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   git worktree add --detach /tmp/baz-scrollbar-before <the base commit>
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=$PWD/target/tb bash -lc \
#     'cd /tmp/baz-scrollbar-before && cargo build --release -p baz --features device-output'
#   cp target/tb/release/baz /tmp/baz-bin-before
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=$PWD/target/tb \
#     cargo build --release -p baz --features device-output
#   cp target/tb/release/baz /tmp/baz-bin-after
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-scrollbar-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-scrollbar-fix \
#     docs/design/impl/wall-scrollbar/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BEFORE=${BEFORE:-/tmp/baz-bin-before}
AFTER=${AFTER:-/tmp/baz-bin-after}
FIX=${FIX:-/tmp/baz-scrollbar-fix}
OUT=${OUT:-$REPO/docs/design/impl/wall-scrollbar}
DISP=${DISP:-:198}
S=/tmp/baz-scrollbar-scratch

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

write_config() { # sidebar_open
  mkdir -p "$S/config/baz"
  cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "artist"
density = "balanced"
sidebar_open = $1
EOF
}

launch() { # binary W H
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$1" >> "$S/app.log" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 40); do
    WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$2" "$3"
  xdotool windowfocus --sync "$WID"
  sleep 6   # let the launch scan land
}

resize() { xdotool windowsize "$WID" "$1" "$2"; sleep 1.5; }
stop()   { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()   { sleep 1.0; magick import -window root -crop "${W}x${H}+0+0" +repage \
             "$OUT/$1.png"; echo "  shot $1"; }
# Park the pointer where it states nothing and — critically — **off the index
# rail**, whose fisheye would otherwise magnify a letter and move the ink this
# capture exists to measure. The wall's own left edge, low down.
park()   { xdotool mousemove 60 $((H - 120)); sleep 0.8; }
key()    { xdotool key "$@"; sleep 0.7; }

Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

# One scenario, rendered by each binary in turn, so the only difference between
# the two frames is the code. The wall is scrolled a little way down in both:
# a bar with its scroller parked at the very top reads as a cap rather than as
# a handle, and the scroller's own ink is what the ruler finds.
run() { # binary tag
  write_config true
  launch "$1" $W $H
  park
  key Down; key Down; key Down
  park
  shot "$2"
  # …and collapsed, where the wall is at its widest and the rail is the only
  # thing between the covers and the window's edge.
  key ctrl+b
  park
  shot "${2/-lane-open-/-lane-shut-}"
  stop
}

# **The two affordances, driven** — the half a still frame cannot show. The
# rail lost the outer `WALL_SCROLLBAR_W` of its press band and kept the rest,
# and the bar has to be grabbable in the 4 px it gained, or the whole move is a
# picture of a scrollbar.
interact() { # binary
  write_config true
  launch "$1" $W $H
  park
  shot 09-at-the-top-of-the-wall

  # A rail jump from **one pixel inboard of the bar** — the narrowest part of
  # the Fitts band that survives the move. `S` at the wall's top, which the
  # fixture always has records under.
  xdotool mousemove $((W - 5)) 497; sleep 0.5
  xdotool click 1; sleep 1.5
  park
  shot 10-the-rail-jumped-from-one-px-inboard

  # …and the bar grabbed **at the window's edge** and dragged to the foot,
  # which is the gesture the bar exists for and the 4 px it was given.
  xdotool mousemove $((W - 3)) 120; sleep 0.4
  xdotool mousedown 1; sleep 0.3
  xdotool mousemove $((W - 3)) 400; sleep 0.3
  xdotool mousemove $((W - 3)) $((H - 80)); sleep 0.5
  xdotool mouseup 1; sleep 0.8
  park
  shot 11-the-bar-dragged-to-the-end
  stop
}

# ---------------------------------------------------------------- 1280 × 860
W=1280; H=860
run "$BEFORE" 01-lane-open-before-1280
run "$AFTER"  02-lane-open-after-1280
interact "$AFTER"

# --------------------------------------------------------------- 1920 × 1080
W=1920; H=1080
run "$BEFORE" 05-lane-open-before-1920
run "$AFTER"  06-lane-open-after-1920

kill "$XPID" 2>/dev/null

# **The right-hand strip, stacked** — 160 px of window edge from each frame,
# before over after, which is the whole of the change at a size a person can
# see without a ruler.
strip() { # before after out width
  magick "$OUT/$1.png" -crop "160x${4}+$(( $5 - 160 ))+0" +repage "$S/a.png"
  magick "$OUT/$2.png" -crop "160x${4}+$(( $5 - 160 ))+0" +repage "$S/b.png"
  magick "$S/a.png" "$S/b.png" +append -bordercolor gray20 -border 2 "$OUT/$3.png"
  echo "  strip $3"
}
strip 01-lane-open-before-1280 02-lane-open-after-1280 \
      03-edge-strip-1280 860 1280
strip 01-lane-shut-before-1280 02-lane-shut-after-1280 \
      04-edge-strip-lane-shut-1280 860 1280
strip 05-lane-open-before-1920 06-lane-open-after-1920 \
      07-edge-strip-1920 1080 1920

# **The difference**: everything that is not the bar should be black.
magick "$OUT/01-lane-open-before-1280.png" "$OUT/02-lane-open-after-1280.png" \
  -compose difference -composite -auto-level "$OUT/08-diff-1280.png"
echo "  shot 08-diff-1280"

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo
echo "--- the bar and the rail, measured ---"
"$REPO/docs/design/impl/wall-scrollbar/measure.py" "$OUT"
