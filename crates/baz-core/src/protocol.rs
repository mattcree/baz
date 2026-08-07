//! The command/event protocol between the engine and its front ends.
//!
//! Every message is serde-serializable so that the in-process GUI and any
//! future remote transport speak the same language (ADR-0003). Both enums are
//! `#[non_exhaustive]`: front ends must tolerate messages they don't know,
//! which is what lets the protocol grow without breaking older clients.
//!
//! The variants below are the initial skeleton; they expand with the engine.

use serde::{Deserialize, Serialize};

/// A request from a front end to the engine.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Resume playback of the current queue position.
    Play,
    /// Pause playback, keeping the current position.
    Pause,
}

/// A notification from the engine to all connected front ends.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// Playback started or resumed.
    PlaybackStarted,
    /// Playback paused.
    PlaybackPaused,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_json_roundtrip() {
        for cmd in [Command::Play, Command::Pause] {
            let json = serde_json::to_string(&cmd).expect("serialize");
            let back: Command = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(cmd, back);
        }
    }

    #[test]
    fn wire_format_is_stable() {
        // The wire format is a public contract; a change here is a protocol
        // break and must be a deliberate, versioned decision.
        let json = serde_json::to_string(&Command::Play).expect("serialize");
        assert_eq!(json, r#"{"cmd":"play"}"#);
        let json = serde_json::to_string(&Event::PlaybackPaused).expect("serialize");
        assert_eq!(json, r#"{"event":"playback_paused"}"#);
    }

    #[test]
    fn unknown_input_is_an_error_not_a_panic() {
        let result = serde_json::from_str::<Command>(r#"{"cmd":"explode"}"#);
        assert!(result.is_err());
    }
}
