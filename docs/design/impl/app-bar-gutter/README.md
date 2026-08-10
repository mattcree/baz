# The app bar's gutter, and its mark

Frames and the measurement for [ADR-0040](../../../adr/0040-the-app-bar.md)'s
amendment of 2026-08-10 — the owner's two corrections to the bar that shipped
that morning, and the question he attached to them.

> *"the settings cog is padded in quite a bit and does not align with the rail"*
>
> *"we probably want an icon for our app to show in the bar"*
>
> *"maybe we could put the search in the top bar?"*

The first two are here. The third is a question and is answered in
[`docs/BACKLOG.md`](../../../BACKLOG.md)'s *What the owner asked for*, with the
arithmetic it turns on asserted in `theme`'s app-bar budget test; nothing was
built for it.

Reproduce with `capture.sh` (read its head first — it takes `BIN0` and `BIN`,
runs inside the toolbox, and does all six XDG redirections). The measurement is
`measure.py`, which the script runs over every frame at the end.

## 1. The measurement, which is what the first ask is about

Ink columns, in window coordinates, at 1280 × 860. **Identical at 1920 × 1080**
— every figure below is measured from the window's right edge and every one of
them is the same at both widths, which is the first thing a reservation-based
bar should be able to say about itself.

| surface | before | after |
|---|---|---|
| index rail's letters | **41** from the right | 41 |
| bottom bar's volume groove | **41** from the right | 41 |
| app bar's trailing control, `owns_chrome` **false** (ships) | **66** | **42** |
| app bar's trailing control, `owns_chrome` **true** | **51** | **43** |

The first two rows are the control. They are two independent surfaces, drawn by
different code, that already agree — `crate::spine` sets the rail's letters
flush to `bounds.width − theme::HANG`, and the bottom bar hangs from the same
`HANG`. That agreement is what makes `W − 40` *the* edge rather than one
surface's opinion, and it is why the owner is right that the gear was the thing
out of line.

**The gear stood 25 px inside it.** Two causes, found by arithmetic and
confirmed by the two chrome states disagreeing with each other:

1. **16 px — a phantom seam.** The window buttons are absent unless baz owns the
   chrome, and the row put a zero-width `Space` where they would go. `Row`'s
   spacing falls between every pair of *children*, and a shrink-width `Space` is
   a child, so the bar spent a `GAP_LG` on a placeholder for nothing. This is
   why the two states disagreed: with the buttons actually drawn the seam has a
   tenant and the error was only 10 px, not 25.
2. **8 px — box versus ink.** Every control on the sheet is an `ICON_PX` 16
   sprite centred in a `TRANSPORT_HIT` 32 box. The box is a *hit target*, the
   sprite is the *drawing*, and they are not the same rectangle. Hanging the
   container from `HANG` puts the box on the line and the drawing 8 px inside
   it. That is invisible on a strip whose neighbours are also boxes, and visible
   the moment a glyph has to line up with **type** — which is exactly the
   rail's case.

**The rule that came out of it**, and it is a rule rather than a repair:

> The app bar's **trailing control puts its sprite box — not its hit box — on
> `W − HANG`**, whichever control that is.

Stated over *the trailing control* because the gear is only trailing while
`app::owns_chrome` is false; the day it is true, the close button is. The rule
gives the same answer in both states with no second clause, and that is asserted
in `theme::the_bars_trailing_ink_lands_on_the_windows_gutter` rather than left
to these frames.

What it is **not** aligned to, since three candidate lines were on the table and
they are three different x:

- **not the rail's lane centre** (`W − 70`). The lane's centre is not drawn.
  `spine` sets its entries flush **right**, so the rail has a visible vertical
  edge and no visible middle. Aligning to it would be aligning to nothing — and
  it is a live trap rather than a hypothetical one, because the gear's ink
  centre before the fix was `W − 72.5`, within 2.5 px of that lane centre. It
  looked like a rule. It was a coincidence.
- **not the wall's scrollbar** (`W − 4 … W`). The bar is deliberately drawn
  *outside* the gutter, in the 4 px of it that never held ink
  (`views::shelf`); it is the one thing L1 exempts, and hanging app furniture
  from it would put the gear 36 px outboard of everything else in the window.
- **the window's `W − HANG`**, measured on ink. Law L1 is about where ink
  stands, and this is the line the rail's letters, the volume groove and the
  last column of covers already stand on.

The residual after the fix is 1 px for the gear and 2 px for the close button.
That is each mark's own inner air inside its sprite square against a
letterform's side bearing, and it is deliberately not chased: hanging each glyph
by its own drawn extent would be a different x per glyph, and the bar's trailing
edge would move whenever a control's drawing changed.

### Frames

| | |
|---|---|
| `01-library-{before,after}-{1280x860,1920x1080}.png` | the whole window, one coordinate system |
| `02-edge-library-3x-*.png` | the bar's trailing 200 px over the rail's first rows, before above after, at 3× |
| `02-edge-borderless-3x-*.png` | the same with `BAZ_BORDERLESS=1`, where the trailing control is the close button |
| `04-borderless-{before,after}-*.png` | those windows whole |

`02-edge-library-3x` is the frame the complaint is about: one vertical line, and
whether the gear stands on it.

## 2. The mark

Zone 1 drew the word `baz` at the metadata size in the faintest readout ink. It
now draws **the application's own icon** — `packaging/icons/hicolor/32x32/`, the
same file the desktop entry and the Flatpak install, decoded once and drawn at
`ICON_PX` 16 logical px. `03-zone1-8x-*.png` is the word above the mark at 8×
with `-filter point`, so what a 16 px full-colour icon actually resolves to in a
41 px band can be looked at rather than assumed.

**The icon already existed and was not redrawn.** `packaging/icons/README.md` is
explicit that the SVG is the master and the PNG ladder is rendered from it, and
the 32 px rung comes from the *small-sizes* master, which is the size-specific
artwork the freedesktop icon theme spec exists to allow — the wall label's
second line is dropped below ~48 px because two 1 px lanes composite into one
grey smudge. Minifying the 256 px master 16:1 here would have thrown that work
away and drawn the smudge.

**It is not on the glyph sheet, and that is the finding rather than an
implementation note.** `crates/baz/src/icon.rs` holds *glyphs*: outlines in a
unit square, rasterized to coverage and **inked by the room** at draw time. A
glyph has no colour of its own. The application icon is full-colour by
construction — a wall gradient, a sleeve in the placeholder gamut, a letterform,
a picture light — and `packaging/README.md` already said the two are unrelated.
Putting the mark on the sheet would mean flattening it to coverage, which
discards what makes it recognisable, and maintaining a second master, which the
packaging README forbids in as many words.

**Instead of the word, not beside it.** The slot is fixed at
`APP_BAR_NAME_W` 24, so this is the option that costs nothing: 24 was
`19.54 + slack` for the word and is `ICON_PX 16 + GAP_SM 8` for the mark — the
same number, re-derived, so the bar's budget, its drag gap and every coordinate
in [`../app-bar/`](../app-bar/) are untouched. Icon *and* word would have widened
zone 1 to 48 and been the only one of the three asks that cost the composition
anything. It would also have said the same thing twice: on a single-window
product this zone never varies — it is `baz` in every place, in every state,
forever — so it carries identity and nothing else, and a mark carries identity
better than a three-letter lowercase word set in the faintest ink in the room.
The reference the owner named does the same (*"similar to stuff like spotify"*).

Measured: zone 1's ink ran `40…59` (the word) and now runs `40…55` (the mark).
Both hang from `x = HANG`, at both widths.

**What it spends, stated rather than absorbed.** The mark carries the lamp dot,
and in the bar that accent is not playback truth — the standing rule is that the
accent appears only where it is. It is admitted as an exception with a boundary:
*the application's mark is the application's, not the room's ink*, and nothing
else in the chrome may reach for colour on this precedent. At 16 px the dot is
about one pixel. **The reversal is a small one and is worth knowing about**: a
monochrome `Glyph::Baz` on the sheet, inked like every other mark in the bar,
which costs a second drawing of the mark and keeps the accent discipline whole.
That is a decision for the owner and ADR-0040's amendment says so.

## What these frames do not show

Under Xvfb there is no window manager, so no platform title bar is drawn above
baz's band in **either** build and at either setting of `BAZ_BORDERLESS`. The
borderless frames therefore prove *which controls baz draws and where*, which is
what this study claims, and not what GNOME does with `decorations: false` —
which is ADR-0040 §6's open question and is not this study's.
