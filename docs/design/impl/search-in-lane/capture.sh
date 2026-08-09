#!/usr/bin/env bash
# Render the search well in the returns lane — headless, on a private Xvfb,
# with all six XDG redirections from docs/DEVELOPMENT.md. Nothing touches the
# owner's session; the run's `[mpris] no session bus` line is the receipt that
# it did not, and this script prints it.
#
# What it shows against the real binary:
#
#   1. **The well in the lane's head** at 1280×860 and 1920×1080 — under the
#      three destinations, over one always-drawn readout line.
#   2. **Mid-query**: the query in the field, `n of m albums` on the line
#      under it, the Songs section at the head of the wall's body.
#   3. **Collapsed**: the magnifier in the destinations' anatomy, at rest and
#      lit under a live query; and the press that opens the lane onto the
#      caret.
#   4. **Type-anywhere from `Home`** — the letter reaches the well, which now
#      means the Library comes back with it.
#   5. **The strip after the move**: no well, no doors — states, acts, gear.
#   6. **The two narrow regimes**, where the lane cannot hold the well and the
#      strip takes it back: single-line at 980 (strip 884 ≥ 872) and split at
#      900 (strip 804 < 872).
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-search-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-search-fix \
#     docs/design/impl/search-in-lane/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-search-fix}
OUT=${OUT:-$REPO/docs/design/impl/search-in-lane}
DISP=${DISP:-:197}
S=/tmp/baz-search-scratch

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

# Three lists at three mtimes, so the lane's order is visibly *last touched
# first*. Each quotes real fixture tracks, which is what a playlist's 2 × 2
# sleeve is drawn from (ADR-0024 §A1).
#
# **Every ninth track, from a different offset each** — the fixture lays nine
# tracks per record, so a list built from `head -n` quotes *one* record and
# draws that record's sleeve full-bleed. Three lists built that way all quote
# the same first record and draw three identical tiles, which is what made the
# lane-and-home frames read as "empty sleeves". Striding gives each list four
# or more distinct records, which is the collage the design is about.
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

resize() { xdotool windowsize "$WID" "$1" "$2"; sleep 1.2; }
stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()  { sleep 0.9; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
# Park the pointer where it states nothing: the lane's own empty middle, below
# the last row and above the marks.
park()  { xdotool mousemove 40 $((H - 200)); sleep 0.6; }
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.8; }
hover() { xdotool mousemove "$1" "$2"; sleep 0.8; }
typein(){ xdotool type --delay 45 "$1"; sleep 1.0; }

# The lane's head, at its own geometry: GAP_XL 24 in, rows 40 tall, then a
# GAP_SM and the well's 52 px block.
HOME_Y=44; LIB_Y=84; NOW_Y=124; WELL_Y=168
OPEN_X=90; RAIL_X=48

# ---------------------------------------------------------------- 1280 × 860
W=1280; H=860
Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

write_config true
launch $W $H

# A record put on from the wall's hover options, so `RECENT` mixes a record
# with the lists and the sleeves can be compared at one size.
hover 440 250
click 370 160
sleep 3
park
shot 01-lane-well-at-rest-1280

# **The well, focused by `/`** — the caret in the lane, the ring on the field.
key slash
park
shot 02-well-focused-by-slash-1280

# **Mid-query.** The query in the field, the match count on the line under it,
# and the Songs section at the head of the wall's body (doc 09 §5).
typein "an"
park
shot 03-well-mid-query-1280

# The strip after the move, on its own: no well, no doors.
magick "$OUT/03-well-mid-query-1280.png" -crop 1280x49+0+0 +repage \
  "$OUT/04-strip-after-the-move-1280.png"
echo "  shot 04-strip-after-the-move-1280"

# **Esc peels the query**, and the readout goes back to the collection.
#
# **Twice, and the first press is iced's not baz's**: iced 0.13's `text_input`
# handles <kbd>Esc</kbd> itself by unfocusing and *capturing* the event, so
# with the caret in the well the first press blurs and the second reaches
# `crate::keys` and peels. That is unchanged by the move — the well has always
# taken the caret — but it is worth a frame rather than a footnote.
key Escape
park
shot 05-esc-blurs-the-well-first-1280
key Escape
park
shot 06-esc-then-peels-the-query-1280

# **Collapsed, at rest**: the magnifier as the head's fourth mark.
key ctrl+b
park
shot 07-lane-collapsed-1280

# **Type-anywhere from the rail.** One keystroke opens the lane and lands the
# caret — one frame, no tween, because the collapse is a hard cut.
typein "an"
park
shot 08-typed-from-the-rail-opened-the-lane-1280

# **Collapsed under a live query**: the mark takes the lit ink, which is the
# one thing 96 px can say about the wall's state without a word on it.
#
# One <kbd>Esc</kbd> first, to blur without peeling: with the caret in the well
# every accelerator belongs to the field (`crate::keys::Focus::TextField` —
# *"the focused text field already had this key and made its decision"*), so
# `Ctrl+B` under the caret asks for nothing. Long-standing, and not this move's.
key Escape
key ctrl+b
park
shot 09-collapsed-mark-lit-by-a-live-query-1280

# …and pressing it opens the lane back onto the caret.
click "$RAIL_X" "$WELL_Y"
park
shot 10-collapsed-magnifier-opened-the-lane-1280

# **Type-anywhere from `Home`.** The query is peeled first so the frame shows
# only what this keystroke did, and the lane is collapsed, so the one letter
# has to do both things: open the lane and bring the wall back under it.
key Escape
key Escape
key Escape
key ctrl+b
click "$RAIL_X" "$HOME_Y"
park
shot 11-home-before-typing-1280
typein "vio"
park
shot 12-typed-from-home-lands-in-the-lane-1280
key Escape
key Escape

# **The narrow regimes**, where the lane cannot hold the well. 980: the strip
# is 884 and holds one line *with* the well. 900: the strip is 804 and splits.
resize 980 $H
W=980
park
shot 13-well-back-in-the-strip-980
resize 900 $H
W=900
park
shot 14-strip-splits-with-the-well-900
resize 1280 $H
W=1280
stop

# --------------------------------------------------------------- 1920 × 1080
W=1920; H=1080
write_config true
launch $W $H
park
shot 20-lane-well-at-rest-1920
typein "ochre"
park
shot 21-well-mid-query-1920
key Escape
key Escape
key ctrl+b
park
shot 22-lane-collapsed-1920
stop

kill "$XPID" 2>/dev/null

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo
echo "--- the lane's head, measured ---"
"$REPO/docs/design/impl/search-in-lane/measure.py" "$OUT"
