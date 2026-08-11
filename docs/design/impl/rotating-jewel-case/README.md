# Rotating jewel case — WIP hand-off

Date: 2026-08-11

This is an experiment, not an accepted design. It replaces the flat artwork
on Now Playing with a slowly rotating CD jewel case that can be dragged. The
current build is useful because it proves the data and interaction seams, but
its rendering is visibly wrong and its active CPU cost is too high to ship.

## What exists

- `jewel_case.rs` is an Iced canvas program with a 32-second unattended turn.
- The timer runs at 20 Hz only while Now Playing is focused and has a track.
- Dragging currently changes horizontal yaw and a small vertical pitch.
- The existing Now Playing cover dissolve is preserved inside the case.
- Rear artwork resolution recognises an embedded `CoverBack` picture, then
  `back.{jpg,jpeg,png}` and `rear.{jpg,jpeg,png}`, case-insensitively.
- When no rear image exists, the canvas generates a coloured rear insert from
  the current album and its selected edition's track list.
- Missing front artwork still uses Baz's deterministic album colours.
- The ordinary album, playlist and library artwork renderers are untouched.

The implementation deliberately began with Canvas instead of a custom shader:
it reused Iced's image cache, text renderer and pointer events and let us test
the idea quickly. Canvas can only apply affine transforms, however. The case
therefore fakes a turn by scaling the face horizontally with `abs(cos(yaw))`
and drawing a separate spine silhouette. That shortcut is the central defect,
not a tuning problem.

## Owner review of the live build

The first live review found:

- The perspective looks wrong; in motion the far edge appears taller.
- The side is missing the black plastic tray characteristic of a jewel case.
- The spine/edge carries nothing.
- Vertical drag introduces top-to-bottom rotation, which makes no sense here.
- Long album titles leave the generated rear insert.
- The generated back should contain only the track list.
- Overall, the object currently looks "kinda weird".

Do not spend tomorrow retuning the cosine scale or the small 2D rotation. They
are why the object reads incorrectly. The next renderer needs actual projected
planes.

## Performance finding

After startup scanning had completed, `pidstat -p <baz> 1 10` measured the
focused Now Playing experiment at an average **13% of one CPU**, with samples
from 6% to 24%. The prior still surface idled around 1% on this machine. The
canvas rebuilds and tessellates its case, images and rear text on every 20 Hz
tick, so this result is consistent with the implementation.

That cost fails Baz's low-resource goal. Focus-gating is still correct—it makes
the timer disappear on every other page and while the window is unfocused—but
the focused cost must fall substantially before this can ship.

## Tomorrow's recommended path

1. Replace the Canvas face renderer with Iced 0.13's native WGPU shader widget.
   Draw a small cuboid/projected-plane mesh with a real perspective matrix.
2. Make drag horizontal-only. Idle motion and drag both change the same yaw;
   there is no pitch state or vertical response.
3. Model the object explicitly: clear front lid, front insert, black rear tray,
   rear insert, and a narrow spine plane. Put artist/album text on the spine.
4. When there is no real rear image, rasterise **only the numbered track list**
   into one cached texture per album. Fit or truncate rows before rasterising;
   never lay text out per animation frame.
5. Keep the real-back resolution order already implemented.
6. Upload front, rear and generated textures once when the album changes. Each
   animation frame should update only a small uniform (yaw/time), not create an
   Iced image handle or rebuild text.
7. Re-measure focused CPU and RSS. A reasonable gate is no more than 2–3
   percentage points above the still Now Playing surface on this machine, with
   zero timer cost off-page or unfocused.
8. Review the silhouette at front, three-quarter, edge-on and rear positions
   before restoring the unattended full turn.

If the shader still cannot meet the resource gate, retain drag rotation and
stop the idle turn; direct manipulation provides most of the idea without a
permanent clock.

## Validation at hand-off

- `cargo check -p baz --all-features`: passed.
- Strict workspace Clippy with all targets/features: passed.
- Jewel-case unit tests: 4 passed.
- Artwork tests, including rear-cover resolution: 9 passed.
- Playback-enabled release build: passed and was reviewed live.
- The full workspace test suite has not been rerun since this experiment began.

Relevant files are `crates/baz/src/jewel_case.rs`, `crates/baz/src/art.rs`, the
hero decode/state in `crates/baz/src/app.rs`, and the case composition in
`crates/baz/src/views/now_playing.rs`.
