# Settings diagnostic log

The owner asked for the app log in Settings, perhaps under Debug. This is the
diagnostic stream (`[startup]`, `[scan]`, `[playback]`, `[config]`, `[mpris]`
and peers), not another door to the notification bell's curated event history.

Every existing tagged console write now passes through `diagnostic::line`. It
still writes to the developer console and also retains the newest 256 lines in
a process-local ring, prefixing each with elapsed session minutes/seconds.
Settings → Debug snapshots the ring newest first, so an arriving line does not
need to mutate scroll state to remain discoverable.

The ring is intentionally not persisted. Diagnostics contain local paths, and
a silent second log file would create privacy, retention and cleanup promises
that the ask does not require. This also gives packaged Windows builds useful
diagnostics without reversing their GUI-subsystem/no-console behavior.

The ring-buffer regression proves the hard capacity and newest-line retention.
An isolated 1280 × 860 tiny-skia/Xvfb launch opened Settings → Debug and showed
the complete startup, engine, history, MPRIS, library, playlist and scan
sequence with the session/no-disk copy visible in the place.
