# More rooms, and they stand the moment you press them — item 54

The owner, 2026-08-15: *"lets create more interesting themes for the app too,
and ideally can we apply them upon selection."*

Two asks with very different costs, and the second one is the reason the picker
said *"applies on restart"* since the day it shipped.

## Applying on selection

**The room was a startup fact.** `theme::ACTIVE` was a `OnceLock`, and
`crate::icon`'s two sprite sheets were `LazyLock` statics rasterized **once per
process in the room's glyph ink** — a glyph has no colour of its own, so the
room gives it one, and it was given it exactly once.

Three changes, and one of them is the only interesting one:

1. **`theme::ACTIVE` became swappable** — an `RwLock` plus a `GENERATION`
   counter. `active()` is called by every style closure of every widget in
   every frame, so it does **not** take the lock: it reads a relaxed atomic and
   a thread-local cache, and takes the lock only on the first read after the
   room actually changed. A lock per call would have been a lock per pixel's
   worth of decision.
2. **The sprite sheets became per room**, keyed on that generation and kept.
   A listener trying all six rooms ends with six pairs of sheets — 18 sprites
   of 32 × 32 × 4 bytes each, 73 KiB a room — and keeping them means going back
   to a room they have already seen mints no **new texture ids** for the
   renderer's atlas to churn on.
3. **Anything else that bakes a colour keys on the generation too.** In
   practice that is the jewel case's generated textures (its front gradient,
   the blurred rear with its drawn track list, the spine's type), so a room
   change is a cache *miss* rather than something a human has to remember to
   invalidate. Everything else in the product reads `theme::active()` per frame
   and simply follows.

An **imported** room stands immediately as well, which matters more than it
sounds: a listener editing a JSON room had to restart to see what they had
written, and that is what made the schema hard to work against.

A room that cannot be resolved when it is pressed leaves the one you are in
standing and says so — dropping somebody into Closing Time over a typo in a
file they are editing would be the worse answer.

## More rooms

Four rooms shipped, and they are deliberately quiet — but quiet is not the same
as colourless, and all four of them are, near enough, grey. Two more, each one
an existing room **in a different light**:

- **Blue Hour** — Closing Time's dark room at hue 264°, chroma 0.045. Every
  surface sits at Closing Time's exact oklch L, so the elevation law is
  satisfied by construction (a tread is made of lightness), and the ink keeps
  its lightnesses with a cool cast so every WCAG ratio lands where Closing
  Time's does. **The lamp does not move**: amber over indigo is the blue hour
  itself, and the accent is playback truth in every room — a listener who
  learns what the amber dot means should not have to learn it again.
- **Sea Glass** — Plaster's light room at hue 175°, chroma 0.030, with
  Plaster's own cool ink and oxblood lamp. Amber on a light ground is a stain;
  oxblood is a different mark, which is Reading Room's argument.

The picker lists them dark-then-light so the six read as a ladder rather than a
bag: Closing Time · Blue Hour · Stone · Sea Glass · Plaster · Reading Room.

### What the laws caught, before a frame was drawn

Blue Hour's first version put the wall's full chroma on **every** plane, and
two tests refused it:

- `the_veil_is_solved_against_a_stated_ground_and_its_residual_is_bounded` —
  3/255 off its intent;
- `the_option_ink_clears_its_floor_on_the_veil_over_any_sleeve` — an option's
  label at **4.37 : 1**, under the 4.5 floor.

Both have the same cause, and it is a good one to have written down. `recess`
is not only the plane below the wall: it is **the ink the hover veil is made
of**, and `veil_alpha` solves one alpha per stop by averaging its three
channels — honest only while the three answers agree, which a strongly
chromatic colour makes them not. So Blue Hour's `recess` keeps the wall's hue
at a **fifth** of its chroma, solved down until the residual came back inside
1/255. The room reads as a floor darker and quieter than its walls, which is
what the name describes anyway.

## The frames

`capture.sh`, headless at 1280 × 860 with all six XDG redirections, and
**`BAZ_ROOM` deliberately unset** — it is the development hatch that pins a
room at startup, and pinning one would photograph the thing this run exists to
disprove. The `[mpris] no session bus` line is the isolation receipt.

All six frames are **one process**. Nothing is restarted between them.

| | |
|---|---|
| `01-closing-time-wall` | The room baz starts in. |
| `02-the-six-rooms` | Settings → Appearance, with the six listed and the copy that now promises what it does. |
| `03-blue-hour-standing` | The frame after `Blue Hour` was pressed. |
| `04-blue-hour-wall` | The wall in it — sleeves, chrome, **glyphs** and the amber lamp. |
| `05-sea-glass-standing` | `Sea Glass` pressed from inside a dark room. |
| `06-sea-glass-wall` | A light room, from a dark one, with no restart: the glyphs are re-inked dark, which is the whole of the sprite-sheet change made visible. |
