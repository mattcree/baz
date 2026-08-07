# ADR-0012: Exclusive-mode output — baz holds the card, and the chain says so

**Status**: accepted (2026-08-07) · **completes the guarantee stated in [ADR-0009](0009-follow-the-source-rate.md)** (that ADR's decisions all stand; what changes is that the claim no longer stops at baz's process boundary) · **resolves the deferral in [ADR-0011](0011-volume-control.md)** (hardware volume, which that ADR measured to be impossible in shared mode and predicted would become possible here) · measurements taken on the maintainer's own machine (Fedora/`PipeWire` 1.6.7)

## Context

ADR-0009 made baz follow the source rate and resample nothing, and was
scrupulous about where the claim stopped:

> This is shared-mode output. What is guaranteed is that *baz* performs no
> conversion […] Whether `PipeWire` or `CoreAudio` then resamples to a graph rate
> it is holding for another client is outside this backend's control. Removing
> that last hop is what exclusive-mode backends (ALSA `hw:`, WASAPI exclusive,
> `CoreAudio` hog) are for, and they remain a later phase — as does `SignalPath`
> growing a field for it.

ADR-0011 then found the same wall from the other side. It went looking for a
bit-exact volume control, established that cpal opens the PCM literally named
`"default"` — which on this desktop is the sound server's bridge, not a card —
and concluded:

> *In shared mode there is no bit-exact per-application volume to reach for on
> any platform.* […] Hardware volume becomes a real option exactly when baz owns
> the card.

Two separate pieces of work, one missing capability. This ADR is that
capability on Linux.

## Decision

### 1. An ALSA `hw:` backend, as an alternative `Sink`

`baz_core::playback::exclusive::ExclusiveSink` opens a hardware PCM directly
and implements the existing `Sink` trait. Nothing else in the engine changes:
same negotiation handshake, same reopen-on-rate-change, same drain-and-restart,
same volume seam. The backend is behind the non-default `exclusive-output`
feature and `cfg(target_os = "linux")`.

**`hw:` only, never `plughw:`.** `plughw:` is libasound's "convert whatever you
give me" wrapper — it accepts a rate the hardware cannot do and resamples
inside the process, silently, which is the exact behaviour this whole line of
work exists to eliminate. `ExclusiveSink::open` rejects any name that does not
begin with `hw:` with a typed error, *and* calls
`snd_pcm_hw_params_set_rate_resample(0)`, so the claim is structural rather
than incidental. (Pinned by `plughw_is_refused_even_when_handed_in_directly`.)

### 2. Devices are enumerated, and baz does not guess between them

`exclusive::devices()` walks each card's **control** interface
(`snd_ctl_pcm_next_device` + `snd_ctl_pcm_info`) and produces `hw:CARD,DEV`
names with the card and PCM descriptions attached. Going through the control
interface rather than by opening PCMs matters twice over: it never disturbs a
device another application is using, and a busy device still *appears in the
list*, so "that one is busy" is something baz can say about a device the
listener chose rather than a device that mysteriously vanished.

On the machine this was written on, that is seven playback PCMs:

```
hw:0,0 (HDA Intel PCH — ALC897 Analog)
hw:0,1 (HDA Intel PCH — ALC897 Digital)
hw:1,3 (HDA ATI HDMI — DELL G3223D)
hw:1,7 (HDA ATI HDMI — DELL U2518D)
hw:1,8 (HDA ATI HDMI — HDMI 2)
hw:1,9 (HDA ATI HDMI — EPSON PJ)
hw:3,0 (iFi (by AMR) HD USB Audio — USB Audio)
```

The capture-only Blue Snowball (card 2) contributes nothing, and neither does
`"default"`, `sysdefault`, `plughw:`, `dmix` or `pulse` — the list is hardware
or it is not a list (`exclusive_enumeration_offers_hardware_and_never_the_sound_server`).

**With several devices and none named, opening is an error that lists them.**
Not a default. Choosing between analog, S/PDIF, four monitors and a USB DAC on
someone's behalf is a coin toss dressed as a policy, and the one it would get
wrong is the one they bought the DAC for. With exactly one device there is
nothing to choose and it is used.

### 3. The opt-in is two environment variables, resolved in one place

```sh
BAZ_OUTPUT=exclusive BAZ_OUTPUT_DEVICE=hw:3,0 baz
```

`OutputMode::from_env()` is read by `engine::spawn_device`, which is the
function the app already calls — so **opting in needs no front-end change at
all**, and a device error already reaches the user through the bottom bar's
existing "audio device unavailable" path. A front end with a settings surface
calls `engine::spawn_device_with(cfg, &OutputMode::…, …)` instead and never
touches the environment.

This is the smallest honest mechanism available to this unit. A config key
would mean `crates/baz`'s hand-rolled TOML writer (which `docs/BACKLOG.md`
already wants replaced) and a CLI flag would mean argument parsing, both in a
crate owned by a parallel unit this round; a settings UI is explicitly out of
scope. Two variables cost nothing and are exactly the shape a UI later reads
from.

**An unrecognised `BAZ_OUTPUT` is an error, not a fall back to shared.** A
listener who typed `exlcusive` and silently got the sound server would have
been misinformed about the one thing the setting exists to state. For the same
reason, an exclusive open that cannot happen — busy, unknown name, several
devices and none chosen, a platform with no backend — **fails the spawn**. baz
never quietly downgrades.

### 4. Reported through `Event::SignalPath`, on the `chain` field

`SignalChain` grows a third state:

```rust
pub enum SignalChain {
    Direct,                                        // shared, nothing converted
    Converting { reason: ConversionReason },        // shared, converting
    Exclusive { conversion: Option<ConversionReason> },  // baz owns the device
}
```

with `is_exclusive()`, `is_converting()` and `conversion_reason()` so a front
end asks about the property rather than enumerating variants — the rule
`VolumePath::is_transparent()` established.

**Why the reason is nested rather than excluded.** Exclusivity and conversion
are independent facts: a DAC with no 96 kHz mode is a DAC with no 96 kHz mode
whoever owns it, so "held exclusively, and converting because the hardware
cannot follow" is a true and perfectly ordinary sentence that the readout has
to be able to say. Two booleans' worth of information, three reachable states,
three variants.

**Why `chain` and not a new field on `Event::SignalPath`.** The backlog
promised a field, and a field is the cleaner long-term shape — it separates the
two axes at the type level instead of by convention. It is not what shipped,
for a mechanical reason worth recording rather than hiding: `Event`'s variants
are not individually `#[non_exhaustive]`, and `crates/baz` destructures
`SignalPath`'s fields exhaustively in three places, so **adding a field is a
source break in a crate a parallel unit owns this round**. ADR-0011 anticipated
exactly this ("If both readouts are ever wanted in one message, that
destructuring gains a `..` first") and the sequencing is unchanged: when that
`..` lands, moving exclusivity to its own field is an additive protocol change
and a mechanical refactor. `SignalChain` is itself `#[non_exhaustive]` and both
downstream matches already carry wildcards, so the variant added here is
source-compatible today and wire-additive either way.

Naming stays informational, per ADR-0009 §5. Shared mode is how everyone plays
music and describes a perfectly good listening experience; `Exclusive` says
which arrangement is in use and nothing more. There is no "degraded", no
"fallback", and no better-or-worse.

### 5. Sample format: the widest exact carrier the hardware offers

Shared mode hands cpal f32 and is done. A `hw:` PCM takes what the converter
takes, and **no hardware PCM on this machine offers float at all** — the ladder
is tried widest-first and settles on `S32_LE` everywhere here:

| Device format | Scale | Exact for |
|---|---|---|
| `FLOAT_LE` | none, written straight through | everything |
| `S32_LE` | ×2³¹ | every integer source of 24 bits or fewer |
| `S24_LE` | ×2²³ | every integer source of 24 bits or fewer |
| `S16_LE` | ×2¹⁵ | 16-bit and 8-bit sources |

The exactness is arithmetic, not a hope. Symphonia normalises integer PCM to
f32 by dividing by a power of two (2¹⁵ for 16-bit, 2²³ for 24-bit), so a
decoded sample is *k*/2ⁿ for an integer *k*; multiplying by 2³¹ gives *k*·2³¹⁻ⁿ,
an integer inside `i32`, produced exactly by the f64 multiply the backend uses.
That is asserted over the **entire code space** rather than a sample of it —
all 65 536 sixteen-bit codes and all 16 777 216 twenty-four-bit codes —
by `s32_is_exact_for_every_16_bit_code` and `s32_is_exact_for_every_24_bit_code`.

`S16_LE` is the one rung that costs a 24-bit master resolution, and it is where
dither becomes the live question ADR-0011 flagged. It is not reached on any
device here, and the ladder is what keeps it last.

### 6. Hardware volume, at last

ADR-0011 built `Sink::set_device_volume` and found nothing correct to put
behind it, for three reasons that **all three vanish when baz owns the card**:
it is card-wide (baz now holds the card's playback PCM, so nothing else is on
it), baz could not identify the card (it is named in the device the listener
chose), and it is not general (a card without an attenuator now reports so and
falls back, per-device rather than per-platform).

The backend opens the card's mixer, prefers the `PCM` element (then `Master`,
`Speaker`, `Headphone`, then the first playback element with a decibel scale —
a USB DAC's single feature unit is named after the device and is picked up by
that last clause), and sets the requested gain in decibels. It declines, and
lets software gain take over, in three cases:

- **no element with a decibel scale** — S/PDIF and HDMI outputs generally have
  none, and guessing an attenuation from a raw 0..87 range is the assumption
  ADR-0011 refused to make;
- **unity** — the element is parked at 0 dB and the answer is `None`, so the
  engine reports `VolumePath::Unity`, which is the more precise of the two true
  statements;
- **below the attenuator's reach, including mute** — a hardware minimum of
  −51 dB is not silence, and software gain reaches exactly zero.

**What is claimed and what is not.** The claim is that the *samples are not
scaled* — the fidelity question, and the engine's half of it is asserted
against the delivered stream itself
(`a_sink_with_an_attenuator_carries_the_volume_and_the_stream_is_untouched`).
It is *not* a claim that the attenuation equals the fader position to the
decibel: mixer travel is quantised far more coarsely than a 1000-position
control, and the element lands on the nearest value it has. Measured: a request
for −6.0206 dB landed on −6.00 dB. Two hundredths of a decibel is a hundred
times below audibility — but it is not zero, and saying so is the habit this
project has.

## Measurements

All on Fedora/`PipeWire` 1.6.7, release-mode ALSA calls, against real hardware.

**`hw:0,0` — HDA Intel PCH, ALC897 Analog** (the device the suite ran on; the
DAC was busy, see below):

| | |
|---|---|
| Negotiated | **44 100 Hz, `S32_LE`, stereo** — the rate asked for, exactly |
| Buffer / period | 8192 / 2048 frames (186 ms / 46 ms) |
| Under-runs over 0.5 s of tone | **0** |
| Rate change 44.1 → 48 kHz | **12.6 ms** (12–22 ms across runs) |
| Re-requesting the open rate | **0.000 ms** |
| `discard_buffered` | **8185 frames (185.6 ms) removed synchronously**; 41 frames (0.93 ms) still reported by `snd_pcm_delay` on the re-prepared stream |
| Hardware volume | `PCM` element, **−51.00 … 0.00 dB** travel; asked −6.02 dB, landed **−6.00 dB** |
| Second open of the held device | refused in **50 µs** with `PlaybackError::DeviceBusy` |
| End-to-end engine run | played 44 100 Hz and reported `Exclusive { conversion: None }` |

The rate-change figure is worth putting beside ADR-0009's: tearing down a cpal
stream and building a new one took **21.4 ms**; releasing and reopening a `hw:`
PCM takes **12.6 ms**. Following the source is, if anything, slightly cheaper
here.

The discard figure is the one that changes shape rather than size. Shared mode
needs a monotone watermark, a callback to observe it, and a settle budget the
test has to poll under; exclusive mode is `snd_pcm_drop`, which is complete
when it returns. The 41 frames left are what this driver reports on a
freshly-prepared stream — 0.9 ms, and not audio anyone hears, since the DAC
stopped.

**`hw:3,0` — the maintainer's iFi HD USB DAC — was busy throughout, and that is
itself a result.** `PipeWire` held it in `RUNNING` state for the whole session
with an active client stream on it. Every exclusive open of it refused in
~50 µs with `DeviceBusy` naming the device. That is the designed behaviour
observed on the exact device ADR-0011 was written about, and it is the honest
answer to "what happens on a desktop": **the default sink is usually taken**.
What could be read without opening it is its mixer, which is the attenuator
exclusive mode would drive: **−127.00 … 0.00 dB in 32 512 steps (0.0039 dB per
step)**, against the ALC897's 0.2 dB. This ADR does not claim a measurement of
playback through it.

**Loopback capture was not contrived.** No device on this machine offers a
playback loopback (`snd-aloop` is not loaded, and card 0's capture side is a
microphone input, not a monitor of its own output). Loading a virtual loopback
would have measured the loopback driver, not the DAC. The bit-exactness
evidence is therefore what can honestly be had: the negotiated rate equals the
source rate, the negotiated format carries every 24-bit code unchanged (proved
exhaustively, not sampled), no resampler is constructed (`Conversions` counters
at zero), and the chain reports itself as exclusive and non-converting.

## Consequences

- **ADR-0009's guarantee is complete on Linux, and only on Linux.** When
  `SignalChain::Exclusive { conversion: None }` is reported, there is no mixer,
  no resampler and no other application between the decoder and the converter.
  `Direct` keeps its exact former meaning — shared mode, nothing converted *by
  baz* — and that is now stated in its docs rather than implied.
- **Bit-exactness is now the conjunction of three facts**, extending ADR-0011's
  amendment: `SignalChain` reports no conversion, `VolumePath::is_transparent()`,
  and — for the claim to reach past baz's process — `SignalChain::is_exclusive()`.
  The first two remain the whole of what baz itself does.
- **Exclusive mode takes the card.** Nothing else can play while baz holds it,
  and baz cannot start while something else does. On a desktop that means the
  default sink is usually unavailable, and the listener must either point baz
  at a device the server is not using or release the one it is. This is
  inherent to exclusive mode everywhere, not a defect of this implementation,
  and the failure is fast and named.
- **No `unsafe`.** The workspace's `unsafe_code = "deny"` was expected to need
  an exemption for a platform audio backend and did not: the `alsa` crate's
  safe wrappers cover `snd_pcm_writei`, the hardware/software parameter
  negotiation, the control interface and the mixer, so this backend is entirely
  safe Rust. `docs/ENGINEERING.md`'s exemption list stays as it was.
- **No realtime thread, and no second ring.** cpal's pull model forces a
  wait-free ring between a host-owned callback and the engine; ALSA's push
  model does not, so the engine's pump thread feeds the kernel buffer directly
  and no code in this backend runs on a realtime thread. `Sink::write` bounds
  its own blocking by asking `snd_pcm_avail_update` for room before writing,
  and abandons the stream after 5 s of no progress rather than waiting forever.
- **The dependency is a line, not a build requirement**, exactly as ADR-0011's
  costing table said: `alsa` 0.9.1 was already in `Cargo.lock` as an indirect
  dependency of cpal's ALSA host, and `alsa-sys` already links `libasound` for
  every `device-output` build. `cargo deny check` is green with no new
  exceptions.
- **Windows and macOS are untouched and unbroken.** `alsa` is scoped to
  `cfg(target_os = "linux")`, and `cargo tree --all-features` for
  `x86_64-pc-windows-msvc` and `aarch64-apple-darwin` shows it absent from the
  graph entirely. Asking for exclusive mode there is a typed error naming what
  is missing.
- **Software gain remains the default path**, because shared mode remains the
  default. Nothing about a listener who never sets `BAZ_OUTPUT` changes.

## What remains platform-specific

| Platform | Mechanism | Cost | Notes |
|---|---|---|---|
| **Linux** | ALSA `hw:` PCM, `snd_pcm_hw_params_set_rate_resample(0)` | **done** — a dependency line | Card mixer gives a real attenuator where the hardware has one |
| **Windows** | WASAPI `AUDCLNT_SHAREMODE_EXCLUSIVE` on an `IAudioClient` cpal does not expose | a `windows`/`windows-sys` dependency and a backend | `IAudioEndpointVolume` becomes legitimate for the same reason it does here: the endpoint is baz's |
| **macOS** | `kAudioDevicePropertyHogMode`, plus `kAudioDevicePropertyNominalSampleRate` to stop `CoreAudio` resampling | a `coreaudio-sys` dependency and a backend | `kAudioDevicePropertyVolumeScalar` is absent on many outputs, so the decline path matters more there |

Both remaining platforms need a system dependency baz does not currently take,
which is the difference between them and this one. The engine side is finished
for all three: a new backend is one `Sink` implementation returning `true` from
`is_exclusive`, and the readout, the negotiation, the drain and the volume
arrangement are already tested against doubles that behave exactly as those
backends would.

## What a front end needs

- **Offer the choice.** `baz_core::playback::exclusive::devices()` returns the
  list, with `pcm_name` (what to store) and `card_name`/`pcm_description` (what
  to show). Pass the chosen one as
  `OutputMode::Exclusive { device: Some(pcm_name) }` to
  `engine::spawn_device_with`. Until there is such a surface, `BAZ_OUTPUT` and
  `BAZ_OUTPUT_DEVICE` do the same job.
- **Handle the failure, because it is the common one.**
  `PlaybackError::DeviceBusy { device }` is its own variant so a front end can
  say "something else is using that device" and offer another, rather than
  matching on prose. Everything else arrives as `PlaybackError::Device` with a
  message that already lists the machine's devices where that is the problem.
- **Render the chain with `chain.is_exclusive()` and
  `chain.conversion_reason()`**, not by matching variants. Small, neutral,
  informational — ADR-0009 §5's rule, unchanged. `is_exclusive()` combined with
  no conversion and `VolumePath::is_transparent()` is the full "nothing is
  touching this" statement.
