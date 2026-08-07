//! The baz application: the iced shelf GUI over `baz-core` (ADR-0005).
//!
//! v0.1 scope: pick (or remember) a music folder, scan it live onto a
//! virtualized album shelf with lazy artwork, search-as-you-type, an album
//! side panel, and — with the `device-output` feature — album playback
//! through `baz-core`'s engine (see `playback.rs` and `player.rs`; without
//! the feature the app builds everywhere with playback UI hidden).
//!
//! Usage: `baz [DIR]` — `DIR` overrides (and updates) the remembered
//! music folder in `~/.config/baz/config.toml`.

use std::path::PathBuf;
use std::time::Instant;

mod app;
mod art;
mod config;
mod playback;
mod player;
mod scan;
mod seek;
mod shelf;
mod theme;
mod vm;

fn main() -> iced::Result {
    let started = Instant::now();
    let arg = std::env::args_os().nth(1);
    if let Some(text) = arg.as_ref().and_then(|a| a.to_str())
        && matches!(text, "-h" | "--help" | "-V" | "--version")
    {
        println!(
            "baz {} — a music player for people who own their music",
            env!("CARGO_PKG_VERSION")
        );
        println!("usage: baz [MUSIC_DIR]");
        return Ok(());
    }
    app::run(started, arg.map(PathBuf::from))
}
