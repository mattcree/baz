# Hover options on a record, and the cover in the bar

Render evidence for the owner's approved design: **hovering a sleeve on the
wall reveals four options laid over it**, and **`Play` sounds the record in one
press**. Captured against the real binary at 1280 × 860 and 1920 × 1080 by
[`capture.sh`](capture.sh), headless on a private Xvfb with all six XDG
redirections from `docs/DEVELOPMENT.md`. Both runs print

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

which is the receipt that nothing touched the owner's session bus.

This reverses two entries of the product's standing rules — *sound from the wall is two
presses* and *nothing is ever drawn on top of a sleeve* — and ADR-0032 §2's
*no hover-revealed verb group on the wall*. All three are rewritten to record
what was decided; none is argued with. The ledger binds contributors and
agents, not the owner.

## The frames

| | |
|---|---|
| `01-wall-at-rest-*` | The wall with nothing hovered. The baseline every veil frame is measured against — and the frame the sampled-pixel table below reads its per-pixel *ground* from. |
| `02-options-bright-sleeve-*` | The four options over a bright sleeve, revealed by hovering the **caption**. That is deliberate: hovering the caption reveals the group without lighting any option row, so every pixel in the sleeve is veil over artwork and nothing else. This is the measurement frame. |
| `03-options-play-row-hovered-*` | The pointer on the `Play` row: the faint *light* wash brightening from the left. A light wash, not a darker one — the veil under it is already the room's darkest ground, and a second dark wash would say "less" where the pointer means "this one". |
| `04-options-dark-sleeve-*` | The same group over a near-black sleeve, which is the case the veil is hardest on. |
| `05-bar-with-cover-*` | `Play` pressed **from the wall**: one press, and the record is sounding — the halo is lit, the needle is running, and the bar carries the 52 px cover beside the track and artist. **The wall is still on screen**, which is the routing claim proved on pixels: an option's press is captured by the option and never also opens the page. |
| `06-press-outside-an-option-opens-the-page-*` | The same sleeve pressed in its right third, outside the hit band: the record's page opens, exactly as it always did. The two frames together are the whole of the press routing. |

## What is drawn

**The veil** gathers at the sleeve's left edge and dissolves to nothing before
the right one, over `recess` `#060708`. Its stops, as designed:

| offset | 0.00 | 0.38 | 0.55 | 0.68 | 0.82 | 1.00 |
|---|---|---|---|---|---|---|
| opacity | 0.92 | 0.86 | 0.66 | 0.30 | 0.05 | 0.00 |

The right of every cover stays exactly as painted, which is the point: you
choose *about a record* while still looking at it.

**Four options**, left-justified on one shared left edge, glyph then label,
each taking a quarter of the sleeve's height as its hit band — 60 px at
1280 × 860, 63 at 1920 × 1080, and 47 at the tightest density baz draws,
against law L7's 32 px floor. In order: `Play`, `Queue`, `Add to…`, `Open`.

**The ink lane ends at the veil's `0.55` stop and the hit band at its `0.68`
stop.** Neither is a number of its own; both are read out of `VEIL_SPEC`.
Type stops where the veil is still thick enough to carry it over *any* sleeve
(the contrast measurement below), and the band stops well short of the right
edge so that **pressing the sleeve outside an option still opens the record's
page**, exactly as before.

## The one departure from the approved mockup

The mockup gives the **amber** glyph to `Play` *and* `Queue`. `Play` has it
here; **`Queue` is paper**. the product's amber entry names this case in
these words — the lamp states what is true about playback right now and *"not
what is queued"* — and it is the one entry the brief did not touch. The
brief's own licence (*"if it reads too loud, drop to paper and say so"*) is
taken rather than argued with. It is one word in `theme::veil_option_ink` to
put back.

`Play` keeps the accent under the licence `theme::primary` already holds: it is
the control that *creates* playback truth, and at most one tile is hovered, so
there is still at most one of it on screen.

## The sampled-pixel table

**The correction this needed runs the opposite way to the one this repo
remembers.** `theme.rs` documents a 3.7× *overdraw*: iced composites in linear
light, and a 7 % light hairline on a dark ground drew at ink 26 %. The veil is
the other case — dark ink over artwork that is mostly lighter than it — and
there linear compositing draws **weaker**. Handed through unchanged, the
design's own numbers would have drawn at roughly half their specified weight
over a mid-grey sleeve: `0.30` reading as an effective `0.16`, `0.05` as
`0.025`. Applying the remembered 3.7× in its remembered direction would have
made that worse. So `theme::veil_alpha` *solves* the direction instead of
remembering it, against a stated reference ground (sRGB mid grey).

[`sample.py`](sample.py) checks the result on real pixels rather than on that
arithmetic. For every pixel of the hovered sleeve it recovers the opacity that
would have produced it **in sRGB** —

```
a_eff = (rest − drawn) / (rest − recess)
```

— per channel, then takes the median down the sleeve's height so the option
glyphs and labels cannot move the answer. Run `python3 sample.py`.

### 1280 × 860

| offset across the sleeve | design (sRGB) | measured on the frame | delta |
|---|---|---|---|
| 0.00 | 0.920 | 0.941 | +0.021 |
| 0.38 | 0.860 | 0.871 | +0.011 |
| 0.55 | 0.660 | 0.670 | +0.010 |
| 0.68 | 0.300 | 0.300 | +0.000 |
| 0.82 | 0.050 | 0.049 | −0.001 |
| 1.00 | 0.000 | 0.000 | +0.000 |

Worst deviation: **0.021** of an opacity.

### 1920 × 1080

| offset across the sleeve | design (sRGB) | measured on the frame | delta |
|---|---|---|---|
| 0.00 | 0.920 | 0.940 | +0.020 |
| 0.38 | 0.860 | 0.871 | +0.011 |
| 0.55 | 0.660 | 0.670 | +0.010 |
| 0.68 | 0.300 | 0.300 | +0.000 |
| 0.82 | 0.050 | 0.049 | −0.001 |
| 1.00 | 0.000 | 0.000 | +0.000 |

Worst deviation: **0.020** of an opacity.

The residual is a property of the single reference ground, and it is bounded in
code as well as measured here:
`the_veil_is_solved_against_a_stated_ground_and_its_residual_is_bounded` holds
it to ≤ 10 / 255 over sleeve grounds from sRGB 0.15 to 0.95 in Closing Time,
and to ≤ 28 / 255 in Reading Room — whose veil is a near-*white* ink, so its
extreme is a near-black sleeve, where the sRGB curve and the linear one are
furthest apart. Stated rather than hidden: one tolerance covering both rooms
would have measured neither.

## Contrast, measured on the composited veil

The floor is checked against **the veil composited over the sleeve**, not
against the sleeve, and over the worst sleeve there is — paper white in the
dark room, black in the light one.
`the_option_ink_clears_its_floor_on_the_veil_over_any_sleeve` measures each
mark where it actually sits: the label at the ink lane's far edge (the thinnest
veil any type stands on) and the glyph at the lead plus one icon box. Both
clear their floors — 4.5 : 1 for the labels, 3 : 1 for the glyphs — in both
rooms over sleeves from black to white. The ink lane ends at the `0.55` stop
*because* of this; one stop wider does not pass.

## Responsiveness

**The reveal is a boolean, not a tween.** It is the `+` slot's own mechanism
(`Shelf::hovered_album`), so there is no new motion class, no clock, no
subscription, and ADR-0020's five transitions are untouched. The tile's 90 ms
rule tween is unchanged and still the only thing on the wall that moves.

Measured against the same binary under the same isolation, frames drawn in a
10-second window with the pointer parked:

| | frames in 10 s |
|---|---|
| nothing hovered | **0** |
| options revealed, pointer resting on the tile | **0** |

**One caveat, stated because it matters.** These runs are on Xvfb with no GPU,
so iced falls back to `tiny-skia` and the process sits at ~99.8 % CPU in that
harness whether or not anything is hovered. The **same measurement on the
pre-change binary gives the same 99.8 %**, so it is the software-rendering
harness and not this change; the invariant that is actually measurable here is
the frame count, and it is zero either way. The 0.0 % idle figure in
`docs/design/04-fluidity.md` §1.4 is a real-hardware number and this branch has
not been run on real hardware — see `docs/BACKLOG.md`.
