# Now Playing visualizers — implementation evidence

Item 36 extends the existing optional Spectrum with two modes the existing
pre-volume audio tap can represent truthfully: a rolling Waveform and a
Spectrogram. One app-bar mark cycles Off → Spectrum → Waveform → Spectrogram →
Off. The choice remains independent of Cover / Jewel case / None and the fresh
default remains Off.

## Fixed costs

- The history is a 32-frame ring: 32 amplitude values and 32 × 24 spectral
  values, exactly 3,200 bytes of `f32` payload. It never grows.
- Waveform builds at most 32 columns. Spectrogram builds at most 32 × 24 = 768
  cells. Spectrum retains its fixed 24 bars and 24 × 256 Goertzel sample walk.
- Spectrum reads the current lock-free snapshot only. Waveform pays only its
  RMS amplitude fold; Spectrogram pays the band transform. Off pays neither.
- The existing subscription and sample-tap gates are unchanged in shape:
  visible Now Playing + a sounding record + an active visualizer (or the
  independently rotating case). Leaving the place, stopping playback or
  selecting Off removes the continuous visualizer clock and disables the tap.

These displays are signal readings, not decorative transitions. As with the
existing Spectrum, reduced-motion does not replace or interpolate the signal;
there is no autonomous drift, easing, persistence animation or movement after
the delivered samples stop. This follows the standing Now Playing reduced-
motion decision in design 12 §7.2: audio instruments remain readings, while
unrelated transition motion degrades to a hard cut.

## Prototype decision

The requested stereo-vector Oscilloscope was not shipped. The engine's existing
`VisualizationFrame` intentionally contains one mono fold of the two source
channels. Plotting that value against itself would produce a decorative
diagonal and falsely claim stereo phase information; widening the realtime
engine handoff is a separate audio-contract change, not a visualizer skin.
Particles, VU and fake vinyl remain excluded by the brief.

Verification on 2026-08-14 used the rootless `baz-dev` Toolbox. The fixed-ring,
mode-cycle, silence and tone tests pass with the complete 811-test GUI crate;
the complete core suite (including engine and index integrations) passes, and
strict all-target Clippy is clean.
