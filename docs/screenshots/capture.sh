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
#   toolbox run -c baz-dev docs/screenshots/capture.sh           # real library
#   FIXTURE=1 toolbox run -c baz-dev docs/screenshots/capture.sh # safe fallback
#
# **The real library is what ships.** The owner asked twice for real music, and a
# generated wall reads as generated however good its sleeves are. It costs
# publishing his record titles, his artists and their cover art on a store
# page. `FIXTURE=1` is the explicit clean-room fallback for local testing or a
# future store policy that cannot spend that.
#
# # Headless, and isolated six ways
#
# Xvfb, so nothing appears on the owner's desktop; all six XDG redirections
# from docs/DEVELOPMENT.md §"Headless UI verification", so nothing reaches his
# library, his settings or his session bus. The run's `[mpris] no session bus`
# line is the receipt and this script prints it. Nothing is audible twice over:
# the scratch `HOME` routes ALSA's default PCM to `null`. The fixture samples
# are zero when `FIXTURE=1` is used; `BAZ_DEVICE_TESTS` stays unset in either
# mode.
#
# Every GUI binary includes device output and the transport at the bottom of
# the window. There is no silent screenshot-only build.
#
# # The fixture fallback, and what is done to it
#
# The default photographs the owner's real collection. `FIXTURE=1` instead
# uses `docs/design/composition/tools/mkfixture.sh`, which builds a wall of
# silent FLACs with drawn sleeves, plausible bands and real years — but it is a
# *test* fixture, and three of its choices are visible as test choices:
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

# ---------------------------------------------------------------- real mode
# **The default photographs the owner's own library instead of the fixture.**
#
# He asked for it twice — *"ideally the screenshots could use my library and/or
# look realistic at least"*, then *"it doesn't seem like you've used my real
# library or at least real music"* — and a fixture that reads as a fixture is
# what the second one is about. It publishes real record titles, real artists
# and real cover art on a store page; `FIXTURE=1` is deliberately explicit so
# a release refresh cannot quietly switch back to generic music.
#
# **Nothing is written to his data.** The library index, the analysis store,
# the art cache and the playlists are *copied* into this run's scratch
# `XDG_DATA_HOME`; the config is written into the scratch `XDG_CONFIG_HOME`
# with his own `music_dirs` read out of his. The copies also make the run fast
# and faithful at once: no scan, no re-analysis, and every sleeve already
# rendered.
if [[ -n ${FIXTURE:-} ]]; then
  REAL=""
else
  REAL=1
fi
if [[ -n $REAL ]]; then
  HIS_DATA=${HIS_DATA:-$HOME/.local/share/baz}
  HIS_CACHE=${HIS_CACHE:-$HOME/.cache/baz}
  HIS_CONFIG=${HIS_CONFIG:-$HOME/.config/baz/config.toml}
  for needed in "$HIS_DATA/library.db" "$HIS_CONFIG"; do
    [[ -e $needed ]] || { echo "REAL=1 but $needed is not there"; exit 1; }
  done
fi

# ------------------------------------------------------------------ fixture
# Rebuilt unless $FIX already holds one **from this generator** — regenerating
# costs a minute and the sleeves are deterministic, so a re-run re-photographs
# the same wall rather than a differently-coloured one.
#
# The stamp is the generator's own hash, and it is here because its absence
# cost a confused half hour: the titles and sleeves were made realistic in
# `mkfixture.sh`, the capture re-ran, and it photographed the old wall — a
# directory that exists satisfied a guard that meant to ask whether it was
# *current*.
STAMP=$(sha256sum "$REPO/docs/design/composition/tools/mkfixture.sh" | cut -c1-16)
if [[ -n $REAL ]]; then
  : # his own music is the fixture, and it is already on disk
elif [[ ! -d $FIX || $(cat "$FIX/.generator" 2>/dev/null) != "$STAMP" ]]; then
  rm -rf "$FIX"
  # **The one title that exists to be too long, made ordinary.** Album 4's
  # name is a clipping test the composition audit needs and a store page does
  # not; the generator takes the override, so the sleeve and the caption agree
  # — which retagging afterwards could never manage.
  #
  # Everything else this script used to do to the fixture after the fact —
  # track titles without their index, typographic sleeves carrying the whole
  # album name — is the generator's own work now. It was duplicated here for
  # as long as the generator did it badly.
  LONG_TITLE="Nightjar" "$REPO/docs/design/composition/tools/mkfixture.sh" "$FIX"

  echo "$STAMP" > "$FIX/.generator"


fi

# ------------------------------------------------------------------ scratch
rm -rf "$S"
mkdir -p "$S"/{home,data,config,cache,run}
chmod 700 "$S/run"
cat > "$S/home/.asoundrc" <<'EOF'
pcm.!default { type null }
ctl.!default { type null }
EOF
mkdir -p "$S/config/baz" "$S/data/baz" "$S/cache/baz"
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
if [[ -n $REAL ]]; then
  # **His library, copied in.** The index and the analysis store make the run
  # need no scan and no listening pass; the art cache makes every sleeve
  # render on the first frame instead of over the following minute; the
  # playlists give the playlists place something real to show. All copies —
  # this run writes only inside `$S`.
  cp "$HIS_DATA/library.db" "$S/data/baz/library.db"
  [[ -e $HIS_DATA/vibe.db ]] && cp "$HIS_DATA/vibe.db" "$S/data/baz/vibe.db"
  [[ -d $HIS_DATA/playlists ]] && cp -r "$HIS_DATA/playlists" "$S/data/baz/playlists"
  [[ -d $HIS_CACHE/art-v1 ]] && cp -r "$HIS_CACHE/art-v1" "$S/cache/baz/art-v1"
  # His own roots, and this script's own arrangement: `year` and `compact` are
  # chosen for what a *wall* photographs like (see above) and that reasoning
  # does not change because the records are real.
  {
    sed -n '/^music_dirs = \[/,/^\]/p' "$HIS_CONFIG"
    # **`alphabet`, where the fixture takes `year`.** The fixture was built
    # with a year on every record; a real collection has rips and bootlegs
    # that never carried one, and `year` opens the wall on a shelf headed
    # `NO YEAR`, which is honest and a poor first impression.
    echo 'group_key = "alphabet"'
    echo 'density = "compact"'
    echo 'sidebar_open = true'
  } > "$S/config/baz/config.toml"
else
cat > "$S/config/baz/config.toml" <<EOF
music_dirs = [
    "$FIX",
]
group_key = "year"
density = "compact"
sidebar_open = true
EOF
fi

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
# **Park off every tile, and move twice.** A tile's four choices are raised by
# the pointer, and a picture of the composition must not be a picture of the
# pointer. Two moves rather than one because a widget only re-reads the cursor
# when the cursor reports itself: a single jump out of a tile whose row has
# just reflowed can leave the veil standing.
#
# (1400, 430) is dead ground on **all four** frames, which is the only kind of
# park worth having: right of the wall's last column and left of the A–Z rail
# on the Library and Playlists places, between Home's `NEW PLAYLIST` block and
# its `RECENTLY ADDED` row, and open background on Now Playing. (1400, 780)
# looked safe and was not — it sits inside Home's recently-added row and
# raised that tile's four choices in the frame.
# **Somewhere with nothing under it.** Parking the pointer is how a frame is
# taken without a hover veil in it — so where it parks has to be empty, and
# (1400, 430) is empty over the fixture's four-tile rows and is *on a tile*
# over his five-tile ones, which is how the first real-mode frame came back
# with a veil open over Frank Zappa. The arrangement strip's right end has
# nothing under it on either wall.
PARK_X=1400; PARK_Y=430
[[ -n ${REAL:-} ]] && { PARK_X=1200; PARK_Y=78; }
park()  { xdotool mousemove $PARK_X $PARK_Y; sleep 0.5; xdotool mousemove $((PARK_X + 2)) $((PARK_Y + 2)); sleep 0.9; }

# ------------------------------------------------------------- the geometry
# Every number below was read off a frame of *this* build at 1600 × 900 with
# `compact` and the app bar. They are constants of a photograph, not of the
# layout — the app bar's 41 px moved all of them once already.
#
# **Re-derived frame by frame on 2026-08-15** (item 71). The previous set was
# stale in three independent ways at once and the run still *succeeded*: the
# playlist was never created, and the frame it was for photographed an empty
# place. Each number below is marked with where it comes from, because the two
# kinds rot differently — arithmetic follows a token, a photograph follows a
# layout.
#
#   app bar          y 0…49                                    [arithmetic]
#   lane rows        Home 81 · Library 133 · Playlists 185 ·    [arithmetic]
#                    Now playing 237 — SIDEBAR_DEST_H 48 on a
#                    SIDEBAR_ROW_GAP 4 seam, from the app bar's 49
#   wall shelf 1     tiles y 172…381, `Seagrass` x 265…475,     [photograph]
#                    `Werkbund` x 510…720
#   wall shelf 2     tiles y 505…714, `Violet Ledger`           [photograph]
#                    x 265…475, `Meadowgrass` x 755…965
#   tile overlay     `Play` +26, `Queue` +78, `Add to…` +130,   [photograph]
#                    `Open` +182 from the tile's top
#   new-playlist     name field y 186, `Save playlist` at       [photograph]
#   place            (315, 595) under a six-track draft
#   playlists panel  `New playlist` row y 252; once a list      [photograph]
#                    exists its row sits at y 303 with its
#                    `Add` at x 1556
ADD_TO_1=302; ADD_TO_2=635; PLAY_1=198; OPEN_1=354

# ------------------------------------------------------------ the playlist
# **Not in real mode**, where he already has playlists and they were copied in
# above. Building one here would mean four presses derived from a fixture's
# wall, on a wall that is not it.
if [[ -z $REAL ]]; then
# Four records into one list, each through its own tile's `Add to…`. The first
# press opens the panel with nowhere to put anything, which is the honest empty
# state and also the door: `New playlist` takes a name and the list exists.
# **The route changed under this script and nobody noticed**, which is the
# whole of item 71: the panel's `New playlist` no longer takes a name in the
# panel — it opens the canonical New playlist place with the record already in
# the draft, and the name and the Save live there. The old three lines typed
# `Sunday Morning` into the app-bar search (type-anywhere took the keystrokes,
# because no field had focus) and pressed a `Save` that was not there.
rest 370 270;   click 361 $ADD_TO_1     # Seagrass → its own `Add to…`
click 1400 252                          # `New playlist` — the draft place opens
click 900 186                           # the name field, which must be focused
xdotool type --clearmodifiers --delay 60 "Sunday Morning"
sleep 1.0
click 315 595                           # `Save playlist` — a .m3u8 from here on
click 105 133                           # back to the wall
sleep 1.0

for tile in "615 270 596 $ADD_TO_1" "860 600 831 $ADD_TO_2" "370 600 361 $ADD_TO_2"; do
  set -- $tile
  rest "$1" "$2"; click "$3" "$4"       # Werkbund, Meadowgrass, Violet Ledger
  click 1556 303                        # `Add`, on the row the list now has
done
xdotool key --clearmodifiers Escape     # the panel closes; the lane keeps the list
sleep 1.2

# **Say so if the list was not made.** The whole of item 71 was a run that
# succeeded while building nothing: every press missed, the frames were taken
# anyway, and the shipped picture of the playlists place was honest and empty.
# A capture that cannot tell those apart is not a verification.
fi
made="$S/data/baz/playlists/Sunday Morning.m3u8"
[[ -n $REAL ]] && made=$(ls -S "$S/data/baz/playlists/"*.m3u8 2>/dev/null | head -1)
if [[ ! -s $made ]]; then
  echo "THE PLAYLIST WAS NEVER MADE — the picture would be of an empty place."
  echo "Re-derive the coordinates above from a frame; see docs/WORK.md item 71."
  exit 1
fi
tracks=$(grep -cE '\.(flac|mp3|m4a|wav)$' "$made" || true)
echo "== playlist: $tracks tracks in $(basename "$made")"
# The fixture run builds a four-record list and must get one. Real mode shows
# a list the owner already made, and how long it is is his business.
if [[ -z $REAL ]]; then
  [[ $tracks -ge 20 ]] || { echo "only $tracks tracks — expected four records"; exit 1; }
fi

# --------------------------------------------------------------- the frames
# Put a record on. Resting on a tile raises its own four choices — `Play`,
# `Queue`, `Add to…`, `Open` — and `Play` is the first of them, at the top of
# the overlay: point at the record, press play. That is the sentence the store
# page makes ("click one and it plays front to back"), performed rather than
# asserted, and it is why all four frames below are of a player that is playing.
if [[ -n $REAL ]]; then
  # **Real mode's own coordinates**, read off `$S/probe-wall.png` — which this
  # writes on every run, into the scratch rather than beside the receipts,
  # because re-reading a frame is the only way this file has ever been
  # correct and none of those frames is a receipt. His wall hangs five tiles to a row rather than the
  # fixture's four, so the columns are elsewhere: 370, 615, 860, 1104, 1349.
  park; magick import -window root "$S/probe-wall.png"
  rest 370 270
  click 370 $PLAY_1
else
  rest 648 255
  click 596 $PLAY_1
fi

# 1 · the wall, with that record playing: its tile lit, the band in the lane's
#     `RECENT`, and the bottom bar a live readout rather than `Nothing playing`.
# Deliberate playback now lands on Now Playing only after TrackStarted; return
# through the resident Library row before photographing the wall.
click 105 133
park
shot library

# **Put the selection on a record before leaving the wall.** The list built
# above is selected from having just been made, and a selected playlist tile
# raises its own four options — right in the app, and a picture of the pointer
# on a store page. Selection is one content item app-wide (ADR-0017's
# select-then-activate), so selecting a record here puts the tile down. The
# **caption** is the safe press: the sleeve carries the hover veil's options,
# the caption carries only select and activate. Nothing after this frame shows
# the wall, so the record's own selection is in no picture.
click 1042 741

# 2 · Equaliser: a global player control, open over the wall it is shaping.
# It lives between the view marks and the notification bell at 1600 × 900.
# Move to dead ground without pressing it: an outside **press** correctly
# dismisses this floating panel, while a move clears the preceding tile hover.
park
click 1327 24
park
shot equalizer
click 1327 24

# 3 · Now playing, by the lane's own row — the record and the rest of the run,
#     side by side, which is the whole of that place.
click 105 237
# **Wait for the case to come round.** It turns once every 32 s
# (`jewel_case::TURN`) and starts a shade off square (`yaw` 0.18 rad), and the
# clock runs from the moment this place opens. Shot on arrival it is caught
# edge-on, which photographs as a black bar. Park and shoot cost ~2.4 s, so
# 28 s here puts the frame near the top of the second turn — the cover facing
# the reader, a few degrees open, which is what the caption promises.
sleep 28
park
shot now-playing

# 4 · Home: `All songs` with its collage sleeve, what arrived recently, and the
#     size of the collection. The one place that is about the library rather
#     than about a record.
click 105 81
park
shot home

# 5 · the playlists place: the list built above beside the built-in
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

# 6 · a record's own page — the sleeve at size, the track list beside it, and
#     the run in play order. Reached from the wall the way a listener reaches
#     it: the caption selects, and `Open` on the sleeve's own veil goes in.
click 105 133
park
if [[ -n $REAL ]]; then
  # The `#` shelf's single tile, which is unambiguous on his wall whatever
  # else is in it: hover it, and press `Open` on its veil.
  rest 377 290
  click 377 368
else
  rest 648 255                          # the veil over `Werkbund`
  click 596 $OPEN_1
fi
park
shot album

# 7 · search, which is app-wide and the fastest way to anything. Typed rather
#     than pasted: the count beside the field is live, and a paste would
#     photograph it mid-debounce.
click 200 24
# A word that finds something in the library being photographed. `hain` is
# `Studio Hain`, who exists only in the fixture.
if [[ -n $REAL ]]; then
  xdotool type --clearmodifiers --delay 55 "arvo"
else
  xdotool type --clearmodifiers --delay 55 "hain"
fi
sleep 1.8
park
shot search
xdotool key --clearmodifiers Escape
sleep 0.8

# ------------------------------------------------- 8 · the smart playlist
# **A second pass, against a different fixture, and it has to be.**
#
# Every sample in the wall's fixture is a zero — one of the two guarantees
# that this run is inaudible — and silence analyses to the same vector for
# every track. A smart playlist photographed against it would show a flat line
# and a list in no order, which is a picture of the feature not working. So
# the frame that shows it is taken against `mkfixture-varied.sh`: twenty-four
# tracks whose tempo and loudness genuinely walk, at −30 dBFS, still routed to
# a null PCM and still never played.
#
# Twenty-four is enough because this frame's subject is the line and the list
# it produced, and neither is a count. The library's size appears on the door
# behind it, which this frame is past.
kill "$APID" 2>/dev/null; wait "$APID" 2>/dev/null
APID=""
sleep 1

VARIED=${VARIED:-/tmp/baz-varied}
[[ -n $REAL ]] || [[ -d $VARIED ]] || "$REPO/docs/design/impl/contour/mkfixture-varied.sh" "$VARIED"
V=$S/vibe
rm -rf "$V"; mkdir -p "$V"/{home,data/baz,config/baz,cache/baz,run}; chmod 700 "$V/run"
cp "$S/home/.asoundrc" "$V/home/.asoundrc"
if [[ -n $REAL ]]; then
  # **His library, already listened to.** The analysis store is the expensive
  # half of this feature — an hour of CPU over five thousand tracks — and he
  # has already paid it, so copying it in is what makes a real-library frame
  # of this page possible at all inside a capture.
  cp "$S/config/baz/config.toml" "$V/config/baz/config.toml"
  cp "$HIS_DATA/library.db" "$V/data/baz/library.db"
  cp "$HIS_DATA/vibe.db" "$V/data/baz/vibe.db"
  [[ -d $HIS_CACHE/art-v1 ]] && cp -r "$HIS_CACHE/art-v1" "$V/cache/baz/art-v1"
else
  printf 'music_dirs = ["%s"]\ngroup_key = "alphabet"\n' "$VARIED" > "$V/config/baz/config.toml"
fi
env -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS DISPLAY="$DISP" \
    WINIT_UNIX_BACKEND=x11 HOME="$V/home" XDG_DATA_HOME="$V/data" \
    XDG_CONFIG_HOME="$V/config" XDG_CACHE_HOME="$V/cache" \
    XDG_RUNTIME_DIR="$V/run" BAZ_ROOM=closing-time BAZ_VIBE_WORKERS=4 \
    BAZ_VIBE_MODEL_DIR="$REPO/models/vibe" \
    "$BIN" >> "$V/app.log" 2>&1 &
APID=$!
WID=""
for _ in $(seq 1 80); do
  WID=$(timeout 3 xdotool search --sync --onlyvisible --class baz 2>/dev/null | head -1)
  [[ -n $WID ]] && break
  sleep 0.25
done
[[ -z $WID ]] && { echo "no window for the smart playlist pass"; tail -20 "$V/app.log"; exit 1; }
xdotool windowmove "$WID" 0 0; xdotool windowsize "$WID" $W $H
xdotool windowfocus --sync "$WID"
sleep 6

click 105 185                           # Playlists
sleep 1.5
click 764 316                           # `New smart playlist` — centre of its tile
sleep 3
if [[ -z $REAL ]]; then
  click 1120 626                        # `Listen to my music` — the one step first
  # Wait by watching the store stop growing, rather than by guessing.
  DB="$V/data/baz/vibe.db"
  last=-1; still=0
  for _ in $(seq 1 120); do
    sleep 5
    size=$(stat -c %s "$DB" 2>/dev/null || echo 0)
    if [[ "$size" == "$last" && "$size" != "0" ]]; then
      still=$((still + 1)); [[ $still -ge 3 ]] && break
    else still=0; fi
    last=$size
  done
  sleep 6
else
  # Already listened to — the door reads what it heard and offers the moods.
  sleep 4
fi
if [[ -n $REAL ]]; then
  # **A mood, not a blank request.** On his library the door carries what it
  # heard as well as the six moods, so `Your own words` is below the fold —
  # and a mood is the better frame anyway: it fills the request and composes
  # in one press, which is the feature doing its thing rather than waiting.
  # **Scroll to `Your own words`, and take the words-free route.** A mood
  # press composes and then composes again when its words settle, and the
  # second one arrives with a diff banner over the list. The door is taller
  # here than on a fixture — it carries what Baz heard as well as the moods —
  # so the seventh tile needs a scroll to reach.
  xdotool mousemove 600 600
  for _ in 1 2 3 4; do xdotool click 5; done
  sleep 1.5
  magick import -window root "$S/probe-door.png"
  click 359 700
  sleep 3
  click 315 586                         # a length — and this composes, once
  sleep 8
  # The door was scrolled to reach that tile and the page inherits the
  # offset, so the frame would open halfway down its own first control.
  xdotool mousemove 600 400
  for _ in 1 2 3 4 5 6; do xdotool click 4; done
  sleep 1.5
else
  click 359 786                         # `Your own words` — into the page itself
  sleep 3
fi
# **One composition, and only one**, because every one after the first carries
# a diff — *10 new · 9 kept* — which is a good line in the app and clutter in
# a store frame.
#
# That one is the **length press**, not `Compose`. Opening the door sets
# `open`, so by the time this page is reached any control that settles
# recomposes — which means choosing a length here *is* a composition, and a
# `Compose` press after it would be the second. Half an hour because the
# fixture is 48 tracks: an hour is reachable but leaves the list at the edge
# of what the diversity rules allow, and a shortfall note is the honest thing
# to say and the wrong thing to photograph.
if [[ -z $REAL ]]; then
  click 315 586                         # `half an hour` — and this composes
  sleep 7
fi
sleep 6
park
shot smart-playlist

echo "== isolation receipt"
grep -m2 '^\[mpris\]' "$S/app.log" || echo "  NO MPRIS LINE — check $S/app.log"
grep -m1 '^\[mpris\]' "$V/app.log" || echo "  NO MPRIS LINE — check $V/app.log"
echo "done — $OUT"
