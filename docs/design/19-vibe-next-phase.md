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
   the mood press composes.* **Superseded by the quorum's R7**: the fork dies
   and the two become one page, which also deletes the top-right sentence by
   deleting its reason.

The quorum below added four more, and refused to answer them itself because
none of them is a designer's to settle:

6. **May baz listen to a library before it is asked to?** Analysis is hours of
   CPU on a large collection and battery on a laptop. Quietly at first launch
   makes the feature instant when it is found; not doing it makes the first use
   slow. *Proposed: opt-in, on the feature's own page, with the cost stated.*
7. **Does `something I haven't played in a while` become a control?** baz keeps
   a play ledger, so it can answer this, and it is the request people actually
   make of a large library. *Proposed: a toggle beside the words, not a seventh
   mood — it composes with all of them.*
8. **Does this reach the wall as `more like this`?** Out of scope for this
   phase; recorded so it is not lost.
9. **Six moods or four** — unchanged from 4 above; the room was content with
   six.

## 6. What the quorum changed

The owner read §1–§5 and said it *"still feels like it hasn't addressed
everything"*, and asked for a room: *"create a quorum of domain experts and UX
experts and have them discuss this."* That room is
`docs/design/quorum/2026-08-15-vibe.jsonl` — nine hats, 89 messages, and it
found nine things this note had missed. The full resolutions are in the file;
these are the ones that change the design above.

1. **The cold start is the primary state, not a footnote.** §4 says a mood
   press composes immediately. On a library baz has not listened to yet that
   sentence is false — analysis is minutes to hours — and most first visits are
   in exactly that state. The right pane becomes an honest progress reading
   with a partial list forming in it; no copy promises a list in one press
   until the library has been heard. *(R1)*
2. **An unweighted blend is degenerate.** Each dimension is a rank axis, so the
   plain mean puts loud-and-slow and quiet-and-fast in the same place: a line
   the engine satisfies with tracks that sound nothing alike — which is the
   *"the dots aren't following my line"* failure, again. The blend is a
   **weighted** mean with energy dominant, labelled in listener words (*loud,
   fast and busy* → *quiet, slow and sparse*). Seeding each dimension from the
   blend stays exactly consistent under weights, so the owner's instruction
   survives. *(R2)*
3. **The words need a vocabulary, not a rule.** *"Describe the sound, not the
   story"* tells somebody what not to do without giving them a route. Three
   rows of four chips — what it is made of, what it feels like, how it moves —
   that append to the field, chosen by a scored run against the baseline corpus
   rather than by taste. *(R3)*
4. **The curve is pointer-only.** Arrow-key nudging with visible focus, grab
   regions at twice the drawn radius, the shape presets promoted to being the
   accessible route to the same outcome, and a permanent sentence under the
   curve stating the shape in words as it is dragged. *(R4)*
5. **The row-to-dot pairing must not rest on hue** — enlarge the dot, drop a
   tick to the axis, and put the position number in the row. *(R5)*
6. **Length belongs on the commitment, in minutes**: `Compose · about an hour`,
   with the count arriving in the result's own line. *(R6)*
7. **The Manual/Vibe fork dies.** One New playlist page: *Start from a mood*
   with the chips, *Start with an empty list* underneath. This deletes the
   top-right sentence the owner asked to remove by deleting its reason, and
   removes a navigation step. *(R7)*
8. **The result is a playlist, not a receipt** — ordinary rows and controls,
   save under a name, and a Recompose that states what it replaces when there
   is something to replace, beside a variation press. *(R8)*
9. **A selected row explains itself in one line**, as a rank and never a score:
   *"position 4 of 20 — louder than 78% of your library, matched 'warm analogue
   soul'."* That is the owner's original demand — that a person can see it
   really worked — in a form that survives a screenshot. *(R9)*

Two more that are corrections rather than additions: a request the collection
cannot fill must **say so** instead of degrading silently *(R10)*, and the
narrow layout must **pin Compose** and land on the list after composing,
because §4 fixed the wide case and left the narrow one exactly as it was
*(R11)*. The local-analysis block moves to Settings *(R12)*.

The room also refused to decide four things, and they are in the file as
`open_question` records: whether baz may listen to a library before being asked
(consent and battery, the owner's call); whether Home keeps its small-caps
sections; whether *"something I haven't played in a while"* becomes a control,
since baz keeps a play ledger and could answer it; and whether this eventually
reaches the wall as *more like this*.

## 7. The feature, designed end to end

The owner, after the quorum: *"lets focus on this feature."* Design note 21 is
that — every state drawn, not only the one that exists. It was delivered as a
page; the states and the resolved decisions are recorded here so the repository
carries them.

**Nine states, of which the shipping build designs one.** Never listened →
listening → ready → asked → composing → a list → edited → saved, plus two
asides: *the shape cannot be filled* and *tuning each dimension*. The five with
no design at all today are the two first-run states, composing, edited and
saved — and the first two are where a new listener spends their entire first
session.

What note 21 resolves, beyond §6's list:

- **The ask pane stays live while baz listens.** Everything except the compose
  is free — pressing a mood, typing, dragging the shape — because a page you
  cannot touch for two hours reads as broken. The commitment states what it
  needs (`Compose · needs listening first`) and then what it can do
  (`Compose from 1 240 so far`), which is true at every point on the bar.
- **The shape control's anatomy**, drawn at size: one blended line; the axis
  labelled *loud, fast, busy* → *quiet, slow, sparse* so there is no legend;
  **the library's own distribution drawn behind it**, so a line through a thin
  part is visible before the press; a sentence stating the shape that updates
  as it is dragged; presets as chips *underneath* rather than thumbnails above,
  which makes them the press-instead-of-drag route; a ring on the focused point
  with arrow keys and doubled grab regions.
- **The degenerate case is warned twice** — before the press against the
  distribution, and on the result in plain numbers — with *lower the line*
  offered as a control. Nothing is padded to reach the asked-for length.
- **Saving happens in place**: the ask pane becomes the naming pane for one
  press, the name is proposed from the mood, and what lands is the same
  `.m3u8` as every other list.
- **Narrow keeps the order, the words and the visible commitment**; only the
  arrangement changes, and under 700 px of height the curve collapses to its
  sentence and its presets — which is its accessible form anyway.

**Four decisions remain**, and only one of them is not taste: may baz listen to
a library before being asked (consent); does a mood compose immediately once
there is something to compose from; do the per-dimension curves ship on day
one; and is the surface still called *Vibe* or *New playlist › From a mood*.

The build order, if it is a yes: the two panes and the measure; the shape
control; the words vocabulary; the result; the first run; then the fork's
removal, last, so it lands on a page that is already right.

## 8. Mood and words are one question (note 21a)

The owner, on note 21: *"I don't really get the 'mood' and 'words' and how they
interact… this will probably confuse people."* He is right, and it is a model
defect rather than a labelling one.

**Pressing a mood does four things at once** — lights the chip, fills the words
field, replaces the curve, sets the length — and three of them happen somewhere
else on screen from the thing that was pressed. Worse, two controls then state
the same request twice, a lit chip and a filled field, with nothing saying
which one the engine reads. The questions a listener is left with have no
answer on screen: does the mood *add* to my words or replace them? If I edit
the words, is the mood still applied? If I press a second mood, do I have two?

**There is one request.** A mood is not a second input; it is a shortcut that
writes into the only input there is. So the page asks **two questions in the
listener's own words** and neither of them is jargon:

- **What do you want to hear?** — the field at the head of the band, with
  *"this is exactly what Baz searches for"* under it. Beneath that, the
  starting points that fill it, and beneath those, the vocabulary chips that
  append to it. One line, three ways to write it.
- **How should it move?** — the curve and its presets, about movement and
  nothing else.

The rules, which are behaviours rather than modes:

| The listener | Then |
|---|---|
| presses a starting point | its words **replace the line**, visibly, and are editable at once |
| edits the line | the starting point's light goes out — not a mode switching off, a label ceasing to be true |
| presses a second starting point | it replaces the line; there is no way to have two, because there is one line. Undo restores |
| presses a word chip | it appends to the line, with a comma |

And the fourth silent effect is bounded: **a starting point sets the curve and
the length only while the listener has not set them themselves.** Drag a point
once and they are yours; from then on a mood changes the words and nothing
else. Invisible when it is right.

Three alternatives were rejected: dropping moods (redundant against the
vocabulary, but a first press that produces a whole playlist is the best moment
in the feature); making them genuinely two inputs (the engine has one text
embedding, so combining them *is* concatenation, which is what filling the
field already does — only hidden); and keeping two bands with a better
explanation (if it needs explaining it is wrong, and the explanation would have
to be re-read every time).

This also settles the naming question without a separate decision: the place is
*New playlist › From a mood*, its first question is what you want to hear, and
**Vibe** stays the feature's name in conversation and in the changelog rather
than a word anybody must understand in order to use it.

## 9. Not on the table

- **The engine.** The contour still steers position against a
  collection-relative rank axis, and retrieval still runs per position. That
  half is measured and works (`docs/design/impl/contour/`).
- **Local only.** No network, no account, no model that is not already in the
  build.
- **The end state.** What is saved is an ordinary `.m3u8`.
