//! The headless engine of the baz music player.
//!
//! `baz-core` owns everything that is not pixels: the playback engine, the
//! library index, and the [`protocol`] through which any front end — the iced
//! GUI today, possibly a server transport later — drives it. Front ends are
//! thin clients by design (see ADR-0003): if a capability isn't reachable
//! through the protocol, it doesn't exist.
//!
//! The workspace and its quality gates (see `docs/ENGINEERING.md`) were
//! established before feature code; the engine grows behind them. Features
//! so far: the [`library`] scanner, the [`index`] it feeds, the gapless
//! [`playback`] machinery, the [`volume`] control, [`replaygain`], the
//! [`loudness`] meter and the [`analysis`] pass that computes ReplayGain for
//! files that carry none, the append-only play [`history`] ledger, the
//! [`playlist`] store that reads and writes the user's own `.m3u8` files,
//! the [`traversal`] order the engine walks a queue in, and the [`engine`]
//! service that runs it all behind the [`protocol`].
//!
//! There are **two** services, deliberately: [`engine`] plays and is given
//! paths, [`analysis`] measures and is given a library. Each takes its own
//! command vocabulary ([`protocol::Command`] and [`protocol::AnalysisCommand`])
//! so that a misrouted command is a compile error, and both announce
//! themselves on the one [`protocol::Event`] stream a front end has an event
//! loop for.

#![forbid(unsafe_code)]

pub mod analysis;
pub mod engine;
pub mod equalizer;
pub mod history;
pub mod index;
pub mod library;
pub mod loudness;
pub mod playback;
pub mod playlist;
pub mod protocol;
pub mod replaygain;
pub mod traversal;
pub mod volume;
