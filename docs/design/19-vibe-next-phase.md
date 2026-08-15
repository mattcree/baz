# 19 — Vibe, next phase

The owner, 2026-08-15, reviewing the contour page that shipped in 0.2.0. This
document is the record of what he said, what is actually wrong, and the three
layouts he was asked to choose between. **Nothing here is built.** It exists to
be approved or rejected first, because his last sentence was *"create some
designs which we can actually approve or not."*

The rendered version — with the mockups drawn at baz's own tokens — was
delivered to him as a page; this is the same argument in the repository, which
is where it has to live to survive.

## 1. What he said

> *"the ui layout for the vibe playlist isn't great… it's just not well
> optimised for a wide screen. we have to scroll to see the playlist. it should
> work on both wide and narrow layouts."*

> *"the home page section headers and this pages section does not match the
> library and playlist. i prefer the library and homepage."*

> *"top right there is 'manual and vibe become the same ordinary playlist'.
> remove that"*

> *"the fact that there is a 'mood' and 'the words'. and 'the shape' seems not
> well explained. I mean, some of these elements aren't really clearly
> clickable. and we know under the hood anything they type has to be anally
> specific to actually work since there's no LLM so they will need examples"*

> *"for the shape it seems like the ways to show the other lines is not a
> standard UX method. our default for this should just be a blend of all of
> them I guess, and then if users want to, they expand to configure all… they
> would all start with the blend option's initial points and curve."*

> *"we don't want to overcomplicate this. this is the sort of thing that would
> have expert UX scratching their heads I think. Probably they would say 'dont
> give them all these options'."*

> *"there is a lack of iconography, colour, etc. — this page looks weird and
> not inviting at all. there's a weird density of just things up in the top
> left."*

## 2. The evidence

`docs/design/impl/vibe-next-phase/` holds four frames from the real binary,
headless, with the isolation receipt: the form cold and one press into a mood,
at **1600 × 900** and at **1000 × 700**.

The wide frame is the argument. The words field is **1 270 px wide for a
six-word phrase**; the right half of the window holds nothing at all; the
contour is cut by the fold; and the list, the length and the Compose control
are all below it. The narrow frame is the *same column* — so the page is only
ever designed for narrow, and never says so.

## 3. Diagnosis

Eight defects. Six of them are one defect wearing different clothes: **the page
is a form, and it should be a place.**

1. **One column at any width.** `views::new_playlist::vibe_form` is a single
   `column!` inside the page scroll. Every other subject page in baz — album,
   playlist, favourites — is `views::page`'s aside beside a body. This is the
   only surface in the product that *makes* something and does not show it
   beside the controls that made it.
2. **The reward is last.** Mood → words → shape → lines → length → Compose →
   list: five decisions and two scrolls before one track is on screen.
3. **Sections read as form labels.** `views::section_rule` is 11 px
   letter-spaced caps over a hairline. The walls use `shelf::group_band` — a
   larger, quieter letter with air around it. **Home uses `section_rule` too**,
   so Home and the Vibe page match each other and neither matches Library or
   Playlists. His sentence reads two ways and this is decision 2 below.
4. **Nothing looks pressable.** Moods, examples and shapes are `word_button`s:
   type with no border, no ground, and hover as the only affordance. Six moods
   in a row read as a sentence of nouns.
5. **The words carry the retrieval and explain nothing.** There is no language
   model: CLAP's text tower answers *descriptive phrases about sound*. "slow
   sparse piano, melancholy" retrieves; "songs about my ex" retrieves noise.
   The three examples exist and read as decoration.
6. **A stepper that mints a control.** The `−`/`+` beside a dimension adds
   another curve. Nothing else in baz creates a control with a stepper, it has
   no name on screen, and it is an expert's tool in a beginner's path — which
   is precisely his *"not a standard UX method"*.
7. **Four registers in one corner.** `Back to choices`, the hero, the byline
   and the analysis note stack inside 90 px, and the strip's right-hand
   sentence explains the product's data model to somebody who wants a playlist.
8. **No colour, no glyphs.** The accent appears only on the contour's result
   dots, and there is not one icon on the page, in a product with a glyph for
   everything.

## 4. The three layouts

### A — Ask on the left, list on the right (recommended)

The page becomes what every other subject page already is: a lane of controls
beside the thing itself. The list is on screen while you tune; the curve sits
directly over it, so a line and its result are one picture; and the press that
composes never leaves the fold. Below the breakpoint (~1180 px) the two panes
stack in reading order — ask, curve, list — which is today's page minus what
was in the way.

Concretely:

- **Mood composes on press.** One press, one list, everything still editable.
  This is option C's first move, folded into A.
- **Words** gain a rule that can be acted on — *describe the sound, not the
  story* — and two examples that are chips rather than labels.
- **Shape is one blended line**, with *Tune each dimension separately* as a
  disclosure, each dimension opening on the blend's own points. His words.
- **The stepper is gone**; the disclosure is what adds lines.
- **The strip sentence is gone** and the hero collapses into the breadcrumb,
  which is what clears the top-left corner.
- **Sections wear the wall's band**, with a glyph, so the page reads like
  Library and Playlists rather than like a settings form.

### B — One question at a time

A three-step wizard: mood, then words, then shape, each filling the page, the
list at the end. Friendliest to a first-timer, most tiring on the second use —
three presses of *Next* before a note plays, and no way to see how a change to
the words moved the result. baz has no wizards; this would be the first.

### C — A list first, tuning after

Press a mood, land on a finished list; the words and shape live in a *Tune*
drawer over the right-hand side. The fastest route to music, and the truest
reading of *"don't give them all these options"* — but it hides the shape,
which is the one thing that makes this feature baz's rather than anyone's, and
a listener who never opens the drawer never learns it is there.

## 5. What needs a decision

1. **Which layout** — A, B or C. *Proposed: A, with C's first press.*
2. **Which section header wins** — the wall's band everywhere including Home,
   or Home stays and only Vibe changes. His sentence is ambiguous and this is
   the one place a guess would be expensive. *Proposed: the band everywhere.*
3. **Does per-dimension tuning ship in this phase** — the blend is the default
   either way. *Proposed: ships, closed, seeded from the blend.*
4. **Six moods or four.** *Proposed: six; they wrap honestly.*
5. **Does Vibe keep its own place** — today it is a mode of `New playlist`
   behind a fork. *Proposed: stays a mode, and the fork loses a step because
   the mood press composes.*

## 6. Not on the table

- **The engine.** The contour still steers position against a
  collection-relative rank axis, and retrieval still runs per position. That
  half is measured and works (`docs/design/impl/contour/`).
- **Local only.** No network, no account, no model that is not already in the
  build.
- **The end state.** What is saved is an ordinary `.m3u8`.
