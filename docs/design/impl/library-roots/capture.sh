#!/usr/bin/env bash
# Render the Settings place's Library section, headless, on a private Xvfb, with
# all six XDG redirections from docs/DEVELOPMENT.md. Nothing touches the owner's
# session; the run's `[mpris] no session bus` line is the receipt that it did
# not, and this script prints it.
#
# It exercises the whole of ADR-0022 against the real binary: two folders held
# at once, a third added by hand, one removed with its confirming press, and a
# force sync. The second folder is *deleted from disk* before the run so the
# unavailable state is the real one rather than a mock.
#
# Build the binary **inside the toolbox**: a release binary built on the host
# links a newer glibc than the container has, and the run dies before it draws.
#
#   toolbox run -c baz-dev cargo build --release -p baz
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-roots-a
#   toolbox run -c baz-dev docs/design/impl/library-roots/capture.sh
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=$REPO/target/release/baz
A=${A:-/tmp/baz-roots-a}
B=${B:-/tmp/baz-roots-b}
C=${C:-/tmp/baz-roots-c}
OUT=${OUT:-$REPO/docs/design/impl/library-roots}
W=${W:-1280}
H=${H:-860}
DISP=${DISP:-:181}
S=/tmp/baz-roots-scratch

mkdir -p "$OUT"
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF

# Two more folders carved out of the one fixture, so the run holds three roots
# that are genuinely distinct trees. The fixture is rebuilt first: this script
# *moves* albums out of it, so a second run over a split fixture would hold
# different folders than the first.
"$REPO/docs/design/composition/tools/mkfixture.sh" "$A" > /dev/null
rm -rf "$B" "$C"; mkdir -p "$B" "$C"
i=0
for album in "$A"/*/; do
  i=$((i + 1))
  case $((i % 3)) in
    1) dest=$B ;;
    2) dest=$C ;;
    *) continue ;;
  esac
  mv "$album" "$dest/"
done

# The config baz will find: two folders, written as the pre-ADR-0022 *single*
# key plus the new list, so the run also proves the reader prefers the list.
mkdir -p "$S/config/baz"
cat > "$S/config/baz/config.toml" <<EOF
music_dir = "$A"
music_dirs = [
    "$A",
    "$B",
]
group_key = "artist"
EOF

Xvfb "$DISP" -screen 0 "${W}x${H}x24" -nolisten tcp &
XPID=$!
sleep 1

env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
    "$BIN" > "$S/app.log" 2>&1 &
APID=$!

# `--sync` waits *forever* when nothing matches, so it is bounded here: a binary
# that cannot start (the host/toolbox glibc mismatch, most likely) must fail the
# script rather than hang it.
WID=""
for _ in $(seq 1 40); do
  WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW"; cat "$S/app.log"; kill $APID $XPID 2>/dev/null; exit 1; fi
export DISPLAY=$DISP
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
sleep 4   # let the launch scan land

shot() { sleep 0.9; magick import -window root "$OUT/$1.png"; echo "  shot $1"; }
klick(){ xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.9; }
key()  { xdotool key "$@"; sleep 0.6; }
typ()  { xdotool type --delay 30 "$1"; sleep 0.8; }

PARK_X=$((W - 6)); PARK_Y=$((H - 6))

# The wall, holding two folders' worth of records.
xdotool mousemove $PARK_X $PARK_Y;      shot 01-wall-two-folders

# Settings → Library. Ctrl+`,` for the first step, deliberately: the well no
# longer takes focus at launch (ADR-0017 §1.2's type-anywhere work removed it),
# so the chord reaches the subscription instead of being typed into the query —
# which it *was* before that landed. The second step is a pointer press because
# the spine is a pointer target and has no binding.
key ctrl+comma
klick 140 139                                   # the spine's `Library`
xdotool mousemove $PARK_X $PARK_Y;      shot 02-library-section

# Add a third folder by typing it, exactly as the first-run screen asks. Enter
# submits, like the first-run field it is the cousin of.
klick 550 267                                   # the add-a-folder well
typ "$C"
key Return
sleep 3
xdotool mousemove $PARK_X $PARK_Y;      shot 03-third-folder-added

# Now take the second folder off the disk entirely and force a sync: the folder
# goes unavailable, and the line under it says nothing was removed from it. Each
# folder block is 58 px tall, so the third one pushes Force sync down by that.
rm -rf "$B"
klick 852 432                                   # `Force sync`
sleep 5
xdotool mousemove $PARK_X $PARK_Y;      shot 04-folder-unavailable-after-force-sync

# Removing is two presses, and the second one names what goes.
klick 860 152                                   # `Remove` on the first folder
xdotool mousemove $PARK_X $PARK_Y;      shot 05-removal-armed
klick 828 152                                   # `Forget`
sleep 3
xdotool mousemove $PARK_X $PARK_Y;      shot 06-folder-forgotten

# Back to the wall: the forgotten folder's records went with it.
klick 70 24                                     # `‹ Library`
sleep 1
xdotool mousemove $PARK_X $PARK_Y;      shot 07-wall-after-forgetting

echo "--- receipts"
grep -m1 "mpris" "$S/app.log"
grep -m4 "\[scan\]" "$S/app.log"
grep -m4 "\[config\]\|\[index\]" "$S/app.log"
echo "--- config baz wrote"
cat "$S/config/baz/config.toml"
kill $APID 2>/dev/null; sleep 0.6; kill -9 $APID 2>/dev/null
kill $XPID 2>/dev/null; sleep 0.3; kill -9 $XPID 2>/dev/null
wait 2>/dev/null
echo "done"
