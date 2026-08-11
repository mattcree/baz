# Rotating jewel case — second live prototype

Date: 2026-08-11

This remains an experiment rather than an accepted design. The first Canvas
prototype proved the data and interaction seams, but its affine face squash
looked wrong and cost about 13% of one CPU while turning. The current working
tree replaces that renderer with a native Iced/WGPU primitive for live review.

## What changed after the first review

- Six projected planes now form a shallow cuboid with real perspective; the
  far edge is no longer approximated by a 2D cosine scale.
- Dragging is horizontal-only. Vertical pointer travel cannot change the case.
- The two side faces carry a generated artist/album spine on a black plastic
  tray; the top and bottom expose the same dark tray.
- Real rear art still resolves from embedded `CoverBack`, then common
  `back.*`/`rear.*` files. Without it, Baz rasterises only the numbered track
  list onto a generated rear insert.
- Track rows and the spine label are measured and ellipsised before drawing,
  so long text stays inside the insert.
- Front, rear, spine, and outgoing crossfade textures are uploaded only when
  their handles change. A turning frame writes one 32-byte uniform and draws
  12 triangles.
- Generated RGBA inserts live in a 12-entry LRU. Canvas support has been
  removed from Iced; `ab_glyph` is now the only added direct dependency.
- The existing 200 ms cover dissolve is preserved in the front-face shader.
- The animation clock remains focus- and page-gated, now at roughly 30 Hz with
  a 32-second unattended turn.

## Still awaiting owner review

Review the silhouette and materials at front, three-quarter, edge-on, and rear
angles. In particular, decide whether the clear-lid rim and reflection are
enough to read as a jewel case, whether the 9% depth is convincing, and whether
the spine direction is right on both edges. If the object still reads as a
floating card, add explicit clear-plastic lid planes and hinge details rather
than returning to 2D distortion.

The released app has opened Now Playing and constructed the shader without a
WGPU validation error. The initial crash was a WGSL local named `from`, which
is reserved by the language; it is now `previous`. A live owner review of the
materials and drag feel is still required before accepting the design.

## Performance

The old Canvas prototype averaged **13% of one CPU** while focused on Now
Playing, with samples from 6% to 24%. The new release build averages **0.25% of
one CPU** while its window is idle away from the animated surface, confirming
that page/focus gating still removes the background cost.

The focused shader averaged **0.30% of one CPU** over ten one-second samples,
with 0–1% individual samples. RSS was about **162 MiB**. It therefore clears
the shipping gate of no more than 2–3 percentage points over a still Now
Playing surface by a wide margin.

## Validation

- `cargo check -p baz --all-features`: passed.
- `cargo clippy -p baz --all-targets -- -D warnings`: passed.
- Jewel-case tests: 4 passed.
- Full `cargo test --workspace`: passed.
- Playback-enabled release build in `baz-dev`: passed.
- Release startup and off-page resource sampling: passed.
- Focused Now Playing GPU/resource validation: passed (visual review pending).

Relevant files are `crates/baz/src/jewel_case.rs`,
`crates/baz/src/jewel_case.wgsl`, `crates/baz/src/art.rs`, the hero decode/state
in `crates/baz/src/app.rs`, and the composition in
`crates/baz/src/views/now_playing.rs`.
