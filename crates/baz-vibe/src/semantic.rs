//! Local LAION CLAP inference and its exact audio preprocessing contract.

use std::f32::consts::TAU;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use baz_core::playback::{DecodedAudio, resample_interleaved};
use ort::session::Session;
use ort::value::Tensor;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

const RATE: u32 = 48_000;
const CHANNELS: usize = 2;
const WINDOW_SAMPLES: usize = 480_000;
const WINDOW_STEP: usize = WINDOW_SAMPLES / 2;
const MAX_WINDOWS: usize = 6;
const FFT_SIZE: usize = 1_024;
const HOP: usize = 480;
const MEL_BINS: usize = 64;
const FRAMES: usize = WINDOW_SAMPLES / HOP + 1;
const EMBEDDING_SIZE: usize = 512;

pub(crate) struct Model {
    audio: Session,
    text: Session,
    tokenizer: Tokenizer,
}

static MODEL: Mutex<Option<Model>> = Mutex::new(None);

pub(crate) fn embed_text(prompt: &str) -> Result<Vec<f32>, String> {
    with_model(|model| model.text(prompt))
}

pub(crate) fn embed_audio(decoded: &DecodedAudio) -> Result<Vec<f32>, String> {
    with_model(|model| model.audio(decoded))
}

fn with_model<T>(run: impl FnOnce(&mut Model) -> Result<T, String>) -> Result<T, String> {
    let mut guard = MODEL
        .lock()
        .map_err(|_| "the local Vibe model stopped unexpectedly".to_owned())?;
    if guard.is_none() {
        *guard = Some(Model::load()?);
    }
    run(guard.as_mut().expect("model inserted above"))
}

impl Model {
    pub(crate) fn load() -> Result<Self, String> {
        let directory = model_directory().ok_or_else(|| {
            "The bundled local Vibe model could not be found. Reinstall Baz's full build."
                .to_owned()
        })?;
        let session = |name: &str| {
            let builder = Session::builder()
                .map_err(|error| format!("could not start local Vibe model: {error}"))?;
            let mut builder = builder
                .with_intra_threads(2)
                .map_err(|error| format!("could not configure local Vibe model: {error}"))?;
            builder
                .commit_from_file(directory.join(name))
                .map_err(|error| format!("could not open local Vibe model: {error}"))
        };
        let mut tokenizer = Tokenizer::from_file(directory.join("tokenizer.json"))
            .map_err(|error| format!("could not open local Vibe vocabulary: {error}"))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: 77,
                ..TruncationParams::default()
            }))
            .map_err(|error| format!("could not configure local Vibe vocabulary: {error}"))?;
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::Fixed(77),
            pad_id: 1,
            pad_token: "<pad>".to_owned(),
            ..PaddingParams::default()
        }));
        Ok(Self {
            audio: session("audio_model_quantized.onnx")?,
            text: session("text_model_quantized.onnx")?,
            tokenizer,
        })
    }

    pub(crate) fn text(&mut self, prompt: &str) -> Result<Vec<f32>, String> {
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|error| format!("could not read the Vibe request: {error}"))?;
        let ids: Vec<i64> = encoding
            .get_ids()
            .iter()
            .map(|value| i64::from(*value))
            .collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|value| i64::from(*value))
            .collect();
        let output = self
            .text
            .run(ort::inputs! {
                "input_ids" => Tensor::from_array(([1_usize, 77], ids))
                    .map_err(|error| error.to_string())?,
                "attention_mask" => Tensor::from_array(([1_usize, 77], mask))
                    .map_err(|error| error.to_string())?,
            })
            .map_err(|error| format!("local Vibe text inference failed: {error}"))?;
        let (_, values) = output[0]
            .try_extract_tensor::<f32>()
            .map_err(|error| format!("local Vibe text output was invalid: {error}"))?;
        normalized(values)
    }

    pub(crate) fn audio(&mut self, decoded: &DecodedAudio) -> Result<Vec<f32>, String> {
        let stereo = resample_interleaved(&decoded.samples, decoded.sample_rate, RATE)
            .map_err(|error| format!("could not resample track for local Vibe: {error}"))?;
        let mono: Vec<f32> = stereo
            .chunks_exact(CHANNELS)
            .map(|frame| (frame[0] + frame[1]) * 0.5)
            .collect();
        let mut vectors = Vec::new();
        for start in sampled_starts(mono.len()) {
            let features = mel_window(&mono, start);
            let output = self
                .audio
                .run(ort::inputs! {
                    "input_features" => Tensor::from_array(
                        ([1_usize, 1, FRAMES, MEL_BINS], features),
                    ).map_err(|error| error.to_string())?,
                })
                .map_err(|error| format!("local Vibe audio inference failed: {error}"))?;
            let (_, values) = output[0]
                .try_extract_tensor::<f32>()
                .map_err(|error| format!("local Vibe audio output was invalid: {error}"))?;
            vectors.push(normalized(values)?);
        }
        let mut mean = vec![0.0; EMBEDDING_SIZE];
        for vector in &vectors {
            for (total, value) in mean.iter_mut().zip(vector) {
                *total += value;
            }
        }
        normalized(&mean)
    }
}

fn model_directory() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("BAZ_VIBE_MODEL_DIR").map(PathBuf::from),
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join("models/vibe"))),
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/vibe")),
        Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tools/vibe-eval/local/laion-reproduced"),
        ),
    ];
    candidates.into_iter().flatten().find(|path| {
        path.join("audio_model_quantized.onnx").is_file()
            && path.join("text_model_quantized.onnx").is_file()
            && path.join("tokenizer.json").is_file()
    })
}

fn sampled_starts(samples: usize) -> Vec<usize> {
    let last = samples.saturating_sub(WINDOW_SAMPLES);
    let all: Vec<_> = (0..=last).step_by(WINDOW_STEP).collect();
    if all.len() <= MAX_WINDOWS {
        return if all.is_empty() { vec![0] } else { all };
    }
    (0..MAX_WINDOWS)
        .map(|index| index * (all.len() - 1) / (MAX_WINDOWS - 1))
        .map(|index| all[index])
        .collect()
}

#[expect(
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    reason = "CLAP's bounded FFT indices and sample positions fit exactly in these numeric domains"
)]
fn mel_window(audio: &[f32], start: usize) -> Vec<f32> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let window: Vec<_> = (0..FFT_SIZE)
        .map(|index| 0.5 - 0.5 * (TAU * index as f32 / FFT_SIZE as f32).cos())
        .collect();
    let filters = mel_filters();
    let mut output = Vec::with_capacity(FRAMES * MEL_BINS);
    let mut spectrum = vec![Complex::default(); FFT_SIZE];
    for frame in 0..FRAMES {
        let centre = start as isize + (frame * HOP) as isize;
        for (index, value) in spectrum.iter_mut().enumerate() {
            let position = reflect(
                centre + index as isize - (FFT_SIZE / 2) as isize,
                audio.len(),
            );
            *value = Complex::new(
                audio.get(position).copied().unwrap_or(0.0) * window[index],
                0.0,
            );
        }
        fft.process(&mut spectrum);
        for filter in &filters {
            let power = spectrum[..=FFT_SIZE / 2]
                .iter()
                .zip(filter)
                .map(|(bin, weight)| bin.norm_sqr() * weight)
                .sum::<f32>();
            output.push(10.0 * power.max(1e-10).log10());
        }
    }
    output
}

#[expect(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "audio buffer positions are bounded by addressable memory and rem_euclid is non-negative"
)]
fn reflect(position: isize, length: usize) -> usize {
    if length <= 1 {
        return 0;
    }
    let edge = length as isize - 1;
    let period = edge * 2;
    let folded = position.rem_euclid(period);
    if folded <= edge {
        folded as usize
    } else {
        (period - folded) as usize
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "the 64 mel bins and 513 FFT bins are exactly representable in f32"
)]
fn mel_filters() -> Vec<Vec<f32>> {
    let to_mel = |hz: f32| {
        let linear = hz / (200.0 / 3.0);
        if hz < 1_000.0 {
            linear
        } else {
            15.0 + (hz / 1_000.0).ln() / (6.4_f32.ln() / 27.0)
        }
    };
    let from_mel = |mel: f32| {
        if mel < 15.0 {
            mel * (200.0 / 3.0)
        } else {
            1_000.0 * ((6.4_f32.ln() / 27.0) * (mel - 15.0)).exp()
        }
    };
    let low = to_mel(50.0);
    let high = to_mel(14_000.0);
    let points: Vec<_> = (0..MEL_BINS + 2)
        .map(|index| low + (high - low) * index as f32 / (MEL_BINS + 1) as f32)
        .map(from_mel)
        .collect();
    (0..MEL_BINS)
        .map(|mel| {
            let scale = 2.0 / (points[mel + 2] - points[mel]);
            (0..=FFT_SIZE / 2)
                .map(|bin| bin as f32 * RATE as f32 / FFT_SIZE as f32)
                .map(|hz| {
                    let lower = (hz - points[mel]) / (points[mel + 1] - points[mel]);
                    let upper = (points[mel + 2] - hz) / (points[mel + 2] - points[mel + 1]);
                    lower.min(upper).max(0.0) * scale
                })
                .collect()
        })
        .collect()
}

fn normalized(values: &[f32]) -> Result<Vec<f32>, String> {
    if values.len() != EMBEDDING_SIZE {
        return Err(format!(
            "local Vibe returned {} values, expected {EMBEDDING_SIZE}",
            values.len()
        ));
    }
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= f32::EPSILON {
        return Err("local Vibe returned an empty semantic vector".to_owned());
    }
    Ok(values.iter().map(|value| value / norm).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_windows_cover_the_track_including_both_ends() {
        let starts = sampled_starts(WINDOW_SAMPLES * 10);
        assert_eq!(starts.len(), MAX_WINDOWS);
        assert_eq!(starts[0], 0);
        assert_eq!(starts.last(), Some(&(WINDOW_SAMPLES * 9)));
    }

    #[test]
    #[expect(
        clippy::cast_precision_loss,
        reason = "the bounded fixture positions are exactly representable in f32"
    )]
    fn mel_shape_and_values_are_bounded() {
        let audio: Vec<_> = (0..WINDOW_SAMPLES)
            .map(|index| index as f32 / RATE as f32)
            .map(|time| 0.3 * (TAU * 440.0 * time).sin() + 0.1 * (TAU * 3_300.0 * time).sin())
            .collect();
        let values = mel_window(&audio, 0);
        assert_eq!(values.len(), FRAMES * MEL_BINS);
        assert!(values.iter().all(|value| value.is_finite()));
        let references = [
            (0, 4.538_053_5),
            (10, 2.710_411_5),
            (MEL_BINS + 10, -25.151_02),
            (500 * MEL_BINS + 40, -48.180_176),
            (63, -42.057_13),
        ];
        for (index, expected) in references {
            assert!(
                (values[index] - expected).abs() < 0.08,
                "mel[{index}] was {}, expected {expected}",
                values[index]
            );
        }
    }

    #[test]
    #[ignore = "requires the separately verified local model artifact set"]
    fn reproduced_model_embeds_an_ordinary_request() {
        let mut model = Model::load().expect("local reproduced model");
        let vector = model
            .text("restless late-night electronic music, becoming warmer")
            .expect("text inference");
        assert_eq!(vector.len(), EMBEDDING_SIZE);
        let norm = vector.iter().map(|value| value * value).sum::<f32>();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[test]
    #[ignore = "requires the separately verified local model artifact set"]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "the bounded sine fixture is deliberately quantized to signed 16-bit wave samples"
    )]
    fn reproduced_model_embeds_local_audio() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("tone.wav");
        let specification = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, specification).expect("wave");
        for index in 0..WINDOW_SAMPLES {
            let time = index as f32 / RATE as f32;
            let sample = (0.4 * (TAU * 440.0 * time).sin() * f32::from(i16::MAX)) as i16;
            writer.write_sample(sample).expect("left");
            writer.write_sample(sample).expect("right");
        }
        writer.finalize().expect("finalize");
        let mut model = Model::load().expect("local reproduced model");
        let decoded = baz_core::playback::AudioSource::decode_all(&path).expect("decode fixture");
        let vector = model.audio(&decoded).expect("audio inference");
        assert_eq!(vector.len(), EMBEDDING_SIZE);
        assert!(vector.iter().all(|value| value.is_finite()));
    }
}
