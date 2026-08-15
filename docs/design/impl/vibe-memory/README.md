# 1.8 GB, and where it actually goes — item 60

The owner, 2026-08-15: *"figure out why we are using so much memory… I see
1.8GB."*

## What the entry guessed, and what the measurement says

`docs/WORK.md` item 60 reasoned from the source: `baz-vibe` held **both** ONNX
towers per worker thread; the text tower is 126 MB on disk and the audio tower
34 MB; a scan ran eight workers; so a compose could materialise `8 × 160 MB` =
1.28 GB of weights on top of a ~260 MB idle baseline. That produced the first
repair, shipped in 0.2.0: **each tower is opened where it is used**, so the
workers hold audio weights and one thread holds the text tower. The doc comment
that landed with it predicted "a little over 400 MB where it was over a
gigabyte".

**It was wrong, and only a measurement could say so.** `measure.sh` drives the
real binary headlessly — Home → New vibe playlist → type a request → Compose —
and samples this process's own `/proc` RSS every two seconds across idle,
composing, and two minutes of quiet afterwards, on a 24-track fixture.

| workers | idle | peak while composing | two minutes later |
|---|---|---|---|
| 2 | 251 MiB | **863 MiB** | 733 MiB |
| 4 | 252 MiB | **1 129 MiB** | 670 MiB |
| 8 | 252 MiB | **1 762 MiB** | 747 MiB |

Three readings, and each says something the source could not:

1. **The owner's 1.8 GB is reproducible on twenty-four tracks.** It is not a
   large-library effect at all; it is the *width* of the scan.
2. **A worker costs about 145 MiB, not 34.** The marginal cost between 2 → 4
   workers is 133 MiB each and between 4 → 8 is 158 MiB each. The weights file
   is 34 MB, so what dominates is **ONNX Runtime's per-session arena**, which
   no amount of lazy loading touches.
3. **Roughly 420–500 MiB never comes back.** Two minutes after the compose
   finishes the process sits at 670–750 MiB against a 252 MiB idle baseline,
   and the floor is nearly independent of how many workers ran — so it is the
   text tower's session plus retained arena rather than the workers'.

## What shipped here

**`DEFAULT_VIBE_WORKERS` 8 → 4.** It halves the peak (1.76 → 1.13 GiB) while
keeping real concurrency, and `vibe_workers` in `config.toml` or
`BAZ_VIBE_WORKERS` still buys speed with memory for anyone who wants that
trade. It is the one change the measurement justifies on its own.

## What it does not fix, and what the numbers say to do next

- **One shared session instead of one per worker.** `Session::run` takes
  `&mut self` in `ort` 2.0-rc.10, so sharing means a `Mutex` and serialised
  inference with more intra-op threads inside it. That is the change that would
  take the peak from 1.13 GiB to roughly 400 MiB, and it trades wall-clock for
  it — so it needs a *time* measurement beside this memory one before anyone
  chooses. The current design chose the other way deliberately, and its comment
  says so.
- **Release the sessions when a scan ends.** The 500 MiB floor is the evidence
  that they are not released today. The workers are tokio blocking-pool
  threads, so their thread-locals outlive the scan by however long the pool
  keeps them, and the arena is not returned to the OS even then.
- **Price ORT's arena options.** `with_memory_pattern(false)` and a shared
  allocator are both one line and both unmeasured.

None of those is a guess-and-ship: this file exists because the last one was.

## The live count bought a second text tower — plan 22's ship gate, 2026-08-15

Design 21 §6 adds a live match count under the field, and §10 names its cost
honestly: *the text tower is roughly 350 MiB, paid once*. **Paid once was the
claim and twice was the fact.** Re-running this harness against the rebuilt
page was the ship gate's own item, and it is the reason the gate exists.

The count's embedding runs on a tokio blocking thread; a compose's runs on the
interface thread. The tower was a `thread_local!` alongside the audio tower —
correct for the audio tower, which wants to be per-worker — so a page whose
count had settled and then composed held **two**.

Same fixture, same four workers, two runs back to back, each with its
`[mpris] no session bus` receipt:

| text tower | idle | peak while composing | two minutes later |
|---|---|---|---|
| thread-local, one per thread | 252 MiB | **1 732 MiB** | 776 MiB |
| **one shared, behind a mutex** | 252 MiB | **1 363 MiB** | 823 MiB |

**369 MiB**, which is one tower and its arena, and the difference is the whole
finding. The mutex costs wall-clock only when two text embeddings race, and
they cannot: there is one debounced count and one compose, and a text
embedding is tens of milliseconds. This is `WORK.md` item 60's remaining half
— *"one shared model session behind a mutex"* — made necessary rather than
optional by the readout that needed it.

### And 1 363 is still 234 MiB above the old baseline, on purpose

The 1 129 MiB in the table above was measured on the **old page**, which had no
live count at all: nothing embedded text until a compose, and a compose happens
*after* the scan. The tower's cost therefore followed the workers' peak instead
of overlapping it.

With a live count it necessarily overlaps — that is what *live* means, and it
is exactly the cost design 21 §10 said this readout would have. 234 MiB is the
real price of the count; 603 would have been the price of not noticing which
thread it ran on.

## How long listening actually takes — plan 22 item 0.4

Design 21 §10 recorded that **nobody had measured a per-track analysis rate on
a real library**, and design 21 §7's first-run copy quotes one anyway: *9 412
tracks · roughly two hours*. Plan 22 §0.4 makes that sentence wait for the
number. `crates/baz-vibe/src/bin/vibe-rate.rs` runs the shipping analysis path
— decode, bliss features, CLAP audio embedding, store — at the shipping worker
count, over tracks drawn from a real library database into a throwaway store.

| workers | tracks | wall clock | **tracks / hour** | median per track | p90 | max |
|---|---|---|---|---|---|---|
| 4 | 180 | 144.3 s | **4 490** | 3.02 s | 4.62 s | 7.21 s |

Twenty of the two hundred were skipped as *no such file* — library rows whose
files are no longer at that path — which is a scanner fact rather than an
analysis failure, and they are excluded from the rate.

**What the copy may now say.** At 4 490 tracks/hour, a 9 412-track library is
**2 h 06 m** and this 5 076-track one is **68 minutes**. Design 21's *"roughly
two hours"* was right, and is now measured rather than assumed.

**The condition this was taken under, because it bounds the number.** The
library sits on an SMB share reached through gvfs, so every track is decoded
across a network mount. That is the owner's real setup and therefore the
honest case to quote, but a local-disk library should be faster and the copy
should not promise the same figure as a floor. The per-track spread — p10
1.81 s against a 7.21 s maximum — is why the first-run reading is a real
progress bar with a count on it rather than a single estimate: a four-fold
spread makes any one number wrong for most of the run.

```sh
toolbox run -c baz-dev ./target/release/vibe-rate \
  <copy-of>/library.db /tmp/scratch/rate-store.db 200 4
```

## Running it

```sh
toolbox run -c baz-dev docs/design/impl/contour/mkfixture-varied.sh /tmp/baz-varied
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/impl/vibe-memory/measure.sh          # the default
BAZ_VIBE_WORKERS=8 toolbox run -c baz-dev env BAZ_VIBE_WORKERS=8 \
  docs/design/impl/vibe-memory/measure.sh
```

Headless on a private Xvfb with all six XDG redirections; each run prints its
`[mpris] no session bus` receipt and leaves its samples in `rss-<workers>.tsv`.
The committed `rss-8.tsv` was taken with no `BAZ_VIBE_WORKERS` set, when eight
was still the default — which is what the owner was running when he saw
1.8 GB.
The fixture is the varied one (`docs/design/impl/contour/mkfixture-varied.sh`)
rather than the silent layout fixture: analysis of digital silence is not the
work being measured.
