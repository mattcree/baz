#!/usr/bin/env bash
# Render doc 10 §7 — controls and iconography — headless, on a private
# Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md. Nothing
# touches the owner's session; the run's `[mpris] no session bus` line is
# the receipt that it did not, and this script prints it.
#
# What it shows against the real binary:
#
#   1. **The single-line strip at 1280**: the well wearing the magnifier
#      with the counts as its placeholder, the triangle on `Play all`, and
#      the gear in the corner where the 84 px word used to stand.
#   2. **The gear's tooltip** — the accessible name of the strip's one
#      icon-only door (ADR-0017 §4c).
#   3. **The match count in the well** — `N / total` in its reserved slot
#      beside the caret, where the readout used to float ~1 100 px from
#      the keys producing it.
#   4. **The single-line regime at its 960 floor**, and **the split at
#      760 and 600**: frame line (well · notes · doors), library line
#      (states · acts). Nothing hides, nothing overflows, no menu.
#   5. **The queue rows' drawn glyph slots** — ↑ ↓ ✕ + as one mark
#      technology at one stroke weight.
#   6. **The settings steppers' drawn − / +**, under the place_header the
#      Settings place now shares with every other place.
#   7. **The bottom bar untouched**: with BASE_BIN set to a binary built
#      from the branch base, the bar's crop is captured from both builds
#      and diffed — the study examines the bar and deliberately leaves it
#      alone (doc 10 §4.4).
#
# Build the binary **inside the toolbox** (a host-built release binary
# links a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-ci-fix
#   toolbox run -c baz-dev env BASE_BIN=/path/to/base/baz \
#     docs/design/impl/controls-iconography/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-ci-fix}
OUT=${OUT:-$REPO/docs/design/impl/controls-iconography}
DISP=${DISP:-:199}
S=/tmp/baz-ci-scratch

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
# Two independent guarantees that nothing is audible: the sink discards
# every sample, and the fixture's samples are all zero (docs/DEVELOPMENT.md).
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

launch() { # BINARY
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
  if [[ -z $WID ]]; then echo "NO WINDOW"; cat "$S/app.log"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowfocus --sync "$WID"
  sleep 4   # let the launch scan land
}

resize() { # W H
  W=$1; H=$2
  xdotool windowsize "$WID" "$W" "$H"
  sleep 1.2
}

shot()  { # NAME  — the window's crop out of the fixed root
  sleep 0.9
  magick import -window root -crop "${W}x${H}+0+0" +repage "$OUT/$1.png"
  echo "  shot $1"
}
key()   { xdotool key "$@"; sleep 0.5; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.5; }
park()  { xdotool mousemove $((W - 6)) 400; sleep 0.3; }

Xvfb "$DISP" -screen 0 1280x1000x24 -nolisten tcp &
XPID=$!
sleep 1

# ---- the branch binary -----------------------------------------------------
launch "$BIN"
resize 1280 860

# 1. The strip at rest: magnifier + counts placeholder, ▶ Play all, gear.
park; shot 01-strip-single-line-1280x860
# 2. The gear's tooltip. The gear is the last 32 px box before the right
#    gutter: centre ≈ (1280 − 40 − 16, 24).
xdotool mousemove 1224 24; sleep 1.0
shot 02-gear-hover-tooltip-1280x860
# 3. Type (type-anywhere reaches the well): the match count lands in the
#    well's reserved right-hand slot.
park
xdotool type --delay 60 "low"; sleep 0.8
shot 03-well-match-count-1280x860
key Escape   # empty the query; the counts return as the placeholder
# 4. The regimes: 960 is the single-line floor; 760 and 600 are the split.
resize 960 860
park; shot 04-strip-single-line-floor-960x860
resize 760 860
park; shot 05-strip-split-760x860
resize 600 860
park; shot 06-strip-split-600x860
resize 1280 860
# 5. Play all → pause → the queue place; hover a row: the drawn ↑ ↓ ✕ +
#    arrive in their reserved lanes. (Coordinates: the acts cluster starts
#    at 682 = 40 + 280 + 24 + 314 + 24; the queue list is the 880 px
#    measure centred at a 1280 window, slots at its right edge.)
click 700 24
sleep 0.8
key space
key ctrl+u
xdotool mousemove 600 236; sleep 0.8
shot 07-queue-row-glyphs-1280x860
key Escape
# 6. Settings: the drawn − / + pair, under the shared place_header.
key ctrl+comma
park; shot 08-settings-steppers-1280x860
key Escape

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null

# ---- the bottom bar, diffed against the branch base ------------------------
# The study examines the bar and leaves it alone (doc 10 §4.4); the crop
# proves it. The bar is BAR_CONTENT_H 80 + hairline 1 + needle 2 = 83 px.
W=1280; H=860
magick "$OUT/01-strip-single-line-1280x860.png" -crop 1280x83+0+777 +repage \
  "$OUT/10-bottom-bar-after.png"
if [[ -n ${BASE_BIN:-} && -x ${BASE_BIN:-} ]]; then
  launch "$BASE_BIN"
  resize 1280 860
  park; sleep 0.9
  magick import -window root -crop "1280x860+0+0" +repage "$S/base-window.png"
  magick "$S/base-window.png" -crop 1280x83+0+777 +repage \
    "$OUT/09-bottom-bar-before.png"
  kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
  DIFF=$(magick compare -metric AE \
    "$OUT/09-bottom-bar-before.png" "$OUT/10-bottom-bar-after.png" null: 2>&1 || true)
  echo "bottom bar diff (pixels differing): $DIFF"
else
  echo "BASE_BIN unset — bottom-bar-before not captured"
fi

kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]|^\[startup\] room" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
