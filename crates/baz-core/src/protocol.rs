//! The command/event protocol between the engine and its front ends.
//!
//! Every message is serde-serializable so that the in-process GUI and any
//! future remote transport speak the same language (ADR-0003). Both enums are
//! `#[non_exhaustive]`: front ends must tolerate messages they don't know,
//! which is what lets the protocol grow without breaking older clients.
//!
//! The engine that executes [`Command`]s and emits [`Event`]s lives in
//! [`crate::engine`]; that module's docs are the authoritative description of
//! each message's runtime semantics (what `Play` does while paused, when
//! `TrackStarted` fires, and so on). This module defines only the wire shape.
//!
//! # Wire format
//!
//! JSON with an internal tag (`"cmd"` for commands, `"event"` for events) and
//! `snake_case` variant names, e.g. `{"cmd":"set_queue","paths":["/a.flac"]}`.
//! The `wire_format_is_stable` test pins one encoding per variant; changing
//! any of them is a protocol break and must be a deliberate, versioned
//! decision. (One such break was taken pre-0.1: the skeleton events
//! `playback_started`/`playback_paused` were replaced by the richer
//! per-track vocabulary below before any client existed.)
//!
//! Paths travel as [`PathBuf`]. In-process transports move them losslessly;
//! JSON serialization requires them to be valid UTF-8 (serde errors on
//! non-UTF-8 paths rather than corrupting them), a constraint any future
//! remote transport inherits.
//!
//! # Time on the wire: integer milliseconds
//!
//! Every duration and position in this protocol is an **unsigned integer
//! count of milliseconds** (`u64`), never floating-point seconds. Three
//! reasons, in order of weight:
//!
//! 1. **One canonical encoding.** A byte-pinned stability test
//!    (`wire_format_is_stable`) is only meaningful if a value has exactly one
//!    serialization. `1`, `1.0`, and `1.0000000000000002` are all plausible
//!    JSON renderings of the same `f64` across serializers and languages; an
//!    integer has one. The pinned bytes therefore test the protocol rather
//!    than `serde_json`'s float formatter.
//! 2. **The enums stay `Eq`.** Both [`Command`] and [`Event`] derive `Eq`,
//!    which every test in the workspace leans on (`assert_eq!` on whole
//!    events) and which any future de-duplication or replay logic would
//!    want. `f64` cannot derive `Eq` — `NaN` is a legal `f64` and a legal
//!    JSON-decoded value — so seconds-as-`f64` would have meant deleting a
//!    working guarantee from the public API.
//! 3. **The resolution is free.** One millisecond is ~44 samples at 44.1 kHz:
//!    two orders of magnitude finer than the [`Event::Progress`] cadence and
//!    finer than any seek a pointing device can express. `u64` milliseconds
//!    span ~5·10⁸ years, so saturation is not a concern.
//!
//! Front ends that want seconds divide by 1000 at the presentation edge,
//! which is where rounding belongs.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A request from a front end to the engine.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Replace the play queue with `paths`. Does not start playback; any
    /// playback in progress stops (the engine emits [`Event::Stopped`]).
    SetQueue {
        /// The new queue, in play order.
        paths: Vec<PathBuf>,
    },
    /// Start playback of the current queue position, or resume if paused.
    Play,
    /// Pause playback, keeping the current position. No-op when not playing.
    Pause,
    /// Stop playback and abandon the current run through the queue. The
    /// queue itself is kept; a later [`Command::Play`] starts from the top.
    Stop,
    /// Skip to the next track in the queue. Past the last track this ends
    /// the queue ([`Event::QueueEnded`]).
    Next,
    /// Jump to an absolute position within the **currently playing track**
    /// and keep the transport state (playing stays playing, paused stays
    /// paused — see [`crate::engine`] for the runtime contract).
    ///
    /// # Range and clamping
    ///
    /// - Below zero is unrepresentable: the field is unsigned, so "seek
    ///   before the start" clamps to 0 by construction.
    /// - At or past the end of the current track the engine treats the seek
    ///   as [`Command::Next`]: the following queue position starts from its
    ///   beginning, or the queue ends ([`Event::QueueEnded`]) if there is no
    ///   following position. It is *not* clamped to the last moment of the
    ///   track — stalling on the final frame is not a state any listener
    ///   asks for.
    /// - While stopped it is a no-op (there is no current track), like
    ///   [`Command::Next`].
    Seek {
        /// Target position from the start of the current track, in
        /// milliseconds (module docs explain the unit).
        position_ms: u64,
    },
}

/// A notification from the engine to its front end.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A track's audio began reaching the sink.
    TrackStarted {
        /// The file being played.
        path: PathBuf,
        /// Zero-based position of the track in the queue.
        position: usize,
    },
    /// Playback paused; no further audio reaches the sink until resumed.
    Paused,
    /// Playback resumed exactly where it paused.
    Resumed,
    /// Playback stopped ([`Command::Stop`], or a queue replacement while
    /// playing).
    Stopped,
    /// The queue finished: every track played, failed, or was skipped.
    QueueEnded,
    /// A track could not be played and was skipped; the queue continues
    /// with the next track (one bad file never kills the queue).
    TrackFailed {
        /// The file that failed.
        path: PathBuf,
        /// Human-readable description of the failure.
        reason: String,
    },
    /// Where playback is inside the current track.
    ///
    /// # Cadence
    ///
    /// Roughly **4 Hz while audio is flowing** — the engine emits one every
    /// quarter-second *of delivered audio*, not of wall time, so the rate is
    /// tied to the stream rather than to a clock — **plus one immediately
    /// after** [`Event::TrackStarted`], [`Event::Resumed`], and every
    /// accepted [`Command::Seek`]. Those three extras are what keep a front
    /// end from ever showing a stale position after a transport action.
    ///
    /// No `Progress` is emitted while paused (the position is not moving) or
    /// while stopped (there is no position); [`Event::Paused`],
    /// [`Event::Stopped`], and [`Event::QueueEnded`] are the transitions
    /// that say so.
    Progress {
        /// Position within the current track, in milliseconds, clamped to
        /// `track_ms` when that is known.
        elapsed_ms: u64,
        /// Total length of the current track in milliseconds, when the
        /// container declares one. `None` for streams whose length is not
        /// known before decoding (an MP3 with no Xing/Info header, say) — a
        /// front end must render that case rather than invent a duration.
        track_ms: Option<u64>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_commands() -> Vec<Command> {
        vec![
            Command::SetQueue {
                paths: vec![
                    PathBuf::from("/music/a.flac"),
                    PathBuf::from("/music/b.wav"),
                ],
            },
            Command::Play,
            Command::Pause,
            Command::Stop,
            Command::Next,
            Command::Seek { position_ms: 0 },
            Command::Seek {
                position_ms: 93_500,
            },
        ]
    }

    fn sample_events() -> Vec<Event> {
        vec![
            Event::TrackStarted {
                path: PathBuf::from("/music/a.flac"),
                position: 3,
            },
            Event::Paused,
            Event::Resumed,
            Event::Stopped,
            Event::QueueEnded,
            Event::TrackFailed {
                path: PathBuf::from("/music/broken.flac"),
                reason: "decode error: oops".into(),
            },
            Event::Progress {
                elapsed_ms: 0,
                track_ms: None,
            },
            Event::Progress {
                elapsed_ms: 93_500,
                track_ms: Some(214_000),
            },
        ]
    }

    #[test]
    fn command_json_roundtrip() {
        for cmd in sample_commands() {
            let json = serde_json::to_string(&cmd).expect("serialize");
            let back: Command = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(cmd, back);
        }
    }

    #[test]
    fn event_json_roundtrip() {
        for event in sample_events() {
            let json = serde_json::to_string(&event).expect("serialize");
            let back: Event = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(event, back);
        }
    }

    #[test]
    fn wire_format_is_stable() {
        // The wire format is a public contract; a change here is a protocol
        // break and must be a deliberate, versioned decision. Every variant
        // of both enums is pinned.
        let cases: Vec<(String, &str)> = vec![
            (
                serde_json::to_string(&Command::SetQueue {
                    paths: vec![PathBuf::from("/music/a.flac")],
                })
                .expect("serialize"),
                r#"{"cmd":"set_queue","paths":["/music/a.flac"]}"#,
            ),
            (
                serde_json::to_string(&Command::Play).expect("serialize"),
                r#"{"cmd":"play"}"#,
            ),
            (
                serde_json::to_string(&Command::Pause).expect("serialize"),
                r#"{"cmd":"pause"}"#,
            ),
            (
                serde_json::to_string(&Command::Stop).expect("serialize"),
                r#"{"cmd":"stop"}"#,
            ),
            (
                serde_json::to_string(&Command::Next).expect("serialize"),
                r#"{"cmd":"next"}"#,
            ),
            (
                serde_json::to_string(&Command::Seek {
                    position_ms: 93_500,
                })
                .expect("serialize"),
                r#"{"cmd":"seek","position_ms":93500}"#,
            ),
            (
                // Zero must encode as `0`, not `0.0` or `-0`: the integer
                // choice is exactly what makes this assertable (module docs).
                serde_json::to_string(&Command::Seek { position_ms: 0 }).expect("serialize"),
                r#"{"cmd":"seek","position_ms":0}"#,
            ),
            (
                serde_json::to_string(&Event::TrackStarted {
                    path: PathBuf::from("/music/a.flac"),
                    position: 3,
                })
                .expect("serialize"),
                r#"{"event":"track_started","path":"/music/a.flac","position":3}"#,
            ),
            (
                serde_json::to_string(&Event::Paused).expect("serialize"),
                r#"{"event":"paused"}"#,
            ),
            (
                serde_json::to_string(&Event::Resumed).expect("serialize"),
                r#"{"event":"resumed"}"#,
            ),
            (
                serde_json::to_string(&Event::Stopped).expect("serialize"),
                r#"{"event":"stopped"}"#,
            ),
            (
                serde_json::to_string(&Event::QueueEnded).expect("serialize"),
                r#"{"event":"queue_ended"}"#,
            ),
            (
                serde_json::to_string(&Event::TrackFailed {
                    path: PathBuf::from("/music/broken.flac"),
                    reason: "decode error: oops".into(),
                })
                .expect("serialize"),
                r#"{"event":"track_failed","path":"/music/broken.flac","reason":"decode error: oops"}"#,
            ),
            (
                serde_json::to_string(&Event::Progress {
                    elapsed_ms: 93_500,
                    track_ms: Some(214_000),
                })
                .expect("serialize"),
                r#"{"event":"progress","elapsed_ms":93500,"track_ms":214000}"#,
            ),
            (
                // An undeclared track length is `null`, never a sentinel
                // number: a front end must be able to tell "unknown" from
                // "zero-length".
                serde_json::to_string(&Event::Progress {
                    elapsed_ms: 0,
                    track_ms: None,
                })
                .expect("serialize"),
                r#"{"event":"progress","elapsed_ms":0,"track_ms":null}"#,
            ),
        ];
        for (got, want) in cases {
            assert_eq!(got, want);
        }
    }

    #[test]
    fn unknown_input_is_an_error_not_a_panic() {
        let result = serde_json::from_str::<Command>(r#"{"cmd":"explode"}"#);
        assert!(result.is_err());
        let result = serde_json::from_str::<Event>(r#"{"event":"explode"}"#);
        assert!(result.is_err());
    }
}
