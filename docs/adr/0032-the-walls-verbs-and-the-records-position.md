# ADR-0032: The wall's verbs, and the record's position

**Status**: **§2 and §4 superseded by the owner's decision (2026-08-09)**;
§1, §3 and §5 stand as proposed · originally proposed (2026-08-09) · extracts the decisions of
[`docs/design/13-everyday-flow.md`](../design/13-everyday-flow.md) §4 and
§5 · adds one context-menu item, one readout, one token and one string ·
**no new message and no new control** — every press already has a named
visible twin in `menu.rs`'s mirror table · §4 carries **one open question
for the owner**, drawn and priced rather than decided · the owner's brief,
verbatim: *"starting to play an album… should not need two clicks. I wonder
if when we mouse over, we can just show two options somehow… send all to
current playlist. or play now… or just view details"*

> **What the owner decided, recorded here so this document is not read as
> current.** §2 refused a hover-revealed verb group on the wall, on the
> measurement that a card does not fit *beside* a tile. §4 drew a `Ctrl`-click
> accelerator and left it open for him. He rejected the modifier outright —
> *"burying things in modifier keys is not great"* — and approved a mockup
> that puts the verbs **inside the tile's own bounds**, which is the one
> placement §2's own closing paragraph observes baz does not have. Four
> options over a veil that gathers at the sleeve's left edge and dissolves
> before the right one; `Play` sounds the record in one press. The
> measurements in §2 are not wrong and are not withdrawn — they are about a
> card floating beside a tile, which is not what shipped. the product's standing rules
> carries the rewritten entries and
> `docs/design/impl/hover-options/` carries the render evidence. This ADR is
> left otherwise unedited: it is a record of what was decided when, not a
> description of the product.

## Context

Three of the owner's requests about the wall, and they have three different
answers.

**The verbs already float beside the record you point at.** Right-press a
sleeve and `Open · Play album · Queue album · Add to playlist…` opens at
the pointer, edge-flipped, with the wall untouched behind it
([`impl/everyday-flow/02-tile-menu-1280x860.png`](../design/impl/everyday-flow/02-tile-menu-1280x860.png)).
Two gestures, no navigation — better than the flow being complained about
on every axis except that the reveal is a right-press and nothing on the
wall says so.

**One verb is genuinely missing.** `Add to "{current}"` exists on track
rows, queue rows, playlist-page rows and the bar's now-playing block
(`menu.rs:192–201`, `:214–223`, `:285–291`, `:313–321`) and **on no album
object at all**. *Send all to current playlist* cannot be done today.

**The hover reveal does not fit, and that is a measurement rather than a
principle.** The wall's clear space between one row's state rule and the
next row's sleeve is the step's hang less `RULE_LANE_H` 6
(`shelf.rs:1037`): **42 / 34 / 22 px** at Spacious / Balanced / Dense.
A three-item card is `3 × TRANSPORT_HIT + 2 × GAP_XS` = **104 px**
(`menu::extent`); a four-item card 136. It overshoots at every step, and
laterally `MENU_W` 232 against a 283 px tile pitch covers most of a
neighbouring record. The flattest legal alternative — one row of words at
L7's height — is 32 px, which does not fit at Dense at all and cannot carry
`Add to "Road Trip"` in the 80 px three verbs would get at art width. Every
surveyed product that reveals a play affordance draws it **inside the
object's own bounds** (doc 13 §10.4), which is the one placement baz does
not have.

## Decision

### 1. `Add to "{current}"` joins the tile menu

Between `Queue album` and `Add to playlist…`, in doc 09 §5.2's table order,
present exactly while playing provenance names a `.m3u8` that still exists
(`playlists::holds`, `playlists.rs:440–445`) — **absent, not disabled,
otherwise**.

```
Open                                     the tile's own press
Play album                               the page's Play album
Queue album                 Shift-click  the picker's Queue row
Add to “Road Trip”                       the card's hoisted row      ← new
Add to playlist…                         the page's Add to playlist…
```

Its presses are `AddAlbumToPlaylist(id)` then `PickPlaylist(id)`, **both
already in the mirror table with a named visible twin**
(`menu.rs:586–615`), so `every_menu_item_is_a_press_some_control_also_makes`
passes with no new `CONTROLS` row.

**The semantics, stated because the field gets this wrong.** The append
goes to the **file**; the live queue is untouched (doc 09 §6's decoupling,
in both directions). *Keep it* is `Add to "Road Trip"`; *hear it tonight*
is `Queue album`; doing both is both gestures. Plexamp's `Play Next` and
`Add to queue` doing the same thing is a defect reported for years
(doc 13 §10.6), and two verbs that claim different things and do the same
thing are worse than one.

Five items is `5 × 32 + 8` = 168 px of card; the edge flip already holds
any height inside the window (`menu.rs:359–372`, tested at `:761`).

**No resident control on the record's page.** The aside holds `Play album`
and `Add to playlist…`; a third word-act naming a file that may not exist
next frame is a control that comes and goes, to save one press on a route
that already exists.

### 2. No hover-revealed verb group on the wall

On the measurements above. Recorded in doc 13 §11 with the numbers, so a
proposal with an answer to the geometry meets an argument rather than a
wall. Two further reasons specific to the obvious shortcut — *open the
existing menu on hover* — are that the card is `opaque` and captures
presses (`app.rs:3236–3249`), so the wall's next click would be spent
dismissing something nobody asked for; and that a menu which opens itself
is no longer an accelerator layer over visible controls, which is its whole
licence.

The wall's hover vocabulary is therefore unchanged: a 1 px → 2 px rule
under the label and one rung of ink on the artist line, over the 90 ms
tween ADR-0020 §2.3 budgets.

### 3. The step pair states the position it already computes

```
‹ Library   Album   ‹ Prev  ·  4 of 25  ·  Next ›       Esc returns to Library
```

- A **readout**, not a control: no message, no press. L8.3's escape valve in
  its ordinary direction — the fact goes where it is watched, the controls
  stay where they are.
- In the header's **existing optional-tenant slot**
  (`place_header_with`, `views/mod.rs:245–258`), so the frame stays one
  function in five places.
- The figures are positions in the wall's **current visible order** — the
  same list `vm::neighbours` steps (`vm.rs:1095`) — so a filtered wall says
  `2 of 7` and **the scope is stated by being counted**.
- Reserved `POSITION_W`, sized for `99 of 9999` at `SIZE_META` in the
  bundled Medium and **asserted against its measured word**, the shape
  ADR-0026's as-shipped note §2 forced into the open — a declaration under
  the measurement is a budget the law cannot honestly assert. Stepping from
  `9 of 25` to `10 of 25` moves neither door.
- **Absent when there is nothing to say**: a record the wall no longer shows
  has no neighbours (`vm.rs:1092`, tested at `:2496–2503`), its doors are
  already inert, and it states no position rather than `0 of 0`.

This is what remains of the owner's *"see the depth that you've went
into"* once ADR-0030's lane delivers the master pane he was describing.
baz's navigation tree is **one level deep from every place**, so there is
no depth to draw; what was missing was position within the level above.

### 4. One press to sound from the wall — **open, for the owner**

Not decided here. The candidate that fits every constraint the product
actually has is a **modifier press on the tile meaning `Play album`** — the
construction already shipped for *sound-later* (shift-click queues the
record, `app.rs:1054–1061`), pointed at *sound-now*:

```
Play album                  Ctrl-click   ← the printed accelerator
Queue album                 Shift-click
```

It draws nothing on a sleeve, reveals nothing on hover, and has no timing
window — the press's meaning depends on a key that is down or not, which is
a state the hand chose rather than one the clock chose. It has a visible
twin twice over, so it is taught exactly where shift-click is taught.

**Against**: three meanings for one press on the product's most-pressed
object; `Ctrl`-click is the platform's add-to-selection chord and the wall
may one day have a selection; ADR-0022 pointed any return of one-press
sound at the shift-click stack rather than at a second meaning for a press.

**For**: W1 *put on an album* is band A and is the product's home intent;
one-press sound-now exists at every scope in the product **except a single
record**, which is the unit the wall is made of; and Nielsen Norman's
finding that *"contextual menus are not appropriate for actions users rely
on frequently"* (doc 13 §10.8) says the tile menu cannot be the answer for
a band-A action — which leaves this candidate or the two-press price,
permanently.

**Cost if taken**: one arm in `Message::AlbumClicked`
(`app.rs:1054–1061`), one accelerator string, and a sibling for
`shift_click_queues_the_record_and_nothing_sounds_unasked`
(`app.rs:5460`). the product's two-press entry would be **narrowed,
not deleted**: no timing gesture, no keyboard-only route and no mark on a
sleeve buys the press; what falls is the enumeration's implicit claim to be
complete.

### 5. The teacher

The reveal gesture is taught nowhere on the wall. The teacher goes where
the person has just paid the cost it would have saved — the record's page,
reached by a tile press, in the header note lane that is one quiet meta
line today:

```
‹ Library   Album   ‹ Prev · 4 of 25 · Next ›     Right-click a sleeve to play it from Library
```

One string, one lane that exists, in the voice the product already teaches
in (`Enter plays the first match.`, `Esc clears the search.`, `When a queue
ends, baz stops.`). Deliberately modest: a fourth kind of teaching surface
for one gesture is the tour doc 11 P6 refused.

## Consequences

- One arm in `menu::items`' `Target::Album` branch; the mirror test passes
  unchanged, and the item-order assertion at `menu.rs:662` gains a sibling
  for the tile.
- One token (`POSITION_W`) with its measured-word assertion in `font.rs`;
  `album::step_pair` takes `(index, of)` from the `vm::neighbours` call it
  already makes.
- Two strings, both governed by the existing room-vocabulary sweep
  (`no_room_vocabulary_ships_in_user_facing_copy`).
- **No new message, no new control, no engine change.**

## Deliberately not done

- **A hover-revealed verb group**, and the context menu opened on hover
  (§2).
- **A play affordance drawn on the sleeve** — the form every surveyed
  product uses, and the one baz has never drawn.
- **A resident `Add to "{current}"` on the record's page** (§1).
- **A tooltip on every wall tile** teaching the right-press: a thousand
  objects wearing chrome to carry one sentence.
- **A breadcrumb, column view, or back stack** — doc 13 §5 measures the
  tree at one level and doc 13 §10.7 carries the guidance that excludes
  exactly this case.
