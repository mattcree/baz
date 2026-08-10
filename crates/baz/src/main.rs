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
mod field;
mod font;
mod groove;
mod icon;
mod implicit;
mod keys;
mod lane;
mod menu;
mod motion;
mod mpris;
mod needle;
mod origin;
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
mod session;
mod shelf;
mod spine;
mod theme;
mod undo;
mod views;
mod vm;

/// **The window presents without waiting for the vertical blank, unless the
/// listener says otherwise.**
///
/// The owner reported dragging an edge as treacle three times over two days.
/// It was measured and it is **not** baz's work: a resize step costs 0.18 ms
/// at 25 records and 0.44 ms at 400, against 16.7 ms of a 60 Hz frame, with
/// 8–9× headroom and no decode on the path at all
/// (`docs/design/impl/resize-cost/`). The cost was in *presentation*, and the
/// three-way test on his own machine settled which part: `tiny-skia`
/// (no wgpu at all) — snappy; wgpu with `mailbox` — snappy; wgpu as shipped —
/// treacle. The default was `Fifo`, which blocks on the vertical blank, and a
/// drag generates resize events far faster than a monitor refreshes.
///
/// **Why `AutoNoVsync` and not `Mailbox`, which is what actually fixed it.**
/// iced asks wgpu for the named mode *literally*, and wgpu panics if the
/// surface does not offer it — his machine refused `Immediate` exactly that
/// way while this was being diagnosed, and the app died before drawing.
/// `AutoNoVsync` is the only form that cannot do that: wgpu picks the best
/// available and is documented to fall back to `Fifo`. baz is built for three
/// platforms and the interface has been run on one, so a default that fails
/// where nobody has tested it is not a default (`docs/BACKLOG.md`, the owner's
/// *"we need mac os and windows compat eventually"*).
///
/// **The listener keeps the last word**: an `ICED_PRESENT_MODE` already in the
/// environment is left exactly as it is, so `fifo` returns the old behaviour
/// for anyone who prefers a vsync-locked window, and every value iced
/// documents still works.
///
/// # Safety, and the first `unsafe` in this crate
///
/// [`std::env::set_var`] is unsafe in edition 2024 because another thread
/// reading the environment concurrently is a data race. This runs on the first
/// line of `main`, before iced, before the engine's threads, and before
/// anything in this process has read an environment variable — there is no
/// second thread in existence to race with. That is the textbook sanctioned
/// use, and it is why the standard library kept the operation available at all
/// rather than removing it.
///
/// **The workspace denies `unsafe_code`** (`Cargo.toml`), and this is the GUI
/// crate's first exception, so it is worth saying why it is taken rather than
/// worked around. iced 0.13 offers **no other lever**: the present mode is read
/// from this variable inside `iced_wgpu`'s compositor
/// (`iced_wgpu-0.13.5/src/window/compositor.rs:281`) and is not reachable
/// through `iced::Settings`, `window::Settings` or anything else the crate
/// exposes. The alternatives were setting it in the Flatpak manifest and the
/// desktop entry — which fixes the *packaged* baz and leaves anyone running the
/// binary, the owner included, with the treacle — or re-executing the process,
/// which is worse than one line of documented `unsafe`.
///
/// **This should stop being unsafe rather than stay excepted.** If baz takes
/// iced 0.14 (already on the table, for `window::drag_resize`), check whether
/// the present mode is settable through the API there and delete this.
#[expect(
    unsafe_code,
    reason = "no API for the present mode in iced 0.13; single-threaded, \
              first statement of main, before any thread exists"
)]
fn prefer_no_vsync() {
    if std::env::var_os("ICED_PRESENT_MODE").is_none() {
        // SAFETY: single-threaded, first statement of `main`. See above.
        unsafe { std::env::set_var("ICED_PRESENT_MODE", "no_vsync") };
    }
}

fn main() -> iced::Result {
    prefer_no_vsync();
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

#[cfg(test)]
mod present_mode_tests {
    /// **A value already in the environment is the listener's and is kept.**
    ///
    /// The whole of the default's contract: it fills an absence, it never
    /// overrides a choice. Asserted rather than assumed because the failure is
    /// silent — a listener who set `fifo` to stop tearing would simply not get
    /// it, and would have no way to tell from the outside.
    ///
    /// The two cases run in one test on purpose: the environment is process-
    /// wide, and two `#[test]`s touching it race each other under the default
    /// threaded harness.
    #[expect(
        unsafe_code,
        reason = "the environment is process-wide; both cases are kept in one \
                  test so no sibling test races this one"
    )]
    #[test]
    fn the_default_fills_an_absence_and_never_overrides_a_choice() {
        // SAFETY: this test owns the variable for its duration; the cases are
        // kept in one test so no sibling test can be running against it.
        unsafe { std::env::set_var("ICED_PRESENT_MODE", "fifo") };
        super::prefer_no_vsync();
        assert_eq!(
            std::env::var("ICED_PRESENT_MODE").as_deref(),
            Ok("fifo"),
            "the default overrode a listener's own setting"
        );

        unsafe { std::env::remove_var("ICED_PRESENT_MODE") };
        super::prefer_no_vsync();
        assert_eq!(
            std::env::var("ICED_PRESENT_MODE").as_deref(),
            Ok("no_vsync"),
            "an unset present mode did not get the default that fixed the drag"
        );

        unsafe { std::env::remove_var("ICED_PRESENT_MODE") };
    }
}
