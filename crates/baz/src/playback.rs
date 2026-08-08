//! Engine lifecycle for the GUI: spawn at app start, bridge events into
//! iced, send commands.
//!
//! Both feature configurations expose the same [`Playback`] API so `app.rs`
//! carries no `cfg` at all:
//!
//! - **With `device-output`** (the passthrough feature on this crate):
//!   [`Playback::start`] spawns the device engine once at app start. Open
//!   failure (headless machine, no output device) is *not* fatal — the shelf
//!   works and the state machine is seeded
//!   [`Availability::NoDevice`](crate::player::Availability::NoDevice),
//!   which the bottom bar shows plainly.
//! - **Without it** (the default host build): no engine, no cpal, no ALSA
//!   headers needed. [`Playback::start`] prints a one-line stdout note and
//!   seeds [`Availability::NotBuilt`](crate::player::Availability::NotBuilt),
//!   which hides all playback UI.
//!
//! # Spawn defaults
//!
//! - **Initial sample rate 44 100 Hz**: a *starting* rate, not a policy. The
//!   engine must hold an open output from the moment it spawns, before any
//!   queue exists, and 44.1 kHz is the CD rate virtually every consumer output
//!   path accepts in shared mode. Per ADR-0009 each session then renegotiates
//!   the stream to the rate of the music it is about to play, so a 48 kHz
//!   album ends up playing at 48 kHz with no conversion — this constant only
//!   decides what the device idles at before the first click.
//! - **Nothing is resampled** by default. When a device cannot run at a
//!   source's rate the engine converts to the nearest rate it can and reports
//!   that through [`Event::SignalPath`];
//!   see "Signal path" below for what a front end does with it.
//! - **Device ring 8192 frames** (~0.19 s at 44.1 kHz): the size the engine
//!   docs describe as ordinary output latency and the size `baz-core`'s
//!   device smoke test uses. It is not merely inherited — `baz-core`'s
//!   `playback::device` module docs carry the underrun measurements that
//!   justify it against the smaller candidates, and the reason shrinking it
//!   no longer buys transport responsiveness now that abandoning a session
//!   discards the device ring outright. Engine-side config is
//!   [`EngineConfig::default`](baz_core::playback::EngineConfig) —
//!   `DeviceSink::write` provides real backpressure, so the default pump
//!   pacing is correct for device output too.
//!
//! # Signal path
//!
//! [`Event::SignalPath`] arrives on the
//! same bridge as every other engine event and carries what the chain is doing:
//! the source rate and declared bit depth, the rate the output is running at,
//! and a [`SignalChain`](baz_core::protocol::SignalChain) that is either
//! `Direct` or `Converting { reason }`. It is emitted when a session starts
//! and only when something about it changes, so every arrival is news.
//!
//! It is **information, not a warning**: `Converting` means the music is
//! playing through a sample-rate conversion because the hardware or a setting
//! requires it, which is a normal thing for a player to do. The unacceptable
//! version is the silent one, which is why the event exists. A front end that
//! surfaces it should do so the way it surfaces a codec name — legible to a
//! listener who cares, invisible to one who does not, and never styled as a
//! fault.
//!
//! # The event bridge
//!
//! The engine delivers events on a **single-consumer** `std::sync::mpsc`
//! [`Receiver`](std::sync::mpsc::Receiver) — a blocking, sync-world handle
//! that an iced subscription cannot poll directly. A dedicated
//! `baz-event-bridge` thread owns it: it blocks on `recv()` and forwards
//! every event into a `futures` unbounded channel, whose receiving half is
//! a proper `Stream` that [`Playback::subscription`] hands to
//! `Subscription::run_with_id`. When `recv()` disconnects (engine shut
//! down) the bridge emits one final [`PlayerEvent::Closed`] and exits; if
//! the UI side is dropped first, the failed forward exits the thread. The
//! stream itself is take-once out of a shared slot — iced instantiates a
//! subscription stream once per id and keeps it running, so later
//! `subscription()` calls (every update cycle) build only a cheap husk that
//! the runtime discards as a duplicate id.

use baz_core::protocol::Event;

/// What the event bridge delivers to the UI.
#[derive(Debug, Clone)]
#[cfg_attr(
    not(feature = "device-output"),
    expect(
        dead_code,
        reason = "only the device build's bridge constructs events; the no-audio \
                  build still matches on them so app.rs stays cfg-free"
    )
)]
pub enum PlayerEvent {
    /// An engine event, verbatim.
    Engine(Event),
    /// The engine shut down: its event channel disconnected. Terminal.
    Closed,
}

#[cfg(feature = "device-output")]
mod imp {
    use std::sync::{Arc, Mutex};
    use std::thread;

    use baz_core::engine::EngineHandle;
    use baz_core::playback::EngineConfig;
    use baz_core::protocol::Command;
    use baz_core::replaygain::ReplayGainState;
    use baz_core::volume::VolumeState;
    use iced::Subscription;
    use iced::futures::channel::mpsc::{UnboundedReceiver, unbounded};
    use iced::futures::stream::{self, StreamExt as _};

    use super::PlayerEvent;
    use crate::player::Availability;

    /// Rate the output device is opened at before any music is queued; every
    /// session renegotiates from there (module docs, ADR-0009).
    const INITIAL_SAMPLE_RATE: u32 = 44_100;
    /// Device ring capacity in frames (~0.19 s at 44.1 kHz; module docs).
    const DEVICE_RING_FRAMES: usize = 8192;

    /// The GUI's connection to the playback engine (`device-output` build).
    pub struct Playback {
        handle: Option<EngineHandle>,
        /// Take-once slot the subscription stream drains (module docs).
        events: Arc<Mutex<Option<UnboundedReceiver<PlayerEvent>>>>,
        availability: Availability,
    }

    impl Playback {
        /// Spawn the device engine and its event bridge. Never fails: an
        /// unusable device becomes [`Availability::NoDevice`] state instead.
        pub fn start() -> Self {
            let spawned = baz_core::engine::spawn_device(
                EngineConfig::default(),
                INITIAL_SAMPLE_RATE,
                DEVICE_RING_FRAMES,
            );
            let (handle, events) = match spawned {
                Ok(pair) => pair,
                Err(error) => {
                    println!("[playback] audio device unavailable: {error}");
                    return Self::unavailable(error.to_string());
                }
            };
            let (tx, rx) = unbounded();
            let bridge = thread::Builder::new()
                .name("baz-event-bridge".into())
                .spawn(move || {
                    while let Ok(event) = events.recv() {
                        if tx.unbounded_send(PlayerEvent::Engine(event)).is_err() {
                            return; // UI stream dropped; no one to tell.
                        }
                    }
                    // The engine dropped its sender: it has shut down.
                    let _ = tx.unbounded_send(PlayerEvent::Closed);
                });
            match bridge {
                Ok(_detached) => {
                    println!(
                        "[playback] engine ready (device opened at {INITIAL_SAMPLE_RATE} Hz, \
                         follows the source from there; device ring {DEVICE_RING_FRAMES} frames)"
                    );
                    Self {
                        handle: Some(handle),
                        events: Arc::new(Mutex::new(Some(rx))),
                        availability: Availability::Ready,
                    }
                }
                Err(error) => {
                    // No bridge means the UI would fly blind on optimistic
                    // state; shut the engine down rather than pretend.
                    handle.shutdown();
                    println!("[playback] event bridge thread failed: {error}");
                    Self::unavailable(format!("event bridge failed: {error}"))
                }
            }
        }

        fn unavailable(reason: String) -> Self {
            Self {
                handle: None,
                events: Arc::new(Mutex::new(None)),
                availability: Availability::NoDevice(reason),
            }
        }

        /// Engine availability at spawn — the seed for
        /// [`PlayerState::new`](crate::player::PlayerState::new).
        pub fn availability(&self) -> Availability {
            self.availability.clone()
        }

        /// The engine's volume right now — the one *pull* in an otherwise
        /// event-driven state machine, taken once at start-up so the fader is
        /// right on the first frame rather than on the first change (ADR-0011
        /// provides `EngineHandle::volume` for exactly this). `None` when
        /// there is no engine to ask.
        pub fn volume(&self) -> Option<VolumeState> {
            self.handle.as_ref().map(EngineHandle::volume)
        }

        /// The engine's ReplayGain state right now — the same start-up pull as
        /// [`Self::volume`], provided by ADR-0013 for the same moment, so the
        /// settings panel is right on the first frame. `None` when there is no
        /// engine to ask.
        pub fn replay_gain(&self) -> Option<ReplayGainState> {
            self.handle.as_ref().map(EngineHandle::replay_gain)
        }

        /// Send a command; `false` means the engine is gone (the caller
        /// should downgrade the state machine, never assume success).
        pub fn send(&self, command: Command) -> bool {
            self.handle
                .as_ref()
                .is_some_and(|handle| handle.send(command).is_ok())
        }

        /// Hand the engine the play ledger to append to (ADR-0018), or take it
        /// away.
        ///
        /// **The whole of a front end's involvement in history.** The engine
        /// is the only thing that knows what reached the output and for how
        /// long; a ledger written from up here would lose an album to a crash
        /// and would be written twice by two front ends attached to one
        /// engine. Reports whether there was an engine to tell — with no
        /// device there is nothing that could produce a play to record, so a
        /// `false` here is not a failure to handle.
        pub fn set_history(&self, ledger: Option<Arc<baz_core::history::HistoryLedger>>) -> bool {
            match &self.handle {
                Some(handle) => {
                    handle.set_history(ledger);
                    true
                }
                None => false,
            }
        }

        /// The bridge's event stream as an iced subscription (module docs).
        pub fn subscription(&self) -> Subscription<PlayerEvent> {
            if self.handle.is_none() {
                return Subscription::none();
            }
            let slot = Arc::clone(&self.events);
            let events = stream::once(async move {
                let taken = slot.lock().ok().and_then(|mut slot| slot.take());
                match taken {
                    Some(rx) => rx.boxed(),
                    None => stream::empty().boxed(),
                }
            })
            .flatten();
            Subscription::run_with_id("baz-playback-events", events)
        }
    }
}

#[cfg(not(feature = "device-output"))]
mod imp {
    use baz_core::protocol::Command;
    use baz_core::replaygain::ReplayGainState;
    use baz_core::volume::VolumeState;
    use iced::Subscription;

    use super::PlayerEvent;
    use crate::player::Availability;

    /// The no-audio stand-in (default host build): same API, no engine.
    pub struct Playback;

    #[expect(
        clippy::unused_self,
        reason = "method-for-method API parity with the device-output Playback"
    )]
    impl Playback {
        /// Print the one-line build note; there is nothing to spawn.
        pub fn start() -> Self {
            println!("built without audio output — see docs/DEVELOPMENT.md");
            Self
        }

        /// Always [`Availability::NotBuilt`] — playback UI stays hidden.
        pub fn availability(&self) -> Availability {
            Availability::NotBuilt
        }

        /// No engine, no volume to read.
        pub fn volume(&self) -> Option<VolumeState> {
            None
        }

        /// No engine, no ReplayGain to read.
        pub fn replay_gain(&self) -> Option<ReplayGainState> {
            None
        }

        /// No engine: every send is refused. (Unreachable in practice —
        /// the UI that would send is hidden.)
        pub fn send(&self, _command: Command) -> bool {
            false
        }

        /// No engine, so nothing can produce a play to record. The ledger is
        /// still opened by the shell (and still read, for the PLAYED key) —
        /// this build simply has nothing to append to it.
        pub fn set_history(
            &self,
            _ledger: Option<std::sync::Arc<baz_core::history::HistoryLedger>>,
        ) -> bool {
            false
        }

        /// No engine, no events.
        pub fn subscription(&self) -> Subscription<PlayerEvent> {
            Subscription::none()
        }
    }
}

pub use imp::Playback;
