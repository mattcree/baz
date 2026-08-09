#!/usr/bin/env bash
# Render the frames the hover options are argued from — headless, on a private
# Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md. Nothing touches
# the owner's session; the run's `[mpris] no session bus` line is the receipt
# that it did not, and this script prints it.
#
# What it captures against the real binary, at 1280×860 and 1920×1080:
#
#   1. The wall at rest — the baseline every veil frame is diffed against.
#   2. A **bright** sleeve hovered from its caption: the four options with no row
#      lit, over the brightest artwork in the fixture. This is the frame the
#      sampled-pixel table is computed from, because a pointer on the caption
#      reveals the options without washing any row.
#   3. The same sleeve with the pointer **on the `Play` row**: the light wash
#      brightening from the left.
#   4. A **near-black** sleeve hovered, so the veil is shown over the case it
#      is hardest on.
#   5. The record sounding after **one** press of `Play` on the wall, so the
#      bar carries the 52 px cover.
#   6. A press on the same sleeve *outside* the option band: the record's page
#      opens, exactly as it always did.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-hover-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-hover-fix \
#     docs/design/impl/hover-options/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-hover-fix}
OUT=${OUT:-$REPO/docs/design/impl/hover-options}
DISP=${DISP:-:198}
S=/tmp/baz-hover-scratch

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

# One kept list, so `Add to…` has a picker to be an option for.
mkdir -p "$S/data/baz/playlists"
{ echo "#EXTM3U"; find "$FIX" -name "*.flac" | sort | head -3; } \
  > "$S/data/baz/playlists/Road Trip.m3u8"

shoot() { # W H suffix
  local W=$1 H=$2 SUF=$3
  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp &
  local XPID=$!
  sleep 1
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$BIN" >> "$S/app-$SUF.log" 2>&1 &
  local APID=$!
  local WID=""
  for _ in $(seq 1 40); do
    WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW at $SUF"; cat "$S/app-$SUF.log"; kill "$APID" "$XPID" 2>/dev/null; return 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$W" "$H"
  xdotool windowfocus --sync "$WID"
  sleep 5

  shot()  { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
  move()  { xdotool mousemove "$1" "$2"; sleep 0.7; }
  click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.9; }

  # **Measured off the rest frame**, the way `everyday-flow/capture.sh` pins
  # its tile: the wall centres its block, so a tile's box is a function of the
  # width and there is nothing to compute at run time. Row 1 column 1 is a
  # bright sleeve; row 2 column 1 is a dark one.
  #
  #   1280×860 : sleeve x 42–281, y 131–370 (edge 240); row 2 y 510–749
  #   1920×1080: sleeve x 42–293, y 131–382 (edge 252); row 2 y 522–774
  local SX=42 SY=131 EDGE=240 DARKY=510
  if [[ $W -ge 1920 ]]; then EDGE=252; DARKY=522; fi
  local CX=$((SX + EDGE / 2))
  # The caption's title line, one GAP_LG under the sleeve. Hovering **here**
  # reveals the options with no option row washed, which is what the pixel
  # table needs: every pixel it reads is veil over artwork and nothing else.
  local CAPY=$((SY + EDGE + 26))
  # The `Play` row is the first quarter of the sleeve; the pointer goes inside
  # the hit band, which ends at 68 % of the width.
  local PLAYX=$((SX + EDGE / 4)) PLAYY=$((SY + EDGE / 8))

  # The pointer parks in empty wall: over the bar it would draw a transport
  # tooltip into every frame that follows.
  local PARK_X=$((W - 260)) PARK_Y=$((H - 200))
  move $PARK_X $PARK_Y
  shot "01-wall-at-rest-${SUF}"
  move $CX $CAPY
  shot "02-options-bright-sleeve-${SUF}"
  move $PLAYX $PLAYY
  shot "03-options-play-row-hovered-${SUF}"
  move $CX $((DARKY + EDGE + 26))
  shot "04-options-dark-sleeve-${SUF}"
  # `Play` from the wall: **one press to sound**. The bar then carries the
  # 52 px cover beside the track and artist.
  move $PLAYX $PLAYY
  click $PLAYX $PLAYY
  move $PARK_X $PARK_Y
  sleep 2
  shot "05-bar-with-cover-${SUF}"
  # **A press on the sleeve outside an option still opens the record's page.**
  # The hit band ends at 68 % of the width, so this lands in the right third,
  # where the veil has already dissolved and the cover is as painted.
  click $((SX + EDGE - 24)) $((SY + EDGE / 2))
  move $PARK_X $PARK_Y
  shot "06-press-outside-an-option-opens-the-page-${SUF}"

  kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
  echo "--- isolation receipt ($SUF) ---"
  grep -m1 mpris "$S/app-$SUF.log" || echo "NO MPRIS LINE — the isolation is unproven"
}

shoot 1280 860 1280x860
shoot 1920 1080 1920x1080
