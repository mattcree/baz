# `Artist › Album`, and the Artist place

Frames from the real binary, rendered headless on a private Xvfb with all six
XDG redirections from [`docs/DEVELOPMENT.md`](../../../DEVELOPMENT.md). Nothing
touched the owner's session; `capture.sh` prints the run's own
`[mpris] no session bus` line as the receipt, quoted at the foot of this page.

Reproduce with [`capture.sh`](capture.sh); the header comment carries the
toolbox build line.

## What the owner asked for

> *"previous and next on albums doesn't make sense on the album view. we could
> add an Artist > album breadcrumb though. and have an artist page."*

Three instructions in one line, and they are one change: **the record page's
header stops naming the kind of page you are on and starts naming where the
record sits.**

## Why the stepper was wrong, in one paragraph

`‹ Prev` / `Next ›` shipped the same day, implementing doc 07 §3.2's ruling
that the Album place *must* carry a step to the next and previous record so
that comparing two releases stays one press per release. It stepped along **the
wall's current arrangement** — its group key, its query, its sort — and none of
that is on screen from a record's page. So the pair offered two labelled doors
whose destination the listener could not know before pressing them. That the
implementation was *correct* is what hid it: the pool really was the visible
set, the edges really were inert, a filtered-out record really did have no
neighbours. The code did what the document asked; the document asked for the
wrong thing. The full reversal is recorded in
[`11-jobs-era-critique.md`](../../11-jobs-era-critique.md) §5 P3, and doc 07
§3.2 is amended in place rather than merely disobeyed — the clause that
survives verbatim is *"not nothing"*.

**The debt is real and this pays it better.** A record's context is its artist,
which is a fact about the record rather than about the frame, and every record
you reach through it is one you saw before you chose it.

| | frame |
|---|---|
| **The record's page**, leading with `Anne-Marie Puig › Ochre` where `Album` and the stepper pair used to be. | [`01`](01-album-page-with-the-breadcrumb.png) |
| **The artist half is a door** and says so under the pointer, in the product's own word-button ground. The `›` is punctuation and is not pressable; the album half is not either, because you are already there. | [`02`](02-the-artist-half-is-a-door.png) |
| **The Artist place**: their name, `2 records · 21 tracks`, and their records in **the wall's own tile** — the same `views::shelf::tile` with the wall's own `Grid`, so the sleeve, the caption, the playing mark and the hover options are the wall's to the pixel. | [`03`](03-the-artist-place.png) |
| **A second record, chosen from a page that showed it first** — the comparison the stepper was trying to buy. | [`04`](04-a-second-record-from-the-artist-page.png) |
| **And back up**, from that record's own breadcrumb. `Ochre → artist → Violet Ledger → artist` is four presses and none of them lands anywhere you could not see. | [`05`](05-and-back-up-to-the-artist.png) |

## Measured

The one geometric claim, checked by [`measure.py`](measure.py) against the
shipped frames rather than against the source:

```
  Album  (leads with the breadcrumb — a button): hairline at y = 48
  Artist (leads with the name — a word)       : hairline at y = 48
  Album  (a second record, from the artist)   : hairline at y = 48

  the header strip is 48 px on both places the breadcrumb joins — the press is not a jump
```

The Album place leads its strip with a **button** (`TRANSPORT_HIT` 32) and the
Artist place with a **word** (`LINE_EMPHASIS` 20), so without care the strip
would be twelve pixels taller on one side of a press than the other — across
exactly the press this feature is about. `views::artist` gives its lead the
same 32. Both are the *same* strip function (`place_header_led`), which is what
keeps them honest; `one_gutter_touches_every_window_edge` pins that neither
place grew a header of its own.

The lead's **left edge** is the other half of it. The door carried `GAP_SM` of
its own padding at first, which put the artist's name eight pixels right of
where the Artist place puts it — visible in the first pass of these captures,
and law L1's *"the frame's left edge is unchanged"* broken by a hover ground.
The door now has no horizontal padding and the word's own box is what lights.

## One defect these captures caught

The first pass showed a record's **hover options open on the Artist place**,
for a record the pointer was nowhere near. `Shelf::hovered_album` is set by
`TileEntered` and cleared by `TileLeft` — and `TileLeft` is published by a
`mouse_area` the pointer actually *leaves*, so navigating out from under the
pointer never delivers one. The mark survived the place change.

It was invisible while the wall was the only surface drawing tiles, because
coming back put the pointer where it had left it. Home's `RECENTLY ADDED` row
made it possible and the Artist place made it obvious. The hovered tile is now
cleared where the open menu and the in-flight drag already were — one rule:
*what was about the place you left does not follow you* — and
`navigating_leaves_no_tile_under_a_pointer_that_moved_on` pins all three
together.

## What the Artist place deliberately does not hold

Each for a reason rather than for want of room: a biography or any critic
metadata, and an artist image (both would come off the network, and nothing in
baz goes to the network); play counts and every other engagement statistic
(ADR-0030 §6 refused those from Home, and the argument does not change with the
surface); and a flat list of every track they appear on, which is the Library's
search one press away and would be ADR-0017 §1.7's *"albums listed as albums,
never flattened"* broken on a page whose whole subject is records.

## The receipt

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```
