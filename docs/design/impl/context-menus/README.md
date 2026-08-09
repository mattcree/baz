# The context menu, as a mirror layer — render captures

Doc 09 §13 **step 4** (`docs/design/09-implicit-playlists.md` §5.2), against
the real binary: right-click opens a float of short verbs at the pointer on
four objects — track rows wherever they appear, queue rows, album tiles, the
bar's now-playing block — and **every item sends only messages some visible
on-screen control also sends** (the mirror rule, pinned by
`every_menu_item_is_a_press_some_control_also_makes` in
`crates/baz/src/menu.rs`). `capture.sh` reproduces everything here headlessly;
its header says how, and its tail prints the `[mpris] no session bus`
isolation receipt and the no-reflow figure.

## The stills, 1280 × 860

| Still | What it shows |
|---|---|
| `01-wall-before` | The wall at rest, the pointer on a sleeve — the no-reflow baseline. |
| `02-tile-menu` | Right press: the **tile menu** at the pointer — `Open · Play album · Queue album · Add to playlist…`, §5.2's table row exactly (a tile's menu carries no `Add to "{current}"` — the table gives it none). No playlist is playing yet, so nothing anywhere names one. |
| `03-track-menu` | The record page's **track-row menu** — `Play · Queue · Add to playlist…`; `Add to "{current}"` absent, not disabled, because no provenance stands (09 §6). |
| `04-playlist-row-menu` | After `Road Trip` plays: the **playlist page row's menu**, the current list named — and the row's slots in frame, including the **transfer `+`** the page's rows gained with this step (§8.2's "same editor" anatomy completed; the visible twin the menu's items mirror). |
| `05-bar-menu-s4` | **S4's first gesture, from anywhere**: the bar's block right-pressed on the wall. The card opens *upward* — the bottom-edge flip — and carries `Add to "Road Trip"`, the pointer resting on it. |
| `06-queue-row-menu` | The **queue row's menu** — `Play · Add to "Road Trip" · Add to playlist… · Remove` — over the place whose summary leads with the same provenance the menu names. |
| `07-songs-row-edge-flip` | A **Songs row** right-pressed near the window's right edge: the same track-row menu, its card flipped to the pointer's *left*, whole and on screen. |
| `08-road-trip-after-s4` | **S4's both halves, visible**: after pressing the bar item, the file's page shows `4 tracks` with the appended row, while the Queue readout still says `3` and the bar still counts `then 2 more` — the file grew, the run did not (09 §6). The lamp dot marks no row: file and snapshot have diverged, and the page marks nothing rather than lying (`player.rs`'s exactness rule). |

## The stills, 1920 × 1080

| Still | What it shows |
|---|---|
| `09-tile-menu` | The tile menu at the larger window — provenance standing, and still no `Add to "{current}"` on a *tile*, per the table. |
| `10-bar-menu-s4` | The bar's menu at 1920: the same upward flip, the same S4 item under the pointer. |

## What the script verifies beyond the stills

- **No reflow**: `01` and `02` are identical outside the card's own masked
  region — `magick compare -metric AE` = **0**. The menu is a layer over the
  place (ADR-0016's stack + `opaque`, no scrim), never a column.
- **The append is real and one-sided**: the run's log shows
  `[playlists] 1 added to "Road Trip" (4 entries)` after the S4 press, and
  still `08` shows the queue untouched.
- **Isolation**: the six XDG redirections of `docs/DEVELOPMENT.md`, a null
  ALSA sink over an all-zero fixture, and the `[mpris] no session bus` line
  printed as the receipt.

## Reproducing

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev docs/design/composition/tools/mkfixture.sh /tmp/baz-cm-fix
toolbox run -c baz-dev env FIX=/tmp/baz-cm-fix docs/design/impl/context-menus/capture.sh
```
