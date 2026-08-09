# Doc 09 §13 step 8 — the reorder drag

Every frame here is the **real binary**, rendered headless by
[`capture.sh`](capture.sh) with all six XDG redirections from
`docs/DEVELOPMENT.md`. The run's receipt that it did not touch the owner's
desktop — one line per launch:

```
[startup] room: Closing Time
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Silent fixtures (`composition/tools/mkfixture.sh` writes zeros) and an
`.asoundrc` routing ALSA's default PCM to `null` — two independent
guarantees that nothing was audible; `BAZ_DEVICE_TESTS` stays unset. The
script cleans up **only the pids it started**, never by name. The drags are
real drags — `xdotool mousedown`, stepped `mousemove`s, `mouseup` — and the
mid-flight frames are taken **with the button down**, not staged.

## What the frames show

Doc 09 §13's last step, shipped whole: doc 11 P5's pointer-capture widget
(`crates/baz/src/drag.rs`), ADR-0024 §6 layer 3 — one hand-built wrapper on
the `groove.rs` precedent paying three surfaces at once. Press a row of
either list editor (09 §8.2's "same editor") past an 8 px threshold and the
row is in the hand: a quiet ghost card names it at the pointer, an
insertion line sits on the boundary the drop would commit to — measured by
the rows themselves against their own bounds, which is what keeps the index
exact under the queue place's virtual window. Release commits **one** edit:
a whole-list `UpdateQueue` (the music keeps playing; the cursor follows its
track by path — frame 04 shows the summary moving from `1 of 62` to
`3 of 62` because the *sounding* row was the one dragged), one atomic file
save on the playlist page, or — dropped on the standing panel's playlist
row — that file's append, the picker row's own act made direct. Esc
discards; the pointer leaving the window or the window losing focus commits
at the line (the groove's capture lessons, inherited and pinned by tests in
`drag.rs`).

**Sugar only.** The ▲▼ steppers, the ✕, the transfer `+`, the picker and
the context menus all remain exactly as shipped — frame 02 *makes* the drop
target through the picker route on purpose. The drag is pointer-only by
nature; the visible controls stay the accessible route the visible-control
rule requires.

| | |
|---|---|
| [`01-queue-at-rest`](01-queue-at-rest-1280x860.png) | `Play all`'s run in the queue place, paused — the editor the drag lands on, unchanged at rest |
| [`02-panel-standing`](02-panel-standing-1280x860.png) | the drop target made by the route the drag is sugar over: a row's `+` → `New playlist` → *Road Trip*, the panel standing |
| [`03-drag-midflight-line`](03-drag-midflight-line-1280x860.png) | the sounding row lifted and pulled down: the ghost card names it at the pointer, the insertion line sits between rows 3 and 4 |
| [`04-after-drop-reordered`](04-after-drop-reordered-1280x860.png) | the drop: one `UpdateQueue` — the row lands where the line said, the numbers renumber, the summary's cursor follows its track (`3 of 62`), nothing else moves |
| [`05-drag-to-panel-midflight`](05-drag-to-panel-midflight-1280x860.png) | a queue row carried over the panel: the ghost rides over it (flipped inside the window by the menu's own anchor), the *Road Trip* row under the pointer draws the room's hover statement |
| [`06-panel-after-add`](06-panel-after-add-1280x860.png) | the drop appends to the file: the row's counts read `2` — the run untouched |
| [`07-playlist-page`](07-playlist-page-1280x860.png) | the artefact's page, opened from the panel row it was dropped on |
| [`08-playlist-drag-midflight`](08-playlist-drag-midflight-1280x860.png) | the page's own drag: row 1 lifted past the end — the one `Bottom`-edge line, on the last row |
| [`09-playlist-after-drop`](09-playlist-after-drop-1280x860.png) | the drop: one atomic file save, the artefact reordered |

## What a still cannot show

Esc's discard (the ghost and line simply vanish; the list is untouched) and
the off-window commit (`CursorLeft`/`Unfocused` publish the same drop the
release does) are gesture-time behaviours with no distinct end-frame; they
are pinned as unit tests in `crates/baz/src/drag.rs` instead —
`a_drag_that_leaves_the_window_commits_there_and_drags_nothing_after` is
the groove's reported bug, re-pinned for this widget.

## One defect these captures caught

The first probe of the drag shipped the ghost as a stack layer that
*appeared at the lift* — and the gesture died the frame it began: iced
diffs the widget tree by position and tag, so the new stack level reshaped
the tree under every widget on screen and reset the drag source's own held
phase. The ghost layer is now stacked **always**, an empty pass-through at
rest, and the note in `app.rs` records the measurement so the conditional
form cannot quietly return.
