# Multichannel downmix — what was measured

Evidence for **ADR-0039**. Everything here reads *output samples*: "the file
played without an error" is the assertion a wrong channel order passes, so it
is never the assertion made.

Reproduce with:

```
./mkfixtures.sh fixtures            # needs ffmpeg
./crosscheck.py fixtures            # needs ffmpeg + numpy
cargo test -p baz-core --test playback -- --nocapture --test-threads=1 \
    each_speaker the_centre a_full_scale the_downmix_takes the_layouts_bs775
```

Taken 2026-08-10 on Fedora, ffmpeg 8.1.2, Symphonia 0.5.5.

## The fixtures

One **distinct, mutually non-harmonic tone per speaker**, assigned in the
channel mask's ascending-bit order — which for 5.1 is `FL FR FC LFE Ls Rs`.
Independent tones are the whole design: six copies of one tone would still
produce a plausible waveform if the centre channel came out of a surround, and
only a per-channel tone makes *which one landed where* a measurable question.

| slot | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|---|
| speaker (5.1) | FL | FR | FC | LFE | Ls | Rs |
| tone (Hz) | 400 | 700 | 1100 | 1700 | 2300 | 3100 |

Amplitude 0.3 in the placement fixtures, so even the 5.1 matrix's worst case
stays inside full scale and a placement measurement can never be confused with
a clipping one. `clip_51` is the separate worst case: **one** tone, full scale,
in phase in all six channels.

**The fixtures are generated, not committed.** `.gitignore` excludes `*.wav`,
`*.flac` and `fixtures/` project-wide, and every `docs/design/impl/` directory
that needs audio ships a `mkfixture*.sh` rather than the bytes — the same rule
`tests/playback.rs` follows, which builds its own fixtures at run time and skips
with a notice on a machine without ffmpeg. `mkfixtures.sh` here produces the
whole set (five WAV layouts, four re-encodings of the 5.1 one, and the two
layouts that must be refused) in about a second.

> **A trap worth recording**, because it cost the first run: ffmpeg's `join`
> filter *guesses* its mapping when given mono inputs — every mono stream's one
> channel is called `FC`, so the guess is ambiguous — and silently produces a
> different layout than the one named. The first measurement showed FL landing
> in plane 2 and the LFE silent, which looked exactly like a Symphonia bug and
> was a fixture bug. `mkfixtures.sh` names the map explicitly. Also: ffmpeg's
> `sine` source has no amplitude option and emits a peak of **0.125**, so every
> level in the script is set with an explicit `volume` rather than assumed.

## 1. Channel order: which speaker lands where

The same music in five containers whose bitstreams order 5.1 differently.
Measured by Goertzel at each speaker's own frequency, in each output channel,
through `AudioSource::decode_all` — i.e. through the real decode path.

Expected from ITU-R BS.775 with the −7.66 dB headroom scale applied:
FL → Lo at `0.3 × 0.41421 = 0.12426`; FC and Ls → Lo at
`0.3 × 0.70711 × 0.41421 = 0.08787`; everything else → 0.

```
                          400      700     1100     1700     2300     3100
                           FL       FR       FC      LFE       Ls       Rs
5.1 WAV            Lo   0.12426  0.00000  0.08787  0.00000  0.08787  0.00000
5.1 WAV            Ro   0.00000  0.12426  0.08787  0.00000  0.00000  0.08787
5.1(side) WAV      Lo   0.12426  0.00000  0.08787  0.00000  0.08787  0.00000
5.1(side) WAV      Ro   0.00000  0.12426  0.08787  0.00000  0.00000  0.08787
5.1 FLAC           Lo   0.12426  0.00000  0.08787  0.00000  0.08787  0.00000
5.1 FLAC           Ro   0.00000  0.12426  0.08787  0.00000  0.00000  0.08787
5.1 ALAC in MP4    Lo   0.12426  0.00000  0.08787  0.00000  0.08787  0.00000
5.1 ALAC in MP4    Ro   0.00000  0.12426  0.08787  0.00000  0.00000  0.08787
5.1 Vorbis in Ogg  Lo   0.12535  0.00000  0.08851  0.00000  0.08837  0.00000
5.1 Vorbis in Ogg  Ro   0.00000  0.12520  0.08851  0.00000  0.00000  0.08843
```

Two smaller layouts, same run:

```
                          400      700     1100     1700
                           FL       FR       Ls       Rs
quadraphonic WAV   Lo   0.17573  0.00000  0.12426  0.00000
quadraphonic WAV   Ro   0.00000  0.17573  0.00000  0.12426

                          400      700     1100
                           FL       FR       FC
3.0 WAV            Lo   0.17573  0.00000  0.12426
3.0 WAV            Ro   0.00000  0.17573  0.12426
```

**Five containers, one answer.** The Vorbis row is the interesting one: its
bitstream orders 5.1 as `FL FC FR BL BR LFE`, so a fold that assumed WAVE's
order would put the **centre** channel in the right output and the **right**
channel in the centre position — audible, and invisible to any test that
checks lengths. It agrees to 1.1e-3, which is libvorbis at `-q:a 8` on a pure
tone and not a channel error.

ALAC declares no layout in the MP4 at all; the answer comes from the codec's
magic cookie through `probe_first_packet`, and it spells the surrounds as
**sides** rather than rears. Same programme, same matrix, different container's
word for it — which is why the matrix is keyed on speakers and treats
`SIDE_LEFT` and `REAR_LEFT` alike.

### Why this works: Symphonia's contract, checked

`SampleBuffer::copy_interleaved_ref` interleaves plane 0..n in order, and the
contract is that plane *i* is the *i*-th set bit of `SignalSpec::channels`.
Symphonia's Vorbis decoder honours it with an explicit permutation table
(`map_vorbis_channel`, `symphonia-codec-vorbis-0.5.5/src/lib.rs:746`), which is
what the table above confirms from the outside. The fold is built from the
layout, never from the channel count, so the contract is the only assumption —
and it is the one measured here.

## 2. Clipping

`clip_51`: one 400 Hz tone, full scale, in phase in all six channels — the
signal that actually reaches the matrix's worst case. (Six *different* tones at
full scale would not: they do not add coherently, which is why the clipping
fixture had to be built deliberately rather than by turning the placement
fixture up.)

| | |
|---|---|
| fixture peak, each of six channels | 1.0000 |
| matrix worst case `1 + 2/√2` | 2.4142 (**+7.66 dB**) |
| ffmpeg `-ac 2` output peak | **2.4136** |
| baz output peak | **1.0000** |

ffmpeg, writing float, takes no headroom and hands on a sample stream 7.66 dB
above full scale. baz scales the whole matrix by `1/2.4142 = 0.41421` instead,
so the fold provably cannot overflow for any input at any position — and
reaches exactly 1.0 rather than something smaller, because attenuating further
than the matrix needs would be quiet for no reason.

Per-layout, the headroom is the matrix's own and no more:

| layout | worst-case row sum | attenuation |
|---|---|---|
| 5.1 / 5.0 | 1 + 2/√2 = 2.4142 | −7.66 dB |
| quad, 3.0 | 1 + 1/√2 = 1.7071 | −4.65 dB |
| stereo, mono | 1 | none — not folded at all |

## 3. Cross-check: ffmpeg's own downmix

`crosscheck.py`, reading the defaults out of the running binary rather than
from memory:

```
center_mix_level     0.707107
surround_mix_level   0.707107
lfe_mix_level        0
```

The same two coefficients, and **the LFE dropped**, which is the decision
ADR-0039 §3 makes and the one place this work departs from the brief it was
given. Placement agrees to 6.5e-5 (fixture rounding). Levels do not agree, and
that is §4's decision measured rather than hidden.

## 4. What is refused, and by whom

| layout | outcome |
|---|---|
| 7.1 (`FL+FR+FC+LFE+RL+RR+SL+SR`) | refused: `UnsupportedChannelLayout` — two surround pairs, BS.775 has one |
| 6.1 (`FL+FR+FC+LFE+RL+RR+RC`) | refused: `UnsupportedChannelLayout` — BS.775 does not place a rear centre |
| **5.1 AAC in MP4** | refused **by Symphonia**: `Unsupported("aac: aac too complex")` |

The AAC line is worth separating from the other two. Symphonia 0.5's AAC
decoder rejects a 5.1 stream before decoding a single frame, so no layout ever
reaches the matrix and no coefficient of baz's is involved. It is pinned by
`multichannel_aac_is_refused_by_the_decoder_not_by_the_downmix`, which will
**fail** the day Symphonia grows past two channels — at which point the fold is
already there waiting for it. Multichannel FLAC, WAV, Vorbis and ALAC all
decode and all play.

## 5. ReplayGain gets the level back

The attenuation's cost is real: a 5.1 file plays 7.66 dB quieter than the
stereo master of the same record. Measured end to end through the real analysis
pass (`a_multichannel_source_is_measured_as_its_downmix`): the same tone as a
stereo file and as a 5.1 file carrying it in the front pair asks for
**766 centidecibels** more gain in the second case, within the pass's own 10
centidB tolerance — the number derived from the matrix on paper, not from a
previous run.

So on an analysed library the fold costs a listener nothing. That is the whole
argument for taking the headroom as a **constant** rather than with a limiter:
it is a level change, and a level change is the one kind of damage ReplayGain
undoes exactly.

## 6. The library side

**No rescan is needed, and none ever was.** `a_multichannel_file_is_listed_like_any_other`
(`tests/scanner.rs`) scans a real `WAVE_FORMAT_EXTENSIBLE` 5.1 file and finds
it on the shelf with its duration, rate and format. The scanner reads headers
with lofty and never decodes; nothing in the scan path looks at a channel
count. A 5.1 record has been *listed* since the day it was copied in — it
simply refused to play when clicked. The fold changes the play side and nothing
on the shelf.

## 7. No device test was run

Everything above is decode-path arithmetic on committed fixtures, verified
against ffmpeg. Nothing in this change touches the device backends: the output
is opened at `CHANNELS = 2` in shared *and* exclusive mode exactly as before,
because the fold happens inside `AudioSource::next_block` and every block
leaving a source is stereo-interleaved f32 as it always was. There is no claim
here that a loudspeaker could settle and a Goertzel could not, so
`BAZ_DEVICE_TESTS=1` was not set.
