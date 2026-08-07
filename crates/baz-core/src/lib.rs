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
//! [`playback`] machinery, and the [`engine`] service that runs it behind
//! the [`protocol`].

#![forbid(unsafe_code)]

pub mod engine;
pub mod index;
pub mod library;
pub mod playback;
pub mod protocol;
