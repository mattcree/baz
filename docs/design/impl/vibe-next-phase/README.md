# The Vibe page before it is redesigned — doc 19's evidence

Four frames from the real binary, headless on a private Xvfb with all six XDG
redirections; each run prints its `[mpris] no session bus` receipt.

| | |
|---|---|
| `01-cold-1600x900` | The form as it opens, in a desktop window. |
| `02-filled-1600x900` | One press into `Sunday morning`, which fills the words, the shape and the length. |
| `01-cold-1000x700` | The same form in a narrow window. |
| `02-filled-1000x700` | The same press, narrow. |

## What the wide frame shows, measured

- The words field is **1 270 px wide** for a six-word phrase, because the form
  is one `Fill` column and there is nothing to the right of it.
- The **right half of the window holds nothing**.
- The contour is **cut by the fold**, and the length, the Compose control and
  the list are all below it — so the page's whole purpose is off screen at the
  moment a listener is deciding what to ask for.
- The narrow frame is the **same column**: the page is only ever designed for
  narrow, and it never says so.

That is the argument of `docs/design/19-vibe-next-phase.md` §2, and it is why
the proposal is a two-pane composition rather than a tidying of this one.

## Re-running it

```sh
toolbox run -c baz-dev docs/design/impl/contour/mkfixture-varied.sh /tmp/baz-varied
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev env FIX=/tmp/baz-varied \
  docs/design/impl/vibe-next-phase/capture.sh
```

The fixture is the varied one rather than the silent layout fixture: a mood
press has to fill a form that a listener would recognise, and the page states
how many tracks it has analysed.
