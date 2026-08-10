# `A–Z`, `ARTISTS`, and the wall's second subject

> **Superseded the same day, and kept because it was shipped.** The owner saw
> this and said *"artists should be grouping stuff by artist not just
> alphabetically"* — a wall of records **grouped under their artist**, which
> satisfies ADR-0019 §1 exactly and is therefore an ordinary group key. So the
> sixth word, the subject, `A–Z` and the artist tiles below are all gone; what
> replaced them is at
> [`../artists-grouped/`](../artists-grouped/README.md), and
> [ADR-0035](../../../adr/0035-the-wall-has-a-subject.md) is the one decision
> both of these are forms of. These frames are the record of what `main`
> carried for an hour, not of what baz does.

Frames from the real binary, rendered headless on a private Xvfb with all six
XDG redirections from [`docs/DEVELOPMENT.md`](../../../DEVELOPMENT.md). Nothing
touched the owner's session; `capture.sh` prints the run's own
`[mpris] no session bus` line as the receipt, quoted at the foot of this page.

Reproduce with [`capture.sh`](capture.sh); the header comment carries the
toolbox build line. The decision is [ADR-0035](../../../adr/0035-the-wall-has-a-subject.md).

## The defect, in one sentence

baz had **two things called artist**: the wall's `ARTIST` group key, which
sorts records by their album artist's initial — its shelves read `Unknown`,
`#`, `A`, `C`, `Various` and its rail is the alphabet — and the **Artist
place**, whose subject is a person. One word, one screen, two meanings.

## What changed

| | frame |
|---|---|
| **The first word is `A–Z`.** It names what the key produces; `NAME` would still read as a subject and collide again. `GroupKey::code()` is still `"artist"` — it is on-disk config data. | [`01`](01-arrangement-row-1280.png) |
| **`ARTISTS` is the sixth word in the same row**, in the same voice: one of a closed set of six, one of them current. Not a sixth group key, not a lens — see the ADR. | [`03`](03-arrangement-row-artists-1280.png) |
| **The artists wall**: one tile per person the collection is filed under, shelved by initial, with the alphabet rail beside it. No key is lit while it stands, because the keys arrange *records* and no record is being arranged. | [`02`](02-artists-wall-1280.png) · [`21`](21-artists-wall-1920.png) |
| **The collage, both its rules on one screen.** Kesh holds six records and takes the 2 × 2 of the first four; Marguerite Vance-Lindqvist holds one and takes it full-bleed. This is `views::playlist_sleeve` — the same function, the same cache, the same gradient while a decode is in flight — not a second collage that could drift. | [`04`](04-artist-collage-1280.png) · [`05`](05-artist-tiles-1280.png) |
| **The readouts follow the subject.** `10 / 11` counts artists, not the 25 records behind them, and the strip's own well reads `11 artists · 206 tracks`. A figure counting albums beside a wall of people would be a readout describing a surface that is not on screen. | [`06`](06-artists-mid-query-1280.png) · [`07`](07-artists-match-count-1280.png) · [`22`](22-artists-mid-query-1920.png) |
| **The rail is the alphabet rail, verbatim.** `K` is one press because `rail::entries` is a pure function of the shelf headers and this wall's headers are `Initial`s. No branch, no new vocabulary, no state. | [`04`](04-artist-collage-1280.png) |
| **The round trip.** Leave the records on `YEAR`, visit the artists, press `YEAR` again. | [`08`](08-year-wall-before-1280.png) · [`09`](09-year-wall-after-the-artists-1280.png) |

## The round trip, measured

`08` and `09` are the same YEAR wall with a visit to the artists between them,
and `capture.sh` diffs them:

```
  YEAR before vs after the artists: 0 differing pixels
```

**Zero.** That is the whole argument for holding the subject beside
`group_key` rather than as a sixth key: nothing on the way out could have
moved the arrangement, because the arrangement is not what the sixth word
changes.

## The strip, re-measured

The sixth word cost the arrangement row 54 px — `ARTISTS` in its box plus the
`GAP_MD` beside it is 77.49, and the first key's rename from `ARTIST` to `A–Z`
gave 23.98 back — so `KEYS_W` is 314 → **368** and every figure derived from
it moved. The costing kept in [`docs/BACKLOG.md`](../../../BACKLOG.md) measured
this against a strip whose acts cluster was still 182 px wide and concluded
that **the single-line-with-well band would cease to exist**. It did not:
`Pull` and `Shuffle` left that cluster in between and paid for the word twice
over.

| | the backlog's costing | shipped |
|---|---:|---:|
| `KEYS_W` | 368 | **368** |
| `LIBRARY_LINE` | 654 | **560** |
| the window's own minimum | 750 | **696** (unmoved) |
| `TOP_BAR_SPLIT` | 926 | **832** |
| `SINGLE_LINE_NO_WELL` vs `WIDEST_LANE_STRIP` 720 | 702 (18 px spare) | **608** (112 px spare) |
| the single-line-with-well band | *deleted* (926 > 904) | **832…904, alive** |

| | frame |
|---|---|
| **The band that was predicted not to exist.** A 928 px window leaves the strip exactly `TOP_BAR_SPLIT` 832 — the widest window at which the strip still draws the well on one line. | [`10`](10-strip-single-line-with-well-928.png) · [`11`](11-strip-band-928.png) |
| **And the window's own minimum**, `TOP_BAR_FLOOR` 600 plus the lane's rail 96, where the strip is two lines: the frame's furniture above, the library's six words and its one act below. Nothing hides, nothing overflows, no menu appears. | [`12`](12-strip-at-the-window-floor-696.png) · [`13`](13-strip-two-lines-696.png) |

The whole budget is const arithmetic in
`theme.rs::the_strip_holds_its_tenants_at_the_single_line_floor`, and the words
are measured against the declaration in the bundled face in
`font.rs::the_strips_declared_tenant_widths_hold_their_measured_words`. The six
words measure **366.50 px** against a declared 368 — 1.50 px, the tightest
reservation in the strip, and the number a seventh word would have to beat.

## One thing the frames caught by accident

The 1920 run opens on the **artists**, because the two strip runs before it
left the subject there and the scratch config carried it across three launches.
`capture.sh` puts the records back before `20` for that reason, and the comment
says so — the subject is persisted on `group_key`'s exact terms, and this is
what that looks like from outside.

## The fixture

`mkfixture.sh`'s own 25 records / 206 tracks, with **four of them re-filed
under one artist** by `capture.sh`. Every artist in the stock fixture holds two
records, which draws the collage's one-to-three rule and never the 2 × 2; a
six-record artist is what makes `04` show both rules at once.

## The isolation receipt

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```
