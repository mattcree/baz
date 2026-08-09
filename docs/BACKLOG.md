# baz — Backlog

> Deliberate deferrals, in one place. Everything here was consciously *not* done,
> with the reason. Roadmap-level scope lives in `VISION.md`; this is the list of
> known gaps and promises. Updated 2026-08-09.

## Product decisions to honour later

- **Shuffle and auto-queueing must prefer the highest-quality edition.** When a
  track exists in several formats (ADR-0007), any automatic selection — library
  shuffle, mood-steered radio, "play something" — picks the best available
  edition, never a random one. The fidelity ranking in ADR-0007 is a
  library-wide policy, not merely the side panel's default. *(Owner, 2026-08-07.)*
- **Per-album edition preference should persist** once the library DB is the
  right home for it (deferred in ADR-0007: persisting today would mean taking a
  TOML-parser dependency for a preference that belongs in a database column).
- ~~**A volume slider**~~ — **shipped, both halves (ADR-0011)**.
  `Command::SetVolume`/`SetMute`, a cubic taper defined once in
  `baz_core::volume`, software gain on the pump path with a 20 ms slew, and a
  structural unity short-circuit that keeps ADR-0009's bit-exactness reachable
  and pinned by test. The GUI control landed after it: a mute affordance and a
  fader in the bottom bar's right-hand end, driven by the same custom groove
  widget as the seek bar; unity is reachable by a 4 px snap at the top of the
  travel and shown by a detent mark that lights when the handle is on it;
  <kbd>↑</kbd>/<kbd>↓</kbd>/<kbd>Ctrl</kbd>+<kbd>M</kbd> on the keyboard (bare
  `M` until type-anywhere took the letters, ADR-0017 §1.2); MPRIS `Volume`
  readable and writable through the same taper. The *device/hardware volume*
  half was investigated and deliberately not built — see below.
  The bit-exactness readout is now the conjunction ADR-0011 defines: the
  bottom bar says `bit-perfect` when the chain is `Direct` **and** the volume
  path is transparent, and says nothing (rather than something apologetic)
  when a volume below unity is scaling the samples — that fact is already on
  screen in the fader beside it.

## Known gaps in shipped features

- **A rare flake in `a_rate_change_is_refused_by_the_bit_perfect_default`**
  (`crates/baz-core/tests/playback.rs`). Observed **once in 13 runs** during a
  full-workspace run with four test binaries competing, on a machine also
  running three build agents. **Not reproduced since**: 12 loaded single-test
  runs and 5 further full-workspace runs, all green. The test asserts the
  *specific* refusal variant, so the likely shapes are a different error
  surfacing first under load, or the session ending before track 1 is reached
  — the 16-sample sink capacity makes producer/consumer ordering tight. Left
  unfixed rather than papered over with a retry or a loosened assertion: CI
  runs `--no-fail-fast`, so a recurrence turns main red with the actual error
  in the log, which is the evidence needed to fix it properly. If it recurs,
  fix the race — do not weaken the assertion.

- **Opus is not played, and therefore not listed.** *(Decided 2026-08-07;
  `.ogg`/Vorbis shipped in the same commit and plays.)* `.opus` is out of
  `AUDIO_EXTENSIONS` and `AudioFormat::is_decodable` returns `false` for it,
  so Opus files — including Opus arriving inside a `.ogg` — do not reach the
  shelf at all. Nothing is silently skipped; there is simply nothing listed.

  **Why not just add a decoder.** Every route costs more than the format is
  worth *today*, and the options were checked rather than assumed:

  - **Symphonia itself has none, in any released version.** 0.5's
    `symphonia-codec-opus` is a 1-byte placeholder and was never published to
    crates.io; **0.6.0 (2026-05-15) still ships no Opus feature**, its README
    codec table lists Opus as `-`, and [issue #8][opus-issue] has been open
    since 2020 with two unmerged WIP PRs. So *upgrading buys nothing for
    Opus* — and 0.6 is a large, unrelated migration in its own right:
    `SampleBuffer` is removed, `AudioBufferRef` becomes
    `GenericAudioBufferRef`, `CodecParameters` splits and loses `n_frames`,
    `delay`, `padding` and `start_ts` to `Track`, and — the one that matters
    most here — **`FormatOptions::enable_gapless` is gone**, replaced by
    negative PTS signalling. Every measured number in `baz_core::playback`
    would have to be re-derived. That is its own ADR and its own commit, not
    a side effect of adding a format.
  - **libopus bindings** (`symphonia-adapter-libopus` → `opusic-sys`, or the
    older `opus`/`audiopus_sys`) work today and are the only *proven* path —
    the adapter is what rodio wires up. The cost is a **C library and a
    `cmake` build dependency on every platform**, which neither this machine
    nor the `baz-dev` toolbox currently has, so it would mean editing
    `scripts/toolbox-setup.sh`, the devcontainer and all three CI runners.
    baz's decode path is pure Rust with **zero system dependencies** today
    (even SQLite is `bundled`); spending that property on one lossy format is
    not a trade worth making unprompted. (`audiopus_sys` additionally links
    libopus *dynamically* on glibc Linux and was last released in 2021.)
  - **Pure-Rust decoders exist but are too young.** `opus-rs` 0.1.26
    (BSD-3-Clause, first released 2026-02) and `opus-decoder` 0.1.1
    (MIT/Apache-2.0, `#![forbid(unsafe_code)]`, claims all 12 RFC 8251
    vectors) would cost no build dependency at all. Both are months old with
    tens of GitHub stars and no maintenance record, and this is a parser
    sitting in front of hostile input from the user's own filesystem —
    exactly where `ENGINEERING.md`'s "prefer proven crates" and the fuzzing
    policy point the other way.

  **What would change the decision**, in preference order: (1) Symphonia
  merges an Opus decoder — then it is a one-line feature flag with no new
  dependency and no build cost, and the container work is *already done*
  (Symphonia's Ogg demuxer parses `OpusHead`, honours the pre-skip and
  derives packet durations from the TOC byte, so gapless Opus would arrive
  working; `opus_bytes_probe_as_ogg_opus_and_never_as_aac` prints the
  pre-skip it already reads); (2) a pure-Rust Opus crate earns a real track
  record — a year of releases, adoption, and the RFC 8251 vectors run in
  *our* CI and fuzzed; (3) the owner decides a bundled-C + `cmake` build
  dependency is acceptable, in which case `symphonia-adapter-libopus` is the
  route. The reversal is small and the tests say so: `AUDIO_EXTENSIONS`
  regains `"opus"`, `AudioFormat::is_decodable` stops excluding it, and the
  probe test's `Ok(_)` arm — which currently fails the build with those
  instructions — goes away.

  [opus-issue]: https://github.com/pdeljanov/Symphonia/issues/8

- **Seeking into a Vorbis stream loses one lapped block** — measured at 1024
  frames (23.2 ms at 44.1 kHz), because Symphonia's Vorbis decoder returns an
  empty buffer for the first packet after a reset and that audio is gone.
  Every other format seeks exactly (WAV/FLAC/ALAC) or time-accurately (MP3).
  The fix is to seek earlier than asked and re-derive the skip from packet
  timestamps, which touches the seek path five working formats share, so it
  is deliberately not bundled with adding the format. Documented per format
  in `playback/mod.rs` and pinned by
  `seek_into_vorbis_ogg_costs_one_lapped_block`.

- **Symphonia 0.6 is available and not taken.** Released 2026-05-15; a large
  breaking migration (see the Opus entry above for the specific API changes)
  that buys baz nothing it currently needs. Worth an ADR when there is a
  reason — video/subtitle support, a codec only 0.6 has, or an upstream fix
  we need — rather than for its own sake.

- **A deleted *directory*'s tracks still linger in the index.** Removal
  landed with ADR-0011 and deleting a *file* now clears its row on the next
  scan — but only under positive confirmation, and one of the four gates is
  "the file's parent directory is present". So `rm -rf ~/Music/Artist/Album`
  leaves eight rows behind, deliberately: from the filesystem's side a
  deleted folder and a mount point that is not mounted right now are the
  same `NotFound` for every path below, and wrongly wiping a present
  listener's library is not a bug worth trading a cosmetic stale row for.

  **What would settle it**, in preference order: (1) a *user-initiated
  prune* — "these 412 rows point at files I cannot find; remove them?" —
  which is the honest home for every case automation should decline, and
  needs a library-maintenance surface baz does not have yet; (2) remembered
  mount points, so "this directory is gone" can be distinguished from "this
  directory's filesystem is not attached". ~~(3) a per-row record of which
  root a track came from~~ — **shipped (ADR-0022)**, and it did replace gate
  2, but it does not touch this case: a deleted album folder and an unmounted
  one are still the same `NotFound` from below whichever root recorded the
  rows. Removing the whole *folder* in the Settings place is now a way out
  that did not exist before, but it is a different act at a different scale.

- ~~**The index has no notion of which root a row came from.**~~ — **closed
  (ADR-0022).** Schema v8 records the root on every row and adds a `roots`
  table, and removal's second gate now reads that record instead of testing
  `starts_with(root_being_scanned)` — which was wrong the moment two roots
  could nest or a file could be reached from both. baz holds an ordered list of
  folders (`config.toml`'s `music_dirs`, migrating a legacy `music_dir`
  silently), each with its track count and last scan in the Settings place,
  and an absent folder now prunes nothing from any root and does not fail the
  pass. Pre-v8 rows are adopted at launch by the front end, which is the one
  place that knows which folder they came from.

  **What remains** is the rootless population: a row under *none* of the
  configured folders is still unprunable by any scan. It is now counted and
  explained rather than invisible (`Library::unrooted_tracks`, and a line in
  the Settings place), and there are two ways out — add the folder back, or
  remove it and let its rows go with it — but the "these 412 rows point at
  files I cannot find; remove them?" prune below is still unbuilt.

- **Removing a music folder loses its tracks' `first_seen_ns`** (ADR-0022 §4).
  Removing a folder forgets its rows outright, so adding it back files every
  album under ADDED = *today*. That is a real loss of the one fact ADR-0019
  built a column and a structural guarantee to protect, accepted because the
  alternative — keeping rows for a folder baz can no longer refresh — is a
  wall of albums nothing can ever correct or remove. A tombstone (remember the
  first-seen for a forgotten root's paths, and restore it if the folder comes
  back) would fix it and is its own small design.
- **Multichannel (>2ch) files are rejected**, not downmixed — a typed error
  rather than silently wrong output. 5.1 downmix is unwritten.
- **Skip and seek are drain-and-restart**, not sample-accurate splices (tens of
  ms of latency, documented in the engine module docs).
- ~~**Bit-perfect is shared-mode only.**~~ — **closed on Linux (ADR-0012)**.
  `baz_core::playback::exclusive::ExclusiveSink` opens an ALSA `hw:` PCM
  directly, with libasound's rate plugin explicitly disabled, so no mixer sits
  between the decoder and the converter. Opt in with
  `BAZ_OUTPUT=exclusive BAZ_OUTPUT_DEVICE=hw:3,0` (or
  `engine::spawn_device_with`), behind the non-default `exclusive-output`
  feature. Reported as `SignalChain::Exclusive { conversion }` on the existing
  `Event::SignalPath`. **Windows and macOS remain outstanding**: WASAPI
  exclusive and `CoreAudio` hog mode each need a per-platform system dependency
  baz does not take yet, and ADR-0012's last section says what each involves.
  The engine side is finished for all three — a backend is one `Sink` impl
  returning `true` from `is_exclusive`.

- ~~**Hardware volume needs exclusive mode**~~ — **shipped on Linux with it
  (ADR-0012)**. All three of ADR-0011's objections vanish when baz holds the
  card: it is no longer shared (nothing else is on that PCM), baz names the
  card it chose, and a card without an attenuator now declines per-device
  rather than the whole platform doing so. The backend drives the card mixer's
  `PCM` element (or `Master`/`Speaker`/`Headphone`, or a USB DAC's own feature
  unit) and leaves the sample stream unscaled. Measured on the ALC897: −51…0 dB
  travel, a −6.02 dB request landing on −6.00 dB. Unity and mute decline on
  purpose — there is nothing to attenuate at one, and only software gain
  reaches exactly zero.

- **`Event::SignalPath` still has no exclusivity *field*.** ADR-0012 reports it
  inside the existing `chain` field instead, because `Event`'s variants are not
  individually `#[non_exhaustive]` and `crates/baz` destructures `SignalPath`
  exhaustively in three places — so a field is a source break, exactly as
  ADR-0011 predicted. The sequencing is unchanged: those destructurings gain a
  `..`, then moving exclusivity onto its own field is additive on the wire and
  mechanical in the code. `SignalChain::is_exclusive()` is the API to use
  either way, so no front end has to care which shape it is.

- **Exclusive mode takes the card, and a desktop usually has it.** Inherent
  rather than a defect: `PipeWire` held the maintainer's own DAC (`hw:3,0`) in
  `RUNNING` state for an entire session with a client stream on it, so every
  exclusive open of it refused — in 50 µs, with `PlaybackError::DeviceBusy`
  naming the device, which is the designed behaviour. What is missing is any
  *help*: a front end can only report it. Options, none built: a "release the
  device" affordance (which means talking to the sound server, and so the
  libpipewire dependency ADR-0011 declined), or simply documenting that
  exclusive mode wants a device the desktop is not routed to.

- **Exclusive mode has no loopback-verified bit-exactness measurement.** No
  device on the maintainer's machine offers a playback loopback, and loading
  `snd-aloop` would have measured the loopback driver rather than a DAC.
  What is asserted instead (ADR-0012): the negotiated rate equals the source
  rate, the negotiated format carries every 16- and 24-bit code exactly — over
  the whole code space, not a sample — and no resampler is constructed. A
  machine with a real digital loopback would close the last inch of this.
- **A converted anchor is decoded whole before first audio.** Reached only when
  the device offers no mode at the source rate; measured at ~2.6 s on a
  5:24 24/48 FLAC (ADR-0009). Streaming the fallback resampler would fix it and
  is deliberately unbuilt — the case is rare and the machinery is not free.
- **The event channel is single-consumer** (`std::sync::mpsc`); a broadcast
  channel is needed before a second front end or a remote transport.
- **FLAC-in-MP4 is labelled ALAC** — lofty exposes no MP4 codec discriminator,
  so bit depth is the proxy. Wrong name, right fidelity tier, vanishingly rare.
- **AAC has no gapless trim** (symphonia limitation) — documented per format in
  `playback/mod.rs` rather than papered over. (Vorbis, added later, *is*
  exactly trimmed: Ogg granule positions are sample counts.)
- ~~**`config.rs` is a hand-rolled single-key TOML writer**~~ — **closed.**
  ReplayGain's persisted setting took the configuration from one key to five,
  which is the growth this entry was waiting for, so `config.rs` now reads and
  writes with the `toml` crate. Three crates entered the lock file (`toml`,
  `serde_spanned`, `toml_writer` — the parser and `serde` were already in the
  graph), all on the existing licence allowlist. Reading stays defensive and
  **per key**: a value baz cannot understand takes its own default and leaves
  its neighbours alone, because a `#[derive(Deserialize)]` would fail the whole
  document over a mistyped pre-amp and cost a listener their music folder.

## Interface

- **A serious UX pass with expert guidance** — the current look is deliberate
  but scaffolding-grade (ADR-0006 exists to make replacing it cheap). Vetted
  community design skills to be shortlisted and owner-approved first.
- **Light theme variant** — the palette is dark-first; tokens are in place, the
  light values are not.
- **No readout for the *direct* signal path** — the bottom bar shows the chain
  only when the engine is converting (ADR-0009 §5, deliberately). The listener
  who wants to *confirm* 24/96 is reaching the device untouched has only the
  `[playback] signal path:` stdout line; the proper home for that, with
  `EngineHandle::conversions()` alongside it, is a diagnostics view.
- **Transport buttons take no keyboard focus and publish no accessibility tree**
  — iced 0.13 offers neither (no AccessKit). Tooltips and 32 px hit targets are
  the whole of what the toolkit currently allows.
- ~~**No settings surface at all**~~ — **shipped, and it is now the pattern.**
  The rail holds a third panel: one heading, one sentence per section, the
  controls, and a readout where the engine has something to say about the here
  and now. ReplayGain is the first section; the next setting is another block
  in the same scroll rather than a new surface. Why a rail panel rather than a
  gear popover — the progressive-disclosure layer already exists, it cannot
  cover the covers or the transport, and it inherits three dismissals iced 0.13
  gives no primitive for — is argued in `panels.rs`.
- **Settings that are not yet settable.** The place has two sections now —
  Playback and Library (ADR-0022) — and the second one cost exactly what the
  first one promised it would: an entry in `SECTIONS`, a block in the same
  scroll, and an `on_press` to make the spine a real control. Still off-screen:
  the output device, the exclusive-mode selection
  (`BAZ_OUTPUT`/`BAZ_OUTPUT_DEVICE` are still environment variables, ADR-0012),
  the boundary policy, and the enrichment toggles. Each is a section, not a
  design question.
- ~~**Music folders are typed, not picked.**~~ — **shipped** (ADR-0025). The
  add-a-folder row now carries `Browse…`, the desktop's own picker through the
  XDG portal (`rfd` 0.17, portal-only: one new crate on Linux, no gtk, deny
  green). The text well stays beside it, load-bearing rather than legacy: a
  dialog cannot name an unmounted share, and every act keeps a visible pointer
  target when no portal service is running. The first-run screen still asks
  for a typed path only.
- **Music folders cannot be reordered in the interface.** The order is data
  (scan order, list order, and the order a nested pair is resolved in) and
  `config.toml` is editable by hand, but a drag handle is a control with its own
  design.
- ~~**Panel hiding**~~ — **shipped.** The right-hand rail holds one panel at a
  time (album, queue or settings), each carries a ✕, Escape closes what is
  showing, `Q` toggles the queue, <kbd>Ctrl</kbd>+<kbd>,</kbd> the settings,
  and <kbd>Ctrl</kbd>+<kbd>B</kbd> dismisses the rail and
  brings back what it dismissed. The shelf reflows to the reclaimed width and
  re-virtualizes at it. The state machine is `crates/baz/src/panels.rs` — pure,
  iced-free and unit-tested, per ADR-0006 layer 1.
  **Visibility is deliberately not persisted**: every panel is contents-driven
  and none's contents survive a restart in a way that would make reopening it
  useful (the album panel needs a selection, which is session state; the queue
  lives in the engine process and is never re-sent at launch; the settings are
  a place you go and then leave), so a remembered "open" would cost the shelf
  340 px on every launch to display the words *Nothing queued*. That the
  settings panel's *contents* are now persisted does not change this — full
  argument in `panels.rs`.
- **Layout flexibility beyond hiding** — panels can be dismissed, not moved,
  resized, or replaced. foobar-style layout editing is a later chapter by
  design (VISION pillar 6); a resizable rail is the plausible next step and
  wants the config-file question answered first.
- **The queue is a view, not a control.** — **the engine half is closed
  (ADR-0014); the panel half is not.** `Command::JumpTo { position }` plays the
  entry it names, and `Command::UpdateQueue { paths }` removes, reorders and
  appends without stopping the music (an edit that misses the playing track
  disturbs no delivered sample). `Event::QueueChanged { len, position }` carries
  the engine's re-derived playing row. What remains is the panel itself: rows
  already carry their queue position, and `PlayerState::queue_list` is the one
  place that gains a message.
- ~~**The queue cannot be built from a record.**~~ — **closed by the
  picker's Queue row** (doc 09 §8.1; ADR-0023's accepted amendment): `Add
  to…` on the record's page, or a track row's `+`, then the picker's first
  row — `UpdateQueue` with rows appended, the music undisturbed, two presses
  inside W8's band-C budget. The dedicated `Queue album` control is
  **withdrawn before being built** (a second control sending the picker
  row's message — L8.6); what remains open is its one-press accelerators,
  shift-click and the context menu's `Queue` item (doc 09 §13 steps 4 and
  7), both resolving to the picker's Queue row as their on-screen control.
- **Playlist reorder and add have no drag** (ADR-0024 §6 layer 3, deliberately
  last). iced 0.13 has no pointer capture, so the drag needs the hand-built
  widget on the `groove.rs` precedent — one investment paying playlist add,
  playlist reorder and queue reorder alike. Until it lands the ▲▼ steppers and
  the two-press adds are the routes, which is the ADR's own shipping order.
- **A missing playlist entry cannot be repaired in place.** ADR-0024 §3
  specifies the surface — candidate matches (same filename under a current
  root) proposed per entry, confirmed by the user, the confirmation being the
  only thing that writes the file — and the page today only counts and shows
  the broken path. Repair by hand (edit the file; the page re-reads) works
  meanwhile.
- **The playlists folder is not yet shown in Settings → Library** beside the
  roots (ADR-0024 §2's sovereignty line). One row and an open-folder
  affordance, so the user learns where their artefacts live the way they
  learn where their music does.
- **Where playlists sit in the information hierarchy** — **answered by the
  implicit-playlists study (design doc 09)**: one kind of list, one sounding
  and unnamed, one transfer gesture. Steps 1–3 of its §13 are shipped (the
  armed collecting mode removed; the picker's Queue row, the hoisted playing
  list, playing provenance; the Songs section over the wall). What remains
  is its steps 4–8, each shipping whole: the context-menu mirror layer
  (§5.2 — and with it S4's two-gesture *"add to the current playlist"* from
  anywhere), queue-place edit parity (▲▼ and the `+` slot), `Play all`
  (gated at large scale on queue-place virtualization, §7.1), shift-click,
  and the drag. Wall membership, rail sorting and search-corpus membership
  for playlists stay deferred (ADR-0024 §A2); the sleeve (§A1) is the
  vocabulary any outcome keeps.
- **The Songs section answers the pointer with a press and a door, and
  nothing else yet.** Until step 4, its rows follow the product's current
  interaction classes exactly: no gesture-only routes, no menus anywhere;
  the right-click mirror menu is §5.2 (step 4), and shift-click as the
  queue-append accelerator is step 7, resolving to the picker's Queue row.
- **No keyboard route out of the search field.** Transport keys are bound
  (`crates/baz/src/keys.rs`), but iced 0.13's `text_input` captures every key
  press while focused except Tab and the vertical arrows, so while the search
  well has focus *nothing* is a shortcut — the field takes the key and the
  subscription never sees it. Escape blurs it, which is the whole of the
  escape hatch today. A proper fix wants a focus-aware shell (or a toolkit
  that reports focus synchronously), which is the same missing capability as
  the accessibility gap above.
- **No shortcut discovery in the interface.** The bindings are in the README
  and nowhere the user can see them while running — no `?` overlay, no menu.

## Platform integration

- **No application icon.** `packaging/baz.desktop` therefore carries no
  `Icon=` key (a key naming a file no package installs is worse than none),
  and desktops fall back to a generic launcher icon. Add the key in the same
  change that adds the artwork; `crates/baz/src/icon.rs` is the in-UI transport
  glyph sheet, not an app icon.
- **`OpenUri` is not implemented**, so MPRIS's `SupportedUriSchemes` and
  `SupportedMimeTypes` are empty and the desktop entry registers no
  `MimeType=`. baz plays what it scanned; "open this file with baz" is a real
  feature (queue-a-path, plus a `%U`-aware `Exec=`) rather than a property, and
  advertising schemes we would refuse is the kind of small lie the honesty rule
  rules out.
- **MPRIS `Previous` is a documented no-op** and `CanGoPrevious` is `false` —
  **but the engine half now exists**. `Command::Previous` restarts the current
  track past `baz_core::engine::PREVIOUS_RESTART_MS` (3 000 ms) and steps back
  a queue position before it, restarting at the head; it resumes when paused,
  exactly as `Next` does. All that is left is the front-end wiring: send the
  command, and advertise `CanGoPrevious = true` whenever a queue is playing —
  unlike `Next` at the end of a queue, `Previous` has no position at which it
  does nothing.
- **No MPRIS `TrackList` or `Playlists` interface** (`HasTrackList` is
  `false`), and no `LoopStatus`/`Shuffle` — baz has neither loop nor shuffle
  yet, so they are absent rather than present-and-fixed.
- **`Rate` and `Volume` are read-only `1.0`.** baz has no rate control
  (ADR-0009: it plays at the source rate) and no volume control at all; a
  writable property that discarded writes would be worse than an error.
- **Windows/macOS media-key and now-playing integration** — untouched. The
  `Media*` key names are bound in `keys.rs`, which covers a focused window;
  SMTC (Windows) and `MPNowPlayingInfoCenter` (macOS) are not.

## ReplayGain

- ~~**No ReplayGain at all.**~~ — **closed for the reading half (ADR-0013).**
  baz honours the `REPLAYGAIN_*` figures files already carry, in off / track /
  album modes with a pre-amp and clipping prevention, applied through the same
  gain stage as the volume and reported through the same `VolumePath`.
  ~~The controls are unbuilt~~ — **also closed**: the settings panel carries
  the modes, both pre-amps and clipping prevention, and a readout that renders
  `applied_centidb` and explains the `source` (`no_tag` reads as a fact about
  the file, `disabled` states no figure at all). It is remembered across
  restarts in `config.toml`'s `[replaygain]` table.
- ~~**No ReplayGain *scanning*.**~~ — **closed (ADR-0015).** baz computes the
  figures for files that carry none: an EBU R128 / BS.1770-4 gated integrated
  loudness meter (`baz_core::loudness`, validated against the EBU Tech 3341
  compliance signals inside the ±0.1 LU the specification states — worst
  measured error 0.0241 LU) driven by a cancellable, resumable background pass
  over the library (`baz_core::analysis`), stored in schema v6's own columns and
  reported through the `ReplayGainSource` vocabulary as `computed_*` so a
  listener can tell a measurement from a tag. Tags still win, field by field.
  ~~The controls are unbuilt~~ — still true: the pass is reachable through
  `AnalysisCommand`, and the UI for it is a parallel unit.
- **baz still does not write ReplayGain into music files.** The figures it
  measures live in its own index, so another player will not see them.
  Writing them means a backup story, a dry run, and an answer for a file that
  is read-only or on a share that lies about being writable — its own unit,
  and the first time baz would ever modify a listener's music.
- **The clipping check trusts a *sample* peak** — the declared one where a file
  has it, and baz's own measurement where it does not. That is what
  ReplayGain 2.0 scanners write and what the tags can support; inter-sample
  (true-peak) overshoot after reconstruction is not modelled, and there is no
  limiter riding the gain. True peak means BS.1770-4 Annex 2's four-times
  oversampling filter **and its own compliance vectors** — shipping the first
  without the second would be the unverified number ADR-0015 exists to rule out.
- **No momentary or short-term loudness meter, and no loudness range.**
  ReplayGain needs the integrated figure and nothing else; the others are a
  meter's features rather than a normaliser's, and EBU Tech 3341's cases 7–9
  would come with them.
- **An analysis pass hydrates a second in-RAM index** (its worker opens the
  library on its own SQLite connection, which WAL makes safe). On a 100k
  library that is real memory for the duration of the service. A lighter
  read-only accessor would fix it; the current shape is not wrong, only
  generous.
- **A file with an album gain but no track gain is treated as untagged in track
  mode.** Deliberate (ADR-0013 §3) and vanishingly rare; noted so the asymmetry
  is a decision on record rather than an oversight.

## Bigger chapters (see `VISION.md` staging)

ReplayGain scanning, cue sheets, batch tag editing, exclusive outputs, bliss-rs
analysis and mood-steered shuffle, the opt-in enrichment pane, scrobbling,
OpenSubsonic client mode, and the paid-parity hit-list in
`research/06-paid-product-teardown.md`.

**Watch folders left this list with a `no`, not a tick** (ADR-0022 §7). baz
holds several folders and rescans them every five minutes while it runs, and
`notify` was evaluated and rejected: inotify is per-directory and capped
(8 192 watches on many distributions, shared with the whole desktop), network
mounts emit no events at all, and `ReadDirectoryChangesW` drops events during
exactly the bulk copy a listener most wants to see. A watcher would therefore
need the periodic pass behind it anyway — the fallback is the whole feature —
and the warm pass costs ~100 ms on a 100k library. What would reverse it: a
measurement showing the periodic pass is too slow on a real large library, in
which case a watcher is an optimisation with a stated fallback rather than the
mechanism.
