//! MPRIS2 desktop integration: baz in the shell's media controls, and
//! hardware media keys routed to the transport.
//!
//! On Linux the desktop shell — GNOME's lock screen and quick-settings
//! media widget, KDE's Media Player applet, `playerctl`, and the media-key
//! daemons that sit behind Play/Pause/Next on a keyboard — all speak one
//! protocol: [MPRIS2] over the D-Bus session bus. This module serves the two
//! interfaces that protocol is made of, `org.mpris.MediaPlayer2` and
//! `org.mpris.MediaPlayer2.Player`, at `/org/mpris/MediaPlayer2` under the
//! well-known name `org.mpris.MediaPlayer2.baz`.
//!
//! [MPRIS2]: https://specifications.freedesktop.org/mpris-spec/latest/
//!
//! # Shape
//!
//! Same shape as [`crate::playback`], and for the same reason — so `app.rs`
//! carries no `cfg` at all:
//!
//! - [`Mpris::start`] spawns a `baz-mpris` thread that owns the D-Bus
//!   connection and the two served objects. It returns immediately and
//!   cannot fail (see "When there is no bus" below).
//! - [`Mpris::publish`] hands the thread a fresh [`state::Snapshot`] after
//!   every engine event. The thread stores it and emits `PropertiesChanged`
//!   for exactly the properties that differ from the last one it published —
//!   never for `Position`, which the spec says is polled rather than
//!   signalled.
//! - [`Mpris::subscription`] is the return path: every D-Bus method call
//!   becomes a [`Request`], which `app.rs` maps to the *same*
//!   [`Message`](crate::app::Message) the corresponding on-screen control
//!   emits. `PlayPause` from the lock screen and a click on the bottom bar's
//!   toggle are the same message, take the same update-loop arm, and reach
//!   the engine as the same [`Command`](baz_core::protocol::Command).
//!
//! On every other platform the same three methods exist and do nothing.
//!
//! # The honesty rule crosses the bus unchanged
//!
//! Everything MPRIS reports is derived from engine events by
//! [`state::Snapshot::from_player`], which reads [`crate::player`]'s
//! event-derived state and nothing else. There is no optimistic path: a
//! `Play` arriving over D-Bus sends a command and changes no reported state
//! until the engine says so, exactly as a click on the toggle does.
//!
//! `Position` deserves the explicit note. It is the last
//! [`Progress`](baz_core::protocol::Event::Progress) reading — accurate to
//! the engine's ~4 Hz cadence — and is **not** extrapolated with a wall
//! clock between reports. Clients that want a smooth scrubber interpolate it
//! themselves, which is what the spec expects them to do; what they get from
//! us is what baz actually knows.
//!
//! # Album art
//!
//! `mpris:artUrl` is set **only** when the album has a cover file on disk
//! (`cover.jpg`, `folder.jpg`, … — [`crate::art`]'s second resolution step),
//! pointed at with a `file://` URL. Art that exists only as bytes embedded
//! in a tag gets no URL at all.
//!
//! The alternative was to decode the embedded picture and write it to a
//! cache file so a URL could exist. That was weighed and rejected: it means
//! writing megabytes into the user's cache directory as a side effect of
//! pressing play, inventing a lifetime policy for those files, and handing
//! the shell a path to a file baz made up rather than to the user's own
//! artwork. A missing thumbnail in a media widget is a small cost; the
//! honest version of "here is your cover" is a file the user actually has.
//! (MPRIS has no way to pass image *bytes*, so there is no third option.)
//!
//! # When there is no bus
//!
//! MPRIS is an enhancement, never a requirement. There is no session bus
//! inside a minimal container, on a headless build machine, or under a
//! `dbus-run-session`-less test harness — and baz must run there exactly as
//! it does on a desktop. So:
//!
//! - The connection is opened on the `baz-mpris` thread, not on the way to
//!   the first frame, so a slow or absent bus cannot delay startup.
//! - Every failure — no `DBUS_SESSION_BUS_ADDRESS`, a refused socket, a
//!   name already owned by something else, a serve that will not register —
//!   prints one `[mpris]` line to stdout and ends the thread. Never a
//!   modal, never a dialog, never a non-zero exit.
//! - The app keeps its sender; publishing into a dead channel is a no-op.
//!   The player works, the shelf works, only the desktop integration is
//!   absent.
//!
//! If the well-known name is already taken (a second baz), the thread falls
//! back to the spec's per-instance spelling,
//! `org.mpris.MediaPlayer2.baz.instance<pid>`, so two copies can run without
//! either of them going silent.
//!
//! # `Previous`
//!
//! Served, and `CanGoPrevious` reports rather than refuses. This was once the
//! first entry under "deliberately not implemented", on the grounds that
//! advertising a control we cannot honour is worse than not having one — and
//! the grounds were right while they lasted: the front end had no button, no
//! key, and nothing to send. The gap was never in the protocol.
//! `Command::Previous` has always been there, with the engine's
//! restart-versus-step-back rule (three seconds in) already specified, and now
//! there is a `|◀` beside the play button, <kbd>Ctrl</kbd>+<kbd>←</kbd>, and
//! this. All three are the same [`Message`](crate::app::Message), so the lock
//! screen's Previous and the bar's are one press.
//!
//! `CanGoPrevious` follows
//! [`PlayerState::previous_enabled`](crate::player::PlayerState::previous_enabled),
//! which is `CanGoNext`'s condition — both are relative commands and both are
//! documented engine no-ops while stopped. It is not `CanGoNext`'s *value*,
//! though, and the difference is real: `Next` runs out at the end of a queue
//! while `Previous` never does, because at the head of the queue it restarts
//! the track rather than declining.
//!
//! # What is deliberately not implemented
//!
//! - **`SupportedUriSchemes` / `SupportedMimeTypes`**: both empty, because
//!   `OpenUri` returns `NotSupported`. baz plays what it scanned; opening an
//!   arbitrary URI is a feature, not a property, and listing schemes we
//!   would refuse is the same lie in a different place.
//! - **`LoopStatus`, `Shuffle`**: optional in the spec and unimplemented in
//!   baz, so they are absent rather than present-and-fixed.
//! - **`Rate`**: read-only `1.0`, with `MinimumRate` and `MaximumRate` pinned
//!   to `1.0`. baz plays at the source's rate and has no rate control
//!   (ADR-0009); a writable property that silently discarded writes would be
//!   worse than an error.
//!
//! # `Volume`
//!
//! Read *and* write, since ADR-0011. Three decisions worth stating, because
//! MPRIS leaves all three open:
//!
//! - **The unit is bridged, never re-invented.** MPRIS's `Volume` is a linear
//!   amplitude where 1.0 is normal; baz's is an integer control position on a
//!   cubic taper. [`state::volume_amplitude`] and
//!   [`state::position_for_amplitude`] are the only crossing, and both go
//!   through `baz_core::volume`'s curve or its exact inverse — so "half
//!   volume" from the lock screen and half-travel on the fader mean the same
//!   sound, which is the whole reason ADR-0011 put the taper in `baz-core`.
//! - **The reading is the *effective* level.** MPRIS has no mute, so a muted
//!   player reports `0.0` — what is actually coming out — rather than where
//!   the fader is sitting. The alternative advertises a player at full volume
//!   that makes no sound.
//! - **Writing a level above zero unmutes.** A person dragging a media
//!   widget's volume up is asking to hear something, and leaving them at
//!   silence while the number climbs would be the writable-property-that-does-
//!   nothing this module refuses elsewhere. It costs one extra `SetMute`, sent
//!   only when the player is actually muted. Writing `0.0` sets the *position*
//!   to zero and does not mute: position 0 is a real place on the control that
//!   survives a mute round trip (ADR-0011 §3).
//!
//! A write is refused with `NotSupported` when `CanControl` is false, which is
//! what the spec asks for and what the rest of this interface already does by
//! never offering a control it cannot honour.
//!
//! # The dependency
//!
//! `zbus` 5, the pure-Rust D-Bus implementation (MIT), declared as a
//! Linux-only target dependency. Two things made it the choice over
//! `mpris-server`, which wraps it:
//!
//! 1. **It adds nothing.** zbus 5 is already linked into every Linux baz
//!    binary by iced 0.14's Linux accessibility stack. Depending on it directly
//!    unifies with that copy and pulls in zero duplicate crates, zero new
//!    licenses, and no C or system library (the whole stack — zvariant,
//!    `zbus_names`, `enumflags2`, `async-io` — is pure Rust; baz's Linux build
//!    stays free of system dependencies as ADR-0005 intends).
//! 2. **`mpris-server` still buys little here.** What it would save is a
//!    typed metadata builder and the trait skeleton — perhaps eighty lines,
//!    against two interfaces whose entire surface is a few properties and
//!    eight methods.

#[cfg_attr(
    not(target_os = "linux"),
    allow(
        dead_code,
        reason = "the MPRIS reading is compiled — and unit-tested — on every platform in the CI \
                  matrix so the microsecond arithmetic and the spec's status strings are checked \
                  wherever the tests run; only the Linux build has a bus to spend them on"
    )
)]
pub(crate) mod state;

#[cfg(target_os = "linux")]
mod server;

pub(crate) use state::Snapshot;

/// The basename of `packaging/io.github.mattcree.baz.desktop`.
///
/// One constant with two customers, deliberately: it is MPRIS's
/// `DesktopEntry` property *and* the window's Wayland `app_id` / X11
/// `WM_CLASS` (set in [`crate::app`]). A desktop associates a running window
/// with a launcher entry by matching exactly these two against the file name,
/// so they cannot be allowed to drift apart.
///
/// The value is the reverse-DNS Flatpak application id (ADR-0002), not the
/// bare `baz` this once was: Flatpak requires the desktop entry, the AppStream
/// component id and the manifest to share one id, and a portal-facing
/// `app_id` that disagrees with it is what makes a sandboxed window lose its
/// icon. The MPRIS *bus* name stays `org.mpris.MediaPlayer2.baz` — that one is
/// the spec's, not the desktop's.
#[cfg(target_os = "linux")]
pub(crate) const DESKTOP_ENTRY: &str = "io.github.mattcree.baz";

/// A D-Bus method call, in baz's vocabulary rather than MPRIS's.
///
/// The bridge stops here: `app.rs` turns each of these into the same
/// [`Message`](crate::app::Message) the equivalent on-screen control emits,
/// so there is exactly one path from an intention to the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(target_os = "linux"),
    allow(
        dead_code,
        reason = "only the Linux server constructs requests; every platform still maps them, so \
                  the mapping to interface messages compiles and is tested everywhere"
    )
)]
pub(crate) enum Request {
    /// `Play` — start or resume. Not a toggle.
    Play,
    /// `Pause`.
    Pause,
    /// `PlayPause` — the toggle, resolved against the confirmed phase.
    PlayPause,
    /// `Stop`.
    Stop,
    /// `Next`.
    Next,
    /// `Previous` — step back, or restart the current track (the engine's
    /// three-second rule decides which, and it is the engine's to decide).
    Previous,
    /// `Seek(offset)`, converted to whole milliseconds; may be negative.
    SeekBy(i64),
    /// `SetPosition`, already checked against the current track id and
    /// converted to milliseconds from the start of the track.
    SeekTo(u64),
    /// A `Volume` write, already mapped through `baz-core`'s taper to a
    /// control position.
    SetVolume(u16),
    /// Unmute, because a `Volume` write above zero arrived while muted (see
    /// the module docs). Never sent to mute — MPRIS has no way to ask.
    SetMute(bool),
    /// **`Shuffle`**, written as a property — a stated value, never a toggle
    /// (see `App::set_shuffle` for why the distinction is load-bearing).
    SetShuffle(bool),
    /// **`LoopStatus`**, written as a property. The spec's three strings map
    /// onto baz's own three states exactly: `None` is off, `Track` repeats the
    /// completed track, and `Playlist` re-walks the run's traversal.
    SetRepeat(baz_core::protocol::Repeat),
    /// `Raise` — bring the window forward. Best effort: a Wayland compositor
    /// may decline, which is its right.
    Raise,
    /// `Quit`.
    Quit,
}

#[cfg(target_os = "linux")]
pub(crate) use server::Mpris;

#[cfg(not(target_os = "linux"))]
pub(crate) use absent::Mpris;

/// The stand-in for every platform without MPRIS: identical API, no thread,
/// no bus, nothing to go wrong.
#[cfg(not(target_os = "linux"))]
mod absent {
    use iced::Subscription;

    use super::{Request, Snapshot};

    /// No-op MPRIS integration (non-Linux builds).
    pub(crate) struct Mpris;

    #[expect(
        clippy::unused_self,
        reason = "method-for-method API parity with the Linux Mpris, so app.rs carries no cfg"
    )]
    impl Mpris {
        /// Nothing to start.
        pub(crate) fn start() -> Self {
            Self
        }

        /// Nothing to publish to.
        pub(crate) fn publish(&self, _snapshot: Snapshot, _seeked: bool) {}

        /// No bus, no requests.
        pub(crate) fn subscription(&self) -> Subscription<Request> {
            Subscription::none()
        }
    }
}
