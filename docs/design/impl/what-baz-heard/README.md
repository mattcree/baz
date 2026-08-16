# What Baz heard — the receipts

Design note `docs/design/24-what-baz-heard.md`, built. An hour of listening
bought the *ability* to compose and showed nothing for itself; this is that
hour becoming legible, and it is legible because every line on it is a
**measurement a listener can grade** rather than a summary they can only
accept.

`capture.sh` runs the shipping binary headless against
`docs/design/impl/contour/mkfixture-varied.sh`'s 24-track fixture, whose
properties are known in advance — tempo and loudness walk from 62 BPM at
amplitude 0.18 to 168 BPM at 1.00, every file is a 220 Hz tone under a click
train, every file is tagged `GENRE=Electronic`. That is what makes these
screenshots evidence rather than decoration: the fixture's own construction
says what the page must say, and the page says it.

## 01 · Before listening

The door as it stands on a library nothing has been heard of. Nothing here is
new; it is the *before* the rest is measured against.

## 02 · What Baz heard

Every claim in the block is checkable against the fixture, and all of them
check out:

| the page says | the fixture says |
|---|---|
| Quietest and Slowest — Ini Kovac | record 01, 62 BPM at amplitude 0.18 — the bottom of both walks |
| Loudest and Fastest — Studio Hain | record 06, 168 BPM at 1.00 — the top of both |
| Tempo runs 66 to 176 BPM, centred on 117 | brackets the fixture's 58–172 BPM span, p05–p95 |
| barely varies in brightness and texture | one tone at one frequency, so there is nothing on either axis to vary |

The last row is the one worth dwelling on. A rank axis spreads whatever it is
given across the full scale, so **a line drawn over a dimension a collection
does not vary in is followed perfectly by the dots while nothing about the
music changes** — a control that looks like it is working and is not. The
threshold under it is measured rather than guessed: `vibe-spread` over a real
5 076-track library puts the narrowest genuinely-varying axis at 0.392, and
`FLAT_AXIS` is 0.12, a third of it.

### What is not on this door, and why

**A never-played count** was here for a day. Design note 24 §3 argued it was
the one line on the reading that *leads somewhere* — and the premise, *baz
keeps a play ledger*, turned out to mean an eight-day one. Measured on the
owner's library: 864 plays over 262 distinct songs against 5 076 analysed
tracks, so the line would have read *you have never played 4 814 of these*.
That is a fact about how long baz has been installed, standing among named
records that can be graded in a second and borrowing their credibility. His
word: *"it's irrelevant."* `docs/WORK.md` item 76 carries the numbers.

**A mood's own pool size**, likewise:

*"Only N songs to draw from"* on a mood the library cannot answer was built,
measured, and taken back out the same day. Design note 24 §7 item 2 assumed
the eligible count answers *can this collection do `Party`?*, and it does not:

```text
request                                      pool  top cos
warm hypnotic music for driving at night      211    0.555
calm instrumental music without vocals        157    0.620
upbeat energetic danceable music              196    0.620
gregorian chant                               175    0.697
bagpipe marching band                         246    0.489
traditional javanese gamelan                  187    0.589
throat singing from mongolia                  221    0.603
```

Four requests a 5 076-track library holds nothing for, drawing pools inside
the range the six real moods draw — and `gregorian chant` returning the
highest similarity of them all. The cause is the one `word-probe.rs` already
recorded: **CLAP text-audio similarities are not comparable across prompts**,
and *does this library contain X* is exactly a cross-prompt question. A
control that fires at random is worse than no control, so there is no control.

## 03 · The example is their music

> These last three shots also show the page after
> `docs/design/25-the-line-leads.md`: the line is the question, first and at
> both depths, and the words are the filter column on the right. Shot 03 is
> the simple depth, which had **no curve at all** before that note.

The field's placeholder reads **`warm electronic, slow and sparse`**, built
from the commonest genre the library actually carries. The frame is fixed and
only the noun is theirs, which is design note 24 §7's own recommendation: a
surface that differs per library is one nobody can screenshot or support, so
the rows stay put and their contents are the listener's.

## 04 · Drew, rather than matched

The count under the field used to read *N of M songs match*. Two of the six
requests measured in `docs/design/impl/vibe-eligibility/` were at or below
chance against their own genre, so that sentence claimed a precision the
retrieval does not have — design note 23 §4's charge, and the worst kind
available, because it is a dishonesty rather than a limitation.

It now reads **`Baz drew 24 of 24 to choose from`** — the same arithmetic with
the claim removed, and the three nearest carry the grading out loud: *if these are not what you meant, it has not understood the
phrase.* The titles are the only part of that readout anybody can check,
which is why they are the part that asks to be checked.

## 05 · The flat axes admit it

Opened, `ENERGY` and `TEMPO` carry no warning — the fixture genuinely varies
in both — and `BRIGHTNESS` says *your music barely varies in brightness —
this line will move the list very little.* The reading is in the words, not
in the ink, so it survives being printed, dimmed, or read by somebody who
cannot separate two hues.

## Isolation

Every run prints its receipt: `[mpris] no session bus`, from a scratch
`XDG_RUNTIME_DIR` that has no bus in it. Six XDG redirections, a private
Xvfb, a scratch `HOME` whose `.asoundrc` routes the default PCM to `null`,
and a fixture peaking at −30 dBFS that is never played. Nothing touches the
owner's library, session or history.

## Reproducing

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/impl/contour/mkfixture-varied.sh /tmp/baz-varied
toolbox run -c baz-dev docs/design/impl/what-baz-heard/capture.sh
```

And the threshold behind the flat-axis flag, against any real store — plus,
with requests after it, the pool measurement that ruled the mood survey out:

```sh
toolbox run -c baz-dev cargo run --release -p baz-vibe --bin vibe-spread -- \
  ~/.local/share/baz/vibe.db "gregorian chant" "bagpipe marching band"
```
