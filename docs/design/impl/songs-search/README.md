# Doc 09 §13 step 3 — the Songs section: search answers in songs

Every frame here is the **real binary**, rendered headless by
[`capture.sh`](capture.sh) with all six XDG redirections from
`docs/DEVELOPMENT.md`. The run's receipt that it did not touch the owner's
desktop — one line per launch:

```
[startup] room: Closing Time
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Silent fixtures (`composition/tools/mkfixture.sh` writes zeros) and an
`.asoundrc` routing ALSA's default PCM to `null` — two independent guarantees
that nothing was audible; `BAZ_DEVICE_TESTS` stays unset. The script cleans up
**only the pids it started**, never by name.

## What the frames show

Design doc 09 §5's decision, shipped as §13 step 3 and accepted into
ADR-0023's amendment block: under a non-empty query the Library place's body
becomes a ranked **Songs** section — up to eight track rows, ADR-0021's track
ranking surfaced instead of thrown away at the album fold — then an `Albums`
rule and the wall, filtered as today. Two sections, separate. A song row's
press (and <kbd>Enter</kbd>) is a **needle-drop**: the record queued whole,
selected edition, cursor on the song, through the record page's own
`play_track`/`play_from` path. The record's name in each row is a door to its
album page. The section sits on the wall's own block ruler — same left edge,
same centring, the shared `TRACK_NO_W`/`DURATION_W` lanes, rows at the
product's one control height.

| | |
|---|---|
| [`01-wall-before-1280x860`](01-wall-before-1280x860.png) | the wall at rest — no query, no section, the composition exactly as it was |
| [`02-songs-mixed-1280x860`](02-songs-mixed-1280x860.png) | `night` typed from the wall (type-anywhere unchanged: the first keystroke lands in the well and filters): eight ranked rows over the 11-album filtered wall, `SONGS` and `ALBUMS` rules on the wall's own edge |
| [`03-songs-enter-playing-1280x860`](03-songs-enter-playing-1280x860.png) | <kbd>Enter</kbd>: the top-ranked song — *Nightwatch 12* — needle-dropped; the lamp dot follows `TrackStarted` into the first row, its record *Ochre* is queued **whole** (`12` in the bar; the needle's segments are the record's real entries) and warms on the wall |
| [`04-songs-album-query-1280x860`](04-songs-album-query-1280x860.png) | an album-name query (`orbits`): ADR-0021's field ranking puts that record's own tracks in the section, opening track on top — the sound of <kbd>Enter</kbd> is unchanged from the album-level answer here, by ranking rather than by a special case |
| [`05-songs-narrow-1280x860`](05-songs-narrow-1280x860.png) | `nightwatch 9`: two matches are two rows — the section is the ranked head, never padded |
| [`06-songs-none-1280x860`](06-songs-none-1280x860.png) | no matching tracks: the section is **absent, not empty**, and the wall's own empty state stands alone |
| [`07-songs-mixed-1920x1080`](07-songs-mixed-1920x1080.png) | the mixed state at 1920: the section spans the wall's block, the duration lane on the block's right edge |
| [`08-songs-narrow-1920x1080`](08-songs-narrow-1920x1080.png) | the narrow state at 1920 |
| [`09-songs-none-1920x1080`](09-songs-none-1920x1080.png) | the empty state at 1920 |

## One reading worth its gloss

**The elapsed times race in the stills.** The scratch `.asoundrc` routes
ALSA's default PCM to `null`, which consumes samples as fast as they are
written, so a silent track plays out in seconds — by the later frames the
queue that <kbd>Enter</kbd> started has already ended (`Nothing playing`,
and the queue count `12` beside the bar's `Queue` door is the finished run's
size). That is the isolation working, not a transport bug; nothing here
measures time.

## Reproduce

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-songs-fix
toolbox run -c baz-dev docs/design/impl/songs-search/capture.sh
```
