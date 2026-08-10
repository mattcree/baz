#!/usr/bin/env bash
# Render **doc 14 tier 1** — the kind stated in words, the byline that makes
# the two identity blocks one shape, and the run strip's two repairs — against
# the real binary, headless, on a private Xvfb, with all six XDG redirections
# from docs/DEVELOPMENT.md §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and every fixture sample is a zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it.
#
# What it shows, at 1280 × 860 and 1920 × 1080:
#
#   1. **The lane with both kinds in it** — records and playlists interleaved
#      in `RECENT`, sorted by touch, one anatomy, one 64 px pitch. The second
#      line is the only thing that differs and it now says which kind you are
#      looking at (ADR-0024 §A3.1).
#   2. **The two pages** — a playlist's and a record's, at the same window, so
#      the identity blocks can be laid over one another (§A4.3).
#   3. **The run strip at `RUN_MEASURE` 440**, in the three states §A5.2 names:
#      a record's run (`Run · … Save as playlist`), a run reified from a file
#      and untouched (`Saved as “Road Trip”`), and the same run after one
#      removal (`Undo … Save as new playlist`). Design 14 §6.3 flagged 440 as
#      tight — *"the one measurement in this study that wants a frame before
#      it ships"* — and the `…strip…` crops are that frame.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lane-fix
#   toolbox run -c baz-dev docs/design/impl/records-and-lists/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-lane-fix}
OUT=${OUT:-$REPO/docs/design/impl/records-and-lists}
DISP=${DISP:-:191}
S=/tmp/baz-rl-scratch

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
printf 'pcm.!default { type null }\nctl.!default { type null }\n' > "$S/home/.asoundrc"

mkdir -p "$S/config/baz"
printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
  "$FIX" > "$S/config/baz/config.toml"

# Four lists at staggered mtimes, striding the file list so each quotes four or
# more distinct records and draws a real 2 × 2 collage — `home-stats`'s recipe,
# plus **`#EXTINF` durations**, because the line under a name is
# `Playlist · 14 · 2:02:56` when the file declares times and `Playlist · 14`
# when it does not, and the frame should show the longer one. `Road Trip` is
# the newest, and it is the study's own example — the name the save readout
# has to print.
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

stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()  { sleep 0.9; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
crop()  { magick "$OUT/$1.png" -crop "$3" +repage "$OUT/$2.png"; echo "  shot $2"; }
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.0; }
typein(){ xdotool type --delay 45 "$1"; sleep 1.0; }
# Park the pointer in the body's left gutter, where it states nothing and
# opens no hover slot.
park()  { xdotool mousemove 292 700; sleep 0.6; }

# The lane's head: three destinations under the well, at GAP_XL 24 in.
HOME_Y=84; LIB_Y=124; NOW_Y=164
# `RECENT`'s rows, at SIDEBAR_ROW_H 64 pitch. With four records played, rows
# 1–4 are records and row 5 is `Road Trip` — the mixing ADR-0030 §1 designed.
REC1_Y=253; LIST_Y=509

for size in "1280 860" "1920 1080"; do
  set -- $size; W=$1; H=$2
  case $W in 1280) P=0 ;; *) P=1 ;; esac
  # The run column is `RUN_MEASURE` 440 hung from the body's right gutter, so
  # every figure below is derived from the window rather than measured twice.
  RUN_X1=$((W - 40)); RUN_X0=$((RUN_X1 - 440))
  ROW_X=$((RUN_X0 + 150)); CLOSE_X=$((RUN_X1 - 50)); ROW2_Y=306
  STRIP="460x40+$((RUN_X0 - 10))+88"
  # The playlist page: aside `ALBUM_ASIDE_W` 320, page width 320 + GAP_XL 24 +
  # the list's measure, centred in what the gutters leave.
  CONTENT=$((W - 280 - 80 - 10))
  MEASURE=$((CONTENT - 320 - 24)); [[ $MEASURE -gt 880 ]] && MEASURE=880
  PAGE=$((320 + 24 + MEASURE)); AIR=$(((CONTENT - PAGE) / 2))
  PLAY_X=$((280 + 40 + AIR + 160)); PLAY_Y=425

  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 2
  export DISPLAY=$DISP
  launch "$W" "$H"

  # Put four records in `RECENT`, from the query itself: Enter plays the
  # top-ranked match (ADR-0017 §1.2), which is one gesture and no pointer
  # arithmetic. Then pause, so the frames hold still.
  for q in verdigris basalt werkbund ochre; do
    key slash; typein "$q"; key Return; sleep 2; key Escape; key Escape
  done
  key space; sleep 1

  # ------------------------------------------------------------- 1 · the lane
  click 40 $LIB_Y
  park; shot "${P}1-lane-both-kinds-${W}x${H}"
  crop "${P}1-lane-both-kinds-${W}x${H}" "${P}2-lane-rows-${W}x${H}" "280x540+0+200"

  # ------------------------------------------------------------ 2 · the pages
  click 140 $LIST_Y
  park; shot "${P}3-playlist-page-${W}x${H}"
  crop "${P}3-playlist-page-${W}x${H}" "${P}4-identity-made-${W}x${H}" \
       "620x120+$((280 + 40 + AIR + 320 + 24))+68"
  key Escape; sleep 0.8
  click 140 $REC1_Y
  park; shot "${P}5-record-page-${W}x${H}"
  crop "${P}5-record-page-${W}x${H}" "${P}6-identity-found-${W}x${H}" \
       "620x120+$((280 + 40 + AIR + 320 + 24))+68"
  key Escape; sleep 0.8

  # -------------------------------------------------------- 3 · the run strip
  # A · a record's run — no file behind it, so the word is live and the strip
  # leads with the noun it never had.
  click 40 $NOW_Y
  park; shot "${P}7-run-of-a-record-${W}x${H}"
  crop "${P}7-run-of-a-record-${W}x${H}" "${P}8-strip-unfiled-${W}x${H}" "$STRIP"

  # B · a run reified from a file, untouched. `Play` on `Road Trip`'s page.
  click 40 $LIB_Y
  click 140 $LIST_Y
  click $PLAY_X $PLAY_Y
  sleep 2
  key space; sleep 1
  click 40 $NOW_Y
  park; shot "${P}9-run-from-a-file-${W}x${H}"
  crop "${P}9-run-from-a-file-${W}x${H}" "${P}a-strip-saved-${W}x${H}" "$STRIP"

  # C · the same run, after one removal. The ✕ is the run diverging from the
  # file it came from, and the live word returns as `Save as new playlist` —
  # a new file, never a write-back. This is the tightest the strip ever gets:
  # provenance, the reading, `Undo` *and* the longer word, all at 440.
  xdotool mousemove $ROW_X $ROW2_Y; sleep 1.0
  click $CLOSE_X $ROW2_Y
  park; shot "${P}b-run-edited-${W}x${H}"
  crop "${P}b-run-edited-${W}x${H}" "${P}c-strip-diverged-${W}x${H}" "$STRIP"

  stop
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
done

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo "done — $OUT"
