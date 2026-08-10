#!/usr/bin/env bash
# Render **one list, drawn once** — the run column on `Now playing` and the
# track lists on a record's page and a playlist's page shown to be one row
# anatomy — and **the wide `Now playing`**, against the real binary, headless,
# on a private Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md
# §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and every fixture sample is a zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it.
#
# **It shoots two builds**, because both claims are comparisons. `BIN0` is the
# commit this branch started from and `BIN` is the branch; every frame is taken
# twice, from the same fixture, at the same window, with the same gestures.
#
# **Three sizes: 1280 × 860, 1920 × 1080 and 2560 × 1440.** The last is where
# the owner's defect lives — *"at full screen the now playing page looks odd
# because the playlist hugs right and the art hugs left"* — and it is the one
# nobody had been shooting.
#
# What it has to show:
#
#   1. **`Now playing`, whole**, from each build, at all three sizes. The
#      before/after pair at 2560 is the width claim in one look: 712 px of bare
#      field between the sleeve and the run, against one `GAP_XL` 24.
#   2. **The two columns' own band**, cropped at *identical window
#      coordinates* in both builds — the method `one-page-two-subjects`
#      established, and the only crop that turns a shape into a position.
#   3. **The three lists side by side**: a record's tracks, a playlist's
#      entries and the run's rows, each at its own column's x but at one width
#      and one height, stacked into a single image. If the merge worked, the
#      number lane, the title and the duration lane land at the same offsets
#      inside all three.
#
# Build both binaries **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lane-fix
#   BIN0=… BIN=… toolbox run -c baz-dev \
#     docs/design/impl/one-list-drawn-once/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
BIN0=${BIN0:-}
FIX=${FIX:-/tmp/baz-lane-fix}
OUT=${OUT:-$REPO/docs/design/impl/one-list-drawn-once}
DISP=${DISP:-:196}
S=${S:-/tmp/baz-onelist-scratch}

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
printf 'pcm.!default { type null }\nctl.!default { type null }\n' > "$S/home/.asoundrc"

mkdir -p "$S/config/baz"
printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
  "$FIX" > "$S/config/baz/config.toml"

# The same four lists the two studies before this one used, so all three sets
# of frames are of one product rather than three fixtures.
mkdir -p "$S/data/baz/playlists"
mklist() { # name age-minutes offset n
  local out="$S/data/baz/playlists/$1.m3u8"
  { echo "#EXTM3U"
    find "$FIX" -name "*.flac" | sort | awk -v o="$3" 'NR % 9 == o' | head -"$4" |
      while read -r f; do
        secs=$(metaflac --show-total-samples --show-sample-rate "$f" |
                 paste -sd' ' | awk '{printf "%d", $1/$2}')
        printf '#EXTINF:%s,%s\n%s\n' "$secs" "$(basename "${f%.flac}")" "$f"
      done
  } > "$out"
  touch -d "-$2 minutes" "$out"
}
mklist "Road Trip" 5 1 14
mklist "Sunday Morning" 90 4 7
mklist "Late Shift" 2000 7 22
mklist "Long Drive" 3000 2 11

APID=""; XPID=""
# `kill` on a `toolbox run` wrapper does not reach the process inside the
# container, so every child this script starts is reaped by **pid**, and only
# pids this script started. The trap covers an interrupted run.
cleanup() {
  [[ -n $APID ]] && kill "$APID" 2>/dev/null
  [[ -n $XPID ]] && kill "$XPID" 2>/dev/null
  return 0
}
trap cleanup INT TERM EXIT

launch() { # binary W H
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
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; exit 1; fi
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$2" "$3"
  xdotool windowfocus --sync "$WID"
  sleep 8   # let the launch scan and the thumbnail decode land
}

stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; APID=""; sleep 0.6; }
shot()  { sleep 1.2; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
crop()  { magick "$OUT/$1.png" -crop "$3" +repage "$OUT/$2.png"; echo "  shot $2"; }
pair()  { magick "$OUT/$2.png" "$OUT/$3.png" -append "$OUT/$1.png"; echo "  shot $1"; }
key()   { xdotool key "$@"; sleep 0.7; }
typein(){ xdotool type --delay 45 "$1"; sleep 1.0; }
# Park the pointer in the lane, where it states nothing and opens no hover
# slot — the row `+`, the ▲▼ and the ✕ are all hover-revealed, and a frame of
# the composition must not be a frame of the pointer.
park()  { xdotool mousemove 140 760; sleep 0.7; }
click() { xdotool mousemove "$1" "$2"; sleep 0.35; xdotool click 1; sleep 1.2; }

# **The lane's rows, re-read off a rendered frame** rather than inherited.
#
# `impl/one-page-two-subjects/capture.sh` used `LIB_Y=124`, `REC1_Y=253`,
# `LIST_Y=509`, and every one of them is stale: the lane has since grown a
# `PLAYLISTS` block above `RECENT`, so 124 is now **Home** and the first run of
# this script photographed the Home place and labelled it a record's page. That
# is the tenth false frame this project has produced and the first one caught
# before it reached a document.
#
# Measured off `04-record-after-1280x860.png` at 1 : 1 — the lane is a fixed
# 280 px and its content is anchored at the top, so these hold at every size:
#
#   Search 81 · Home 125 · Library 165 · Now playing 205
#   PLAYLISTS: Road Trip 294 · Sunday Morning 358 · Late Shift 422 · Long Drive 486
#   RECENT: Ochre 574 · Werkbund 638 · Basalt 693
#
# `Ochre` is deliberately the record and it is also the sounding one, so the
# same twelve tracks appear on a record's page and in the run — which is what
# makes the three-list strip a comparison of one list rather than of three.
LANE_X=140
REC1_Y=574      # RECENT → Ochre, the record whose run is playing
LIST_Y=294      # PLAYLISTS → Road Trip

for size in "1280 860" "1920 1080" "2560 1440"; do
  set -- $size; W=$1; H=$2
  case $W in 1280) P=0 ;; 1920) P=1 ;; *) P=2 ;; esac

  # ---------------------------------------------------------------- the crops
  # The body is the window less the lane, and the two studies before this one
  # established the rest of the arithmetic.
  BODY_X=280
  # **The two columns' band**, at identical *window* coordinates in both
  # builds. This is the crop that shows the gap: it spans the whole body at the
  # sleeve's own vertical middle, so the sleeve, the field between and the run
  # are all in one strip and their x-positions are directly comparable.
  BAND="$((W - BODY_X))x420+${BODY_X}+180"
  # One list's own block: the same width and height in all three surfaces, so
  # the rows can be compared as positions. A page's main column starts after
  # the centred aside; the run column stands at the body's right.
  LIST_W=420; LIST_H=300

  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 2
  export DISPLAY=$DISP

  for build in before after; do
    case $build in
      before) [[ -z $BIN0 ]] && { echo "  (no BIN0 — skipping the before frames)"; continue; }
              WHICH=$BIN0 ;;
      *)      WHICH=$BIN ;;
    esac
    echo "== $build @ ${W}x${H} =="
    # A fresh library per build, so neither inherits the other's `RECENT`.
    rm -rf "$S/data/baz/library.sqlite"* "$S/cache/baz"
    launch "$WHICH" "$W" "$H"

    # Four records into `RECENT`, from the query itself: Enter plays the
    # top-ranked match (ADR-0017 §1.2), which is one gesture and no pointer
    # arithmetic. The last one played is the run these frames are of.
    for q in verdigris basalt werkbund ochre; do
      key slash; typein "$q"; key Return; sleep 2; key Escape; key Escape
    done
    # Paused, so the frames hold still and the needle does not move between the
    # two builds' shots.
    key space; sleep 1.2

    # ------------------------------------------------- 1 · `Now playing`, wide
    # **Ctrl+U, not a click.** `keys.rs:416` binds `ShowNowPlaying` to it, so
    # the transition is a named act rather than a y-coordinate that has
    # photographed the wrong page nine times on this project.
    key ctrl+u
    park; shot "${P}1-now-playing-${build}-${W}x${H}"
    crop "${P}1-now-playing-${build}-${W}x${H}" "${P}2-band-${build}-${W}x${H}" "$BAND"

    # ------------------------------------------------- 2 · a record's page
    # Straight from the lane, which is in every place — no Escape, and no
    # intermediate page whose own rows could be what the pointer actually hit.
    click $LANE_X $REC1_Y
    park; shot "${P}4-record-${build}-${W}x${H}"

    # ------------------------------------------------ 3 · a playlist's page
    click $LANE_X $LIST_Y
    park; shot "${P}5-playlist-${build}-${W}x${H}"

    stop
  done

  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null; XPID=""
done

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo "done — $OUT"

# ---------------------------------------------------------------- derivations
#
# Everything below is cut from the frames already saved, so it can be re-run
# without relaunching the app. Two kinds:
#
#   · **the before/after pairs**, stacked, at *identical window coordinates* —
#     `one-page-two-subjects`'s method, and the only crop that turns a shape
#     into a position;
#   · **the three lists**, each at its own column's left edge and its own
#     measure, first rows aligned. The number lane and the title lane must land
#     at the same offset in all three; the duration lane does not, and must
#     not — a record's row reserves one trailing slot and an editable list's
#     reserves four (doc 09 §8.2), so their right-hand lanes are 28 px and
#     112 px in from their own column's edge respectively.
derive() {
  for s in 0:1280x860 1:1920x1080 2:2560x1440; do
    p=${s%%:*}; sz=${s##*:}
    pair "${p}6-band-both-${sz}" "${p}2-band-before-${sz}" "${p}2-band-after-${sz}"
  done
  # The three lists, at the audited window. The x and y below are read off the
  # frames rather than derived: a page's main column starts at 664 with an
  # 880-clamped measure of 566, and the run column stands at 800 × 430.
  magick "$OUT/04-record-after-1280x860.png"   -crop 566x300+664+256 +repage "$OUT/tmp-l1.png"
  magick "$OUT/05-playlist-after-1280x860.png" -crop 566x300+664+299 +repage "$OUT/tmp-l2.png"
  magick "$OUT/01-now-playing-after-1280x860.png" -crop 430x300+800+180 +repage \
    -background '#0e100c' -gravity west -extent 566x300 "$OUT/tmp-l3.png"
  magick "$OUT/tmp-l1.png" "$OUT/tmp-l2.png" "$OUT/tmp-l3.png" -append \
    "$OUT/08-three-lists-1280x860.png"
  rm -f "$OUT"/tmp-l?.png
  echo "  shot 08-three-lists-1280x860"
}
derive
