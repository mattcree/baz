# Home's `CONTINUE` band — the question you ask in the silence

Frames from the real binary for ADR-0030's **third amendment**, rendered
headless on a private Xvfb with all six XDG redirections from
[`docs/DEVELOPMENT.md`](../../../DEVELOPMENT.md). Nothing touched the owner's
session; `capture.sh` prints the run's own `[mpris] no session bus` line as the
receipt, and it is quoted at the foot of this page.

Reproduce with [`capture.sh`](capture.sh); the header comment carries the
toolbox build line.

## What was wrong

The owner, looking at the shipped band: *"when you click 'continue' and on the
home thing it does not update to show what is currently playing"*. §6 specified
`CONTINUE` as a reading of ADR-0023 §6's **snapshot**, and a snapshot is a
record of where you *were* — so `Resume` started the music and left a frozen
placard describing the interrupted run on screen while something else was
sounding.

The first answer drafted was a band with **two readings**, `CONTINUE` and
`NOW PLAYING`, swapping on the engine at identical geometry. The owner replaced
it with a better one, in three messages: *"in fact, keep it simple with the
continue part… once you select resume, it just disappears"*, *"or takes you to
now playing"*, *"it just reappears when you stop the player"*.

## The rule these frames walk

> **The band stands whenever there is a run to carry on with and nothing is
> sounding.** Start anything, anywhere in the product, and it is gone; stop,
> and it is back, describing where you now are.

One predicate, in one function (`views::home::standing`), read off the engine.

| | frame |
|---|---|
| **The band, at launch** — the run baz was interrupted in the middle of, at the position it was interrupted at. The only state in which `session.toml` is read at all. | [`01`](01-home-with-a-run-to-carry-on-with.png) |
| **`Resume` starts the run *and* takes you to `Now playing`** — one press, and the place is populated on arrival rather than reading "Nothing playing." for the frames before the engine confirms. | [`02`](02-resume-takes-you-to-now-playing.png) |
| **Home while it is sounding** — no band. The page starts at `RECENTLY ADDED` and there is nothing above it. What is playing is the bar's job, in this place as in every other. | [`03`](03-home-while-it-is-sounding.png) |
| **Home after a pause** — the band is back, describing **what you paused**: `Blue Hour 4 · 4:44 of 5:12`, the engine's own position, agreeing with the bar to the second. Not the launch snapshot, which still names `Marginalia 2`. | [`04`](04-home-after-a-pause.png) |
| **A record put on from the wall's own hover options** — a route that has never heard of `session.toml` — takes the band away exactly as `Resume` did. The rule is read off the engine, not off the gesture. | [`05`](05-home-after-playing-something-else.png) |
| **And the band that comes back describes *that* record**: a different album, a different track, a different position. | [`06`](06-the-band-follows-the-engine-not-the-snapshot.png) |
| **`Now playing`, paused** — the division of labour the amendment rests on: the band is Home's, the sounding record is this place's and the bar's. | [`07`](07-now-playing-paused.png) |

## Measured

**The placard in both runs**, cropped from `01` and `06` and stacked
([`09`](09-the-placard-in-both-runs.png)): identical geometry, and the only
difference is which run it is about — `Ochre / Marginalia 2 · 3:12 of 6:27`
from the launch snapshot above, `Violet Ledger / Anhydrous 2 · 5:14 of 6:27`
from the live paused run below. This is the frame that shows the content
follows the engine.

**What the band's absence costs the page** — `measure.py`, which finds the
section rules in the body of four frames:

```
  band present (the launch snapshot): section rules at y = [40, 249]
  band absent  (something sounding) : section rules at y = [40]
  band back    (paused)             : section rules at y = [40, 249]
  band back    (a different run)    : section rules at y = [40, 249]

  `CONTINUE` rule           : y = 40
  `RECENTLY ADDED` rule     : y = 249  (band present)
                              y = 40  (band absent)
  the band's whole room     : 209 px, gap included
  and it comes back at exactly the same height, every time, to the pixel
```

The band is **absent, not empty**: its disappearance moves `RECENTLY ADDED` up
by exactly the 209 px it was occupying and does nothing else, and it comes back
at the same height in all three of its states.
[`08`](08-diff-band-present-vs-gone.png) is the same fact as a difference
image.

## What is not filmed, and why

**The run ending.** There is a third state in which there is no band — the run
is *over* rather than something sounding — and it goes the other way from a
pause: a run played to its end has no *where you stopped*, and
[`docs/REFUSALS.md`](../../../REFUSALS.md) states the silence at the end of a
run as a feature. Filming it needs a queue played to its end, and the fixture's
shortest track is 97 s. It is covered by tests on both sides:
`a_run_that_finished_is_not_a_run_to_carry_on_with` (the band) and
`a_run_played_to_its_end_is_written_away` (the file), so the on-screen and
on-disk judgements cannot disagree across a restart.

## Two things about the harness, stated rather than hidden

**The null sink is unpaced, so playback races.** `~/.asoundrc` points the
default PCM at ALSA's `null` plugin — one of the two independent guarantees
that nothing is audible, the other being that every fixture sample is a zero.
A null device accepts writes as fast as the engine offers them, so there is no
backpressure and a run advances some tens of times faster than real time.
That is why frame `02` is 41 s into the *fourth* track a few wall-clock seconds
after `Resume` was pressed, and why the positions in `04` and `06` are further
along than the sleeps in the script suggest. Nothing about the seek or the jump
is wrong — the same artefact is present, undocumented, in
[`lane-and-home`](../lane-and-home/README.md)'s
`18-now-playing-after-resume-1280.png`.

**The elapsed figure in the snapshot is seeded.** Two moments write
`session.toml`: the run moving (a track boundary — the file the second launch
read already named a cursor, and `capture.sh` prints it) and the exit, which is
the only writer of the *elapsed* milliseconds. Under Xvfb with no window
manager `xdotool windowclose` races winit's own X11 teardown and the process
dies before the update loop sees the close request, so the position on disk is
the track boundary's 0. The exit path is one function (`App::leave_for_good`)
reached by both exit routes and covered by tests; what cannot be shown
headlessly is a compositor delivering the request. So the script writes
`position_ms = 192000` in, to render a needle that is partway rather than a
needle at zero.

## The receipt

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

…and what the shell said about the run, across the two launches:

```
[session] no interrupted run
[session] 12 tracks held, cursor 1 at 192000 ms
```
