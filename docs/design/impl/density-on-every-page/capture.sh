#!/usr/bin/env bash
# Render **the fourth density step, and the control on every page that hangs
# works** against the real binary, headless, on a private Xvfb, with all six
# XDG redirections from docs/DEVELOPMENT.md §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and every fixture sample is a zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it.
#
# **It shoots two builds**, because two of the four claims are comparisons.
# `BIN0` is the commit this branch started from and `BIN` is the branch; the
# comparison frames are taken twice, from the same fixture, at the same window,
# with the same gestures.
#
# # The route is a listener's, and that is load-bearing
#
# Density is **one piece of state for the whole product**. So the step is set
# the way a listener sets it — by **pressing a detent mark with the pointer**,
# on the wall, where the marks close the index rail's lane — and then the other
# two places are walked to and photographed *without touching the control
# again*. That is the whole claim of the change in one gesture: set it once,
# and every place that hangs works follows. Before this branch, only the wall
# did.
#
# The marks are then pressed **again, on Home and on the artist page**, because
# a control that is only ever driven from the Library is not evidence that it
# is live where it is drawn (frames `05` and `06`).
#
# What it has to show, at 1280 × 860 and 1920 × 1080:
#
#   1. **Every step, on three pages** — the wall, Home and an artist's page —
#      reached by one press each on the wall's marks. Four steps × three pages
#      × two widths (frames `01`, `02`, `03`).
#   2. **The rail's foot, cropped**, so the four marks and which of them is lit
#      can be read without hunting (frame `07`).
#   3. **A page of rows carries no marks** (frame `04`): the record's page, at
#      both widths. This is the *decided absence* — density scales columns and
#      a page of rows has none — and a frame is how a reader checks that the
#      absence is clean rather than a control that failed to draw.
#   4. **The artist page against the wall at 1920 with the lane collapsed,
#      before and after** (frame `08`): the defect the owner named. Before, the
#      page draws six columns of 244 px art where the wall draws five of 294;
#      after, both draw the same because both read the same grid.
#
# Build both binaries **inside the toolbox** (a host-built release binary links
# a newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/impl/density-on-every-page/mkfixture.sh
#   BIN0=/tmp/baz-bin-before BIN=/tmp/baz-bin-after \
#     toolbox run -c baz-dev docs/design/impl/density-on-every-page/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
BIN0=${BIN0:-}
FIX=${FIX:-/tmp/baz-density-fix}
OUT=${OUT:-$REPO/docs/design/impl/density-on-every-page}
DISP=${DISP:-:195}
S=/tmp/baz-density-scratch

mkdir -p "$OUT"
rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
printf 'pcm.!default { type null }\nctl.!default { type null }\n' > "$S/home/.asoundrc"

mkdir -p "$S/config/baz"
# `artist` grouping, because the group header is the door to the artist's page
# and that is the route these frames take to it. The lane starts open: the
# comparison frames close it with its own `Collapse` control rather than with a
# config key, so the collapsed state in frame `08` is one a listener produced.
printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
  "$FIX" > "$S/config/baz/config.toml"

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
  sleep 10   # let the launch scan and the thumbnail decode land
}

stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
shot()  { sleep 1.0; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
crop()  { magick "$OUT/$1.png" -crop "$3" +repage "$OUT/$2.png"; echo "  shot $2"; }
row()   { magick "$OUT/$2.png" "$OUT/$3.png" +append "$OUT/$1.png"; echo "  shot $1"; }
stack() { magick "$OUT/$2.png" "$OUT/$3.png" -append "$OUT/$1.png"; echo "  shot $1"; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.2; }
typein(){ xdotool type --delay 45 "$1"; sleep 1.4; }
# Park the pointer in the strip's empty middle, where it states nothing and
# opens no hover slot — a tile's four options are hover-revealed, and a frame
# of the wall must not be a frame of the pointer.
park()  { xdotool mousemove 700 60; sleep 0.7; }

# ---------------------------------------------------------------- coordinates
#
# **The wall's marks**, from `views::shelf::density_control` and `theme.rs`:
# each is a `STEPPER_HIT` 24 box, right-aligned in `INDEX_LANE_W` with the
# sprite's ink on `W − HANG`, so the box's right edge is `W − (HANG −
# MARK_INSET)` = `W − 36` and its centre `W − 48`. The run's foot sits one
# un-zoomed `HANG` above the bar, and the bar is `BAR_CONTENT_H` 80 plus its
# hairline — so the run's bottom is `H − 121`, its top `H − 217` (four marks,
# 24 each), and mark *k* (0 = Spacious … 3 = Dense) centres on `H − 205 + 24k`.
#
# **The section rule's marks**, from `views::section_rule_hung`: right-aligned
# at the body's own trailing gutter, which `place_pad` puts at `HANG +
# SCROLLBAR_LANE` = 50 in — so the run's right edge is `W − 50`, its left
# `W − 146`, and mark *k* centres on `W − 134 + 24k`.
#
# The lane's `Collapse`, from `views::lane`: the foot of a body that is
# `H − 81` tall, less the lane's own `GAP_XL` 24 flank, less `marks`' `GAP_MD`
# 12, less half a `STEPPER_HIT` — `H − 131`.
mark_x()   { echo $(( W - 48 )); }                 # the wall's marks
mark_y()   { echo $(( H - 205 + 24 * $1 )); }
rule_x()   { echo $(( W - 134 + 24 * $1 )); }      # a section rule's marks
COLLAPSE_X=140

# The lane's head — `Home` and `Library`, at `SIDEBAR_ROW_H` 64 pitch under the
# well. The `×` that clears a query sits in the well's own mark box.
HOME_Y=84; LIB_Y=124; CLEAR_X=40; CLEAR_Y=40
# The filtered wall's `ALBUMS` block: its `HALVARD STEN` group header, the door
# to the artist's place. The `SONGS` block above it is eight rows of fixed
# height and the query resolves to the same eight at both widths, so this y
# does not move — with the density or with the window.
ARTIST_HEADER_Y=469
ARTIST_HEADER_X=350

STEPS=(spacious balanced compact dense)

# The wall's own marks, pressed. Density is one piece of state for the whole
# product, so this is the only place the control is touched in the loop below —
# and Home and the artist page are then *walked to*, which is the claim.
set_step() { click "$(mark_x)" "$(mark_y "$1")"; park; }

for size in "1280 860" "1920 1080"; do
  set -- $size; W=$1; H=$2
  COLLAPSE_Y=$(( H - 131 ))

  Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp & XPID=$!
  sleep 2
  export DISPLAY=$DISP

  # ============================================================ the branch
  rm -rf "$S/data/baz/library.sqlite"* "$S/cache/baz"
  launch "$BIN" "$W" "$H"
  park

  for k in 0 1 2 3; do
    step=${STEPS[$k]}
    # ---- 1 · the wall, at this step, set by pressing this step's own mark
    click "$(mark_x)" "$(mark_y "$k")"
    park
    shot "01-wall-${step}-${W}x${H}"
    # The rail's foot on its own: four marks, one of them lit.
    crop "01-wall-${step}-${W}x${H}" "07-rail-foot-${step}-${W}x${H}" \
         "72x120+$(( W - 72 ))+$(( H - 229 ))"

    # ---- 2 · Home, walked to, control untouched
    click 85 $HOME_Y
    park
    shot "02-home-${step}-${W}x${H}"

    # ---- 3 · an artist's page, walked to, control untouched
    click 88 $LIB_Y
    typein "halvard"
    click $ARTIST_HEADER_X $ARTIST_HEADER_Y
    park
    shot "03-artist-${step}-${W}x${H}"
    click $CLEAR_X $CLEAR_Y      # the well's own `×`
    click 88 $LIB_Y              # back to the wall for the next step
    park
  done

  # ---- 4 · a page of rows carries no marks (the decided absence)
  # A record's page, reached the way a listener reaches one: press its sleeve.
  click 88 $LIB_Y; park
  click 440 260
  park
  shot "04-record-page-${W}x${H}"
  click 88 $LIB_Y; park

  # ---- 5 · the marks are live on Home, pressed there
  # Home is left at `dense` by the loop above, so the `All songs` tile is at
  # its smallest and the `RECENTLY ADDED` rule is at its highest. The rule's y
  # is a function of the tile above it, so it is read from the frame rather
  # than assumed: `RULE_Y` is measured once per width in the README.
  click 85 $HOME_Y; park
  shot "05-home-before-press-${W}x${H}"
  RULE_Y=$(magick "$OUT/05-home-before-press-${W}x${H}.png" \
             -crop "104x$((H - 200))+$((W - 150))+80" +repage \
             -fuzz 45% -trim -format "%[fx:page.y+80+12]" info: 2>/dev/null)
  RULE_Y=${RULE_Y%%.*}
  [[ -z $RULE_Y || $RULE_Y -lt 80 ]] && RULE_Y=200
  echo "  home's rule row measured at y=$RULE_Y"
  click "$(rule_x 0)" "$RULE_Y"          # press Spacious, on Home
  park
  shot "05-home-after-press-${W}x${H}"

  # ---- 6 · the marks are live on an artist's page, pressed there
  # Its rule is the first thing under a header strip of fixed height, so the
  # row is at a fixed y: `place_pad`'s top 40 under the strip, the hairline,
  # `GAP_SM` 8, and half the 24 px box.
  click 88 $LIB_Y; typein "halvard"; click $ARTIST_HEADER_X $ARTIST_HEADER_Y; park
  ART_RULE_Y=110
  shot "06-artist-before-press-${W}x${H}"
  click "$(rule_x 3)" $ART_RULE_Y        # press Dense, on the artist's page
  park
  shot "06-artist-after-press-${W}x${H}"
  click $CLEAR_X $CLEAR_Y

  stop

  # ============================== 8 · the artist page against the wall, both
  # builds, at 1920 with the lane collapsed — the defect the owner named.
  if [[ $W -eq 1920 ]]; then
    for build in before after; do
      case $build in
        before) [[ -z $BIN0 ]] && { echo "  (no BIN0 — skipping the before frames)"; continue; }
                WHICH=$BIN0 ;;
        *)      WHICH=$BIN ;;
      esac
      rm -rf "$S/data/baz/library.sqlite"* "$S/cache/baz"
      # Both builds are photographed at **balanced**, the default and the only
      # step both builds have — the claim is about the *width* arithmetic, and
      # naming a step only one build knows would confound the two.
      printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
        "$FIX" > "$S/config/baz/config.toml"
      launch "$WHICH" "$W" "$H"
      park
      # **The artist's page first, then the collapse.** A type-anywhere query
      # *opens* the lane (`views::lane`'s collapsed-well clause), so a run that
      # collapsed first and searched second would photograph an open lane and
      # call it collapsed. Walk to the page while the lane is open, then close
      # it with its own `Collapse` control — one press, and the page is already
      # the subject.
      typein "halvard"
      # **Via a record, not via the shelf header.** The filtered wall's
      # `HALVARD STEN` header sits at a different y in the two builds, and a
      # coordinate that is right for one and wrong for the other would
      # photograph a wall and label it an artist's page — which it did, once.
      # The record's breadcrumb is at the header strip's own fixed height in
      # both, and it is the route ADR-0037 built the artist's page for:
      # *"we could add an Artist > album breadcrumb though"*.
      #
      # The press lands on the tile's **caption**, below the sleeve: the whole
      # tile is one button, and the sleeve carries four hover-revealed options
      # over it that a press in the middle would hit instead.
      click 400 797
      click 365 24
      park
      click $COLLAPSE_X $COLLAPSE_Y
      park
      shot "08-artist-collapsed-${build}-${W}x${H}"
      # Back to the wall, still collapsed and still filtered to the same
      # sixteen records — so the two frames are the *same records* at the same
      # window, and the only thing that can differ is the arithmetic.
      click 48 $LIB_Y
      park
      shot "08-wall-collapsed-${build}-${W}x${H}"
      # One band of tiles from each, the same height, cropped where that page
      # puts its first row: the artist page's under its `RECORDS` rule, the
      # filtered wall's under its `HALVARD STEN` shelf header.
      crop "08-artist-collapsed-${build}-${W}x${H}" \
           "08a-artist-band-${build}-${W}x${H}" "$(( W - 96 ))x430+96+130"
      crop "08-wall-collapsed-${build}-${W}x${H}" \
           "08b-wall-band-${build}-${W}x${H}" "$(( W - 96 ))x430+96+500"
      stack "08c-artist-over-wall-${build}-${W}x${H}" \
            "08a-artist-band-${build}-${W}x${H}" \
            "08b-wall-band-${build}-${W}x${H}"
      stop
    done
    # Restore the lane for any later run of this script.
    printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
      "$FIX" > "$S/config/baz/config.toml"
  fi

  kill "$XPID" 2>/dev/null; wait "$XPID" 2>/dev/null
done

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo "done — $OUT"
