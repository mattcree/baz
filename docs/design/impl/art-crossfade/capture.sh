#!/usr/bin/env bash
# Render **the hero's crossfade** — the artwork dissolving when the record
# changes, and the field travelling with it — against the real binary,
# headless, on a private Xvfb, with all six XDG redirections from
# docs/DEVELOPMENT.md §"Headless UI verification".
#
# Nothing touches the owner's session and nothing is audible: the scratch HOME
# routes ALSA's default PCM to null and every fixture sample is a zero. The
# run's `[mpris] no session bus` line is the receipt that the isolation held,
# and this script prints it.
#
# # A crossfade is motion, and a still frame cannot prove it
#
# So this script **films** rather than screenshots. `ffmpeg -f x11grab` at
# 60 fps records the private display across the gestures; `measure.py` then
# reads every frame back and reports, per frame, how far the sleeve and the
# field have travelled between the two records. The frames committed beside
# this file are extracted from that film, so what is shown is what the
# compositor actually put on the screen and not a state the script arranged.
#
# `ffmpeg` is not in the `baz-dev` toolbox and the binary cannot run on the
# host (a host-built release links a newer glibc than the container has), so
# the split is: **Xvfb, the app and xdotool inside the container; ffmpeg,
# ImageMagick and python on the host**. `/tmp/.X11-unix` is shared between the
# two, which is what lets one grab the other's display.
#
# # The gestures are one gesture, pressed three times
#
# The whole claim is a *comparison*, and the strongest form of it is the same
# act producing motion once and no motion twice:
#
#   1. `Play all`, then the run column's last row of the first record — one
#      click each, both of them things a listener does — puts the cursor on
#      **the last track of `Ochre`**.
#   2. <kbd>Ctrl</kbd>+<kbd>→</kbd> is Next (`crate::keys`). The first press
#      crosses into `Violet Ledger`: **the record changes, so the picture
#      dissolves.**
#   3. The second and third presses are Next again, *inside* `Violet Ledger`:
#      **the record does not change, so nothing moves at all.** That is the
#      negative case the feature is defined by — consecutive tracks on one
#      record share a cover, and fading a picture into an identical picture is
#      a flicker nobody can find a reason for.
#
# It shoots two builds, because "there was no transition before" is half the
# claim. `BIN0` is the commit this branch started from and `BIN` is the branch;
# the same film is taken from both, at the same window, with the same presses.
#
# Build both binaries inside the toolbox:
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lane-fix
#   BIN0=/tmp/baz-before BIN=/tmp/baz-after \
#     docs/design/impl/art-crossfade/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
BIN0=${BIN0:-}
FIX=${FIX:-/tmp/baz-lane-fix}
OUT=${OUT:-$REPO/docs/design/impl/art-crossfade}
DISP=${DISP:-:195}
S=${S:-/tmp/baz-xfade-scratch}
# The films live **outside** the scratch world, because the world is wiped per
# build and the frames of the first build have to outlive the second — both so
# the two can be re-measured without re-shooting and so a probe that turns out
# to be in the wrong place costs a `measure.py` run rather than a capture.
FILM=${FILM:-$S-films}
W=1280
H=860

mkdir -p "$OUT" "$FILM"

tb()  { toolbox run -c baz-dev env DISPLAY="$DISP" "$@"; }

# **Killing a `toolbox run` does not kill what it started.** The wrapper is a
# `podman exec` on the host; the process it launched lives in the container and
# survives the wrapper's death. Every run of an earlier draft of this script
# therefore left a `baz` and an `Xvfb` behind, and enough of them accumulated
# to exhaust the machine's thread limit — the symptom was the app failing to
# start its audio and D-Bus threads with `EAGAIN`, which reads like a bug in
# the product and is a bug in the harness.
#
# A toolbox shares the host's PID namespace, so the host can see and signal
# them. `reap` matches the **whole command line, anchored** — never a bare
# name, which would also match the maintainer's own running copy of baz.
reap() {
  local pid
  pid=$(pgrep -x -f "$1" || true)
  [[ -n $pid ]] && kill $pid 2>/dev/null
  for _ in $(seq 1 40); do
    pgrep -x -f "$1" >/dev/null || return 0
    sleep 0.25
  done
  pgrep -x -f "$1" >/dev/null && kill -9 $(pgrep -x -f "$1") 2>/dev/null
  return 0
}
key() { tb xdotool key "$1"; }
# Park the pointer on the bottom bar's dead ground: the run's rows carry
# hover-revealed controls (a ✕, a `+`, the ▲▼), and a frame of the composition
# must not be a frame of the pointer.
# The lane's dead ground, below `RECENT` and above `Collapse`. The bottom
# bar is *not* dead ground: resting on Next raises its preview tooltip, which
# is a frame of the pointer rather than of the composition.
park(){ tb xdotool mousemove 140 450; sleep 0.5; }
click(){ tb bash -c "xdotool mousemove $1 $2; sleep 0.3; xdotool click 1"; sleep 1.4; }

# ---------------------------------------------------------------- the display
XVFB="Xvfb $DISP -screen 0 ${W}x${H}x24 -nolisten tcp"
toolbox run -c baz-dev $XVFB >/dev/null 2>&1 &
XPID=$!
sleep 2
# Both halves, always: the wrapper on the host and the process in the
# container. An interrupted run must leave nothing behind either.
trap 'kill $XPID 2>/dev/null; reap "$XVFB"; [[ -n ${WHICH:-} ]] && reap "$WHICH"' EXIT INT TERM

for build in before after; do
  case $build in
    before) [[ -z $BIN0 ]] && { echo "  (no BIN0 — skipping the before film)"; continue; }
            WHICH=$BIN0 ;;
    *)      WHICH=$BIN ;;
  esac

  # ------------------------------------------------- a scratch world per build
  rm -rf "$S"; mkdir -p "$S"/{home,data,config,cache,run}; chmod 700 "$S/run"
  printf 'pcm.!default { type null }\nctl.!default { type null }\n' > "$S/home/.asoundrc"
  mkdir -p "$S/config/baz"
  printf 'music_dirs = [\n    "%s",\n]\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
    "$FIX" > "$S/config/baz/config.toml"

  toolbox run -c baz-dev env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS \
      DISPLAY="$DISP" WINIT_UNIX_BACKEND=x11 HOME="$S/home" \
      XDG_DATA_HOME="$S/data" XDG_CONFIG_HOME="$S/config" \
      XDG_CACHE_HOME="$S/cache" XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$WHICH" >> "$S/app.log" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 60); do
    WID=$(tb timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; exit 1; fi
  tb bash -c "xdotool windowmove $WID 0 0; xdotool windowsize $WID $W $H; xdotool windowfocus --sync $WID"
  # **The launch scan, and long enough for it under load.** `Play all` queues
  # the library in wall order, so pressing it before the scan has finished
  # queues a *different* library and the film is of some other record — which
  # happened, silently, on a loaded machine at eight seconds. `measure.py`
  # refuses a film whose two ends are the same record, so a race is now loud;
  # this is what stops it happening.
  sleep 20

  # ------------------------------------------------------------ the two clicks
  click 745 24     # `Play all` — the whole library as one run, in wall order
  sleep 2
  click 105 164    # the lane's `Now playing`
  # The run column's row 12 — `Nightwatch 12`, the **last** track of `Ochre`.
  # Clicking a run row plays from there (ADR-0014's `JumpTo`); a single press,
  # which is the gesture, and never a double one.
  click 900 512
  sleep 2
  park
  DISPLAY=$DISP magick import -window root -crop ${W}x${H}+0+0 +repage \
    "$OUT/00-before-the-change-$build.png"

  # ------------------------------------------------------------------ the film
  # Six seconds at 60 fps across three presses of Next. The first crosses into
  # `Violet Ledger`; the second and third stay inside it.
  # **All three presses in one container call, and the film starts when the
  # container says it is ready.** Each `toolbox run` is a `podman exec` and
  # costs about two seconds to start — enough, measured, to push two of the
  # three presses off the end of a six-second film and leave a "negative case"
  # that was really a key that never arrived. So the sleeps live inside the
  # container, where they are the only thing between the keys, and the shell
  # waits on a file the container touches before the first of them.
  rm -f "$S/ready"
  cat > "$S/press.sh" <<'PRESS'
#!/usr/bin/env bash
# 1: the record changes.  2 and 3: it does not — the same key, inside one
# record, which is the negative case the feature is defined by.
touch "$1/ready"
for _ in 1 2 3; do sleep 2.6; xdotool key ctrl+Right; done
PRESS
  chmod +x "$S/press.sh"
  tb bash "$S/press.sh" "$S" &
  KPID=$!
  for _ in $(seq 1 400); do [[ -e $S/ready ]] && break; sleep 0.05; done
  # **`rawvideo`, not a codec.** The app software-rasterises on Xvfb — there
  # is no GPU on this display — so ffmpeg and the renderer compete for the same
  # cores, and an encoder in the loop steals frames from the very transition
  # being filmed. Raw frames cost disk (about 2.4 GB for twelve seconds) and no
  # arithmetic. The file is deleted with the scratch world.
  DISPLAY=$DISP ffmpeg -y -loglevel error -f x11grab -framerate 60 \
      -video_size ${W}x${H} -i "$DISP" -t 12 -c:v rawvideo "$S/film.nut"
  wait $KPID

  # ------------------------------------------------------------------ the cost
  # **The owner's standing rule is responsiveness, so the idle is measured
  # rather than asserted** (ADR-0020 §Measurements, `docs/design/04-fluidity.md`
  # §1.4). The transition has settled by now — its subscription is a function
  # of state, so a tween that has landed removes the timer — and the process
  # must be spending nothing. Read off the host's own `/proc`, which sees the
  # container's PIDs because a toolbox shares the PID namespace.
  #
  # The two ends of the crossfade were four seconds apart in the film, so this
  # samples *after* both: what it prices is the floor the surface returns to.
  # **Paused first.** A run that is sounding repaints on every position event
  # — that is playback's cost, not the transition's — and measuring it would
  # price the wrong thing. Space is the play/pause key in every place.
  key space
  BPID=$(pgrep -n -f "^$WHICH\$" || true)
  if [[ -n $BPID ]]; then
    TICKS=$(getconf CLK_TCK)
    sleep 2
    # **Five samples, not one.** The absolute figure belongs to `llvmpipe` on a
    # display with no vsync and is nobody's desktop; what is being asked is
    # whether the two builds differ at rest, and one sample of a noisy quantity
    # cannot answer that.
    SAMPLES=""
    for _ in 1 2 3 4 5; do
      read -r _ _ _ _ _ _ _ _ _ _ _ _ _ U0 S0 _ < /proc/$BPID/stat
      sleep 2
      read -r _ _ _ _ _ _ _ _ _ _ _ _ _ U1 S1 _ < /proc/$BPID/stat
      SAMPLES="$SAMPLES $(python3 -c "print(f'{(($U1-$U0)+($S1-$S0))/$TICKS/2*100:.1f}')")"
    done
    echo "# at rest, paused, after the transition ($build): five 2 s samples," \
         "% of one core:$SAMPLES"
  fi

  # Per build, so the two sets of frames both survive the run and either can
  # be re-measured without re-shooting.
  rm -rf "$FILM/$build"; mkdir -p "$FILM/$build"
  ffmpeg -loglevel error -i "$S/film.nut" "$FILM/$build/%04d.png"
  rm -f "$S/film.nut"
  python3 "$OUT/measure.py" "$FILM/$build" "$OUT" "$build" | tee "$OUT/measured-$build.txt"

  kill $APID 2>/dev/null; wait $APID 2>/dev/null
  reap "$WHICH"
  echo
  echo "--- the isolation receipt (docs/DEVELOPMENT.md), $build ---"
  grep -m2 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
  sleep 1
done

echo "done — $OUT"
