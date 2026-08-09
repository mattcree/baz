//! The baz application: the iced shelf GUI over `baz-core` (ADR-0005).
//!
//! v0.1 scope: pick (or remember) a music folder, scan it live onto a
//! virtualized album shelf with lazy artwork, search-as-you-type, a dismissible
//! album inspector beside it, an **Queue** popover anchored to the
//! now-playing bar, a Settings place beside the Library, and — with the
//! `device-output` feature — album playback
//! through `baz-core`'s engine (see `playback.rs` and `player.rs`; without
//! the feature the app builds everywhere with playback UI hidden).
//!
//! The interface's shape is ADR-0016's: **one place at a time, one inspector
//! attached to that place, one popover attached to the transport, and the
//! now-playing bar always** — `place`, `selection`, `overlay` and
//! `views::bottom_bar` respectively.
//!
//! The modules follow ADR-0006's three layers: pure state and logic
//! (`vm`, `player`, `place`, `selection`, `overlay`, `motion`, `queue_edit`,
//! `replaygain`, `shelf`, `config`, `scan`, `art`, `mpris::state` — no iced
//! imports), design tokens (`theme`, over the typeface `font` bundles), and view
//! composition (`views/`,
//! one module per surface) with the application shell that drives it
//! (`app`, and beside it `keys` and `mpris`, which produce the shell's
//! messages from a keyboard and from the desktop respectively). Three controls
//! are hand-built `Widget`s rather than view composition — `groove` (the
//! volume fader) and `needle` (the queue's seek line) over the pointer
//! machinery they share (`pointer`), and `spine` (the index rail's fisheye
//! lane), which reads the pointer's position rather than its gestures and so
//! shares nothing; ADR-0017 §5 records why that is now the norm for anything
//! with pointer semantics rather than the exception ADR-0005 treated it as.
//!
//! Usage: `baz [DIR]` — `DIR` overrides (and updates) the remembered
//! music folder in `~/.config/baz/config.toml`.

use std::path::PathBuf;
use std::time::Instant;

mod app;
mod art;
mod config;
mod drag;
mod font;
mod groove;
mod icon;
mod keys;
mod menu;
mod motion;
mod mpris;
mod needle;
mod place;
mod playback;
mod player;
mod playlists;
mod pointer;
mod queue_edit;
mod queue_window;
mod rail;
mod replaygain;
mod scan;
mod shelf;
mod shuffle;
mod spine;
mod theme;
mod undo;
mod views;
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
        // The `baz DIR` teaching lived on the first-run screen until doc 11
        // §5 P1 moved it here, where its audience lives: someone reading
        // --help is exactly the person the sentence is for.
        println!("       MUSIC_DIR points baz at a folder for this run and is remembered;");
        println!("       without it, baz opens the folders it already knows.");
        return Ok(());
    }
    app::run(started, arg.map(PathBuf::from))
}
