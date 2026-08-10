# The merged surface — frames of what M1 and M2 shipped

`Place::Queue` is gone and `Place::NowPlaying` absorbed it whole
([`docs/design/12-now-playing-and-kiosk.md`](../../12-now-playing-and-kiosk.md)
§3.4, §5.5a, §6.4; [ADR-0029](../../../adr/0029-the-ambient-surface.md),
[ADR-0034](../../../adr/0034-the-run-and-its-list.md)). The owner: *"the queue
and the now playing need integrated in some way so we can remove the queue
option from the bottom bar"*.

**Read these against
[`../queue-in-now-playing/`](../queue-in-now-playing/README.md).** That set is
the before: `01a` there is the queue place and `01b` is the unmerged
now-playing place, side by side, each holding half of a run. `01a` here is the
one surface that took over from both.

Rendered by [`capture.sh`](capture.sh) against the release binary, headless on
a private Xvfb, with all six XDG redirections from `docs/DEVELOPMENT.md`
§"Headless UI verification". Nothing touched the owner's session and nothing
was audible — the scratch `HOME` routes ALSA's default PCM to null and every
fixture sample is a zero. The receipt the run printed:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

## The frames

| | What it shows |
|---|---|
| `01a-run-on-1280x860.png` | **The merge.** The record left-hung, the run beside it at `RUN_MEASURE` 440, the `Run` word in the top-right, the bar with no `Queue` door |
| `01b-run-off-1280x860.png` | The second density: the same place with the list stood down. The record returns to the centre |
| `01c-run-on-collapsed-1280x860.png` | The run at the same 440 with the returns lane collapsed — the case §5.5a says costs the record nothing |
| `01d-sleeve-collapsed-1280x860.png` | The widest the record gets at this window, and the one that shows ADR-0029's recovered 32 px |
| `02a`…`02d` | The same four at 1920 × 1080 |

## What the pixels say

[`measure.py`](measure.py) reads both load-bearing figures off the committed
PNGs. Nothing below is computed from tokens alone.

### The sleeve, and ADR-0029's unspent 32 px

| Frame | Sleeve | Bound by |
|---|---|---|
| 1280 × 860, lane open, run on | **456** | width — `body 1000 − 2·HANG − (440 + GAP_XL)` |
| 1280 × 860, lane collapsed, run off | **569** | height — `779 − 2·HANG − below 130` |
| 1920 × 1080, either density | **720** | `NOW_PLAYING_MAX`, which step A2 deletes |

The 569 is the number the fix bought. `art_edge`'s `below` summed
`TRANSPORT_HIT` 32 for a transport this place stopped drawing when ADR-0029's
first step landed, so every height-bound sleeve was **537**. `below` is 130 now
and the sleeve is 569 — the same window, 32 px more work.

**§5.5's `below` of 190 is still the future number.** It is 130 plus the
momentary meter's 24, the feed's 20 and one `GAP_LG` — steps A9 and A5, neither
built, and neither may reserve height before it exists. So every `by_height`
figure in §5.5 and §5.5a is 60 px larger in this build than the table states,
and will shrink to the table's number as those two land. The *properties* the
tables argue for — the record is height-bound above 1280, the run takes width
the record structurally cannot use — hold at both values, which is why the
tests state them as properties rather than as rows.

### The bar's title lane

Doc 12 §6.4.1 computed **288 → 448** at 1280 from the tokens and flagged the
figure as unverified. Measured:

| Window | Left zone | With the door | After |
|---|---|---|---|
| 1280 | 528 | **248** | **408** |
| 1920 | 848 | **568** | **728** |

The design's derivation spent `(W − TRANSPORT_W 112 − 2 · GAP_LG 16) / 2` and
left out the bar's own two `HANG` gutters, so it is 40 px optimistic in both
columns. **The delta it was making the argument about — 160 px, all of it to
the title — is exactly right**, and that is the claim that mattered:
`the_left_zone_reserves_the_stamps_and_the_continuation` pins 248, 408 and 160
against the tokens, and this reads the same 408 off the frame.

Method: the two timestamps sit in fixed `STAMP_W` 52 slots, the elapsed one
right-aligned and the total one left-aligned, so the total's leftmost ink *is*
its slot's left edge and everything else follows by arithmetic the frame does
not have to be trusted for. The frames read 408 at 1280 and 728 at 1920, with
one frame reading 409 — a single pixel of antialiasing on a glyph's stem, not a
disagreement.

**And the transport did not move.** Its left edge is
`HANG + zone + GAP_LG` = 584 at 1280 and 904 at 1920, an expression the door
never appeared in. That is why a 152 px slot could come off a zone without a
re-derivation of anything outside it.

## Reproducing

```
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-qm-fix
toolbox run -c baz-dev docs/design/impl/queue-merged/capture.sh
python3 docs/design/impl/queue-merged/measure.py
```

The fixture is silent FLAC with generated covers, so the frames are of the real
binary drawing real decoded artwork and real durations — the gradient squares
are the fixture's own covers, not a stand-in the place drew.
