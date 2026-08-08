# baz — the refusals ledger

> Things considered and rejected **on principle**. Adopted by
> [ADR-0016](adr/0016-design-direction.md) §6 from
> `docs/design/critique/01-foundations.md`, extended with ours.
>
> **The editing rule.** Adding an entry needs a pull request. **Removing one
> needs an ADR that beats its argument.** A refusal you can delete because you
> changed your mind is not a refusal; it is a preference.
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
open decision ADR-0015 recorded and refused to answer silently: *what happens
when an album ends.* Nothing happens.

**No invisible shuffle pools.** Shuffle draws only from what the wall currently
shows — a shelf, the filter's matches, everything — and the pool is visible:
non-pool covers dim, the next draws carry faint rings. A shuffle whose source
you cannot see is a recommendation engine wearing a dice icon.

**No auto-generated playlists.** Every crate and every mixtape is made by a
person.

---

## History

**No engagement stats.** No Wrapped, no streaks, no charts, no "top artists of
the year", no listening-time totals. **History records; it never performs.**

What history is allowed to surface: the PLAYED group key, the inspector card
("PLAYED — N times since YYYY", plus a column of date stamps), and the pull's
weighting. Nothing else.

**The ledger is the user's.** Append-only, one line per play, in a plain local
file they can grep, back up or burn. Scrobbling to Last.fm or ListenBrainz is an
optional *output*, never a dependency and never the source of truth.

---

## The interface

**No view-options menus.** No grid-size picker, no list-mode toggle, no column
chooser, no sort dropdown. Group keys are a row of words; the lens switcher is
two words; density is a zoom gesture.

**Nothing is ever drawn on top of a sleeve.** No play overlay on hover, no
badge, no duration chip, no gradient scrim, no selection tint, no queue numeral.
The only thing that touches artwork is light around it.

**No artwork is ever drawn larger than its source.** `ART_MAX == THUMB_PX`,
asserted in code.

**No scrim, ever.** Dimming ten thousand covers to show twelve rows is the exact
mistake the palette exists to avoid.

**No spinner and no progress bar, anywhere.** During a scan the shelf filling
with covers *is* the progress indicator. No importer dialog, no progress modal.

**A slot may be added to the now-playing bar. None may be removed for
tidiness.** (`docs/design/03-interface-prior-art.md` R11: three vendors bought
"visual calm" by removing control density inside two years and all three
reversed; what was lost was always position, provenance and skip.) Replacing a
slot with a *better statement of the same fact* is the one permitted move — it
is how the seek row became the needle.

---

## Accessibility

**Every action in baz has a visible, pointer-reachable control. No action is
keyboard-only, and no control's only affordance is hover.**

This is the mitigation for a toolkit that publishes no accessibility tree and
gives buttons no keyboard focus (ADR-0016 §4). It is why the transport buttons,
the search field and the labelled `Up next` door survive designs that wanted
them gone.

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

**No motion.** Every state change takes 0 ms; hard cuts by design. The two
permitted movements are not animation: the needle advancing with playback (data
arriving) and scrolling.

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
refuses them. ADR-0016 §1.4 overrules: without labels the grid has no structure,
a near-black sleeve on a near-black wall has no anchor at all, and the claim
that "the shelf contains exactly two kinds of thing, artwork and type" becomes
false at rest. Every tile keeps its two-line wall label.

**Radii.** The critique bans them outright. ADR-0016 §3 declines on churn rather
than principle: artwork and the wall are already at 0; controls keep
`RADIUS_CTRL` 4 and `RADIUS_SEGMENT` 3.
