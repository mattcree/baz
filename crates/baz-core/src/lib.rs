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
//! so far: the [`library`] scanner and the [`index`] it feeds.

#![forbid(unsafe_code)]

pub mod index;
pub mod library;
pub mod protocol;
