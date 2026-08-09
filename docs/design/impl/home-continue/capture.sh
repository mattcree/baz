#!/usr/bin/env bash
# Render Home's `CONTINUE` band appearing and disappearing — headless, on a
# private Xvfb, with all six XDG redirections from docs/DEVELOPMENT.md.
# Nothing touches the owner's session; the run's `[mpris] no session bus` line
# is the receipt that it did not, and this script prints it.
#
# **The whole visual story is one rule** (ADR-0030's third amendment):
#
#   > The band stands whenever there is a run to carry on with and nothing is
#   > sounding. Start anything, anywhere in the product, and it is gone; stop,
#   > and it is back, describing where you now are.
#
# So the frames are that rule walked, against the real binary:
#
#   1. **The band, at launch**, on the run the last baz was interrupted in the
#      middle of — the only state in which `session.toml` is read at all.
#   2. **`Resume` takes you to `Now playing`** in the same press that starts
#      the run. The one play gesture in the product that navigates.
#   3. **Home while it is sounding**: no band. The body starts at
#      `RECENTLY ADDED` and there is nothing above it.
#   4. **Home after a pause**: the band is back, at the position the engine
#      stopped at rather than the snapshot's.
#   5. **A record put on from the wall's own hover options** — playback started
#      by a route that has never heard of the snapshot — takes the band away
#      exactly as `Resume` did.
#   6. **And the band that comes back describes _that_ record**, not the one
#      `session.toml` names. This is the frame that proves the content follows
#      the engine: compare its placard with frame 1's.
#
# Plus two measurements: a diff of 1 against 3 (what the band's absence does to
# the page), and the placard's own strip out of 1 and 6 side by side (what the
# band's *content* does when the run changes under it).
#
# **The run ending** — the third state, where there is no band because the run
# is over rather than because something is sounding — is not filmed: the
# fixture's shortest track is 97 s and a queue has to be played to its end for
# it. It is covered by `a_run_that_finished_is_not_a_run_to_carry_on_with` and
# `a_run_played_to_its_end_is_written_away`, on screen and on disk.
#
# Build the binary **inside the toolbox** (a host-built release binary links a
# newer glibc than the container has and dies before it draws):
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz --features device-output
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-continue-fix
#   toolbox run -c baz-dev env FIX=/tmp/baz-continue-fix \
#     docs/design/impl/home-continue/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-continue-fix}
OUT=${OUT:-$REPO/docs/design/impl/home-continue}
DISP=${DISP:-:197}
S=/tmp/baz-continue-scratch

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
# Two independent guarantees that nothing is audible: the sink discards every
# sample, and the fixture's samples are all zero (docs/DEVELOPMENT.md).
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

launch() { # W H
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$BIN" >> "$S/app.log" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 40); do
    WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW"; cat "$S/app.log"; kill "$APID" "$XPID" 2>/dev/null; exit 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$1" "$2"
  xdotool windowfocus --sync "$WID"
  sleep 5   # let the launch scan land
}

stop()  { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; sleep 0.6; }
# Cropped to the window: the Xvfb screen is 1920×1080 for every run so the
# window can be resized without restarting X, and a frame with a field of
# black around it measures nothing.
shot()  { sleep 0.9; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
# Park the pointer where it states nothing: the lane's own empty middle, below
# the last row and above the marks. A pointer left on a tile summons the wall's
# hover options, which is a different frame's subject.
park()  { xdotool mousemove 40 $((H - 200)); sleep 0.6; }
key()   { xdotool key "$@"; sleep 0.6; }
click() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.2; }
hover() { xdotool mousemove "$1" "$2"; sleep 0.8; }

W=1280; H=860
Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

# The lane's head, in its post-`c0ef601` order: **the well leads**, then the
# three destinations. GAP_XL 24 in from the top, SIDEBAR_WELL_H 52 for the
# well, GAP_SM 8, then SIDEBAR_DEST_H 40 per row.
LANE_X=90
HOME_Y=104
LIB_Y=144
NOW_Y=184
# The wall's first tile, and the `Play` in the hover options it summons.
TILE1_X=440; TILE1_Y=250; PLAY1_X=370; PLAY1_Y=160
# A different record, so the band has something new to describe.
TILE2_X=724; TILE2_Y=250; PLAY2_X=654; PLAY2_Y=160
# `Resume` on the placard: TRANSPORT_HIT 32 tall, under the needle.
RESUME_X=520; RESUME_Y=180

# ------------------------------------------------------ the interrupted run
# **Written by playing, which is the only way it is written now.** The guard
# on `session.toml` is stated as *has anything sounded*, so a launch that
# presses nothing leaves the file exactly as it found it — including on the
# way out. This first run therefore has to actually put a record on.
launch $W $H
hover $TILE1_X $TILE1_Y
click $PLAY1_X $PLAY1_Y
sleep 4
# **The exit writes the elapsed position** (ADR-0023 §6), and only a real close
# request does it — `Message::Quit` is the one exit path. `windowclose` sends
# WM_DELETE_WINDOW, which is what a title bar's × sends.
xdotool windowclose "$WID"
sleep 3
stop
# **The elapsed figure is seeded here, and that is a limitation of the harness
# rather than of the feature.** Under Xvfb with no window manager,
# `windowclose` races winit's own X11 teardown and the process dies in
# `GetGeometry` before the update loop sees the close request, so the position
# on disk is the track boundary's 0. The exit path is one function
# (`App::leave_for_good`) reached by both exit routes and covered by tests;
# what cannot be shown headlessly is the compositor delivering the request. So
# the position is written in, to render a needle that is partway rather than a
# needle at zero.
if [[ -f "$S/config/baz/session.toml" ]]; then
  sed -i 's/^position_ms = .*/position_ms = 192000/' "$S/config/baz/session.toml"
fi
echo "  --- session.toml the second launch will read ---"
sed -n '1,6p' "$S/config/baz/session.toml" 2>/dev/null || echo "  (none written)"

# ------------------------------------------------------------- the rule walked
launch $W $H

# 1. The band, on the run baz was interrupted in the middle of.
click $LANE_X $HOME_Y
park
shot 01-home-with-a-run-to-carry-on-with

# 2. `Resume` starts the run **and** takes you to `Now playing` — one press,
#    and the place it lands on is populated rather than reading "Nothing
#    playing." for the frames before the engine confirms.
click $RESUME_X $RESUME_Y
sleep 2
park
shot 02-resume-takes-you-to-now-playing

# 3. Home, while it is sounding: no band, and the page starts at
#    `RECENTLY ADDED`.
click $LANE_X $HOME_Y
park
shot 03-home-while-it-is-sounding

# 4. Stop the player, and it is back — at the engine's position, not the
#    snapshot's. Space is the bar's own toggle (`keys.rs`).
key space
sleep 1
park
shot 04-home-after-a-pause

# 5. A record put on from **the wall's own hover options** — a route that has
#    never heard of `session.toml` — takes the band away exactly the same way.
key space
sleep 1
click $LANE_X $LIB_Y
hover $TILE2_X $TILE2_Y
click $PLAY2_X $PLAY2_Y
sleep 3
click $LANE_X $HOME_Y
park
shot 05-home-after-playing-something-else

# 6. …and the band that comes back describes **that** record. Compare the
#    placard with frame 1's: same geometry, different run.
key space
sleep 1
park
shot 06-the-band-follows-the-engine-not-the-snapshot

# The Now playing place with nothing sounding, for the pair: the band is on
# Home and this place says so plainly, which is the division of labour the
# amendment rests on.
click $LANE_X $NOW_Y
park
shot 07-now-playing-paused
stop

kill "$XPID" 2>/dev/null

# ------------------------------------------------------------------ measured
cd "$OUT" || exit 1
# What the band's absence does to the page: everything below it moves up by the
# band's own height, and nothing else changes.
magick 01-home-with-a-run-to-carry-on-with.png 03-home-while-it-is-sounding.png \
  -compose difference -composite -auto-level \
  08-diff-band-present-vs-gone.png
# The placard's own strip out of the two states it can describe, stacked: the
# launch snapshot's run above, the paused run's below. Same geometry, and the
# only difference is which record it is about.
magick 01-home-with-a-run-to-carry-on-with.png -crop 620x180+310+40 +repage /tmp/c-a.png
magick 06-the-band-follows-the-engine-not-the-snapshot.png -crop 620x180+310+40 +repage /tmp/c-b.png
magick /tmp/c-a.png /tmp/c-b.png -append 09-the-placard-in-both-runs.png
echo "  measured 08, 09"
echo
echo "--- what the band's absence does to the page ---"
"$OUT/measure.py"

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m3 '^\[mpris\]' "$S/app.log" || echo "NO MPRIS LINE — check the log"
echo
echo "--- what the shell said about the run ---"
grep -m6 '^\[session\]' "$S/app.log" || echo "  (the session module only speaks on failure)"
