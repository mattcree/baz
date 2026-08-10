# What a resize costs baz on the CPU — 2026-08-10

The owner, after the day's grid-arithmetic build: *"resize is much better now
but somehow it just doesn't seem… smooth? maybe need some basic debounce on
layout"*.

A debounce is the right fix for one cause and the wrong fix for the others, so
this measures the half a debounce would address **before** anything is built:
how many messages a dragged edge delivers, what a view build costs at each
width, how often `Grid::new` and `Shelves::new` run, and whether any part of
the artwork path is on the resize path at all.

**The findings and the verdict live in `docs/BACKLOG.md`**, under *"Feels like
treacle when I resize" — measured, not reproduced*, so that the record of this
complaint stays in one place rather than two. This directory holds the harness
and the raw logs those numbers were read off.

## What is here

| file | what it is |
|---|---|
| `measure.sh` | the run: private Xvfb, six-variable XDG isolation, a driven resize sweep at ~30 Hz, a saturation sweep with no pause, and phase marks in the log |
| `mkfixture-large.sh` | a fixture larger than `docs/design/composition/tools/mkfixture.sh`'s 25 records — N records of silent FLAC with generated covers, because library size is one of the three things the 2026-08-09 investigation named as differing from the owner's machine |
| `lib25.log`, `lib400-artist.log`, `lib400-genre.log` | the raw runs: 25 records, and 400 records shelved two ways (120 shelves of three, and one shelf of 400 — the shape where widening the window reveals the most new records per step) |

`lib400-artist.log` was taken before the script learned to open the log with
`>>`, so it carries no `### ` phase marks: the app held the file at its own
offset and overwrote every mark the shell appended. Its numbers are the run
read whole, which is why the backlog quotes it without a per-phase breakdown.

## The probe

The per-second `[probe]` lines in those logs come from a **temporary**
instrumentation patch that is deliberately **not** in the tree: a wrapper
around `App::view` timing element-tree construction, counters in `Grid::new`
and `Shelves::new`, and three counters in `request_visible_thumbs` (calls,
range-guard hits, decodes actually spawned). It printed one line a second
behind `BAZ_PROBE=1`, in `BAZ_MSG_LOG`'s shape.

It was reverted after the numbers were taken, because a permanent meter is a
maintenance cost and this question is answered. To take it again: wrap `view`,
bump an atomic in each of those four functions, and print the tally on
`note_message`'s one-second cadence.

**What the probe does not cover, and the reason the conclusion is careful:** it
times the construction of the element tree, not iced's layout, text shaping or
draw, which run after `view` returns and which baz cannot defer from its own
update loop. The `draw-to-draw` figure in each line brackets those — it is the
whole cycle, software rasterisation included.

## Reproducing

Inside the toolbox, so the PIDs are the script's own — `kill` on a
`toolbox run` wrapper does not reach the process inside the container:

```sh
toolbox run -c baz-dev cargo build --release -p baz
toolbox run -c baz-dev docs/design/impl/resize-cost/mkfixture-large.sh /tmp/baz-fix-400 400 8
toolbox run -c baz-dev env BIN=$PWD/target/release/baz FIX=/tmp/baz-fix-400 \
  OUT=/tmp/baz-resize-logs LABEL=lib400 GROUP=genre SETTLE=45 \
  docs/design/impl/resize-cost/measure.sh
```

`[mpris] no session bus` in the log is the receipt that no real session was
touched. `amdgpu_device_initialize failed` is expected and is the whole
limitation: **Xvfb has no GPU**, so nothing here measures swapchain, vsync or
surface reconfiguration, and none of it may be read as evidence that the
owner's machine is fast.
