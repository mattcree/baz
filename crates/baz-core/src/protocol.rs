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
//!
//! # ReplayGain on the wire: integer centidecibels
//!
//! [`Command::SetReplayGain`]'s pre-amps and
//! [`Event::ReplayGainChanged`]'s applied figure are `i16` **centidecibels** —
//! hundredths of a decibel, zero meaning unity — for the third time on the
//! same argument: one canonical JSON encoding, and the enums keep their `Eq`.
//! 0.01 dB is finer than the two decimal places the `"-7.75 dB"` tag
//! convention itself carries, so nothing is lost by not being a float.
//! [`crate::replaygain`] owns the unit and everything computed from it.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A request from a front end to the engine.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Replace the play queue with `paths`. Does not start playback; any
    /// playback in progress stops (the engine emits [`Event::Stopped`]).
    ///
    /// This is the **reset**: "forget what you were doing and hold this
    /// queue instead". [`Command::UpdateQueue`] is the edit — same payload,
    /// same absolute shape, but the music keeps playing (ADR-0014).
    SetQueue {
        /// The new queue, in play order.
        paths: Vec<PathBuf>,
    },
    /// Edit the play queue **without interrupting the music**: remove, insert,
    /// append or reorder by sending the queue as it should now be (ADR-0014).
    ///
    /// [`Command::SetQueue`]'s documented behaviour is to stop, which makes it
    /// useless for editing — re-sending the queue minus one track would silence
    /// the music to delete a track nobody was listening to. This command exists
    /// for exactly that gesture and guarantees the opposite: **an edit that
    /// does not touch the playing track does not disturb a single delivered
    /// sample.**
    ///
    /// # Absolute, like every other setting in this protocol
    ///
    /// It carries the whole queue rather than an operation on it
    /// (`RemoveAt { index }`, `MoveTo { from, to }`, …) for the reason
    /// [`Command::Seek`] and [`Command::SetMute`] are absolute: an index-based
    /// delta applied against a front end's stale picture removes *a different
    /// track*, and there is no way for either side to notice. A whole-queue
    /// command cannot desynchronize, expresses every edit including
    /// multi-selection removal and drag-reorder, and costs a front end nothing
    /// — it already holds the list it is editing. Sending the queue the engine
    /// already has emits nothing and changes nothing.
    ///
    /// # Identity, not index
    ///
    /// The thing that survives an edit is the **playing track**, never its
    /// position: removing two tracks above it renumbers it, and a front end
    /// that assumed otherwise would mark the wrong row. The engine therefore
    /// re-derives its position from the path it is playing — believing the old
    /// index when the new queue still holds that path there, and otherwise
    /// taking the first occurrence of it — and reports the answer on
    /// [`Event::QueueChanged`]. (That is the same reconciliation rule front
    /// ends already apply to [`Event::TrackStarted`], now stated once, in the
    /// engine.)
    ///
    /// # What happens to the rest of the run
    ///
    /// The track being delivered plays to its end and the run then continues
    /// from the **new** queue, one position past where that track now sits. The
    /// handover starts a fresh session the way [`Command::Next`] does — except
    /// that nothing already accepted by the output is thrown away, because a
    /// track that played out is owed its tail. So the current track is never cut
    /// short, and the boundary out of it is a fresh decode rather than a
    /// sample-accurate splice: an edit costs the *next* boundary its gaplessness
    /// and nothing else. [`crate::engine`] carries the detail.
    ///
    /// # When the edit removes the playing track
    ///
    /// That edit *does* touch the playing track, so the guarantee above does
    /// not apply and the engine says what happens instead: playback moves to
    /// the entry that took its place — the **same index** in the new queue,
    /// which for the ordinary "remove the track I am listening to" gesture is
    /// the track that follows it — starting from its beginning, exactly as
    /// [`Command::JumpTo`] would. Index is the right answer here precisely
    /// because identity did not survive. If the new queue is too short (or
    /// empty), the run ends ([`Event::QueueEnded`]).
    ///
    /// # While stopped or paused
    ///
    /// While stopped it replaces the queue and starts nothing — the difference
    /// from [`Command::SetQueue`] there is only that no [`Event::Stopped`] is
    /// emitted, because nothing was playing. While **paused** it stays paused:
    /// an edit is not a transport command and must not start the music.
    UpdateQueue {
        /// The queue as it should now be, in play order.
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
    /// Play the queue entry at `position`, from its beginning — the
    /// queue-relative sibling of [`Command::Seek`], which is track-relative
    /// (ADR-0014).
    ///
    /// This is what a click on a queue row sends. Without it, reaching row 9
    /// means eight [`Command::Next`]s: eight sessions, eight
    /// [`Event::SignalPath`] reports, and eight tracks of audio briefly
    /// reaching the output. That is not a jump.
    ///
    /// # What it does, in each transport state
    ///
    /// - **While playing** — the current session is abandoned exactly as
    ///   [`Command::Next`] abandons it (its buffered audio is discarded, so the
    ///   position being left is not heard afterwards) and a fresh one starts at
    ///   `position`.
    /// - **While paused** — it moves *and resumes*, because
    ///   [`Command::Next`] and [`Command::Previous`] do, and three transport
    ///   commands that select a queue position must not disagree about whether
    ///   pressing them starts the music ([`crate::engine`]'s command table
    ///   states it for all three).
    /// - **While stopped** — it starts playing at `position`. This is the one
    ///   place it parts company with `Next` and `Previous`, and the reason is
    ///   the difference between an absolute command and a relative one: they
    ///   are no-ops while stopped because there is no current track to step
    ///   from, which is not a difficulty an absolute position has. `JumpTo` is
    ///   [`Command::Play`] aimed at a chosen entry.
    ///
    /// Aimed at the track already playing it **restarts** it from the
    /// beginning; that is a change of position, not a redundant command, and
    /// it is what a click on the playing row plainly means.
    ///
    /// # Out of range
    ///
    /// `position` past the last entry (and any `position` at all on an empty
    /// queue) **ends the run**: [`Event::QueueEnded`], and a later
    /// [`Command::Play`] starts from the top. Not clamped to the last entry —
    /// playing a track the listener did not point at is a worse answer than
    /// stopping — and not an error, because a queue that shrank under a click
    /// is an ordinary race rather than a fault, and this protocol has no error
    /// channel to report it on. It is exactly what [`Command::Next`] does past
    /// the end of the queue.
    JumpTo {
        /// Zero-based queue position to play, counted in the engine's current
        /// queue ([`Event::QueueChanged`] is how a front end knows what that
        /// is after an edit).
        position: usize,
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
    /// Configure ReplayGain: which of a track's tagged gains to honour, the two
    /// pre-amps, and whether to stay below full scale (ADR-0013).
    ///
    /// Absolute and idempotent, like [`Command::Seek`] and
    /// [`Command::SetMute`] and for the same reason: a command that carries the
    /// whole setting cannot desynchronize from a front end that missed an
    /// event. Sending the settings the engine already has emits nothing.
    ///
    /// # What it does and does not do
    ///
    /// baz **reads** the `REPLAYGAIN_*` (and Opus-style `R128_*`) tags files
    /// already carry and applies them. It does not *compute* them: there is no
    /// analysis pass here, so a library nothing has ever scanned is unaffected
    /// by this command except through
    /// [`no_tag_preamp_centidb`](Self::SetReplayGain::no_tag_preamp_centidb),
    /// which is zero by default.
    ///
    /// # Honesty
    ///
    /// ReplayGain is a software gain. Whenever it resolves to anything other
    /// than unity the sample stream is being scaled, and
    /// [`Event::VolumeChanged`]'s [`VolumePath`] says so — baz has **one**
    /// gain stage and one readout for it, so a front end asks
    /// [`VolumePath::is_transparent`] exactly as it did before ReplayGain
    /// existed. [`Event::ReplayGainChanged`] adds the ReplayGain-specific
    /// detail (which figure, how much, whether clipping prevention bit); it
    /// does not carry a second, parallel notion of fidelity.
    SetReplayGain {
        /// Which of a track's figures to use, or [`ReplayGainMode::Off`].
        mode: ReplayGainMode,
        /// Added to whatever gain the tags asked for, in hundredths of a
        /// decibel. Clamped to
        /// ±[`MAX_PREAMP_CENTIDB`](crate::replaygain::MAX_PREAMP_CENTIDB).
        preamp_centidb: i16,
        /// Applied instead, in hundredths of a decibel, when a file carries no
        /// usable ReplayGain at all. Clamped the same way.
        ///
        /// **Zero is the documented default**: an untagged file is then played
        /// exactly as stored, so switching ReplayGain on cannot quieten a
        /// library that has never been through a scanner, and such a track
        /// keeps ADR-0009's untouched path.
        no_tag_preamp_centidb: i16,
        /// Whether to reduce a gain that would push the file's declared peak
        /// above full scale. On by default; the exact rule is documented on
        /// [`ReplayGainSettings::resolve`](crate::replaygain::ReplayGainSettings::resolve).
        prevent_clipping: bool,
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
    /// The engine's queue changed, and this is where the run now sits in it
    /// (ADR-0014).
    ///
    /// # Cadence
    ///
    /// Only on an accepted [`Command::SetQueue`] or [`Command::UpdateQueue`] —
    /// user gestures, not a cadence — and only when something actually changed,
    /// like every other event here. It is **not** how a front end follows
    /// playback: a track boundary moves the position and announces itself with
    /// [`Event::TrackStarted`], which carries the position too. This event
    /// exists for the one thing `TrackStarted` cannot say, which is that the
    /// position moved *without* the music moving — the renumbering an edit
    /// causes.
    ///
    /// # Reading it
    ///
    /// `position` is the engine's own answer to "which entry is playing", after
    /// re-deriving it from the path being played
    /// ([`Command::UpdateQueue`] states the rule); `None` means nothing is
    /// playing. A front end should take it rather than its own computed
    /// answer — the two can differ when an edit races a track boundary, and the
    /// engine's is the one the audio agrees with.
    ///
    /// `len` is how many entries the engine now holds. The paths are
    /// deliberately **not** echoed: the sender just supplied them verbatim (the
    /// engine neither filters, validates nor de-duplicates a queue), so
    /// repeating a whole album back on every edit would be churn to state a
    /// fact the front end has already got. What it cannot know without being
    /// told is whether the engine ended up holding the same *number* of entries
    /// it sent, and that is the cheap disagreement check this field is for.
    QueueChanged {
        /// How many entries the queue now holds.
        len: usize,
        /// Zero-based position of the entry now playing, or `None` when
        /// nothing is.
        position: Option<usize>,
    },
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
    /// The ReplayGain settings, and what they resolved to for the track now
    /// playing (ADR-0013).
    ///
    /// # Cadence
    ///
    /// Emitted whenever any of it changes, which is two separate occasions:
    /// an accepted [`Command::SetReplayGain`], and a **track boundary** at
    /// which the new track's tags resolve to a different figure. In album mode
    /// across an album the second never happens — every track shares one album
    /// gain — so an album states its ReplayGain once, exactly as
    /// [`Event::SignalPath`] states its format once. Redundant commands emit
    /// nothing.
    ///
    /// # Reading it
    ///
    /// The first four fields are the confirmed settings, echoed back: a front
    /// end should follow them rather than its own optimistic copy, so two
    /// front ends on one engine agree. The last three describe the *current
    /// track*: `source` says which figure the number came from (including
    /// [`ReplayGainSource::NoTag`], the file declared none, and
    /// [`ReplayGainSource::Disabled`], the feature is off),
    /// `applied_centidb` is the gain in hundredths of a decibel with zero
    /// meaning unity, and `clipping_prevented` says the tags asked for more
    /// than the declared peak had room for.
    ///
    /// # This event does not carry the fidelity readout
    ///
    /// Deliberately. baz has one software gain stage, the volume and
    /// ReplayGain both feed it, and [`Event::VolumeChanged`]'s [`VolumePath`]
    /// is where that stage reports itself — before ReplayGain existed and
    /// after. A non-unity `applied_centidb` is therefore always accompanied by
    /// a `VolumeChanged` whose `path` is [`VolumePath::SoftwareGain`], and
    /// [`VolumePath::is_transparent`] remains the whole question. Adding a
    /// second "is this bit-exact" flag here would have given a front end two
    /// answers to reconcile, which is how the two come to disagree.
    ///
    /// **This is information, not a warning**, on exactly the terms
    /// [`Event::SignalPath`] and [`Event::VolumeChanged`] set out. ReplayGain
    /// is a correctness feature a listener asked for; nothing here is a fault
    /// condition and nothing here should be styled as one.
    ReplayGainChanged {
        /// Which of a track's figures is being used, or
        /// [`ReplayGainMode::Off`].
        mode: ReplayGainMode,
        /// The pre-amp for tagged files, in hundredths of a decibel, as the
        /// engine clamped it.
        preamp_centidb: i16,
        /// The pre-amp for untagged files, in hundredths of a decibel, as the
        /// engine clamped it.
        no_tag_preamp_centidb: i16,
        /// Whether clipping prevention is armed.
        prevent_clipping: bool,
        /// Where the applied gain came from.
        source: ReplayGainSource,
        /// The gain actually applied to the current track, in hundredths of a
        /// decibel. Zero means unity, and unity means no arithmetic.
        applied_centidb: i16,
        /// Whether the applied gain is lower than the tags asked for because
        /// the full figure would have exceeded full scale.
        clipping_prevented: bool,
    },
}

/// Which of a track's ReplayGain figures to honour, in
/// [`Command::SetReplayGain`].
///
/// The three-mode vocabulary every player that implements ReplayGain uses, and
/// the one a foobar2000 refugee expects to find. See
/// [`ReplayGainSettings::resolve`](crate::replaygain::ReplayGainSettings::resolve)
/// for the exact selection rule, fallbacks included.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayGainMode {
    /// Do not apply ReplayGain. **The default**, for the reason ADR-0009 makes
    /// the bit-perfect path the default: a player that has not been told to
    /// change the samples does not change them.
    ///
    /// Off is not "a gain of 0 dB that happens to be inaudible" — the engine
    /// performs no ReplayGain arithmetic at all, so the delivered stream is
    /// bit-identical to a baz built before ReplayGain existed.
    #[default]
    Off,
    /// Normalise each track to the reference loudness independently, using its
    /// `REPLAYGAIN_TRACK_GAIN`. What a shuffled queue of unrelated tracks
    /// wants: every track arrives at the same loudness.
    Track,
    /// Normalise each *album* as a whole, using its `REPLAYGAIN_ALBUM_GAIN`, so
    /// that the level differences its mastering engineer put between its tracks
    /// survive. What an album — and especially a continuous one — wants.
    ///
    /// Falls back to the track gain for a file that declares no album value;
    /// see [`ReplayGainSource::TrackFallback`].
    Album,
}

/// Where the gain in [`Event::ReplayGainChanged`] came from.
///
/// Modelled as a state rather than an `Option`, for [`SignalChain`]'s reason:
/// "the file has no ReplayGain", "ReplayGain is switched off" and "album mode
/// found only a track gain" are three different facts about the system, and a
/// front end that wants to explain itself needs to tell them apart.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayGainSource {
    /// ReplayGain is [`ReplayGainMode::Off`]. The gain is unity and no
    /// arithmetic happens.
    #[default]
    Disabled,
    /// The track's own `REPLAYGAIN_TRACK_GAIN`.
    Track,
    /// The album's `REPLAYGAIN_ALBUM_GAIN`.
    Album,
    /// Album mode, but the file declares no album gain — so its track gain was
    /// used instead. The ordinary reading for a single downloaded track, which
    /// has no album to be relative to.
    TrackFallback,
    /// The file declares no usable ReplayGain at all, so the "no ReplayGain"
    /// pre-amp applies (zero by default, i.e. the file is played as stored).
    ///
    /// This is the reading for an unscanned library, and it is not a failure:
    /// baz reads ReplayGain, it does not compute it.
    NoTag,
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

/// Where baz's software gain stage is applied, in [`Event::VolumeChanged`] —
/// and therefore whether the sample stream is still literally untouched.
///
/// This is the ADR-0011 half of the fidelity readout, and it exists for one
/// reason: **software gain is not bit-exact, and saying otherwise would be the
/// silent conversion ADR-0009 exists to rule out.** baz decodes to f32, so
/// scaling costs ~1 ULP of a 24-bit mantissa — around −140 dBFS, inaudible by
/// any measure a listener could apply — but "inaudible" and "identical" are
/// different claims and only one of them is true.
///
/// # It covers ReplayGain too (ADR-0013)
///
/// ADR-0011 introduced this type for the volume, which was then the only gain
/// baz applied. ReplayGain is a second *input* to the same stage — one fader,
/// one multiply per sample, the product of the two — so this type answers for
/// that stage as a whole rather than gaining a sibling. Concretely: with the
/// volume at unity and a ReplayGain of −7.75 dB in effect, the path is
/// [`Self::SoftwareGain`], because it is. The ReplayGain-specific detail
/// travels on [`Event::ReplayGainChanged`], which deliberately carries **no**
/// fidelity field of its own — two answers to one question is how two answers
/// come to disagree.
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
    /// No gain stage at all: the volume is at unity and unmuted **and**
    /// ReplayGain is contributing nothing, so the engine performs no arithmetic
    /// on the samples — not even a multiply by one (see [`crate::volume`] for
    /// why the difference is structural and not pedantry). This is the state in
    /// which ADR-0009's bit-perfect claim is unqualified.
    Unity,
    /// baz scales every sample by an f32 multiply on its way to the output.
    /// The ordinary state for any volume other than unity, for mute, and for
    /// any ReplayGain figure other than 0.00 dB (ADR-0013).
    SoftwareGain,
    /// The output device is carrying the volume in its own attenuator and the
    /// sample stream reaches it unscaled — bit-exact, with the volume applied
    /// downstream of everything baz does.
    ///
    /// Reported only when baz itself scales nothing. A device attenuator can
    /// carry the volume but not a ReplayGain figure, so an active,
    /// non-unity ReplayGain reads as [`Self::SoftwareGain`] even on a sink
    /// that took the volume — the samples are being multiplied, and that is
    /// what this type answers.
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
    /// Whether baz's gain stage leaves the sample stream untouched — true for
    /// [`Self::Unity`] and [`Self::DeviceAttenuator`], false for
    /// [`Self::SoftwareGain`].
    ///
    /// Since ADR-0013 this covers ReplayGain as well as the volume, because
    /// they are the same stage; a front end that already asked this question
    /// keeps getting the right answer without knowing ReplayGain exists.
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
            Command::UpdateQueue {
                paths: vec![
                    PathBuf::from("/music/a.flac"),
                    PathBuf::from("/music/b.wav"),
                ],
            },
            Command::UpdateQueue { paths: Vec::new() },
            Command::Play,
            Command::Pause,
            Command::Stop,
            Command::Next,
            Command::Previous,
            Command::JumpTo { position: 0 },
            Command::JumpTo { position: 9 },
            Command::Seek { position_ms: 0 },
            Command::Seek {
                position_ms: 93_500,
            },
            Command::SetVolume { position: 0 },
            Command::SetVolume { position: 618 },
            Command::SetVolume { position: 1000 },
            Command::SetMute { muted: true },
            Command::SetMute { muted: false },
            Command::SetReplayGain {
                mode: ReplayGainMode::Off,
                preamp_centidb: 0,
                no_tag_preamp_centidb: 0,
                prevent_clipping: true,
            },
            Command::SetReplayGain {
                mode: ReplayGainMode::Track,
                preamp_centidb: 300,
                no_tag_preamp_centidb: -450,
                prevent_clipping: false,
            },
            Command::SetReplayGain {
                mode: ReplayGainMode::Album,
                preamp_centidb: -1_200,
                no_tag_preamp_centidb: 0,
                prevent_clipping: true,
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
            Event::QueueChanged {
                len: 12,
                position: Some(3),
            },
            Event::QueueChanged {
                len: 0,
                position: None,
            },
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
        .into_iter()
        .chain(sample_replay_gain_events())
        .collect()
    }

    /// The [`Event::ReplayGainChanged`] samples, one per [`ReplayGainSource`].
    /// Split from [`sample_events`] only to keep either function readable.
    fn sample_replay_gain_events() -> Vec<Event> {
        vec![
            Event::ReplayGainChanged {
                mode: ReplayGainMode::Off,
                preamp_centidb: 0,
                no_tag_preamp_centidb: 0,
                prevent_clipping: true,
                source: ReplayGainSource::Disabled,
                applied_centidb: 0,
                clipping_prevented: false,
            },
            Event::ReplayGainChanged {
                mode: ReplayGainMode::Track,
                preamp_centidb: 0,
                no_tag_preamp_centidb: 0,
                prevent_clipping: true,
                source: ReplayGainSource::Track,
                applied_centidb: -775,
                clipping_prevented: false,
            },
            Event::ReplayGainChanged {
                mode: ReplayGainMode::Album,
                preamp_centidb: 600,
                no_tag_preamp_centidb: -300,
                prevent_clipping: true,
                source: ReplayGainSource::Album,
                applied_centidb: 104,
                clipping_prevented: true,
            },
            Event::ReplayGainChanged {
                mode: ReplayGainMode::Album,
                preamp_centidb: 0,
                no_tag_preamp_centidb: 0,
                prevent_clipping: true,
                source: ReplayGainSource::TrackFallback,
                applied_centidb: 233,
                clipping_prevented: false,
            },
            Event::ReplayGainChanged {
                mode: ReplayGainMode::Track,
                preamp_centidb: 0,
                no_tag_preamp_centidb: -500,
                prevent_clipping: true,
                source: ReplayGainSource::NoTag,
                applied_centidb: -500,
                clipping_prevented: false,
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
                // The edit (ADR-0014): the same payload as its sibling above,
                // and a different name because it means a different thing —
                // this one does not stop the music.
                serde_json::to_string(&Command::UpdateQueue {
                    paths: vec![PathBuf::from("/music/a.flac")],
                })
                .expect("serialize"),
                r#"{"cmd":"update_queue","paths":["/music/a.flac"]}"#,
            ),
            (
                // Emptying the queue is a legal edit, and an empty list must
                // encode as `[]` rather than as an absent key.
                serde_json::to_string(&Command::UpdateQueue { paths: Vec::new() })
                    .expect("serialize"),
                r#"{"cmd":"update_queue","paths":[]}"#,
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
                // A queue position, not a time: an integer for the reason
                // every other number here is one (module docs).
                serde_json::to_string(&Command::JumpTo { position: 9 }).expect("serialize"),
                r#"{"cmd":"jump_to","position":9}"#,
            ),
            (
                serde_json::to_string(&Command::JumpTo { position: 0 }).expect("serialize"),
                r#"{"cmd":"jump_to","position":0}"#,
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

    /// [`Command::SetReplayGain`]'s bytes, pinned — every mode, and both signs
    /// of a pre-amp. Split from its sibling above for the reason the volume
    /// event was: one test listing every variant of a growing enum outgrows
    /// what is readable, not because the contract is any weaker.
    #[test]
    fn replay_gain_command_wire_format_is_stable() {
        let cases: Vec<(String, &str)> = vec![
            (
                // The default state: off, no pre-amp, clipping prevention
                // armed. Zero encodes as `0`, never `0.0` or `-0` — the
                // centidecibel choice is what makes this assertable.
                serde_json::to_string(&Command::SetReplayGain {
                    mode: ReplayGainMode::Off,
                    preamp_centidb: 0,
                    no_tag_preamp_centidb: 0,
                    prevent_clipping: true,
                })
                .expect("serialize"),
                r#"{"cmd":"set_replay_gain","mode":"off","preamp_centidb":0,"no_tag_preamp_centidb":0,"prevent_clipping":true}"#,
            ),
            (
                serde_json::to_string(&Command::SetReplayGain {
                    mode: ReplayGainMode::Track,
                    preamp_centidb: 300,
                    no_tag_preamp_centidb: -450,
                    prevent_clipping: false,
                })
                .expect("serialize"),
                r#"{"cmd":"set_replay_gain","mode":"track","preamp_centidb":300,"no_tag_preamp_centidb":-450,"prevent_clipping":false}"#,
            ),
            (
                serde_json::to_string(&Command::SetReplayGain {
                    mode: ReplayGainMode::Album,
                    preamp_centidb: -1_200,
                    no_tag_preamp_centidb: 0,
                    prevent_clipping: true,
                })
                .expect("serialize"),
                r#"{"cmd":"set_replay_gain","mode":"album","preamp_centidb":-1200,"no_tag_preamp_centidb":0,"prevent_clipping":true}"#,
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

    /// [`Event::QueueChanged`]'s bytes, pinned (ADR-0014) — split from its
    /// sibling above for the reason the others were: one test listing every
    /// variant of a growing enum outgrows what is readable, not because the
    /// contract is any weaker.
    #[test]
    fn queue_event_wire_format_is_stable() {
        let cases: Vec<(String, &str)> = vec![
            (
                // The position is the engine's own re-derived answer; the
                // queue's paths are deliberately not echoed (see the variant).
                serde_json::to_string(&Event::QueueChanged {
                    len: 12,
                    position: Some(3),
                })
                .expect("serialize"),
                r#"{"event":"queue_changed","len":12,"position":3}"#,
            ),
            (
                // Nothing playing is `null`, never a sentinel index: a reader
                // must be able to tell "no row is playing" from "row 0 is".
                serde_json::to_string(&Event::QueueChanged {
                    len: 0,
                    position: None,
                })
                .expect("serialize"),
                r#"{"event":"queue_changed","len":0,"position":null}"#,
            ),
            (
                serde_json::to_string(&Event::QueueChanged {
                    len: 1,
                    position: Some(0),
                })
                .expect("serialize"),
                r#"{"event":"queue_changed","len":1,"position":0}"#,
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

    /// [`Event::ReplayGainChanged`]'s bytes, pinned — one per
    /// [`ReplayGainSource`], because the source is the field a front end
    /// switches on.
    #[test]
    fn replay_gain_event_wire_format_is_stable() {
        let cases: Vec<(String, &str)> = vec![
            (
                // Off: unity, and the engine touches nothing.
                serde_json::to_string(&Event::ReplayGainChanged {
                    mode: ReplayGainMode::Off,
                    preamp_centidb: 0,
                    no_tag_preamp_centidb: 0,
                    prevent_clipping: true,
                    source: ReplayGainSource::Disabled,
                    applied_centidb: 0,
                    clipping_prevented: false,
                })
                .expect("serialize"),
                r#"{"event":"replay_gain_changed","mode":"off","preamp_centidb":0,"no_tag_preamp_centidb":0,"prevent_clipping":true,"source":"disabled","applied_centidb":0,"clipping_prevented":false}"#,
            ),
            (
                // The conventional `"-7.75 dB"` track gain, as an integer.
                serde_json::to_string(&Event::ReplayGainChanged {
                    mode: ReplayGainMode::Track,
                    preamp_centidb: 0,
                    no_tag_preamp_centidb: 0,
                    prevent_clipping: true,
                    source: ReplayGainSource::Track,
                    applied_centidb: -775,
                    clipping_prevented: false,
                })
                .expect("serialize"),
                r#"{"event":"replay_gain_changed","mode":"track","preamp_centidb":0,"no_tag_preamp_centidb":0,"prevent_clipping":true,"source":"track","applied_centidb":-775,"clipping_prevented":false}"#,
            ),
            (
                // The case clipping prevention exists for: the tags plus the
                // pre-amp asked for more than the album peak had room for.
                serde_json::to_string(&Event::ReplayGainChanged {
                    mode: ReplayGainMode::Album,
                    preamp_centidb: 600,
                    no_tag_preamp_centidb: -300,
                    prevent_clipping: true,
                    source: ReplayGainSource::Album,
                    applied_centidb: 104,
                    clipping_prevented: true,
                })
                .expect("serialize"),
                r#"{"event":"replay_gain_changed","mode":"album","preamp_centidb":600,"no_tag_preamp_centidb":-300,"prevent_clipping":true,"source":"album","applied_centidb":104,"clipping_prevented":true}"#,
            ),
            (
                // Album mode on a single track that has no album gain.
                serde_json::to_string(&Event::ReplayGainChanged {
                    mode: ReplayGainMode::Album,
                    preamp_centidb: 0,
                    no_tag_preamp_centidb: 0,
                    prevent_clipping: true,
                    source: ReplayGainSource::TrackFallback,
                    applied_centidb: 233,
                    clipping_prevented: false,
                })
                .expect("serialize"),
                r#"{"event":"replay_gain_changed","mode":"album","preamp_centidb":0,"no_tag_preamp_centidb":0,"prevent_clipping":true,"source":"track_fallback","applied_centidb":233,"clipping_prevented":false}"#,
            ),
            (
                // An unscanned file: the "no ReplayGain" pre-amp, and nothing
                // else. A reader must be able to tell this from `disabled`.
                serde_json::to_string(&Event::ReplayGainChanged {
                    mode: ReplayGainMode::Track,
                    preamp_centidb: 0,
                    no_tag_preamp_centidb: -500,
                    prevent_clipping: true,
                    source: ReplayGainSource::NoTag,
                    applied_centidb: -500,
                    clipping_prevented: false,
                })
                .expect("serialize"),
                r#"{"event":"replay_gain_changed","mode":"track","preamp_centidb":0,"no_tag_preamp_centidb":-500,"prevent_clipping":true,"source":"no_tag","applied_centidb":-500,"clipping_prevented":false}"#,
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
