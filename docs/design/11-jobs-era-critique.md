# 11 — The Jobs-era critique: baz against the Apple of 1984–2010

> The owner's brief, verbatim: *"can we create an adversarial agent which
> examines the current UI layout and UX and proposes changes which match more
> with the UI UX theory of Steve Jobs era apple."*
>
> This document is that agent's report. It attacks the shipped interface from
> the standpoint of Jobs-era Apple design theory — the classic Macintosh Human
> Interface Guidelines through the iPod / iTunes / early-iPhone era — and it
> attacks the repo's own reasoning as evidence rather than authority: every
> REFUSALS entry and ADR it challenges is steelmanned first, at full strength,
> before the counter-argument is made. Where the existing design *wins* by
> Jobs-era standards, that is said plainly, because an adversary who calls
> everything wrong is as useless as one who calls everything right.
>
> **This document changes no code and edits no ledger.** §5's proposals are
> ranked and tiered; the owner adjudicates. Proposals that would overturn a
> `REFUSALS.md` entry or an ADR say so explicitly and carry the argument the
> ledger's editing rule demands — an ADR that beats the entry's argument, not
> a preference.

---

## 0. Method and evidence

**What was examined.** Every render capture in `docs/design/impl/` with the
newest sets read closely (`places/`, `playlists/`, `queue-parity/`,
`context-menus/`, `controls-iconography/`, `songs-search/`,
`index-magnification/`); the design corpus (`docs/design/01–10`,
`.interface-design/system.md`, `docs/REFUSALS.md`); ADR-0014 and ADR-0016
through ADR-0026; and the shipped view code where a claim needed the source
(`views/top_bar.rs`, `views/queue.rs`, `views/playlist.rs`,
`views/bottom_bar.rs`).

**The captures were verified against HEAD.** One headless run of the real
binary (`target/tb/release/baz`, built 2026-08-09) on a private Xvfb at
1280×860, with all six redirections from `docs/DEVELOPMENT.md` — scratch
`HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`,
`XDG_RUNTIME_DIR`, and `DBUS_SESSION_BUS_ADDRESS` unset — a null-device
`.asoundrc`, a throwaway 25-album silent fixture, and processes stopped by
pid. The isolation receipt, from both launches:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

Two frames were captured fresh (first run with no config; the wall against
the fixture) and both match the committed captures: ADR-0026's strip is
shipped (magnifier in the well, counts as its placeholder, the gear far
right), and the first-run screen still asks for a typed absolute path. Every
visual claim below is against that verified state.

---

## 1. The theory argued from, with sources

Jobs-era Apple left an unusually explicit written theory. The attacks in §2
each name their principle; the principles are these.

### 1.1 The classic HIG's named principles

The 1987 *Human Interface Guidelines: The Apple Desktop Interface* names ten
design principles; the 1992 *Macintosh Human Interface Guidelines* carries
the same ten and promotes an eleventh, modelessness. Four of them do most of
the work in this critique. Quotations are verbatim from those texts (archived
copies cited in §6):

- **Metaphor.** *"Metaphors from the real world"* — concrete, familiar, and
  judged here by function: a metaphor earns its keep by predicting
  behaviour, not by decorating it.
- **Direct manipulation.** *"Direct manipulation allows people to feel that
  they are directly controlling the objects represented by the computer"*
  (1992). The user acts on the object itself, and the screen changes as the
  hand moves. Its strongest instrument in practice was **drag and drop**.
- **See-and-point.** The 1987 heading is, in full, *"See-and-point (instead
  of remember-and-type)"*, and its body is the era's sharpest sentence
  against command-line survivals inside a GUI: *"Users rely on recognition,
  not recall; they shouldn't have to remember anything the computer already
  knows. Most programmers have no trouble working with a command-line
  interface that requires memorization and Boolean logic. The average user
  is not a programmer."*
- **Forgiveness.** 1987: *"Users make mistakes; forgive them. The user's
  actions are generally reversible — let users know about any that aren't."*
  1992: *"Always warn people before they initiate a task that will cause
  irretrievable data loss"* — and, the sentence that ranks the mechanisms,
  *"frequent alert boxes are a good indication that something is wrong with
  the program design."* The 2001 Aqua HIG names the instrument outright:
  *"create safety nets, such as the Undo command, so people feel comfortable
  learning and using your product."* Reversibility first; a warning is the
  fallback for the case undo cannot reach, never the default posture.

The rest of the list — **consistency** (*"allows people to transfer their
knowledge and skills from one application to any other"*), **WYSIWYG**,
**user control** (*"Allow the user, not the computer, to initiate and
control actions"*), **feedback and dialog**, **perceived stability**
(*"stable reference points"* against complexity), **aesthetic integrity**
(*"information is well organized and consistent with principles of visual
design"* — beauty in the service of comprehension), and **modelessness** (a
mode *"restricts the operations that the user can perform while it is in
effect"*; modes are to be avoided or made visible and transient) — appears
where relevant, by name.

Two more era doctrines matter here. **Progressive disclosure** is the 1992
HIG's own vocabulary, verbatim: *"It allows you to present the most common
choices to users while initially hiding more complex choices or additional
information"* — the power is hidden until *disclosed*, which is an act, not
an accident of scroll position. And the era's **menu discipline**: *"People
don't have to remember command names because they can see all the options at
any time and choose any available option"* (1992, Menus) — the menu bar as
the discoverable enumeration of capability, with keyboard shortcuts taught
*in* the menus, beside the verbs they accelerate. The same chapter's rule
for gestures is load-bearing for §2 and §5: *"Double-clicking must never be
the only way to perform a given action"* — every gesture is an accelerator
over a route that exists in the visible interface.

### 1.2 Jobs's own additions

- **Focus is subtraction.** *"People think focus means saying yes to the
  thing you've got to focus on. But that's not what it means at all. It means
  saying no to the hundred other good ideas that there are."* (WWDC 1997,
  closing session.) And, to Isaacson: *"Deciding what not to do is as
  important as deciding what to do."*
- **Simplicity is work.** *"That's been one of my mantras — focus and
  simplicity. Simple can be harder than complex: You have to work hard to
  get your thinking clean to make it simple."* (BusinessWeek, "There's
  Sanity Returning", 1998.)
- **Design is behaviour.** *"Design is not just what it looks like and feels
  like. Design is how it works."* (Rob Walker, "The Guts of a New Machine",
  The New York Times Magazine, 2003.)
- **Defaults over configuration.** The recurring keynote mantra "it just
  works" (no single canonical era utterance — cited as posture, not text):
  the product ships with the right answer, and configuration is the
  admission of a decision the designer refused to make.

### 1.3 The iPod lesson

One navigation primitive — the wheel plus one drill-in hierarchy plus one
Menu button meaning *up*, always — done perfectly, beats many primitives done
adequately. Apple's own 2001 launch copy sells exactly this: one-handed
operation through the scroll wheel, acceleration through long lists, "1,000
songs in your pocket". Note what the iPod did **not** promise: one click to
sound. A song was several clicks deep and nobody minded, because every click
was the same click and the hierarchy never lied. The iPod also shipped
without an on/off switch — the canonical "deciding what not to do" artefact,
as recounted in Walker's 2003 NYT profile. The lesson is *uniformity of the
primitive*, not minimal click counts.

### 1.4 The iTunes lesson

iTunes 1–4's enduring layout: **source list + library + detail, visible at
once**, a persistent spatial frame in which nothing you did made the library
disappear. Its two defining gestures: double-click anything to play it, and
**drag** anything onto a playlist in the always-visible source list. (The
three-pane characterisation is this report's own reading of the shipped
screens — Version Museum's iTunes 1–4 captures — rather than a quotation;
Apple's 2001 launch copy claims the *effect*: "Apple has done what Apple
does best — make complex applications easy", one window, single-click
browsing, real-time search.) The lesson is *simultaneity and spatial
permanence* — the library as the ground you stand on while you work — and it
is the strongest era evidence against this product's central structural
decision, which is why §2.3 and §5's P10 take it at full strength.

### 1.5 Language

Era Apple never shipped a control a first-timer couldn't name. Buttons are
verbs in the user's vocabulary; the interface speaks about the user's things
("your music"), not the program's internals; poetic names were reserved for
*features with marketing weight behind them* (Genius, Cover Flow) and even
those were explained on screen at first contact.

Sources for §1: the archived 1987 and 1992 HIG texts and the 2001 Aqua HIG,
the WWDC 1997 closing session recording, the 1998 BusinessWeek interview,
Isaacson, the 2003 NYT profile, and Apple's 2001 iPod and iTunes press
releases. Exact URLs are in §6; two flags are recorded there for claims that
could not be verified to the sentence.

---

## 2. The examination

Screen by screen, principle by principle. Each finding is marked **[attack]**
or **[credit]**; attacks carry the principle they proceed from.

### 2.1 First run — `impl/00-first-run-after.png`, re-verified fresh

**[attack — see-and-point]** This is the worst screen in the product by
Jobs-era standards, and it is the first one a person meets. The hero question
("Where's your music?") is era-grade copy — plain, confident, second person —
and then the *only* affordance under it is a text field whose placeholder is
`/path/to/your/music` and whose footnote reads `Enter confirms · next time
baz remembers, or run baz DIR`. That is **remember-and-type**, verbatim: the
user must recall an absolute filesystem path and type it without a typo, on a
screen with nothing to see and nothing to point at. The era's own answer has
existed since 1984: a button that opens the system's file browser, and (since
drag and drop) the window as a drop target. The footnote compounds it by
teaching CLI invocation syntax — `baz DIR` — to every first-run user,
including the majority who will never open a terminal.

The repo knows. `01-ux-audit-and-ia.md` §1.1 called this *"the single worst
moment in the product today, because it is the first one"* and specified
*"a folder picker and a drag-target"* (§4.7). ADR-0025 then shipped the
picker — `Browse…`, portal-backed, four packages, `cargo deny` green — **to
the Settings place only**, and deliberately not here: *"One question, one
field is that screen's whole design, and its validation still stats on the UI
thread… If the picker earns its place there, it earns a look of its own."*
(ADR-0025 §3.) Steelmanned: one-field purity is a real aesthetic; the UI-thread
stat is a real defect to avoid; a NAS path a dialog cannot show is a real
case. But none of the three survives contact: a `Browse…` button beside the
field *is* one question with two doors (ADR-0025's own §1 argues exactly this
shape for Settings); `check_folder` already exists on the blocking pool; and
the typed field *stays* under this critique's proposal, exactly as ADR-0025
kept it. The deferral has no remaining argument. §5 P1.

**[credit]** Everything else about the screen is era-correct: one question,
no wizard, no account, no theme chooser, the wordmark deliberately unlit
"because nothing is playing, which is the product teaching what lit means
before it ever means anything" (`02-visual-language.md` §6.8). The scan-status
line added since the audit — *"Covers land on the wall as they are read.
Nothing waits for the scan."* — is feedback-and-dialog done right.

### 2.2 The Library place — `impl/places/01`, `impl/queue-parity/01`, fresh capture

**[credit — aesthetic integrity]** The wall is the pillar and it lands. Art
at full size on a near-black room, chrome that is genuinely type, captions
that are quiet, an accent spent *only* on playback truth, and a collection
share (73–100 % of the window at rest) that embarrasses the tradition's
0–26 % (`03-interface-prior-art.md` §2.3). The 1992 HIG's aesthetic-integrity
clause — organization serving comprehension — is met here better than
Jobs-era Apple itself usually managed; iTunes 4 gave your library a table.

**[credit — see-and-point]** The index rail is the era's alphabet index done
one better: it *names the shelf it will take you to* (`T`, `1974`, `Never
played`) and magnifies under the pointer as pure function of position, no
clock, no tween. Losing the wall's scrollbar for it (ADR-0022) trades a
convention for a strictly richer statement of the same fact — the era would
have blinked at a missing scrollbar and then conceded the rail says more.
Sonos's most-quoted regression was losing jump-to-letter (`03` §4.5); baz's
answer is structural.

**[attack — direct manipulation / the removed double-click]** Playing a
record from the wall — the product's *"one gesture to music"* pillar, the
single most frequent intent in the product — now takes two presses across a
full navigation: open the page, find `Play album`, press it. ADR-0022 states
the loss with complete honesty: *"The friction budget's intent → sound = 1
click is **not met from the wall** and this ADR does not pretend otherwise."*
By iTunes' bedrock — double-click anything, hear it — this is a regression on
the product's own home surface. Steelmanned (ADR-0022 "Deliberately not
done"): the candidates it refused are genuinely refused elsewhere — a bare
`Enter` is keyboard-only (REFUSALS, accessibility), a play glyph on hover is
a mark on a sleeve (REFUSALS, artwork) — and the double-click died
structurally, because the first press now navigates and no tile survives
under the pointer for the second. All true. But the era offers a fourth
candidate the ADR never lists, examined as §5 P7.

**[attack — their own visible-control rule / density]** Wall density is
`Ctrl+scroll` / `Ctrl+-` / `Ctrl+=` with — `system.md` §7.1's own words —
*"no density row, no grid-size picker and no zoom readout."* The corpus's
governing rule is REFUSALS': *"Every action in baz has a visible,
pointer-reachable control. No action is keyboard-only, and no control's only
affordance is hover."* A modifier-scroll gesture is not a visible control; a
first-timer will die never knowing the wall zooms. The prior-art study's own
R7 says *"Ship a density control"* and marks itself **CONTRADICTS** against
the refusal, citing Steam and Google Photos taking *"durable reputational
damage"* for removing size controls. Era precedent is direct: Jobs-era
iPhoto and iTunes' own grid view each carried a small thumbnail-size slider
in the window chrome. The refusal ("no grid-size picker") and the gesture's
undiscoverability cannot both stand comfortably; §5 P8 presents the options.

**[attack — feedback]** `Shuffle` and `Pull` are bare words with no tooltip
(`views/top_bar.rs::draw_word` — the tooltip machinery two hundred lines
below them is spent on the gear). For `Shuffle` the word is load-bearing and
almost enough (but see §2.10); for `Pull` it is not (see the language audit).
The era named icon-only controls in tooltips *and* taught every command's
meaning in a menu; baz has no menus by refusal, which raises the price of
every unexplained word.

### 2.3 The Album place — `impl/places/02`, and the round-trip question

The mandate's first named angle: the places model's round-trips against the
iTunes lesson.

**Steelman first, at full strength.** ADR-0022 is the best-argued document in
the corpus. The same owner rejected resident side surfaces twice, in plain
words. The 340 px column the prior art defended was measured into
indefensibility — a sleeve at 93.6 % of the panel's ink, the album's own name
fifth of eight, three of twelve tracks visible on a soundtrack. The
full-window page gives the record 3.5× the width, twenty `Details` fields,
and a hierarchy the composition laws can actually declare. The wall keeps its
scroll, query and arrangement across every navigation; the last-opened record
keeps a mark; `Esc` is one press, one meaning. And the model has era
pedigree the ADR never claims for itself: **the window holds one place at a
time, every place has a labelled door in and a labelled way out, `Esc` always
goes up** — that is the iPhone (2007) and the iPod before it, the era's own
answer at small scale. When a prior (sixteen products) and an observation
(this owner, twice) disagree, the observation wins. All of this stands.

**[attack — the iTunes lesson, priced honestly]** What the places model
cannot express is **simultaneity**, and the era's verdict is that a music
library tool is *built* on simultaneity: source list, library and detail
visible at once, the library as ground. ADR-0022 concedes the costs one by
one — *"Comparing two records is a round trip… the single biggest thing this
costs"* (W15, named as Marta's actual loop, supported by no music player and
formerly by baz); knowing what is queued costs the wall; drag targets cannot
exist between surfaces that are never co-present. And the corpus itself has
already been forced into the first exception: the playlist panel (ADR-0024)
exists *because* "collecting is two-surface work, source and destination on
screen at once" — the model's own amendment concedes the era's point for the
one workflow where the owner felt it personally. The panel is the proof that
simultaneity was a real loss, not a nostalgia.

This critique nevertheless does **not** propose restoring a resident library
pane — §5 P10 explains why the rejection stands on era grounds as well as on
the owner's. What it does propose is paying the debts the repo has already
acknowledged and not paid:

**[attack — an unpaid, self-prescribed debt]** `07-control-placement.md`
§3.2, on the album's promotion to a place: *"the Album place must carry the
wall's own step to the next and previous record, so that comparing two
releases stays one press per release… Either a strip of sleeves along the
bottom of the Album place, or a labelled previous/next pair in its header.
Not a gesture, and not nothing."* **This shipped as nothing.** The place has
no previous/next affordance of any kind (verified in `views/` and the
captures). Lightroom's Loupe keeps the Filmstrip; Calibre shares one model
across views; the study named both. The repo's own law document prescribed
the mitigation; the implementation skipped it; W15 is now strictly worse than
the column ever made it. §5 P3.

**[credit — language]** `‹ Library` as the way out, the place named `Album`
in the header, the door being the tile itself: all era-plain. But—

**[attack — consistency of vocabulary]** The same header strip says
`‹ Library` at its left and *"Esc returns to the wall"* at its right. Two
names for one destination, thirty ems apart. "The wall" is the design
corpus's beloved internal name — correctly *internal*, like "the hang" and
"the pull's weighting", none of which ship as labels. Here the poetry leaks:
a first-timer has been told about a place called Library and is now promised
return to something called "the wall" he has never heard named. The 1992
HIG's consistency clause is about exactly this. One word per thing, and the
word already chosen is `Library`. §5 P4.

### 2.4 The Queue place — `impl/places/05`, `impl/queue-parity/03`

**[credit]** One list, one cursor, albums listed as albums, the lamp dot
replacing the track number in a column that never changes width, history rows
faint above the cursor, `2 of 12 · 49:31 left` as the summary, and the empty
state saying *"Nothing queued / Play an album and it appears here. / When a
queue ends, baz stops."* That empty state is era-grade writing — it teaches
the fill gesture and states the product's most controversial refusal in six
words, at the exact moment the refusal is felt. Edit parity with the playlist
page (▲▼, ✕, `+`, `Save as playlist`) makes the queue and playlist pages one
editor — one grammar, learned once, the iPod lesson applied to editing.

**[attack — forgiveness]** Every edit on this surface is instant and
irreversible. The ✕ removes an entry with no confirmation *and no undo*; ▲▼
reorders with no undo; there is no `Ctrl+Z` anywhere in the product; the word
"undo" does not appear in the UI corpus at all (the only match in
`docs/` is a persona note about future tag editing being "undoable"). The
1992 HIG's forgiveness principle puts reversibility *first* and warnings
second; baz ships neither on the queue and the warning-only pattern on
playlist delete. What makes this attack sharp rather than generic is that
**baz's own architecture has already paid for undo**: every queue edit is a
whole-list `UpdateQueue` computed by pure functions from the previous list,
and every playlist edit is an atomic whole-file rewrite. The previous state
is a value the code held in its hand and dropped. Undo here is a stack of
values and one message — no engine change, no protocol change. §5 P2.

**[attack — direct manipulation]** ▲▼ steppers are the era's definition of
manipulation-by-proxy: pressing an abstraction that nudges the object,
instead of picking the object up. The repo knows this too — the steppers are
documented as *"the no-drag pointer route the visible-control rule requires;
drag-to-reorder arrives with the shared pointer-capture widget"*
(ADR-0024 §4) — and the drag is deliberately last: *"it ships last and is
sugar over routes that already work"* (§6 layer 3). Steelmanned: iced 0.13
has no pointer capture; the visible-control rule needs the steppers to exist
*anyway* as the accessible route; shipping working routes first was correct.
But "sugar" is the one word the era would strike. Drag is not sugar over
direct manipulation; it *is* direct manipulation — iTunes' primary gesture,
the owner's original ask (*"it should be really easy to drag a song into a
playlist"*, quoted in ADR-0024), and doc 09 §11 prices its absence at a
twenty-track build going from ~22 presses to ~40. One hand-built widget
(`groove.rs` precedent) unlocks queue reorder, playlist reorder and
drag-to-panel at once. It is the single highest-leverage piece of interaction
work left in the product, and it is sequenced as dessert. §5 P5.

**[observation, not attack]** The hover-revealed ✕/▲▼/`+` with permanently
reserved slots: the refusals ban controls whose *only* affordance is hover,
and each of these has a non-hover route (menu, and removal-by-playing-out).
The reserved slot means no layout shift. Era Apple mostly drew controls at
rest, but a column of twelve permanent crosses down a list of what you are
about to hear is, as ADR-0022 puts it, "a column of invitations to destroy
something". This is defensible; noted, not proposed against.

### 2.5 Playlists — `impl/playlists/02–08`, the panel and the picker

**[credit — focus, honesty]** The model underneath is the best thing in this
feature area by any era's standards: a playlist is a file the user owns, in
the format the audience already has, written only on the user's own edit,
never mutated by the machine (ADR-0024 §1–§2). *Files as the interface* is
sovereignty the era professed and rarely delivered. The three honesty clauses
would have survived Jobs's red pen untouched.

**[credit — modelessness]** The armed collecting mode shipped one day,
answered *"what does this press do"* with *"it depends what you armed
earlier"*, and was removed on the owner's own observation (doc 09 §9). That
is the classic HIG's modelessness principle enforced by instinct, and the
withdrawal — recorded, priced, with the one-press economy passed to the menu
and the future drag — is the healthiest single decision in the recent record.

**[credit — simultaneity, correctly rationed]** The panel itself: summoned by
a labelled door, one tenant forever, overlaying without reflow, closed at
rest. As argued in §2.3, it quietly concedes the iTunes point — and it
concedes it in the era's own best form, a *transient* source list.

**[attack — forgiveness, again]** `Delete` on the playlist page is a
two-press confirm: *"Delete "{name}"? The file goes; your music stays."* The
copy is genuinely excellent — plain, confident, names what survives — but the
*pattern* is the era's fallback, shipped as the default. The HIG's ranking is
reversible-first: deletion of a small file is the textbook reversible act.
Move the `.m3u8` to the platform trash (or a baz-owned graveyard the page can
restore from) and the confirm becomes unnecessary; the era's Trash metaphor
exists precisely so that delete never needs a dialog. Also under this head:
`Save as playlist` refuses name collisions outright rather than offering
overwrite-with-confirm — correctly cautious, explicitly deferred in doc 09
§8.3, fine — and playlist row edits share the queue's no-undo posture. §5 P2.

**[attack — see-and-point, minor]** The picker's flow — press `Add to…`, pick
a destination — is modeless, two presses, and its first row is the Queue:
good grammar. But the *entry* control on the album page, `Add to…`, is a
word floating below `Play album` with no glyph and no object; the era would
give the verb its object (`Add to playlist…` — the context menu already says
exactly this) so the button predicts the picker. One word of debt. §5 P4.

### 2.6 Songs search — `impl/songs-search/02`

**[credit]** Type-anywhere with the well kept visible is the correct
resolution of see-and-point against remember-and-type: the *field* is the
visible affordance, the typing is accelerated from anywhere, and the counts
live in the well's placeholder so the corpus is described at rest. A ranked
`Songs` section above the filtered wall, rows that play on press (needle-drop
— the record queued whole, cursor on the song), sections that never mix:
tiles navigate, rows play, *"the two meanings never mix"* (doc 09 §5). That
sentence is the iPod lesson stated better than Apple ever stated it. The
ranking is explainable-by-construction — *"any two results can be explained
by naming the first signal that separates them"* (ADR-0021) — which is "it
just works" done with receipts.

**[attack, minor — feedback]** `Enter` plays the top-ranked song, and nothing
on screen says so before the first press. The era taught accelerators beside
their commands; baz's teaching surfaces are tooltips and empty states, and
neither carries this. A hint in the empty-query well or the Songs section
rule would close it. Folded into §5 P6.

### 2.7 Context menus — `impl/context-menus/02–06`

**[credit — the era's own accelerator doctrine, exactly]** The mirror
rule — *"every menu item sends a message some visible on-screen control also
sends, and no action's only route is a menu"*, pinned by
`every_menu_item_is_a_press_some_control_also_makes` (doc 09 §5.2, shipped) —
is the 1992 HIG's own gesture contract (*"Double-clicking must never be the
only way to perform a given action"*) generalised to a whole interaction
class, and stated more rigorously than Apple ever stated it. Menu items as verbs (`Open`, `Play album`, `Queue album`, `Add to
playlist…`), edge-flipped floats, one at a time, `Esc` peels first. This is
the strongest new-in-2026 surface in the product by era standards.

**[attack — discoverability of the whole layer]** The menus mirror controls;
nothing mirrors the *gestures*. Shift-click-to-queue exists nowhere visible —
not in a tooltip, not in a hint, not beside the menu's `Queue album` item
(where the era would print the accelerator, as menus always printed `⌘Q`
beside Quit). `Ctrl+scroll` density likewise (§2.2). Type-anywhere is carried
by the well; `Esc` by the header hints; but the modifier layer as a whole has
no enumeration anywhere a user could stumble on. The era's answer was the
menu bar as the complete catalogue of capability; baz refuses the menu bar
(`03` §6.1 treats it purely as a dated-product marker, and — the distillation
of the corpus is blunt on this — *the discoverability cost of having no menus
is never priced anywhere in the study*). The refusal can stand — the context
menu, the tooltips and the empty states are together a sufficient teaching
surface — but only if they are actually spent on the gestures. Today they are
not. §5 P6.

### 2.8 The bar and the needle — `impl/controls-iconography/10`, `impl/places/05`

**[credit — perceived stability]** The bar is the product's masterpiece by
the era's own hardest criterion: *nothing moves as the music moves*. Every
slot reserved in every state, timestamps in fixed slots, the signal note
appearing into space that was always its own, the continuation lane (`then 10
more · 49:31 left`) as an ambient fact costing no gesture, and the slot
ratchet in REFUSALS (a slot may be added, never removed for tidiness) built
from R11's three-vendor reversal evidence. TIDAL, Spotify and YouTube Music
each bought calm with removed facts and reversed within two years; baz wrote
the lesson into a ledger. `bit-perfect` beside the one control that could
make it untrue is feedback-and-dialog at its best, and *"no snake oil"* is
scientific honesty the era's marketing sometimes lacked.

**[credit — language]** `Queue` as a labelled word-door with the count in a
reserved slot; the now-playing text as the labelled door to the sounding
record (W12, band A, unowned for months, closed by ADR-0022 §"Getting back");
`Nothing playing` at rest. All plain, all nameable by a first-timer.

**[observation]** The needle at 2 px flush on the window's bottom edge is the
one control a first-timer may never find (its hit lane is 12 px, its visual
is 2). It has era pedigree regardless — it is a progress bar being honest —
and seek-by-click has the transport as its visible sibling. Not proposed
against; recorded because a fresh-eyes test should watch for it.

### 2.9 The gesture layer, audited as a whole

The on-ramp table the mandate asked for. "On-ramp" = where a user who does
not read documentation meets the capability.

| Capability | Gesture | Visible control (rule) | On-ramp today | Verdict |
|---|---|---|---|---|
| Filter the wall | type anywhere | the well, magnifier, counts placeholder | the well itself | **sound** |
| Play top match | `Enter` after typing | the Songs rows | none — unannounced | gap (§5 P6) |
| Queue a record | shift-click a tile | menu `Queue album` → picker Queue row | context menu only; accelerator taught nowhere | gap (§5 P6) |
| Zoom the wall | `Ctrl+scroll` / `Ctrl+±` | **none** — `system.md` §7.1: no row, no picker, no readout | none | **breach of their own rule** (§5 P8) |
| Peel a layer | `Esc` | header hint lines; `‹ Library` | the hint lines | sound (wording, §5 P4) |
| Go to queue / settings / playlists | `Ctrl+U` / `Ctrl+,` / `Ctrl+P` | labelled doors | the doors | sound |
| Context menus | right-click | mirror rule: every item has a visible twin | convention itself | sound |
| Reorder / remove / transfer rows | hover-revealed glyphs | reserved slots; menu twins | hover discovery + menus | acceptable |
| Drag anything | — | — | **does not exist** | the era's primary, missing (§5 P5) |

Two sound systems (typing, doors), one principled accelerator layer taught
nowhere, one gesture that breaches the product's own visible-control rule,
and the era's primary gesture absent entirely. The pattern: baz is rigorous
about *routes* (everything has one) and casual about *introductions* (almost
nothing is introduced).

### 2.10 The language audit

The era test: could a first-timer name what the control does before pressing
it? Every word that ships on screen:

| On-screen | First-timer test | Notes |
|---|---|---|
| `Where's your music?`, `Play album`, `Play all`, `Queue`, `Playlists`, `Settings`, `‹ Library`, `Back`, `Browse…`, `New playlist`, `Save as playlist`, `Rename`, `Delete`, `Open` | **pass** | Verbs and place names in user vocabulary; the `…` convention honestly promising a dialog (ADR-0025 §1) is itself classic-HIG. |
| `ARTIST · YEAR · GENRE · ADDED · PLAYED` | pass | One row of words, no menus; self-revealing on press, reversible on press. |
| `Shuffle` | **pass with a caveat** | The word is universal; baz's behaviour is a bounded draw of 8 that ends in silence. ADR-0026 refused the crossed-arrows icon *because the convention promises a mode* — the sharpest icon reasoning in the corpus — but the **word** carries the same promise at lower volume. A first press behaves as expected; the surprise is deferred to the 8th record's end, where the queue's empty state ("When a queue ends, baz stops.") catches it — but does not say a new press is a new draw. One sentence closes it (§5 P6). Renaming is examined and rejected (§5 P12). |
| `Pull` | **fail** | No convention (ADR-0026: "the pull has no convention at all"), no tooltip (`top_bar.rs::draw_word`), no on-screen explanation until *after* the press. A first-timer cannot form any expectation. The name is crate-digger's vocabulary — pulling a record from the shelf — and the metaphor rule (REFUSALS, skeuomorphism: the record supplies *vocabulary*) genuinely licenses it; but the era licensed poetic names only with explanation at first contact. The offer's own note (*"The pull · Last played 3 years ago"*) is that explanation, one press too late. §5 P9. |
| *"Esc returns to the wall"* | **fail** | The one leak of internal poetry into shipping copy, inconsistent with `‹ Library` in the same strip. §5 P4. |
| `Add to…` | marginal | The object is missing; the menu's own `Add to playlist…` is the better label. §5 P4. |
| `bit-perfect`, `48 → 44.1 kHz`, `FLAC · 16-bit · 44.1 kHz` | pass | Audience vocabulary (Karl's), quiet, never a sales pitch. The era would have hidden it; baz's audience is why it must not. Credit. |
| `then 10 more · 49:31 left`, `2 of 12 · 49:31 left`, `Nothing playing`, `Nothing queued`, `25 albums · 206 tracks` | pass | Readouts in plain figures; `MONO` standing in for tabular figures. |
| `The file goes; your music stays.` | pass (wording), fail (pattern) | See §2.5. |

The scorecard is strong: of the whole shipped vocabulary, two fails and two
marginals — and *"drop the needle"*, *"the hang"*, *"the stack"*, *"the
wall"*, *"places"* are all correctly kept internal (the one leak noted).
The corpus's own rule (`02` §2.7: room vocabulary for the system's private
names, plain words wherever the software speaks) is era-correct and almost
perfectly executed.

### 2.11 Undo, systemically

Gathered from §2.4 and §2.5 because it is one finding, the largest in this
report: **the product contains no reversal of any kind.** No undo message, no
undo stack, no trash, no restore. Its two destructive-edit surfaces (queue,
playlist) ship instant irreversible edits; its one deletion ships a
confirmation. The classic HIG's forgiveness principle is the era doctrine
baz violates most squarely — and, as argued in §2.4, most cheaply repaired,
because the whole-list edit model means every prior state was a value in
hand. The visible-control rule does not block it; it *shapes* it (undo needs
a visible twin — a transient word in the edited place's header — exactly as
every accelerator in baz already has one). §5 P2 carries the design.

---

## 3. Where baz beats the era standard

Named plainly, per the mandate, because several of these are the product's
identity and this report proposes building on them, not around them.

1. **The refusals ledger is "deciding what not to do" made institutional.**
   Jobs practised subtraction as personal taste; baz wrote the subtraction
   down, gave every "no" an argument, and made removal cost an ADR that beats
   the argument. The era has no artefact like `REFUSALS.md`. It is the most
   Jobs-era thing in the repository, *including* its willingness to be
   overruled by measurement (ADR-0020's motion amendment, ADR-0025's picker).
2. **Silence at the end of the queue.** *"The software will not decide to
   continue for you"* (ADR-0023 §5, re-tested against Longplay's reversal and
   reaffirmed with three standing answers-in-advance). Autoplay is the
   engagement era's invention; refusing it is user control in the 1992 sense.
3. **Defaults over configuration, near-absolutely.** No view-options menus,
   no theme chooser, no accent picker, no layout editor — the entire
   foobar2000 disease ("configuration before usability", fooyin's first frame
   being a layout-editing mode) refused at the root. Doc 09 adds features and
   *zero* settings. "It just works" as a budget line.
4. **One grammar per meaning.** Tiles navigate, rows play; play means now, a
   second gesture means later; `Esc` peels; every place has a door in and a
   way out. The iPod lesson — one primitive done perfectly — applied to a
   desktop product, and enforced by tests
   (`every_menu_item_is_a_press_some_control_also_makes`, the placement law's
   non-wildcard match).
5. **Modelessness enforced against its own shipped feature** (the armed
   mode's next-day withdrawal, §2.5). The 1992 HIG would quote this episode
   approvingly if it could.
6. **The bar's ratchet and the reserved-slot discipline** (§2.8): perceived
   stability as an asserted property, with the era's three-vendor counter-
   evidence archived in the ledger.
7. **Honesty as copy**: genre tags shown verbatim ("messy tags show,
   honestly"), `ADDED`'s refusal to fabricate a backfill, "not reachable
   right now — N tracks kept, nothing removed", the condition report that is
   never a sales pitch. Feedback-and-dialog with the dial set to truthful.
8. **The empty states teach.** "Play an album and it appears here." / "When a
   queue ends, baz stops." / "Esc clears the search." Era Apple's care at
   exactly these moments, present and correctly spent.
9. **Aesthetic integrity with receipts**: one accent meaning one true thing,
   contrast-weighted ink hierarchies *measured* per surface, typography as
   structure on a 4 px lattice with laws and tests. The era asserted taste;
   baz proves it frame by frame.

These are not consolation prizes; §5's proposals are constrained to preserve
every one of them.

---

## 4. A note on what this report declines to attack

- **The gallery metaphor over the hi-fi metaphor.** The record supplies
  physics and vocabulary, never surface; no wood grain, no tonearms. Judged
  by function (per the mandate), the gallery metaphor is carrying real
  weight — the lamp, the hang, the wall label all *predict behaviour* — and
  skeuomorphic surface was the one thing even era Apple over-shipped
  (Corinthian leather came later, but brushed metal was Jobs-era). No attack.
- **No back stack; `Esc` is total.** Album → Queue → `Esc` lands on the
  Library, not the album. This looks like a consistency defect and is
  actually the iPod's Menu button: *up*, always, never *back along my path*.
  One meaning per gesture beats path-memory. ADR-0022's refusal of a history
  stack survives era scrutiny.
- **The one-window model itself** — see §5 P10.
- **Type-anywhere despite `03` R8's warning.** The resolution shipped (bare
  letters filter; transport keys move to the modifier layer; the field stays
  visible) is see-and-point *and* speed; the era would sign it.

---

## 5. The proposals, ranked

Each proposal names the principle it serves, steelmans the strongest existing
rationale against it (cited), states the concrete change, prices it, and
carries a verdict tier: **adopt** / **adopt-modified** / **present-to-owner**
/ **rejected-with-reasons**. Nothing here is implementation; the owner
adjudicates.

---

### P1 — First run gets the picker and the drop target · **adopt**

- **Principle**: see-and-point, instead of remember-and-type (1992 HIG); the
  first-run care the era lavished (§1.5).
- **Existing rationale, steelmanned**: ADR-0025 §3 deferred the picker here
  on three grounds — the screen's one-question purity; first-run validation
  still stats on the UI thread; "if the picker earns its place there, it
  earns a look of its own." The purity argument is real: this screen's whole
  design is one question, and it is good.
- **Why it loses**: ADR-0025 §1 itself ships the two-door shape (`Browse…`
  beside a typed well) for Settings and argues it is *one* acceptance path,
  not two designs. The same shape here is still one question — "Where's your
  music?" — with a pointing answer and a typing answer. `check_folder`
  already runs on the blocking pool; reusing it removes the UI-thread stat
  rather than inheriting it. The typed field **stays** (the unmounted-NAS
  argument is correct and is not disturbed).
- **The change**: a `Browse…` control beside the field (same acceptance path
  as Settings); the window as a drop target for a folder if/when iced's
  file-drop event is available on all three targets, else deferred without
  blocking the button; placeholder text becomes a human sentence (`Your music
  folder`) rather than `/path/to/your/music`; the `baz DIR` CLI teaching line
  moves to `--help`/README where its audience lives. Hero question and
  unlit wordmark untouched.
- **Cost**: small — the dependency shipped in ADR-0025; one view; copy.
- **Refusals/ADR impact**: supersedes one clause of ADR-0025 §3
  ("deliberately not taken: the picker on the first-run screen") by that
  ADR's own two-door argument. No REFUSALS entry touched.

> **Shipped** (2026-08-09). `Browse…` beside the field (the Settings door's
> anatomy and message), the typed submission moved onto the blocking pool
> (`check_folder` — the deferral's UI-thread-stat ground removed, not
> inherited), the placeholder now *Your music folder*, and the `baz DIR`
> teaching moved to `--help`. The drop target shipped **as far as the
> toolkit delivers one**: winit 0.30 publishes file-drop events on X11 and
> not on Wayland (verified in its Wayland backend — no data-device handling
> at all), so a drop opens the folder where the event exists, hover feedback
> appears only when the platform actually reports a drag, and the screen's
> copy advertises nothing it cannot keep — the adopt-modified deferral,
> recorded in ADR-0025 §3's superseded clause. One extra find: the screen's
> scan-status line itself said "the wall"; P4's sweep caught it and the
> copy now reads *"Covers land as they are read."*

### P2 — Forgiveness: undo for list edits, trash for deletion · **adopt-modified**

- **Principle**: forgiveness (1992 HIG: reversibility first, warnings only
  for the irreversible); low-stakes exploration.
- **Existing rationale, steelmanned**: no document refuses undo — it is
  absent, not rejected. The nearest arguments: the visible-control rule (no
  keyboard-only actions — so `Ctrl+Z` alone would be illegal by their own
  law); doc 09 §8.3's deliberate deferral of overwrite-on-confirm ("guessing
  at it now would add a destructive path to the product's one naming flow") —
  a *good* forgiveness instinct, note; and the general subtraction posture:
  every mechanism must earn its place. Also real: a global undo *system*
  (every action reversible, app-wide stack) is a large concept for a product
  this disciplined about concepts.
- **Why the absence still loses**: the two surfaces that destroy user work —
  queue and playlist edits — are whole-list/whole-file value semantics
  already (`queue_edit`'s pure functions over `UpdateQueue`; atomic `.m3u8`
  rewrites). The previous state is computed and discarded on every edit
  today. And the product's own delete confirm proves the need is felt; the
  era's ranking just orders the mechanisms differently.
- **The change**, modified to fit baz's laws rather than imported wholesale:
  1. **One-deep edit undo per list surface.** After any destructive edit on
     the Queue or a Playlist page (remove, reorder), the place's header
     carries a transient word — `Undo` — beside the summary, until the next
     edit, a navigation, or the run ending. Pressing it restores the prior
     list (`UpdateQueue` / one file rewrite). `Ctrl+Z` is its accelerator,
     legal because the visible twin exists (the same construction as every
     accelerator in doc 09 §5.2). No toast, no popover, no timer — a word in
     a strip, in the product's own grammar.
  2. **Playlist `Delete` becomes reversible instead of confirmed.** The
     `.m3u8` moves to the platform trash (one small crate, priced and
     `cargo-deny`-gated like `rfd` was in ADR-0025) — or, if the dependency
     is refused, to `$XDG_DATA_HOME/baz/playlists/.deleted/` with a
     restore row on the Playlists panel. The two-press confirm and its
     excellent sentence are then retired with honour: forgiveness beats
     warning. If neither mechanism is accepted, the confirm stays — it is
     the correct fallback, and the era would agree.
  3. **Not proposed**: app-wide undo of playback acts (play/seek/volume are
     not destructive; the era never undid Play either).
- **Cost**: one small pure state (previous-list value per place), one header
  word, one keybinding; optionally one vetted crate. No engine change, no
  protocol change.
- **Refusals/ADR impact**: none overturned. Extends ADR-0014/0024's edit
  surfaces; the header word must clear doc 07's L8 (it reads the place's own
  edit history: subject = the place; resident while band-B-frequent — it
  clears) and L9's strip budget (one short word in pages whose strips are
  near-empty; it clears).

> **Shipped** (2026-08-09) as **ADR-0027**, with two upgrades inside P2's
> own shape: the history is a bounded stack (depth 8, `crates/baz/src/undo.rs`)
> rather than one-deep, and **append** joins remove and reorder in the
> undoable set on both surfaces. The transient `Undo` word stands beside the
> queue's summary and the playlist page's counts, `Ctrl+Z` over it; a queue
> undo goes out as `UpdateQueue` and nothing else (pinned: nothing ever
> sounds), a playlist undo passes the same fingerprint guard as the edit it
> reverses, and provenance rides in the snapshots. The trash crate was
> priced and accepted (`cargo deny` green; fourteen lock entries, three
> crates on the compiled Linux graph; flatpak sources regenerated), so
> `Delete` is one press into `$XDG_DATA_HOME/Trash` and the confirm dialog
> is retired with honour, exactly as §2 prescribed — the fallback graveyard
> was not needed.

### P3 — Pay the comparison debt: previous/next record in the Album place · **adopt**

- **Principle**: the iTunes lesson's minimum viable form under a one-place
  model; user control (the collection reachable from its detail).
- **Existing rationale, steelmanned**: ADR-0022 lists W15's round trip as the
  decision's single biggest cost, takes it knowingly, and ships real
  mitigations (wall state preserved; last-opened marked; `Esc` one press).
  "Deliberately not done" lists a shelf strip beside the page — a side
  surface, rejected twice — and nothing else.
- **Why the gap still loses**: the corpus's own law document already ruled:
  doc 07 §3.2 — *"the Album place **must** carry the wall's own step to the
  next and previous record… Either a strip of sleeves along the bottom… or a
  labelled previous/next pair in its header. Not a gesture, and not
  nothing."* The strip variant is arguably the side surface the owner
  rejected; the **header pair is not** — it is two labelled doors in a strip
  that has exactly three tenants today (`‹ Library`, `Album`, the hint). It
  was prescribed, it cleared L8/L9 by the prescribing document's own
  analysis, and it never shipped. This is not even an era attack; it is the
  repo agreeing with the era and forgetting.
- **The change**: `‹ Prev` / `Next ›` (or the two records' own names,
  truncated — the more era move) in the Album place's header, stepping along
  the wall's current arrangement — the same order, the same filtered set, so
  the pool is always the visible one (the shuffle rule's logic, applied to
  navigation). Comparing two releases returns to one press per release.
- **Cost**: one message, two header words, the wall's neighbour computation
  (exists — the arrangement is in memory). No new surface.
- **Refusals/ADR impact**: none. Implements doc 07 §3.2 as written;
  ADR-0022's "no side surfaces" untouched (nothing resident, nothing beside
  the page).

> **Shipped** (2026-08-09). `‹ Prev` / `Next ›` in the Album place's header
> strip (the shared `place_header`, grown one optional tenant — the frame's
> geometry cannot drift), stepping `vm::neighbours` over the wall's own
> visible order: same arrangement, same filtered set, edges inert rather
> than wrapping, and a record the wall no longer shows has no neighbours at
> all. `Ctrl+[` / `Ctrl+]` accelerate the pair (the Finder's own
> Back/Forward chord), legal because the two visible twins stand in the
> strip. Comparing two releases is one press per release again — W15's debt
> paid where doc 07 §3.2 said it must land.

### P4 — One vocabulary: retire the leak, complete the labels · **adopt**

- **Principle**: consistency; plain user-vocabulary language (§1.5).
- **Existing rationale, steelmanned**: `02` §2.7 already draws the correct
  line — room vocabulary for the system's private names, plain words when the
  software speaks — and the corpus follows it everywhere but once. There is
  no argued defence of "the wall" as shipping copy; it is a drift, not a
  decision.
- **The change**: (a) *"Esc returns to the wall"* → *"Esc returns to
  Library"* in every place header (three sites); (b) `Add to…` on the album
  and playlist pages → `Add to playlist…`, matching its own context-menu
  mirror and naming the object (the picker's first row being the Queue is
  then a discovery, not a contradiction — the menu item that queues is
  separately named `Queue album`); (c) a sweep test in the spirit of the
  existing copy tests: no string from the room-vocabulary list (`wall`,
  `hang`, `pull's` internals, `stack`, `marquee`) ships in user-facing copy
  outside its licensed uses (`Pull` the control pending P9).
- **Cost**: strings and one test.
- **Refusals/ADR impact**: none.

> **Shipped** (2026-08-09). (a) *"Esc returns to Library"* in all three
> place headers; (b) `Add to playlist…` on the record page and in the
> playlist page's empty-state copy; (c) the sweep
> (`no_room_vocabulary_ships_in_user_facing_copy`) walks every string
> literal the view sources ship, word-boundaried, licensed uses excepted —
> and it earned its keep on arrival, catching **two leaks this document
> missed**: the first-run scan line and the empty wall's own heading
> (*"Nothing on the wall yet"* → *"Nothing here yet"*), plus the "wall" in
> P6's own proposed sentences, adjusted to "the Library" before shipping.

### P5 — The drag is not sugar: build the pointer-capture widget now · **adopt**

- **Principle**: direct manipulation as the *primary* gesture, the picker as
  fallback (iTunes lesson; the owner's own verbatim ask).
- **Existing rationale, steelmanned**: ADR-0024 §6 — iced 0.13 has no pointer
  capture; the drag needs a hand-built widget; the two-press pick is the
  modeless floor that works today; *"'really easy to drag' must not mean
  'waiting on the hardest widget in the plan'"*. Sequencing routes-first was
  right, and the steppers/picker must remain regardless (they are the
  visible, accessible route the refusals require).
- **Why the sequencing now loses**: the routes shipped. The floor exists.
  Doc 09 §11 prices the drag's absence in the product's own currency (bulk
  build at 2 presses per addition, band-D conceded; ~40 presses for a
  twenty-track build); §9 names the drag as "the owner's original ask
  verbatim"; and one widget unlocks three surfaces (queue reorder, playlist
  reorder, drag-to-panel/picker). Every week it stays "last" is a week the
  product's primary curation gesture is a stepper. `groove.rs` is the
  precedent that hand-built pointer geometry is affordable here.
- **The change**: promote the shared pointer-capture widget to the next
  interaction increment; drag lands as sugar *over* the existing routes
  (exactly ADR-0024's design), with `CursorLeft`/`Unfocused` committing the
  gesture (doc 04 §2.2's documented workaround). Steppers, picker and menus
  remain — the era kept the menu command beside every drag, too.
- **Cost**: the hardest widget in the plan; already budgeted by ADR-0024 as
  "one investment, three surfaces". This proposal changes only its priority.
- **Refusals/ADR impact**: none — ADR-0024 layer 3, resequenced.

### P6 — Teach at the moment of relevance · **adopt-modified**

- **Principle**: menus taught the era's users what a program could do and
  printed the accelerator beside the verb; without menus, that duty transfers
  to the surfaces baz does have. Feedback and dialog; progressive disclosure
  in its real sense (disclosure *to* the user, not just below the fold —
  `02` §6.2's scroll-position reading of the term is the one place the corpus
  defines a principle down).
- **Existing rationale, steelmanned**: REFUSALS bans view-options menus and
  L9 bans overflow menus — neither bans teaching. The corpus already teaches
  beautifully in empty states and hints; the gap (§2.9) is that the
  *accelerator layer* is untaught, and tooltips exist only where ADR-0026's
  form rule required them (icon-only controls).
- **The change**, in the product's own grammar, no new surface kinds:
  1. Context-menu items whose accelerator exists print it, quietly, at the
     row's right edge: `Queue album   ⇧-click` — the era's menu convention,
     applied to the mirror layer. One line in the menu row view.
  2. `Shuffle` and `Pull` gain tooltips (the mechanism exists on the gear):
     *"Play 8 records drawn from what the wall shows"*; *"Offer one record
     you haven't played in years — nothing plays until you say so."* Words,
     not marketing.
  3. The queue's end-state empty line gains its missing half: *"When a queue
     ends, baz stops."* → add *"Shuffle draws again; Play all plays the
     wall."* — the refusal stated *with* the three answers ADR-0023 §5 says
     exist in advance.
  4. The Songs section's rule (or the well, once, on first search) notes
     *"Enter plays the first match."*
- **Cost**: strings, one tooltip call site, one menu-row slot.
- **Refusals/ADR impact**: none. (A first-run coach screen and any overlay
  tour were considered and are **not** proposed: the era did not ship tours;
  it shipped self-explanatory surfaces.)

> **Shipped** (2026-08-09), all four, with P4 applied to this proposal's own
> sentences ("the wall" → "the Library"): (1) the tile menu's `Queue album`
> prints `Shift-click` at its right edge — the one item with a gesture to
> print, pinned so no item can invent one; a word rather than P6's `⇧`,
> because doc 10 §3.6 bans borrowed characters and the shipped face draws
> U+21E7 as tofu (verified on a rendered frame); (2) `Shuffle` and `Pull` carry tooltips
> (*"Play 8 records drawn from what the Library shows"* — the figure pinned
> to `shuffle::SLEEVES` — and *"Offer one record you haven't played in
> years — nothing plays until you say so"*); (3) the queue's empty state
> reads *"When a queue ends, baz stops. Shuffle draws again; Play all plays
> the Library."*; (4) the Songs rule notes *"Enter plays the first match."*
> No tour, no overlay, no new surface kinds.

### P7 — One gesture to sound from the wall · **present-to-owner**

- **Principle**: direct manipulation; the product's own friction budget
  (*intent → sound = 1 press*), which ADR-0022 states is unmet from the wall.
- **Existing rationale, steelmanned — and it is strong**: the double-click
  died structurally (the first press navigates; no tile remains for the
  second); the refused candidates (`Enter` = keyboard-only; hover-play = a
  mark on a sleeve) are refused by entries this report does not contest;
  ADR-0022 points any return at the shift-click stack, "a queueing gesture
  rather than a second meaning for a press"; and the iPod's own answer
  (§1.3) was navigate-then-play — two presses of one uniform primitive, and
  nobody minded. `Play album` is a fixed 320×32 target where the old gesture
  had a 400 ms window and a documented failure mode. The current design is
  *defensible in era terms*.
- **The era counter**: the iPod was a 2-inch screen; the iTunes rule
  (double-click = play) governed the era's *library-on-a-desktop*, which is
  what baz is. The wall is the product's home; its most frequent intent
  should not be its only two-press intent. And a fourth candidate exists that
  ADR-0022 never lists — one with era pedigree (the Finder's own
  single-click-selects / double-click-opens):
  **first press opens the album place exactly as today; a second press
  arriving within the double-click window is received by the album place as
  `Play album`.** No reflow (the place is a hard cut, already painted), no
  tile needed under the pointer, no 400 ms hold on the wall, no new mark on
  any sleeve, no keyboard-only path. The fast hand gets sound in one gesture;
  the slow hand gets the page and a labelled button; nothing else changes.
  The cost is one timestamp and the rule that the album place's first ~400 ms
  treat a press in the sleeve's region as the double-click's completion — a
  hair of cleverness in a product that hates cleverness, which is exactly why
  this is presented rather than adopted.
- **Alternatives if refused**: accept the two-press state permanently and
  record it in REFUSALS (elevating ADR-0022's honest admission into a
  standing entry with its argument — the budget line then stops being "not
  met" and becomes "re-priced"); or spend the shift-click stack as the
  one-press *sound-later* and leave sound-now at two.
- **Cost**: small mechanically; nonzero conceptually (a press whose meaning
  depends on arrival time is a micro-mode).
- **Refusals/ADR impact**: touches ADR-0022's "Deliberately not done —
  restoring one-click-to-sound"; contradicts no REFUSALS entry (nothing
  drawn on sleeves, nothing keyboard-only, no reflow).

### P8 — Density: give the zoom a visible handle, or refuse it properly · **present-to-owner**

- **Principle**: their own visible-control rule; see-and-point; R7's evidence
  (Steam, Google Photos). Era precedent: iPhoto's and iTunes grid view's size
  slider, both Jobs-era.
- **Existing rationale, steelmanned**: REFUSALS — *"No view-options menus. No
  grid-size picker… density is a zoom gesture."* `02` §2.7: three named
  steps, not a free zoom, because a slider makes every screenshot different
  and every reserved-slot argument conditional; ADR-0017 §1.3: *"Settings
  must never be the answer to a view question."* These are real arguments,
  and the three-step design (not continuous) already concedes half of R7.
- **The conflict that must be resolved either way**: a gesture with no
  visible control, no readout and no Settings row is an action whose only
  route is a gesture — the exact thing doc 09 §5.2 says the visible-control
  rule forbids (*"a right-click is a gesture, and no action may be
  gesture-only"*). Today the product's own laws contradict each other, and
  the contradiction is resolved silently in favour of invisibility.
- **Options presented**:
  - **(a)** A three-detent control somewhere subject-correct. L8.1 says
    density reads *the viewport* → its home is "the place's body, or
    nowhere"; the index rail's lane or the wall's empty leading band could
    carry three quiet marks. This **overturns the REFUSALS clause** ("no
    grid-size picker") and must carry an ADR per the editing rule; the
    argument is the rule-contradiction above plus R7's CONTRADICTS finding —
    the ledger's own required form.
  - **(b)** Keep the gesture, close the contradiction the other way: amend
    the visible-control rule to enumerate its exceptions (density joining
    scroll itself, which also has no control since the scrollbar's deletion —
    the honest reading is that *view-position acts* were always exempt), and
    teach the gesture once (P6's tooltip on the group-key row, or a line in
    Settings → Appearance beside a readout of the current step). Cheaper,
    era-weaker.
  - **(c)** Status quo, refused: the contradiction stands unwritten. Not
    acceptable under the corpus's own standards regardless of era theory.
- **Cost**: (a) one small control + ADR; (b) one ledger amendment + a string.
- **Refusals/ADR impact**: (a) overturns a REFUSALS clause by ADR — stated
  openly, argument supplied; (b) amends the accessibility entry's scope by
  ADR.

### P9 — `Pull`: explain it or rename it · **present-to-owner**

- **Principle**: language a first-timer can name (§1.5); see-and-point
  (expectation before action).
- **Existing rationale, steelmanned**: the metaphor rule licenses record-
  culture vocabulary, and "pull" is genuine crate-digger usage; ADR-0026
  confirmed it as a word precisely because no icon convention exists; the
  control starts nothing (its press is safe — exploration is genuinely
  low-stakes, which blunts the harm); and the era itself shipped `Genius` —
  a poetic brand-noun — on a button in Jobs's lifetime.
- **The counter**: Genius shipped with a splash screen, a sidebar
  explanation and a marketing campaign; `Pull` ships as four letters with no
  tooltip. The era's licence for poetic names was *explanation at first
  contact*, and P6.2's tooltip is the minimum honest form of it.
- **Options presented**: (a) keep the name, ship the tooltip (P6 covers it;
  this is the recommended floor and makes P9 cost zero); (b) rename to a
  self-naming verb phrase the product can afford — candidates that survive
  the plain-voice rule: `Dust off`, `Long unplayed`, `Resurface` — any
  adoption re-clears the strip's L9 single-line budget first, since a longer
  word in the acts lane moves the declared floor; (c) both — rename *and* keep the note. The
  report's own lean: (a). The name is the product's one licensed poem, the
  press is harmless, and the note it reveals (*"Last played 3 years ago"*) is
  the explanation in the right voice one press late — acceptable once the
  tooltip closes the gap in front.
- **Refusals/ADR impact**: none; ADR-0026's word-not-icon ruling untouched.

### P10 — Restore persistent library visibility (the full iTunes three-pane) · **rejected-with-reasons**

- **Principle it would serve**: the iTunes lesson (§1.4) — simultaneity,
  spatial permanence, drag targets always on screen.
- **Why this report — the era's own advocate — rejects it**: (1) The owner
  rejected resident side surfaces twice in plain words, and ADR-0022's
  epistemology is correct: a direct observation of *this* user beats a prior
  about users in general, and beats it harder the second time. (2) The era is
  not univocal: against iTunes stands the iPod and the 2007 iPhone — one
  place at a time, doors and `Esc`, the model baz shipped — and Apple itself
  moved its consumer surfaces steadily toward the drill-in as screens
  shrank and libraries grew. (3) The measured column (340 px, hierarchy
  inverted, 3 of 12 tracks) was not the iTunes layout; it was its ghost, and
  restoring the pattern at a width that works would cost the wall the very
  share (73–100 %) that is the product's positioning number. (4) The
  simultaneity that matters has been re-admitted where it earns its keep: the
  playlist panel for collecting, P3's header step for comparison, the bar's
  continuation lane for the queue. That is the era's *substance* — the
  library as ground you never lose — delivered by state preservation and
  ambient facts instead of by permanent panes.
- **What keeps this rejection honest**: if W15-class work (Marta's
  compare-two-releases) grows despite P3 — if the owner finds himself
  round-tripping daily — the evidence base shifts, and `03` §7.2(5)'s
  Lightroom Filmstrip (a *bottom* strip inside the Album place, not a side
  surface) is the re-proposal that meets ADR-0022's argument on its own
  terms. Recorded so the door has a name.

### P11 — A macOS menu bar, when macOS ships · **present-to-owner** (future)

- **Principle**: consistency with platform conventions — the classic HIG's
  first commandment on its own platform. A Mac app without a populated menu
  bar was, to era Apple, simply broken.
- **Existing rationale, steelmanned**: baz targets Linux today; iced's menu
  support is nascent; `03` §6.1 treats in-window menu bars as a dated-product
  marker, and for the in-window kind it is right; the mirror rule already
  provides the enumeration function menus serve.
- **The change, when the macOS target becomes real**: the *system* menu bar
  (not an in-window one) populated as a third mirror layer under doc 09
  §5.2's exact rule — every item sends a message some visible control also
  sends, accelerators printed beside verbs. No new capability, no new
  surface in the window; platform-native discoverability for free.
- **Cost**: deferred until the target exists; recorded now so the decision
  meets an argument instead of a shrug (the ledger's own standard).
- **Refusals/ADR impact**: none today. Would amend `03` §6.1's reading for
  the macOS target only.

### P12 — Rename `Shuffle` · **rejected-with-reasons**

- **The case**: the word promises the industry's mode; baz ships a bounded
  draw ending in silence; a control should not surprise the convention its
  name invokes.
- **Why rejected**: every candidate is worse. `Shuffle 8`, `Draw`, `Mix`
  each trade a universally-understood *first* press for jargon; the surprise
  the word risks is deferred, harmless (silence, never sound unasked), and
  after P6.3 it is *explained at the moment it happens* in the queue's empty
  state. ADR-0026's icon refusal already contains the correct analysis — the
  convention's *symbol* promises a mode and is refused; the *word* is the
  audience's name for "random records now", which is what the press
  delivers. The bounded draw is a REFUSALS identity entry ("no invisible
  shuffle pools", "a thing you start") that this report has already credited
  as an era win; renaming the control to advertise the bound would spend
  clarity's coin on a distinction the empty state teaches for free.

---

## 6. Summary scorecard, and sources

### The examination in one table

| Era principle | baz today | Verdict |
|---|---|---|
| Focus / deciding what not to do | REFUSALS.md, the ratchet, zero-settings features | **exceeds the era** |
| Defaults over configuration | near-absolute | **exceeds the era** |
| Aesthetic integrity | measured, law-bound, tested | **meets it with receipts** |
| One primitive, done perfectly | places + doors + `Esc`; tiles navigate, rows play | **meets it** |
| Modelessness | armed mode withdrawn next day; menus as mirrors | **meets it** |
| Feedback & dialog | reserved slots, honest readouts, teaching empty states | meets it; accelerators untaught (P6) |
| Language | two fails, two marginals, else clean | near miss (P4, P9) |
| Metaphor | vocabulary and physics, never surface | meets it |
| Consistency | one leak ("the wall"), one internal law-contradiction (density) | near miss (P4, P8) |
| See-and-point | index rail, wells, doors — except first run and density | **fails at the front door** (P1, P8) |
| Direct manipulation | steppers where drag belongs; no drag anywhere | **fails** (P5), partial (P7) |
| Forgiveness | no undo anywhere; confirm where trash belongs | **fails outright** (P2) |
| Simultaneity (iTunes lesson) | one panel, ambient facts, unpaid comparison debt | partial by choice (P3 owed; P10 rejected) |

The pattern across the whole examination: **baz has out-Jobsed Jobs on
subtraction, honesty and visual discipline, and under-delivered the era's
mechanics of grace — pointing, dragging, undoing, and the first minute.**
Every proposal above spends the first list's surplus to pay the second
list's debts, and none touches the identity: no autoplay, no telemetry, no
second accent, no resident surfaces, silence at the end.

### Sources

The Apple-side quotations in this document were verified against archived
primary texts (fetched 2026-08-09); the two claims that could not be checked
to the sentence are flagged below rather than silently kept.

**The HIG texts.**
- *Human Interface Guidelines: The Apple Desktop Interface*, Apple /
  Addison-Wesley, 1987 — the ten principles, "See-and-point (instead of
  remember-and-type)", 1987 Forgiveness. Archived:
  <https://archive.org/download/apple-hig/Apple_Human_Interface_Guidelines_1987.pdf>
  (collection: <https://archive.org/details/apple-hig>).
- *Macintosh Human Interface Guidelines*, Apple / Addison-Wesley, 1992 —
  the eleven principles (modelessness promoted to a named principle here),
  Forgiveness ("irretrievable data loss"; "frequent alert boxes…"),
  progressive disclosure verbatim (ch. 3, "Managing Complexity"), Menus
  ("People don't have to remember command names…"), Double-Clicking
  ("Double-clicking must never be the only way to perform a given action").
  PDF: <https://archive.org/download/apple-hig/Macintosh_HIG_1992.pdf>;
  HTML mirror of the principles:
  <https://dev.os9.ca/techpubs/mac/HIGuidelines/HIGuidelines-15.html>
  (Forgiveness -24, Menus -75, Double-Clicking -205).
- *Aqua HIG*, Apple, Oct 2001 — "create safety nets, such as the Undo
  command…" (p. 31):
  <https://archive.org/download/apple-hig/MacOSX_HIG_2001_10_01.pdf>.

**Jobs.**
- WWDC 1997 closing session ("focusing is about saying no"):
  <https://www.youtube.com/watch?v=H8eP99neOVs>; transcription with context:
  <https://donhopkins.medium.com/focusing-is-about-saying-no-steve-jobs-wwdc-97-ff0174c171d0>.
- "There's Sanity Returning", BusinessWeek, May 25 1998 (Andy Reinhardt) —
  "focus and simplicity… Simple can be harder than complex":
  <http://www.bloomberg.com/news/articles/1998-05-25/steve-jobs-theres-sanity-returning>.
- "Deciding what not to do is as important as deciding what to do" — cited
  from Walter Isaacson, *Steve Jobs* (2011), where the attribution is
  first-hand. *(Flag: often attributed to the 2004 BusinessWeek interview
  "The Seed of Apple's Innovation"; that placement was not verifiable to the
  sentence and is not relied on.)*
- Rob Walker, "The Guts of a New Machine", *The New York Times Magazine*,
  Nov 30 2003 — "Design is how it works"; the on/off-switch account:
  <https://www.nytimes.com/2003/11/30/magazine/the-guts-of-a-new-machine.html>.
  *(Flag: the on/off-switch sentence was not retrievable verbatim here;
  the article is the canonical account and should be quoted from directly
  if the exact wording is wanted.)*
- "It just works" is cited as a recurring keynote posture only; no single
  pre-2010 utterance is pinned.

**iPod / iTunes.**
- "Apple Presents iPod", press release, Oct 23 2001 — "1,000 Songs in Your
  Pocket", the scroll wheel's one-handed operation:
  <https://www.apple.com/newsroom/2001/10/23Apple-Presents-iPod/>.
- "Apple Introduces iTunes", press release, Jan 9 2001 — "Apple has done
  what Apple does best — make complex applications easy":
  <https://www.apple.com/newsroom/2001/01/09Apple-Introduces-iTunes-Worlds-Best-and-Easiest-To-Use-Jukebox-Software/>.
- The click wheel's provenance: Steven Levy, *The Perfect Thing* (2006;
  excerpt: <https://www.wired.com/2006/11/ipod/>); US Patent 7,710,394
  (Robbin, Jobs, Schiller): <https://patents.google.com/patent/US7710394B2>;
  Leander Kahney, "Inside Look at Birth of the iPod", Wired, July 2004:
  <https://www.wired.com/2004/07/the-birth-of-the-ipod/>.
- iTunes 1–4 layout evidence: Version Museum's screenshot history,
  <https://www.versionmuseum.com/history-of/itunes-app>; participant
  retrospective: Cabel Sasser, "The True Story of Audion" (Panic),
  <https://panic.com/extras/audionstory/>.

**This repo.** `docs/REFUSALS.md`; ADR-0014, -0016, -0017, -0020, -0021,
-0022 (places), -0023, -0024, -0025, -0026; `docs/design/01–10`;
`.interface-design/system.md`; captures in `docs/design/impl/` as cited
inline; fresh renders and the isolation receipt in §0.
