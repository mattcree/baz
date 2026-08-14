# A little air between the lane's rows

The owner, having read the tightened lane: *"can we add a tiny bit of a gap
between items in the top sidebar and the recent history part of the sidebar.
basically make things have just a little bit of air."*

## This is not the ask that came before it, reversed

Item 39 answered *"the vertical padding on the sidebar recent list should not
be like that… there doesn't need to be any"* by taking `SIDEBAR_ROW_H` from 64
to the sleeve's own 48. That padding was **inside** the row: a `GAP_SM` above
and below the sleeve, so the card the pointer lights was 16 px taller than the
only thing drawn in it, and the list read as loosely spaced rather than as
generous.

At 48 the cards **touch**, and a column of touching cards reads as one block
the pointer cuts a slice out of rather than as a list of things.

So the two asks are about different quantities and both are right:

| | where | what it does |
|---|---|---|
| the padding that went | inside the row | makes the **card** bigger than its content |
| the gap that arrived | between the rows | leaves the card its content's size, separates neighbours |

`SIDEBAR_ROW_GAP` is `GAP_XS` **4** — the smallest step on the 4 px lattice
(law L2), which is what *"a tiny bit"* buys without reaching for `GAP_XXS`, the
ladder's one named exception. The row height is untouched.

## Both halves, one rhythm

The ask names the head *and* the recent list, so it is one rhythm rather than
two numbers: `views::lane`'s two columns both carry
`.spacing(theme::SIDEBAR_ROW_GAP)`, and `the_head_and_the_list_stand_on_one_row_rhythm`
asserts they carry the *same token* rather than measuring each. The failure
that guards against is the two drifting apart under a later edit, which would
look right in a screenshot of the head and wrong in a screenshot of the lane.

## The pitch the virtualization counts against

`SIDEBAR_ROW_PITCH` = 48 + 4 = **52** is declared because
`App::request_offscreen_art` counts lane rows against a pitch to decide which
covers to ask for. A pitch that read the row's own height would drift by the
gap once per row — four rows down the list it would be requesting the wrong
art. The drawn pitch and the counted pitch are one number.

## What was verified how

`head-with-air.png` — the lane's head at 2×, isolated Xvfb. The four
destinations are visibly separated and the `Library` card is exactly the 48 px
row, not swollen: the air is between the cards, which is the whole distinction.

**The `RECENT` half is verified by test rather than by eye, and that is worth
saying rather than glossing.** The lane's recency is session-scoped — it is
built from what was touched in this run, not read from the play ledger — so a
headless run with no confirmable playback (there is no audio device) shows an
empty list, and seeding `history.tsv` does not populate it either, because the
ledger only supplies the *order key*. What holds that half is the shared token
and the test above, not a render.
