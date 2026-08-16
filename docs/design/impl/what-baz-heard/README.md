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
| You have never played 22 of these | the seeded ledger holds exactly two plays |
| barely varies in brightness and texture | one tone at one frequency, so there is nothing on either axis to vary |

The last row is the one worth dwelling on. A rank axis spreads whatever it is
given across the full scale, so **a line drawn over a dimension a collection
does not vary in is followed perfectly by the dots while nothing about the
music changes** — a control that looks like it is working and is not. The
threshold under it is measured rather than guessed: `vibe-spread` over a real
5 076-track library puts the narrowest genuinely-varying axis at 0.392, and
`FLAT_AXIS` is 0.12, a third of it.

What these shots cannot show is the mood survey's own line, *Only N songs to
draw from*. It is suppressed here on purpose: a library smaller than a
playlist would put the same number on all six tiles, which says something
about the library and nothing about any mood — and the block above has
already said how much has been heard. It appears when the pool is thin
against a collection that is not.

## 03 · Composing from what you forgot

The never-played count is a **press**, not a sentence. Design note 24 §3
argues that a profile of measurements is a one-time curio while a never-played
count changes as you listen and is a route back into your own collection —
so it is the one item here that leads somewhere, and it leads somewhere by
being pressable.

It leaves the door with the filter already on, and the request sentence says
so: **`Any song you have never played, starting quiet and climbing the whole
way, for about an hour.`** The chip beside the words carries the same state
and the same number, because they are one control seen twice.

It is a **filter, not a lean** (`docs/WORK.md` item 76, Marcus's reading). A
filter is a fact a listener can check against their own memory; a weighting
that quietly *preferred* unplayed songs would be one more thing happening for
reasons nobody could read. The restriction is applied before anything is
scored, so the ranks and the eligibility knee are computed within the pool
that was asked for rather than within the library and then filtered — which
would put a "loud" song at the quiet end of its own list.

## 04 · The example is their music

The field's placeholder reads **`warm electronic, slow and sparse`**, built
from the commonest genre the library actually carries. The frame is fixed and
only the noun is theirs, which is design note 24 §7's own recommendation: a
surface that differs per library is one nobody can screenshot or support, so
the rows stay put and their contents are the listener's.

## 05 · Drew, rather than matched

The count under the field used to read *N of M songs match*. Two of the six
requests measured in `docs/design/impl/vibe-eligibility/` were at or below
chance against their own genre, so that sentence claimed a precision the
retrieval does not have — design note 23 §4's charge, and the worst kind
available, because it is a dishonesty rather than a limitation.

It now reads **`Baz drew 22 of 24 to choose from`** — the numerator carries
the request's restrictions and the denominator stays the library, so the
sentence says how much was set aside as well as how much was kept. Same
arithmetic with the claim removed, and the three nearest carry the grading
out loud: *if these are not what you meant, it has not understood the
phrase.* The titles are the only part of that readout anybody can check,
which is why they are the part that asks to be checked.

## 06 · The flat axes admit it

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

And the threshold behind the flat-axis flag, against any real store:

```sh
toolbox run -c baz-dev cargo run --release -p baz-vibe --bin vibe-spread -- \
  ~/.local/share/baz/vibe.db
```
