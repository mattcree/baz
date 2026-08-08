# ADR-0017: One design direction — resolving the audit, the visual language and the critique

**Status**: accepted (2026-08-08) · supersedes parts of `docs/design/01-ux-audit-and-ia.md`
and `docs/design/02-visual-language.md` named item by item below · adopts most of
`docs/design/critique/` · answers ADR-0016's open decision *"what happens when an
album ends"* · amends ADR-0006's cost claim and ADR-0008's grouping key · is the
build order for everything that follows

---

## Context

Three specifications exist and they contradict each other.

1. **`docs/design/01-ux-audit-and-ia.md`** — our UX audit and information
   architecture. Increments 1–8 have **shipped** (ADR-0016): the queue is a
   popover from a labelled `Up next 3 / 12` control in the bar, Settings is a
   *place*, `panels.rs` became `selection.rs`, Previous exists, a track row plays
   from there, the playing track is dotted in the inspector.
2. **`docs/design/02-visual-language.md`** + `.interface-design/system.md` — the
   gallery direction. **Phase A has shipped**: the monospace and the serif are
   deleted, the near-black palette is in (`#0C0D0E` wall / `#060708` recess /
   `#141517` plinth / `#1C1D20` plinth-lit), the inks are corrected, radii and
   leadings are set, the reserved slots are re-derived in the face that draws
   them. Phases B and C are not built.
3. **`docs/design/critique/`** — an independent design session (Claude Design)
   commissioned after seeing the shipped app. It goes further than ours: it is a
   product design, not only a visual system.

The owner's reaction to (3) was that it is *better than what we produced* and is
*a product design, not only a visual system*. Both halves of that are true, and
the second half is why it cannot simply be adopted: a product design proposes
things that ADR-0006 never promised would be cheap, and it proposes removing
three controls whose removal we have direct evidence against.

This ADR decides, item by item, and leaves one target.

**Standing judgement.** Where the critique is better it supersedes us and we say
so plainly. Where it is wrong *for this codebase* we defend ours with reasons.
The critique was written against the app and its toolkit constraints, but it was
**not given `docs/design/03-interface-prior-art.md`** — sixteen products
surveyed, three installed and rendered — so three of its proposals argue from
first principles against evidence we already hold. That is the single most
common shape of disagreement below, and it is why the disagreements are narrow
and the agreements are broad.

---

## 1. The decisions

### 1.1 Transport — adopt the needle, keep the buttons, delete the seek row

**The critique**: no transport bar at all. A 2 px needle flush on the window's
bottom edge, segmented by the album's real track lengths, 2 px gaps at track
boundaries and 6 px at a side break; click a segment to jump; prev/next stop
existing; transport glyphs appear over the playing cover on hover.

**We do**: build the segmented needle and give it the seek row's whole job.
Delete the 260 px groove, the 52 px timestamps' row position and the 15 px hover
preview lane from the bar. **Keep Previous · Play/Pause · Next.** Keep the wall
label, the `3 / 12` slot, the signal note, volume, mute and the labelled `Up
next` door. The bar goes from 102 px to **58 px** and the needle takes 2 px at
the window's bottom edge.

```
  1  rule (HAIRLINE)
 12  GAP_MD
 32  TRANSPORT_HIT      — Previous · Play/Pause · Next, and the volume block
 12  GAP_MD
---
 58   + 2 px needle, flush on the bottom edge
```

Generalise the needle past the critique's framing while building it: baz's queue
is **one list with a cursor** (ADR-0016), not always an album, so the segments
are *queue entries*, the 6 px gap falls at an **album boundary** rather than a
side break, and click-to-jump is ADR-0014's `JumpTo` at a segment index. The
critique's spec is the album case of this one. Elapsed and total move into the
bar's left zone in their existing `STAMP_W` 52 slots, beside the wall label.

**Why the needle.** It is the best single idea in any of the three documents. It
states position *and* structure in the same 2 px — you can see that you are
three minutes into a nine-minute closer, which no scalar groove has ever said —
it costs the collection 2 px instead of 45, and it makes track navigation
spatial instead of stepwise. `docs/design/02-visual-language.md` §6.5 called the
bar "the best thing in the product"; the needle is better at the one thing the
bar spent 45 of its 102 px on.

**Why the buttons stay.**

- **Our own prior-art study, R11**: three vendors bought "visual calm" by
  removing control density inside two years and **all three reversed**, and what
  was lost was always *position, provenance and skip*. `02` §6.5 turned that
  into a rule this project already accepted: *a slot may be added to this bar;
  none may be removed for tidiness.* Skip is exactly what is being removed here.
- **Our own audit** found "There is no Previous" was *"the most-missed control
  in the app"*, and we shipped it two commits ago along with MPRIS
  `CanGoPrevious`.
- **The hover-reveal has a hole the critique does not close.** Transport glyphs
  over the playing cover require the playing cover to be *on screen*. At
  Marta's 40 000 albums the wall is ~3.5 million pixels of scroll; after a
  filter the playing album may not be in the result set at all. The critique's
  own build guide lists this affordance as an untested open question ("crisp or
  broken? Test the hard cut") — it is shipping the product's only pointer-
  reachable transport on an affordance it flags as unvalidated.
- **§4's visible-control rule** forbids it outright.

**Cost.**
- New hand-built widget, `crates/baz/src/needle.rs`, on the `groove.rs` pattern
  (iced's `advanced::Widget`; hit-test on segment index rather than a scalar
  fraction). Estimate ~350–450 lines with tests. `groove.rs` (891 lines) keeps
  the volume instance and loses nothing.
- `crates/baz/src/views/bottom_bar.rs` (717 lines) loses roughly 250 — `seek_row`,
  `preview_lane`, the seek message plumbing.
- `crates/baz/src/theme.rs`: `SEEK_W` 260, `SEEK_ROW_H` 37, `SEEK_ROW_W` 380,
  `PREVIEW_H` 15 lose their meaning; `NEEDLE_H` 2, `NEEDLE_HIT` 22,
  `SEGMENT_GAP` 2, `ALBUM_GAP` 6 arrive.
- The bar-height invariant test at `views/bottom_bar.rs:659` pins
  `CENTRE_H = TRANSPORT_HIT + GAP_SM + SEEK_ROW_H`. It is rewritten, not
  deleted: the property (nothing in the bar moves when the music moves) is the
  thing worth keeping and it now has fewer slots to hold.
- `crates/baz/src/player.rs` (4457 lines, ADR-0006 layer 1) exposes `SeekBar`
  shaped for the groove. It gains a segment list derived from queue durations.
  **This is layer 1 changing because the view changed** — see §5.

**What we give up.** The critique's bottom furniture is ~32 px; ours is 60. We
concede 28 px (3.3 % of an 860 px window) to keep skip pointer-reachable. Against
today we recover 44 px, and the collection's share at 1280 × 860 with no
inspector goes from 81.6 % to **86.5 %**.

---

### 1.2 Find — keep the field, adopt type-anywhere, move the bare letters

**The critique**: no search field exists; any bare keystroke filters the wall;
the query renders as ~48 px display type bottom-left with a match count; Enter
plays the first match; all other shortcuts move to a modifier layer.

**We do**: **adopt type-anywhere. Keep the field. Reject the 48 px poster
query.** A bare printable character with nothing focused routes into the query
*and* focuses the search well, so the first keystroke both filters and lands
somewhere visible. `n`, `m` and `q` give up their bare bindings and move to the
modifier layer. `/` and `Ctrl+F` survive as the explicit door. `Esc` clears, then
blurs.

**Why type-anywhere wins.** Our audit rejected it — *"type-ahead search cannot
coexist with bare-letter transport bindings; the transport wins"* — and `02`
§2.8 restated the rejection. **That resolution is superseded.** The frequency
argument runs the other way: on a 40 000-album wall, filtering is the primary
act of navigation and muting is not, and `n`/`m`/`q` are baz's own inventions,
argued in `keys.rs`, not muscle memory inherited from any player we surveyed.
The critique's friction budget — *keystroke → filtered wall = next frame* — is
the right budget, and a door you must open first is a click before sound.

**Why the field stays.** This is not taste; it is the only load-bearing widget
in the application.

- `text_input` is the **only widget in baz that takes keyboard focus** and the
  only one that draws a focus ring (`PAPER_RING`, `text_input` only, `02` §5.1).
  Delete it and baz has zero focusable widgets and no focus ring anywhere.
- It is the only thing an AccessKit adapter would have to describe if iced ever
  publishes a tree. See §4: a design with no fields and no buttons has nothing
  for an accessibility tree to expose, so the removals do not merely make today
  worse — they make the eventual fix more expensive.
- The critique itself flags *"'No search box' onboarding — one quiet first-run
  hint, once, or nothing"* as unresolved. A visible well answers it for free.

**Why not the 48 px poster query.** Two reasons, one of them a defect in the
critique. First, `02` §3.2 reserves poster sizes (40 px+) for the *work*
(Marquee, the first-run question); chrome at 48 px inverts the direction's own
hierarchy. Second, **the critique puts two different things bottom-left at
once**: `02-surfaces.md` places the ~48 px query bottom-left *and* the 11 px wall
label ("Title — Artist · elapsed") bottom-left. You filter while music is
playing constantly. They collide.

**A second internal contradiction, named.** `02-surfaces.md` says *"All other
shortcuts on a modifier layer, since bare letters are query"* and, four lines
later, *"Keys: space play/pause, left/right seek, up/down or scroll volume, **M
mute**"*. `M` is a bare letter. We resolve it the consistent way: `Ctrl+M` mutes,
and the mute glyph in the bar remains the pointer route.

**The keyboard table after this decision:**

| Key | Meaning |
|---|---|
| any bare printable character | append to the query, focus the well, filter next frame |
| `Space` | play/pause (when the query is empty or the well is unfocused) |
| `Enter` | play the top-ranked match; play the selected album |
| `Esc` | clear the query; then blur; then peel the layer stack (popover → inspector → back from Settings) |
| `←` `→` / `Shift`+ | seek ±5 s / ±30 s |
| `↑` `↓` | volume |
| `/` , `Ctrl+F` | focus the well explicitly |
| `Ctrl+←` `Ctrl+→` | previous / next |
| `Ctrl+M` | mute (was bare `M`) |
| `Ctrl+U` | Up next popover (was bare `Q`) |
| `Ctrl+B` | hide/restore the inspector |
| `Ctrl+,` | Settings place |
| `Ctrl+-` `Ctrl+=` | density step down / up |
| `Ctrl+R` | the pull |
| `1` `2` | Wall / Marquee lens — **exception**: adopted from the critique, and the one place bare characters are not query. Digits are not letters and no album title begins with one often enough to matter; the tradeoff is stated rather than discovered. |

**Cost.** `crates/baz/src/keys.rs` is 645 lines of which ~400 are the module's
argument for each key and an exhaustive test matrix. All of it is re-argued and
rewritten. The module stays pure and iced-free (ADR-0006 layer 1 holds
structurally), but this is the most expensive test rewrite in the plan. Plus a
new `Focus` path in `app.rs`: today `keys::binding_for` returns `None` for every
key while the well has focus; it now must distinguish "the well has focus and
this is text" from "the well has focus and this is `Ctrl+…`".

**One consequence in `baz-core`.** "Enter plays the first match" is only
defensible if the first match is the best match. `Library::search`
(`crates/baz-core/src/index.rs:656`) returns results in **corpus order, which is
library order**, explicitly not ranked. Making find the primary navigation makes
that a defect. Ranking is step 12.

---

### 1.3 Settings — stays a place

**The critique**: not a screen; a small panel over the wall (music folder,
appearance, scrobbling, output device). *"If it grows past one panel, something
upstream went wrong."*

**We do**: **defend ours. Settings stays a place.** Not superseded.

**Why.** The sentence "if it grows past one panel, something upstream went
wrong" is a good rule written without knowledge of what is committed. What is
already committed, in `docs/VISION.md`:

- **Pillar 5, opt-in enrichment**: *"Every fetch explicit, cached, individually
  toggleable, off by default"* — Wikipedia/Wikidata, TheAudioDB, Cover Art
  Archive, fanart.tv, ListenBrainz, Last.fm, LRCLIB. Seven consent toggles, each
  needing a provenance note. That is a panel on its own.
- **Karl's signal path**: a *diagram* of the direct chain (source rate/depth →
  gain stage → output rate → shared or exclusive), named in the backlog as
  missing. Not a caption.
- Output device and exclusive mode; library roots and watch folders; ReplayGain's
  five existing keys; the room picker (§1.5).

That is the exact junk-drawer the audit diagnosed, re-created. And the cost of
reversal is real: `crates/baz/src/place.rs` (159 lines) and
`crates/baz/src/views/settings.rs` (437 lines, with `SECTIONS`,
`SETTINGS_NAV_W` 200, `SETTINGS_CONTENT_W` 640, `SETTINGS_BREAKPOINT` 1000)
would be unshipped to build something smaller that must grow back.

**What we adopt from the critique instead — and it is the better half of its
argument.** *Settings must never be the answer to a **view** question.* So:

- **Density becomes a zoom gesture, not a Settings row.** This **supersedes
  `02` §2.7's** placement ("Settings → Appearance"). The three named steps
  survive as data — `02`'s evidence that removing a density level does durable
  damage stands — but the control is `Ctrl+-` / `Ctrl+=` and `Ctrl+scroll`, and
  the current step persists in config as *state*, not as a preference the user
  goes somewhere to set.
- **No view-options menus**, adopted as a refusal (§6): no grid-size picker, no
  list-mode toggle, no column chooser, no sort dropdown. Group keys are a row of
  words; the lens switcher is two words; density is a gesture.

---

### 1.4 Captions at rest — kept

**The critique**: no captions at rest on the wall; selected gains a caption
(title / artist-year), playing gains caption + dot + halo.

**We do**: **defend ours. Every tile keeps its two-line wall label.** Not
superseded.

**Why.** Three reasons, the first of which the critique does not answer.

1. **A black sleeve on a black wall is nothing.** `02` §1.3 committed to the
   merge deliberately — *"the covers that vanish are the ones whose designers
   chose black"* — but it survives only because **the grid's structure is
   carried by the labels, not by the sleeves' edges**. Delete the labels and a
   near-black cover on `#0C0D0E` has no anchor, no hit-target hint, and no
   evidence it is there at all. The critique bans borders on artwork (correctly)
   and bans captions at rest, which leaves nothing.
2. **It hollows out the critique's own best claim.** *"The shelf contains
   exactly two kinds of thing: artwork and type."* With no captions at rest, at
   rest the wall contains one kind of thing, and the claim is about the selected
   state only.
3. **Identification at scale.** 40 000 albums include bootlegs, live sets and
   reissues with identical or text-free art. A hang you cannot read is a hang
   you must click through.

**And keeping it earns something.** The critique wants shift-click-to-stack
marked by a *"numeral chip on the cover"* — which violates its own foundations
and `02` §4.1 (*nothing is ever drawn on top of a sleeve*). Because we have a
label, the chip goes in the label's first line, where the lamp dot goes. The
caption is the reason that contradiction is resolvable.

**Cost of refusing.** We decline "wall at rest = 100 % collection" purity. The
label block costs `GAP_LG` 16 + `LABEL_H` 36.4 = 52.4 px of a 240–320 px row
pitch — about 18 % fewer rows on screen than a captionless wall. Stated, and
paid.

---

### 1.5 Rooms — adopt the model, ship two, defer two

**The critique**: four rooms (Closing Time / Stone / Plaster / Reading Room)
following the OS, with a dead zone (never oklch L .45–.58) and per-room accents
(amber in dark rooms, oxblood in light).

**We do**: **adopt the room model and its structure; supersede `02` §10's
"still defer".** Ship **Closing Time** (unchanged values, ours) and **Reading
Room**, follow-system. **Stone and Plaster are deferred** as manual picks.
Adopt the L .45–.58 dead zone verbatim as a rule.

**Why adopt.** `02` §10 deferred a light variant on three arguments; the
critique answers two and a half of them, which we did not expect and should say:

| `02` §10 objection | Critique's answer |
|---|---|
| The amber halo has almost no contrast on a paper ground | **Answered.** Per-room accent: oxblood `oklch(0.50 0.14 35)` in light rooms — a different mark, not a recoloured one. |
| A light variant must switch depth strategies, not just values | **Answered.** *"Surfaces rise toward the lamp: lighter+warmer in dark rooms, darker in light rooms; recesses invert."* |
| Pale sleeves disappear on a paper ground and would need a hairline the dark variant refuses | **Not answered.** The critique's build guide flags exactly this ("mid-value sleeve melt … validate on real libraries; remedy = nudge room L, never borders on art"). |

So Reading Room ships **only when the third question has an answer**, and the
answer may not be a border on artwork. Stone and Plaster wait because four rooms
is four times the visual QA on a product with no automated screenshot diffing,
and because they are manual picks nobody has asked for.

**Why the mechanism lands early regardless (step 2 of the plan).** Today every
colour is a `pub const Color` in `theme.rs` and ~30 style functions close over
those constants. Every per-surface style in `02`'s Phase C would be written
against constants and then rewritten against a palette. That is precisely
"building on a foundation about to be replaced". `02` §10 costed the indirection
at half a day. It goes first, with Closing Time as the only resolvable room.

**Cost.** `theme.rs` (2327 lines): every `pub const Color` becomes a field on a
`Palette` resolved at startup; ~30 style functions take the palette; ~10 tests
that assert against constants become assertions over each room. The dark-light
system-preference read already exists in the process — `iced_core` pulls
`dark-light`, which asks the desktop portal over the D-Bus stack baz already
links (see the `zbus` note in `Cargo.toml`) — so following the OS costs no new
dependency.

---

### 1.6 Contrast — both rules govern, over disjoint domains

**The critique**: *"Each step ≥ 0.03 oklch L. WCAG ratios are meaningless at
these lightnesses; do not use them here."*

**We shipped**: `every_ink_clears_its_contrast_floor_on_every_surface_it_lands_on`
(`crates/baz/src/theme.rs:1863`) — WCAG 2.1 relative luminance computed, not
estimated; 4.5 : 1 for text, 3.0 : 1 for a non-text mark; 25 asserted pairings
over `WALL` / `PLINTH` / `RECESS` / `PLINTH_LIT`.

**Decision: they are not in conflict, because they measure different things, and
both are adopted as assertions. The shipped test stays and grows.**

The critique's sentence is **true of surface-versus-surface and false of
ink-versus-surface**, and it states the true half as a global licence.
`WALL` against `PLINTH` is 1.30 : 1 and that number tells you nothing —
oklch-L is the right instrument. `PAPER` against `WALL` is 15.33 : 1 and that
number is the entire reason `PAPER_FAINT` and `PAPER_MUTED` were corrected in
v0.1, when they shipped at 3.4 : 1 and 1.9 : 1.

**The measurement that settles it.** Our four shipped planes, in oklch L:

| Surface | Hex | oklch L | Step |
|---|---|---|---|
| `RECESS` | `#060708` | 0.1276 | — |
| `WALL` | `#0C0D0E` | 0.1583 | **+0.0308** |
| `PLINTH` | `#141517` | 0.1955 | **+0.0371** |
| `PLINTH_LIT` | `#1C1D20` | 0.2309 | **+0.0354** |

**Our shipped surfaces already satisfy the critique's elevation law**, on all
three steps, without having been designed to. Meanwhile the critique's own
Closing Time room fails it: `#070809` → `#0C0D0E` is **+0.0248**, below its own
0.03 floor. So we keep our four values, and we adopt its rule as an assertion —
which is the cheapest possible way to gain a good rule.

**And the ink hierarchy is where deleting WCAG would have cost us.** The
critique specifies *"ink opacity is the hierarchy: 100 % names, 65 % working
text, 40–45 % metadata/labels, 35 % disabled."* Composited over each room's wall
and measured:

| Room | ink @ 65 % | @ 40 % | @ 35 % |
|---|---|---|---|
| Closing Time | 6.83 | **3.24** | **2.74** |
| Stone | **4.19** | **2.55** | **2.29** |
| Plaster | **3.61** | **2.09** | **1.90** |
| Reading Room | 5.09 | **2.45** | **2.15** |

The 40 % tier is shelf headers, group keys, the index rail's present letters,
album counts and every label in the app — the "only chrome voice" the critique
names. It lands between 2.09 : 1 and 3.24 : 1 in every room. Ten of the twelve
cells above are below the 4.5 : 1 text floor and five are below the 3.0 : 1 mark
floor. **The rule that would have caught this is the rule being deleted.**

**What happens to the shipped test.** It is kept, renamed to
`every_ink_and_every_surface_clears_its_floor`, and extended three ways:

1. **Adopt the oklch-L step law.** A new assertion: adjacent surface levels in
   every room differ by ≥ 0.03 oklch L, and no room's wall sits in L .45–.58.
   Requires an oklab conversion in the test module (~25 lines, no dependency).
2. **Composite before you measure.** An opacity is a colour once it is drawn. The
   ink ramp is resolved against each surface it can land on *before* the ratio is
   taken, so an opacity-expressed hierarchy cannot smuggle an unreadable value
   past a test that only sees opaque tokens. This is the concrete defence against
   the failure in the table above.
3. **Sweep rooms.** When a second room lands the sweep becomes room × ink ×
   surface. 25 pairings today; ~50 with two rooms.

**One bounded concession to the critique.** For **non-text marks that exist only
to be locatable and are never read** — the hairline edges (`PAPER` @ 7 % / 15 %),
the needle's unfilled track, the index rail's *absent* letters — WCAG's 3 : 1
mark floor is the wrong instrument, and they are exempted **by name, in a list in
the test**, governed by the oklch-L step rule instead. Anything a user reads,
including `PAPER_MUTED` at 3.60 : 1, keeps its floor. An exemption list you must
add a name to is a rule; "WCAG is meaningless here" is not.

**Also kept unchanged**: `the_lamp_is_spent_only_on_playback_truth` and
`the_lamp_is_named_only_where_playback_truth_is_drawn`. The critique's accent law
is ours restated and it is enforced in code already.

---

### 1.7 The things the critique brings that we lack

| Item | Verdict |
|---|---|
| **History ledger** — append-only plain local file, one line per play, written from the first beta with zero UI | **Adopt, first.** Its sequencing claim is the strongest in any of the three documents and it is correct: `baz-core` has *no* history of any kind today (no play counts, no `last_played`, no timestamps), so it cannot be backfilled. PLAYED, the inspector card, the pull and shuffle weighting all feed on it. In flight in a parallel agent. |
| **Shuffle draws only from what the wall shows; pool visible (non-pool covers dim to 35 %, next two draws carry faint ink rings)** | **Adopt.** A strictly better specification of `VISION.md` pillar 4, and it is nearly free: `vm::matching_album_ids()` already computes "what the wall shows". The code for the critique's rule exists under another name. |
| **Group keys** — ARTIST / YEAR / GENRE / ADDED / PLAYED as one row of words, genre verbatim from tags | **Adopt.** It replaces "the shelf has one sort and no facets", which our own audit called an IA problem. **This is the largest breach of ADR-0006 in the plan** — see §5. |
| **Index rail** — 36 px type-only lane, a pure projection of the active group key, no state of its own | **Adopt, and it supersedes `02` §2.8's spine index.** Ours was `#`+A–Z at `INDEX_W` 20 and had to be re-specified for every future grouping. The critique's derives from the key and therefore never needs re-specifying: ARTIST → A–Z, YEAR → decades, GENRE → names, ADDED/PLAYED → recency buckets. `INDEX_W` becomes 36. |
| **The stack** — shift-click queues a sleeve or a track; ephemeral; clears when it ends; albums listed as albums | **Adopt**, with the numeral chip moved off the artwork into the wall label's first line (§1.4). Our queue is already one-list-with-a-cursor and `UpdateQueue` exists, so this is a view + `vm` change. |
| **Lenses — Wall / Marquee**, keys 1/2 | **Adopt the lens. Reject the idle auto-switch.** The critique makes Marquee *"default after ~30 s idle while playing"*. Nothing in baz may change what is on screen without being asked — it is the "lobby" failure mode the critique itself names, arriving by itself, and it contradicts the refusal that nothing begins that the user did not begin. Marquee is a lens you press `2` for. |
| **The pull** (`Ctrl+R`) — one sleeve weighted toward long-unplayed, in Marquee, nothing plays until Space | **Adopt**, after history and Marquee. Distinctive, cheap once the ledger exists, and it respects the friction budget by not playing anything. |
| **Refusals ledger** | **Adopt.** See §6. |
| **Crate lens, mixtapes, crates, overview zoom stop** | **Defer**, as the critique does. Each slots into machinery this plan builds: a crate is a group key, a lens is a word, a mixtape is the stack's save path. |
| **Art-derived accent hue** | **Defer**, as both documents already do (`02` Phase C7, critique "labelled experiment only"). It is the last item, because everything else must be true before it means anything. |

---

## 2. What this answers that was left open

**ADR-0016's open decision — "what happens when an album ends".** It is
answered by the refusals ledger: **the queue empties and there is silence.**
Silence is a feature. No autoplay, no radio, no continuation rule.

This is a real decision and it costs something. Our prior-art study (R4) found
Longplay 1.0 shipped exactly this and reversed it within one major version. We
take the other side, and here is why the reversal does not bind us: Longplay had
no *stack*. In baz, continuing past an album is a thing you can ask for before
it happens (shift-click stacks the next sleeve) and a thing you can ask for
after (shuffle from what the wall shows). The refusal is not "you may not
continue"; it is "the software will not decide to continue for you". ADR-0016
suggested that if the maximalist position were ever taken it should ship as an
opt-in setting; this ADR declines to take it, and the seam ADR-0016 left is
simply not spent.

---

## 3. What is superseded, and what it costs in code

Every superseded decision, the code that must change, and the honest cost.

| Superseded | By | Code that changes | Cost |
|---|---|---|---|
| `01` §4.8 *"type-ahead search cannot coexist with bare-letter transport"* | §1.2 | `keys.rs` (645 lines, ~400 of doc + exhaustive tests) rewritten; a text-vs-chord branch in `app.rs` | High — the largest test rewrite in the plan; the logic is small |
| `01` §4.2 / `02` §6.5 the 102 px bar with a seek row | §1.1 | new `needle.rs` (~400); `views/bottom_bar.rs` −250; `theme.rs` slot tokens; `player.rs` gains a segment list; the `CENTRE_H` test rewritten | High — and one of the two ADR-0006 breaches |
| `02` §2.7 density placed in Settings → Appearance | §1.3 | `shelf.rs` (pure) takes the step; `keys.rs`; `config.rs` gains one key; **no** Settings row | Low |
| `02` §2.8 the `#`+A–Z spine index at `INDEX_W` 20 | §1.7 | `shelf.rs`, `views/shelf.rs`; `INDEX_W` 20 → 36; the rail reads the active group key | Medium, and it replaces work not yet built |
| `02` §10 *"a light variant: still defer"* | §1.5 | `theme.rs` `pub const Color` → `Palette`; ~30 style fns; ~10 tests | Medium — half a day of mechanism, then a design question per room |
| `02` §5.2's implicit claim that WCAG alone governs | §1.6 | `theme.rs` test module: oklab conversion, composited ink ramp, named exemption list, room sweep | Low |
| ADR-0008's grouping as the only grouping | §1.7 | `baz-core/src/index.rs:702–748` (`Library::albums`) takes a key; schema gains genre and first-seen columns | **High, and in `baz-core`** |
| `01` §1.2's "the shelf has one sort and no facets" as a scope call | §1.7 | as above | — |

**Not superseded**, and defended above: Settings as a place (§1.3); captions at
rest (§1.4); Previous and Next as buttons (§1.1); the search field (§1.2); the
places/inspector/popover/bar model of ADR-0016 in full; the accent law; the
four surface values; `02`'s hang arithmetic, reserved slots, three-face bundle
and 0 ms motion.

**One item declined on cost rather than principle**, stated so it is not
mistaken for an oversight: the critique bans radii outright. `02` already brought
them down to `RADIUS_CTRL` 4 / `RADIUS_SEGMENT` 3 and deleted `RADIUS_TILE`, and
artwork and the wall are already at 0. Taking controls to 0 as well is a taste
call worth less than the churn across ~30 style functions. Radii stay as shipped.

---

## 4. Accessibility — the stance for 1.0

The critique lists this as an open question. It is closer to a blocker, and
leaving it open is not available.

**The facts.** iced 0.13 publishes **no accessibility tree**. Buttons take **no
keyboard focus**. Screen-reader support is **zero**, today and after this plan.
The only focusable widget in the application is the search field. Every removal
the critique proposes — the search field, the transport buttons, the settings
screen — deletes a visible, pointer-reachable, labelled control and replaces it
with an invisible keyboard convention. Each removal makes it worse, and they
compound.

**The position for 1.0, in four parts.**

**(a) baz 1.0 ships with no screen-reader support, and says so where a user
deciding whether to install will see it** — the README, the release notes, and
an `About → Accessibility` section in the Settings place. Not declared in an ADR
and left there. ADR-0005 chose iced knowing this ("AccessKit-dependent
accessibility" is in its accepted costs); the honest form of that choice is to
publish it, not to inherit it quietly.

**(b) The visible-control rule, binding on every future surface.**

> **Every action in baz has a visible, pointer-reachable control. No action is
> keyboard-only, and no control's only affordance is hover.**

This is the rule that kills the needle-only transport, the field-less search and
the hover-reveal-over-the-cover glyphs. It is the mitigation, and it is written
as a **refusal** (§6) so it constrains work that has not been proposed yet. It
costs the critique's proportion purity — the 28 px in §1.1, the search well's
360 px in the top bar — and that is the price of the stance.

**(c) The guarantees baz *can* make are honoured exactly and tested.** Since the
tree is absent, contrast and hit targets are all that is left, which is a reason
to be strict rather than a reason to shrug (`02` §5.2 said this and it is
upheld). Concretely: the contrast test of §1.6 stays and grows; minimum hit
target is `TRANSPORT_HIT` 32 and gets an assertion; every icon-only control
carries a tooltip that is its accessible name in waiting (already shipped); and
**no state is signalled by colour alone** — the lamp dot *replaces* the track
number rather than tinting it, the halo is accompanied by a dot, the pool that
shuffle draws from is marked by dimming *and* by rings.

**(d) AccessKit is the real answer, and it is a dependency decision, not a
design one.** It is recorded as the single largest known defect in the product.
Revisit at the first iced release exposing an accessibility tree; until then, **do
not let the design drift further from a tree**. That is the argument that
matters most and it is not aesthetic: an adapter can expose a named button and a
text field. It cannot expose a 2 px line at the bottom of a window whose meaning
is positional, or a wall that responds to bare letters with no field. **The
critique's removals do not merely make 1.0 worse for keyboard users; they make
the eventual fix expensive.** Keeping the buttons and the field is, among other
things, keeping the surface an adapter would attach to.

**What we accept.** A blind user cannot use baz 1.0. We accept that, we publish
it, and we decline to make it worse for visual purity.

---

## 5. ADR-0006, tested honestly

ADR-0006 promised that a redesign costs **layer 3 and nothing else** — view
composition, plus tokens in `theme.rs`. ADR-0016 reported that it very nearly
held: four small pure modules and everything else in `views/`. Tested against
*this* plan, it holds for restyles and relayouts and **fails for everything the
critique adds that is a product feature**.

**Where it holds.**
- The tile, the inspector, the popover, the Settings place, the bar's
  composition, every surface style, the hang, the density steps, the Marquee
  lens: `views/` + `theme.rs` + `shelf.rs` (pure). Layer 1 is genuinely
  iced-free — `grep -rn "iced" crates/baz-core/src/` returns two hits, both
  prose in doc comments — and `vm.rs` (1311 lines) is a real boundary.

**Where it fails, in descending order of cost.**

1. **The history ledger is new construction in `baz-core`** — schema, write
   path, and protocol vocabulary. Nothing in `baz-core` records a listening
   event today. Not a view change by any reading.
2. **Group keys change `baz-core`.** `Library::albums()`
   (`index.rs:702–748`) groups by album artist + album, case-folded, with
   editions ranked underneath (ADR-0007/0008). YEAR, GENRE, ADDED and PLAYED are
   new grouping dimensions plus new persisted columns (genre; first-seen; and the
   ledger for PLAYED). **ADR-0006 never anticipated that a "redesign" would ask
   the library for a different shape of answer** — and that is exactly what "the
   collection is the interface" means when taken seriously.
3. **Search ordering changes `baz-core`.** `Library::search` returns corpus
   order and documents that it needs no re-sorting. Making find the primary
   navigation makes ranking a requirement (§1.2).
4. **The needle is layer 3 by the letter and not by the spirit.** A ~400-line
   hand-built `Widget` with pointer geometry and its own tests is not
   "disposable view composition". It is the *second* such widget after
   `groove.rs`, which means hand-built widgets are now the norm for anything
   with pointer semantics, not the exception ADR-0005 treated them as.
5. **Layer 1 is shaped by the view more than admitted.** `player.rs` (4457
   lines) exposes `SeekBar`, `playing_row_in()` and glyph/enabled readings
   fitted to the current bar. Deleting the seek row changes what a layer-1
   module must publish. Pure and testable, but not inert.
6. **`theme.rs`'s geometry is load-bearing in tests by design.** Roughly ten
   tests in `theme.rs` and `views/bottom_bar.rs` assert reserved-slot
   invariants. That is a feature — it is how "nothing moves when playback
   starts" is enforced — but it means changing sizes is a test-suite
   conversation, not a constant edit. `theme.rs` is 2327 lines, ~60 % of it
   geometry.

**Amendment to ADR-0006, adopted here.** Its claim is narrowed and kept:

> A **redesign** — different surfaces, different composition, different tokens —
> costs layer 3 and `theme.rs`. A **product change** — new questions asked of
> the library, new facts persisted, new pointer semantics — costs `baz-core`
> and hand-built widgets, and calling it a redesign does not make it cheap. The
> layering is what makes the first kind cheap; it was never a promise about the
> second.

The critique is a product design, not only a visual system. That is the reason
it is better than what we produced, and it is the reason it breaches ADR-0006.
Both facts are the same fact.

---

## 6. The refusals ledger — adopted, and where it lives

**Adopted.** It is the most product-defining thing in any of the three
documents: a list of things considered and rejected *on principle*, where
re-opening one requires beating the argument rather than merely wanting to.

**It lives in [`docs/REFUSALS.md`](../REFUSALS.md)**, which ships with this ADR
— a standing document, not a section of this ADR and not an ADR of its own.
Reasons: an ADR records one decision at one moment and is never edited; the
ledger must be added to as the product grows.
It is linked from `docs/VISION.md`'s "Refuse (the fixes)" and "Betrayal list"
sections, which are the same genre and were the ledger's ancestors, and from the
README.

**Its editing rule, which is what gives it teeth:** *an entry leaves the ledger
only by an ADR that beats its argument.* Adding an entry needs only a pull
request; removing one needs a decision record.

**The entries, at adoption.** From the critique, adopted:

- No autoplay, no radio. The queue empties and there is silence. **Silence is a
  feature.** (Gloss: shuffle is a thing you *start*, never a thing that starts
  itself. `VISION.md` pillar 4's steered shuffle survives as an explicit
  gesture.)
- No invisible shuffle pools. Shuffle draws only from what the wall shows, and
  the pool is visible.
- No auto-generated playlists. Every crate and mixtape is made by a person.
- No engagement stats. No Wrapped, no streaks, no charts. **History records; it
  never performs.** (Binding on the ledger work now in flight: the inspector's
  card is "PLAYED — N times since YYYY" plus date stamps, and nothing else.)
- No user-picked accent colour. (The art-derived lamp is data, not a
  preference; its off switch is binary, not a colour picker.)
- No view-options menus: no grid-size picker, no list-mode toggle, no column
  chooser, no sort dropdown.
- Nothing at oklch L .45–.58 for a room; no borders on artwork; no shadows
  except the playing halo; no motion — hard cuts by design.

From the critique, **rejected** — recorded as rejected, with the argument, so
the ledger's own rule is honoured:

- *No captions at rest on the wall.* Beaten by §1.4: without labels the grid has
  no structure and a black sleeve on a black wall has no anchor.
- *No radii.* Declined on churn, not principle (§3).

Ours, added:

- **No action is keyboard-only, and no control's only affordance is hover** —
  the visible-control rule (§4).
- No state signalled by colour alone.
- Nothing is ever drawn on top of a sleeve.
- No artwork is ever drawn larger than its source (`ART_MAX == THUMB_PX`,
  asserted).
- Amber is never an opaque fill: a ≤ 6 px mark, a 4 px rail, a 1 px line, or
  light.
- No scrim, ever. Dimming ten thousand covers to show twelve rows is the mistake
  the palette exists to avoid.
- No spinner and no progress bar. The shelf filling with covers *is* the
  progress indicator.
- No telemetry, no accounts, no nags (`VISION.md`, restated here because a
  ledger that omits it is incomplete).
- **A slot may be added to the now-playing bar; none may be removed for
  tidiness** (prior-art R11). §1.1 removes the seek row by *replacing* it with a
  better statement of the same fact, which is the one permitted move.

---

## 7. The build plan

One sequence, superseding `01` §5's fourteen increments and `02` §11's Phases
A/B/C. Ordered so nothing is built on a foundation about to be replaced.

**Already done** — do not rebuild:

- **D1.** Places / album inspector / Up next popover / persistent bar; Previous;
  click-a-row-plays-from-there; the playing track dotted in the inspector
  (ADR-0016, `01` increments 1–8).
- **D2.** `02` Phase A: `MONO` and `SERIF` deleted; the gallery surfaces and
  inks; radii and per-token leadings; reserved slots re-derived in the Sans;
  `format_centidb` emits U+2212.

**The sequence:**

| # | Step | Layer | Why here |
|---|---|---|---|
| **1** | **History ledger.** Append-only plain local file, one line per play, written from the first beta with zero UI. The user's to grep, back up or burn. Last.fm/ListenBrainz output later, never a dependency. | `baz-core` (new) | **Cannot be backfilled.** PLAYED, the inspector card, the pull and shuffle weighting all feed on it. *In flight in a parallel agent; assumed to land.* |
| **2** | **The palette indirection.** `theme.rs` `pub const Color` → a `Palette` resolved at startup; Closing Time the only selectable room; Reading Room's tokens defined, not selectable. Adopt the oklch-L step assertion and the L .45–.58 dead-zone rule; extend the contrast test per §1.6 (composite the ink ramp, name the exemptions). | 2 | Every per-surface style below is written against this. Writing them against constants first means writing them twice. |
| **3** | **The accessibility stance, published.** README section; `About → Accessibility` in the Settings place; a hit-target assertion; a tooltip audit over every icon-only control. (`docs/REFUSALS.md` ships **with this ADR** — see §6.) | docs + small view | It is the constraint every step below is checked against, so it precedes them. Cheap, and it is a commitment rather than a note. |
| **4** | **Group keys in `baz-core`.** `Library::albums(key)` for ARTIST / YEAR / GENRE / ADDED; schema gains a genre column and a first-seen column; genre verbatim from tags. PLAYED joins at step 8. Amends ADR-0008. | `baz-core` | The shelf's headers, the index rail and the PLAYED key are all projections of this. Building the rail first would build it against one key. |
| **5** | **The hang.** `HANG` 40, `ART_MIN` 240, `ART_TARGET` 272, `ART_MAX` 320 as functions of grid width; `floor(x + 0.5)`; `gutter == HANG` wherever art is uncapped; extend the virtualization test from two widths to the band. (`02` B3.) | 1 (`shelf.rs`) + 3 | Density, the rail's lane and the thumbnail size are all functions of it. |
| **6** | **Density as zoom.** Three steps (Spacious / Balanced / Dense) as pure data; `Ctrl+-` / `Ctrl+=` / `Ctrl+scroll`; current step persisted in `config.rs`. **Not a Settings row** (§1.3). | 1 + 3 + config | Parameterises step 5's arithmetic; must not be added after the rail's lane is tuned. |
| **7** | **`THUMB_PX` 320** and the LRU re-derivation to 384 entries at the same 150 MiB; assert `max(ART_MAX over all steps) == THUMB_PX`. (`02` B4.) | 1 (`art.rs`) | After 5 and 6, or the cache grows for art that has not arrived. |
| **8** | **Group-key row, shelf headers and the index rail.** The keys as one row of words; sticky 9–10 px caps headers in the virtualizer; the rail as a pure projection of the active key, `INDEX_W` 36 off the grid width, absent values drawn not hidden, never the accent. **PLAYED lands here** (needs step 1's data). | 3 + 1 | After 4 (the keys) and 5–7 (the grid width). |
| **9** | **The needle.** New `needle.rs` widget; segments from queue-entry durations, 2 px gaps at track boundaries, 6 px at album boundaries; fill `LAMP`, track `RECESS`; click-to-jump via `JumpTo`; the hover preview tip moves onto it. | 3 (widget) + 1 (`player.rs`) | Before 10, which re-geometries the bar around its absence. |
| **10** | **The bar at 58 px.** Drop the seek row and preview lane; elapsed/total move into the left zone beside the wall label; keep Previous · Play/Pause · Next, `3 / 12`, the signal note, volume, mute and the `Up next` door. Re-pin every reserved-slot test; rewrite the `CENTRE_H` invariant. | 2 + 3 | After 9. |
| **11** | **Type-anywhere and the modifier layer.** Bare printable characters route into the query and focus the well; `n` / `m` / `q` → `Ctrl+…`; `/` and `Ctrl+F` survive; `Esc` clears, then blurs, then peels. `keys.rs` re-argued and its exhaustive matrix rewritten. **The field stays.** | 1 (`keys.rs`) + 3 | After 10, because it rebinds keys the bar's controls also send, and the binding-resolves-to-an-on-screen-control test must see the final set. |
| **12** | **Search ranking in `baz-core`.** Prefix and word-boundary ranking so "Enter plays the top match" is defensible. | `baz-core` | Only required because 11 makes find primary. |
| **13** | **The stack.** Shift-click appends a sleeve; shift-click a track row appends a track; numeral chip in the wall label's first line, never on the art; ephemeral — clears when it ends; albums listed as albums in the popover. | `vm` + 3 | After 11 (shift-click is a modifier gesture in the new layer). |
| **14** | **The tile in the new vocabulary.** (`02` B1/B2.) Delete the tile's background and border, `SHADOW` and `SELECTION_EDGE`; hover = 1 px `HAIRLINE_STRONG` rule under the label + artist ink lift; selected = 2 px `PAPER_FAINT` rule; playing = halo (blur 24) + dot. | 2 + 3 | After 2 (palette) and 5 (the cell). |
| **15** | **The inspector in the new vocabulary.** (`02` C2/C2b.) Flush-left sleeve capped at `ART_MAX`; catalogue and condition lines; two-line title cap; the `Details` block (13 fields, present only when the scan read one); SIDE A/B headers when rip metadata carries them; **the PLAYED card** — "N times since YYYY" plus date stamps, no charts. | 3 | After 2 and 14; the card needs step 1. |
| **16** | **Inspector responsiveness.** The width band `clamp(0.28 × W, 340, 420)`; below 940 px the inspector takes the content area and a shelf *strip* remains (`02` §6.9); whole-panel scroll below 700 px of height. | 2 + 3 | ADR-0016 deferred it; it is also the prototype of the eventual full-window Album place. |
| **17** | **Shuffle from what the wall shows.** Pool = `vm::matching_album_ids()`; non-pool covers dim to 35 %; the next two draws carry faint ink rings. No invisible pool, ever. | `vm` + 3 | After 8 (the wall's contents are now a function of the group key and the filter). |
| **18** | **The Marquee lens.** `WALL · MARQUEE` type switcher, keys `1` / `2`; playing sleeve at half-window full-bleed, poster type over a vertex-alpha scrim, wall dims to 35 %. **No idle auto-switch** (§1.7). | 3 | After 10; it hosts the pull. |
| **19** | **The pull** (`Ctrl+R`). One sleeve weighted toward long-unplayed, presented in Marquee with "last played N years ago". Nothing plays until `Space`; `Ctrl+R` re-pulls; `Esc` returns. | `vm` + 3 | Needs step 1's weights and step 18's surface. |
| **20** | **Reading Room.** The second room, follow-system, **only with an answer to the pale-sleeve-on-paper question that is not a border on artwork** (§1.5). If there is no answer, the room does not ship. | 2 | After every surface exists, so the room is validated against all of them at once. |
| **21** | **First run and import.** Folder picker and drop target; covers land as read; `RECESS` squares mark what is coming; header "watching ~/Music — N of M imported". No importer dialog or progress modal, ever. | 3 (+ one dependency decision) | The first thing every new user meets; it depends on the wall being final. |
| **22** | **The Settings place, filled.** Output device, exclusive mode, the signal-path diagram, watch folders, scrobbling output, the room picker, per-source enrichment consent. | 3 | The place exists; this is content arriving into it. |

**Deferred, designed, not discarded**: the Crate lens (needs edge-sampled spine
colours and rotated text rasterisation); mixtapes and the CRATES key; the
full-collection overview zoom stop; the art-derived lamp hue; Stone and Plaster;
drag-to-reorder the queue (no pointer capture in iced 0.13); the album as a
full-window place.

**Definition of done for 1.0.** The friction budget holds on a 20 000-album
library on mid hardware — launch → resume < 200 ms with the wall position and
paused track restored; click → sound < 100 ms perceived; keystroke → filtered
wall next frame; import with zero dialogs; a tag fix without pausing playback —
the wall at rest is 100 % collection in every shipped room, **and every action in
the product has a visible, pointer-reachable control.**

---

## Consequences

- **One target exists.** `01` and `02` keep their reasoning and lose their
  authority where this ADR names them; `docs/design/critique/` is adopted except
  where §1.1–§1.6 supersede it. Preambles added to `01` and `02` pointing here.
- **ADR-0006's claim is narrowed rather than defended.** Two `baz-core` changes
  (grouping, search ranking), one new `baz-core` subsystem (history), and a
  second hand-built widget are the honest price of a product design.
- **The contrast test survives its challenge and gets stronger.** It gains the
  critique's elevation law as an assertion, gains the ability to see an opacity
  as a colour, and gains a named exemption list instead of a global waiver.
- **Accessibility stops being an open question.** No screen-reader support at
  1.0, published where users will see it; the visible-control rule as the
  mitigation; AccessKit as the named fix; no further drift away from a tree.
- **The refusals ledger becomes a standing document with an editing rule**, and
  it immediately answers ADR-0016's open decision.
- **`docs/NEXT-STEPS.md` and `docs/BACKLOG.md` are superseded for UI work** by
  §7's sequence. Engine and library work continues to be ordered by them.
