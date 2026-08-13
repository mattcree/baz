# Local sonic vibe playlists

> **Status: local Vibe playlists shipped; semantic integration deferred,
> 2026-08-13.** The owner accepted the conventional local controls after using
> them. The reproducible LAION export and identity-blind ballot remain research
> tooling; semantic integration resumes only if it is chosen and demonstrated
> useful.

## Product bar

Baz should be able, with the network physically disconnected, to turn a
musical intention into a playlist that feels composed from the listener's own
collection. The intended surface eventually spans:

- approachable mood/quality choices and energy/tempo steering;
- “more like this” and positive/negative track anchors;
- free text such as “restless late-night electronic music, becoming warmer”;
- a shape across time—build, settle, peak, cool down or travel between tracks;
- explicit scope, analysis progress, cancellation and removal of analysis data;
- explanations grounded in real signals rather than invented certainty.

The result is always an ordinary editable playlist. Nothing auto-plays,
silently regenerates, uploads audio, creates an account or contacts a model.

## The listener contract: one request, one result

The feature is **Make a mix**, not an analyser and not a model console. Its
ordinary path has one required field and one decisive press:

1. describe where the music should go;
2. press **Create mix**;
3. receive a silent, editable preview.

For example:

> Start sparse and nocturnal, build into restless electronic music, then
> finish warm and expansive.

The listener never chooses a model, opens an index, runs analysis as a separate
job, supplies a baseline track, edits JSON, or handles an evaluation ballot.
Those are development and implementation concerns. A first request may need
one consent decision before it can continue, but it remains the same request:
the text, shape, scope and length survive consent, analysis, cancellation and a
restart.

### Home at rest

Make a mix is a full-width section after **All songs** and before **Recently
added**. It is useful without opening a second place and does not displace
Home's Continue question. At rest it shows the complete simple path rather than
a button which opens another unexplained panel:

```text
MAKE A MIX
Describe a journey through your own music
┌──────────────────────────────────────────────────────────────────────┐
│ Start sparse and nocturnal, build, then finish warm and expansive   │
└──────────────────────────────────────────────────────────────────────┘

JOURNEY   ▁▃▆█▅▂   Rise & fall ▾       LENGTH   60 minutes ▾
                                            [ Create mix ]
```

The example is placeholder copy, not a request Baz runs. The section carries
one quiet assurance beneath its heading: **On this device · your audio never
leaves Baz**. It does not lead with model names or the word “analysis”.

The first shipped shape is **Rise & fall**, because a mix normally moves. The
menu also offers Build, Cool down and Steady. Steady is available but is not
the conceptual default. **Custom** turns the little graph into two to four
keyboard- and pointer-adjustable energy points; adding or moving a point is
immediately visible in the curve but regenerates nothing until Create mix is
pressed.

Length is expressed in listening time, not an implementation track count: 30,
60, 90 and 120 minutes, plus Custom. Baz fills toward that duration using known
track lengths and states when missing durations made the result approximate.
The initial default is 60 minutes.

### A prompt becomes an editable journey

One prompt may describe a constant quality or movement. Baz recognizes a
small, documented set of structural cues—start/open/begin, then/build/become,
and finish/end/land—and separates only clauses those cues make explicit. It
does not claim general language understanding where there is none.

Before generation, an interpreted multi-part request appears as an editable
timeline:

```text
OPENING                 TURN                    LANDING
sparse · nocturnal  ──  restless · electronic  ──  warm · expansive
0%                      55%                         100%
```

Each phrase remains ordinary editable text. A listener can add a turn, remove
one, or drag its position. A request without structural cues remains one sound
target while the selected energy curve still gives the list movement. Baz does
not split prose speculatively, hide an interpretation, or average “start dark,
end bright” into one meaningless middle.

Two optional controls sit behind **Refine**, below the simple path:

- **Avoid** — a distinct negative target such as “sweet vocals” or “harsh
  guitars”; this is not left for an embedding model to guess from the word
  “not”. Explicit “without/avoid” clauses in the prompt are moved here visibly
  and remain editable.
- **Scope** — Whole library by default, or a visible artist, genre, selected
  albums, or current Library result set. The scope always names its track and
  analysed-track counts; a hidden candidate pool is forbidden.

The curve and semantic waypoints are complementary. The curve states the
energy contour. Waypoint text states how the musical character changes. A
listener may use either or both without learning those implementation terms.

### The current track is never an implicit baseline

Whatever happens to be playing has no effect on Make a mix. There is no
default “current track” state and no seed label in the ordinary composer.

Track and Now Playing menus may separately offer **Make a mix from this**. That
explicit action opens the same composer with a removable reference chip:

```text
BEGIN NEAR   Thème Libre — Art Ensemble of Chicago                 Remove
```

The reference can belong to a particular waypoint—begin near it, arrive near
it, or remain generally related—but its title and role are always visible.
**More like this** is a useful shortcut into the same request model, not a
second generator and not a mode inferred from playback.

### First use: consent is part of Create

The full build performs no sonic or semantic analysis merely because it was
installed. On the first Create mix press, the composed request remains in place
and a bounded consent sheet explains:

- how many selected-edition tracks Baz will read;
- that processing and the disposable index stay on this device;
- an honest time estimate or “Baz will learn as it goes” when no estimate is
  available;
- that playback remains available;
- how to cancel now and remove the index later.

The affirmative action is **Analyse locally & create**; the other action is
**Not now**. Consent enables incremental analysis for this library. New and
changed tracks may then be analysed in the background without another prompt,
while Settings exposes **Pause local analysis**, **Resume**, and **Remove local
mix data**. Removing the disposable index never removes music or playlists.

While a requested mix is waiting, the Home section changes in place:

```text
PREPARING YOUR MIX
4,863 of 4,880 tracks ready · 17 remaining
You can leave this page or keep listening.                         [ Cancel ]
```

Cancellation stops scheduling after the bounded current track, retains useful
completed work and cancels the waiting generation. Pressing Create again
resumes it. Recoverable skipped files produce one compact summary and never a
raw mount path or parser diagnostic.

### Result: preview first, ownership second

Create mix never starts playback, replaces the live run, or writes a playlist
file. It produces one preview beneath the still-visible request:

```text
NOCTURNAL TO WARM                                      58 minutes · 18 tracks
Opening ───────────── Turn ─────────────────────────────── Landing

01  …
02  …                         ordinary shared track rows
…

[ Play ]  [ Save playlist ]  [ Another version ]  [ Edit journey ]
```

The preview uses the same rows and direct actions as every other list. Tracks
can be removed and reordered before saving. Play explicitly makes that edited
preview the run; Save asks for or accepts the suggested name and writes an
ordinary `.m3u8`; neither operation silently performs the other. Closing an
unsaved preview asks only when it has been edited.

**Another version** preserves the request and uses a recorded variation seed
to explore a different high-quality shortlist. It is never automatic, and the
same request plus seed is reproducible. **Edit journey** moves focus back to
the composer and preserves the current preview until the next Create press, so
an experiment cannot destroy a good list without warning.

Saved files carry inert, comment-only provenance: request schema version,
model/index versions, variation seed, named scope and journey phrases. They do
not carry embeddings or private cache paths. Older and light builds ignore the
comments and play the list normally. A saved mix never regenerates itself when
the library or model changes.

### Honest outcomes

- If too little of the requested scope is analysed, Baz waits or offers a
  clearly labelled partial result; it never silently expands the scope.
- If the scope cannot meet duration plus artist/album diversity, Baz returns a
  shorter list and states why rather than repeating tracks to hit a number.
- If no track fits an unusual phrase confidently, Baz says the request was a
  weak match and invites an edit; it does not invent genre or mood labels.
- Empty prompts do not generate. Very long prompts are bounded visibly rather
  than truncated behind the listener's back.
- Generation and preview cost no playback interruption. Indexing stays below
  playback work in scheduling priority.

### Build boundary

The normal Baz build includes the opt-in conventional analyser and performs no
first-run download. `--no-default-features` contains no analyser, model,
tokenizer or embedding index and exists as an internal dependency-boundary
check, not a separately named product. It omits Vibe from Home. Both builds
read, edit and play generated `.m3u8` files identically.

A future semantic integration may bundle an audited model/runtime payload. It
must not download a model after installation unless a later signed model
discovery, verification, staged replacement and removal design explicitly
permits that.

### Responsive and accessible behavior

At wide measures, prompt, curve, length and action share the arrangement drawn
above. Below the existing Home content measure, they stack in that reading
order; the timeline becomes vertical but keeps Opening/Turn/Landing language.
Nothing becomes horizontal scrolling.

Every curve point has a focus stop and a textual value such as “55% through,
energy 4 of 5”; arrow keys move energy, modified arrows move position, Delete
removes a non-endpoint and Add turn is a normal button. Color is not the only
carrier of phase or energy. Reduced motion changes transitions, not state.

## Product-state acceptance

The interaction is not complete until all of these are exercised in the real
application:

1. A first-time listener types one moving request, consents once, leaves Home,
   returns, and receives the preserved silent preview.
2. A returning listener creates a 60-minute mix with one press and no analysis
   ceremony.
3. Start/turn/landing phrases and a custom energy curve produce distinct,
   visible targets across the list rather than a single averaged mood.
4. A playing track changes nothing until **Make a mix from this** is explicitly
   chosen; its named reference is removable.
5. Avoid, scope, duration and insufficient-corpus outcomes remain visible and
   honest.
6. Preview, edit, Play and Save preserve the existing queue/file boundaries
   and never autoplay or silently write.
7. Cancellation, restart, changed files, corrupt files and index removal lose
   no music, playlist or completed reusable work.
8. The light build exposes no unavailable control and plays a full build's
   generated playlist normally.
9. Pointer, keyboard, screen-reader and narrow-window paths reach the same
   request and result states.
10. Internal blind testing shows the semantic/hybrid system reliably beats a
    diversity-matched random control before this replaces the comparator UI.

## Implementation contract behind the composer

The UI owns a model-independent request, not a text string handed directly to
one checkpoint:

```text
MixRequest
  title suggestion
  duration target
  visible scope
  1–4 semantic waypoints { position, phrase, optional track reference }
  2–4 energy points { position, level }
  optional avoid phrase
  variation seed
```

The structural-cue parser only proposes this request. The visible timeline is
the authority after parsing, and every later edit changes the structured value
directly. This keeps the product contract stable if the text/audio model is
replaced and makes a curve reproducible rather than prompt folklore.

Indexing stores both kinds of evidence per compatible file stamp: the
conventional tempo/loudness/timbre features already built and the selected
joint-space audio embedding formed from the accepted representative-window
policy. Cache schema, decoder policy, preprocessing, model artifact hash and
feature versions all participate in validity. The large audio and text towers
need not be resident together: background indexing owns the audio session;
generation owns the much shorter-lived text session.

Generation is position-aware:

1. embed each semantic waypoint and the optional negative target;
2. interpolate normalized semantic targets at prospective playlist positions;
3. interpolate the explicit energy curve at the same positions;
4. retrieve a broad candidate pool from semantic fit, negative distance and
   energy/tempo agreement;
5. sequence with a bounded look-ahead search over request fit, adjacent sonic
   continuity, duration, album freshness and artist caps;
6. record the chosen variation seed and the evidence used.

A curve therefore changes the target for every position. It is not a sequence
of separately shuffled mood buckets, a filter followed by random order, or a
line drawn over a single ranking. Semantic interpolation supplies musical
direction; conventional features keep requested energy movement physically
grounded; look-ahead prevents the locally nearest next track from spending all
good landing candidates too early.

The first product integration keeps the runtime behind the existing
`vibe-analysis` capability boundary but gives the crate a model-neutral
analyser/index/query API. The GUI receives progress, a generated candidate list
and bounded explanations; it never imports ONNX or tokenizer types. Model
files are compile/package inputs of the full edition, pinned and verified in
release automation, not mutable application data and not a network service.

Internal quality controls do not enter this state model. Diversity-matched
random, metadata and conventional runs remain evaluation systems. Only a
candidate that clears the predeclared blind preference gate is allowed behind
Make a mix; the listener never chooses among algorithms.

## What exists now: the honest comparator

The full build includes a separate `baz-vibe` crate behind the default
`vibe-analysis` feature. `cargo build -p baz --no-default-features` produces a
light build without that crate; playback and ordinary playlists remain.

On explicit consent, Home analyzes the tracks in each record's selected
edition, one cancellable worker task at a time. It reuses `baz-core`'s hardened
offline Symphonia decoder, converts the decoded stereo PCM to 22.05 kHz mono,
and feeds it to `bliss-audio` 0.11.4. No FFmpeg, Python, model download or
realtime-path work is introduced.

The independent `vibe.db` cache stores the exact path, byte size, modification
time, bliss feature version and feature vector. Changed/missing/version-stale
rows are reanalysed; light builds do not need to understand this disposable
database. Cancel stops scheduling after the bounded current-track task and a
run token makes late completion inert.

The Home controls map onto transparent conventional signals:

- energy: tempo plus mean/dynamic loudness;
- warm ↔ bright: zero-crossing rate, spectral centroid and rolloff;
- a sounding-track anchor: distance in the complete tempo/timbre/chroma vector.

Retrieval and ordering are distinct. Baz takes a broad best-fit shortlist,
then walks it for sonic continuity while taking one track per album before
repeats, limiting an artist to two tracks and preventing adjacent repeats.
The preview states coverage, selected count and BPM span. Create writes a
normal `.m3u8` with inert local-sonic provenance and opens it without playback.

This comparator cannot honestly interpret “wistful”, instruments, genre from
sound, or a time-shaped sentence. It therefore does not expose a text box.

## Current landscape and licensing

| Candidate | Capability | Packaging/licence finding | Current place |
|---|---|---|---|
| `bliss-audio` | Native tempo, loudness, timbre/chroma similarity and custom distance | GPL-3.0-only; combining it with Baz's GPL-3.0-or-later code selects GPLv3 for the full artifact, while the light build excludes it. Its Symphonia path documents about 65 minutes for 10k tracks. | Built comparator |
| LAION CLAP | Joint text/audio 512-D space for free-text retrieval | GitHub code is CC0 and the official Hugging Face model card is Apache-2.0. The pinned official checkpoint contains a 614,525,833-byte PyTorch weight file; Baz's reproducible paired quantized ONNX export is 162.7 MB including tokenizer/configuration. | Preferred engineering candidate for blind evaluation; not yet a product choice |
| AudioMuse DCLAP | Distilled CLAP audio tower, 7M parameters, same 512-D space | The v1 audio graph/data are 22.4 MB, but its unchanged text tower is 501.4 MB plus a 2.1 MB tokenizer. The repository is AGPL-3.0-only and the release gives no separate weight grant; its training inventory also includes some CC BY-ND sources. Treat artifacts as evaluation-only pending a combined-work, weight and training-provenance review. | Comparative evaluation only; not redistributable by assumption |
| Microsoft CLAP | General audio/text retrieval | Code MIT, but published weights are labelled Microsoft Public License; not assume-redistributable from the code licence. | Research only pending weight audit |
| Essentia models | Strong mood, danceability, arousal/valence and Discogs heads | Official models are non-commercial Creative Commons or proprietary; unsuitable as Baz's default redistributable dependency. | Reject absent separate permission |
| MuQ-MuLan | Modern music/text model | ~700M parameters and CC-BY-NC weights. | Reject for distribution/size |

Primary references: [bliss-audio](https://docs.rs/crate/bliss-audio/latest),
[LAION CLAP](https://github.com/LAION-AI/CLAP),
[DCLAP](https://github.com/NeptuneHub/AudioMuse-AI-DCLAP),
[AudioMuse's complete local pipeline](https://github.com/NeptuneHub/AudioMuse-AI/blob/main/docs/ALGORITHM.md),
[Essentia licensing](https://essentia.upf.edu/licensing_information.html), and
[MuQ](https://github.com/tencent-ailab/muq).

## Evaluation gate

No semantic UI ships on benchmark reputation alone. Create an owned corpus
large and varied enough to expose failure, with tag-rich and tag-poor music,
multiple languages, live/electronic/acoustic material, repeated artists and
editions. Keep private audio out of the repository; commit only corpus
manifests, prompts and anonymous judgements.

Compare at least four systems blind:

1. diversity-matched random control;
2. deliberately simple metadata/history control;
3. the built conventional-feature baseline;
4. the audited joint text–audio embedding candidate, alone and hybridized with
   conventional tempo/energy constraints.

The prompt set must cover controlled moods, instruments/genre, subtle affect,
seed similarity, negative anchors and temporal arcs. Record prompt relevance,
within-list coherence, adjacent transitions, diversity, rediscovery value and
whether the listener would replay the list. Also record cold/warm generation
latency, tracks/hour, peak RAM, index bytes/track, shipped/model size, CPU
support, licences and Windows/macOS/Linux packaging. The scorer must refuse an
incomplete ballot: every candidate needs every rating and each request needs
one forced preference before identities are restored.

Advance only if the richer system repeatedly wins blind listener preference,
not merely vector-retrieval metrics. If it does not feel impressive, the item
remains active.

## Reproducible evaluation harness

`tools/vibe-eval/` now holds the experiment rather than the GUI. It has no
downloader and never opens Baz's library: the evaluator supplies an explicit
private corpus manifest and local artifact directory. Reviewed DCLAP v1, the
earlier reviewed LAION conversion and Baz's reproduced LAION pair are
independently pinned by byte length and SHA-256; corrupt, missing or substituted
files are refused before inference.
Candidate selection changes model, manifest and recorded system identity
together, preventing an audio tower from being silently queried with the
wrong text tower.

The harness produces four compatible run formats:

1. a diversity-matched seeded-random control;
2. a deliberately weak metadata token-overlap control;
3. `vibe-baseline`, which exports the shipped Bliss axes/cache/diversity path;
4. model-swappable LAION or DCLAP audio/text retrieval, either full-overlap or
   an explicitly labelled evenly sampled window policy.

Indexing constructs only the audio session; querying constructs only the text
session. The large towers therefore cannot become accidentally co-resident in
the evaluation path, matching the capability boundary a product build would
need.

The committed request set covers concrete instruments/genres, mood, subtle
affect, negative guidance and two playlist arcs. Semantic arcs interpolate the
text target across list position, then combine target relevance with adjacent
audio-vector continuity and the same album/artist caps. The conventional
control only steps through energy/brightness targets it can substantiate.

`blind` checks that every run names the same non-empty corpus and the same
fingerprint over every identity/ranking field. Reusing anonymous IDs for
different tracks is therefore refused. It randomizes each request independently
and writes the identity key separately. Its ballot asks
for relevance, coherence, transitions, diversity, rediscovery and replay value
plus a forced preference. `score` restores identities only after listening.
Private paths, embeddings, ballots and notes are ignored under `local/`.

## First CPU/package measurements

Measured 2026-08-13 on a 12th-generation Intel i5-12600K, CPU-only ONNX Runtime
1.23.2. These are spike measurements, not cross-platform acceptance figures:

| Probe | Result |
|---|---|
| Original text tower | 501,445,503 bytes; 0.519 s session load, 0.029 s first one-prompt inference, 714,088 KiB peak process RSS |
| Per-channel QInt8 text tower | 126,531,567 bytes; 0.144 s load, 0.012 s first inference, 233,968 KiB peak RSS |
| QInt8 alignment probe | Across 20 committed-style prompts, cosine to the FP32 text vector was 0.978 mean and 0.915 worst. This is not retrieval-quality proof. |
| Full reference audio policy | One 6:27 fixture produced 77 overlapping windows: 8.11 s Python decode, 5.66 s mel extraction, 3.03 s audio inference, 17.24 s wall time and 576,420 KiB peak RSS. |
| One-window bounded smoke | Eight long fixture tracks: 5.28 s total; 4.50 s decode, 0.22 s mel and 0.21 s inference. It proves bounded cost and protocol operation, not musical equivalence. |
| Reproduced quantized LAION package | 162.7 MB total: 34,065,939-byte audio tower, 126,552,434-byte text tower and 2,108,746-byte tokenizer, plus small configuration. Exported in 15.67 s from the verified 614,525,833-byte official checkpoint. |
| Reproduced export alignment | FP32 ONNX/PyTorch cosine was 1.000 mean for text and 1.000 for audio. QInt8/PyTorch was 0.9794 mean and 0.9490 worst over the text probe, and 0.9995 for audio. |
| LAION full-window smoke | Three generated fixture tracks, 209 windows total: 2.19 s decode, 6.63 s mel, 15.65 s inference and 24.46 s wall time; 619,100 KiB peak process RSS on a repeated run. |
| LAION bounded smoke | The same three tracks at one representative window each took 3.15 s wall time and 601,396 KiB peak RSS. |

The earlier reviewed LAION text graph omitted `attention_mask`. Batched padding
therefore corrupted its text embeddings (about 0.074 mean cosine to the
official model); naturally sized prompts recover about 0.980 mean and 0.960
worst. Earlier padded rankings are invalid. Baz's reproduced graph carries the
mask and is the preferred evaluation pair.

The LAION audio preprocessor was checked numerically against Transformers
4.36.0's `ClapFeatureExtractor` implementation for the pinned configuration:
both produced `(1, 1, 1001, 64)` features, with maximum absolute difference
`3.82e-6` on a deterministic input. That establishes preprocessing parity,
not music/text retrieval quality.

The generated-fixture smoke completed all 12 requests, kept the ballot free of
system identity and restored mappings during scoring; generated silence does
not count toward the gate. A deliberately consented private corpus now covers
72 real tracks (36 FLAC, 35 MP3 and one WAV), 14 genre labels and 71 artists.
Its six-window LAION pass indexed 432 windows in 85.81 s on the CPU above:
43.35 s decode, 10.72 s mel extraction and 31.70 s inference. Metadata,
conventional and sampled-LAION runs all contain 12 twenty-track rankings and
the same exact-corpus fingerprint. The resulting 36 ordinary M3U8 candidates
are materialized under ignored `local/`; listener ratings remain intentionally
unfilled and the separate identity key stays closed. The harness now also has
a deterministic diversity-matched random control; regenerate this pre-control
ballot with it before collecting ratings.

The same corpus's full-overlap reference used 3,930 windows and took 453.10 s.
Six-window embeddings retained 0.9837 mean and 0.9871 median cosine to that
reference, with a 0.9095 worst track. Across the 12 requests, sampled and full
top-20 lists shared 17 tracks on average and at least 15. Sampling was 5.3×
faster for this run, so its anonymous lists are the practical listening
candidate; those agreement figures do not establish musical quality.

## Next implementation spike

1. **Done for evaluation:** keep the standalone harness and its four controls
   outside the GUI; CI runs its dependency-free protocol tests.
2. **Done for evaluation:** reproduce and pin both towers from the official
   Apache-2.0 LAION checkpoint, including attention-mask-correct text inference
   and PyTorch/ONNX alignment checks. Complete distribution notices remain a
   release requirement. Retain DCLAP only as a quality comparator unless its
   separate weight/provenance questions are resolved.
3. **Listening now:** rate the identity-blind real-music ballot. The completed
   full-overlap comparison supports six representative windows for this gate;
   then measure the accepted policy on CPU-only Linux, Windows and macOS. Model files must ship with the full
   offline edition, never appear as an unannounced first-run download.
4. Add free text to the product only after audio/text vectors demonstrably
   align on the owned prompt corpus. Then add hybrid explicit controls and
   playlist arcs.
5. Keep the analyzer/index capability boundary stable so another model can
   replace it without changing playlist files or the Home contract.
