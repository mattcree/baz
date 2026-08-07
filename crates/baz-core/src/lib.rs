//! The headless engine of the baz music player.
//!
//! `baz-core` owns everything that is not pixels: the playback engine, the
//! library index, and the [`protocol`] through which any front end — the iced
//! GUI today, possibly a server transport later — drives it. Front ends are
//! thin clients by design (see ADR-0003): if a capability isn't reachable
//! through the protocol, it doesn't exist.
//!
//! This crate is currently a skeleton: the workspace and its quality gates
//! (see `docs/ENGINEERING.md`) were established before feature code, and the
//! engine grows behind them.

#![forbid(unsafe_code)]

pub mod protocol;
