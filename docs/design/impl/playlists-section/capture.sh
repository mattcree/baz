#!/usr/bin/env bash
# Render the lane's new `PLAYLISTS` section — headless, on a private Xvfb, with
# all six XDG redirections from docs/DEVELOPMENT.md. Nothing touches the
# owner's session, his library or his session bus; the run's
# `[mpris] no session bus` line is the receipt that it did not, and this script
# prints it.
#
# The owner, 2026-08-10: *"I guess we need to add playlists into their own
# section under library"*. What the frames have to show, in the order they are
# taken:
#
#   0. **Both sections absent** on a library nobody has played with no lists on
#      disk — the absent-not-empty rule, so the split does not cost a first run
#      two headings over nothing.
#   1. **`PLAYLISTS` alone**, before anything has been played: the records'
#      section is not drawn, and the lists sit directly under the head.
#   2. **Both sections**, after four records are played: `PLAYLISTS` under the
#      head, `RECENT` under it, each last touched first.
#   3. **Collapsed**: no heading either side, exactly as `RECENT` has always
#      done at 96 px — the two runs of sleeves separated by the sections' own
#      `GAP_MD`.
#   4. **A list sounding**: the lamp on the list's row in `PLAYLISTS`, and the
#      records it quotes unmarked and unmoved in `RECENT`.
#   5. **Thirty lists** — the section has no cap, which is the one place this
#      change can produce a real defect. The lane at the top, then wheel-
#      scrolled to the foot: `RECENT` is still reachable, in one scroller, with
#      one bar.
#
# **Every state is reached the way a listener reaches it.** Records enter
# `RECENT` by `/`, a query and <kbd>Enter</kbd> — ADR-0017 §1.2's play-the-top-
# match, one gesture. The list is played by opening it from the lane and
# pressing `Play` on its page. Nothing here writes a ledger by hand, and
# nothing double-clicks a sleeve.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lists-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-lists-fix \
#     docs/design/impl/playlists-section/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-lists-fix}
OUT=${OUT:-$REPO/docs/design/impl/playlists-section}
DISP=${DISP:-:198}
S=${S:-/tmp/baz-lists-scratch}

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

# A list, at an age, quoting **every ninth track from a different offset** —
# the fixture lays nine tracks per record, so a list built from `head -n`
# quotes one record and draws that record's cover full-bleed. Striding gives
# each list four or more distinct records, which is the collage ADR-0024 §A1
# is about.
mklist() { # name age-minutes offset n
  mkdir -p "$S/data/baz/playlists"
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

launch() { # W H
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$BIN" >> "$S/app.log" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 60); do
    WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$1" "$2"
  xdotool windowfocus --sync "$WID"
  sleep 8   # let the launch scan and the thumbnail decode land
}

stop()   { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()   { sleep 0.9; magick import -window root -crop "${W}x${H}+0+0" +repage \
             "$OUT/$1.png"; echo "  shot $1"; }
crop()   { magick "$OUT/$1.png" -crop "$3" +repage "$OUT/$2.png"; echo "  shot $2"; }
key()    { xdotool key "$@"; sleep 0.6; }
click()  { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.0; }
typein() { xdotool type --delay 45 "$1"; sleep 1.0; }
# Park in the lane's own left gutter: inside the scroller, outside every row's
# hit area (the rows are inset `GAP_XL` 24), so nothing is hovered and the
# wheel still lands on the lane.
park()   { xdotool mousemove 10 500; sleep 0.6; }
wheel()  { xdotool mousemove 10 500; sleep 0.3; xdotool click --repeat "$2" --delay 60 "$1"; sleep 1.0; }

# The lane's head: the well, then the three destinations at `SIDEBAR_DEST_H`
# 40, `GAP_XL` 24 in.
HOME_Y=84; LIB_Y=124; NOW_Y=164
# The sections, from the same tokens: the hairline block ends at 209, a
# heading is `LINE_HEADING` 12 tall and a row is `SIDEBAR_ROW_H` 64.
#   PLAYLISTS heading 209 → 221, four lists 221 → 477 (centres 253 … 445)
#   GAP_MD 12, RECENT heading 489 → 501, records 501 → … (centres 533 …)
LIST1_Y=253; LIST2_Y=317; REC1_Y=533

for size in "1280 860" "1920 1080"; do
  set -- $size; W=$1; H=$2
  case $W in 1280) P=0 ;; *) P=1 ;; esac
  # The playlist page's `Play`, derived from the window exactly as
  # `records-and-lists/capture.sh` derives it.
  CONTENT=$((W - 280 - 80 - 10))
  MEASURE=$((CONTENT - 320 - 24)); [[ $MEASURE -gt 880 ]] && MEASURE=880
  PAGE=$((320 + 24 + MEASURE)); AIR=$(((CONTENT - PAGE) / 2))
  PLAY_X=$((280 + 40 + AIR + 160)); PLAY_Y=425

  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 2
  export DISPLAY=$DISP

  # ------------------------------------------------- 0 · nothing touched yet
  # No lists on disk, no ledger: **both sections absent**, and the lane below
  # the hairline is bare rather than carrying two words over two gaps.
  fresh_scratch
  launch "$W" "$H"
  park; shot "${P}0-lane-bare-${W}x${H}"
  crop "${P}0-lane-bare-${W}x${H}" "${P}1-lane-bare-crop-${W}x${H}" "280x400+0+180"
  stop

  # ------------------------------------------------ 1 · lists, nothing played
  mklist "Road Trip" 5 1 14
  mklist "Sunday Morning" 90 4 7
  mklist "Late Shift" 2000 7 22
  mklist "Long Drive" 3000 2 11
  launch "$W" "$H"
  park; shot "${P}2-lists-only-${W}x${H}"
  crop "${P}2-lists-only-${W}x${H}" "${P}3-lists-only-crop-${W}x${H}" "280x400+0+180"

  # ------------------------------------------------------- 2 · both sections
  # Four records into `RECENT`, from the query itself: <kbd>/</kbd>, a word,
  # <kbd>Enter</kbd> plays the top-ranked match (ADR-0017 §1.2). Then pause, so
  # the frames hold still.
  for q in verdigris basalt werkbund ochre; do
    key slash; typein "$q"; key Return; sleep 2; key Escape; key Escape
  done
  key space; sleep 1
  click 40 $LIB_Y
  park; shot "${P}4-two-sections-${W}x${H}"
  crop "${P}4-two-sections-${W}x${H}" "${P}5-two-sections-crop-${W}x${H}" "280x620+0+180"

  # ------------------------------------------------------------ 3 · collapsed
  key ctrl+b
  park; shot "${P}6-collapsed-${W}x${H}"
  crop "${P}6-collapsed-${W}x${H}" "${P}7-collapsed-crop-${W}x${H}" "96x620+0+180"
  key ctrl+b
  park

  # ------------------------------------------------------ 4 · a list sounding
  # Opened from the lane and played from its own page — the listener's route,
  # and the only one that gives the run a provenance. The lamp lands on the
  # **list**, and the records it quotes stay where `RECENT` had them.
  click 140 $LIST1_Y
  click "$PLAY_X" "$PLAY_Y"
  sleep 2
  key space; sleep 1
  click 40 $LIB_Y
  park; shot "${P}8-list-sounding-${W}x${H}"
  crop "${P}8-list-sounding-${W}x${H}" "${P}9-list-sounding-crop-${W}x${H}" "280x620+0+180"
  stop

  # ------------------------------------------- 5 · thirty lists, and the floor
  # `PLAYLISTS` has no cap. Twenty-six more lists, so the section is four times
  # the lane's own height — and `RECENT` must still be reachable, in the one
  # scroller, with the one bar at the lane's edge.
  for i in $(seq 5 30); do
    mklist "$(printf 'List %02d' "$i")" $((i * 60)) $((i % 9)) $((4 + i % 7))
  done
  launch "$W" "$H"
  park; shot "${P}a-thirty-lists-top-${W}x${H}"
  # …wheel to the foot of the lane. The pointer is in the lane's gutter, over
  # no row, which is where a listener's wheel lands when they are reading the
  # column rather than aiming at a row.
  wheel 5 40
  shot "${P}b-thirty-lists-recent-reachable-${W}x${H}"
  crop "${P}b-thirty-lists-recent-reachable-${W}x${H}" \
       "${P}c-thirty-lists-foot-crop-${W}x${H}" "280x620+0+180"
  # …and back to the top, so the frame pair is one scroller moving rather than
  # two surfaces.
  wheel 4 60
  shot "${P}d-thirty-lists-back-at-the-top-${W}x${H}"
  # Collapsed with thirty lists: the rail scrolls the same one column.
  key ctrl+b
  wheel 5 40
  park; shot "${P}e-thirty-lists-collapsed-foot-${W}x${H}"
  stop

  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
done

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo "done — $OUT"
