//! The baz application: the iced shelf GUI over `baz-core` (ADR-0005).
//!
//! v0.1 scope: pick (or remember) a music folder, scan it live onto a
//! virtualized album shelf with lazy artwork, search-as-you-type, and an
//! album side panel. Playback is the next unit; its seam is the side
//! panel's documented no-op play button (see `app.rs`).
//!
//! Usage: `baz [DIR]` — `DIR` overrides (and updates) the remembered
//! music folder in `~/.config/baz/config.toml`.

use std::path::PathBuf;
use std::time::Instant;

mod app;
mod art;
mod config;
mod scan;
mod shelf;
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
