# ADR-0028 — the density detents

Pixel evidence for [ADR-0028](../../../adr/0028-density-detents.md) — doc 11
§5 **P8**, the owner choosing option **(a)**: a visible three-detent density
control in the place's own body, at the foot of the index rail's lane.

Every frame is the real release binary, captured by
[`capture.sh`](capture.sh) on a private `Xvfb`, with the six-variable
isolation of `docs/DEVELOPMENT.md`: scratch `HOME` / `XDG_DATA_HOME` /
`XDG_CONFIG_HOME` / `XDG_CACHE_HOME` / `XDG_RUNTIME_DIR`, no
`DBUS_SESSION_BUS_ADDRESS`. The run printed the receipt:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

The fixture is the generated, digitally silent 25-album / 206-track set from
`mkfixture.sh`, never `~/Music`; the scratch `HOME` carries an `.asoundrc`
routing ALSA's default PCM to `null`.

**Every density frame was reached by pressing the marks themselves**, not by
a seeded config and not by the gesture — the frames are evidence that the
visible control works, which is the thing P8 said did not exist.

## The frames

| Frame | What it shows |
|---|---|
| `01-marks-balanced.png` | The wall at launch: `Balanced`, 4 columns. Three detent marks at the lane's foot — the wall at one, four, nine works — with the middle mark at full glyph ink. |
| `02-marks-spacious.png` | After one press on the **top mark**: the wall re-hangs at `Spacious` (3 × 320), the full-ink mark moves to the top. The marks themselves have not moved a pixel — the lane's geometry is constant across steps. |
| `03-marks-dense.png` | After one press on the **bottom mark from Spacious** — a two-notch jump in one press, the mirror delta (`Density::steps_to(±2)`) doing what two gesture notches would: the wall re-hangs at `Dense` (5 columns), the full-ink mark moves to the bottom. |
| `04-mark-tooltip.png` | The hovered mark carrying its tooltip — `Spacious`, the accessible name the icon-only law requires in a toolkit with no accessibility tree. |
| `05-lane-strip.png` | The lane's foot cropped from the three step frames, side by side (spacious · balanced · dense): the active mark walking the ladder while everything else holds still. |

## The geometry, stated against the tokens

- The marks stand inside `INDEX_LANE_W` 108, which the wall already cedes to
  the lane at every step — so **no width test changed**: the grid is still
  `Grid::new(window − INDEX_LANE_W, density)` and the hang algebra is
  untouched (`the_hang_holds_with_the_index_rail_taken_off_the_wall` passes
  unmodified).
- Each mark is a `STEPPER_HIT` 24 box (law L7's named secondary) holding the
  16 px sprite; the box overhangs the window gutter by the sprite's 4 px
  centring inset so the **ink** stands on `W − HANG` — the lane's one
  declared edge (laws L1/L5), the same line the rail's letters hang from.
- The band keeps one un-zoomed `HANG` 40 above the bar; the spine above it
  elides against the height the marks leave, per frame, which is the same
  arithmetic that already fitted it to short windows.
- Active treatment: full glyph ink (1.00) against resting glyph ink (0.57) —
  the group-key row's paper-against-faint, in sprite form, and never the
  accent. The active mark is **inert**: the fact, while the other two are
  the controls.

## Reproducing

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-density-fix
toolbox run -c baz-dev docs/design/impl/density-control/capture.sh
```
