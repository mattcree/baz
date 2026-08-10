# The artwork crosses when the record changes

The owner, 2026-08-10: *"when changing track there isn't any kind of nice
visual transition for album art in now playing. we should have something a bit
nicer, like a quick fade"*.

Rendered from the real binary under a private `Xvfb` with the six XDG
redirections of `docs/DEVELOPMENT.md`. `capture.sh` regenerates everything here
and prints the `[mpris] no session bus` receipt that says the owner's session
was not touched; nothing was audible — the scratch `HOME` routes ALSA's default
PCM to `null` and every fixture sample is a zero.

**It films rather than screenshots.** A crossfade is motion and a still frame
cannot prove it; worse, a *pair* of stills cannot tell a crossfade from a cut,
because a cut also has a before and an after. So `capture.sh` records the
display at 60 fps with `ffmpeg -f x11grab` and `measure.py` reads every frame
back. The images below are cut out of that film — they are what the compositor
put on the screen, not a state the script arranged.

Two builds are filmed: `before` is the commit this branch started from
(`e8f9a54`) and `after` is the branch. Same fixture, same window, same gestures.

---

## The gesture is one gesture, pressed three times

The claim is a comparison, and the strongest form of it is *the same act
producing motion once and no motion twice*:

1. `Play all`, then the run column's row 12 — one click each, both of them
   things a listener does — puts the cursor on **the last track of `Ochre`**.
2. <kbd>Ctrl</kbd>+<kbd>→</kbd> is Next (`crate::keys`). The **first** press
   crosses into `Violet Ledger`: the record changes, so the picture dissolves.
3. The **second and third** presses are Next again, *inside* `Violet Ledger`:
   the record does not change, so nothing moves at all.

The two records were chosen for their hues — `Ochre` is olive over green,
`Violet Ledger` is aubergine over magenta — so that the field, which is derived
from the cover, has somewhere visible to travel.

---

## The frames

| Frame | What it shows |
|---|---|
| `00-before-the-change-{build}` | The state the film opens on: `Nightwatch 12`, the last track of `Ochre`, with `Violet Ledger` next in the run. Both builds, identical. |
| `01-the-cover-crossing-{build}` | **The headline.** The sleeve at every frame the app actually drew across the record change — not a fixed cadence, which would draw a cut as a ladder by repeating one frame five times. |
| `02-the-field-crossing-{build}` | The same instants, at the strip of pure field in the window's right margin. The room travels with the cover. |
| `03-mid-crossing-{build}` | One whole frame from the middle of the change, so the crop above can be located in the composition. |
| `04-one-record-two-tracks-{build}` | **The negative case.** Three frames spanning a later press of the same key, inside one record: the title changes and the picture does not. **The two builds agree here, and that is the point** — the old build could not fade because it never faded; the new one must not fade *because the picture did not change*. This frame is what says the new machinery stayed asleep. |
| `measured-{build}.txt` / `.json` | Every frame of the film, with its two readings and the events found in it. |

---

## What the film says

### 1 · The cover crosses, and before this branch it did not

`measure.py` projects each frame's mean sleeve colour onto the line between the
two records' settled colours. `t` is 0 at the outgoing record and 1 at the
incoming one, and the question is **how many distinct frames the surface spends
strictly between them**. A cut spends none.

**The counts are the whole finding**, and they are not close:

| | `before` | `after` |
|---|---|---|
| frames the sleeve was repainted in | **1** | **12** |
| frames strictly between the two covers | **0** | **11** |
| span of the crossing | — | 183 ms of film |
| largest disagreement, cover's `t` vs field's `t` | — | **0.018** |
| window repaints in that second | 60 | 60 |

**Twelve is the number the decision predicted.** `motion.rs`'s
`a_200ms_transition_is_about_twelve_frames_at_60hz` derives twelve frames from
the tween's arithmetic without a window anywhere near it; the film counted
twelve on screen. The eleven "strictly between" are those twelve less the one
that landed on the target, which the band excludes by construction.

`01-the-cover-crossing-before.png` is two panels because there are two frames:
olive over green, then aubergine over magenta. There is nothing between them to
photograph. `01-the-cover-crossing-after.png` is fourteen — the two ends and
every frame the app drew between them.

**A second defect the hold removes, which this film does not happen to show.**
Before this branch, a record change whose hero had not decoded cut to the
record's **320 px thumbnail**, small, on a room with **no field at all**, and
cut again to the full-size hero when the decode landed: two cuts and a size
change. `measure.py` probes for it — *the artwork did NOT fill its box* — and
caught it on a loaded machine, where the decode took long enough to be seen. On
the quiet machine these frames were shot on, the decode landed inside a single
frame and the `before` film shows one clean cut instead. The wart is real and
timing-dependent; the hold removes it in both cases, because the surface now
never draws a thumbnail where a hero is coming.

### 2 · The room crosses with it, off the same number

`field::dissolve` takes the same `t` the cover's incoming layer is drawn at, so
the two cannot disagree by construction. The film measures them independently
anyway, at two probes that share no pixels: the largest disagreement between the
cover's `t` and the field's `t` at any frame of the crossing is **0.018** — the
two ladders are one ladder. `02-the-field-crossing-after.png` is the same
fourteen instants at the field probe: the tint walks from olive-green to violet
across them. It is subtle in absolute terms because it is meant to be — the
field's chroma is pinned at `field::CHROMA` 0.024, a tint and never a colour —
and the measurement is what makes "it moved, and it moved with the cover"
checkable rather than a matter of eyesight.

### 3 · Nothing moves where nothing should

At **each** of the three later track changes — all of them inside
`Violet Ledger` — the sleeve's pixels move in **0** of the following 60 frames,
and both `t` readings stay pinned at 1.000. `04-one-record-two-tracks-after.png`
is three frames spanning one of them: `Field Recording 1` becomes
`Anhydrous 2`, the needle resets, and the artwork above is unchanged to the
pixel. That is the whole of *compare the picture, not the track*, and it is the
case that would have made this feature worse than the cut it replaces.

(The film catches three within-record changes for two presses of Next: the null
sink accepts writes as fast as they arrive, so playback free-wheels and a track
ends on its own during the twelve seconds. It is another negative case, taken
for free, and driven by the engine rather than by the script.)

### 4 · What it costs

- **At rest: nothing.** The transition's clock is a function of state, so a
  settled tween removes its own timer (`the_motion_clock_is_off_until_something_moves`,
  extended for this transition), and a settled surface draws one `image` with
  no `stack!` around it (`a_settled_surface_has_nothing_to_dissolve`).
- **While it runs:** one extra `image`, for 200 ms, once per **record** change.
  No new decode and no new cache — `art::HERO_CACHE_ENTRIES` is 2 and its second
  entry already holds the record that just stopped, which
  `the_hero_lru_holds_both_records_a_dissolve_needs` checks rather than trusts;
  the outgoing handle is an `Arc` onto those same pixels.
- **CPU at rest**, five 2 s samples per build, paused, well after the
  transition: `before` **101.0 101.0 101.0 100.5 101.0**, `after` **100.5 101.0
  100.5 101.0 100.5** % of one core. Indistinguishable. Read the *absolute*
  figure as the harness's and not the product's — this display is `llvmpipe`
  with no vsync and no GPU, and a steady one core is what that costs; what the
  five samples are for is the comparison, which is flat.

---

## What this film cannot tell you, and what does

**The 183 ms is a span of film, not the tween's duration.** The transition is
200 ms by construction and `motion.rs` pins it —
`the_transitions_run_for_the_times_the_decision_names`,
`a_200ms_transition_is_about_twelve_frames_at_60hz`, and
`the_dissolve_is_the_lamps_own_number`, which asserts that the picture and the
lamp are the same constant and land on the same tick. What the film measures is
the frames between the first that is not the old cover and the last that is not
yet the new one, which is 200 ms less the two ends the band excludes.

**And the frame count is a property of the machine as much as of the tween.**
An earlier version of this capture, taken while thirty leaked processes from
its own earlier runs were still on the box, filmed the same transition in
**four or five** distinct frames spread over 300 ms — because the app
software-rasterises a 1280 × 860 window on `Xvfb` and the renderer, not the
tween, decides how many frames land. The numbers above were taken after that
was cleaned up, and the window repaints 60 times a second in them. **A frame
count read off a loaded machine and reported as the product's would be a
measurement of the measuring apparatus**, which is why `measure.py` prints the
repaint rate beside every claim.

---

## One thing for the owner's eye

**The hold is real, and its length is not baz's to control.** The surface
deliberately keeps the picture it has until the incoming record's hero has
decoded — that is what makes this a dissolve rather than a fade to nothing
followed by a pop — and for that interval the previous record's cover stands
under the new record's title.

On the quiet machine these frames were shot on it is **33 ms**, two frames,
which nobody will see. On the same machine under load it filmed at **100–320
ms**, which somebody might. The variable is `art::load_hero` — a JPEG decode of
whatever size the listener's own covers are — and the fixture's are 600 × 600.
A library of 3000 × 3000 scans will hold longer.

It is bounded, and it is strictly better than what it replaced. But it is the
one number in this work that depends on his files and his machine rather than
on the product.

**If he finds it long, the fix is not a shorter tween.** It is the hero
*prefetch* — `art::HERO_CACHE_ENTRIES` already describes it as one line once
ADR-0034's `Origin` work can name the successor record, and with the next
record's hero already decoded the hold is zero and the dissolve starts on the
track change itself. The crossfade is the first consumer that makes that
prefetch worth building.

---

## Reproducing

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lane-fix
BIN0=/path/to/before BIN=/path/to/after \
  docs/design/impl/art-crossfade/capture.sh
```

`ffmpeg` is on the host and the binary runs in the container (a host-built
release links a newer glibc than the container has); `/tmp/.X11-unix` is shared
between them, which is what lets one grab the other's display.

**Two harness bugs are worth not repeating**, and both are fixed in the scripts
here rather than only described:

- **Killing a `toolbox run` does not kill what it started.** The wrapper is a
  `podman exec`; the process it launched lives in the container and outlives
  it. Enough leaked runs will exhaust the machine's thread limit, at which
  point the app fails to start its audio and D-Bus threads with `EAGAIN` and
  the failure reads like a bug in the product. `reap` matches the **whole
  command line, anchored** — never a bare name, which would also match the
  maintainer's own running copy.
- **A film whose two ends are the same record proves nothing**, and that is
  what a race between the launch scan and `Play all` looks like from here: the
  gestures all land, the app is healthy, and the record simply never changed.
  `measure.py` refuses such a film loudly instead of reporting a crossing of
  zero frames as a finding.
