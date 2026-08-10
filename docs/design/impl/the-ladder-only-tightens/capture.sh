#!/usr/bin/env bash
# Render **the density ladder, before and after it was made monotonic**, against
# the real binary, headless, on a private Xvfb, with all six XDG redirections
# from docs/DEVELOPMENT.md §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and every fixture sample is a zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it.
#
# **It shoots two builds.** `BIN0` is the commit this branch started from and
# `BIN` is the branch. Every frame is taken twice, from the same fixture, at the
# same window, moved to the same screen coordinates, with the same gestures —
# so a before/after pair differs in the arithmetic and in nothing else.
#
# # What it has to show
#
# The defect is that a **tighter** density step drew **larger** works, because
# each step carries its own hang and the art rises as the hang falls. It appears
# and disappears with the window, so the widths are chosen rather than
# convenient — three of them, each one an inversion in the `before` column:
#
#   * **1120 × 860 → 728 px of grid.** `Spacious` 2 × 292 against `Balanced`
#     2 × 304 — the *looser* step smaller. This pair predates the fourth step,
#     which is what makes this the frame that shows `Compact` **exposed** the
#     defect rather than causing it.
#   * **1280 × 860 → 888 px of grid**, the shipped window. `Balanced`
#     3 × 242.7 against `Compact` 3 × 253.3 — **the owner's sentence,
#     photographed** (*"why is balanced smaller than compact"*).
#   * **1600 × 1000 → 1208 px of grid.** `Balanced` 4 × 252 against `Compact`
#     4 × 262, and the width where the second half of the ask is loudest:
#     `Dense` goes 5 × 208 to 6 × 168.7.
#
# The grid is the window less **392** — `SIDEBAR_W` 280 for the open lane,
# `INDEX_LANE_W` 108 for the index rail, and the scrollbar's own lane. That
# number was measured off the frames rather than assumed, which is how the
# first pass at this script came to name three widths that were 8 px out.
#
# # The route is a listener's
#
# The step is set the way a listener sets it: by **pressing that step's own
# detent mark with the pointer**, at the foot of the index rail's lane, once per
# frame. No config key names a step, so a frame cannot be of a wall the control
# never reached. The pointer is then parked in the strip's empty middle, because
# a tile's options are hover-revealed and a frame of the wall must not be a
# frame of the pointer.
#
# The wall is grouped by **genre**, so the first shelf is `AMBIENT` — eighteen
# records, which fills every row at every step and both widths. The alternative
# (the default artist grouping) puts a two-record shelf first, and two covers
# cannot show a column count.
#
# # The measurement is taken from the frames, not from the code
#
# `measure()` scans a pixel row through the first row of covers and reports the
# left edge, the width of the first work and the gap to the next. Those are the
# numbers the README quotes. A frame that photographed the wrong wall shows up
# as a width that does not match the sweep, which is the check that the
# coordinates below are still right.
#
# Build both binaries **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=/tmp/tb-after \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/impl/density-on-every-page/mkfixture.sh /tmp/baz-ladder-fix
#   toolbox run -c baz-dev env BIN0=/tmp/tb-before/release/baz BIN=/tmp/tb-after/release/baz \
#     FIX=/tmp/baz-ladder-fix docs/design/impl/the-ladder-only-tightens/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
# No apostrophes in these messages: the text of a `${VAR:?…}` is still parsed
# for quotes, so one would open a string that runs to the next `'` in the file.
BIN=${BIN:?set BIN to the branch binary}
BIN0=${BIN0:?set BIN0 to the binary this branch started from}
FIX=${FIX:-/tmp/baz-ladder-fix}
OUT=${OUT:-$REPO/docs/design/impl/the-ladder-only-tightens}
DISP=${DISP:-:196}
S=${S:-/tmp/baz-ladder-scratch}

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
printf 'pcm.!default { type null }\nctl.!default { type null }\n' > "$S/home/.asoundrc"
mkdir -p "$S/config/baz"

APID=""; XPID=""
# Reap by the PID this script started, with a trap, and never by name: a
# `pkill baz` on this machine has taken out the owner's own running instance,
# and a `kill` aimed at a `toolbox run` wrapper does not reach the process
# inside the container at all. This script runs *inside* the container, so the
# PID it holds is the real one.
reap() {
  [[ -n $APID ]] && { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; }
  [[ -n $XPID ]] && { kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null; }
  APID=""; XPID=""
}
trap 'reap; exit 130' INT TERM

# The wall is grouped by genre so the first shelf is `AMBIENT`, eighteen
# records deep. The step is **not** named here — every frame's step is reached
# by pressing its own mark, so `balanced` is only ever the starting point.
config() {
  printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "genre"\ndensity = "balanced"\nsidebar_open = true\n' \
    "$FIX" > "$S/config/baz/config.toml"
}

launch() { # binary W H
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$1" >> "$S/app.log" 2>&1 &
  APID=$!
  local WID=""
  for _ in $(seq 1 60); do
    WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; reap; exit 1; fi
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$2" "$3"
  xdotool windowfocus --sync "$WID"
  sleep 10   # the launch scan and the thumbnail decode
}

stop()  { [[ -n $APID ]] && { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; APID=""; }; sleep 0.6; }
shot()  { sleep 1.0; magick import -window root -crop "${W}x${H}+0+0" +repage "$OUT/$1.png"; echo "  shot $1"; }
crop()  { magick "$OUT/$1.png" -crop "$3" +repage "$OUT/$2.png"; }
_paths() { local p=(); for f in "$@"; do p+=("$OUT/$f.png"); done; printf '%s\n' "${p[@]}"; }
# A band, captioned with the step it is and the work it drew. The frames are
# the evidence and a reader must not have to count rows to know which rung a
# row is — the first pass at this shot had four unlabelled bands and the only
# way to read it was to already know the answer.
label() { # in-name out-name "caption"
  magick "$OUT/$1.png" -background '#0d0d0d' -fill '#e8e4dc' -pointsize 22 \
    -gravity northwest -splice 0x34 -annotate +12+6 "$3" "$OUT/$2.png"
}
stack() { local o=$1; shift; mapfile -t p < <(_paths "$@"); magick "${p[@]}" -append "$OUT/$o.png" && echo "  shot $o"; }
row()   { local o=$1; shift; mapfile -t p < <(_paths "$@"); magick "${p[@]}" +append "$OUT/$o.png" && echo "  shot $o"; }
park()  { xdotool mousemove 700 60; sleep 0.8; }

# **The wall's marks**, from `views::shelf::density_control` and `theme.rs`:
# each is a `STEPPER_HIT` 24 box right-aligned in `INDEX_LANE_W` with its ink on
# `W − HANG`, so the box centres on `W − 48`. The run's foot is one un-zoomed
# `HANG` above the bar (`BAR_CONTENT_H` 80 and its hairline), so its bottom is
# `H − 121`, its top `H − 217`, and mark *k* (0 = Spacious … 3 = Dense) centres
# on `H − 205 + 24k`. Verified against the frames by `measure()` below: a mark
# that missed would leave two steps drawing the same wall.
mark_x() { echo $(( W - 48 )); }
mark_y() { echo $(( H - 205 + 24 * $1 )); }

# The first row of covers, in window coordinates. The wall's top is fixed and
# the shelf header is one hang (28 … 48), so the row's top drifts by 20 px
# across the ladder and `COVER_Y` 250 is inside the first work at every step —
# the tightest step's work is 160 px on an edge at worst, which still spans it.
COVER_Y=250
# The wall's own left edge: the lane is open at every width here, so it is
# `SIDEBAR_W` 280. Everything left of it is the lane and is not the grid.
WALL_X=280

# Scan one pixel row and report the first work's left edge, its width, and the
# gap to the next work — measured off the frame, in the frame's own pixels.
#
# **The reported width is `art − 4` at every step and every width**, because a
# sleeve is drawn inside a `theme::SLEEVE_MAT` 2 mat and the scan sees the
# picture, not the tile. It is a constant, so it is left in rather than
# corrected away: a frame whose width is off by anything other than 4 is a
# frame of a wall this script did not mean to take.
measure() { # png
  magick "$OUT/$1.png" -crop "$(( W - WALL_X ))x1+${WALL_X}+${COVER_Y}" +repage \
    -depth 8 txt: | python3 -c '
import sys,re
xs=[]
for line in sys.stdin:
    m=re.match(r"(\d+),0: \(([^)]*)\)", line)
    if not m: continue
    r,g,b=[int(v) for v in m.group(2).split(",")[:3]]
    xs.append((int(m.group(1)), r+g+b))
if not xs: print("  (no pixels)"); raise SystemExit
base=xs[0][1]
on=[x for x,v in xs if abs(v-base)>24]
if not on: print("  (no work found on this row)"); raise SystemExit
runs=[];s=p=on[0]
for x in on[1:]:
    if x>p+2: runs.append((s,p)); s=x
    p=x
runs.append((s,p))
runs=[(a,b) for a,b in runs if b-a>40]
if not runs: print("  (no work wide enough)"); raise SystemExit
first=runs[0]
gap = runs[1][0]-first[1]-1 if len(runs)>1 else 0
print("  margin %d  work %d  gutter %d  works-on-row %d"
      % (first[0], first[1]-first[0]+1, gap, len(runs)))
'
}

STEPS=(spacious balanced compact dense)

for size in "1120 860" "1280 860" "1600 1000"; do
  set -- $size; W=$1; H=$2
  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 2
  export DISPLAY=$DISP

  for build in before after; do
    case $build in before) WHICH=$BIN0 ;; *) WHICH=$BIN ;; esac
    rm -rf "$S/data/baz/library.sqlite"* "$S/cache/baz"
    config
    launch "$WHICH" "$W" "$H"
    park
    echo "== $build, ${W}x${H} =="
    for k in 0 1 2 3; do
      step=${STEPS[$k]}
      # The step's own mark, pressed. This is the only thing that sets density.
      xdotool mousemove "$(mark_x)" "$(mark_y "$k")"; sleep 0.3; xdotool click 1; sleep 1.4
      park
      shot "01-wall-${step}-${build}-${W}x${H}"
      echo -n "  ${step}:"
      read -r _ MARGIN _ WORK _ GUTTER _ COLS < <(measure "01-wall-${step}-${build}-${W}x${H}" | tee /dev/stderr)
      # One band of the first cover row, cropped at the same y in both builds,
      # so the ladder can be read down a column as four sizes of one wall.
      crop "01-wall-${step}-${build}-${W}x${H}" \
           "02-band-${step}-${build}-${W}x${H}" \
           "$(( W - WALL_X ))x340+${WALL_X}+110"
      # The caption carries the measured work width, so the frame states its
      # own evidence: `art − 4` is what the scan sees (the sleeve's mat).
      label "02-band-${step}-${build}-${W}x${H}" \
            "02-band-${step}-${build}-${W}x${H}" \
            "${step^} — $((WORK + 4)) px works"
    done
    stop
    # The ladder itself: four steps down one column, loosest first.
    stack "03-ladder-${build}-${W}x${H}" \
          "02-band-spacious-${build}-${W}x${H}" \
          "02-band-balanced-${build}-${W}x${H}" \
          "02-band-compact-${build}-${W}x${H}" \
          "02-band-dense-${build}-${W}x${H}"
    label "03-ladder-${build}-${W}x${H}" "03-ladder-${build}-${W}x${H}" \
          "${build^} — ${W} × ${H}"
  done

  # Before beside after, one window per frame: the inversion is a *break in the
  # sequence* down the left column and a clean descent down the right.
  row "04-before-beside-after-${W}x${H}" \
      "03-ladder-before-${W}x${H}" "03-ladder-after-${W}x${H}"

  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null; XPID=""
done

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo "done — $OUT"
