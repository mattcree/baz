# The contour — a playlist you draw

The owner, after the 2026-08-15 flow rebuild: *"the vibe flow just looks crap.
the entire UX and flow of it. I wanted something more graphical, like tuning it
via curves and so on. it makes no sense to anyone trying to use it. Do you
think it makes sense?"* — and then, given the answer: *"lets try something else
with the vibe thing… get your experimental/frontier UX/UI hat on and work
through a way to create an interesting contoured playlist."*

## The finding, which is what made this cheap

The control he was looking at — `Shape the journey`, four buttons named
`Steady`, `Build`, `Peak & settle`, `Cool down` — **was not connected to
anything.** Pressing one appended the words `energy shape: Build` to the *text*
prompt, and the selector that consumed that prompt (`select_semantic`) scored
every candidate as

```text
0.72 · relevance + 0.23 · continuity + 0.05 · noise
```

with **no position term at all**. Nothing about those buttons could move a
track by one place in the list. The phrase went to a text tower that was being
asked to match audio, where it contributed noise at best.

Meanwhile, twenty lines away in the same crate, `select_journey` interpolated
`(energy, brightness)` targets **across playlist position** and picked against
them — the real thing — with no caller anywhere in the product but its own
tests.

The interface was decorative and the engine was dead code. They were each
other's missing half.

## The engine: one walk, three ways to ask

`baz_vibe::Contour` is a list of `ContourPoint { at, level }` per axis, where
`at` is a fraction of the finished playlist and `level` is the
collection-relative −2…+2 scale the fit already scored against. `select_contour`
is now the one walk, and **the two older entry points are written in terms of
it**:

| the request | what it is | weights |
|---|---|---|
| words only | `select_semantic` | `0.72 relevance + 0.23 continuity` |
| a shape only | `select_journey` | `0.67 fit + 0.28 continuity` |
| **both** | `select_contour` | `0.45 relevance + 0.30 fit + 0.20 continuity` |

One row per kind of request, so adding the third could not move the two that
shipped — and the whole existing test suite, including
`a_journey_changes_the_target_across_the_list`, passes through the new walk
unchanged.

Two things came out with it, because a picture needs them:

- **`levels(candidates)`** — where every analysed track sits on the contour's
  own axes, so a surface can draw the library behind the line without holding
  a second opinion about what *energetic* means;
- **`Selection::levels`** — where each chosen track landed, in listening order,
  which is the result in the request's own units.

An axis a contour does not draw is **unconstrained** rather than pinned to the
middle, which is what lets one line be drawn alone
(`an_undrawn_axis_does_not_steer`).

## The control

`crate::contour` is a widget in the family of `groove`, `needle` and `spine`:
drawn in quads, holding only which point the pointer has hold of. iced's
`canvas` would give real strokes and is not in this build's feature set — it
pulls a tessellation stack (`lyon`) into a dependency graph this project
prices deliberately — so the line is a column per 4 px, filled to the floor,
which is the visual language of the spectrum analyser two places away.

What it draws, in order:

1. **the library's own distribution** — how much music sits at each height,
   faintest ink on the control, because it is context rather than a request;
2. the middle of the collection, once, so the box has a scale in it;
3. **the line**, as a band filled to the floor, with its draggable points;
4. **what the request produced** — a dot per chosen track with a thread
   between them, so the shape you asked for and the shape you *got* are one
   picture;
5. **the hovered track**, larger and on a guide to the floor.

Six presets are **drawn rather than named**: each thumbnail is this same widget
at 44 px with its handles off, so what a preset does is visible before it is
pressed. `Any` is first and is not a shape at all — it is the honest way to say
*the words alone*, which had to stay reachable once a line became the default.

## Hovering a row shows where it landed

The owner: *"it would be cool if we generate a playlist and when we hover the
playlist items it is showing where on the curve it's meant to be… the idea here
is that a person can see it really worked."*

Every row of the composed preview reports the pointer
(`Message::VibePreviewHovered`), the contour lights that track's dot on a
guide, and the line under the box says the same thing in words:

```text
7 of 12 · Ferrous 7 · asked for lively, landed at the middle
```

Five bands over the collection's own range — *the calm end, quiet, the middle,
lively, the loud end* — because the analysis measures loudness and spectrum,
not moods, and the words should not claim more than the numbers do.

## What the render caught that the tests did not

**The first press did nothing at all on a cold index.** `Message::VibeCreate`
required the analysis store to *already exist* (`.filter(|path| path.exists())`)
before it would read the library. That was survivable while a second button —
`Analyse locally & create` — created the store, and it became a dead press the
moment the 2026-08-15 rebuild folded the consent gate into the one press. Every
test passed; the button simply sat there. `prepare` creates the store, so the
only real failure is a system with no data directory, which the arm now says
out loud. `a_cold_index_still_composes_on_the_one_press` pins it.

## What went with it

`EnergyShape`, the three `SEMANTIC WAYPOINTS` fields, the `journey: String`
that carried a shape into a text prompt, and their three messages. A shape
travels as a shape now.

## Frames

Captured against a **real** analysis of the 206-track fixture library — every
claim this control makes needs an analysed collection to be true rather than
seeded. `capture.sh`; the run prints its `[mpris] no session bus` receipt.

| frame | what it shows |
|---|---|
| `01-vibe-form-cold` | the form before anything is analysed: the shapes, the line, no field behind it |
| `02-request-typed` | words and shape together — the two halves of one request |
| `03-analysing` | the one press reading the library |
| `04-composed` | the field behind the line, and the composed list as dots on a thread |
| `05-row-hovered` | a row's track lit on the line, with its sentence |
| `06-another-row-hovered` | the same, further along the list |
