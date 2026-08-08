# Changelog

All notable changes to baz are recorded here, in the format of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Versioning

baz follows [Semantic Versioning](https://semver.org/). It is **pre-1.0**, and
what that means here is specific rather than decorative:

- **`0.y.z` promises nothing about compatibility.** The on-disk library
  database, the `baz-core` command/event protocol, the configuration file and
  the user interface may all change in a `0.y+1.0` release. Schema migrations
  are still written and tested — a database from an older baz is upgraded, not
  discarded — but the shape of things is not yet settled.
- **`0.y.z` → `0.y.z+1`** is bug fixes and additions that break nothing.
- **`0.y.z` → `0.y+1.0`** is anything else.
- **1.0.0** is not a quality claim about the code; it is the point at which the
  library format and the protocol become promises. It arrives when they are
  worth promising, not on a date.

Every release is built from a tag by CI, gated on the full test suite — see
[`docs/RELEASING.md`](docs/RELEASING.md). Nothing below has been tagged yet.

## [Unreleased]

Everything so far. baz has never been released; this section is the state of
`main`, and it becomes the first version's section when a tag is cut.

**Status: pre-alpha.** It scans a music folder, shows the albums and plays
them. It is not a finished player, and nothing here is a promise about the
next commit.

### Added

**Library**

- Directory scanner reading tags with `lofty`, falling back to folder-structure
  inference (artist/album/track) where tags are absent or unusable.
- Persistent library index in SQLite (bundled, so no system library is
  required), with an in-RAM corpus for search. Schema versioning via
  `user_version` with migrations tested against databases built by older schema
  SQL rather than by baz itself; currently at v5.
- Search across the whole library as you type, over a memchr-backed substring
  scan.
- Albums grouped by **album artist**, not track artist, so a soundtrack or
  compilation is one album rather than one per credited performer. The album
  artist resolves from the `ALBUMARTIST` tag, else the compilation flag, else
  the track artist; where no signal exists baz declines to merge rather than
  guess (ADR-0008, schema v3).
- **Album editions**: one album that exists in several formats — a lossless rip
  and an MP3 copy — is one shelf entry with an edition per codec, keyed on the
  codec read from file headers rather than on folder names. The default edition
  is ranked lossless before lossy, then by track count, then by mean bitrate
  (ADR-0007, schema v2).
- **Incremental scanning**: a file whose (mtime, size) stamp is unchanged is not
  reopened. Measured over 10 000 synthetic tagged files, scan 61.2 ms → 10.3 ms
  and a whole warm launch 83.4 ms → 11.6 ms (ADR-0010, schema v4).
- **Removal by positive confirmation only.** A row is deleted only when the walk
  saw something, the path is under the root just scanned, no ancestor directory
  failed to be read, and the filesystem confirms the file is gone. The stated
  price: deleting a whole album *folder* leaves its rows, because from below
  that is indistinguishable from an unmounted share.

**Playback**

- Gapless playback engine ported from the Phase 1 spike, with sample-level
  continuity asserted against synthesized ground truth rather than against
  baz's own output.
- Formats: FLAC (including FLAC-in-Ogg), WAV, MP3, M4A/MP4 (AAC and ALAC) and
  Ogg Vorbis, all through pure-Rust `symphonia` — no C library and no system
  dependency anywhere in the decode path. Per-format gapless behaviour is
  documented and tested, including what MP4 does not trim.
- **The output follows the source sample rate** and never resamples silently.
  A session opens the device at the native rate of the track that starts it; a
  track at a different rate drains the sink and reopens. Where a device offers
  no mode at the source rate the track still plays, converted, and the
  conversion is reported. Measured on a 24/48 file: play-to-first-sample
  2 224 ms → 12.5 ms on a rate change, 0.7 ms when the device is already there
  (ADR-0009).
- Seeking, with playback position reported from the engine's own knowledge and
  never extrapolated between reports.
- **Volume**, as engine state: a cubic 60 dB fader law shared by every front
  end, applied as software gain in the one place every sample passes. At
  exactly unity the samples reach the sink with no copy and no arithmetic, so
  bit-exactness at full volume is a property of the control flow rather than of
  floating-point luck. Reported honestly through `VolumePath` (ADR-0011).
- **ReplayGain**, read from the tags files already carry — `REPLAYGAIN_*` in
  Vorbis comments, `ID3v2` `TXXX` frames, MP4 freeform atoms and APE items, plus
  the Opus-style `R128_*` integer form — in **off / track / album** modes with a
  pre-amp, a separate pre-amp for files that carry none, and clipping prevention
  from the declared peaks. Album mode falls back to a track's own gain when the
  file declares no album value, and an untagged file is played exactly as stored
  by default, so switching ReplayGain on cannot alter a library nothing has
  scanned. It shares the volume's gain stage — one multiply per sample for both
  — and changes on the track boundary's own first sample. Reported through the
  same `VolumePath` the volume uses, because there is one gain stage and one
  answer to "is this bit-exact" (ADR-0013, schema v5).
  baz reads these figures; it does not compute them.
- **Queue editing that does not stop the music.** `JumpTo { position }` plays
  the entry it names — the queue-relative sibling of `Seek` — from wherever the
  transport is, including stopped. `UpdateQueue { paths }` removes, reorders,
  inserts and appends by sending the queue as it should now be: an edit that
  does not touch the playing track leaves the delivered stream bit-identical to
  an unedited run, because the running session plays its track out and only then
  hands over to the edited queue. What survives an edit is the playing *track*,
  never its index, so the engine re-derives the position by identity and reports
  it on `QueueChanged { len, position }`; removing the playing track continues
  at the entry that took its place (ADR-0014).
- Command/event protocol between the engine and any front end, with the wire
  format pinned by test.

**Interface**

- The iced GUI (ADR-0005): first-run screen, album shelf with virtualized
  scrolling and album art, side panel with the track list and an edition
  selector, and a bottom bar with transport, seek groove and now-playing.
- A visual design pass — the "listening room" theme — and a seek groove with a
  click-versus-drag threshold, hover preview and an honest cursor.
- **baz has a typeface.** The IBM Plex superfamily — Sans at Regular, Medium and
  SemiBold, Mono, and Serif — ships inside the binary and is installed as the
  application default. Previously baz asked the *generic* `sans-serif` for
  weights it might not have, and the platform's fallback answered with whatever
  it liked: on one machine every tile title and the first-run question rendered
  in a monospace. The faces are unmodified upstream files under the OFL, with
  provenance and hashes in `crates/baz/assets/fonts/README.md`, and the mono's
  advance width is measured against every reserved slot in the bottom bar so a
  font change can never silently clip a duration.
- **Two inks that failed contrast were corrected.** `PAPER_FAINT`, which carries
  every duration, count, hint and signal note, was 3.4 : 1 on the panel — below
  the WCAG AA floor; `PAPER_MUTED`, the muted fader, was 1.9 : 1, which made the
  position mute exists to restore effectively invisible. Both now clear their
  floors on every surface, and a test computes every ink-on-surface pairing the room can produce, so
  neither can drift back.
- **The lamp is reserved again.** The amber accent means playback truth and was
  being spent on input focus — and the search field takes focus at launch, so
  baz's first frame was an amber ring with no music — on the scanning note, and
  on the first-run wordmark. Focus and text selection are now paper, the scanning
  note is a dim sans sentence rather than an amber figure, and the accent is left
  to the four things that are the playhead plus the one control that creates it.
  A test asserts the accent appears in no other style.
- A **volume control** in the bottom bar: a mute affordance and a fader on the
  same custom groove widget as the seek bar, so it inherits that bar's cursor,
  its hover preview (in dB) and its click-versus-drag threshold. Unity — the
  position at which baz touches not one sample — is reachable by a four-pixel
  snap at the top of the travel and marked by a detent that lights when the
  handle is on it. Drawn in paper ink rather than the accent, because a volume
  is a setting and the lamp means playback truth.
- A signal-path readout in a fixed-width slot beside the fader: the chain
  (`48 → 44.1 kHz`) when the engine is converting, `bit-perfect` when the path
  is literally untouched — a direct chain *and* a transparent volume, which is
  the conjunction ADR-0011 made of ADR-0009's guarantee — and nothing at all in
  between. Same faint ink as the rest of the secondary text, no icon, no fault
  vocabulary, and no layout shift when it appears (ADR-0009 §5).
- **A visible play queue.** What baz handed the engine, in play order, with the
  playing track marked by the same amber lamp dot the shelf gives the playing
  album, the tracks behind it dimmed, and a `3 of 12 · 51:20` count. It shares
  the right-hand rail with the album panel rather than adding a second one —
  the shelf is the interface, and one panel width is the whole budget for
  chrome beside it — so switching between the two reflows nothing.
  Deliberately a *view*: reordering, removal and click-to-jump each need an
  engine command that does not exist, and `player.rs` names exactly which
  rather than faking any of them.
- **Hideable panels**, the half of the v0.1 sketch that was never built. Both
  rail panels carry a ✕, Escape closes whichever is showing, `Q` toggles the
  queue and Ctrl+B dismisses the rail outright and brings back what was
  dismissed. The shelf reflows to the reclaimed width and re-virtualizes at it
  — five columns to three and back, in the shipped window.
- **A settings panel, and baz's first settings surface.** It is a third panel
  in the same right-hand rail as the album panel and the queue — not a popover
  — so it cannot cover the covers or the transport, it inherits the ✕, Escape
  and Ctrl+B that already dismiss a panel, and adding the next setting is a
  section in one scroll rather than a second surface. The rail is the "one
  deliberate layer down" the vision's progressive-disclosure pillar names, and
  this is now the pattern every future setting follows.
- **ReplayGain controls** in it: the three modes as a segmented control, a
  pre-amp and a separate pre-amp for untagged files (half-decibel steps,
  stopping exactly at the engine's ±20 dB), and clipping prevention. Every
  press sends the whole absolute setting and changes nothing on screen until
  `ReplayGainChanged` confirms it, so a clamp or a second front end is
  rendered as the engine's answer rather than as the request. Underneath, what
  the setting came to for the track playing: `-7.75 dB · from this track's
  ReplayGain tag`, and for an unscanned file `0.00 dB · this file carries no
  ReplayGain, so it plays exactly as stored` — a fact, said differently from
  "off", which states no figure at all because the engine performs no
  arithmetic in that mode. Clipping prevention says what it cut and why. No
  alarm colours, no fault vocabulary, and no amber: the lamp stays reserved for
  playback truth (ADR-0013 §8). **The fidelity indicator is unchanged** —
  it is still `VolumePath` plus `SignalChain`, one gain stage and one answer.
- **The ReplayGain setting is remembered** across restarts, in
  `config.toml`'s new `[replaygain]` table. Written from what the engine
  *confirmed*, never from what was asked for.
- **Keyboard control**: space to play/pause, arrows to seek (shifted for 30 s),
  up/down for volume and `M` for mute, `N` or Ctrl+Right for next, `/` or
  Ctrl+F for search, `Q` for the queue, Ctrl+`,` for the settings, Ctrl+B for
  the panels, Escape to back out.
  While the search field has focus no binding is live — baz asks the toolkit
  whether the widget consumed the key and never second-guesses the answer.
- Presentation split into a `views/` module tree, verified pixel-identical
  across six screens before and after the move (ADR-0006).

**Desktop integration (Linux)**

- **MPRIS2**: both interfaces on the session bus, so GNOME's and KDE's media
  controls, the lock screen, `playerctl` and hardware media keys drive baz and
  show title, artist, album and cover art. `Volume` is readable and writable,
  mapped through `baz-core`'s taper in both directions so a lock-screen slider
  and the fader in the window mean the same sound. Position, playback status
  and volume come from engine events only. With no session bus baz prints one
  line and runs exactly as before.
- A desktop entry, and the window's Wayland `app_id` / X11 `WM_CLASS`, so a
  launcher can associate the running window with the entry that started it.

**Distribution**

- Release workflow building Linux x86\_64, Windows x86\_64 and a universal
  macOS binary from a version tag, gated on the full CI suite, with SHA-256
  checksums.
- Flatpak manifest and AppStream metadata under `packaging/flatpak/`, and
  packaging metadata validated on every pull request.
- [`docs/INSTALL.md`](docs/INSTALL.md) and
  [`docs/RELEASING.md`](docs/RELEASING.md).

### Changed

- **`config.toml` is now read and written with the `toml` crate**, replacing
  the hand-rolled single-key writer whose own documentation named this as the
  plan of record once the configuration grew past a couple of keys. It grew
  today. Three crates enter the lock file (`toml`, `serde_spanned`,
  `toml_writer`; the parser and `serde` were already there), all MIT OR
  Apache-2.0. Reading is per-key and defensive rather than derived: a value
  baz cannot understand takes its own default and its neighbours are
  untouched, so a mistyped pre-amp cannot cost anybody their music folder.
  A non-UTF-8 music directory now omits its key instead of preventing the
  whole file from being written.
- Application id is now the reverse-DNS `io.github.mattcree.baz` rather than
  the bare `baz`: the desktop entry's basename, the AppStream component id, the
  Flatpak id, the window's `app_id` and MPRIS's `DesktopEntry` property are one
  string, and Flatpak requires that string to be reverse-DNS. The MPRIS *bus*
  name is unaffected and remains `org.mpris.MediaPlayer2.baz`.

### Removed

- `.opus` no longer appears in the library. Symphonia ships no Opus decoder in
  any released version, and the alternatives cost either a C library on every
  platform or an unmaintained parser on a path that reads hostile input.
  Advertising a file baz cannot play is worse than not listing it; the three
  things that would reverse this are recorded in `docs/BACKLOG.md`.

### Fixed

- **The rail panels' scrollbar no longer covers the duration column.** iced
  draws a `scrollable`'s bar over the right edge of its contents rather than
  beside them, so a track list long enough to scroll clipped every duration by
  its last character — `1:15` read as `1:1`. Both lists now keep a lane clear
  for the bar, reserved whether or not the list currently overflows so that a
  twelfth track does not shunt eleven durations sideways, and the lane and the
  bar are one token so they cannot drift apart. Nothing else about either
  panel moved.
- `.m4a`/`.mp4` files the library listed but could not play.
- `.ogg` files the library listed but could not play (Vorbis, above).
- Format detection now probes file *content* rather than trusting the
  extension, so an Ogg Opus named `.ogg` is identified as Opus instead of
  failing with a complaint about the file, and a FLAC named `.mp3` reads
  correctly. `every_advertised_extension_decodes` asserts that every extension
  the shelf advertises actually decodes real audio, so this class of bug fails
  the build rather than reaching a listener.
- Albums no longer shatter into one shelf entry per credited artist (album
  artist grouping, above).
- Duplicate interleaved track lists for an album held in two formats (editions,
  above).
- The device ring buffer is discarded when a playback session is abandoned,
  instead of leaking the previous session's audio into the next.
- `cargo test --workspace --all-features` no longer plays tones out of the
  developer's speakers. The device-gated tests still open, feed, reopen and
  tear down the real output — they write silence, which every assertion they
  make is indifferent to. The tests that can only be judged by ear (a full
  engine session through real hardware) are now opt-in behind
  `BAZ_DEVICE_TESTS=1`; see `docs/DEVELOPMENT.md`.
- **Windows: opening the audio output a second time in one process no longer
  crashes it** (`STATUS_ACCESS_VIOLATION`). cpal's WASAPI backend caches a
  process-global device enumerator inside the COM apartment of whichever thread
  touched it first, but initialises COM per-thread and calls `CoUninitialize()`
  from a thread-local destructor — so the first thread to exit tore the
  apartment down underneath the still-published global, and the next device
  open dereferenced freed state. baz opens its device on the engine thread
  (cpal streams are not `Send`), so anything that spawns a second engine — an
  output-mode change, a retry after a device error, a front end restarting
  playback — was affected. baz now makes the process's first cpal call from a
  dedicated thread that never exits; see `playback::device`'s "Why cpal is
  first touched from a thread that never exits". This is what had the Windows
  CI job dying part-way through the `baz-core` integration suite.
- The seek bar and the volume fader no longer stay stuck to the pointer after
  a drag that ends outside the window. If the pointer leaves the window, or
  the window loses focus, mid-drag, the gesture now ends there and commits at
  the last position it saw — the release that would normally end it is being
  delivered to somebody else, and neither iced 0.13 nor `winit` offers a
  pointer grab a widget could hold instead.

### Known limitations

- Seeking within Ogg Vorbis loses one lapped block — measured at 1024 frames,
  23.2 ms at 44.1 kHz — because symphonia's Vorbis decoder returns an empty
  buffer for the first packet after a reset. Pinned by a test and backlogged;
  fixing it means changing a seek path five formats share.
- Deleting an entire album folder leaves its rows in the index (see Removal,
  above).
- **No ReplayGain *scanning*.** baz honours the tags a file already has; it
  cannot produce them. A library that has never been through foobar2000,
  `rsgain` or `loudgain` gets no normalisation from baz, and is played exactly
  as stored rather than guessed at (ADR-0013).
- **ReplayGain's clipping check uses the declared *sample* peak**, which is
  what ReplayGain 2.0 scanners write. Inter-sample (true-peak) overshoot after
  reconstruction is not modelled, and there is no limiter: if the gain has to
  be cut, the whole track's gain is cut rather than ridden.
- No playlists, no cue sheets, no watch folders, no tag editing, no application
  icon, and no exclusive-mode output (which is also what puts hardware volume
  out of reach). `docs/BACKLOG.md` is the honest list.

[Unreleased]: https://github.com/mattcree/baz/commits/main
