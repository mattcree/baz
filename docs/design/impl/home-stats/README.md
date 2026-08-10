# The counts leave the well: `COLLECTION` on Home, the match count in the field

Rendered from the real binary under Xvfb with the six XDG redirections of
`docs/DEVELOPMENT.md`; `capture.sh` regenerates every frame here and prints the
`[mpris] no session bus` receipt that says the owner's session was not touched.
Nothing was audible: the sink discards every sample and the fixture's samples
are all zero.

The owner's brief, verbatim: *"the album and track count below the search bar
doesn't look good… maybe this should go into the home as some basic stats?"*

---

## What was decided

The well shipped with a quiet line under it carrying two strings —
`25 albums · 206 tracks` at rest, `12 of 25 albums` while narrowing. **They were
never one readout.** They have different subjects, so they get different homes:

- **The resting counts are a statistic about the collection.** Nothing is being
  searched while they are on screen, and they were standing in the lane's most
  valuable space — the block directly above `RECENT`, which is the surface's
  whole point. They are **Home's `COLLECTION` footer** now.
- **The match count is feedback about the query**, so it stays with the field
  that answers it — and it goes **inside** it, right-aligned. It does not go to
  Home, and not because of taste: while you are narrowing the collection, Home
  is not on screen.

---

## The frames

| Frame | What it shows |
|---|---|
| `00-lane-before-1280` | **Before.** The well over its readout line, `25 albums · 206 tracks`. Same fixture, same lists, same records played — this is `capture.sh` run against the binary one commit earlier. |
| `00-well-before-1280` | The same well, cropped. |
| `01-lane-at-rest-1280` | **After.** One control tall, placeholder `Search`, and one more `RECENT` row on screen. |
| `03-well-at-rest-1280` | The well at rest, cropped: a field and nothing else. |
| `02-lane-mid-query-1280` | `an`, with **`16 / 25` inside the field**, right-aligned. The first `RECENT` row has not moved. |
| `04-well-mid-query-1280` | The same well, cropped — the query, the gap, the count. |
| `05-home-with-stats-1280` | Home: `CONTINUE`, `RECENTLY ADDED`, and `COLLECTION` closing the page. |
| `06-collection-footer-1280` | The footer, cropped: `25 ALBUMS · 13 ARTISTS · 206 TRACKS · 17 hours OF MUSIC`. |
| `19-lane-before-1920` / `20-lane-at-rest-1920` | The same before/after pair at 1920 × 1080 — **10 rows against 11**. |
| `21-lane-mid-query-1920` | Mid-query at 1920. |
| `22-home-with-stats-1920` | Home at 1920, where the footer sits under a five-tile `RECENTLY ADDED`. |

---

## The row the lane gets back — measured, and not where the arithmetic said

`SIDEBAR_WELL_H` falls from **52** (the field at `TRANSPORT_HIT` 32, `GAP_XS`,
one `LINE_META` readout line) to **32**. The head is 20 px shorter and the list
starts 20 px higher: first row at **y = 221** where it was **y = 241**, in every
frame at both windows.

Twenty pixels against a 64 px row pitch is *three eighths of a row*, so whether
it buys a whole one depends entirely on where the remainder was sitting.
`measure.py` counts sleeves down the lane's left flank in both frames of each
pair:

| Window | Whole `RECENT` rows, before | after | of the next row |
|---|---:|---:|---|
| 1280 × 860 | 7 | **7** | 8 px → 28 px |
| 1920 × 1080 | 10 | **11** | 36 px → 0 |

**The recovered row is at 1920 × 1080, not at 1280 × 860.** ADR-0030's second
amendment and `docs/design/13-everyday-flow.md` §9.2 both said the opposite —
that the well cost a row at 860, taking 7 to 6 — from the formula
`(H − 83 − 48 − head − 25 − 36 − 48) / 64`. The shipped frame from *before* this
change holds **7** rows at 860, so that formula is about 24 px out somewhere
between the bar and the foot marks. Both documents are corrected, and the
before-frame is committed here so the correction is checkable rather than
asserted.

What the 20 px does buy at 860 is real but partial: the eighth row goes from an
8 px sliver to a 28 px one, which is the difference between a row you cannot see
and a row you can see is there.

---

## The match count, back inside the field

```
the strip's well                        280
  − text inset (12 + 16 + 8)             36
  − reserved MATCH_W slot (12 + 88)     100
  = the query's own lane                144

the lane's well  (SIDEBAR_MEASURE)      232
  − text inset (SIDEBAR_HEAD_TEXT_X)     44
  − reserved MATCH_W slot (12 + 88)     100
  = the query's own lane                 88     ← what drove both figures out
  − reserved SIDEBAR_MATCH_W (12 + 72)   84
  = the query's own lane                104     ← what brings one back
```

The lane keeps a **72 px** slot rather than the strip's `MATCH_W` 88, and that
is the whole difference. 88 was sized for `40000 / 40000` because the strip
could afford it; 72 holds `9999 / 9999` — a collection ten times the owner's —
measured in the bundled face by
`font.rs`'s `the_lanes_well_holds_a_query_beside_its_match_count`. Above 9 999
records the figure clips inside its own box rather than running left under the
query, because the box is fixed and clipped.

**The form is `3 / 25`**, which is the identical string the strip's own well has
drawn all along. The pair rather than the bare figure: inside the control being
typed into, the query is the count's subject, so it needs no caption — and the
denominator is what turns *three* into *three of a small collection*, which is
what a match count is for. It is albums, because the collection being narrowed
is a collection of records; the `Songs` section states its own count in its own
heading.

**Nothing moves when the first character lands.** Three separate reasons, all
kept from the design the readout line used:

1. The reservation is on the **right** and the query sets from the left, so the
   caret and the character under it do not shift.
2. The slot is a **fixed width with the figure right-aligned**, so `16 / 25`
   becoming `3 / 25` changes in place.
3. The well's block is a **fixed height** in both states, so no `RECENT` row is
   pushed down. `measure.py` checks the third against the frames: first row at
   `y = 221` at rest and mid-query, at both windows.

---

## `COLLECTION`, and why it is a footer

```
COLLECTION
25          13          206         17 hours
ALBUMS      ARTISTS     TRACKS      OF MUSIC
```

A figure at the emphasis size over a tracked word at the section heading's, on a
96 px lattice (`STAT_W`). No card, no rule, no colour, nothing pressable — it is
the page's own two smallest voices stacked, which is a footnote with structure
rather than a dashboard tile.

**It is last on the page.** Home's job is to put you back into music: `CONTINUE`
is the one thing here you press and `RECENTLY ADDED` is a row of records you can
start from, and neither may be pushed down by an inventory. The figures are
something you read once in a while — *how big has this got* — and a fact you
consult occasionally goes where the page ends. It is also the only section here
that is pure statement, so leading with it would be leading with the part you
cannot use.

**It reads as a sentence, not a table**: *25 albums, 13 artists, 206 tracks,
17 hours of music.* That is what picked the four, and it is what cut three
others — each for a reason rather than for room:

- **When the collection was last added to.** `RECENTLY ADDED` is drawn one
  section above and says it with covers. One fact drawn twice is doc 07 L8.6's
  test, and the section that already passes it keeps the fact.
- **Records never played.** A figure about the *listener*, read out of the play
  ledger, and it changes while you sit looking at it. ADR-0030 §6 refuses every
  engagement statistic; this is the one on the list that is easy to mistake for
  an inventory fact, so it is named here rather than quietly dropped.
- **Size on disk.** True, cheap and dull — a fact about a filesystem, and
  nothing a listener would act on. The record page's `Details` block is where
  bytes belong.

**This does not reopen §6's refusal.** Every figure here describes what you
**own**, and every one would be identical if the application had never been
opened.

Three notes on honesty in the figures themselves:

- **Artists** are *named album artists, case-folded*. `Various` and `Unknown`
  are the two answers the view model gives when there is no artist to count, and
  counting them would put "we could not read this" into a figure about people;
  the folding is the wall's own `ARTIST` arrangement, so a library that spells
  one band two ways is not told it has two.
- **Tracks** is the library's own count — the same 206 the retired readout
  stated, so it is the figure the owner already recognises. An album owned in
  two formats is one record and two sets of files, which is what it is.
- **Playing time** is one unit, largest first, and it sums only the durations
  the scan actually read. A track with no readable duration contributes nothing
  rather than an estimate — the same rule the `CONTINUE` needle keeps.

---

## What it costs at rest

Three of the four figures are a walk over every track, and ADR-0030 §4 forbids
paying that per frame. So `vm::Collection` is counted in
`Shelf::rebuild_shelves` — where the albums it counts are built — and held as
four scalars: **one pass per rebuild, zero per frame**, which is the same shape
as the ledger fold §4 already licenses. Home adds no subscription, no clock and
no timer, exactly as before.
