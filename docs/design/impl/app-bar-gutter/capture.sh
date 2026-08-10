#!/usr/bin/env bash
# Render **the app bar's trailing gutter and its zone 1, before and after**,
# against the real binary, headless, on a private Xvfb, with all six XDG
# redirections from docs/DEVELOPMENT.md §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and every fixture sample is a zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it.
#
# # What it has to show
#
# The owner, 2026-08-10: *"the settings cog is padded in quite a bit and does
# not align with the rail"*, and *"we probably want an icon for our app to show
# in the bar"*. So:
#
#   1. **The Library, before and after, at 1280 x 860 and 1920 x 1080**
#      (frames `01`) — the whole window, at the same window coordinates in both
#      builds, because the claim is about where one thing sits relative to
#      another and a claim like that can only be read off frames taken in one
#      coordinate system.
#   2. **The right-hand edge, cropped and stacked** (frame `02`): the app bar's
#      trailing 200 px over the index rail's first rows, before above after.
#      This is the frame the complaint is actually about — one vertical line,
#      and whether the gear stands on it.
#   3. **Zone 1, magnified** (frame `03`): the word, then the mark, at 8x with
#      `-filter point`, so what a 16 px full-colour icon actually resolves to in
#      a 41 px band can be looked at rather than assumed.
#   4. **Both chrome states** (frame `04`): the same bar with `BAZ_BORDERLESS=1`,
#      where `app::owns_chrome` is true and the three window buttons draw. The
#      rule is written over *the trailing control*, not over the gear, and this
#      is the frame that shows it gives the same answer when the trailing
#      control is the close button instead.
#   5. **The measurement** — printed, not eyeballed. `measure.py` reads the
#      rightmost ink column of the gear, of the rail's letters and of the bottom
#      bar's volume groove out of each frame. This ask is literally about
#      pixels, and six false frames have been produced on this project.
#
# # The route is a listener's
#
# The Library is where the window opens, so no navigation is needed and none is
# done: every frame here is the first place, reached by launching. The pointer
# is **parked** in the returns lane's dead lower half before every shot, so no
# frame is a frame of a hover state. Every shot is `import -window root` cropped
# to the window at `+0+0`, with the window moved to 0,0 and sized explicitly.
#
# **The two builds differ by nothing above the bar**, unlike the app-bar study
# this one follows: `APP_BAR_H` is unchanged at 41, so there is no `y()` offset
# here and a literal y is the same pixel in both builds. That is worth stating
# because it is the assumption the whole comparison rests on, and it is checked
# — frame `02`'s crops are taken at one rectangle from both.
#
# Build both binaries **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   BIN0=/path/to/base BIN=/path/to/branch \
#     toolbox run -c baz-dev docs/design/impl/app-bar-gutter/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
BIN0=${BIN0:-}
FIX=${FIX:-/tmp/baz-density-fix}
OUT=${OUT:-$REPO/docs/design/impl/app-bar-gutter}
DISP=${DISP:-:198}
S=/tmp/baz-gutter-scratch

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
printf 'pcm.!default { type null }\nctl.!default { type null }\n' > "$S/home/.asoundrc"
mkdir -p "$S/config/baz"

# The shipped defaults for everything these frames are not about, **rewritten
# before every launch**: baz persists the arrangement, the density and the
# lane's state on exit, so a stray press in one build's walk would otherwise
# arrive in the other's config and the two would be photographed with different
# walls. A comparison whose two halves are not the same state is not one.
fresh_config() {
  printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
    "$FIX" > "$S/config/baz/config.toml"
}

launch() { # binary W H [extra env assignments...]
  local bin=$1 w=$2 h=$3; shift 3
  fresh_config
  rm -rf "$S/data/baz/library.sqlite"* "$S/cache/baz"
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$@" "$bin" >> "$S/app.log" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 60); do
    WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; kill "$APID" 2>/dev/null; return 1; fi
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$w" "$h"
  xdotool windowfocus --sync "$WID"
  sleep 10   # let the launch scan and the thumbnail decode land
}

# **Reap by the pid this script started, with a trap.** `kill` on a name is how
# 37 processes leaked on this machine once, and `kill` on a `toolbox run`
# wrapper does not reach the process inside the container — which is why this
# script runs *inside* the container and kills what it started itself.
stop() { [[ -n ${APID:-} ]] && kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; APID=""; sleep 0.6; }
cleanup() { stop; [[ -n ${XPID:-} ]] && kill "$XPID" 2>/dev/null; }
trap cleanup INT TERM EXIT

shot()  { sleep 1.0; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
crop()  { magick "$OUT/$1.png" -crop "$3" +repage "$OUT/$2.png"; }
# **Park the pointer in the returns lane's empty lower half** — the one region
# that is dead in both builds and in every place: below the playlists and above
# `Collapse`, no row, no control, no hover state.
park()  { xdotool mousemove 200 $(( H - 250 )); sleep 0.8; }

for size in "1280 860" "1920 1080"; do
  set -- $size; W=$1; H=$2

  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 2
  export DISPLAY=$DISP

  for build in before after; do
    bin=$BIN; [[ $build == before ]] && bin=$BIN0
    [[ -z $bin ]] && { echo "  (no BIN0 - skipping the before frames)"; continue; }

    # ---- 1 - the Library, the whole window
    launch "$bin" "$W" "$H" || exit 1
    park
    shot "01-library-${build}-${W}x${H}"
    stop

    # ---- 4 - the same bar with baz owning the chrome, so the three window
    # buttons draw and the *trailing control* is the close button rather than
    # the gear. Under Xvfb there is no window manager drawing a title bar
    # either way, so what this frame proves is which controls baz draws and
    # where, not what GNOME does with `decorations: false`.
    launch "$bin" "$W" "$H" BAZ_BORDERLESS=1 || exit 1
    park
    shot "04-borderless-${build}-${W}x${H}"
    stop
  done

  # ---- 2 - the right-hand edge: the bar's trailing 200 px over the rail's
  # first rows, before above after, at one rectangle from both builds.
  for state in library borderless; do
    src=01-library; [[ $state == borderless ]] && src=04-borderless
    for build in before after; do
      [[ -f "$OUT/${src}-${build}-${W}x${H}.png" ]] || continue
      crop "${src}-${build}-${W}x${H}" "edge-${state}-${build}-${W}x${H}" "200x260+$(( W - 200 ))+0"
    done
    if [[ -f "$OUT/edge-${state}-before-${W}x${H}.png" ]]; then
      magick "$OUT/edge-${state}-before-${W}x${H}.png" "$OUT/edge-${state}-after-${W}x${H}.png" \
             -background '#3A3A3A' -splice 0x2 -append \
             -filter point -resize 300% "$OUT/02-edge-${state}-3x-${W}x${H}.png"
      echo "  shot 02-edge-${state}-3x-${W}x${H}"
    fi
    rm -f "$OUT/edge-${state}-"*"-${W}x${H}.png"
  done

  # ---- 3 - zone 1, magnified: the word, then the mark.
  for build in before after; do
    [[ -f "$OUT/01-library-${build}-${W}x${H}.png" ]] || continue
    crop "01-library-${build}-${W}x${H}" "zone1-${build}-${W}x${H}" "72x41+32+0"
  done
  if [[ -f "$OUT/zone1-before-${W}x${H}.png" ]]; then
    magick "$OUT/zone1-before-${W}x${H}.png" "$OUT/zone1-after-${W}x${H}.png" \
           -background '#3A3A3A' -splice 0x2 -append \
           -filter point -resize 800% "$OUT/03-zone1-8x-${W}x${H}.png"
    echo "  shot 03-zone1-8x-${W}x${H}"
  fi
  rm -f "$OUT/zone1-"*"-${W}x${H}.png"

  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null; XPID=""
done

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE - check the log"

echo
echo "--- the measurement ---"
for size in "1280 860" "1920 1080"; do
  set -- $size; W=$1
  for f in "01-library" "04-borderless"; do
    for build in before after; do
      [[ -f "$OUT/${f}-${build}-${W}x$2.png" ]] || continue
      echo "== ${f}-${build}-${W}x$2"
      python3 "$OUT/measure.py" "$OUT/${f}-${build}-${W}x$2.png" "$W" "$2"
    done
  done
done
echo "done - $OUT"
