#!/usr/bin/env bash
# Render **one page, two subjects** — the record's page and the playlist's page
# made one composition — against the real binary, headless, on a private Xvfb,
# with all six XDG redirections from docs/DEVELOPMENT.md §"Headless UI
# verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and every fixture sample is a zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it.
#
# **It shoots two builds**, because the claim is a comparison. `BIN0` is the
# commit this branch started from and `BIN` is the branch; every frame is taken
# twice, from the same fixture, at the same window, with the same gestures.
#
# What it has to show, at 1280 × 860 and 1920 × 1080:
#
#   1. **The two pages, whole**, from each build. The `…-before` pair is the
#      state the owner was describing — *"right now they are different but for
#      no good reason"*.
#   2. **The strips, stacked.** Before: a record leads with `Anne-Marie Puig ›
#      Ochre` and a playlist leads with the word `Playlist` — one names the
#      subject, the other names the kind. After: both name the subject.
#   3. **The asides, stacked.** This is where the quiet act's drift is visible
#      and nowhere else: a record's `Add to playlist…` was a centred,
#      full-width, `paper_dim` box and a playlist's `Queue` · `Rename` ·
#      `Delete` were natural-width `paper` words. After, one word in one slot.
#   4. **The identity blocks, at tier 1's own crop**, so this set can be laid
#      over `../records-and-lists/` and `../serif-titles/` and shown to have
#      moved nothing: 80 px, three lines, and the middle line saying two
#      different sorts of thing.
#   5. **The two pages at one crop, stacked into a single image** — the whole
#      claim of the change in one look, before and after.
#
# Build both binaries **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lane-fix
#   BIN0=/tmp/baz-bin-before BIN=/tmp/baz-bin-after \
#     toolbox run -c baz-dev docs/design/impl/one-page-two-subjects/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
BIN0=${BIN0:-}
FIX=${FIX:-/tmp/baz-lane-fix}
OUT=${OUT:-$REPO/docs/design/impl/one-page-two-subjects}
DISP=${DISP:-:194}
S=/tmp/baz-opts-scratch

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
printf 'pcm.!default { type null }\nctl.!default { type null }\n' > "$S/home/.asoundrc"

mkdir -p "$S/config/baz"
printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
  "$FIX" > "$S/config/baz/config.toml"

# The same four lists tiers 1 and 2 used, so all three sets of frames are of one
# product rather than three fixtures. `Road Trip` strides the file list, so its
# fourteen entries resolve to twelve distinct records and its byline reads
# `Playlist · 12 records`.
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
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$2" "$3"
  xdotool windowfocus --sync "$WID"
  sleep 8   # let the launch scan and the thumbnail decode land
}

stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()  { sleep 0.9; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
crop()  { magick "$OUT/$1.png" -crop "$3" +repage "$OUT/$2.png"; echo "  shot $2"; }
stack() { magick "$OUT/$2.png" "$OUT/$3.png" -append "$OUT/$1.png"; echo "  shot $1"; }
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.0; }
typein(){ xdotool type --delay 45 "$1"; sleep 1.0; }
# Park the pointer in the body's left gutter, where it states nothing and opens
# no hover slot — the row `+`, the ▲▼ and the ✕ are all hover-revealed, and a
# frame of the composition must not be a frame of the pointer.
park()  { xdotool mousemove 292 700; sleep 0.6; }

# The lane's head: three destinations under the well, at GAP_XL 24 in.
LIB_Y=124
# `RECENT`'s rows, at SIDEBAR_ROW_H 64 pitch. Four records are played below, so
# rows 1–4 are records and row 5 is `Road Trip`.
REC1_Y=253; LIST_Y=509

for size in "1280 860" "1920 1080"; do
  set -- $size; W=$1; H=$2
  case $W in 1280) P=0 ;; *) P=1 ;; esac
  # Both pages are one composition now, so one set of crop expressions serves
  # both — which is itself the claim, and it is why the arithmetic below is
  # written once rather than per page. The aside is ALBUM_ASIDE_W 320, the list
  # clamps at LIST_MEASURE 880, and the page centres in what the gutters leave.
  CONTENT=$((W - 280 - 80 - 10))
  MEASURE=$((CONTENT - 320 - 24)); [[ $MEASURE -gt 880 ]] && MEASURE=880
  PAGE=$((320 + 24 + MEASURE)); AIR=$(((CONTENT - PAGE) / 2))
  X0=$((280 + 40 + AIR))                      # the page's own left edge
  STRIP="$((W - 280))x48+280+0"               # the header strip, body width
  ASIDE="320x150+${X0}+412"                   # under the sleeve: Play, the acts
  # 120 px tall, at the identity block's own x: tier 1's crop exactly, so the
  # three sets of frames overlay.
  HERO="620x120+$((X0 + 320 + 24))+68"
  BOTH="${PAGE}x520+${X0}+48"                 # the whole two-column composition

  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 2
  export DISPLAY=$DISP

  for build in before after; do
    case $build in
      before) [[ -z $BIN0 ]] && { echo "  (no BIN0 — skipping the before frames)"; continue; }
              WHICH=$BIN0 ;;
      *)      WHICH=$BIN ;;
    esac
    # A fresh library per build, so neither inherits the other's `RECENT`.
    rm -rf "$S/data/baz/library.sqlite"* "$S/cache/baz"
    launch "$WHICH" "$W" "$H"

    # Four records into `RECENT`, from the query itself: Enter plays the
    # top-ranked match (ADR-0017 §1.2), which is one gesture and no pointer
    # arithmetic. Then pause, so the frames hold still.
    for q in verdigris basalt werkbund ochre; do
      key slash; typein "$q"; key Return; sleep 2; key Escape; key Escape
    done
    key space; sleep 1

    # ------------------------------------------------ 1 · a found thing's page
    click 40 $LIB_Y
    click 140 $REC1_Y
    park; shot "${P}1-record-${build}-${W}x${H}"
    crop "${P}1-record-${build}-${W}x${H}" "${P}3-strip-record-${build}-${W}x${H}" "$STRIP"
    crop "${P}1-record-${build}-${W}x${H}" "${P}5-aside-record-${build}-${W}x${H}" "$ASIDE"
    crop "${P}1-record-${build}-${W}x${H}" "${P}7-identity-found-${build}-${W}x${H}" "$HERO"
    crop "${P}1-record-${build}-${W}x${H}" "${P}9-page-record-${build}-${W}x${H}" "$BOTH"
    key Escape; sleep 0.8

    # ------------------------------------------------- 2 · a made thing's page
    click 140 $LIST_Y
    park; shot "${P}2-playlist-${build}-${W}x${H}"
    crop "${P}2-playlist-${build}-${W}x${H}" "${P}4-strip-playlist-${build}-${W}x${H}" "$STRIP"
    crop "${P}2-playlist-${build}-${W}x${H}" "${P}6-aside-playlist-${build}-${W}x${H}" "$ASIDE"
    crop "${P}2-playlist-${build}-${W}x${H}" "${P}8-identity-made-${build}-${W}x${H}" "$HERO"
    crop "${P}2-playlist-${build}-${W}x${H}" "${P}a-page-playlist-${build}-${W}x${H}" "$BOTH"
    key Escape; sleep 0.8

    # --------------------------------------- 3 · the three claims, as one look
    stack "${P}b-strips-together-${build}-${W}x${H}" \
          "${P}3-strip-record-${build}-${W}x${H}" \
          "${P}4-strip-playlist-${build}-${W}x${H}"
    stack "${P}c-asides-together-${build}-${W}x${H}" \
          "${P}5-aside-record-${build}-${W}x${H}" \
          "${P}6-aside-playlist-${build}-${W}x${H}"
    stack "${P}d-identities-together-${build}-${W}x${H}" \
          "${P}7-identity-found-${build}-${W}x${H}" \
          "${P}8-identity-made-${build}-${W}x${H}"
    # The headline frame: the two pages at one crop, one above the other. If
    # the change worked, the two halves of this image differ in their words and
    # in nothing else that is not a fact about the subject.
    stack "${P}e-pages-together-${build}-${W}x${H}" \
          "${P}9-page-record-${build}-${W}x${H}" \
          "${P}a-page-playlist-${build}-${W}x${H}"

    stop
  done

  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
done

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo "done — $OUT"
