# baz — Backlog

> Deliberate deferrals, in one place. Everything here was consciously *not* done,
> with the reason. Roadmap-level scope lives in `VISION.md`; this is the list of
> known gaps and promises. Updated 2026-08-07.

## Product decisions to honour later

- **Shuffle and auto-queueing must prefer the highest-quality edition.** When a
  track exists in several formats (ADR-0007), any automatic selection — library
  shuffle, mood-steered radio, "play something" — picks the best available
  edition, never a random one. The fidelity ranking in ADR-0007 is a
  library-wide policy, not merely the side panel's default. *(Owner, 2026-08-07.)*
- **Per-album edition preference should persist** once the library DB is the
  right home for it (deferred in ADR-0007: persisting today would mean taking a
  TOML-parser dependency for a preference that belongs in a database column).

## Known gaps in shipped features

- **`.ogg` and `.opus` are scanned but unplayable** — the same
  advertises-what-it-can't-play bug that m4a had. Vorbis is a one-feature fix
  (`ogg` + `vorbis`); **Opus has no decoder in symphonia 0.5 at all**, so it
  needs a real decision (an external decoder crate, or dropping the extension
  until one exists). Until fixed, these files appear on the shelf and skip.

- **Deleted files linger in the index** — `add_tracks` is upsert-only; removal
  support has not been written, so a file deleted on disk stays on the shelf.
- **A full rescan runs on every launch** — cheap on small libraries, wasteful on
  large ones; incremental scanning by mtime is the fix.
- **Multichannel (>2ch) files are rejected**, not downmixed — a typed error
  rather than silently wrong output. 5.1 downmix is unwritten.
- **Skip and seek are drain-and-restart**, not sample-accurate splices (tens of
  ms of latency, documented in the engine module docs).
- **Bit-perfect is shared-mode only.** Following the source rate and reopening
  on a change is implemented and is now the default (ADR-0009): baz converts
  nothing. What is still outstanding is the last hop — the system mixer may
  resample downstream of us, and only exclusive-mode backends (ALSA `hw:`,
  WASAPI exclusive, `CoreAudio` hog) can close that. `Event::SignalPath` will
  grow a field for it when they land.
- **A converted anchor is decoded whole before first audio.** Reached only when
  the device offers no mode at the source rate; measured at ~2.6 s on a
  5:24 24/48 FLAC (ADR-0009). Streaming the fallback resampler would fix it and
  is deliberately unbuilt — the case is rare and the machinery is not free.
- **The event channel is single-consumer** (`std::sync::mpsc`); a broadcast
  channel is needed before a second front end or a remote transport.
- **FLAC-in-MP4 is labelled ALAC** — lofty exposes no MP4 codec discriminator,
  so bit depth is the proxy. Wrong name, right fidelity tier, vanishingly rare.
- **AAC has no gapless trim** (symphonia limitation) — documented per format in
  `playback/mod.rs` rather than papered over.
- **`config.rs` is a hand-rolled single-key TOML writer** — adopt the `toml`
  crate when configuration grows beyond a couple of keys.

## Interface

- **A serious UX pass with expert guidance** — the current look is deliberate
  but scaffolding-grade (ADR-0006 exists to make replacing it cheap). Vetted
  community design skills to be shortlisted and owner-approved first.
- **Light theme variant** — the palette is dark-first; tokens are in place, the
  light values are not.
- **Split `app.rs` into a `views/` module tree** — ADR-0006 mandates this at the
  next substantial UI change; deliberately deferred during the transport-icon
  work so a behaviour-sensitive diff wasn't buried in file moves. `app.rs` is
  ~1490 lines. Book it as its own commit.
- **Transport buttons take no keyboard focus and publish no accessibility tree**
  — iced 0.13 offers neither (no AccessKit). Tooltips and 32 px hit targets are
  the whole of what the toolkit currently allows.
- **Panel hiding / layout flexibility** — the v0.1 sketch promised a fixed
  layout *with hideable panels*; hiding is unwritten.
- **Keyboard control beyond Escape** — no transport keybindings yet, which also
  makes GUI automation impossible for testing.

## Platform integration

- **MPRIS + media keys** (Linux) — in the v0.1 scope sketch, deferred.
- **Windows/macOS media-key and now-playing integration** — untouched.

## Bigger chapters (see `VISION.md` staging)

ReplayGain scanning, cue sheets, watch folders, batch tag editing, exclusive
outputs, bliss-rs analysis and mood-steered shuffle, the opt-in enrichment pane,
scrobbling, OpenSubsonic client mode, and the paid-parity hit-list in
`research/06-paid-product-teardown.md`.
