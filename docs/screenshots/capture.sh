#!/usr/bin/env bash
# The pictures on baz's store page and in its README, taken from the real
# binary.
#
# `packaging/flatpak/io.github.mattcree.baz.metainfo.xml` points Flathub at two
# of the four PNGs this writes beside it, and `README.md` shows all four. They
# are the first thing anyone sees, and a store page showing a baz nobody can
# install is worse than no page — so this is a script and not a one-off, and it
# is re-run when the interface changes.
#
# **It has been re-run because the interface changed.** The app bar (ADR-0040)
# put a 41 px band above everything, and the two frames committed before it
# were taken by a script whose click coordinates predated it — the `Play` press
# landed on the tile instead of the overlay and photographed a wall captioned
# `Nothing playing`. Every coordinate below was re-derived from a frame rather
# than from arithmetic, which is the only way this file has ever been correct.
#
#   toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
#     cargo build --release -p baz
#   toolbox run -c baz-dev docs/screenshots/capture.sh
#
# # Headless, and isolated six ways
#
# Xvfb, so nothing appears on the owner's desktop; all six XDG redirections
# from docs/DEVELOPMENT.md §"Headless UI verification", so nothing reaches his
# library, his settings or his session bus. The run's `[mpris] no session bus`
# line is the receipt and this script prints it. Nothing is audible twice over:
# the scratch `HOME` routes ALSA's default PCM to `null`, and every sample in
# the fixture is a zero. `BAZ_DEVICE_TESTS` stays unset.
#
# Every GUI binary includes device output and the transport at the bottom of
# the window. There is no silent screenshot-only build.
#
# # Why the fixture, and what is done to it
#
# The owner's own collection is not photographed for a public store page, and
# it is read-only to agents besides. `docs/design/composition/tools/mkfixture.sh`
# already builds a wall of silent FLACs with drawn sleeves, plausible bands and
# real years — but it is a *test* fixture, and three of its choices are visible
# as test choices:
#
#   - album 4 is titled "A Rather Considerably Overlong Album Title That Will
#     Clip", which is exactly what a layout harness needs and exactly what a
#     store page must not show;
#   - every track title carries its own index ("Nightwatch 12"), which reads as
#     generated the moment a track list is on screen;
#   - the four sleeves of its `type` family are drawn with the *first two
#     letters* of the album title — "We", "Hy", "Ma", "Ze" — which is a fine
#     stand-in for a typographic cover in a layout test and unmistakably a
#     generated image on a wall someone is being asked to install.
#
# So this retags rather than forks: one generator, and a pass over it that
# leaves nothing in frame reading as placeholder. Everything else — the bands,
# the years, the genres, the other five sleeve families — is the fixture's own.
#
# # The route is a listener's route
#
# Every state below is reached by clicking what a listener would click: a
# record's own `Play`, raised by resting on its tile, and then the lane's own
# rows. **The playlist is built by hand in the running app** — four records
# added through each tile's own `Add to…`, into a list named in the panel's own
# field — because a playlist dropped into the folder before launch would be a
# picture of a file rather than of the feature. No state is arranged from the
# outside: nothing is deep-linked and nothing is set in the config that a press
# could not reach. A capture that arrives at its frame by a route nobody takes
# has produced a false picture on this project four times.
#
# # The order is playlist first, then play
#
# Deliberate, and it is about the clock: the fixture's records run about 50
# minutes, and every frame after the music starts has to be taken before it
# ends or the bottom bar reads `Nothing playing` in a picture whose whole point
# is that something is. Building the list costs a dozen presses and needs no
# playback, so it happens first and the four photographed states then follow
# one press apart.
set -uo pipefail

REPO=${REPO:-$(git rev-parse --show-toplevel)}
BIN=${BIN:-$REPO/target/tb/release/baz}
FIX=${FIX:-/tmp/baz-shot-fix}
OUT=${OUT:-$REPO/docs/screenshots}
DISP=${DISP:-:198}
S=${S:-/tmp/baz-shot-scratch}
# 16:9 at a size a store page shows without shrinking, and comfortably over
# the 768 px `display_length` the metainfo requires.
W=1600; H=900

mkdir -p "$OUT"

# ------------------------------------------------------------------ fixture
# Rebuilt unless $FIX already holds one — regenerating costs a minute and the
# sleeves are deterministic, so a re-run of this script re-photographs the same
# wall rather than a differently-coloured one.
if [[ ! -d $FIX ]]; then
  "$REPO/docs/design/composition/tools/mkfixture.sh" "$FIX"

  # The clipping title, retitled. Same band, same year, same sleeve.
  for f in "$FIX"/04\ *?/*.flac; do
    metaflac --remove-tag=ALBUM --set-tag="ALBUM=Nightjar" "$f"
  done
  mv "$FIX"/04\ -\ * "$FIX/04 - Marguerite Vance-Lindqvist - Nightjar"

  # Track titles without their index. Forty of them, strided by the track
  # number, so no album repeats a title inside itself.
  python3 - "$FIX" <<'RETAG'
import pathlib, subprocess, sys

TITLES = [
    "Slow Return", "Field Recording", "Anhydrous", "Nightwatch",
    "The Long Lie Down", "Cassette Weather", "Pilot Light", "Undertow",
    "Marginalia", "Sixth Street", "Blue Hour", "Ledger", "Attic Tape",
    "Ferrous", "Quiet Part Loud", "Low Tide", "Signal Fire", "Winter Count",
    "Halfway House", "Lantern Hours", "Bright Ash", "Nine Bells",
    "Saltmarsh", "Coastal Path", "Every Little Light", "Thaw",
    "The Quiet Wing", "Bellrock", "Paper Anniversary", "Northern Line",
    "Dust and Copper", "A Better Room", "Standing Water", "Fathom",
    "Green Room", "Storm Glass", "The Turning Year", "Hollow Way",
    "Small Hours", "Driftwood",
]

root = pathlib.Path(sys.argv[1])
for i, album in enumerate(sorted(p for p in root.iterdir() if p.is_dir())):
    for track in sorted(album.glob("*.flac")):
        # `NN Title.flac` — the leading number is the track number the
        # fixture already tagged, and it is what orders this loop.
        n = int(track.name.split(" ", 1)[0])
        title = TITLES[(i * 11 + n) % len(TITLES)]
        subprocess.run(
            ["metaflac", "--remove-tag=TITLE", f"--set-tag=TITLE={title}",
             str(track)],
            check=True,
        )
        track.rename(album / f"{n:02d} {title}.flac")
RETAG

  # The four typographic sleeves, redrawn with the album's whole name instead
  # of its first two letters — which is what a typographic sleeve is. The
  # ground colour is sampled from the sleeve being replaced, so the wall keeps
  # the hue spread the fixture chose. Album 6, 12, 18 and 24 are mkfixture's
  # `type` family; the guard makes a fixture that renumbers itself fail loudly
  # rather than quietly ship two-letter covers again.
  for i in 06 12 18 24; do
    dir=$(echo "$FIX/$i - "*)
    [[ -d $dir ]] || { echo "no album $i in the fixture — has mkfixture.sh changed?"; exit 1; }
    title=$(metaflac --show-tag=ALBUM "$dir"/01*.flac | head -1 | cut -d= -f2-)
    bg=$(magick "$dir/cover.jpg[1x1+0+0]" -format '%[pixel:p{0,0}]' info:)
    magick -size 440x440 -background "$bg" -fill '#e6e3da' -gravity center \
      caption:"$(echo "$title" | tr '[:lower:]' '[:upper:]')" \
      -background "$bg" -extent 600x600 "$dir/cover.jpg"
  done
fi

# ------------------------------------------------------------------ scratch
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF
mkdir -p "$S/config/baz"
# **Both of these were chosen by photographing the alternatives**, and both are
# about one thing: the picture has to contain a *collection*, because that is
# what baz is for.
#
# `compact` rather than the default `balanced`, because a 1600-wide balanced
# wall is two columns of very large sleeves — a page with four records on it.
#
# **It was `dense` until ADR-0028's second amendment**, which retuned that step
# from 176 … 240 down to 160 … 200 on the owner's own *"the dense should be a
# bit smaller"*. At 1600 px that takes the tile to 165 px, and 165 px is
# narrower than `Marguerite Vance-Lindqvist · 1984` — so the fixture's longest
# caption photographed as `Marguerite Vance-Lindqvist ·`, an artist's name cut
# mid-line with a separator dangling off the end of it. `compact` is one step
# looser, hangs the same three shelves with a fourth beginning, and every
# caption on the wall is whole. Nothing about the ladder is wrong here; the
# store frame simply moved down a rung when the rung moved under it.
#
# `year` rather than the default `artist`, because the wall breaks a row at
# every group boundary and this fixture has two or three records per band: by
# artist it photographs as a column of pairs with two thirds of the window
# empty, which is a picture of a fixture and not of a library. By decade the
# groups are four to six deep, the rows fill, and `1970s / 1980s / 1990s` down
# the page is a shelf anyone recognises. `added` fills the wall too and was
# rejected: every record in a fresh fixture arrives at once, so the whole wall
# sits under one heading reading `THIS EVENING`.
cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "year"
density = "compact"
sidebar_open = true
EOF

# ------------------------------------------------------------------ display
Xvfb "$DISP" -screen 0 ${W}x${H}x24 -nolisten tcp >/dev/null 2>&1 &
XPID=$!
sleep 2
export DISPLAY=$DISP
# Both the app and the display, on every exit path — an interrupted run must
# leave nothing behind. Anchored on the full path, never a bare name: a bare
# `baz` also matches the owner's own running copy.
cleanup() {
  [[ -n ${APID:-} ]] && kill "$APID" 2>/dev/null
  local pid
  pid=$(pgrep -x -f "$BIN" || true)
  [[ -n $pid ]] && kill $pid 2>/dev/null
  kill "$XPID" 2>/dev/null
  return 0
}
trap cleanup EXIT INT TERM

env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$S/home" XDG_DATA_HOME="$S/data" \
    XDG_CONFIG_HOME="$S/config" XDG_CACHE_HOME="$S/cache" \
    XDG_RUNTIME_DIR="$S/run" BAZ_ROOM=closing-time \
    "$BIN" >> "$S/app.log" 2>&1 &
APID=$!

WID=""
for _ in $(seq 1 80); do
  WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
if [[ -z $WID ]]; then echo "NO WINDOW"; tail -30 "$S/app.log"; exit 1; fi
xdotool windowmove "$WID" 0 0
xdotool windowsize "$WID" "$W" "$H"
xdotool windowfocus --sync "$WID"
# The launch scan, and the sleeve decode after it. Twenty seconds rather than
# eight: on a loaded machine a shorter wait photographs a half-built wall, and
# `Play all` pressed before the scan lands queues a different library.
sleep 20

shot()  { sleep 1.0; magick import -window root -crop "${W}x${H}+0+0" +repage \
            "$OUT/$1.png"; echo "  shot $1"; }
click() { xdotool mousemove "$1" "$2"; sleep 0.4; xdotool click 1; sleep 1.6; }
rest()  { xdotool mousemove "$1" "$2"; sleep 1.5; }
# Dead ground in the lane, below its rows: the wall's tiles and the run's rows
# both reveal controls under the pointer, and a picture of the composition must
# not be a picture of the pointer.
park()  { xdotool mousemove 140 620; sleep 0.8; }

# ------------------------------------------------------------- the geometry
# Every number below was read off a frame of *this* build at 1600 × 900 with
# `compact` and the app bar. They are constants of a photograph, not of the
# layout — the app bar's 41 px moved all of them once already.
#
#   app bar          y 0…41
#   lane rows        Home 81 · Library 133 · Playlists 185 · Now playing 237
#                    — SIDEBAR_DEST_H 48 on a SIDEBAR_ROW_GAP 4 seam, from the
#                    app bar's 49. These were 85/125/165/205 until 2026-08-15,
#                    which is the pitch the lane had *before* its rows became
#                    their own sleeves; the stale numbers photographed Home
#                    twice and called one of them `playlist`.
#   wall shelf 1     tiles y 156…355, `Seagrass` x 313…513, `Werkbund` x 548…748
#   wall shelf 2     tiles y 479…678, `Violet Ledger` x 313…513,
#                    `Meadowgrass` x 783…983
#   tile overlay     `Play` +24, `Queue` +74, `Add to…` +124, `Open` +174 from
#                    the tile's top, inked ~48 px in from its left
#   playlists panel  `New playlist` y 244 with `Save` at x 1546; in pick mode a
#                    named list sits at y 296 with its `Add` at x 1556
ADD_TO_1=280; ADD_TO_2=603; PLAY_1=180

# ------------------------------------------------------------ the playlist
# Four records into one list, each through its own tile's `Add to…`. The first
# press opens the panel with nowhere to put anything, which is the honest empty
# state and also the door: `New playlist` takes a name and the list exists.
rest 413 255;   click 361 $ADD_TO_1     # Seagrass
click 1400 244                          # New playlist
xdotool type --clearmodifiers --delay 60 "Sunday Morning"
sleep 1.0
click 1546 244                          # Save — the list is a .m3u8 from here on

for tile in "648 255 596 $ADD_TO_1" "883 578 831 $ADD_TO_2" "413 578 361 $ADD_TO_2"; do
  set -- $tile
  rest "$1" "$2"; click "$3" "$4"       # Werkbund, Meadowgrass, Violet Ledger
  click 1556 296                        # `Add`, on the row the list now has
done
xdotool key --clearmodifiers Escape     # the panel closes; the lane keeps the list
sleep 1.2

# --------------------------------------------------------------- the frames
# Put a record on. Resting on a tile raises its own four choices — `Play`,
# `Queue`, `Add to…`, `Open` — and `Play` is the first of them, at the top of
# the overlay: point at the record, press play. That is the sentence the store
# page makes ("click one and it plays front to back"), performed rather than
# asserted, and it is why all four frames below are of a player that is playing.
rest 648 255
click 596 $PLAY_1

# 1 · the wall, with that record playing: its tile lit, the band in the lane's
#     `RECENT`, and the bottom bar a live readout rather than `Nothing playing`.
# Deliberate playback now lands on Now Playing only after TrackStarted; return
# through the resident Library row before photographing the wall.
click 105 133
park
shot library

# 2 · Now playing, by the lane's own row — the record and the rest of the run,
#     side by side, which is the whole of that place.
click 105 237
park
shot now-playing

# 3 · Home: `All songs` with its collage sleeve, what arrived recently, and the
#     size of the collection. The one place that is about the library rather
#     than about a record.
click 105 81
park
shot home

# 4 · the playlists place: the list built above beside the built-in
#     `Favourites` and the ghost tile a new one starts from, grouped A–Z the
#     way the library groups records.
#
# It was the playlist's *page*, reached by clicking whichever row the lane
# happened to have at y 350 — a coordinate that depended on what had been
# touched in what order, and that photographed Home when the lane's pitch
# changed. The place is reached from the lane's own destination now, which is
# a fixed row, and it is the better picture besides: one frame with the
# collection, the built-in and the way in.
click 105 185
park
shot playlist

echo "== isolation receipt"
grep -m2 '^\[mpris\]' "$S/app.log" || echo "  NO MPRIS LINE — check $S/app.log"
echo "done — $OUT"
