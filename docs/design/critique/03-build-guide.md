# baz — Build Guide

Rust + iced 0.13. Constraints: no clipped images, no animation runtime, 4-sided borders only, no icon set, no accessibility tree / keyboard focus, 100k-track virtualization, ~200ms cold start.

## Scope
**v1:** rooms + elevation tokens (8a) | wall grid + shelves (1b) | type-to-filter (4a) | needle + segmented groove (5a, 9a) | inspector + sides + card (9b, 12b) | stack (6a, 13a) | group keys incl. PLAYED (10a, 12a) | index rail (11a) | shuffle-what-you-see (10b) | the pull (6b) | Marquee lens (2a, 3a) | first-run wall (4b) | history ledger + optional scrobbling.

**Deferred (designed, not discarded):** Crate lens (2b) | overview zoom stop (11b) | mixtapes (13b) | crates + CRATES key (13c) | art-derived accent hue (clamped experiment) | Lamplight/Gallery mixed rooms (7a — superseded by 8a). Each slots into existing machinery: Crate = a lens word; overview = a zoom stop; crates = a group key; mixtapes = the stack's save path.

## Build order
1. **History ledger first.** Append-only file, written from the first beta even with zero UI — history cannot be backfilled; PLAYED, the card, and the pull feed on it.
2. Room tokens + elevation primitives (4 surface levels, hairline-edge helper, state overlays).
3. Wall: virtualized grid, shelf headers, selection/playing states, halo.
4. Type-to-filter + Enter-plays-first; modifier-layer shortcuts.
5. Needle: segmented groove, wall label, transport keys, hover-reveal transport.
6. Inspector: track list, sides, inline tag edit, the card.
7. Stack: shift+click, numeral chips, popover, ephemerality.
8. Group keys + index rail (rail derives from active key).
9. Shuffle-what-you-see + the pull.
10. Marquee lens + idle behavior.
11. First-run/import wall; folder watching.
12. Rooms 2-4 + follow-system switching (cheap if step 2 was honest).

Steps 3-5 are the product: wall -> click -> sound. Nothing ships without them.

## iced notes
- Hairline edges: 4-sided borders can't draw one edge — stack a 1px-tall container (or custom quad) on the lit side.
- Halo: ring = 2px offset quad behind art; bloom (Closing Time only) = layered translucent quads or small custom widget. Test banding at 6-10% ivory on cheap panels.
- Marquee scrim: one quad with per-vertex alpha.
- Zoom/density: 3-4 discrete stops, no tweening; a hard cut between grid metrics is shelf-like.
- Glyph inventory (total): play, pause, prev, next — 4 hand-rasterised polygons, shown only over the playing cover on hover. Everything else is type.
- Groove: plain rects from track durations; side break = wider gap; click-to-jump = hit-test on segment index.
- Rotated text (Crate lens) means rasterising glyph runs — why it's deferred.
- Keyboard: global shortcuts don't need the focus tree. No AT tree = zero screen-reader support (see open questions).
- Cold start: persist wall scroll offset + paused track/position on every change; restore before first paint. Thumbnail cache keyed by file mtime; wall paints from cache, art decodes lazily.

## Open questions
- **Accessibility.** No AT tree + no keyboard focus is a product risk beyond visuals; this audience lives on keyboards. Type-to-filter + modifier layer covers operation; screen readers have no current answer. Decide the stance before 1.0.
- Hover-reveal transport with no animation — crisp or broken? Test the hard cut.
- Mid-value sleeve melt on Stone/Plaster — validate on real libraries; remedy = nudge room L (.33/.76), never borders on art.
- Art-derived accent hue — if revisited: clamp to the room's arc, fallback below a chroma threshold; ship only if users notice unprompted.
- "No search box" onboarding — one quiet first-run hint, once, or nothing.

## Definition of done
v1 is done when the friction budget holds on a 20,000-album library on mid hardware: launch->resume <200ms | click->sound <100ms perceived | keystroke->filter next frame | import with zero dialogs | tag fix without pausing playback — and the wall at rest is 100% collection in all four rooms.
