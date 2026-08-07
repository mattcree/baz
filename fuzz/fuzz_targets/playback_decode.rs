//! Fuzz the playback decoder wrapper: arbitrary bytes fed through the
//! Symphonia probe/decode path must produce `Ok` or `Err`, never a panic.
//! Sibling of `protocol_deserialize.rs` per docs/ENGINEERING.md: every
//! parser that touches external file bytes gets a fuzz target — media
//! parsers process hostile input.
//!
//! Coverage tracks whatever codecs baz-core enables: the probe here
//! registers every format/decoder in baz-core's symphonia feature set, so
//! enabling MP3 there put the whole MPEG-audio demux/decode path (including
//! the Xing/LAME gapless-trim parsing) under this target. That MP3 bytes
//! probe successfully with no extension hint — i.e. that this entry point
//! really reaches the MP3 path — is asserted by
//! `mp3_decoded_length_is_exact` in crates/baz-core/tests/playback.rs.
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
