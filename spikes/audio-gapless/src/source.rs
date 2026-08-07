//! `AudioSource`: a thin, format-agnostic wrapper over Symphonia decode.
//!
//! Yields interleaved f32 blocks. Everything (WAV f32, WAV i16, FLAC) comes out
//! through the same path, so bit-exactness comparisons between engine output
//! and reference single-file decodes are apples-to-apples.

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::{Error, Result};

/// Streaming decoder for one audio file.
pub struct AudioSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: usize,
    sbuf: Option<SampleBuffer<f32>>,
}

impl AudioSource {
    /// Open and probe a file, ready to decode its default track.
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| Error::from("no default track"))?;
        let track_id = track.id;
        let sample_rate = track
            .codec_params
            .sample_rate
            .ok_or_else(|| Error::from("unknown sample rate"))?;
        let channels = track
            .codec_params
            .channels
            .ok_or_else(|| Error::from("unknown channel layout"))?
            .count();
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;
        Ok(Self {
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            sbuf: None,
        })
    }

    /// Sample rate of the stream.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Number of interleaved channels.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Decode the next packet; `Ok(None)` at end of stream. The returned slice
    /// is interleaved f32 and valid until the next call.
    pub fn next_block(&mut self) -> Result<Option<&[f32]>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(e) => return Err(e.into()),
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            let decoded = self.decoder.decode(&packet)?;
            if decoded.frames() == 0 {
                continue;
            }
            let spec = *decoded.spec();
            let needed = decoded.capacity() as u64;
            let recreate = match &self.sbuf {
                Some(b) => b.capacity() < decoded.capacity() * self.channels,
                None => true,
            };
            if recreate {
                self.sbuf = Some(SampleBuffer::new(needed, spec));
            }
            let sbuf = self.sbuf.as_mut().expect("sample buffer just ensured");
            sbuf.copy_interleaved_ref(decoded);
            return Ok(Some(sbuf.samples()));
        }
    }

    /// Convenience: decode a whole file into memory.
    /// Returns `(interleaved_samples, sample_rate, channels)`.
    pub fn decode_all(path: &Path) -> Result<(Vec<f32>, u32, usize)> {
        let mut src = Self::open(path)?;
        let mut v = Vec::new();
        while let Some(block) = src.next_block()? {
            v.extend_from_slice(block);
        }
        Ok((v, src.sample_rate, src.channels))
    }
}
