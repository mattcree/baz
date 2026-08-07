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
//!
//! # Volume on the wire: an integer control position
//!
//! [`Command::SetVolume`] carries a `u16` **control position** in
//! `0..=`[`MAX_POSITION`](crate::volume::MAX_POSITION), not a linear
//! amplitude and not decibels. The integer choice is the one argued above,
//! for the same two reasons: one canonical encoding for the byte-pinned
//! stability test, and [`Command`] keeps its `Eq`.
//!
//! *Which* number the integer is, is a separate decision and it belongs to
//! [`crate::volume`], which owns the taper that turns a position into a gain
//! and explains why the taper is defined in `baz-core` rather than in each
//! front end. The short version: a linear-amplitude control feels wrong to a
//! human, so the correction has to happen somewhere, and if it happened in the
//! front ends they would disagree with each other.

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
    /// Go back: restart the current track, or step to the one before it.
    ///
    /// The conventional two-in-one transport button, and the counterpart
    /// [`Command::Next`] has been missing. Which of the two things it does is
    /// decided by how far into the current track playback has reached:
    ///
    /// - **At or past
    ///   [`PREVIOUS_RESTART_MS`](crate::engine::PREVIOUS_RESTART_MS)** (3 000 ms)
    ///   — restart the current track from its beginning.
    /// - **Before it** — start the preceding queue entry from its beginning.
    /// - **Before it, at the head of the queue** — restart, because there is
    ///   nothing before position 0 and stopping would be a worse answer than
    ///   the thing the button does everywhere else.
    ///
    /// While stopped it is a no-op, exactly like [`Command::Next`]: there is
    /// no current track to be some number of seconds into. While **paused** it
    /// behaves like [`Command::Next`] too — it moves and *resumes* — because
    /// the two halves of one transport control must not disagree about that
    /// ([`crate::engine`]'s command table states it for both).
    ///
    /// A front end can therefore advertise this as always available whenever a
    /// queue is playing: unlike `Next` at the end of the queue, `Previous` has
    /// no position at which it does nothing.
    Previous,
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
    /// Set the playback volume to a position on the control's travel.
    ///
    /// Takes effect within one pump iteration and survives everything the
    /// transport does — pause, resume, seek, skip, track and rate changes,
    /// queue replacement — because it is engine state, not session state.
    ///
    /// # The unit
    ///
    /// `position` is `0..=`[`MAX_POSITION`](crate::volume::MAX_POSITION)
    /// (1000), *not* a linear amplitude: it is where the fader sits, and
    /// [`Volume::amplitude`](crate::volume::Volume::amplitude) is the taper
    /// that turns it into gain. Values above the maximum clamp to it (a
    /// pointer at the end of a slider lands one past the last pixel often
    /// enough that rejecting would be the wrong answer). There is no gain
    /// above unity: baz attenuates, it does not amplify, so the loudest baz
    /// plays a file is exactly as loud as the file is.
    ///
    /// # Unity is a real position
    ///
    /// [`MAX_POSITION`](crate::volume::MAX_POSITION) is exactly unity gain,
    /// and at unity the engine applies no arithmetic to the sample stream at
    /// all — it is the position at which ADR-0009's bit-perfect claim is
    /// unqualified. A front end should make it reachable and obvious.
    SetVolume {
        /// Position on the control's travel,
        /// `0..=`[`MAX_POSITION`](crate::volume::MAX_POSITION).
        position: u16,
    },
    /// Mute or unmute, independently of [`Command::SetVolume`].
    ///
    /// Deliberately *not* the same thing as position 0, and deliberately
    /// idempotent (`SetMute { muted }` rather than a toggle), which is the
    /// same choice [`Command::Seek`] makes for the same reason: an absolute
    /// command cannot desynchronize from a front end that missed an event.
    /// [`crate::volume`] gives the full argument for keeping mute separate —
    /// in short, mute has to remember the position it will restore, and if the
    /// engine did not remember it every front end would have to.
    SetMute {
        /// Whether output is silenced.
        muted: bool,
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
    /// The signal chain for the audio now playing: what the file is, what the
    /// output is running at, and whether anything sits between them.
    ///
    /// This is the readout ADR-0004 promised and ADR-0009 made load-bearing.
    /// It is emitted when a session starts and whenever any part of it
    /// changes — never once per track for an album that does not change, so a
    /// front end can treat every arrival as news.
    ///
    /// # Reading it
    ///
    /// [`SignalChain::Direct`] is the ordinary state: baz is playing the file
    /// at its own rate, converting nothing. [`SignalChain::Converting`] means
    /// a sample-rate conversion is in the path, and `reason` says which of the
    /// two ordinary causes it is.
    ///
    /// **This is information, not a warning.** Converting is a normal thing
    /// for a player to do when hardware or a setting requires it, and the only
    /// unacceptable version of it is the *silent* one. A front end should
    /// render this the way it renders a codec name — available to a listener
    /// who wants it, ignorable by one who does not. Nothing here is a fault
    /// condition and nothing here should be styled as one.
    ///
    /// # What `Direct` does not claim
    ///
    /// It says *baz* converted nothing: the decoder's samples reached the
    /// output at the file's own rate, in a format that carries them exactly.
    /// It does not claim the operating system's mixer left them alone
    /// downstream — that is the claim [`SignalChain::Exclusive`] makes, and
    /// only an exclusive-mode backend can make it (ADR-0012). `Direct` is
    /// therefore precisely "shared mode, nothing converted by baz".
    ///
    /// Since ADR-0011 it also does not claim, on its own, that the samples
    /// were unaltered: **a volume below unity is a second, independent gain
    /// stage**, reported by [`Event::VolumeChanged`]'s [`VolumePath`]. A path
    /// is literally bit-exact when `chain` is [`SignalChain::Direct`] *and*
    /// that `path` is [`VolumePath::Unity`]; neither fact alone is the whole
    /// statement, and [`VolumePath::is_transparent`] exists so a front end
    /// does not have to remember which is which. The volume fact is a
    /// separate event rather than a field here because the two change on
    /// completely different cadences — this one per session, that one per
    /// pointer drag — and folding them would mean restating a track's whole
    /// format every time someone nudged a slider.
    SignalPath {
        /// Sample rate of the track now playing, in Hz.
        source_rate_hz: u32,
        /// Bit depth the track's container declares, when it declares one.
        /// `None` for sources that carry no integer depth (float PCM) or do
        /// not say. The engine's own output format is f32, whose 24-bit
        /// mantissa carries 24-bit and narrower sources exactly, so this
        /// number is never truncated on baz's side of the chain.
        source_bits: Option<u32>,
        /// Rate the output stream is running at, in Hz.
        output_rate_hz: u32,
        /// What the engine is doing between the two.
        chain: SignalChain,
    },
    /// The volume, the mute state, and where the volume is being applied.
    ///
    /// Emitted whenever any of the three changes — including the changes the
    /// engine makes on its own behalf, such as re-establishing the volume
    /// after the output is reopened at a new sample rate. Redundant commands
    /// (setting the volume it already has) emit nothing, like every other
    /// command in this protocol.
    ///
    /// # Reading it
    ///
    /// `position` and `muted` are what the front end sent, echoed back as the
    /// engine's confirmed state — a slider should follow this rather than its
    /// own optimistic value, so that two front ends attached to one engine
    /// agree. `path` is the engine's own report of *where* the volume is being
    /// applied, which is the fidelity half of the story and is described on
    /// [`VolumePath`].
    ///
    /// **This is information, not a warning**, on exactly the terms
    /// [`Event::SignalPath`] sets out. Software gain is the ordinary way a
    /// player implements a volume control; the only unacceptable version is
    /// the one that claims the stream is untouched while scaling it.
    VolumeChanged {
        /// Position on the control's travel,
        /// `0..=`[`MAX_POSITION`](crate::volume::MAX_POSITION).
        position: u16,
        /// Whether output is muted, independently of `position`.
        muted: bool,
        /// Where the volume is being applied.
        path: VolumePath,
    },
}

/// What sits between the decoded file and the output, in
/// [`Event::SignalPath`].
///
/// Modelled as a state rather than a flag on purpose: "converting, because the
/// device has no 48 kHz mode" and "converting, because you asked for a fixed
/// output rate" are different facts about the system, and a front end that
/// wants to explain itself needs to tell them apart.
///
/// # Two questions, three states
///
/// The chain answers two questions — *does baz convert?* and *how far down
/// does the claim reach?* — and the variants are the combinations that
/// actually occur. [`Self::Direct`] and [`Self::Converting`] are shared-mode
/// output, where the system mixer owns the last hop and baz can say nothing
/// about it; [`Self::Exclusive`] is the exclusive-mode backend of ADR-0012,
/// which holds the device itself and so *can*. Exclusive mode can still be
/// converting (a DAC that has no 96 kHz mode is a DAC that has no 96 kHz mode
/// whoever owns it), which is why that variant carries the reason as an
/// `Option` rather than there being a fourth state.
///
/// Ask through [`Self::is_exclusive`] and [`Self::conversion_reason`] rather
/// than by enumerating variants, for the reason
/// [`VolumePath::is_transparent`] exists: the questions are stable, the list
/// of variants is not.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SignalChain {
    /// The output is running at the source's own sample rate and the
    /// decoder's samples reach it unconverted. The default, and the case for
    /// every album whose rate the output device can run at.
    ///
    /// Shared mode: what the system mixer does downstream is not claimed.
    Direct,
    /// The output is running at a different rate, so the engine converts the
    /// source to it (rubato windowed sinc — see
    /// [`crate::playback`]). Ordinary and expected in the situations
    /// [`ConversionReason`] enumerates; the point of reporting it is that a
    /// listener who cares can see it, not that anything has gone wrong.
    ///
    /// Shared mode, like [`Self::Direct`].
    Converting {
        /// Why the conversion is in the path.
        reason: ConversionReason,
    },
    /// baz holds the output device itself, so nothing — no mixer, no
    /// resampler, no other application's stream — sits between the samples and
    /// the converter (ADR-0012: ALSA `hw:` today; WASAPI exclusive and
    /// `CoreAudio` hog are the platform equivalents).
    ///
    /// `conversion` is `None` in the ordinary case, where the device was
    /// opened at the source's own rate. It is `Some` when the device offers no
    /// mode for this material and the engine converted to the nearest one it
    /// does offer — the same fact [`Self::Converting`] reports, on a chain
    /// that still owns the device.
    ///
    /// **Still information, not a badge of merit.** Shared mode is the normal
    /// way to play music and describes a perfectly good listening experience;
    /// this variant says which device arrangement is in use, no more.
    Exclusive {
        /// Why the engine is converting, when it is. `None` — the ordinary
        /// case — means the device is running at the source's own rate.
        conversion: Option<ConversionReason>,
    },
}

impl SignalChain {
    /// Whether baz holds the output device exclusively, so that the "nothing
    /// is between these samples and the converter" claim extends past baz's
    /// own process.
    #[must_use]
    pub fn is_exclusive(self) -> bool {
        matches!(self, Self::Exclusive { .. })
    }

    /// Why a sample-rate conversion is in the path, or `None` when there is
    /// none — in either output mode.
    #[must_use]
    pub fn conversion_reason(self) -> Option<ConversionReason> {
        match self {
            Self::Direct => None,
            Self::Converting { reason } => Some(reason),
            Self::Exclusive { conversion } => conversion,
        }
    }

    /// Whether the engine is sample-rate converting. The question most front
    /// ends actually have; `conversion_reason` is for the one that wants to
    /// say *why*.
    #[must_use]
    pub fn is_converting(self) -> bool {
        self.conversion_reason().is_some()
    }
}

/// Why a [`SignalChain::Converting`] chain is converting.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionReason {
    /// The output device does not offer the source's sample rate, so baz
    /// plays it at the nearest rate the device does offer. Playing the music
    /// is the right answer here; saying so is the other half of it.
    DeviceRateUnavailable,
    /// A fixed output rate was selected
    /// ([`BoundaryPolicy::ResampleToStreamRate`](crate::playback::BoundaryPolicy::ResampleToStreamRate)),
    /// so every track is brought to it regardless of what the device could
    /// have played directly.
    FixedOutputRate,
}

/// Where the volume is being applied, in [`Event::VolumeChanged`] — and
/// therefore whether the sample stream is still literally untouched.
///
/// This is the ADR-0011 half of the fidelity readout, and it exists for one
/// reason: **software gain is not bit-exact, and saying otherwise would be the
/// silent conversion ADR-0009 exists to rule out.** baz decodes to f32, so
/// scaling costs ~1 ULP of a 24-bit mantissa — around −140 dBFS, inaudible by
/// any measure a listener could apply — but "inaudible" and "identical" are
/// different claims and only one of them is true.
///
/// # Tone
///
/// The same rule as [`SignalChain`]: this is information, not a warning.
/// [`Self::SoftwareGain`] is what every ordinary player does with a volume
/// control and describes a perfectly good listening experience. A front end
/// should render it the way it renders a sample rate — available to the
/// listener who wants it, ignorable by the one who does not — and nothing here
/// should be styled as a fault.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumePath {
    /// No gain stage at all: the volume is at unity and unmuted, so the engine
    /// performs no arithmetic on the samples — not even a multiply by one (see
    /// [`crate::volume`] for why the difference is structural and not
    /// pedantry). This is the state in which ADR-0009's bit-perfect claim is
    /// unqualified.
    Unity,
    /// baz scales every sample by an f32 multiply on its way to the output.
    /// The ordinary state for any volume other than unity, and for mute.
    SoftwareGain,
    /// The output device is carrying the volume in its own attenuator and the
    /// sample stream reaches it unscaled — bit-exact, with the volume applied
    /// downstream of everything baz does.
    ///
    /// Reachable through [`Sink::set_device_volume`](crate::playback::Sink::set_device_volume),
    /// which **no backend baz ships implements**: shared-mode output has no
    /// per-application hardware volume to reach for, and the card-wide
    /// controls that do exist belong to the whole system rather than to this
    /// player. ADR-0011 records the measurements behind that and what would
    /// change it (exclusive-mode output, where baz owns the card and may
    /// legitimately drive its attenuator).
    DeviceAttenuator,
}

impl VolumePath {
    /// Whether the volume stage leaves the sample stream untouched — true for
    /// [`Self::Unity`] and [`Self::DeviceAttenuator`], false for
    /// [`Self::SoftwareGain`].
    ///
    /// Combine with [`SignalChain::Direct`] for the whole bit-exactness
    /// question; the method exists so a front end asks about the property it
    /// cares about rather than enumerating the variants that happen to have it
    /// today.
    #[must_use]
    pub fn is_transparent(self) -> bool {
        matches!(self, Self::Unity | Self::DeviceAttenuator)
    }
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
            Command::Previous,
            Command::Seek { position_ms: 0 },
            Command::Seek {
                position_ms: 93_500,
            },
            Command::SetVolume { position: 0 },
            Command::SetVolume { position: 618 },
            Command::SetVolume { position: 1000 },
            Command::SetMute { muted: true },
            Command::SetMute { muted: false },
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
            Event::SignalPath {
                source_rate_hz: 48_000,
                source_bits: Some(24),
                output_rate_hz: 48_000,
                chain: SignalChain::Direct,
            },
            Event::SignalPath {
                source_rate_hz: 48_000,
                source_bits: None,
                output_rate_hz: 44_100,
                chain: SignalChain::Converting {
                    reason: ConversionReason::DeviceRateUnavailable,
                },
            },
            Event::SignalPath {
                source_rate_hz: 96_000,
                source_bits: Some(24),
                output_rate_hz: 44_100,
                chain: SignalChain::Converting {
                    reason: ConversionReason::FixedOutputRate,
                },
            },
            Event::SignalPath {
                source_rate_hz: 96_000,
                source_bits: Some(24),
                output_rate_hz: 96_000,
                chain: SignalChain::Exclusive { conversion: None },
            },
            Event::SignalPath {
                source_rate_hz: 96_000,
                source_bits: Some(24),
                output_rate_hz: 48_000,
                chain: SignalChain::Exclusive {
                    conversion: Some(ConversionReason::DeviceRateUnavailable),
                },
            },
            Event::VolumeChanged {
                position: 1000,
                muted: false,
                path: VolumePath::Unity,
            },
            Event::VolumeChanged {
                position: 618,
                muted: false,
                path: VolumePath::SoftwareGain,
            },
            Event::VolumeChanged {
                position: 0,
                muted: true,
                path: VolumePath::SoftwareGain,
            },
            Event::VolumeChanged {
                position: 750,
                muted: false,
                path: VolumePath::DeviceAttenuator,
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

    /// Every [`Command`] variant's bytes, pinned. The wire format is a public
    /// contract; a change here is a protocol break and must be a deliberate,
    /// versioned decision.
    #[test]
    fn command_wire_format_is_stable() {
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
                serde_json::to_string(&Command::Previous).expect("serialize"),
                r#"{"cmd":"previous"}"#,
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
                // The top of the travel, which is unity gain. Integer for the
                // same reason `position_ms` is (module docs): `1000` has one
                // JSON rendering and `1.0` has several.
                serde_json::to_string(&Command::SetVolume { position: 1000 }).expect("serialize"),
                r#"{"cmd":"set_volume","position":1000}"#,
            ),
            (
                serde_json::to_string(&Command::SetVolume { position: 618 }).expect("serialize"),
                r#"{"cmd":"set_volume","position":618}"#,
            ),
            (
                serde_json::to_string(&Command::SetVolume { position: 0 }).expect("serialize"),
                r#"{"cmd":"set_volume","position":0}"#,
            ),
            (
                serde_json::to_string(&Command::SetMute { muted: true }).expect("serialize"),
                r#"{"cmd":"set_mute","muted":true}"#,
            ),
            (
                serde_json::to_string(&Command::SetMute { muted: false }).expect("serialize"),
                r#"{"cmd":"set_mute","muted":false}"#,
            ),
        ];
        for (got, want) in cases {
            assert_eq!(got, want);
        }
    }

    /// Every [`Event`] variant's bytes, pinned — same contract, same rule.
    #[test]
    fn event_wire_format_is_stable() {
        let cases: Vec<(String, &str)> = vec![
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
            (
                // The ordinary state: a 24/48 master played at 48 kHz.
                serde_json::to_string(&Event::SignalPath {
                    source_rate_hz: 48_000,
                    source_bits: Some(24),
                    output_rate_hz: 48_000,
                    chain: SignalChain::Direct,
                })
                .expect("serialize"),
                r#"{"event":"signal_path","source_rate_hz":48000,"source_bits":24,"output_rate_hz":48000,"chain":{"state":"direct"}}"#,
            ),
            (
                // The case the readout exists for: conversion is happening,
                // and the wire says so — with the reason, not a bare flag.
                serde_json::to_string(&Event::SignalPath {
                    source_rate_hz: 48_000,
                    source_bits: None,
                    output_rate_hz: 44_100,
                    chain: SignalChain::Converting {
                        reason: ConversionReason::DeviceRateUnavailable,
                    },
                })
                .expect("serialize"),
                r#"{"event":"signal_path","source_rate_hz":48000,"source_bits":null,"output_rate_hz":44100,"chain":{"state":"converting","reason":"device_rate_unavailable"}}"#,
            ),
            (
                serde_json::to_string(&Event::SignalPath {
                    source_rate_hz: 96_000,
                    source_bits: Some(24),
                    output_rate_hz: 44_100,
                    chain: SignalChain::Converting {
                        reason: ConversionReason::FixedOutputRate,
                    },
                })
                .expect("serialize"),
                r#"{"event":"signal_path","source_rate_hz":96000,"source_bits":24,"output_rate_hz":44100,"chain":{"state":"converting","reason":"fixed_output_rate"}}"#,
            ),
        ];
        for (got, want) in cases {
            assert_eq!(got, want);
        }
    }

    /// [`SignalChain::Exclusive`]'s bytes, pinned — split from its sibling
    /// above for the same reason the volume event was: one test listing every
    /// variant of a growing enum outgrows what is readable, not because the
    /// contract is any weaker.
    #[test]
    fn exclusive_signal_path_wire_format_is_stable() {
        let cases: Vec<(String, &str)> = vec![
            (
                // ADR-0012: baz owns the device, and nothing is converted.
                // The absent conversion is `null`, never a missing key: a
                // reader must be able to tell "not converting" from "this
                // sender did not say".
                serde_json::to_string(&Event::SignalPath {
                    source_rate_hz: 96_000,
                    source_bits: Some(24),
                    output_rate_hz: 96_000,
                    chain: SignalChain::Exclusive { conversion: None },
                })
                .expect("serialize"),
                r#"{"event":"signal_path","source_rate_hz":96000,"source_bits":24,"output_rate_hz":96000,"chain":{"state":"exclusive","conversion":null}}"#,
            ),
            (
                // Owning the device does not give it modes it does not have:
                // exclusive and converting is a real, reportable state.
                serde_json::to_string(&Event::SignalPath {
                    source_rate_hz: 96_000,
                    source_bits: Some(24),
                    output_rate_hz: 48_000,
                    chain: SignalChain::Exclusive {
                        conversion: Some(ConversionReason::DeviceRateUnavailable),
                    },
                })
                .expect("serialize"),
                r#"{"event":"signal_path","source_rate_hz":96000,"source_bits":24,"output_rate_hz":48000,"chain":{"state":"exclusive","conversion":"device_rate_unavailable"}}"#,
            ),
        ];
        for (got, want) in cases {
            assert_eq!(got, want);
        }
    }

    /// The two questions a front end has about the chain, answered by the type
    /// in every output mode rather than by enumerating variants at each call
    /// site (the rule [`VolumePath::is_transparent`] set).
    #[test]
    fn the_chain_answers_exclusivity_and_conversion_separately() {
        let shared_direct = SignalChain::Direct;
        let shared_converting = SignalChain::Converting {
            reason: ConversionReason::FixedOutputRate,
        };
        let exclusive_direct = SignalChain::Exclusive { conversion: None };
        let exclusive_converting = SignalChain::Exclusive {
            conversion: Some(ConversionReason::DeviceRateUnavailable),
        };

        assert!(!shared_direct.is_exclusive());
        assert!(!shared_converting.is_exclusive());
        assert!(exclusive_direct.is_exclusive());
        assert!(exclusive_converting.is_exclusive());

        assert!(!shared_direct.is_converting());
        assert!(shared_converting.is_converting());
        assert!(!exclusive_direct.is_converting());
        assert!(exclusive_converting.is_converting());

        assert_eq!(exclusive_direct.conversion_reason(), None);
        assert_eq!(
            exclusive_converting.conversion_reason(),
            Some(ConversionReason::DeviceRateUnavailable),
        );
    }

    /// [`Event::VolumeChanged`]'s bytes, pinned — split from its sibling above
    /// only because one test listing every variant of a growing enum outgrows
    /// what is readable, not because the contract is any weaker.
    #[test]
    fn volume_event_wire_format_is_stable() {
        let cases: Vec<(String, &str)> = vec![
            (
                // Unity: the position at which nothing is applied to the
                // samples at all, and the one a front end must make obvious.
                serde_json::to_string(&Event::VolumeChanged {
                    position: 1000,
                    muted: false,
                    path: VolumePath::Unity,
                })
                .expect("serialize"),
                r#"{"event":"volume_changed","position":1000,"muted":false,"path":"unity"}"#,
            ),
            (
                // The case the readout exists for: the stream is being scaled,
                // and the wire says so rather than implying otherwise.
                serde_json::to_string(&Event::VolumeChanged {
                    position: 618,
                    muted: false,
                    path: VolumePath::SoftwareGain,
                })
                .expect("serialize"),
                r#"{"event":"volume_changed","position":618,"muted":false,"path":"software_gain"}"#,
            ),
            (
                // Mute travels beside the position, never as a position: the
                // 618 survives the round trip (see `Command::SetMute`).
                serde_json::to_string(&Event::VolumeChanged {
                    position: 618,
                    muted: true,
                    path: VolumePath::SoftwareGain,
                })
                .expect("serialize"),
                r#"{"event":"volume_changed","position":618,"muted":true,"path":"software_gain"}"#,
            ),
            (
                serde_json::to_string(&Event::VolumeChanged {
                    position: 750,
                    muted: false,
                    path: VolumePath::DeviceAttenuator,
                })
                .expect("serialize"),
                r#"{"event":"volume_changed","position":750,"muted":false,"path":"device_attenuator"}"#,
            ),
        ];
        for (got, want) in cases {
            assert_eq!(got, want);
        }
    }

    /// The fidelity question a front end actually asks, answered by the type
    /// rather than by enumerating variants at the call site.
    #[test]
    fn only_software_gain_touches_the_samples() {
        assert!(VolumePath::Unity.is_transparent());
        assert!(VolumePath::DeviceAttenuator.is_transparent());
        assert!(!VolumePath::SoftwareGain.is_transparent());
    }

    #[test]
    fn unknown_input_is_an_error_not_a_panic() {
        let result = serde_json::from_str::<Command>(r#"{"cmd":"explode"}"#);
        assert!(result.is_err());
        let result = serde_json::from_str::<Event>(r#"{"event":"explode"}"#);
        assert!(result.is_err());
    }
}
