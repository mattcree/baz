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

| Image | What it shows, and which section spends it |
|---|---|
| [`01-wall-tile-hovered-1280x860.png`](01-wall-tile-hovered-1280x860.png) | **The wall at rest, one tile under the pointer** — §2.2's baseline. The whole hover vocabulary is visible in it: a 1 px → 2 px rule under *Violet Ledger*'s label and its artist line one rung brighter. Nothing else on the wall changes, and nothing is drawn on the sleeve. |
| [`02-tile-menu-1280x860.png`](02-tile-menu-1280x860.png) | **The verbs the owner asked for, already floating beside the tile he pointed at** — §2.3. `Open · Play album · Queue album (Shift-click) · Add to playlist…`, a 232 px card whose top-left corner is the pointer. What is missing from it is one item: `Add to "{current}"` (§2.6). |
| [`03-picker-hint-1280x860.png`](03-picker-hint-1280x860.png) | **The complaint, in one frame** — §4.1. `Add “Violet Ledger” — pick a destination` is set at `SIZE_META` 12 in `paper_dim`, under a `Playlists` heading at `SIZE_EMPHASIS` 15 Medium and level with `Esc closes`: the panel's quietest line carries its only statement of what the surface is now for. Measure the trip too — the tile pressed is at x 444, the nearest pick target's row begins at x 963. |
| [`04-record-page-header-1280x860.png`](04-record-page-header-1280x860.png) | **The shipped depth affordance** — §3.2. `‹ Library · Album · ‹ Prev · Next ›` at the left of the header, `Esc returns to Library` at its right. The pair steps the wall's own arrangement (doc 11 P3) and says nothing about *where in it* this record stands, which is §3.4's proposal. |

Frames 01 and 03 also settle a claim the study leans on twice: the panel
overlays and does not re-hang the wall. *Ochre* occupies x 41–281 and *Violet
Ledger* x 324–564 in both, to the pixel, with 341 px of panel over the right of
the window in one and not the other (ADR-0016's float mechanics, ADR-0024 §5.4).
What the panel does cover while it stands is the index rail and the density
detents — recorded in §5.4 as a cost, not proposed against.
