# Selecting more than one thing

**Item 62.** `docs/design/18-feature-parity.md` §4 calls multi-select the floor
every other list feature stands on — tag editing and bulk actions both need a
bulk *selection* before they can mean anything.

| | |
|---|---|
| [`ctrl-picked.png`](ctrl-picked.png) | Three non-adjacent rows, built with <kbd>Ctrl</kbd> — the half a range cannot do. |
| [`shift-range.png`](shift-range.png) | Five, taken with <kbd>Shift</kbd> from the anchor the last press left. |
| [`cleared.png`](cleared.png) | <kbd>Esc</kbd>, and the strip is gone with the set. |

## What the frames show that the tests cannot

**No view changed.** `selection::State::is` answers for the whole set, so the
album page, the queue, a playlist's page and the wall draw a set with the code
that drew one — which is why the lit rows here look exactly like a single
selected row always did, only more of them.

**The strip is the whole of the new interface**, and it is a visible control
rather than a right-click menu because of the rule `crate::menu` is built on:
*no action's only route is a gesture*. It states the count first, because that
is the fact it exists for, and it carries `Remove` only over a queue or a
playlist page — the two lists a listener owns and can shorten. On this album
page there is no `Remove`, which is the frame's other reading.

## Reproducing it

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb cargo build --release -p baz
toolbox run -c baz-dev docs/design/impl/multi-select/capture.sh
```

Headless and isolated six ways, on the silent fixture — nothing here needs to
sound, so unlike `docs/design/impl/liquid-glass/capture.sh` this one leaves the
fixture as the generator wrote it. The row coordinates (`ROW_1`, `ROW_PITCH`)
were read off `album-page.png` in the scratch, which the run writes on the way
past; a run that guesses them photographs a set nobody built.
