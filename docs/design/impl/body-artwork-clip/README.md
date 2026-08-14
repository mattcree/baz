# Body artwork containment

Item 21 closes the intermittent case where a scrolling sleeve could paint over
the resident app bar or bottom transport and a reset appeared to repair it.

## Cause

This was not a static z-order mistake. In iced 0.14,
`iced_widget::scrollable::draw` calls `renderer.with_layer(visible_bounds)` only
while the scrollbar layout is active. Its inactive branch passes the visible
rectangle to child widgets as a logical viewport. `iced_widget::Image::draw`
does not use that viewport and draws its complete translated bounds. A
navigation, resize or conditional-layer transition could consequently leave a
scrollable in the inactive cached state while a sleeve crossed its intended
viewport; rebuilding widget state explains the observed reset recovery.

Padding individual tiles would hide one symptom and leave every other sleeve
consumer—and pointer delivery—wrong.

## One boundary

`window_frame::body_clip` wraps the composed place and lane body before the app
and bottom bars are added. Its custom widget intersects its layout with the
parent viewport and:

- always opens a physical renderer layer for child drawing;
- supplies the same intersection to child updates and body overlays;
- makes the cursor unavailable outside the body and reports no interaction
  there.

That single boundary covers Library, Home, Playlists, album/artist/playlist
pages, Queue/Now Playing and floating playlist-panel sleeves. Search, health,
status and menu dropovers are composed after the bars and deliberately remain
whole-window overlays.

## Verification

Unit regressions cover the rectangle intersection, unconditional
`renderer.with_layer` call, pointer refusal and root ordering: body clip, app
bar, then bottom bar. A rebuilt isolated 1280 × 860 run used 39 albums and
combined dense scrolling, playlist-panel open/close, Home → Library routing,
1280 → 900 → 1280 resizing, density changes and further scrolling. The final
frame retained clean artwork boundaries beneath both bars without resetting the
process (`/tmp/baz-clip-stress.png` during the implementation run).
