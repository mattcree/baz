#!/usr/bin/env bash
# Render **the Now playing surface before and after step A2** — the hero
# decode, `NOW_PLAYING_MAX`'s deletion and the derived ambient field — at three
# window sizes and in both densities, against two real binaries, headless, on a
# private Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md
# §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and the fixture's samples are all zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it.
#
# **Two binaries, one script**, because the complaint is visual and the only
# honest evidence is the same window drawn twice:
#
#   BEFORE=/tmp/baz-before   the commit A2 landed on (NOW_PLAYING_MAX 720,
#                            the 320 px thumbnail upscaled to reach it, the
#                            room #0C0D0E behind it)
#   AFTER=/tmp/baz-after     this branch
#
# Build them with:
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output   # → AFTER
#   git checkout HEAD~1 -- crates/baz && …same…               # → BEFORE
#
# **The fixture's covers are re-drawn at 1400 px** (mkfixture.sh ships 600),
# because 600 is smaller than the hero tier's own ceiling and the whole subject
# of this step is what happens when the *source* is the binding term and when it
# is not. A second pass re-draws them at 300 px for story S7's small-cover case.
#
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-hero-fix
#   toolbox run -c baz-dev docs/design/impl/artwork-at-size/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BEFORE=${BEFORE:-/tmp/baz-before}
AFTER=${AFTER:-/tmp/baz-after}
FIX=${FIX:-/tmp/baz-hero-fix}
OUT=${OUT:-$REPO/docs/design/impl/artwork-at-size}
DISP=${DISP:-:197}
S=/tmp/baz-hero-scratch

mkdir -p "$OUT"

# --- the covers, re-drawn at a size the hero tier can actually use ----------
# Six visual families as mkfixture.sh draws them, at $1 px instead of 600. The
# hues are the fixture's own, so a frame here and a frame in
# `impl/queue-merged/` are the same records wearing the same colours.
redraw() { # edge
  local S=$1 i=0
  local -a HUES=(28 42 210 12 340 68 200 100 35 270 15 180 300 55 120 88 250 320 10 160 280 45 195 0 140)
  local -a FAMS=(mono pale chroma split rings type mono pale chroma split rings type mono pale chroma split rings type mono pale chroma split rings type mono)
  for dir in "$FIX"/*/; do
    local hue=${HUES[$i]} fam=${FAMS[$i]}
    case $fam in
      mono)   magick -size ${S}x${S} "xc:hsl(${hue},18%,7%)" \
                -fill "hsl(${hue},22%,17%)" -draw "rectangle $((S*5/12)),$((S*47/60)) $((S*7/12)),$((S*5/6))" "$dir/cover.jpg" ;;
      pale)   magick -size ${S}x${S} "xc:hsl(${hue},14%,88%)" \
                -fill "hsl(${hue},30%,42%)" -draw "circle $((S/2)),$((S/2)) $((S/2)),$((S/5))" "$dir/cover.jpg" ;;
      chroma) magick -size ${S}x${S} "xc:hsl(${hue},72%,46%)" \
                -fill "hsl($(((hue+180)%360)),72%,22%)" -draw "rectangle 0,$((S*43/60)) ${S},$((S*47/60))" "$dir/cover.jpg" ;;
      split)  magick -size ${S}x${S} "xc:hsl(${hue},40%,30%)" \
                -fill "hsl($(((hue+40)%360)),55%,62%)" -draw "polygon 0,${S} ${S},0 ${S},${S}" "$dir/cover.jpg" ;;
      rings)  magick -size ${S}x${S} "xc:hsl(${hue},30%,14%)" -fill none \
                -stroke "hsl(${hue},60%,58%)" -strokewidth $((S/75)) \
                -draw "circle $((S/2)),$((S/2)) $((S/2)),$((S*3/20))" \
                -draw "circle $((S/2)),$((S/2)) $((S/2)),$((S*4/15))" \
                -draw "circle $((S/2)),$((S/2)) $((S/2)),$((S*23/60))" "$dir/cover.jpg" ;;
      type)   magick -size ${S}x${S} "xc:hsl(${hue},25%,20%)" \
                -fill "hsl(${hue},18%,86%)" -pointsize $((S*23/150)) -gravity center \
                -annotate +0+0 "Az" "$dir/cover.jpg" ;;
    esac
    i=$(((i + 1) % 25))
  done
  echo "  covers redrawn at ${S} px"
}

scratch() {
  rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
  cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF
  mkdir -p "$S/config/baz"
  # `run_column` left unwritten: a fresh baz opens with the run standing, and
  # the first frame is what a listener actually sees.
  printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\nsidebar_open = true\n' "$FIX" \
    > "$S/config/baz/config.toml"
}

launch() { # binary w h
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$1" >> "$S/app.log" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 40); do
    WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$2" "$3"
  xdotool windowfocus --sync "$WID"
  # **Wait for the scan, not for a guess.** 206 tracks with 1400 px covers take
  # longer to walk than the 600 px fixture does, and a double-click that lands
  # on a wall with no tiles on it yet is a frame of the Library place with
  # `Nothing playing` in the bar. The scan prints when it is done.
  for _ in $(seq 1 60); do
    grep -q "^\[scan\] done:" "$S/app.log" && break
    sleep 0.5
  done
  sleep 4
}

shot() { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
key()  { xdotool key "$@"; sleep 0.5; }
park() { xdotool mousemove $((W - 6)) $((H - 200)); }
# The `Run` word sits one HANG in from the body's right edge and one HANG down
# from its top; the body's right edge is the window's.
run_word() { xdotool mousemove $((W - 74)) 58; sleep 0.3; xdotool click 1; sleep 0.6; }

surface() { # prefix
  # A double-click on a sleeve plays that record whole (ADR-0023 §2's
  # needle-drop at the album level). Space pauses immediately so the cursor
  # holds still for the stills; the null sink otherwise races through silent
  # tracks. Then Ctrl+U — the lane's `Now playing` row plus the `Run` word,
  # made for you.
  xdotool mousemove 340 250; sleep 0.4
  xdotool click --repeat 2 --delay 120 1; sleep 1.5
  key space
  key ctrl+u
  # The hero decode is asked for the moment the engine names a record and
  # answered on a blocking worker; a second is several orders of magnitude
  # more than it needs, and the frame is of the settled surface either way.
  sleep 1.5
  park; shot "${1}-run-on"
  run_word
  park; shot "${1}-run-off"
}

run_size() { # binary tag w h
  W=$3; H=$4
  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 1
  scratch
  launch "$1" "$W" "$H"
  surface "$2"
  kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
}

echo "--- covers at 1400 px (the ordinary case: a well-kept collection) ---"
redraw 1400

for size in "1280 860" "1920 1080" "2560 1440"; do
  set -- $size
  echo "=== ${1}x${2} ==="
  run_size "$BEFORE" "$(printf '%02d' $((${1} / 100)))-before-${1}x${2}" "$1" "$2"
  run_size "$AFTER"  "$(printf '%02d' $((${1} / 100)))-after-${1}x${2}"  "$1" "$2"
done

# **Story S7 — the artwork is small.** The same window, the same binaries, and
# a collection ripped with 300 px covers. Before, the surface upscaled it to
# 720; after, it is drawn at 300 on a field that fills what it cannot.
echo "--- covers at 300 px (story S7: the small cover) ---"
redraw 300
run_size "$BEFORE" "40-before-small-source-1920x1080" 1920 1080
run_size "$AFTER"  "40-after-small-source-1920x1080"  1920 1080

# **Below `SPLIT_FLOOR`.** A 1000 x 800 window with the lane open is a 720 px
# body, under the floor of 784, where the two columns re-stack into one and the
# record becomes the run's head block. Step A2 has three things to hold here:
# the head is bound by the source like everything else, the whole field is the
# run's ground (`Ground::Still`) because the whole body is the list, and the
# head's own height reservation is its own rather than the record column's.
#
# Taken after the 300 px pass above, and it changes nothing measurable: the
# head is `ART_MIN` 240 and 240 is under both 300 and 1400, so the head block
# is the same object at either cover size.
restacked() {
  xdotool mousemove 340 250; sleep 0.4
  xdotool click --repeat 2 --delay 120 1; sleep 1.5
  key space
  key ctrl+u
  sleep 1.5
  # The single column opens at the playing row, which is what a listener wants
  # and is not what this frame is of. Wheel to the top so the head block — the
  # record, re-hung as the run's first object — is on screen.
  xdotool mousemove $((W / 2)) $((H / 2))
  for _ in $(seq 1 40); do xdotool click 4; done
  sleep 1.0
  park; shot "$1"
}
for pair in "$BEFORE 30-before-restacked-1000x800" "$AFTER 30-after-restacked-1000x800"; do
  set -- $pair
  W=1000; H=800
  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 1
  scratch
  launch "$1" "$W" "$H"
  restacked "$2"
  kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
done

# **The field with nothing to read.** Every cover replaced by a neutral grey,
# which is the `mono` sleeve taken to its limit: no hue over the presence
# floor, so `Field::derive` answers `None` and the room shows through.
echo "--- a monochrome collection: the field falls back to the room ---"
for dir in "$FIX"/*/; do
  magick -size 300x300 "xc:gray12" -fill "gray28" -draw "rectangle 120,230 180,250" "$dir/cover.jpg"
done
run_size "$AFTER" "41-after-monochrome-1920x1080" 1920 1080

echo "--- the isolation receipt ---"
grep -E "^\[mpris\]" "$S/app.log" | head -1 || echo "(no mpris line — look at $S/app.log)"
echo "done — $OUT"
