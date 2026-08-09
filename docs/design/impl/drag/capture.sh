#!/usr/bin/env bash
# Render doc 09 §13 step 8 — the reorder drag (doc 11 P5's pointer-capture
# widget, ADR-0024 §6 layer 3) — headless, on a private Xvfb, with all six
# XDG redirections from docs/DEVELOPMENT.md. Nothing touches the owner's
# session; the run's `[mpris] no session bus` line is the receipt that it
# did not, and this script prints it.
#
# What it shows against the real binary, at 1280×860:
#
#   1. **A queue-row drag mid-flight**: `Play all` reifies the wall, Space
#      pauses it so the frames hold still, then the *sounding* row is
#      pressed and pulled down past the threshold — the ghost card names
#      it at the pointer and the insertion line sits on the boundary the
#      drop would commit to. (`xdotool mousedown … mousemove … mouseup` is
#      the drag; the mid-flight frames are taken with the button down.)
#   2. **The drop**: one whole-list `UpdateQueue` — the row lands where the
#      line said, the numbers renumber, and the summary's cursor follows
#      its track (ADR-0014's guarantee, visible).
#   3. **Drag-to-add**: with the panel standing (a playlist made first via
#      the row's `+` → `New playlist` — the picker route the drag is sugar
#      over), a queue row is carried over the panel's playlist row —
#      mid-flight the ghost rides over the panel (flipped inside the
#      window by the menu's own anchor) and the target draws the room's
#      hover statement — and the drop appends to the file: the row's
#      counts tick up.
#   4. **The playlist page's own drag**: the same gesture on the artefact's
#      rows, mid-flight and after — one atomic file save.
#
# Build the binary **inside the toolbox** (a host-built release binary
# links a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-drag-fix
#   toolbox run -c baz-dev docs/design/impl/drag/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-drag-fix}
OUT=${OUT:-$REPO/docs/design/impl/drag}
DISP=${DISP:-:199}
S=/tmp/baz-drag-scratch

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
park()  { xdotool mousemove $((W - 6)) 500; sleep 0.6; }
# A held pull: press at the first point, travel through the rest in steps
# (every step is a CursorMoved the rows measure; the first exceeds the 8 px
# threshold, so the lift happens on the way). No release — the mid-flight
# frame is taken with the button down, and drop() ends the gesture.
pull() { # "x y" "x y" ...
  local first=$1; shift
  # shellcheck disable=SC2086
  xdotool mousemove $first; sleep 0.4
  xdotool mousedown 1; sleep 0.3
  local p
  for p in "$@"; do
    # shellcheck disable=SC2086
    xdotool mousemove $p; sleep 0.08
  done
  sleep 0.5
}
drop() { xdotool mouseup 1; sleep 0.8; }

# ---- 1280 × 860 -----------------------------------------------------------
W=1280; H=860
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp &
XPID=$!
sleep 1
launch $W $H

# The place's coordinates at 1280 (measured off the rendered frames): the
# 880 px list centred in the window (left edge ≈ 195), one-line queue rows
# at a 32 px pitch from the first row's centre at y = 209, the hovered
# row's `+` in its reserved lane, and the panel's 340 px band on the right.
PA_X=785;  PA_Y=24            # `Play all`, first of the wall's three acts
ROW_X=600                     # a queue row's body
Y1=209; Y2=241; Y3=273        # the first three rows' centres
PLUS_X=1063                   # the hovered row's transfer `+`
NEW_X=1005; NEW_Y=737         # the panel's `New playlist` row
PR_X=1100;  PR_Y=126          # the panel's first named playlist row
PGY1=250; PGY2=282            # the playlist page's first two row centres

# 1. The wall becomes a queue (`Play all`), paused so the frames hold
#    still, and the place opened.
click $PA_X $PA_Y
sleep 0.8
key space
key ctrl+u
park; shot 01-queue-at-rest-1280x860

# 2. A playlist to drop on later, made by the route the drag is sugar
#    over: the second row's `+` opens the panel as the picker,
#    `New playlist` names one, and the pick completes into the new file.
xdotool mousemove $ROW_X $Y2; sleep 0.6
click $PLUS_X $Y2
click $NEW_X $NEW_Y
xdotool type --delay 40 "Road Trip"; sleep 0.3
key Return
park; shot 02-panel-standing-1280x860

# 3. The drag, mid-flight: the sounding first row pressed and pulled below
#    the third — the ghost names it at the pointer, the insertion line
#    sits on the boundary the drop will commit to.
pull "$ROW_X $Y1" "$ROW_X 219" "$ROW_X 229" "$ROW_X 241" "$ROW_X 253" "$ROW_X 265" "$ROW_X 277" "$ROW_X 289"
shot 03-drag-midflight-line-1280x860
# 4. The drop: one whole-list UpdateQueue — the row lands where the line
#    said, the numbers renumber, the cursor follows its track.
drop
park; shot 04-after-drop-reordered-1280x860

# 5. Drag-to-add, mid-flight: the second row carried over the panel's
#    playlist row — the ghost rides over the panel, the target draws the
#    room's hover statement.
pull "$ROW_X $Y2" "650 240" "720 235" "800 225" "880 210" "960 190" "1030 160" "$PR_X 132" "$PR_X $PR_Y"
shot 05-drag-to-panel-midflight-1280x860
# 6. The drop appends to the file: the row's counts tick up.
drop
park; shot 06-panel-after-add-1280x860

# 7. The playlist page's own drag: open the page from the panel row, lift
#    its first row below the second — mid-flight, then the drop: one
#    atomic file save, the artefact reordered.
click $PR_X $PR_Y
sleep 0.8
park; shot 07-playlist-page-1280x860
pull "$ROW_X $PGY1" "$ROW_X 260" "$ROW_X 270" "$ROW_X 282" "$ROW_X 296"
shot 08-playlist-drag-midflight-1280x860
drop
park; shot 09-playlist-after-drop-1280x860

kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]|^\[startup\] room|^\[play-all\]|^\[playlists\]" "$S/app.log" || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
