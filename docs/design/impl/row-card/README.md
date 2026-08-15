# The card reaches the row's controls — item 53

The owner, 2026-08-15: *"can we make sure the playlist row controls are inside
the highlighted row as well."*

## What was wrong

A track row's highlight was painted by the row's **button**, and the button is
only the row's *body* — the number, the title, the artist line, the album
column, the duration. Every surface that draws one then hangs its controls off
the side of it: a heart on all four, the transfer `+` where a pick can land,
and ▲ ▼ ✕ on a list you can edit. So a hovered row lit up to the duration and
stopped, leaving two or four unlit controls on bare wall beside a lit card.

## What it is now

The card moved out of the button and into a container that holds the whole
row — `page::row_card`, drawn from `theme::track_row_card`, which is
`selectable_track_row` in container form and asserted equal to it
(`theme::row_card_tests::a_rows_card_paints_what_its_button_used_to`).

**Nothing about a press changed.** The body keeps its own press, each control
keeps its own, and no control was nested inside another control's bounds — the
alternative fix, and the one that would have made a single press mean two
different things depending on which pixel it landed on.

The button now paints nothing at all (`theme::track_row_body`), which makes the
obvious future mistake silent: a new surface that draws a row and forgets the
wrapper would simply never light. So the wrapper is asserted over the source,
the way the ground argument already is —
`views::tests::every_track_row_is_wrapped_in_its_card`, five surfaces in the
walk.

## What it cost

Two of the five surfaces had no idea where the pointer was. A row cannot ask
whether its *sibling* is hovered — a style function learns its own status and
nothing else — so the shell holds the one answer, which is the mechanism
`hovered_queue_row` has used since the queue's ✕ was reserved. Favourites
gained `hovered_favourite_row`; the new-playlist draft gained
`CreationDraft::hovered_row`.

**Both new pairs are enter/left messages carrying the row, not a
`Hovered(Option<usize>)`,** and the first version of this change got that
wrong. Moving from row 3 to row 4 delivers row 4's *enter* before row 3's
*exit*, so an exit that cleared the state unconditionally unlit the row the
pointer was actually on. The existing three surfaces already guarded it
(`if self.hovered_playlist_row == Some(row)`); the two new ones now do, and so
does the vibe preview's own hover, which had the same latent crossing.

## The frames

Shot by `capture.sh` against the real binary at 1280 × 860, headless on a
private Xvfb with all six XDG redirections; the run's `[mpris] no session bus`
line is the receipt that the owner's session was untouched. In each one the
pointer is **resting** on a row rather than pressing it.

| | |
|---|---|
| `01-record-row-hovered` | A record's page, pointer on track 3. The lit card runs past the duration, under the heart and the transfer `+`. |
| `02-playlist-row-hovered` | A saved playlist's page — the surface the owner was looking at. The card now holds four controls: heart, ▲, ▼, ✕. |
| `03-favourites-row-hovered` | Favourites, pointer on row 2. The place had no hover answer at all before this. |

The measurement, rather than the impression: at x = 1200 — past the duration
lane, in the controls' own lane — the hovered row reads `srgb(20,21,23)` and
its neighbours read `srgb(12,13,14)`, on all three frames. That is
`Palette::step_up` over the wall, which is exactly what the body used to paint
and stop.

**A capture note worth keeping.** A page's row artwork arrives *after* the page
does, and the reflow moves the row out from under a stationary pointer: iced
re-evaluates a `mouse_area`'s bounds on cursor movement, so the row publishes
its exit and never its re-entry. The script parks off the list, lets the
thumbnails land, and only then moves onto the row. It cost a wrong diagnosis
first — the state was correct and the picture was not.

The run also marks three favourites by **pressing the hearts** on a record's
page, which is the shortest available proof that a control under the new card
still presses.
