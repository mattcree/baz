//! Fuzz the M3U playlist parser (ADR-0024): arbitrary bytes must parse to a
//! value, never a panic — a playlist is a file the user edits by hand and
//! imports from other players, so its reader parses external bytes and gets
//! a target like every other one (docs/ENGINEERING.md).
//!
//! The second assertion is the module's round-trip law: whatever the bytes,
//! parse → render → parse is a fixed point, and the rendered bytes are
//! stable from then on — a rewrite can never lose what a read preserved.
#![no_main]

use std::path::Path;

use baz_core::playlist;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Any absolute base does; the real caller passes the playlist file's
    // own directory.
    let base = Path::new("/fuzz/playlists");
    let first = playlist::parse(data, base);
    let bytes = playlist::render(&first);
    let second = playlist::parse(&bytes, base);
    assert_eq!(first, second, "parse(render(parse(f))) must be a fixed point");
    assert_eq!(playlist::render(&second), bytes, "the rewrite must be idempotent");
});
