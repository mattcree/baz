# Playlists folder in Settings

ADR-0024 makes playlist files the listener's artefacts. Settings already
showed every music root but not the directory holding those files, so the
sovereignty promise was true only in documentation.

Library settings now show the exact `Folder::open_default` path and an `Open
folder` word control. The control asks the host to reveal that folder; it does
not read, move or edit a playlist. Linux calls the XDG desktop portal with a
`file://` URI whose arbitrary path bytes are percent-encoded while separators
remain separators. macOS passes the path to `open`; Windows passes it to
Explorer. A failed request becomes a canonical health event.

Tests pin spaces and non-UTF-8 Linux path encoding. The all-feature check
covers the cross-module Settings projection and message route; platform CI
continues to compile the native branches.
