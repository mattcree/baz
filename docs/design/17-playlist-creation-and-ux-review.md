# Playlist creation and the 2026-08-14 UX review

> **Status: Phase E1 shipped as WORK item 31; items 32–36 continue below.** This
> review turns the owner's live observations into one creation model and four
> follow-on phases. It amends the placement decision in
> [design 16](16-local-vibe-playlists.md): the local model and preview contract
> stand, but Home no longer owns playlist generation.

## The finding

Vibe is not a destination. It is one way to make a playlist.

The current interface divides one listener intention according to how Baz
implements it:

- a manual playlist begins in the summoned playlist panel's `New playlist`
  row;
- a generated playlist begins in a full composer embedded in Home;
- the Playlists root is where the resulting files are browsed, but is not the
  visible starting point for either creation route;
- `Save playlist` on a Vibe preview immediately mints `Vibe playlist`, then
  `Vibe playlist 2`, rather than letting the listener see or edit its name.

That is why the feature feels misplaced even though each individual state is
legible. Home answers “what can I pick up?”; Playlists answers “what lists do I
own?”; Vibe answers “how should this new list be composed?” The third question
belongs under the second, not between Home's All songs and Recently added.

The repair is a single **New playlist** flow with two sibling starts:

```text
NEW PLAYLIST

  Manual                              Vibe
  Start with an empty list            Describe a journey through your music
  Name it, then add tracks             Baz composes a local editable draft
```

Both routes converge on the same ordinary playlist draft and the same review,
name and save boundary. There is still only one playlist species and one file
format. “Manual” and “Vibe” describe how the initial ordered entries arrive;
they are not permanent types printed on saved playlists.

## Information architecture

### The canonical door

The Playlists root gets the visible **New playlist** door. It remains available
in the empty state, where it is the primary act, and in the populated state,
where it stands in the collection header rather than masquerading as a saved
tile. Entering it is a place transition and therefore participates in normal
Back/Forward history.

The panel's existing ghost row remains useful while adding a track: it is the
short route to “put this held thing in a new manual list.” Outside pick mode it
should lead to the same New playlist flow rather than maintain a second inline
creation grammar.

Home may retain a compact **Make a vibe playlist** shortcut because discovery
matters, but it opens the Vibe branch of New playlist. Home does not retain the
composer, analysis consent, progress or preview. Leaving the creation place
does not cancel work; returning restores the draft.

### The flow

The flow is shallow and resumable, not a modal questionnaire:

1. **Choose** — Manual or Vibe. This step is skipped when a contextual shortcut
   already made the choice explicit.
2. **Compose** — Manual asks for a name and opens an empty draft. Vibe asks for
   a description and duration, with optional journey shaping below it.
3. **Review** — one shared editable track list. Reorder and remove work exactly
   as they do in today's Vibe preview and playlist page.
4. **Name and save** — the editable name is visible before the file is written.
   Save writes an ordinary `.m3u8`; Play remains a separate act.

Back moves between these steps without discarding entered text or a generated
draft. Cancel/close with an untouched empty draft is silent. Leaving after a
manual edit or after changing a generated draft asks before discarding it.
Analysis cancellation remains separate: it stops scheduling but preserves the
request and completed disposable analysis.

### Manual is not a dead-end empty file

Today's panel creates an empty file as soon as a name is submitted. The
creation flow should instead hold an unsaved draft until Save. This makes
Manual and Vibe genuinely converge and avoids filesystem litter from a person
who merely explored the flow. The draft offers the established global search
dropover in an explicit `Add to playlist` context; the app-bar well keeps its
single library-search meaning.

## The Vibe branch

### Progressive guidance, not a model console

Free text remains the primary expression because it is the feature's unique
strength. Curves should not replace it or require a listener to translate a
musical idea into DSP vocabulary before Baz will help. The branch starts with:

```text
DESCRIBE THE VIBE
┌──────────────────────────────────────────────────────────────┐
│ dreamy shoegaze for a rainy evening                         │
└──────────────────────────────────────────────────────────────┘

Try: Late-night focus · Warm Sunday morning · Restless then calm

LENGTH  60 minutes                         [ Shape the journey ]
                                             [ Create draft ]
```

The examples are pressable starting points which populate the ordinary field;
they are not hidden presets sent instead of the text the listener sees. Their
small fixed set must cover a steady state, an energy build and a build-and-
settle journey so the product teaches that a prompt may move over time.

### A few curves, with textual equivalents

`Shape the journey` reveals two controls, initially set from visible defaults:

- **Energy over time** — Steady, Build, Peak & settle, Cool down, or Custom.
- **Similarity over time** — Stay close, Travel, or Custom. “Travel” allows the
  semantic target to move more strongly between named waypoints; it does not
  invent a second opaque randomness setting.

The first implementation should ship the energy shapes and 1–3 semantic
waypoints before a freehand curve editor. Every shape has a textual reading,
keyboard editing and discrete values. A smooth drawing may visualize the
interpolation, but it is not the only control or the only explanation.

Custom energy exposes a small number of points rather than a DAW envelope.
Endpoints remain, an optional middle point can be added, Up/Down changes the
level and modified Left/Right changes its position. The description beside it
speaks the result: `Starts low · peaks near the middle · settles gently`.

The request passed to generation becomes the structured, model-independent
`MixRequest` already specified in design 16: duration, semantic waypoints,
energy points, optional avoid phrase, and variation seed. The prompt proposes
that structure; the visible controls become authoritative after an edit.

### Naming from the prompt

The prompt is the best default starting name because it is the listener's own
description of the artifact. It must still be a suggestion, never an invisible
filename side effect.

At Review, the name field is prefilled deterministically from the first
semantic phrase:

- use the first waypoint/phrase, not the whole multi-stage instruction;
- remove structural lead-ins such as `start`, `then` and `finish` only when the
  parser has identified them as structure;
- retain the listener's casing and words;
- replace filename-forbidden separators visibly and stop at the last word
  boundary before 48 characters;
- use `Vibe playlist` only when no usable phrase remains;
- show collision suffixes in the editable field before Save.

For example, `Start sparse and nocturnal, build into restless electronic
music, then finish warm and expansive` suggests `Sparse and nocturnal`.
`dreamy shoegaze for a rainy evening` stays exactly that. Baz does not have a
generative language model and should not imply that CLAP “wrote” a title.

### Review remains the product boundary

Create produces a silent draft. It does not start playback or write a file.
Another version keeps prompt, curves, duration and name unless the name still
equals the prior automatic suggestion, in which case the recomputed suggestion
may follow a changed first waypoint. A name the listener edited is never
overwritten.

The draft shows why it may be short or approximate, but model confidence does
not become a score beside every track. Reorder/remove, Play and Save retain the
existing distinctions. A saved Vibe list is thereafter completely ordinary.

## Functional additions

### Favourites is a built-in playlist

Favourites belongs in the Playlists collection as a pinned built-in list, not
as a hidden filter or a file Baz pretends the listener created. It cannot be
renamed, moved to trash or overwritten by importing `Favourites.m3u8`.

Every shared track-row anatomy gains one reserved heart action. Outline means
not present; filled accent means present. The tooltip and accessible name say
`Add to Favourites` or `Remove from Favourites`. The hit target exists in album,
saved/unsaved playlist, queue, search and Vibe-review rows without changing row
height. Toggling it never starts playback and never edits the list currently
being viewed.

Membership is durable library data, not listening history. The implementation
brief must settle identity across file moves and missing/remounted roots before
choosing a schema; a path-only side file is not accepted merely because it is
easy. Removing a favourite is recoverable by pressing the heart again, but no
music file is ever changed.

### Repeat one is a player property

The requested behavior is a binary **Repeat current track** property. At a
natural track end it seeks/restarts the same queue entry. Explicit Next,
Previous or selecting another track still acts immediately; the newly current
track is then the one repeated. Shuffle may remain on: repeat wins only at a
natural end, while an explicit Next resumes the shuffled traversal.

Repeat stands beside Shuffle in the bottom bar's property group, has an
unambiguous one-track loop glyph, a lit state, and state-specific tooltip copy.
Like Shuffle it persists because it changes what the first run after relaunch
will do. It does not mutate or duplicate the run.

## Interaction and accessibility repairs

These reports share one underlying problem: controls exist, but their state or
scope is too quiet.

- **Album-cover actions:** keep the full-row hover target, but give the hovered
  option a distinct field/rule in addition to ink change. Keyboard Left/Right
  selects Play, Queue and Open; Enter confirms. The selected option must remain
  legible over both dark and light artwork.
- **Albums in search:** Left/Right selects `Play | Open`, matching tracks'
  existing action axis. The choice clamps and remains visible while keyboard
  selected.
- **Next/End:** on a live run, label the actions `Add next` and `Add to end`.
  They edit the run, not a saved playlist. With no run, retain `Enqueue`; on a
  saved playlist page retain `Add to playlist`.
- **Search row clipping:** make the two-line result row's height a derivation of
  the two actual line boxes plus vertical padding. Do not fix one font/theme by
  adding an unexplained pixel. Sweep long descenders, 100–200% scale, every
  bundled theme and the narrow app-bar dropover.
- **Artist/album links on playlist rows:** their pointer interaction must set
  the link cursor, preserve row selection outside their bounds and use the same
  focus/activation treatment as the equivalent album-page links.
- **Playlist collection header:** compare the rendered Playlists and Library
  strips at identical window coordinates. They already share scaffold height,
  but Playlists substitutes order words/count without the Library's immediately
  readable section pattern. Converge the title/order/count hierarchy through a
  shared header composition rather than offsets local to Playlists.

## Bottom bar and resident chrome

### One right-anchored control cluster

The current bottom bar deliberately centres transport in an independent middle
column and floats elapsed/total stamps at the end of the left metadata zone.
That arithmetic is stable, but it explains the reported visual unease: the two
times look like unrelated facts sitting in open space, while transport and
properties form two separate control islands.

The new composition is:

```text
[cover  title / artist / what follows]   [2:31 / 5:04] [previous play next]
                                         [shuffle repeat] [mute — volume]
```

At wide measures the bracketed controls occupy one row and share a trailing
axis. The diagram wraps only to name groups; the implementation stays a single
right-justified cluster. The signal-path note sits immediately before volume
when present and yields before a control does.

Elapsed and total become one compact `elapsed / total` readout adjacent to the
transport. The progress needle still spans the bar and its hover tip still
shows the aimed time. Grouping the figures states their relationship and
attaching them to transport makes them look controlled rather than abandoned.
The readout keeps tabular figures and a fixed maximum measure, but it no longer
reserves two separated boxes inside track metadata.

At narrow widths, passive facts yield in this order: signal-path prose,
continuation prose, then the time readout. Previous/Play/Next, Repeat/Shuffle,
Mute and an operable volume target remain. No control drops below the bar or
changes order.

### Stable left-axis icons

The application mark and every expanded/collapsed lane destination must share
one immutable glyph centre. Collapsing the lane removes label width to the
right of that centre; it must not recenter the glyphs in the narrower lane.
The app mark's optical bounds, not merely its image box, are measured against
that axis. This repairs both the mark mismatch and the sidebar shift with one
geometry rule.

Back/Forward need browser arrows with a short shaft and head, not bare
chevrons that can read as disclosure/Open and not skip-to-track marks with a
bar. Disabled state retains the box and dims the same drawing. Their position
and Alt+Left/Right behavior do not change.

## Now Playing: motion with a reason

“More exciting and dynamic” should extend the existing optional visual system,
not make the page animate by default or import nostalgic decoration. Keep
Cover / Jewel case / None as the foreground choice and let a visualizer remain
independent.

The visualizer choice becomes a small family:

- **Spectrum** — the existing frequency bars, refined rather than replaced;
- **Waveform** — a rolling amplitude trace with a bounded history;
- **Oscilloscope** — stereo vectors/phase, useful as well as lively;
- **Spectrogram** — frequency over recent time, with a fixed bounded texture;
- **Particles/constellation** — only if driven directly by bands and still
  legible with reduced motion; it is the least “classic” and ships last.

VU meters remain excluded by the owner's prior decision. Fake vinyl, tonearms,
wood, wear and unrelated ambient particles remain excluded by the product's
skeuomorphism rule. Each mode is off by default, pauses its clock when hidden,
shares the existing audio sample feed, respects reduced motion and is measured
for idle frames, CPU/GPU use and allocation bounds. Fullscreen should amplify
the chosen mode through scale and persistence, not add controls or a second
composition.

## Ordered implementation phases

### Phase E1 — one New playlist flow (WORK 31)

Add the canonical door and resumable draft state; converge Manual and Vibe;
move the composer/progress/preview out of Home; add prompt-derived editable
naming; then add the guided examples, energy shapes and semantic waypoints.
Keep every shipped model, consent, analysis and ordinary-file guarantee.

### Phase E2 — playlist and player functions (WORK 32–33)

Build Favourites with durable identity/migration tests, then Repeat current
track with natural-end versus explicit-skip engine tests. These alter what Baz
can do and therefore precede cosmetic repair.

### Phase E3 — interaction clarity (WORK 34)

Repair the Playlists header, album hover states, album search keyboard action,
run insertion labels, two-line search height, and row-link cursors/focus as one
pointer/keyboard/accessibility sweep.

### Phase E4 — bottom bar and chrome geometry (WORK 35)

Right-anchor all controls, group elapsed/total beside transport, establish the
immutable left icon axis and redraw browser navigation arrows. Verify at the
window floor, 1280, 1920 and 4K in expanded/collapsed and playback states.

### Phase E5 — richer Now Playing visuals (WORK 36)

Prototype the classic visualizers against the existing sample subscription,
measure them, ship the strongest bounded modes, and record any rejected mode
with evidence. This is last because it is expressive scope rather than a
missing core action.

## Acceptance through the complete journey

1. From an empty Playlists page, New playlist can create and save a Manual
   list without summoning a hidden panel.
2. From Playlists or Home's shortcut, Vibe reaches the same branch, preserves
   its request across navigation/analysis and returns a silent editable draft.
3. The suggested name is visible, editable, filename-valid and collision-safe
   before any file exists.
4. Manual and Vibe saves appear as indistinguishable ordinary playlist files;
   neither automatically plays.
5. Every shared track row toggles Favourites with pointer and keyboard without
   changing selection or playback.
6. Repeat restarts only natural ends; explicit navigation still navigates.
7. Search and cover action axes expose the same visible choice under pointer
   and keyboard, and no descender clips at supported scale/theme combinations.
8. The bottom bar retains all essential controls at the width floor and no
   dynamic readout moves their trailing axis.
9. Expanded/collapsed lane glyphs and the app mark retain one measured centre;
   Back/Forward cannot be mistaken for disclosure or track skip.
10. Every visualizer is optional, bounded, reduced-motion aware and silent in
    CPU/GPU scheduling when the Now Playing surface is hidden.
