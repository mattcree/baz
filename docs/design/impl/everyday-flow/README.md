# Everyday flow — the four frames the study argues from

Rendered evidence for [`../../13-everyday-flow.md`](../../13-everyday-flow.md).
Every image is the real binary at `c7e0f8c` (the last commit to touch
`crates/`; `target/tb/release/baz`, built inside the toolbox), captured by
[`capture.sh`](capture.sh) at 1280 × 860 — the shipped default window — on a
private `Xvfb :197` with all six redirections from
[`docs/DEVELOPMENT.md`](../../../DEVELOPMENT.md): scratch `HOME`,
`XDG_DATA_HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_RUNTIME_DIR`, and
`DBUS_SESSION_BUS_ADDRESS` unset. A null-device `.asoundrc` in the scratch
`HOME` and a 25-album / 206-track fixture of digitally silent FLAC are the two
independent guarantees that nothing was audible; the owner's `~/Music`, library
database and session bus were never opened, and the run was stopped by pid.

The isolation receipt, printed by the script and reproduced here because a
claim of isolation with no receipt is a promise:

```
[mpris] no session bus; desktop media controls unavailable (I/O error: No such file or directory (os error 2))
```

## The frames

These are **before** frames. The study proposes removing the panel that
appears in frame `03` (ADR-0030 §5) and moving the picker it is serving as
into a card at the pointer (ADR-0031), so three of the four are evidence for
a change rather than pictures of a design.

| Image | What it shows, and which section spends it |
|---|---|
| [`01-wall-tile-hovered-1280x860.png`](01-wall-tile-hovered-1280x860.png) | **The wall at rest, one tile under the pointer** — §4.2's baseline. The whole hover vocabulary is visible in it: a 1 px → 2 px rule under *Violet Ledger*'s label and its artist line one rung brighter. Nothing else on the wall changes, and nothing is drawn on the sleeve. It is also the *before* for §9.1's grid table: four columns at art 243, gutter 40. |
| [`02-tile-menu-1280x860.png`](02-tile-menu-1280x860.png) | **The verbs the owner asked for, already floating beside the tile he pointed at** — §4.1. `Open · Play album · Queue album (Shift-click) · Add to playlist…`, a 232 px card whose top-left corner is the pointer. What is missing from it is one item: `Add to "{current}"` at album scope (§4.4). |
| [`03-picker-hint-1280x860.png`](03-picker-hint-1280x860.png) | **The complaint, in one frame** — §6.1. `Add “Violet Ledger” — pick a destination` is set at `SIZE_META` 12 in `paper_dim`, under a `Playlists` heading at `SIZE_EMPHASIS` 15 Medium and level with `Esc closes`: the panel's quietest line carries its only statement of what the surface is now for. Measure the trip too — the tile pressed is centred at x 444, the nearest destination row begins at x 963. |
| [`04-record-page-header-1280x860.png`](04-record-page-header-1280x860.png) | **The shipped depth affordance** — §5.3. `‹ Library · Album · ‹ Prev · Next ›` at the left of the header, `Esc returns to Library` at its right. The pair steps the wall's own arrangement (doc 11 P3) and says nothing about *where in it* this record stands, which is §5.3's proposal. |

Frames 01 and 03 also settle a claim the study leans on twice: **a float
overlays and does not re-hang the wall.** *Ochre* occupies x 41–281 and
*Violet Ledger* x 324–564 in both, to the pixel, with 341 px of panel over
the right of the window in one and not the other (ADR-0016's float mechanics,
ADR-0024 §5.4). That is the property §2.4(a) tests the returns lane against
and finds it cannot have: a *resident* surface at that width would cover
content rather than re-lay it, which is why the lane takes width and the
collapse is allowed to re-hang.

What the panel covers while it stands — the index rail and the density
detents, `INDEX_LANE_W` 108 of the wall's right edge — is visible in frame
`03` by comparison with `02`, and is one more reason the picker leaves it.
