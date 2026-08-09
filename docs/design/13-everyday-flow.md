# 13 — The everyday flow: starting a record, knowing where you are, and putting things somewhere

> The owner's brief, verbatim (voice-dictated, lightly punctuated):
>
> *"the way that playlists work as a sidebar, I don't hate it, but I don't
> really love it either. I wonder if there are just simply better ways to
> deal with the flow of, firstly, starting to play an album. I feel like
> that does should not need two clicks. I wonder if when we mouse over, we
> can just show two options somehow. Um, I Or I... yeah. I mean, I wonder
> should it be there's options that appear above the icon that you're
> hovering on? For example, send all to current playlist. or play now,
> which just starts to play the album, or just view details, which might
> take you into the screen that we have now. However, I think we should
> consider the way that information hierarchy works and use some elements
> of nesting. For example, when there's a master details view, often you
> can pop back up a level quite easily because you can already see the
> depth that you've went into with the options on the left. If you imagine
> multiple multiple date... list depth options. I'm not one hundred percent
> sure how we would do that. It does seem like other players such as
> Spotify, which I guess could be considered state of the art. Although I
> don't like many things about it that it's UX. I think we probably still
> need to consider that how they do certain operations is reasonably good.
> I think iDaily things like managing playlists, they just need to be more
> clear how it works. For example, when you click to add something to a
> playlist currently, it shows a playlist thing, and there's a very minor
> tip at the very top of the playlist window that indicates you need to
> click on a playlist to add it to it. I mean, it makes sense to some
> degree. But is there a better way to do it? these are the things that I
> think will finally decide how good this is."*
>
> A design study, not an implementation. Written 2026-08-09 against `b795a06`
> (`c7e0f8c` for everything in `crates/`) — the tip after the Jobs-era adopt
> tier, the drag, undo and the trash, and ADR-0028's density detents. Every
> claim about shipped behaviour is cited `file:line` or to a frame in
> [`impl/everyday-flow/`](impl/everyday-flow/), captured for this study on
> the real binary; every prior-art claim carries a named source. Its
> decisions are proposed as [ADR-0030](../adr/0030-the-picker-at-the-pointer.md),
> [ADR-0031](../adr/0031-the-walls-verbs.md) and
> [ADR-0032](../adr/0032-depth-diagnosed.md).
>
> **The short version.** Four problems, and they do not have four answers of
> the same kind. **(1)** The verbs the owner wants beside a hovered sleeve
> already float beside a pressed one — that is the tile's context menu, and
> it is short exactly one item and one teacher; hover-*revealing* them is
> refused here on geometry and calm rather than on the sleeve entry's
> letter, and the argument is given in full because the letter alone would
> have let it through. **(2)** There is no depth to draw: baz's navigation
> tree is one level deep everywhere, and a breadcrumb over a depth-1 tree is
> furniture. The felt problem is not *how deep am I* but *where in the wall
> is this*, and it costs one readout in a slot that already exists.
> **(3)** The picking posture is not a copy defect. Pressing `Add to
> playlist…` throws the destination 682 px across the window, and the
> distance is a function of the window's width rather than of the gesture:
> **the picker leaves the panel and becomes a card at the pointer**, with
> the sentence as its heading instead of its smallest line. **(4)** The
> panel survives — but on a different justification than the one ADR-0024
> §5 gave it, and it may never be a picker again.

---

## 0. What decides this

### 0.1 The three entries that bind, quoted

This study does not get to invent freely, and three `docs/REFUSALS.md`
entries reach the owner's first request directly. They are quoted whole
because §2 turns on their exact words.

**The sleeve entry** (`REFUSALS.md`, The interface):

> **Nothing is ever drawn on top of a sleeve.** No play overlay on hover, no
> badge, no duration chip, no gradient scrim, no selection tint, no queue
> numeral. The only thing that touches artwork is light around it.

**The visible-control rule** (`REFUSALS.md`, Accessibility):

> **Every action in baz has a visible, pointer-reachable control. No action
> is keyboard-only, and no control's only affordance is hover.**

This one is not taste. It is the stated mitigation for a toolkit that
publishes no accessibility tree and gives buttons no keyboard focus
(ADR-0017 §4), which is why ADR-0028 ruled that where it collides with an
aesthetic entry, it outranks it.

**The two-press entry** (`REFUSALS.md`, The interface — added this morning,
by the owner, hours before the brief above):

> **Sound from the wall is two presses, and that is a price, not a debt.**
> Open the record's page, press `Play album` — deliberately, twice
> considered: ADR-0022 removed the double-click structurally (the first
> press navigates; no tile remains under the pointer for a second), and the
> Jobs-era critique (`docs/design/11-jobs-era-critique.md` P7) surfaced the
> one candidate no ADR had listed — a second press landing on the
> just-opened page as `Play album` — and the owner refused it: a press whose
> meaning depends on arrival time is a micro-mode, in a product that hates
> cleverness. The friction budget's *intent → sound = 1 press* line is
> hereby **re-priced at the wall**, not unmet […]. (Owner's decision,
> 2026-08-09.)

The brief's first request is a request to reopen the third entry by a
mechanism the third entry does not name. The ledger's editing rule governs:

> **Removing one needs an ADR that beats its argument.** A refusal you can
> delete because you changed your mind is not a refusal; it is a preference.

So the owner's asking is **evidence** — a direct observation of the one user
this product is for, which ADR-0022's own epistemology ranks above priors —
and it is **not an argument**. §2 supplies the argument, or declines to, and
says which.

### 0.2 The laws already in force

- **L8** (doc 07): one home per control; the home is the surface whose
  subject it shares, where subject is *what a control must consult to know
  what to do*. Frequency decides residency, never the surface. Facts may be
  restated everywhere; controls may not (L8.6).
- **L8.5**: an object is not a control. A sleeve may be pressed; nothing may
  be drawn on it to say so — the clause is already bounded by the sleeve
  entry, in doc 07's own words.
- **L9** (ADR-0026): a strip declares its tenants and holds them at its
  floor; it never hides one, never sweeps one into a menu, never overflows.
- **The mirror rule** (doc 09 §5.2, `menu.rs:1–22`): every menu item sends
  only messages some visible on-screen control also sends, and no action's
  only route is a menu. Pinned by
  `every_menu_item_is_a_press_some_control_also_makes` (`menu.rs:581`).
- **ADR-0020's motion budget**: five tweens may exist, three ship, and
  nothing may cost a redraw while the window is idle. *Anything requiring a
  redraw while the window is idle* is the clause the rest hangs on.
- **The frequency bands** (`03-interface-prior-art.md` §1.2): A constant ·
  B frequent · C occasional · D rare · E very rare. What happens dozens of
  times a session must be nearly free; what happens monthly may cost a
  place.

### 0.3 What is on screen today

Read off the code and confirmed on the four frames captured for this study
([`impl/everyday-flow/`](impl/everyday-flow/), receipt in its README):

- **The wall's tile** is one object with one press: `AlbumClicked(album.id)`
  (`views/shelf.rs:1021`), wrapped in a `mouse_area` that reports the hover
  (`shelf.rs:1023–1024`) and in `menu::area(…, Target::Album)`
  (`shelf.rs:996`, `shelf.rs:1025`). Shift held, the same press queues the
  record instead (`app.rs:1054–1061`). Its whole hover vocabulary is a
  1 px → 2 px rule under the label and one rung of ink on the artist line
  (`shelf.rs:972–976`, frame `01`).
- **The tile's menu**: `Open · Play album · Queue album · Add to playlist…`
  (`menu.rs:233–264`), a `MENU_W` 232 card whose top-left corner is the
  pointer, flipped to the pointer's other side at an edge it would cross
  (`menu.rs:359–372`), `Esc` peeling it before every other layer. `Queue
  album` prints `Shift-click` at its right edge — the one item in the
  product with a printed accelerator (`menu.rs:249–257`). Frame `02`.
- **The record's page**: the sleeve at `ART_MAX` 320, `Play album` as a
  320 × 32 lamp-outlined glyph-and-word beneath it (`views/album.rs:368`
  onward), `Add to playlist…` as a bare word under that, and — since doc 11
  P3 — `‹ Prev` / `Next ›` in the header, stepping `vm::neighbours` over the
  wall's own visible order (`views/album.rs:156–191`, `vm.rs:1095`),
  accelerated by `Ctrl+[` / `Ctrl+]`. Frame `04`.
- **The picker** is the playlist panel wearing a second job. A `+` slot, the
  record page's `Add to playlist…`, or a menu item calls
  `Playlists::begin_pick` (`playlists.rs:490–509`), which holds what was
  pointed at and opens the panel; every row becomes a destination
  (`views/playlist_panel.rs:222–247`, `:362–384`); `Playlists::pick` appends
  and puts the pick down (`playlists.rs:536–541`). Frame `03`.
- **The panel** itself: `PANEL_W` 340 plus a 1 px seam, right-aligned in a
  `stack` over the place, wrapped in `opaque`, no scrim, wheel passing
  through (`app.rs:3198–3223`; ADR-0016's mechanics, ADR-0024 §5.4). It is a
  directory of lists — the Queue at its head as a readout, one row per
  `.m3u8` with its collage sleeve, `New playlist` last — and, since the drag
  shipped, a **drop target**: a row dragged from the queue or a playlist
  page and released on a panel row is that file's append, the picker row's
  act made direct (`views/playlist_panel.rs:400–409`, ADR-0024 §6.3).
- **The places**: `Library · Album · Queue · Playlist · Settings`, one at a
  time, `Place::back` total and argument-free — every place returns to
  Library (`place.rs:171`). `place.rs:33–46` argues the absence of a history
  stack in its own words, and this study engages it in §3.

---

## 1. The four problems, restated as questions this document must answer

| # | The owner's words | The question |
|---|---|---|
| **1** | *"starting to play an album… should not need two clicks"* · *"options that appear above the icon that you're hovering on"* | Can the wall reveal a record's verbs beside it, and can sound from the wall cost one gesture, without breaking the sleeve entry, the hover clause, or the two-press entry its own author wrote this morning? |
| **2** | *"use some elements of nesting… you can already see the depth that you've went into"* | Does baz have depth to show, and if not, what is the felt problem that reads as missing depth? |
| **3** | *"a very minor tip at the very top of the playlist window"* | What is wrong with the picking posture — and is it the copy, or the interaction? |
| **4** | *"playlists work as a sidebar, I don't hate it, but I don't really love it either"* | Does the panel survive its own justification once problem 3 is answered? |

Problems 3 and 4 are one decision taken twice and are decided together in
§4–§5. Problems 1 and 2 are independent and are taken first.

---

## 2. Problem 1 — the wall's verbs, and one gesture to sound

### 2.1 What the owner is asking for, named precisely

Three verbs, revealed by hovering a tile, drawn *above* it: **play now**,
**send all to current playlist**, **view details**. Two of the three exist
as controls already (`Play album` on the record's page; `Open` is the tile's
own press). The third — *send all to current playlist* — **does not exist at
album scope anywhere in the product**: `Add to "{current}"` is offered on
track rows, queue rows, playlist-page rows and the bar's now-playing block
(`menu.rs:192–201`, `:214–223`, `:285–291`, `:313–321`) and on no album
object at all. That is a real gap and §2.6 closes it independently of
everything else in this section.

What is *new* in the ask is therefore not the verbs. It is **the reveal**:
that pointing at a record, without pressing, should show what can be done to
it.

### 2.2 The sleeve entry, tested honestly

The brief's own reading is that *above the icon* may mean nothing is drawn
**on** the sleeve at all — a floating group adjacent to the hovered tile, on
the menu's float layer. Tested against the entry as written:

> Nothing is ever drawn on top of a sleeve. No play overlay on hover, no
> badge, no duration chip, no gradient scrim, no selection tint, no queue
> numeral. The only thing that touches artwork is light around it.

**On the letter, the adjacent group passes.** Every member of the entry's
enumerated list is a mark *inside the artwork's own bounds*, and the closing
sentence — *the only thing that touches artwork is light around it* —
regulates what touches artwork, not what stands beside it. A group of word
controls floating in the wall's gutter touches no artwork. The entry as
written does not forbid it, and this document says so plainly rather than
reading the entry generously in its own favour: **if the design failed only
on this entry, the entry would have to be beaten, and it is not clear it
could be.**

It fails on three other grounds, each independently sufficient, and none of
them is a preference.

**(a) Geometry. There is nowhere on a full-bleed grid to put it.** The wall's
clear space between one row's state rule and the next row's sleeve is the
step's hang less the rule's lane — `RULE_LANE_H = GAP_XS + SELECTION_EDGE`
= 6 (`shelf.rs:1037`) — which is exactly the 34 px `shelf.rs:1035–1036`
claims at `Balanced`:

| Density step | hang | clear wall between rows |
|---|---:|---:|
| Spacious | 48 | **42** |
| Balanced | 40 | **34** |
| Dense | 28 | **22** |

Against that, the two forms such a group can take:

- **A card**, the shape the product already draws — three items is
  `3 × TRANSPORT_HIT + 2 × GAP_XS` = **104 px** tall, four items **136**
  (`menu::extent`, `menu.rs:344–354`). It exceeds the clear wall by 62 to
  114 px at every step, so it must overlay a neighbouring record — its
  sleeve, if the group hangs above a tile in the top row of a shelf, and its
  label everywhere else. Sideways it is `MENU_W` 232 against a tile pitch of
  ~240 at Balanced/1280: one whole record, covered, every time you point at
  another.
- **A single horizontal row** at law L7's one control height, laid flat in
  the gutter — 32 px. It does not fit at Dense at all (22 px), and at
  Balanced it fits with 1 px of air on each side, which is the band-lead
  refusal (*a band's content may not touch the band's edges*) broken to make
  a control fit. And it cannot carry the owner's own verbs: at the art width
  the row would have to hang from, three items share 240 px — 80 px each —
  and `Add to "Road Trip"` is why `MENU_W` is 232 in the first place.

The wall is the one surface in baz with no slack, because slack in a gallery
is the gallery. **A hover group has no legal placement, at any density.**

**(b) Calm. The wall would twitch.** Today, sweeping the pointer across the
wall changes a 1 px rule to 2 px and lifts one caption line by one rung of
the ink ramp, over the 90 ms tween ADR-0020 §2.3 budgets (`shelf.rs:972–976`,
visible whole in frame `01`). A group of verbs arriving and leaving under a
moving pointer is a categorically different surface: crossing a 1280 px wall
from the first column to the index rail passes over four or five tiles, so
four or five opaque cards appear and vanish in one gesture that was meant to
reach the rail. The cost is not paid by the person hovering deliberately; it
is paid by everybody *crossing*, which is everybody, all the time.

The standard escape is a **dwell delay** — reveal after N ms of stillness.
That is refused here for the entry's own reason, one clause up: *a press
whose meaning depends on arrival time is a micro-mode, in a product that
hates cleverness.* A **reveal** whose appearance depends on dwell time is
the same species as a press whose meaning depends on arrival time, and it
would need a `window::frames()` subscription running while the pointer moves
— which ADR-0020's load-bearing clause (*anything requiring a redraw while
the window is idle*) tolerates only because a hovering pointer is not idle,
but which adds a sixth tween the motion budget does not have.

**(c) The conjunction, which is the real finding.** Every product that ships
hover-to-play on a cover grid draws the affordance **inside the object's own
bounds** — on the artwork or on the card that contains it (§8.1: Spotify,
Apple Music, YouTube Music, Tidal, Plex all do this, and the study's own
survey found *nobody* who does otherwise, `03` §7.3). That is not a
coincidence of taste; it is the only placement that occludes nothing else,
because an object's own bounds are the one region guaranteed not to belong
to another object. baz refuses marks inside a sleeve's bounds. **The two
constraints together close hover-reveal on baz's wall completely** — not
because either forbids it, but because the sleeve entry removes the only
placement that (a) has room for.

> **Decision.** The wall does not gain a hover-revealed control group, and
> the sleeve entry is not proposed for amendment — it does not need to be.
> Recorded in the ledger's *"Considered, and not refused"* register rather
> than as a new refusal, so a future proposal with an answer to (a) meets an
> argument instead of a wall.

### 2.3 Is the hover group just the menu, unhidden?

The brief requires this possibility to be adopted or rejected with reasons.
It is the strongest version of the ask, because it costs no new interaction
class: the card exists, the verbs exist, the geometry exists, the mirror
rule already governs it, and frame `02` is a photograph of it.

**Rejected**, on two grounds beyond §2.2's, both specific to the menu:

1. **The card is `opaque` and captures presses** (`app.rs:3236–3249`;
   `menu.rs:449–468`). Under the menu sits a full-window backdrop whose left
   press puts it down. A wall that grows an opaque card wherever the pointer
   rests is a wall whose next click is spent dismissing something you did
   not ask for. The menu's dismissal model is correct *because* it is
   summoned; unhidden, it makes every crossing of the wall a mode you fall
   into by moving, which is exactly what the armed collecting mode was
   removed for (doc 09 §9: *"the one thing in the product that answers 'what
   does this press do' with 'it depends'"*).
2. **It would make right-click a duplicate of nothing.** The menu's whole
   licence is the mirror rule — it is an accelerator layer over controls
   that exist. A menu that opens itself is no longer an accelerator; it is a
   resident surface with a floating position, and L8's residency arithmetic
   has never admitted one.

What survives from the idea is the useful half, and §2.5 adopts it: **the
menu is already the adjacent group the owner is describing.** It is short
one verb and it has no teacher.

### 2.4 What the owner will actually get, compared honestly

| Route | Presses | Navigation | Where the second press is |
|---|---:|---|---|
| Today, page route | 2 | replaces the window; `Esc` to return | a 320 × 32 target, ~500 px away, after a place change |
| Today, tile menu | 2 | **none** | ~127 px from the first, at the pointer (§4.2's arithmetic) |
| The owner's hover group | 1–2 | none | at the pointer, but no legal placement (§2.2) |

The middle row is the finding. **baz already ships "play a record without
leaving the Library, with the options shown beside the record you pointed
at, in two gestures"** — which is strictly better than the flow the owner is
complaining about, and better than the flow he proposed on every axis except
the reveal gesture. The gap is that the reveal gesture is a right-click, and
right-click is taught nowhere on the wall. Doc 11 §2.7 found this and named
it: *"The menus mirror controls; nothing mirrors the gestures."* The repair
that shipped (doc 11 P6.1) prints `Shift-click` **inside the menu** — which
teaches the accelerator to people who already found the menu, and nothing to
anyone else.

### 2.5 Adopted: the menu completed, and taught where the cost is felt

**(i) The tile menu gains `Add to "{current}"`**, in doc 09 §5.2's table
order, present exactly while playing provenance stands and the file still
exists — absent, not disabled, otherwise (§2.6 designs it).

**(ii) The reveal is taught at the moment of relevance.** Not with a tooltip
on every tile (a thousand objects wearing chrome to explain a gesture), not
with a coach mark, and not in the strip (L9 has a budget and a teacher is
not a tenant it can price). The teacher goes where the person has *just
paid the cost it would have saved*: the record's page, reached by a tile
press, whose header note lane is a single quiet meta line today reading
`Esc returns to Library`.

The lane becomes a two-fact line — the return, and the thing the trip cost:

```
‹ Library   Album   ‹ Prev  ·  4 of 25  ·  Next ›       Right-click a sleeve to play it from Library
```

One string, one lane that already exists, in the voice the product already
teaches in (`Enter plays the first match.` on the Songs rule,
`shelf.rs:216`; `Esc clears the search.`; `When a queue ends, baz stops.`).
It is the era's accelerator-beside-the-verb convention (doc 11 §1.1) pointed
the other way: print the gesture beside the *cost* it removes.

This is deliberately modest, and the modesty is the argument. The product's
teaching surfaces are its empty states, its rules and its tooltips; doc 11
P6 spent all three and shipped, and adding a fourth kind of teacher to carry
one gesture would be the tour that document explicitly refused.

### 2.6 `Send all to current playlist`, designed

The owner's second named verb, and the one thing in his list that genuinely
does not exist.

**What it is.** With a current playlist standing — playing provenance naming
a `.m3u8` that still exists, doc 09 §6 — a record's whole selected edition
is appended to **that file**. The run is untouched: doc 09 §6's decoupling
holds in both directions and the both-at-once gesture stays refused (*a
gesture that writes two structures at once is the two-lane confusion coming
home*). *Keep it* is `Add to "Road Trip"`; *hear it tonight* is `Queue
album`; doing both is both gestures.

**Where it lives.** The tile's menu, as item three of five, and the record
page's `Add to playlist…` → the picker, whose hoisted first named row is the
current playlist (`playlists.rs:526–531`). Two gestures by either road.

**What it costs to build: nothing new.** The item's presses are
`AddAlbumToPlaylist(id)` then `PickPlaylist(current)` — both already in the
mirror table with a named visible twin (`menu.rs:586–615`: the record page's
`Add to playlist…`, and the picker's playlist rows *"the hoisted playing one
included"*). No new message, no new control, no ledger entry touched.

**What it does not get: a resident control of its own on the record's page.**
The aside holds `Play album` and `Add to playlist…`; a third word-act naming
a list that may not exist next frame would be a control that comes and goes,
and L8.2's arithmetic does not admit a band-C act into a two-tenant cluster
to save one press over a route that already exists.

The menu, after this study:

```
Open                                   the tile's own press
Play album                             the page's Play album
Queue album                Shift-click the picker's Queue row
Add to “Road Trip”                     the picker's hoisted row      ← new
Add to playlist…                       the page's Add to playlist…
```

Five items is `5 × 32 + 8` = **168 px** of card against the four-item 136
(`menu::extent`), and the edge flip already holds any height inside the
window (`menu.rs:359–372`, tested at `menu.rs:761`).

### 2.7 One press to sound from the wall — the fourth candidate · **present-to-owner**

Everything above leaves sound from the wall at two presses. The owner asked
for one. The ledger entry that says two is the price was written by him this
morning. This section supplies the argument the editing rule demands and
declines to spend it, because the entry is one day old and its author is the
person asking.

**The entry's argument, steelmanned.** Its enumeration of candidates is:
a bare `Enter` (keyboard-only — refused by the visible-control rule); a play
affordance on the tile (a mark on a sleeve — refused); and P7's second press
landing on the just-opened page (a micro-mode — refused by the owner, in the
entry's own sentence). Against those three the entry's conclusion is
airtight, and its re-pricing is generous rather than defensive: one press
from the Songs section, from a playlist, from the queue, from `Play all`;
two from a sleeve, *"where the second is a fixed 320 × 32 target with a
name"*. It also keeps a one-press gesture on the tile already — shift-click,
the *sound-later*.

**The candidate the enumeration does not contain.** A **modifier press on
the tile that means `Play album`** — the exact construction the entry itself
keeps for sound-later, pointed at sound-now. It is not keyboard-only (the
pointer makes it). It draws nothing on a sleeve. It has no dwell and no
timing window, so it is not a micro-mode: the press's meaning depends on a
key that is either down or not, which is a state the hand chose, not a state
the clock chose. And it has a visible twin twice over — `Play album` on the
record's page and in the tile's own menu — so the mirror rule resolves it
exactly as it resolves shift-click, and it is taught in exactly the same
place, the menu's accelerator column:

```
Play album                  Ctrl-click     ← the printed accelerator, new
Queue album                 Shift-click
```

**What is against it.** Three things, stated because this is presented
rather than adopted. (1) Three meanings for one press on one object — plain,
shift, ctrl — is a modifier layer on the product's most-pressed object, and
the entry's real target is *cleverness on the wall*, which this is a form
of. (2) `Ctrl`-click is the platform's add-to-selection chord, and the wall
has no selection to add to today but may one day. (3) ADR-0022 already
pointed any return of one-press sound at the stack — *"a queueing gesture
rather than a second meaning for a press"* — and this is precisely a second
meaning for a press.

**What is for it.** The friction budget's own line, at the surface that is
the product's home and its most frequent intent (W1, band A); the fact that
one-press sound-now exists at every scope in the product *except a single
record*, which is the unit the wall is made of; and the symmetry — a product
that has a one-press *later* and no one-press *now* has its two gestures the
wrong way round.

> **Tier: present-to-owner.** ADR-0031 §4 carries it as an open question
> with the amendment text drafted, in the shape ADR-0028 used for P8: the
> entry would be **narrowed, not deleted** — what stands is that no timing
> gesture, no keyboard-only route and no mark on a sleeve buys the press;
> what would fall is the entry's implicit claim that its enumeration was
> complete.

---

## 3. Problem 2 — nesting, depth, and getting back up

### 3.1 Diagnose before prescribing: the tree, measured

Every navigation baz has, with its depth from home:

| From | To | Route | Depth |
|---|---|---|---|
| Library | Album | a tile's press (`shelf.rs:1021`) | 1 |
| Library | Album | a Songs row's record door (`shelf.rs:361–379`) | 1 |
| Library | Queue | the bar's `Queue` door, `Ctrl+U` | 1 |
| Library | Playlist | a panel row (`playlist_panel.rs:399`) | 1 |
| Library | Settings | the gear, `Ctrl+,` | 1 |
| anywhere | Album | the bar's now-playing block (`ShowPlayingAlbum`) | 1 |
| Album | Album | `‹ Prev` / `Next ›` (`album.rs:169–191`) | 1 → 1 |

**The maximum depth of baz's navigation tree is one, and it is one from
every place.** There is no Album inside a Playlist, no Artist inside a
Genre, no folder inside a folder. `Place` is five members and an enum, and
`Place::back` is total because there is nothing for it to be partial about
(`place.rs:33–46`, `:171`).

A breadcrumb over a depth-1 tree renders `Library › Album` — one separator
and two words, one of which is already the header's back door and the other
of which is already the header's title. Miller columns over a depth-1 tree
render one column. Both are furniture that states what the header already
states, and building either would mean **manufacturing hierarchy in order to
have some to display**. Nielsen Norman's own condition on breadcrumbs is
that they suit deep hierarchical structures and add nothing to shallow or
non-hierarchical ones (§8.4).

And the naive Miller answer is refused before it is proposed: doc 11 P10
rejected restoring a persistent library pane on four grounds, of which the
first is that the owner rejected resident side surfaces twice in plain words
and *"a direct observation of this user beats a prior about users in
general, and beats it harder the second time"*.

### 3.2 What already shipped against this problem

The owner's brief was written against a build that already carries most of
the mitigation, and the study has to say so:

- **The wall is preserved across every navigation.** Scroll, query and
  arrangement are untouched by a place change, and the record you last
  opened keeps a 2 px rule under its label so the return leg is *return*,
  not *re-find* (`shelf.rs:950–956`; ADR-0022 *"What is now worse"*).
- **`‹ Prev` / `Next ›`** step the wall's own visible order from inside the
  record's page — the same arrangement, the same filtered set, edges inert
  (`vm.rs:1095`, `album.rs:169–191`), with `Ctrl+[` / `Ctrl+]` over them.
  Comparing two releases is one press per release again (doc 11 P3).
- **Master-detail with both surfaces visible already exists, once.** Open a
  playlist from the panel and the panel *stays* — it is present in Library,
  Album and Queue, absent only in Settings (ADR-0024 §5.5), so a playlist's
  page is drawn with the list of every playlist beside it. That is the
  master-detail arrangement the owner is describing, shipped, and it is also
  the surface he opened the brief by saying he does not love.

### 3.3 The diagnosis

Depth is not the felt problem, because there is no depth. Three candidates
for what is:

**(a) "Back" does not mean back.** `Place::back` returns Library from
everywhere. There is exactly one reachable path where this orphans you: from
the Queue place or a playlist's page, press the bar's now-playing block →
the record's page → `Esc` → **Library**, not the place you left. This is
real, and it is small: the bar's block is a *teleport*, not a descent, and
one lateral jump per session is band D at best.

It is also already argued. ADR-0022: *"a history that could land you
somewhere you did not navigate from is the rail in a different shape."* Doc
11 §4 declines to attack it and gives the era's reason: `Esc` is the iPod's
Menu button — *up*, always, never *back along my path* — and one meaning per
gesture beats path-memory. **Both stand.** A back stack is not proposed.

**(b) You cannot see which place you are in.** False: the header names it,
in the same geometry in five places (`views/mod.rs:236–281`), with a
labelled way out and `Esc` beside it.

**(c) You cannot see *where in the collection* the thing you opened sits.**
**True, and this is it.** A record's page says `Album`. It does not say which
of 25 records it is, nor that the wall behind it is filtered to 7, nor which
shelf it came from. The `‹ Prev` / `Next ›` pair *knows* all of this — it is
computed from the wall's visible order — and states none of it. Frame `04`
is the evidence: two doors with no position between them.

What the owner is describing when he says *"you can already see the depth
that you've went into with the options on the left"* is not a count of
levels. It is the master pane's other job: **a detail view that shows you
your place in the set you came from.** Lightroom's Filmstrip does it,
Calibre's shared model does it, and a depth-1 product can do it in one
readout.

### 3.4 Adopted: position, not path

> **The Album place's step pair states the position it already computes.**

```
‹ Library   Album   ‹ Prev  ·  4 of 25  ·  Next ›            Esc returns to Library
            ▲                     ▲
            the place            the position in the wall's current arrangement,
                                 which is also the scope: a filtered wall says
                                 `2 of 7`, and that is the filter, stated
```

- It is a **readout**, not a control: L8.3's escape valve run in its ordinary
  direction — make the fact resident where it is watched and leave the
  controls where they are. No message, no press, no new tenant class.
- It goes in the header's **existing optional-tenant slot**
  (`place_header_with`, `views/mod.rs:245–258`), between the pair's two
  doors, so the frame stays one function in five places and cannot drift.
- Reserved width: `POSITION_W`, sized for `99 of 9999` at `SIZE_META` in the
  bundled Medium — the reserved-slot rule the bar's stamps already follow,
  so a step from `9 of 25` to `10 of 25` moves neither door.
- **It says nothing when there is nothing to say.** A record the wall no
  longer shows has no neighbours at all (`vm.rs:1092`, tested at
  `vm.rs:2496–2503`); its pair is already inert and its position is absent,
  not `0 of 0`.

The cost is one string and one token. What it buys is the sentence the owner
asked for, in the only form a depth-1 product can honestly make it: *you are
four records into the twenty-five this arrangement is showing, and two doors
step them.*

### 3.5 The filmstrip's door, named rather than opened · **present-to-owner**

Doc 11 P10 rejected the three-pane restore and left one door open by name:

> *"if W15-class work (Marta's compare-two-releases) grows despite P3 — if
> the owner finds himself round-tripping daily — the evidence base shifts,
> and `03` §7.2(5)'s Lightroom Filmstrip (a **bottom** strip inside the
> Album place, not a side surface) is the re-proposal that meets ADR-0022's
> argument on its own terms."*

The owner has now raised the felt need in his own words, which is the
evidence P10 said would shift the base — but he raised it as *nesting and
depth*, and §3.1 finds no depth. So the door is named, drawn, and left for
him rather than walked through:

```
┌────────────────────────────────────────────────────────────────┐
│ ‹ Library  Album   ‹ Prev · 4 of 25 · Next ›   Esc returns to… │
├────────────────────────────────────────────────────────────────┤
│  ┌───────────┐   VIOLET LEDGER                                 │
│  │  320×320  │   Anne-Marie Puig                               │
│  └───────────┘   ──── TRACKS ─────────────────────────────     │
│  [ ▶ Play album ]  1  Field Recording 1              3:23      │
│  Add to playlist…  2  Anhydrous 2                    6:27      │
├────────────────────────────────────────────────────────────────┤
│ ▫ ▫ ▪ ▫ ▫ ▫ ▫ ▫ ▫ ▫   ← the wall's own order, this record lit  │
└────────────────────────────────────────────────────────────────┘
   one row of 64 px sleeves, HANG-led, scrolled to the current record
```

**For it**: it is not a side surface (ADR-0022's rejection was of resident
*side* surfaces, twice, in the owner's own words); it makes the position
readout of §3.4 redundant by *showing* it; it is the era's own answer for
cataloguer-grade products (§8.5); and it makes compare-two-releases one
press with the collection never leaving the screen.

**Against it**: it is resident chrome inside a place, paid for by the
record's page every frame; it re-opens the content-share number `03` §2.3
calls the single most important in the corpus; and it would be the first
surface in the product whose subject is the *library* living inside a place
whose subject is *one record*, which L8.1 has no clause for. §3.4's readout
buys most of what it buys for one string.

> **Tier: present-to-owner.** Not proposed. Recorded with its drawing so
> that a re-proposal meets a design, and its costs, rather than a shrug.

---

## 4. Problem 3 — the picking posture

### 4.1 The complaint, found in the code and photographed

The owner: *"there's a very minor tip at the very top of the playlist window
that indicates you need to click on a playlist to add it to it."*

He is describing `views/playlist_panel.rs:122–129`, verbatim:

```rust
    // A pick in flight: the panel is the picker, and it says what is in the
    // hand so the next press is legible before it is made.
    if let Some(pending) = &playlists.pending {
        body = body.push(
            text(format!("{} — pick a destination", pending.label))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
        );
    }
```

`pending.label` is built at the gesture — `format!("Add \u{201c}{}\u{201d}",
…)` (`app.rs:1718–1721` for a record, `:1756` for a track, `:1784` for a
queue row, `:1810` for a playlist row) — so the rendered line is
`Add “Violet Ledger” — pick a destination`. Frame `03` is that line on
screen. Everything the owner says about it is true, and the code's own
comment claims the opposite of what the frame shows:

- It is set at **`SIZE_META` 12** — the product's second-smallest size, the
  size of counts and hints — in **`paper_dim`**, under a `Playlists` heading
  at `SIZE_EMPHASIS` 15 in Medium. **The panel's only statement of what it
  is now for is quieter than its own title and no louder than `Esc
  closes`.**
- The surface's *name* still says `Playlists`. Nothing about the frame says
  a task is in flight except the one dim line and the word `Add` that has
  appeared at the right of every row.

But the copy is the smaller half. Three interaction defects are visible in
the same frame:

1. **The destination is thrown across the window.** The panel is anchored to
   the window's right edge whatever the gesture was (`app.rs:3216–3218`).
   At 1280 the panel's left edge is `1280 − 341` = 939 and its rows begin at
   `939 + GAP_XL` = **963**; the tile that was right-clicked in frame `03`
   is centred at x 444. From that press to the first destination row's
   centre is **≈ 682 px** of pointer travel. In the menu the second press
   was **≈ 127 px** away (frame `02`: the card's corner *is* the pointer,
   `MENU_W` 232, items at `TRANSPORT_HIT` 32).
2. **That distance is a function of the window, not of the gesture.** Widen
   to 1920 and the same pick grows by 640 px. A gesture whose cost rises
   with the size of the display is not a gesture the product has designed;
   it is one it has inherited from where the surface happens to live.
3. **The surface is enormously larger than the task.** `PANEL_W` 340 at full
   window height, to offer three destinations. And while it stands it covers
   the index rail and, since ADR-0028, the density detents
   (`INDEX_LANE_W` 108 of the wall's right edge) — a picking task occluding
   the wall's navigation.

**The complaint is not about the tip. It is that the picker is a 340 px
resident-shaped surface at the far edge of the window doing a two-press,
pointer-local job.** Fixing the string would leave all three defects and the
owner would be back.

### 4.2 The options, weighed

**(a) Fix the copy and the framing; keep the panel as picker.** The heading
becomes the sentence, the rows are visibly targets, the panel's title
changes while a pick stands. **Cheapest, and it is the fallback.** It fixes
the owner's literal complaint and none of §4.1's three defects.

**(b) An inline submenu on the context menu** — `Add to playlist ▸` opening
a list of lists. This is what Spotify and MusicBee do (§8.3). **Rejected.**
`menu.rs`'s module docs close it in four words — *"No submenus, no new key
bindings"* — and the reason holds: a submenu needs hover-to-open and a
safe-triangle to survive a diagonal pointer, which is the dwell-timing
species §2.2(b) refuses. Worse, it answers only the menu route: the `+`
slots and the record page's `Add to playlist…` are not menus and would still
need a destination surface, so the product would have **two** of them.

**(c) A modal dialog.** Rejected without much ceremony: baz has no dialogs,
the 1992 HIG's own ranking treats frequent alert boxes as a symptom (doc 11
§1.1), and a scrim is refused outright by name.

**(d) The picker leaves the panel and becomes a card at the pointer.**
Adopted. It puts the second press where the first one was, it sizes the
surface to the task, it frees the panel to be one thing again (§5), and it
costs no new mechanism — the float-at-pointer card with edge flipping, an
`opaque` body, a click-outside backdrop and `Esc` peeling first is shipped
and tested (`menu.rs:344–372`, `menu.rs:761`, `app.rs:3236–3249`).

### 4.3 Adopted: the destination card

> **A pick opens a card at the pointer, headed by the sentence, holding the
> destinations and nothing else. It is not a menu: its rows are controls,
> and they are the visible controls the mirror rule resolves `PickQueue` and
> `PickPlaylist` against.**

```
        ┌─ PICKER_W 280 ────────────────────────────┐
   ●────│ Add “Violet Ledger” to…          SIZE_BODY│  ← the sentence, as the heading
 pointer│ 9 tracks · 45:26            SIZE_META dim │  ← what is in the hand, stated
        ├───────────────────────────────────────────┤  ← hairline, GAP_SM either side
        │ ▫  Queue                     8 · 32:10    │  48
        │ ▫  Road Trip — playing      14 · 51:08    │  48   ← hoisted, marked
        │ ▫  Late Nights              23 · 1:40:11  │  48
        │ ▫  Autumn                    9 · 38:44    │  48
        ├───────────────────────────────────────────┤
        │    New playlist                           │  32
        └───────────────────────────────────────────┘
             Esc cancels · a press outside puts it down
```

**Geometry**, all on the 4 px lattice:

| Part | Height | Token arithmetic |
|---|---:|---|
| card lead | 8 | `GAP_SM` |
| heading line | 20 | `LINE_BODY` |
| what-is-held line | 16 | `LINE_META`, `GAP_XXS` above |
| rule + its air | 17 | `GAP_SM` + 1 + `GAP_SM` |
| each destination row | **48** | `PANEL_SLEEVE` 40 + 2 × `GAP_XS` |
| `New playlist` | 32 | `TRANSPORT_HIT`, L7's floor |
| card foot | 8 | `GAP_SM` |

`PICKER_W` **280** = `PANEL_W` 340 − 2 × `GAP_XL` 24 − 12: the panel's own
content measure, less the sleeve column's slack, rounded to the lattice. It
is wider than `MENU_W` 232 because a destination row carries a sleeve, a
name and a counts line where a menu item carries a verb.

Four destinations is `8 + 20 + 16 + 17 + 4 × 48 + 32 + 8` = **293 px** tall.
`PICKER_MAX_H` **400** caps it at six rows plus chrome; beyond that the row
list scrolls inside the card with the list scrollbar the panel already uses
(`theme::list_scrollbar`), heading and `New playlist` pinned outside the
scroll so the sentence and the creation row never leave.

**Placement**: `menu::anchor`'s rule exactly — the card's top-left corner is
the pointer, flipped to the pointer's other side at any edge it would cross,
clamped as the last resort (`menu.rs:359–372`). One shared function, so the
two floats cannot disagree about what an edge is.

**What the card says, and why each line is there:**

- **`Add “Violet Ledger” to…`** at `SIZE_BODY` 13 in full paper, Medium.
  The verb, the object, and an ellipsis that promises the list underneath —
  the same honest `…` convention `Add to playlist…` and `Browse…` already
  use (ADR-0025 §1). It is the heading, not a hint: a surface whose entire
  reason for existing is a question states the question at the size the
  surface's name would have taken.
- **`9 tracks · 45:26`** at `SIZE_META` in `paper_dim`. What is in the hand,
  in figures — the one fact the shipped picker never states, and the one
  that distinguishes *this record* from *this track* when the label alone is
  ambiguous.
- **The rows lose their `Add` word.** The shipped picker prints `Add` at the
  right of every row (`playlist_panel.rs:227–232`, `:369–374`), which is the
  design compensating for a heading nobody reads. With the heading carrying
  the verb, a row is a destination and says so by being one.
- **`Esc cancels`** in the meta voice at the foot, matching the panel's own
  `Esc closes`.

**Order**, unchanged from doc 09 §8.1 and already pure and tested
(`playlists::picker_order`, `playlists.rs:526–531`, tested at
`playlists.rs:1845–1856`): the **Queue** first — the unnamed sounding list —
then the **current playlist** hoisted and marked *playing*, then the folder's
own order, then `New playlist`.

**Dismissal**, the product's one model: `Esc` peels the card before every
other layer, a left press outside puts it down (never a spent click), a
right press outside falls through to whatever is beneath. Identical to the
menu's, because it is the menu's (`menu.rs:50–58`).

**`New playlist` inside the card.** Pressing it turns the row into the name
field the panel already has (`playlist_panel.rs:273–296`), with the storage
layer's refusal under it in its own words; submitting creates and completes
the pick (`playlists.rs:516–518`). The card grows by the field's height and
does not move — it is anchored at its corner, so it grows downward, or
upward where the flip put it.

### 4.4 What this changes about the gestures, exactly

| Gesture | Today | After |
|---|---|---|
| A track row's `+` | opens the panel at the window's edge | opens the card at the `+` |
| The record page's `Add to playlist…` | opens the panel | opens the card at the control |
| A menu's `Add to playlist…` | opens the panel | opens the card where the menu was |
| A menu's `Add to "{current}"` | `+` then the panel's hoisted row, both auto | unchanged — two messages, no card shown |
| A menu's `Queue` / `Queue album` | `+` then the panel's Queue row, both auto | unchanged |
| `Playlists` door / `Ctrl+P` | opens the panel | **unchanged** — the panel is the directory (§5) |
| Drag a row onto a panel row | appends to that file | **unchanged** — the panel is still the drop target |

The two-message menu items are the important row: they never *showed* the
picker, they made both presses (`menu.rs:189–190`, `:194–199`). They keep
working because the messages are unchanged; only the surface that draws the
second press has moved.

---

## 5. Problem 4 — does the panel survive?

### 5.1 ADR-0024 §5's five justifications, re-run

The owner: *"I don't hate it, but I don't really love it either."* The
amended refusal names this panel and closes the slot, so its survival is a
question the ledger requires to be asked properly.

| # | ADR-0024 §5's justification | After §4 |
|---|---|---|
| 1 | **One tenant, forever** — ordered lists of tracks | **Holds.** The card takes a *job* away, not a tenant. |
| 2 | **Summoned, not resident** — a labelled door, `Esc` or the door closes it | **Holds, and strengthens.** Today the panel is also summoned by every `+` in the product (`playlists.rs:505–508`) — it appears without its door being pressed. After §4 the door is the only thing that opens it. |
| 3 | **It exists for simultaneity** — source and destination on screen at once | **The one that moves.** The pick is no longer two-surface work. |
| 4 | **Overlays, never re-hangs the wall** | **Holds**, and frames `01`/`03` prove it: *Ochre* at x 41–281 and *Violet Ledger* at 324–564 in both, to the pixel, with 341 px of panel over one of them. |
| 5 | **Present in Library, Album and Queue; absent in Settings** | **Holds**, unchanged. |

So the verdict turns on justification 3, and the honest answer is that
**simultaneity is still the panel's reason — it has simply moved from the
pick to the drag.**

When ADR-0024 was written the drag did not exist; §6 layer 3 was the future
and the picker was the floor. The drag shipped on 2026-08-09 (doc 11 P5,
`crates/baz/src/drag.rs`), and dropping a row on a standing panel row is a
file append — *"the picker row's own act, made direct"* (ADR-0024 §6.3,
`playlist_panel.rs:400–409`). A drag needs its destination **on screen while
the hand is carrying the source**, which is the definition of simultaneity
and the one thing a place model cannot express. It is also the owner's
original verbatim ask in this feature area: *"it should be really easy to
drag a song into a playlist."*

### 5.2 The verdict

> **The panel survives, as a directory of lists and the drag's destination.
> It is never again a picker. Its tenant clause is untouched; its
> justification 3 is restated from the pick to the drag.**

What it becomes, concretely:

- **A directory.** Every list baz holds, the unnamed sounding one at its
  head as a readout, each named row a door to its page with its collage
  sleeve and counts, `New playlist` last. That is doc 09 §9's own
  prescription finally complete — *"the panel stops being a workspace and
  becomes what the first brief asked for: a way to see playlists."*
- **A drop target**, which is the resident-simultaneity job nothing else can
  do.
- **A master pane, once.** Open a playlist from it and the panel stays
  beside the page (ADR-0024 §5.5) — the one master-detail arrangement in the
  product, and the one the owner's problem-2 sentence describes. §3 declines
  to build a second one; this is why the first is worth keeping.

**Rejected alternatives**, each with the reason:

- **A Playlists place.** Refused already, and still right: *"a full-window
  list of twelve names would be the settings-rail emptiness at window
  scale"* (ADR-0024 §4). It would also destroy the drop target, since a
  place cannot be beside the thing you are dragging from.
- **A menu of lists.** A menu's items must mirror visible controls
  (`menu.rs:1–22`). If the panel went, the doors to playlist pages would
  exist only in the menu, which is the gesture-only route the mirror rule
  exists to refuse.
- **Nothing.** Then there is no visible route to a playlist's page, no
  directory, and no drop target — three refusals in one deletion.

### 5.3 What leaves the panel

- `Playlists::pending` and every `picking` branch in the view
  (`playlist_panel.rs:122–129`, `:222–241`, `:362–384`) — the hint line, the
  per-row `Add` word, and the whole-row-as-target arm.
- The panel's auto-summon on `begin_pick` (`playlists.rs:505–508`): the
  door and `Ctrl+P` become its only openings.
- `Playlists::peel`'s middle layer (`playlists.rs:481–483`): the peel order
  drops from three layers to two — the name field, then the panel — because
  a pick can no longer be in flight *inside* the panel. The card peels
  first, on its own layer, as the menu does.

`picker_order` (`playlists.rs:526–531`) survives verbatim and moves to the
card, unchanged and still tested: the ordering of the *files* was always
kept in the model *"so it is a tested fact rather than a rendering
accident"*, and the accident it is guarding against is now a different
renderer.

### 5.4 A cost this study does not fix, stated

While the panel stands it covers `INDEX_LANE_W` 108 of the wall's right
edge — the index rail and, since ADR-0028, the density detents. The wall
keeps scrolling underneath (wheel passes through, ADR-0016's mechanics), and
the panel is closed at rest, so the occlusion lasts exactly as long as a
deliberate task. After §4 that task is *browsing your lists* rather than
*every add in the product*, so the occlusion becomes rarer by the same
change. Recorded, not proposed against.
