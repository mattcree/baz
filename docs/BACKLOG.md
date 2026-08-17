# baz — Backlog

> Deliberate deferrals, in one place. Everything here was consciously *not* done,
> with the reason. Roadmap-level scope lives in `VISION.md`; this is the list of
> known gaps and promises. Updated 2026-08-15.

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

### Owner asks and outcomes

| Ask | State | Where |
|---|---|---|
| *"should we just default to showing the last thing that was playing in the bottom bar since we already seem to know? it only makes sense to have the nothing playing state when the user really has never played anything before"* | **shipped 2026-08-17** | baz keeps a play history (`history.tsv`) and restores a run on resume, so the last track is known at launch; `Nothing playing` is currently shown whenever the engine holds nothing, which is most launches rather than only the first. **It interacts with item 7**: if the bar carries the last track, the heart has a subject again and the *hide the heart* half of that item narrows to the genuinely-never-played case. **The decision, made conservatively and stated:** the bar names the track and claims *nothing* about playback. The transport beside it already offers `Play` rather than `Pause` and the timecode is blank, which is how a listener reads *stopped* — so this adds the name of the thing and no false state. Nothing starts, seeks or queues.

The subject is `views::home::standing`'s, unchanged, so `CONTINUE` and the bar cannot name different tracks: `None` while anything sounds, and `None` on a first run with no history — which is exactly the state he says the empty bar is *for*. The heart follows the subject, since a track the bar names is a track you can keep.

**Not photographed**: the proof box has no sound card, so `availability_note` — correctly — outranks the track name in the bar and the standing state cannot be seen there. Pinned in source tests instead. |
| *"can we remove smart playlists from the Home screen"* | **shipped 2026-08-17** | The `SMART PLAYLISTS` band on Home: a heading, a sentence of explanation, and a lone `New smart playlist` control. The door stays where it belongs (Playlists), and Home stops explaining a feature to somebody who came to listen. Gone with it: `word_button_maybe`, whose only caller it was. Home now reads `All songs` → `RECENTLY ADDED` → `COLLECTION`, and a test asserts the strings cannot come back. |
| *"it seems some albums are not grouped properly e.g. look at the home.png, it contains a bunch of different things from the same album — maybe worth investigating that album"* | **shipped 2026-08-17 — `docs/design/impl/untagged-compilation/`** | Visible in the frame he pointed at: **`O Brother, Where Art Thou? (Soundtrack)` stands four times** in Recently added, each as its own record with its own sleeve. A soundtrack is the exact case ADR-0008 exists for — many track artists, one album — so either the files carry no consistent `ALBUMARTIST` and the fallback is shattering them, or the grouping key is picking up something else that differs per file. **Investigate the album before proposing a rule**: the answer is in what those files actually say. **It was the first.** Every one of those files declares the same album in the same folder, names no album artist, sets no compilation flag, and carries a different track-artist string — so step 3 of ADR-0008's chain groups them by track artist, correctly, into fifteen records. Across the library: **3 records of 663 shatter, into 18 tiles.** `SearchIndex::merge_folders` now merges a folder that agrees on an album and disagrees on the artist, keeping the artist a *majority* of its tracks name (so a featured credit never costs a record its name) and refusing outright when two tracks claim the same number (so two `Greatest Hits` loose in one directory stay two records). It merges and never splits, so discs in their own folders are untouched. **One argued decision was reversed with it**, loudly rather than quietly: a tag literally reading `Various Artists` is now the compilation bucket rather than a name — measured at **exactly one record changed** across 4 880 tracks, and that record is this one. Proven against a copy of his own `library.db`: fifteen tiles to one. |
| *"the details area in the album view should be scrollable, but the play all button should not be in that scroll area"* | **shipped 2026-08-17** | The line was in the wrong place rather than missing: the sleeve was already held and everything under it scrolled, so `Play album` went with the facts. It joins the held half; the quieter acts stay with `DETAILS`. **Taken at his word and no further, because the arithmetic agrees**: the sleeve is 320 px and clears a 587 px window, the sleeve and the commitment are 372 and clear 639, and dragging the acts along would be 424 (691) or the playlist page's 476 (743). So the instruction costs a 52 px band of window height and the generous reading would cost three times that, for controls nobody named. The band sits inside the range where the sleeve is already clipped — a concession this form has always made, not a new failure — and the test states the trade in numbers so it cannot drift. |
| *"the alignment and general styling of the song title area of the now playing view is poor"* | **shipped 2026-08-17** | Three lines under a centred artwork, and no two of them agreeing where the middle was. The placard shrank to its content and then *left-aligned* that content inside itself, so the eyebrow sat on the title's left edge; the fact line took the whole measure and set its text from the far left of it; and the title was centred **as a pair with the heart**, which put the title itself half a heart off the composition's axis. Now: one axis for all of it, the fact centred on it, and the heart's slot mirrored by an empty one so the work's title lands on the work's centre line. |
| *"can we make the three album cover views into a toggle cycle similar to the background visualisation button"* | **shipped 2026-08-17** | One control showing the state it is in and naming the state the press leads to, lit on the same rule its neighbour uses. **No new message**: `VisualizationForeground` already means *be this object* and the button computes which — so the shell's arm, the persisted config and every test over them are untouched by the change of grammar. The app bar's scarcest lane gives back two slots. **The cost, stated**: the states are no longer all visible at once, so the tooltip carries the promise (*"Cover art — choose Jewel case"*) exactly as the visualizer's always has — which is why he named that control as the one to match. |
| *"colours of the background and visualisations aren't very striking or dynamic"* | **the visualisations, shipped 2026-08-17; the background field, measured and blocked — needs his word** | **The visualisations were flat and now are not.** Every field drew at one ink (the lamp at `a` 0.18), so only height moved and at that ink the movement was hard to see; the ink is the level's own now, 0.10 at silence to 0.72 at full scale. The reading stays in the *height*, which matters — brightness and hue are exactly what he cannot rely on, so the ink is decoration over a measure length already carries.<br><br>**And they could not have been bolder before, for a reason worth keeping.** `placard_mask` was written for the objectless state and used only there, so in the ordinary state the placard's words sat straight on the bars: swept, `paper_faint` was **already under its 4.5 floor at the old 0.18**. That was a legibility defect as well as an obstacle. The mask is under the type in every state now, so the field is background again and bounded by taste rather than by contrast — with a test that asserts both halves, including that the bare field still *would not* carry type, so the mask cannot be quietly dropped.<br><br>**The background field itself did not move, and should not without a decision.** Its ceiling (L 0.22) is what keeps the artwork the brightest thing on screen, and its chroma (0.024) is already within 11 % of the largest value that clips nowhere in sRGB across every hue, ladder and room — so there is no headroom to spend, only properties to trade. The richer wash design doc 12 §5.3 actually asks for is *"a slow radial-plus-linear"*, and **iced 0.14 still publishes only `gradient::Linear`** (checked, not assumed), so the radial half remains unavailable in the toolkit. Raising the ceiling is the one lever left and it costs the artwork its primacy: **his call, not mine.** |
| *"the now playing bottom bar area still does not seem to have a min size that makes sense which should only grow up to a max based on the content of the artist name and song title etc. — this avoids the heart icons being out in the middle of nowhere. don't show the heart icon if we are not playing anything"* | **logged 2026-08-17 — blitz item 7** | **Shipped 2026-08-17.** The word was *still*, which by the repeated-ask rule means the last fix was the wrong shape — 2026-08-16 gave the placard a `max_width`, and he is describing the other end: it needs a **minimum** too, so a short title does not collapse the box and strand the heart. The second half is a plain defect and visible in his own frame: `home.png` reads `Nothing playing` in the bar with **the heart still drawn beside it**.

**The cause was one line, two levels down.** `views::fitted_line` set every line to exactly its `measure`, so the block was the whole lane whatever it held, and the heart — its sibling — sat at the lane's far end. A measure is now a *ceiling*: a line that fits is as wide as its own words, and only a line that had to be cut spends the lane. The block is `Shrink` between `theme::BAR_TITLE_MIN_W` and the lane (iced 0.14 has no `min_width`; a zero-height spacer in a column is the floor). And the heart is drawn only when `now_playing().is_some()` — the reserved slot stays for the state it is *for*, a held track with no library row.

**Not photographed in the playing state**, and worth saying: Xvfb has no sound card, so `now_playing()` is `None` on the proof box and only the empty state could be shot (the heart is gone there). The geometry is pinned in source instead. |
| *"the ui layout for the vibe playlist isn't great… not well optimised for a wide screen. we have to scroll to see the playlist"* / *"'mood', 'the words', 'the shape' seems not well explained… not really clearly clickable… they will need examples"* / *"for the shape… our default should just be a blend of all of them… then they expand to configure all"* / *"this looks weird and not inviting at all"* / *"create some designs which we can actually approve or not"* | **designed 2026-08-15, awaiting the owner's word — `docs/design/19-vibe-next-phase.md`, item 72** | Eight defects, six of which are one defect: **the page is a form and should be a place.** Photographed at 1600 × 900 and 1000 × 700 in `docs/design/impl/vibe-next-phase/` — the words field is 1 270 px wide for a six-word phrase, the right half of the window is empty, and the list, the length and Compose are all below the fold. Three layouts drawn at baz's own tokens: **A** ask-left/list-right, stacking under ~1180 px, with a mood press composing immediately; **B** a three-step wizard; **C** a list first with a tuning drawer. A is recommended. The shape becomes **one blended line** with *Tune each dimension separately* as a disclosure seeded from the blend's own points, which is his instruction verbatim, and the `−`/`+` stepper that mints a curve goes. **Five decisions need his word before anything is built**, and one of them — whether the wall's heading band replaces `section_rule` on Home as well as here — is where his sentence reads two ways. |
| *"top right there is 'manual and vibe become the same ordinary playlist'. remove that"* | **queued — item 72, with the redesign** | One `place_header_led` tenant on the New playlist place. It explains the product's data model to somebody who wants a playlist, and it is the clearest single line of the *"weird density"* he named: with it and the hero gone, the strip carries `New playlist › Vibe` and nothing else. Removed as part of the layout it belongs to rather than on its own, so the corner is composed once. |
| *"it still feels like it hasn't addressed everything. can you create a quorum of domain experts and UX experts and have them discuss this in a JSONL file as a chatroom"* | **held 2026-08-15 — `docs/design/quorum/2026-08-15-vibe.jsonl`; nine findings folded into doc 19 §6** | Nine hats — desktop players, music-information retrieval, interaction, content, accessibility, product scope, systems, and a listener with 9 000 tracks — over 89 messages, 12 resolutions and 4 questions the room refused to answer because none of them is a designer's to settle. It found what the redesign had missed: the **cold start is the primary state** (a mood press cannot compose immediately on an unlistened library, which is what most first visits are); an **unweighted blend is degenerate**, because a plain mean of rank axes puts loud-and-slow and quiet-and-fast in the same place and reproduces the exact *"dots aren't following my line"* failure; the words need a **vocabulary**, not a rule and two examples; the curve is **pointer-only**; the row-to-dot pairing rests on **hue**; **length went missing**; the **Manual/Vibe fork** should die and take the offending sentence with it; the result is a **receipt rather than a playlist**; and nothing **explains a track**. `docs/design/quorum/README.md` states the format and what a quorum is not — not evidence, not a decision, and not a substitute for a user. |
| *"lets write this up what our current thinking is"* | **written 2026-08-15 — `docs/design/21-vibe-the-design.md`** | The design as one document instead of a sediment of reviews: what the feature is, the one-request model, what each control decides (**which** songs / **where** they go / **how many**), the two questions in the listener's words, the shape control's seven parts, the four readouts that make cause and effect visible, all nine states, both layouts, the language table of what is never said on screen, the measured costs, the three open decisions, what it does not touch, the build order — and a provenance table naming where every decision came from, his words or the quorum's or a measurement. Doc 19 keeps the *why* and now says so at the top. |
| *"lets focus on this feature"* / *"the general idea isn't wrong around some of the controls. I think we could tackle that separately"* | **designed 2026-08-15 — doc 19 §7 (note 21); controls split out as item 77** | The whole feature drawn state by state rather than page by page: **nine states, of which the shipping build designs one**. The five with no design at all are the two first-run states, composing, edited and saved — and the first two are where a listener spends their whole first session. Resolved in the drawing: the ask pane stays live while baz listens, with a commitment that states what it needs and then what it can do; the shape control gets its axis labelled in words, the library's own distribution behind the line, a sentence that updates as it is dragged, presets as chips underneath, and a focused point with keys; the degenerate case is warned twice with *lower the line* offered as a control; saving happens in place with a name proposed from the mood. **Four decisions remain** — one consent (may baz listen unasked), three taste. The control-affordance pass he asked to separate is item 77, on its own track. |
| *"the bell icon is still weirdly skinny… backlog it"* | **reopened 2026-08-15 — item 74** | Item 48 measured the wrong thing and passed. It widened the bell to **0.78 of its box at the widest point**, and the widest point is the **rim** — a single hairline at the bottom of the glyph. What a reader sees as the bell's weight is its **dome**, which item 40 drew at 0.30 and item 48's uniform 1.147 scale about the vertical axis carried to about **0.34** — against the gear's disc at 0.84 in the identical box, so the mark is still less than half its neighbour's mass and still reads skinny, exactly as he says. The test is complicit: `the_bell_is_as_wide_as_the_cluster_it_stands_in` takes the **maximum** run width and allows 0.07 of slack under the narrowest neighbour, so a wide rim under a narrow dome passes it. **The fix is the dome**, with the flare following it, and a test that measures the body's *median* width rather than its maximum — a glyph is read by its mass, not by its widest scanline. |
| *(not an ask — found while re-shooting the store pictures for the release)* the screenshot harness photographed the wrong places | **both halves fixed 2026-08-15 (item 71)** | `docs/screenshots/capture.sh` drives the real binary by coordinates, and its lane rows were `85 · 125 · 165 · 205` — the pitch the returns lane had **before** its rows became their own sleeves. Every one of the four store frames was off by a row: `playlist.png` was a photograph of *Home*. The lane numbers are re-derived (`81 · 133 · 185 · 237`, from `SIDEBAR_DEST_H` 48 on a `SIDEBAR_ROW_GAP` 4 seam under the app bar's 49) and the frames are correct. **The half that is backlogged**: the same drift silently broke the script's *playlist-building* sequence — a dozen presses through the picker panel at coordinates that have also moved — so the playlists frame is honest but empty, showing the ghost tile and `Favourites` and no listener-made list. Re-deriving that sequence needs a frame-by-frame pass, and the store page is not wrong without it. **That pass is done.** Every coordinate is now marked `[arithmetic]` or `[photograph]` in the script, and the sequence itself was rewritten because *the route had changed*: the panel's `New playlist` opens the New playlist place with the record already in the draft, so the name and the Save are there, not in the panel — the old lines typed the name into the app-bar search and pressed a Save that did not exist. The run now **exits 1** if `Sunday Morning.m3u8` is missing or short, because a capture that cannot tell a built list from an empty place is not a verification. Two more pointer artefacts fell out of the re-shoot: the parking spot sat inside Home's recently-added row, and the just-saved list stood selected (a selected playlist tile raises its options), so the run parks on dead ground and moves the selection to a record first. The jewel case turns once every 32 s and was being caught edge-on, so the Now Playing frame now waits for it. All four frames re-shot. |
| *"lets ensure our changelog is up to date and accurate"* / *"ideally can we update our README etc. and remember the audience is just general music listeners not coders… we just need to pick the clear feature set"* | **both done 2026-08-15 (item 61)** | The changelog is current and states what each change *is for a listener*. The README is a bigger job than a trim: it is written for someone reading the repository — build instructions, dependency reasoning, known limitations by codec, the whole engineering argument — and the audience the owner names is *a person who wants to play their music*. **The shape to aim at**: what baz is in two lines, one picture, the eight or so things it does that a listener would choose it *for*, how to install it on each platform, and a link to the engineering documents for anyone who wants them. Everything now in the README that is really about *how it is built* moves to `docs/`, where it already has homes (`DEVELOPMENT.md`, `ENGINEERING.md`, `VISION.md`). **Shipped**: the README is now 190 lines against 341, opens with *what you get* in ten listener-sized bullets, and points at the releases page before it mentions `cargo`. Known limitations **stayed on the front page** — trimmed to the five a listener can actually hit (Opus, AAC gapless, stale rows after a deleted folder, one-at-a-time selection, the hostile-file reservation) with the reasoning one link away; they are what makes the rest of the page worth believing, and hiding them would be the first dishonest thing on it. Everything cut is in the new `docs/FEATURES.md` — the long version, nothing lost — and `release.yml`'s deep link to `#known-limitations` still resolves. |
| *"we should also do a feature parity run to see what other players out there have and what we're missing"* | **analysed 2026-08-15 — `docs/design/18-feature-parity.md`; one shipped, nine queued as items 62–70** | The comparison set is foobar2000, MusicBee, Strawberry, Quod Libet, Rhythmbox and the streaming clients. Two questions per feature — *would a listener notice its absence in the first hour*, and *does it fit baz's promises* — so a gap and a refusal are told apart on the record. **One was shipped immediately because it is a floor rather than a feature**: baz had `repeat_one` and no *repeat the list*, which is the state most people mean by the word (item 2.1 of the doc). Queued, in the order the doc argues for: **multi-select and bulk actions** (62), **sleep timer** (63), **lyrics from tags and `.lrc`** (64), **ratings** (65), **rule-based playlists** (66), **tag editing** (67), **a folder view** (68), **crossfade** (69), **drag-and-drop from the file manager** (70). Refused on the record, with reasons: scrobbling, online metadata/cover fetching, ripping and transcoding, podcasts and subscription streaming, and a plugin API before its time. |
| *(not an ask — found by the parity run)* baz had no **repeat the list** | **shipped 2026-08-15 — item 2.1 of the parity doc** | `repeat_one` was the only repeat state: a track could repeat and a run could not. One control cycles **off → the list → this track** now, the two lit states carrying different marks — the same loop, with and without a `1`, so the state does not rest on the accent alone. In the engine, `Repeat::All` restarts the **traversal's** top rather than queue position zero, so a shuffled run repeats the order it drew rather than jumping to whichever file happens to be first; and repeat-one remains the only state that shortens the plan, so every gapless splice inside a repeated list is unchanged. The config key moved `repeat_one = true` → `repeat = "one"`, and a file written by an older baz is still read. |
| *"figure out why we are using so much memory… I see 1.8GB"*<br>*(recorded with a first diagnosis, not yet fixed)* | **backlogged 2026-08-15 — item 60** | **The arithmetic points almost entirely at the Vibe model sessions, and it lands on his figure.** `baz-vibe`'s `semantic::Model::load` opens **both** ONNX towers — `text_model_quantized.onnx` **126 MB** and `audio_model_quantized.onnx` **34 MB** — and the model is a `thread_local!`, created lazily per analysis worker and **never released**. The default worker count is `DEFAULT_VIBE_WORKERS` **8** (`MAX_VIBE_WORKERS` 16), so a scan materialises up to `8 × 160 MB` = **1.28 GB of weights** before ONNX Runtime's own per-session arenas are counted — and **every worker loads the text tower it never uses**: a worker only ever calls `audio()`, while the prompt is embedded once, on whichever thread runs the request. Measured against that: three idle baz processes on a 206-track fixture library, no analysis run, sat at **~260 MB** RSS each, which is the ordinary baseline (artwork's stated 170 MiB budget, the index, wgpu's driver mappings). 260 MB + 1.28 GB is 1.5 GB before arenas, and 1.8 GB is exactly where that lands. **How to tell it apart in ten seconds**: Settings → Debug reports this process's own RSS (item 39). If it reads ~250–300 MB before a Vibe scan and >1 GB after, this is it; if it is already high before any scan, the suspect is artwork or the renderer instead and the entry is wrong. *The fixes, cheapest first: load each tower only where it is used (8 workers × audio alone is 272 MB, not 1.28 GB); release the sessions when a scan ends rather than holding them for the process's life; cap workers against memory rather than against cores; and price ORT's arena/memory-pattern options with a measurement rather than a guess.* |
| *"lets try something else with the vibe thing… get your experimental/frontier UX/UI hat on and work through a way to create an interesting contoured playlist"* / *"I wanted something more graphical, like tuning it via curves"* | **shipped 2026-08-15 — item 55** | **The old control was not merely ugly: it was not connected to anything.** `Shape the journey`'s four buttons appended their own name to the *text* prompt (`"energy shape: Slow build"`) and the selector that consumed it — `select_semantic` — had **no position term at all**, so no arrangement of them could move a track by one place. The engine that could was in the same crate, unused: position-aware targets interpolated across the list, with no caller but its own tests. So: `baz_vibe::Contour` (points of *position* × *level*) and one `select_contour` that the two older selectors are now written in terms of — **words choose the pool, the shape chooses the walk**, either may be absent, and the weights are stated per case so neither shipped behaviour moved to make room for the third. The control is `crate::contour`: a line over the collection's own axes, dragged by its points, with six **drawn** presets (including `Any`, which is the honest way to say *the words alone*), the analysed library's own distribution behind it, and — after composing — a dot per chosen track with a thread between them, so the shape you asked for and the shape you got are one picture. The retired machinery went with it: `EnergyShape`, the three semantic waypoints, and the `journey` string that smuggled a shape into a text prompt. Evidence: `docs/design/impl/contour/`. |
| *"part of how this will work is in how we prompt the underlying engine"* | **backlogged 2026-08-15 — item 59** | He is right, and it is measurable rather than a matter of taste — `crates/baz-vibe/src/bin/vibe-baseline.rs` already exists to score retrieval against a corpus of requests. Two distinct threads. **The template**: `semantic::embed_text` embeds the listener's words *verbatim*, and CLAP-family text towers are known to answer natural-sentence framing differently from bare noun phrases — which is exactly what baz's own examples are (`Late-night focus`). A stated template (and the evidence for it) is a small change with a measurable answer. **The arc**: the baseline corpus already describes `arc: [{ at, query }]` — a *different text at different positions*, interpolated in embedding space — which is the honest version of the semantic waypoints item 55 deleted. Item 55's contour steers energy by position; this would steer *meaning* by position, and the two compose. *Needs later: a scored run of both against the corpus before either ships, because a prompt change that cannot be measured is a superstition.* |
| *"we probably want a way to allow users to create and save EQ presets"* | **backlogged 2026-08-15 — item 58** | baz has **no equaliser at all** today, so this is two features: the filter, and the presets over it. The filter is `baz-core`'s: a biquad chain between the decoder and the volume stage, which is ordinary DSP but lands in the one place the project has been most careful — the **signal path**. An EQ is a conversion, so Settings' *"direct path"* readout and the resampling warning have to learn a second reason a path is not direct, and **exclusive output's bit-perfect promise and an active EQ cannot both be true**: the honest shape is off-by-default, stated when on, and refused (or stated as refused) on an exclusive device. The presets are then ordinary: named curves in the config directory beside the themes, with the same *"paste some JSON"* route the rooms already have. **The control surface already exists in family**: `crate::contour` is a line over bands with draggable points, which is what a graphic EQ is drawn as — the same widget with frequency across instead of position. *Needs later: how many bands and at what centres; whether a preset is per-output or global; and whether ReplayGain's headroom and the EQ's gain are one budget or two, since together they are the clipping case.* |
| *"we should have like 5-6 standard recipes -- as part of the wizard we should be asking users if they want to make a preset one. as long as the presets are some really common moods and themes"* | **shipped 2026-08-15 — item 56** | Six moods — `Late-night drive`, `Sunday morning`, `Focus`, `Workout`, `Wind down`, `Party` — offered in a `Start from` block **above** the words, which is the *asking* half of the request. A recipe is words + a shape + a length, so pressing one fills the whole form and touches nothing else; the row lights the one the request matches and stops the moment a word, a point or the length changes, because from then on the request is the listener's. They are a `const` table for now; the *"data beside the themes"* end state (a listener writing their own without a build) is a small follow-on. **The original note, kept:** | A **recipe** is more than the contour shapes item 55 ships: it is a named mood carrying *words + a contour + a length* — `Late-night drive`, `Sunday morning`, `Focus`, `Workout`, `Wind down`, `Party`. The wizard asks first (*start from a recipe, or from scratch*) and a chosen recipe fills the whole form, which is then editable — a recipe is a starting point, never a mode. Two things to settle: **where they live** (a `const` table in the source, or data beside the themes, which would let a listener write their own without a build) and **what makes one honest** — a recipe promises a result from *your* library, and `Workout` over a library of chamber music must degrade to something rather than to nothing. The pool note (`N of M tracks analysed`) is the surface that already tells that truth. |
| *"I also thought that shuffle might be one of those things where instead of shuffle, we have some sort of smart shuffle?"* | **backlogged 2026-08-15 — item 57; investigation first** | Shuffle today is a player-level traversal: a seeded draw over the run's own entries (ADR-0034's `Origin`), and its whole virtue is that it is *honest* — a draw is an order, not a place, and it can be reasoned about. A smart shuffle is a different claim: that the order is **better** rather than random, which needs a definition of better and a way for the listener to tell what happened. baz already owns the machinery to do it well — `baz-vibe`'s continuity term is exactly *"do not put these two next to each other"* — so the shape most likely to be right is **shuffle-with-flow**: the same set, ordered so adjacent tracks are sonically close and artists/albums do not clump, rather than a different or generated set. What it must not become: a second recommender that quietly changes *what* you are listening to. *Needs investigation: whether it is a mode beside Shuffle or a replacement for it; whether it applies to any run or only to large ones; what it costs on a 9,000-track All songs run; and what the control says, since `Shuffle` currently means one thing and would then mean two.* |
| *"lets allow setting an image/removing the image for a playlist"* | **shipped 2026-08-15 — item 52** | A playlist's sleeve is a **collage of quotations** from the records it holds (ADR-0024 §A1) — generated, never authored, so it cannot disagree with the tiles of the records it quotes. A chosen image is a second kind of sleeve and needs three decisions the ADR does not make: **where the bytes live** (baz stores playlists as ordinary `.m3u8` files a listener can edit in vim, and the format has no cover field — so the image is either a sibling file the listener would see in their own folder, or a row in baz's own database, which makes it invisible to everything else and lost on a reinstall); **what happens to the collage** (replaced, or shown until the image loads); and **what the picker is** (a file dialog is a platform surface baz has one of already, for music folders). Not hard, but it is a schema decision rather than a view one. **Both answered, and shipped.** The bytes live **beside the list** (`<name>.png` next to `<name>.m3u8`, copied rather than referenced): the database answer would make a listener's own picture invisible to every other program, absent from a copy of the playlists folder and lost on a reinstall, which no product promising *your files are the truth* can choose. It follows a rename and goes to the trash with a delete. Removal **restores the collage** — the collage is what a playlist's sleeve *is* when nobody has said otherwise — and goes to the platform trash, because it is the listener's own picture in their own folder. The picker is `rfd`, on the blocking pool, `pick_folder`'s own rule. One decode at `art::THUMB_PX`, which is exactly `ART_MAX`, serves the tile, the lane and the page; cover-cropped to the square hole every sleeve is drawn in, never enlarged past its own pixels. It also cost `page::view` a change: four acts do not fit an aside that does not grow, so acts now lay **two to a line** (every page with one or two is pixel-identical). `docs/design/impl/playlist-image/` |
| *"can we make sure the playlist row controls are inside the highlighted row as well"* | **shipped 2026-08-15 — item 53** | The row's hover/selection card is `views::page::track_row`'s button; the Favourites heart and the ▲▼✕/`+` slots are **siblings** of it in the enclosing `row!`, so the highlight stops short of the controls it belongs to — visible on any hovered row of a record page or a playlist page. The sibling arrangement is not accidental: iced runs the inner control's press first, so nesting a control inside the row's button is how one press comes to mean two things depending on which pixel it lands on (`views::bottom_bar`'s Favourites slot carries the same note). So the fix is to make the **card** wider rather than to nest the controls in the button — the paint reaches the row's full width while the press targets stay separate. **Shipped exactly that way**: `theme::track_row_card` is `selectable_track_row` in container form (asserted equal to it), `theme::track_row_body` paints nothing, and `page::row_card` wraps the assembled `row![body, slots…]`. No press moved. Measured on the frames in `docs/design/impl/row-card/`: at x = 1200, in the controls' own lane, a hovered row reads `srgb(20,21,23)` against its neighbours' `srgb(12,13,14)`. Two surfaces had no hover answer and gained one (`hovered_favourite_row`, `CreationDraft::hovered_row`), **as guarded enter/left pairs** — the first version used `Hovered(Option<usize>)` and unlit the row the pointer was on, because row 4's enter arrives before row 3's exit; the vibe preview's own hover had the same latent bug and is fixed with it. The trailing slots' hover states needed no change: they light on their own ground, one step above the card. |
| *"lets create more interesting themes for the app too, and ideally can we apply them upon selection"* | **shipped 2026-08-15 — item 54** | Two asks. **The rooms**: four ship today (Closing Time, Stone, Plaster, Reading Room) and they are deliberately quiet; the v1 JSON schema, its validator and its documented prompt already exist (`docs/themes/`), so new rooms cost design rather than machinery — the constraint they must clear is the one the validator enforces: the Oklab elevation law, the dead-zone rule and the WCAG ink/status floors, which is what stops a "more interesting" room from being an unreadable one. **Applying on selection** is the harder half and the reason it is next-launch today: `crate::icon`'s sprite sheets are `LazyLock` statics rasterized **once per process** in the room's glyph ink, and `theme::active()` is read by every view function each frame. Live switching means rebuilding both sheets and invalidating anything that baked a colour — the jewel case's textures and the visualizer among them. **Both halves shipped.** The sheets became **per room, keyed on a generation counter** rather than a swappable single handle: `theme::ACTIVE` is now an `RwLock` behind a relaxed atomic and a thread-local, so `active()` — called by every style closure of every frame — costs no lock, and anything that *bakes* a colour (the sheets; the jewel case's generated textures) keys on `theme::generation()` and so misses rather than needing hand invalidation. An imported room stands immediately too, which is what made the JSON schema hard to work against. *"More" is two*: **Blue Hour** (Closing Time's dark room at hue 264°) and **Sea Glass** (Plaster's light room at hue 175°), each holding its parent's exact oklch L so the elevation law is satisfied by construction and every WCAG ratio lands where its parent's does; the lamp does not move, because the accent is playback truth in every room. The laws earned their keep: Blue Hour's first version failed the veil residual and the option-ink floor, both because `recess` is *the ink the hover veil is made of* and `veil_alpha` averages three channels — so that one plane keeps the wall's hue at a fifth of its chroma. `docs/design/impl/live-rooms/` |
| *"the details on the album view is not scrollable"* | **shipped 2026-08-15 — item 46** | Two-column form only, and that asymmetry was the diagnosis: `views::page::view`'s desktop branch scrolls the track table alone — deliberately, and the reason `TRACKS` is a sticky head — while the column beside it was a plain container in a `Fill`-height row with no scroller and no clip of its own. On a short window the 320 px sleeve, `Play album`, `Add to playlist…` and the whole `DETAILS` block ran past the body, where the body clip cut them and nothing scrolled them; the stacked form was one document, which is why the same record read correctly narrow. The aside is a scroller now, and **the render caught two things the fix got wrong that no passing test could**: iced *clips* a scrollable's content at the viewport edge rather than painting the bar over it (at the aside's own 320 the sleeve lost nine pixels — `theme::ALBUM_ASIDE_LANE` declares the column's lane now, and the measure beside it yields, which costs nothing at any width where the list has reached `LIST_MEASURE`), and a `Length::Fill` child in a `Shrink` column resolves against the *parent*, so `Play album` stretched past the sleeve to the clip and lost its right border — three sides of a rounded rectangle on the page's one commitment. The column states its width. Evidence, including `DETAILS` reached at a 620 px window where no gesture could reach it before: `docs/design/impl/second-review-pass/`. |
| *"some albums do not show the album details in the bottom bar now playing even though the album page shows it"* | **shipped 2026-08-15 — item 49; one reading fixed, the other left with him** | **The bar has never drawn an album title** — `bottom_bar::now_playing_line` is three lanes (title, artist, continuation), so a record reaches it as its sleeve and as *"then 2 albums"*. Naming the album there is a composition change on the one surface whose geometry may not move, and it waits for him to ask for it directly. **The other reading was a real defect and is fixed**: `AlbumArtistVm::name()` is `None` for a compilation (`Various`) and an untagged record (`Unknown`), and the bar and the Now playing placard both stopped there and drew an empty lane — while the album page, the wall tile and the picker panel all draw `label()`, which always says something. A compilation whose file carried no artist tag therefore lost its artist line *in the bar only*, which is exactly *"the album page shows it, the bar doesn't"*. `NowPlaying::artist_line` is the one answer now — the track's own tag, then the album's artist, then who the record is filed under — and both surfaces call it. `NowPlaying::artist` is untouched, because MPRIS publishes it as `albumArtist` and baz's placeholder words are not an artist's name; the test asserts that boundary too. |
| *"the bell icon is a little bit narrow/skinny"* | **shipped 2026-08-15 — item 48** | Measured off the sheet rather than judged: `BELL`'s widest point was its rim at `0.160 → 0.840` = **0.68** of the em box, against `GEAR` 0.84 and `HOME`/`NOW_PLAYING` 0.88 in the identical `ICON_PX` 20 box — about 13.6 px of ink where the gear one seam away lays 16.8, and the narrowest mark in the app bar's right cluster. The mouth is **0.78** now, by a 1.147 scale about the vertical axis so the dome, the flare's shoulders and the rim keep their proportions to each other exactly: the profile was right and only its width was wrong. The height is untouched at 0.79, so the bell stays no wider than it is tall, and `the_bell_is_as_wide_as_the_cluster_it_stands_in` holds it in its neighbours' range from below and against its own height from above. |
| *"the new playlist should be like a ghost playlist with a + in the middle called 'New Playlist' on the playlist page, not a button"* | **shipped 2026-08-15 — item 45** | It was a word button in the strip — a control about *making* a thing, filed in the row that says how the collection is *arranged*. It is the wall's **first cell** now, at the same edge, mat, caption block and state-rule lane as a real tile, so nothing moves when the ghost becomes a list: the picker panel's own ghost row at wall scale, keeping that row's two rules — the sleeve is `theme::ghost_sleeve` with the drawn `Glyph::Plus` and never anything resembling artwork, and it answers the pointer like its neighbours. It is not selectable `Content`, does not enter the rail's index, and stands in item 44's unlettered lead run beside `Favourites`. The mark is drawn at `theme::GHOST_MARK_PX` = `ICON_PX × 2`, the sprite's own raster edge, so it is pixel-exact rather than an upscale. |
| *"we need to examine the flow for the vibe playlist. the ux is terrible and it makes no sense right now"* | **shipped 2026-08-15 — item 50** | Six faults, all visible in the source before anything was run, all answered. **The order was inverted**: `Shape the journey` — the energy shape and the waypoints, which exist to *inform* the request — stood below the button that spends it, and `Save playlist` stood above the name field it needs; the form reads describe → shape → compose → review → name → save now. **The consent gate stood mid-flow** (prompt → `Create mix` → a paragraph → a second, differently named button) when the engine never needed two presses: `Message::VibeCreate` already starts the analysis and composes when it lands, so the paragraph moved *above* the press and the second button went — `VibeCancel` had no sender left and went with it. **One vocabulary**: `Make a mix`, `Create mix` and `Another version` are gone; the place makes a *playlist* and the Vibe route *composes*. **Manual and Vibe draw one row** (`draft_row`: the shared track row, the favourite slot, the icon slots) where Manual had bare `Up | Down | Remove` word buttons and no artwork. **The composer moved out of `views::home`**, which had exactly one caller and it was this place. And the analysis-failure note is the room's *alert* ink rather than its accent, which it had been riding into that file on Home's permit for the `CONTINUE` needle. |
| *"can you make sure the player controls on the bottom are right justified. there seems to be a gap between controls and the mute button. the top bar has weird spacing as well for icons/controls"* | **shipped 2026-08-15 — item 47** | **The justification was already right** and saying so is part of the answer: the cluster is `align_x(Right)` against a `Fill` identity zone on `BAR_EDGE_PAD` 14. The gap was `signal_path` reserving `SIGNAL_W` **96** whenever the chain is direct — every ordinary run — *between* `Shuffle` and the mute button. The reservation is right and the **position** was wrong: a note that appeared mid-run and shoved the volume sideways is movement on the one surface ADR-0020 forbids it on, and at the cluster's leading edge the same reservation abuts the identity zone's `Length::Fill`, so it is invisible while empty and still moves nothing when it fills. `BAR_TRAILING_W` is unchanged at 636 — the seam the signal path gave back is exactly what pairing `Repeat` with `Shuffle` saves. **The app bar had three rhythms for one kind of object** (`GAP_XS` 4 in the history pair and the window buttons, `GAP_LG` 16 between the bell and the gear — the between-clusters number spent inside a cluster); the bottom bar had been on 8-inside/16-between all along, so that is the rule and it is now `theme::CONTROL_CLUSTER_GAP`, with a detent run still touching because it is one control with several states. The budget moved with the geometry rather than being renumbered: `APP_BAR_LINE` 850 → **854**, `WINDOW_FLOOR_W` 860 → **864** by its own derivation, slack unchanged at 10. The app bar's own 160 px empty slot was **left alone**, with the reason now written down: it already abuts the drag gap's fill, and collapsing it would slide the right cluster 160 px as you walk between places. |
| *"the playlists page does not need the word 'playlists' at the top"* | **shipped 2026-08-15 — item 43** | A divergence rather than a preference, which is what made it easy to settle: **the Library names no place**. Its strip is `views::top_bar` — arrangement keys, then transient scan status — while `views::playlists` led `place_header_led` with `place_name("Playlists")`. The place is already named by the lit lane destination and by every playlist page's `Playlists › Name` breadcrumb. What remains in the strip is what the Library's carries: how the collection is arranged. |
| *"a-z playlists should group alphabetically -- use the exact same pattern as the library please"* | **shipped 2026-08-15 — item 44** | Half of it already existed and none of it was visible: `views::playlists`' rail computed `GroupHeaderVm::Initial` runs and handed the shared `Spine` each group's first row, so the rail jumped to boundaries drawn nowhere over a flat grid under one `section_rule("All playlists")`. *"The exact same pattern"* is taken literally — the layout engine is `shelf::Shelves`, the heading band and its pinned copy are `views::shelf::group_band`/`pinned_band`, **extracted from the Library's private ones so there is one band rather than two that agree**, and all three orderings group (the rail already projected `Date created` and `Played` into the Library's elapsed buckets). Two decisions rather than assumptions: the **lead run has no heading and holds the create tile and `Favourites`** — neither belongs in a letter, one being a control and the other a built-in with no creation stamp and no alphabetical place among the listener's own lists — and it may never be pinned, because the pinned layer paints an opaque band and a blank one would be a strip over the covers under it. `App::request_playlist_art` reads the same `Wall` projection: a grouped wall's visible tiles are no longer `scroll / row_h`, and the flat arithmetic would have decoded the collages of tiles a screen away while the ones on screen stayed gradients. |
| *"no need for the playlist count and another noise"* | **shipped 2026-08-15 — item 43** | Both counts went, and the header's was also against its own documented rule: the strip's note (`13 playlists`) sat in a slot whose doc says it is *"for a statement about the place, never a keyboard hint"*, with Settings' *"Kept in config.toml…"* named as its only customer. The per-tile `Playlist · 12 · 41:03` went too — a line spent saying *playlist* under every tile on a wall of nothing but playlists. **The note slot survives for the deletion confirmation** (`Delete “Zed”?`), which *is* a statement about the place, and **`PanelRow::counts` is untouched**: ADR-0024 §A3.1's leading noun earns its place in the returns lane and the picker panel, where a made thing's line sits beside a found thing's and must not be read as an artist's name. The caption lane stays, empty, because `theme::CAPTION_H` is the grid's and a tile one line shorter than the Library's would break the pitch the two walls share. The Artist page's count is the neighbouring case and **stands**, with the distinction now stated in doc 06 §11: a record count is a fact about that artist, where a count of the tiles in front of you is a fact about your own scroll position. |
| *"Can we do a pass for consistency of design across the app"* | **shipped 2026-08-15 — item 51** | `docs/design/06-composition-audit.md` §11 — an inventory with **a verdict per divergence**, because an audit that only lists differences hands the decisions back. Eight closed by items 43–50 (place strips that do and do not name their place; the note slot spent on tallies; three anatomies for a row's controls; reserved empty slots that read as holes; three control seams where there should be two and an exception; two collections with two layout engines; the accent on a failure note; four names for one act) and three deliberately left open, with reasons: the bar naming no album, the app bar's 160 px reservation, and `place_header_with`/`place_header_led` being two functions over one geometry. |
| *"we made this app aggressively optimise cached images and memory usage… but we never specified a sensible limit"* — and the wall that would not load until you touched it | **shipped 2026-08-14 — item 37** | **Two halves, one subject.** *The wall:* `ThumbJobs::focus` drains the foreground queue and re-adds only its argument — a re-aim, correctly — but `request_target_thumbs` was handing it a **delta**, the targets neither cached nor already queued. So a re-aim over an unchanged viewport passed the empty set, and the replace threw the queue away and put nothing back. On an untouched cold start that happens twice (iced emits `Scrolled` when the scrollable measures its real bounds, and `WindowResized` when the first resize lands) and nothing re-queues afterwards. Measured on a fresh 25-album library with **no interaction at all**: 2 decodes in 15 seconds and four frames pixel-identical with every cover a gradient; after the fix, 8 — the whole visible wall — and a warm resize shows no gradient flash and re-decodes nothing. *The limit:* the **retained** tier was a `HashMap` bounded only by the size of the collection, so every figure this project published was a measurement of what that came to on the owner's 393 albums, and a 5,000-album library retained over two gigabytes with nothing saying so. `THUMB_BUDGET_BYTES` is **160 MiB**, chosen against the collection the feature exists for and the smallest 32 MiB step that clears it; `THUMB_CACHE_ENTRIES` is now **derived** from a stated 25 MiB speculative share and comes back to the same 64, so the decision moved to the side of the equation that can be argued with without changing behaviour. `retained` is an LRU so "least recently visited" is a real ordering, and the trim takes speculative art first. The resident tier stays exempt — a visible sleeve becoming a gradient is what the tier exists to prevent — and that hole is stated rather than assumed: the widest supported window pins 51 MiB at its worst density, a third of the budget. With the hero and artist tiers, **170 MiB is all of baz's decoded artwork**, which item 39's Debug readout now lets you check against the real resident set. Evidence and both harnesses: `docs/design/impl/art-memory-budget/`. |
| *"can we add a tiny bit of a gap between items in the top sidebar and the recent history part of the sidebar. basically make things have just a little bit of air"* | **shipped 2026-08-14 — item 42** | **Not the previous ask reversed** — a different quantity, and both readings are right. *"The vertical padding on the sidebar recent list… there doesn't need to be any"* was about padding **inside** the row: `SIDEBAR_ROW_H` carried a `GAP_SM` above and below its own sleeve, so the card the pointer lights was 16 px taller than the only thing drawn in it. Item 39 made the row its sleeve, at which point the cards **touch**, and a column of touching cards reads as one block the pointer cuts a slice out of rather than as a list of things. So the air arrives **between** the rows: `SIDEBAR_ROW_GAP` = `GAP_XS` **4**, the smallest step on the 4 px lattice, which is what *"a tiny bit"* buys without reaching for `GAP_XXS` (the ladder's one named exception). The row height is untouched, so the card stays exactly its content's size. The ask names both halves, so it is one rhythm and not two numbers: both of `views::lane`'s columns carry the same token, asserted as the *same token* rather than measured, because the failure worth guarding is the two drifting apart. `SIDEBAR_ROW_PITCH` 52 is declared alongside because the lane's virtualization counts rows against a pitch, and one that read the row's own height would ask for the wrong covers four rows down. *Verified by eye for the head; the `RECENT` half is held by the shared token and its test rather than by a render, because the lane's recency is session-scoped and a headless run has no confirmable playback to fill it.* Evidence: `docs/design/impl/lane-row-air/`. |
| **Not an ask — found while verifying the above.** The notification bell draws as a solid disc | **shipped 2026-08-14 — item 40** | **Three faults stacked in one 20 px square, each of which made the next invisible.** (1) `views::status`'s dot carried `theme::status_dot` **and** `align_right(Length::Fill)` / `align_bottom(Length::Fill)` on **one** container — and those two calls set that container's bounds to `Fill`, while a container paints its *own* bounds, so a 999 px corner radius was painted across the whole glyph box. The app bar's health indicator has been a plain coloured circle for as long as it has existed, and nothing was ever visible under it. (2) What it was covering was not the bell anyway: the sheet was handing `Bell` the forward arrow's sprite (the row below). (3) With both of those fixed, the real `BELL` outlines drew for the first time and were **also** a blob — 0.56 wide by 0.60 tall with near-vertical sides, and a "base" flush with the body rather than a rim, so there was no mouth to read; the doc comment's claim that it was drawn *"at the shared icon stroke"* had never been true either. It is a silhouette on `HOME`'s precedent — the sheet's stroke rule is about **open angles**, so that `OPEN` and the history arrows cannot read as `PLAY`'s solid mass, and a bell has no such twin — reshaped so the ratio does the work: a **0.30** dome flaring into a **0.68** mouth, a crown proud of the dome and a clapper across a real gap. The badge deliberately overlaps the rim rather than the rim being cut short to clear it; a lopsided bell would read as badly drawn in the three tones out of four where the badge is quiet ink. A test pins which container the paint is on, which is the only one of the three a source scan can hold. |
| *"the window controls disappear when we make the window narrow"* | **shipped 2026-08-14 — item 39** | Not the 10 px floor case the first suspect named. **The bar's declared line was missing 156 px of tenants it was drawing**: the Back/Forward pair (84 px and a `GAP_LG` seam) and the health bell (40 px and a `GAP_LG` seam) both entered `app_bar::view`'s `row!` on 2026-08-13 and neither ever entered `APP_BAR_LINE`. The real line was 858 against a window that opened as narrow as 712, so the three buttons — the row's last child — left the trailing edge **146 px before the floor**, not at it. Every test that could have caught it recomputed the constant's own expression, so the arithmetic agreed with itself and never met the geometry; the budget test now walks the tenants of that `row!`, pinned to its source. `APP_BAR_LINE` 702 → **850**, `WINDOW_FLOOR_W` 712 → **860** — the floor moved because the bar was measured, not because it grew. Letting the search well yield instead was considered and refused: it would put the one app-wide control on a measure that changes underneath the query in it, and ADR-0040 §4 makes the buttons unconditional. Evidence: `docs/design/impl/backlog-pass-2026-08-14/`. |
| *"the back button icon is wrong and so is the forward"* | **shipped 2026-08-14 — item 39; the outlines were never what was on screen** | The first telling was answered by redrawing the two glyphs, and it came back because the drawing was not the fault. **`Glyph::ALL` and `Glyph::index` are two hand-written orderings of one list and they disagreed**: `VisualFacts` was appended to `ALL` before the history pair and numbered after them in `index`, so the sheet handed out four wrong sprites — `HistoryBack` drew the facts mark, `HistoryForward` drew the back arrow, `Bell` drew the forward arrow and `VisualFacts` drew the bell. No test could see it: every sprite existed, every sprite was the right size and every glyph got *a* stable handle, and only the pairing was wrong. It is a module-scope `const` assertion now — verified to fire by reintroducing the swap — rather than a test, because the duplication has to stay (a match arm per variant is what makes a new glyph a compile error) and a duplication that must stay is checked where it cannot be run past. **The outlines were separately wrong and are separately fixed**: one self-intersecting polygon whose overlap cancelled under the even-odd cast, leaving a hollow head whose "stroke" was a tapering sliver — a hairline at the back corners, six times that near the tip — so there was no stroke weight to re-proportion for `ICON_PX` 20. They are three plain outlines now, two 45° arms and a shaft at the set's 0.145, which is `OPEN`'s and `ARROW_UP`'s weight. The stale "at the same 16 px size" sentence went with them. |
| *"the right hand rail is acting strangely"* / *"when mousing over the rail on the playlist and library view the zoom doesn't really seem to work. it sometimes zooms and the other times it doesn't"* | **shipped 2026-08-14 — item 41** | The second telling is what made it findable, and it was in neither of the places the first diagnosis looked. **`spine.rs` said the premise out loud and the premise had expired**: *"there is no tween, no clock, no subscription and no message: iced requests a redraw for every window event."* True of iced 0.13; **false of 0.14**, which baz migrated to — `Shell`'s redraw request now defaults to `Wait` and a widget that wants a frame has to ask. The spine never asked, because its whole design is *no state, no message*, and a widget that publishes neither gives the runtime no reason to draw. It is the only widget in baz whose own appearance is a function of the live cursor with nothing published — `groove` and `needle` publish on cursor motion, so their frames come from the shell's own update; `menu::Area`, `drag::Source` and `window_frame`'s wrappers only forward a cursor to their children. Measured at 1280 × 860 with nothing else on screen moving: the lens drew **once**, when the pointer entered the lane, and then froze — a sweep from y = 200 to y = 480 gave **seven consecutive pixel-identical frames**, and a one-pixel nudge changed nothing. The *"sometimes"* is other work forcing frames the lens happened to be redrawn in: entering the lane changes `mouse::Interaction`, and so do scrolls and tooltips. `update` now requests a redraw while the pointer is in the lane **and for one event after it leaves** — that last clause is the snap back, and it is the whole of the widget's new state, one `bool`, so the module's claim to have *"no state at all"* is qualified rather than deleted. At rest nothing is asked for, so a mouse crossing the wall costs what it did. The two views are one `Spine` (`views::shelf::index_rail_from`), so this is one defect seen twice. **The earlier diagnosis in this row was wrong twice over** — `RAIL_HIT` is the volume groove's and the seek needle's rather than the rail's, and the 16 px of lane height the control pass took was a real change but not this one. Evidence: `docs/design/impl/rail-lens-redraw/`. |
| *"the vertical padding on the sidebar recent list should not be like that… there doesn't need to be any"* | **shipped 2026-08-14 — item 39** | `SIDEBAR_ROW_H` is the sleeve's own **48**. The 16 px lived *inside* the row, so every row carried air around its own sleeve and the card the pointer lights was 16 px taller than the only thing drawn in it — which is why it read as loose rather than generous. The pitch is now `SIDEBAR_DEST_H`'s too, so the head's destination tiles and the list below the rule share one rhythm. Re-derived rather than renumbered: the two-line block (`LINE_BODY` 20 + `GAP_XXS` 2 + `LINE_META` 16 = 38) still fits centred with 5 px over and under, and 48 still clears the hit floor, both asserted. |
| *"the now playing pip on the recent list is also in a strange position"* | **shipped 2026-08-14 — item 39** | The trailing slot stays (ADR-0030's 2026-08-12 amendment: a conditional dot before the name reflows the text). What was never settled is settled: the dot stands on the **title's** line, which is the line it is a fact about, rather than on the two-line block's centre — the `GAP_XXS` seam between the two lines, level with neither. The slot carries the text column's own shape (a `LINE_BODY` box over the same seam and `LINE_META`), so the row's `Center` alignment lands it by construction and there is no offset for a later edit to leave stale. Done together with the row-height ask above, as that entry asked. |
| *"can we make the icon for the app align with the icons in the sidebar"* | **shipped 2026-08-14 — item 39** | The assertion is back, and **the mark yielded**, as that entry predicted: the lane's 8 px pad is load-bearing for the whole collapse-cannot-shift-a-pixel rule, so it may not move. The mark's `GAP_MD` **lead** is gone — it was putting zone 1's ink 12 px inside law L1's gutter, which `APP_MARK_PX`'s own doc comment already claimed it did not — and the mark is drawn at `SIDEBAR_GLYPH_PX` **32**, so its centre is `APP_BAR_EDGE` 16 + 16 = 32, the lane's `SIDEBAR_HEAD_GLYPH_X` exactly. An equality of *tokens*, not of two numbers that happen to land together, and asserted in the lane's own test because the lane is the side that may not move. The mark and the four destinations below it are now the same square as well as on the same spine, which is the stronger reading of the ask; the committed 64 px raster at 32 is crisper than it was at 28. `APP_BAR_LINE` and `WINDOW_FLOOR_W` were re-derived, not renumbered — see the window-controls row above, which moved them much further for a different reason. |
| *"the pip when Now playing is active is in a strange position"* | **shipped 2026-08-14 — item 39** | The dot tucks against the **mark's** top-right corner rather than the tile's, inset by `SIDEBAR_GLYPH_INSET` — a new declared token, `(SIDEBAR_GLYPH_BOX − SIDEBAR_GLYPH_PX) / 2`, so the next change to either size carries the dot along instead of stranding it a third time. It stays stacked on the tile rather than moving to the row's trailing edge: the trailing slot does not survive the collapse, which is the entire reason ADR-0030 §3 put it on the glyph. |
| *"when I narrow the window, it force collapses the sidebar, but it still shows the collapse icon"* | **shipped 2026-08-14 — item 39** | `marks` is handed the resolved `open`, not the persisted intent. The footer is the inert `Expanded` mark below the floor — the branch `lane_toggle` always had and in this state never reached. **The intent is deliberately kept**, so widening restores the open lane, and that is exactly why the footer must read the resolution of it: the remembered value is a wish, and the foot of the lane states what the lane *is*. Pinned by a test that also caps how many times `view` may read the intent at all. |
| *"the now playing song title seems cut off when it is long"* | **shipped 2026-08-14 — item 39** | The lane's reading is now `views::fitted_line` — lifted out of `lane.rs` rather than copied, so there is one fitted-prefix-plus-ellipsis-subslot implementation and `SIDEBAR_ELLIPSIS_SLOT_W` is `ELLIPSIS_SLOT_W`. **All three lines are fitted, not just the title**: the artist and the continuation carried the same `Wrapping::None` and the same clip, so they had the same failure and the title was merely the one long enough often enough to be noticed. The measure is `theme::bar_title_lane_w(window_w)` — the window less the bar's two edges, the newly-declared `BAR_TRAILING_W` 636, the sleeve and its seam, and the new Favourites slot and its seam. It matters most at the narrow end: at the window's floor the lane is a little over a hundred pixels. The one tenant outside the sum is the skipped-tracks note, which is sized to its content and absent in every ordinary run — so the ellipsis is a floor on the failure, not a promise clipping can never happen, and the constant says so. |
| *"add to favourites should be beside the playing song in the bottom bar"* | **shipped 2026-08-14 — item 39** | `views::page::favourite_slot`, the same control the record page's rows, the playlist page's rows, the wall's `Songs` rows and Now playing's title line draw. It is a **sibling** of the block's door, not a child: iced runs the inner control first, so nesting it inside `back_to_source`'s button is how a control ends up meaning two things depending on which pixel you hit. The slot is reserved in every state and `bar_title_lane_w` has already subtracted it, so hearting a song changes ink and never geometry — the bar's own law. A sounding file with **no library row** cannot be favourited at all (Favourites is durable library data keyed on a row) and gets the slot drawn **inert** rather than absent: `favourite_slot_maybe`, with a tooltip that says why. Absent would move the title lane, which is the one thing this bar may not do; *absent, not disabled* is a rule about controls inert **in a place**, and this one is inert for one file on a permanent surface. |
| *"can you remove 'Recent' from the sidebar when it is not collapsed"* | **shipped 2026-08-14 — item 39** | Gone, and `heading` with it. The test was **rewritten to assert the absence** rather than deleted, as that entry asked — an unasserted absence is an invitation, and the next edit that felt the list wanted introducing would re-add a word over it unchallenged. It also asserts `heading` itself is gone, so `-D warnings` is not what finds it. The empty-history special case went too: with no heading to stand over nothing, an empty column is already honest. |
| *"I'd like to also backlog a resource usage feature in the app to show how much RAM/CPU it is using in the debug menu"* | **shipped 2026-08-14 — item 39** | Settings → Debug leads with `This process`: resident memory and a percentage **of one core**, `top`'s convention, so a build saturating four cores reads 400 % rather than a clamped 100. The clock is the section's and nowhere else's — `add_place_clocks` installs it under the same guard every place-owned clock carries, and leaving the section resets the meter so returning warms up again rather than dividing a fresh counter by however long the listener was elsewhere. The first tick reports memory and refuses a rate rather than printing a zero. **No new dependency**: `/proc/self/status`'s `VmRSS` is in kB directly (`statm` reports pages, and a page is 4, 16 or 64 KiB depending on the machine), and `utime`/`stime` are in `USER_HZ`, which the Linux ABI fixes at 100 — so neither read needs `sysconf` and `libc` stays transitive. Windows and macOS answer `Unavailable` and the place says why; every route to their figures is a reviewed dependency, and a zero would be worse than nothing. Six pure tests cover the arithmetic. **Its first reading was of baz itself**, and it is worth recording what it found: 99.9 % of one core idle on the Library — which is the *harness*, not the product. That run forced `ICED_BACKEND=tiny-skia`, the software path, which has no vertical blank to block on; the shipped renderer measures 4 %. `DEVELOPMENT.md`'s headless recipe and `capture.sh` both reach for tiny-skia, so the next idle measurement taken the documented way will find the same phantom. |
| *"the vibe feature… feels like it belongs on a 'New playlist' flow with two options that are 'manual' and 'vibe'"* | **shipped 2026-08-14 — item 31** | The Playlists root now opens one navigable, resumable creation place. Manual and Vibe populate the same visible unsaved draft and converge on one ordinary `.m3u8` Save boundary; Home retains only a discovery shortcut. Manual accepts app-bar search additions, and the panel's contextual New row carries its held tracks into the same flow. Neither route starts playback. |
| *"the prompt might be a good default starting name for the playlist"* | **shipped 2026-08-14 — item 31** | Review exposes a deterministic editable suggestion from the first semantic phrase, strips structural lead-ins, replaces forbidden separators, stops at a word boundary before 48 characters and presents any collision suffix before Save. |
| *"can you maybe allow for the vibe playlist to be configured via a few curves instead where we can control the prompt somewhat? or maybe we give a few defaults? when they pick to make a vibe playlist they should be guided by a wizard"* | **shipped 2026-08-14 — item 31** | Free text remains primary. The shallow flow adds three pressable examples, four discrete energy shapes and up to three semantic waypoints under progressive `Shape the journey` disclosure; the visible choices become part of the local semantic request. |
| *"can we get the playlist page to have the same basic section header pattern as the library"* | **shipped 2026-08-14 — item 34** | Playlists retains the common place strip and now introduces its tile collection with the shared section-rule hierarchy. |
| *"can you make the controls on the bottom bar all right justified"* | **shipped 2026-08-14 — item 35** | Track/art identity is the sole left fill; time, transport, Repeat/Shuffle, signal path, mute and volume now form one stable trailing cluster. |
| *"a favourites playlist which is built-in — and a heart icon next to the track to allow it to be added there"* | **shipped 2026-08-14 — item 32; Now Playing follow-up** | Favourites is pinned and protected as durable schema-v10 library data. Shared row hearts toggle without playback or list edits, including the sounding library track's title line on Now Playing; tagged song identity follows moves/remounts and retains missing members, while fully untagged tracks use exact native paths. |
| *"the current time and length of track just feels like it's in the wrong place or doesn't look good… it just looks like it's sitting in the open"* | **shipped 2026-08-14 — item 35** | The floating stamps are one fixed `elapsed / total` readout immediately beside transport; the full-width needle and aimed-time hover remain. |
| *"the app icon doesn't align with icons on the left hand bar… the icons on that sidebar shift when we collapse the sidebar"* | **shipped 2026-08-14 — item 35** | App mark and lane glyphs share one measured 40 px global centre in both lane states; collapse removes only label width to its right. |
| *"the back and forward icons seem wrong on the top bar"* | **shipped 2026-08-14 — item 35** | History keeps its fixed disabled boxes and Alt+Left/Right behavior but now uses short-shaft browser arrows distinct from Open and track skip. |
| *"can we make the now playing more exciting and dynamic. more options for classic visualisations"* | **shipped 2026-08-14 — item 36** | One independent mark cycles Off, Spectrum, a bounded 32-frame Waveform and a bounded 32 × 24 Spectrogram. The fixed 3,200-byte history, transform, sample tap and clock stop outside visible sounding Now Playing; all modes are direct signal readings without decorative transition motion. The stereo-vector Oscilloscope prototype was rejected because the established feed is mono and cannot truthfully report phase. VU, particles and fake vinyl remain excluded. Evidence: `docs/design/impl/now-playing-visualizers/`. |
| *"can you make it more clear which options you are hovering on for the album cover hover buttons"* | **shipped 2026-08-14 — item 34** | Hovered/keyboard-selected veil options gain a persistent contrasting field and one-pixel rule in addition to ink, with a clamped Play/Queue/Open keyboard axis. |
| *"a repeat option which makes the current song play again and again"* | **shipped 2026-08-14 — item 33** | Repeat current track is a persisted engine/player property with a lit resident toggle. Natural end restarts the same entry; explicit skip, seek and selection remain explicit, and Repeat coexists with Shuffle. |
| *"on the search the bottom of the text can be cut off… seems like the row isn't high enough for two rows of text"* | **shipped 2026-08-14 — item 34** | Result height is derived from the body and metadata line boxes plus shared vertical padding, so descenders cannot be clipped by an unexplained fixed height. |
| *"can you make sure the 'Next' and 'End' are clearly adding to the playlist since they seem a bit obscure?"* | **shipped 2026-08-14 — item 34** | Live-run actions now say `Add next | Add to end`; empty-run `Enqueue` and saved-file `Add to playlist` remain distinct. |
| *"for albums can you also allow selection via arrows for Play and Open… on the playlist view, the links to artist and album do not make the mouse a cursor"* | **shipped 2026-08-14 — item 34; narrow-row follow-up** | Album search gains a clamped Play/Open axis; cover selection gains Play/Queue/Open. Playlist metadata routes are focusable buttons with link cursors and exact bounds; their shared flexible title/artist lane is physically clipped before the fixed Album column and trailing controls as the window narrows. |
| *"I still think our thumbnail handling isn't right… sometimes thumbs unload themselves as you scroll around"* | **shipped 2026-08-14 — item 30** | A sleeve that actually reached a visible wall/page/chrome target now moves into collection-bounded session retention when it scrolls away; only speculative completions compete in the unchanged 64-entry LRU. Returning therefore reuses the same RGBA handle synchronously instead of exposing a gradient and scheduling another decode. One complete target snapshot feeds the queue, panel opening invalidates it, Home Continue nominates its silent record, and a stale-density retry no longer replaces other visible work. The regression traverses 810 displayed covers before returning to the first 18. The owner's 393-album / 8,602-track index caps full Dense CPU retention at 60.0 MiB; real and synthetic density ceilings plus renderer-trimming evidence are recorded in `design/impl/scrolling-thumbnail-retention/`. |
| *"the sticky bar when scrolling through artists etc. has too much padding on the left so it is misaligned"* | **shipped 2026-08-14** | The ordinary group heading was centered inside the wall scroller's content width, but the pinned copy was centered across the complete wall. Because the scroller reserves 112 px on the right for the rail and scrollbar, sticking introduced an exact 56 px rightward jump. The pinned field still paints full-width, but now reserves the identical right lane before centering its shared header block. One regression proves the ordinary and sticky left edges at 696/900/1280/1920/2560 widths and all four densities; real 900, 1280 and 1920 renders exercised A–Z, Artist, Year, Genre, Added and Played. The vertical hand-over, clip and artist-header hit geometry are unchanged. Evidence: `docs/design/impl/sticky-header-alignment/`. |
| *"when I scroll the album images, I am seeing them actually over the top of the top and bottom bar, as if their 'z index' is wrong"* / *"after a reset it's ok... maybe worth investigating how this might happen"* | **shipped 2026-08-14** | The fault was iced 0.14's inactive-scrollbar path: it passes a logical viewport without opening a renderer layer, while image drawing ignores that viewport. A stale scrollbar/layout transition could therefore let translated sleeves escape until reset rebuilt widget state. Baz now wraps the shared body—outside every individual scroller—in an unconditional physical renderer clip between the resident app and transport bars, and applies the same intersection to pointer updates, interaction and body overlays. Later search/status/menu overlays intentionally remain whole-window. Structural regressions lock the clip and composition order; an isolated 39-album run combined dense scrolling, panel churn, navigation, resize round-trips and density changes without crossing either boundary. Evidence: `docs/design/impl/body-artwork-clip/`. |
| *"we need a way to prune nonexistent albums when they are removed"* | **shipped 2026-08-14** | The existing scanner already removes individual files only after its four positive-evidence gates. Its remaining case—an album directory whose parent is absent—is now emitted separately only after the owning root completes a walk; unavailable roots, cancelled workers and unreadable ancestors nominate nothing. Settings previews every exact missing path and requires an explicit `Prune index` confirmation. Confirm reuses `Library::forget_paths`, atomically removes only the previewed rows and retains first-seen tombstones so bringing a mistaken share back repairs the index. Audio, playlist files, listening history and the current run remain untouched; wall/search/selection and artwork requests rebuild without starting playback. Cancel is inert. |
| *"delete playlist should be possible from the playlists page with 'are you sure' confirmation"* | **shipped 2026-08-14** | Every saved-playlist tile now exposes Delete beside Play/Open. Pressing it names the list in the collection header and replaces that tile's actions with `Move to Trash | Keep`; Keep is inert. Confirm and the existing detail-page action call one `delete_id` implementation using the platform trash, close hover/confirmation/undo state, refresh the rows and select the next tile at the same position (or the preceding last tile) without starting playback. Foreign playlists follow the same path. |
| *"ensure that resampling is shown as a warning in our settings, and ensure the event log notices it and makes it a warning indicating how to fix"* | **shipped 2026-08-14** | An active engine-reported conversion now draws an alert in Settings → Playback and enters the canonical event history as a warning. Both surfaces use one event-derived explanation naming the source/output rates, the exact device-rate or fixed-boundary cause, and the choice that restores a direct path. A session tracker deduplicates identical continuing reports, clears on a direct path or playback end, and treats a later conversion as new. Exclusive conversions are included. The copy explicitly scopes the warning to Baz's boundary resampler and says OS-mixer conversion downstream is outside Baz and unreported. |
| *"is the picker for audio devices actually taking effect immediately?"* | **verified and shipped 2026-08-14 — next launch** | The complete path is restart-scoped: launch reads `output_device`, opens the engine once against that endpoint, and subsequent picker changes only update the selected value and persist config. There is deliberately no live reopen command, so the current run is never interrupted and no new signal-path event can honestly arrive until the next engine starts. Settings now holds and shows both facts—`In use now` and, when different, `Selected for next launch`—instead of letting the picker imply an immediate move. On restart, the engine's real events become the new signal-path truth. A chosen endpoint that cannot open already seeds playback unavailable and the canonical health error; the same failure is now shown beside the picker with `Select another output and restart Baz.` |
| *"cached images seem to be spotty about loading/unloading… ensure what is on screen always has an image loaded if it exists… let's not overcomplicate it"* | **shipped 2026-08-14** | The existing simple policy remains: one un-evictable union of current wall/page/chrome targets plus the same 64-entry LRU for off-screen recent art. The all-consumer audit found three supply defects: Queue page rows were requested into `page` and then cleared by the later chrome pass; Home nominated Recently added but omitted its always-visible All songs collage; and the floating playlist panel read collages without nominating them. All now enter the same resident union and decode scheduler, with no new tier or prefetcher. A synthetic 800-album churn regression covers simultaneous wall, page and chrome targets, proves every visible handle survives, and proves off-screen residency remains capped at 64. The platform-neutral path runs under the project's Windows CI as well as Linux; prior rendered GUI stresses remain applicable because the defect was target supply rather than renderer scheduling. `design/impl/visible-art-residency/` carries the inventory. |
| *"make the left and right gutters smaller for the top and bottom bar… the settings cog [is] farther in… the app icon is also too small and sits too far in… [and] the album art in the bottom left… x and y padding [should be] the same"* | **shipped 2026-08-14** | Resident chrome now has compact edge geometry distinct from the collection's unchanged 40 px hang. The app bar uses a 16 px ink edge, grows the committed full-colour mark from 16 to 24 logical pixels, and derives 8 px trailing container padding so a centred glyph ends at 16 while its 32 px target remains outside the six-pixel borderless resize band. Its 600 px maximum line retains 96 px of drag slack at the 696 px window floor. The bottom bar uses `(80 − 52) / 2 = 14` px on both horizontal edges, making the sounding sleeve's rendered left and top insets exactly equal. Isolated 1280×860 and 1920×1080 Xvfb frames, including a playing state, verified the pixels; tests pin the edge, budget, resize clearance and equality. ADR-0040 and `design/impl/bar-edge-composition/` record why the two bars deliberately differ by 2 px. |
| *"can we show the app's log somewhere in the settings? maybe under debug"* | **shipped 2026-08-14** | Settings now has a Debug section showing Baz's existing tagged runtime diagnostics—not a second copy of the notification bell's curated event history. Every `[startup]`, `[scan]`, `[playback]`, `[config]`, `[mpris]`, playlist and related console line also enters a process-local 256-line ring with elapsed time; Debug presents newest first so new arrivals need no forced scroll. The developer console remains, while packaged Windows can finally expose these diagnostics without restoring one. The stream is deliberately session-only and never writes potentially private filesystem paths to another disk log; the place says that explicitly. The development-only message meter enters the same bounded stream only when `BAZ_MSG_LOG=1` already enables it. Unit tests pin capacity/newest retention and a real isolated Settings render verified the complete launch history. |
| *"should we add 'enqueue next' and 'enqueue at end' as well"* | **shipped 2026-08-14** | Search tracks over an existing live run now expose the compact `Play | Next | End` axis; pointer presses and Left/Right choose the same actions. Next inserts immediately after the engine-confirmed cursor and consecutive presses preserve the listener's order instead of reversing. End keeps the existing append. With no run the row honestly collapses to `Play | Enqueue`; on a saved-playlist page it remains `Play | Add to playlist`, because that is a file edit. Neither insertion starts playback. `UpdateQueueNext` carries the complete absolute edited queue plus its forced successor, so shuffle yields to the explicit next choice once and then continues its remaining shuffled pass; the sounding track and buffered audio remain untouched. Pure tests cover action clamping, collapsed states, repeated insertion order, absolute splice geometry and shuffled successor preservation; real 1280 px renders verified both row forms. |
| *"we should probably allow themes -- if we create some basic ones e.g. light, dark, light dark, dark light (in betweeners with a bias) but build it out in such a way that it is easy to generate your own theme -- that would be nice. like provide some JSON (ask an AI to generate it for you etc.)"* | **shipped 2026-08-14** | Settings → Appearance selects and persists Closing Time, Stone, Plaster or Reading Room and previews their surface/ink/playback polarity. Custom v1 JSON can be pasted or opened locally, is normalized into Baz's config `themes` directory only after validation, and is selected for the next launch; the selected room can be exported through a save dialog. Runtime validation bounds documents to 64 KiB, rejects unknown fields and malformed colours, enforces stable IDs, the Oklab elevation/dead-zone law and WCAG ink/status floors, and reports the exact failing field. Missing or invalid custom selections fall back to Closing Time without blocking startup. Whole-app switching on restart was the decided behaviour at the time, because the glyph atlas was process-cached; **item 54 closed that on 2026-08-15** — the sheets are kept per room and a picked or imported room stands on the next frame. Two more rooms landed with it. `docs/themes/` carries the v1 schema, one example document per built-in and an AI prompt. No network or executable theme content was added. |
| *"we would like to be able to update the app when new releases are put out -- this is something we should look into solving"* / *"I am not 100% sure I need to use Flatpak"* / *"if we have a check for updates option, how would one go about letting the thing update? either way that is part of that task to discover"* | **shipped 2026-08-14 — manual GitHub archives** | Item 14 completed the requested discovery and the owner chose GitHub Release archives rather than an in-app updater or mandatory Flatpak channel; item 15 then published `v0.1.0` with Linux x86-64, Windows x86-64 and universal macOS archives plus verified SHA-256 sums. Baz stays offline and asks for no network permission. Discovery, archive download, checksum verification, quitting, replacement, relaunch and rollback remain explicit listener actions; config, indexed library data and playlists live outside the replaced application files. Automatic checking/installing, signing identities, installers, stores and a self-updater are not unfinished implementation under this policy: they are a future policy reversal requiring explicit owner approval. |
| *"on Windows, the user could see a visible command line window opened alongside the app"* | **shipped** 2026-08-13 | A normal Windows launch creates only the baz GUI window. `crates/baz/src/main.rs` links packaged/release Windows builds for the GUI subsystem via the crate-root `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`, while debug/developer builds keep the console for stderr diagnostics. User-facing failures were never console-only: they flow through the canonical health/event surface, so no release file or Windows logging sink was added (decided). The final acceptance launch of the actual packaged `.exe` from Explorer and the Start menu is the owner's. |
| *"when playing a playlist or album the recent shows a pip which reflows text... just show the pip at the right of the little row, and ensure it just makes the text ellipsis when it is eating into the space. we don't want reflowing text"* | **shipped** | Every expanded `RECENT` row reserves the same six-pixel lamp slot at the far trailing edge. Title and metadata keep a fixed shared 146 px one-line boundary and are shortened with a measured end ellipsis in the bundled Medium/Regular face; changing the sounding album or playlist therefore changes ink and the existing row card, never the sleeve, text origin, metadata origin, 64 px row pitch or neighbours. Collapsed rows retain their compact sounding card. |
| *"the Playlists page does not have the rail on the right and seems to be different from the Library? it should not be significantly different"* | **shipped** 2026-08-13 | The saved-playlist root and Library now call one collection scaffold, which owns the right-aligned rail-under-body stack, edge scrollbar relationship and lane geometry. Playlists keeps its legitimate collage projection and ordering, but shares the Library grid/density, virtualization, selection/hover grammar and `Spine` rail anatomy. Its rail says `A–Z` only for alphabetical order; `Date created` and `Played` project the ordered files into the same elapsed buckets the Library uses, including inert gaps. Missing creation timestamps read `Not recorded`; no session play reads `Never played`, so an edit cannot impersonate listening. |
| *"the component for unsaved playlists does not look the same as the saved playlists... we don't want too many similar components as it's just tech debt"* | **shipped** 2026-08-12 | Saved and unsaved lists now enter one parameterized `views::playlist_page` component. It alone owns their collage, sleeve size, breakpoint, fixed-aside/table or stacked document, identity hierarchy, `TRACKS` block, empty state, scroller and fixed-pitch row presentation. The durable state supplies Play/Rename/Delete and file counts; the transient state supplies Save/readout, live cursor/remaining time and provenance through the same capability slots. Unsaved rows now carry the saved page's artwork and Album context rather than private record headings. Same-viewport before/after frames and the drift inventory are in `docs/design/impl/one-playlist-page/`, and source guards reject a second private page/sleeve/breakpoint/scroll composition. |
| *"ideally we could have a back and forward functionality via back and forward nav arrows in the top left similar to Spotify"* | **shipped 2026-08-13** | The app bar now carries always-present Back/Forward chevrons beside the mark. They walk an in-session history of `Place` identities with ordinary browser branching: a new visit after Back clears Forward, revisiting the current place adds no entry, and unavailable historical subjects resolve through the existing safe fallback. Disabled arrows retain their fixed boxes and dim honestly. Alt+Left/Right accelerates the same controls; track Previous/Next remains separate. Search is an overlay on the unchanged place, so opening, clearing or dismissing it creates no history entry. Esc retains its established peel-then-Library behavior; all ordinary place doors, breadcrumbs and resident destinations record a visit. |
| *"when folders are offline we show a message in two places. the little status log is where I'd prefer is the source of truth"* / *"if there is an important status i.e. red status, can we make it pulse a bit and be noticeable?"* / *"for any event we come up with, can we have the concept of 'retry' ... either that or ... retries occasionally ... maybe exponential backoff"* / *"our status indicator should just go up into the top bar as a notification bell"* | **shipped 2026-08-13** | The fixed app-bar bell beside Settings is the one operational-health door in every place; the former bottom-bar dot is gone. Its panel is anchored below the bar and contains the bounded canonical event history. Opening/closing acknowledges transient attention without animating a standing condition. Warning/error summaries expose a safe manual incremental Retry, and the existing five-minute refresh remains bounded automatic retry for unavailable roots. A fresh successful scan replaces the current scan state rather than creating a second status surface. |
| *"can we add a playlists page which has all our playlists with a-z and date created ordering then the playlist viewer can show the playlist page as the root of the breadcrumbs"* | **shipped** | `Place::Playlists`, resident in the lane, draws every saved list as its generated collage on the shared density grid. Its Library-style arrangement strip offers `A–Z` by default, newest-first `Date created`, and most-recent-first `Played`; unknown or never-played lists follow the dated rows. The viewer strip is `Playlists › Name`, and the first segment opens the root |
| *"please remove the 'Play all' button at the top of the library"* | **shipped** | ADR-0040. The control **and the action**: `Message::PlayAll` and `App::play_all` went with the button, because a message no control sends is the visible-control rule failing in the direction nobody checks for. Home's `All songs` tile is untouched — different scope (the collection, not the wall as arranged) |
| *"please put the display options at the top bar"* / *"we should have replaced the top window chrome with an app bar which has this + settings + the window controls, the same on all screens"* / *"I am wanting it to function as the window chrome mixed with controls similar to stuff like spotify"* | **shipped.** ADR-0040. The app bar is resident in all eight places: application mark, search, honest drag region, display options, Settings and minimise/maximise/close. It owns the borderless window by default, drags it, maximises on a double press, right-presses to the desktop's own window menu and retains ordinary eight-way edge/corner resize. Display marks are present only where works hang but their slot remains stable. |
| *"we need some sort of min height as well"* | **shipped** | there was none — `app.rs` passed `min_size` a height of literally `0.0`, so the window could be dragged shut to fixed furniture with no collection between it. `theme::WINDOW_FLOOR_H` is **derived, not chosen**: the resident app bar, one-line arrangement strip, bottom bar/hairline/needle, plus **one row of the tightest wall**. One row is the whole claim; it is not a claim that one row is comfortable. |
| *"remember also we need mac os and windows compat eventually"* | **standing constraint, recorded** | not a task — a **revisit trigger** on three decisions taken for Linux and priced as Linux-only. **(1) Window buttons always on the right** (his *"I don't mind if we have the controls on the right hand side"*), which is wrong on macOS and right on Windows; the app-bar ADR carries the one-line reversal. **(2) The present-mode default**, chosen as `AutoNoVsync` precisely because wgpu guarantees it degrades to Fifo on any surface — the one form of this fix that cannot fail on a platform nobody has tested it on. **(3) The trash/undo layout test** is already `#![cfg(target_os = "linux")]`. CI builds all three platforms every push, and Windows has taught this project three lessons the hard way — drive-less fixture paths, UTF-16LE stored paths, FILETIME stamps — so the gate is real; what is untested is the *interface* on the other two, because nobody has run it there |
| *"resize is much better now but somehow it just doesn't seem... smooth?"* / *"that is really snappy"* / *"that also feels fast"* | **shipped** | **not baz's work at all** — a resize step costs 0.18 ms at 25 records and 0.44 ms at 400 against 16.7 ms of a frame, 8–9× headroom, no decode on the path (`docs/design/impl/resize-cost/`). The cost was *presentation*, and three launches on his own machine isolated it: `tiny-skia` snappy, wgpu + `mailbox` snappy, wgpu as shipped treacle. The default was **`Fifo`**, which blocks on the vertical blank while a drag outruns the monitor. baz now defaults to **`AutoNoVsync`** — not `Mailbox`, which is what actually fixed it, because iced asks wgpu for a named mode *literally* and it **panics** where the surface lacks it (his machine refused `Immediate` exactly that way mid-diagnosis). `ICED_PRESENT_MODE` set by hand still wins |
| *"the settings cog is padded in quite a bit and does not align with the rail"* | **shipped** | he is right and the number is **25 px**, at 1280 × 860 and again at 1920 × 1080 (`docs/design/impl/app-bar-gutter/`). The index rail's letters and the bottom bar's volume groove both end 41 px from the window's right edge — two surfaces, drawn by different code, already agreeing on law L1's line — while the gear's ink stopped at 66. **16 px was a phantom seam**: the window buttons are absent unless baz owns the chrome, and the row put a zero-width `Space` where they would go, which still collects a `GAP_LG` because a row's spacing falls between *children*. **8 px was the box not being the drawing**: every control is a 16 px sprite centred in a 32 px hit box, so hanging the container from `HANG` puts the box on the line and the ink inside it. The rule is now written over *the trailing control* rather than over the gear — it is the close button when baz owns the chrome — and both states are measured after the fix (42 and 43 against the rail's 41; the residual is each mark's own inner air). **Two lines were candidates and are not the answer**: the rail's *lane centre* is not drawn at all, and the gear's old ink centre sat within 2.5 px of it, so it looked like a rule and was a coincidence; the wall's scrollbar is deliberately outside the gutter and is the one thing L1 exempts. ADR-0040's amendment §1 |
| *"we probably want an icon for our app to show in the bar"* | **shipped, one thing for him to look at** | the mark that was already there — `packaging/icons/`'s hicolor ladder, the same file the desktop entry and the Flatpak install — decoded from the 32 px rung and drawn at 16 logical px, which is the `@2x` contract every sprite on the sheet already keeps. **Not on the glyph sheet**, and the two are different kinds of asset: `icon.rs` holds outlines rasterized to coverage and **inked by the room**, and the application icon is full-colour by construction; `packaging/README.md` already said they were unrelated, and a monochrome copy would be a second master, which `packaging/icons/README.md` forbids. **Instead of the word `baz`, not beside it** — the slot does not move (24 was `19.54 + slack` for the word and is `ICON_PX 16 + GAP_SM 8` for the mark), so this is the option that costs the composition nothing, where icon-and-word would have widened zone 1 to 48 to say the same thing twice. **What wants his eye**: the mark carries the lamp dot, and in the bar that accent is not playback truth. It is admitted as a stated exception — *the application's mark is the application's, not the room's ink* — and the reversal is a monochrome `Glyph::Baz` on the sheet if he would rather not spend it. ADR-0040's amendment §2 |
| *"until we have no window chrome, remove the window controls..."* | **shipped** | `app::owns_chrome()` remains the single answer for decorations and controls. Baz now owns chrome by default, so minimise/maximise/close are present once at the trailing edge; `BAZ_NATIVE_CHROME=1` restores the platform frame and hides Baz's duplicate controls. The app-bar slot is not held open in that diagnostic state. |
| *"I think ideally we could ensure our playlist view in the now playing and the playlist view/album view are the same thing. the only thing that changes in now playing is that we don't see file details etc. -- that is more like a album exploration type data"* | **done** 2026-08-10 | the **third** copy of one list, merged. The row was three literal copies (`album`, `playlist`, `queue`), the record head was two, and `views::queue` held four more copies of the reserved icon slot `impl/one-page-two-subjects/` had already shared — all now `views::page::track_row`, `list_head` and `icon_slot`, moving **no pixels**. What stayed different is what he named plus three facts about the subject: `DETAILS`, the next-track ring (a run has a cursor), the trailing slot sets, and the head — a page states a *name*, the run a *position*. The run column is **not** drawn through `page::view` and that is the honest limit: it is a virtualized column inside another surface's two-column layout, not a document in one scroll. `impl/one-list-drawn-once/` |
| *"also please make sure the layout of the now playing makes sense on wider screens"* | **done** 2026-08-10 | doc 12 step A4's run half, **and the second fault his first telling named**. At 2560 the measured gap was **1171 px**, not the ~700 the queue carried — that figure assumed a 1024 px cover, and the field is everything the work cannot use. Both edges were real: `RUN_MEASURE` was flat 440 at every size *and* the record column hung from the left gutter with the run pinned right. A4 alone closes 1171 → 919; the pair centring closes the rest. **36 px** at every size now, the work unchanged, and 1280 × 860 pixel-identical. `impl/one-list-drawn-once/` |
| The ambient Now playing — cover as the background, stylised VU over it, a feed of facts, all toggle-able; *"a spectrum analyzer or graphic thing with the bars going up and down"* | **shipped 2026-08-14; VU removed by owner** | The cover-derived field sits behind Cover / Jewel Case / None, with an independent Spectrum toggle and gated lock-free pre-volume sample tap. The remaining independent persisted Facts control now shows one fixed-height local line, cycling in a fixed inspectable order through exactly the sounding file's available ledger, collection, engine signal-path, encoding, provenance, release and track-position facts. Track changes reset it; press or 20 seconds advances it; the clock is absent elsewhere and missing data produces no placeholder. `PlayRecorded` refreshes the session ledger. It never emits streaks, rankings, congratulations or listening totals. The four-way VU experiment remains explicitly rejected by the owner on 2026-08-11. Evidence: `docs/design/impl/now-playing-facts/`. |
| *"adding controls that apply to all windows makes sense in the top bar"* | **shipped as law, with one recorded app-wide admission.** ADR-0040 asserts the closed tenancy: search is resident because it has one library-wide meaning in every place and overlays rather than navigates; a place's identity remains the place's, transport remains the bottom bar's, and arrangement keys remain the wall's. |
| *"I don't mind if we have the controls on the right hand side as long as we have a sensible consistent pattern"* | **shipped** | buttons right, always. A `chrome` module that read GNOME's `button-layout` and KDE's `kwinrc` and mirrored the bar was built and then deleted against this sentence. The *pattern* half is ADR-0040 §2's five zones and its one rule — **scope widens rightward** — which answers where a future control goes without an argument. Known cost: macOS puts its buttons left and will look foreign; one-line reversal recorded |
| *"the way they appear for the library is nice"* | **shipped, one thing to look at** | the marks moved into the bar unchanged — same sprites, same boxes, same resting ink with the current step lifted, same tooltips. But `Dense` is a 4 × 4 whose cells minify to 2.25 px at 1× and reads visibly softer than its three neighbours at the bar's real size. `docs/design/impl/app-bar/12-marks-4x-*.png` has all four magnified with a point filter. A larger sprite for that one mark is small work |
| *"I would really like it if we could get rid of the native window chrome"* | **shipped 2026-08-14** | Baz migrated to iced 0.14 and now disables platform decorations by default. A six-logical-pixel inside-edge hit band calls upstream `window::drag_resize` for N/S/E/W and all four corners, yields the interior to normal content, and stands down while maximised. The existing app bar retains window drag, double-click maximise, system-menu access and its right-side controls. `BAZ_NATIVE_CHROME=1` is the comparison/diagnostic escape hatch. Direction and boundary tests cover the frame, the full Baz suite is green, an isolated X11 release render showed only Baz's chrome, and a live Wayland launch reached the interactive window normally. ADR-0040 records the migration outcome. |
| Kiosk mode — full screen on a second monitor | **shipped 2026-08-14** | Design 12's honest single-window route is complete: move Baz to the intended display, open Now Playing, press F11. F11 toggles iced's current-monitor fullscreen from every place and remains available while search owns focus; Escape returns the same place to its prior window before peeling content. Baz creates no second window and offers no monitor picker because iced still exposes no monitor enumeration. The public keyboard table records the gesture. An unmanaged Xvfb release run accepted F11/Escape without a crash; pure mode tests pin the request, while final placement remains the compositor's native responsibility. |
| *"lets make sure we tackle the home page after critical usability stuff is done -- lets get the vibe playlists etc. based either on lightweight nlp/machine learning options etc. etc."* / *"it should be genuinely impressive and a unique selling point for a local music player app without internet connectivity"* / *"the ux of the current vibe is too poor to leave incomplete"* / *"we want the free text one. I honestly just want it - no need for gates"* | **done 2026-08-14 — free-text local composer shipped** | Home's full-width Make a mix composer accepts an ordinary-language musical description plus duration. Create drives one-time local indexing, cancellable progress and a silent editable preview with reorder/remove, explicit Play, Save and Another version. Baz bundles its reproducibly exported Apache-2.0 LAION CLAP audio/text towers and tokenizer; there is no download, cloud, account or remote model. The current track never changes the request implicitly. Retrieval is semantic, duration-aware and retains continuity plus album/artist diversity. Exact paths cannot duplicate within a list, and a persistent bounded recent-offer history prevents the same standout song repeatedly winning separate generated playlists while allowing fallback in a small library. The normal build includes the assets; `--no-default-features` remains an internal dependency-boundary check. |
| *"every album has a playlist implicitly... which playlist and which track"* | **shipped 2026-08-10** | Every run carries a typed `Origin` plus an engine-confirmed cursor. Albums are implicit fixed lists, saved playlists retain file identity, artist/All songs/draw/hand runs remain distinct, and the queue/session snapshot preserves origin and cursor across an interruption. Now Playing, the source door, queue summary, continuation band and edit/save rules all read that one run record rather than reconstructing attribution from the current file. ADR-0034. |
| *"I still see albums specifically appearing as if they are playing rather than the playlist... it only affects the little pip"* | **shipped 2026-08-10** | `lane::subject_of` maps a playlist origin to its playlist row and an album origin to its implicit-record row; `playing_subject` uses that same origin truth for the lamp. A named list therefore carries the sounding pip instead of whichever album happens to contain its current track, while artist/All songs/draw/hand runs invent no lane subject. Recency and playback indication now agree. |
| *"the seek bar at the bottom should have a toggle indicating for song or for whole playlist"* | **superseded by owner** 2026-08-10 | Tried and rejected in use. The bottom edge is one current-song seek line with track figures; the selector, cumulative list reading, queue-segment geometry and jump targeting were removed. |
| *"the album count in the bottom bar when in shuffle mode is weird... way too many albums shown"* | **shipped** | not the traversal's bag as first suspected: `continuation` folded only *adjacent* items sharing an album title, so a shuffled walk opened a new entry every time it returned to a record — `then 10 albums` for a run holding three |
| *"the artist page should have its own 'all songs' playlist I think"* | **shipped** 2026-08-10 | one wall-sized `All songs` collage above `RECORDS`, drawn by the exact component Home uses. It is scoped to the artist, orders dated releases by year then title with undated releases last, and preserves each selected edition's disc/track order. Playing it materializes the list as the current unsaved playlist; the bottom bar and Now playing source open that editable queue, where it can be saved |
| *"ideally the by artist page could have more info, maybe just the wikipedia for the band or something?"* | **shipped offline 2026-08-14** | The artist page now has a cached one-line local inventory (playing time, years, formats, genres and earliest added year), an `ALSO ON` guest-record section, local artist-image discovery, and artist-scoped All songs. A quiet `Look up` opens a Wikipedia search in the listener's browser through the desktop portal. That fulfills the encyclopaedia route without making Baz's first network request, adding a network permission or importing a hostile-input stack; an in-app network client remains a deliberate policy reversal, not an unfinished half. Design 15 tiers 1–2 and ADR-0037. |

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
- ~~**A run still carries a playlist's name, not its `Origin`.**~~ **Closed.**
  `QueueVm` now carries both its capability classification and an optional
  typed `Origin`; album, playlist, artist, All songs, draw and hand constructors
  state their identities. Session persistence encodes the same type. Ledger
  markers remain deliberately limited to origins `lane::subject_of` can credit,
  so attribution is never discarded merely because the type is richer.
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

- **Search folds case and nothing else, so `and` never finds `&`.**
  **Written up as `docs/WORK.md` item 80.** *(The owner, 2026-08-17: "can we make sure our search treats 'and' as & or and…
  because I searched for a song which used the ampersand which wasn't found by
  searching with the word.")*

  `Index::ranked` builds its needle with `query.to_lowercase()` and matches
  against titles folded the same way. `Day & Night` and `day and night` are
  therefore different strings, and so are every `Simon & Garfunkel`,
  `Earth, Wind & Fire` and `Above & Beyond` in a library that spells them the
  way the sleeve does.

  **Wanted:** one fold, applied to both sides, that treats `&` and `and` as
  the same token. Worth doing as a *token* rule rather than a substring
  replace — `Sand` must not become `S&`, and `R&B` must not become `Rand B`.
  The same fold is the natural home for two neighbours already known to bite:
  punctuation between words (`R.E.M.` against `REM`) and accents
  (`Beyoncé` against `Beyonce`), which the module docs already admit are
  folded by `to_lowercase` alone.

- **A named extreme can be the analyser's worst reading rather than the
  library's.** **Written up as `docs/WORK.md` item 79**, which the owner has
  named the most important thing outstanding. *(The owner, 2026-08-17: "the 'what baz heard' classified Day &
  Night by thundercat as the fastest… it really isn't.")*

  He is right, and the cause is measurable. The top of the tempo ranking on
  his own 5 076-track library:

  ```text
  194.9  Thundercat — Day & Night
  192.2  Walk On By
  191.5  Maurice Jarre — Lara's Theme (Swing Version)
  191.4  Orlando di Lasso — Matona mia cara
  190.6  Klára Körmendi — Childish Chatter, for piano
  ```

  A Renaissance madrigal and a solo piano miniature are not at 190 BPM. These
  are **octave errors** — beat trackers routinely report double or half the
  felt tempo, and the top of an argmax ranking is exactly where they collect.
  The bottom is worse in its own way: the three slowest read `0.0`, which is a
  detection failure rather than a tempo.

  **And the inconsistency is mine.** The same block already states the tempo
  *range* as p05–p95, precisely so one bad reading cannot describe a library —
  and then names the extremes with an argmin and an argmax, which is the one
  place a single bad reading gets a sentence to itself.

  **Wanted, in order of confidence:**

  1. **Name the extremes from a robust rank**, not the end. The record at the
     2nd percentile is still *the quiet end of your library* and cannot be one
     misdetection.
  2. **Fold tempo into a sane band** (roughly 60–180 BPM) by halving or
     doubling before anything ranks it, which is what beat-tracking libraries
     do and what would make the tempo axis itself better as well as the
     reading.
  3. **Drop `0.0` from consideration**, since it is an absence and not a
     measurement.

  This is the *check me* framing working exactly as designed
  (`docs/design/24-what-baz-heard.md` §2): the block named a record, its owner
  knew in one second it was wrong, and the analysis is what has to answer.

- **Right-clicking in a playlist resets the scroll position.**
  **`docs/WORK.md` item 81.** *(The owner, 2026-08-17.)* Not diagnosed. Suspicion is that opening the context menu
  rebuilds the list and the scrollable comes back at the top rather than at
  the offset it was holding — the same shape as the bugs the `scroll_offset`
  fields on other places exist to prevent. Reproduce first: right-click deep
  in a long playlist and watch whether the offset survives the menu opening,
  the menu closing, and an action taken from it.

- **Now Playing should fit its content, and the heart should belong to it.**
  **`docs/WORK.md` item 82.** *(The owner, 2026-08-17: "can you make the now-playing fit the content up to
  a max width and ensure the heart is snapped to the right hand side of that
  box so it doesn't appear to be off on its own.")*

  The block takes the full measure whatever is in it, so the favourite mark
  ends up against the window's edge with a stretch of nothing between it and
  the title it belongs to — it reads as a control of the *page* rather than of
  *this song*. **Wanted:** the block sized to its content up to a maximum, and
  the heart against the right edge of that box, so proximity says what it is
  attached to.

- **A library's own spread is measured, known, and never shown.** **Shipped
  2026-08-16** — both wanted forms, evidence in
  `docs/design/impl/what-baz-heard/`. *(The owner,
  2026-08-16: "so we're sort of making a determination based on my music pool?
  that there are certain signals that stand out way more than others? is there
  a way to make that available to users on a per-library basis?")*

  Yes to both halves, and the second is nearly free — everything needed is
  already in the analysis store.

  Measured on the owner's own 5 076 analysed tracks, the p05–p95 span of each
  drawn dimension, as a share of the widest one in **that** library:

  | dimension | span | in real units where they exist |
  |---|---|---|
  | Texture | 100% | |
  | Tempo | 96% | 91 → 167 BPM |
  | Dynamics | 65% | |
  | Energy | 53% | |
  | Brightness | 51% | |

  **The caveat is load-bearing and this entry is worthless without it.** Those
  are raw bliss features in different units — spectral flatness against
  loudness variance — so the column does *not* say texture is twice as useful
  as brightness. Comparing dimensions against each other needs a perceptual
  normalisation nobody here has. What it does support is comparing **one
  dimension across libraries**, and spotting the degenerate case: a collection
  of solo piano has no meaningful brightness axis, and a DJ set has no
  meaningful tempo axis.

  **And it exposes something about the rank axis worth stating.** A rank axis
  spreads whatever it is given across the full −2…+2 by construction — that is
  exactly why the line is always fillable, and why it fixed the *"dots aren't
  following my line"* failure. The cost is that **every dimension looks
  equally responsive whether or not the library varies on it at all.** Draw a
  tempo curve over a library with one tempo and the dots follow the line
  perfectly while the music does not change. Nothing on screen could tell you.

  **Both wanted forms are built**, per `docs/design/24-what-baz-heard.md`:

  1. **The degenerate axes are named.** Opened, a line whose dimension this
     collection barely varies in says so in the alert voice —
     *your music barely varies in brightness — this line will move the list
     very little.* The threshold is measured rather than judged:
     `cargo run -p baz-vibe --bin vibe-spread` puts the narrowest
     genuinely-varying axis of a real 5 076-track library at 0.392, and
     `FLAT_AXIS` is 0.12.
  2. **A "what Baz heard" reading** stands on the door, where the hour was
     paid for: the quietest, loudest, slowest and fastest records **by name**,
     the tempo range in BPM, and how many of them have never been played. Not
     a dashboard — a few lines, and every one of them either checkable against
     a record the listener knows or actionable.

  What is deliberately **not** there is the cross-dimension comparison this
  entry's own table warns about: *most varied in texture, least in
  brightness* is arithmetic over incommensurable units, and it stayed out.

  **Also not there, and this one was built first and then measured out.** A
  mood that a library cannot answer saying so on its own tile is the obvious
  sibling of the flat-axis flag, and it does not work: on a library holding
  no gregorian chant, no bagpipes and no gamelan, those requests draw pools
  of 175, 246 and 187 against the six real moods' 157–252, and `gregorian
  chant` returns the highest similarity of the lot. CLAP text-audio
  similarities are not comparable across prompts — the same wall
  `crates/baz-vibe/src/bin/word-probe.rs` hit three times — and *does this
  library contain X* is exactly a cross-prompt question. Reproduce with
  `vibe-spread STORE "gregorian chant" ...`.

  Related: `docs/design/23-the-three-dimensions.md` asks whether the semantic
  step earns its place at all. If it does not, this becomes the main evidence
  that the listening step did anything — which is the argument for having
  built it first.

- **Listening produces numbers, not tags — and the two are one short step
  apart.** *(The owner, 2026-08-16: "should we show tags on the tracks? does
  that exist in terms of our 'listen to my music' step? does it allow us to
  tag the music?")*

  What the listening step actually produces, per track, is two things and
  neither is a tag: bliss' conventional measurements (tempo in BPM, loudness,
  how much the loudness moves, spectral centroid, rolloff, zero crossings,
  flatness) and a 512-dimension CLAP embedding — a point in a space shared
  with text. Nothing anywhere is a word.

  **The first derived tags shipped on 2026-08-16 and are not stored.** A
  smart playlist's rows read `loud · fast · swinging`, computed live from the
  measurements against the collection's own range. They exist to prove the
  feature works rather than to describe the library, and they vanish with the
  page.

  Two things could follow, and they are very different in cost and risk:

  1. **Zero-shot tags from the embedding.** Score each track against a fixed
     vocabulary of text labels and keep what clears a threshold — the same
     tower, no new model, no network. There is evidence it would work for
     *instruments*: `docs/design/impl/vibe-eligibility/` measured *piano*,
     *synthesizers* and *strings* concentrating the matching genre 3.5–4.1×.
     There is evidence it would **not** work for moods: that sweep is exactly
     why six mood words were cut from the vocabulary, and a tag that says
     *dreamy* on the strength of a number that near zero would be the
     interface inventing a fact about somebody's record.
  2. **Writing tags into the files.** A different feature entirely — item 67,
     tag editing — and the risky one: it rewrites the listener's own files.
     Derived tags must not be written into files silently under any
     circumstances, and probably should never be written without being marked
     as machine-guessed.

  **Wanted first, and cheaply:** the derived reading beside a track outside a
  smart playlist — on the record page, in search, in the queue — so the
  analysis is worth something to somebody who never composes. That needs the
  measurements to be reachable outside `vibe::State`, which today they are
  not.

- **A song whose drive is not mounted looks exactly like one that plays.**
  **Shipped 2026-08-17.** Every track surface — a record's page, a playlist,
  the queue, favourites, a draft — draws such a row dimmed and says
  *· drive not connected* beside its title. Dimmed **and** worded, because a
  reading that rested on telling two inks apart is the one thing nothing in
  this product may do; and beside the title rather than instead of it, because
  what the row *is* has not changed, only whether you can play it.

  **The knowing was already here**, which is the part worth recording. This
  entry said *"the hard part is not the badge, it is knowing"* and asked for a
  root-level answer on the rescan clock. `Shelf::unavailable` has been exactly
  that all along: cleared at the start of every pass, filled by the folders
  that pass could not walk, so it describes the latest attempt and clears
  itself when the drive comes back. The first attempt at this shipped a
  second probe on the same clock before noticing — `crate::reach` is what
  survived, and it is now only the join, a path-prefix test a row can afford
  per frame.

  *(The owner, 2026-08-16: "we need to show beside songs when they are not
  available due to the drive not being loaded or being removed.")*

  baz keeps a row for every track it has scanned, and it is right to: an
  unplugged disk or an unmounted share is a **temporary** absence, and the
  scanner's four positive-evidence gates exist precisely so that a missing
  root never prunes the index. But the library says nothing about it. Every
  row looks playable, search returns them, a playlist containing them looks
  whole, and the only way to find out is to press one and meet a decode
  failure in the health log — which is the wrong end of the interaction.

  It matters more here than in most players because the owner's own library
  lives on an SMB share reached through gvfs: *every* track is one unmount
  away from this state, and the same is true of anyone with an external drive.

  **Wanted:** a per-row reading — beside the row rather than instead of it,
  and carrying its meaning in more than a colour — that says this file is not
  reachable right now. Plus the honest consequences: an unavailable track is
  not queued by *Play album*, a playlist says how many of its songs are
  currently reachable, and the reading clears itself when the drive comes
  back.

  **The hard part is not the badge, it is knowing.** Answering it per row per
  frame means a `stat` per visible track, which is exactly the kind of
  per-frame filesystem work the wall must never do — and on a network mount a
  `stat` on a dead share can block for seconds. So this needs a *root-level*
  answer (is this root reachable, once, on a schedule) that rows read for
  free, with the five-minute rescan that already exists as the natural clock.
  Design that first; the mark is easy once the fact is cheap.

  Related, and deliberately separate: `Library::forget_paths` and the Settings
  prune flow already handle the **permanent** case, and must not be reached by
  this one — a share that is merely unmounted has not been deleted.

- **Nothing in baz can be reached by keyboard except the search well.**
  *(WORK.md item 78, opened 2026-08-15.)* There is no focus traversal at all:
  `text_input` and — since the composing page — `crate::contour` are the only
  widgets a key press can be routed to, and both take focus from a pointer.
  So every button, chip, tile and row in the product is pointer-only. The
  binding table is not the obstacle; `crate::keys` already reads iced's own
  capture report rather than tracking focus itself, which is the seam a real
  focus order would use. What is missing is the order, per place, and the ring
  that shows it. This is recorded here rather than inside item 72 because it
  was true of every place before that page existed.

- **How well the local model actually retrieves has never been blind-tested.**
  *(Note 16's acceptance item 10; plan 22 §0.1.)* The harness
  (`tools/vibe-eval/`), a consented 72-track corpus, four systems including a
  deterministic diversity-matched random control, and 36 anonymous candidate
  lists are all in the tree; the ratings are unfilled and no agent can fill
  them. Everything measured about this feature so far is *comparative* —
  which policy concentrates the right songs better than another, which words
  move a pool more than others — and none of it answers whether the model's
  idea of *warm analogue soul* is anybody's. Note 16 set its own consequence:
  if the semantic system does not beat the random control, the next work is
  engine quality rather than interface.

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

- ~~**The density cache still decodes one size for three steps.**~~ **Closed
  2026-08-14 — it had already been taken, and this entry had gone stale.** `02`
  §2.7 priced it (at `Dense` the LRU held 320² thumbnails for ~200 px tiles,
  2.5× the pixels needed) and this entry said the density-aware decode size
  *"stays deliberately untaken"*, on the argument that it would make the
  cache's contents depend on the setting. Both halves were answered without
  this line being updated: `app.rs` requests
  `density.art_max_px().min(art::THUMB_PX)`, so Dense asks for 200 and
  Spacious for 320; and the objection is handled by **retry rather than
  invalidation** — a decode that completes too small after the step loosened is
  re-queued for that one id (`ThumbJobs::retry`, which prepends rather than
  replacing), so a step change costs the covers that are actually short of
  pixels and not the cache.

  Kept as a closed entry rather than deleted, because *why it was closable* is
  the useful part: the reversal this line asked for was *"a measured
  decode-latency or memory problem on a real large library at Dense"*, and
  item 37 supplies the measurement it was waiting for — Dense is now the
  **most** expensive density for resident art (336 tiles at 200 px = 51 MiB at
  a 4K window) precisely because it hangs the most tiles, which is the opposite
  of the arithmetic this entry was written against.

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

  **Checked against the cause found for the rate-change flake below and ruled
  out** (2026-08-17): that one was two processes sharing a fixture path, and
  `history.rs` builds its ledger in a `tempfile::tempdir()` unique per run, so
  no second process can reach it. The suspects above stand unchanged.

- ~~**A rare flake in `a_rate_change_is_refused_by_the_bit_perfect_default`**
  (`crates/baz-core/tests/playback.rs`).~~ **Closed 2026-08-17 — reproduced,
  diagnosed, fixed, and the assertion is untouched.**

  The guess recorded here was the 16-sample sink and producer/consumer
  ordering. **Both were wrong**: `OfflineSink` is synchronous and drops
  overflow, so it cannot race anything. What was right was the other half of
  the guess — *a different error surfacing first under load* — and the load
  that mattered was not CPU but **a second process**.

  The fixtures live at fixed paths under `CARGO_TARGET_TMPDIR`, and the
  `OnceLock` that builds them guards one process only. Two runs of the binary
  at once — a workspace run beside a build agent, exactly the reported
  condition — put one process's writer and another's decoder on the same WAV.
  A WAV caught mid-write has a header and no `data` chunk, the prefetch thread
  fails to decode track 1, and `produce` propagates that through `join()??`
  **before** it ever reaches the rate-change check. The test then reports
  `wrong refusal: decode error: unsupported feature: wav: missing data chunk`,
  which looks nothing like a race and is one.

  Starting eight copies of the binary together **reproduced it 2 runs in 8** —
  the step that turned a year-old guess into a cause.

  The fix is a one-step publish: every fixture is written under a name the
  process owns and `rename`d into place, so a reader sees a whole file or the
  previous whole file, never half of one. Applied to all four WAV writers, the
  raw layout WAV, and both FLAC encoder branches (the ffmpeg one gained an
  explicit `-f flac`, since the scratch name no longer carries the extension
  it used to infer the muxer from — verified by hand against real ffmpeg).
  Afterwards: **24/24 concurrent single-test runs green**, and four concurrent
  copies of the full `playback` and `engine` suites, 496 test executions, all
  green.

  `tests/engine.rs` had the identical defect and had simply never fired; it
  got the same treatment, plus a wrap around the ReplayGain fixtures so their
  write-then-tag read-modify-write happens on the private name — atomic
  publish alone would still have let one process tag another's already-tagged
  file.

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

- ~~**A deleted *directory*'s tracks still linger in the index.**~~ **Closed
  2026-08-14 by the Settings preview/confirm prune.** Removal landed with
  **ADR-0010** (this entry
  said ADR-0011 for two months; that is the volume ADR) and deleting a *file*
  now clears its row on the next scan — but only under positive confirmation,
  and one of the four gates is "the file's parent directory is present". So
  `rm -rf ~/Music/Artist/Album` leaves eight rows behind, deliberately: from the
  filesystem's side a deleted folder and a mount point that is not mounted right
  now are the same `NotFound` for every path below, and wrongly wiping a present
  listener's library is not a bug worth trading a cosmetic stale row for.
  **The unavailable-root guarantee stands.** A completed root walk now
  nominates missing descendants separately; unavailable, cancelled and
  unreadable roots nominate nothing. Settings reveals every exact path before
  `Prune index`, which reuses the reversible tombstone mechanism.

  **What would settle it**, in preference order: ~~(1) a *user-initiated
  prune*~~ — **the mechanism shipped (ADR-0042)**: `Library::forget_paths`
  deletes exactly the rows a listener names, and keeps their first-seen in a
  tombstone so that being wrong — the share was only unmounted — costs a rescan
  and nothing else. That reversibility is the whole reason a listener-initiated
  forget would be mechanically reversible. **The owner explicitly rejected
  that control on 2026-08-10:** if someone wants a record removed, they delete
  or move its files out of the held library; baz should prune the index. Do not
  rebuild `Forget this record`. The wider *"these rows point at files I cannot
  find; remove them?"* surface is the shipped exact Settings preview. (2)
  remembered mount points, so "this directory is
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

  ~~**What remained was the rootless population.**~~ **Closed 2026-08-14:**
  Settings now offers `Review unheld paths`, exposes every exact row behind the
  existing count, and requires `Remove from index | Keep`. Confirmation uses
  the same transactional removal and first-seen tombstones as folder removal;
  files, playlists, history and the current run are untouched. A listener may
  still add the folder back instead, which adopts and refreshes the rows.

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
  - **And it makes `an_exclusive_sink_reopens_at_the_requested_rate` a
    load-flake** (`crates/baz-core/tests/playback.rs`), observed once on
    2026-08-17 during a full-workspace run and green on the next run and when
    run alone. The test opens the real card exclusively and asserts the rate it
    negotiated; another test binary holding the device is exactly the
    `DeviceBusy` case above, arriving as a wrong rate rather than a refusal.
    **Not the fixture race fixed the same day** — that one is a shared *file*
    and this is a shared *device*, and no rename can isolate a sound card. The
    fix is for the test to state which device it needs and skip when it cannot
    have it, the way the `HI_RATE` branch already skips a card with no such
    mode.

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
- ~~**Light theme variant**~~ — **closed.** Reading Room now ships beside
  Closing Time, with Stone and Plaster completing the four-room polarity
  system; Settings also accepts validated local JSON themes.
- ~~**No readout for the direct signal path.**~~ **Closed 2026-08-14.** The
  bottom bar remains intentionally quiet for direct shared mode, while Now
  Playing's optional local facts cycle reads the complete retained
  `PlayerState::signal_path`: source/output rates, direct vs resampled and
  shared vs exclusive. The Debug session log retains the detailed event too.
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
- **Settings that are not yet settable.** Playback, Library, Appearance, Vibe
  and Debug use one indexed scrolling form. The shared output device is now an
  honest next-launch picker. Still intentionally off-screen are exclusive-mode
  selection (`BAZ_OUTPUT` remains the expert route), boundary policy and any
  future enrichment toggles; none is implied by the shared-device control.
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
- ~~**Music folders cannot be reordered in the interface.**~~ **Closed
  2026-08-14.** Each Settings root row now carries explicit Up/Down controls in
  the existing word-control grammar. End moves are disabled, movement stands
  down during a scan or removal confirmation, and an adjacent swap immediately
  persists the ordered `music_dirs` list. It starts no work and touches no
  files or indexed rows; the next scan consumes the new order. A drag handle
  was unnecessary for a normally short, exact list with accessible steppers.
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
- ~~**A missing playlist entry cannot be repaired in place.**~~ **Shipped
  2026-08-17** — `docs/design/impl/locate-missing-entry/`. §3's surface, built
  to its own terms: `crate::repair` proposes and cannot write; the candidates
  are **filename** matches, not tag matches, because a missing entry's
  `#EXTINF` came from whatever wrote the playlist and a tag match's failure
  mode is a confident swap the listener cannot see; *"under a current root"*
  needed no code, since the index holds exactly what the scanner walked; and
  the order is shared path tail, so a remounted drive's true match leads
  without the ordering claiming to know anything about likelihood.

  The control is a magnifier in the slot a missing row leaves free — an entry
  whose file has gone cannot be favourited, so the heart's place is spent on
  the one act the row can offer. Its card is `crate::menu`'s float opened by a
  **left** press, which makes `Target::LocatePlaylistEntry` the only target
  outside the mirror layer; a test pins that rather than leaving it to be
  rediscovered. The write goes through `edit_open`, so a repair inherits the
  externally-edited re-read, the save and the undo step.

  Driven end to end on a real X server against a playlist with a deliberately
  dead path: one press on one candidate, `2 of 3 · 1 missing` becomes
  `3 tracks · 4:48`, and exactly one line of the file changed.
- ~~**The playlists folder is not shown in Settings → Library.**~~ **Closed
  2026-08-14.** The exact listener-owned directory now stands beside the music
  roots with `Open folder`. Linux uses the desktop portal with a correctly
  percent-encoded file URI; macOS and Windows use their native file managers.
  Failure enters the canonical health history, and the action never parses,
  moves or edits a playlist.
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
- ~~**The settings steppers' marks do not ride the transport's hover tween.**~~
  **Closed 2026-08-17** — `docs/design/impl/stepper-ink/`. The price this
  entry quoted was the right one and slightly understated: six
  `motion::Control` identities rather than two, because there are three
  stepper rows, plus the two-line `mouse_area` every other icon button carries
  and `Ink` threaded into `settings::view`. `theme::glyph_ink` — the whole
  ladder with its 90 ms tween — already existed; the steppers were the last
  icon button in the product still reading `glyph_opacity`, which is that
  ladder with the pointer's part left out. **Six identities and not one**: a
  shared identity would light all six marks whenever the pointer found any of
  them. Measured on a real X server — mark peak 137 at rest, 232 under the
  pointer, and the `−` beside the hovered `+` unmoved at 137.
- **The strip's split regime never hosts a third line** (doc 10 §8, stated
  so a future proposal meets the reason): a tenant that does not fit the L9
  budget re-homes by subject (doc 07's L8) or displaces an argued
  incumbent — the budget law's answer is re-homing, not accretion. The
  Marquee lens's switcher form (ADR-0017 step 18) is likewise left to its
  own design: `WALL · MARQUEE` will be a state row in the state row's
  vocabulary, and nothing shipped pre-empts its keys.
- **No keyboard route out of the search field.** Transport keys are bound
  (`crates/baz/src/keys.rs`), but `text_input` captures every key press while
  focused except Tab and the vertical arrows, so while the search well has
  focus almost nothing is a shortcut — the field takes the key, and the focus
  rule in `crate::keys` honours that rather than second-guessing it.
  <kbd>F11</kbd> and <kbd>Esc</kbd> are the two exceptions, each for a stated
  reason. Everything else waits on a focus order per place, which is the same
  missing capability as the accessibility gap above.
  - ~~**<kbd>Esc</kbd> takes two presses to peel a query you are still
    typing.**~~ **Fixed 2026-08-17** —
    `docs/design/impl/escape-in-the-well/`. The claim that this needed "a
    focus-aware shell (or a toolkit that reports focus synchronously)" was
    **written against iced 0.13 and never revisited**: baz is on 0.14 and
    `keys.rs` has read `iced::event::Status` into its own `Focus` for some
    time. `Captured` *is* that synchronous report, and it says the one thing
    needed — the caret is in the well. A captured <kbd>Esc</kbd> now binds to
    `Message::EscapeInField`, which clears the query on the same press iced is
    blurring on, and peels nothing else: a press the field has already spent
    must not also take a layer out from under it, so an empty well gets the
    blur alone rather than sending you home. Proven on a real X server, not
    only in unit tests — one press, query gone, wall back, still on Library.
  - **<kbd>Ctrl</kbd>+<kbd>B</kbd> still asks for nothing while the caret is
    in the well**, and neither does any other chord. **This is the focus rule
    working**, and `a_focused_text_field_swallows_every_binding` pins it.
    Letting modified keys through is a real design change with a real risk of
    taking <kbd>Ctrl</kbd>+<kbd>A</kbd>/<kbd>C</kbd>/<kbd>V</kbd>/<kbd>X</kbd>
    off the field, so it is a decision to make rather than a defect to fix.
    Both behaviours are captured in `docs/design/impl/search-in-lane/05`
    and `06`.
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

**Shipped 2026-08-14** — ADR-0040. Baz now owns the window by default. The app
bar is resident in every place; it moves, maximises, minimises and closes the
window, opens the native system menu, and a six-logical-pixel inside-edge band
provides all eight compositor resize directions through iced 0.14. Maximised
windows yield that band to content. `BAZ_NATIVE_CHROME=1` restores platform
decorations and hides Baz's window buttons for comparison and diagnostics.

The following is the historical investigation that made the migration an
informed decision:

**Why the flip was a decision, re-verified against the then-pinned sources
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

- ~~**No application icon.**~~ — **shipped on Linux and in the window.**
  `packaging/icons/` holds the SVG master and the hicolor PNG ladder, the
  desktop entry names it, the Flatpak and the Linux tarball install it, and
  `app.rs` decodes the 64 px PNG into `window::Settings::icon` (Windows and
  X11; winit supports per-window icons nowhere else).

- **The application's own mark is not on the macOS app, and the Windows
  embedding has never been seen on Windows.** *(The owner, 2026-08-15: "we
  need to use our app icon on the executable. e.g. when opening the mac app it
  should use that icon and be seen in their apps with it, and in the tray. the
  same for windows.")*

  Three different states, and they should not be described as one:

  1. ~~**macOS is the real gap and it is not an icon problem.**~~ **Built
     2026-08-15, unverified on a Mac.** baz had no `.app` bundle at all, so
     there was nothing for Finder, the Dock, Launchpad or Spotlight to draw a
     mark *on* — a bare Mach-O cannot carry one, and double-clicking it opened
     Terminal. `packaging/macos/` now holds `Info.plist.in` and a `bundle.sh`
     that assembles `baz.app` around the universal binary, the release
     workflow ships the bundle instead of the loose executable, and
     `packaging/icons/render.sh` renders and validates a committed `.icns`.
     The models travel in `Contents/Resources`, and `baz-vibe` learned to look
     there — that path is a unit test rather than a comment, because ancestor
     walking never reaches a sibling directory. CI assembles a bundle on Linux
     on every push and runs its structural checks. **What remains is looking
     at it on a Mac**, which is the acceptance below.
  2. **Windows is written but unverified.** `crates/baz/build.rs` embeds
     `logo-transparent-circle-red.ico` through `winres`, and the release
     workflow builds Windows natively so the resource compiler runs. Nobody
     has opened the resulting `.exe` on a Windows machine and looked at
     Explorer, the Start menu, the taskbar and Alt-Tab. Until somebody has,
     this is a claim rather than a fact — and `winres` fails *silently enough*
     on a cross-build that "it compiles" is not evidence.
  3. **"In the tray" has no implementation on any platform**, so it is a
     feature request rather than an icon defect. baz has no tray or
     status-item integration: not on Linux (`StatusNotifierItem`), not on
     Windows (`Shell_NotifyIcon`), not on macOS (`NSStatusItem`). What the
     window and taskbar draw today is the window icon above. If a tray is
     wanted it needs its own decision — what it shows, what its menu does,
     whether closing to tray is a behaviour baz has — and it is a larger item
     than the mark it would wear.

  **Acceptance:** the mark visible on the `.app` in Finder, the Dock and
  Launchpad on a real Mac; visible on `baz.exe` in Explorer, the Start menu
  and the taskbar on a real Windows machine; a screenshot of each in
  `docs/design/impl/`. Neither can be verified from the Linux development
  machine, so both are owner-verified steps — the same boundary as the
  packaged `.exe` launch in `WORK.md` item 12.

  **And one thing the bundle makes newly visible:** an unsigned app that a
  browser downloaded is refused by Gatekeeper with *"baz is damaged and can't
  be opened"*, which is false and is the most confusing message an independent
  macOS application can give. `docs/INSTALL.md` documents both ways through
  it. Removing it needs a paid Apple Developer account, a Developer ID
  certificate in CI, and `codesign` → `notarytool` → `stapler` in the release
  — an external boundary, and the owner's to cross.
  `packaging/macos/README.md` says exactly where those two steps would go.
- **`OpenUri` is not implemented**, so MPRIS's `SupportedUriSchemes` and
  `SupportedMimeTypes` are empty and the desktop entry registers no
  `MimeType=`. baz plays what it scanned; "open this file with baz" is a real
  feature (queue-a-path, plus a `%U`-aware `Exec=`) rather than a property, and
  advertising schemes we would refuse is the kind of small lie the honesty rule
  rules out.
- ~~**MPRIS Previous is a documented no-op.**~~ **Closed.** The front end sends
  `Command::Previous`; MPRIS serves the request and publishes
  `CanGoPrevious` from `PlayerState::previous_enabled()`. It restarts past the
  engine's three-second threshold, otherwise steps back, and restarts at the
  head, matching the resident transport and media key.
- **No MPRIS `TrackList` or `Playlists` interface** (`HasTrackList` is
  `false`), and no `LoopStatus`/`Shuffle` — baz has neither loop nor shuffle
  yet, so they are absent rather than present-and-fixed.
- **`Rate` is read-only `1.0`**, with `MinimumRate` and `MaximumRate` pinned
  to it. baz plays at the source rate (ADR-0009) and has no rate control; a
  writable property that silently discarded writes would be worse than an
  error, so the property is honest rather than present.

  ~~**and `Volume`**~~ — **wrong since ADR-0011, corrected 2026-08-17.** This
  entry said baz had "no volume control at all". It has had a fader, a mute,
  a wheel and a persisted level for months, and MPRIS `Volume` has been read
  *and* write for as long: `set_volume` maps the level back through the same
  taper the fader uses, unmutes when a client asks for sound while muted, and
  refuses only when there is no engine to set a level on. Nothing was fixed
  here; a stale line was, which is worth doing on its own account — a backlog
  that misdescribes the product costs more than the gap it claims.
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
