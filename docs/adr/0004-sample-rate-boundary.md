# ADR-0004: Sample-rate changes at track boundaries — resample by default, reopen in bit-perfect mode

**Status**: accepted (2026-08-07) · based on Spike B measurements (`git show dc13d7e` — spikes/audio-gapless/RESULTS.md)

## Context

Gapless playback requires one continuous device stream, but consecutive tracks can differ in sample rate (44.1 kHz album → 48 kHz album). The stream can either be reconfigured ("reopen") — losing continuity — or the incoming track can be resampled to the current stream rate — losing bit-exactness. Spike B implemented and measured both.

## Measurements (Spike B, i5-12600K)

- **Gapless baseline**: engine output across a two-file split is sample-for-sample identical to the single-file reference (WAV and FLAC), verified against synthesized ground truth.
- **Reopen**: at the boundary, 7,508 frames (~170 ms) of already-buffered audio must be either discarded (hard flush) or drained (audible wait); real device reopen latency adds on top (unmeasured until exclusive-mode work lands). A gap is guaranteed.
- **Resample** (rubato `SincFixedIn`): 48 kHz → 44.1 kHz conversion of a 5 s stereo track costs 33 ms (~152× realtime) on the prefetch thread; splice continuity within 4.25e-3 max error (−45.5 dB); zero cost on same-rate boundaries (the common case).

## Decision

1. **Default**: resample the incoming track to the negotiated stream rate; the stream never closes between tracks. Gapless always.
2. **Bit-perfect/exclusive mode** (explicit user setting): never resample; reopen the device at the new rate and accept the gap. The signal-path readout shows exactly which mode is active — honesty over magic (see ENGINEERING.md principles).
3. Stream-rate negotiation (e.g. follow-first-track vs fixed user-chosen rate) is an implementation detail left to `baz-core`; the two-mode contract above is the commitment.

## Consequences

- The resampler lives on the prefetch thread — the realtime pull path stays allocation- and computation-free.
- Engineering note recorded from the spike: rubato's `output_delay()` must NOT be compensated for at the splice point — its output is already time-aligned with input frame 0 (verified by impulse test). Trusting it caused a 2.7 ms shift and an audible-scale discontinuity. Use anti-reflective padding sized to a multiple of `from/gcd(from,to)`.
- Remaining future work: MP3/AAC encoder-delay gapless trim (WAV/FLAC carry exact sample counts; lossy formats need delay/padding metadata handling), and real device-reopen latency measurement once exclusive-mode backends exist.
