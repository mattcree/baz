# ADR-0036: The well has one meaning, and it says so — search off the Library, and the `×`

**Status**: accepted (2026-08-10) · **extends
[ADR-0017](0017-design-direction.md) §1.2** (type-anywhere, whose promise is the
reason a scoped well is refused) · **amends
[ADR-0030](0030-the-returns-lane-and-the-home-band.md) §2**'s well by naming its
subject in the placeholder and putting a control in its mark's box · the frames
are [`docs/design/impl/search-scope/`](../design/impl/search-scope/)

## Context

The owner, looking at the shipped lane:

> *"how the search works when we're not on the library needs to be decided.
> should it just pop to the library view when you start typing? or should it
> search whatever page you are on? maybe worth deciding as both makes sense to
> me. maybe a little x or esc to clear would make sense too"*

### What ships today, because it is half-answered already

The question is live but the first half of it is not open. When the well moved
into the returns lane it became **resident in all seven places**, and the work
that moved it gave every road to the query one destination. `App::reach_the_well`:

```rust
let there = self.go(|place| place.go(crate::lane::Destination::Library));
let open  = self.set_lane(true);
```

Every road goes through it — a printable key from anywhere
(`App::type_anywhere`), <kbd>/</kbd>, <kbd>Ctrl</kbd>+<kbd>F</kbd>, and the
collapsed lane's magnifier (`App::focus_the_well`, both bound in
`crate::keys`). So **option (a) is what ships**: typing on a playlist page today
puts you on the Library with the wall narrowed. The reasoning is recorded in
that function — *"the well searches the collection and the collection is what
answers"* — and it was written to fix a real defect, not to close this question:
before the lane, typing from Home filled a field that was not on screen and
narrowed a wall that was not either.

So the live question is **whether to add a contextual meaning beside the global
one**, not whether to build the global one.

### What the field does not do today

It does not say which of the two it is. The placeholder reads `Search` — a
promise about the *control*, not about its *subject* — and a resident field
reading `Search` while the window is showing a page called `Road Trip` is fairly
read as offering to search `Road Trip`. That is the actual defect behind the
question, and it is smaller than the question.

And there is no visible way to clear. <kbd>Esc</kbd> peels the query
(`App::escape` layer 5, `Shelf::peel`), and nothing on screen says so or offers
the same act to a pointer.

## Decision

### 1. The well keeps one meaning: it searches the collection

Unchanged behaviour, now a decision rather than a consequence.
`reach_the_well` stands: a printable key, <kbd>/</kbd>,
<kbd>Ctrl</kbd>+<kbd>F</kbd> and the collapsed magnifier all go to the Library
first, from every place.

### 2. The placeholder names the scope: `Search library`

The field states its subject, permanently, in every place — the same noun as the
`Library` destination two rows below it, which is where the query lands.

**It costs nothing, and the reason is a coincidence the design can rely on.** A
placeholder is drawn exactly when the query is empty, and the match count's
`SIDEBAR_MATCH_W` 72 slot is reserved exactly when it is not. So the placeholder
sets in the field's *resting* width — `232 − 44 − 12 = 176` px — and not in the
104 px a query gets. `Search library` measures 80.7 px in the bundled face;
`the_lanes_well_names_the_scope_it_searches` pins it, and sweeps two longer
candidates so a later edit to the word cannot silently clip.

The strip's well below `SIDEBAR_FLOOR` keeps the collection's counts as its
placeholder. It is drawn only in the Library place, so there is no other place
for it to be ambiguous against.

### 3. Contextual search is refused, and the reason is type-anywhere

Both halves of the owner's question make sense in the abstract, and the second
one loses on a hard constraint rather than on taste.

**Type-anywhere is a promise about the collection.** ADR-0017 §1.2 states it as
*any printable key filters from anywhere, with no field to click into first* —
and "filters" there means the wall. A scoped well would make the same keystroke
mean different things in different places, and — this is the part that decides
it — it would mean *the collection is no longer reachable by typing* on exactly
the pages a scope applies to. You would have to leave the page to search the
library from it. That is the distinctive gesture, revoked where the feature is
most wanted.

Three further costs, each real on its own:

- **Two live queries, or a field that empties as you walk.** The wall's query
  survives navigation on purpose (the Library keeps its scroll, its query and
  its arrangement). A scoped query on top of it is either a second live state —
  and the well is then a readout of whichever one it happens to be showing — or
  the scoped query is a place-transient, in which case the field's contents
  change under you every time you navigate. Both are worse than the round trip.
- **The peel gets a layer whose position depends on where you are.**
  <kbd>Esc</kbd>'s order is defined and short (`App::escape`): menu, panel,
  place transients, the place, the query. A scoped query slots in at layer 3 and
  the collection's stays at 5, so the same key takes one press or four to reach
  the same state depending on the place. The order is the product's one
  learnable rule about that key.
- **A second field needs a second key.** The honest scoped shape is not a second
  meaning for one field, it is a *separate* filter on the page that has
  something to filter — which is what Apple Music does, and it is defensible.
  But `Ctrl+F` and `/` are already spoken for by the well, so a page filter is
  either a keyboard-unreachable control (the first in the product) or a new
  chord for one surface.

**Which surfaces would have earned one, had the constraint not bitten:** exactly
one. A playlist's rows can run to hundreds. A record's tracks are 1–20 and an
artist's records 1–30 — a filter over either is noise. The run column is longer
but is the one list you *reorder* by dragging, and a drag onto a filtered list
has no honest drop index. Home is a fixed page of bands. The wall is the
Library's own and already has the well. One surface is not enough to buy a
control class.

The playlist filter is recorded in [`docs/BACKLOG.md`](../BACKLOG.md) with the
shape it would take, because the need is real even though this answer declines
it. **If the owner wants it, it is a field on the playlist page, not a second
meaning for the well.**

### 4. The `×`, in the mark's own box, and it is <kbd>Esc</kbd>'s pointer route

Built, as asked. Two decisions in it.

**Where it sits: the left, in the magnifier's box.** The right-hand furniture is
full and the query's room is the scarce thing. The field's right edge already
spends `GAP_MD` 12 + `SIDEBAR_MATCH_W` 72 — the slot is sized for `1284 / 1284`,
a library ten times the owner's — and a glyph box beside it would take the
query's own room from 104 px to 80, *below* the 88 that the design measured and
rejected when it moved the count into the field
(`the_lanes_well_holds_a_query_beside_its_match_count`). Sharing the 72 fails
too: `1284 / 1284` measures 67.9 px in it.

The mark's box on the left is already paid for. It is `SIDEBAR_GLYPH_BOX` 24
wide — which is `STEPPER_HIT`, a control's own box — standing on
`SIDEBAR_HEAD_GLYPH_X`, the destinations' glyph vertical. So the swap moves
nothing on either edge, which is the same guarantee the count's fixed slot gives
on the other side. **At rest the box holds the magnifier, which is a label
saying *this field searches*; with a query standing it holds the cross, which is
a control saying *press to stop*.** A field with text and a count in it does not
need to be told it is a search field.

**What it does: exactly what <kbd>Esc</kbd> does.** `Message::ClearSearch` and
the peel both call `Shelf::clear_query` — the query goes, the caret leaves the
field, and the transport gets the keyboard back (an empty field holding focus is
where <kbd>Space</kbd> types a space instead of pausing). One function, so the
two roads cannot drift; `the_wells_clear_mark_and_escape_are_one_act` pins it.

**When it is drawn: exactly while a query stands**, which is exactly when the
key has that layer to peel. No cross over an empty field. Its tooltip names the
key — `Clear the search (Esc)` — which is the icon-only law (doc 10 §3.1) doing
a second job.

Both wells draw it, from one function (`views::clear_mark`): the lane's above
`SIDEBAR_FLOOR` and the strip's below it, so the pointer route exists at every
width the keyboard route does.

## Prior art, and the trap in it

- **Spotify** makes search a destination and always global. baz already refused
  the destination (ADR-0030 §2: type-anywhere means the query is open before you
  have decided to search), but the *always global* half is what this ADR keeps.
- **Apple Music** has both: a global search and per-view filters. It is the only
  honest version of "both", and it is honest precisely because they are
  **two different controls**. Read as a licence for one field with two meanings
  it is a trap — that is not what it does.
- **foobar2000** and **MusicBee** filter the active list. Coherent, and it is
  the posture baz cannot take without giving up type-anywhere, because neither
  of them has it: both make you click into a field first, which is the cost baz
  spent a feature to avoid.

## Consequences

- Typing from a playlist page still takes you to the Library. The field says so
  before you press a key, which is the whole change to that road.
- A listener with a long playlist has no filter for it. Recorded, with its
  shape, rather than pretended away.
- The magnifier is not on screen while a query stands. It is the affordance for
  a field that has nothing in it, and a field with a query and a count in it has
  already been used.
- `×` on the left is not where the convention puts it. The convention's corner
  is occupied by a readout the design fought for, and the alternative is a
  narrower query field.

## What was considered and not taken

- **Scoping the well by place, with the placeholder naming the scope**
  (`Search Road Trip`). Self-describing and cheap in view code — and it is the
  version §3 refuses, because the view code is not where it costs.
- **Widening the field's right furniture for the `×`.** 80 px of query, below
  the 88 already rejected.
- **Sharing the count's 72 px slot.** The count's own worst case does not leave
  room for a glyph.
- **Revealing the `×` on hover.** A control you cannot see is not the visible
  route the owner asked for.
