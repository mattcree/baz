# ADR-0031: The picker is a card at the pointer

**Status**: proposed (2026-08-09) · extracts the decisions of
[`docs/design/13-everyday-flow.md`](../design/13-everyday-flow.md) §6 ·
sibling of [ADR-0030](0030-the-returns-lane-and-the-home-band.md), which
removes the panel this picker used to live in · **amends ADR-0024 §6 layer
1**: the two-press pick is unchanged in count and changed in place ·
changes no message, no engine command and no ordering — `picker_order` moves
verbatim · adds one float and two tokens · the owner's brief, verbatim:
*"when you click to add something to a playlist currently, it shows a
playlist thing, and there's a very minor tip at the very top of the
playlist window that indicates you need to click on a playlist to add it to
it. I mean, it makes sense to some degree. But is there a better way to do
it?"*

## Context

He is describing `views/playlist_panel.rs:122–129`, which renders
`Add “Violet Ledger” — pick a destination` at **`SIZE_META` 12** in
**`paper_dim`** — under a `Playlists` heading at `SIZE_EMPHASIS` 15 Medium
and level with `Esc closes`. **The panel's only statement of what it is
now for is quieter than its own title.** The frame is
[`impl/everyday-flow/03-picker-hint-1280x860.png`](../design/impl/everyday-flow/03-picker-hint-1280x860.png).

The copy is the smaller half. Three defects are in the same frame, and none
of them is a wording problem:

1. **The destination is thrown across the window.** The panel is anchored
   to the window's right edge whatever the gesture was
   (`app.rs:3216–3218`). At 1280 its rows begin at x 963; the tile
   right-pressed in the frame is centred at x 444. **≈ 682 px** of pointer
   travel to the first destination, against **≈ 127 px** for the second
   press of a context menu.
2. **That distance is set by the window, not by the gesture.** At 1920 the
   same pick costs 640 px more. A gesture whose cost rises with the size of
   the display has not been designed.
3. **The surface is enormously larger than the task** — 340 px at full
   window height to offer three destinations — and while it stands it
   covers the index rail and the density detents.

The prior art is one-sided (doc 13 §10.6): **four of five surveyed products
put the destination list at the pointer**, and none throws it to the far
edge of the window. On this operation the owner's instinct that Spotify is
state of the art is correct.

## Decision

### 1. The card

> **A pick opens a card at the pointer, headed by the sentence, holding the
> destinations and nothing else.**

```
        ● pointer
        ┌─ PICKER_W 280 ────────────────────────────┐
        │                                           │ GAP_SM 8
        │ Add “Violet Ledger” to…                   │ LINE_BODY 20   SIZE_BODY, paper
        │ 9 tracks · 45:26                          │ LINE_META 16   SIZE_META, dim
        │                                           │ GAP_SM 8
        ├───────────────────────────────────────────┤ 1  hairline
        │                                           │ GAP_SM 8
        │ ▫ 48  Queue                   8 · 32:10   │ 64
        │ ▫ 48  Road Trip — playing    14 · 51:08   │ 64
        │ ▫ 48  Late Nights            23 · 1:40:11 │ 64
        │                                           │ GAP_XS 4
        │    New playlist                           │ 32
        │                                           │ GAP_SM 8
        └───────────────────────────────────────────┘   = 297 for three
```

| Token | Value | Derivation |
|---|---:|---|
| `PICKER_W` | **280** | `SIDEBAR_W`'s number exactly (ADR-0030 §2) |
| `PICKER_MAX_H` | **400** | five rows plus chrome, on the lattice; beyond it the rows scroll with the heading and `New playlist` pinned outside the scroll |

The row is the lane's row — `SIDEBAR_ROW_H` 64, `SIDEBAR_SLEEVE` 48 — and
that is the point: **a list looks like itself wherever it appears**, so the
destinations on the card are visibly the same objects as the rows in the
lane.

### 2. Placement and dismissal are the menu's, exactly

`menu::anchor`'s rule (`menu.rs:359–372`): top-left corner at the pointer,
flipped to the pointer's other side at any edge it would cross, clamped as
a last resort. One shared function, so the two floats cannot disagree about
where an edge is. `Esc` peels the card **before every other layer**; a left
press outside puts it down and is never a spent click; a right press
outside falls through. ADR-0016's verified mechanics, `opaque`, **no
scrim**.

### 3. What the card says

- **`Add “{label}” to…`** at `SIZE_BODY` 13 in full paper, Medium: the
  verb, the object, and an ellipsis promising the list below. A surface
  whose whole reason for existing is a question states the question at the
  size its own name would have taken.
- **`9 tracks · 45:26`** — what is in the hand, in figures. The shipped
  picker never states it, and it is what distinguishes *this record* from
  *this track* when the label alone is ambiguous.
- **The rows lose their `Add` word** (`playlist_panel.rs:227–232`,
  `:369–374`), which was the design compensating for a heading nobody read.
  With the heading carrying the verb, a row is a destination and says so by
  being one.

### 4. What does not change

- **The order.** `playlists::picker_order` (`playlists.rs:526–531`) moves
  **verbatim**, with its tests: the **Queue** first, the **current
  playlist** hoisted and marked *playing*, the folder's order, `New
  playlist` last. It was kept in the model *"so it is a tested fact rather
  than a rendering accident"*, and the accident it guards against is now a
  different renderer.
- **The gesture count.** Two presses, as ADR-0024 §6 layer 1 specified and
  doc 09 §11 budgeted. This record removes **550 px of pointer travel**, a
  window-width dependence and a 340 px surface — not a press. A gesture's
  cost is not only its count.
- **The two-message menu items** (`Queue`, `Queue album`,
  `Add to "{current}"`) never *showed* the picker; they make both presses
  themselves (`menu.rs:189–190`, `:194–199`). Unchanged.
- **`New playlist`** turns the row into the name field the panel already
  has (`playlist_panel.rs:273–296`), storage refusals surfaced in it in
  their own words; submitting creates and completes the pick.

## Consequences

- One float — `views/picker.rs` — on `menu::anchor`/`extent`'s geometry and
  `app.rs:3236–3249`'s stacking; two tokens; `App::escape` gains a layer at
  its head.
- **A test worth having**: `no_transfer_gesture_opens_a_panel`, swept over
  every message that reached `Playlists::begin_pick`. Under ADR-0030 there
  is no panel to open, and this pins the intent as well as the fact.
- `Playlists::pending` leaves the panel's view entirely; `Playlists::peel`
  loses its middle layer.
- The picker no longer covers the index rail or the density detents,
  because it is 280 × ≤ 400 at the pointer rather than 341 × full height at
  the window's edge.

## Deliberately not done

- **An `Add to playlist ▸` submenu.** `menu.rs`'s *"No submenus"* stands:
  a submenu needs hover-to-open and a safe triangle, which is a timing
  affordance; Material caps nesting at *"one level deep"*; Spotify's own
  submenu needed a search field inside it to survive real libraries; and
  the `+` slots are not menus, so baz would ship two destination surfaces.
- **A modal dialog.** No dialogs in the product, no scrim by refusal, and
  Apple's own guidance is to minimise modality. The card is an *action
  sheet* by the HIG's own distinction — *"use an action sheet, not a menu,
  to provide choices related to the action people initiated"* — which is
  exactly what a pick is.
- **A search field inside the card.** Deferred rather than refused
  (doc 13 §11.17): the hoist plus the folder's order answers it until
  someone has more lists than `PICKER_MAX_H` holds, and a second text field
  in a float is a focus-and-dismissal problem worth solving when someone
  has it.
- **Fixing only the copy.** It is the fallback if this record is refused,
  and it leaves all three defects standing.
