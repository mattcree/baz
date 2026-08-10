# The frame is the frame in every place

> `views/mod.rs` has claimed, in prose, that *"the frame is the frame in every
> place — navigating may not slide the content area by a pixel"*. It was false
> by **12 px**, and had been for about a month. These frames are from the real
> binary on a private Xvfb with all six XDG redirections; each run prints its
> `[mpris] no session bus` receipt.

## What was wrong

`place_header_led` lays out whatever lead it is handed, and what it is handed
differs in *kind*:

| lead | height it declares | strip |
|---|---|---|
| a **control** — the Album place's breadcrumb | `TRANSPORT_HIT` **32** | 49 px |
| a **word** — a bare `place_name` | `LEADING_EMPHASIS` **20** | 37 px |

`theme::TOP_BAR_H` is `2 · TOP_BAR_PAD_V + TRANSPORT_HIT + 1` = **49** and is
correct; the Library's own strip honours it. The drift was in the *other*
strip not being held to it — which is why the source reads as though nothing
is wrong.

## What the measurement actually found, against what was expected

`docs/WORK.md` carried this as *"Queue, Settings and the Artist place all sit
12 px above"*. Measured, **one of those three was true**:

| place | lead | hairline before | after | |
|---|---|---|---|---|
| Library | its own `top_bar` | 48 | 48 | control, must not move |
| a record's page | breadcrumb | 48 | 48 | control, must not move |
| the Artist place | a word | **48** | 48 | *already right* — see below |
| Settings | a word | **36** | **48** | the only place that drifted |

- **Queue no longer exists.** `Place::Queue` was deleted when the run column
  merged into `Now playing`; the item outlived the place it named.
- **The Artist place had grown its own copy of the box** — `container(lead)
  .height(TRANSPORT_HIT)` in `views/artist.rs`, the second of three such
  copies, the third being `views/page.rs`'s. So it measured 48 already. Both
  local copies are deleted here and the artist page **does not move**, which is
  the evidence that the general fix subsumes them exactly rather than merely
  agreeing with them today.

So the defect was one place wide, and the *fix* is three places wide: one
answer where there were two local ones and one absence.

## How to read the frames

`01-library` · `02-record` · `03-artist` · `04-settings`, each `-before` and
`-after`, at 1280×860. `04-settings-together.png` is the pair side by side.

**The harness shoots two builds**, because one build cannot show a thing
moving — the lesson `one-page-two-subjects/` paid for. It also composites at
the same **window** coordinates rather than cropping each place to its own
content: cropping compares *shapes*, a shared crop compares *positions*, and
this defect was identical shapes at different positions.

## Two things this harness got wrong first, both worth keeping written down

**The clicks missed, and the frames looked fine.** The first run clicked the
breadcrumb at y = 96 when it sits at y = 24, hit nothing, and photographed the
record's page twice — once labelled `03-artist`. The frames were real frames of
a real place; only the *name* was wrong. What caught it was the script
asserting `md5` equality and printing *"the fix did not reach this place"*,
not an eye on the picture.

**Two agents collided on a filename.** This harness and the crossfade work
both wrote a built binary to `$CLAUDE_JOB_DIR/tmp/baz-after`. The second run
therefore compared this branch's base against *another branch's build*, and
produced a difference on the Library — a page this change cannot reach. That
number was nearly believed. It was caught only because the claim *"the Library
must not move"* was written down first and checked, so the harness had
something to be surprised by. Both are now under private paths.

**And a third thing, which is why these frames were shot twice.** A parallel
branch's harness was leaking a `baz` and an `Xvfb` per run — `kill` on a
`toolbox run` wrapper signals the wrapper, not the `podman exec`'d process
inside the container — and by the time it was noticed, thirty-seven had
accumulated and the machine had hit its thread limit: an app failed to start
its audio and MPRIS threads with `EAGAIN`, and a library scan lost a race it
normally wins. **These frames were taken inside that window.** They were
therefore re-shot afterwards on a quiet machine and are **byte-identical to
the committed ones, all eight**, with the same hairline numbers — so the
evidence stands on its own re-verification rather than on the absence of a
reason to doubt it. This harness does not leak (it runs *inside* the
container, so its `kill` reaches the real process), but "my script is fine"
was a claim worth checking rather than asserting.

## Reproducing

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb cargo build --release -p baz
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-frame-fix
toolbox run -c baz-dev env FIX=/tmp/baz-frame-fix \
  BEFORE=<base binary> AFTER=<built binary> \
  docs/design/impl/one-frame-everywhere/capture.sh
```

The hairline row is measured with the snippet in this directory's history: the
first row in the content area where >95 % of a 700 px span is lighter than the
wall — text never spans 700 px, so the only thing that can satisfy it is the
rule under the strip.
