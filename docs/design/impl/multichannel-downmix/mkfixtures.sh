#!/usr/bin/env bash
# Multichannel fixtures for ADR-0039, one distinct tone per speaker.
#
# The point of a per-speaker tone is that a downmix which put the centre
# channel in a surround still produces a plausible waveform of the right
# length: only an independent tone per channel makes "which one landed where"
# a measurable question.
#
#   ./mkfixtures.sh [outdir]      (needs ffmpeg)
#
# `join`'s mapping is given explicitly. Left to guess it — which is what
# happens when the inputs are mono, since every mono stream's one channel is
# called FC — ffmpeg silently produces a different layout than asked for, and
# then every measurement taken from the result is measuring the fixture.
set -euo pipefail
out="${1:-fixtures}"
mkdir -p "$out"
rate=48000
dur=0.5
# ffmpeg's `sine` source has no amplitude option and emits a peak of 0.125
# (1/8 of full scale), so every level here is set with an explicit `volume`
# rather than assumed. Measured, not remembered: `ffprobe`/`astats` on a bare
# `sine` output reads 0.125.
sine_peak=0.125
amp=0.3
gain=$(python3 -c "print($amp / $sine_peak)")
# Slot order is the mask's ascending bit order, which is also WAVE's
# interleave order: FL FR FC LFE BL/SL BR/SR.
tones=(400 700 1100 1700 2300 3100)
for i in "${!tones[@]}"; do
  ffmpeg -y -v error -f lavfi \
    -i "sine=frequency=${tones[$i]}:sample_rate=$rate:duration=$dur,volume=$gain" \
    -c:a pcm_f32le "$out/slot$i.wav"
done
mono() { printf -- "-i %s/slot%s.wav " "$out" "$1"; }

join6() { # $1=layout $2=map $3=output
  # shellcheck disable=SC2046
  ffmpeg -y -v error $(mono 0) $(mono 1) $(mono 2) $(mono 3) $(mono 4) $(mono 5) \
    -filter_complex "[0:a][1:a][2:a][3:a][4:a][5:a]join=inputs=6:channel_layout=$1:map=$2[a]" \
    -map "[a]" -c:a pcm_f32le "$3"
}
join4() {
  # shellcheck disable=SC2046
  ffmpeg -y -v error $(mono 0) $(mono 1) $(mono 2) $(mono 3) \
    -filter_complex "[0:a][1:a][2:a][3:a]join=inputs=4:channel_layout=$1:map=$2[a]" \
    -map "[a]" -c:a pcm_f32le "$3"
}

join6 "5.1"       "0.0-FL|1.0-FR|2.0-FC|3.0-LFE|4.0-BL|5.0-BR" "$out/tones_51.wav"
join6 "5.1(side)" "0.0-FL|1.0-FR|2.0-FC|3.0-LFE|4.0-SL|5.0-SR" "$out/tones_51_side.wav"
join4 "quad"      "0.0-FL|1.0-FR|2.0-BL|3.0-BR"                "$out/tones_quad.wav"

# The same music in four containers. Each stores 5.1 in its own channel order,
# which is the variation a downmix built on an assumption gets wrong.
ffmpeg -y -v error -i "$out/tones_51.wav" -c:a flac                "$out/tones_51.flac"
ffmpeg -y -v error -i "$out/tones_51.wav" -c:a libvorbis -q:a 8    "$out/tones_51.ogg"
ffmpeg -y -v error -i "$out/tones_51.wav" -c:a alac                "$out/tones_51_alac.m4a"
ffmpeg -y -v error -i "$out/tones_51.wav" -c:a aac -b:a 320k       "$out/tones_51_aac.m4a"

# Layouts ITU-R BS.775 does not place: baz must refuse these, not fold them.
ffmpeg -y -v error -i "$out/tones_51.wav" -af "pan=7.1|FL=FL|FR=FR|FC=FC|LFE=LFE|BL=BL|BR=BR|SL=BL|SR=BR" \
  -c:a pcm_f32le "$out/tones_71.wav"
ffmpeg -y -v error -i "$out/tones_51.wav" -af "pan=6.1|FL=FL|FR=FR|FC=FC|LFE=LFE|BC=FC|SL=BL|SR=BR" \
  -c:a pcm_f32le "$out/tones_61.wav"

# The clipping case: full scale, one tone, in phase in all six channels. Six
# *different* tones at full scale would not clip — they do not add coherently —
# so the worst case has to be built deliberately.
full=$(python3 -c "print(1.0 / $sine_peak)")
ffmpeg -y -v error -f lavfi -i "sine=frequency=400:sample_rate=$rate:duration=$dur,volume=$full" \
  -af "pan=5.1|FL=c0|FR=c0|FC=c0|LFE=c0|BL=c0|BR=c0" -c:a pcm_f32le "$out/clip_51.wav"

ls -l "$out"
