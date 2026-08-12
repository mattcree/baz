# ADR-0022: Places, and nothing else — removing every side surface

**Status**: accepted (2026-08-08), **interaction amended 2026-08-12** · **supersedes the surface model of
[ADR-0016](0016-places-inspector-popover-bar.md)** (its inspector, its popover
and the four-kinds rule; its queue vocabulary and its `Esc` layering survive
re-stated) · spends the deferral [ADR-0017](0017-design-direction.md) costed as
*"a change of breakpoint, not a rewrite"* and takes its steps 15 and 16 in one
move · deletes two of [ADR-0020](0020-motion.md)'s five transitions by removing
their subjects · changes no engine command and no protocol message · spends
[ADR-0014](0014-queue-editing.md)'s `JumpTo` and `UpdateQueue` unchanged

---

## 2026-08-12 amendment — selection no longer navigates

The owner replaced the mixed tile/row grammar with one desktop content rule:
an ordinary first click selects and visibly highlights an album, playlist or
playable row; a second click on the same object inside the double-click interval
activates it. Activation means play for an album/list and needle-drop or jump
for a track row. Explicit labelled `Play` controls remain direct one-click
commands, as do explicit `Open` controls and named navigation links.

This restores a `Selection` state, but not ADR-0016's selected-album side
surface. It is one session-scoped content key plus one click clock and changes
no layout: selecting a sleeve does not open its page, resize the wall or move
the second press's target. Album and playlist detail remain reachable through
their explicit `Open` option, context menu and source/navigation links. Enter
activates the current selection when no query stands; the search amendment
owns Enter while a query is active. Space retains its global play/pause meaning
outside text entry rather than becoming a second content-activation key. iced
0.13 exposes no platform double-click setting, so all surfaces share the
existing 400 ms desktop interval until the 0.14 migration can consume toolkit
click counts. Touch uses the same repeated press state machine, and the first
tap also leaves a tile's labelled Play/Open veil visible so neither action
depends on hover.

Everything below is the historical place decision. Its one-place model and
removal of side surfaces stand; claims that a tile's first press navigates are
superseded by this amendment.

---

## Context

The owner's verdict, on the model ADR-0016 shipped:

> *"I really hate the way queue and selected albums appear… I hate the sidebar…
> the fact that the alphabet bar has a scroll to its left isn't nice either"*

This is the **second** rejection of the same shape. The first was against the
model before it:

> *"an example of a strange UI is the two side panels we have now. that seems
> unreasonable"*

ADR-0016 answered that one by reducing three rail tenants to one and moving the
queue to a float. The reduction was right about the rail and wrong about the
conclusion: what was rejected was not *how many* side surfaces there were. It
was that there were any.

### What the rail cost, and what the survivors still cost

ADR-0016's own ledger against the three-tenant rail — a dismissal model that
needed a paragraph, frequencies two orders of magnitude apart, the wrong tenant
paying the width, a panel simultaneously too narrow and too empty, and a
double-click broken by the reflow the click itself caused — is all correct, and
none of it was fully paid off by cutting to one tenant plus a popover:

| what the rail cost | after ADR-0016 | after this |
|---|---|---|
| two of five cover columns, on a press | **340 px on a press**, for one tenant | nothing — the wall's width is the window's |
| a reflow between the two presses of a double-click | still there, repaired by a 400 ms `GridHold` | **no reflow exists**; the hold is deleted |
| a dismissal rule per tenant | four gestures for the inspector, four for the popover | one: `Esc`, or `‹ Library` |
| `Ctrl+B` conjuring content out of an empty rail | `Ctrl+B` offering the *playing* album | **`Ctrl+B` is unbound** |
| a state machine arbitrating three tenants | `Selection` (2 fields) + `Overlay` (1) + `Place` (1) | one enum, four members |
| a 340 px column that a soundtrack showed 3 of 12 tracks in | **the same 340 px**, one tenant | the window |

The last row is the one the evidence kept missing. ADR-0016 defended the album
as a column on the grounds that *the browse loop needs the shelf beside it*.
That is true of the loop and false of the surface: a 340 px column is not a
place to read a record in, and the composition audit measured what it actually
became — a sleeve at **93.6 %** of the panel's contrast-weighted ink with the
album's own name **fifth of eight**. The fix at the time was to cap the sleeve
at 120 px, which made the hierarchy right and the *column* no better.

### ADR-0017 anticipated this exactly

Its "biggest trade-off" section kept the album as a side column, said a
full-window view *"is where this should eventually go"*, and made the deferral
cheap **on purpose**:

> the `< 940 px` regime where the inspector replaces the shelf **is** the
> full-view code path, so promotion later is a change of breakpoint, not a
> rewrite.

That claim is now tested. It was **half right**, and the half it got wrong is
the interesting half — see §4.

---

## Decision

> **The window holds one PLACE at a time, and the now-playing BAR is in every
> one of them.**

One kind, four members, and nothing else on screen. There is no inspector, no
popover, no rail and no float.

| Place | What it is | Door | Back |
|---|---|---|---|
| **Library** (home) | The wall, its search, its arrangement, its counts | — | — |
| **Album**(id) | One record's page: art, identity, `Play album`, its tracks, its condition | a tile; the bar's now-playing block | `Esc`, `‹ Library`, the same tile again |
| **Queue** | What the engine holds and where it is in it | the bar's labelled `Queue`; <kbd>Ctrl</kbd>+<kbd>U</kbd> | `Esc`, `‹ Library`, the door again |
| **Settings** | Everything that is a standing decision | the top bar's `Settings`; <kbd>Ctrl</kbd>+<kbd>,</kbd> | `Esc`, `‹ Library`, the door again |

Four rules, and they are the whole model a listener has to learn:

1. **A place fills the window.** Places replace each other; two are never on
   screen together.
2. **The bar is in every place**, unchanged, and it is the only thing that is.
3. **`Esc` goes back.** One press, one meaning, everywhere. In the Library it
   falls through to the wall's own layers (the pull's offer, then the query,
   then the shuffle pool's marks).
4. **A place change is a hard cut.** See §5.

`crates/baz/src/selection.rs` and `crates/baz/src/overlay.rs` are **deleted**.
`Place` gains `Album(u64)` and `Queue`. `Ctrl+B` is unbound.

### The wall's scrollbar goes, and the rail carries position

The owner's third complaint is two vertical strips doing one job. **The index
rail is the scroll affordance for a shelved wall**: it says where you are (the
current shelf at full paper in the Medium face), it jumps, it drags — and it
does the one thing a scroller cannot, which is *name the place it will take you
to*. A grey bar 8 px to its left says nothing the rail does not say better.

So the wall's `scrollable` asks for a zero-width `Scrollbar`. **iced 0.13 does
suppress the bar while keeping the scroll behaviour** — a zero-width bar is a
bar the widget lays out and paints nothing for; the wheel, the touchpad, drags
and every programmatic `scroll_to` are untouched. This is the same primitive the
inspector's reveal viewport used and it was verified against `iced_widget`
0.13.4 before being specified, so **no fallback is needed and none is
proposed**. `theme::wall_scrollbar` is the geometry; `WALL_SCROLLBAR_W == 0.0`
is asserted beside `SCROLLBAR_W > 0.0`, because every *other* list in baz keeps
its bar — none of them has a rail beside it, and a page with neither has no
readout of how much of it there is.

> **Amended, 2026-08-09 and 2026-08-10 — the bar came back, and then it moved.**
>
> The owner reversed this section the day he used it: *"can we allow there to be
> a scroll bar for any view? Just a very minimal scroll bar because otherwise,
> it's hard to just jump to the end"*. The reasoning above is right about what
> the rail *does* and wrong about what it leaves out: the rail jumps to shelves
> by group key, and **the end is not a group key** — under `ARTIST` the last
> shelf may be `Z` or `#`, and under a filter it is whatever survived. So
> `WALL_SCROLLBAR_W` is `theme::RAIL` 4, not 0, and the "two vertical strips" rule
> now reads that the second strip must be the *lesser* mark and must not repeat
> what the first one says. It says nothing; it is a handle.
>
> **Where it goes is this ADR's business too**, because the lane it stands
> beside is the rail's. It shipped at the right edge of the wall's scrollable,
> which put the rail's `INDEX_LANE_W` 108 *outboard* of it: measured at
> 1280 × 860, a bar at x 1168–1171 with the window's edge at 1280 — *"scroll
> bar is in a strange location… it seems to have padding on the right"*. It is
> now drawn on the **window's** edge, x 1276–1279, in the outer 4 px of the
> rail's own window gutter, where there is no ink. **The rail's lane, its
> letters' `W − HANG` edge and the density detents at its foot (ADR-0028 §1)
> are untouched to the pixel** — the reservation moved, the type did not.
>
> The cost is the rail's Fitts win. Its press band ran to the window's edge on
> purpose, so a fling at the edge always hit it; it now stops 4 px short and
> those 4 px are the bar's. Taken deliberately: the band is still 104 px wide,
> and what the screen edge now hits is the other scroll affordance for the same
> wall rather than nothing. `docs/design/impl/wall-scrollbar/` holds the
> before/after frames, the ruler and the alternatives that were refused.

### Getting back to what is playing

`docs/design/03-interface-prior-art.md`'s **R3** is band A in the study: *get
back to what is playing*. Every one of the sixteen products surveyed spends an
affordance on it; baz has never had one. ADR-0016 deliberately reserved the
now-playing block for it and gave the queue the labelled control beside it,
"resolved on purpose rather than by whichever landed first".

Removing the last persistent surface that knew which record was under the lamp
turns that reservation from a courtesy into a requirement. **The now-playing
text is now the control that opens the sounding record's page.**

Two doors, side by side, both labelled, two subjects: *the text is the record,
the word `Queue` is the queue*. Neither is a bare gesture and neither is an
icon. The accessibility refusal binds and is honoured: the control is visible,
it is pointer-reachable, its label is the name of the thing the press leads to,
its tooltip is the verb (*"Go to the record that is playing"* — the accessible
name in waiting, iced publishing no tree), and it is **not offered when nothing
is sounding**, because a control that cannot act must not pretend it can.

It is a pointer target 56 px tall against law L7's floor of 32. The law sets one
height for a control that is a *box*; a control that is a block of type is
bounded below by the same number rather than exempt from it, and the assertion
is in `views::bottom_bar`. Becoming a control moved nothing: `theme::now_playing_text`
is the one button style in the product with a **0 px** border, because every
other one carries a transparent 1 px edge to hold its geometry across states and
a 1 px edge here would make a 56 px block 58 in a band derived from 56.

### The bar, re-derived — 57 → 81

Mid-flight the owner added: *"proportion is becoming an issue e.g. bottom bar is
too short"*. He is right, and the arithmetic says why: the needle's work took
the band to **56**, and the left zone's three line boxes are 20 + 16 + 20 = **56**.
The type filled the bar edge to edge. Every token was correct and the
proportion was not.

**The lane count was re-argued before the height.** Three lanes stay: the
continuation (`then 2 albums · 1:58:00 left`) earns its rung *because* of this
ADR — reading what is next used to cost a popover that reflowed nothing and now
costs leaving the wall, so the ambient line is the only free reading of the
queue baz has, and it became more valuable at the exact moment the bar became
shorter.

**Then the height follows from the content plus a stated lead:**

> A band's content may not touch the band's edges. The lead is a **named gap**
> on each side — never a ratio, because a constant ink-to-band ratio is not
> reachable on the 4 px lattice for two bands of different content heights, and
> a lead off the lattice is law L2 broken to make a proportion true.

| | tallest zone | lead | band | ink-to-band |
|---|---:|---:|---:|---:|
| top bar | 32 (a control row) | `GAP_SM` 8 | 48 | 0.67 |
| bottom bar | 56 (a type block) | `GAP_MD` 12 | **80** | **0.70** |

One rung more for the bottom bar because a hit box carries its own internal
padding and a stack of line boxes carries only its leading — 3.5 px above the
title's ink and 2.5 below the continuation's, which is what read as cramped.

**And the band lands on `2 × HANG`.** That is why 80 and not the 72 one rung
below it: `HANG` is the product's one structural unit — the window gutter, the
wall label's height, the shelf header's band, the clear wall between two rows —
so the bar is measured in the same unit as the collection above it rather than
in a number of its own, and *both* of its leads come out as named tokens
(`BAR_ZONE_LEAD` = `GAP_MD` 12 for the type, `BAR_LEAD` = `GAP_XL` 24 for the
transport) rather than as pixels chosen to look right. Every figure in the band
is a token that already existed.

| | before the needle | after the needle | now |
|---|---:|---:|---:|
| band | 104 | 56 | **80** |
| bar (band + hairline) | 105 | 57 | **81** |
| bottom furniture (+ needle) | 105 | 59 | **83** |
| of an 860 px window | 12.2 % | 6.9 % | **9.7 %** |
| of a 1080 px window | 9.7 % | 5.5 % | **7.7 %** |
| collection's share at 1280 × 860 | 82.1 % | 87.4 % | **84.7 %** |

The needle bought the wall 46 px; this spends 24 and keeps 22. That is the
minimum that buys real air on the lattice: 72 (an 8 px lead) is defensible and
64 (a 4 px lead) is not air at all. **81 is reachable and 80 was never going to
be** — a bar is `2ℓ + 32 + 1`, odd for every integer lead, which is the same
parity fact that made ADR-0017's "58" ship as 57.

`BAR_LEAD` is now **derived** — `(BAR_CONTENT_H − TRANSPORT_HIT) / 2` — rather
than chosen, so law L4's centre line is true by construction rather than by an
assertion somebody has to keep re-checking, and the transport row states the
lead as padding rather than borrowing the row's centring.

---

## The album's page, and why it is a re-lay rather than a re-parent

A 340 px column and a 1200 px page are not the same composition at two sizes.

```
┌──────────────────────────────────────────────────────────────┐
│ ‹ Library   Album                    Esc returns to the wall │
├──────────────────────────────────────────────────────────────┤
│  ┌───────────┐   TITLE                              (hero 28)│
│  │           │   Artist                            (title 19)│
│  │  320 × 320│   1992 · 13 tracks · 45:35 · FLAC 16/44.1     │
│  │           │                                               │
│  └───────────┘   ──── TRACKS ────────────────────────────────│
│  [ ▶ Play album ]  1  Airbag                            4:44 │
│  [ FLAC | MP3 ]    ●  Paranoid Android                  6:23 │
│  ──── DETAILS ──── ...                                       │
│    Format  FLAC                                              │
│    …19 more                                                  │
└──────────────────────────────────────────────────────────────┘
```

**Two columns.** Left, fixed at the sleeve's own edge: the object, the one thing
you can do to it, and its condition report. Right: who made it, what it is, and
every track on it. One scroll for the page — the column had two and the popover
had one inside another; a page is one document and turning it over is one
gesture.

**The page grows with the window until its track list reaches `LIST_MEASURE`
880, then stops and centres.** One rule, one cap, shared with the queue place:
below it the body hangs from the window's two gutters (law L1); at and above it
the body's own two edges are what the surface declares (law L5). A row whose
title is at one end of 1800 px and whose right-aligned duration is at the other
is two words, not a row, and the ruled right edge `DURATION_W` buys stops
meaning anything at that distance. 880 is the same number — and the same
argument — as `SETTINGS_CONTENT_MAX`: one measure in the product, not two.
Below `ALBUM_BREAKPOINT` 744 the columns stack, that being exactly the width at
which the list stops being wider than the sleeve beside it.

### The declared hierarchy, and the sleeve being first

> **the work ≫ `Play album` → the title → the artist → the catalogue line →
> the track list → the condition** — and among *type*, the title is first.

The work is first **by declaration**, the way the wall's sleeves are, and law L6
requires the declaration to say by how much. This is *not* the audit's defect 5
returning. That defect was not "a large sleeve"; it was **a second, larger copy
of a work already on the wall 24 px to the left**, drowning the one thing the
panel added. A place has replaced the wall: there is no other copy on screen,
and the record is the subject.

Measured off the rendered frame with the committed rulers
(`docs/design/impl/places/`), contrast-weighted ink mass **per line** — which is
loudness, where total mass is quantity and would rank a twelve-row list above a
one-word title for the trivial reason that it is twelve lines:

| | per line | |
|---|---:|---|
| the work (sleeve) | 13 322 759 | 88.5 % of the page's total ink |
| `Play album` | 178 767 | a 1 px amber border round 320 × 32 |
| **the title** | **135 544** | **first among type** |
| the artist | 90 063 | |
| the catalogue line | 77 335 | |
| a track row | 65 035 | |
| a `Details` row | 35 725 | |

`Play album` outranking the title is not an inversion: it is 704 px of
full-contrast accent against five glyphs, and it is the one commitment the page
makes. The line the audit's defect was about holds — the album's own name went
from **fifth of eight** in the column to **first among type** on the page, by a
clear step (`SIZE_HERO` 28 over `SIZE_TITLE` 19 over `SIZE_META` 12, each at
least a quarter again as large as the one under it, with the title at the top of
the whole scale).

`ALBUM_SLEEVE` is `ART_MAX` 320, which is `art::THUMB_PX` — so the refusal *no
artwork is ever drawn larger than its source* is satisfied at the boundary
rather than approached, and the decoded thumbnail is drawn 1 : 1.

`Details` moved **above the fold**. In the column it rode below the track list
in the same scroll — the honest arrangement for 340 px, and it made the block a
page you had to reach. Beside the sleeve it is simply there, at every shipped
width, which is what `docs/design/03-interface-prior-art.md` R6 actually asked
for: *fooyin shows twenty fields for free and baz showed four, and baz's
audience came from products in the first camp.*

### The queue's page

The same shape, one column: the header strip, the summary that reads **what is
left** (`3 of 12 · 38:12 left`, MusicBee's one-list-with-a-cursor named rather
than resembled), a header per record because **albums are listed as albums,
never flattened**, the playing row carrying the lamp dot in place of its number
in a column that never changes width, click-to-jump (`JumpTo`) and a per-row ✕
(`UpdateQueue` through the pure `queue_edit`). Every fact and every gesture the
popover had, at 880 px instead of 360.

The ✕ still appears on hover and its slot is still reserved either way — a
column of permanent crosses down a list of what you are about to hear is a
column of invitations to destroy something, and a slot that came and went would
slide every duration sideways as the pointer crossed a row.

---

## What the prior-art evidence said, and why it did not survive contact

`docs/design/03-interface-prior-art.md` is the strongest evidence baz holds
against this decision, and it is worth stating at full strength rather than
quietly not citing:

- **Every cataloguing product studied runs a right-hand inspector.** Calibre,
  Lightroom Classic, Apple Photos, and fooyin's Selection Info. It is the
  consumption products that run a full page.
- **R12** asked to *reduce* the inspector's dismissal gestures from four to two
   — an argument for keeping it, tuned.
- **W15**, *compare two releases*, is named as **Marta's actual loop, and no
  music player supports it**, with the inspector-follows-selection pattern as
  the answer. The study is explicit that below 940 px, where the shelf is
  replaced, "comparison becomes back-and-forth" — i.e. it names this ADR's cost
  in advance and calls it a break.

Three things are true at once and the third decides it.

**First, the evidence is about a pattern, not about a width.** What Calibre and
Lightroom run is *an inspector at a width where an inspector works*: Calibre's
is user-resizable and routinely 400–500 px, Lightroom's right panel likewise.
baz's was 340 px with a 292 px content lane, and the composition audit measured
what that produced — 8 distinct x-edges, 5 of them singletons, a track list
showing 3 of 12 rows on a soundtrack, and a hierarchy so inverted the album's
name ranked fifth of eight in its own panel. **We adopted the pattern's shape
and not its size.** The prior art supports a column that baz was never actually
shipping.

**Second, the study's own §5.3 warns against exactly the reversal we then made.**
R11 records three vendors that bought visual calm by removing control density
and reversed within two years; the lesson is *do not remove a stated fact*. This
ADR removes no fact. Every reading the inspector and the popover published — the
lamp dot in the number column, the summary that says what is left, the encoding
line, the disc headers, the per-row ✕, the click-to-jump, the pull's note — is
on screen, in the same voice, in the same row geometry. What is removed is the
**container**, and the container was 340 px of the collection.

**Third — and this is the part evidence cannot settle — the same owner has now
rejected the same shape twice.** The study surveyed sixteen products and ran
three headless; it did not survey this listener. A cataloguer's inspector is a
strong prior about a population, and a direct rejection is data about the user.
When a prior and an observation disagree, the observation wins, and it wins
harder the second time.

So the evidence is not overturned. It is **relocated**: it argues for the album
having a rich, resident, high-density surface, and baz now has one that is three
and a half times wider, which is where the twenty `Details` fields the same
study asked for finally fit.

---

## What ADR-0017 got right about the cost, and what it got wrong

Its claim: *"the `< 940 px` regime where the inspector replaces the shelf is the
full-view code path, so promotion later is a change of breakpoint, not a
rewrite."*

**Right about the wiring, wrong about the composition.** The shell change *was*
a change of breakpoint — the album's content already had one code path, and
`app.rs`'s composition went from a three-way `row!` to a `match` arm. The
deferral being cheap is real and it is why this landed in one increment rather
than three.

But the `< 940 px` regime was specified as *the inspector taking the content
area with a shelf strip beside it* — i.e. **the column at a different width**,
not a page. A column stretched to 1200 px is a 292 px lane with 900 px of
whitespace, or a track row with its duration a thousand pixels from its title.
Everything in §2 above — two columns, the hero title, the sleeve at source size,
`Details` above the fold, the measure and the centring — is composition that had
to be *designed*, not re-parented, and none of it existed in the `< 940 px`
path. ADR-0017's own §5 amendment is the honest frame for this: *a redesign is
cheap; a product change is not, and calling it a redesign does not make it
cheap.*

---

## Motion

Two of ADR-0020's five transitions **lose their subject**:

- **§2.2**, the queue popover's 140 ms fade and 8 px rise — there is no popover.
- **§2.4**, the album inspector's 150 ms width — there is no column. **The
  inspector-width tween dies with the rail**, and with it `motion::PANEL`,
  `Shelf::panel`, `panel_album`, the reveal viewport, and the `GridHold` that
  existed to keep the wall still while the column arrived.

Neither is *forbidden*; ADR-0020 is not reversed, and if either surface ever
returns its number returns with it. They are removed because a `Duration`
constant nothing reads is worse than a paragraph saying why, and the paragraph
is in `motion.rs`.

**A place change is a hard cut**, and that is a decision rather than an
omission. It is not argued into one of the five: the surfaces either side of a
navigation share no element to move, so any transition between them would be
*decoration*, which ADR-0020 §3 forbids by name. Three of the five ship — the
icon-button ink, the shelf tile's hover, the lamp warming — and there is no
sixth.

The needle, the bar's L4 spread and every reserved slot in the bar survive; the
bar's **height** is re-derived above and its spread is re-derived with it, to
**0 px** against the law's ceiling of 2.

---

## Consequences

### What is now worse

Stated plainly, because a decision that only lists its wins is a sales pitch.

- **Comparing two records is a round trip.** wall → album → wall → album, where
  the inspector made it two clicks and no navigation. This is prior-art W15,
  *Marta's actual loop*, and it is the single biggest thing this costs.
  **Mitigation**: the wall keeps its scroll, its query and its arrangement
  across every navigation (nothing about the Library is reachable from `Place`),
  and it **marks the record you last opened** with the 2 px rule the selection
  used to carry — so the return leg is *return* and not re-find. `Esc` is one
  press. It is a real mitigation and it is not a substitute; comparing two
  sleeves is now four gestures where it was two.
- **Double-click-to-play from the wall is gone.** The first press navigates, so
  there is no tile left for the second to land on. Playing a record from the
  wall is now two presses (open, then `Play album`) where it was two presses of
  one gesture. **Mitigation**: `Play album` is a 320 × 32 target in a fixed
  place under the sleeve, where the gesture it replaces had a 400 ms window and
  a documented failure mode; `Shuffle` and `Pull` still start sound from the
  wall without leaving it. The friction budget's *intent → sound = 1 click* is
  **not met from the wall** and this ADR does not pretend otherwise.
- **Knowing what is queued costs the wall.** The popover reflowed nothing;
  the place replaces everything. **Mitigation**: the bar's third line states the
  continuation ambiently, so the place is for *changing* the queue rather than
  reading it — which is why that lane earned its keep in the bar's
  re-derivation above.
- **The wall gives 24 px back to the bar**, from the 46 the needle won it.
- **`Ctrl+B` is unbound**, and a reflex from every editor written this decade
  now does nothing. Left unbound deliberately: a key that survives a redesign
  pointing at a new meaning is worse than one that stops.
- **There is no history stack.** Album → Queue → `Esc` lands on the wall, not on
  the album. `Place::back` is total and argument-free by design; a history that
  could land you somewhere you did not navigate from is the rail in a different
  shape.

### What is unreachable that was reachable before

- Seeing a record's detail **and** the wall at the same time. That is the
  decision, not a side effect.
- Seeing the queue **and** the wall at the same time.
- Hiding a surface while keeping its contents (`Ctrl+B`'s reversible dismissal).
  Every dismissal is now a close; there is nothing to keep.
- Dismissing the queue by clicking outside it, and by its ✕. Both were dismissal
  gestures for a float; a place has `Esc`, `‹ Library`, and its own door.

### What got simpler, in code

- **Two pure modules deleted** (`selection.rs`, `overlay.rs`) and one grown by
  two enum members. `Place` is now the whole surface model, and "an album page
  with no album" and "two popovers at once" are states that do not exist to be
  got into rather than states walked exhaustively to prove absent.
- **`Shelf::grid_width` has two terms**: the window, less the index rail's lane.
  No press anywhere in the product can re-hang the collection. The reflow, the
  width tween, the panel's lagging album, the grid hold, the double-click
  detector and the `ColumnHoldTick` subscription all existed to make a re-hang
  survivable; none of them has anything left to do.
- **`Esc` is one `if`.** It was one per layer.
- **The shelf's virtualization sweep lost a dimension**: it ran over
  `[0, PANEL_W]` because a press could take 340 px off the wall, and now runs
  over the window's band alone. Same band, reached only by dragging an edge.
- `theme.rs` loses `PANEL_W`, `POPOVER_W`, `POPOVER_MAX_H`, `INSPECTOR_SLEEVE`,
  `popover_pad`, `panel`, `popover` and the whole `fade*` family (the one alpha
  baz drew on purpose was the popover's arrival); it gains `LIST_MEASURE`,
  `ALBUM_SLEEVE`, `ALBUM_ASIDE_W`, `ALBUM_BREAKPOINT`, `SETTINGS_CONTENT_MIN`,
  `NOW_PLAYING_H`, `BAR_ZONE_LEAD` and `WALL_SCROLLBAR_W`.

### The four must-not-regress properties (`01-ux-audit-and-ia.md` §5)

All four keep a test, three of them simplified by the model rather than by
weakening the claim:

1. **The bar reserves every slot it can be in** — re-checked at the new band,
   with the now-playing block's own reservation (`NOW_PLAYING_H`) added.
2. **The shelf virtualizes at every width** — swept at 1 px over 640…2560, minus
   the inspector dimension.
3. **Every keyboard binding resolves to a message an on-screen control also
   sends** — and **the one exception is gone**: `Ctrl+B` was it.
4. **No reachable state shows an inspector without an album** — now structural:
   the album id is *in* the place.

### Accessibility

Unchanged in what iced can offer and improved in what baz offers. R3 gains a
visible, labelled, pointer-reachable control where there was none. No action in
the product is keyboard-only and no control's only affordance is hover — the
queue row's ✕ is the one hover-revealed control and it is not the only route to
its action (`UpdateQueue` is also what a queue ending does). The declared gap
stands: iced 0.13 publishes no accessibility tree, buttons take no keyboard
focus, and the wall cannot be arrow-navigated.

---

## Deliberately not done

- **A back stack.** See above.
- **A shelf strip beside the album's page** (ADR-0017 step 16's `< 940 px`
  regime, inverted). It would be a side surface, which is the thing that was
  rejected twice.
- **Restoring one-click-to-sound from the wall.** The candidates — a bare
  `Enter` on the wall, a play affordance on the tile — are respectively a
  keyboard-only capability and a mark drawn on a sleeve, and both are refused
  already. If it returns it should return through the **stack** (ADR-0017 step
  13's shift-click), which is a queueing gesture rather than a second meaning
  for a press.
- **Marquee**, which is where the pull's note belongs; it borrows the record's
  page for now, one line of type in the ordinary voice, exactly as it borrowed
  the inspector.
