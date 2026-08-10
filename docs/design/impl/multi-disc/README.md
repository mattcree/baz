# Multi-disc — one record, several discs

> The owner, 2026-08-10: *"it would be good if multi CD albums were a single
> item"*.

The decision is [ADR-0038](../../../adr/0038-the-record-and-its-discs.md).
This folder is the evidence: a fixture holding **every shape a two-disc rip
actually arrives in**, and frames of the wall before and after.

Rebuild both with:

```sh
docs/design/impl/multi-disc/mkfixture.sh /tmp/baz-multidisc-fixture
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev env BEFORE=…/before/target/tb/release/baz \
  docs/design/impl/multi-disc/capture.sh
```

`BEFORE` is a binary built from this branch's base commit; without it frame 1
is skipped and the rest still render. The run is headless on a private Xvfb
with all six XDG redirections (`docs/DEVELOPMENT.md`), and prints its
`[mpris] no session bus` receipt. Nothing is audible: every sample in the
fixture is a zero. `~/Music` is never read.

## What the fixture holds

`mkfixture.sh` writes eight records as silent FLAC (and, for one of them, MP3)
with generated covers — one album artist each, so a mis-merge cannot hide
behind a neighbour.

| | album artist | how the discs are filed | before | after |
|---|---|---|---|---|
| 1 | Prince | one `ALBUM` tag, `DISCNUMBER` 1/2, **one folder** | 1 record | 1 record |
| 2 | The Clash | one `ALBUM` tag, `DISCNUMBER` 1/2, **two folders** | 1 record | 1 record |
| 3a | Miles Davis | `Bitches Brew (Disc 1)` / `(Disc 2)` | **2 records** | 1 record |
| 3b | Fleetwood Mac | `Tusk CD1` / `CD2`, **no `DISCNUMBER`**, in FLAC *and* MP3 | **2 records** | 1 record, 2 editions |
| 3c | The Beatles | `The Beatles [Disc 1]` / `[Disc 2]` | **2 records** | 1 record |
| 3d | Wu-Tang Clan | `Wu-Tang Forever CD1` **alone** | 1 record | 1 record, *name unchanged* |
| 3e | Talk Talk | `Spirit of Eden` + `Spirit of Eden - Disc 2` | **2 records** | 1 record |
| 4 | Genesis | **no disc signal at all**, two folders, colliding track numbers | 1 record | 1 record |

Twelve tiles become eight. Three of the four shapes were already single items;
the shatter was **shape 3**, where the ripper put the disc in the `ALBUM` tag.

## The frames

### 1 · `01-the-wall-before.png` — twelve tiles

`Tusk CD1` beside `Tusk CD2`. `Bitches Brew (Disc 1)` beside `(Disc 2)`.
`Spirit of Eden` beside `Spirit of Eden - Disc 2`. `The Beatles [Disc 1]`
beside `[Disc 2]`. Meanwhile Prince, The Clash and Genesis are already one
tile each — their discs share an `ALBUM` tag, and the grouping key reads no
path, so the folder split was never a fact the shelf could see.

### 2 · `02-the-wall-after.png` — eight tiles

The four shattered records are one record each, under the name with the marker
taken off. **`Wu-Tang Forever CD1` is still called that**, and that is the
declined guess in one tile: nothing merged, so nothing is renamed
(ADR-0038 §3).

### 3 · `03-bitches-brew-breaks-into-two-discs.png`

The merged record's page. One sleeve, eight tracks, `DISC 1` and `DISC 2`
breaking the run where the discs meet, and `Discs 2` in the condition report.
The page already knew how to draw this — the breaks are the run column's own
vocabulary, and all that changed is that the record now reaches them.

### 4 · `04-tusk-two-editions-two-discs.png`

The two axes at once. `Tusk` was ripped to `Tusk CD1` and `Tusk CD2`, twice
over — once FLAC and once MP3 — and neither copy wrote `DISCNUMBER`. It is
**one record, two editions, two discs each**: the `FLAC` / `MP3` selector under
the sleeve, `DISC 1` / `DISC 2` in the list, `Discs 2 · Tracks 8`. The disc
numbers come from the `CD1`/`CD2` in the title, which is the only place those
files state them.

### 5 · `05-spirit-of-eden-one-disc-unnumbered.png`

The asymmetric rip, and the line the rule will not cross. Half of this record
names a disc; half does not, because the tagger marked the second and left the
first alone. The unmarked half **leads with no header over it** — nothing wrote
"disc 1", so nothing says it — and `DISC 2` breaks where the marked half
starts. `Discs 2` counts it without numbering it.
