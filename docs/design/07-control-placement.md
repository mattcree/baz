# 07 — Control placement: where every control lives, and why

> The owner's complaint, in full: *"feels like controls are not well thought out
> in terms of location."*
>
> He is right, and the reason is not that any one control is in the wrong place.
> Every placement in baz is locally defensible. The group keys went beside the
> search field because both are about the library. `Shuffle` and `Pull` went next
> to them because there was room in that cluster. `Settings` went top-right
> because that is where settings usually are. The `Queue` door went bottom-right
> because the queue is playback. `mute` and the fader went bottom-right because
> that is where a hand reaches for volume. Each of those sentences is true and
> together they are not a system — they are five separate arguments that happen
> not to have collided yet.
>
> **What baz has never had is a rule that decides the next one.** This document
> is that rule. It is the eighth composition law, and like the other seven it is
> stated so that it settles cases nobody has raised yet, and it carries the test
> that pins it.
>
> **Written for the model that is arriving, not the one on screen.** A parallel
> agent is removing every side surface: the album inspector and the queue popover
> become full-window **places** alongside Library and Settings, the rail and the
> popover are deleted, the wall's scrollbar goes and the index rail carries
> position. Everything below is written for that model
> ([ADR-0016](../adr/0016-places-inspector-popover-bar.md) as amended,
> [ADR-0017](../adr/0017-design-direction.md)). Where the shipped app differs,
> the inventory says what is there **today** and the law says where it goes.
>
> **This document changes no code.** It proposes the `.interface-design/system.md`
> §13 text in §7 and does not edit that file, the way
> [`06-composition-audit.md`](06-composition-audit.md) proposed L1–L7 and did not.

---

## 0. What decides this

### 0.1 The evidence base

Placement is not a taste question here, because the frequency work has already
been done. [`03-interface-prior-art.md`](03-interface-prior-art.md) §1.1
enumerates twenty-one workflows read off sixteen products, and §1.2 ranks them
for **this** audience into five bands:

| Band | Per session | What the layout owes them |
|---|---|---|
| **A — constant** | 10–100× | Zero clicks. The resting state of the window, or one keystroke from it |
| **B — frequent** | 3–20× | One click, no navigation, nothing else lost |
| **C — occasional** | 0–3× | One click to reach; may cost a surface |
| **D — rare** | ~weekly | May cost a layer down |
| **E — very rare** | ~monthly or less | May cost a whole place. **Should never cost the shelf a pixel at rest** |

Its own summary of the consequence is the sentence this whole document is a
mechanism for: *what happens dozens of times a session must be nearly free; what
happens once a month can cost a click and a surface.*

Three other documents bind the answer before it is written:

- **[the product's standing rules](../the product's standing rules), the visible-control rule.** *Every
  action in baz has a visible, pointer-reachable control. No action is
  keyboard-only, and no control's only affordance is hover.* This makes
  placement compulsory: an action cannot be "somewhere in the keyboard". It also
  makes deletion expensive, which is why §5 is short.
- **[the product's standing rules](../the product's standing rules), no view-options menus.** No list-mode
  toggle, no column chooser, no sort dropdown, no free zoom slider (the
  grid-size clause narrowed by ADR-0028 to exactly those forms; three detents
  in the place's own body are not a menu). There is no overflow menu to sweep a
  control into, so every control must have a real home. A law that could answer
  "put it in a menu" would not have to be a law.
- **`.interface-design/system.md` §13, L5 and L6.** Each surface declares the
  alignment edges it permits and the hierarchy it intends. Adding a control to a
  surface is therefore already an argued act at the composition level; this law
  is the same discipline one level up, at the level of *which surface*.

### 0.2 The surfaces, after the places change

Five kinds of home exist, and only five:

| Home | What it is | Present when |
|---|---|---|
| **The bar** | The now-playing bar, at the window's bottom edge, with the needle flush under it | Always, in every place |
| **A place's strip** | The band across the top of a place. The Library's strip is today's top bar; the Album, Queue and Settings places have a header of the same height | Only in its own place |
| **A place's body** | The wall; the album's tracklist; the queue's rows; the settings' sections | Only in its own place |
| **Another place** | Library · Album · Queue · Settings. One at a time, each filling the window | When you navigate to it |
| *(nowhere)* | Not a control: a gesture, a key with no button, a hover reveal | Refused |

the product's standing rules deletes the fifth. There is no sixth: no rail, no popover, no
panel, no menu, no dialog, no tray.

### 0.3 The vocabulary this law needs

Four words, because the law behaves differently for each and the disagreements
in §2 are all disagreements about which of the four a thing is.

- **An act** changes something: `Play album`, `Shuffle`, a stepper, the fader.
- **A door** changes only where you are: `Settings`, `Queue`, `Back`.
- **A readout** states a fact and takes no press: the counts, the signal note,
  the continuation lane, `3 / 12`.
- **An object** is the content itself, acted on in place: a sleeve, a track row,
  a queue row, an index letter, a needle segment.

### 0.4 The four subjects

Every act, door and readout in baz is about exactly one of four things, and so
is every surface:

| Subject | Means | Its resident surface |
|---|---|---|
| **library** | the collection: what you own, how it is arranged, what matches | the Library place's strip |
| **playback** | the sounding music: what is playing, where in it, how loud, what is next | the bar |
| **view** | this window: where you are looking, how big the covers are, which place you are in | the place's own body, or nowhere |
| **preference** | the application's standing decisions | the Settings place |

The audit's original diagnosis of the top bar was that it *had no subject* — it
carried search and the counts (library), `Queue` (playback) and `Settings`
(preference) in one strip. ADR-0016 fixed that instance. This law is the general
form of that fix.

---

## 1. The inventory — every control baz has

Bands are from `03` §1.2. Class is from §0.4. "Today" is the shipped app at
`b9e57a0`; the places change moves several of these wholesale, and §3 says where
they land.

### 1.1 The Library place's strip (today: the top bar)

| Control | What it does | Workflow | Band | Class | Today |
|---|---|---|---|---|---|
| Search well | Filters the wall; the only widget in baz that takes keyboard focus | W3 find a known thing | **B** | library | strip, far left |
| `ARTIST` `YEAR` `GENRE` `ADDED` `PLAYED` | Ask `baz-core` for a different arrangement of the collection (ADR-0019) | W13 deal with a large library; W4 browse | **C** | library | strip, beside the well |
| `Shuffle` | Plays from the pool the wall is currently showing | W5 play without deciding | **C** | library | strip, after the keys |
| `Pull` | Offers one long-unplayed record; starts nothing | W5, the deliberate half | **C/D** | library | strip, after `Shuffle` |
| `Settings` | Door to the Settings place | W11 adjust sound, W14 manage roots | **E** | *door* | strip, far right |
| Counts — `1 284 albums · 9 902 tracks` | How big the collection is | W4 context | — | *readout* | strip, far right |
| Counts — `7 of 1 284 albums` | How many matched what you typed | W3 | — | *readout* | strip, far right |
| `scanning…` / `N files skipped` / a problem | What the library is doing to itself | W14 | — | *readout* | strip, far right |

### 1.2 The Library place's body — the wall

| Control | What it does | Workflow | Band | Class | Today |
|---|---|---|---|---|---|
| Album tile — single press | Selects; raises the album | W9 inspect | **B** | *object* | the wall |
| Album tile — double press | Plays from track 1 | W1 put on an album | **A** | *object* | the wall |
| Index rail entry | Jumps the wall to a letter, decade, genre or recency bucket; marks where you are | W13, W4; the Sonos regression (`03` §4.5) | **A** | view | the wall's right edge, `INDEX_LANE_W` 108 |
| Scrollbar | States and changes position | W4 | **A** | view | the wall's right edge, a 10 px lane |

### 1.3 The album (today an inspector; becoming a place)

| Control | What it does | Workflow | Band | Class | Today |
|---|---|---|---|---|---|
| `Play album` | Plays the selected album | W1 | **A** | library | inspector, under the header |
| Edition selector | Chooses which rip of an album you own plays | W9, W15 compare | **D** | library | inspector, under the header |
| Track row | Plays from that track | W1, W7 | **B** | *object* | inspector, the list |
| ✕ | Closes the inspector | — | — | *door* | inspector, header line |
| `Details`, catalogue, condition, the PLAYED card | Twenty fields' worth of what the scan read | W9, W16 verify | — | *readout* | inspector body |
| The pull's note | *The pull · Last played 3 years ago* | W5 | — | *readout* | inspector, above `Play album` |

### 1.4 The queue (today a popover; becoming a place)

| Control | What it does | Workflow | Band | Class | Today |
|---|---|---|---|---|---|
| Queue row | Jumps playback to that entry | W7 change what is next | **C** | playback | popover body |
| Row ✕ | Removes that entry | W7 | **C** | *object* | popover row, on hover |
| ✕ | Closes the popover | — | — | *door* | popover header |
| `3 of 12 · 38:12 left` | What remains, not what the list contains | W6 see what is next | — | *readout* | popover header |

### 1.5 The bar

| Control | What it does | Workflow | Band | Class | Today |
|---|---|---|---|---|---|
| Previous | Steps back an entry | — (`03` §4.1.1) | **A** | playback | bar centre |
| Play / Pause | Toggles the engine | — | **A** | playback | bar centre |
| Next | Steps on an entry | — | **A** | playback | bar centre |
| The needle | Press inside the sounding entry seeks; press elsewhere jumps | W7, W12-adjacent | **B** | playback | flush on the window's bottom edge, 2 px |
| Volume fader | Sets the per-application level (ADR-0011) | — | **B** | playback | bar, far right |
| Mute | Silences without losing the level | — | **C** | playback | on the fader's rail |
| `Queue · N` | Door to the queue | W7 | **C** | *door* | bar, left zone |
| Now-playing title / artist | What is sounding | W12 | — | *readout* | bar, left zone |
| Continuation — `then 8 more · 1:39:10 left` | What follows, without being asked | W6 | — | *readout* | bar, left zone, third line |
| Elapsed / total | Where in the track | — | — | *readout* | bar, left zone |
| Signal note | Whether the chain is bit-exact | W16 verify | — | *readout* | bar, right of the fader |
| **Back to what is playing** | Scrolls the wall to the sounding record | **W12** | **A** | view | **does not exist**; ADR-0016 reserved the now-playing text for it |

### 1.6 The Settings place

| Control | What it does | Workflow | Band | Class | Today |
|---|---|---|---|---|---|
| `Back` | Returns to the Library | — | — | *door* | place header |
| Section list (`Playback`) | Chooses a section | W11 | **E** | *door* | place, left column |
| ReplayGain mode | Off / Track / Album | W11 | **E** | preference | Playback section |
| Pre-amp ± | Trims tagged gain | W11 | **E** | preference | Playback section |
| Untagged pre-amp ± | Trims files with no tags | W11 | **E** | preference | Playback section |
| `Keep peaks below full scale` | Limiter on or off | W11 | **E** | preference | Playback section |

### 1.7 First run

| Control | What it does | Workflow | Band | Class | Today |
|---|---|---|---|---|---|
| Folder well | Where the music lives | W14 | once | preference | the setup place |
| Submit | Starts the scan | W14 | once | preference | the setup place |

### 1.8 What the inventory shows before any law is applied

- **Thirty-four pressable things.** Nine are objects, six are doors, and only
  nineteen are acts and preferences — which is why placement, not inventory
  size, is the problem.
- **The strip holds one band-E control and the bar holds none.** That is the
  right shape already, and it is entirely due to ADR-0016 having moved
  `Settings` out of the rail. Nothing in the bar is rarer than band C.
- **The wall carries two controls for one fact.** The index rail and the
  scrollbar both state position; only one of them says it in the collection's
  own words.
- **The loudest object in the top bar is not a control at all.** `06` §6.1
  measured the empty search well's 1 px border at **33.2 %** of the strip's
  total ink — a box drawn around nothing, louder than the collection's own
  count. It is the same failure this document exists to prevent, wearing a
  different costume: something occupying prime resident space that no workflow
  asked for.
- **The most frequent workflow in the study has no control anywhere.** W12,
  *get back to what is playing*, is band A. Roon, Spotify, Apple Music and Tauon
  each spend a dedicated affordance on it (`03` §3, W12). baz spends none.

---

## 2. The law

> ### L8 — One home per control, and the home is the surface that shares its subject
>
> **Every control has exactly one home.** Its home is the surface whose subject
> it shares, and a control's subject is **what it must consult in order to
> know what to do** — never what it changes.
>
> **Frequency does not choose the surface. It chooses only whether that surface
> is resident or a place you navigate to.** Bands A–C may be resident; band D
> may be resident only as one word in a cluster that already exists; band E
> never is.
>
> **A control that only navigates is placed where the hand already is** when it
> wants to go there, and it is labelled with the name of what it opens.
>
> **Facts may be restated in every place that has a vocabulary for them.
> Controls may not: no two controls may send the same message.**

Seven clauses follow. Each is written to settle a case rather than to describe
one.

### L8.1 — Subject is what a control reads, not what it writes

`Shuffle` writes the engine and reads the wall. `Next` writes the engine and
reads the queue. `Play album` writes the engine and reads the selection. If
writing the engine decided a control's home, all three would belong in the
transport — and so would a double-click on a sleeve, which would put the wall in
the bar. The rule has to run the other way round, and reading is the half that
discriminates:

| Control | Must consult | Subject | Home |
|---|---|---|---|
| `Shuffle` | what the wall is currently showing | library | the Library strip |
| `Pull` | the collection and the play ledger | library | the Library strip |
| Group keys | `baz-core`, for a differently shaped answer | library | the Library strip |
| `Play album` | the selected album | library | with the album |
| Next / Previous / needle | the queue the engine holds | playback | the bar |
| Volume / mute | the engine's output stage | playback | the bar |
| Index rail | the arrangement and the viewport | view | the wall's own edge |
| Density | the viewport, and nothing else | view | three detent marks on the rail's lane; the gesture accelerates them (ADR-0028 — this row read *a gesture, no chrome* until doc 11 §5 P8 showed that answer contradicted the visible-control rule) |
| ReplayGain | `config.toml` | preference | the Settings place |

This is the clause that answers the brief's hardest question — *where does an
action that starts playback from the library belong?* — and it answers it
without special-casing: **`Shuffle` and `Pull` are library controls, and they
stay in the Library place's strip.** Their pool is what the wall shows
(a standing rule of the product — *no invisible shuffle pools*), so moving them away from the
wall would separate a control from the only thing that tells it what to do.
`views/top_bar.rs` already argues this in its own words; the law makes the
argument general rather than local.

It also decides cases that have not been raised. *Shuffle this album* would read
one album and therefore belong with the album, not in the strip. *Repeat* would
read the queue and belong in the bar. *Scan now* reads the roots and belongs in
Settings.

### L8.2 — Frequency decides residency, never the surface

A control earns a resident slot only if **all three** hold:

1. its workflow is band A, B or C;
2. the surface that shares its subject is resident in the place where the
   workflow actually happens;
3. it can be said in one word or one 32 px box, inside a cluster that already
   exists.

Everything else navigates. Band E never resides even at one word, because a
resident control is paid for **by the collection, every frame, forever** — `03`
§2.3's measurement of content share at rest (73–100 % against a tradition that
manages 0–26 %) is the single number the product is positioned on, and residency
is the only thing that spends it.

Band D is the interesting boundary, and `Pull` is the case: it is roughly weekly,
and it is admitted because it is one word joining a row of words that is already
drawn. A band-D control that needed its own cluster, its own line or its own
glyph would not clear (3) and would navigate.

### L8.3 — When subject and frequency disagree, subject wins the surface and frequency wins the prominence

They disagree constantly, and the resolution is asymmetric on purpose: a control
in the wrong *place* cannot be found at all, whereas a control that is one layer
too deep merely costs a click. So frequency is never allowed to move a control
onto a surface with the wrong subject. What frequency is allowed to decide is
whether the control is resident (L8.2), whether it is a word or a glyph, and
which keyboard layer it gets (§4).

**And when frequency demands residency that the budget refuses, make the *fact*
resident and leave the *control* where it belongs.** This is the escape valve,
and baz has already built it twice without naming it:

- **The queue.** Seeing what is next (W6) is band B; changing it (W7) is band C.
  The place cannot be resident — it is a place. So the bar states the fact
  ambiently in `continuation_note` — *then 8 more · 1:39:10 left* — and the door
  beside it opens the place. `views/bottom_bar.rs` says it exactly: *knowing
  costs nothing; opening is for changing.*
- **The chain.** Verifying bit-exactness (W16) is band D and its controls are
  band E, in Settings. The *fact* is resident, one line, beside the fader that
  is the only control that can break it.

A resident readout costs a slot; a resident control costs a slot **and** a
decision every time the eye passes it. The valve is why this law can be strict
about controls without making anything harder to know.

### L8.4 — Doors are placed by the hand, not by the subject

A door's subject is the place it opens, but placing it there is a contradiction
in terms. So a door is placed where the listener already is when they want it,
and it takes three obligations:

1. **It is labelled with the name of the place**, in words. `03` §5.2(e) and
   §4.5 are unambiguous: the closest product to baz in ambition hid the same
   surface behind an unlabelled gesture and generated years of *"where is my
   queue / what did I just do"*, and a gesture-first redesign elsewhere was
   reversed after two years and a CEO.
2. **A door is not a toggle.** It has no lit "open" state, because the place it
   opens fills the window and there is no frame in which the door could be both
   lit and visible. `views/top_bar.rs` already draws `Settings` this way; under
   the places model the `Queue` control joins it (§3).
3. **Every place has a visible door in and a visible way out**, and the way out
   is the word `Back`. This is the Sonos mitigation (`03` §4.5): named
   destinations, reachable and leavable without a gesture. It is also the only
   part of an accessibility tree baz can honestly offer today (ADR-0017 §4).

Consequently `Settings` stays at the far right of the Library strip, `Queue`
stays in the bar's left zone beside the track it counts, and the album's door is
the sleeve. baz has **no nav rail**, because three of its four places have a
subject you can only mean while looking at something specific — a record you
pointed at, the queue the bar is reporting, the application. A rail listing
places you cannot enter without context would be a fifth resident surface
charging the collection for a list.

### L8.5 — An object is not a control

The law governs chrome: things that stand apart from what they act on. Acting on
the thing itself is not a placement question — the object is its own control and
it lives where the object lives. A sleeve, a track row, a queue row, an index
letter and a needle segment are all in this class.

This is why `Play album` is a control and double-clicking a sleeve is not, and
it is the clause that keeps the two from being read as duplicates under L8.6.
It is also bounded by an existing refusal — *nothing is ever drawn on top of a
sleeve* — so an object may be pressed, but nothing may be drawn on it to say so.

### L8.6 — One control per message; facts may repeat, controls may not

Two controls may not send the same message. The failure this prevents is
documented in our own history: the top bar's `Queue · 13` and the bar's position
readout stated one fact in two places, and *the far one went stale* — it went on
saying 13 after the run had ended (ADR-0016). Two controls for one message means
two states to keep in step, and the product will eventually fail to.

Facts are the opposite. The playing record is haloed and dotted on the wall,
named in the bar, dotted in the album's track list and lamped in the queue's
row — four statements, each in its own surface's vocabulary, all derived from
one `PlayerState`. That is not duplication; that is a single fact having a word
in every language on screen, and `03` §4.2's warning about YouTube Music's 48 px
thumbnail duplicating an 860 px cover on the same screen is about a *redundant*
statement, not a translated one.

The test is mechanical: **would a reader have to be told which of the two is
authoritative?** If yes, delete one.

### L8.7 — The keyboard is the same decision, made twice

Full treatment in §4. The clause: a control's key layer is decided by the same
frequency argument as its residency, and its screen home decides which key
within the layer. A shortcut may never reach a control that has no visible home,
and the rule is one-directional — every action needs a control; not every
control needs a key.

---

## 3. Applying it

Every control in §1, with where it is, where the law puts it, and why. The rows
that do not move are here on purpose: a law that only justifies changes is a
rationalisation of changes already wanted.

### 3.1 The full table

| Control | Today | Prescribed | Why |
|---|---|---|---|
| Search well | Library strip, left | **unchanged** | Reads the collection; band B; one box in an existing cluster (L8.1, L8.2) |
| *— its border at rest* | 1 px ring, always | **deleted** | Not a control: a mark aiming you at a control that type-anywhere makes unnecessary. 33.2 % of the strip's ink (`06` §6.1, defect 6) |
| Group keys ×5 | Library strip | **unchanged** | They ask `baz-core` for a different answer, so they read the collection, not the view (L8.1) |
| `Shuffle` | Library strip | **unchanged** | Reads the wall's current pool; the wall is what tells it what to do (L8.1) |
| `Pull` | Library strip | **unchanged** | Same subject; band D admitted as one word in an existing cluster (L8.2) |
| `Settings` | Library strip, right | **unchanged** — and confirmed as a door, not a toggle | A door goes where the hand is; not in the bar, whose pixels are reserved (L8.4) |
| Collection counts | strip, right | **unchanged** | A readout of the collection, resident because the fact changes unasked (L8.3) |
| Match count `7 of 1 284` | strip, right | **moves to the search well's own slot** | A readout belongs beside the control whose effect it reports. Today the number you are watching sits ~1 200 px from the keys producing it (L8.3, L8.6) |
| Scan / skipped / problem notes | strip, right | **unchanged** | Facts about the library that change without being asked |
| Album tile | the wall | **unchanged** | An object (L8.5) |
| Index rail | the wall's right edge | **unchanged, and it inherits position** | Reads the arrangement and the viewport; the only position control left (L8.1, L8.6) |
| **Scrollbar** | the wall's right edge | **deleted** | Two controls, one fact, and this one says it in pixels while the rail says it in letters and decades (L8.6) |
| `Play album` | inspector | **the Album place**, same relation to the sleeve | Reads the selected album; the album is now a place (L8.1) |
| Edition selector | inspector | **the Album place** | Same subject |
| Track row | inspector | **the Album place** | An object (L8.5) |
| Inspector ✕ | inspector header | **becomes `Back`** in the Album place | Every place leaves by a labelled way out (L8.4) |
| **`Ctrl+B`, hide the inspector** | keyboard only | **deleted** | Its subject was a sidebar that no longer exists; and it is the one binding in the product with no control at all (`app.rs`'s declared exception) |
| Queue row | popover | **the Queue place** | An object (L8.5) |
| Row ✕ (remove) | popover row, on hover | **the Queue place's row, drawn at rest** | An object — but hover-only affordances are refused (a standing rule of the product); a place has the width to draw it |
| Popover ✕ | popover header | **becomes `Back`** in the Queue place | L8.4 |
| `3 of 12 · 38:12 left` | popover header | **the Queue place's header** | A readout of the place you are in |
| `Queue · N` | bar, left zone | **unchanged in position; becomes a door, losing its lit "open" state** | The place it opens fills the window; a door cannot be lit and visible at once (L8.4) |
| Continuation lane | bar, left zone | **unchanged** | The resident *fact* that pays for the queue being a place (L8.3) |
| Previous / Play / Next | bar centre | **unchanged** | Read the sounding queue; band A; and the product's standing rules ratchets the bar's slots |
| Needle | window's bottom edge | **unchanged** | Reads the sounding queue; it is the object of position (L8.5) |
| Elapsed / total | bar, left zone | **unchanged** | Readouts of the sounding track |
| Volume fader / mute | bar, right | **unchanged** | Read the engine's output stage (ADR-0011) |
| Signal note | bar, right of the fader | **unchanged** | The band-D fact made resident beside the one control that can break it (L8.3) |
| Now-playing title / artist | bar, left zone | **becomes the control for W12** — press returns to the Library place and scrolls the wall to the sounding record | Band A with no home at all; ADR-0016 reserved this exact target and left the gesture to a later increment |
| Settings `Back`, section list, ReplayGain controls | Settings place | **unchanged** | Band E, preference subject, a place (L8.2) |
| First-run well and submit | the setup place | **unchanged** | A place you are in exactly once |

### 3.2 The debt the places change creates, and where it must be paid

ADR-0016 kept the album as a column for one stated reason: *the browse loop is
click, read, click the next sleeve, and a full-window album view turns a
one-click compare into a three-step round trip.* Promoting the album to a place
spends that, and W15 — *compare two releases* — is band B, which the ranking says
may cost one click **and nothing else lost**. A round trip loses the wall.

The law does not get to reverse a decision the owner has taken, but it does get
to say where the cost lands: **the Album place must carry the wall's own step to
the next and previous record**, so that comparing two releases stays one press
per release. This is `03` R9 and §7.2(5) arriving from a different direction —
Lightroom keeps the Filmstrip in Loupe, Calibre shares one `BooksModel` across
all four views with a `PreserveViewState` guard, and both are cataloguer-grade
products that refused to let detail hide the collection. Either a strip of
sleeves along the bottom of the Album place, or a labelled previous/next pair in
its header. Not a gesture, and not nothing.

> **Amended 2026-08-09 by the owner** (the owner's own decision: his
> decision is sufficient on its own, and the entry gets rewritten rather than
> argued). The header pair shipped, and he withdrew it: *"previous and next on
> albums doesn't make sense on the album view. we could add an Artist > album
> breadcrumb though. and have an artist page."*
>
> **The principle above stands; the two prescribed forms do not.** Both stepped
> along *the wall's current arrangement* — a property of the Library place, and
> one that is not on screen from a record's page. So both offered a door whose
> destination the listener could not know before pressing it, which is not the
> collection reachable from its detail; it is a walk through a context you have
> left. The third form the owner supplied satisfies this section's own stated
> aim better: **`Artist › Album` in the header, with the artist half a door to
> an Artist place holding their records.** The record's context is its artist —
> a fact about the record rather than about the frame — and every record you
> reach that way is one you saw before choosing it. W15's *compare two
> releases* is one press up and one press down.
>
> The clause that survives verbatim is **"not nothing"**. See
> `11-jobs-era-critique.md` §5 P3 for the full reversal.

---

## 4. The keyboard is part of placement

`keys.rs` already decides keys by frequency, and it is right: *`Q` is bare
because a view key is pressed dozens of times a session; `Ctrl+,` earns its
modifier because a preferences key is pressed a handful of times in a lifetime.*
ADR-0016 quotes both and observes that together they are an argument that the
two surfaces should not be siblings. That is this law in miniature — so the key
map and the screen map are one decision made twice, and the rule is:

> **Frequency chooses the layer. The screen home chooses the key within it.**

### 4.1 The layers, after type-anywhere

ADR-0017 §1.2 spends the bare-letter layer on the query: any bare printable
character filters the wall. That abolishes a layer, and **an abolished layer
grants no exceptions on frequency grounds** — `Q` was correctly bare and is
correctly `Ctrl+U` now, because the argument that beat it is not "the queue got
rarer" but "there is no bare-letter layer left to be in". What compensates is
L8.3: the bar states the continuation ambiently, so the frequent half of W6
costs no key at all.

| Layer | What may live there | Because |
|---|---|---|
| Bare **letters** | the query, and nothing else | ADR-0017 §1.2 |
| Bare **digits** | controls resident in the current place's strip, in the order that strip draws them | a projection of what is on screen |
| Bare **Space, arrows, Esc** | the transport, the fader, and peeling a layer | not letters; universal; and `Space` is the one binding every listener already owns |
| **Modifier layer** | doors to other places, layout acts, and anything band D or rarer | a modifier is a tax that a monthly act never actually pays |
| **No key** | band-E controls inside a place you have already navigated to | you are already there; a shortcut into a section is a memory tax with no repayment |

### 4.2 The digits, and the collision the code already flagged

`keys.rs` binds `1`–`5` to the five group keys, derived from `GroupKey::ALL`
rather than copied from it, and `6` to the wall's **subject**
([ADR-0035](../adr/0035-the-wall-has-a-subject.md)) — the sixth word in the
same row, which is the law below applied to a word rather than an exception to
it. It records that ADR-0017 §1.2 also spends `1` and
`2` on the Wall / Marquee lens switcher — *"the resolution is a decision for step
18 rather than a thing to guess at now"*.

The law resolves it without a tie-break: **the digits belong to the row of words
that projects the collection.** The group keys read `baz-core` (L8.1) and are the
Library strip's own arrangement; the lens switcher decides how the whole place is
drawn, which is a layout act, and layout acts are modified — the same reasoning
that put `Ctrl+B` on a modifier when there was a sidebar to hide. So the digits
stay with the arrangement and the lens takes a modified key when it is built.

### 4.3 What the mirror forbids

- **A key that reaches a control with no visible home.** Already tested in
  `app.rs`; the law adds that a new exception may not be *declared*, only
  *removed* — and it removes the only one, since `Ctrl+B`'s subject is gone.
- **A bare letter in any place.** Including in the Album, Queue and Settings
  places, where there is no wall to filter: bare letters there do nothing, rather
  than meaning something different in each place.
- **A key for an action with no control.** One-directional, and `keys.rs` already
  holds the line in the right direction: `Shuffle` has a visible control and
  deliberately no key, *"because starting a shuffle is a decision made once an
  evening, from a word that is already on screen."*

---

## 5. What should not exist

Five, and the last two are the same defect at different scales.

1. **The search well's border at rest.** The loudest object in the top bar is a
   `360 × 30` rectangle drawn around an empty field — 33.2 % of the strip's ink
   (`06` §6.1, defect 6), for a control that, after type-anywhere, you reach by
   typing rather than by aiming. A mark whose only job is to say *a control is
   here* is not a control, and it is charging resident rates. The well stays; the
   ring becomes focus-only.
2. **The wall's scrollbar.** Two controls state position and one of them says it
   in a vocabulary the collection does not have. The index rail says `T`, `1974`,
   `Never played`; the scrollbar says *38 %*. Sonos's most-quoted regression was
   losing jump-to-letter (`03` §4.5), and the rail is what answers it. (This
   deletion is already in flight; the law supplies the general reason.)
3. **`Ctrl+B` and `TogglePanels`.** A layout key for a sidebar that the places
   model deletes. It is also the product's only binding with no on-screen
   control — `app.rs` names it as the single declared exception to the
   visible-control test — so deleting it closes the exception rather than
   widening it.
4. **The lit "open" state on the `Queue` door.** A door to a full-window place
   cannot be both lit and visible; the state would only ever be drawn in frames
   nobody sees. `Settings` already does without it.
5. **Any second dismissal gesture.** `03` R12 counted four ways to dismiss one
   inspector — ✕, `Esc`, `Ctrl+B`, and clicking the tile again — and named it as
   Strawberry's six-sidebar-modes instinct in miniature. Under the places model
   there are exactly two: `Back` and `Esc`. A third is a defect however
   convenient it feels.

**And what the law declines to delete**, because a rule that only subtracts is
as unprincipled as one that only adds:

- **Mute, next to a fader that can reach zero.** Not one fact twice: mute is a
  state you return *from* with the level intact. Different messages, different
  capabilities, both stay.
- **Previous and Next, beside a needle that can jump to any entry.** `03` R11:
  three vendors bought visual calm by removing control density inside two years
  and all three reversed, and what was lost was always position, provenance and
  skip. the product's standing rules ratchets this bar.
- **`Play album`, beside a sleeve you can double-click.** L8.5: the gesture is
  the object, the button is the control, and the visible-control rule requires
  the second.
- **The five group keys, beside a search field.** Search narrows, keys arrange.
  Both consult the collection and neither can be derived from the other.

---

## 6. How it is pinned

The other seven laws each carry a test, because the project's own history is
that an unpinned rule drifts. The shape here is the one L6 already uses —
**declare, then assert the declaration against what the code actually does** —
and the project has the exact precedent in
`theme::the_declared_hierarchy_is_the_geometry_that_produces_it`.

### 6.1 The declaration

A small pure module — `crates/baz/src/placement.rs`, iced-free, ADR-0006 layer 1
— holding one table:

```rust
pub(crate) enum Subject { Library, Playback, View, Preference }
pub(crate) enum Kind    { Act, Door, Readout, Object }
pub(crate) enum Band    { A, B, C, D, E }
pub(crate) enum Home {
    Bar,
    Strip(Place),
    Body(Place),
    Place(Place),
}

pub(crate) struct Control {
    pub name:    &'static str,
    pub subject: Subject,
    pub kind:    Kind,
    pub band:    Band,
    pub home:    Home,
}

/// Exhaustive over `Message`: adding a variant fails the build until it
/// declares a home.
pub(crate) fn home_of(message: &Message) -> Option<Control> {
    match message { /* no wildcard arm */ }
}
```

The non-wildcard `match` is the load-bearing part. It makes a new user-causable
message a **compile error** until someone writes down where the control that
sends it lives — the same trick L2 uses to fail the build rather than the review.
`None` is reserved for messages no control sends (`Playback`, `ThumbLoaded`,
`ScanTick`, `WindowResized`) and the arm has to be written to claim it.

### 6.2 The assertions

| Assertion | What it catches |
|---|---|
| `every_control_declares_one_home` | Two controls sending one message (L8.6); a message with no declared home |
| `the_declared_home_is_the_surface_that_draws_it` | The declaration drifting from the view. Each view module publishes the messages it can emit, and the set must equal the table's partition for that home |
| `no_surface_holds_a_control_from_another_subject` | The top bar's original defect — a strip with no subject (L8.1). The one permitted exception is `Kind::Door`, and doors are enumerated |
| `frequency_never_promotes_a_control_past_its_band` | Band E resident anywhere; band D resident outside an existing cluster; a band-A control more than one press from rest (L8.2) |
| `every_place_has_a_door_in_and_a_way_out` | A place reachable only by a key, or leavable only by `Esc` (L8.4) |
| `every_bare_key_belongs_to_a_control_this_place_draws` | The keyboard mirror (L8.7), and it subsumes the existing `every_keyboard_binding_is_a_press_some_control_also_makes` |

### 6.3 One thing worth fixing while this is built

`app.rs`'s `every_keyboard_binding_is_a_press_some_control_also_makes` documents
itself as *"checked exhaustively rather than by sampling"*, but its exhaustiveness
comes from a **hand-written list of keys to sweep** — and that list has no digits
in it. `1`–`5` were bound to the group keys after the test was written, so
`GroupKeySelected` never had to appear in its `CONTROLS` table and nobody
noticed. The property is still true (the words are on screen); the *proof* has a
hole, and it is exactly the hole a table keyed off `Message` rather than off a
list of keys does not have.

### 6.4 What it cannot check

Stated so the test is not read as more than it is. It cannot measure whether a
control is *findable*, it cannot rank two controls' prominence within one surface
(L6 does that, over rendered ink), and it cannot tell a translated fact from a
duplicated one — L8.6's *"would a reader have to be told which is
authoritative?"* is a judgement made in review. What it can do is make every
future placement an argued act, which is the whole of the complaint this document
answers.

---

## 7. Proposed text for `.interface-design/system.md` §13

Proposed only. **This document does not edit that file.**

Add to the laws table:

| Law | Pinned by |
|---|---|
| L8 one home per control, and the home shares its subject | `placement::every_control_declares_one_home`, `placement::the_declared_home_is_the_surface_that_draws_it`, plus `app::every_bare_key_belongs_to_a_control_this_place_draws` |

And the section:

### L8 — One home per control, and the home is the surface that shares its subject

> Every control has exactly one home: the surface whose subject it shares, where
> a control's subject is **what it must consult to know what to do** — never
> what it changes. Frequency does not choose the surface; it chooses only whether
> that surface is resident or a place you navigate to. Bands A–C may be resident;
> band D only as one word in a cluster that already exists; band E never.
> A control that only navigates is placed where the hand already is, is labelled
> with the name of what it opens, and is not a toggle. Objects are acted on where
> they are and are not controls. **Facts may be restated in every place that has
> a vocabulary for them; controls may not — no two controls send the same
> message.**

baz added controls where they fit at the time and each placement was locally
defensible: the group keys beside the search field because both are about the
library, `Shuffle` and `Pull` beside them because there was room, `Settings`
top-right because that is where settings go, `Queue` and the fader bottom-right
because both are playback. The result had no rule in it, and the audit's own
diagnosis of the top bar — *a strip with no subject* — was the first symptom.
The bands are `docs/design/03-interface-prior-art.md` §1.2, measured against
sixteen products rather than guessed; the four subjects are ADR-0016's four
kinds seen from the control's side. The full argument, the inventory of all
thirty-four controls and the case-by-case application are
`docs/design/07-control-placement.md`.

The clause that does the most work is the first: subject is what a control
**reads**. It is what puts `Shuffle` in the library strip rather than the
transport (it must consult the wall to know its pool), the needle in the bar
(it must consult the queue), and the index rail on the wall's own edge. And the
escape valve is the second most useful: when frequency demands residency the
budget refuses, **make the fact resident and leave the control where it belongs**
— which is what the bar's continuation lane and its signal note already are.

---

## 8. What this document does not decide

- **The Marquee lens's key**, beyond ruling that it is not a digit (§4.2). Which
  modifier is step 18's.
- **Where a control goes that reads two subjects at once.** None exists today.
  When one is proposed, the law's answer is that it is two controls or it is
  mis-specified — but that is an assertion this document has no case to test it
  on.
- **Anything about size, weight or order within a surface.** L5 and L6 own that,
  and `06` §6 measures it.
- **Whether the album should have become a place.** That is the owner's, and it
  is taken. §3.2 says only where the debt lands.
