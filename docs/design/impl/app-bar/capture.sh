#!/usr/bin/env bash
# Render **the app bar, in every place, before and after**, against the real
# binary, headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and every fixture sample is a zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it.
#
# **It shoots two builds.** `BIN0` is the commit this branch started from and
# `BIN` is the branch; every place is photographed twice, from the same
# fixture, at the same window size, at the same window coordinates, by the same
# gestures. The comparison is the whole point — the claim is *"one bar, the
# same in every place"*, and a claim about sameness can only be read off frames
# that were taken at the same coordinates.
#
# # The route is a listener's, and every frame says how it was reached
#
# Five false frames have been produced on this project, and every one of them
# came from a coordinate that was right for one build and wrong for the other,
# or right for one place and wrong for the one it was labelled. So:
#
#   * every navigation is a **pointer press on a control that is visible in the
#     frame before it**, never a keyboard shortcut and never a jump;
#   * the two builds differ by exactly `APP_BAR_H` 41 px in where the body
#     starts, so **every y below the bar is expressed as `$(y …)`**, which adds
#     the bar's height for the branch build and nothing for the base one. A
#     literal y would have photographed the right pixel in one build and the
#     wrong one in the other, which is failure mode one, twice;
#   * a record's page is reached by pressing a tile's **caption**, below the
#     sleeve, because the sleeve carries four hover-revealed options that a
#     press in the middle would hit instead;
#   * an artist's page is reached from a record's **breadcrumb**, not from a
#     filtered wall's shelf header — the header sits at a different y in the
#     two builds and that is exactly how a filtered wall got labelled an
#     artist's page once;
#   * the pointer is **parked** off every control before each shot, so no frame
#     is a frame of a hover state;
#   * every shot is `import -window root` cropped to the window at `+0+0`, and
#     the window is moved to 0,0 and sized explicitly, so the two builds are
#     photographed in one coordinate system.
#
# # What it has to show, at 1280 × 860 and 1920 × 1080
#
#   1. **The bar in all seven places** (frames `01`…`07`), before and after —
#      Library, Home, a record's page, a playlist's page, an artist's page,
#      Now playing, Settings. The `after` set is the ask: one band, identical,
#      at the top of every one of them.
#   2. **The band cropped from all seven and stacked** (frame `10`), which is
#      how *"the same on all screens"* is actually checked: seven 41 px strips
#      one above another, in which the gear and the window buttons must not
#      move by a pixel and the display options must be present on exactly
#      three.
#   3. **`Play all` is gone** (frame `01`): the before frame has it beside the
#      arrangement row, the after frame does not.
#   4. **The display options moved** (frames `01`, `08`): before, at the foot
#      of the index rail's lane; after, in the bar. And they are **live from
#      the bar** — frame `08` presses `Dense` in the bar on the Library and
#      the wall re-hangs.
#   5. **The window controls work** (frame `09`): maximise pressed from the
#      bar, and the button's own drawing changed from a square to two offset
#      squares because the window is now maximised.
#   6. **Borderless** (frame `11`): the same window with `BAZ_BORDERLESS=1`,
#      which is the one field between this branch and what the owner asked
#      for. Under Xvfb there is no window manager drawing a title bar either
#      way, so this frame proves the *setting is wired*, not what GNOME does
#      with it — which is stated here rather than implied, because a frame that
#      cannot show what it claims is worse than no frame.
#
# Build both binaries **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   BIN0=/path/to/base BIN=/path/to/branch \
#     toolbox run -c baz-dev docs/design/impl/app-bar/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
BIN0=${BIN0:-}
FIX=${FIX:-/tmp/baz-density-fix}
OUT=${OUT:-$REPO/docs/design/impl/app-bar}
DISP=${DISP:-:197}
S=/tmp/baz-appbar-scratch

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
printf 'pcm.!default { type null }\nctl.!default { type null }\n' > "$S/home/.asoundrc"

mkdir -p "$S/config/baz"
# `artist` grouping, the lane open, `balanced` density — the shipped defaults
# for everything the frames are not about, so that what differs between the
# two builds is the bar and nothing else.
#
# **Rewritten before every launch**, which the first two runs of this script
# did not do and paid for: baz persists the arrangement, the density and the
# lane's state on exit, so a stray press in the base build's walk arrived in
# the branch build's config and the two were photographed with different
# walls. A comparison whose two halves are not the same state is not a
# comparison. `fresh_config` is called by `launch`.
fresh_config() {
  printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
    "$FIX" > "$S/config/baz/config.toml"
}
fresh_config

# **A playlist to photograph.** A place cannot be shot if nothing takes you
# there, and this product has no playlists until someone makes one. Written as
# a file rather than built through the context menu because it is *fixture*,
# not evidence: what these frames claim is about the bar above the playlist's
# page, and a scripted six-press detour to create the list would be six more
# coordinates that could be wrong. baz reads any `.m3u8` in this folder.
mkdir -p "$S/data/baz/playlists"
{
  printf '#EXTM3U\n'
  for track in "$FIX/01 - Halvard Sten - Closing Time"/*.flac; do printf '%s\n' "$track"; done
} > "$S/data/baz/playlists/Evening.m3u8"

launch() { # binary W H [extra env assignments…]
  local bin=$1 w=$2 h=$3; shift 3
  fresh_config
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

# **Reap by anchored full path, with a trap.** `kill` on a name is how 37
# processes leaked on this machine today; `kill` on a `toolbox run` wrapper
# does not reach the process inside the container, which is why this script
# runs *inside* the container and kills the pid it started itself.
stop() { [[ -n ${APID:-} ]] && kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; APID=""; sleep 0.6; }
cleanup() { stop; [[ -n ${XPID:-} ]] && kill "$XPID" 2>/dev/null; }
trap cleanup INT TERM EXIT

shot()  { sleep 1.0; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
crop()  { magick "$OUT/$1.png" -crop "$3" +repage "$OUT/$2.png"; echo "  shot $2"; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.4; }
# **Park the pointer in the returns lane's empty lower half**, which is the
# one region that is dead in *both* builds and in every place: below the
# playlists and above `Collapse`, no row, no control, no hover state.
#
# Two earlier parks were wrong and each produced a frame of the pointer rather
# than of the design: the wall (a tile opened its four hover options) and the
# app bar's middle (dead on the branch, but the *base* build has the Library
# strip there, and `PLAYED` came up lit).
park()  { xdotool mousemove 200 $(( H - 250 )); sleep 0.8; }

# ---------------------------------------------------------------- coordinates
#
# **The one offset that matters.** The branch adds `APP_BAR_H` 41 px above
# everything; the base build has nothing there. `BAR` is set per build and
# `y()` adds it, so one set of coordinates drives both and neither is a guess.
BAR=0
y() { echo $(( $1 + BAR )); }

# The returns lane's head, from `views::lane`: the well first at
# `SIDEBAR_WELL_H` 32 under the lane's own `GAP_XL` 24 flank, then the three
# destinations at `SIDEBAR_DEST_H` 40 pitch. These are the base build's
# numbers, measured off its own frames, and `y()` carries them to the branch.
LANE_X=88
HOME_Y=84
LIB_Y=124
NP_Y=164
# The first record's tile on the wall — **the first shelf's**, not a later
# one. The press lands on the **caption**, below the sleeve, because the
# sleeve holds four hover-revealed options that a press in the middle would
# hit instead.
#
# The first shelf is chosen because its y is the one the wall cannot move: it
# sits directly under the first group header, at the top of an unscrolled
# wall, at every density and in both builds. A tile two shelves down is a
# function of how many records the shelves above it hold, which is a function
# of the fixture — and 797 (a later shelf's caption, inherited from another
# study) landed in the gap between sleeve and caption in the base build while
# hitting the caption in the branch. That is one press, right in one build and
# wrong in the other, which is the whole family of false frames this project
# has produced.
TILE_X=440
TILE_Y=412
# A record page's breadcrumb — `Artist › Album`, whose first half is the door
# to the artist's page. It stands in the strip's own fixed-height lead, so its
# y is the same in every place that wears one.
# `Corvin › Red Shift` starts at the strip's `HANG` inside the body — x 320 at
# every width, the artist's name running to about 365. **340, not 365**: the
# first run of this script pressed 365, which is the separator rather than the
# door, and photographed the record's page while labelling it the artist's.
# That is failure mode four, reproduced exactly, and it is why every frame in
# this study was looked at rather than counted.
CRUMB_X=340
CRUMB_Y=24
# The playlist's row in the lane's list, under the head's three destinations
# and their hairline.
PL_Y=268

# The app bar's own controls, from `views::app_bar` and `theme.rs`. Everything
# here is measured from the window's **right** edge, because that is what the
# reserved slots make constant.
#
# **Derived box by box, not from the two `*_FROM_RIGHT` constants**, and that
# is the correction the first run of this script needed: those constants name
# each slot's *trailing* edge, and reading them as centres put the density
# press on the minimise button and the gear press on the window controls. Two
# frames were photographed, labelled, and wrong. So the boxes are laid out
# here the way the row lays them out — right to left, from `W − HANG`:
#
#   close   W−1880…  → box [W−72, W−40)   centre W−56
#   maximise         → box [W−108, W−76)  centre W−92
#   minimise         → box [W−144, W−112) centre W−128
#   (GAP_LG 16)
#   gear             → box [W−192, W−160) centre W−176
#   (GAP_LG 16)
#   marks, 4 x 24    → box [W−304, W−208) centre of k: W−292+24k
#
# The band's own centre line is `APP_BAR_PAD_V` 4 + half a control = 20.
BAR_MID=20
# The buttons are minimise, maximise, close, right-aligned in their slot, on
# the right, always — the owner's decision of 2026-08-10. There is no platform
# branch to parameterise here any more, which is why these are three literals
# off one edge rather than a lookup.
close_x()  { echo $(( W - 40 - 16 )); }
max_x()    { echo $(( W - 40 - 16 - 36 )); }
min_x()    { echo $(( W - 40 - 16 - 72 )); }
gear_x()   { echo $(( W - 176 )); }
mark_x()   { echo $(( W - 292 + 24 * $1 )); }        # k = 0 Spacious … 3 Dense

# The wall's own density marks in the **base** build: at the foot of the index
# rail's lane, `W − 48` across, the run's bottom one `HANG` above the bar.
old_mark_x() { echo $(( W - 48 )); }
old_mark_y() { echo $(( H - 205 + 24 * $1 )); }

# --------------------------------------------------------------- the walkabout
#
# One function, run once per build, so the two are the *same* walk by
# construction rather than by two lists agreeing.
walk() { # tag
  local tag=$1
  park
  shot "01-library-${tag}-${W}x${H}"

  click $LANE_X "$(y $HOME_Y)"; park
  shot "02-home-${tag}-${W}x${H}"

  click $LANE_X "$(y $LIB_Y)"; park
  click $TILE_X "$(y $TILE_Y)"; park
  shot "03-record-${tag}-${W}x${H}"

  # The artist's page, from the record's own breadcrumb.
  click $CRUMB_X "$(y $CRUMB_Y)"; park
  shot "05-artist-${tag}-${W}x${H}"

  click $LANE_X "$(y $LIB_Y)"; park
  click $LANE_X "$(y $PL_Y)"; park
  shot "04-playlist-${tag}-${W}x${H}"

  click $LANE_X "$(y $NP_Y)"; park
  shot "06-nowplaying-${tag}-${W}x${H}"

  # **Settings, reached by pressing the gear** — which on the branch is the
  # bar's own gear, in every place, and on the base build is the Library
  # strip's, so the base walk has to go back to the Library first. That
  # difference *is* one of the claims, so it is spelled rather than smoothed.
  if [[ $tag == before ]]; then
    click $LANE_X "$(y $LIB_Y)"; park
    click $(( W - 56 )) $(( 24 ))
  else
    click "$(gear_x)" $BAR_MID
  fi
  park
  shot "07-settings-${tag}-${W}x${H}"

  # The seven bands, stacked: the frame that actually answers *"the same on
  # all screens"*.
  local bands=()
  for n in 01-library 02-home 03-record 04-playlist 05-artist 06-nowplaying 07-settings; do
    crop "${n}-${tag}-${W}x${H}" "band-${n}-${tag}-${W}x${H}" "${W}x$(( BAR > 0 ? BAR : 49 ))+0+0"
    bands+=("$OUT/band-${n}-${tag}-${W}x${H}.png")
  done
  magick "${bands[@]}" -append "$OUT/10-every-band-${tag}-${W}x${H}.png"
  rm -f "${bands[@]}"
  echo "  shot 10-every-band-${tag}-${W}x${H}"
}

for size in "1280 860" "1920 1080"; do
  set -- $size; W=$1; H=$2

  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 2
  export DISPLAY=$DISP

  # ===================================================== the base commit
  if [[ -n $BIN0 ]]; then
    rm -rf "$S/data/baz/library.sqlite"* "$S/cache/baz"
    BAR=0
    launch "$BIN0" "$W" "$H" && walk before
    stop
  else
    echo "  (no BIN0 — skipping the before frames)"
  fi

  # ========================================================== the branch
  rm -rf "$S/data/baz/library.sqlite"* "$S/cache/baz"
  BAR=41
  launch "$BIN" "$W" "$H" || exit 1
  walk after

  # ---- 8 · the display options are live **from the bar**
  click $LANE_X "$(y $LIB_Y)"; park
  shot "08-density-before-press-${W}x${H}"
  click "$(mark_x 3)" $BAR_MID          # press Dense, in the bar
  park
  shot "08-density-after-press-${W}x${H}"
  click "$(mark_x 1)" $BAR_MID          # back to Balanced for the rest
  park

  # ---- 12 · the four display options at the bar's real size, and 4x
  # A press-free crop of zone 3, so the owner's *"the way they appear for the
  # library is nice"* can be checked against how they actually came out in a
  # 41 px band — and a nearest-neighbour magnification beside it, because the
  # question standing over the `Dense` mark is whether a 4 x 4 of 2.25 px
  # cells reads as sixteen works or as mush. Magnified with `-filter point`:
  # a smooth resample would answer the question by blurring it.
  crop "08-density-before-press-${W}x${H}" "12-marks-1x-${W}x${H}" "112x41+$(( W - 312 ))+0"
  magick "$OUT/12-marks-1x-${W}x${H}.png" -filter point -resize 400% \
         "$OUT/12-marks-4x-${W}x${H}.png"
  echo "  shot 12-marks-4x-${W}x${H}"

  # ---- 9 · the window controls: maximise, from the bar
  crop "08-density-after-press-${W}x${H}" "09-buttons-before-${W}x${H}" \
       "260x41+$(( W - 260 ))+0"
  click "$(max_x)" $BAR_MID
  sleep 1.5
  shot "09-maximised-${W}x${H}"
  crop "09-maximised-${W}x${H}" "09-buttons-after-${W}x${H}" "260x41+$(( W - 260 ))+0"
  # …and restore, so the window is where the next frame expects it.
  click "$(max_x)" $BAR_MID
  sleep 1.5
  stop

  # ---- 11 · borderless: the one field that is not flipped by default
  rm -rf "$S/data/baz/library.sqlite"* "$S/cache/baz"
  launch "$BIN" "$W" "$H" BAZ_BORDERLESS=1 && { park; shot "11-borderless-${W}x${H}"; }
  stop

  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null; XPID=""
done

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo "done — $OUT"
