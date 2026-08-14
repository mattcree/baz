# baz — the work queue

> **This file answers one question: what is next?** Read the top of the *Next*
> list below and start. If you are an agent picking this project up cold, this
> is the only file you need to begin; everything else explains *why*.
>
> **The rule.** Every item here is one of four states, and an item leaves only
> by being **done** or by the owner saying it should go. "Blocked on a
> decision" is a *note on the item*, never a reason to delete it — that failure
> has happened, and this file exists because of it.
>
> | state | means |
> |---|---|
> | **next** | ready to start, in the order listed |
> | **doing** | an agent has it right now |
> | **waiting** | needs a decision from the owner, named on the item |
> | **done** | shipped and on `main` |
>
> **Every agent updates this file in the commit that lands its work** — moving
> its item to done and adding anything it discovered. A branch that changes the
> product and not this file is incomplete.
>
> Where the other documents fit: `CHANGELOG.md` is what shipped, and
> `BACKLOG.md` is everything not done yet — deliberate deferrals, known gaps,
> and, in *What the owner asked for*, his asks verbatim with their fate.
> `NEXT-STEPS.md` is the shape of the project.
> **This is the ordered queue.** If they disagree, this one is wrong — fix it.
>
> ## How this list is ordered
>
> **Functional work comes first.** The owner, 2026-08-10: *"please ensure we
> prioritise functional changes"*. A functional change is one that alters what
> baz can *do* — a file that would not play now plays, a library fact that was
> wrong is right, something that could not be installed can be. An interface
> change makes what it already does easier or nicer to look at. Both are worth
> doing and this project has shipped a great deal of the second; the ordering
> rule is that the first does not wait behind the second.
>
> **This rule exists because the queue had drifted.** Every item on it was an
> interface item, while `BACKLOG.md`'s *"Known gaps in shipped features"*
> section held formats that do not play, an index that keeps deleted folders
> and a stamp that is lost on removal — none of them queued, because a backlog
> is where things go to be *reasoned about* and this is where they go to be
> *done*. The gaps below were promoted out of it on the strength of that
> sentence.
>
> **What still comes before everything:** a defect the owner is looking at.
> A report from him is a functional change by definition, because the thing he
> is looking at is the product.
>
> ## The public beta remains a goal; the 2026-08-12 review now orders the work
>
> The owner, 2026-08-10: *"can we start to hone in on what a public beta would
> look like? can we trim anything ongoing/in the backlog so we can focus on
> getting that ready?"*
>
> **The bar he chose: the core loop feels complete.** Not *"it won't break"*
> (which would have cut the interface work) and not *"it looks finished"*
> (which would have pulled in the ambient surface and become a 1.0). So the
> beta promises: **you can find your music, play it, make lists of it, and
> nothing baz does loses or corrupts anything** — and the few things it cannot
> do are stated on the front page rather than discovered.
>
> The beta argument above still defines the quality bar, but the owner's live
> review on 2026-08-12 supersedes the former interface freeze. He asked to work
> through the **critical usability** issues first, then tackle **Home through
> vibe/generated playlists**. `## Next` is therefore a phased execution queue,
> not a list containing only beta blockers. Its order is authoritative after a
> context reset.
>
> **What is deliberately outside the critical-usability tranche**, recorded so
> the phases do not blur together:
>
> - **The remainder of ambient `Now playing`** — the local facts feed and kiosk
>   affordances. The cover-derived field, rotating jewel case and independently
>   toggled full-body spectrum shipped during the owner's visual pass on
>   2026-08-11; the VU mode was tried and explicitly removed. The remaining
>   pieces are 1.0 work, not beta blockers.
> - **Kiosk mode**, for the same reason and because iced cannot enumerate
>   monitors.
> - **Vibe- or prompt-generated playlists** — the opt-in conventional sonic
>   baseline and model-swappable semantic evaluation path are now built, but the
>   owned listening evaluation and product interaction remain a whole feature
>   rather than a finish. On 2026-08-12 the owner placed
>   Home/vibe work immediately after the critical-usability tranche; that makes
>   it the next feature area, not a beta blocker ahead of those fixes.
> - **Borderless window chrome and the iced migration.** This was deliberately
>   ordered after usability and Home, and shipped as item 16 on 2026-08-14.
> - **Opus.** Closed rather than deferred: the owner's library was scanned on
>   2026-08-10 and holds **zero** `.opus`, `.ogg` or `.oga` files across
>   `~/Music` and both NAS shares. The refusal in *Known gaps* stands on its
>   own evidence and becomes a documented limitation, not a dependency
>   decision. It reopens only if a beta tester asks.
> - **Resize smoothness** — the owner's own *"lower priority"*, measured and
>   left with its numbers.
> - **An individual-record forget control.** Explicitly rejected by the owner
>   on 2026-08-10 when it was shown: deleting or moving the files is the removal
>   gesture, and baz's index should follow the filesystem. The internal
>   `forget_paths` mechanism is not permission to add a second UI workflow.
> - **The engine's remaining known gaps** — sample-accurate splices, the
>   Symphonia 0.6 upgrade, the density cache's decode size, FLAC-in-MP4's ALAC
>   label, AAC's missing gapless trim. Each is real, none is a reason a
>   listener would put baz down, and **each must appear in the README's known
>   limitations** — which is the item that converts a gap from a defect into an
>   honest promise.
>
> **The one thing that outranks this list** is unchanged: a defect the owner is
> looking at. A beta scope is not a reason to tell him his own product is fine.

## Next — authoritative execution order

When the owner says *"work through the backlog"*, begin at the first unfinished
number below. Do not ask him to reconstruct the review or choose an item. Read
the matching detailed brief later in this file, its row in `BACKLOG.md`, and
the ADR/design it names; settle ordinary implementation details from those
constraints. Complete one coherent numbered item, verify it in the dev
container, update both queue and backlog, then continue to the next safe item.
Ask only when a genuinely unrecorded choice would materially change the
product.

### Phase A — critical usability

1. **Done 2026-08-12 — Unify selection and activation.** One shared content
   selection now covers album, playlist and implicit-list tiles plus album,
   search, playlist and queue rows: one click highlights, a second matching
   click activates, and labelled Play/Open controls remain direct. Enter
   activates the current selection outside search; Space remains transport.
2. **Done 2026-08-12 — Build the app-bar search dropover.** The sole full well
   now lives in the app bar and type-anywhere opens ranked Tracks and Albums
   over the unchanged place in one virtualized scroll/selection surface.
   Up/Down clamp and reveal, Left/Right choose track `Play | Enqueue`, Enter
   confirms, and Esc/click-outside clears and dismisses. The lane, narrow
   strip and Library-body search presentations are gone.
3. **Done 2026-08-12 — Make deliberate playback land correctly.** Every
   explicit or double-click album start uses one start-and-show path. It arms
   the destination only after both queue and Play commands are accepted, then
   opens Now Playing on a matching `TrackStarted`; empty, exhausted, refused
   and closed-engine starts stay put.
4. **Done 2026-08-12 — Add wheel-over-volume control.** Vertical wheel travel
   over the live fader uses the keyboard's bounded ~1 dB step. Line input maps
   to notches; pixel input accumulates at 32 px per step. The fader captures
   the whole gesture, mute remains independent, endpoints clamp, and one
   settled confirmed value is persisted after a 240 ms quiet boundary.
5. **Done 2026-08-12 — Keep visible artwork visible.** Current wall, page and
   chrome targets now live in a resident handle tier; only off-screen recent
   artwork competes in the existing 64-entry LRU. An 80-album Artist-page
   stress run retained every sleeve through churn at about 25.3 MiB decoded.
6. **Done 2026-08-12 — Finish the Now Playing visual states.** The persisted
   foreground is Cover / Jewel case / None, independent of Spectrum. None has
   no object or reserved square, uses a soft metadata mask, and skips hero/case
   work. Focus no longer pauses visible motion or sampling; place, sounding
   state and enabled visuals still gate every continuous cost.
7. **Done 2026-08-12 — Stop the Recent-row sounding pip from reflowing text.**
   Every expanded row now reserves a far-trailing lamp slot and the same fixed
   146 px measure for both lines. Long album and playlist strings use measured
   end ellipses; playback changes ink without moving row geometry.
8. **Done 2026-08-12 — Converge saved and unsaved playlist detail.**
   `views::playlist_page` now owns one sleeve, breakpoint, responsive document,
   identity hierarchy, empty state, scroller and row presentation for both
   persistence states. Saved Play/Rename/Delete/counts and unsaved
   Save/cursor/remaining-time/provenance occupy named capability slots. The
   live-review follow-up also makes global search's action contextual here:
   `Add to playlist` edits the saved file on screen, while other places retain
   live-run `Enqueue`; the chooser starts unselected and visibly teaches its
   now-focus-safe arrow grammar.
9. **Done 2026-08-13 — Converge the Playlists root with Library.** Library and
   Playlists now draw through one collection scaffold: the same right-hand
   rail stack, edge scrollbar, rail lane, grid/density geometry and tile
   selection grammar. The playlist rail projects `A–Z` under alphabetical
   order and elapsed `Date created` / `Played` buckets under chronological
   orders, with unavailable creation dates and never-played lists stated
   separately and inert absent buckets retained.
10. **Done 2026-08-13 — Add browser-style place history.** Top-left Back and
    Forward arrows, with Alt+Left/Right accelerators, walk an in-session
    `Place`-identity history; normal branch semantics, duplicate suppression,
    safe missing-subject fallback and stable disabled states are covered by
    tests. They are explicitly page navigation, not track transport; opening
    or dismissing search creates no entry.
11. **Done 2026-08-13 — Move operational health behind the app-bar bell.**
    The fixed app-bar bell is the sole health door; it anchors the canonical
    event panel below the bar, replaces the bottom-bar dot, and acknowledges
    transient attention on open/close. Recoverable warning/error states expose
    a safe incremental Retry, while the existing bounded five-minute rescan
    remains the automatic retry. Every skipped path and scanner reason is now
    recorded in the listener-visible event history rather than terminal-only.
12. **Done 2026-08-13 — Fix Windows GUI launch packaging.** Release/package
    builds open only baz, no companion console; debug builds retain the console
    and its diagnostics. `crates/baz/src/main.rs` carries the crate-root gate
    `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem =
    "windows")]`. The final acceptance launch of the actual packaged `.exe` on
    Windows is the owner's.

Phase A is complete only when all twelve items are implemented and verified;
“critical usability” does not need to be redefined in a future conversation.

### Phase B — Home is the next feature area

13. **Done 2026-08-14 — Finish local-first sonic vibe playlists on Home.** The owner
    rejected the metadata/history grammar as the feature: the product bar is a
    genuinely impressive offline musical model of the listener's collection.
    The first end-to-end sonic slice introduced explicit/cancellable
    incremental audio analysis, a separate versioned/stale-file-aware cache,
    conventional sonic controls and continuity/diversity sequencing behind the
    default `vibe-analysis` feature. Its hidden, analysis-first preset panel and
    sounding-track anchor were accepted, then explicitly reopened after use as
    too poor to leave as the product. The finished composer keeps the whole
    request visible on Home as ordinary free text rather than presets,
    30/60/90/120-minute targets, Create-driven first-use consent, incremental
    progress, and a silent in-place preview with reorder/remove, Play, Save and
    Another version as separate acts. The playing track is never an implicit
    baseline; an existing local index makes later Create presses resume without
    repeating consent; cancellation retains the request without leaving the
    consent notice standing. The normal build bundles Baz's reproducibly
    exported quantized LAION CLAP audio/text pair and tokenizer, so local
    track windows and prompts meet in the same 512-dimensional space. Retrieval
    iterates against real durations, retains continuity and album/artist
    diversity, prevents exact duplicate paths, and persistently avoids songs
    offered by recent generated mixes. Release archives and Flatpak carry the
    pinned model, licences and runtime without a first-run download.

### Phase C — make releases sustainable, then ship

14. **Done 2026-08-13 — Choose the beta distribution/update policy.** The owner
    chose the existing GitHub Release archives. Baz stays offline; discovery,
    download, checksum verification, replacement and rollback are explicit
    user actions. Automatic signed updating and managed stores are deferred.
15. **Ready for owner release authority — Ship the public beta through GitHub
    Releases.** The corrected non-publishing rehearsal and all three platform
    builds are green. Creating the `v0.1.0` tag and public GitHub Release is the
    remaining external boundary; do not assume a Flathub submission.

### Phase D — accepted follow-on work

16. **Done 2026-08-14 — Upgrade to iced 0.14 and make baz's app bar the
    default window chrome.** A six-logical-pixel inside-edge band dispatches
    all eight upstream resize directions; drag, maximise, system menu and
    window controls remain, with `BAZ_NATIVE_CHROME=1` as a diagnostic escape.
17. **Ship built-in and JSON custom themes.** Four polarity rooms, versioned
    schema/examples/import/export, runtime accessibility validation and safe
    fallback; no executable theme content.
18. **Continue the remaining briefs below and `BACKLOG.md` functional-first.**
    Defects and data correctness precede interface polish; do not silently drop
    an ask merely because it is not named in phases A–D.
19. **Tighten the app and bottom-bar edge composition.** Re-measure and reduce
    their gutters without compromising hit targets or the rail/scrollbar law;
    enlarge and correctly hang the app mark, and equalize the bottom-left
    artwork block's x/y padding.
20. **Make all visible artwork resident.** A visible target with available art
    must hold an image handle through cache churn; simplify toward visible
    residency plus the existing bounded off-screen LRU, rather than adding
    another cache policy or speculative prefetcher.
21. **Contain scrolling artwork beneath the resident chrome.** Reproduce the
    intermittent state—currently cleared by a reset—and repair the shared
    invalidation, clipping or paint-order fault that lets sleeves cross over
    the app bar or bottom transport. Off-viewport content must neither draw nor
    receive input through either bar.
22. **Align sticky section headers with their grouped content.** Remove the
    excess left inset in the shared sticky bar without changing its full-width
    field; its visible heading edge must stay aligned before and after it sticks
    across Artists and every other consumer.

## Doing

- **Item 15 — GitHub public beta.** Item 14 chose GitHub Release
  archives only for the beta. Updates are manual and user-initiated: Baz never
  contacts a service, checks, downloads, overwrites itself or replaces its own
  files. A listener explicitly downloads the new Baz archive, verifies its
  published SHA-256 checksum, quits Baz, replaces the application files, and
  relaunches; config, library data and playlists remain in their existing data
  directories. The corrected locked CI release rehearsal and all three
  platform builds are green. The 2026-08-13 local gate passed warnings-denied
  clippy, all workspace tests (including 775 Baz tests), rustdoc, cargo-deny,
  packaging validation, the 14-test Vibe evaluator harness and the
  explicit-feature Linux release build. Its staged 0.1.0 archive is 13 MB,
  contains the expected desktop assets and verifies against its generated
  SHA-256. GitHub
  Actions rehearsal [31751846915](https://github.com/mattcree/baz/actions/runs/31751846915)
  then passed the complete CI gate, built Linux x86-64, Windows x86-64 and
  universal macOS archives from commit `8f5c1cf`, and completed its guarded
  checksums-and-release job without publishing. The artifacts were downloaded
  again as a consumer: all three match the generated `SHA256SUMS`, and each
  archive has the expected Baz executable, README, changelog and licence (plus
  Linux desktop metadata and icons). Next: obtain the owner's explicit
  authority to create the `v0.1.0` tag and public release. Flathub, MSIX/App
  Installer, Sparkle, signing identities and a self-updater remain deferred.


## Detailed briefs, later work, and genuine unresolved choices

- **Sticky section bars sit too far right.** Recorded 2026-08-14 from the
  owner's live use and queued as item 22. Measure the shared sticky-header
  component's visible heading edge against the content it labels rather than
  assuming the container edge is the ink edge. Audit Artists and every other
  grouped wall using it at all density and responsive breakpoints, in both its
  ordinary and stuck positions; scrolling into the sticky state must not create
  a horizontal jump. Preserve the intended full-width background, clipping and
  interaction geometry. Fix the shared alignment token/composition rather than
  introducing a page-specific negative margin. Acceptance is same-viewport
  captures with measured content and heading edges for every consumer in both
  states.
- **Scrolling artwork escapes over the app and bottom bars.** Recorded
  2026-08-14 from the owner's live use and queued as item 21. Treat “z-index”
  as the observed effect, not a predetermined implementation: first establish
  whether the shared collection viewport fails to clip, whether nested scroll
  content is painted after resident chrome, or whether a renderer-specific
  layer escapes its bounds. The fault disappeared after a reset, making it an
  intermittent state/invalidation investigation rather than a static layout
  correction. Establish exactly what “reset” rebuilt, then preserve the
  preceding navigation, resize, density/display changes, scrolling, artwork
  load/cache churn, renderer and chrome state in a repeatable reproducer. Cover
  every scrolling surface that can draw album or playlist sleeves,
  fast/inertial movement, all density and responsive breakpoints, and both
  native-titlebar and Baz-owned-chrome arrangements.
  Content outside the viewport between the two bars must not paint or accept
  pointer input through them. Fix the shared viewport/chrome composition rather
  than hiding the symptom with tile padding. Acceptance is same-frame evidence
  at the top and bottom boundaries plus a structural regression check that the
  clip and resident paint order remain intact across the triggering state
  transition and supported renderers. A fresh launch alone cannot close it.
- **Prune albums whose files have genuinely been removed.** Recorded
  2026-08-13 on the side of item 13; do not investigate or implement as part
  of the vibe work. First establish what the successful scanner currently
  removes and identify the cases that leave stale albums. Any automatic or
  confirmed manual pruning must require positive evidence that the owning root
  completed a successful scan: an offline/unmounted root, GVfs/SMB outage,
  permission failure or cancelled/incomplete scan is not evidence of deletion.
  Preview and confirm a manual bulk removal; update the library atomically;
  never delete audio, playlist files or history; decide how their now-missing
  references are represented; and reconcile the wall, search, selection,
  current run/provenance and artwork caches without starting playback.
- **Delete a saved playlist from the Playlists overview with confirmation.**
  Recorded 2026-08-13 on the side of item 13; do not implement as part of the
  vibe work. Give each saved list's overview affordances a Delete action which
  opens an explicit confirmation naming that list. Cancel is inert; confirm
  must reuse the existing move-to-trash deletion path, close transient state,
  remove the overview row and leave a sensible remaining destination selected
  without playback. Preserve the existing playlist-page action and make both
  doors invoke one behavior, including for foreign playlists.
- **Warn when the active signal path resamples.** Recorded 2026-08-13 on the
  side of item 13; do not implement as part of the vibe work. Settings and the
  canonical event history should both treat active conversion as a warning,
  name source/output rates when known and explain the exact device/output or
  boundary choice that can restore a direct path. Deduplicate a continuing
  condition and clear it when conversion ends; distinguish Baz's boundary
  resampler from conversion owned by the operating-system mixer.
- **Verify when an audio-device picker change takes effect.** Recorded
  2026-08-13 as an explicitly uninvestigated question; do not probe it during
  item 13. Later audit selection, persistence, engine reopen/command, signal-
  path event and visible selected value together, then make immediate, next-
  track or restart behavior honest and consistent and expose open failures.
- **Tighten the top and bottom bars' edge composition.** Recorded 2026-08-13.
  The Settings cog still reads too far in from the right, and the application
  mark is both too small and too far from the left edge. The bottom bar's
  left-hand artwork also has unequal padding: its x inset visibly exceeds its
  y inset. Treat these as one measured edge-composition pass, not three local
  nudges: preserve control hit targets, the app bar's conditional window
  controls, and the wall rail/scrollbar relationship. The resulting bar
  gutters should be deliberately shared or deliberately different with the
  reason and measurements recorded; the artwork block must have equal x/y
  padding. See item 19 and ADR-0040's existing gutter amendment.
- **Make visible artwork unconditionally resident.** Recorded 2026-08-13. The
  current cache sometimes leaves a sleeve blank or unloads it while it remains
  on screen, likely from earlier memory optimisation. Re-establish the simple
  product guarantee: if a visible target has art, it has a loaded handle. Audit
  the visible-target collection and cache handoff across wall, page, chrome and
  lane consumers; retain only the current bounded LRU for off-screen recent
  artwork. Reproduce around 500 and 800 albums, including Windows: it is mild
  at 393 albums on the owner's Linux machine and reportedly worse near 800 on
  a friend's Windows machine. See item 20.

This section preserves the evidence and acceptance detail behind `## Next`,
plus lower-priority and truly owner-blocked work. It is **not execution order**
and a brief repeated in `## Next` is ready—not waiting for the owner. Search by
its bold title from the numbered item above.

- **Wheel over volume adjusts volume and consumes the scroll.** Recorded from
  the owner's 2026-08-12 live review. Give the interactive volume block
  target-aware wheel handling: vertical travel reuses `VolumeStep` and
  `PlayerState::step_volume`; while hovered it captures the event so no wall,
  lane or dropover beneath it scrolls, and it takes precedence over global
  Ctrl+wheel density. Normalize line input and accumulate high-resolution pixel
  deltas, clamp endpoints, preserve the unity detent and existing engine/
  persistence path, and ignore horizontal travel. *Critical usability. Needs
  during implementation: step/acceleration, mute semantics, exact hover bounds
  and settled-write boundary; tests for wheel/trackpad, endpoints, muted state,
  no scroll leak and pointer-elsewhere behavior. **Done 2026-08-12:** the live
  fader's own hit band is the target (the mute button remains a discrete act);
  line notches and accumulated 32 px touchpad steps feed `step_volume`, capped
  to the fader's 25-step span per event. Wheel changes the level behind mute
  without unmuting, capture prevents underlying scroll and Ctrl+density, and a
  240 ms quiet boundary coalesces confirmation-driven persistence.*
- **Now Playing foreground has 2D, 3D and None states.** Recorded from the
  owner's 2026-08-12 live review so Spectrum can be enjoyed without an album
  object. Add `None` beside Cover and Jewel Case while keeping Spectrum an
  independent toggle. None removes both the object and its reserved stage/
  spectrum exclusion pocket—never a blank square—while keeping the placard
  readable; None + Spectrum on is the spectrum-led view, and neither control
  silently changes the other. Persist the choice and stop artwork/rotation work
  when unused. *Critical usability. Needs during implementation: third mark/
  tooltip, metadata composition and spectrum mask in the absent-art state,
  transition behavior, and all six foreground × spectrum combinations across
  width, track change and focus loss. **Done 2026-08-12:** a third crossed-cover
  mark selects persisted `none`; its branch never constructs the hero or case,
  reserves no square and centres the stable placard inside a soft horizontal
  mask while the spectrum remains full-body. All six states have independent
  clock tests, objectless measures are swept from narrow through 4K, and
  returning to Cover/Case self-heals the hero request and existing dissolve.*
- **Done 2026-08-14: Home's local-first free-text Vibe composer.** This is the
  owner's explicit ordering from the 2026-08-12 live
  review. Do not spend the Home pass on decorative rearrangement: give it the
  entry point for a listener-requested vibe and the ordinary editable playlist
  that results. The existing ground rules remain binding—explicit
  request, visible candidate pool, inert provenance, no autoplay, no silent
  regeneration, no cloud/account, and no second playlist species. The honest
  bundled paired LAION engine now drives the ordinary-language request and
  silent editable preview. The earlier ballot gate was explicitly removed by
  the owner; the reproducible export, preprocessing checks and pinned payload
  remain the implementation evidence. Recent-offer persistence prevents one
  standout match from recurring across separate mixes.
- **Ship four polarity themes and a safe, AI-friendly custom-theme JSON
  format.** Recorded from the owner's live review on 2026-08-12. The rendering
  system already routes styles through one semantic `Palette`; Closing Time is
  the dark endpoint, Reading Room is the defined-but-unselectable light
  endpoint, and Stone/Plaster were deferred candidates for intermediate rooms.
  Turn that foundation into four selectable/persisted built-ins—Light,
  light-biased middle, dark-biased middle, Dark—and a versioned custom-theme
  document. JSON is data only: semantic palette colors and tightly bounded
  visual values, never code, URLs, paths, downloaded fonts, layout or behavior.
  Ship a JSON Schema, built-in examples, exportable template and concise prompt
  so a listener can ask an external AI for a theme, then import/paste and
  preview it locally without baz gaining an AI/network dependency. Runtime
  validation must enforce the same contrast/elevation/accent laws as built-ins,
  diagnose exact fields, and always retain/fall back to a valid room. *Needs
  later: priority; final names and middle palettes; live switching versus
  restart (today `ACTIVE` and themed glyphs are startup caches); picker/import/
  export UI; theme directory and stable IDs; schema migration and OS-following;
  derived-token policy; and malformed, unreadable, missing-selected-theme and
  round-trip tests.*
- **Users need a safe, distribution-aware path to new baz releases; Flatpak is
  optional.** Recorded from the owner's live review on 2026-08-12; amended
  2026-08-13: the beta distributes only through GitHub Release archives. Baz
  remains offline and has no updater; the listener chooses when to download a
  new Baz archive and replace the prior one after checking its published
  SHA-256. The current release pipeline already creates direct Linux, Windows
  and macOS archives. Do not let the
  existing Flatpak manifest choose the distribution strategy by inertia.
  Compare a first-class direct installer/updater and Linux formats such as
  AppImage or native packages with Flatpak/store channels. Split release
  discovery from installation: a managed build delegates to its actual owner
  and never self-overwrites, while a direct install needs a signed, verified,
  atomic update path with user-controlled restart and preservation of config,
  library data and playlists. This deliberately reopens baz's “no network at
  all” promise, so checking needs explicit policy rather than silently phoning
  home. An available update can use the pending notification bell; network
  failure stays non-disruptive. *Needs later: priority and a research/design
  spike choosing supported formats, install-origin detection, stable/prerelease
  channels, metadata/cadence/consent, signing and platform installers,
  package-manager handoff, rollback/restart, and database downgrade safety.
  The beta deliberately stops at manual replacement. Any later automatic
  update path requires a separately approved signed external-updater design;
  a notification or GitHub link alone must not be described as automatic
  update support.*
- **The packaged Windows app must not open a companion command window.**
  Recorded from the owner's live review on 2026-08-12. `crates/baz/src/main.rs`
  currently declares no Windows GUI subsystem, so the executable is linked as
  a console application. Apply the GUI subsystem to packaged/release Windows
  builds while retaining the console in debug builds for developer stderr.
  User-facing failures must continue through the health/event UI; decide on an
  intentional file or Windows-native diagnostic sink before relying on release
  console output that will no longer exist. *Needs later: priority and a test
  of the actual packaged `.exe` from Explorer and Start—no terminal flash or
  persistent console—plus confirmation that debug launches retain useful
  diagnostics.*
- **The sounding pip must trail a Recent row without reflowing its text.**
  **Done 2026-08-12.** Every expanded row now carries the same six-pixel slot
  at its far trailing edge whether it is sounding or quiet. Both title and
  metadata share the exact 146 px boundary left by the existing sleeve,
  padding and gaps; each is fitted with its actual bundled face and ends in an
  ellipsis when necessary. The sleeve/text origins and 64 px row pitch are
  unchanged, and the collapsed lane keeps its existing card treatment.
- **Typing should reveal a keyboard-navigable search result/action chooser.**
  Recorded and refined during the owner's 2026-08-12 live review. A
  type-anywhere query opens a bounded dropover from the app-bar well over the
  current place; it does not navigate to Library or alter history. Tracks and
  Albums are the primary sections in one continuous, optionally virtualized
  scroll surface—no eight-track cap, separate wall scroller or nested trap.
  Up/Down select, reveal and highlight a result across the section boundary;
  Left/Right select `Play` or `Enqueue` for that track, and Enter executes the
  selected action. Merely moving selection cannot play or enqueue anything;
  enqueue means append to the current run without replacing it or starting
  playback. This mode contextually owns the arrows that currently control
  volume vertically and seek horizontally, while those transport bindings stay
  unchanged outside result selection. Do not steal Left/Right caret movement
  while the query field itself is active: define an explicit handoff between
  editing and result navigation. Share the pending click-selection highlight
  model rather than creating a search-only visual state. *Needs later:
  priority; dropover geometry/virtualization; default track action; album
  activation; Enter/Esc/click-outside and post-action behavior; clamp/wrap;
  selection reset/auto-scroll as ranking changes; no-result handling; and
  whether any result type beyond Tracks/Albums earns admission.* **Done
  2026-08-12:** Play is the default; movement clamps; reranking selects the
  first answer; selected rows auto-scroll; Enqueue keeps the chooser open;
  Play and Open complete it; Esc/click-outside clear rather than hiding a live
  query; Albums retain Play/Open and the shared selection/double-activation
  grammar. No secondary result type was admitted.
- **The Playlists collection should be the Library wall with playlist data,
  including its right-hand rail.** Recorded from the owner's live review on
  2026-08-12; “Playlists page” here means the saved-playlist root. It currently
  borrows `shelf::Grid` but owns a separate top-level scroll/virtualization/tile
  composition and has no index rail, so visual similarity is convention rather
  than structure. Later extract/use one collection scaffold for viewport,
  gutters, density, virtualization, scrollbar, selection/hover grammar and the
  right rail, parameterized by album versus playlist items and their grouping.
  This decides ADR-0024's deferred playlist-rail question in favor of having
  one. *Needs later: priority and rail semantics for `Date created` and `Played`
  ordering—date/recency buckets, or a rail present only for `A–Z`; never an
  alphabetical rail whose jumps disagree with the visible order.*
- **Now Playing animation freezes whenever the window loses focus.** Recorded
  from the owner's live review on 2026-08-12. This is the current code's
  deliberate `window_focused` gate on both the continuous timer and spectrum
  sampling, but focus is not visibility: an ambient player on a second monitor
  is expected to keep moving while another app receives input. Later remove
  focus from animation eligibility while keeping the meaningful guards—Now
  Playing is the current place, a record is sounding, and Jewel Case or
  Spectrum is enabled. Minimized/fully occluded throttling is still reasonable
  only if iced/platform state can say it reliably. *Needs later: priority and
  performance measurement; acceptance covers continuous jewel rotation and
  live spectrum across focus loss, no reset/jump on refocus, and zero continuous
  redraws on non-Now-Playing places. **Done 2026-08-12:** focus is absent from
  both timer and tap eligibility; losing it only releases a case drag. The
  remaining gate is exactly visible Now Playing + sounding record + Spectrum
  or Jewel case. Rotation ticks retain their bounded elapsed step, and the
  pre-volume spectrum tap now stays truthful even while output is muted.*
- **`Play album` should open Now playing after playback starts.** Recorded from
  the owner's live review on 2026-08-12. This reverses the current source-level
  rule and test that Resume is the only play gesture which navigates. The
  transition must follow a successful start—not an empty queue, refused command
  or closed engine—and should be expressed as a shared start-and-show path so
  the playback request and destination cannot disagree. **Done 2026-08-12:**
  command acceptance arms the destination and a matching `TrackStarted` spends
  it. `QueueEnded` and engine closure cancel it; explicit Play, search album
  Play and item 1's album activation all share the route.
- **Single click selects; double click plays.** Recorded from the owner's live
  review on 2026-08-12 as a product-wide content interaction rule. One ordinary
  click must only select and visibly highlight the album, playlist or playable
  row; activation requires a double click. This reverses the present mix of
  one-click navigation and one-click row playback, as well as the earlier
  removal of wall double-click-to-play. Build one selection/activation state
  machine rather than per-view timers. The explicit `Play album` ask above
  suggests labelled Play controls remain one-click commands; confirm that when
  this is designed. *Needs later: priority and a complete surface matrix—what
  single-click selection does to detail navigation; album/playlist/implicit
  tiles; album, saved-list and unsaved-run rows; keyboard activation; touch;
  queue jump semantics; and existing explicit hover Play controls.*
- **Saved and unsaved playlists must be one component, not lookalike pages.**
  Recorded from the owner's live review on 2026-08-12. His observed mismatch is
  evidence that the current partial reuse is insufficient: `views::playlist`
  and `views::queue` share identity, rows and collage primitives but retain two
  top-level compositions, despite the source calling them “the same editor.”
  Later, capture both at the same viewport, inventory the drift, and replace
  the pair with one playlist-page component parameterized by state/capability.
  Saved-only rename/delete/file identity and unsaved-only Save/live-cursor/
  remaining-time behavior are legitimate slots; geometry, hierarchy,
  typography, sleeve, breakpoint, empty state and row presentation are not.
  **Done 2026-08-12.** The saved page is the reference anatomy. One
  `views::playlist_page` call now owns the collage, responsive breakpoint,
  fixed aside/table or stacked document, identity, empty state and scroller.
  The unsaved run moved its live summary into the shared facts line and Save
  into the shared acts slot; its rows now use the saved page's fixed pitch,
  artwork and Album context. Same-size before/after frames and the drift
  inventory live in `docs/design/impl/one-playlist-page/`; source guards refuse
  a private `page::view`, sleeve, breakpoint or scroll composition in either
  persistence-state module.
- **Browser-style Back and Forward in the app bar.** Recorded from the owner's
  live review on 2026-08-12, with Spotify's top-left navigation arrows as the
  interaction reference. These navigate places/subjects, never playback
  tracks. This is a deliberate reversal of `place.rs`'s current model: places
  replace each other, no history exists, and Back is a total function that
  always returns Library. It also reopens ADR-0040's closed app-bar tenancy and
  its identity-only top-left zone. Begin later from ordinary browser semantics:
  Back/Forward walk visited entries, a new branch after Back drops the forward
  branch, revisiting the current identity creates no duplicate, and each arrow
  has a stable disabled state. *Needs later: priority; whether an entry stores
  only `Place` or query/scroll/local state too; how Esc, breadcrumbs and resident
  destinations interact with history; placement beside or instead of the baz
  mark; and whether Alt+Left/Right plus mouse navigation buttons ship with the
  visible controls.*
- **Visible artwork sometimes unloads under the bounded cache.** Recorded from
  the owner's live review on 2026-08-12. **Done 2026-08-12:** reproduction
  proved that a page consumer could churn the shared 64-entry LRU, evict an
  on-screen sleeve, and then meet the unchanged-viewport guard that suppressed
  its reload. Current wall, page and resident-chrome IDs now pin decoded
  handles in a resident tier; leaving every current target returns a handle to
  the same bounded recent LRU immediately. Unit stress reproduces the old
  eviction and proves the new invariant. A release GUI run loaded an 80-album
  Artist page, scrolled away and back after all 80 decodes, and retained every
  visible cover; its Balanced-density resident cost was about **25.3 MiB**,
  deliberately proportional to that non-virtual page rather than a blank/pop.
- **Move the canonical health/event surface behind a notification bell in the
  app bar and give failures a recovery path.** Recorded from the owner's live
  review on 2026-08-12, for later prioritisation rather than implementation
  during the beta freeze. This refines the earlier status-log direction: the
  log remains the source of truth for an offline folder or other operational
  condition, but its one resident entry point moves from the current bottom-bar
  dot to a notification bell in the app bar. Put it in the application zone
  beside Settings, reserve its hit box in every place, anchor the history layer
  to it, and remove the bottom-bar indicator rather than drawing two doors.
  Important/unread red status may briefly pulse or use a restrained badge; a
  standing condition must not cause permanent animation. Recoverable events
  carry `Retry`, bounded automatic exponential backoff, or both, and recovery
  resolves the existing condition and returns the bell/summary to good rather
  than accumulating duplicates. The existing five-minute periodic rescan
  already retries offline roots coarsely, so first decide whether to expose it,
  add targeted immediate retry, or replace it with per-condition scheduling.
  *Needs later: priority; bell/read/severity states and acknowledgement; panel
  placement and narrow-width app-bar tenancy; the canonical event/condition
  model; safe retry classes; manual/automatic and backoff/cap/reset rules; and
  what Settings retains once status owns runtime health.*
- **Where does a listener find out *which* files were skipped?** The Zappa
  album above was lost for as long as it was because the answer was
  "nowhere" — `14 files skipped` in the status line is a statistic, and a
  statistic cannot name a record. A scan now prints one
  `[scan] skipped <path>: <reason>` line, which is enough for him at a
  terminal and **nothing at all** for a listener running the Flatpak, so the
  product question is still open. Three shapes, in increasing cost:
  - **The log line alone** (what shipped). Free, honest, and invisible to the
    people the beta is for.
  - **A readout in Settings** — *"14 files could not be read"*, expanding to
    the list with each file's reason, beside the *last scanned* line that is
    already there. This is the recommendation: Settings is where a scan's
    other facts live, the skipped set is small and already in hand at the end
    of a pass, and it costs one `ScanUpdate` field carrying the paths instead
    of a count. Nothing about it is a modal, which is the rule
    `crates/baz/src/scan.rs` exists to keep.
  - **Something on the wall.** Refused unless he asks: a record baz could not
    read is not a record, and drawing a placeholder for one puts a thing you
    cannot play in the one place everything is playable.

  *Needs: Settings readout, or is the log line enough for the beta?*
- **Move the one full search well into the app bar.** The owner settled the
  earlier maybe on 2026-08-12: *"I think we should move the search up into the
  top bar"*. Remove the lane well and narrow Library-strip copy rather than
  adding another road. The former layout blocker no longer exists: resting
  library totals now live on Home and the live match count sits inside the
  field, so the 232 px well plus seam fits the bar's measured 304 px slack.
  Type-anywhere focuses this resident `Search library` field without changing
  place; its non-empty state anchors the scrollable Tracks/Albums dropover over
  whatever page remains beneath it. This supersedes the current
  `reach_the_well` navigation to Library while retaining library-wide scope.
  *Needs later: priority and a minimum-width
  app-bar composition with Back/Forward, density marks, notification bell,
  Settings and conditional window controls; sufficient borderless drag region;
  responsive behavior that never duplicates/hides a standing query; and tests
  that place/width changes preserve query, selection, count and clear action.*
- **Does baz's chrome spend an accent that is not playback truth?** The app
  bar now draws the application's own icon in zone 1 (his *"we probably want an
  icon for our app to show in the bar"*), and the mark carries the lamp dot —
  about one pixel at 16 px, and permanently on screen in a place that says
  nothing about playback. It ships as a **stated exception** with a boundary
  (*the application's mark is the application's, not the room's ink*) rather
  than as an oversight. *Needs: keep it, or take the reversal* — a monochrome
  `Glyph::Baz` on the sheet, inked like every other mark in the bar, which
  costs a second drawing of the mark and keeps the accent discipline whole.
- **Should baz play Opus?** Promoted here rather than into `## Next`, because
  it is the largest *functional* gap in the product and the only one whose fix
  is a decision rather than work. `.opus` is out of `AUDIO_EXTENSIONS`
  entirely, so those files do not reach the shelf — nothing is skipped
  silently, there is simply nothing listed, which is worse in one way (a
  listener cannot tell baz from a missing folder) and better in another.
  **Symphonia has no Opus decoder in any released version** — 0.5's crate is a
  one-byte placeholder never published, 0.6.0 still lists it as `-` — so the
  route is libopus, which is C, and `BACKLOG.md` refused a C dependency in
  those words for `reqwest`'s TLS. *Needs: is Opus in his collection?* If it
  is not, the refusal stands on its own and this line can close; if it is,
  the price is worth paying and the decision reverses cleanly.
- **May baz make its first network request?** Doc 15 tier 4 / ADR-0037 §6,
  priced rather than argued. `ureq` 3.4.0 costs **14 net-new crates**, one
  new `deny.toml` licence (`CDLA-Permissive-2.0`), and no new build tool —
  but its TLS core, `ring`, is C and per-architecture assembly with a
  `links` key, and it would be the first C in baz parsing hostile input off
  the wire. `reqwest` is **57** net-new crates and `aws-lc-sys` with `cmake`
  + `bindgen`, which is `BACKLOG.md`'s Opus refusal word for word; it is
  recorded as a finding, not offered as a choice. **The most expensive line
  is not technical**: `packaging/flatpak/…yml` has no `--share=network`
  today, so this puts *"Network access"* on baz's Flathub page permanently,
  for an offline-first player. *Needs: yes or no.* **If no, that is a
  complete outcome** — doc 15 tier 1's `Look up` already puts the
  encyclopaedia one press away in his own browser for nothing.
- **Doc 15's Tier 3**, five questions, three of them about the play ledger.
  ADR-0018 §6 says **"no totals-by-artist"** in those words, so *first
  heard*, *last played* and *records never played* on an artist's page each
  reverse a written decision — and reversing one as a side effect of a page
  redesign is the failure `WORK.md`'s preamble exists to prevent. The
  smallest admissible form is drawn for him: `First heard 2019 · Last played
  3 months ago`, in `Recency::label()`'s own vocabulary, **no counts, no
  ranking, no comparison to another artist** — history as a door, which the
  returns lane already does without performing. The other two are a frame
  question (one band line or two) and the tile-size one below. *Needs: one
  sentence each.*
- **Done 2026-08-14 — borderless window chrome and iced 0.14.** The app bar
  now owns the window by default, including its minimise/maximise/close
  controls. A six-logical-pixel band inside every edge and corner spends
  upstream `window::drag_resize` in all eight directions and is disabled while
  maximised; the rest of the band retains drag, double-press maximise and the
  desktop system menu. `BAZ_NATIVE_CHROME=1` restores platform decorations for
  comparison and diagnostics. All five custom widgets, the jewel-case shader,
  boot/subscription APIs and the Linux zbus stack moved with iced 0.14; the
  obsolete `rustybuzz` advisory ignore is gone. iced still retains
  `ttf-parser` and `lru 0.16.4`, so the former's narrowly documented transitive
  ignore and the latter's audit note remain while Baz's own cache stays on
  fixed `lru 0.18.2`. Unit tests cover direction/boundary routing and the
  changed child-release capture semantics. A release build rendered cleanly
  under isolated X11/Xvfb with no platform frame and the expected Baz
  controls; an isolated live Wayland launch reached the interactive window
  normally. Item 15 remains at its
  explicit public-release boundary; this work did not publish anything.
- **Is the `Dense` display-option mark legible enough?** He said *"the way they
  appear for the library is nice"* and the four marks moved into the app bar
  unchanged on the strength of it. The fourth is a 4 × 4 whose cells minify to
  2.25 px at 1×, and beside the other three at the bar's real size it is
  visibly softer — `docs/design/impl/app-bar/12-marks-4x-*.png`, magnified with
  a point filter so the question is not answered by blurring it. A larger
  sprite for that one mark is small. *Needs: his eye on that frame.*
- **Doc 14's Tier 3**, three questions rather than tasks. Tiers 1 and 2 both
  shipped without touching any of them, and each needs one sentence from him.
  The first has got **sharper** rather than softer now that tier 2 has landed:
  the serif is on a record's page, so he can see the face at 28 px in the
  product before answering whether it should also be on sixty tile captions at
  13 px — `docs/design/impl/serif-titles/` has it magnified. Nothing in tier 2
  presumes an answer; the wall and the lane are untouched.
  - **Should a record's title be set in serif italic everywhere it is named —
    the wall's tile captions and the lane's rows included?** *Needs: his eye on
    a frame, not an argument.* It is the strongest possible answer to his own
    question — every record typographically a *work*, every playlist a
    *label*, at every size, with no badge — and it is also sixty italic serif
    captions on a wall of covers. He approved the serif once, for one string
    (Home's `CONTINUE` placard); this is a different magnitude. The 13 px
    legibility of an italic serif in a lane row is answerable only from a
    rendered frame.
  - **Should a playlist of one to three distinct records draw the rest tile
    instead of that record's cover full-bleed?** *Needs: yes or no.* It is the
    only change that makes the sleeve honest at every count and the direct cure
    for the loop doc 14 §0 names — but it costs a two-record list the best
    sleeve available to it, at 320 px on its own page. A genuine aesthetic
    trade with no right answer from the code, and aesthetics is his rule.
    (ADR-0024 §A1 rule 2; `views/mod.rs`'s deciding match.)
  - **On `Save as playlist` — did he mean *remove it* rather than make it make
    sense?** *Needs: his intent.* Tier 1 kept it and labelled it honestly,
    because the repo's one rule is that what he asks for goes in the app and
    the act is real for a shuffle, a `Play all` or an edited run. But he is the
    one who noticed it, and if the intent was *"this should not be here"* that
    is a sentence only he can write.
- **Resizing is still slow**, reported twice. Two commands separate the toolkit
  from us and have never been run on the machine that has the bug — Xvfb has no
  GPU and cannot reproduce it:
  ```sh
  ICED_BACKEND=tiny-skia baz        # smooth ⇒ wgpu surface reconfiguration
  ICED_PRESENT_MODE=immediate baz   # smooth ⇒ vsync/swapchain
  BAZ_MSG_LOG=1 baz                 # names what actually fires while dragging
  ```
  *Needs: one run on the owner's machine.*

## Recently done

- **The packaged Windows app no longer opens a companion command window.**
  Item 12. `crates/baz/src/main.rs` carries the crate-root gate
  `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem =
  "windows")]`, so release/package builds link as a GUI application and a
  normal launch from Explorer or the Start menu creates only the baz window.
  Debug builds keep the console, where the `[scan]`, `[playback]` and `[mpris]`
  diagnostics remain useful. Nothing a listener must know is console-only:
  user-facing failures already flow through the canonical health/event surface,
  so no release file/Windows-logging sink was added — the decided answer, and
  the skipped-file readout remains item 11's. Other platforms are untouched.
  The final acceptance launch of the actual packaged `.exe` on Windows is the
  owner's.
- **The release dependency gate is green again.** A newly published
  RUSTSEC-2026-0253 advisory made the existing `lru 0.16.4` fail `cargo deny`;
  baz now requires the fixed 0.18.2 release. The API is unchanged, and the
  complete dependency, lint and workspace-test gates pass with the new lock.
- **The artist's page is worth visiting, entirely offline** — doc 15 tiers 1
  and 2. Its one-line facts band now states playing time, release years,
  formats, up to three case-folded genres and the earliest added year, omitting
  facts the files do not provide. `ALSO ON` draws guest records in the wall's
  own tiles from one cached library fold. A bounded, off-thread image path
  reads `artist.jpg`, `artist.png` or `folder.jpg` above the album folder, and
  a quiet `Look up` opens Wikipedia through the desktop portal without giving
  baz network access or adding a crate. The page reads cached facts during a
  frame; no track walk or image decode was added to scrolling.

Newest first. Fuller detail in `CHANGELOG.md`.

- **Playlists have a collection root.** The lane's `Playlists` destination
  shows every saved list as a collage tile on the shared density grid, ordered
  either A–Z or by creation date, newest first. A playlist viewer now reads
  `Playlists › Name`; its first segment returns to the collection.
- **The volume fader survives a restart.** Its exact protocol position is
  restored from `config.toml`; only engine-confirmed movement is saved, and a
  pointer drag is coalesced to the release rather than writing once per pixel.
  Mute remains an independent, session-only switch.
- **Every artist has an `All songs` of their own.** One collage tile above
  `RECORDS`, drawn by the exact component Home uses, plays only that artist.
  Releases run by year then title, undated last, with every selected edition's
  disc/track order intact. It materializes as the current unsaved playlist;
  Now playing and the bottom bar open that editable queue, where it can be
  saved.
- **The bottom edge is for one song.** The attempted song/list toggle was
  rejected in use and removed completely: no selector, cumulative list time,
  queue segments or jump targeting remain. The full-width needle is a single
  continuous seek line for the current track, with track elapsed/total figures.
- **Now playing is one current song, not another list page.** The large centred
  cover, track-led placard, needle and figures are the whole composition. A
  quiet, borderless source footer spans the bottom of the place and leads to
  the originating playlist when it still exists, to the editable queue for an
  unsaved list, otherwise to the sounding track's album. The bottom bar's track
  block follows that same source route, while the lane keeps the dedicated road
  into the artwork view. The queue is not drawn on this surface.
- **The README is the project's public face, and the word on the front is
  `public beta`.** The owner's *"can we get the README sorted"*, and his
  earlier *"a real public facing view of the app and its features. with an
  icon and stuff."* Deliberately the last item before the tag, so it describes
  what actually ships.
  - **Four screenshots, all four re-shot, and the two committed ones were
    already false.** The app bar put a 41 px band above everything and
    `capture.sh`'s coordinates predated it, so its `Play` press landed on the
    tile instead of the overlay: the committed `library.png` is a wall
    captioned **`Nothing playing`** on a store page whose whole sentence is
    *click one and it plays*. Caught by looking at the frame rather than at the
    exit code, which is the eleventh time on this project that was the
    difference. Every coordinate in the script is now re-derived from a frame
    and written down beside the state it reaches.
  - **`Home` and a playlist join the wall and Now playing**, and the playlist
    is **built by hand in the running app** — four records through each tile's
    own `Add to…`, into a list named in the panel's own field. A `.m3u8`
    dropped into the folder before launch would have been a picture of a file
    rather than of the feature.
  - **The store frame moved down a rung of the ladder**, and that is a finding
    rather than taste: ADR-0028's second amendment retuned `Dense` to
    160 … 200 the same morning, which at 1600 px is 165 px of tile — narrower
    than `Marguerite Vance-Lindqvist · 1984`. The fixture's longest caption was
    photographing as an artist's name cut mid-line with the separator dangling.
    `compact` hangs the same three shelves and every caption is whole.
  - **The keyboard table was derived from `crates/baz/src/keys.rs`, not
    edited.** That was the right call and the old table proves it: besides the
    four errors this item already knew about, it was missing
    <kbd>Ctrl</kbd>+<kbd>P</kbd> and <kbd>Ctrl</kbd>+<kbd>Z</kbd> entirely,
    described <kbd>Ctrl</kbd>+<kbd>B</kbd>'s subject as an album inspector that
    has not existed since ADR-0022, named three densities where there are four,
    and gave <kbd>Esc</kbd> a peel order that ended at *the shuffle's marks* —
    a layer removed when shuffle became a mode. A wrong keyboard table is worse
    than none.
  - **`keys.rs`'s own module doc contradicted itself** and lost: its opening
    section says the search field takes focus at startup and the very next
    section describes the mechanism (type-anywhere, step 11) that made that
    false. `app.rs`'s `Shelf::open` is the truth — nothing is focused, so
    <kbd>Space</kbd> means play/pause on the first frame. The stale
    parenthetical is fixed.
  - **The maturity word was `pre-alpha` in six places and is `public beta` in
    all of them**, because it is one claim made to one stranger: `README.md`,
    `docs/INSTALL.md`, `CHANGELOG.md`'s `[0.1.0]`, the Flathub metainfo's
    description *and* its release note, and the GitHub release body that
    `release.yml` writes. The metainfo now carries the rule in a comment so the
    next edit does not split them again. **The honest half is stated where a
    reader stands**: the tag is not cut, the releases page is empty, and
    building from source is the way in today.
  - **The Flathub description and the README now say the same thing**, which
    they did not: the metainfo listed six capabilities and omitted playlists,
    ReplayGain, multichannel and multiple folders. Two more screenshots were
    added to the store listing with them.
  - **The owner's correction to the first sentence anyone reads**: *"make sure
    we say inspired by foobar but not a spiritual successor as they are
    different."* Fixed in `README.md`, `docs/VISION.md` (three places) and
    ADR-0002, which is where the claim originated — as an amendment rather than
    a rewrite, since the ADR is a record. The metainfo and the desktop entry
    never made the claim and still do not.
  - **Checked against the code rather than against memory, and memory lost
    twice.** The brief's *"scans folders you name"* is an ordered list with no
    names; its *"searches by song and by record"*, the `.m3u8` ownership, the
    trash, undo's depth, the downmix, MPRIS and the incremental rescan all held
    exactly. `1255 tests` held exactly too — it is what `cargo test
    --workspace` counts on Linux at default features, against
    `NEXT-STEPS.md`'s stale 1047.
  - **Found on the way, and left for the owner's eye:** an agent reading the
    code reported that *FLAC-in-MP4 is labelled ALAC* is not a real gap. It is
    — `library.rs:921` says so in its own words — and it is in the README's
    limitations. The lesson is the one this file keeps relearning: a claim is
    checked by reading the line, not by asking whether it sounds right.
- **`Now playing` makes sense at width, and the run column stopped being the
  third copy of the track list.** Two of the owner's asks on 2026-08-10, on one
  surface. Frames, both builds, at 1280 / 1920 / 2560:
  [`docs/design/impl/one-list-drawn-once/`](design/impl/one-list-drawn-once/README.md).
  - **Doc 12 step A4 (the run's half), and a second fault beside it.** The
    queued item had one cause; the owner's own phrasing — *"the playlist hugs
    right and the art hugs left"* — had two, and it was right. `RUN_MEASURE`
    was flat 440 at every size **and** the record column hung from the body's
    left gutter while the run was pinned to the right, so every pixel the two
    could not use piled up between them. Measured at 2560 × 1440: **1171 px**
    of bare field, not the ~700 this item carried — that figure assumed a
    1024 px cover and the field is *everything the work cannot use*, so a
    smaller cover leaves more of it.
  - **A4 alone would not have fixed it.** It takes the run 440 → 692 at that
    window and the gap 1171 → 919. The work there is bound by the **file**, so
    none of that field was the run's to give back; the pair centring is the
    other half. The gap is **36 px** at every size now, and 1280 × 860 is
    *pixel-identical* — the two columns' band diffs at **0**.
  - **The run now grows with the panel**: `RUN_MEASURE · kiosk_scale`, 440 up
    to a 720 px work, 472 at 1920, 692 at 2560, 1100 at 4K, capped so the
    record keeps `ART_MIN` above `SPLIT_FLOOR`. **Doc 12 §11.2's claim that
    every window at or below 720 is untouched is not quite true** and the code
    says so: a 1920 body's work is 773 px, because `below` is 146 today and not
    the 190 §11.2 was written against. Nothing a listener sees moves badly —
    the work is 773 px at either measure — but it is a real disagreement
    between the document and the build, recorded rather than tuned away.
  - **One row, drawn once.** A record's track, a playlist's entry and a run's
    row were three literal copies of one anatomy; the record head was two
    (`playlist::record_head`, `queue::album_group`); and `views::queue` held
    **four more copies** of the reserved icon slot that
    `impl/one-page-two-subjects/` had already shared for the two pages. All of
    it is `views::page::track_row`, `list_head` and `icon_slot` now. The
    refactor moves **no pixels**: both pages diff at 1–3 px between the builds
    outside the bottom bar's clock.
  - **What honestly did not merge**, each named where it is drawn: `DETAILS`
    (the owner's own *"album exploration type data"*), the next-track ring (a
    run has a cursor, a document does not), the trailing slot sets, the head (a
    *name* against a *position*), and the page composition itself — the run is
    a virtualized column inside another surface's two-column layout, not a
    document in one scroll.
  - **A test that could not fail**, found on the way:
    `queue::tests::the_queue_place_is_virtual_and_its_rows_are_the_playlist_editors`
    reads its own file, so every needle it spelled satisfied itself. It had
    gone stale twice — `window.height` after that argument became `viewport_h`,
    and a slot literal that had changed module — and passed both times. It now
    searches the code half only, the way `views::page::tests::pages` does.
- **The app bar hangs by its ink, and zone 1 is baz's own mark.** The owner's
  two corrections to the bar that shipped the same morning (ADR-0040's
  amendment; frames and the measurement in
  `docs/design/impl/app-bar-gutter/`).
  - **The gear stood 25 px inside the window's gutter** — *"the settings cog is
    padded in quite a bit and does not align with the rail"*. Measured, not
    eyeballed, and the same at both widths: the index rail's letters and the
    bottom bar's volume groove both end 41 px from the window's right edge, and
    the gear's ink stopped at 66. **16 px was a phantom seam** on the
    zero-width `Space` standing in for the absent window buttons — a row's
    spacing falls between *children*, and a shrink-width `Space` is one.
    **8 px was the hit box not being the drawing**: a 16 px sprite centred in a
    32 px box, hung by the box.
  - **The finding worth carrying forward is the false claim, not the pixels.**
    `views/app_bar.rs` said its right edge was `W − HANG` and that was true of
    the container and false of everything drawn in it; `views/shelf.rs` still
    cited *"the alignment edge the `Settings` word above already established"*,
    which had stopped being a word two ADRs earlier. Nothing in the product
    measured either, which is why a 25 px error survived a study with its own
    capture harness. The rule is now `const`-asserted and written over *the
    trailing control* rather than over the gear, so it holds when baz owns the
    chrome and the close button is trailing instead.
  - **Two things for the owner** are on *Waiting* above: the accent the
    application's mark carries, and his search question.
- **Removing a music folder no longer destroys `first_seen_ns` — and the two
  blockers that looked like opposites turned out to be one question.**
  ADR-0042. *"A deleted folder's records never leave"* forgets too little and
  *"removing a root destroys first-seen"* forgets too much, and the single
  answer is: **baz remembers, about music it was told to stop holding, exactly
  one thing — when it first saw it — because nothing else is unrecoverable.**
  - **Which is why they had to be answered together.** The reason a
    listener-initiated *forget* could not be offered was not that nobody had
    written the verb; it was that being **wrong** about it was unrecoverable,
    and on the owner's NAS being wrong is one unmounted share away. Fixing the
    data loss is what makes the destructive act safe enough to put in front of
    somebody. Two mechanisms would have disagreed; there is one, and
    `forgetting_a_root_and_forgetting_its_paths_leave_the_same_memory` is the
    guard that keeps it one.
  - **Schema v9**: a `forgotten` table of path + first-seen, written in the same
    transaction as the delete, consumed by the rescan that brings the path back,
    swept at open, primary-keyed so forgetting the same thing ten times leaves
    one row, and with **no expiry** — because the case is a folder removed one
    year and added back another.
  - **ADR-0019 §5 is not weakened.** `first_seen_ns` is still in the insert list
    and still absent from the update list; a rescan still cannot move it. The
    once-only write is simply given the true value instead of the clock.
  - **`remove_tracks` is untouched, on purpose.** ADR-0010's four gates are
    evidence baz gathered and evidence needs no reversal, so the scan's door
    leaves nothing behind. A listener's word is a decision, and decisions get a
    tombstone. That line is now in the code rather than implied.
  - **Checked rather than assumed**: the play ledger is a separate file and was
    never lost, playlists are files on disk and were never touched, and the
    measured ReplayGain is *recomputable* — so the tombstone stays one column
    wide. The criterion is written down: **remember only what nothing can
    recompute.**
  - **Proof, driven by presses**: `docs/design/impl/forget-and-remember/`. The
    ADDED wall before the round trip and after it differ in **zero pixels**,
    against timestamps planted four years in the past; the harness fails loudly
    on either assertion. `scan/launch_cold_10k` is 81.0 ms against ADR-0010's
    recorded 83.4 ms — the addition does not appear.
  - **Blocker 2's mechanism landed here, but its proposed control was later
    rejected by the owner.** Filesystem changes followed by automatic pruning
    are the product workflow; `BACKLOG.md` retains the unreachable-directory
    distinction that still needs solving.
  - **Found on the way**: `WORK.md` and `BACKLOG.md` both credited the removal
    policy to ADR-0011, which is the volume ADR. It is ADR-0010. Both fixed.
- **Fourteen files skipped were one whole album, and one junk byte was the
  whole cause.** The owner's launch scan on 2026-08-10 reported `14 files
  skipped` under his `CDs/MP3` root. All fourteen were Frank Zappa's
  *Unmitigated Audacity*, complete — an entire record absent from the wall,
  with a number in a status line as the only evidence it had ever existed.
  - **The files are fine and always were.** 320 kbit/s MPEG-1 Layer III,
    ID3v2.3, ripped by dBpoweramp. Every one opens and decodes through
    `AudioSource` — verified over all 3 735 MP3s under that root, zero
    refusals.
  - **The cause is `TYER`.** ID3v2.3's year frame, holding the single
    character `0` where the specification says four digits. lofty forgives a
    malformed frame *header* and not a malformed frame *body*, so it returns
    that as an error from the **whole-file** read, and the scanner turns a
    failed read into a skipped file — one unparseable byte in one optional
    frame cost the title, the artist, the album, the track number and the row.
    The read is now retried in lofty's `Relaxed` mode, which drops the frame
    instead of the file: 3 721 tracks and 14 skipped became **3 735 and 0**,
    losing only the year the tag never legibly held. Strict runs first and wins
    whenever it succeeds, so nothing that reads today reads differently.
  - **Not a regression from the format-registry change**, which was the first
    suspicion and a reasonable one — `file(1)` calls these files *"MPEG ADTS,
    layer III"*, exactly the shape the removed raw-ADTS reader could have
    claimed. It was tested rather than argued: both `fbb0af7` (before
    ADR-0040 §2.5) and `main` were built and run over the same folder, and
    both skip the same fourteen with the same sentence. **The scanner never
    touches Symphonia's probe** — it reads tags with lofty — so no change to
    the registry can reach it. A file can be unlistable and perfectly
    playable, and these were. ADR-0040 carries the exoneration so the next
    reader of that doc comment does not re-investigate.
  - **The second defect is the one that let this last.** The scanner's count
    was the *only* signal: `ScanEntry::Failed` carries a path and a reason,
    and `crates/baz/src/scan.rs` discarded both and incremented an integer.
    Nothing logged them, nothing stored them, and no surface in baz could name
    a skipped file. A scan now prints one `[scan] skipped <path>: <reason>`
    line per failure. **That is a floor, not the answer** — it is invisible to
    anyone running the Flatpak — and the surface question is in *Waiting on
    the owner* below.
  - **Seen from the other side, this is the same lesson as the conflict
    markers**: a defect whose only evidence is a line of output nobody is
    obliged to read stays alive for as long as nobody reads it. That one got a
    gate (`no_conflict_markers.rs`); this one cannot have the same kind,
    because no test can know which of a listener's files ought to be there.
- **A library from a newer baz is a statement now, not a first run.** The
  owner's *"it shows me 'where's your music' … it also tells me the schema
  version is version 8 if I pick any directory"*, answered with a distinct
  state (ADR-0041, `docs/design/impl/blocked-library/`). One line in `app.rs`
  turned **every** failure to open the library into the first-run screen, so a
  correctly-refused database from a newer build was reported as *you have no
  library* — and every door on that screen led straight back into the same
  refusal, which is the loop he was in.
  - **The data was checked before the words were chosen**, because that is the
    difference between a presentation defect and a data-loss one. It is safe,
    and it is now safe *by construction*: `Library::open` reads `user_version`
    before it sets a pragma (`journal_mode` is persistent, so the old order
    could have rewritten a header on the way to refusing to touch the file),
    and a test opens a stamped database **three times** — the retry a listener
    performs by typing folders — and compares the bytes each time. The capture
    script prints the same fact as a SHA-256, unchanged after both builds have
    been run against it.
  - **One screen, three reasons, not three screens**: a newer baz, an index
    that cannot be read, and a machine with nowhere to keep one. All three say
    *"Your music and your playlists are untouched"* in the same place, because
    in all three it is true. What differs is the rest of the words and **which
    controls exist** — `Try again` only where trying again could change the
    answer, which is not the case for a schema version.
  - **The escape hatch renames and never deletes.** `set_aside` moves
    `library.db` (and its write-ahead log) to `library.db.set-aside-1`, so
    *"nothing is deleted, renaming it back restores it exactly"* is a round
    trip a test performs. It is never the default, never the only control, and
    the first press only **reveals** what a new index costs — the ADDED dates,
    which is `first_seen_ns` and is the one thing in the schema a rescan cannot
    recover. On a downgrade the revealed paragraph opens by saying *this is not
    the fix*.
  - **Checked and left alone**: a music folder that has gone away never reaches
    either screen. The library opens, the scan reports it unavailable, and the
    strip says *"1 folder is not reachable"* with every record still on the
    wall (ADR-0022). That case was already answered.
  - **Found on the way**: a `cfg(test)` helper added to `app.rs` silently
    truncated the source that several `views` tests read, because they split
    that file at its first test attribute. It failed loudly rather than passing
    vacuously, which is the design working — but a test-only constructor in
    `app.rs`'s head is now a thing not to add.
- **The app bar — baz draws the window's chrome, and it is the same band in
  every place.** Three asks in one change (ADR-0040): `Play all` removed, the
  display options moved to the top bar, and a resident app bar carrying them
  with the gear and the window buttons. It drags the window, maximises on a
  double press, and right-presses to the desktop's own window menu; the buttons
  are minimise, maximise, close, on the right, always.
  - **The admission rule is the owner's and it is a test, not a sentiment**:
    *"adding controls that apply to all windows makes sense in the top bar"* —
    a control enters only if it applies in **every** place, and if it applies to
    one place the bar is not where it goes. That is what puts `Play all` out
    (the Library's alone) and the gear in (the application's), and it is what
    the closed tenancy is asserted against, because the failure mode of a
    resident bar is accretion.
  - **The tension worth knowing about**: ADR-0028 decided that morning that a
    density mark must be *absent* rather than present-and-inert where no works
    hang, and *"the same on all screens"* pulls the other way. Resolved by
    making the **slot** resident and the **control** conditional — so nothing
    is ever inert, and the gear and the window buttons are in register across
    all seven places (`docs/design/impl/app-bar/10-every-band-after-*.png` is
    that claim as one picture).
  - **The strip got smaller rather than rearranged**, which is the point
    against the owner's standing *"just adding stuff into that top bar isn't
    good"*: it lost two tenants, and `TOP_BAR_SPLIT` fell 824 → 680 with them.
  - **Found on the way**: `Message::PlayAll` and `App::play_all` were deleted
    with the button. A message no control sends is the visible-control rule
    failing in the direction nobody checks for.
  - A `chrome` module that read GNOME's `button-layout` and KDE's `kwinrc` and
    mirrored the bar was built and then **deleted**, on his *"I don't mind if
    we have the controls on the right hand side as long as we have a sensible
    consistent pattern"*. macOS will look foreign; the reversal is one line and
    is recorded in ADR-0040 §4.
- **A 29-byte malformed FLAC asks baz for 4 GB — and the answer is that the
  bound is symphonia's, while the *panics* were baz's to catch and are caught.**
  ADR-0040. Reproduced first, as the item asked: `cargo fuzz run
  playback_decode` on the artifact, frame 18 in `symphonia-metadata`'s
  `read_picture_block`, `vec![0u8; media_type_len]` from an unchecked `u32`.
  - **Whose bound, settled with four findings rather than an argument.** It is
    **unfixed in 0.6.0** as well as 0.5.5, so an upgrade buys nothing.
    `MetadataOptions::limit_visual_bytes` and `limit_metadata_bytes` — the API
    *designed* for this bound — exist in `symphonia-core` and are read by **no
    reader in the released tree**. The same demuxer checks a block length
    before allocating twenty lines away, and `symphonia-format-riff`'s
    equivalent site is commented `// TODO: Apply limit.` And it is a **class**:
    four sites in two containers, three of them found by the fuzzer and one
    built by hand to test the hypothesis, with Ogg and five MP4 sample-table
    sites behind them.
  - **So no guard, and the reason is not effort.** A header walk catches these
    four inputs and misses the real case, a large honest file with one corrupt
    inner length; a body walk is symphonia's parser rewritten four times; and
    the day baz's copy and symphonia's original disagree, baz refuses a file
    that *plays* — which for a music player is the worse failure. WAV files in
    the wild declare `0xFFFFFFFF` routinely, so even a blunt size-sanity rule
    would refuse real records. The residue is in `docs/BACKLOG.md` with all
    three reproducers in base64 and the upstream report written out, because
    filing it is a GitHub account and not an agent's to do.
  - **The severity was measured and it is not what the item said.** *"An
    allocation the machine cannot serve"* is wrong on the machine baz ships
    to: the 4.28 GB `calloc` is a lazy zero mapping, `open` returns `decode
    error: out of bounds`, peak RSS **3.4 MB**. Under `ulimit -v 2G` the same
    call is `memory allocation of 4278208769 bytes failed` and `SIGABRT`. So
    the real exposure is a small machine, a container limit, strict overcommit
    or a 32-bit build — and libFuzzer's `-malloc_limit_mb` is what turned it
    into a red job.
  - **The fuzzer found something worse on the way, and that *is* fixed.**
    **Three panics in symphonia's AAC reader**, reachable from
    `AudioSource::open` in 27 to 33 bytes — two while opening (`step_by(0)`, a
    subtraction underflow) and one while decoding (a band index one past the
    end). No overcommit caveat: a panic kills the decode thread on every
    platform, and **all six of `AUDIO_EXTENSIONS` reach it** on the same bytes,
    because a probe identifies a stream by searching its bytes. Answered
    twice. **baz now probes only for the formats it plays**: the raw-ADTS
    demuxer is out of the registry, because `.aac` is not a scanned extension
    so that reader could only ever have fired on a file that is not what its
    name says — which is exactly where it fired. And baz **contains an unwind**
    at `open`, `next_block` and `seek`, for the parsers it does keep — a
    boundary that knows no container, cannot refuse a file that plays, and does
    not grow when symphonia grows a format. Two findings fell out of the first:
    MPEG audio's sync word and ADTS's **overlap**, so two readers were
    competing for the same corrupt `.mp3`; and nothing in the suite covered the
    ID3v2 metadata reader, whose absence would have stopped MP3 ReplayGain
    silently — it has a test now, confirmed to fail without the registration.
  - **Five more defects, every one of them baz's own, all fixed outright.**
    Seven minutes per target on all six targets, from an empty corpus:
    - the **play ledger's date arithmetic** multiplied an unbounded year into
      days (`1120120120176761-02-15T10:44:44Z`, found in under a minute) —
      a panic under overflow checks and a *wrong instant* without them;
    - a **mis-encoded ReplayGain tag** panicked `AudioSource::open`, because
      `parse_gain` cut two *bytes* off the end of text that came out of a file
      to look for `dB`, and the cut could land inside a character;
    - and **three ways a playlist edited itself on every save** — a comment
      losing one carriage return per save, a superseded `#EXTINF` hopping over
      the comment beside it, and a title ending in a vertical tab being
      shortened by the writer but not the reader. Each is a break of the
      module's own round-trip law, which is why each repeated on every save.
    `protocol_deserialize` and `scanner_inference` came out clean.
  - **Everything the fuzzer found is now a `push` gate**, in
    `crates/baz-core/tests/hostile_media.rs`, driven through the real on-disk
    `open` under every scanned extension — because the fuzz job is skipped on
    `push`, so a reproducer in a corpus is not a gate at all.
  - **`playback_decode` is still expected red, and saying so is a correction
    to this branch's own first draft.** It claimed the job could now go green;
    a verification run said otherwise inside a minute, with **122 bytes of
    ISO-MP4** that reach `symphonia-format-isomp4`'s `atoms/mod.rs:449` and
    `attempt to add with overflow`. baz survives it — that is §2's containment
    on a parser baz *keeps*, which is the live demonstration the ADR had
    otherwise lost — but **libfuzzer-sys's panic hook aborts before
    `catch_unwind` runs**, so no containment can make a panic invisible to the
    fuzzer, and none should. Backlogged with its reproducer.
    - Two things did change so the job is still worth reading: the loop now
      runs **every** target and fails at the end with the list (a `run:` block
      is `bash -e`, so one red target used to hide the five after it), and
      `-malloc_limit_mb` goes to 6144 with `-rss_limit_mb` kept at 2048, so
      the *reservation* class does not also turn it red. The first attempt at
      that set the flag to `0`, which in libFuzzer means *"use
      `rss_limit_mb`"* rather than *"off"* and changed nothing — caught by
      re-running, not by reading.
  - **The release rehearsal no longer inherits discovery fuzzing.** A direct
    manual CI dispatch and the weekly schedule still fuzz; PRs, pushes, release
    dry runs and tags run the hostile-input regression tests instead. That
    gives the rehearsal and tag the same gate and lets the rehearsal build.
- **The ladder only tightens** — the owner, looking at the running app: *"why
  is balanced smaller than compact... I think the dense should be a bit
  smaller."* Two things in one sentence, and they are kept apart in the commit
  because one is arithmetic and one is taste. ADR-0028's second amendment;
  sweeps and frames in `docs/design/impl/the-ladder-only-tightens/`.
  - **The defect, verified before it was fixed.** Each step brings its own
    `hang`, and `art = (w − (columns + 1)·hang) / columns` **rises as the hang
    falls** — so wherever two steps landed on the same column count the
    *tighter* one drew the *larger* work. **30 of 96 widths** swept 700–2600,
    and at the shipped window `Balanced` hung 3 × 242.7 against `Compact`'s
    3 × 253.3.
  - **It is older than `Compact`, which the history says rather than the
    assumption.** The same sweep against the three-step ladder inverts at
    **11 of 96** — `Spacious` under `Balanced` at 720 … 780 and 1060 … 1140 —
    so the fourth step exposed the defect and did not cause it. It has been
    true since `b935a4e` gave the wall a zoom.
  - **The test asserted the wrong quantity, and the file knew.** Column count
    is monotone by construction and was right the whole time;
    `a_tighter_step_never_hangs_fewer_works`'s own doc comment had written the
    art inversion down as a property (*"at 1120 px Spacious hangs 3 × 309.3
    while Balanced hangs 3 × 320"*). `the_ladder_only_tightens_the_work_it_
    draws` now sweeps every whole pixel of the band and every quarter pixel
    below 420, asserting on `art` — a single width proves nothing, since 880
    inverted and 920 did not.
  - **Fixed in the construction, not in a guard.** `Density::art_max` is
    **derived** — the next-looser step's `art_min`, and `art::THUMB_PX` at the
    loosest — so the four art ranges abut and cannot overlap, and a tighter
    step's largest work *is* a looser step's smallest. `Grid::art_cap` adds
    `w − 2 × WIDEST_HANG` for the degenerate tail below 416 px, which is what
    finally makes `ART_FLOOR`'s own promise about inversion true. Clean at
    quarter-pixel resolution from 0 to 4000 px.
  - **It moves the default wall, and that is in the commit rather than in a
    footnote.** `Balanced` caps at 288 rather than 320, so **744 of 2261
    widths** draw smaller art with wider gutters, and three rows of §7's
    published table changed. At those widths the default step had been drawing
    Spacious-sized covers; about 132 of the 744 were not inversions, and they
    are the price of the ranges being disjoint rather than merely ordered.
    **Worth the owner's eye** — it is the one part of this that touches a wall
    he was not complaining about.
  - **`Dense` is 160 … 200**, his taste, and `Compact` is re-derived rather
    than re-tuned (still the widest rung halved). The floor is `THUMB_PX`
    halved and one hang above `CONTINUE_SLEEVE`, the smallest sleeve the
    product identifies a record by — `ART_FLOOR` 1.0 was never a design floor.
    It costs the first amendment's *"nobody loses what they have"* claim,
    knowingly: `Dense` is no longer the shelf baz drew before density existed.
- **A 5.1 record is a record baz has** — the queue's *"multichannel files do
  not play at all"*, answered with the ITU-R BS.775 stereo downmix. **3.0, 4.0
  (quadraphonic), 5.0 and 5.1 play**, in WAV, FLAC, Vorbis and ALAC. The matrix
  is written down where the next reader will find it
  (`crates/baz-core/src/playback/downmix.rs`), with the recommendation cited and
  cross-checked against a second implementation. ADR-0039; measurements,
  fixtures and the generator in `docs/design/impl/multichannel-downmix/`.
  - **The layout is read, never inferred from the channel count.** Which plane
    of a decoded packet holds the centre channel is a property of the container
    *and* the codec, and they disagree — Vorbis's bitstream orders 5.1 as
    `FL FC FR BL BR LFE` against WAVE's `FL FR FC LFE Ls Rs`, and ALAC declares
    no layout in the container at all. **Measured**: the same music through five
    containers, a distinct tone in each speaker, profiled per frequency per
    output — all five produce the same stereo pair. A fold that assumed WAVE's
    order would have put Vorbis's centre channel in the right output, which is
    audible and which no test that checks lengths catches.
  - **Clipping is answered by a constant, not a limiter.** The matrix's
    worst case is +7.66 dB and it is reachable by ordinary loud material, so
    every coefficient is scaled by the reciprocal of it: −7.66 dB for 5.1,
    −4.65 dB for quad, provably no overflow for any input at any position. A
    limiter was rejected on a structural ground before a taste one — it is
    stateful, and the decode path must be a pure function of position or a
    seeked decode stops matching a whole-file decode.
  - **The cost is named, and paid for by something baz already has.** A 5.1
    file plays 7.66 dB below its stereo master until it is analysed; the
    ReplayGain pass measures this decoder's own output, so it recovers the
    level exactly. **Measured** end to end: 766 centidecibels, derived from the
    matrix rather than from a previous run.
  - **A downmixed track is not bit-perfect, and now the readout admits it.**
    ADR-0009 and ADR-0012 promise baz converts nothing; a matrix fold is a
    conversion. `Event::SignalPath` carries `source_channels`, and a
    multichannel file plays under the exclusive path *folded and labelled*
    rather than being refused there — the output has always been opened stereo
    in both modes, so nothing was ever going to reach a converter as six
    channels.
  - **The refusal is narrowed, not removed.** 7.1, 6.1, height and wide
    channels and half a surround pair still fail — BS.775 places one surround
    pair and does not place a rear centre, and folding two pairs at −3 dB each
    would be a coefficient invented here. The error now names the layout it
    found. `docs/BACKLOG.md` narrowed accordingly.
  - **Found while building it, and left for the owner's eye:** the brief asked
    for *"centre and LFE folded at −3 dB"*, and the LFE is **dropped** instead.
    BS.775's equations contain no LFE term; folding a band-limited effects
    channel mixed +10 dB hot into a stereo pair puts subsonic energy the mix
    engineer never auditioned into two loudspeakers, and libswresample's
    `lfe_mix_level` defaults to `0` for the same reason. Recorded as a
    departure rather than absorbed quietly — it is one row of the table if he
    disagrees.
  - **Separately, and not ours: multichannel AAC does not decode at all.**
    Symphonia 0.5 rejects a 5.1 AAC stream with `aac too complex` before a
    frame exists. Pinned by a test that will fail the day that changes, at
    which point the fold is already there waiting for it.
  - **No rescan.** The scanner reads headers and never looked at a channel
    count, so multichannel files have always been *listed* — they refused to
    play when clicked. Proven rather than assumed
    (`a_multichannel_file_is_listed_like_any_other`).
- **v0.1.0 is cut up to the tag, and the tag is still the owner's.** Everything
  `docs/RELEASING.md` §"Cutting a release" asks for before step 7 is on this
  branch: the workspace at `0.1.0` in `Cargo.toml`, `Cargo.lock` and the
  `baz-core` dependency entry; `CHANGELOG.md`'s `[Unreleased]` moved into a
  dated `[0.1.0]` section with an empty `[Unreleased]` above it; the metainfo's
  placeholder release entry replaced with the real one; and **two screenshots
  Flathub can actually show**, which was the one deliverable that did not
  exist. `docs/RELEASING.md` §"What is left for the owner" is now the whole
  remaining list, in order, and it is four commands long.
  - **The screenshots come from the real binary**, headless on Xvfb with all
    six XDG redirections, driven the way a listener drives it: rest on a
    record, press its own `Play`, then the lane's `Now playing` row. Nothing is
    deep-linked. `docs/screenshots/capture.sh` is the harness and re-running it
    is how they stay true.
  - **The fixture is retagged before it is photographed**, because a test
    fixture is allowed to say things a store page is not: the deliberately
    clipping album title, the track titles carrying their own index, and the
    four sleeves drawn with the first two letters of the album's name. One
    generator still, with a pass over it.
  - **The wall is hung by decade at `dense`**, chosen by photographing the
    alternatives rather than by taste: the wall breaks a row at every group
    boundary, and this fixture has two or three records per band, so the
    default `artist` grouping photographs as a column of pairs with two thirds
    of the window empty. The frames of the rejected arrangements are not
    committed; the reasoning is, in `capture.sh`.
  - **The `workflow_dispatch` dry run has now happened** — the first time
    `.github/workflows/release.yml` has ever run at all. It ran against `main`
    and not against the release commit, because `workflow_dispatch` takes a
    branch and an agent may not push one; the owner re-runs it on the pushed
    release commit as step 2 of his list.
  - **It went red, and the red is the entry above this one** (it was item 1 of
    *Next* until ADR-0040 answered it). The `version` job and every
    CI job but one came out green; the fuzz job — which had never run anywhere,
    because it goes on `schedule` and `workflow_dispatch` and neither had
    fired — found an OOM in forty seconds. `build` `needs` the gate, so **the
    three platform builds and the checksum step did not run in CI**. They are
    the last unexercised thing in the release path. The reusable workflow now
    takes an explicit `run_fuzz` input: direct manual CI defaults it on, while
    the release passes `false`. A corrected dry run and a tag therefore use
    the same gate and both reach the builds; the former publishes nothing.
  - **Nothing was tagged, pushed or published**, which is the standing rule and
    not an omission.

- **The artwork crosses when the record changes** — the owner's *"when
  changing track there isn't any kind of nice visual transition for album art
  in now playing. we should have something a bit nicer, like a quick fade"*.
  A 200 ms linear dissolve of the Now playing hero, with the field crossing on
  the same number. Filmed at 60 fps and read back frame by frame:
  `docs/design/impl/art-crossfade/`. ADR-0020's third amendment (which
  **reverses** its own §3 refusal of album-art crossfades, narrowly and with
  the argument written down), ADR-0029's.
  - **`motion::DISSOLVE` is `motion::LAMP`**, not a copy of its digits: the
    lamp warms because the light moved to another record and the hero crosses
    because that record's picture changed, so they are one event and land on
    the same tick. No number was invented; 90 ms was considered and is a cut
    with a smear on it.
  - **The predicate is the handle being drawn.** A twelve-track record is
    twelve track changes, no transition and no clock.
  - **The surface holds what it has until the incoming hero lands.** That is
    what makes it a dissolve rather than a fade to nothing followed by a pop —
    and it removed a wart nobody had reported: a record change used to cut to
    the 320 px thumbnail on a room with no field and then pop to full size.
    Both cuts are in `01-the-cover-crossing-before.png`.
  - **Measured**: twelve distinct frames on screen against the old build's
    one — and twelve is what `a_200ms_transition_is_about_twelve_frames_at_60hz`
    derives from the tween's arithmetic with no window in sight. The cover's
    fraction and the field's never part by more than 0.018. CPU at rest is flat
    between the two builds.
  - **Found while building it, and left for the owner's eye:** the hold is the
    wait for `art::load_hero` — **33 ms** on a quiet machine with 600 px
    fixture covers, 100–320 ms on a loaded one, and longer for a library of
    3000 px scans. It goes to **zero** the moment the *successor's* hero is
    prefetched, which `art::HERO_CACHE_ENTRIES` already describes as one line
    once ADR-0034's `Origin` work can name the next record. **The crossfade is
    the first consumer that makes that prefetch worth having**, so if he finds
    the hold long, that is the fix rather than a shorter tween.
  - The two refusals kept: a record with **no art** does not dissolve (the
    gradient is a stand-in, and fading a stand-in is decoration), and two
    covers whose decodes resolve to **different squares** do not (a dissolve
    that was also a resize would animate geometry).

- **The frame is the frame in every place, and now it is.**
  `views::place_header_led` boxes its lead at `TRANSPORT_HIT`, so a strip led
  by a word is the same 49 px as a strip led by a control. Frames and the
  hairline measurements at `docs/design/impl/one-frame-everywhere/`.
  - **The item this closes was wrong about two of its three places, and that
    is the useful part.** It said *"Queue, Settings and the Artist place all
    sit 12 px above"*. Measured: **`Place::Queue` no longer exists** — it was
    deleted when the run column merged into `Now playing`, and the item
    outlived the place it named. **The Artist place was already at 48**,
    because it had grown its own copy of the box. Only **Settings** was
    actually drifting, at 36.
  - So the defect was one place wide and the fix is three places wide: one
    answer where there were two local copies (`views/page.rs`'s and
    `views/artist.rs`'s, both deleted here) and one absence. The artist page
    **does not move** across the change, which is the evidence that the shared
    box subsumes the local ones exactly rather than merely agreeing with them
    today.
  - The `y = 105`-style measurements the item said to re-read do not exist in
    `docs/design/impl/queue-merged/` or `docs/design/12-now-playing-and-kiosk.md`
    — searched for and not found. Neither surface uses `place_header_led`, so
    neither moved.
  - **Two harness faults are written up rather than quietly fixed**, because
    both produced frames that looked like results: a click at the wrong `y`
    photographed the record's page and labelled it the artist's, and two agents
    writing a binary to the same scratch filename made one run compare this
    branch's base against a different branch's build. The second is why the
    Library appeared to move at all, and it was caught only because *"the
    Library must not move"* had been written down first.

- **Density: a fourth step, and the control wherever the works are** — the
  owner's *"we should ensure the density options are available on all
  pages..."* and *"4 levels makes sense to me"*. ADR-0028's fourth-step
  amendment; measurements, every step on three pages at two windows, and the
  before/after of the defect below at
  `docs/design/impl/density-on-every-page/`.
  - **`Compact` goes *inside* the ladder, and that was measured rather than
    chosen.** Swept at the width the wall really gets, for seven windows in
    both lane states: `Balanced` → `Dense` jumps two to four columns at every
    window from 1280 up, where `Spacious` → `Balanced` jumps nought or one.
    Both other directions are closed — **looser than `Spacious` cannot draw a
    larger work at all**, because its `ART_MAX` is already `art::THUMB_PX`, and
    tighter than `Dense` would leave the widest rung exactly where it is. The
    numbers are that rung halved (208 / 236 / 280) with the hang's midpoint 34
    taken to the 4 px lattice's 32; nothing is tuned, and a test says so.
  - **The three shipped words keep their spellings** and `Balanced` is still
    the default, so nobody's wall re-hangs. The new word is `compact`.
  - **Density does not apply to a page of rows, and the marks are absent
    there** — decided, with the reason written down. A track row's height is
    `TRANSPORT_HIT` 32, which is the **pointer-target floor** rather than a
    spacing choice, so a tighter step would break the very rule ADR-0028
    exists to serve and a looser one could only pad text. The alternative
    placement that would have put marks on all seven places — the returns lane
    — is refused for the same reason: on four of them they would be *present
    and inert*.
  - **The marks stand at the trailing edge of the block of works they hang.**
    On the Library that is the index rail's lane, unmoved and unchanged; on
    Home and an artist's page it is the block's own section rule. Not the top
    bar — and that needed no appeal to his standing complaint about it, since
    doc 07 L8.1 already makes density's subject the viewport.
  - **The keyboard was half the defect and is fixed by the same change.**
    `Ctrl`+`=` / `Ctrl`+`-` were never gated by place — they stepped the state
    from anywhere, and Home and the artist page named `Density::Balanced` in
    their own source, so off the wall the keys moved nothing on screen. Not a
    line of `keys.rs` changed.
  - **This answers *"should the artist page's covers be the wall's size to the
    pixel?"***, which stood in *Waiting on the owner* asking for a frame rather
    than an argument. It has one: at 1920 with the lane collapsed the page drew
    **six columns of 244 px art where the wall drew five of 294** — the same
    record, one press apart, 50 px different. Both pages resolved a grid of
    their own; now the shell resolves one and hands it down, and
    `every_place_that_hangs_works_hangs_them_on_one_grid` fails if a view file
    grows a `Grid::new` again. It costs those pages 22 px at the trailing edge.
  - **Needs his eye on one frame** (`07-rail-foot-*`): the `Dense` mark is
    sixteen squares now. The detents depict the wall at their own hang and
    there is no whole number of columns between two and three, so the family
    re-keyed — `Compact` wears the old 3 × 3, `Dense` a new 4 × 4 whose cells
    minify to 2.25 px at 1×. If it reads as mush rather than as a grid, a
    larger sprite for that one mark is a small change.
- **One page, two subjects** — the owner's *"can we reuse the basic layout and
  view of the playlist for the album view and the playlist view accessed via
  clicking into info — right now they are different but for no good reason."*
  ADR-0024 §A2 had given the two pages one arrangement; what shipped was a
  second **copy** of it. `crates/baz/src/views/page.rs` is that arrangement
  written once, and the two pages hand it what is about their subject.
  ADR-0024 §A2 (amended) and §A4.5; frames from both builds at
  `docs/design/impl/one-page-two-subjects/`.
  - **His second sentence needed nothing built.** *"clear via some sort of
    title/subtitle telling us if it's an Album or a Playlist"* is what design
    14's tiers 1 and 2 shipped: a serif italic title over a person's name
    against a sans name over `Playlist · 12 records`. The frame of the two
    blocks at one crop says so, so no eyebrow is drawn. If a later frame says
    otherwise, an eyebrow goes on **both** pages or neither.
  - **The strip leads with the subject on both now.** A playlist's page led
    with the word `Playlist`, which is the kind stated twice — 58 px above
    the byline that already says it, 4 px smaller.
  - **Found in a frame, and it is the reason the harness shoots two builds:**
    the quiet act hung from two lanes (x = 115 against x = 12), and **a
    playlist's whole page rode 12 px higher than a record's**.
- **The lists have a section of their own in the lane** — the owner: *"I guess
  we need to add playlists into their own section under library"*. A **split,
  not an addition**: `RECENT` already held every playlist mixed with the last 24
  records, so what shipped is `PLAYLISTS` (every list) above `RECENT` (records
  only), under the head rather than inside it — the three destinations are a
  closed triple and a section between two of them would split it. It reverses
  his own *"recent albums and playlists mixed based on some order"*, so ADR-0030
  is **amended (sixth), not rewritten**, and both sentences stay on the record.
  The one order is untouched — last touched first in each section, so a list
  played this morning moved section and not rank; alphabetical was considered,
  declined, and named as the reopen if the section ever outgrows an eye. The
  real risk was the unbounded section: `PLAYLISTS` has no cap, and two
  scrollers or a fixed first section would have put `RECENT` off the bottom of
  the window at about a dozen lists, so **both sections live inside the one
  scroller the lane already had** — proved at thirty lists, expanded and
  collapsed, at 1280 and 1920 (`docs/design/impl/playlists-section/`). Two
  things found on the way and left alone deliberately, both needing the owner's
  eye: the **playlist panel is not made redundant** by this (it exists for
  simultaneity while collecting, which no resident section provides — only its
  *index* job is now labelled rather than merely complete), and ADR-0030 has
  **two amendments both headed "Fifth"**, noted in the new one rather than
  silently renumbered.
- **A multi-CD album is one record** — the owner's *"it would be good if multi
  CD albums were a single item"*. ADR-0038; fixture, before/after frames and
  the shape table at `docs/design/impl/multi-disc/`.
  - **Three of the four shapes already were one item**, and that was
    established with a fixture of real tagged files before anything was
    changed. The grouping key is (album artist, album title) and reads no path,
    so discs sharing an `ALBUM` tag were always one record whether they sat in
    one folder or two — and `disc` has always been the third field of the
    library's sort key, so a merged set has never played its two track-ones
    interleaved.
  - **The shatter was the disc in the *title*** — `… (Disc 2)`, `… CD2`,
    `… [Disc 2]`, which is how a great many rips arrive. `split_disc_marker`
    takes it off: three words, one or two digits, at the end, on a bracket or
    whitespace boundary. A closed list, never a distance.
  - **It fires only when a sibling exists.** A lone `Bitches Brew CD1` keeps
    its name; the rule can never rename a record it did not merge. That is the
    ADR-0008 posture held as far as it can be held, and the ADR is explicit
    about what it costs where it is let go.
  - **The marker also supplies the missing disc number**, which is the
    correctness half: a `CD1`/`CD2` rip that never wrote `DISCNUMBER` now plays
    in disc order and its page draws the breaks. Tags still win where both
    exist.
  - **Left unmerged deliberately**: two folders with no disc signal at all
    (shape 4) still interleave, because folder names are evidence about nothing
    and inventing an order from them is the guess this project does not make.
- **A shuffled run's continuation counts records, not visits** — the owner:
  *"the album count in the bottom bar when in shuffle mode is weird... way too
  many albums shown"*. `continuation` folded only *adjacent* items sharing an
  album title, so a shuffled walk opened a fresh entry every time it returned
  to a record: seed 0 of a three-record run read `then 10 albums`. The old rule
  is not wrong, it is narrow — adjacency is a statement about *the listener's
  own order*, which a shuffled walk has none of to break — so the fold is by
  title under shuffle and by adjacency without it, and
  `a_record_stacked_twice_is_two_entries` keeps its deliberate reading. The
  function's own comment had claimed shuffle *"generalises rather than
  changes"* here; that claim was the defect. Pinned by both readings of one
  five-item run, and by a 32-seed sweep of the shuffle the player really
  performs — confirmed to fail before the fix, because this defect does not
  appear until the walk happens to return.
- **The ledger remembers the list** — the owner: *"when I play a song from a
  playlist it should only bump the recency of that playlist, not the underlying
  albums please"*. The live half already worked; this is the **cross-quit**
  half, which `docs/BACKLOG.md`'s first entry had carried as **Owner decision**
  since the live fix landed. ADR-0034 §2–§5 shipped: `SetQueue` carries the
  run's origin, the ledger opens each run with a `# baz run` comment, and the
  lane's launch fold reads the markers. `docs/BACKLOG.md`'s entry is struck.
  Frames of his own check — play, quit, relaunch — at
  `docs/design/impl/ledger-remembers-the-list/`.
  - **The backlog's own prescription was wrong, and specifying it is what
    found that.** It called for *"a sixth field in the ledger line (format v1 →
    v2)"*. `format::decode` rejects a six-column line outright and ADR-0018 §3
    guarantees the file is never rewritten, so a v2 writer would have left a
    permanently mixed file that every older baz reads as **partly corrupt** —
    silently losing those plays from the play counts, the `PLAYED` key and the
    lane. `#` lines were already skipped and already not damage, so the grain
    of the file changed and the grammar of a line did not. Every byte-exact and
    four-tab pin passes unmodified; `command_wire_format_is_stable` was not
    touched, and `protocol.rs` has 92 insertions and no deletions.
  - **Found on the way, three things.** (a) ADR-0034 §1's `Album { id, name,
    artist }` **cannot round-trip** through §3's encoding, which has one
    display field — `Album` and `Artist` carry `id` and `name`, and the lane
    resolves the artist from the index as it already did. (b) §4's fourth rule,
    a second header block appended to an older file, is **refused**: detecting
    "already appended" in an append-only file needs an unbounded scan at every
    launch or a second store of a fact the ledger should hold. ADR-0018's
    amendment records it. (c) Marking a run *excludes* its plays from the
    records they quoted, so a kind `lane::subject_of` cannot credit must never
    be written as a marker, or the touch is lost rather than moved — asserted
    as `no_kind_is_written_that_the_lane_cannot_credit`, and it is the rule the
    §1 work below has to satisfy.
  - **And the mark, which he read off the same surface separately**: *"I still
    see albums specifically appearing as if they are playing rather than the
    playlist … it is showing next to the album rather than the playlist"*. The
    lamp dot follows the run's **origin** now, through `lane::sounding_subject`
    — the same call the recency ordering makes, so the dot and the order cannot
    drift into two answers about one run. `views/lane.rs`'s argument against a
    list lighting *incidentally* is **kept and amended rather than deleted**: it
    is still true, and it never reached the case where the list is what the
    listener put on. Frame at
    `docs/design/impl/ledger-remembers-the-list/04-…`.
  - **A defect this change introduced and the frame caught**: a run's origin
    *outlives* the run (ADR-0023 §4), where the sounding record the mark used to
    read went to `None` when the music stopped — so it had been carrying the
    liveness by accident, and reading the origin alone left the lamp lit on a
    list that finished an hour ago. Liveness is its own argument now, answered
    first; `a_finished_run_leaves_no_lamp_behind` is the test.
- **The owner's `Now playing` batch, 2026-08-10** — six asks on one surface in
  one afternoon, with frames for each at
  `docs/design/impl/now-playing-shows-the-run/`.
  - the **`Run` word and the two densities** removed. The run column is not
    what went — it stands whenever there is a run, and all fifteen of its
    affordances are untouched. `Ctrl+U` folded into `Message::ShowNowPlaying`.
  - the place **shows whatever the bar names**, sounding or not. The record's
    column is drawn even when there is no record, so a loaded run becoming a
    sounding one moves nothing — and the field had believed that all along
    (`Ground::Split`), which is how the disagreement was found.
  - the **`Nothing queued` state** inset like the rows it replaces. The wall's
    and the playlist page's were checked and are correct.
  - **three kinds of list** (`RunSource::Fixed · Playlist · Assembled`), so the
    save word appears only for a run assembled from nothing. *Has a file* was
    never the predicate; *did the listener assemble this* is.
  - the **field runs continuously under the run column**. The clamp that made
    the seam was protecting the rows' contrast, so it is replaced by a
    measurement: binding case `paper_faint` at 4.71 : 1 against a 4.5 floor.
  - the **run column follows the music** — on the engine's confirmation only,
    only when the row is off screen, landing it two rows down.
  - **Deferred out of the batch, deliberately:** the artwork crossfade, now
    item 1 of *Next* with everything it needs written down.
  - **Left as it is, with evidence:** *"that needs a scrollbar as well"* — the
    run column already draws one, at the list's 10 px form, at the column's own
    right edge (frames `30`/`31`). `theme.rs`'s rule is that a list's bar is
    its only readout of how much list there is, which is why the wall's is the
    narrow one and this is not. **Needs the owner's eye on the frame**: if he
    still cannot find it, the change is one line.
- **`A–Z` is a group key again, first in the row** — the owner's *"that feels
  like it should go back and honestly it's the first option, followed by
  artist"*. The strip is `A–Z · ARTIST · YEAR · GENRE · ADDED · PLAYED` and the
  number row is `1`–`6`. ADR-0035's third amendment; frames at
  `docs/design/impl/az-and-artist/`.
  - **The new key does not take `"artist"`'s code back.** It is `"alphabet"`,
    because `"artist"` was already repurposed once without saying so — it named
    the initial grouping before ADR-0035 and the artist grouping after, so a
    `config.toml` written before that day quietly changed meaning. That is now
    a paragraph on `GroupKey::code` itself, where the never-repurpose rule
    lives, rather than folklore.
  - **The budget was re-measured, not reused.** The last sixth word was
    `ARTISTS` at 77.49 px; `A–Z` costs 44.92, so the row is 357.91 and
    `KEYS_W` is **360** rather than the earlier costing's 368. Downstream:
    `LIBRARY_LINE` 552, `TOP_BAR_SPLIT` 824, `SINGLE_LINE_NO_WELL` 600.
    **Nothing forced the window's minimum**, which was the thing to confirm —
    the library line sits 48 px under the 600 floor, and the
    single-line-with-well band survives at 824…904.
  - **Found on the way**: `views::top_bar::group_key`'s doc still carried a
    paragraph about *"none of the five is current while the artists are on the
    wall"*, describing a wall deleted the same day. Corrected.
- **Design doc 15 — the artist's page**, and ADR-0037. The owner's *"maybe
  just the wikipedia for the band or something?"* turned out to be **two asks
  wearing one sentence**, and the study's whole structure is the separation:
  the page can be worth visiting for **nothing**, and the encyclopaedia is
  **baz's first network request**. Eleven local facts sort cleanly on
  `views/home.rs:71-76`'s test — *would this figure be identical if the
  application had never been opened?* — and the three that fail it are the
  ledger's, which ADR-0018 §6 already refused **by name** (*"no
  totals-by-artist"*), so they went to him as three questions rather than
  into the page.
  - **The network half was priced rather than argued**, against `deny.toml`
    and against `BACKLOG.md`'s Opus refusal, by resolving both candidates in
    a scratch crate outside the repo and intersecting against `Cargo.lock`:
    `ureq` **14 net-new crates**, `reqwest` **57** plus `aws-lc-sys` with
    `cmake` and `bindgen`. Three findings fell out that no argument would
    have produced: `ureq`'s TLS is **not pure Rust** (`ring` has a `links`
    key and compiles C and per-arch assembly), a C compiler is **already**
    required so it is a new *C surface* rather than a new *build
    requirement*, and the largest cost is not technical at all — the
    Flatpak manifest has no `--share=network`, so this puts *"Network
    access"* on baz's Flathub page for good.
  - **Found on the way, twice.** (a) MusicBrainz **returned 503 on the first
    request** from a non-descriptive User-Agent, which is the receipt that
    its User-Agent and 1-req/s obligations are enforced rather than
    advisory. (b) `views/artist.rs:19-26` claims the tiles are *"the wall's
    to the pixel"* and the arithmetic says otherwise — 4–11 px wider at
    every size, and six columns against the wall's five at 1920 with the
    lane collapsed.
  - **The zero-cost answer that nobody had costed**: `OpenURI` over the
    D-Bus connection `mpris/server.rs:33` already makes opens the artist on
    Wikipedia in the listener's own browser for **zero new crates and no
    sandbox permission**, because the portal runs on the host. It ships in
    tier 1 as what the page has while the dependency question is open, and
    it may be all he wanted.

- **Search off the Library, decided and built** — ADR-0036, the owner's *"how
  the search works when we're not on the library needs to be decided… maybe a
  little x or esc to clear would make sense too"*. The first half was **already
  half-answered**: every road to the query has gone to the Library first since
  the well moved into the lane (`App::reach_the_well`). What was missing is that
  the field never said so, so the placeholder now names its subject —
  **`Search library`**, in every place, in the field's resting 176 px, which
  costs nothing because a placeholder and the count's slot are never on screen
  together. **Contextual search is refused** on one hard constraint rather than
  on taste: type-anywhere is a promise about the collection, and a scoped well
  would revoke it on exactly the pages a scope applies to. And the **`×`** ships
  in the mark's own box — the magnifier at rest, the cross while a query stands
  — because the field's right edge is full and the swap costs the query none of
  its 104 px. It runs `Esc`'s own function. Frames at
  `docs/design/impl/search-scope/`.
  - **The one thing this declines and does not dismiss**: a filter for a long
    playlist's rows. Costed in `BACKLOG.md` as a *second control on the
    playlist page* — its state beside `renaming`, peeled by
    `peel_place_states`, and needing a key of its own because `/` and `Ctrl`+`F`
    belong to the well. One surface earns it; that is the owner's call to make.

- Doc 14 Tier 2 — **the distinction moves into the type**. A record's page sets
  its title in the serif italic; a playlist's page deliberately keeps the sans,
  and that asymmetry *is* the design. The two identity blocks did not move a
  pixel: three ink bands, 71 px of ink, a 35 px pitch to the byline and 27 px
  to the facts, identical on both pages at 1280 and 1920
  (`docs/design/impl/serif-titles/measure.py`). The byline also gained its
  composition, `Playlist · 12 records`. Frames at
  `docs/design/impl/serif-titles/`.
  - **`now_playing.rs`'s prose argued against the serif, and it was half
    right.** Its concern — *a display face arriving one surface at a time* — is
    kept verbatim and is exactly why the test stays an **enumeration** rather
    than becoming a `contains`. Its **boundary** was wrong: *"there is one
    placard in the product"* is a quantity, and a quantity cannot say whether
    the next string may have the face. The rule that replaced it is *the serif
    sets an album's title, on the surface whose subject that album is* — under
    which Now playing stays sans **more firmly** than before, since its hero is
    a **track's** title and the album under it is a fact about that track.
  - **Found on the way, twice.** (a) Doc 14 costed the byline's count as free
    from the sleeve's quotation list; that list stops at four, so `Road Trip` —
    fourteen tracks, twelve records — would have read `Playlist · 4 records`
    over a page listing twelve. The distinct set is walked to its end now. (b)
    A frame cannot prove the *bundled* serif rendered rather than a host serif
    iced silently fell back to, so two `font.rs` tests do: the family strings
    against what the bytes spell, and every Latin-1 letter an album title can
    arrive with. Writing the first turned up that the family a matcher reads is
    `name` record **16** — record 1 is the legacy family, and Plex Sans
    Medium's reads `IBM Plex Sans Medm`.
  - **Tier 2 #8 was declined from its own frame**, not skipped: the strip reads
    `Run · 2 of 12 · 55:00 left … Save as playlist`, subject first, and a
    variable-length `Save these N as a playlist` in the 440 px strip is the one
    measurement doc 14 §6.3 flagged as wanting a frame before it ships.
- **The artwork stops at the file, and the room takes the record's colour**
  (doc 12 A2 **and A3** — A2 alone did not answer the complaint: at 1920 the
  record is height-bound, so deleting the 720 px clamp bought 53 px and left
  the same empty room. The clamp made the square small; the *absent field*
  made the room empty). A 1024 px hero decode, the sleeve now source-bound
  (1024 at 2560; **300** for a 300 px cover, where it used to be a 2.25×
  upscale of a 320 px thumb), and a three-hue field on the room's own
  lightness ladder.
- **Shuffle is a property of the walk.** The run keeps its order; the engine
  gained one standing `traversal` and nothing else. `shuffle.rs` deleted whole,
  along with two invalidation rules, the restore walk and the snapshot case.
  Gapless survives by handing a `Session` an itinerary plus a slot→position
  plan, so the decode-ahead producer is unchanged to the line — every existing
  gapless test passes untouched. The rule is a **bag**: one shuffled pass, no
  repeat until everything has played, and the next row is *shown* (an open ring
  beside the sounding row's dot) rather than hidden.
- **`All songs` has a tile on Home**, second under `CONTINUE`. The strip's
  `Play all` stays: it plays what the *wall* shows, which is the only way to
  play seven search results; the tile plays the collection whole.
- The wall's scrollbar moved to the **window's** right edge. It was the wall's
  bar, not the lane's — this file's item said "the lane's", and a rendered
  frame said otherwise: the lane draws no bar at all with a short list, and the
  wall's sat at x 1168–1171 in a 1280 px window with the rail's 108 px lane
  outboard of it. Now x 1276–1279, with the rail, its letters and the density
  detents at exactly the x they had. It costs the rail 4 px of the press band
  that ran to the screen edge; taken on purpose, and argued at
  `docs/design/impl/wall-scrollbar/`.
- `ARTIST` groups albums under their artist. It turned out to be an ordinary
  group key rather than a subject beside one — `shelves(Artist)` is `albums()`
  with its breaks named — and that identity retired `A–Z` too, since both are
  `albums()` differing only in where the headers fall. **−700 lines**, no
  migration.
- Doc 14 Tier 1 — a record is a work you found, a playlist is a label you made.
  The line under a name declares its kind first; the playlist page gets back the
  byline the record page always had (52 → 80 px, the record's own block); the
  run strip names its subject; and `Save as playlist` becomes the readout
  `Saved as "…"` while the run *is* that file. Frames at
  `docs/design/impl/records-and-lists/`.
  - **Found on the way**: doc 14 §1.4 costed the save fix at *"no new state"*,
    reading saved-ness off `can_undo`. That is wrong — `App::queue_undo` is
    cleared by leaving the place, by standing the run column down and by the
    run ending, none of which un-edits a run, so an edited run would have
    claimed to be its source file again after one navigation. Divergence is now
    a flag beside the queue record (`PlayerState::queue_edited`, one bool, two
    writers).
  - `views/queue.rs` had `save_control`'s doc comment attached to
    `undo_control` — two blocks run together, so the save word carried none.
    Repaired with the change.
- Settings wears the lane — it had neither lane nor door, so `Esc` was the only
  way out of a place you reach with the pointer.
- The bar's now-playing block leads to `Now playing` rather than to the record.
- The artists wall, and `A–Z` naming what it breaks on.
- The queue merged into `Now playing`; the bar's `Queue` door removed.
- The collection's counts moved from the lane to Home as a `COLLECTION` footer.
- `Pull` removed; shuffle became a player property; `All songs` became an
  implicit list.
- Design doc 14 — records versus lists; found the two complaints were one
  defect, and that a one-to-three-record playlist's sleeve is byte-for-byte the
  widget a record's own row builds.
- The refusals ledger deleted — it had become law over the owner.
