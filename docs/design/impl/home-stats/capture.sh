#!/usr/bin/env bash
# Render the search well without its second line, and the Home place with the
# `COLLECTION` footer that line's resting half became — headless, on a private
# Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md. Nothing touches
# the owner's session; the run's `[mpris] no session bus` line is the receipt
# that it did not, and this script prints it.
#
# What it shows against the real binary:
#
#   1. **The lane at rest** at 1280×860 and 1920×1080 — the well one control
#      tall, no readout line, and `RECENT` holding the row the line cost.
#   2. **The lane mid-query** — the match count `n / m` right-aligned *inside*
#      the field, the query beside it, and no row below it moved.
#   3. **Home with its stats** — `CONTINUE`, `RECENTLY ADDED`, and
#      `COLLECTION` closing the page.
#
# The fixture is the lane frames' own (`/tmp/baz-lane-fix`, 25 records / 206
# tracks) and the three lists are built exactly as `search-in-lane/capture.sh`
# builds them, so `01-lane-at-rest-1280.png` here is directly comparable with
# `../search-in-lane/01-lane-well-at-rest-1280.png` — which is the *before*
# frame for the recovered `RECENT` row.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lane-fix
#   toolbox run -c baz-dev docs/design/impl/home-stats/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-lane-fix}
OUT=${OUT:-$REPO/docs/design/impl/home-stats}
DISP=${DISP:-:198}
S=/tmp/baz-home-stats-scratch

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
density = "balanced"
sidebar_open = true
EOF

# Lists at staggered mtimes, striding the file list so each quotes four or
# more distinct records and draws a real 2 × 2 collage — `search-in-lane`'s
# recipe. There are **eight** of them rather than that script's three, and
# four records are played below, because the claim being measured here is a
# *capacity*: a list of four rows cannot show whether the lane holds six or
# seven.
mkdir -p "$S/data/baz/playlists"
mklist() { # name age-minutes offset n
  { echo "#EXTM3U"
    find "$FIX" -name "*.flac" | sort | awk -v o="$3" 'NR % 9 == o' | head -"$4"
  } > "$S/data/baz/playlists/$1.m3u8"
  touch -d "-$2 minutes" "$S/data/baz/playlists/$1.m3u8"
}
mklist "Road Trip" 5 1 14
mklist "Sunday Morning" 90 4 7
mklist "Late Shift" 2000 7 22
mklist "Long Drive" 3000 2 11
mklist "Kitchen" 4000 5 9
mklist "Rain" 5000 8 17
mklist "Winter" 6000 3 13
mklist "Reading" 7000 6 19

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
shot()  { sleep 0.9; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
crop()  { magick "$OUT/$1.png" -crop "$3" +repage "$OUT/$2.png"; echo "  shot $2"; }
# Park the pointer where it states nothing: the lane's own empty middle, below
# the last row and above the marks.
park()  { xdotool mousemove 40 $((H - 200)); sleep 0.6; }
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.8; }
typein(){ xdotool type --delay 45 "$1"; sleep 1.0; }

# The lane's head, at its own geometry: GAP_XL 24 in, the well's block 32,
# GAP_SM, then three 40 px rows. The well *leads* (ADR-0030's second
# amendment as the owner corrected it).
WELL_Y=40; HOME_Y=84; LIB_Y=124; NOW_Y=164

Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

# ---------------------------------------------------------------- 1280 × 860
W=1280; H=860
launch $W $H

# Put four records on, from the query itself: Enter plays the top-ranked match
# (ADR-0017 §1.2), which is one gesture and no pointer arithmetic. Then peel
# the query, so `RECENT` mixes records with the lists and the frame is the
# resting one — twelve entries, which is more than either window can show, so
# what the frame measures is the lane's capacity rather than the fixture's.
for q in verdigris basalt werkbund ochre; do
  key slash
  typein "$q"
  key Return
  sleep 2
  key Escape
  key Escape
done
park
shot 01-lane-at-rest-1280
crop 01-lane-at-rest-1280 03-well-at-rest-1280 "280x120+0+0"

# **Mid-query.** The count is inside the field now, right-aligned, and the
# first row of `RECENT` has not moved by a pixel.
key slash
typein "an"
park
shot 02-lane-mid-query-1280
crop 02-lane-mid-query-1280 04-well-mid-query-1280 "280x120+0+0"
key Escape
key Escape

# **Home, with the footer.** Pause first, so `CONTINUE` stands: the band is
# the question you ask in the silence (ADR-0030's third amendment).
key space
sleep 1
click 40 $HOME_Y
park
shot 05-home-with-stats-1280
crop 05-home-with-stats-1280 06-collection-footer-1280 "620x110+300+640"
stop

# --------------------------------------------------------------- 1920 × 1080
W=1920; H=1080
launch $W $H
for q in verdigris basalt werkbund ochre; do
  key slash
  typein "$q"
  key Return
  sleep 2
  key Escape
  key Escape
done
park
shot 20-lane-at-rest-1920
key slash
typein "an"
park
shot 21-lane-mid-query-1920
key Escape
key Escape
key space
sleep 1
click 40 $HOME_Y
park
shot 22-home-with-stats-1920
stop

kill "$XPID" 2>/dev/null

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo
echo "--- the lane, measured ---"
"$REPO/docs/design/impl/home-stats/measure.py" "$OUT"
