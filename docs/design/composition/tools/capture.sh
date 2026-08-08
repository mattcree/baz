#!/usr/bin/env bash
# Render every baz surface headless, on a private Xvfb, with all six XDG
# redirections from docs/DEVELOPMENT.md. Nothing touches the owner's session.
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
# The binary is whatever `BIN` names, defaulting to the host build. Run this
# from inside the toolbox and point `BIN` at a toolbox-built binary: the host's
# glibc is newer than the container's, so a host `target/release/baz` dies in
# the container with a `GLIBC_… not found` link error and the script sits in
# its wait-for-a-window loop until it is killed. (It did.)
BIN=${BIN:-$REPO/target/release/baz}
FIX=/tmp/baz-comp-fixture
OUT=${OUT:-/tmp/baz-comp-shots}
W=${W:-1280}
H=${H:-860}
DISP=${DISP:-:171}
SCEN=${SCEN:-A}
S=/tmp/baz-comp-scratch-$SCEN-$W

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
# Two independent guarantees of silence: zero samples, and a null sink.
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
XPID=$!
sleep 1

MUSIC=$FIX
[[ $SCEN == C ]] && MUSIC=""
[[ $SCEN == D ]] && { mkdir -p "$S/emptymusic"; MUSIC="$S/emptymusic"; }

# shellcheck disable=SC2086
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
    "$BIN" $MUSIC > "$S/app.log" 2>&1 &
APID=$!

# Wait for the window, then focus it (no WM on this display sets focus).
WID=""
for _ in $(seq 1 80); do
  WID=$(DISPLAY=$DISP xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW"; cat "$S/app.log"; kill $APID $XPID 2>/dev/null; exit 1; fi
# The app can also *die* before the loop notices — a link error, a missing
# fixture — in which case the loop above finds no window and we have already
# reported it. Belt and braces: if the process is gone, say so rather than
# shooting an empty root window for the rest of the scenario.
kill -0 $APID 2>/dev/null || { echo "APP EXITED"; cat "$S/app.log"; kill $XPID 2>/dev/null; exit 1; }
export DISPLAY=$DISP
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 3   # let the scan land

shot() { sleep 0.7; magick import -window root "$OUT/$1-${W}x${H}.png"; echo "  shot $1"; }
mv()   { xdotool mousemove "$1" "$2"; sleep 0.4; }
klick(){ xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.6; }
key()  { xdotool key "$@"; sleep 0.5; }
typ()  { xdotool type --delay 40 "$1"; sleep 0.8; }

PARK_X=$((W - 6)); PARK_Y=$((H / 2))

case $SCEN in
A)
  mv $PARK_X $PARK_Y;                       shot wall-rest
  mv 180 260;                               shot wall-hover
  xdotool click --repeat 6 5; sleep 1.0     # wheel down = scroll the wall
  mv $PARK_X $PARK_Y;                       shot wall-scrolled
  xdotool click --repeat 12 4; sleep 1.0    # back to the top
  klick 180 260;                            shot inspector
  key ctrl+b;  mv $PARK_X $PARK_Y;          shot wall-selected
  key ctrl+b; sleep 0.4
  # **The queue key is Ctrl+U now** (ADR-0017 step 11). It was bare `q`, and it
  # needed a blur-click before it because the well took focus at launch — so `q`
  # went into the *field* and every "queue" frame in the set was a search result
  # with a one-letter query. Both halves of that are gone: type-anywhere put
  # every letter on the query and the layer keys on a modifier, and nothing
  # takes focus at launch any more. A modifier chord reaches the application
  # even when the well *does* have focus, because iced's `text_input` ignores
  # everything under Ctrl but its own clipboard four.
  key ctrl+u; mv $PARK_X $PARK_Y;           shot queue-empty
  key Escape
  key ctrl+comma; mv $PARK_X $PARK_Y;       shot settings
  key ctrl+comma; sleep 0.5
  key slash; typ "zzzzz";                   shot search-no-match
  key Escape; key Escape
  ;;
B)
  # "Closing Time", whose first track is one hour of digital silence — long
  # enough that the null sink cannot burn through it while the frames are taken.
  #
  # **Found by name, not by pixel.** It used to be two hardcoded coordinates per
  # window size, which is a fixture of the layout in a script whose whole job is
  # to measure the layout: the moment the hang, the rail lane or the group keys
  # moved, the click landed on wall and every "playing" frame in the set was
  # silently an idle one. Searching narrows the wall to one work, which is
  # always the first cell of the first shelf.
  # No `/` first, and none is needed: bare letters *are* the query now
  # (ADR-0017 step 11), so the first keystroke both filters and takes the caret.
  # Under the old launch-focus behaviour a leading `/` was typed *into* the
  # field and the query became `/Closing Time`, which matches nothing — and
  # every B frame in the set was silently an idle bar.
  typ "Closing Time"; sleep 0.6
  xdotool mousemove $((40 + 60)) $((H / 3)); sleep 0.3
  xdotool click --repeat 2 --delay 120 1; sleep 1.2
  key Escape; sleep 0.6
  key ctrl+b
  mv $PARK_X $PARK_Y;                       shot wall-playing
  # The groove hangs one `BAR_LEAD` below the bar's centre line and the fader
  # sits on it, so the two hover targets are no longer the same row.
  mv $((W / 2)) $((H - 24));                shot bar-seek-hover
  mv $((W - 80)) $((H - 52));               shot bar-volume-hover
  klick $((W / 2 + 150)) 24
  key q; mv $PARK_X $PARK_Y;                shot queue-playing
  key Escape; sleep 0.4
  key ctrl+b;                               shot inspector-playing
  key ctrl+b; sleep 0.4
  key space; mv $PARK_X $PARK_Y;            shot wall-paused
  ;;
C)
  mv $PARK_X $PARK_Y;                       shot first-run
  ;;
D)
  mv $PARK_X $PARK_Y;                       shot empty-library
  ;;
E)
  # **Density as zoom** (ADR-0017 step 6). Every frame is reached by the real
  # gesture rather than by a seeded config, so the frames prove the binding as
  # well as the geometry. Balanced is where a fresh config starts.
  mv $PARK_X $PARK_Y;                       shot density-balanced
  key ctrl+minus;  mv $PARK_X $PARK_Y;      shot density-dense
  # Two presses back up: Dense -> Balanced -> Spacious. The ladder saturates,
  # so a third would be a no-op and is spent below as the proof of that.
  key ctrl+equal; key ctrl+equal
  mv $PARK_X $PARK_Y;                       shot density-spacious
  key ctrl+equal; mv $PARK_X $PARK_Y;       shot density-spacious-again
  # And the pointer half of the same gesture: Ctrl+wheel-down, from Spacious.
  xdotool keydown ctrl; xdotool click --repeat 2 5; xdotool keyup ctrl; sleep 1.0
  mv $PARK_X $PARK_Y;                       shot density-wheeled
  ;;
F)
  # **Type anywhere.** No `/`, no click, nothing focused: the letters go
  # straight onto the wall from a cold start.
  typ "co"; sleep 0.6
  mv $PARK_X $PARK_Y;                       shot find-typed
  # **The modifier layer reaches the application through a focused well.**
  # iced's `text_input` ignores everything under Ctrl but its own clipboard
  # four, so this must re-hang the wall and must *not* type a `-` into the
  # query. If it ever does, the frame will say `co-` in the well.
  key ctrl+minus
  mv $PARK_X $PARK_Y;                       shot find-typed-then-zoomed
  key ctrl+equal
  # Esc, Esc: the field blurs, then the query clears and the well is left
  # blurred, so Space is the transport again on the next press.
  key Escape; key Escape
  mv $PARK_X $PARK_Y;                       shot find-cleared
  ;;
esac

grep -c "no session bus" "$S/app.log" > /dev/null && echo "RECEIPT OK: $(grep -m1 mpris "$S/app.log")"
kill $APID 2>/dev/null; sleep 0.6; kill -9 $APID 2>/dev/null
kill $XPID 2>/dev/null; sleep 0.3; kill -9 $XPID 2>/dev/null
wait 2>/dev/null
echo "done $SCEN $W"
