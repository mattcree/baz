# ADR-0028: Three visible detents for the wall's density

**Status**: accepted (2026-08-09) · ships doc 11 §5 P8, the owner choosing
option **(a)** · **overturns one clause of a REFUSALS entry under the
ledger's editing rule** — "no grid-size picker" as applied to three quiet
detent marks in the place's own body; the entry otherwise stands and is
narrowed, not deleted · amends doc 07 L8.1's density row · adds no state, no
message, no token and no dependency.

## Context

The product's own laws contradicted each other, and the contradiction was
being resolved silently in favour of invisibility.

- **The visible-control rule** (the product's standing rule, Accessibility): *"Every
  action in baz has a visible, pointer-reachable control. No action is
  keyboard-only, and no control's only affordance is hover."* Doc 09 §5.2
  applied it to gestures in so many words when it admitted the context menu:
  *"a right-click is a gesture, and no action may be gesture-only."* This
  entry is not taste — it is **the mitigation for a toolkit that publishes
  no accessibility tree** (ADR-0017 §4), which is why it has survived every
  design that wanted a control gone.
- **The view-options refusal** (the product's standing rule, The interface): *"No
  view-options menus. No grid-size picker, no list-mode toggle, no column
  chooser, no sort dropdown… density is a zoom gesture."*

Density's only routes were <kbd>Ctrl</kbd>+<kbd>-</kbd> /
<kbd>Ctrl</kbd>+<kbd>=</kbd> and <kbd>Ctrl</kbd>+scroll — a keyboard chord
and a modified gesture, no visible control, no readout, no Settings row. An
action whose every route is a gesture is exactly what the first entry
forbids; the second entry is what kept any visible route from existing. The
Jobs-era critique named the deadlock (doc 11 §4's scorecard row: *"breach of
their own rule"*) and presented the resolution both ways (§5 P8): give the
zoom a visible handle, or amend the visible-control rule to exempt
view-position acts. **The owner chose (a), the visible handle
(2026-08-09).**

## The entry's argument, engaged rather than snuck past

The editing rule: removing a refusal needs an ADR that beats its argument.
The clause's real argument, steelmanned from `02` §2.7 and ADR-0017 §1.3:

1. **A free zoom destroys reproducibility.** A slider makes every screenshot
   different, every layout report unreproducible, and every reserved-slot
   argument conditional.
2. **Settings must never be the answer to a view question** (ADR-0017 §1.3).
3. **View-options menus are the junk drawer** — a chooser row grows tenants,
   and a surface that enumerates view state invites more view state.

All three arguments **survive this decision untouched**:

1. The control is the three named steps and nothing between them — detents,
   not a slider. Every screenshot is still one of three walls per width.
   `Density` gains no variant, no fourth number, no interpolation.
2. There is still no Settings row, no Appearance section, no preference.
   The step persists exactly as it did — as state in `config.toml`, the way
   the group key does.
3. There is still no menu, no dropdown, no chooser, no readout row. Three
   marks of glyph ink stand in the place's body the way the group keys stand
   in the strip: words-or-marks on the surface itself, which is the shape
   the ledger's own gloss endorses (*"group keys are a row of words; the
   lens switcher is two words"*).

What does **not** survive is the clause's application to *any* visible
density control. It is beaten on two grounds the ledger itself supplies:

- **Its own corpus outranks it.** The accessibility entry protects listeners
  a toolkit already fails; the view-options entry protects the frame from
  chrome. Where the two collide — and for density they collide exactly —
  the ledger cannot coherently prefer the aesthetic entry: the
  visible-control rule is the stated reason the transport buttons, the
  search field and the labelled Queue door exist at all. A ledger that
  waives its accessibility mitigation to keep a wall quiet has inverted its
  own order of precedence.
- **The evidence base said CONTRADICTS from the start.** `03` R7 (Steam,
  Google Photos: durable damage for removing a density level; iPhoto's and
  iTunes grid view's size slider both Jobs-era) is what forced §2.7 into
  existence. The refusal never argued against a *visible* three-step
  control; it argued against menus and sliders, and the gesture-only
  outcome was inherited from ADR-0017 §1.3's (correct) rejection of the
  Settings placement, not decided on its own merits.

## Decision

### 1. The home: the foot of the index rail's lane

Doc 07 L8.1: density reads **the viewport, and nothing else** — subject:
view — so its home is the place's body, or nowhere. P8 names the two
candidate homes in the body; the rail's lane wins over the wall's empty
leading band on every axis that is not taste:

- **Subject.** The lane is the body's one resident view-subject surface —
  the rail reads *the arrangement and the viewport* (L8.1's own row). The
  two view controls share one strip; no new surface exists (L8.2(3): a
  cluster that already exists).
- **The leading band fails three ways.** It scrolls away with the wall — a
  control that leaves is a control back to undiscoverable; the pinned
  header claims the same band the moment the wall moves, so the lane would
  hold two tenants in alternation; and the band's height *is* the step's
  hang (28/40/48), so the control would resize itself as its own effect.
- **The wall's algebra is untouched.** `INDEX_LANE_W` 108 is constant at
  every step and every window; the grid is still resolved for
  `width − INDEX_LANE_W` and no width test changes by a character. The
  marks stand *below* the spine's strip, so the fisheye — a pure function
  of the pointer inside the spine's own bounds — never sees them, and the
  spine's per-frame elision absorbs the shorter lane exactly as it absorbs
  a short window (that arithmetic was already a function of the widget's
  real bounds; ADR-0020's amendment).

The marks are right-aligned so their glyph boxes stand on `W − HANG` — the
lane's one declared ink edge (law L1/L5), the same line the letters hang
from. The foot keeps one `HANG` of air above the bar. No new alignment edge,
no new height: each mark is a [`STEPPER_HIT`] box, L7's named secondary.

### 2. The form: three marks, spacious / balanced / dense

Three sprite glyphs in the existing sheet — one square, four squares, nine
squares: **the wall itself at its three densities**, which is as close to
self-depicting as a density mark can be (the convention every grid-size
control from Finder to Lightroom draws some variant of). Words were
considered and refused by geometry rather than principle: the lane's ink cap
is 68 px and the group-key anatomy at the meta size does not fit it without
clipping, and a control label that clips is worse than a glyph with a name.

- **Active mark**: full glyph ink (`GLYPH_OPACITY_HOVER` 1.0) against the
  resting `GLYPH_OPACITY` 0.57 of the other two — the group-key row's
  active treatment translated to sprite ink (paper against paper-faint),
  and **never the accent**: density is not playback truth. The wall itself
  is the primary readout — the covers' own size states the step — so the
  mark's lift is confirmation, not the sole carrier of the state.
- **The active mark is inert** — a container, not a button. Pressing the
  step you are on would do nothing, and a control that does nothing when
  pressed is the lie the rail's absent letters already refuse. This is
  L8.3's split in miniature: the active mark is the *fact*; the other two
  are the *controls*.
- **Hover** is the wash chip (`theme::transport`'s own background), the
  lane's established hover vocabulary — the spine's winner chip, the group
  keys' wash. No new tween: the 90 ms ink fade is the transport cluster's
  vocabulary, and `add_slot` set the precedent for a static-ink sprite
  button outside it.
- **Tooltips** per the icon-only law (doc 10 §3.1's accessibility clause,
  pinned by `theme::every_icon_only_control_carries_a_tooltip`): the
  step's name — `Spacious`, `Balanced`, `Dense` — is the accessible name.

### 3. The message: the gesture's own, exactly

A mark sends **`Message::DensityStep(current.steps_to(target))`** — the
same message, the same saturating `Density::step` walk, computed as the
signed number of gesture notches between here and the pressed mark.
`steps_to` is pure and pinned by
`a_marks_delta_is_the_gestures_own_notches`: applying `step(±1)` |delta|
times lands where one `DensityStep(delta)` lands, for every pair of steps.
The keys and <kbd>Ctrl</kbd>+scroll remain, **as accelerators of a visible
control** — the mirror rule's ordinary state (L8.7: the keyboard is the
same decision, made twice), where before they were the whole control. No
new message, no `DensitySet`, no second grammar.

### 4. The ledger, amended

the product's interface entry is narrowed under the editing rule and
carries the amendment note. What stands: no view-options menus, no
list-mode toggle, no column chooser, no sort dropdown, no free zoom, no
Settings row for a view question. What falls: "no grid-size picker" *as
applied to* three quiet detents in the place's own body. The entry's gloss
now reads "density is three detent marks on the rail's lane, and the zoom
gesture accelerates them".

## Deliberately not done

- **No Settings → Appearance row** — ADR-0017 §1.3 stands whole.
- **No readout beside the marks** — the wall is the readout; the active
  mark confirms.
- **No fourth step, no slider** — the three-step design is what makes the
  reserved-slot arguments unconditional, and this ADR leans on that.
- **No whole-lane-wide hit targets for the marks.** The spine's
  nearest-slot press owns the lane's band; giving the marks the full 108 px
  would put two press grammars on one x-range with nothing visible dividing
  them. The marks take the named 24 px square.

## Consequences

- `crates/baz/src/shelf.rs`: `Density::steps_to`, tested.
- `crates/baz/src/icon.rs`: three glyphs (one/four/nine squares), swept by
  the existing sheet tests plus their own run-count assertions.
- `crates/baz/src/views/shelf.rs`: the lane becomes spine over marks;
  placement and mirror tests
  (`the_density_marks_mirror_the_gestures_exact_messages`,
  `the_density_marks_stand_in_the_lanes_own_geometry`).
- `crates/baz/src/app.rs`: the keyboard-mirror table's `DensityStep` row now
  names the marks instead of declaring the gesture its own control.
- Captures under `docs/design/impl/density-control/` — all three detents,
  the active mark moving, taken by the real press on the real binary.
