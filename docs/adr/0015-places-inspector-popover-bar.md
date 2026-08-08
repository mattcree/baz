# ADR-0015: Places, an inspector, a popover, and the bar — retiring the rail

**Status**: accepted (2026-08-08) · implements `docs/design/01-ux-audit-and-ia.md`
§2, as amended by `docs/design/03-interface-prior-art.md` R1 / R3 / R5 ·
supersedes the "why settings are a rail panel" argument in `panels.rs`'s module
docs · changes no engine command and no protocol message · spends ADR-0014's
`JumpTo` and `UpdateQueue` · costs ADR-0006 layer 3 plus four small pure
modules, which §5 of the spec named in advance

## Context

The owner's complaint was concrete: *"an example of a strange UI is the two side
panels we have now. that seems unreasonable."* By the time the audit was
written there were three.

The right-hand rail is 340 px beside the shelf, and three unrelated subjects
took turns in it:

- **the selected album** — a thing you pointed at, in the library;
- **the play queue** — a live readout of the engine;
- **the settings** — the application's standing decisions.

They shared nothing except a width. What that cost, in the audit's own
findings:

- **A dismissal model that needs a paragraph.** `Q` toggled the queue, `Ctrl+,`
  the settings, clicking a tile raised the album, `Esc` and `✕` closed *what was
  showing* (revealing the album underneath), and `Ctrl+B` hid the rail and
  restored whatever it held. And — the rule that gave the game away — un-hiding
  an **empty** rail opened the queue, because a key whose entire job is "give
  the shelf its width back" had to invent content from somewhere.
- **Frequencies two orders of magnitude apart, argued in our own source.**
  `keys.rs` said `Ctrl+,` earns its modifier because a preferences key is
  pressed "a handful of times in a *lifetime*", and that `Q` is bare because a
  view key is pressed "dozens of times a session". Both arguments are right.
  Together they are an argument that the two surfaces should not be siblings.
- **The wrong tenant paying the cost.** The rail takes two of five columns of
  covers at the shipped window. Of the three tenants only the album panel
  *needs* the shelf beside it — you compare, you click the next sleeve. The
  queue does not; the settings certainly do not.
- **Simultaneously too narrow and too empty.** The settings panel had five
  controls and ~360 px of nothing beneath them; a soundtrack's album panel
  showed 3 of 12 tracks. Same 340 px.
- **A broken gesture.** Opening the rail reflowed the shelf under the pointer,
  so the second press of a double-click landed 180 px from the tile — while the
  panel that had just opened said "double-click a tile to play" at the bottom
  of it.

`panels.rs` was not the problem. It is careful, pure and exhaustively tested; it
was correctly implementing a model that has no answer.

## Decision

**The rail is a slot, not a place.** The fix is not to arbitrate the slot
better; it is to give baz *places*, so each thing can go where it belongs.

> **The window holds one PLACE at a time, one INSPECTOR attached to that place,
> one POPOVER attached to the transport, and the now-playing BAR always.**

Four kinds, and — the point — **one member of each kind**. There is nothing to
arbitrate, no stack to remember, and one dismissal rule per layer.

| Kind | Member | What it is | Dismissed by |
|---|---|---|---|
| **Place** | **Library** (home) | The shelf, its search, its counts | — |
| **Place** | **Settings** | Everything that is a standing decision | `Esc`, Back |
| **Inspector** | **Album** | The detail of the thing you pointed at *in* the Library | `Esc`, ✕, clicking the tile again, `Ctrl+B` |
| **Popover** | **Up next** | The queue: what the engine holds and where it is | `Esc`, `Q`, click-outside, the affordance again |
| **Bar** | **Now playing** | What is playing, where in it, how loud, what the chain is doing | never |

Five rules follow, and they are the whole of the model a listener has to learn:

1. **A place fills the window.** Places replace each other; two are never on
   screen together.
2. **An inspector belongs to a place and to nothing else.** The Library's
   inspector is the Album inspector, permanently, and it is open **exactly when
   an album is selected** — selection and visibility are one fact, which is what
   `panels.rs` already believed; it just had roommates.
3. **A popover belongs to the bar.** It overlays, it never reflows, and it is
   anchored to the control that opened it.
4. **The bar is in every place**, unchanged.
5. **`Esc` peels one layer, top down**: popover → inspector → (in Settings) back
   to the Library.

### Where each tenant went, and why

**The album stays a column, and becomes the column's only tenant.** It is the
one surface that genuinely needs the shelf beside it: the browse loop is *click,
read, click the next sleeve*, and a full-window album view turns a one-click
compare into a three-step round trip. Making it the sole tenant collapses the
rail's rule from a paragraph to a sentence — **the column is open when an album
is selected** — and lets `Ctrl+B` become an honest sidebar toggle, because there
is now exactly one sidebar.

**The queue becomes a popover from the now-playing bar.** It is not about the
library; it is about the transport, and it should live next to the thing it
describes. It is transient by construction, it never reflows the shelf, and it
is the natural home for `JumpTo` and `UpdateQueue`.

**Settings becomes a place.** It is not a glance and it is not about the shelf.
The settings that exist barely fill half a column; the ones that are coming —
output device and exclusive mode, a signal-path diagram, library roots and watch
folders, per-feature enrichment consent — do not fit in one at all. The cost is
leaving the shelf, which is the right cost: you are not browsing while you set a
pre-amp.

### What the popover is, exactly, given iced 0.13

**It is explicitly not modal, and it says so.** The toolkit has no focus
containment and publishes no accessibility tree, so a modal overlay is a claim
it cannot back. Rather than imitate one:

- `Esc` closes it, and every other binding keeps working underneath;
- the shelf still scrolls beneath it (the dismissal layer handles presses only,
  so a wheel event passes straight through);
- it is anchored **above** the bar and covers no pixel of the transport, which
  stays live and clickable;
- there is **no scrim**. Dimming ten thousand covers to show twelve rows would
  contradict the palette rationale the whole room is built on.

It is composed of three stacked layers: the place, a full-bleed
`mouse_area(…).on_press(close)` for click-outside, and the popover itself
wrapped in `opaque`, which is documented to capture mouse presses inside its
bounds precisely so events do not fall through a stack. All three primitives
were verified against `iced_widget` 0.13.4 before the surface was specified, and
driven on a headless display afterwards.

No arrow or notch: container borders here are four-sided only, so the anchor is
expressed by position and by the **Up next** control taking its raised "open"
style instead.

### The door is labelled, and it is not the now-playing text

Two amendments to the design spec, both on evidence the spec did not have
(`docs/design/03-interface-prior-art.md`, which studied sixteen products and ran
three peers headless):

- **The bar carries a visible, labelled control.** The spec offered `Q` and a
  press on the now-playing block. The study's finding is blunt: the closest
  product to baz in ambition hides the same surface behind an unlabelled
  gesture and has generated years of *"where is my queue / what did I just
  do"* complaints, and a gesture-first redesign elsewhere in the field was
  reversed after two years. **Transient must not mean unverifiable.** So the
  door says *Up next* in words, in every state including with nothing queued,
  and carries the `3 / 12` readout inside it.
- **The now-playing text is deliberately left alone.** The most-supported
  affordance in the study is *get back to what is playing* — scroll the shelf
  to the sounding album — and every product spends the now-playing block's
  click on it. baz does not have it yet. Two surfaces wanted one target, so the
  popover took the labelled control beside the text and the text stays free for
  the gesture that has no other home. Resolved on purpose rather than by
  whichever landed first.

### The queue's summary says what is left

`3 of 12 · 38:12 left`, not `3 of 12 · 51:20`. The model is MusicBee's **one
list with a cursor** — history behind, queue ahead, one surface — named here
rather than merely resembled, because it is the model a large share of baz's own
audience already knows and because naming it decides the summary: a queue is a
thing you are partway through, so the reading is a clock, not a property of the
list. Elisa's *"tracks remaining"* and MusicBee's header both do this.

### What this costs in code (ADR-0006's ledger)

Almost all of it is layer 3. The honest list of what is not:

- `overlay.rs` — `Overlay`/`Popover`: which popover, if any, is floating.
- `place.rs` — `Place`: `Library` | `Settings`.
- `selection.rs` — `panels.rs` with the roommates removed: `selected:
  Option<u64>` + `hidden: bool`. **Strictly less state than today.**
- `queue_edit.rs` — turning "remove entry *i*" into the whole path vector
  `UpdateQueue` wants. The only genuinely new logic.

Each is pure, iced-free and unit-tested. Everything else — every layout, every
surface, every style — is `views/` and `theme.rs`.

### Implementation order

Three commits, each leaving the app usable and shippable, and each of the two
that removes a surface replacing it in the same commit:

1. **Up next becomes a popover.** `Rail::Queue` → `Overlay`; the bar gains the
   now-playing affordance and the `3 / 12` slot; `Q` retargets; the top bar's
   Queue toggle goes.
2. **Queue rows become interactive.** `JumpTo` on click, a per-row ✕ via
   `UpdateQueue` through `queue_edit`.
3. **Settings becomes a place.** `Place` on the shell; `Rail::Settings` removed;
   the settings content moves verbatim into the new place's first section;
   `panels.rs` → `selection.rs`.

## Consequences

- **The rail ceases to exist as a concept.** What is left is a single-tenant
  album inspector, and the state machine that arbitrated three tenants is
  replaced by two booleans' worth of question.
- **`Ctrl+B` stops conjuring a panel.** The "un-hiding an empty rail opens the
  queue" rule is deleted by name. Un-hiding with nothing selected does nothing,
  which is what a layout key should do.
- **`Esc` means one thing per layer** instead of choosing between unrelated
  things. It is one `if` in the shell, and the top layer reports whether it
  consumed the press so the layers under it are not skipped.
- **The queue's count moved closer to its subject and stopped being stale.** The
  top bar's `Queue · 13` went on saying 13 after the run ended, because it
  reported the length of the last queue rather than what was next; the bar's
  `3 / 12` is `None` when there is no position, and the slot is reserved either
  way so the absence costs no movement.
- **`Settings` stops wrapping at narrow windows.** It was sized to the `Queue`
  toggle beside it so the pair would read as a pair, and 92 px was fitted to the
  shorter word.
- **The four properties `docs/design/01-ux-audit-and-ia.md` §5 names as
  must-not-regress each keep a test**, extended rather than replaced: the bar
  reserves every slot it can be in (now including `3 / 12`); the shelf
  virtualizes at every width the inspector can produce (now swept over the
  band, not sampled at two widths); every keyboard binding resolves to a
  message an on-screen control also sends (now checked exhaustively over
  `keys::binding_for`'s whole output, with the one exception written down); and
  no reachable state shows an inspector without an album (the exhaustive walk,
  which got simpler).
- **An accessibility gap is declared rather than designed around.** The popover
  cannot contain focus, buttons take no keyboard focus, and the shelf cannot be
  arrow-navigated. None of that is new with this change; it is stated here so
  that "the queue is a popover now" is not read as a claim that it behaves like
  a dialog.

## Open decisions this ADR does not make

**What happens when an album ends.** Recorded here because increment 8 opens the
Settings place and this is the surface the answer will live on — and because it
must not be answered silently.

Today baz emits `QueueEnded` and the bar reads *Nothing playing*. The prior-art
study (`docs/design/03-interface-prior-art.md` §5.4, R4) points out that this is
exactly what Longplay 1.0 shipped, and that its developer reversed it within one
major version because the album boundary broke the flow. Every album-first
product has had to solve it, and none solved it by doing nothing. The candidate
answers — stop; continue into a related album by some rule; shuffle on — have
different homes in the information architecture, and one of them needs
`VISION.md`'s steered shuffle to exist first.

**No behaviour changes here.** This is a product decision for the owner, not a
refactor, and inventing a continuation policy inside a layout increment would be
exactly the wrong way to make it. What increments 6–8 leave is a clean seam
rather than an answer:

- the Settings place is sectioned, and a continuation policy is a section entry
  beside ReplayGain — no layout work is owed to add it;
- the popover already renders `QueueEnded` honestly (no marked row, no invented
  position, `Nothing queued` when the record is gone), so a policy that
  continues the run needs no change to what the queue surface says;
- when it is taken, the study's shape is worth copying: ship the maximalist
  position (next/previous move between *albums*) as an **opt-in setting, not a
  default**, because baz's audience contains both the album listener and the
  shuffler.

## Deliberately deferred

- **The album as a full-window place.** It is the eventual destination — when
  the album view earns more than a column (credits, relationships, lyrics,
  editorial) it should be a Place with Back. Today it would cost the browse loop
  a three-step round trip for a one-click question and hide the shelf, which is
  the identity. The design spec's `< 940 px` regime is the *same view function*,
  so building that breakpoint later makes the promotion **a change of
  breakpoint, not a rewrite**. Nothing in this ADR forecloses it.
- **Drag-to-reorder the queue.** iced 0.13 has no pointer capture, so a drag
  that leaves the row is not tracked. It needs a hand-built widget on the
  `groove.rs` pattern — the precedent for "we need pointer geometry, so we wrote
  the widget" — and it is its own increment. Click-to-jump and remove ship
  first because they need no new widget.
- **A second popover.** `Popover` has one variant. The type earns its place by
  making "two are open at once" a state that does not exist, and by being where
  a second one would have to declare itself rather than adding a parallel flag.
- **A full queue surface.** A 360 px overlay is the right home for an album
  queue and the wrong home for a shuffle queue you are steering, and two
  products in the study ran that reversal (hidden → persistent) in public. The
  growth path is named the way the album-becomes-a-place path is named: **when
  the queue stops being an album** — after shuffle and radio, which `VISION.md`
  commits to for v0.3 — the popover gains a door to a full surface, and its rows
  become two-level (albums collapsible to tracks). No code today.
- **Get back to what is playing.** Band A in the study and absent from baz. The
  target is reserved for it (above); the gesture is a separate increment.
- **Inspector responsiveness** (the width band, the replace-the-shelf regime
  below 940 px, whole-panel scroll below 700 px of height), the **bottom bar's
  maximum left-zone width**, the **search field's inline ✕ and the startup
  focus**, and the **Settings → Playback → signal-path readout** are all
  specified in §4 and §5 of the design document and are separate increments.
  They are consequences of this decision, not part of it.
