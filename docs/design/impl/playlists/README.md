# ADR-0024 §4–§6 — the playlist surfaces

Every frame here is the **real binary**, rendered headless by
[`capture.sh`](capture.sh) with all six XDG redirections from
`docs/DEVELOPMENT.md`. The run's receipt that it did not touch the owner's
desktop — one line per launch, plus the proof the playlists folder was the
scratch one:

```
[startup] room: Closing Time
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
[playlists] folder: /tmp/baz-playlists-scratch/data/baz/playlists
```

Silent fixtures (`composition/tools/mkfixture.sh` writes zeros) and an
`.asoundrc` routing ALSA's default PCM to `null` — two independent guarantees
that nothing was audible; `BAZ_DEVICE_TESTS` stays unset. The script cleans up
**only the pids it started**, never by name. The playlists themselves are
seeded the way a migrating listener would seed them: two `.m3u8` files written
into the scratch data directory before launch, one of them carrying a dead
path on purpose.

## The no-reflow assertion

ADR-0024 §5's panel floats by ADR-0016's verified mechanics (`stack` +
`opaque`, no scrim, wheel passing through), so **opening it may not re-lay the
wall by a pixel**. `capture.sh` asserts it rather than promising it: the
before/after pair at each window size is diffed with the panel's own region
(340 px + its 1 px seam, full height) blanked on both frames, and the
remainder must be pixel-identical.

```
no-reflow @1280x860:  AE=0 outside the panel's region
no-reflow @1920x1080: AE=0 outside the panel's region
```

## The frames

| | |
|---|---|
| [`01-wall-before`](01-wall-before.png) | the wall at rest — the diff's "before" |
| [`02-wall-panel-open`](02-wall-panel-open.png) | `Ctrl+P` (or the strip's `Playlists` door): the panel over the wall's right edge — `New playlist`, one row per list with its counts, the receive `+` per row. The wall behind is pixel-identical |
| [`03-panel-armed`](03-panel-armed.png) | the receive target pressed: the row carries the surface step and hairline (never the accent), its mark flips to `−`, and **every wall label gains the quiet `+`** — while it stands, a tile press pulls the record in, one press per addition (§6 layer 2) |
| [`04-album-page-with-panel`](04-album-page-with-panel.png) | the record's page beside the open panel: `Add to playlist` under `Play album`, quiet, no accent (§6 layer 1) |
| [`05-playlist-page`](05-playlist-page.png) | the playlist's page by the panel row's name: hero name, counts, `Play` / `Queue` / `Rename` / `Delete`, record-group headers over consecutive same-record runs |
| [`06-playlist-page-missing`](06-playlist-page-missing.png) | the seeded broken list: `4 of 5 · 1 missing · 22:21`, the dead entry dimmed from its stem with its path on the row, still in the file (§3) |
| [`07-playlist-playing`](07-playlist-playing.png) | its first row pressed: the playable subset queued, the lamp dot in the number column — the queue is exactly this list, so the page may mark it |
| [`08-queue-save-control`](08-queue-save-control.png) | the queue place holding exactly what `Play` sent — four rows, grouped under their records' names — with `Save as playlist` beside the summary |
| [`09-queue-save-field`](09-queue-save-field.png) | the save word become a name field (the roots field's anatomy; the storage layer's refusals land under it in its own words) |
| [`10`–`11`](10-wall-before-1920.png) | the before/after pair again at 1920 × 1080 |

## Reproduce

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-playlists-fix
toolbox run -c baz-dev docs/design/impl/playlists/capture.sh
```

## Two readings worth their gloss

- **The elapsed times race in the stills.** The scratch `.asoundrc` routes
  ALSA's default PCM to `null`, which consumes samples as fast as they are
  written, so a six-minute silent track plays out in seconds. That is the
  isolation working, not a transport bug; nothing here measures time.
- **At 1280 with the panel open, the duration lane of a page's rows sits
  under the panel.** The panel floats — that is the whole deal; nothing may
  reflow — so a page whose list runs under the panel's lane is occluded
  there, exactly as the dead popover occluded the wall behind it. The track
  rows' reserved `+` slot shares that lane, so at narrow widths the
  track-level add is made through its hover route before the panel opens
  (press `+`, then pick — the panel opens as the picker on top), and the
  record-level routes are unaffected. At 1920 the centred measure clears the
  panel and every slot is beside it.
