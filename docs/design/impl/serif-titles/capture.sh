#!/usr/bin/env bash
# Render **doc 14 tier 2** — the typographic axis — against the real binary,
# headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and every fixture sample is a zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it.
#
# What it has to show, and why the crops are the point rather than the pages:
#
#   1. **The two page heroes**, same size, same ink, same slot, at the two
#      window sizes — a record's title in `theme::WORK_TITLE` (IBM Plex Serif
#      Italic) against a playlist's name in the sans (ADR-0024 §A4.4).
#   2. **Each hero at 2×, point-sampled**, so the letterforms in the artefact
#      are the pixels that were rendered rather than a resampling of them.
#      This is what a reader can check the face on: the `a`'s single storey,
#      the serifs on `O`, the slope. A silent fallback to a host serif would
#      look plausible at 1× on the machine that took the frame and wrong on a
#      fresh one, so the frames are magnified and the glyph coverage is also
#      asserted mechanically
#      (`font::the_serif_face_carries_every_letter_an_album_title_arrives_with`).
#   3. **A long title**, because the serif's real risk is not `Ochre` — it is
#      a box-set title clipping at two lines at 28 px in italic.
#   4. **The byline stating its composition** (`Playlist · N records`), which
#      rides with the type change, in the same crop as the playlist's hero.
#   5. **The run strip**, unchanged by this tier and re-shot to judge doc 14
#      tier 2 #8 from a frame rather than from the text: whether tier 1's
#      `Run · ` prefix left `Save as playlist` unambiguous, or whether the
#      label has to name its subject.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lane-fix
#   toolbox run -c baz-dev docs/design/impl/serif-titles/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-lane-fix}
OUT=${OUT:-$REPO/docs/design/impl/serif-titles}
DISP=${DISP:-:192}
S=/tmp/baz-serif-scratch

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
printf 'pcm.!default { type null }\nctl.!default { type null }\n' > "$S/home/.asoundrc"

mkdir -p "$S/config/baz"
printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
  "$FIX" > "$S/config/baz/config.toml"

# The same four lists the tier 1 frames used, so the two sets of captures are
# of one product rather than two fixtures. `Road Trip` strides the file list,
# so its fourteen entries resolve to fourteen *distinct* records — which is
# exactly the case the byline's count had to be walked out for: quoting the
# sleeve's list would have printed `Playlist · 4 records` over it.
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
# **Point sampling, not interpolation.** A smooth 2× would invent the very
# letterform detail the frame is evidence about; `-filter point` shows the
# rendered pixels four times larger and nothing else.
mag()   { magick "$OUT/$1.png" -filter point -resize 200% "$OUT/$2.png"; echo "  shot $2"; }
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.0; }
typein(){ xdotool type --delay 45 "$1"; sleep 1.0; }
# Park the pointer in the body's left gutter, where it states nothing and
# opens no hover slot.
park()  { xdotool mousemove 292 700; sleep 0.6; }

# The lane's head: three destinations under the well, at GAP_XL 24 in.
HOME_Y=84; LIB_Y=124; NOW_Y=164
# `RECENT`'s rows, at SIDEBAR_ROW_H 64 pitch. Five records are played below,
# so rows 1–5 are records and row 6 is `Road Trip`.
REC1_Y=253; REC2_Y=$((REC1_Y + 64)); LIST_Y=$((REC1_Y + 5 * 64))

for size in "1280 860" "1920 1080"; do
  set -- $size; W=$1; H=$2
  case $W in 1280) P=0 ;; *) P=1 ;; esac
  RUN_X1=$((W - 40)); RUN_X0=$((RUN_X1 - 440))
  STRIP="460x40+$((RUN_X0 - 10))+88"
  # Both pages are the same arrangement (ADR-0024 §A2), so one crop expression
  # serves both: the aside is ALBUM_ASIDE_W 320, the page is centred in what
  # the gutters leave, and the identity block starts one GAP_XL right of the
  # sleeve.
  CONTENT=$((W - 280 - 80 - 10))
  MEASURE=$((CONTENT - 320 - 24)); [[ $MEASURE -gt 880 ]] && MEASURE=880
  PAGE=$((320 + 24 + MEASURE)); AIR=$(((CONTENT - PAGE) / 2))
  # 120 px tall: the whole 80 px identity block plus the air above and below
  # it, which is tier 1's own identity crop — so a reader can lay the two sets
  # of frames over each other and see that only the face changed.
  HERO="620x120+$((280 + 40 + AIR + 320 + 24))+68"

  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 2
  export DISPLAY=$DISP
  launch "$W" "$H"

  # Five records into `RECENT`, from the query itself: Enter plays the
  # top-ranked match (ADR-0017 §1.2), which is one gesture and no pointer
  # arithmetic. `overlong` is played second-last so it lands at row 2 — its
  # title is the fixture's box-set-length one, and a 28 px italic serif
  # clipping at two lines is the case worth a frame.
  for q in verdigris basalt werkbund overlong ochre; do
    key slash; typein "$q"; key Return; sleep 2; key Escape; key Escape
  done
  key space; sleep 1

  # ------------------------------------------------- 1 · a found thing's page
  click 40 $LIB_Y
  click 140 $REC1_Y
  park; shot "${P}1-record-page-${W}x${H}"
  crop "${P}1-record-page-${W}x${H}" "${P}3-hero-found-${W}x${H}" "$HERO"
  mag  "${P}3-hero-found-${W}x${H}" "${P}5-hero-found-2x-${W}x${H}"
  key Escape; sleep 0.8

  # The long title, at the same crop: two lines of italic serif, clipped where
  # `max_height(2.0 * LINE_HERO)` puts the cut.
  click 140 $REC2_Y
  park; crop_src="${P}7-record-long-title-${W}x${H}"; shot "$crop_src"
  crop "$crop_src" "${P}8-hero-long-title-${W}x${H}" "$HERO"
  mag  "${P}8-hero-long-title-${W}x${H}" "${P}9-hero-long-title-2x-${W}x${H}"
  key Escape; sleep 0.8

  # -------------------------------------------------- 2 · a made thing's page
  click 140 $LIST_Y
  park; shot "${P}2-playlist-page-${W}x${H}"
  crop "${P}2-playlist-page-${W}x${H}" "${P}4-hero-made-${W}x${H}" "$HERO"
  mag  "${P}4-hero-made-${W}x${H}" "${P}6-hero-made-2x-${W}x${H}"
  key Escape; sleep 0.8

  # ------------------------------------------------- 3 · the axis, as a look
  # The two heroes stacked at the same crop, 1× and 2×. This is the whole
  # claim of the tier in one image: same size, same ink, same slot, two kinds
  # of string.
  magick "$OUT/${P}3-hero-found-${W}x${H}.png" \
         "$OUT/${P}4-hero-made-${W}x${H}.png" -append \
         "$OUT/${P}a-heroes-together-${W}x${H}.png"
  echo "  shot ${P}a-heroes-together-${W}x${H}"
  mag "${P}a-heroes-together-${W}x${H}" "${P}b-heroes-together-2x-${W}x${H}"

  # ------------------------------ 4 · the run strip, for tier 2 #8's judgement
  # Unchanged by this tier. Re-shot so the question *"was `Run · ` enough?"*
  # is answered from this build's own frame.
  click 40 $NOW_Y
  park; shot "${P}c-run-of-a-record-${W}x${H}"
  crop "${P}c-run-of-a-record-${W}x${H}" "${P}d-strip-unfiled-${W}x${H}" "$STRIP"
  mag  "${P}d-strip-unfiled-${W}x${H}" "${P}e-strip-unfiled-2x-${W}x${H}"

  stop
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
done

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo "done — $OUT"
