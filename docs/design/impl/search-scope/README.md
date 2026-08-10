# The well's one meaning, and the `×`

Rendered from the real binary under Xvfb with the six XDG redirections of
`docs/DEVELOPMENT.md`; `capture.sh` regenerates every frame here and prints the
`[mpris] no session bus` receipt that says the owner's session was not touched.
Nothing was audible: the sink discards every sample and the fixture's samples
are all zero.

The owner's brief, verbatim:

> *"how the search works when we're not on the library needs to be decided.
> should it just pop to the library view when you start typing? or should it
> search whatever page you are on? maybe worth deciding as both makes sense to
> me. maybe a little x or esc to clear would make sense too"*

The decision is [ADR-0036](../../../adr/0036-the-wells-one-meaning.md).

---

## First: half of it already ships

Worth stating before anything is argued, because the code answers it. When the
well moved into the returns lane it became resident in all seven places, and the
work that moved it gave every road to the query one destination —
`App::reach_the_well`:

```rust
let there = self.go(|place| place.go(crate::lane::Destination::Library));
let open  = self.set_lane(true);
```

A printable key from anywhere, <kbd>/</kbd>, <kbd>Ctrl</kbd>+<kbd>F</kbd> and
the collapsed lane's magnifier all go through it. **So option (a) is what the
product does today**, and frame 10 is it happening. The live question was only
ever whether to *add* a contextual meaning beside the global one.

## What was decided

**One field, one meaning: it searches the collection.** And two changes, which
are the whole of the build:

1. **The placeholder names the scope** — `Search library`, in every place,
   permanently. The field never said what it searched, and a resident field
   reading `Search` while the window shows a page called `Road Trip` is fairly
   read as offering to search `Road Trip`. That is the real defect the question
   was pointing at, and it is smaller than the question.
2. **The `×` sits in the mark's own box**, on the left, where the magnifier is
   at rest — because the field's right edge is already spent on the count's
   reserved slot, and the query's own room is the scarce thing.

**Contextual search is refused**, on a constraint rather than on taste:
type-anywhere (ADR-0017 §1.2) is a promise *about the collection*, and a well
scoped to the page would make the collection unreachable by typing on exactly
the pages a scope applies to. The one surface that would genuinely have earned a
filter — a long playlist's rows, frame 08's `Road Trip · 60 tracks` — is costed
in [`docs/BACKLOG.md`](../../../BACKLOG.md) as a **second control on that page**,
which is the only honest shape for it.

---

## The frames

| Frame | What it shows |
|---|---|
| `01-well-at-rest-library-1280` | The well at rest on the Library: `Search library` in the placeholder lane, the magnifier in the mark's box. The word sets in the field's **resting** 176 px, because a placeholder is drawn exactly when the count's 72 px slot is not reserved. |
| `02-well-mid-query-the-x-and-the-count-1280` | Mid-query. The `×` standing where the magnifier was, `16 / 25` unmoved in its reserved slot at the other edge, the query between them at the same 68 px inset. |
| `03-mark-box-at-rest-1280` · `04-mark-box-under-a-query-1280` | The mark's box alone, in each state. |
| `05-the-swap-stacked-1280` | The two, stacked — one image of *nothing moves*. Both marks stand on 44, which is `SIDEBAR_HEAD_GLYPH_X`, the destinations' own glyph vertical, and both are drawn at the same `GLYPH_OPACITY` 0.57. |
| `06-the-x-hovered-1280` | The `×` under the pointer: the transport's own hover wash and the tooltip `Clear the search (Esc)`, which names the key it mirrors. |
| `07-the-x-pressed-clears-like-esc-1280` | Pressed. The query gone, the wall back, **and the field unfocused** — which is `Esc`'s behaviour exactly, because it is `Esc`'s function exactly (`Shelf::clear_query`). An empty field holding the caret is where <kbd>Space</kbd> types a space instead of pausing. |
| `08-a-playlist-page-the-well-names-its-scope-1280` | The question's own frame: `Road Trip · 60 tracks · 5:53:18` on screen, and the well saying `Search library`. This is the page a contextual filter would be for, and the page the placeholder exists to be honest on. |
| `09-scope-word-against-the-page-name-1280` | The same, cropped — the field's promise beside the page's name. |
| `10-typed-on-the-playlist-lands-on-the-library-1280` | Typing `an` there. The Library comes back under the query, the lane's `Library` row lights, `16 / 25`. The behaviour that already shipped, now decided. |
| `11-strip-well-at-rest-900` · `12-strip-well-the-x-under-a-query-900` | The narrow regime, below `SIDEBAR_FLOOR`, where the lane cannot hold the well and the strip takes it back. The same mark makes the same swap, from the same function, so the pointer route exists at every width the keyboard route does. The strip's placeholder stays the collection's counts — it is drawn only in the Library, so there is nothing there for it to be ambiguous against. |

---

## The `×` is on the left, and here is the arithmetic

The convention's corner is the right, and in this field the right is full:

```text
  SIDEBAR_MEASURE                     232
    − SIDEBAR_HEAD_TEXT_X              44
    − GAP_MD                           12
    − SIDEBAR_MATCH_W                  72   ← sized for `1284 / 1284`
    = the query's own room            104
```

That 104 is the number that justified moving the count into the field at all:
the design measured the strip's 88 px slot in the same 232 px well, called it
too tight, and took the lane's own 72 instead. A glyph box beside the count
takes 104 back down to **80** — below the arrangement already rejected. Sharing
the 72 fails too: `1284 / 1284` measures 67.9 px in it, and a slot that clips a
big library's count is not a slot.

The mark's box on the left is **already paid for**. It is `SIDEBAR_GLYPH_BOX` 24
— which is `STEPPER_HIT`, a control's own box — centred on
`SIDEBAR_HEAD_GLYPH_X` 20, so `GAP_SM + STEPPER_HIT / 2 = SIDEBAR_HEAD_GLYPH_X`
places it with no constant of its own. The swap therefore costs the field
nothing on either edge, which is the same guarantee the count's fixed slot gives
on the other side.

At rest the box holds a **label** — *this field searches*. Under a query it
holds a **control** — *press to stop*. A field with text and a count in it does
not need to be told it is a search field.

---

## Measured, not asserted

`measure.py` reads the two 1280 frames and checks the claims that are about
pixels a unit test cannot see. It needs Pillow, which the build toolbox does not
carry — run it on the host:

```sh
docs/design/impl/search-scope/measure.py docs/design/impl/search-scope
```

```text
  ok   magnifier centre x (at rest): 44 (want 44)
  ok   clear mark centre x (under a query): 43 (want 44)
  ok   placeholder first glyph x: 68 (want 68)
  ok   query first glyph x: 68 (want 68)
  ok   match count right edge x: 242 (want 244)
  ok   the placeholder is not drawn beside a query: clear
  measured: all good
```

One harness property worth stating rather than hiding: **the mid-query frame is
focused, so the field draws its ring**, and a focus ring is a line of ink down
the field's own edge and across its top and bottom. Every window `measure.py`
samples starts inside that border; a window that did not would report the
border's x rather than the mark's, which is exactly the wrong answer arriving
plausibly.

## The unit pins that go with these frames

- `views::lane::the_wells_placeholder_names_the_one_thing_it_searches` — the
  placeholder is the scope word, and it is a **constant**, not anything resolved
  from `Place`. A scoped well has to be a visible edit.
- `views::lane::the_wells_mark_is_a_label_at_rest_and_the_clear_under_a_query` —
  the swap, and the two identities (`STEPPER_HIT == SIDEBAR_GLYPH_BOX`,
  `GAP_SM + STEPPER_HIT / 2 == SIDEBAR_HEAD_GLYPH_X`) that let it need no
  constant.
- `app::the_wells_clear_mark_and_escape_are_one_act` — both roads call
  `clear_query`, both wells draw the mark under the same predicate.
- `font::the_lanes_well_names_the_scope_it_searches` — the word fits the resting
  176 px in the bundled face, and so do two longer candidates.
