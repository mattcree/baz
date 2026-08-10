# ADR-0040: The app bar — the window's chrome is baz's, and it holds what is true in every place

**Status**: accepted (2026-08-10), **with one field not yet flipped** (§6) · **amends [ADR-0022](0022-places-and-nothing-else.md)'s foundational sentence for the third time**, after [ADR-0030](0030-the-returns-lane-and-the-home-band.md)'s lane · **reverses [ADR-0028](0028-density-detents.md)'s amendment §3** on where the density marks stand, and keeps its §2 · **empties the strip charter in [ADR-0026](0026-iconography-and-the-strip-budget.md) §3** of two tenants and re-derives its budget · frames in `docs/design/impl/app-bar/`

> ## Amendment (2026-08-10) — the bar hangs by its ink, and zone 1 is the mark
>
> The owner, with the bar in front of him, in three more messages. Two are
> corrections and one is a question; the question is not answered here, because
> it is his (see below and `docs/BACKLOG.md`).
>
> Frames, the measurement and the harness:
> [`docs/design/impl/app-bar-gutter/`](../design/impl/app-bar-gutter/README.md).
>
> ### 1. The trailing gutter is the **ink** gutter — and §2's claim was false
>
> > *"the settings cog is padded in quite a bit and does not align with the
> > rail"*
>
> He is right, and the number is **25 px** at 1280 × 860 and again at
> 1920 × 1080. The index rail's letters end 41 px from the window's right edge
> and the bottom bar's volume groove ends 41 px from it — two independent
> surfaces, drawn by different code, already agreeing on law L1's line, which is
> what makes `W − HANG` *the* edge rather than one surface's opinion. The gear's
> ink stopped at 66.
>
> **Two causes, and the first is a plain defect:**
>
> 1. **16 px — a phantom seam.** The window buttons are absent unless baz owns
>    the chrome, and `views/app_bar.rs` put a zero-width `Space` where they would
>    go. `Row::spacing` falls between every pair of **children**, and a
>    shrink-width `Space` is a child, so the bar spent a `GAP_LG` on a
>    placeholder for a control that was not there. The two chrome states
>    therefore disagreed with each other — 25 px out with the buttons hidden,
>    10 px out with them drawn — which is how the cause was isolated.
>    `Row::push_maybe` pushes nothing for a `None`.
> 2. **8 px — the box is not the drawing.** Every control on the sheet is an
>    `ICON_PX` 16 sprite centred in a `TRANSPORT_HIT` 32 box: 32 is law L7's
>    pointer floor and 16 is the size the glyph is drawn at, and they are
>    concentric but not the same rectangle. Hanging the container from `HANG`
>    puts the **box** on the line and the **drawing** 8 px inside it. Invisible
>    on a strip whose neighbours are also boxes; visible the moment a glyph has
>    to line up with type, which is exactly the rail's case.
>
> **This means §2's `theme::pad(APP_BAR_PAD_V, HANG)` was carrying a false
> claim** — `views/app_bar.rs` said *"its right edge is the window's `W − HANG`"*
> and that was true of the container and false of everything drawn in it. The
> false claim is the actual bug; the pixels are its symptom. This ADR's own §2
> repeated it, and so did `views/shelf.rs`'s index-rail note, which still cited
> *"the alignment edge the `Settings` word above already established"* — a
> **word**, whose ink does start at its box edge. ADR-0026 replaced that word
> with a 32 px square and the visual edge moved 8 px inboard with nobody
> noticing, because nothing in the product measured it.
>
> **The rule, which is stated over the trailing control and not over the gear:**
>
> > **The app bar's trailing control puts its *sprite box* — not its hit box —
> > on `W − HANG`, whichever control that is.**
>
> The gear is only the trailing control while `app::owns_chrome` is false. The
> day it is true, the close button is, and the rule must give the same answer
> without a second clause — so the correction is on the **band's padding**
> (`theme::APP_BAR_HANG_R` = `HANG − CONTROL_INK_INSET` = 32) rather than a
> nudge on the gear, which would have been a state-dependent rule and would have
> moved the gear by a further 8 px relative to the marks the day the buttons
> arrived. Both states are measured after the fix: 42 and 43 from the right
> against the rail's 41, the residual being each mark's own inner air.
>
> The leading gutter stays `HANG`. The asymmetry is this law being **obeyed** on
> both edges rather than excused on one: zone 1 holds a mark whose ink fills its
> own box, so `HANG` puts its ink on the line already.
>
> **Two lines that were candidates and are not the answer**, recorded because
> one of them is a trap:
>
> - **the rail's lane centre**, `W − 70`. `crate::spine` sets its entries flush
>   *right*, so the rail has a visible vertical edge and no visible middle;
>   aligning to the centre is aligning to nothing drawn. The trap is that the
>   gear's ink centre before the fix was `W − 72.5` — within 2.5 px of it. It
>   looked like a rule and it was a coincidence, and a future reader who
>   "restores" it will undo this.
> - **the wall's scrollbar**, `W − 4 … W`. It is deliberately drawn *outside*
>   the gutter, in the 4 px of it that never held ink (ADR-0022, `views/shelf`);
>   it is the one thing L1 exempts, and hanging app furniture from it would put
>   the gear 36 px outboard of the whole window.
>
> **The budget moves**, and §2's figure was wrong twice over: the line was 400
> against a floor of 696 and this ADR reported the slack as 288 when the test
> asserted 296. With the trailing gutter at 32 the line is **392** and the slack
> **304**, and both are `const`-asserted rather than written in prose.
>
> ### 2. Zone 1 is the application's mark
>
> > *"we probably want an icon for our app to show in the bar"*
>
> **The icon already existed** and was not redrawn: `packaging/icons/`'s hicolor
> ladder, which the desktop entry, the release tarball and the Flatpak already
> install. The bar decodes the **32 px** rung once and draws it at `ICON_PX` 16
> — exactly the `@2x` contract every sprite on the sheet is drawn under. That
> rung comes from the *small-sizes* master, the size-specific artwork the
> freedesktop icon theme spec exists to allow; minifying the 256 px master 16:1
> here would have drawn the grey smudge that master deliberately avoids below
> ~48 px.
>
> **It is not on the glyph sheet, and the two are different kinds of asset.**
> `icon.rs` holds outlines in a unit square, rasterized to coverage and **inked
> by the room** — a glyph has no colour of its own. The application icon is
> full-colour by construction, and `packaging/README.md` already said so in as
> many words (*"`crates/baz/src/icon.rs` is unrelated"*). Putting the mark on the
> sheet would flatten away what makes it recognisable and would create a
> **second master**, which `packaging/icons/README.md` forbids.
>
> **Instead of the word, not beside it**, and the slot does not move.
> `APP_BAR_NAME_W` was 24 for a 19.54 px word and is now `ICON_PX 16 + GAP_SM 8`
> — the same 24, re-derived — so the bar's budget, its drag gap and every
> coordinate in `docs/design/impl/app-bar/` are untouched. Icon *and* word would
> have widened zone 1 to 48 and been the only one of the three asks that cost the
> composition anything, to say the same thing twice: on a single-window product
> this zone never varies, so it carries identity and nothing else, and a mark
> carries identity better than a three-letter lowercase word at the faintest ink
> in the room. The reference the owner named carries the mark, not the word.
> §2's zone 1 keeps its meaning exactly — a **statement**, not a control (L8.5) —
> and because it is not a control, the icon-only law (doc 10 §3.1) does not reach
> it, by the same reasoning §3 used to admit the three window buttons.
>
> **What it spends, and this is the one thing on his desk.** The mark carries the
> lamp dot, and in the bar that accent is **not playback truth** — the standing
> rule is that the accent appears only where it is. It is admitted as an
> exception with a stated boundary: *the application's mark is the
> application's, not the room's ink*, and nothing else in the chrome may reach
> for colour on this precedent. At 16 px the dot is about one pixel. **The
> reversal**: a monochrome `Glyph::Baz` on the sheet, inked like every other mark
> in the bar — it costs a second drawing of the mark and keeps the accent
> discipline whole.
>
> ### 3. Search in the bar is **his question**, not this record's answer
>
> > *"maybe we could put the search in the top bar?"*
>
> Not built, and deliberately not decided here. It is genuinely arguable — §1's
> admission rule is the owner's own and search passes it more cleanly than the
> display options do, while ADR-0036 gave the well one meaning and one home and
> ADR-0030's second amendment records what moving it into the lane *cured*. The
> case each way, the costs and the recommendation are in `docs/BACKLOG.md`'s
> *What the owner asked for*, which is where an ask lives until he answers it.
>
> What this record adds is the arithmetic, so the answer is a decision rather
> than a discovery. It is `const`-asserted beside the bar's budget: the lane's
> well is `SIDEBAR_MEASURE` 232 and its seam `GAP_LG` 16, which **fits** in the
> 304 px of slack with 56 px to spare — and the band is one `TRANSPORT_HIT`
> tall, which is exactly the well's own height and leaves **nothing** for the
> always-drawn counts line under it that ADR-0030's second amendment §3 put
> there. The field fits. Its readout does not.
>
> ### What this amendment does not touch
>
> §3's gestures, §4's right-always, §5's marks and §6's open `decorations`
> question are all unchanged.

## Context

The owner, 2026-08-10, in three messages and then three clarifications.

The instructions:

> *"please remove the 'Play all' button at the top of the library"*
>
> *"and please put the display options at the top bar"*
>
> *"we should have replaced the top window chrome with an app bar which has
> this + settings + the window controls, the same on all screens"*

Then, asked what he meant by the third:

> *"do you understand regarding the top bar is that I am wanting it to function
> as the window chrome mixed with controls similar to stuff like spotify"*

Then, the admission rule — which is the part this record is really about:

> *"adding controls that apply to all windows makes sense in the top bar"*

Then, on placement:

> *"I don't mind if we have the controls on the right hand side as long as we
> have a sensible consistent pattern"*

And, on the marks themselves:

> *"the way they appear for the library is nice"*

**What stood before this.** There were two strips: the Library's
(`views::top_bar` — the arrangement row, `Play all`, the gear, and the search
well at the widths the lane cannot hold it) and every other place's
(`views::place_header_led` — a lead and a quiet note). `views/mod.rs` claimed
in prose that *"the frame is the frame in every place"*, and it was true about
their geometry and false about their tenancy. The density marks stood at the
trailing edge of the block of works they hang — the index rail's foot on the
Library, a section rule on Home and an artist's page — which ADR-0028's
amendment had decided that same morning, having explicitly **refused the top
bar** for them. And the window's title bar was the platform's, which on
GNOME/Wayland means it was already being drawn inside baz's own process by
`sctk-adwaita`.

**The standing complaint this has to answer**, from 2026-08-09:

> *"just adding stuff into that top bar isn't good. I just find we need to have
> a proper think about how we lay out controls and what is intuitive."*

So the one thing this change was not allowed to be was three more buttons in a
strip he already thinks is crowded.

## Decision

**A resident app bar, at the top of the window, in every place, identical.**
It is the band a platform title bar occupies, drawn by baz: it moves the
window, it maximises it, it closes it, and it carries the two controls whose
subject is the whole application.

### 1. The admission rule, which is the owner's

> **A control enters the app bar only if it applies in every place. If it
> applies to one place, the bar is not where it goes.**

That is *"adding controls that apply to all windows makes sense in the top
bar"*, read on a single-window product as *all places*. It is a test, and it
settles the four tenants without appeal to taste:

| Tenant | Applies everywhere because | In |
|---|---|---|
| The window's name | the window is the window in every place, and a decorated-off window says its name nowhere else | ✓ |
| The display options | every surface that hangs works hangs them the same way, and the step is one piece of state for the whole product | ✓ |
| The gear | Settings is the *application's*, which is exactly why being Library-only was wrong | ✓ |
| Minimise · maximise · close | the window exists in every place | ✓ |
| **`Play all`** | **the Library's, and only the Library's** | ✗ |

`Play all` is therefore **removed rather than relocated**, which is what the
owner asked for and now has a reason as well as an instruction. The action went
with the control: `Message::PlayAll` and `App::play_all` are deleted, because a
message no control sends is the visible-control rule failing in the direction
nobody checks for. Home's `All songs` tile is untouched — it is a different
scope (the collection, not the wall as arranged) and it keeps its own gesture.

**And the rule's other half, which matters more, because the failure mode of a
resident bar is accretion one locally argued admission at a time — exactly how
the Library strip got crowded.** The closed tenancy, by the surface each
belongs to:

- **the search well is the lane's.** ADR-0036 gave it one meaning and one home;
- **a place's identity, breadcrumb and note are the place's.** They stay in
  `place_header_led` and in `top_bar`;
- **the transport is the bottom bar's**, whose own ratchet says nothing leaves
  it either;
- **the wall's arrangement keys are the wall's.**

This is the app bar's `A fourth destination is the nav rail L8.4 refused`, and
it is asserted in `views/app_bar.rs`'s tests rather than left as prose.

### 2. The zones, and the pattern

The owner's acceptance criterion is *"a sensible consistent pattern"*. So the
bar is five named zones, in this order, at every width and in every place:

```
  baz  ·······························  ▤ ▤ ▤ ▤    ⚙    ─  □  ✕
   1              2                        3       4       5
  name          handle                    view    app    window
```

1. **The window's name.** What this window *is* — the one thing a title bar
   says that nothing else in baz says, and the only zone that is a statement
   rather than a control (doc 07 L8.5).
2. **The handle.** Never a tenant. Press and travel moves the window; press
   twice maximises it. A control admitted here would eat the window's own
   gesture.
3. **The view** — controls that change *how the place you are in is shown*.
4. **The application** — doors to what the application holds rather than what a
   place does.
5. **The window** — controls that act on the window as an object of the desktop.

**The pattern is that scope widens rightward**: the view you are in, then the
application around it, then the window around that. That is a rule and not a
description, which is the test the owner set — *given a new control, the rule
says where it goes without an argument*. Ask what it acts on. One place: it
does not enter the bar at all. The view, in every place: zone 3. The
application: zone 4. The window: zone 5. Two readers cannot disagree, which is
what *consistent* has to mean if it is worth anything.

**The band is 41 px** — `2 × APP_BAR_PAD_V 4 + TRANSPORT_HIT 32 + 1`, a control
row plus a named lead each side (law L4). It is deliberately shorter than the
place strip's 49: the strip holds words and a text well, which need optical
air; this bar holds 32 px boxes and one short word, and a control box carries
its air inside it. Its ground is `theme::bar` — the now-playing bar's own —
because the window's two chrome bands are one idea interrupted by the places
between them.

**Its budget is L9's, asserted as const arithmetic** in
`theme::the_app_bar_holds_its_tenants_at_the_windows_own_floor`: the line comes
to 392 against the window's declared minimum of 696, so there is 304 px of
slack and **the bar has no split regime**. Giving it one for symmetry with the
strip below would be inventing a breakpoint nothing can reach.

### 3. It does what a title bar does

Every one of these is iced 0.13's own call, serviced by winit and then by the
compositor. baz reimplements none of them, which is the whole argument for
drawing the band rather than the behaviour.

| Gesture | Mechanism |
|---|---|
| Drag to move | `window::drag` on a press the band's controls did not capture |
| Double-press to maximise / restore | `window::toggle_maximize`, the second press recognised against the first's clock (`BAR_DOUBLE_CLICK` 400 ms) — iced 0.13's `mouse_area` has no `on_double_click`; 0.14 adds one |
| Minimise | `window::minimize` |
| Maximise / restore | `window::toggle_maximize`, with the drawn state read back by `window::get_maximized` after every resize |
| Close | `Message::Quit` — the same exit path MPRIS's Quit and the compositor's close request already take, so the session is written on the way out |
| Right-press → the window menu | `window::show_system_menu`, best-effort by nature |

**The whole band is the handle, and that is delivered by capture rather than by
a hole in the middle of it.** iced 0.13's `mouse_area` runs its content's
handler first and returns if the content took the press
(`iced_widget-0.13.4/src/mouse_area.rs:211`), so every button keeps its own
press and everything else — the gaps, the name, the empty slots — moves the
window. `docs/BACKLOG.md` had this as *"dragging is what the gaps do, not what
the bar does"*; the sentence is about controls keeping their presses, and the
mechanism is capture.

**This ordering is not arbitrary**: a borderless window that cannot be moved is
broken, whereas one that cannot be edge-resized is annoying. Move and maximise
were treated as non-negotiable and resize as the open question (§6).

**Three new glyphs** — minimise, maximise, restore — drawn on the sheet's own
0.14–0.15 stroke band and pinned in `icon.rs`. Minimise is deliberately *not*
`Glyph::Minus`: a stepper's minus is centred and full-measure, a minimise bar
sits low and inset, and a sheet where one drawing means two things is a sheet a
reader has to be told about. They are icon-only and need **no new licence**:
doc 10 §3.4's enumerated list of two is about symbols standing as a *door's*
label, and these are not doors — L8.4 does not reach them. What they must clear
is the icon-only law itself (§3.1), and they clear all three of its tests more
comfortably than anything already on the sheet.

### 4. The buttons are on the right, always

A `chrome` module that read GNOME's `button-layout` and KDE's `kwinrc`, parsed
both dialects and mirrored the bar was built, tested and then **deleted**, on
the owner's *"I don't mind if we have the controls on the right hand side"*.
Right-always is one fewer conditional in the layout, one fewer subprocess at
startup, and — the real argument — one arrangement rather than two, which is
what *consistent* means.

**The known cost is macOS**, where window buttons belong at the left and this
bar will look foreign. baz builds three platforms and lives on Flathub; the
owner weighed that and declined the per-platform path. **The reversal is one
line**: give `window_controls` a side and mirror `row![name, gap, furniture,
buttons]`. It is recorded here rather than papered over.

### 5. The display options: the marks move, and *absent, not disabled* survives

ADR-0028's amendment §3 put the marks at the trailing edge of the block of
works they hang and **refused the top bar in so many words**, citing doc 07
L8.1 and this owner's own standing complaint. The owner has now put them there
by instruction. That is sufficient — *the owner's decision is sufficient on its
own; an entry he reverses gets rewritten to say what was decided and why* — but
the interesting part is the condition that does **not** fall.

**The tension, named rather than resolved by accident.** ADR-0028 §2 says a
control that is present and inert is a lie, and made the marks *absent* on the
four places that hang no works. A bar that is *"the same on all screens"* pulls
the other way. Both cannot be literally true.

**The resolution: the control is absent, the slot is resident.**
`APP_BAR_MARKS_W` 96 is reserved at every width and in every place, and the
marks are drawn into it only where the place hangs works — the Library, Home,
an artist's page. On a record's page, a playlist's, Now playing and Settings
the slot holds air. So:

- nothing is ever present and inert (ADR-0028 §2 holds, unamended);
- the gear and the window buttons stand on the same two vertical lines in all
  seven places, so **the bar is the same bar** and navigating moves nothing.
  `docs/design/impl/app-bar/10-every-band-after-*.png` is that claim as a
  picture: seven bands stacked, three with marks, and every other pixel in
  register.

What *is* amended is the placement sentence, and doc 07 L8.6 forces the rest:
**no two controls may send the same message**, so the marks left the index
rail's foot and both section rules entirely rather than gaining a copy.
`views::shelf`'s `density_control` is gone, `section_rule_hung` is gone,
`DetentAxis` is gone with them (a bar is horizontal in every place there is),
and `views::density_marks` is the one function it always was.

**The treatment is unchanged, on the owner's *"the way they appear for the
library is nice"***: the same four sprites of the wall at its four hangs, the
same `STEPPER_HIT` boxes, the same resting ink with the current step lifted,
the same tooltips, the same inert active mark. What changed is where the
function is called from and which way the run is laid. If this bar could not
have carried them as they are, that would have been a fact about the bar to
solve rather than a licence to redraw them.

**One thing to look at rather than assume**: `Dense` is a 4 × 4 whose cells
minify to 2.25 px at 1×, and at the bar's real size it is visibly softer than
the other three. `docs/design/impl/app-bar/12-marks-4x-*.png` puts all four
side by side, magnified with a point filter so the question is not answered by
blurring it. A larger sprite for that one mark is small work if he wants it.

### 6. The one field that is not flipped, and why

`window::Settings { decorations: false }` is **wired and not defaulted**.
`BAZ_BORDERLESS=1` turns it on.

**The blocker is resize, and it was re-verified against the pinned sources
rather than trusted.** `iced_runtime-0.13.2/src/window.rs`'s whole `Action`
enum has no resize-direction variant, and `ResizeDirection` appears nowhere in
`iced_runtime`, `iced_winit`, `iced_core` or `iced` 0.13. Nor does the platform
cover for it: winit's `set_decorate(false)` calls `frame.set_hidden(true)`
(`winit-0.30.13/.../wayland/window/state.rs:1000`), and sctk-adwaita's hidden
frame drops its decoration subsurfaces, after which `click_point_moved` returns
`None` before it looks at anything (`sctk-adwaita-0.10.1/src/lib.rs:400,512`).
So under CSD-off on GNOME/Wayland there are no resize edges at all and no
compositor fallback for them. `Super`+right-drag still resizes, and the system
menu's `Resize` is reachable from this bar's right-press — but the edges go.

**`docs/BACKLOG.md` priced the fix as a ~30-line fork of iced. That is now
wrong, and in the owner's favour: the change landed upstream.** iced **0.14.0**
ships `window::drag_resize(id, Direction)`
(`iced_runtime-0.14.0/src/window.rs:304`), `window::Direction`'s eight variants
(`iced_core-0.14.0/src/window/direction.rs`), and the winit arm that services
it (`iced_winit-0.14.0/src/lib.rs:1438`). **There is nothing to fork.** The
question is no longer *"do we maintain a patched dependency?"* but *"do we take
the 0.13 → 0.14 upgrade?"*, which is a different and better question with a
measured price (§7).

Until he answers it, the default stays decorated. That is the brief's
instruction and it is also the right call on the merits: the trade is his to
make, and a window whose whole job is to be sized to the wall you want should
not lose its resize edges by an implementation detail nobody chose.

## Consequences

**The strip below is smaller, which is the answer to the crowding complaint.**
Nothing was added to it; it **lost two tenants**. `TOP_BAR_SPLIT` falls 824 →
**680** (the acts cluster's 88 + its `GAP_XL` 24 + the gear's 32), the library
line falls 552 → **440**, and the well-less single line falls 600 → **456**.
The floor stays 600 because it is also the window's sensible minimum. The
two-line regime survives but its band narrows from 224 px to 80: a third
departure would close it, and when that happens the regime should end because
the strip no longer needs it, not because a number was tuned.

**The window's top now costs 90 px on the five places that wear a strip** (41 +
49), against 49 before — **plus**, today and only today, the platform title bar
still drawn above it. Doc 10 §6.10 refused a permanent second header line at 40
px on exactly this basis: *"the wall pays and the wide window gains nothing."*
That refusal is met head-on rather than around: **the 41 px is the title bar's
own height moved inside the window, and the debt is cleared in full the day
`decorations` goes false** — at which point the top costs 90 where the platform
was charging ~37–46 for a band that held nothing of baz's. The frames in
`docs/design/impl/app-bar/` are taken under Xvfb, which has no window manager
and therefore draws no title bar in either build, so they show the *end state's*
arithmetic rather than today's; that is stated in the study's README rather
than left for a reader to discover.

**ADR-0022's foundational sentence gains a fourth clause**, and it is now:

> The window holds one place at a time, with **the app bar across the top of
> every one of them**, the returns lane to its left in every place but
> Settings, the index rail at the wall's right edge in Library as always, and
> the now-playing bar under all of them.

**Doc 07 §0.2's surface list gains its sixth entry** — *the app bar* — and the
"no sixth" clause is now "no seventh", with the same force. A surface is
admitted by an ADR that states its admission rule; this one's is §1.

**L1's window-edge census goes from four surfaces to five**, and the app bar is
the only one that touches three window edges. `theme`'s
`one_gutter_touches_every_window_edge` names it.

**Doc 10 §3.4's gear licence survives intact.** The owner's earlier sketch
(*"the settings on the left"*, 2026-08-09) would have spent half of it —
`docs/BACKLOG.md` had already priced that — and the scope-widens-rightward
pattern happens to keep the gear in the top-right corner where its "universal
in symbol **and** position" argument lives. The amendment that record
anticipated is not needed.

**Now playing and Home gain a band they did not have.** Both wore no strip; the
app bar is resident, so it is over them too. `App::body_height` subtracts it,
which matters for Now playing specifically because it is the one place bounded
in both axes.

**One `struct_excessive_bools` expectation comes back** on `App`, for
`window_maximized`. The note that recorded its removal is rewritten to say what
put it back rather than left claiming a reduction that no longer holds.

## Alternatives considered

**Fold the place's identity into the app bar and have exactly one band.**
Genuinely tempting: it costs no extra chrome at all and would make *one strip*
literally true. Refused for now because the Library's identity is a 360 px
arrangement row plus a 200 px well at narrow widths, which does not fit beside
264 px of app furniture at the window's floor without reviving a split regime
inside the window's own chrome — and because the place's lead must align with
the body beneath it while the window buttons must sit on the window's edge, so
the two want different gutters. It is the right shape to reach for if the
strip below ever empties further; recorded here so the next person does not
have to rediscover it.

**Present-but-disabled display options on the four places that hang no works.**
The literal reading of *"the same on all screens"*. Refused: ADR-0028 §2 called
a present-and-inert control a lie, on the owner's own ask, and the reserved
slot delivers the sameness the sentence is actually about (§5).

**Hand-rolled edge resize** — an 8 px hit band spending `window::resize` +
`window::move_to` per frame. Still refused, for `docs/BACKLOG.md`'s reason: it
would visibly lag under Wayland because every step is a round trip the
compositor would otherwise have done itself, and it re-implements badly the one
thing the platform is definitely better at.

**Forking iced for `drag_resize`.** Moot: it is upstream in 0.14 (§6).

## What the owner still has to answer

**One question: do we take iced 0.13 → 0.14?** It is the only thing between
this branch and the window he described. Measured, against the vendored 0.14.0
sources:

- **~130–170 edited lines across 12–14 files**, roughly half production and
  half test. The five hand-built `Widget` impls all move (`on_event` → `update`
  with capture on `Shell` rather than a return value; `layout` and `operate`
  take `&mut self`; `Overlay::is_over` is deleted). `iced::application`'s first
  parameter becomes a boot closure and `run_with` goes. `text_input::Id` and
  `text_input::focus` move to the generic widget id and `operation::focus`.
  Nine style literals gain a `snap` field; `rule::Style` loses `width`.
- **The riskiest parts are behavioural, not compile errors**: `drag.rs` and
  `menu.rs` both branch on a child's returned capture status to decide routing,
  and `spine.rs` measures label widths through a text stack whose shaper
  changes (rustybuzz → harfrust), so the fisheye lane's metrics will shift.
- **The dependency graph turns over**: wgpu 0.19 → 27, glyphon → cryoglyph,
  cosmic-text 0.12 → 0.15, thiserror 1 → 2. Expect roughly 15–25 new pins in
  `packaging/flatpak/cargo-sources.json` and 10–15 removed.
- **Two things get better for free**: cosmic-text 0.15 completes the
  fontations migration, so **both RUSTSEC ignores in `deny.toml`**
  (RUSTSEC-2026-0192 ttf-parser, RUSTSEC-2026-0206 rustybuzz) can be deleted;
  and the duplicate `lru` that `iced_glyphon` pins resolves to one copy.
- **One recorded rationale stops being true**: `Cargo.toml` justifies `zbus 4`
  on the grounds that `iced_core → dark-light` already links it. 0.14 drops
  `dark-light`, so that comment must be rewritten and the zbus 4-vs-5 choice
  reopened. It is a net *reduction* in crates.

If the answer is yes, the flip is `decorations: false` plus an eight-way hit
band spending `window::drag_resize`, and baz has the window he asked for. If it
is no, `BAZ_BORDERLESS=1` shows what it looks like and the platform keeps
drawing the bar above ours.
