# Single-window kiosk

Design 12 §11 already settled the achievable product: Baz has one window, the
desktop puts it on a display, and F11 makes that window fill its current
monitor. iced exposes `window::Mode::Fullscreen` but no monitor enumeration, so
a second window or in-app display picker would promise control the toolkit does
not have.

`keys::binding_for` treats bare F11 as a window act before text-field capture,
so the search caret cannot suppress it. The app reads the actual current mode,
requests its opposite through iced, and records fullscreen as the outermost
Escape layer. The first Escape therefore restores the same place instead of
navigating away. The behavior is intentionally available in every place;
Now Playing is the useful kiosk content, not the owner of a private window key.

Tests pin the binding with and without search focus, reject modified F11, and
round-trip Windowed/Fullscreen mode selection. The release binary was also
driven under the isolated silent Xvfb fixture: it accepted F11 and Escape and
kept the application alive and responsive. That display intentionally has no
window manager, so it cannot honor or visually prove EWMH fullscreen geometry;
monitor placement and the mode transition are, correctly, the real
compositor's responsibility.
