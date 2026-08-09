# Doc 11 §5, the adopt tier — P1, P2, P3, P4, P6, shipped

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

The Jobs-era critique's adopt tier (doc 11 §5), implemented: the first minute
gets see-and-point (P1), the product gains forgiveness — undo for list edits,
the trash for deletion (P2, ADR-0027) — the Album place pays the comparison
debt (P3, doc 07 §3.2), the shipping copy speaks one vocabulary (P4), and the
gesture layer is taught at the moment of relevance (P6).

| | |
|---|---|
| [`01-first-run`](01-first-run.png) | **P1**: one question, two doors — `Browse…` beside the typed well (ADR-0025 §1's shape, at the screen it was deferred from), the placeholder a human sentence, the footnote CLI-free (`baz DIR` moved to `--help`) |
| [`02-first-run-refusal`](02-first-run-refusal.png) | **P1**: a typed path that is not a directory, refused in place — the stat ran on the blocking pool (`check_folder`), not the UI thread, which retires ADR-0025 §3's other deferral ground |
| [`03-wall`](03-wall.png) | the wall against the fixture, for reference |
| [`04-songs-rule-note`](04-songs-rule-note.png) | **P6.4**: the Songs rule carries *"Enter plays the first match."* at its right edge — the accelerator taught beside the rows it accelerates |
| [`05-tile-menu-accelerator`](05-tile-menu-accelerator.png) | **P6.1**: the tile menu prints `Shift-click` beside `Queue album` — the era's accelerator column, on the one item with a gesture to print. A word, not P6's `⇧`: doc 10 §3.6 bans borrowed characters and the face draws U+21E7 as tofu (an earlier frame of this very set caught it) |
| [`06-album-prev-next`](06-album-prev-next.png) | **P3 + P4**: `‹ Prev` / `Next ›` in the Album place's header (Prev inert — this is the wall's first record), *"Esc returns to Library"* across the strip |
| [`07-album-stepped`](07-album-stepped.png) | **P3**: one press on `Next ›` — the neighbouring record along the wall's own arrangement, both doors now live |
| [`08-queue-undo-armed`](08-queue-undo-armed.png) | **P2**: a shift-click append doubled the run; the transient `Undo` stands beside the summary, exactly while there is an edit to take back |
| [`09-queue-undone`](09-queue-undone.png) | **P2**: Ctrl+Z — the run restored whole (9 rows again), the word gone with the spent history, and the music never stopped: the cursor walked on through the undo, because an undo restores the *list*, never the playback position |
| [`10-playlist-one-press-delete`](10-playlist-one-press-delete.png) | **P2**: the playlist page's acts row — `Queue · Rename · Delete`, one word each, no armed sentence anywhere on the surface |
| [`11-playlist-deleted-to-trash`](11-playlist-deleted-to-trash.png) | **P2**: one press on `Delete` — the page leaves for the Library, the panel reads *"None yet…"*, and the run's log holds the receipt: `"Road Trip" moved to the trash — the file; the music stays`, with `Road Trip.m3u8` listed in the scratch `$XDG_DATA_HOME/Trash/files/` |
| [`12-queue-empty-taught`](12-queue-empty-taught.png) | **P6.3**: *"When a queue ends, baz stops. Shuffle draws again; Play all plays the Library."* — the refusal stated with its answers, at the moment it is felt |
| [`13-shuffle-tooltip`](13-shuffle-tooltip.png) | **P6.2**: *"Play 8 records drawn from what the Library shows"* — the bound pinned to `shuffle::SLEEVES` by test |
| [`14-pull-tooltip`](14-pull-tooltip.png) | **P6.2**: *"Offer one record you haven't played in years — nothing plays until you say so"* — the poetic name explained before the first press, which is the era's licence for keeping it (P9 stays the owner's) |

## Two readings worth their gloss

**The elapsed times race in the stills.** The scratch `.asoundrc` routes
ALSA's default PCM to `null`, which consumes samples as fast as they are
written, so silent tracks play out in seconds. That is the isolation working,
not a transport bug; nothing here measures time.

**What the drop target looks like is deliberately absent.** winit 0.30
delivers file-drop events on X11 and not on Wayland (its Wayland backend has
no data-device handling), so P1's drop half ships as an unadvertised
accelerator: the hover line exists only in response to an event only X11
sends, and synthesizing a real X DnD handshake was not worth a staged frame.
The wiring is in `app.rs`'s subscription; the deferral of the full
cross-platform target is recorded in ADR-0025 §3's superseded clause, per
P1's adopt-modified text.

## Reproduce

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-jea-fix
toolbox run -c baz-dev docs/design/impl/jobs-era-adopt/capture.sh
```
