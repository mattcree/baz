# baz — the refusals ledger

> Things considered and rejected **on principle**. Adopted by
> [ADR-0017](adr/0017-design-direction.md) §6 from
> `docs/design/critique/01-foundations.md`, extended with ours.
>
> **Who this binds.** Contributors and agents — not the owner. Its job is to
> stop settled questions being re-litigated by whoever touches the code next,
> and to stop baz drifting into the generic-music-player defaults it exists to
> avoid. **The owner's decision is sufficient on its own**; an entry he
> reverses gets rewritten to say what was decided and why, and that is the
> whole of the process. Nobody argues with a document to change their own
> product.
>
> **The editing rule, for everyone else.** Adding an entry needs a pull
> request. Removing one needs an argument that beats it. A refusal a
> contributor can delete because they changed their mind is not a refusal; it
> is a preference.
>
> **What is actually hard here** is responsiveness and the aesthetic — the
> product must stay fast and must look excellent. The entries below serve those
> two ends. Where an entry ever works against them, the entry is wrong.
>
> This is a standing document, not a decision record. It is the descendant of
> `VISION.md`'s "Refuse (the fixes)" and "Betrayal list", and it is where new
> refusals go.

---

## Playback

**No autoplay. No radio.** The queue empties and there is silence. **Silence is
a feature.**

Shuffle is a thing you *start*, never a thing that starts itself. `VISION.md`
pillar 4's steered shuffle survives as an explicit gesture. This answers the
open decision ADR-0016 recorded and refused to answer silently: *what happens
when an album ends.* Nothing happens.

**No invisible shuffle pools.** Shuffle draws only from what the wall currently
shows — a shelf, the filter's matches, everything — and the pool is visible:
non-pool covers dim, the next draws carry faint rings. A shuffle whose source
you cannot see is a recommendation engine wearing a dice icon. `Play all`
(doc 09 §7.1) keeps the same rule from the other direction: its scope is
exactly what the wall shows, in the wall's own order — playing what you
cannot see is refused for the reason drawing from it is, and "everything in
the library" is the empty query, one `Esc` away.

**No auto-generated playlists.** Every playlist is asked for by a person and
owned by them thereafter. Refused: generation without a request, mutation
without an edit, and any candidate pool the person cannot see.

*Amended by ADR-0024 §6 under the editing rule.* The entry's force is against
what it was written against — playlists that generate themselves, unbidden, as
engagement surfaces — and its old gloss (*"every crate and every mixtape is
made by a person"*) could be read to forbid the owner's stated goal of
sentiment-generated lists a person explicitly asks for. ADR-0024 §7 sets the
ground any generator inherits: its output is an ordinary `.m3u8` with ordinary
rights, generation is an act not a condition, provenance is recorded and
inert, nothing plays until the person says so, and the candidate pool is
statable in a sentence. *Made by a person* includes *asked for by a person*;
nothing else moved.

---

## History

**No engagement stats.** No Wrapped, no streaks, no charts, no "top artists of
the year", no listening-time totals. **History records; it never performs.**

What history is allowed to surface: the PLAYED group key, the inspector card
("PLAYED — N times since YYYY", plus a column of date stamps), and the returns
lane's *when did I last touch this* order (ADR-0030 §1). Nothing else.

*Rewritten 2026-08-10 to record the owner's decision.* This entry named a
fourth permitted surface, **the pull's weighting** — `History::pull_weight`,
one per day since a record was last heard, drawn from by the strip's `Pull`.
The owner removed the control: *"please can we remove pull since it doesn't
make sense here."* The weighting had exactly one consumer, so it went with it,
constants and all. What the list loses is not a permission but a *use*: a
weighting nothing spends is a recommendation engine's foundations poured and
left, and the shortest way to keep history from performing is to keep the
surfaces down to the ones something reads. Re-adding a weighted draw is a new
entry and a new argument, not a revival of this one.

**The ledger is the user's.** Append-only, one line per play, in a plain local
file they can grep, back up or burn. Scrobbling to Last.fm or ListenBrainz is an
optional *output*, never a dependency and never the source of truth.

---

## The interface

**No view-options menus.** No list-mode toggle, no column chooser, no sort
dropdown, no free zoom slider. Group keys are a row of words; the lens switcher
is two words; density is three detent marks on the index rail's lane, and the
zoom gesture accelerates them.

*Amended by [ADR-0028](adr/0028-density-detents.md) under the editing rule.*
The entry's force is against what it was written against — menus, choosers and
sliders that enumerate view state and grow tenants — and all of that stands.
What falls is one clause, *"no grid-size picker"*, as applied to three quiet
detents in the place's own body: the clause had come to forbid the only visible
route to an action, which put it in direct contradiction with this ledger's own
accessibility entry (*no action is gesture-only* — doc 09 §5.2's reading of the
visible-control rule below), and the accessibility entry is the mitigation for
a toolkit with no accessibility tree, which outranks a quietness preference.
The three named steps, the gesture, the persistence-as-state and the absence of
any Settings row are all unchanged. (Owner's decision, 2026-08-09.)

**One thing is drawn on a sleeve, and only under the pointer**: the hover
options — a veil gathering at the sleeve's left edge and dissolving to nothing
before its right one, carrying `Play`, `Queue`, `Add to…`, `Open`. It is
present on exactly one tile at a time, it is gone the moment the pointer
leaves, and the right of every cover stays as painted so the record stays
recognisable while you choose.

Still refused, and these are refusals rather than omissions: a badge, a
duration chip, a selection tint, a queue numeral, **anything at rest**, and
anything on artwork anywhere but a wall tile — not the Songs rows, not the
lane, not the record's page. Nothing is drawn on a sleeve nobody is pointing
at.

*Rewritten on the owner's decision, 2026-08-09.* This entry used to read
**"Nothing is ever drawn on top of a sleeve"** and listed a play overlay on
hover and a gradient scrim among the things it refused. The owner approved a
mockup that draws exactly that, and his decision is sufficient on its own
(this ledger's own preamble). The design constraint that replaced the blanket
refusal is the veil's asymmetry: it must be a gradient that dies before the
right edge, never a flat panel over the whole cover, and
`the_veil_is_a_gradient_over_one_sleeve_and_never_a_flat_panel` is what holds
it to that.

**No artwork is ever drawn larger than its source.** `ART_MAX == THUMB_PX`,
asserted in code.

**No scrim, ever.** Dimming ten thousand covers to show twelve rows is the exact
mistake the palette exists to avoid. Unchanged by the hover veil above and
worth saying why: a scrim is a surface laid over *the collection* to make
something else readable, and the veil is a mark on **one** object under the
pointer that stops before that object is hidden. The shuffle pool's dimming is
governed here too, and it is not a scrim for the same reason — it is the
artwork's own opacity, not a layer over it.

**No spinner and no progress bar, anywhere.** During a scan the shelf filling
with covers *is* the progress indicator. No importer dialog, no progress modal.

**A slot may be added to the now-playing bar. None may be removed for
tidiness.** (`docs/design/03-interface-prior-art.md` R11: three vendors bought
"visual calm" by removing control density inside two years and all three
reversed; what was lost was always position, provenance and skip.) Replacing a
slot with a *better statement of the same fact* is the one permitted move — it
is how the seek row became the needle.

**Sound from the wall is one press.** Hover a sleeve and four options are laid
over it — `Play`, `Queue`, `Add to…`, `Open`. `Play` sounds the record from
the wall. The friction budget's *intent → sound = 1 press* line is met at
every scope in the product, the wall included.

*Rewritten on the owner's decision, 2026-08-09.* This entry used to read
**"Sound from the wall is two presses, and that is a price, not a debt"**, and
it priced the wall at two: open the record's page, press `Play album`. The
owner decided the price should not be paid, and that the reveal should be the
hover group ADR-0032 §2 had measured as not fitting *beside* a tile — it fits
*inside* one. What the old entry was written against is untouched and still
refused: no double-click, no press whose meaning depends on arrival time, and
no route buried in a modifier key (the owner on ADR-0032 §4's `Ctrl`-click
proposal: *"burying things in modifier keys is not great"*). Shift-click
remains the one-press *sound-later*, and the tile's right-press menu remains
the pointer-reachable twin of all four options, so nothing here is reachable
only by hover.

**A band's content may not touch the band's edges.** Every bar leads its tallest
zone by a **named gap** on each side — never a ratio, because a constant
ink-to-band ratio is not reachable on the 4 px lattice for two bands of
different content heights, and a lead off the lattice is law L2 broken to make a
proportion true. The top bar leads its 32 px control row by `GAP_SM` 8; the
bottom bar leads its 56 px type block by `GAP_MD` 12 (a hit box carries its own
internal padding, a stack of line boxes carries only its leading). Added by
ADR-0022, on the owner's *"proportion is becoming an issue e.g. bottom bar is
too short"* — the needle's bar was correct in every token and had no air at all.

---

## Surfaces

**baz has one resident side surface, and no surface that is a slot.** No
inspector, no drawer, no popover. **The window holds one place at a time, with
the returns lane to its left in every place but Settings, and the now-playing
bar under all of them** (ADR-0022 as ADR-0030 restates it). One summoned,
single-tenant panel exists beside it: the playlist panel (ADR-0024) — summoned
by <kbd>Ctrl</kbd>+<kbd>P</kbd> for the duration of a collecting task,
overlaying without reflow, closed at rest. **Neither may ever gain a second
tenant.**

*Rewritten on the owner's decision, 2026-08-09* — *"let's do the ground work
for adding a home page and left hand side bar… we can collapse it into only an
icon list. similar to Spotify"*. This entry said *no resident side surfaces*
and it was rejected twice before it was written down; the owner reversed it,
and the preamble says that settles it. What the reversal is **not** is a
licence for panels generally: what was rejected twice was a **slot** — a
340 px column that showed the selected album, then the queue, then Settings,
with arbitration state and a re-hang of the wall on every tile press. The lane
has one subject (*things you have touched*), one list, one order, no
arbitration, and one press that may re-hang the wall — the press whose subject
*is* the wall's width, and which lands outside it. ADR-0030 §1 tabulates each
of the five findings that killed the rail against the thing that makes it
unreachable here. **A second resident surface needs an argument that beats
this one, and so does a second tenant in this one.**

The head's three destinations are the owner's too, and they are the one
concession worth naming: a nav rail is refused (doc 07 L8.4) and the head is a
**closed set of three**. A fourth is the refused thing.

Rejected twice by the owner before this was written down — *"an example of a
strange UI is the two side panels we have now"*, and then *"I really hate the
way queue and selected albums appear… I hate the sidebar"*. The prior-art study
supports a right-hand inspector for cataloguer audiences and that evidence is
not overturned; it is relocated. It argues for the album having a rich resident
surface, and the record's page is three and a half times wider than the column
was.

*Amended by ADR-0024 §5 under the editing rule*, which is the required
argument, engaged rather than snuck past. The rail died of five findings —
three unrelated tenants, a paragraph of dismissal, the wrong tenant paying
resident width, a gesture-breaking reflow, arbitration state — and the panel
has none of them by construction: one tenant forever (the junk-drawer disease
requires vacancy, and this entry closes the slot), summoned by a labelled door
and closed by `Esc` or the door (the wall keeps 100 % at rest), floating over
the place without re-hanging it (ADR-0016's verified `stack` + `opaque`
mechanics, no scrim, wheel passing through), present in Library, Album and
Queue and absent in Settings. What it buys is the one thing no place can have:
**simultaneity** — collecting is two-surface work, source and destination on
screen at once. It *receives*; it does not display a selection, which is what
the dead column did and what places do better. The owner blessed this surface
explicitly; this entry records the argument so the blessing is not a precedent
for panels generally.

**Every scrolling surface in baz has a scrollbar, and the wall's is 4 px.**
The rail says *where you are* and names the shelf it will take you to; the bar
answers the one thing a rail cannot be asked — *take me to the end*, which is
not a group key and so is not a rung on the rail. The two strips are not doing
one job; they are doing two, and the bar is the narrower of them because it is
the lesser.

*Rewritten on the owner's decision, 2026-08-09* — *"can we allow there to be a
scroll bar for any view? Just a very minimal scroll bar because otherwise, it's
hard to just jump to the end"*. This entry used to read **"Two vertical strips
may not do one job"** and refused the wall a scrollbar outright. Every other
list in baz already had one, so the wall was the only surface the entry
applied to. What stands from ADR-0022's complaint (*"the fact that the alphabet
bar has a scroll to its left isn't nice either"*) is that the bar must not
compete with the rail: 4 px against the rail's 60, no trough, the room's own
hairline, and it reserves its own lane so it is never drawn over a cover.

---

## Accessibility

**Every action in baz has a visible, pointer-reachable control. No action is
keyboard-only, and no control's only affordance is hover.**

This is the mitigation for a toolkit that publishes no accessibility tree and
gives buttons no keyboard focus (ADR-0017 §4). It is why the transport buttons,
the search field and the labelled `Queue` door survive designs that wanted them
gone — and why, when ADR-0022 removed the last surface that knew which record
was under the lamp, *get back to what is playing* got a labelled control (the
bar's now-playing text) rather than a gesture.

**No state is signalled by colour alone.** The lamp dot *replaces* the track
number rather than tinting it; the halo is accompanied by a dot; the shuffle
pool is marked by dimming *and* by rings.

---

## Colour, depth and motion

**No user-picked accent colour.** The art-derived lamp is *data* — hue read from
the record, lightness and chroma pinned — not a preference; its off switch is
binary, not a colour picker.

**Amber is never an opaque fill.** It appears only as a ≤ 6 px mark, a 4 px
rail, a 1 px line, or light. It states what is true about playback right now and
nothing else: not what is queued, not what is selected, not what has focus, not
what the scanner is doing.

**No room at oklch L .45–.58.** Neither ink works there and mid-value sleeves
melt into it.

**No borders on artwork.** Including as the remedy for a sleeve that melts into
its room. Nudge the room's lightness instead, or do not ship the room.

**No shadows** except the playing halo, which is not elevation — it is light.

**No motion that costs anything when nothing is moving.**
[ADR-0020](adr/0020-motion.md) amends this entry under the ledger's own rule —
the old text said *"every state change takes 0 ms; hard cuts by design"*, and it
rested on a premise that did not survive measurement: a transition was said to
need a `window::frames()` subscription "which redraws whether or not anything is
moving". A **bounded** subscription does not, and baz already shipped the
pattern twice. `docs/design/04-fluidity.md` §1.4 carries the numbers: 0.0 % CPU
and one frame in 3.88 s once the last tween settles.

So **five** transitions may exist and no others, each expressible as a bounded
tween: the icon-button ink fade (90 ms), the queue popover's arrival (140 ms),
the shelf tile's hover rule (90 ms), the album inspector's width (150 ms), and
the lamp warming when the light moves to another record (200 ms). Each degrades
to a hard cut by passing a zero duration, which is how a *reduce motion* setting
will be implemented.

**Three of the five ship.** ADR-0022 deleted the queue popover and the album
inspector, so §2.2's arrival and §2.4's width have no surface left to move.
Neither is *forbidden* — if either surface ever returns, its number returns with
it — but a `Duration` nothing reads is worse than a sentence saying why. **A
place change is a hard cut**, and that is a decision rather than an omission:
the surfaces either side of a navigation share no element to move, so any
transition between them would be decoration, which this entry forbids.

**One thing moves without being a transition at all**: the index rail's fisheye
(ADR-0020's amendment). It is a pure function of the pointer's position — no
clock, no tween, no subscription — so it costs exactly nothing when nothing is
moving, which is this entry's actual rule. Its snap back to rest when the
pointer leaves is a hard cut, argued in the amendment; a sixth tween is not the
door it came in through.

**Still refused, and these are refusals rather than omissions**: shelf-grid
stagger or pop-in; any fade as a thumbnail decodes (a thumbnail replacing its
placeholder stays an instant swap); album-art crossfades; **any animation of the
bar's geometry**; springs, bounces and overshoot; and — the clause the rest hangs
on — **anything requiring a redraw while the window is idle**, which is now a
boolean the subscription reads and a test asserts rather than a promise.

The two movements that were never animation are unchanged: the needle advancing
with playback (data arriving) and scrolling.

**Motion states what changed. It never decorates, and it never moves the
transport.**

---

## Skeuomorphism

The record supplies **physics, structure and vocabulary** — the stack, sides,
groove spacing, "drop the needle". It never supplies **surface**. Banned: vinyl
discs peeking from sleeves, wood grain, tonearms, VU meters, wear, patina, and
any circle pretending to be a record.

---

## The product

**No telemetry. No accounts. No nags.** (`VISION.md`, restated here because a
ledger that omits it is incomplete.)

**No cloud dependency for anything that works on the user's own files.** Files
are the source of truth; the database is a cache; baz never writes to a file
unbidden.

**No snake oil.** Nothing in the interface may claim an audio benefit the signal
path cannot demonstrate. The condition report is an archivist's note, never a
sales pitch.

---

## Considered, and *not* refused

Recorded so the ledger is honest about what it declined to adopt, and so that
re-proposing either of these is met with an argument rather than a shrug.

**Captions at rest on the wall.** `docs/design/critique/01-foundations.md`
refuses them. ADR-0017 §1.4 overrules: without labels the grid has no structure,
a near-black sleeve on a near-black wall has no anchor at all, and the claim
that "the shelf contains exactly two kinds of thing, artwork and type" becomes
false at rest. Every tile keeps its two-line wall label.

**Radii.** The critique bans them outright. ADR-0017 §3 declines on churn rather
than principle: artwork and the wall are already at 0; controls keep
`RADIUS_CTRL` 4 and `RADIUS_SEGMENT` 3.
