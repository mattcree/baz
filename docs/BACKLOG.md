# baz — Backlog

> Deliberate deferrals, in one place. Everything here was consciously *not* done,
> with the reason. Roadmap-level scope lives in `VISION.md`; this is the list of
> known gaps and promises. Updated 2026-08-13.

## What the owner asked for

> **One line per ask, in his own words where they were short enough to keep,
> with its fate.** This lived in a file of its own until 2026-08-10, when the
> owner's *"requests need to be in backlog, no need for different concepts"*
> folded it here. Nothing was dropped in the move: both tables are as they
> were.
>
> **The rule it carries with it, which is why it exists at all.** On
> 2026-08-09 three asks — remove `Pull`, make shuffle a player mode, make *all
> songs* an implicit playlist — were requested more than once, mapped in this
> backlog, and then reported back to him as *"decisions waiting on the owner"*
> rather than built. They were not decisions. They were instructions, and they
> lived only in conversation, which scrolls away. His verdict: *"again we seem
> to be losing these things. I've mentioned them multiple times."*
>
> So: **an ask is written here the moment it is made, and it leaves only as
> shipped, or as declined with his agreement.** Nothing else removes a line. If
> an ask needs a decision from him before it can be built, that is a *note on
> the line*, never a reason to drop it. An ask logged here with no item in
> [`WORK.md`](WORK.md) is half-tracked — this records the asks, that orders the
> work.
>
> The distinction this section does **not** make is between an ask and a
> deferral. They are the same kind of thing — something not done yet, with a
> reason — which is what the owner meant by *"no need for different concepts"*.

### Still open

| Ask | State | Where |
|---|---|---|
| *"we need a way to prune nonexistent albums when they are removed"* | **recorded for later prioritisation; do not fold into the current vibe work** | Provide an explicit, understandable way to remove library albums whose files genuinely no longer exist. Pruning must be conservative: distinguish confirmed deletion beneath an available, successfully scanned root from a temporarily offline/unmounted root, transient SMB/GVfs failure, permissions failure or incomplete/cancelled scan. Never erase playlist files, listening history or user audio; define separately whether their references remain visible as missing entries. Show what will be removed and require confirmation for any manual bulk action, keep the library/database update atomic and recoverable where practical, and refresh the wall, search, selection, queue provenance and artwork/cache state without starting or disrupting playback. Later implementation must first establish whether ordinary successful rescans already retire some stale rows, then give the remaining cases one source of truth rather than layering a second deletion policy over the scanner. |
| *"delete playlist should be possible from the playlists page with 'are you sure' confirmation"* | **recorded for later prioritisation; do not fold into the current vibe work** | Add deletion to each saved playlist's affordances on the Playlists overview rather than requiring the listener to open its page first. The destructive press must open an explicit confirmation naming the playlist; cancel changes nothing, confirm uses the existing recoverable move-to-trash path rather than unlinking, closes any menu/confirmation state, removes the row and selects a sensible remaining destination without starting playback. Foreign playlists retain the same deletion semantics as on their page. Keep the page-level Delete action and its behavior consistent rather than creating a second deletion implementation. |
| *"ensure that resampling is shown as a warning in our settings, and ensure the event log notices it and makes it a warning indicating how to fix"* | **recorded for later prioritisation; do not fold into the current vibe work** | When the active signal path is resampling, Settings must present that as a warning rather than a neutral technical readout, and the canonical event log must receive the same warning once per continuing condition rather than per audio block or track event. The warning must say why conversion is happening and give an actionable route back to a direct path—choose a source-rate-capable output/device or change the relevant output/boundary setting—using the exact negotiated source and output rates when known. Clearing the condition clears its standing warning; repeated tracks under the same conversion must not flood the bounded event history. Later implementation must reconcile shared-mode conversion, Baz's own boundary resampler and any conversion reported downstream so it never claims Baz can fix an OS mixer it does not control. |
| *"is the picker for audio devices actually taking effect immediately?"* | **unverified question recorded for later; explicitly not investigated now** | Audit the complete Settings device-picker path from selection through persisted configuration, engine command/reopen, signal-path event and visible selected value. Establish whether a change takes effect immediately, only on the next track, or only after restart; then make the behavior explicit and consistent, preserve playback or explain an unavoidable interruption, and surface an actionable failure if the chosen device cannot be opened. This row records the question only—the current vibe implementation must not consume time by probing or changing device behavior. |
| *"cached images seem to be spotty about loading/unloading… ensure what is on screen always has an image loaded if it exists… let's not overcomplicate it"* | **recorded for later prioritisation; queued as item 20** | Treat visible artwork as a hard residency requirement, not as a best-effort cache hint. Audit the resident-handle tier, visibility calculations and request deduplication across every screen that draws a sleeve; an existing decoded image for an on-screen target must not be evicted, and an existing source image missing from that tier must be requested promptly. Keep the policy simple: protect current visible targets, retain the bounded LRU only for off-screen recent work, and avoid another cache hierarchy or speculative prefetch scheme. The owner sees it only mildly at 393 albums on Linux but reports a much worse Windows experience around 800 albums, so reproduce at 500+ and roughly 800 covers on both platforms before treating either cache pressure or platform scheduling as the cause. Acceptance uses scroll/churn reproduction captures and asserts that every visible target with available art keeps a handle through cache pressure, while off-screen memory remains bounded. |
| *"make the left and right gutters smaller for the top and bottom bar… the settings cog [is] farther in… the app icon is also too small and sits too far in… [and] the album art in the bottom left… x and y padding [should be] the same"* | **recorded for later prioritisation; queued as item 19** | Re-measure the app bar and bottom bar together before changing tokens: the current app-bar ink gutter intentionally differs from its box gutter (ADR-0040), while the owner now wants less edge air and a larger/less-inset application mark. Reconcile the two bars’ horizontal gutters with the Library rail and scrollbar, preserve every control’s hit target and borderless-window controls, and make the bottom-left album-art block’s horizontal and vertical padding equal rather than leaving a left-only gap. Acceptance is same-size captures with measured ink and box edges in chrome-owned and native-titlebar states, plus a narrow-width check that no app-bar tenant overlaps. |
| *"can we show the app's log somewhere in the settings? maybe under debug"* | **recorded for later prioritisation; sits beside item 11** | There is no log file today, so "the app's log" needs one decision before it can be built: whether he means the **event history** or the **diagnostic log**. The event history is the bounded 64-event session `health::Log` (level/title/detail/age) that the bottom-right status card opens — item 11's surface, destined for the app-bar bell — so a second copy of it in Settings is the *two doors to one history* the app-bar tenancy rules already refuse. The diagnostic log — the `[scan]` / `[playback]` / `[config]` / `[mpris]` stderr lines — is the real app log and is currently invisible to the Flatpak and, since item 12, to release Windows builds that have no console at all; a Settings view is the natural place those lines become listener-visible, and item 11's skipped-file details must land there too, so the two decisions should not be made apart. His shape is a **Debug** section beside the existing Library / Output / ReplayGain sections: what it renders, whether it persists to disk or stays session-scoped (and its retention if it persists), which levels/events qualify, and whether the dev-only `BAZ_MSG_LOG=1` message meter is exposed there. |
| *"should we add 'enqueue next' and 'enqueue at end' as well"* | **recorded; not part of the current search fix** | Split live-run insertion into two explicit actions wherever a searched track is being queued: **Enqueue next** inserts immediately after the current cursor, while **Enqueue at end** preserves today's append. Neither starts playback. With no run, both collapse honestly to the same one-item stopped queue rather than inventing a cursor; the interface should avoid presenting two controls that do the same thing in that state. This does not alter the saved-playlist context: there the action remains **Add to playlist**, a file edit rather than a live-run insertion. Later design should settle the compact three-action keyboard/pointer presentation (`Play` plus the two insertion positions), behavior while the current item is the last item, repeated Next insert ordering, and parity with menus/picker routes before implementation. |
| *"we should probably allow themes -- if we create some basic ones e.g. light, dark, light dark, dark light (in betweeners with a bias) but build it out in such a way that it is easy to generate your own theme -- that would be nice. like provide some JSON (ask an AI to generate it for you etc.)"* | **extensible theme system recorded for later prioritisation** | Ship four coordinated built-ins spanning **Light**, **light-biased intermediate**, **dark-biased intermediate**, and **Dark**, with final listener-facing names chosen in the visual pass. The existing architecture is the right base: every style already consumes one semantic `Palette`; Closing Time supplies the dark end, Reading Room the light end, and the deferred Stone/Plaster work is the natural place to test the two middle polarities. Custom themes should be portable, versioned JSON data—not code and not arbitrary CSS—with documented semantic color slots for the four surfaces, four ink levels, playback accent states, accent ink, alert, success, shadow and focus treatment. Publish a JSON Schema with field descriptions, the built-ins as examples, an `Export current theme` template and a short prompt/example suitable for asking an external AI to generate another. Import/paste and preview locally without baz itself needing an AI service or network access. A runtime validator must parse bounded data, reject unknown/missing or malformed values clearly, and apply the same contrast, elevation and accent-discipline laws currently asserted for built-ins; invalid or missing custom themes never crash or strand the UI and fall back to the last valid built-in with exact diagnostics. Initially keep the extension surface to visual palette tokens—no executable expressions, URLs, filesystem paths, font downloads, layout, spacing or behavior—so sharing a theme cannot run code or break interaction geometry. Later design must settle live preview versus restart (the current palette and glyph sheet are installed once at startup), theme-directory/import ownership, stable IDs and schema migrations, OS-following behavior, how a deleted selected theme falls back, and whether advanced authors may override derived alpha tokens. Acceptance includes switching/persisting all four built-ins, importing a valid generated JSON theme, actionable rejection of unsafe/unreadable examples, exporting a round-trippable file, and returning to a known-good theme from any failure. |
| *"we would like to be able to update the app when new releases are put out -- this is something we should look into solving"* / *"I am not 100% sure I need to use Flatpak"* / *"if we have a check for updates option, how would one go about letting the thing update? either way that is part of that task to discover"* | **end-to-end release-update capability recorded; Flatpak explicitly optional** | This task is not complete when baz merely discovers a version or links to GitHub: a listener should be able to check, understand what will happen, install the update safely, and restart at a chosen time without replacing files by hand. Flatpak is one possible channel, not the product's assumed installation model: the current release pipeline already emits direct Linux, Windows and macOS archives, and later distribution research should compare a direct installer/updater, AppImage or other Linux packages, and store/package-manager channels on their merits. Separate **checking** from **installing**, detect who owns the actual installation, and never self-overwrite a package-manager-owned build; if the user installed through a manager, `Update` hands off to that manager. For a direct install, research the conventional staged flow: download a signed artifact to temporary storage, verify it, preserve the running session, ask baz to exit, let an external updater/installer atomically replace the installation, relaunch, and retain a rollback path if the new build fails. The exact mechanism is platform-specific—a running Windows `.exe` needs another process to replace it, macOS replaces a signed/notarized app bundle, and Linux depends on the chosen package format—and choosing or building those mechanisms is explicitly part of this task. Today the artifacts are unsigned, baz promises no network access, and the Flatpak has no network permission, so unattended self-update is not yet honest or secure. Research must choose supported formats, release metadata/channel, consent and check cadence, signing and verification, platform installers/updater helper, install privileges, atomicity/restart/rollback, stable versus prerelease policy, and database compatibility. An available release may surface through the notification bell, but failed checks stay quiet and never degrade local playback. Acceptance covers each supported distribution: installed/latest versions are accurate, `Update` either performs a verified direct update or hands off correctly, data survives, restart can be deferred, and the feature is not passed off as finished with only a download-page link. |
| *"on Windows, the user could see a visible command line window opened alongside the app"* | **shipped** 2026-08-13 | A normal Windows launch creates only the baz GUI window. `crates/baz/src/main.rs` links packaged/release Windows builds for the GUI subsystem via the crate-root `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`, while debug/developer builds keep the console for stderr diagnostics. User-facing failures were never console-only: they flow through the canonical health/event surface, so no release file or Windows logging sink was added (decided). The final acceptance launch of the actual packaged `.exe` from Explorer and the Start menu is the owner's. |
| *"when playing a playlist or album the recent shows a pip which reflows text... just show the pip at the right of the little row, and ensure it just makes the text ellipsis when it is eating into the space. we don't want reflowing text"* | **shipped** | Every expanded `RECENT` row reserves the same six-pixel lamp slot at the far trailing edge. Title and metadata keep a fixed shared 146 px one-line boundary and are shortened with a measured end ellipsis in the bundled Medium/Regular face; changing the sounding album or playlist therefore changes ink and the existing row card, never the sleeve, text origin, metadata origin, 64 px row pitch or neighbours. Collapsed rows retain their compact sounding card. |
| *"the Playlists page does not have the rail on the right and seems to be different from the Library? it should not be significantly different"* | **shipped** 2026-08-13 | The saved-playlist root and Library now call one collection scaffold, which owns the right-aligned rail-under-body stack, edge scrollbar relationship and lane geometry. Playlists keeps its legitimate collage projection and ordering, but shares the Library grid/density, virtualization, selection/hover grammar and `Spine` rail anatomy. Its rail says `A–Z` only for alphabetical order; `Date created` and `Played` project the ordered files into the same elapsed buckets the Library uses, including inert gaps. Missing creation timestamps read `Not recorded`; no session play reads `Never played`, so an edit cannot impersonate listening. |
| *"the component for unsaved playlists does not look the same as the saved playlists... we don't want too many similar components as it's just tech debt"* | **shipped** 2026-08-12 | Saved and unsaved lists now enter one parameterized `views::playlist_page` component. It alone owns their collage, sleeve size, breakpoint, fixed-aside/table or stacked document, identity hierarchy, `TRACKS` block, empty state, scroller and fixed-pitch row presentation. The durable state supplies Play/Rename/Delete and file counts; the transient state supplies Save/readout, live cursor/remaining time and provenance through the same capability slots. Unsaved rows now carry the saved page's artwork and Album context rather than private record headings. Same-viewport before/after frames and the drift inventory are in `docs/design/impl/one-playlist-page/`, and source guards reject a second private page/sleeve/breakpoint/scroll composition. |
| *"ideally we could have a back and forward functionality via back and forward nav arrows in the top left similar to Spotify"* | **shipped 2026-08-13** | The app bar now carries always-present Back/Forward chevrons beside the mark. They walk an in-session history of `Place` identities with ordinary browser branching: a new visit after Back clears Forward, revisiting the current place adds no entry, and unavailable historical subjects resolve through the existing safe fallback. Disabled arrows retain their fixed boxes and dim honestly. Alt+Left/Right accelerates the same controls; track Previous/Next remains separate. Search is an overlay on the unchanged place, so opening, clearing or dismissing it creates no history entry. Esc retains its established peel-then-Library behavior; all ordinary place doors, breadcrumbs and resident destinations record a visit. |
| *"when folders are offline we show a message in two places. the little status log is where I'd prefer is the source of truth"* / *"if there is an important status i.e. red status, can we make it pulse a bit and be noticeable?"* / *"for any event we come up with, can we have the concept of 'retry' ... either that or ... retries occasionally ... maybe exponential backoff"* / *"our status indicator should just go up into the top bar as a notification bell"* | **shipped 2026-08-13** | The fixed app-bar bell beside Settings is the one operational-health door in every place; the former bottom-bar dot is gone. Its panel is anchored below the bar and contains the bounded canonical event history. Opening/closing acknowledges transient attention without animating a standing condition. Warning/error summaries expose a safe manual incremental Retry, and the existing five-minute refresh remains bounded automatic retry for unavailable roots. A fresh successful scan replaces the current scan state rather than creating a second status surface. |
| *"can we add a playlists page which has all our playlists with a-z and date created ordering then the playlist viewer can show the playlist page as the root of the breadcrumbs"* | **shipped** | `Place::Playlists`, resident in the lane, draws every saved list as its generated collage on the shared density grid. Its Library-style arrangement strip offers `A–Z` by default, newest-first `Date created`, and most-recent-first `Played`; unknown or never-played lists follow the dated rows. The viewer strip is `Playlists › Name`, and the first segment opens the root |
| *"please remove the 'Play all' button at the top of the library"* | **shipped** | ADR-0040. The control **and the action**: `Message::PlayAll` and `App::play_all` went with the button, because a message no control sends is the visible-control rule failing in the direction nobody checks for. Home's `All songs` tile is untouched — different scope (the collection, not the wall as arranged) |
| *"please put the display options at the top bar"* / *"we should have replaced the top window chrome with an app bar which has this + settings + the window controls, the same on all screens"* / *"I am wanting it to function as the window chrome mixed with controls similar to stuff like spotify"* | **shipped.** ADR-0040. The app bar is resident in all eight places: application mark, search, honest drag region, display options, Settings and conditional minimise/maximise/close. It drags the window, maximises on a double press, and right-presses to the desktop's own window menu. Display marks are present only where works hang but their slot remains stable. **What is not done is `decorations: false` by default**, so the platform still draws its bar above ours unless `BAZ_BORDERLESS=1`; the iced 0.14 item below owns that boundary. |
| *"we need some sort of min height as well"* | **shipped** | there was none — `app.rs` passed `min_size` a height of literally `0.0`, so the window could be dragged shut to fixed furniture with no collection between it. `theme::WINDOW_FLOOR_H` is **derived, not chosen**: the resident app bar, one-line arrangement strip, bottom bar/hairline/needle, plus **one row of the tightest wall**. One row is the whole claim; it is not a claim that one row is comfortable. |
| *"remember also we need mac os and windows compat eventually"* | **standing constraint, recorded** | not a task — a **revisit trigger** on three decisions taken for Linux and priced as Linux-only. **(1) Window buttons always on the right** (his *"I don't mind if we have the controls on the right hand side"*), which is wrong on macOS and right on Windows; the app-bar ADR carries the one-line reversal. **(2) The present-mode default**, chosen as `AutoNoVsync` precisely because wgpu guarantees it degrades to Fifo on any surface — the one form of this fix that cannot fail on a platform nobody has tested it on. **(3) The trash/undo layout test** is already `#![cfg(target_os = "linux")]`. CI builds all three platforms every push, and Windows has taught this project three lessons the hard way — drive-less fixture paths, UTF-16LE stored paths, FILETIME stamps — so the gate is real; what is untested is the *interface* on the other two, because nobody has run it there |
| *"resize is much better now but somehow it just doesn't seem... smooth?"* / *"that is really snappy"* / *"that also feels fast"* | **shipped** | **not baz's work at all** — a resize step costs 0.18 ms at 25 records and 0.44 ms at 400 against 16.7 ms of a frame, 8–9× headroom, no decode on the path (`docs/design/impl/resize-cost/`). The cost was *presentation*, and three launches on his own machine isolated it: `tiny-skia` snappy, wgpu + `mailbox` snappy, wgpu as shipped treacle. The default was **`Fifo`**, which blocks on the vertical blank while a drag outruns the monitor. baz now defaults to **`AutoNoVsync`** — not `Mailbox`, which is what actually fixed it, because iced asks wgpu for a named mode *literally* and it **panics** where the surface lacks it (his machine refused `Immediate` exactly that way mid-diagnosis). `ICED_PRESENT_MODE` set by hand still wins |
| *"the settings cog is padded in quite a bit and does not align with the rail"* | **shipped** | he is right and the number is **25 px**, at 1280 × 860 and again at 1920 × 1080 (`docs/design/impl/app-bar-gutter/`). The index rail's letters and the bottom bar's volume groove both end 41 px from the window's right edge — two surfaces, drawn by different code, already agreeing on law L1's line — while the gear's ink stopped at 66. **16 px was a phantom seam**: the window buttons are absent unless baz owns the chrome, and the row put a zero-width `Space` where they would go, which still collects a `GAP_LG` because a row's spacing falls between *children*. **8 px was the box not being the drawing**: every control is a 16 px sprite centred in a 32 px hit box, so hanging the container from `HANG` puts the box on the line and the ink inside it. The rule is now written over *the trailing control* rather than over the gear — it is the close button when baz owns the chrome — and both states are measured after the fix (42 and 43 against the rail's 41; the residual is each mark's own inner air). **Two lines were candidates and are not the answer**: the rail's *lane centre* is not drawn at all, and the gear's old ink centre sat within 2.5 px of it, so it looked like a rule and was a coincidence; the wall's scrollbar is deliberately outside the gutter and is the one thing L1 exempts. ADR-0040's amendment §1 |
| *"we probably want an icon for our app to show in the bar"* | **shipped, one thing for him to look at** | the mark that was already there — `packaging/icons/`'s hicolor ladder, the same file the desktop entry and the Flatpak install — decoded from the 32 px rung and drawn at 16 logical px, which is the `@2x` contract every sprite on the sheet already keeps. **Not on the glyph sheet**, and the two are different kinds of asset: `icon.rs` holds outlines rasterized to coverage and **inked by the room**, and the application icon is full-colour by construction; `packaging/README.md` already said they were unrelated, and a monochrome copy would be a second master, which `packaging/icons/README.md` forbids. **Instead of the word `baz`, not beside it** — the slot does not move (24 was `19.54 + slack` for the word and is `ICON_PX 16 + GAP_SM 8` for the mark), so this is the option that costs the composition nothing, where icon-and-word would have widened zone 1 to 48 to say the same thing twice. **What wants his eye**: the mark carries the lamp dot, and in the bar that accent is not playback truth. It is admitted as a stated exception — *the application's mark is the application's, not the room's ink* — and the reversal is a monochrome `Glyph::Baz` on the sheet if he would rather not spend it. ADR-0040's amendment §2 |
| *"until we have no window chrome, remove the window controls..."* | **shipped** | with the platform's title bar still above baz's own band, minimise/maximise/close were drawn **twice**, and one pair did nothing the other did not. They are **conditional, not removed**: `app::owns_chrome()` is the single answer that both turns `decorations` off and tells the bar whether to draw them, so the day `BAZ_BORDERLESS` becomes the default they appear with no second edit and there is no build that owns its chrome and cannot be closed. The slot is **not** held open — an empty reservation for a control that cannot exist in this state is the present-and-inert failure the bar's own admission rule refuses, and nothing to its left moves, because they sit at the trailing edge. The band keeps its drag and its double-press either way: those *add* a way to move a window that already had one, where a second close button subtracts clarity |
| *"I think ideally we could ensure our playlist view in the now playing and the playlist view/album view are the same thing. the only thing that changes in now playing is that we don't see file details etc. -- that is more like a album exploration type data"* | **done** 2026-08-10 | the **third** copy of one list, merged. The row was three literal copies (`album`, `playlist`, `queue`), the record head was two, and `views::queue` held four more copies of the reserved icon slot `impl/one-page-two-subjects/` had already shared — all now `views::page::track_row`, `list_head` and `icon_slot`, moving **no pixels**. What stayed different is what he named plus three facts about the subject: `DETAILS`, the next-track ring (a run has a cursor), the trailing slot sets, and the head — a page states a *name*, the run a *position*. The run column is **not** drawn through `page::view` and that is the honest limit: it is a virtualized column inside another surface's two-column layout, not a document in one scroll. `impl/one-list-drawn-once/` |
| *"also please make sure the layout of the now playing makes sense on wider screens"* | **done** 2026-08-10 | doc 12 step A4's run half, **and the second fault his first telling named**. At 2560 the measured gap was **1171 px**, not the ~700 the queue carried — that figure assumed a 1024 px cover, and the field is everything the work cannot use. Both edges were real: `RUN_MEASURE` was flat 440 at every size *and* the record column hung from the left gutter with the run pinned right. A4 alone closes 1171 → 919; the pair centring closes the rest. **36 px** at every size now, the work unchanged, and 1280 × 860 pixel-identical. `impl/one-list-drawn-once/` |
| The ambient Now playing — cover as the background, stylised VU over it, a feed of facts, all toggle-able; *"a spectrum analyzer or graphic thing with the bars going up and down"* | **spectrum and visual controls shipped; feed deferred; VU removed by owner** | The cover-derived field now sits behind plain artwork or the rotating jewel case; a third None foreground removes the album object for a true spectrum-led room. Cover / Jewel Case / None are one persisted mutually exclusive choice; Spectrum is an independent full-body toggle with a gated lock-free pre-volume sample tap, and visible motion survives keyboard-focus loss. The four-way Cover / Case / Spectrum / VU selector was tried in the app and rejected on 2026-08-11: *"remove the VU meter... looks bad"*. Design 12's implementation amendments record the replacement. The local facts feed remains unbuilt |
| *"adding controls that apply to all windows makes sense in the top bar"* | **shipped as law, with one recorded app-wide admission.** ADR-0040 asserts the closed tenancy: search is resident because it has one library-wide meaning in every place and overlays rather than navigates; a place's identity remains the place's, transport remains the bottom bar's, and arrangement keys remain the wall's. |
| *"I don't mind if we have the controls on the right hand side as long as we have a sensible consistent pattern"* | **shipped** | buttons right, always. A `chrome` module that read GNOME's `button-layout` and KDE's `kwinrc` and mirrored the bar was built and then deleted against this sentence. The *pattern* half is ADR-0040 §2's five zones and its one rule — **scope widens rightward** — which answers where a future control goes without an argument. Known cost: macOS puts its buttons left and will look foreign; one-line reversal recorded |
| *"the way they appear for the library is nice"* | **shipped, one thing to look at** | the marks moved into the bar unchanged — same sprites, same boxes, same resting ink with the current step lifted, same tooltips. But `Dense` is a 4 × 4 whose cells minify to 2.25 px at 1× and reads visibly softer than its three neighbours at the bar's real size. `docs/design/impl/app-bar/12-marks-4x-*.png` has all four magnified with a point filter. A larger sprite for that one mark is small work |
| *"I would really like it if we could get rid of the native window chrome"* | **accepted direction; ready for later prioritisation** | Yes: remove the platform title bar and let baz's app bar own the window chrome. The old objection was loss of pointer edge-resizing under iced 0.13, not an objection to the product direction, and the formerly proposed fork is no longer required: **iced 0.14.0 ships `window::drag_resize` upstream**. Implement this through the 0.13 → 0.14 migration, an eight-way resize hit band, `decorations: false` by default, and baz's already-conditional minimise/maximise/close controls. This is real migration work—estimated at ~130–170 edited lines across 12–14 files, all five custom `Widget`s, wgpu/text-stack changes and Flatpak source churn—but it is implementable now and no longer waits on a technical decision. Retain normal edge/corner resizing, window dragging, double-click maximise, system-menu access and platform builds; do not ship the existing `BAZ_BORDERLESS=1` preview as the default while it lacks resize edges. Priced in full in ADR-0040. |
| Kiosk mode — full screen on a second monitor | **designed, unbuilt** | design 12; single window, iced has no monitor enumeration |
| *"lets make sure we tackle the home page after critical usability stuff is done -- lets get the vibe playlists etc. based either on lightweight nlp/machine learning options etc. etc."* / *"it should be genuinely impressive and a unique selling point for a local music player app without internet connectivity"* | **doing — listener workflow designed; random-controlled listening and semantic integration remain** | The metadata grammar was explicitly rejected as the feature. The active direction is a private musical model of the listener's files: controlled moods, concrete energy/timbre steering, positive/negative or sounding-track anchors, free-text music prompts and intentional playlist arcs, all offline. The first real comparator is wired end to end: opt-in cancellable incremental analysis over Baz's hardened decoder, a separate versioned cache, conventional tempo/loudness/timbral/chroma features, five mood controls, sounding-track similarity, continuity sequencing and artist/album diversity. It is split behind the `vibe-analysis` build feature so a light build excludes the analyzer; generated output remains a normal silent editable `.m3u8` with inert provenance. The separate `tools/vibe-eval` path pins and refuses substituted artifacts, measures full and bounded-window policies, exports compatible metadata/Bliss/semantic runs, evaluates negative prompts and arcs, and creates identity-separated listening ballots. Baz now reproducibly exports the official Apache-2.0 LAION checkpoint into a 162.7 MB paired quantized candidate with numerical PyTorch/ONNX checks and correct text attention masking. A consented 72-track private corpus has produced 12 twenty-track rankings for metadata, conventional and LAION systems; the harness fingerprints every corpus field and refuses anonymous-ID reuse, and 36 identity-free M3U8 candidates are ready under ignored local storage. The replacement product workflow is fixed in design 16: one natural-language request on Home, a visible default Rise-and-fall energy curve with editable semantic turns, optional explicit Avoid/scope/reference controls, first-use consent that continues the same request, and a silent editable preview. The playing track is never an implicit baseline; Make a mix from this is a separate named/removable reference action. DCLAP remains a quality comparator only because its source/weight/provenance terms and larger text path are unattractive. **Do not mark complete at this baseline, design or harness.** Add a diversity-matched random control, complete the blind listening gate, and integrate the winner only if it repeatedly wins while lawfully redistributable and acceptably packaged. No cloud, account, remote model, hidden pool, engagement optimisation or automatic regeneration. |
| *"every album has a playlist implicitly... which playlist and which track"* | **building** | everything playing is a list and a cursor. The run knows its origin today (`RunSource`, three kinds); what remains is the ledger remembering it across a quit — ADR-0018 reopened |
| *"I still see albums specifically appearing as if they are playing rather than the playlist... it only affects the little pip"* | **building** | the same fact seen from the lane: recency already credits the list, but `lane.rs`'s lamp dot asks *which record sounds* instead of *which list is playing*. Taken with the ledger work, since one answer must serve both |
| *"the seek bar at the bottom should have a toggle indicating for song or for whole playlist"* | **superseded by owner** 2026-08-10 | Tried and rejected in use. The bottom edge is one current-song seek line with track figures; the selector, cumulative list reading, queue-segment geometry and jump targeting were removed. |
| *"the album count in the bottom bar when in shuffle mode is weird... way too many albums shown"* | **shipped** | not the traversal's bag as first suspected: `continuation` folded only *adjacent* items sharing an album title, so a shuffled walk opened a new entry every time it returned to a record — `then 10 albums` for a run holding three |
| *"the artist page should have its own 'all songs' playlist I think"* | **shipped** 2026-08-10 | one wall-sized `All songs` collage above `RECORDS`, drawn by the exact component Home uses. It is scoped to the artist, orders dated releases by year then title with undated releases last, and preserves each selected edition's disc/track order. Playing it materializes the list as the current unsaved playlist; the bottom bar and Now playing source open that editable queue, where it can be saved |
| *"ideally the by artist page could have more info, maybe just the wikipedia for the band or something?"* | **designed; the local half unbuilt, the network half is his call** | design 15 · ADR-0037. Two asks in one sentence: the page can be worth visiting for **nothing** (hours, years, formats, in-library-since, and the records they guest on) and that is tiers 1–2 in [WORK.md](WORK.md); Wikipedia is **baz's first network request**, priced at 14 net-new crates and a permanent *"Network access"* line on Flathub, and put to him as ADR-0037 §6. Tier 1 ships `Look up` — the encyclopaedia in his own browser through the D-Bus portal, **zero new crates** — while he decides |

### Shipped

Newest first. Each was asked for in conversation and is now in the product.

| Ask | Landed as |
|---|---|
| *"possible issue here"* — the Vibe panel showed a full SMB path and `malformed stream: mpa: invalid main_data_offset` after analysing 4,863 of 4,880 tracks | **The reported MP3 now analyses instead of being blamed.** Independent FFmpeg decoding proved the exact 8:53, 320 kbps file was usable. Baz had treated Symphonia's recoverable packet-level `DecodeError` as a whole-stream failure; it now advances to the next packet, matching the decoder's documented recovery contract, while container, I/O and reset failures remain fatal and hostile-media containment stays in place. The Vibe surface also bounds its warning, names only a shortened track title, says how many tracks were skipped, translates decoder internals into listener language, retries on a later pass and writes one summary warning to the canonical event history rather than exposing a private mount path or flooding it per track. |
| *"on an existing playlist, enqueue does not add it to that playlist"* / *"after typing the search I can't use the arrow keys right to change the play/enqueue"* / *"maybe it needs to indicate somewhere use arrows to select and don't automatically select the top"* | **Search now waits for an explicit choice and names its destination.** Typing marks no result; the Tracks heading teaches `↑↓ select · ←→ action · Enter confirm`, and the open chooser claims those bare arrows before iced's focused field can consume Left/Right as caret movement. Search selection has its own clock, leaving the place underneath unchanged. On a saved-playlist page the second action reads `Add to playlist` and atomically appends the searched track to that file (with ordinary playlist undo); elsewhere it remains `Enqueue` and appends to the live run. Neither starts playback. ADR-0036's interaction correction. |
| *"can we make the album toggle off on now playing so we can enjoy just the spectrum meter. (3 states, 2d art, 3d CD case, and nothing)"* | **Cover / Jewel case / None is the persisted foreground choice, independent of Spectrum.** None removes the object and the square stage rather than drawing a blank placeholder; the placard recentres inside a soft mask and the full-body bars occupy the room. It pays no hero composition, hero prefetch or case rotation until an object is selected again. All six combinations have structural clock tests and the objectless measure is swept through narrow, desktop and 4K widths. |
| *"when the app is not focused, the animations on the Now playing page do not play"* | **Visible Now Playing no longer confuses keyboard focus with visibility.** Focus is absent from both the continuous redraw and spectrum-tap gates; losing it only ends a direct case drag. The actual cost gate remains Now Playing + sounding record + Jewel case or Spectrum, so other places still install no clock or sample tap. The pre-volume tap also means mute no longer erases the visual reading. |
| *"we made this app aggressively optimise cached images and memory usage and tokio etc. etc. and it means the images sometimes kinda 'unload' themselves"* / *"can we have a concept of only showing images that are on screen? virtualising? either that or it honestly just needs to take the memory hit? it looks really poor otherwise"* | **Current artwork is resident; only off-screen recent art competes for the 64-entry LRU.** Reproduction found that a current page could churn the shared cache past 64, evict a still-visible sleeve, then meet an unchanged-viewport guard which suppressed its reload. Wall, page and resident-chrome targets now pin their decoded handles until all current surfaces release them. Unit stress proves both protection and return to eviction; an 80-album release-GUI Artist page retained its top sleeves after every decode and a scroll-away/return cycle, at a measured Balanced-density cost of about 25.3 MiB. |
| *"when hovering the volume can we make scroll control the volume"* | **The live fader owns vertical wheel and trackpad travel.** Line deltas become the same bounded ~1 dB steps as Up/Down; high-resolution pixel deltas accumulate at 32 px per step and one event is capped at the fader's 25-step span. Every event over the fader is captured—even sub-step and horizontal travel—so neither the page nor Ctrl+wheel density moves underneath; elsewhere the wheel is unchanged. Scrolling while muted prepares the independent level without unmuting, endpoints and unity retain the existing state machine, and confirmed changes coalesce behind a 240 ms quiet boundary before one config write. ADR-0011's 2026-08-12 direct-manipulation amendment. |
| *"after clicking Play album it should go to Now playing"* | **A confirmed start-and-show transition.** Explicit `Play album`, search album Play, Enter's album result and item 1's album double-click all use one request path. Accepted queue and Play commands arm the destination; only a matching engine `TrackStarted` opens Now Playing. Empty albums, refused commands, an exhausted wholly unplayable run and engine closure cancel or never arm it, so the interface cannot answer failure with an empty playback claim. ADR-0023's 2026-08-12 start-confirmation amendment. |
| *"when we start to type a search it should show the search component"* / *"arrow keys should allow us to select up and down"* / *"when selecting a track, we should be able to use right and left to go to play \| enqueue"* / *"consider the search more like a dropover so it can appear anywhere we are in the app, but it searches mostly just tracks and albums, and it's all just scrollable"* | **One app-wide keyboard chooser.** Type-anywhere opens ranked Tracks then Albums over the unchanged place in one virtualized scroller. Up/Down clamp and auto-reveal an explicit search-only selection; Left/Right choose a track's action; Enter confirms. The heading teaches the full key grammar and no answer is preselected. The second action is `Add to playlist` over a saved playlist page and `Enqueue` elsewhere. Play and album Open complete the chooser; Esc, the clear mark and click-outside clear it. Albums retain explicit Play/Open plus select-then-activate, and no secondary entity entered the surface. ADR-0036's 2026-08-12 amendment and interaction correction; design 09 §5. |
| *"maybe we could put the search in the top bar?"* / *"I think we should move the search up into the top bar"* | **The sole full `Search library` well is resident in app-bar zone 2.** Its lane and narrow-strip copies are removed at every width; the Library body no longer filters under it. The 232 px field and 16 px seam spend 248 px of the measured 304 px slack, leaving 56 px at the minimum window. Its live result card starts below the bar so the query, count and clear mark remain usable while it stands. Place and width changes preserve the one app-wide query/selection rather than duplicating or hiding it. ADR-0040's 2026-08-12 amendment. |
| *"can we make it a double click operation to start playing anything? in other words, one click selects and highlights, but it requires a double click to start playing"* | **One content-selection state machine across every playable tile and row.** A first click highlights without navigating or sounding; a second matching click within the shared 400 ms interval plays an album/list, needle-drops an album/search/list track, or jumps in Queue. Explicit labelled `Play` and `Open` controls remain direct, and a selected tile keeps them visible for touch/no-hover access. Enter activates the current selection outside search; Space remains play/pause. Selection's paper wash/ring is deliberately distinct from amber playback truth. Select-then-activate tiles and rows keep the default cursor; the pointing hand is reserved for named navigation links such as artist and album labels. iced 0.13 exposes no platform click count or double-click setting, so the interval lives in one module ready for the 0.14 migration rather than being copied into components. ADR-0022's 2026-08-12 amendment; ADR-0023; ADR-0024; design 08. |
| *"one of our final tasks should be to update our README and include screenshots. a real public facing view of the app and its features. with an icon and stuff."* · *"can we get the README sorted"* | **A public-facing README**, deliberately the last item before the tag so that it describes what actually ships. The icon in the header from `packaging/icons/`; **four screenshots, every one re-shot** — and the two that were already committed were **false**, because the app bar added a 41 px band and `capture.sh`'s click coordinates predated it, so the `Play` press missed the tile overlay and the store page's hero was a wall captioned `Nothing playing`. `home.png` and `playlist.png` are new, the playlist **built by hand in the running app** through each tile's own `Add to…` rather than dropped into the folder as a file, and the wall moved `dense` → `compact` because ADR-0028's second amendment had made `dense` narrower than the fixture's longest caption (`Marguerite Vance-Lindqvist ·`, cut mid-line). **The keyboard table is derived from `crates/baz/src/keys.rs`, not edited** — the old one was wrong nine ways, including two chords it never listed. **Known limitations are a section rather than an omission**, all of `BACKLOG.md`'s listener-visible gaps plus ADR-0040's unguarded allocation. `docs/screenshots/capture.sh` |
| *"make sure we say inspired by foobar but not a spiritual successor as they are different"* | **The claim is corrected everywhere it was made**, because it is the first sentence a stranger reads. `README.md` now says what baz took from foobar2000 — the posture that your files are the point — and says plainly that it is not foobar2000 and is not trying to be: different platform, different decade, and its own opinions about the interface. `docs/VISION.md` in three places (*"in the spirit of"*, *"inherits … while fixing its three great weaknesses"*, *"what we're succeeding"*), and **ADR-0002**, where the claim originated — amended with the correction rather than rewritten, since an ADR is a record of what was decided. The Flathub metainfo and the desktop entry never made the claim and still do not |
| *"it shows me 'where's your music' which has no browse function and it also tells me the schema version is version 8 if I pick any directory"* | **A distinct state, because the setup screen was answering the wrong question** (ADR-0041, `docs/design/impl/blocked-library/`). Both symptoms he names were the stale binary he happened to run — `Browse…` shipped after it and its `SCHEMA_VERSION` predated 8 — **so nothing in the shipped product was broken, and the failure *mode* was the defect.** One line turned every failure to open the library into the first-run screen, so a correctly-refused database from a *newer* baz was reported as *you have no library*, and every door on that screen led back into the same refusal — which is exactly the loop *"if I pick any directory"* describes. It is now a statement: what happened, **that your music and your playlists are untouched**, the two version numbers, where the file is, and that the newer baz opens it unchanged. **The data was verified before the words were chosen** — `Library::open` now reads `user_version` before it sets a pragma, and a test opens a stamped database three times and compares its bytes. One escape hatch, fenced three ways: it **renames** rather than deletes, it is never the default, and the first press only reveals what a new index costs (the ADDED dates — the `first_seen_ns` loss recorded below). **Related, and done in the same change**: the two release binaries (`target/release/` from the host, `target/tb/release/` from the toolbox) are indistinguishable to anyone looking for an executable, and he reached for the obvious one; `docs/DEVELOPMENT.md` now says which is which and why |
| *"why is balanced smaller than compact... I think the dense should be a bit smaller"* | **The ladder is monotonic by construction, and `Dense` is tighter.** Two things in one sentence, kept apart. **The defect**: each step carries its own `hang` and the art *rises* as the hang falls, so wherever two steps tied on column count the tighter one drew the larger work — `Balanced` 3 × 242.7 against `Compact` 3 × 253.3 at the shipped window. **30 of 96 swept widths**, and **11 of them before `Compact` existed** (`Spacious` under `Balanced` at 720 … 780), so the fourth step exposed it rather than caused it. The test asserted *column count*, which was right all along, and the file's own doc comment had written the inversion down as a property. Fixed by making `art_max` **derived** — it is the next-looser step's `art_min`, so the four art ranges abut and cannot overlap and a tighter step can meet a looser one but never cross it; swept clean at quarter-pixel resolution to 4000 px. **It moves the default wall**: `Balanced` caps at 288 rather than 320, so 744 of 2261 widths draw smaller art with wider gutters — the fix seen from the other side, since the default step was drawing Spacious-sized covers. **The preference**: `Dense` 176 … 240 → **160 … 200**, so 1280 hangs 6 × 162.7 where it hung 5 × 200.8; the floor is `THUMB_PX` halved and one hang above the smallest sleeve baz identifies a record by. ADR-0028's second amendment; `docs/design/impl/the-ladder-only-tightens/` |
| *"when changing track there isn't any kind of nice visual transition for album art in now playing. we should have something a bit nicer, like a quick fade"* | **a 200 ms dissolve of the Now playing hero, and the room crossing with it.** `motion::DISSOLVE` *is* `motion::LAMP`: the lamp warms and the picture crosses on the same event, so no number was invented and the two land together. It is ADR-0020 §3's *album-art crossfade* refusal **reversed** — narrowly, on one surface, with the argument written down, because that refusal was written when the artwork was a tile on a wall and this surface's subject is the work itself. Three rules make it the thing he asked for rather than a flicker: it compares **the picture, not the track** (a twelve-track record is twelve changes and no motion), it **waits for the decode** rather than fading to nothing and popping, and the **field travels on the same number** so the wash he had de-seamed in space cannot re-seam in time. It also removes a wart he had not named: a record change used to cut to the 320 px thumbnail on a fieldless room and then pop to full size. Filmed and measured frame by frame: `docs/design/impl/art-crossfade/`; ADR-0020's third amendment, ADR-0029's |
| *"we should ensure the density options are available on all pages..."* / *"4 levels makes sense to me"* | **`Compact`, and the marks wherever the works are.** The fourth step goes *inside* the ladder because that is where the measurement put the gap — `Balanced` → `Dense` jumps two to four columns at every window from 1280 up, and both ends are closed (looser than `Spacious` cannot draw a larger work; its cap is already the thumbnail's). Its numbers are that rung halved. **Density does not apply to a page of rows and the marks are absent there**: a track row's height is the pointer-target floor, so a tighter step would break the rule ADR-0028 exists to serve — which also rules out the returns lane, where the marks would be present and inert on four places. They stand instead at the trailing edge of the block they hang: the rail's lane on the Library, the block's own section rule on Home and an artist's page. **One correction to the reading below**: Home did *not* lay its tiles at the density — it named `Balanced` in its own source, as the artist page did, so `Ctrl`+`=` moved nothing off the wall either. ADR-0028's amendment; `docs/design/impl/density-on-every-page/` |
| *"I guess we need to add playlists into their own section under library"* | the lane's list body is **two sections** — `PLAYLISTS`, every list, then `RECENT`, the records — under the head rather than inside it, since the three destinations are a closed triple. A **split, not an addition**: `RECENT` already held every playlist mixed with the last 24 records, and it now holds no list at all, because a list in both sections is one door drawn twice. It **reverses his own** *"the side bar will have recent albums and playlists mixed based on some order"*, so ADR-0030 is amended (sixth) rather than rewritten and both sentences stay on the record. The **order is untouched** — last touched first in each section, so a list played this morning moved section, not rank; alphabetical was considered and declined, and named as the reopen. One scroller over both sections, which is what keeps `RECENT` reachable at thirty lists (`docs/design/impl/playlists-section/`) |
| *"it would be good if multi CD albums were a single item"* | it already was, for three of the four shapes a two-disc rip arrives in — the shatter was **shape 3**, where the ripper put the disc in the `ALBUM` tag (`… (Disc 2)`, `… CD2`, `… [Disc 2]`). A closed-list marker rule takes it off, but only when a **sibling** exists to merge with, so a lone `Bitches Brew CD1` is never renamed; the marker also supplies the disc number a `CD1`/`CD2` rip never wrote, so the merged record plays and breaks in disc order (ADR-0038, `docs/design/impl/multi-disc/`) |
| *"how the search works when we're not on the library needs to be decided. should it just pop to the library view when you start typing? or should it search whatever page you are on?"* | **Decided, then amended: it searches the whole Library over the current place.** `Search library` keeps one global meaning, so contextual page filtering remains refused; what changed on 2026-08-12 is presentation. Type-anywhere no longer navigates to Library or filters the page underneath—it opens the app-bar Tracks/Albums dropover and returns to that unchanged place when dismissed (ADR-0036 amendment). |
| *"maybe a little x or esc to clear would make sense too"* | the `×` in the well's mark box — the magnifier at rest, the cross while a query stands, so it costs the query none of its 104 px. Pressing it is the identical function `Esc` runs, and it is drawn exactly when `Esc` has that layer to peel (ADR-0036 §4) |
| *"also, we have removed the a-z option from grouping? that feels like it should go back and honestly it's the first option, followed by artist"* | `A–Z` is a group key again and first in the row — `A–Z · ARTIST · YEAR · GENRE · ADDED · PLAYED`, <kbd>1</kbd>…<kbd>6</kbd>. It gets a **new** code, `"alphabet"`, because `"artist"` has been repurposed once already (ADR-0035's third amendment). `KEYS_W` 314 → 360; the window's minimum did not move |
| *"artists should be grouping stuff by artist not just alphabetically"* | `ARTIST` shelves one artist per shelf, the header a door to their place; the key groups by artist rather than by initial (ADR-0035). It cost `A–Z`, `WallSubject` and some 700 lines at the time — and `A–Z` came back the next day by the row above, which is why the two rows here read as one story rather than two |
| *"the background fade behind the album art seems to abruptly end beside the track list which looks bad -- the fade should continue under the playlist area too"* | one wash over the whole body; `field::Reach` and `now_playing::Ground` deleted. The clamp existed to protect the rows' contrast, so the clamp is replaced by a **measurement** — every room × hue × ink against the field's brightest stop, binding case `paper_faint` at 4.71 : 1 against a 4.5 floor (ADR-0029 §8.7) |
| *"ideally the currently playing item in the playlist is where our scroll goes to i.e. it should be visible when we change track"* | the run column follows the music on `TrackStarted` only, only when the row is not already on screen, landing it two rows down; arriving at the place does the same. The playlist and record pages deliberately do not — they are documents you read, not the run you are hearing |
| *"that needs a scrollbar as well since playlists can be long"* | **already there and kept at the list's 10 px form** — `theme.rs`'s own rule is that a list's bar is its only readout of how much list there is, and the wall's narrower 4 px is narrower *because* the index rail is a second readout. Frames `30`/`31` show the thumb at the run column's right edge. If he still cannot find it, the change is one line — needs his eye on the frame |
| *"remove the run button from the now playing"* (and *"run button is what I'm referring to; just to be clear"*) | the `Run` word deleted, and the two densities with it — `ToggleRun`, `App::run_column`, `set_run`, the `run_column` config key, the place's `run: bool`, `theme::now_playing` and the column's 48 px clearance strip. **The run column stands whenever there is a run**; nothing about the list changed (ADR-0029 §8.5) |
| *"it should probably just show whatever the now playing is indicating, just not playing"* | the place's two halves read the bar's own two questions — the record when `now_playing` answers, the run when `queue_list` does — so a paused run and a run restored at launch both draw. The record's column is drawn even when empty, so a loaded run becoming a sounding one moves nothing |
| *"the nothing queued thing is hugging the left with no padding"* | drawn in the run column's own frame — the place's gutter and the rows' own measure — instead of `width(Fill)` inside a centring container. The wall's and the playlist page's empty states were checked and are correct |
| *"I still see save as playlist on the queue when playing a CD"* / *"nah I think adding more stuff to an existing playlist is fine, that does not need a save"* | three kinds of list on the record itself (`RunSource::Fixed` · `Playlist(name)` · `Assembled`): the save word appears **only** for a run assembled from nothing, a named run reads `From "Road Trip"` once edited, and a fixed one says nothing at all. **Run edits** never write back (ADR-0029 §8.6); the later explicit `Add to playlist` search action on a saved playlist page is a separate file edit and says so before it acts. |
| The `ARTIST` group key and the `Artist` place are both called artist | the key groups by artist now, so the word is true and the two are one thing (ADR-0035) |
| *"the information heirarchy isn't great to be able to tell the difference between an album and a playlist"* | the line under a name declares its kind first — `Playlist · 14 · 42:10` in the lane and the panel — and the playlist page gets back the byline line the record page always had, so the two identity blocks are one 80 px shape that differ in what the middle line *says* (ADR-0024 §A3, §A4.3). Then the type itself: a record's page sets its title in serif italic, a playlist's keeps the sans, because a work's title and a label somebody typed are different sorts of string (§A4.4, `docs/design/impl/serif-titles/`). Three of design 14's questions are still his: [WORK.md](WORK.md) waiting |
| *"can we reuse the basic layout and view of the playlist for the album view and the playlist view accessed via clicking into info — right now they are different but for no good reason. it would be good if it was clear via some sort of title/subtitle telling us if it's an Album or a Playlist"* | **The second half was already shipped and nothing was added for it**: a serif italic title over a person's name against a sans name over `Playlist · 12 records` is unambiguous in a frame, so no eyebrow is drawn (ADR-0024 §A4.5). The first half was real — ADR-0024 §A2 gave the two pages one arrangement and what shipped was a second *copy* of it. `views/page.rs` is that arrangement written once; the pages hand it the breadcrumb against the name, the cover against the collage, `Play album` against `Play`, the acts, the two heroes' faces, the byline, `Undo`, the edit slots. The drift went: one quiet act was a centred full-width box and the other natural-width words (x = 115 against x = 12), a record's page had no empty state, and **a playlist's whole page rode 12 px higher** because its strip's lead was a word rather than a control. Frames from both builds at `docs/design/impl/one-page-two-subjects/` |
| *"'save as playlist' really makes no sense on the playlist page for a CD"* | the run strip names its subject (`Run · 1 of 24 · 1:56:19 left`), and the word takes the shape the run permits (ADR-0024 §A5) — **narrowed the next day** to the row above: it appears only for a run assembled from nothing |
| *"integrate the queue with now playing so we can remove the queue option from the bottom bar"* | `Place::Queue` deleted, its whole body the merged surface's run column; the bar's door off, its 152 px to the title |
| *"remove pull since it doesn't make sense here"* | gone, with `History::pull_weight` — its only consumer |
| *"also fullscreen the now playing looks weird"* | two faults, not one — the 720 px clamp made the sleeve small, and the absent field left the room empty. `NOW_PLAYING_MAX` deleted and the art scaled by the room (design 12 steps A2 + A3) |
| *"the album and track count below the search bar doesn't look good... maybe this should go into the home as some basic stats?"* | the resting counts moved to Home as `ALBUMS` / `TRACKS` stats; the strip under the well now speaks only when a query does, and says how many it matched |
| *"shuffle as a concept is more about going to an unknown next track rather than actually mutating the track list"* | a traversal in the engine, not a permutation: the run keeps its own order and the walk is a bag. `crates/baz/src/shuffle.rs` deleted with it |
| *"again I wanted the Play all, to be more like a tile on the home screen, a special 'playlist'"* | an `All songs` tile on Home, second on the page, in the wall's tile anatomy with a list's collage sleeve |
| *"make shuffle a property of the player i.e. toggle on/off"* | player state in the bar, persisted; a mode rather than an act |
| *"the 'all songs' should be an implicit playlist"* | `implicit::ImplicitList` with an `Origin` kind; `Play all` is its `Play` |
| A breadcrumb instead of Prev/Next, and Artists alongside the group keys | `Place::Artist`, `Artist › Album`; the stepper withdrawn — it walked an order you cannot see. *Alongside* the keys became *one of* them (ADR-0035) |
| *"the recent bit shows albums popping up even though it was the playlist which was played"* | a run reified from a list credits the **list**, and now across a quit too |
| *"when I play a song from a playlist it should only bump the recency of that playlist, not the underlying albums please"* | the ledger is a list of runs: `SetQueue` carries the run's origin, a `# baz run` comment opens each run in the file, and the lane's launch fold reads them. The five-field line format did not move — the sixth field the backlog called for turned out to be the wrong answer, and the marker costs no migration and no downgrade (ADR-0034, ADR-0018 amended). An album's run still credits the album: a fixed list is not a playlist. `docs/design/impl/ledger-remembers-the-list/` |
| *"I still see albums specifically appearing as if they are playing rather than the playlist. in a sense we need to track which playlist + track is playing to actually understand what is happening… it is showing next to the album rather than the playlist"* | the lamp dot follows the run's **origin**, not the sounding file's record — through `lane::sounding_subject`, which is the same call the recency ordering makes, so the dot and the order cannot disagree about one run. At most one row is ever marked. `views/lane.rs`'s argument against a list lighting *incidentally* is kept: it is still true, and it never reached the case where the list is what you put on |
| *"search belongs at the top"* | the well is resident in app-bar zone 2 |
| *"the search should really be in the sidebar"* | shipped in the lane at the time; superseded by the owner's later app-bar decision, preserved in ADR-0030's history |
| Remove the nav controls from the playlist and album views | place headers lost `‹ Library` and the Esc hint |
| The lane's scrollbar at its edge; no rectangle round the collapsed `Now playing` | the gutter moved onto the lane's contents; the mark is the glyph's own box |
| *"now playing does not need the play pause controls"* | it was drawn twice; the duplicate and its wrapper are gone |
| `CONTINUE` disappears on resume, takes you to Now playing, returns when you stop | one predicate: the band stands when there is a run and nothing sounds |
| A home page and a left sidebar with recents, collapsible to icons | `Place::Home`, the returns lane, ADR-0030 |
| A Now playing page beside Home and Library | `Place::NowPlaying` |
| Clearer click affordance on playlist rows; `New playlist` as a ghost row with `Save` | the row-hover family, fixed as a system |
| One press to play from the wall, via options over the cover | the hover options; retired the two-press cost |
| *"a very minimal scroll bar because otherwise it's hard to just jump to the end"* | the wall's 4 px bar in its own lane |
| The album art beside the bar's now-playing block | 52 px cover, one control with the text |
| Dock-style magnification on the index rail | the fisheye, 2.5× with displacement |
| A directory picker, and a NAS as a library folder | ADR-0025 |
| Playlists, modelled honestly | ADR-0024 — `.m3u8` files you own |
| *"how users interact to play, create playlists, edit playlists"* | design 08, 09; the whole implicit-playlist epic |
| Rethink control layout and iconography | ADR-0026, design 10 |
| A Jobs-era adversarial critique | design 11 — P1–P6, P8 shipped |

## Product decisions to honour later

- **A long playlist has no filter of its own.**
  [ADR-0036](adr/0036-the-wells-one-meaning.md) decided that the search well
  keeps one meaning — it searches the collection — and refused a well scoped to
  the page you are standing in, because type-anywhere is a promise *about the
  collection* and a scoped well would revoke it on exactly the pages a scope
  applies to. The owner's underlying observation is still right: *"both makes
  sense to me"*, and filtering 200 rows of `Road Trip` is a real want.
  **The honest shape is a second control, not a second meaning for the well**:
  a filter field on the playlist page's own header, with its own state on
  `Playlists::open` (beside `renaming`), peeled by `App::peel_place_states` —
  which already exists for exactly this class of transient — before the place
  itself leaves. It costs a key: `/` and `Ctrl`+`F` reach the well from
  everywhere and must keep doing so, so the page filter needs its own binding
  or it becomes the first pointer-only control in the product.
  **It is one surface.** §3 of the ADR walks the others: a record's tracks are
  1–20 and an artist's records 1–30, where a filter is noise; the run column is
  long but is the one list you reorder by dragging, and a drop index into a
  filtered list is not honest; Home is fixed; the wall has the well. One
  surface did not buy a control class — but it is the owner's call, and this is
  what it would take. **Owner decision.**

- ~~**A list played in a *previous* session still shows as its records in the
  lane.**~~ **Closed 2026-08-10** — ADR-0034 §2–§5 shipped. `SetQueue` carries
  the run's origin, the ledger opens each run with a `# baz run` marker, and
  the lane's launch fold reads the markers, so a list played last week comes
  back as the list. Two frames of the owner's own check —
  play, quit, relaunch — are in
  [`docs/design/impl/ledger-remembers-the-list/`](design/impl/ledger-remembers-the-list/README.md).
  The **sixth ledger field this entry called for was the wrong answer**, and
  specifying it is what found that; the marker costs no pinned wire byte, no
  migration and no downgrade hazard, and every byte-exact and four-tab pin in
  the repository passes unmodified. The entry as it stood is kept below,
  because the reasoning it records is what the fix was measured against.

  The owner: *"the recent bit shows albums popping up even though it
  was the playlist which was played"*. The live half is fixed — a run reified
  from a list touches the **list** and not the records it quotes
  (`lane::played_list`) — but the fix cannot reach across a quit, and the
  reason is structural rather than lazy. The lane's records half is folded at
  launch out of `baz-core`'s **play ledger**, which is per *path*; the engine
  appends it, and the engine is never told a run's provenance — it receives
  `SetQueue { paths }` and nothing else. So a relaunch re-derives exactly the
  attribution the fix removes.
  Closing it properly means **a provenance field on the queue command and a
  sixth field in the ledger line** (format v1 → v2, and the format is
  documented inside every ledger file). That reopens **ADR-0018**, which is the
  owner's decision and not a bug-fix's. The cheap alternative — the front end
  writing its own small "lists played" file beside `session.toml` — is a second
  store of a fact the ledger should hold, and is recorded here as *considered
  and not taken* rather than done quietly. **Owner decision.**
  > **Decided, 2026-08-10** — the owner: *"when we track the state of what is
  > playing now or what our recent plays were… it should be basically which
  > playlist and which track"*. The design is
  > [ADR-0034](adr/0034-the-run-and-its-list.md), and specifying it changed one
  > half of the sentence above: **the sixth ledger field is the wrong answer.**
  > `history::format::decode` rejects a six-column line outright
  > (`format.rs:128–133`), and the file is never rewritten (ADR-0018 §3), so a
  > v2 writer would leave a permanently mixed file that every older baz reads as
  > partly corrupt. `#` lines are already skipped and **not** counted as damage
  > (`read.rs:266–269`), so the ledger gains a **`# baz run` marker** instead:
  > the grain of the file changes, the grammar of a line does not, and there is
  > no downgrade hazard in either direction. The command field stands as
  > written, and costs **no pinned wire byte** — `skip_serializing_if` keeps
  > `command_wire_format_is_stable`'s bytes exactly. Closed when M4 of doc 12
  > §12.0 ships.
- **A run still carries a playlist's *name*, not its `Origin`** — ADR-0034 §1's
  half, deliberately not built with §2–§5. `QueueVm::provenance` is still
  `Option<String>`, so the product constructs `Origin::Playlist` and nothing
  else: an album's run, `Play all`, a draw and a run made by hand all reach the
  ledger as *we do not know*, which folds them onto their records exactly as an
  unmarked ledger always did. **That is correct for the owner's ask** — a fixed
  list is not a playlist, and an album's run should credit the album — so what
  is deferred is not a defect but a *subject*: `queue_summary` reading `Ochre ·
  2 of 9 · 31:04 left` instead of `2 of 9 · 31:04 left`, five kinds of run
  naming themselves in the ledger, and `Origin::Draw` crediting **nothing**
  where today it credits every record it quoted. The type is built and its
  decoder already reads all six kinds, so this is a `QueueVm` field and its
  construction sites, not a design.
  **One rule it must satisfy**, discovered while building the fold: marking a
  run *excludes* its plays from the records they quoted, so a kind
  `lane::subject_of` answers `None` for — `Artist`, `AllSongs`, `Draw`, `Hand`
  — **must not be written as a marker until the lane can credit it**, or the
  touch is lost rather than moved. `origin.rs`'s
  `no_kind_is_written_that_the_lane_cannot_credit` fails the moment a second
  constructor appears without that question being answered.
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
- ~~**The two-line strip split could move from 960 to 872.**~~ **Done**
  (ADR-0030's second amendment). Moving the search well into the lane meant
  re-deriving the budget anyway, so the seam was set to its exact sum in the
  same change rather than left rounded. The well's 80 px fluid range went with
  it, as unreachable: the strip only draws the well below `SIDEBAR_FLOOR`, and
  no strip that narrow can climb a ramp that starts at 1200.
- **The lane's head is now three destinations and a field, and a fourth
  destination is still refused.** ADR-0030 §1 refused destination rows; the
  owner's first amendment admitted exactly three, and the second added the well
  — which is a field, not a destination, and holds no place. The shape of both
  concessions is the closed set. **A proposal for a fifth head row needs an
  argument that beats L8.4's, and "there is room" is not one.**

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
  readable and writable through the same taper. The exact fader position is
  remembered in `config.toml` and restored before the next run's first track;
  mute remains independent and session-only. The *device/hardware volume*
  half was investigated and deliberately not built — see below.
  The bit-exactness readout is now the conjunction ADR-0011 defines: the
  bottom bar says `bit-perfect` when the chain is `Direct` **and** the volume
  path is transparent, and says nothing (rather than something apologetic)
  when a volume below unity is scaling the samples — that fact is already on
  screen in the fader beside it.

- ~~**The `ARTIST` group key and the `Artist` place are both called artist**~~ —
  **shipped, 2026-08-10**, as [ADR-0035](adr/0035-the-wall-has-a-subject.md).
  The proposal below is struck rather than deleted, because it was measured and
  the measurements are the interesting part of what happened next.

  **What shipped**, after the owner looked at the first form: `GroupKey::Artist`
  **groups by the artist** — one shelf per person, headed by their name, in
  `ArtistKey`'s own order with each artist's records alphabetical under them —
  and the shelf header is the door to `Place::Artist`. The index rail is still
  the alphabet, a letter landing on the first artist filed under it
  (`rail::genre`'s shape, arrived at from the other direction). The key's code
  is still `"artist"`, so nothing on disk needed migrating.

  **The proposal's central claim was true and about the wrong wall.** It argued
  — correctly — that a wall of *artist tiles* cannot be a `GroupKey`, because
  ADR-0019 §1's sweep asserts every album appears exactly once under every key
  and a wall of tiles showing no albums falsifies it. So it built the subject
  beside the key: `A–Z` as the first word, `ARTISTS` as a sixth, a parallel
  projection, a second search projection, and readouts that follow the subject.
  It shipped and the owner said *"artists should be grouping stuff by artist
  not just alphabetically"* — which is a wall of **records grouped under their
  artist**, and that satisfies §1 exactly. So it is an ordinary key, the sixth
  word is not needed, and `A–Z` is the same traversal under coarser headers.
  All of it came out: `vm::WallSubject`, the parallel projection, the second
  search projection, the `wall_counts` / `wall_noun` split, the artist tile, the
  `6` accelerator and the `wall_subject` config key. Net −700 lines across
  `crates/` against the first form, tests included.

  **The measurements, twice.** `KEYS_W` 314 → **368** for the six-word row,
  which the costing below got exactly right; every figure downstream of it did
  **not** match, because `Pull` was removed and `Shuffle` moved to the
  now-playing bar in between, taking `ACTS_W` from 182 to 88. Then the sixth
  word went and all 54 px came back:

  | | costed below | six words | now |
  |---|---:|---:|---:|
  | `KEYS_W` | 368 | **368** | **314** |
  | `LIBRARY_LINE` | 654 | **560** | **506** |
  | the window's own minimum | 750 | **696** (unmoved) | **696** (unmoved) |
  | `TOP_BAR_SPLIT` | 926 | **832** | **778** |
  | `SINGLE_LINE_NO_WELL` vs `WIDEST_LANE_STRIP` 720 | 702 (18 spare) | **608** (112 spare) | **554** (166 spare) |
  | the single-line-with-well band | *deleted* | **832…904** | **778…904** |

  The costing's most valuable line — *"a consequence nobody would predict"* —
  turned out not to happen, and that is the reason it was worth keeping: the
  band is **asserted** in `theme.rs`, because it was predicted not to exist,
  and the assertion has now survived the row growing *and* shrinking. Frames:
  [`docs/design/impl/artists-grouped/`](design/impl/artists-grouped/), with the
  first form's at [`artists-wall/`](design/impl/artists-wall/).

  <details>
  <summary>The costed proposal as it stood, 2026-08-09</summary>

  **The `ARTIST` group key and the `Artist` place are both called artist**, and
  a measured proposal exists for fixing it. An agent built the whole thing
  before its work was discarded as a duplicate; the numbers below are its
  measurements, kept because re-deriving them is expensive and the decision is
  the owner's.

  **The proposal**: rename the key's *label* to `A–Z` (it breaks records on the
  album artist's initial — its shelves read `Unknown`, `#`, `A`, `C`, `Various`
  and its rail is the alphabet, so `A–Z` names what it produces; `NAME` would
  still read as a subject and collide again). `GroupKey::code()` stays
  `"artist"` — it is on-disk config data. Add `ARTISTS` as a sixth word in the
  same row, held as a `WallSubject` beside `group_key` rather than as a sixth
  `GroupKey`: ADR-0019 §1 promises every key is a projection where every album
  appears exactly once, and a key that shelves *artists* falsifies that sweep
  rather than extending it. Not a lens either — the product's standing rules fixes the
  lens switcher at two words and both are spoken for (`WALL` · `MARQUEE`).

  **What it costs, measured in the bundled face at `SIZE_META`**: six words
  put `KEYS_W` 314 → **368**, so `LIBRARY_LINE` 600 → **654**, the window's own
  minimum 696 → **750**, and `TOP_BAR_SPLIT` 872 → **926**. `SINGLE_LINE_NO_WELL`
  648 → 702 against `WIDEST_LANE_STRIP` 720 — it holds, but the headroom falls
  from 72 px to **18**, which is the first thing to check if a seventh word is
  ever proposed. **And a consequence nobody would predict**: the widest strip
  that can still hold the well is 904, which is below 926, so the
  single-line-with-well band 872…904 *ceases to exist* — below `SIDEBAR_FLOOR`
  the strip is always two lines.

  </details>

- **An artist is not admitted to the returns lane, and the rule says why.**
  Still true after ADR-0035 made artists the wall's own shelves and their names
  its doors — opening is a thing you look at, and the lane is a record of things
  you touched. The
  lane holds records you have *played* and lists you have *made or edited* —
  both backed by an external store with a timestamp (the play ledger, `.m3u8`
  mtimes), which is what makes `(last touched, name)` a total order. An artist
  has neither, so admitting one means a third store: *places I visited* — a
  navigation history, which `place.rs` refuses by name. Opening is not touching
  in this surface's sense; if it were, so would be opening a record's page
  without playing it, and the lane becomes a browse history. **What would
  change it**: if an artist ever gains a durable *act* — a follow, a pin, an
  artist-scoped play the ledger records — the lane's existing rule admits it
  with no amendment, because it would then be a thing you touched at a recorded
  moment.

- ~~**An artists wall would cost the rail, density and the sticky headers
  nothing**~~ — **confirmed twice, and the second time it cost even less**
  (ADR-0035). The estimate held in every part: the density steps and the sticky
  headers needed no change, and `rail::entries` needed no branch — though not
  for the reason given. It predicted a wall *headed by `Initial`* reusing
  `rail::artist()` verbatim, which is what the first form built; what shipped is
  headed by the **artists**, so the rail maps each header through `Initial::of`
  and takes the first shelf of each letter's run. Still no branch in `entries`,
  still no new vocabulary, still no state — the pure-function-of-the-headers
  design absorbed a change of headers, which is a better result than the one
  estimated.

  The two costs it named as real were both built and then both deleted: one
  query projected twice (`vm::visible_artists`) and the readouts following the
  subject (`wall_counts` / `wall_noun`). Neither exists, because there is one
  wall. Its last sentence stands unchanged: **artist search is not built**, and
  ADR-0021's ranking is still thrown away at the album fold. What narrows is
  records, and an artist's shelf survives when one of their records does.

  <details>
  <summary>The estimate as it stood, 2026-08-09</summary>

  **An artists wall would cost the rail, density and the sticky headers
  nothing** — `rail::entries` is a pure function of the shelf headers, and an
  artists wall headed by `Initial` reuses `rail::artist()` verbatim with no new
  branch. Two things it *would* need: **one query projected twice, not two
  queries** (compute `matching_album_ids` once and project records → artists,
  or the two walls get two chances to disagree and every keystroke costs
  double), and the readouts following the subject (`views::lane::readout` and
  the well's counts hard-code *albums*; there is a test pinning those strings).
  Artist search is not built and the level makes it obvious: ADR-0021 already
  ranks by *which field the query landed in* and throws that away.

  </details>

## Known gaps in shipped features

- **A skipped file has no name anywhere a listener can see it.** **Closed
  2026-08-13.** Each `ScanEntry::Failed` path and reason now enters the bounded
  health history behind the app-bar bell; the terminal line remains diagnostic
  parity rather than the only listener-facing route. The prior count-only
  status line cost an album: fourteen Frank Zappa MP3s sat outside a 3 735-file
  library with no visible explanation. The log remains bounded and non-modal.
- **A corrupt file can still make Symphonia reserve gigabytes, and that is
  upstream's bound to take.** *(Decided 2026-08-10, ADR-0040 §1. The **panics**
  the same sweep found are fixed — baz stopped registering the raw-ADTS
  demuxer it never needed, §2.5, and contains an unwind out of the parsers it
  does use, §2. This entry is only the allocation.)* Metadata buffers are
  allocated from unchecked 32-bit lengths in at least four places across two
  containers, before a byte is read and without any check against the block
  that contains them:

  | reproducer (base64) | container | site | asks for |
  |---|---|---|---|
  | `ZkxhQwYn///7/1IA/wBJAQAAABMAAAAAAAAA/z0=` | FLAC | `symphonia-metadata/flac.rs:53` picture MIME type | 4,278,208,769 |
  | `ZkxhQwQAACAAAAAAAQAAAAD///8=` | FLAC | `symphonia-metadata/vorbis.rs:175` comment value | 4,294,967,040 |
  | `UklGRiAAAIBXQVZFZm10IBAAAAABAAIARKwAABCxAgAEABAATElTVPz//39JTkZPSU5BTfD//38=` | WAV | `symphonia-format-riff/wave/chunks.rs:538` `LIST`/`INFO` value | 2,147,483,632 |

  Each decodes with `base64 -d` and reproduces through `AudioSource::open` on
  a file with any of `AUDIO_EXTENSIONS`. `flac.rs:66` (the picture
  *description*) is the fourth and the fuzzer found it too; the comment reader
  is shared with Ogg Vorbis, and `symphonia-format-isomp4` reserves sample
  tables from a `u32` entry count in five more places (`stsz`, `stts`, `stsc`,
  `stco`, `co64`).

  **macOS finds the pages, and that is the exposure list being wrong.** The
  paragraph below prices this as a small machine, a container limit, strict
  overcommit or 32-bit, on the strength of a Linux measurement. On
  `macos-latest` the same three reproducers cost **5.02 s**, and that is how it
  surfaced: `no_hostile_input_is_slow` asserted one second over the whole set
  and took `main` red on the merge that added it. Not a regression and not slow
  code — the platform actually finding the pages where Linux hands back a lazy
  mapping. So the list was short by one entry, the entry is **a platform baz
  ships to**, and this moves from theoretical to something a beta tester on a
  Mac can feel with one corrupt file. The test now budgets what baz controls
  and times these three separately, so the number is printed on every run and
  cannot quietly grow.

  **What it costs, measured rather than assumed.** On a 64-bit machine with
  ordinary overcommit — the platform baz ships to — the reservation is a lazy
  zero mapping that is never written: `open` returns `decode error: out of
  bounds`, peak RSS **3.4 MB**, and nothing is wrong except the arithmetic.
  Where the reservation cannot be made (`ulimit -v`, `vm.overcommit_memory=2`,
  a container memory limit, a small machine, a 32-bit build) the same call is
  `memory allocation of 4278208769 bytes failed` followed by `SIGABRT` — no
  unwind, so ADR-0040 §2's containment cannot reach it and nothing can, in
  process.

  **Why baz does not guard it**, in one line each and at length in ADR-0040
  §1: a header walk misses the real case (a large honest file with one corrupt
  inner length); a body walk is symphonia's parser rewritten for four
  containers; the day the two disagree, baz refuses a file that plays; WAV
  files in the wild routinely declare `0xFFFFFFFF` so a size-sanity rule would
  refuse real ones; a pin buys nothing because 0.5.5 and **0.6.0 both have
  it**; and a vendored fork is three thousand lines of MPL-2.0 to re-sync
  forever, covering one of the two crates involved.

  **What would reverse it**: symphonia honouring its own
  `MetadataOptions::limit_metadata_bytes` / `limit_visual_bytes`, which exist
  in `symphonia-core` and which **no reader in the released tree reads** — the
  bound is already designed, just not wired up. *Needs the owner's hand:* an
  upstream issue is a GitHub account. The text is above; the three reproducers,
  the two versions and the unused-knob finding are the whole report.

- **Symphonia still panics on a crafted MP4, and `playback_decode` is
  therefore expected red on scheduled runs.** *(2026-08-10, ADR-0040 §4.)*
  122 bytes whose `ftyp` atom declares a length of `u64::MAX` reach
  `symphonia-format-isomp4-0.5.5/src/atoms/mod.rs:449` and `attempt to add
  with overflow`:

  ```text
  frx/AAAAAAAAAWZ0eXD///////////////////////////////////////////////////////
  //////////////////////////////////////////////////////////7//sAAPIAZnR5cPv7Jfc=
  ```

  (one line, `base64 -d`; it reproduces through `AudioSource::open` under any
  extension).

  **baz survives it**: ADR-0040 §2's containment turns it into
  `PlaybackError::DecoderPanicked` in any build with overflow checks, and in a
  release build the sum wraps and symphonia's own next check refuses the file
  as *"invalid ftyp data length"* — a wrong number rather than a dead thread.
  Pinned both ways by `a_contained_panic_is_named_as_one`.

  **What cannot be fixed here**: libfuzzer-sys installs a panic hook that
  calls `process::abort()`, so the abort happens before `catch_unwind` can
  regain control — no containment in baz can make a panic invisible to the
  fuzzer, and it should not, because finding them is what the fuzzer is for.
  So the target stays red until symphonia fixes the arithmetic. The CI step
  now runs **every** target and fails at the end with the list, so this one
  does not hide the other five.

  *Needs the owner's hand:* the same upstream issue as the entry above, or its
  own — this is `symphonia-format-isomp4`, that one is `symphonia-metadata`
  and `symphonia-format-riff`.

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

- **Opus is not played, and therefore not listed.** **Closed 2026-08-10 on
  evidence rather than deferred**: the owner's library was scanned for
  `.opus`, `.ogg` and `.oga` across `~/Music` and both NAS shares and holds
  **zero** of them. The reasoning below stands unchanged and is kept because it
  is what a future request has to argue against — but the question *"is it
  worth a C dependency"* had an unexamined premise, and the premise was false.
  It reopens if a beta tester asks, and the README's known limitations say so
  out loud rather than leaving it to be discovered.

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

- **A deleted *directory*'s tracks still linger in the index** — **the answer
  must be automatic pruning, not a per-record control.** Removal landed with
  **ADR-0010** (this entry
  said ADR-0011 for two months; that is the volume ADR) and deleting a *file*
  now clears its row on the next scan — but only under positive confirmation,
  and one of the four gates is "the file's parent directory is present". So
  `rm -rf ~/Music/Artist/Album` leaves eight rows behind, deliberately: from the
  filesystem's side a deleted folder and a mount point that is not mounted right
  now are the same `NotFound` for every path below, and wrongly wiping a present
  listener's library is not a bug worth trading a cosmetic stale row for.
  **The unavailable-root guarantee stands; the coarse result does not.** The
  replacement must distinguish a reachable held root with a deleted child from
  a root that is itself unavailable, then let the ordinary case follow the
  filesystem without another removal gesture.

  **What would settle it**, in preference order: ~~(1) a *user-initiated
  prune*~~ — **the mechanism shipped (ADR-0042)**: `Library::forget_paths`
  deletes exactly the rows a listener names, and keeps their first-seen in a
  tombstone so that being wrong — the share was only unmounted — costs a rescan
  and nothing else. That reversibility is the whole reason a listener-initiated
  forget would be mechanically reversible. **The owner explicitly rejected
  that control on 2026-08-10:** if someone wants a record removed, they delete
  or move its files out of the held library; baz should prune the index. Do not
  rebuild `Forget this record`. The remaining problem is therefore the wider
  *"these 412 rows point at files I cannot
  find; remove them?"* surface — grouped by root and counted, so an unmounted
  share is visible as the shape it makes — is still unbuilt and still wants a
  library-maintenance place. (2) remembered mount points, so "this directory is
  gone" can be distinguished from "this directory's filesystem is not attached"
  — **now the preferred route**, because automatic pruning needs that signal.
  ~~(3) a per-row record of which root a track came from~~ —
  **shipped (ADR-0022)**, and it did replace gate 2, but it does not touch this
  case: a deleted album folder and an unmounted one are still the same
  `NotFound` from below whichever root recorded the rows.

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

- ~~**Removing a music folder loses its tracks' `first_seen_ns`**~~ (ADR-0022
  §4) — **closed (ADR-0042).** The tombstone this entry named was designed and
  built: schema v9's `forgotten` table keeps the path and the first-seen of
  every row a listener tells baz to stop holding, the rescan that finds the
  files again spends it, and the memory is consumed by that and swept at open,
  so it cannot accumulate. ADR-0022 §4's accepted price is withdrawn in place
  and the Settings sentence says so. Proved through the product, driven by
  presses, in `docs/design/impl/forget-and-remember/`: the ADDED wall before the
  round trip and after it differ in **zero pixels**, against timestamps planted
  four years in the past.

  **What it does not repair**: a folder removed *before* v9. Nothing recorded
  the fact, so there is nothing to restore — the same wall ADR-0019 §5 hit. The
  fix is prospective and it is the last such loss.
- ~~**Multichannel (>2ch) files are rejected**, not downmixed.~~ — **narrowed
  by ADR-0039.** 3.0, 4.0 (quadraphonic), 5.0 and 5.1 now play, folded with the
  ITU-R BS.775 matrix in `playback::downmix`, in WAV, FLAC, Vorbis and ALAC.
  **What remains** is three separate things, and only the first is baz's to
  fix:
  - **7.1 and 6.1 are still refused**, now by layout rather than by count and
    with the layout named in the error. BS.775's downmix has **one** surround
    pair and does not place a rear centre; folding 7.1's two pairs at −3 dB
    each would put 3 dB too much surround in the mix, and any other number
    would be invented. The ordinary answer is a two-stage 7.1 → 5.1 → 2.0
    fold, and it wants a citation for the first stage that this work did not
    have. Height, wide and top channels, and layouts with half a surround
    pair, are refused on the same terms.
  - **Multichannel AAC does not decode**, and not because of the fold:
    Symphonia 0.5 rejects a 5.1 AAC stream with `aac too complex` before a
    frame exists. Upstream; pinned by a test that fails the day it changes.
  - **A folded 5.1 file plays 7.66 dB below its stereo master** until it is
    analysed, because the headroom the matrix needs is taken as a constant
    attenuation rather than by a limiter (ADR-0039 §4 has the argument, which
    turns on the decode path having to stay a pure function of position). The
    ReplayGain pass recovers it exactly, so the gap is only on an unanalysed
    library. A per-file peak normalisation would close it without a limiter —
    baz already computes true peak — and is the follow-on if it ever bites.
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
  - **This got more visible when the well became resident** (ADR-0030's second
    amendment), and it is worth stating in its own words because it looks like
    a new defect and is not: **<kbd>Esc</kbd> takes two presses to peel a query
    you are still typing** — the first is iced's `text_input` unfocusing and
    *capturing*, the second reaches `crate::keys`. Same for
    <kbd>Ctrl</kbd>+<kbd>B</kbd>, which asks for nothing at all while the caret
    is in the well. Both are captured in
    `docs/design/impl/search-in-lane/05` and `06`. Unchanged behaviour, older
    than this move, and it will not be fixed by anything short of the
    focus-aware shell above.
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

> **Withdrawn on 2026-08-10 by the measurement below.** The burst is real but
> it is *one-shot and already small*: over a whole session of driven dragging
> across 400 records the decode counter reads **40**, every one of them at
> startup or at the single moment the wall first revealed a row it had never
> shown, and **nought** in every subsequent resize second. The range guard
> above is what does it, and it does it at the answer rather than at the
> question. A debounce would remove only the decodes for albums revealed
> *transiently* mid-drag and un-revealed before it ends — and it would still
> decode them the moment the drag passed that width again. The paragraph is
> left standing because its file:line reading of the decode path is correct
> and is what made the guard findable.

### The CPU side, counted and timed — 2026-08-10

The owner, after the day's grid-arithmetic build: *"resize is much better now
but somehow it just doesn't seem… smooth? maybe need some basic debounce on
layout"*. **A debounce is the right fix for one cause and actively wrong for
the others** — deferring layout that was never slow rubber-bands content that
used to track the pointer — so the half of it Xvfb *can* answer was counted
first. Harness, raw logs and the shape of the (temporary, reverted) probe:
`docs/design/impl/resize-cost/`.

Per second of a dragged edge driven at ~30 Hz, release build, private Xvfb:

| library, as shelved | msg/s | resize steps/s | view builds/s | view build p50 / p90 | `Grid::new`/s | `Shelves::new`/s | decodes |
|---|---|---|---|---|---|---|---|
| 25 records, one shelf | 88 | 31 | 59 | **0.08 / 0.11 ms** | 266 | 148 | 0 |
| 400 records, 120 shelves | 89 | 30 | 60 | **0.09 / 0.12 ms** | 269 | 149 | 0 |
| 400 records, one shelf | 86 | 31 | 57 | **0.14 / 0.18 ms** | 258 | 143 | 0 |
| …the same, at 1900 px only | 58 | 24 | 35 | **0.22 / 0.28 ms** | 161 | 91 | 0 |

Read per resize *step*, those are the same four numbers every time: **three
messages, two view builds, nine `Grid::new`, five `Shelves::new`, and three
calls to `request_visible_thumbs` of which the range guard answers all three.**
Idle draws nothing at all — every phase mark that is not a sweep produced no
`[probe]` line, because there was no redraw to time.

So **the whole of baz's own per-step work is 0.18 ms at 25 records and 0.44 ms
at 400** — two view builds — against 16.7 ms of a 60 Hz frame. Sixteen times
the library costs 2.75 times the view build, and only because a wider window
with one big shelf has more tiles on it; the *shelving* is free, 120 shelves
being no dearer than one. `Grid::new` and `Shelves::new` are called nine and
five times a step and are not measurable in the total: they are arithmetic and
one pass over the group counts.

**Nothing on the artwork path is on the resize path.** The thumbnail cache is
keyed on the album id and `THUMB_PX` is a constant, so **no width change can
re-tier or re-decode anything** — that is a property of the code, not a
sample. What a width change *can* do is reveal a record never seen before, and
the counter says how often: 40 decodes in a 400-record run, none of them
during a resize second after the first.

**There is between eight and nine times the headroom needed.** Driven flat out
with no pause between steps, the app took **284 `WindowResized` a second** (447
messages, 163 draws) at 25 records and 261 (410, 153) at 400 — while
rasterising a 1900 × 900 window **in software**, because Xvfb has no GPU. A
dragged edge delivers about 30. And the coalescing a debounce would add is
already there and already engages under load: at 30 Hz the shell sees three
messages and draws twice per step; at 284 Hz it sees 1.6 and draws 0.6. The
toolkit sheds this work by itself, exactly when it is worth shedding.

**So the debounce is refused, on the numbers, and nothing was built.** It would
defer 0.44 ms of a frame's 16.7 and buy its own window of tracking lag — and
it could not defer the part that might actually be expensive, because
`WindowResized` is a report of a reconfiguration that has *already happened*:
iced lays out and draws on the toolkit's own schedule whether or not baz has
updated its grid. All a debounce can hold back is baz's `grid_size`, whose
whole cost is measured above. If the wall is later wanted to stop re-columning
mid-drag, that is an **aesthetic** decision for the owner and should be argued
as one, not sold as a performance fix.

**Which leaves the presentation path, and one honest gap in this record.** The
probe times the construction of the element tree; it does not separate iced's
own layout, text shaping and draw, which run after `view` returns. The
draw-to-draw figure brackets all of it — 6.0 ms a draw at saturation with
software rasterisation included, so under 6 ms of CPU for everything iced does
per frame at these widths, and less than that on a machine with a GPU doing the
drawing. Nothing in the CPU side, ours or the toolkit's, accounts for treacle.

**And the day's improvement is itself evidence.** The owner felt this get
better after the grid-arithmetic build, but the numbers above are the same
three messages and two view builds a step that `BAZ_MSG_LOG` recorded the night
before: the fourth detent (`shelf.rs`'s `Density`) and the wall's scrollbar
move (`WALL_RESERVE` 4 → 112, and the rail stacked under the scrollable rather
than a `row!` sibling) change *what is drawn and where*, not how much baz
computes to draw it. The one thing that did cut per-step work — the
`request_visible_thumbs` range guard — landed the evening *before* the build he
was reacting to. A change that improves the feel while leaving the CPU side
untouched is a change in how many pixels the wall asks the presentation path
for, which is the third suspect below and not one Xvfb can see.

**The single most useful thing to do, still, is one command on his machine:**

```sh
ICED_BACKEND=tiny-skia baz    # smooth => it is wgpu surface reconfiguration
                              # treacle here too => it is iced's own layout,
                              #   which is the only CPU cost not bounded above
```

If that comes back smooth, `ICED_PRESENT_MODE=immediate baz` is the follow-up
and a one-line default if it settles it.

## The strip demolition — four removals the owner asked for

> **Three of the four shipped on 2026-08-10** (`feat/shuffle-and-all-songs`).
> The map below is left as written, because it is what the work was scoped
> from and its file:line references are the record of what the cost estimate
> actually was. What changed against it:
>
> - **1. `Pull`** — done, and it cost what this said it would. It also took
>   `baz-core`'s `History::pull_weight` and its two constants (ADR-0018 §6
>   amended), and closed doc 11 P9 by answering it with removal.
> - **2. `Shuffle` as a toggle** — done, and the three questions "only the
>   owner can answer" were answered by him: it is a property of the *player*,
>   not of a playlist. So question 2 dissolves — the toggle is not keyed to
>   provenance and provenance stays a statement about origin. Question 3's
>   answer is that there is no pool to keep visible — what shuffle can play is
>   the run, which is a list you can open and read.
>
>   **Question 1 — "what does *off* restore?" — was answered twice**, and the
>   second answer deleted the first. It shipped as an inert `Vec<PathBuf>`
>   retained beside the run and invalidated by a new run or a hand reorder;
>   the owner then said *"shuffle as a concept is more about going to an
>   unknown next track rather than actually mutating the track list"*, and the
>   question dissolved with the permutation. **Off restores nothing, because
>   nothing was changed.** Shuffle is a traversal the engine walks
>   (`baz_core::traversal`, ADR-0023's amendment rewritten as one decision) and
>   the retained order, its two invalidation rules, the restore walk and the
>   whole of `crates/baz/src/shuffle.rs` are gone.
> - **3. `Play all` → an implicit playlist** — done as `crate::implicit`.
>   Both traps were live and both are closed: the picker never offers
>   `Add to "All songs"` (asserted over every target, and at its source by the
>   list carrying no provenance), and the order question is answered by
>   *following the wall and saying so* rather than by snapshotting.
>
>   **And it has a face**: the owner's *"again I wanted the Play all, to be
>   more like a tile on the home screen, a special 'playlist'"* shipped the
>   same day as a tile on Home, second on the page, in the wall's tile anatomy
>   with a list's collage sleeve (ADR-0030's fifth amendment).
> - **4. The `Queue` door leaving the bar** — **not done**, and the collisions
>   this section names are why. Still open.
>
> **Cross-cutting, resolved:** `ACTS_W` went 182 → 88 rather than to zero,
> because `Play all` stayed (redefined). It stays at 88: Home's tile is a
> second **scope** of the same list rather than the strip's word relocated —
> `Play all` plays exactly what the wall shows, which is the only way to play a
> handful of search results, and Home shows no wall to filter. So the acts
> lane's budget does not move a third time. The strip's split seam moved 872 →
> 778; `TOP_BAR_FLOOR` did **not** follow, because it is also the window's
> sensible minimum. The `impl/shuffle-and-pull/` harness now documents a
> surface that does not exist; `impl/shuffle-and-all-songs-tile/` replaces it
> and `impl/shuffle-and-all-songs/` in turn.



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
- the product's *no invisible shuffle pools* is satisfiable today only
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
(the product's standing rule): *"its scope is exactly what the wall shows, in the
wall's own order — playing what you cannot see is refused."*

### 4. The `Queue` door leaves the bar — ~~the ratchet's escape hatch does not quite fit~~

> **Shipped, 2026-08-10** (doc 12 §12's M1 and M2), and the analysis below was
> right about the shape of the problem and wrong about the answer to it. The
> owner asked for it directly — *"the queue and the now playing need integrated
> in some way so we can remove the queue option from the bottom bar"* — and the
> ledger's preamble settles the process: it binds contributors and agents, not
> him. Both halves of the ratchet are answered rather than blurred (doc 12
> §6.4.2): the door's **readout** is replaced by the merged surface's head,
> which states the size *with the cursor in it* (`2 of 24`); the door's
> **route** is removed, and reaching an editable run went from one press on a
> bar control to one press on the lane's `Now playing` row — the press count
> unchanged, the muscle memory not.
>
> The three collisions below resolved as follows. **`Ctrl+U` is not
> keyboard-only**: it is the accelerator of *two* visible controls at once, the
> lane's row and the place's `Run` word, which is the precedent ADR-0023's
> amendment already blessed. **The now-playing block was not repointed** — it
> still presses `ShowPlayingAlbum`, still has no lit state, and the argument at
> `bottom_bar.rs` stands untouched; the block simply got 160 px wider. And the
> place did not need a new member: the enum went from eight to **seven**,
> because the merged surface is `Place::NowPlaying` absorbing `Place::Queue`
> rather than a new place beside them.

The ratchet (the product's standing rule) permits exactly one removal: *"Replacing a
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
  the product's standing rule names *both* halves: the `Queue` door as a survivor of
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

**Built, less one field** — ADR-0040. The app bar is resident in every place
and does everything a title bar does: `window::drag` moves the window from any
part of the band no control captured, a double press is `toggle_maximize`,
`window::minimize` and `Message::Quit` are the other two buttons, and a right
press is `window::show_system_menu`. What has **not** happened is
`window::Settings { decorations: false }`, so on the owner's GNOME the platform
still draws its own bar above baz's. `BAZ_BORDERLESS=1` turns it off today.

**Why the flip is still a decision, re-verified against the pinned sources
rather than inherited from this note's earlier draft:**

- iced 0.13 exposes no edge-drag resize. The whole `window::Action` enum is
  `iced_runtime-0.13.2/src/window.rs:24–161` and there is no resize-direction
  variant anywhere in `iced_runtime`, `iced_winit`, `iced_core` or `iced`.
- **And the platform does not cover for it.** winit's `set_decorate(false)`
  calls `frame.set_hidden(true)`
  (`winit-0.30.13/src/platform_impl/linux/wayland/window/state.rs:1000`), and
  sctk-adwaita's hidden frame drops its decoration subsurfaces — after which
  `click_point_moved` returns `None` before it inspects anything
  (`sctk-adwaita-0.10.1/src/lib.rs:400,512`). So under CSD-off there are no
  resize edges at all, on Wayland or X11, and no compositor fallback. What
  survives on GNOME is `Super`+right-drag and the system menu's keyboard
  `Resize`, which this bar's own right press reaches.

**This note used to price the fix as a ~30-line fork of iced. That is now
wrong, in the owner's favour: the change landed upstream.** iced **0.14.0**
ships `window::drag_resize(id, Direction)`
(`iced_runtime-0.14.0/src/window.rs:304`), `window::Direction`'s eight variants
(`iced_core-0.14.0/src/window/direction.rs`) and the winit arm that services it
(`iced_winit-0.14.0/src/lib.rs:1438`). **There is nothing to fork.** The
standing question is now the upgrade, priced in ADR-0040's closing section:
~130–170 edited lines across 12–14 files, all five hand-built `Widget` impls
(`on_event` → `update`, capture moving onto `Shell`, `Overlay::is_over`
deleted), `iced::application`'s boot inversion, wgpu 0.19 → 27 and cosmic-text
0.12 → 0.15 in the graph, ~15–25 new Flatpak pins — and, on the credit side,
**both RUSTSEC ignores in `deny.toml` become deletable** (cosmic-text 0.15
completes the fontations migration they are explicitly waiting on) and the
duplicate `lru` resolves. One recorded rationale also stops being true:
`Cargo.toml` justifies `zbus 4` on `iced_core → dark-light` already linking it,
and 0.14 drops `dark-light`.

**Two things this note predicted that did not happen, recorded because the
predictions were reasonable and wrong:**

- *"the gear moves to the left"*, and with it *"the gear loses half its
  licence"* — doc 10 §3.4's *"universal, **and top-right is its universal
  position**"*. The gear did **not** move: ADR-0040 §2's scope-widens-rightward
  pattern puts it in the top-right corner, so the licence survives intact and
  the amendment this note anticipated is not needed.
- *"the strip's width budget works out… net −78 px"*, computed against an
  `ACTS_W` of 182 and a 960 seam. Both were stale by the time it was built:
  `Play all` was the whole cluster by then and the gear left as well, so the
  seam fell 824 → **680**, which is 144 rather than 78.

**The drag region rule** stated here — *"every control in the strip keeps its
own press, so dragging is what the gaps do, not what the bar does"* — is kept,
and it is worth recording how it is delivered, because the obvious reading
would have been wrong. It is **capture**, not a hole cut in the band: iced
0.13's `mouse_area` runs its content's handler first and returns if the content
took the press (`iced_widget-0.13.4/src/mouse_area.rs:211`), so the whole band
is the handle and every button in it still takes its own press. A bar whose
handle was only the literal gap would have left the window's name and the empty
display-options slot dead, which on four of seven places is most of the band.

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
  launch* line — the shape the product's standing rules tolerates least well.
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
