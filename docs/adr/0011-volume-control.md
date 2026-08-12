# ADR-0011: Volume — a cubic control position, software gain, and an honest readout

> **Direct-manipulation amendment (2026-08-12).** Vertical wheel travel over
> the live fader now feeds the exact `PlayerState::step_volume` path used by
> Up/Down. Line deltas are notches; high-resolution pixel travel accumulates at
> 32 px per step. The fader captures even sub-step/horizontal events so neither
> page scroll nor Ctrl+density leaks through, while the same event elsewhere is
> untouched. The mute button is outside the target and mute stays independent:
> scrolling while muted prepares the restored level without unmuting. Engine
> confirmation remains truth, with confirmed wheel changes coalesced behind a
> 240 ms quiet boundary before persistence rather than written per delta.

**Status**: accepted (2026-08-07) · **amends the guarantee stated in [ADR-0009](0009-follow-the-source-rate.md)** (that ADR's decisions all stand; what changes is that "bit-exact" is now the conjunction of two facts instead of one) · resolves the volume entry in `docs/BACKLOG.md` · measurements taken on the maintainer's own machine (Fedora/PipeWire 1.6.7, iFi HD USB DAC)

## Context

The backlog carried a volume slider as *wanted, but not to be built naively*:

> Scaling samples in software is by definition no longer bit-perfect (and at
> 16-bit it costs real resolution unless dithered). The resolution to design,
> not assume: prefer **device/hardware volume** where the backend exposes it,
> so the stream stays untouched; fall back to software gain only when it
> doesn't, and say so through the existing `Event::SignalPath` mechanism […]
> A "unity / bit-perfect" position on the control should be reachable and
> obvious.

That is the right shape of answer, and the first half of it turns out not to be
available. This ADR records what was actually found, what shipped, and exactly
which sentence of ADR-0009 stops being true on its own.

## The device-volume investigation

**cpal exposes no volume API.** Confirmed rather than assumed: `grep -ri volume`
over cpal 0.16.0's entire `src/` returns nothing. So a hardware volume means
platform-specific code below cpal, and the question is what that code could
reach.

**Linux, as measured on this machine.** cpal's ALSA backend opens the PCM
literally named `"default"` (`host/alsa/enumerate.rs`: `pcm_id: "default"`), and
on any PipeWire or PulseAudio desktop that is the sound server's ALSA bridge,
not a card. Its mixer says so:

```
$ amixer -D default scontrols
Simple mixer control 'Master',0
Simple mixer control 'Capture',0
```

That `Master` is **PipeWire's own system volume** — currently 80 % / −5.78 dB on
this machine — applied as a software multiply inside the graph. Driving it from
baz's slider would move every other application's volume with it, and would not
be bit-exact anyway. It is the wrong control in two independent ways.

The *right* hardware control does exist, one layer down. The owner's DAC has a
genuine USB Audio Class feature-unit attenuator:

```
$ amixer -c 3 cget numid=4
numid=4,iface=MIXER,name='iFi (by AMR) HD USB Audio  Playback Volume'
  ; type=INTEGER,access=rw---R--,values=2,min=0,max=32512,step=0
  | dBminmax-min=-127.00dB,max=0.00dB
```

−127 dB of real analogue-domain attenuation, and using it would leave the digital
stream untouched. Three things stop baz from reaching for it today:

1. **It is card-wide, not per-application.** Moving it changes the volume of
   every program playing to that DAC. A media player's own slider must not do
   that.
2. **baz cannot tell which card it is playing to.** cpal reports the device name
   as the string `"default"` and nothing more; PipeWire chose card 3 for reasons
   invisible from inside the process, and can reroute at any moment. Guessing a
   card from `/proc/asound/cards` and driving its mixer would be acting on an
   assumption the audio stack does not license.
3. **It is not general.** The same machine's other sinks have no equivalent —
   card 0's S/PDIF output offers `IEC958` switches and no attenuator at all, and
   HDMI has nothing.

**The per-application volume that does exist is not bit-exact either.** PipeWire
does expose a per-stream volume (`pactl list sink-inputs` shows one per client),
and that is the semantically correct control. But it is a float multiply inside
PipeWire's mixer: using it would not preserve one more bit than doing the
multiply ourselves. It would only move the arithmetic out of our process, in
exchange for a libpipewire or libpulse dependency and a Linux-only backend —
spending baz's zero-system-dependency property (`docs/BACKLOG.md`, the Opus
entry) for no fidelity gain whatsoever.

**Windows and macOS are the same shape.** `IAudioEndpointVolume` is an *endpoint*
(device-wide) control, the exact counterpart of the ALSA card mixer; the
per-application one is `ISimpleAudioVolume` on the WASAPI session, which is
again the mixer's own software gain, and reaching either means an
`IAudioClient` cpal does not hand out plus a `windows`/`windows-sys` dependency.
CoreAudio's `kAudioDevicePropertyVolumeScalar` is device-wide and is simply
absent on many outputs (HDMI, most USB DACs in their default mode).

**The conclusion, stated plainly.** *In shared mode there is no bit-exact
per-application volume to reach for on any platform.* The controls that are
per-application are software gain in someone else's process; the controls that
are hardware belong to the whole system. Hardware volume becomes a real option
exactly when baz owns the card — which is the **exclusive-mode output** phase
ADR-0009 already defers (ALSA `hw:`, WASAPI exclusive, CoreAudio hog). It is the
same deferral, and it now has a second reason to happen.

### Cost, so the owner can price it

| | Cost | Buys |
|---|---|---|
| **Ship software gain behind an abstraction** (what this ADR does) | ~0 new deps; the fader is 60 lines | A working volume control, honestly labelled |
| Linux per-stream volume via PipeWire/Pulse | libpipewire *or* libpulse system dep; Linux-only backend; ~a week with the routing edge cases | **Nothing.** Still a float multiply, just elsewhere |
| Linux hardware volume via ALSA mixer | The `alsa` crate is **already an indirect dependency** through cpal on Linux (0.9.1 in `Cargo.lock`, `alsa-sys` already links `libasound`), so this is a direct-dependency line and a `cfg(target_os)` module, not a new build requirement. ~2 days | Bit-exactness — but on the **wrong control**: card-wide, and only when baz can identify the card, which in shared mode it cannot |
| Windows / macOS hardware volume | New `windows`/`windows-sys` or `coreaudio-sys` dependency per platform; ~1 week each | Same: device-wide, not per-app |
| **Hardware volume under exclusive mode** | The exclusive-mode phase itself (large, already deferred by ADR-0009) | The real thing: baz owns the card, so driving its attenuator is legitimate *and* the volume is genuinely baz's |

The only row that buys the thing the backlog asked for is the last, and it is
gated on work that is already scheduled behind a bigger decision. So: ship
software gain now, behind the seam the device path will slot into.

## Decision

1. **The control's unit is an integer position, `0..=1000`, not an amplitude
   and not decibels.** `Command::SetVolume { position: u16 }`. Integer for the
   reasons `protocol.rs` already argues for `position_ms` and the seek work
   already settled on: one canonical JSON encoding so `wire_format_is_stable`
   tests the protocol rather than a float formatter, and `Command`/`Event` keep
   their `Eq`. 1000 steps is ~0.06 dB per step at the top — two orders of
   magnitude below audibility and finer than a pointer can drive a slider.

2. **The taper is a cube, defined in `baz-core`.** `amplitude = (position/1000)³`
   — the classical 60 dB fader law (10 % of travel is −60 dB, half is
   −18.06 dB). It lives in `baz_core::volume` so that every front end, including
   any future remote transport, means the same thing by "half way up"; a
   linear-amplitude slider feels wrong to a human, so *some* correction happens
   somewhere, and one shared answer beats several private ones.

   It was chosen over a dB-linear taper for a structural reason as well as feel:
   **a cube reaches exactly 0 and exactly 1 with no special case.** `1000/1000`
   is `1.0f32` exactly and `1.0³` is `1.0` exactly, so the top of the travel is
   unity *provably*, not to within a rounding error — which is what lets the
   engine recognise unity with `==`. A dB-linear law approaches silence
   asymptotically and needs a hard-coded "below this, actually zero" branch at
   the bottom of the control, a hidden discontinuity exactly where a listener
   drags.

3. **Mute is a separate command and separate state, not gain zero.**
   `Command::SetMute { muted: bool }` — idempotent rather than a toggle, the
   same choice `Seek` makes and for the same reason. Mute must remember the
   position it will restore; encoding it as position 0 would destroy that,
   forcing every front end to keep a shadow copy and letting two front ends on
   one engine disagree about what unmuting does. Position 0 remains a real,
   distinct thing: *this is how loud I want it*, and it survives a mute round
   trip.

4. **Software gain, applied on the pump path, with a slew.** The gain is applied
   in `Session::pump` between the ring read and the sink write — the one place
   every sample passes exactly once whatever route it took (streamed anchor,
   prefetched track, resampled fallback), so it cannot be bypassed by a path
   added later. Applying it in the producer instead would bake it into decoded
   audio that outlives the setting by a ring's worth of buffering, which is how
   a volume control comes to feel late.

   Realtime discipline (`docs/ENGINEERING.md`, "the audio thread is sacred"):
   **one atomic load and one branch per block**, one multiply per sample, into a
   `Box<[f32]>` allocated when the engine thread starts — a boxed slice rather
   than a `Vec` so that "never grows" is forbidden by the type rather than asked
   for by a comment. Changes slew at a constant rate of full scale per 20 ms,
   monotonically and without overshoot (each frame steps a fixed signed amount
   and is clamped at the target), so a drag produces no zipper noise. The slew is
   skipped when nothing is audible — before a session starts, while paused —
   because a step in silence is not a click.

5. **Unity is a structural short circuit, not a multiply by one.** When the
   fader is at rest at exactly `1.0`, `pump` hands the ring's slices to the sink
   *without copying or scaling them* — the same code, instruction for
   instruction, that existed before volume control did. Bit-exactness at unity
   is therefore a property of the control flow, not of floating-point identity.
   (`x * 1.0 == x` for every finite `x` anyway; but "we do not multiply" is a
   claim that survives someone moving the code, and "we multiply by one and it
   happens to be a no-op" is one that has to be re-checked every time.)

6. **The volume is engine state, not session state.** Pause, resume, seek, skip,
   queue replacement, a track boundary and a sample-rate reopen all leave it
   untouched by construction. The one thing the engine does on its own
   initiative is **re-offer the gain to the sink after the output is reopened**,
   because a reopened stream is a new stream and carries none of the old one's
   settings.

7. **The device path is a `Sink` method, not a stub.**
   `Sink::set_device_volume(gain) -> Option<()>`, shaped deliberately like
   `Sink::negotiate_rate`: the engine asks, the sink answers, and the engine
   reports whichever answer it got. `Some(())` means the sink took the gain, so
   baz leaves the fader at unity and the stream stays bit-exact; `None` — the
   default, and the answer from **every backend baz ships** — means software
   gain. Choosing a trait method over an unimplemented `VolumeControl::Device`
   enum variant matters: the branch is reachable by a test double *today*, so
   the engine's half of the arrangement is tested before the backend exists
   rather than asserted to work when it does.

8. **The readout is `Event::VolumeChanged { position, muted, path }`**, where
   `path` is `VolumePath::Unity` | `SoftwareGain` | `DeviceAttenuator`, and
   `VolumePath::is_transparent()` answers the fidelity question directly.

   **It is a separate event from `Event::SignalPath`, not a field on it**, and
   the reason is cadence: `SignalPath` describes the *format* chain and is
   emitted once per session, so folding volume in would restate a track's whole
   format every time somebody nudged a slider. (There is a second, contingent
   reason: `Event`'s variants are not individually `#[non_exhaustive]` and the
   GUI destructures `SignalPath`'s fields exhaustively, so adding a field there
   is a source break. If both readouts are ever wanted in one message, that
   destructuring gains a `..` first.)

   **Tone is part of the decision**, exactly as in ADR-0009 §5. `SoftwareGain`
   is what every ordinary player does with a volume control and describes a
   perfectly good listening experience. No "degraded", no "fallback", no
   warning styling. The unacceptable version is the *silent* one.

## How this amends ADR-0009

ADR-0009 established that when the chain is `SignalChain::Direct`, baz converts
nothing and the decoder's samples reach the output exactly. **That sentence is
still true about sample rate, and is no longer the whole story about the
samples.**

Precisely:

- **Before**: bit-exact ⟺ `SignalChain::Direct`.
- **After**: bit-exact ⟺ `SignalChain::Direct` **and**
  `VolumePath::is_transparent()` (i.e. `Unity` or `DeviceAttenuator`).

Nothing about rate handling changes; a second, independent gain stage now exists
and is reported on its own channel. Both `Event::SignalPath` and
`Event::VolumeChanged` carry that rule in their docs, and `is_transparent()`
exists so a front end asks about the property rather than enumerating whichever
variants happen to have it.

**The default is unaffected.** A freshly spawned engine is at unity, unmuted,
`VolumePath::Unity` — so a listener who never touches the volume gets exactly
what ADR-0009 promised, and `EngineHandle::volume()` says so without being asked
twice.

## Measurements

Software gain, on this machine, release build:

| | |
|---|---|
| Throughput | **0.2 ns per sample** — 600 s of 44.1 kHz stereo scaled in **13.2 ms**, i.e. **45 000× realtime** |
| Worst-case error, f32 multiply vs. the exact product, swept over `[-1, 1]` at 1e-6 granularity | **−150.5 dBFS** (2.98e-8 absolute) at gains 0.512–0.997 |
| Analytic bound | half an f32 ULP at full scale = 5.96e-8 = **−144.5 dBFS** |
| Gains that are powers of two (position 500 → 0.125) | **exactly zero error** — which is why `half_travel_scales_by_exactly_one_eighth` can assert `==` |

−144.5 dBFS is ~120 dB below the noise floor of the best 24-bit recording and
about 60 dB below the threshold of hearing referenced to a full-scale playback
level. **It is inaudible by any measure a listener could apply — and it is still
not identical**, which is the whole reason it is reported rather than glossed.

The CPU cost is not a consideration at 45 000× realtime; it is recorded only so
that "cheap" is a number rather than an adjective.

Slew: 20 ms for a full-travel change (882 frames at 44.1 kHz), proportionally
less for a smaller one, pinned by
`a_mid_playback_volume_change_ramps_monotonically_and_drops_nothing` and
`the_slew_is_monotonic_and_completes_on_time`.

## Consequences

- **A listener who wants bit-perfect has to leave the volume at the top**, and
  the control says so. That is an honest trade rather than a hidden one, and it
  is the same trade every bit-perfect player makes.
- **16-bit sources are not dithered on the way down.** Undither is a real
  omission and it is stated rather than hidden: baz decodes to f32 and hands f32
  to the host, so nothing is *truncated* to 16 bits anywhere in baz's path — the
  quantisation that would justify dither happens downstream, at whatever depth
  the device runs. If an exclusive-mode backend later emits integer samples at
  the source's own depth, dither becomes a live question and belongs to that
  work.
- **No gain above unity.** baz attenuates and does not amplify: the loudest baz
  plays a file is exactly as loud as the file. Makeup gain is a
  ReplayGain-shaped question and belongs with ReplayGain.
- **Software gain is `f32` and stays `f32`.** No wider intermediate: the sink
  takes f32, so computing in f64 and rounding back would add a rounding rather
  than remove one.
- **The device-volume seam exists and is tested.** `Sink::set_device_volume` has
  a real implementation path through the engine — offer, fall back, report,
  re-offer after a reopen — exercised by `AttenuatingDouble` in
  `engine.rs`'s tests. Landing a backend is implementing one method, not
  designing the arrangement.
- **`docs/BACKLOG.md`'s volume entry is resolved**, and gains a successor: the
  hardware-volume half is now an explicit consequence of the exclusive-mode
  phase rather than an open question.
- **The GUI slider is deliberately not built here.** The protocol, the taper,
  the engine and the readout are; the control is a parallel unit. What a front
  end needs is below.

## What a front end needs

- **Send** `Command::SetVolume { position }` with `position` in
  `0..=baz_core::volume::MAX_POSITION` (1000). Do *not* apply a curve of your
  own — `Volume::amplitude` is the shared one, and `Volume::decibels` is there
  if you want to label the control in dB. Mute is `Command::SetMute { muted }`,
  separately.
- **Observe** `Event::VolumeChanged { position, muted, path }` and follow it
  rather than your own optimistic value, so two front ends on one engine agree.
  Redundant commands emit nothing, so an event is always news.
- **Read** `EngineHandle::volume() -> VolumeState` once at start-up, for the
  state before anybody changes anything.
- **Render** `MAX_POSITION` as a reachable, obvious detent — it is the position
  at which baz touches nothing, and the backlog asked for it to be findable.
  `path.is_transparent()` combined with a `SignalChain::Direct` from
  `Event::SignalPath` is the "this is bit-exact right now" indicator; render it
  the way ADR-0009 §5 asks — small, neutral, informational, never a warning.
