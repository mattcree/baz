# Doc 09 §13 steps 5–7 — queue parity, `Play all`, shift-click

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

Three of design doc 09 §13's steps, shipped together and accepted into the
ADR-0023/-0024 amendment blocks:

- **Step 5 — queue-place edit parity** (09 §8.2): the queue's rows carry the
  playlist page's whole reserved edit set — ↑↓ steppers, ✕, and the transfer
  `+` (the sounding row's included) — every edit a whole-list `UpdateQueue`
  through the pure `queue_edit` ops, so the music keeps playing and the
  cursor follows its track. The queue place and the playlist page are the
  same editor.
- **Step 6 — `Play all`** (09 §7.1, S6): one word leading the Library
  strip's acts; one press reifies the wall — every visible record, whole,
  in the arrangement's order — into the queue and plays from the top. The
  place is **virtualized** (`queue_window`, §7.1's named gate), so the
  206-track run here and a 40 000-track run cost the frame the same.
- **Step 7 — shift-click** (ADR-0023 §3's accelerator): shift held, the
  press that would open a record's page appends it to the run instead —
  nothing sounds unasked, and the record joins the tail as its own headed
  group.

Playback is **paused** (Space) right after the `Play all` press so the
cursor holds still for the stills — the null sink otherwise consumes silent
tracks in seconds (the songs-search captures' known racing, `[play-all] 25
records · 206 tracks` in the log either way).

| | |
|---|---|
| [`01-strip-play-all-1280x860`](01-strip-play-all-1280x860.png) | the wall at rest: `Play all · Shuffle · Pull` — the wall's three acts, one cluster after the group keys |
| [`02-play-all-queue-1280x860`](02-play-all-queue-1280x860.png) | one press: the queue place opens on `1 of 206 · 17:56:25 left`, the first track marked, records grouped under their own names |
| [`03-queue-row-parity-1280x860`](03-queue-row-parity-1280x860.png) | a hovered row offers ↑ ↓ ✕ + in the playlist page's reserved slots; the durations do not move |
| [`04-queue-reordered-1280x860`](04-queue-reordered-1280x860.png) | ↓ pressed on row 2: *Marginalia 2* and *Sixth Street 3* swap, the numbers renumber, the run is undisturbed (`1 of 206` still; the lamp holds row 1) |
| [`05-transfer-picker-1280x860`](05-transfer-picker-1280x860.png) | the row's `+`: the panel opens as the picker — *Add “Marginalia 2” — pick a destination* — the Queue row first, its counts live |
| [`06-wall-after-shift-click-1280x860`](06-wall-after-shift-click-1280x860.png) | shift-click on *Violet Ledger*: the bar's `Queue` count reads 215 and the continuation line `then 25 albums` — appended, not played; *Undertow 1* still paused at 1:39 |
| [`07-queue-tail-appended-1280x860`](07-queue-tail-appended-1280x860.png) | the place scrolled to its end: the appended *Violet Ledger* is the tail's own headed group (rows 207–215) — and rows 199+ rendering exactly is the virtual window's spacer arithmetic holding at depth |
| [`08-strip-play-all-1920x1080`](08-strip-play-all-1920x1080.png) | the strip at 1920 |
| [`09-play-all-queue-1920x1080`](09-play-all-queue-1920x1080.png) | the reified wall at 1920, the list at its 880 px measure |
| [`10-queue-row-parity-1920x1080`](10-queue-row-parity-1920x1080.png) | the parity slots at 1920 |

## One defect these captures caught

The design docs write the steppers as ▲▼, and the playlist page shipped them
as U+25B4/U+25BE literals — but IBM Plex Sans (the product's one face)
carries **no triangle glyphs at any code point**, so they rasterised as tofu
boxes. No earlier capture had ever hovered a row to see it. Both surfaces
now use ↑/↓ (U+2191/U+2193, in the face); the first run of this script is
what caught it.

## Reproduce

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-qp-fix
toolbox run -c baz-dev docs/design/impl/queue-parity/capture.sh
```
