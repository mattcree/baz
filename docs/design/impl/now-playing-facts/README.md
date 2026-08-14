# Now Playing local facts

Item 25 completes the one accepted ambient layer that remained unbuilt from
design 12 §8. The feed is local-only and deliberately small: one fixed-height,
non-wrapping line under the sounding track's identity. It is a button; a press
advances the fact instead of making rotation something that only happens to the
listener.

The cycle follows the recorded order: play count, last/never played, artist
collection count, measured loudness when the owned view model eventually has
it, full signal path, encoding/size, source playlist, release tags, and
track/disc position. The implementation emits only entries whose source data
exists. In particular, loudness remains absent because the current owned
album view model does not carry the analysis result; it does not invent or
substitute a tag value.

The independent Facts mark is on for a fresh config and persists beside the
foreground choice. A track change resets the index. The same `AdvanceFact`
message serves a press and the 20-second timer, while `fact_clock` admits that
timer only when Now Playing is visible, a record is sounding, and the control
is on. `Event::PlayRecorded` re-reads the append-only ledger after the engine
has recorded it, so a completed play becomes visible without restarting Baz.

Verification used the repository's release binary, digitally silent 25-album
fixture, ALSA null sink, private Xvfb display and all six XDG redirections. The
1600 × 900 Now Playing frame showed `Never played before` in its reserved line
without moving the source footer or resident bars. Targeted tests pin Unix date
and elapsed labels, flat non-engagement language, persisted default/on-off
behavior, and the zero-cost timer guard. The full workspace gates run with the
rest of the backlog close-out.
