# 10 — Controls and iconography: what each control wears, and where it stands

> The owner, verbatim, after the implicit-playlists epic landed:
>
> *"just adding stuff into that top bar isn't good. I just find we need to
> have a proper think about how we lay out controls and what is intuitive.
> we are also using quite poor HCI due to almost no icons etc."*
>
> Both halves are right, and they are the same finding at two scales. The
> strip is crowded because every tenant was admitted by a *placement*
> argument (doc 07's L8 decides which surface a control lives on) and no law
> has ever governed a surface's **population** or a control's **form** — so
> ten controls stand in one line, all of them words, all at one size, and
> the line is 97 % spoken for at the shipped window. And the words are
> word-only because `02-visual-language.md` §1.3 rejected *decorative* icon
> conventions (play buttons hovering over art, colour-washed chrome) and the
> product read that as a posture against icons generally — which it never
> was: the transport, the speaker and the ✕ are already drawn glyphs
> (`crates/baz/src/icon.rs`), and nobody has ever needed a tooltip to find
> them.
>
> A design study, not an implementation. Written 2026-08-09 against
> `97c288a` (the merge that shipped doc 09's context menus — every step of
> the implicit-playlists plan but the drag). Every claim about the shipped
> UI is cited `file:line` or to a frame in `docs/design/impl/`; every
> prior-art claim carries `03-interface-prior-art.md`'s section or a named
> source. Its decisions are proposed as
> [ADR-0026](../adr/0026-iconography-and-the-strip-budget.md).
>
> The short version: **one rule decides every control's form** (icon-only
> where the convention is universal *and* the semantics match; word-only
> where baz-specific; the word carries the glyph where the convention is
> close but not exact), **the strip gets a charter and a budget law with a
> test**, the counts fold into the search well, `Settings` becomes the
> gear, the well gains the magnifier and loses 80 px, and the strip learns
> to split into two lines below 960 px instead of overflowing — which it
> does today, silently, whenever a scan is running.

---

## 0. What decides this

### 0.1 The constraint set already in force

This study does not get to invent freely. Eight composition laws
(`.interface-design/system.md` §13, plus doc 07's L8), the refusals ledger,
and three shipped postures bound every answer:

- **The visible-control rule** (`docs/the product's standing rules:147–149`): every action
  has a visible, pointer-reachable control; nothing is keyboard-only; no
  control's only affordance is hover. Hiding controls behind hover is not
  on the table — it is refused, not merely rejected here.
- **No view-options menus** (the product's standing rule) and doc 07 §0.1's
  corollary: *there is no overflow menu to sweep a control into, so every
  control must have a real home.* A hamburger, a "⋯", or a toolbar
  preferences panel cannot be this document's answer to crowding.
- **The bar's ratchet** (the product's standing rule): a slot may be added to the
  now-playing bar; none may be removed for tidiness. The bottom bar is
  therefore examined and — this is a finding, not an omission — left
  alone (§4.4).
- **Doors are labelled** (doc 07 L8.4): a control that only navigates is
  labelled with the name of what it opens, in words, on the Sonos and
  Plexamp evidence (`03` §4.5, §5.2(e)). §3.4 amends this clause by one
  named exception list rather than weakening it.
- **Tooltips are the accessible name** (ADR-0017 §4(c)): iced 0.13
  publishes no accessibility tree, so every icon-only control carries a
  tooltip and a hit box at or above the floor. The precedent is shipped:
  `views/bottom_bar.rs:721–730` names every transport glyph this way.
- **L7, one control height**: every pointer target is `TRANSPORT_HIT` 32
  or the named `STEPPER_HIT` 24 / `NEEDLE_HIT` 12 (`theme.rs:1389, 1663,
  1243`). New icon controls change nothing here; they inherit the floor.
- **Contrast is computed, not estimated** (ADR-0017 §1.6): an icon's ink
  is composited before it is measured, exactly as text ink is.

### 0.2 What this study governs, and what it does not

It governs **form** (word, glyph, or both, per control), **population**
(what the Library strip may hold, and the law that bounds it), and
**arrangement under pressure** (what happens as the window narrows).

It does not reopen placement: doc 07's L8 assigned every control a home by
subject, every assignment below is checked against it, and none moves to a
different surface. The one control that changes *rooms within its surface*
is the counts readout (§4.2), and it moves under L8.3's own valve — a fact
restated where it is watched — discharging a move doc 07 §3.1 already
prescribed.

### 0.3 The three vocabularies, named

The strip's module docs claim *"two vocabularies for two kinds of thing,
and no third"* — sentence-case Medium words for **acts**, caps-and-tracked
for **states** (`views/top_bar.rs:170–174`). That sentence was written
about type, and it holds for type. This study adds the third vocabulary
the product already half-has, and gives it the rule the first two have:

| Vocabulary | Form | Means | Examples today |
|---|---|---|---|
| **The word** | sentence case, Medium | an act or a door named in baz's own language | `Play all`, `Shuffle`, `Pull`, `Playlists`, `Queue`, `Save as playlist`, `‹ Library` |
| **The state row** | caps, tracked, `SIZE_META`/`SIZE_HEADING` | a set of values, one current | `ARTIST YEAR GENRE ADDED PLAYED`, shelf headers, the index rail |
| **The glyph** | drawn polygon, `ICON_PX` 16 in a fixed box | a conventional act whose symbol everyone already owns | ▶ ⏸ ⏮ ⏭ 🔈 ✕ (`icon.rs:97–112`) |

The glyph vocabulary exists (seven members, rasterized in
`icon.rs`), is inked on one opacity ladder (`theme.rs:1404–1459`), and is
named by tooltips. What it lacks is a membership rule — which is why the
product also carries a **fourth, accidental vocabulary**: font characters
standing in for glyphs. The queue row's reorder marks are U+2191/U+2193 at
`SIZE_BODY` (`views/queue.rs:522–550`), the transfer mark is a text `+`
(`views/album.rs:701–733`), the settings steppers are `−`/`+` — all
borrowed from IBM Plex Sans because *"the face carries no triangles"*
(`queue.rs:519–521`), all sitting in the same rows as the drawn ✕
(`queue.rs:599–633`) at a visibly different stroke weight. Two mark
technologies in one row of slots is the accidental part; §3.6 ends it.

---

## 1. The inventory — every control, its form, and its mirrors

Bands are `03` §1.2's (A constant · B frequent · C occasional · D rare ·
E very rare). *Form* is what the control wears today. *Eye travel* is
where the eye must go from the resting wall to find it. Accelerators list
the keyboard (`keys.rs:372–456`) and context-menu (`menu.rs:159–281`)
mirror layers — the same decision made twice more, per L8.7 and its menu
amendment.

### 1.1 The Library strip (`views/top_bar.rs`)

| Control | Form today | Message | Band | Eye travel | Accelerators |
|---|---|---|---|---|---|
| Search well | recessed box, placeholder text, 360 px (`top_bar.rs:35,52`) | `SearchChanged` / `PlayFirstMatch` | B | top-left | any bare printable; `/`, `Ctrl+F`; `Enter` |
| Group keys ×5 | caps-tracked words (`top_bar.rs:343`) | `GroupKeySelected` | C | top, left-centre | `1`–`5` |
| `Play all` | word, Medium (`top_bar.rs:175–184`) | `PlayAll` | C | top-centre | — |
| `Shuffle` | word | `Shuffle` | C | top-centre | — |
| `Pull` | word | `Pull` | C/D | top-centre | `Ctrl+R` |
| `Playlists` door | word (`top_bar.rs:222–240`) | `TogglePlaylists` | C | top-centre-right | `Ctrl+P` |
| Counts | readout, `paper_faint` (`top_bar.rs:300–310`) | — | — | top-right | — |
| `scanning…` / skipped / problem | readouts (`top_bar.rs:75–101`) | — | — | top-right | — |
| `Settings` door | word, 84 px reserved (`top_bar.rs:259–289`, `theme.rs:2877`) | `ToggleSettings` | E | top-right corner | `Ctrl+,` |

### 1.2 The wall and its lanes (`views/shelf.rs`)

| Control | Form | Message | Band | Eye travel | Accelerators |
|---|---|---|---|---|---|
| Album tile | object (art + label) | `AlbumClicked` | A | the wall | tile menu: `Open` (`menu.rs:210–226`) |
| Tile, shift-click | modifier gesture | `AddAlbumToPlaylist`+`PickQueue` | C | — | menu `Queue album` (its on-screen twin) |
| Index rail entry | type, magnified under the pointer (`shelf.rs:588–620`) | `RailJumped` | A | wall's right edge | — |
| Songs row (under a query) | list row (`shelf.rs:242–345`) | `PlayTrack` | B | above the wall | `Enter`; track menu |
| Songs row's record door | word (`shelf.rs:352–370`) | `AlbumClicked` | B | in the row | — |
| Songs row `+` | text `+` in reserved slot | `AddTrackToPlaylist` | C | row's outer edge | menu `Add to…` items |

### 1.3 The record's page (`views/album.rs`)

| Control | Form | Message | Band | Accelerators |
|---|---|---|---|---|
| `‹ Library` | word (`views/mod.rs:236–258`) | `LeavePlace` | B | `Esc` |
| `Play album` | **glyph + word**, lamp outline (`album.rs:314–344`) | `PlayAlbum` | A | `Enter` at selection; tile menu |
| `Add to…` | word (`album.rs:353–374`) | `AddAlbumToPlaylist` | C | tile menu |
| Edition selector | segmented words (`album.rs:533–565`) | `EditionSelected` | D | — |
| Track row | object (`album.rs:594–692`) | `PlayTrack` | B | track menu `Play` |
| Track row `+` | text `+`, hover/panel-open (`album.rs:701–733`) | `AddTrackToPlaylist` | C | track menu |

### 1.4 The queue place (`views/queue.rs`)

| Control | Form | Message | Band | Accelerators |
|---|---|---|---|---|
| `‹ Library` | word | `LeavePlace` | — | `Esc` |
| `Save as playlist` | word → name field (`queue.rs:235–284`) | `SaveQueueStart…` | D | — |
| Queue row | object | `JumpToQueued` | C | row menu `Play` |
| Row ▲▼ | **font arrows** U+2191/U+2193 (`queue.rs:522–550`) | `ShiftQueued` | C | — |
| Row ✕ | **drawn glyph** Close (`queue.rs:599–633`) | `RemoveQueued` | C | row menu `Remove` |
| Row `+` | text `+` (`queue.rs:555–587`) | `AddQueuedToPlaylist` | C | row menu items |

### 1.5 The playlist page and panel (`views/playlist.rs`, `views/playlist_panel.rs`)

| Control | Form | Message | Band |
|---|---|---|---|
| `Play` | glyph + word, lamp outline (`playlist.rs:210–238`) | `PlaylistPlay` | B |
| `Queue` / `Rename` / `Delete` | words (`playlist.rs:243–266`) | … | C/D |
| Row ✕ ▲▼ `+` | as the queue's rows | … | C |
| Panel row | door / pick target (`playlist_panel.rs:299–370`) | `OpenPlaylist` / `PickPlaylist` | C |
| `New playlist` | word → field (`playlist_panel.rs:241–259`) | `NewPlaylistStart` | D |
| Queue row in panel | **readout** at rest; pick target while picking (`playlist_panel.rs:185–237`) | `PickQueue` | C |

### 1.6 The bar (`views/bottom_bar.rs`)

| Control | Form | Message | Band | Accelerators |
|---|---|---|---|---|
| Now-playing block | type, a door (`bottom_bar.rs:407–441`) | `ShowPlayingAlbum` | A | bar menu `Go to record` |
| `Queue · N` door | word + reserved figure (`bottom_bar.rs:339–380`) | `ToggleQueue` | C | `Ctrl+U` |
| Previous · Play/Pause · Next | **drawn glyphs**, tooltipped (`bottom_bar.rs:616–735`) | … | A | `Space`, `Ctrl+←/→`, media keys |
| Needle | object, 2 px + 12 px band | `Needle…` | B | `←/→`, `Shift+←/→` |
| Speaker / mute | **drawn glyph** (`bottom_bar.rs:754–820`) | `ToggleMute` | C | `Ctrl+M` |
| Volume fader | groove + detent | `Volume…` | B | `↑/↓` |
| Signal note, stamps, continuation | readouts — *no icon, no background, never the accent* (`02` §6.7) | — | — | — |

### 1.7 Settings and setup (`views/settings.rs`, `views/setup.rs`)

Back word, two section words, ReplayGain segments, two `−`/`+` stepper
pairs (font glyphs, `STEPPER_HIT` 24), one checkbox, the roots list with
its add-folder field and per-folder acts, the first-run field and submit.
All band E, all words, all confirmed in place — a place you navigated to
needs no recognition aids; you are already where its subject is.

### 1.8 What the inventory shows

1. **The product is not icon-less — it is icon-inconsistent.** Seven drawn
   glyphs exist and carry the highest-frequency acts in the product (the
   transport). But the *conventionally iconic* controls a first-time user
   scans for — search and settings — are a borderless box and a word, and
   the row-edit marks are split between two technologies (drawn ✕ beside
   font ▲▼ and `+` in the same slot row, `queue.rs:480–495`). The owner's
   "almost no icons" is precisely the strip's and the rows' condition, not
   the bar's.
2. **The strip holds ten controls and three readouts; every other surface
   holds two to five.** The strip is the only surface whose population has
   grown release over release (Queue left; keys, three acts, a door and a
   count arrived), because L8 admits by subject and nothing bounds the
   total. §2 measures the consequence.
3. **Every act already has its keyboard and menu mirrors** where doc 09
   §5.2 specified them — the accelerator layers are healthy; the visible
   layer is where the work is.
4. **Word-only is doing real work in exactly the places the rule of §3
   keeps it**: `Pull` (no convention on earth), the group keys (data, not
   acts), the doors (`Queue`, `Playlists`, `‹ Library` — the Sonos
   lesson), the segments and the word-acts on pages.

---

## 2. The strip, measured honestly

### 2.1 The crowding, in pixels

Measured off the render capture
[`impl/queue-parity/01-strip-play-all-1280x860.png`](impl/queue-parity/01-strip-play-all-1280x860.png)
(1280 × 860, the shipped default):

```
x:  40        400  424          734  758        912  936   1001   1032     1148  1156    1240
    [ search well ] [ARTIST…PLAYED]  [Play all…Pull]  [Playlists]   [counts]    [Settings]
    ←—— 360 ——→     ←—— 310 ——→     ←—— 154 ——→     ←— 65 —→  ↕31   ←— 116 —→   ←— 84 —→
```

- The strip's one flexible region — the `Fill` between the left cluster
  and the status row (`top_bar.rs:113`) — measures **≈ 31 px of 1280**.
  Content and gutters claim ≈ 1249 px: **97.6 % of the strip is spoken
  for at the default window.**
- **The strip cannot respond to width even in principle**: its view takes
  no window size at all — `pub(crate) fn view(shelf: &Shelf)`
  (`top_bar.rs:39`) — and unlike the bar's zones
  (`bottom_bar.rs:101–110`), nothing in it clips. Below ≈ 1250 px the row
  overflows: iced lays the surplus past the window's edge.
- **It already overflows at 1280, today, during a scan.** The status row
  gains `scanning…` (≈ 55 px) and `N files skipped` (≈ 85 px) as extra
  segments (`top_bar.rs:75–93`); 140 px of transient status against 31 px
  of slack pushes `Settings` — the only route to the Settings place —
  off the right edge of the shipped window. Derived from the measured
  frame and the code, not captured; flagged as the first thing the fix
  must make impossible.

### 2.2 The accretion pattern, named

Each tenant is individually well-argued, and the arguments are all
*placement* arguments:

- the keys read the collection → the strip (L8.1, `top_bar.rs:17–24`);
- `Play all` / `Shuffle` / `Pull` read the wall → the strip (L8.1,
  `top_bar.rs:138–152`);
- `Playlists` is a door placed where the hand is (L8.4,
  `top_bar.rs:209–216`);
- `Settings` is a door at the corner where an application's affairs
  belong (`top_bar.rs:244–248`);
- the counts are a fact that changes unasked → resident (L8.3).

Every sentence is true. **Together they are not a layout** — they are six
admission tickets and no fire code. L8.2 bounds *residency by band*
("one word in a cluster that already exists") but a cluster may
apparently grow without limit one word at a time, and L8 has no clause
that ever says *the surface is full*. That is the missing law, and it is
the general form of the owner's complaint exactly as L8 was the general
form of the 2026-08-08 one.

### 2.3 The charter

> **The Library strip holds the library's verbs and states — the controls
> that read the wall to know what to do — plus at most the doors the hand
> expects at a frame's corners. Its population is bounded by arithmetic:
> the sum of every tenant's reserved width and the frame's gutters must
> fit the strip's declared single-line floor, and the sum is asserted.
> What cannot fit does not enter — it re-homes by subject, or the strip
> splits at its declared seam. A strip never hides a tenant and never
> overflows.**

Every current tenant, re-examined against it:

| Tenant | Verdict | Why |
|---|---|---|
| Search well | **confirmed**, narrowed 360 → 200–280 fluid (§4.1) | Reads the collection; band B. Its *width* was sized in the era when you aimed at it; under type-anywhere (ADR-0017 §1.2) you reach it by typing, and 280 px holds a 40-character query |
| Group keys ×5 | **confirmed** as words | States, not acts; the words are the data (arrangement names); a compressed or iconic form would be the view-options dropdown's ghost, refused by name (the product's standing rule, `top_bar.rs:326–332`) |
| `Play all` | **confirmed**, gains the play triangle (§3.5) | The strip's one press that makes sound from rest; the glyph+word anatomy is `Play album`'s own, not a new pattern |
| `Shuffle`, `Pull` | **confirmed** as words | §3.5's rule: the shuffle convention's symbol promises a mode baz refuses to have; the pull has no convention at all |
| `Playlists` door | **confirmed** as a word | L8.4; no universal symbol distinguishes *playlists* from *queue* from *menu* — three list-shaped glyphs in one product would be the ambiguity the law exists to prevent |
| Counts | **re-homed into the well** (§4.2) | L8.3's valve run in reverse: the fact moves to where it is watched. Discharges doc 07 §3.1's prescribed match-count move and frees the widest readout in the strip |
| Scan / skipped / problem notes | **confirmed**, right side | Facts that change unasked; transient; they inherit the freed slack |
| `Settings` door | **confirmed as tenant; its form becomes the gear** (§3.4) | The one door whose symbol is a genuine universal; 84 px → 32 px |

---

## 3. The iconography system

### 3.1 The rule

> **A control is drawn as an icon alone only when all three hold:**
>
> 1. **the symbol is universal** — standardized across the mainstream
>    players and operating systems this audience arrives from, such that a
>    first-time user reads it without a legend (the magnifier, the gear,
>    the play/pause/skip family, the speaker, the ✕, the +, the
>    directional arrows — and almost nothing else);
> 2. **baz's semantics are the convention's semantics, exactly** — a
>    symbol whose convention promises a mode may not label an act, and a
>    symbol whose convention promises one scope may not act on another;
> 3. **the control's meaning is stable in every state it can be in** — a
>    glyph that would need a second glyph to explain its current state
>    goes back to words.
>
> An icon-only control carries a tooltip that names it (the accessible
> name, ADR-0017 §4c) and a hit box at the floor (L7). **Where the
> convention is close but not exact, the word stays and may carry the
> glyph as its leading mark** — recognition from the symbol, semantics
> from the word. **Where no convention exists, the control is a word**: an
> invented icon is a private code that every user must be taught, and a
> word never needs a tooltip.

The rule's second clause is what the tradition gets wrong and baz can get
right. Recognition beats recall only when the symbol *means the same
thing* here as everywhere else; a familiar glyph over unfamiliar
semantics is worse than an unfamiliar word, because the user does not
know they need to learn it.

### 3.2 The rule applied, control by control

| Control | Convention | Semantics match? | Verdict |
|---|---|---|---|
| Previous / Play / Pause / Next | universal (every player since the cassette deck) | exact | **icon-only** — shipped (`icon.rs:97–112`) |
| Speaker / mute | universal | exact | **icon-only** — shipped |
| Remove ✕ | universal | exact | **icon-only** — shipped |
| Search | the magnifier: universal (ISO/IEC 11581-lineage; every browser, OS and player) | exact — the well filters what it names | **glyph marks the well** (§4.1). Not icon-only: the well itself stays, per ADR-0017 §1.2's defence of the field |
| Settings | the gear: universal, and top-right is its universal *position* | exact — a door to preferences | **icon-only, with tooltip** — the one door L8.4 licenses to a symbol (§3.4) |
| Add-to `+` | universal | exact — "put this somewhere" | **icon-only** — shipped as a font character; promoted to a drawn glyph (§3.6) |
| Reorder ▲▼ | universal directional | exact | **icon-only** — promoted to drawn glyphs (§3.6) |
| `Play all` | the triangle: universal for "press = sound now" | exact for the act, but the *scope* (the wall, as arranged) is baz's own | **icon + word** — the triangle says a press sounds; the words say what (§3.5) |
| `Shuffle` | crossed arrows: universal — **as a mode toggle** with a lit state | ~~**no** — baz's shuffle is a bounded draw of 8 from a visible pool, an act that ends; wearing the mode glyph claims a mode the product refuses~~ · **yes, from 2026-08-10**: the owner made shuffle a property of the player, so the glyph's promise is the control's meaning (§3.2) | **the glyph, lit in the accent**, on the now-playing bar |
| `Pull` | none exists | — | **word-only** — the case the brief names: an invented icon is worse than a word |
| Group keys | none (arrangement *names* are data) | — | **words** — caps-tracked states, unchanged |
| `Queue` door, `Playlists` door, `‹ Library`, `Back` | no symbol distinguishes these three list-shaped destinations | — | **words** — L8.4 holds; the Sonos/Plexamp evidence (`03` §4.5, §5.2e) is about exactly this class |
| `Save as playlist`, `Rename`, `Delete`, `Add to…`, `New playlist` | weak or colliding conventions (floppy disks, pencils, trash cans — a 1995 vocabulary this room refuses) | — | **words** — baz-specific acts in baz's own language |
| Readouts (signal note, counts, continuation, stamps) | — | — | **never an icon** — `02` §6.7's rule survives verbatim: no icon, no background, never the accent |

### 3.3 Why the gallery survives this

`02` §1.3's three rejected defaults — play buttons hovering over art,
colour-washed chrome, a "data" face — are all *decorative* conventions.
The glyphs admitted here are **functional** conventions, and the room's
own discipline already contains them: theme-inked, one opacity ladder
(`theme.rs:1404–1459`), fixed boxes, no colour of their own, never the
accent. A gear at `paper` @ 0.57 in a 32 px box is quieter than the word
`Settings` it replaces — it is *less* ink, not more chrome — and the wall
label / picture-light signature is untouched, because not one glyph lands
on a sleeve or in a readout.

### 3.4 The named exception to L8.4, argued

L8.4(1): *a door is labelled with the name of the place, in words.* The
evidence behind it — Sonos hiding system controls behind a swipe-up,
Plexamp hiding the queue behind an unlabelled wheel gesture — is about
**unlabelled and unconventional** routes. A gear in the top-right corner
is neither: it is the single most standardized door in interactive
software, in both symbol and position, and its tooltip carries the word
for the hover.

> **Proposed amendment to L8.4(1)**: *a door is labelled with the name of
> what it opens — in words, or by its universal symbol where one exists.
> The symbols that count as labels are enumerated, and the list is two:
> the gear (Settings) and the magnifier (search). A door whose symbol is
> merely familiar rather than universal keeps its word.*

The list is closed the way the contrast exemption list is closed
(ADR-0017 §1.6): adding a name means arguing it here. `Queue` and
`Playlists` were considered for it and refused — a queue glyph, a
playlist glyph and a menu glyph are one triangle-and-lines drawing
apart, and a door you can misread is worse than a door you must read.

### 3.5 The two strip promotions

**`Play all` gains the triangle.** The anatomy already exists twice:
`Play album` (`album.rs:314–344`) and the playlist page's `Play`
(`playlist.rs:210–238`) are both glyph + word. `Play all` is the same
act at wall scope and dresses the same — with one deliberate difference:
**no lamp**. The accent belongs to `Play album`/`Play` as the pages' one
commitment (`02` §5.3); in the strip the triangle takes the ordinary
glyph ink. The triangle also does the strip a compositional favour: it
is the one non-type mark in the left cluster, and it anchors the seam
where states (caps words) end and acts (sentence words) begin.

**~~`Shuffle` stays a word~~ — `Shuffle` takes the crossed arrows
(rewritten 2026-08-10).** The clause read: *the crossed-arrows
convention is a mode with a lit state — press it and every subsequent
play is shuffled until you press it again. baz's shuffle is an act: one
press, eight records, an end, silence. Convention's symbol over baz's
semantics would be a promise the product is built to break.*

That argument was conditional, and it named its own condition exactly.
**The owner made shuffle a mode** — *"can you make shuffle a property of
the player i.e. toggle on/off"* — so the sentence the symbol promises is
now the sentence the control means: press it and every subsequent play
is shuffled until you press it again, and it carries the lit state to
say which way it stands. Rule 2 of §3.1 admits a convention exactly
where baz's semantics *are* the convention's, and this is that case
arriving.

The older half of the argument — *a die is a recommendation engine's
costume* — still stands and is not what changed: crossed arrows are not
a die. A die says *chance*, which is a promise about how a machine
chose; crossed arrows say *these swap places*, which is a statement
about order, and order is all baz's shuffle touches.

Where the control lives changed with it. It was the strip's second act,
beside `Play all`; a property of the player belongs on the player's
surface, so it is a slot on the now-playing bar — an **addition**, with
nothing traded away for it and the transport unmoved, which is the bar's
standing concern (`03` R11: three vendors bought "visual calm" by
removing control density and all three reversed). Lit is the
**accent** — the one place beside `Play album` the accent-discipline
note admits, because this control creates playback truth about what
sounds *next* in the way `Play album` creates it about what sounds now.
`ACTS_W` fell 144 → 88 with the word, and the strip's split seam
872 → 778.

### 3.6 One mark technology, and the vocabulary extension

The glyph set grows from seven to **twelve**, all in `icon.rs`'s existing
form — closed polygons in a unit square, supersampled once, theme-inked,
no new dependency (the module's own survey of the four routes,
`icon.rs:12–50`, still holds):

| New glyph | Replaces | Where |
|---|---|---|
| `Magnifier` | the deleted resting ring as the well's "a control is here" statement (doc 07 §5.1) | the search well (§4.1) |
| `Gear` | the word `Settings` (84 px → 32 px) | the strip's right corner |
| `Plus` | the font `+` | every transfer slot (`album.rs:701`, `queue.rs:555`, songs rows), and the settings steppers' `+` |
| `ArrowUp` / `ArrowDown` | U+2191 / U+2193 | the reorder slots (`queue.rs:522`, `playlist.rs`) |

And one correction to the accidental fourth vocabulary: **a control slot
carries a drawn glyph or a word, never a borrowed character** — the
borrowed-character rows (`queue.rs:480–495`: drawn ✕ beside font ▲▼ and
`+` in one slot row) become uniform in stroke and ink. Font characters
remain legitimate *inside labels* (`‹ Library`'s chevron is part of a
word; U+2212 in a value is a figure), because a label is type and type
belongs to the face.

Implementation note, stated so the plan is honest: the magnifier and the
gear are **rings**, and `Glyph::covers` takes the union of outlines
(`icon.rs:267–276`) — a hole would cancel under union. No new rasterizer
is needed: a ring is one *keyhole outline* (trace the outer ring, bridge
to the inner, trace it counter-clockwise, bridge back), which the
existing even-odd test (`icon.rs:372–395`) fills correctly. The gear is
the keyhole ring with teeth on the outer trace; eight teeth read cleanly
at 16 px. Each new glyph lands with the coverage-shape tests the seven
shipped glyphs have (`icon.rs:407–698`).

### 3.7 The laws, applied to icons

- **Hit targets**: the gear is a `TRANSPORT_HIT` 32 box; every row slot
  stays `STEPPER_HIT` 24; the magnifier is not a control (the well is
  the target; the glyph is its label) and needs no box.
- **Stroke weight on the lattice**: the shipped glyphs draw their bars at
  0.14–0.15 of the unit square — 2.24–2.4 px at 16, ≈ 4.5–4.8 px in the
  32 px raster. New glyphs use the same band: the magnifier's ring and
  handle, the gear's ring, the arrows' strokes and the plus's bars all at
  0.14–0.15. One weight is what makes twelve marks one set; the test per
  glyph asserts its solid runs the way `pause_is_two_bars…` does.
- **Ink**: the shipped ladder, unchanged — `GLYPH_OPACITY` 0.57 at rest
  through 1.0 hovered (`theme.rs:1404–1459`), the transport's own tween
  (ADR-0020 §2). The contrast test's composited-ink rule (ADR-0017
  §1.6(2)) already governs: resting glyph ink composited over its
  surface clears the 3 : 1 mark floor on every surface a glyph lands on,
  and the sweep extends to the five new glyphs by extending nothing —
  they are the same ink.
- **Tooltips**: `every_icon_only_control_carries_a_tooltip` becomes a
  test rather than an audit (§7 step 7), in the source-pinned shape
  `queue.rs:647–705` already uses.

---

## 4. The layouts, drawn

> **Superseded in part, 2026-08-09 — the well left this strip.** The owner:
> *"the design does not match properly… the search should really be in the
> sidebar."* §4.1's anatomy and §4.2's and §4.3's budgets are kept below as
> written, because they are still exactly what the strip draws **below
> `SIDEBAR_FLOOR` 1000**, where the returns lane is a rail that cannot hold a
> field. At every wider window the well is the lane's, in the two-line form
> ADR-0030's second amendment records, and this strip's left cluster begins at
> the group keys. What changes here in arithmetic:
>
> - `TOP_BAR_SPLIT` is **872**, not 960 — §4.2's sum less the `Playlists`
>   door's 88 px, and exact rather than rounded up.
> - The well's width is a flat **200**. §4.1's `clamp(W − 1000, 200, 280)` ramp
>   is deleted as unreachable: only a strip 1200 px or wider could climb it, and
>   a strip that draws the well is at most `SIDEBAR_FLOOR − SIDEBAR_RAIL_W` =
>   904. **The split is now the whole of the collapse order**, not its second
>   step.
> - The widths below are read against the **strip's** width — the window less
>   the lane — never the window's.
> - Where the well is the lane's, the strip's tenants sum to **648** against a
>   narrowest possible strip of 720, so the split cannot fire there at all.
>
> Frames and the full re-derivation:
> [`docs/design/impl/search-in-lane/`](impl/search-in-lane/README.md).

All numbers logical px on the 4-lattice; gutters are `HANG` 40 (law L1);
control height `TRANSPORT_HIT` 32 throughout (L7). Reserved widths:
well 200–280 fluid · keys 312 · acts 182 (triangle + three words) ·
`Playlists` 64 · gear 32 · seam gaps `GAP_XL` 24 · status lead `GAP_LG`
16.

### 4.1 The well, re-anatomized

```
┌────────────────────────────────────────────┐
│ ⌕  1 284 albums · 9 902 tracks             │   at rest: the magnifier, and the
└────────────────────────────────────────────┘   counts as the placeholder
┌────────────────────────────────────────────┐
│ ⌕  low                          7 / 1 284  │   filtering: the query, and the
└────────────────────────────────────────────┘   match count in a reserved slot
   16 + GAP_SM                    right-aligned, `paper_faint`
```

- The magnifier sits in the well's left padding (a `stack` over the
  input, the mechanism the bar's tip layers already use,
  `bottom_bar.rs:120–144`; iced 0.13's `text_input::Icon` is font-based
  and therefore not it). It is the well's label, not a control; the well
  remains the only focusable widget (ADR-0017 §1.2).
- **The counts become the placeholder.** The placeholder lane is by
  definition empty exactly when the query is empty — the one lane in the
  product that is free whenever the counts have something to say. The
  fact lands where it is consulted: the corpus size, behind the glyph
  that says "search this". During a scan the placeholder ticks up, which
  is the shelf-filling progress rule (the product's standing rule) restated in
  figures.
- **The match count gets the in-well slot doc 07 §3.1 prescribed** —
  `7 / 1 284`, right-aligned, reserved width so arriving moves nothing.
  Today that number sits ≈ 1 100 px from the keys producing it; now it
  is inside the control being typed into.
- Width: `WELL_W = clamp(W − 1000, 200, 280)` — 280 at ≥ 1280, floor
  200. The resting border stays deleted (doc 07 §5.1); ring on focus
  only.

### 4.2 The strip at 1280, 1440, 1920

```
1280 (single-line regime, ≥ 960):
40 [⌕ well 280] 24 [ARTIST YEAR GENRE ADDED PLAYED] 24 [▶ Play all · Shuffle · Pull] 24 [Playlists] ··fill·· [scan notes][⚙ 32] 40
   40–320          344–656 (312)                       680–862 (182)                    886–950     ≈ 242 px            1208–1240

1440: identical; the fill grows 242 → 402. The keys, acts and doors do not move
      relative to the well — the left cluster is fixed-width, so the eye's
      landmarks survive a resize.

1920: identical; fill ≈ 882. Slack is air, not new tenants: the budget law
      admits nothing merely because room exists.
```

Sum at the floor (well 200): 40+200+24+312+24+182+24+64+16+32+40 =
**958**. The single-line regime therefore holds to **960**, asserted
(§7 step 7) in the shape the bar already uses for its own arithmetic
(`bottom_bar.rs:874–885`).

What the redesign frees at 1280: the counts' ≈ 120 px + `Settings`'
52 px + the well's 80 px − the triangle's 26 px ≈ **225 px of new
slack** —
the transient scan notes now fit at every width down to the regime floor
with the gear still on screen, which repairs §2.1's overflow.

### 4.3 The strip below 960 — the split, not a menu

```
< 960 (two-line regime, floor 600):

40 [⌕ well 200–280]            ··fill··            [Playlists] 16 [⚙] 40     ← the frame line
40 [ARTIST YEAR GENRE ADDED PLAYED] 24 [▶ Play all · Shuffle · Pull] ··fill·· 40   ← the library line
────────────────────────────────────────────────────────────────────── hairline

line 1 min: 40+200+16+64+16+32+40 = 408
line 2 min: 40+312+24+182+40      = 598  → the regime floor is 600
height: 8+32+8+32+8+1 = 89 (TOP_BAR_2LINE_H), against 49 single-line
```

- **Nothing hides, nothing overflows, no menu appears.** Every control
  keeps its exact form; what changes is one seam: the frame's furniture
  (search, doors) stays on the window line, and the library's verbs and
  states take a line of their own. The split *is* the strip's charter
  drawn: frame line, library line.
- The collapse order is therefore one step, not a cascade — first the
  well spends its fluid 80 px (1040 → 960), then the strip splits. A
  cascade of partial hidings was considered and rejected: each hidden
  tenant would need an overflow home, and there is none to give
  (doc 07 §0.1).
- Below 600 nothing further collapses; 600 is declared as the strip's
  floor and the window's sensible minimum (the hang lays 2 columns at a
  640 window; the bar's own floor test is < 760, `bottom_bar.rs:884`).
- Cost, stated: 40 px of wall height at narrow windows only. The
  virtualizer's estimate reads the strip's height token, so the regime
  is a pair of tokens (`TOP_BAR_H` 49 / `TOP_BAR_2LINE_H` 89) and a
  breakpoint the app resolves — `top_bar::view` gains the `window_width`
  parameter it has never had (`top_bar.rs:39`).

Prior art for the split rather than the sweep: the modern cluster's
fourth trait is *a layout that reflows at narrow widths with a real
breakpoint* (`03` §6.1(4) — Amberol's overlay split, Feishin's restack,
Elisa down to Plasma Mobile), and the three vendors who solved narrowness
by *removing* controls all reversed within two years (`03` §4.4.6, R11).

### 4.4 The bar — examined and deliberately untouched

The bar passes the form rule as shipped: its acts are the universal
glyph family (icon-only, tooltipped), its doors are words (L8.4), its
readouts are bare type (`02` §6.7), its geometry is reserved-slot and
ratcheted (the product's standing rule). The one candidate change — an icon for
the `Queue` door — fails §3.1(1) (no universal queue symbol distinct
from playlist/menu) and would break the door law for nothing: the word
plus its count *is* the readout-and-door compound the study's own R1
demanded (`03` §8 R1; `bottom_bar.rs:311–338`). Stability here is the
returning user's half of the intuition test: the transport a listener
uses forty times a session does not move a pixel under this study.

### 4.5 The page headers — one strip, four places

`place_header` (`views/mod.rs:236–281`) already gives Album, Queue and
Playlist one frame: `‹ Library` · the place's name · a quiet note, in
the top bar's exact geometry. Confirmed unchanged, including *Back is a
word, not a chevron-glyph* (`views/mod.rs:230–235`) — a door, named.
The Settings place's own header (`settings.rs:260–282`) draws the same
shape with its own function; step 8 folds it into `place_header` so the
frame is one function in five places rather than two that can drift.
The page-level acts keep their forms: the lamp-outlined glyph+word
`Play`/`Play album` as each page's one commitment, word acts beside it,
uniform drawn glyphs in the row slots (§3.6).

---

## 5. The intuition test

Doc 09 §10's answer sheet, extended with the four questions this study
was commissioned on. *Today* and *proposed* name where the eye goes;
the last column is why the proposal wins the first-time user without
taxing the returning one.

| Question | Today | Proposed | Why it wins |
|---|---|---|---|
| How do I search? | a borderless placeholder line, top-left — recognizable only by reading it (the resting ring is deleted, doc 07 §5.1) | **⌕ top-left** — the universal mark, in the universal corner | The magnifier is read pre-attentively; no reading needed. Returning users: same place, same field, same keys (type-anywhere untouched) |
| How do I get to settings? | the word `Settings`, far right, one of six words on that half of the strip | **⚙ in the corner** | Symbol *and* position are the convention; the corner gains a mark instead of a sixth word. Tooltip carries the word |
| How big is my library / how many matched? | counts floating mid-strip, ≈ 1 100 px from the query that filters them | **inside the well** — placeholder at rest, `7 / 1 284` beside the caret while typing | The fact lands where it is watched (L8.3); doc 07 §3.1's move, delivered |
| How do I change how the wall is arranged? | five caps words beside the well | unchanged | Already right: states as words, current one in Medium — a form with no conventional icon to beat it |
| How do I shuffle? | the word `Shuffle` | unchanged (word) | Deliberate: the conventional glyph promises a mode baz refuses. The word is the honest form, and it sits where the eye already found `Play all`'s triangle — the acts cluster now has a visual anchor |
| How do I play everything? | the word `Play all` | **▶ Play all** | The triangle is the product's one universal "press = sound" mark, already learned from `Play album` |
| What is playing / what plays next / what list is this run from? | the bar; the continuation line; the Queue place's summary | unchanged | Doc 09 §10's answers survive untouched — the bar does not move |
| What lists do I have? | `Playlists` door, then the panel | unchanged | A named door (L8.4); the panel unchanged |
| How do I keep it / put *this* somewhere? | `+` / `Add to…` / right-click → picker | unchanged in flow; the `+` becomes a drawn glyph matching the ✕ beside it | One mark technology per row; the gesture grammar of doc 09 §8.1 is untouched |
| How do I leave a list? | play anything else | unchanged | — |
| Where is the song I'm thinking of? | type — the Songs section | unchanged | — |

The stability half, summarized: **no control changes surface, order, or
gesture.** Two words become glyphs standing in the same positions; one
readout moves 700 px left into the control it describes; one act gains a
mark. A returning user's muscle memory survives every row of the table;
the first-time user gains the two marks and the corner conventions that
every other application taught them.

---

## 6. Considered and rejected

Recorded with the argument, so re-proposal meets a reason.

1. **A hamburger / overflow "⋯" for the strip's excess.** There is no
   overflow menu to sweep a control into (doc 07 §0.1); a menu that held
   acts would also break the context-menu mirror rule (menus are
   accelerators over visible controls, never homes — doc 09 §5.2). The
   answer to crowding is the budget law and the split, not a drawer.
2. **A toolbar-preferences panel** ("customize which controls show").
   The customisable-panel tradition's own record (`03` §4.3) — layout as
   the medium of self-expression is the disease, W21 is its tell — and
   a control that can be hidden by preference violates the
   visible-control rule for whoever hid it.
3. **Hiding controls behind hover.** Refused outright
   (the product's standing rule); listed only because toolbars in the wild do
   it.
4. **A dropdown for the group keys** at narrow widths. Refused by name —
   no view-options menus, no sort dropdown (the product's standing rule); the
   row of words *is* the design (`top_bar.rs:326–332`).
5. **Crossed-arrows for `Shuffle`, a die for `Pull`.** §3.5: the first
   fails the semantics clause (mode symbol on an act), the second is the
   recommendation engine's costume (the product's standing rule,
   `top_bar.rs:163–169`) — and `Pull` has no convention, which is the
   brief's own example of where an invented icon loses to a word.
6. **Icons for the `Queue` and `Playlists` doors.** §3.4: no universal
   symbol separates queue / playlist / menu; L8.4's evidence (Sonos,
   Plexamp — `03` §4.5, §5.2e) is about exactly this ambiguity.
7. **Icon + word everywhere** (belt and braces). Doubles the strip's ink
   and width to hedge a decision; the rule of §3.1 exists so each
   control makes one honest statement. The two hybrids admitted
   (`Play album`'s family, `Play all`) are there because the *act* is
   conventional and the *scope* is baz's — a real semantic split, not a
   hedge.
8. **An icon font or SVG assets.** Re-litigated and re-rejected on
   `icon.rs:12–50`'s own survey: a binary asset with a licence to vet,
   or a dependency tree, for five glyphs a page of polygon literals
   draws in the product's exact ink.
9. **Shrinking the group keys** (initials, or dropping tracking) at
   narrow widths. The words are the data; five initials `A Y G A P` are
   a cipher, and two of them collide. The split regime keeps the words
   whole at every width above the floor.
10. **A second strip row at every width** (permanent two-line header).
    40 px of the collection at rest, forever, to avoid one breakpoint —
    the wall pays and the wide window gains nothing. The content share
    at rest (`03` §2.3) is the product's positioning number; it is not
    spent on symmetry.

---

## 7. The implementation plan

Doc 09 §13's shape: ordered so the highest-relief change lands first,
each step whole and shippable, none waiting on a later one.

1. **The gear and the magnifier.** `icon.rs` gains `Gear` and
   `Magnifier` (keyhole outlines, §3.6's note) with coverage tests in
   the module's own idiom; the strip's `settings_toggle` becomes a
   32 px glyph button with the tooltip `Settings`
   (`bottom_bar.rs:690–735`'s anatomy); `SETTINGS_TOGGLE_W` dies
   (`theme.rs:2877`); the well gains the magnifier layer. The owner's
   headline complaint — the missing conventional marks — lands in one
   step, and the strip breathes 52 px.
2. **The counts fold into the well.** Placeholder = the counts line;
   the match count takes its reserved in-well slot; the status row
   drops to the transient notes and the gear. Frees ≈ 125 px and
   discharges doc 07 §3.1's prescribed move. (`top_bar.rs:300–310`
   moves; the strip's declared hierarchy in L6's table — *counts →
   well → Settings* — is restated as *well → acts → gear* and
   re-measured.)
3. **The well narrows.** `SEARCH_W` 360 → `WELL_W = clamp(W − 1000,
   200, 280)`; `top_bar::view` gains `window_width` — the parameter it
   needs for every later step (`top_bar.rs:39`).
4. **`Play all` gains the triangle.** `album.rs:314–344`'s glyph+word
   anatomy at `word_button` ink; no accent. One function changes.
5. **The split regime.** `TOP_BAR_2LINE_H` 89; the breakpoint at 960;
   the app's layout estimate reads the resolved height; captures at
   1280 / 960 / 760 / 600 join `docs/design/impl/`.
6. **One mark technology in the rows.** `Plus`, `ArrowUp`, `ArrowDown`
   join `icon.rs`; the transfer and reorder slots
   (`queue.rs:522–587`, `album.rs:701–733`, `playlist.rs`, songs rows)
   and the settings steppers swap their font characters for the drawn
   set. Pure form; every message and slot width unchanged.
7. **The laws, pinned.**
   - `theme::the_strip_holds_its_tenants_at_the_single_line_floor` —
     the budget law as const arithmetic, the
     `bottom_bar.rs:874–885` pattern: reserved widths + gutters ≤ 960,
     and the two-line pair ≤ 600.
   - `every_icon_only_control_carries_a_tooltip` — source-pinned over
     the views, the `queue.rs:647–705` shape.
   - Stroke-band assertions per new glyph in `icon.rs`'s tests.
   - The L8.4 amendment and the L9 text land in
     `.interface-design/system.md` §13 via ADR-0026, with doc 07 §6's
     `placement.rs` gaining nothing (no control changed homes).
8. **The Settings header folds into `place_header`**
   (`settings.rs:260–282` → `views/mod.rs:236`), so the frame is one
   function in five places. Cosmetic-structural; last because nothing
   depends on it.

Steps 1–2 are the relief; 3–5 are the responsive repair; 6 is the
polish that makes the glyph set one set; 7 is what keeps all of it true
next year.

---

## 8. What this study does not decide

- **The Marquee lens's switcher form** (ADR-0017 step 18) — when it is
  built, `WALL · MARQUEE` is a state row and takes the state row's
  vocabulary; nothing here pre-empts its keys.
- **Any second room's glyph ink values.** The sheet is baked per room
  (`icon.rs:293–302`); Reading Room's arrival re-rasterizes and the
  contrast sweep governs, but the glyph *shapes* are room-independent
  by construction.
- **A drag's cursor and drop-target marks** (ADR-0024 §6 layer 3) —
  the pointer-capture widget will need affordance marks; they are that
  design's to argue against this rule, not this document's to guess.
- **Whether the strip's split regime ever hosts a third line.** It
  does not, and a proposal that needs one has outgrown the strip — the
  budget law's answer is re-homing, argued at L8, not accretion.

---

## 9. Summary

The strip got crowded the way the rail got crowded: one admission at a
time, each locally argued, no law bounding the whole. The fix is the
same shape as last time — a charter and a law with a test — plus the
thing the owner actually asked for: the conventional marks, admitted
under a rule strict enough that they cannot spread. Icon-only where the
symbol is universal *and* the semantics are exactly baz's (the gear,
the magnifier, the transport family, ✕, +, the arrows); word-only
where the act is baz's own (`Pull`, `Shuffle` — whose conventional
glyph promises a mode this product refuses to have — the doors, the
arrangement names); glyph-plus-word where the act is conventional and
the scope is not (`Play all`, in `Play album`'s shipped clothes). The
counts move into the well they describe, the well wears the magnifier
and gives back 80 px, the gear gives back 52, and the strip learns the
one honest answer to narrowness this product's refusals leave open:
split, never sweep, never hide. Ten controls become legible as three
clusters wearing three vocabularies — and every one of them is exactly
where it was yesterday.
