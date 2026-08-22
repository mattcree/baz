# The background visualisation, away from the place that owns it

**Item 84.** The owner, 2026-08-22:

> can you make sure when we switch to other screens and the visualizer stays in
> the background that it continues animating, but it should be heavily blurred
> or opaque? that is where I'm thinking now. I'm not a hundred percent sure I
> really like the visualizer being in the background. But if I can see it in the
> state that I mentioned there, almost that liquid glass look, then I might like
> it.

**This is a question, so the deliverable is a picture.** Whether the background
visualiser stays at all is his call; what follows is the state he asked to see
it in, so the call can be made against a thing rather than a description.

## What the frames show

| | |
|---|---|
| [`now-playing.png`](now-playing.png) | The control. Unchanged: the spectrum at full strength, because here it is the thing you are looking at. |
| [`settings.png`](settings.png) | The clearest read of the new state — a plain form with the ground behind it. |
| [`library.png`](library.png) | Over a wall of covers, where most of it is behind something. |
| [`home.png`](home.png) | The same, over a mixed page. |

## What changed, and the two findings behind it

**It was not animating at all away from Now playing.** The backdrop was drawn
everywhere and its clock existed in one place, so every other screen showed a
frozen last frame. That is now a clock everywhere a record is sounding — at
100 ms rather than 33 ms, because the veil is what makes two thirds of the
wake-ups an invisible saving.

**All four visualisations become one ground here.** At this blur a spectrum and
a spectrogram are the same picture, so keeping four constructions would be four
things to maintain for one appearance — and it would put a mode switch behind a
surface where its effect cannot be seen. `crate::glass` draws that ground: seven
soft blobs across the window, each a stack of twenty-four nested capsules whose
alphas add into a falloff with no edge you can point at. iced has no blur pass
and `canvas` is priced deliberately, so accumulated alpha is what a blur looks
like from the outside.

**Two numbers were chosen by looking**, which is the only way either could have
been:

- `glass::RINGS` was nine and the first capture came back with visible contour
  rings — a topographic map of a blur. Twenty-four reads as continuous.
- `now_playing::FROST` was 0.72 and erased the thing entirely; 0.40 leaves
  weather behind the glass while every contrast floor over it stays the floor
  over the bare wall.

## Reproducing it

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb cargo build --release -p baz
toolbox run -c baz-dev docs/design/impl/liquid-glass/capture.sh
```

`capture.sh` is headless and isolated six ways, and it prints its receipts. Two
things it had to learn, both of which produced a plausible screenshot of nothing
happening before they were fixed and are now checked rather than hoped for:

- **the owner's own library cannot be used here.** `docs/screenshots/capture.sh`
  photographs it happily, because it only needs the index and the art cache —
  both files on this machine. This capture needs something to *play*, and his
  music is on SMB shares reached through gvfs, which an isolated run has none
  of. The first attempt photographed `3 folders are not reachable`.
- **the fixture is silent on purpose**, and a visualisation of zeroes is a flat
  line. One album is given a four-tone chord with a slow vibrato, ten minutes a
  track — because a `null` ALSA PCM accepts samples as fast as they are written
  and thirty-second tracks were raced to the end of the record before the third
  frame.
