# One playlist page — saved and unsaved

Shipped 2026-08-12 from the owner's live-review finding:

> *"the component for unsaved playlists does not look the same as the saved
> playlists... we don't want too many similar components as it's just tech
> debt"*

The four frames below are the real release binary at **1280 × 860**, the same
library and room, with a fixture whose samples are all zero and an ALSA null
default. `BAZ_DEVICE_TESTS` was unset. The before pair was captured before the
composition changed; the after pair is the final implementation.

| | saved file | unsaved run |
|---|---|---|
| before | [`01-saved-before.png`](01-saved-before.png) | [`02-unsaved-before.png`](02-unsaved-before.png) |
| after | [`03-saved-after.png`](03-saved-after.png) | [`04-unsaved-after.png`](04-unsaved-after.png) |

## What the before pair found

The lower-level primitives agreed and the pages did not:

- saved used `views::page`: a fixed 320 px aside beside a main table; unsaved
  owned a separate full-width scroll document;
- saved used `PLAYLIST_BREAKPOINT`; unsaved laid its identity head against
  `ALBUM_BREAKPOINT`;
- saved kept Play and Rename/Delete under the sleeve; unsaved put
  cursor/remaining time, Undo and Save in a private summary strip above rows;
- saved rows were flat 49 px entries with artwork and an Album column;
  unsaved rows had variable pitch, no artwork/Album cell and record headings;
- each implementation owned its scroll prefix and empty state.

That is why sharing `identity_block`, `track_row`, collage cells and icon slots
was insufficient: presentation could still change in one state without the
other.

## The boundary after

`views::playlist_page` is now the only playlist-specific caller of
`views::page`. It owns the collage and sleeve edge, commitment reservation,
acts lane, three-line identity, breakpoint, fixed-aside/table and stacked
forms, `TRACKS`/empty anatomy, scroller and row-space translation.

The two state modules supply only meaning:

| slot | saved | unsaved |
|---|---|---|
| lead | `Playlists › Name` file identity | transient list name |
| commitment | Play | reserved: the run is already current |
| acts | Rename/Delete | Save or provenance readout |
| byline | `Playlist · N records` | `Unsaved playlist · N records` |
| facts | durable track/time counts | live cursor and remaining time |
| facts companion | file Undo | run Undo |
| row marker | number / sounding lamp | number / sounding lamp / next ring |

Both row builders spend the same fixed pitch, shared artwork helper, Album
context, base `track_row` and reserved edit slots. The next ring remains a run
fact, not a visual fork.

`playlist_page::tests::both_persistence_states_reach_one_playlist_page` is the
acceptance guard: neither `playlist.rs` nor `queue.rs` may call `page::view`,
draw a collage, choose a breakpoint, pad a private page or create a scroller.
The queue's virtualization test separately pins the shared fixed pitch,
artwork and Album context.

## Reproduction

The binary and fixture were built inside `baz-dev`:

```sh
toolbox run -c baz-dev env CARGO_TARGET_DIR=target/tb \
  cargo build --release -p baz --features device-output
toolbox run -c baz-dev \
  docs/design/composition/tools/mkfixture.sh \
  /tmp/baz-playlist-unification-fixture
```

Each headless launch redirected HOME and all XDG roots to scratch storage,
unset the session bus, routed `pcm.!default` and `ctl.!default` to ALSA null,
and used the fixture above. The application log carried the expected
`[mpris] no session bus` isolation receipt.
