//! Fuzz the playback decoder wrapper: arbitrary bytes fed through the
//! Symphonia probe/decode path must produce `Ok` or `Err`, never a panic.
//! Sibling of `protocol_deserialize.rs` per docs/ENGINEERING.md: every
//! parser that touches external file bytes gets a fuzz target — media
//! parsers process hostile input.
//!
//! Coverage tracks whatever codecs baz-core enables: the probe here
//! registers every format/decoder in baz-core's symphonia feature set, so
//! enabling MP3 there put the whole MPEG-audio demux/decode path (including
//! the Xing/LAME gapless-trim parsing) under this target, and enabling
//! `isomp4`/`aac`/`alac` added the ISO-MP4 demuxer — atom tree walking,
//! sample tables (`stsz`/`stsc`/`stco`/`stts`), the `esds`
//! AudioSpecificConfig and the ALAC magic cookie — plus both MP4 decoders.
//! That is a large new attack surface reached from the same entry point:
//! MP4 is a nested, length-prefixed, table-driven container, which is
//! precisely the shape of parser that rewards fuzzing.
//!
//! `AudioSource::open_bytes` passes no extension hint, so each format has to
//! probe by *content* for this target to reach it at all. That MP3 and MP4
//! bytes do is asserted, not assumed, by `mp3_decoded_length_is_exact`,
//! `alac_m4a_is_lossless_and_length_exact` and
//! `aac_m4a_delay_is_untrimmed_and_measured` in
//! crates/baz-core/tests/playback.rs, each of which drives a real encoded
//! file through this same hint-less path.
#![no_main]

use baz_core::playback::AudioSource;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(mut src) = AudioSource::open_bytes(data.to_vec()) else {
        return;
    };
    // Bound the work per input: enough blocks to exercise mid-stream decode
    // and the trim window without letting a crafted header demand unbounded
    // output.
    for _ in 0..64 {
        match src.next_block() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => break,
        }
    }
});
