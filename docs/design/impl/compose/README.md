# The composing place, state by state — plan 22 §7

Design 21 §7 draws nine states of this feature and the shipping build designed
one. `capture.sh` exercises them in the **real binary**, headless on a private
Xvfb with all six XDG redirections, against a fixture library it analyses for
real — because everything the page claims (the live count, the eligible cloud
thinning under the curve, the ticks, the diff naming its cause) needs an
analysed collection to be *true* rather than asserted.

Every run prints its `[mpris] no session bus` line. That is the receipt that
nothing touched the owner's session, library or analysis store.

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/impl/contour/mkfixture-varied.sh /tmp/baz-varied
toolbox run -c baz-dev docs/design/impl/compose/capture.sh
```

1 600 × 980, which is wide enough for the two panes (`COMPOSE_BREAKPOINT` is
1 184 **of body**, and the returns lane takes 232 of the window) and tall
enough for the curve (`COMPOSE_SHORT_H` is 700).

## The frames

| frame | design 21 §7 | what it has to show |
|---|---|---|
| `01-never-listened` | 1 | the ask fully drawn and fully pressable, the commitment saying what it needs, the invitation with its cost in the result pane |
| `02-ask-live-while-cold` | 1 | a request typed before anything has been heard — the state is not a wall |
| `03-listening` | 2 | a real reading, not a spinner: how many, how long left, a way to stop |
| `04-ready` | 3 | the default shape and no words is a good request; the commitment says *about an hour* |
| `05-started-from-a-mood` | 4 | a mood writes into the one input there is |
| `06-a-list` | 6 | the curve with a dot per song over the rows with their ticks |
| `07-why-this-song` | 6 | a selected row explaining itself as a rank, in three cues, none a colour |
| `08-diff-after-narrowing` | 7 | new/kept with the sentence naming the words as the cause, and both counts |
| `09-another-version` | 7 | the same request, a different draw, and the diff saying so |
| `10-each-thing-baz-listens-for` | aside | the expander revealing the five lines rather than seeding them |

**One worker, not eight.** The fixture is 24 tracks, which eight workers finish
inside a couple of seconds — faster than state 2 can be photographed. The
capture runs at one worker so the listening reading exists long enough to be
looked at. The *rate* is measured elsewhere and on a real library
(`vibe-rate`, `docs/design/impl/vibe-memory/`); this run is about the states.

## What the frames caught that review had not

Rendering rather than arguing is the point, and it earned its keep four times.

1. **The two panes did not fire at 1 600 px.** The breakpoint had been derived
   from `LIST_MEASURE` 880 — the *maximum* a row lane may take rather than what
   it needs — and measured against the window rather than the place's body.
   The returns lane is 232 px, so a 1 600 px window has 1 368 px of body and
   the page silently stacked. It is derived from `COMPOSE_RESULT_MIN` 600 now,
   and both mistakes are written into the constant's own doc.
2. **The axis words ran straight through the foot.** *quiet, slow, sparse* does
   not fit a 48 px gutter, and it overprinted *first song*. The words sit above
   and below the line now, where there is the whole measure for them.
3. **Two accent-weight controls stood on one screen.** On a cold library both
   `Compose` and `Listen to my music` wore the commitment outline. The act on
   offer is the listening; `Compose` says what it needs and waits.
4. **The count read *matches 0 songs of the 0 Baz has heard*, and then went
   stale.** Arithmetically honest, and nonsense as a sentence; and after the
   scan finished it still described the library as it had been when the phrase
   settled. The empty case says what is true instead, and finishing a scan
   asks for a recount, because the pool the count is against has just changed.

Three more, from reading the frames rather than the code:

5. **The why-line started its second sentence in lower case.** It had been
   written as one sentence with an embedded clause and became two.
6. **The diff said *"your words left what is eligible, from 24 to 24"*.**
   Arithmetically right, and not a sentence. When the pool does not move it now
   says so directly.
7. **The chips had no lit state and a call that implied one.** They were styled
   with `theme::tile`, which ignores its own `selected` argument and returns
   the same transparent style either way — so *which starting point am I on*
   was carried entirely by the text colour, and the code read as though it were
   not. Weight and value now carry it, deliberately in two non-hue dimensions.

None of the seven is visible in the source. All seven are visible in a frame.

## The colour-blindness pass, state by state

The standing rule in this product is that **no reading may rest on telling two
hues apart**. Every reading this page adds carries its meaning somewhere else
as well:

| reading | not-a-colour cue |
|---|---|
| match strength on a row | **count** of filled ticks, and each tick's **height** |
| the chosen songs on the curve | **shape** — dots against a line — and, for the selected one, **size** plus a guide dropped to the floor |
| which row is selected | the row's **card**, and the why-line that appears **under it** |
| which starting point is lit | the medium **face** against the regular one, and full paper against dimmed |
| the one commitment | its **outline** and its **position**, and it is the only one on screen in any state |
| cannot-fill | a **sentence with numbers**, not a tint |

The accent appears twice — the commitment's outline and the result dots — and
in both cases something non-chromatic says the same thing.

## What is not here

**Design 21 §7 state 5, *composing*, has no frame** because it has no
duration. That state was drawn as skeleton rows under *looking through 9 412
tracks…*, and a compose over an already-analysed library is sub-second: the
skeletons would be a flash. If a library ever gets large enough that a compose
is visibly slow, this is the state to build, and the reason it was not built
now is that it would have been decoration.
