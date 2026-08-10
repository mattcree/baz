#!/usr/bin/env bash
# Prove the frame is the frame in every place — headless, on a private Xvfb,
# with all six XDG redirections from docs/DEVELOPMENT.md. Nothing touches the
# owner's session, his library or his session bus; each run's
# `[mpris] no session bus` line is the receipt, and this script prints it.
#
# `views/mod.rs`'s own sentence is *"the frame is the frame in every place —
# navigating may not slide the content area by a pixel"*, and it was false by
# **12 px** for about a month. `place_header_led` laid out whatever lead it was
# handed: the Album place's breadcrumb and the Artist place's name are
# *controls* declaring `TRANSPORT_HIT` 32, while a bare `place_name` is
# `LEADING_EMPHASIS` 20. So the strip came to 49 px under a control and 37 px
# under a word.
#
# **This harness shoots two builds**, because a single build cannot show a
# thing moving. That is the lesson `one-page-two-subjects` paid for, and the
# second lesson is here too: **the pages are shot at the same window
# coordinates and composited without cropping either one to its own content.**
# Cropping each place out of its own picture compares *shapes*; a shared crop
# compares *positions*, and this defect was two identical shapes 12 px apart.
#
# What the frames have to show:
#
#   1. **Settings and the Artist place move down 12 px** — they led with a bare
#      word and now stand where everything else does.
#   2. **The Library, a record's page and a playlist's page do not move at
#      all.** This is the half that can silently go wrong: the fix is in the
#      shared strip, so a mistake there moves the places that were already
#      right. Byte-identical before and after is the assertion, by `md5sum`.
#   3. **The hairline under each strip lands on one y across every place**,
#      which is the sentence itself rather than a proxy for it.
#
# Every place is reached the way a listener reaches it: the lane's own rows and
# the wall's own tiles. Nothing is deep-linked.
#
# Build both binaries **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz          # once per revision, copied aside
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-frame-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-frame-fix \
#     BEFORE=… AFTER=… docs/design/impl/one-frame-everywhere/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
FIX=${FIX:-/tmp/baz-frame-fix}
OUT=${OUT:-$REPO/docs/design/impl/one-frame-everywhere}
DISP=${DISP:-:197}
S=${S:-/tmp/baz-frame-scratch}
W=1280; H=860

mkdir -p "$OUT"

# Two independent guarantees that nothing is audible: the sink discards every
# sample, and the fixture's samples are all zero (docs/DEVELOPMENT.md).
fresh_scratch() {
  rm -rf "$S"
  mkdir -p "$S"/{home,data,config,cache,run}
  chmod 700 "$S/run"
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
}

launch() { # binary
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$1" >> "$S/app.log" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 60); do
    WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$W" "$H"
  xdotool windowfocus --sync "$WID"
  sleep 8   # let the launch scan and the thumbnail decode land
}

stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()  { sleep 0.9; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.2; }
park()  { xdotool mousemove 10 500; sleep 0.6; }

# The lane's head: the well, then the three destinations at `SIDEBAR_DEST_H`
# 40, `GAP_XL` 24 in — the same numbers the playlists-section harness derives.
LIB_X=88; LIB_Y=124
# The wall's first tile at `balanced`, read off `01-library-*.png` rather than
# derived: the sleeve spans x 321…561, y 131…371.
TILE_X=441; TILE_Y=251
# The record page's breadcrumb — `Anne-Marie Puig › Ochre` — whose first half
# is the door to the artist. It sits **in the strip at y = 24**, not at 96; the
# first run of this harness clicked 96, hit nothing, and photographed the
# record's page twice under the artist's name. The md5 check caught it, which
# is the whole reason this script asserts rather than eyeballs.
CRUMB_X=377; CRUMB_Y=24
# The gear, at the top bar's right edge — Library only, so Settings is reached
# from there rather than from wherever the previous shot left off.
GEAR_X=1224; GEAR_Y=24

# Take one build's five places. `$1` is the binary, `$2` the tag.
sweep() { # binary tag
  fresh_scratch
  launch "$1"

  # 1 · the Library. Led by its own strip (`top_bar`), which was never the
  #     defect — it is here as the control that must not move.
  park; shot "01-library-$2"

  # 2 · a record's page, by its tile on the wall. Led by a breadcrumb, so it
  #     was already at 49 and must not move either.
  click $TILE_X $TILE_Y
  park; shot "02-record-$2"

  # 3 · the artist's place, by the breadcrumb's first half — a door, and the
  #     route a listener actually takes to an artist from a record. Led by a
  #     bare word: one of the two that must move.
  click $CRUMB_X $CRUMB_Y
  park; shot "03-artist-$2"

  # 4 · Settings, by the gear, which lives in the Library's strip — so go back
  #     to the Library by the lane's own row first rather than assuming the
  #     gear is wherever this place left the pointer.
  click $LIB_X $LIB_Y
  click $GEAR_X $GEAR_Y
  park; shot "04-settings-$2"

  stop
}

Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
sleep 2
export DISPLAY=$DISP

echo "== before"
sweep "${BEFORE:?set BEFORE to the base binary}" before
echo "== after"
sweep "${AFTER:?set AFTER to the built binary}" after

kill "$XPID" 2>/dev/null

# The receipt that isolation held, printed rather than claimed.
echo "== isolation"
grep -m3 "mpris" "$S/app.log" || echo "  (no mpris line — CHECK THIS)"

# The places that must not have moved, asserted rather than eyeballed.
echo "== unchanged places (identical md5 = did not move)"
for p in 01-library 02-record; do
  a=$(md5sum "$OUT/$p-before.png" | cut -d' ' -f1)
  b=$(md5sum "$OUT/$p-after.png"  | cut -d' ' -f1)
  [[ $a == "$b" ]] && echo "  $p  identical" || echo "  $p  DIFFERS — the fix moved a place that was already right"
done

# The two that must have. A 12 px slide shows as a band of difference down the
# body and none in the top bar or the lane.
echo "== moved places (differ = moved; the strip crop says by how much)"
for p in 03-artist 04-settings; do
  a=$(md5sum "$OUT/$p-before.png" | cut -d' ' -f1)
  b=$(md5sum "$OUT/$p-after.png"  | cut -d' ' -f1)
  [[ $a == "$b" ]] && echo "  $p  IDENTICAL — the fix did not reach this place" || echo "  $p  differs"
  magick "$OUT/$p-before.png" "$OUT/$p-after.png" +append "$OUT/$p-together.png"
done
echo "done — $OUT"
