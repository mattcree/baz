# One list, drawn once — and `Now playing` at width

> **The owner, 2026-08-10**, with the app in front of him:
>
> *"I think ideally we could ensure our playlist view in the now playing and
> the playlist view/album view are the same thing. the only thing that changes
> in now playing is that we don't see file details etc. -- that is more like a
> album exploration type data"*
>
> *"also please make sure the layout of the now playing makes sense on wider
> screens"*

Shipped 2026-08-10. Two asks on one surface, and they are recorded together
because the second is what made the first worth looking at: the run column is
the third copy of the track list, and it was also the half of `Now playing`
that had never been drawn at 2560.

`capture.sh` shoots **two builds** — `BIN0` is the commit this branch started
from, `BIN` is the branch — from the same fixture, at the same windows, with
the same gestures, at **1280 × 860, 1920 × 1080 and 2560 × 1440**.
`measure.py` reads the positions back out of the pixels.

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-lane-fix
BIN0=… BIN=… toolbox run -c baz-dev \
  docs/design/impl/one-list-drawn-once/capture.sh
python3 docs/design/impl/one-list-drawn-once/measure.py
```

Isolation receipt, all six runs (docs/DEVELOPMENT.md §"Headless UI
verification", all six XDG redirections, scratch `HOME` routing ALSA to null,
silent fixtures, `BAZ_DEVICE_TESTS` unset):

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

---

## 1 · `Now playing` at width — the measurement

[`26-band-both-2560x1440.png`](26-band-both-2560x1440.png) is the whole of it
in one look: the same window coordinates, before above and after below.

`measure.py`, in window coordinates. The **gap** is the field between the
sleeve's right edge and the run's first ink; the **air** is what is left
outside the pair.

| window | build | the work | the gap | air left \| right |
|---|---|---|---|---|
| 1280 × 860 | before | 455 | 35 | 40 \| 163 |
| 1280 × 860 | **after** | **455** | **35** | **40 \| 163** |
| 1920 × 1080 | before | 599 | 531 | 40 \| 163 |
| 1920 × 1080 | **after** | **599** | **36** | **284 \| 407** |
| 2560 × 1440 | before | 599 | **1171** | 40 \| 163 |
| 2560 × 1440 | **after** | **599** | **36** | **494 \| 617** |

Three things in that table.

**The gap at 2560 was 1171 px, not the ~700 the queue carried.** Doc 12 §5.5a's
note computed *"~700"* from a 1024 px cover; the fixture's covers are 600 px,
and the field between the two columns is *everything the work cannot use*, so a
smaller cover leaves more of it. Both figures are of the same defect and the
smaller one was the optimistic reading.

**1280 × 860 is untouched — not approximately, exactly.** The band crop
`02-band-*` diffs at **0 pixels** between the builds, and the whole-window diff
is 428 px confined to the bottom bar's clock. This is the property the scale's
floor of `1.0` exists to give, and it is the one that matters most: 1280 is the
window every composition audit in this project is taken at.

**The work does not shrink anywhere.** 455 → 455 and 599 → 599. The run takes
width the record structurally cannot use, which is
`the_run_costs_the_record_nothing_where_it_is_height_bound`'s claim, here in
pixels rather than in arithmetic.

> The `air right` column reads the run's rightmost **ink**, not its column
> edge, so it is 112 px larger than the true gutter — an editable row reserves
> four trailing slots whether or not the pointer is on it. The true gutters at
> 2560 after are 494 left and 469 right, which is the pair centred to within
> the scrollbar's own lane.

### The two faults, because there were two

The owner named both edges — *"the playlist hugs right and the art hugs left"* —
and `docs/WORK.md` A4 only had the first.

**1 · the run was flat at `RUN_MEASURE` 440 at every size.** Doc 12 step A4:
`run_w = RUN_MEASURE · kiosk_scale`, keyed to the height-bound candidate for
the work so the run's width cannot depend on the record's width depending on
the run's. 440 up to a 720 px work, **472** at 1920, **692** at 2560, **1100**
at 4K, capped so the record keeps `ART_MIN` at every window above
`SPLIT_FLOOR`.

**2 · the pair hung from both edges, and every spare pixel piled up in the
middle.** The record's container was `width(Fill)` with no `align_x`; the run
was pinned right by a trailing `HANG`. So the work sat at exactly `HANG` from
the body's left edge and the slack went between them.

**A4 alone would not have fixed it.** At 2560 it takes the run 440 → 692 and
the gap 1171 → 919. The work there is bound by the **file** (599 px from a
600 px cover, `record_edge` is source-bound at that window), so none of that
field was ever the run's to give back. The centring is the other half and it is
the half that closes the gap.

The comment that defended the left hang — *"centring the work in what remains
would leave the placard's left alignment pointing at nothing"* — does not
survive reading `record_column`: the placard is `width(Fixed(edge))` and the
sleeve is `edge` wide, so the two share a left edge **with each other**
wherever the column is put. What the hang aligned the placard to was the body's
gutter, and nothing else on that surface is on it.

The pair is `edge + GAP_XL + run_w`, centred — which is
`views::page::view`'s own rule (*grow with the window until the measures cap,
then centre in what is left*) reaching the one surface that did not have it.

---

## 2 · One list, drawn once

[`08-three-lists-1280x860.png`](08-three-lists-1280x860.png) — a
record's tracks, a playlist's entries and the run's rows, each cropped at its
own column's left edge, first rows aligned.

The number lane and the title lane land at the **same offset in all three**.
The duration lanes do not, and must not: measured off the frames, a page's main
column is 566 px and the run's is 430, and a record's row ends its duration
lane 28 px in from its column's edge while an editable list's ends 112 px in —
one reserved trailing slot against four (doc 09 §8.2). Those are the two facts
the anatomy is parameterised by, not drift.

### What was actually duplicated

Three literal copies of the row — the `TRACK_NO_W` number lane right-aligned
and centred on the title's own line, the title over its second line, the
`DURATION_W` duration lane, the `GAP_SM` between them, the top alignment, the
`theme::track_row` paint and its `pad(GAP_XS, 0.0)` — in `views::album`,
`views::playlist` and `views::queue`. Now `views::page::track_row`, once.

Two copies of the record head (`playlist::record_head` and
`queue::album_group`), differing only in how they spelled *is this the first
one* — a `bool` on one side, a raw pixel count on the other. Now
`views::page::list_head`.

And **four more copies of the reserved icon slot**, in `views::queue` alone —
`step_slot`, `remove_slot`, `transfer_slot`, `lamp_dot` — byte-for-byte
identical to `views::page`'s shared ones, which the study before this one had
already de-duplicated for the two pages. The run column simply had not been
looked at.

**The refactor moved no pixels**, which is the claim a refactor has to make:
the record page and the playlist page diff at **1–3 px** between the builds
outside the bottom bar's clock, at every one of the three windows.

### What did not merge, and why

- **`DETAILS`** — format, depth, sample rate, size, folder. The owner's own
  line: *"album exploration type data"*. It is on a record's page, in the
  aside, which the run column does not have.
- **The head.** A page states a *name* (`Identity`, 80 px, three lines); the
  run states a *position* (`Run · 2 of 12 · 54:25 left`). Different sentences,
  not two spellings of one.
- **The next-track ring.** A run has a cursor, so it has a *next*; a document
  has neither. Visible in the three-list strip as the open circle on row 3 of
  the bottom list where the others carry a number.
- **The trailing slot set.** ▲▼✕ belong to an editable list; a published
  record's tracks are not one.
- **The composition.** The run column is **not** drawn through
  `views::page::view`, and this is the honest limit of the merge: `view`
  composes a centred aside-and-main document in one scroll, and the run is a
  virtualized column standing beside the record inside another surface's
  two-column layout. Same rows, same heads, same slots; a different thing
  holding them. Forcing one through the other would mean giving the run an
  aside it has no use for and taking the virtual window off a list that
  `Play all` can fill with a whole library.

---

## 3 · Two things this found that were not on the list

**A test that could not fail.**
`queue::tests::the_queue_place_is_virtual_and_its_rows_are_the_playlist_editors`
reads **its own file** and then asserts that the file contains the literals the
assertions themselves spell — so every needle was satisfied by the needle. It
had gone stale twice without failing: it looked for `window.height` after that
argument had been renamed `viewport_h`, and for a slot literal that had moved
module. It now takes the same cut `views::page::tests::pages` takes and searches
only the code half, and the two stale needles are corrected.

**A false frame, caught before it reached this document.** The first run of
`capture.sh` inherited `LIB_Y=124` / `REC1_Y=253` / `LIST_Y=509` from
`impl/one-page-two-subjects/capture.sh`. The lane has since grown a `PLAYLISTS`
block above `RECENT`, so 124 is now **Home**, and the run photographed the Home
place and saved it as `04-record-*`. It was caught by looking at the frame
rather than at the log. The coordinates in `capture.sh` are now read off a
rendered frame and the reading is written down beside them.

---

## The frames

Prefix `0` is 1280 × 860, `1` is 1920 × 1080, `2` is 2560 × 1440. Every name
carries `-before` or `-after`.

| | what it is |
|---|---|
| `01/11/21-now-playing-…` | the merged place, whole |
| `02/12/22-band-…` | the two columns' band, at identical **window** coordinates |
| `04/14/24-record-…` | a record's page — the `DETAILS` block is the named difference |
| `05/15/25-playlist-…` | a playlist's page |
| `06/16/26-band-both-…` | **the band, before over after** — the width claim in one look |
| `08-three-lists-1280x860.png` | **the three lists, stacked** — the merge claim in one look |

---

## 2026-08-12 supersession — the documented limit did drift

Section 2 accepted the run's separate top-level composition. The owner's later
same-size review found exactly the failure that boundary allowed: the two
playlist states shared row primitives but no longer looked like one component.
The virtual-window concern did not require a separate page; both states now use
the saved playlist's fixed row pitch and continue to build only a bounded
slice.

`views::playlist_page` now owns both persistence states through
`views::page::view`. The run keeps its cursor, remaining-time, Save/provenance
and next-ring semantics as capability/marker content while adopting the saved
page's sleeve, breakpoint, document, empty state, artwork and Album row
presentation. See `docs/design/impl/one-playlist-page/` for the before/after
frames and source guard.
