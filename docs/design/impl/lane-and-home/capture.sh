#!/usr/bin/env bash
# Render the returns lane, Home and Now playing — headless, on a private
# Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md. Nothing
# touches the owner's session; the run's `[mpris] no session bus` line is the
# receipt that it did not, and this script prints it.
#
# What it shows against the real binary:
#
#   1. **The lane open and collapsed** at 1280×860 and 1920×1080, with the
#      `RECENT` list mixing playlists and records in one order.
#   2. **The lamp dot on `Now playing`** in both states — the dot survives the
#      collapse, tucked against the glyph's corner.
#   3. **A panel row at rest and under the pointer**, before and after the
#      `Palette::step_up` correction: the before frames were rendered from
#      the parent commit and are kept because the defect is the argument.
#   4. **`Place::Home`** with and without an interrupted run, and
#      **`Place::NowPlaying`**.
#   5. **The column-count table** the collapse produces at three widths,
#      printed at the end and copied into README.md.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lane-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-lane-fix \
#     docs/design/impl/lane-and-home/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-lane-fix}
OUT=${OUT:-$REPO/docs/design/impl/lane-and-home}
DISP=${DISP:-:196}
S=/tmp/baz-lane-scratch

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
write_config() { # sidebar_open
  cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "artist"
density = "balanced"
sidebar_open = $1
EOF
}

# Three lists, written at three different mtimes so the lane's order is
# visibly *last touched first* rather than alphabetical.
mkdir -p "$S/data/baz/playlists"
mklist() { # name age-minutes n
  { echo "#EXTM3U"; find "$FIX" -name "*.flac" | sort | head -"$3"; } \
    > "$S/data/baz/playlists/$1.m3u8"
  touch -d "-$2 minutes" "$S/data/baz/playlists/$1.m3u8"
}
mklist "Road Trip" 5 14
mklist "Sunday Morning" 90 7
mklist "Late Shift" 2000 22

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
  sleep 5   # let the launch scan land
}

stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
# Cropped to the window: the Xvfb screen is 1920×1080 for every run so the
# window can be resized without restarting X, and a frame with a field of
# black around it measures nothing.
shot()  { sleep 0.9; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
# Park the pointer where it states nothing: the lane's own empty middle, below
# the last row and above the marks. A pointer left on a tile summons the
# wall's hover options, which is a different frame's subject.
park()  { xdotool mousemove 40 $((H - 200)); sleep 0.6; }
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.8; }
hover() { xdotool mousemove "$1" "$2"; sleep 0.8; }

# ---------------------------------------------------------------- 1280 × 860
W=1280; H=860
Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

write_config true
launch $W $H

# The head's three rows, at the lane's own geometry: GAP_XL 24 in, rows 40
# tall, starting under the window's top edge.
HOME_X=90;  HOME_Y=46
LIB_X=90;   LIB_Y=86
NOW_X=90;   NOW_Y=126
MARKS_Y=$((H - 60))
FIRST_ROW_X=140; FIRST_ROW_Y=250

park
shot 01-lane-open-1280

# **A record put on**, from the wall's own hover options — one press, which is
# what they are for. The lamp dot on `Now playing` then has something to say
# and the lane has a most-recent *record* among its lists.
hover 440 250
shot 02-wall-hover-options-1280
click 370 160
sleep 3
park
shot 03-lane-open-sounding-1280

# The record's page, reached from the tile the ordinary way.
hover 724 250
click 660 340
shot 04-album-page-1280
key Escape

# The collapse: the one press that re-hangs the wall.
key ctrl+b
park
shot 05-lane-collapsed-1280

key ctrl+b
park
shot 06-lane-open-again-1280

# **Home, with an interrupted run.** A record is sounding, so the snapshot on
# disk names it — but `CONTINUE` draws from what was *interrupted*, which is
# only true after a restart. Both states are captured: this launch has no
# snapshot yet (the band is absent, not empty), and the relaunch below has one.
click "$HOME_X" "$HOME_Y"
park
shot 07-home-without-an-interrupted-run-1280
click "$NOW_X" "$NOW_Y"
park
shot 08-now-playing-sounding-1280
click "$LIB_X" "$LIB_Y"

# **The panel**: the affordance the owner named, and the ghost row that
# replaced `New playlist`. Ctrl+P is the panel's only summons now — the
# strip's door is gone.
key ctrl+p
park
shot 09-panel-at-rest-1280            # every row at rest, ghost at the head
hover $((W - 170)) 126
shot 10-panel-ghost-hovered-1280      # the ghost answers the pointer
hover $((W - 170)) 232
shot 11-panel-row-hovered-1280        # a real list answers it the same way
click $((W - 170)) 126
shot 12-ghost-in-entry-1280           # the caret, and `Save` inert while empty
xdotool type --delay 40 "Evening"
sleep 0.8
shot 13-ghost-named-save-live-1280    # `Save` acts
xdotool key ctrl+a
xdotool type --delay 40 "Road Trip"
sleep 0.8
shot 14-ghost-refused-1280            # a taken name, refused before the press
xdotool key ctrl+a
xdotool type --delay 40 "Evening"
sleep 0.5
click $((W - 130)) 126                # Save
sleep 1
park
shot 15-ghost-saved-and-returned-1280 # the list is real; the ghost is back
key ctrl+p
# **The exit writes the elapsed position** (ADR-0023 §6), and only a real
# close request does it — `Message::Quit` is the one exit path. `windowclose`
# sends WM_DELETE_WINDOW, which is exactly what a title bar's × sends; killing
# the process instead would leave the position at the last track boundary and
# the relaunch would prove nothing.
sleep 6                       # let the run get somewhere worth resuming from
xdotool windowclose "$WID"
sleep 3
stop
# **The elapsed figure is seeded here, and that is a limitation of the
# harness rather than of the feature.** Two things write the snapshot: the run
# moving (a track boundary — proved above, the file already names a cursor),
# and the exit, which is the only writer of the *elapsed* milliseconds. Under
# Xvfb with no window manager, `windowclose` races winit's own X11 teardown and
# the process dies in `GetGeometry` before the update loop sees the close
# request, so the position on disk is the track boundary's 0. The exit path
# itself is one function (`App::leave_for_good`) reached by both exit routes
# and covered by tests; what cannot be shown headlessly is the compositor
# delivering the request. So the position is written in, to render a needle
# that is partway rather than a needle at zero.
if [[ -f "$S/config/baz/session.toml" ]]; then
  sed -i 's/^position_ms = .*/position_ms = 192000/' "$S/config/baz/session.toml"
fi
echo "  --- session.toml after the quit ---"
sed -n '1,6p' "$S/config/baz/session.toml" 2>/dev/null || echo "  (none written)"

# **Home, with an interrupted run** — the same baz, reopened. And the Now
# playing place with nothing sounding, which is the same relaunch: the run is
# loaded and silent until `Resume` is pressed, so this frame is the honest
# "nothing is playing" state rather than a contrivance.
launch $W $H
click "$HOME_X" "$HOME_Y"
park
shot 16-home-with-an-interrupted-run-1280
click "$NOW_X" "$NOW_Y"
park
shot 17-now-playing-silent-1280
# …and `Resume` puts the run back on where it stopped.
click "$HOME_X" "$HOME_Y"
click 500 180
sleep 2
click "$NOW_X" "$NOW_Y"
park
shot 18-now-playing-after-resume-1280
stop

# --------------------------------------------------------------- 1920 × 1080
W=1920; H=1080
write_config true
launch $W $H
park
shot 20-lane-open-1920
key ctrl+b
park
shot 21-lane-collapsed-1920
key ctrl+b
# The Now playing place at 1920 — the same surface, bigger, which is the whole
# of what makes the kiosk mode this surface at a larger size.
click "$HOME_X" "$HOME_Y"
click 500 180
sleep 2
click "$NOW_X" "$NOW_Y"
park
shot 23-now-playing-1920
stop

# A launch with the lane stored collapsed — the state survives a restart.
write_config false
launch $W $H
park
shot 22-lane-collapsed-on-launch-1920
stop

kill "$XPID" 2>/dev/null

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo
echo "--- what the collapse does to the grid ---"
"$REPO/docs/design/impl/lane-and-home/columns.py"
