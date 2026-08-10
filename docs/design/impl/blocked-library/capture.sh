#!/usr/bin/env bash
# Render the downgrade — a real binary against a real library whose
# `user_version` has been bumped — before and after ADR-0041.
#
# The defect, in the owner's words (2026-08-10): *"it shows me 'where's your
# music' which has no browse function and it also tells me the schema version
# is version 8 if I pick any directory"*. The missing `Browse…` was that stale
# build predating ADR-0025 and is not in scope; the **failure mode** is. baz
# refused a library written by a newer build — correctly — and reported it by
# drawing the first-run screen, which asks a listener whose collection is
# exactly where they left it where their music is.
#
# What this captures, headless on a private Xvfb with all six XDG redirections
# from docs/DEVELOPMENT.md (nothing touches the owner's session; the run's
# `[mpris] no session bus` line is the receipt, and this script prints it):
#
#   01  **before** — launched with the folders it has always had: the first-run
#       screen, on a library that is right there.
#   02  **before** — a *genuine* first run, from a config naming no folder.
#       There is no error on this screen, which is what makes 04 evidence.
#   03  **before** — the fixture typed into its well.
#   04  **before** — submitted: the version message arrives. This is the "it
#       tells me the schema version if I pick any directory" half, and because
#       03 was clean, the message *appearing* proves the folder was taken and
#       refused rather than ignored.
#   05  **after**  — the statement: what happened, what is safe, what to do.
#   06  **after**  — the same at 1920 × 1080; a block centred in a window has
#       to survive being given more window.
#   07  **after**  — the second door opened: what starting over would cost, and
#       the two words that answer it. The first press *reveals*.
#   08  **after**  — `Keep it`, which lands back on 05 to the pixel.
#   09  **after**  — `Set aside and start over` taken: the wall, over the same
#       folders, with the old library renamed rather than deleted.
#   10  **after**  — an unreadable index: the same surface, different words,
#       and one control the downgrade does not get.
#
# …and the **receipts**, which are the point of the whole exercise:
#
#   - the SHA-256 of `library.db` after each build has been run against it,
#     unchanged in both cases — *"your music and your playlists are untouched"*
#     as a measurement rather than a sentence;
#   - the directory listing after 09, showing `library.db.set-aside-1` beside
#     the new index, nothing deleted;
#   - `[mpris] no session bus` from every run.
#
# Build **both** binaries inside the toolbox (a host-built release binary links
# a newer glibc than the container has and dies before it draws) and give them
# **different filenames** — a filename collision has measured the wrong build
# on this project before:
#
#   git worktree add --detach /tmp/baz-blocked-before <the base commit>
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=$PWD/target/tb bash -lc \
#     'cd /tmp/baz-blocked-before && cargo build --release -p baz --features device-output'
#   cp target/tb/release/baz /tmp/baz-bin-before
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=$PWD/target/tb \
#     cargo build --release -p baz --features device-output
#   cp target/tb/release/baz /tmp/baz-bin-after
#   toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-blocked-fixture
#   toolbox run -c baz-dev env FIX=/tmp/baz-blocked-fixture \
#     BEFORE=/tmp/baz-bin-before AFTER=/tmp/baz-bin-after \
#     docs/design/impl/blocked-library/capture.sh
#
# **Never point any of this at ~/Music or at ~/.local/share/baz/library.db.**
# This capture is *about* databases; it builds its own from a silent fixture.
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BEFORE=${BEFORE:-/tmp/baz-bin-before}
AFTER=${AFTER:-/tmp/baz-bin-after}
FIX=${FIX:-/tmp/baz-blocked-fixture}
OUT=${OUT:-$REPO/docs/design/impl/blocked-library}
DISP=${DISP:-:197}
S=${S:-/tmp/baz-blocked-scratch}
# The version a "newer baz" stamped on the library. Two ahead of this build's,
# so the frame cannot be mistaken for an off-by-one.
FUTURE=${FUTURE:-10}

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

DB="$S/data/baz/library.db"
APID=""
XPID=""
# **Reap by PID, never by name**, and only ever the PIDs this script started.
# The owner's own instance is running and playing music.
cleanup() {
  [[ -n $APID ]] && kill "$APID" 2>/dev/null
  [[ -n $XPID ]] && kill "$XPID" 2>/dev/null
  return 0
}
trap 'cleanup; exit 130' INT TERM
trap cleanup EXIT

write_config() { # optional: "empty" for a config that names no folder
  mkdir -p "$S/config/baz"
  if [[ ${1:-} == empty ]]; then
    printf 'music_dirs = []\ngroup_key = "artist"\ndensity = "balanced"\nsidebar_open = true\n' \
      > "$S/config/baz/config.toml"
    return
  fi
  cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "artist"
density = "balanced"
sidebar_open = true
EOF
}

launch() { # binary W H tag
  : > "$S/$4.log"
  env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
      WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
      XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
      XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
      "$1" >> "$S/$4.log" 2>&1 &
  APID=$!
  WID=""
  for _ in $(seq 1 40); do
    WID=$(DISPLAY=$DISP timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
    [[ -n $WID ]] && break
    sleep 0.25
  done
  if [[ -z $WID ]]; then echo "NO WINDOW ($4)"; tail -30 "$S/$4.log"; exit 1; fi
  export DISPLAY=$DISP
  xdotool windowmove "$WID" 0 0
  xdotool windowsize "$WID" "$2" "$3"
  xdotool windowfocus --sync "$WID"
  sleep 4
}

stop() { kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null; APID=""; sleep 0.6; }

# Park the pointer where it states nothing and, above all, **off every
# control** — a frame shot with a word under the pointer photographs a hover
# wash, and a frame shot mid-press photographs a press. Both have happened on
# this project.
park() { xdotool mousemove 20 20; sleep 0.8; }

shot() { park; sleep 0.4; magick import -window root -crop "${W}x${H}+0+0" +repage "$OUT/$1.png"; echo "  shot $1"; }

# The SHA-256 of the library and of its sidecars, as one string — the receipt
# that a refused open wrote nothing.
fingerprint() {
  { sha256sum "$DB" 2>/dev/null
    sha256sum "$DB-wal" 2>/dev/null
    sha256sum "$DB-shm" 2>/dev/null
  } | awk '{print $1}' | tr '\n' ' '
}

# **The block's ink box**, as `w h x y`. Every measurement below is taken off
# the frame that is on screen at the time, never from a constant: a hard-coded
# y has photographed the wrong page on this project before.
ink_box() { # tag-of-a-frame-just-shot
  local bg
  bg=$(magick "$OUT/$1.png" -format '%[pixel:p{2,2}]' info:)
  magick "$OUT/$1.png" -bordercolor "$bg" -border 1 -fuzz 8% -trim -format "%w %h %X %Y" info: \
    | tr -d '+'
}

# **The first-run screen's folder well, clicked into.**
#
# The well is the third element of a block whose two above it are one line of
# `baz` and one line of hero type that does not wrap at this measure, so its
# centre sits a fixed 95 px below the block's ink top — on the screen carrying
# an error line and on the one that does not, which is exactly the pair this
# capture needs. With the top read off the frame, the same offset lands on
# both; a constant `y` landed on neither.
click_the_well() { # tag-of-a-frame-just-shot
  local width height left top
  read -r width height left top <<< "$(ink_box "$1")"
  local y=$(( top + 95 ))
  echo "  block ${width}x${height}@${left},${top} — the well at $(( W / 2 )),${y}"
  xdotool mousemove $(( W / 2 )) "$y"; sleep 0.3
  xdotool click 1; sleep 0.6
}

# **A word of the blocked screen's control row, found rather than guessed.**
#
# That screen is one centred column of type whose *last* element is the row of
# words, so the control band is the bottom `TRANSPORT_HIT` (32 px) of the
# block's ink. Trimming the band on its own then gives the words' own left edge
# and width, which is what makes `last` reliable: the labels differ by reason
# and by step, so a press placed from a string length would be a press placed
# from a guess.
press_word() { # first|last tag-of-a-frame-just-shot
  local which=$1 tag=$2 bg box width height left top bx bw
  read -r width height left top <<< "$(ink_box "$tag")"
  bg=$(magick "$OUT/$tag.png" -format '%[pixel:p{2,2}]' info:)
  local band_y=$(( top + height - 32 ))
  box=$(magick "$OUT/$tag.png" -crop "${width}x32+${left}+${band_y}" +repage \
        -bordercolor "$bg" -border 1 -fuzz 8% -trim -format "%w %X" info: | tr -d '+')
  read -r bw bx <<< "$box"
  local x y=$(( band_y + 16 ))
  if [[ $which == first ]]; then x=$(( left + bx + 12 )); else x=$(( left + bx + bw - 12 )); fi
  echo "  block ${width}x${height}@${left},${top} · words ${bw}px from +${bx} — $which at ${x},${y}"
  xdotool mousemove "$x" "$y"; sleep 0.4
  xdotool click 1; sleep 1.2
}

# **How many pixels two frames differ in** — the figure in parentheses.
#
# Not the leading one, and this is worth stating because getting it wrong cost
# a run: `magick compare -metric AE` prints its metric with `%g`, so it arrives
# as `1.86447e+08 (2845)` and a shell integer comparison on the first field
# reads *one*. Every assertion below then fired backwards, which is the worse
# of the two failures — it throws away good evidence rather than accepting bad.
#
# The parenthesised figure is the absolute pixel count on the toolbox's
# ImageMagick, printed below for the record. A build with a different quantum
# depth prints a normalised fraction there instead, which would make both gates
# trivially true — so if these ever stop firing, check that number first.
difference() { # a b
  magick compare -metric AE "$OUT/$1.png" "$OUT/$2.png" null: 2>&1 \
    | sed -n 's/.*(\([0-9.e+-]*\)).*/\1/p'
}

# Assert two frames differ by more than renderer noise, so "the press landed"
# is measured and not assumed. A frame that changed by a handful of antialiased
# pixels is a frame that did not change, and photographing one as evidence of a
# state transition is exactly how a false frame gets made.
changed() { # a b
  local d
  d=$(difference "$1" "$2")
  echo "  $1 vs $2: ${d:-?} pixels differ"
  awk -v d="${d:-0}" 'BEGIN { exit !(d > 500) }' || {
    echo "  !! nothing meaningful changed — the frame is not what it claims"; exit 1; }
}

# …and its opposite: two frames that must be the same picture. The way back out
# of a two-step has to *land* where it started, or it is not a way back. wgpu
# is run-to-run deterministic here (docs/DEVELOPMENT.md), so this is strict.
same() { # a b
  local d
  d=$(difference "$1" "$2")
  echo "  $1 vs $2: ${d:-?} pixels differ"
  awk -v d="${d:-999999}" 'BEGIN { exit !(d < 50) }' || {
    echo "  !! the way back did not land where it started"; exit 1; }
}

magick --version | head -1

Xvfb "$DISP" -screen 0 1920x1080x24 -nolisten tcp &
XPID=$!
sleep 1

# --------------------------------------------------------------------------
# A real library, made by baz, then stamped with a version from the future.
# --------------------------------------------------------------------------
W=1280; H=860
write_config
echo "== seeding a real library from $FIX =="
launch "$AFTER" $W $H seed
sleep 8            # let the scan land and the rows commit
stop
python3 - "$DB" "$FUTURE" <<'PY'
import sqlite3, sys
db, future = sys.argv[1], int(sys.argv[2])
con = sqlite3.connect(db)
was = con.execute("pragma user_version").fetchone()[0]
rows = con.execute("select count(*) from tracks").fetchone()[0]
con.execute(f"pragma user_version = {future}")
con.execute("pragma wal_checkpoint(truncate)")
con.commit(); con.close()
print(f"  {rows} tracks, user_version {was} -> {future}")
PY
rm -f "$DB-wal" "$DB-shm"
STAMP0=$(fingerprint)
echo "  library fingerprint: $STAMP0"

# --------------------------------------------------------------------------
# 01–04 — before: the first-run screen, and the loop it puts a listener in.
# --------------------------------------------------------------------------
echo "== before =="
launch "$BEFORE" $W $H before
shot 01-before-where-is-your-music
stop

write_config empty
launch "$BEFORE" $W $H before-typed
shot 02-before-a-genuine-first-run
click_the_well 02-before-a-genuine-first-run
# No `ctrl+a` first: the field is empty (the scratch HOME has no ~/Music to
# suggest), and a stray latched modifier from an earlier `key` swallowed the
# whole typed path the first time this ran — one pixel of difference between
# two frames that were supposed to show a path being typed.
xdotool type --clearmodifiers --delay 30 "$FIX"; sleep 0.8
shot 03-before-a-folder-typed
changed 02-before-a-genuine-first-run 03-before-a-folder-typed
# **The well has to be clicked into again**: `magick import -window root` takes
# the X keyboard focus, so a key pressed after a screenshot goes to the root
# window. A `Return` sent that way submitted nothing and produced two
# byte-identical frames.
click_the_well 03-before-a-folder-typed
xdotool key --clearmodifiers Return; sleep 3
shot 04-before-any-directory-says-the-version
changed 03-before-a-folder-typed 04-before-any-directory-says-the-version
stop
write_config
STAMP1=$(fingerprint)

# --------------------------------------------------------------------------
# 05, 07, 08, 09 — after: the statement, its second door, and the way out.
# --------------------------------------------------------------------------
echo "== after =="
launch "$AFTER" $W $H after
shot 05-after-a-newer-baz
press_word first 05-after-a-newer-baz            # `Set this library aside…`
shot 07-after-what-starting-over-costs
changed 05-after-a-newer-baz 07-after-what-starting-over-costs
# **The way back**, which is the half a two-step is worthless without: `Keep
# it` is the last word of the pair, and pressing it must land back on 05
# exactly.
press_word last 07-after-what-starting-over-costs
shot 08-after-keeping-it
same 05-after-a-newer-baz 08-after-keeping-it
# …and then the act itself, which is two presses and never one.
press_word first 08-after-keeping-it                 # `Set this library aside…`
press_word first 07-after-what-starting-over-costs   # `Set aside and start over`
sleep 4
shot 09-after-set-aside-the-wall-opens
changed 05-after-a-newer-baz 09-after-set-aside-the-wall-opens
stop
echo "--- what the set-aside did to the directory ---"
ls -1 "$S/data/baz/"
grep -m2 '^\[library\]' "$S/after.log" || echo "  (no [library] line)"

# The set-aside library, put back, so what follows starts from the same place —
# and so the round trip is exercised the way the screen promises it.
rm -f "$DB" "$DB-wal" "$DB-shm"
mv "$S/data/baz/library.db.set-aside-1" "$DB"
STAMP2=$(fingerprint)

# 06 — the same statement with more window under it.
W=1920; H=1080
launch "$AFTER" $W $H after1920
shot 06-after-a-newer-baz-1920
stop
W=1280; H=860

# --------------------------------------------------------------------------
# 10 — the sibling: an index that is there and cannot be read.
# --------------------------------------------------------------------------
echo "== after: an unreadable index =="
mv "$DB" "$S/library.db.newer"
head -c 65536 /dev/urandom > "$DB"
launch "$AFTER" $W $H unreadable
shot 10-after-an-unreadable-index
changed 05-after-a-newer-baz 10-after-an-unreadable-index
stop

kill "$XPID" 2>/dev/null; XPID=""

echo
echo "--- the isolation receipt (docs/DEVELOPMENT.md) ---"
grep -m1 '^\[mpris\]' "$S"/*.log || echo "NO MPRIS LINE — check the logs"
echo
echo "--- the library, before and after each build was run against it ---"
echo "  stamped from the future : $STAMP0"
echo "  after the BEFORE binary : $STAMP1"
echo "  after the AFTER binary  : $STAMP2   (set aside and moved back)"
[[ "$STAMP0" == "$STAMP1" ]] && echo "  OK  the old build did not write a byte" \
                             || echo "  !!  the old build changed the database"
[[ "$STAMP0" == "$STAMP2" ]] && echo "  OK  the new build did not write a byte, and set-aside is lossless" \
                             || echo "  !!  the new build changed the database"
