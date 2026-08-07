# ADR-0009: Bit-perfect by default — follow the source rate, reopen on change, never convert silently

**Status**: accepted (2026-08-07) · **supersedes the default chosen in [ADR-0004](0004-sample-rate-boundary.md)** (that ADR's two-mode contract survives; which mode is the default is inverted here) · based on measurements taken on the maintainer's own 24-bit/48 kHz album and on this machine's PipeWire output

## Context

ADR-0004 made *resample the incoming track to the negotiated stream rate* the default, on the reasoning that one never-closing stream buys gapless playback across every boundary, and that the conversion cost is small and confined to the prefetch thread. It also left stream-rate negotiation "an implementation detail" — and negotiation was then never implemented.

Both halves of that turned out to be wrong in practice, and the second is what exposed the first.

**What shipped.** `crates/baz/src/playback.rs` opened the output device at a hardcoded 44 100 Hz. With no negotiation, every session ran in forced-rate mode, so the owner's 24-bit/48 kHz FLAC album was sample-rate converted to 44.1 kHz — on hardware that supports 48 kHz natively. Two consequences, one measured as a bug report ("takes a while to start playing") and one silent:

- **Latency.** The forced-rate path decodes the anchor track *whole* and resamples it *whole* before the first sample is audible. Measured on `1 As for Dreams.flac` (5:24, 62 MB, 24-bit/48 kHz stereo): **`Play` → first sample delivered = 2 224 ms median** (5 runs; min 2 199, max 2 338). The same file on a same-rate stream: **0.4 ms**. The gap is a 278 ms whole-file decode materialising 124.5 MB of f32, plus a **2 290 ms** whole-file sinc resample of 15 567 236 frames.
- **Fidelity.** A 48 kHz master was being converted to 44.1 kHz for no reason at all — the device could always have played it directly. Persona P4 in `docs/research/05-personas.md` names this exactly: *"a status readout proving the chain (source rate → output rate, no resampling)"*, and *"mandatory DSP in the path"* as a bounce reason.

The owner's direction, on being shown the above: *"lets not resample anything. we want 100% accurate reproduction"* — and, on the unavoidable exception: *"it's okay if we can resample in cases where it simply won't play otherwise… but maybe worth showing a small info icon or something indicating that is happening (dont make it look like a warning as it will annoy people who are OCD about such things)."*

## Decision

1. **The output follows the source. baz resamples nothing by default.** `BoundaryPolicy::BitPerfectReopen` becomes `#[default]`; `ResampleToStreamRate` remains available as an explicit opt-in.

2. **Negotiation policy: the session's anchor track.** A session opens (or reopens) the output at the native rate of its **first playable track** — the anchor — counting from wherever the session started. The alternatives were considered and rejected:
   - *Most common rate in the queue* would have to probe every file in the queue before a single sample could play, adding start latency proportional to queue length, and would convert the very track the listener just clicked.
   - *Highest rate in the queue* would upsample the majority of a mixed queue: DSP nobody asked for, and the persona's stated bounce reason.
   - *Follow the anchor* costs exactly one header probe (0.4 ms on the owner's 62 MB FLAC), is the rate of the album the listener chose, and is the rule `run_playlist` already used — so the offline and device paths agree by construction rather than by coincidence.

3. **A rate change inside a queue ends the session and reopens the output.** The producer learns the next track's rate from its header during decode-ahead (no decode is wasted), publishes the queue index instead of pushing audio, and the engine plays the ring out, **drains** the sink so the previous track's tail is actually heard, and starts a fresh session there — which renegotiates, and so reopens. A front end sees one `QueueEnded`, at the true end; the split is an internal handover.

4. **When the device cannot do the source rate, play it anyway and say so.** `cpal`'s `supported_output_configs` is consulted (never guessed at, and filtered to stereo f32 — the only configuration the engine can feed); if the source rate is not offered, the nearest offered rate is used and the track is resampled to it. Refusing to play a file because the DAC is fussy is the wrong answer. The *silent* version of this is the one outcome ruled out.

5. **The chain is reported through the protocol.** `Event::SignalPath { source_rate_hz, source_bits, output_rate_hz, chain }` is emitted when a session starts and whenever anything about it changes — never once per track for an album that does not change. `chain` is `SignalChain::Direct` or `SignalChain::Converting { reason }`, where `ConversionReason` distinguishes `DeviceRateUnavailable` from `FixedOutputRate`. It is modelled as a state with a reason, not a boolean, because "the device has no 48 kHz mode" and "you chose a fixed output rate" are different facts and a front end that explains itself needs to tell them apart.

   **Tone is part of the decision.** This is information, not a warning. No "degraded", no "fallback", no alarm styling. A listener who cares can see it; everyone else can ignore it without being nagged.

6. **Bit depth is never silently truncated.** Every stream is opened as f32. f32's 24-bit mantissa represents every 24-bit integer PCM value exactly, so a 24-bit master reaches the device untruncated and undithered; f32 is a lossless carrier here, not a compromise. The declared source depth travels in `SignalPath.source_bits`.

## Measurements

On the owner's file (`1 As for Dreams.flac`, 5:24, 62 MB, 24-bit/48 kHz stereo), engine spawned exactly as the app spawns it (device opened at 44 100 Hz, 8192-frame ring), 5 runs each, release build, Fedora/PipeWire:

| | `Play` → first sample delivered |
|---|---|
| Before (forced 44.1 kHz, whole-track resample) | **2 224 ms** median (2 199 – 2 338) |
| After, device reopened 44.1 → 48 kHz | **12.5 ms** median (9.8 – 19.2) |
| After, device already at 48 kHz (every later track of the album) | **0.7 ms** median (0.4 – 1.1) |

**178× faster on the first track of the album, ~3 000× on the rest — and with zero sample-rate conversion where there used to be a full one.**

Rate-change gap, measured on real hardware (`device_sink_reopens_at_the_requested_rate`): tearing down the 44.1 kHz cpal stream and building a 48 kHz one takes **21.4 ms**. Re-requesting the rate already open takes **0.000 ms** — which is what makes following the source free within an album. The audible gap at a rate change is that reopen plus the first block's decode (0.5 ms on this file); the previous track's tail is drained first, so nothing is truncated.

The fallback path's cost is unchanged and now stated rather than hidden: on a device with no 48 kHz mode, the owner's file would cost 278 ms of decode plus **2 290 ms** of whole-track resample before first audio. That is the old default's number, and it is why it is no longer the default.

## Consequences

- **Gapless is unaffected within a rate**, which is the ordinary case — an album is one rate, and every boundary inside it is the same sample-accurate splice ADR-0004's Spike B verified. `gapless_wav_bit_exact`, `gapless_flac_bit_exact`, `gapless_alac_m4a_bit_exact` and the pause-bit-identical test are untouched and still pass.
- **A boundary between two *different* rates carries a ~21 ms gap.** Accepted. ADR-0004 rejected exactly this trade when the alternative was "convert everything, always"; with negotiation implemented the trade is now "a gap at the rare rate change" versus "conversion at every track", and the numbers above make it one-sided.
- **`run_playlist` refuses a mixed-rate queue** under the default (`PlaybackError::SampleRateChangeRequiresReopen`, naming the index and both rates). It fills one buffer at one rate and has no output to reopen; converting behind the caller's back is what the default exists to prevent. Callers who want one buffer at one rate select `ResampleToStreamRate`.
- **The resampler is still there and still tested.** `resample_boundary_is_continuous` still pins the −45 dB splice quality, now under the explicit opt-in — the mode is the fallback for hardware that cannot follow, so its quality still matters.
- **The claim has a boundary and the docs state it.** This is shared-mode output. What is guaranteed is that *baz* performs no conversion: the decoder's samples reach the host at the file's own rate in a lossless format. Whether PipeWire or `CoreAudio` then resamples to a graph rate it is holding for another client is outside this backend's control. Removing that last hop is what exclusive-mode backends (ALSA `hw:`, WASAPI exclusive, `CoreAudio` hog) are for, and they remain a later phase — as does `SignalPath` growing a field for it.
- **Pause is untouched.** Reopening happens only when a session *starts*; pause never starts one, so its kept-buffer, bit-identical-resume guarantee is unaffected.
- ADR-0004's engineering note about rubato's `output_delay()` — do **not** compensate for it, use anti-reflective padding — still governs `playback::resample`, which is unchanged by this ADR.

## What a front end needs

To render the indicator (deliberately not built here — the bottom bar is being restructured in parallel):

- Observe `Event::SignalPath` off the existing bridge. Fold `chain` into player state; show something small and neutral when it is `Converting`, and nothing (or the same affordance in a resting state) when it is `Direct`.
- `EngineHandle::conversions()` returns cumulative `Conversions { resampled_tracks, resample_ms, output_reconfigurations }` for a diagnostics view. All zeroes is the expected reading on hardware that can follow the music.
