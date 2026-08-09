# baz — Backlog

> Deliberate deferrals, in one place. Everything here was consciously *not* done,
> with the reason. Roadmap-level scope lives in `VISION.md`; this is the list of
> known gaps and promises. Updated 2026-08-09.

## Product decisions to honour later

- **The lane and the panel both list playlists**, and that is an accepted
  transitional state rather than a design (ADR-0030's amendment). The panel
  cannot go while it is the picker for `Add to…` — ADR-0031's card at the
  pointer is unbuilt — and the owner has since said the panel *"might be
  alright for keeps"*. What is open is the **division of labour**, not the
  panel's existence: today the lane is the index (resident, complete, ordered
  by touch) and the panel is the picker and the workshop (create, rename,
  delete, drop-target). If the panel keeps its place, its list of names is the
  duplicate and the argument for removing *that* is L8.6's; if it goes, its
  three jobs move to the card and the lane. **Owner decision, not an agent's.**
- **`Resume` restores the run silently rather than paused.** ADR-0023 §6 says
  *paused*; the engine's command table makes "loaded and paused at a non-zero
  cursor" unrepresentable without an engine change, which §6 costed at zero
  (`crate::session` argues it). If the engine ever gains a command that selects
  a queue position **without** starting playback, the restore becomes two
  commands instead of one press and the transport reads *paused* on launch.
  That is the one change that would reopen this.
- **The two-line strip split could move from 960 to 872.** Removing the
  `Playlists` door freed 88 px and the single line now fits at 872, but
  `TOP_BAR_SPLIT` was left at 960: lowering it changes when the strip splits
  for every user, which is a visible behaviour change and not a consequence of
  this work. The arithmetic is pinned either way
  (`the_strip_holds_its_tenants_at_the_single_line_floor`).

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

- **The density cache still decodes one size for three steps.** `02` §2.7
  prices it: at `Dense` the LRU holds 320² thumbnails for ~200 px tiles —
  2.5× the pixels needed. The density-aware decode size stays deliberately
  untaken (it would make the cache's contents depend on the setting, which
  means invalidating the whole cache on a step change), and ADR-0028's
  visible detents make the step easier to reach without changing that
  arithmetic. What would reverse it: a measured decode-latency or memory
  problem on a real large library at `Dense`.

- **A rare flake in `the_play_recorded_event_follows_the_line_into_the_file`**
  (`crates/baz-core/tests/history.rs:125`), **Windows only, observed once** —
  2026-08-09, CI run 31331470261 on `bcbba7f`. It timed out waiting for an
  event after the full `EVENT_TIMEOUT` of **20 s**, which is long enough that
  a merely slow runner is an uncomfortable explanation. Re-running the same
  job on the same commit passed, and the commit that surfaced it **touched no
  `baz-core` file at all** (a GUI-only change), so the ledger's write path was
  not modified by anything nearby.

  Left unfixed rather than papered over, on the same terms as the flake
  below: **do not raise the timeout** — 20 s is already generous, and a longer
  one would only make the next occurrence slower to learn from. The suspects
  worth checking first are the ledger's writer thread and its shutdown
  handshake (`finish()` waits for every queued line), where a Windows file
  handle or a join that never returns would present exactly as this does. A
  recurrence turns main red with the log, which is the evidence needed.

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
  target when no portal service is running. ~~The first-run screen still asks
  for a typed path only.~~ — **shipped** (doc 11 §5 P1): the first-run screen
  now carries the same `Browse…` beside its field, checks the typed path on
  the blocking pool, and takes a dropped folder where the toolkit delivers
  drops (X11 only; winit 0.30's Wayland backend has none — recorded in
  ADR-0025 §3's superseded clause).
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
- ~~**The queue is a view, not a control.**~~ — **closed end to end.** The
  engine half closed first (ADR-0014): `Command::JumpTo { position }` plays
  the entry it names, and `Command::UpdateQueue { paths }` removes, reorders
  and appends without stopping the music (an edit that misses the playing
  track disturbs no delivered sample), with `Event::QueueChanged` carrying
  the engine's re-derived playing row. The surface half followed piecewise
  and finished with doc 09 §13 step 5: a row click jumps, the ✕ removes,
  the ▲▼ steppers reorder (`queue_edit::shifted`, the cursor following its
  track), and the `+` transfers toward the picker — the queue place and the
  playlist page are one editor (09 §8.2). The drag shipped last, closing the
  step (its own entry below).
- ~~**The queue cannot be built from a record.**~~ — **closed by the
  picker's Queue row** (doc 09 §8.1; ADR-0023's accepted amendment): `Add
  to…` on the record's page, or a track row's `+`, then the picker's first
  row — `UpdateQueue` with rows appended, the music undisturbed, two presses
  inside W8's band-C budget. The dedicated `Queue album` control is
  **withdrawn before being built** (a second control sending the picker
  row's message — L8.6). Its one-press accelerators resolve to the picker's
  Queue row as their on-screen control, and both shipped: **shift-click**
  (doc 09 §13 step 7 — shift turns *open the record* into *queue the
  record*, nothing sounding unasked) and **the context menu's `Queue` item**
  (step 4 — the mirror layer's presses are the `+` then the picker's Queue
  row, made for you).
- ~~**Playlist reorder and add have no drag**~~ (ADR-0024 §6 layer 3,
  deliberately last; resequenced by doc 11 P5) — **shipped**, as the
  hand-built widget on the `groove.rs` precedent (`crates/baz/src/drag.rs`):
  one investment paying all three surfaces — queue reorder, playlist
  reorder, and drag-to-add onto the standing panel's rows. Press a row past
  the 8 px threshold and an insertion line rides the boundaries; release
  commits one whole-list `UpdateQueue` or one atomic file save; a drop on a
  panel row appends to that file. Esc discards; `CursorLeft`/`Unfocused`
  commit (the groove's capture lessons, inherited and pinned by tests). The
  ▲▼ steppers, the `+` and the picker remain the visible routes — the drag
  is sugar, exactly as the ADR ordered it. Captures at
  `docs/design/impl/drag/`.
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
  and unnamed, one transfer gesture. Steps 1–7 of its §13 are shipped (the
  armed collecting mode removed; the picker's Queue row, the hoisted
  playing list, playing provenance; the Songs section over the wall;
  the context-menu mirror layer of §5.2 — and with it S4's two-gesture
  *"add to the current playlist"* from anywhere, right-click the bar and
  press the item; queue-place edit parity — ▲▼, the `+` slot on the queue's
  and the playlist page's rows alike, and the place's virtualization;
  `Play all` in the Library strip; shift-click as the queue-append
  accelerator). **Step 8 — the drag — shipped whole** (its own entry
  above), which closes §13: all eight steps are on screen.
  Wall membership, rail sorting and search-corpus membership
  for playlists stay deferred (ADR-0024 §A2); the sleeve (§A1) is the
  vocabulary any outcome keeps.
- **The settings steppers' marks do not ride the transport's hover tween.**
  Doc 10 §7 step 6 swapped their font `−`/`+` for the drawn glyph pair at
  the resting ink; the row-slot glyphs draw at the hovered weight because
  they exist only under the pointer, but the steppers stand at rest, and
  brightening their marks on hover would need two more `motion::Control`
  identities and the `mouse_area` wiring the transport carries. The button
  ground answers hover meanwhile, which is what every word control gets;
  wire the ink if the steppers ever read as dead.
- **The strip's split regime never hosts a third line** (doc 10 §8, stated
  so a future proposal meets the reason): a tenant that does not fit the L9
  budget re-homes by subject (doc 07's L8) or displaces an argued
  incumbent — the budget law's answer is re-homing, not accretion. The
  Marquee lens's switcher form (ADR-0017 step 18) is likewise left to its
  own design: `WALL · MARQUEE` will be a state row in the state row's
  vocabulary, and nothing shipped pre-empts its keys.
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

## "Feels like treacle when I resize" — measured, not reproduced

Reported 2026-08-09. **It is not caused by the hover options, the bar cover or
the wall's scrollbar**: an A/B of this branch against `main`, same harness, same
fixture, driving 60 window resizes at ~30 Hz, gives the same numbers to within
a millisecond —

| | frames drawn | median gap | p90 | max |
|---|---|---|---|---|
| this branch | 118 | 7 ms | 31 ms | 33 ms |
| `main` | 118 | 8 ms | 32 ms | 33 ms |

and repeating it against a fixture whose covers are 3000 × 3000 JPEGs (rather
than the harness's 600 px ones) changes nothing either: 118 frames, 7 ms
median, 35 ms max.

**So the harness does not reproduce it**, and that is the finding rather than a
failure to find one. Three things differ between it and the owner's machine,
and they are the three places to look:

1. **Programmatic resize is not drag resize.** `xdotool windowsize` delivers
   discrete size changes; a real drag on a compositor delivers a continuous
   stream of `configure` events, each of which makes wgpu **reconfigure the
   surface**. Swapchain recreation per frame is the classic iced/wgpu resize
   jank on Linux, and nothing in baz would show it under Xvfb, which has no
   GPU and falls back to `tiny-skia`.
2. **Library size.** The harness has 25 albums.
3. **Present mode.** `iced_wgpu` reads `ICED_PRESENT_MODE`
   (`iced_wgpu-0.13.5/src/settings.rs:67-79`) and otherwise takes its default.

Two commands bisect it in under a minute, and they should be run before
anything is changed:

```sh
ICED_BACKEND=tiny-skia baz    # smooth here => it is wgpu surface reconfiguration
                              # treacle here too => it is baz's own layout
ICED_PRESENT_MODE=immediate baz   # smooth here => it is vsync/swapchain
```

**Measured since, and partly fixed.** `BAZ_MSG_LOG=1` (new, see
`docs/DEVELOPMENT.md`) says **87 messages a second** under a dragged edge:
three per resize step — `WindowResized`, then `Scrolled` twice. Idle is
silent. Two of the three were doing the full thumbnail scan the first had just
done, and `request_visible_thumbs` now guards on the visible album range, so
they cost a comparison. The **message count is structural** and unchanged: the
second `Scrolled` is iced republishing a viewport whose `content_bounds` moved,
which is true and worth being told. What is left to find is why three cheap
messages a step feel like treacle on the owner's machine and not in the
harness — run the meter there.

**One thing worth fixing regardless of the outcome**, found while looking:
`Message::WindowResized` calls `request_visible_thumbs()` on **every** resize
event (`app.rs:3759-3772`), and `art::load_thumb` (`art.rs:131-139`) does a
*full-resolution* decode — `image::open` on a 3000 × 3000 cover is ~9 M pixels
— before downscaling to `THUMB_PX` 320. `spawn_blocking`'s pool is 512 threads
by default, so widening the window into unseen albums can start dozens of
full-resolution JPEG decodes at once, all competing with the thread trying to
draw the resize. It is deduped by `pending`/`no_art` so it only bites on first
sight of an album, which is exactly when a listener is dragging the window to
see more of the wall. A debounce on the resize path — request thumbs when the
size *settles*, not on every configure — costs nothing and removes the burst.

## The strip demolition — four removals the owner asked for

2026-08-09: *"I think the pull option will just disappear, and so will the
shuffle. Shuffle is the sort of thing I expect to be at the playlist level. As
in if we're currently playing a playlist, I can toggle it on or off. The play
all thing also does not need to exist. That should be existing as a kind of
playlist that is implicit."* Plus, from the same brief, the `Queue` door
leaving the bar as the now-playing block becomes the route to what is playing.

None of these is blocked — the ledger binds contributors and agents, not the
owner. What follows is the **cost**, mapped before anything is touched, because
four source-scanning tests (`app.rs:5381`, `:5439`, `:5601`,
`theme.rs:6531`) assert against literal function names and **panic** rather
than fail if the function is gone. Every edit below must land with its test
rewritten in the same commit.

### 1. `Pull` — self-contained, do it first

Touches `app.rs` (message `:523`, arm `:1120`, `draw_pull` `:2741-2790`,
`Shelf::pull` `:3589`, `struct Pull` `:3595-3626`, the Escape peel `:4008`),
`keys.rs` (`Ctrl+R` at `:418`), `top_bar.rs:276-280`, `album.rs:299-345`
(the `The pull · Last played 3 years ago` line), `shuffle.rs:190-314` (~125
lines of pure code), `font.rs:519`. `baz-core`'s `History::pull_weight` and
`PULL_NEVER_WEIGHT` become dead.

It shares exactly one thing with shuffle — `shuffle::Pool::from_wall` — and
owns no engine state, no persisted state, no queue: it sends **no command at
all**, it navigates. Tests to rewrite: four in `shuffle.rs:639-732`,
`app.rs:5381` (which also asserts the shuffle half — items 1 and 2 collide
inside one test), `app.rs:5601` (the ordered Escape triple),
`app.rs:5190`'s `CONTROLS: [_; 22]`.

**It also closes doc 11 P9**, which is an open question addressed to the owner
— *"`Pull`: explain it or rename it · present-to-owner"* — by answering it
with removal. That verdict wants writing down, not just deleting.

### 2. `Shuffle` as a playlist-level toggle — three real questions first

**This turns an act into a mode**, and four places in the product are built on
it being an act:

- `docs/adr/0023-playback-model.md:73-74`: the engine's queue has *"no shuffle
  flag, no repeat flag and no continuation policy"*, and `:193` keeps both as
  *"front-end expressions over `SetQueue`/`UpdateQueue`"*.
- **ADR-0024 §1 honesty clause 1 is the direct blocker** (`:115-116`): *"The
  playlist a user edits is exactly what plays — entries, order, verbatim; **no
  shuffle-on-play**, no dedup, no silent skipping."*
- Doc 10 §3.2 refuses the crossed-arrows glyph *specifically* because it is
  *"a mode toggle with a lit state"* and baz's shuffle *"is an act"*. Reversing
  the semantics **un-blocks the glyph**; the two decisions are joined.
- `REFUSALS.md`'s *no invisible shuffle pools* is satisfiable today only
  because the pool is the wall and the wall can mark itself — dimmed covers
  plus rings (`shelf.rs:885-935`). **A playlist is not a wall**, and
  `views/playlist.rs` has no equivalent marking, so the visibility mitigation
  disappears with the surface.

Three questions only the owner can answer:

1. **What does toggling *off* restore?** Playing a playlist *copies* it and
   decouples (ADR-0024 §1, `:108-111`); nothing keeps the pre-shuffle order.
   Off would need either a re-read of the `.m3u8` (writing back into a
   decoupled run) or a new `original_order` field on `QueueVm` — which is the
   *"live context object that keeps acting after the gesture"* ADR-0023 §1
   refuses at `:97-99`.
2. **What is "currently playing a playlist"?** It is
   `QueueVm.provenance: Option<String>` — and doc 09 `:620` calls provenance
   *"a statement about **origin**, never a live link"*. A toggle keyed to it
   makes it a live link.
3. **How does the pool stay visible** on a surface that cannot dim covers?

### 3. `Play all` → an implicit playlist — the vocabulary exists, the type does not

Doc 09 §2 **already lists the wall as an implicit playlist** (`:130`): *"| The
wall, in its arrangement | the group key and the filter | by arranging | no |
the wall itself |"*, and `:148` states the model — *"baz has one kind of list.
One of them is sounding and has no name; the rest are named and silent."*

But **"implicit playlist" is design vocabulary, not a type**: `grep -rn
"implicit playlist" crates/` returns one comment (`vm.rs:912`). There is no
`Playlist` abstraction a non-file list can inhabit — `playlists.rs` is entirely
folder-and-`.m3u8`-backed, and `Place::Playlist(u64)` hashes a *filename*
because ADR-0024 §2 makes filename = name. ADR-0024 §1 defines a playlist as
*"stored in a file that person owns"* (`:103`), so the implicit list is not one
under the ADR's own definition; that sentence needs amending under the editing
rule.

Two traps:

- **Giving the wall's run provenance** immediately makes the picker offer
  *Add to "Everything"*, which has no file to write to (`menu.rs:662` pins the
  coupling).
- **The wall's order is not stored** — it is recomputed from `group_key` +
  `query` every frame. An implicit list that is a *place* re-derives on every
  visit; a *snapshot* is the silently-re-deriving pool problem in a new coat
  (`shuffle.rs:86-89`).

`Play all`'s scope rule must survive whatever shape it takes
(`REFUSALS.md:43-47`): *"its scope is exactly what the wall shows, in the
wall's own order — playing what you cannot see is refused."*

### 4. The `Queue` door leaves the bar — the ratchet's escape hatch does not quite fit

The ratchet (`REFUSALS.md:137-142`) permits exactly one removal: *"Replacing a
slot with a **better statement of the same fact**."* The door's fact is **how
much is left** (`queue_size_note`, `player.rs:1663`). A now-playing block that
opens the current playlist states **where the run came from**. Doc 09 §6
(`:632`) treats those as separate readouts (`Road Trip · 3 of 12 · 38:12
left`). So the replacement either carries the count too, or this is a removal
rather than a replacement and the entry gets rewritten rather than satisfied.

Two more collisions:

- **`Ctrl+U` would become keyboard-only.** `keys.rs:401` binds it to
  `ToggleQueue`, and `app.rs:5187` exists to make keyboard-only actions
  impossible — its own doc says *"There are no exceptions left."*
- **The now-playing block already means something else.** It presses
  `ShowPlayingAlbum` → the record's **page** (`bottom_bar.rs:465`, tooltip
  *"Go to the record that is playing"*, mirrored by `menu.rs:303`'s *"Go to
  record"*). `bottom_bar.rs:452-457` argues explicitly that this block must
  **not** have a lit state, because that would make it a door rather than a
  record control — repointing it inverts that argument. And
  `REFUSALS.md:215-223` names *both* halves: the `Queue` door as a survivor of
  prior removal attempts, and the now-playing block as the labelled control for
  *get back to what is playing*.

`Place` itself is cheap: `place.rs` is 303 lines and pure, a new member is the
enum plus a 6-line door fn plus an exhaustive match the compiler finds. The
hand-enumerated tests are `place.rs:204`, `:226`, `:246` (the `showing` sum at
`:291` counts members and asserts `== 1`), and `app.rs:5915`. And
`views/queue.rs` already carries the playlist page's full edit set, so a
"current playlist" place is mostly a header swap rather than a new list.

### Cross-cutting

If 1-3 all land, `top_bar.rs`'s `draws()` and `play_all()` disappear and
`ACTS_W` 182 drops to zero — which removes the reason the strip's two-line
split at 960 px exists (`theme.rs:6500-6517`, ADR-0026 §3's *"asserted in
code"* budget). The Library strip would then hold: the well and its counts,
five group keys, `Playlists`, the gear. **That is a smaller strip than any
mockup in doc 10**, and it is worth drawing before it is built.

Three *"every X is a press some control also makes"* tables need re-counting by
hand: `app.rs:5190` (22), `menu.rs:586` (11), `menu.rs:609` (2). Two render
harnesses (`impl/shuffle-and-pull/`, `impl/queue-parity/`) document surfaces
that would no longer exist.

## The window's own chrome

**Drawing baz's own title bar — researched, not built.** The owner
(2026-08-09): *"get rid of the bar at the top, you know, the native chrome, and
just… implement those buttons in our app to be in the same sort of position. I
think that would look a lot cleaner."*

The good news first: **on Wayland that bar is already drawn inside baz's own
process.** GNOME expects applications to decorate themselves, so winit 0.30
pulls in `sctk-adwaita` (in `Cargo.lock`, via winit's
`wayland-csd-adwaita` feature) and draws the title bar itself. Turning it off
is one field — `window::Settings { decorations: false }`
(`iced_core-0.13.2/src/window/settings.rs:53`) — and the three buttons are all
available as tasks: `window::minimize`, `window::toggle_maximize`,
`window::close`. Dragging the window by our own strip is `window::drag`
(`iced_runtime-0.13.0/src/window.rs:40`), which is the same "start an
interactive move" the compositor gives a real title bar. Double-click to
maximise is `toggle_maximize`; the right-click system menu is
`window::show_system_menu`. All of it exists.

**The blocker is resize.** iced 0.13 exposes no `drag_resize_window`: the whole
`window::Action` enum is `iced_runtime-0.13.0/src/window.rs:24–161` and there
is no resize-direction variant anywhere in `iced_runtime`, `iced_winit` or
`iced_core`. winit 0.30 *has* `Window::drag_resize_window(ResizeDirection)`;
iced simply does not surface it. So `decorations: false` today buys the clean
strip and **loses the pointer resize edges**, on both Wayland and X11. That is
not a trade worth making silently for a window whose whole job is to be resized
to the wall you want.

Three ways out, in the order they should be considered:

1. **Expose the winit call.** One `Action::DragResize(Id, ResizeDirection)`
   variant, one arm in `iced_winit`'s runner, one `window::drag_resize` helper
   — perhaps thirty lines, upstreamable. It needs a patched iced, which under
   this project's rules is a reviewed dependency decision rather than a
   detail: a `[patch.crates-io]` on a fork pins baz to a tree the owner
   maintains until the change lands upstream.
2. **Hand-roll it** — an 8 px hit band at the window's edges that on drag
   computes a new size and origin and spends `window::resize` + `window::move_to`
   each frame. It works, and it will visibly lag under Wayland because every
   step is a round trip the compositor would otherwise have done itself. It
   also re-implements, badly, the one thing the platform is definitely better
   at.
3. **Keep `decorations: true` and restyle nothing** — the honest null option,
   and the one to take if 1 is not wanted, because 2 buys a cleaner top edge at
   the cost of the gesture people use most.

**The owner's placement, 2026-08-09**: *"I want the top bar with the search etc
to become the chrome… we have the close, resize, minimise on that bar with the
settings on the left."* So: the three window buttons take the strip's right
corner, and the gear moves to the left. Each in the [`theme::TRANSPORT_HIT`] 32
box every other icon control uses. `Glyph::Close` is already drawn; minimise
and maximise are two more on the sheet in the same 0.14–0.15 stroke band. The
"resize" button is `window::toggle_maximize`, which iced has — it is only
*edge-drag* resize that is missing.

Two consequences worth naming before it is built:

- **The gear loses half its licence.** Doc 10 §3.4 admits the gear as one of
  exactly two symbols allowed to stand without a word, and the argument is
  *"universal, **and top-right is its universal position**"*. Moved to the
  left, the symbol is still universal but the position argument is gone. Either
  the amendment is rewritten to drop the position clause, or the gear takes its
  word back on the left where there is room for it.
- **The strip's width budget works out**, and it works out *because* of the
  other brief. Three buttons plus their gaps is ~104 px on the right; the four
  removals above free `ACTS_W` 182. Net −78 px, so the two-line split at 960
  (`theme.rs:6500-6517`) gets easier rather than harder. The two briefs should
  land in that order.

The drag region is **the strip's empty space**, which needs stating carefully:
every control in the strip keeps its own press, so dragging is what the *gaps*
do, not what the bar does.

## The wall's hover options

**The bar's cover depends on the wall's thumbnail LRU.** `App::bar_cover`
reads the sounding record's sleeve out of `Shelf::thumbs` with `peek`, so the
bar observes the wall's art rather than competing for it. In a very large
library, scrolling far enough past `art::THUMB_CACHE_ENTRIES` can evict the
playing record's thumbnail, and the cover then disappears and the type shifts
left — the one kind of movement this bar is built not to make. The fix is to
keep the sounding record's thumbnail warm: `Shelf::request_thumbs` already
exists for the playlist sleeves and is the right pipeline, but the hook is
`App::warm_lamp`, which is called from a handler that returns no `Task`.
Threading one out is the whole of the work; it was left out of the hover-options
change because it touches the playback event path and that change touches the
view layer only.

**Idle CPU has not been measured on real hardware for this change.** The frame
count is measured and is what the design constrains — 0 frames in 10 s with a
tile hovered and with none — but the harness is Xvfb with no GPU, where iced
falls back to `tiny-skia` and the process sits at ~99.8 % CPU regardless. The
pre-change binary measures the same 99.8 % under the same harness, so it is the
harness; but `docs/design/04-fluidity.md` §1.4's 0.0 % is a real-hardware
number and has not been re-taken. Re-take it on the owner's machine next time
one is being taken anyway.

**The options are wall tiles' alone, for now.** Not on the Songs section's
rows, not in the lane — a row plays and a tile navigates, and a verb group over
a one-line row would be neither. If the Songs rows ever want an accelerator it
is a different design, not this one stretched.

## Rendering

**A renderer toggle in Settings — asked for, not built.** The owner asked
(2026-08-09) whether GPU acceleration can be allowed and toggled. The first
half needs nothing: baz takes iced's default features, so `wgpu` and
`tiny-skia` are both compiled in and `iced_renderer`'s fallback compositor
already tries the GPU first and the CPU second
(`iced_renderer-0.13.0/src/fallback.rs:214–262`). Every user with a working
adapter is accelerated today, and everyone else degrades silently — which is
what the headless captures in `docs/design/impl/` exercise, since Xvfb has no
GPU (`amdgpu_device_initialize failed` → tiny-skia).

The second half is a real, small piece of work and a real design question:

- **The mechanism exists.** `ICED_BACKEND=tiny-skia|wgpu` and
  `WGPU_BACKEND=vulkan|metal|dx12|gl` are read when the compositor is built.
  Documented in `docs/INSTALL.md` so the escape hatch is available now.
- **A Settings row would have to be restart-scoped.** The compositor is
  created once when the window opens and iced 0.13 exposes no way to swap it
  live, so the row is a stored preference plus an honest *takes effect next
  launch* line — the shape `docs/REFUSALS.md` tolerates least well.
- **The open question is whether it earns a row at all.** The automatic
  fallback covers "no GPU". A toggle only buys the case where the GPU path is
  present and bad: a tearing driver, or a hybrid laptop spinning up a discrete
  card for a music player. That is a real class of bug report and it is also
  the sort of tenant a Settings place accretes; deciding it is the owner's.
- **If it ships**: one `config.toml` key, one row in the existing Settings
  section machinery, the value passed to `iced::application(...).settings()`
  rather than to the environment, and a line in the signal-path vocabulary's
  neighbourhood saying which renderer is live — because a preference whose
  effect you cannot see is a preference nobody can debug.

## Platform integration

- ~~**No application icon.**~~ — **shipped.** `packaging/icons/` holds the SVG
  master and the hicolor PNG ladder, the desktop entry names it, and the
  Flatpak and the Linux tarball install it. **The binary still sets no window
  icon**: winit 0.30 supports that on Windows and X11 only — never Wayland or
  macOS — so it buys nothing on baz's primary platform and is worth doing for
  the Windows build alone. The reasoning and the patch are in
  `packaging/icons/README.md`.
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
