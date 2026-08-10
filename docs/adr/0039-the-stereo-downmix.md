# ADR-0039: The stereo downmix — the matrix is written down, the layout is read, and the headroom is a constant

**Status**: accepted (2026-08-10) · **narrows the guarantee restated in [ADR-0012](0012-exclusive-output.md)** (which completed [ADR-0009](0009-follow-the-source-rate.md)'s; every earlier decision stands, and what changes is that a *channel* fold is now a fourth thing the chain must be able to admit to, alongside rate conversion and software gain) · leans on [ADR-0013](0013-replaygain.md) and [ADR-0015](0015-replaygain-analysis.md) to pay for its one real cost · measurements in `docs/design/impl/multichannel-downmix/`

## Context

A 5.1 record was not a record baz had. Anything over two channels was refused
at `AudioSource::open` with `UnsupportedChannelCount`, and the file sat on the
shelf — scanned, listed, titled, with its duration and its rate — and did
nothing when clicked.

**The refusal was the right call at the time and it is worth saying why**,
because the same reasoning shapes what this ADR does *not* do. A downmix has
exactly one interesting failure mode and it is silent: put the centre channel
where a surround belongs and the file still decodes, still has the right
length, still sounds like music, and is wrong in a way nobody reports and no
test that checks lengths catches. `docs/BACKLOG.md` recorded the refusal as
"a typed error rather than silently wrong output", which is the correct
preference. What it left was a gap: *"this file is not supported"* is not a
feature, and the owner's 2026-08-10 instruction to prioritise functional work
promoted it to `docs/WORK.md` item 2.

Three questions had to be answered before any coefficient could be written
down, and the ordering matters — the third is the one that turns a matrix into
a *correct* matrix.

1. **Which coefficients?** ITU-R BS.775 is the ordinary answer and was named in
   the queue item.
2. **What happens on overflow?** Summing five channels into two has a
   worst-case gain of +7.66 dB, which is not a corner case in a loud passage.
3. **Which channel is which?** The order of channels inside a decoded packet is
   a property of the container and the codec, and WAVE, FLAC, Vorbis, AAC and
   ALAC do not agree. Any answer to (1) is worthless without an answer to this.

And one that belongs to a different document entirely: baz promises, in
ADR-0009 and again in ADR-0012, that it converts nothing. A matrix fold is a
conversion.

## Decision

### 1. The matrix is ITU-R BS.775's, written per speaker

**ITU-R BS.775, "Multichannel stereophonic sound system with and without
accompanying picture"**, gives the two-channel downmix of a 3/2 programme as

```text
Lo = L + kC·C + kS·Ls
Ro = R + kC·C + kS·Rs        with kC = kS = 1/√2  (−3 dB)
```

`crates/baz-core/src/playback/downmix.rs` holds it as a table keyed on
**speakers**, not on a channel count:

| speaker | → Lo | → Ro |
|---|---|---|
| front left | 1 | 0 |
| front right | 0 | 1 |
| front centre | 1/√2 | 1/√2 |
| rear **or side** left | 1/√2 | 0 |
| rear **or side** right | 0 | 1/√2 |
| LFE | 0 | 0 |

Keying on speakers rather than counts is what makes 3.0, 4.0, 5.0 and 5.1 one
rule instead of four, and it is what lets a 5.1 whose surrounds a container
calls *sides* — which is what ALAC's magic cookie and ffmpeg's `5.1(side)`
produce for the same music — get the same treatment as one that calls them
rears.

**Nothing was invented, and the citation is checkable without a standards
document**: ffmpeg's libswresample calls these `center_mix_level` and
`surround_mix_level` and defaults both to `0.707107`.
`docs/design/impl/multichannel-downmix/crosscheck.py` prints those defaults out
of the running binary and measures ffmpeg's own downmix of the same fixtures
against ours: placement agrees to 6.5e-5.

### 2. The layout is read from the file, never inferred from the channel count

This is the decision the whole thing turns on.

Symphonia's contract is that the *n*-th plane of a decoded buffer is the *n*-th
channel of `SignalSpec::channels` **in ascending bit order** — and those bits
are WAVE's `SPEAKER_*` values, so the bitmask is a *set* of speakers and the
plane order follows from it. Decoders whose bitstreams disagree permute back:
Symphonia's Vorbis decoder carries an explicit table (`map_vorbis_channel`) for
exactly this.

So `AudioSource` keeps the `Channels` **set** the decoder reports and hands it
to the matrix. It never counts channels and never assumes a position. That
sentence is the difference between a downmix and a plausible-sounding bug.

**Measured, not assumed.** The same music was put through five containers whose
bitstreams order 5.1 differently — WAVE and FLAC as `FL FR FC LFE Ls Rs`,
Vorbis as `FL FC FR BL BR LFE`, ALAC by its magic cookie and with the surrounds
spelled as sides — with a *distinct tone in each speaker*, and the output
profiled per frequency per output channel. All five produce the same stereo
pair. `each_speaker_lands_where_the_layout_says` is the test; the table is in
`measurements.md`. A fold that assumed WAVE's order would put Vorbis's centre
channel in the right output, which is audible and which no length check sees.

### 3. The LFE is dropped, not folded — a stated departure from the brief

The work was asked for with "centre and LFE folded at −3 dB". **BS.775's
downmix equations contain no LFE term**, and this implementation does not add
one. Folding a band-limited effects channel — mixed at +10 dB relative to the
main channels by the same recommendation's convention — into a stereo pair puts
subsonic energy into a signal two loudspeakers will try to reproduce, at a level
the mix engineer never auditioned. Where a standard *does* offer it, ATSC
A/52's `lfemixlevcod`, it is optional and off by default; libswresample's
`lfe_mix_level` defaults to `0` for the same reason, which
`crosscheck.py` prints.

In a competently mastered 5.1 music release the main channels already carry
full-range bass and the LFE holds what is *additional*. Dropping it is what a
stereo listener of that release is supposed to hear.

This is recorded as a departure rather than absorbed quietly, because the brief
was specific and someone should be able to overrule it knowingly. Changing it is
one row of the table.

### 4. Headroom is a constant attenuation, not a limiter and not nothing

The matrix overflows: `1 + 1/√2 + 1/√2 = 2.4142`, **+7.66 dB**, reached by any
signal that is full-scale and correlated across L, C and Ls. Three answers were
available.

- **Nothing.** Samples leave the decoder above ±1.0 and are clipped by whatever
  is furthest downstream — *after* the resampler, which turns an out-of-range
  sample into ringing on both sides of it, and *after* the volume stage, which
  makes whether it clips at all depend on where the slider is. Rejected: it
  makes the distortion's existence depend on unrelated settings. (This is
  ffmpeg's behaviour for float output, and `measurements.md` shows its downmix
  of the clipping fixture peaking at 2.4136.)
- **A limiter.** Transparent on peaks, and **stateful** — which is the
  disqualifying property, before any question of whether baz wants to design
  one. The decode path is a pure function of position: `AudioSource::seek` must
  produce the same samples for the same frame however you arrived at it, and the
  integration tests compare a seeked decode against a reference decode of the
  whole file. A gain that depended on what it heard a moment ago would make
  those two disagree, and there would be no honest way to say which was the
  file.
- **Constant attenuation by the matrix's own worst case.** Chosen. Every
  coefficient is scaled by `1 / max(Σ|Lo|, Σ|Ro|)`, so the fold is a pure linear
  matrix that provably cannot overflow, for any input, at any position, with no
  state and no dependence on playback history.

Per layout: **−7.66 dB** for 5.1 and 5.0, **−4.65 dB** for quadraphonic and
3.0, nothing at all for stereo and mono, which are not folded. It is the
matrix's own worst case and not a round number chosen for safety — the clipping
fixture reaches exactly 1.0, and a test asserts that it does, because
attenuating further than the matrix needs would be quiet for no reason.

**The cost is real and is named rather than left to be discovered**: a 5.1 file
plays quieter than the stereo master of the same record. Two things soften it.
It is a constant, so the record is quieter and not squashed. And baz has
ReplayGain (ADR-0013) and an analysis pass (ADR-0015) that measures *this
decoder's output* — so an analysed 5.1 file gets its level back exactly and
automatically. Measured end to end: the same tone as stereo and as 5.1 asks for
**766 centidecibels** more gain in the second case, a number derived from the
matrix rather than from a previous run.

### 5. A downmixed track is not bit-perfect, and `SignalPath` says so

ADR-0009 promised baz converts nothing; ADR-0012 extended the claim past baz's
process boundary. A matrix fold is a conversion by any reading, so the guarantee
is **narrowed honestly** rather than quietly kept.

`Event::SignalPath` carries a new `source_channels: usize`. Above `CHANNELS`
means a BS.775 matrix is in the path and the track is not bit-exact however
`chain` and `VolumePath` read.

It is a **field and not a fourth `SignalChain` variant**, for the reason
`Event::VolumeChanged` carries the volume path separately: the facts are
orthogonal. A 5.1 file can be downmixed *and* played at its own rate *and* on a
device baz holds exclusively — all three at once — and
`SignalChain::Exclusive { conversion: None }` continues to mean exactly what it
always meant, that the device is held and the rate is the source's. It was never
making a channel claim; now it does not have to.

**What happens to a multichannel file under the exclusive path is therefore:
it plays, folded, and the readout admits it.** Refusing multichannel in
exclusive mode was considered and rejected — a file that plays in shared mode
and not in exclusive mode would be a worse surprise than an honest label, and
the output device is opened at `CHANNELS = 2` in *both* modes anyway, so a 5.1
file was never going to reach a converter as six channels whatever this ADR
decided.

### 6. What is still refused, and it is refused honestly

The matrix describes exactly the speakers BS.775's equations name. Everything
else fails at `open` with `UnsupportedChannelLayout`, which names the layout it
found (`FL+FR+FC+LFE+RL+RR+SL+SR`) so a person can act on it:

- **7.1** — both a rear *and* a side surround pair. BS.775's 3/2 programme has
  **one**. Folding two pairs at `kS` each would put 3 dB too much surround in
  the mix; folding them at anything else would be a coefficient invented here.
  The ordinary answer is a two-stage 7.1 → 5.1 → 2.0 fold, and it belongs to
  whoever can cite the first stage.
- **6.1** — a rear centre, which BS.775 does not place.
- **Height, wide and top channels**, and any speaker the table does not name.
- **A layout without both front speakers**, or with half a surround pair.
  Neither is a programme BS.775 describes, and inferring what the orphan channel
  *meant* is precisely the guess this refusal exists to avoid.

This is the blanket refusal narrowed to the cases that still need it, which is
what a partial fix that stays honest at its edge looks like.

**Separately, and not our doing: multichannel AAC does not decode at all.**
Symphonia 0.5's AAC decoder rejects a 5.1 stream with
`Unsupported("aac: aac too complex")` before a single frame exists, so no
layout ever reaches the matrix. It is reported as the decode error it is, and
pinned by a test that will *fail* the day Symphonia grows past two channels —
at which point the fold is already there waiting for it.

### 7. Where it lives

Inside `AudioSource::next_block`, in the same place mono has always been
upmixed. Every block leaving a source is `CHANNELS`-interleaved f32 exactly as
before, so **the ring, the resampler, the splice, the sink and the gapless
machinery are untouched and never learn the file had six channels**. The
library side is untouched too: the scanner reads headers with lofty and never
looked at a channel count, so multichannel files have always been *listed* —
they simply refused to play. **No rescan is needed to gain this**, and
`a_multichannel_file_is_listed_like_any_other` says so rather than leaving it
to be assumed.

## Consequences

- 3.0, 4.0 (quadraphonic), 5.0 and 5.1 play, in WAV, FLAC, Vorbis and ALAC.
  That is the great majority of multichannel music in a real library.
- 7.1, 6.1 and anything with height channels still do not play, now with a
  message that names the layout and the reason. Narrowed in `docs/BACKLOG.md`
  rather than struck.
- Multichannel AAC still does not play, and the reason is upstream. Stated in
  the `playback` module docs next to the other Symphonia limitations that are
  measured rather than papered over.
- **A 5.1 file plays 7.66 dB below its stereo master on an untagged, unanalysed
  library.** This is the change a listener is most likely to notice, and the
  answer is to run the ReplayGain pass — which is a thing baz already has, and
  which recovers it exactly.
- `Event::SignalPath` grew a field. The wire format changed; the JSON case for
  it is pinned in `protocol.rs`, including one for a downmixed track on an
  exclusive chain.
- The decode path stays a pure function of position, which is what keeps
  `a_seek_into_a_downmixed_file_lands_on_the_same_samples` meaningful and what
  ruled the limiter out.
- No new dependency. The matrix is nine constants and a multiply-accumulate.
- The measurement rig is committed, fixtures included, so the next person to
  touch a coefficient can re-run it against ffmpeg rather than trusting this
  document.
