//! The engine service: the running half of the ADR-0003 command/event API.
//!
//! Front ends drive playback by sending [`Command`]s through an
//! [`EngineHandle`] and reacting to the [`Event`]s the engine emits; this
//! module is the authoritative contract for what each message does at
//! runtime. The wire shapes live in [`crate::protocol`]; the audio machinery
//! (decode, gapless splice, resample) lives in [`crate::playback`] and is
//! reused here unchanged.
//!
//! # Spawning
//!
//! [`spawn_offline`] runs the engine against a preallocated in-memory
//! [`OfflineSink`] — the headless configuration every test uses, and the way
//! to render a queue offline. With the non-default `device-output` feature,
//! `spawn_device` (feature `device-output`) plays through the default audio device instead. Both
//! return an [`EngineHandle`] plus the event [`Receiver`].
//!
//! # Which output, and what it claims
//!
//! `spawn_device` opens the arrangement the listener configured
//! ([`crate::playback::OutputMode`]); `spawn_device_with` takes it
//! as an argument, for a front end with a setting of its own.
//!
//! - **Shared** (the default) goes through cpal and the system's audio server.
//!   baz converts nothing (ADR-0009), and says nothing about what the mixer
//!   does downstream — because it cannot.
//! - **Exclusive** (ADR-0012, `exclusive-output`, Linux/ALSA today) holds a
//!   hardware device outright, so there is no mixer downstream to say anything
//!   about. It also makes a hardware volume legitimate for the first time
//!   (ADR-0011 built the seam and found nothing correct to put behind it in
//!   shared mode; owning the card is what changes that).
//!
//! The engine's own behaviour is identical either way — same negotiation, same
//! reopen, same drain-and-restart — and the difference reaches a front end as
//! one fact on [`Event::SignalPath`]: [`SignalChain::Exclusive`] instead of
//! [`SignalChain::Direct`]. **Neither is a better or worse state to be in**,
//! and the vocabulary stays informational for exactly the reason ADR-0009 §5
//! gives.
//!
//! An exclusive open that cannot happen — the device is busy, the name is not
//! one this machine has, the platform has no backend — **fails the spawn**. It
//! is never quietly downgraded to shared mode: a listener who asked baz to
//! hold the card and was told it had would have been misinformed about the one
//! thing the setting exists to state.
//!
//! # Threading model
//!
//! - **Engine (control + pump) thread** — spawned by `spawn_*`, owns the
//!   sink. It alternates between processing commands and pumping decoded
//!   audio from the session ring buffer into the sink. Because commands and
//!   pumping share one thread, control is race-free by construction: after
//!   [`Event::Paused`] is emitted, *no* further samples reach the sink until
//!   resume — there is no "one more chunk in flight". The pump iteration
//!   itself keeps the realtime discipline of `playback::engine::consume`:
//!   wait-free ring reads, writes into the preallocated sink, atomic
//!   flag/counter updates — no locks and no allocation on the pump path
//!   (event emission and command receipt happen *between* pump iterations,
//!   and for device output the true realtime thread is the cpal callback
//!   inside the device sink, which never runs any of this module's code).
//! - **Producer thread** — one per playback session (a session is one run
//!   through the queue, started by [`Command::Play`]). It streams the
//!   current track into an `rtrb` SPSC ring and decodes the next track ahead
//!   on a **prefetch thread**, exactly like [`run_playlist`](crate::playback::run_playlist), so track
//!   boundaries stay gapless by construction. Per-track boundary and failure
//!   notices travel to the engine thread over two more SPSC rings, keeping
//!   the pump side lock-free.
//!
//! All cross-thread control flags (`stop`, `producer_done`) are atomics; the
//! pause gate is plain single-threaded state on the engine thread.
//!
//! # Sample rate: the output follows the source
//!
//! ADR-0009 is the governing decision and it is short: **baz does not resample
//! unless it has to.** The output stream is opened at the rate of the music
//! rather than the music being converted to the rate of the output.
//!
//! ## Negotiation, per session
//!
//! A session's rate is settled once, at its start, by a two-atomic handshake
//! between the producer and the engine thread:
//!
//! 1. The producer opens the session's first playable track — the **anchor** —
//!    and publishes that track's own rate as a *proposal*.
//! 2. The engine thread, which is the only thread allowed to touch the sink,
//!    passes the proposal to [`Sink::negotiate_rate`] and publishes whatever
//!    comes back as the session's stream rate. Nothing has been pushed yet, so
//!    reopening a device here interrupts nothing.
//! 3. The producer, parked on that value, wakes and decodes against it.
//!
//! **Which rate a queue negotiates is therefore the anchor's** — the first
//! track that actually plays, counting from wherever the session started. That
//! is one header probe, so it adds nothing measurable to the time before first
//! audio; it is the rate of the album the listener just clicked, which is the
//! one they asked to hear; and it is the same rule
//! [`run_playlist`](crate::playback::run_playlist) already used, so offline and
//! device paths agree. The alternatives were considered and rejected: *most
//! common in the queue* would have to probe every file before a single sample
//! could play, and would convert the very track the user chose; *highest in
//! the queue* would upsample most of a mixed queue, which is DSP nobody asked
//! for.
//!
//! An [`OfflineSink`] has no rate and grants every proposal, so a headless
//! session simply runs at its source's rate.
//!
//! ## Rate changes inside a queue
//!
//! A track stored at a different rate from the running stream **ends the
//! session at that track**. The producer discovers this from the next track's
//! header during decode-ahead — no decode is wasted — and publishes the queue
//! index instead of pushing audio. The engine plays the ring out, drains the
//! sink so the previous track's tail is actually heard
//! ([`Sink::drain_buffered`] — the one place a session boundary drains instead
//! of discarding), and starts a fresh session at that index, which negotiates
//! and so reopens the output.
//!
//! Consequences, stated rather than hidden:
//!
//! - **Gapless is unaffected within a rate**, which is the ordinary case: an
//!   album is one rate, and every boundary inside it is the same
//!   sample-accurate splice it always was.
//! - **A boundary between two different rates carries a short gap** while the
//!   device is reconfigured. ADR-0009 measures it and accepts it.
//! - A front end sees exactly one [`Event::QueueEnded`], at the true end. The
//!   split is an internal handover; `TrackStarted` fires for every track in
//!   order as usual.
//! - **Pause is untouched.** Reopening happens only when a session *starts*,
//!   and pause never starts one — it gates the pump and keeps the sink's
//!   buffer, exactly as described below, so resume stays bit-identical.
//!
//! ## When the device will not follow
//!
//! If [`Sink::negotiate_rate`] answers with a rate other than the one asked
//! for — a device with no mode for this material — the engine converts to what
//! it was given and **says so**: [`Event::SignalPath`] carries
//! `Converting { reason: DeviceRateUnavailable }`. Playing the music is the
//! right answer; doing it silently is not. That readout, and the
//! [`Conversions`] counters behind [`EngineHandle::conversions`], are the whole
//! visible surface of the fallback.
//!
//! The one thing to know about this path's *cost*: a converted anchor is
//! decoded whole before its first sample is pushed, because the whole-buffer
//! resampler needs the whole buffer. On a five-minute 48 kHz FLAC that is a
//! measurable wait. It is the price of a case that no longer happens on
//! hardware that can play the file, and ADR-0009 records the number rather
//! than leaving it to be rediscovered.
//!
//! # Command semantics
//!
//! | Command | While stopped | While playing | While paused |
//! |---|---|---|---|
//! | [`Command::SetQueue`] | replaces queue | stops playback ([`Event::Stopped`]), replaces queue | same |
//! | [`Command::Play`] | starts at the queue top (or emits [`Event::QueueEnded`] if the queue is empty) | no-op | resumes ([`Event::Resumed`]) |
//! | [`Command::Pause`] | no-op | pauses ([`Event::Paused`]) | no-op |
//! | [`Command::Stop`] | no-op | stops ([`Event::Stopped`]); a later `Play` starts from the queue top | same |
//! | [`Command::Next`] | no-op | skips to the next queue position (see below) | skips and *resumes playing* |
//! | [`Command::Previous`] | no-op | restarts the current track past [`PREVIOUS_RESTART_MS`], else steps back one position | same, and *resumes playing* |
//! | [`Command::Seek`] | no-op | jumps within the current track, keeps playing | jumps within the current track, **stays paused** |
//! | [`Command::SetVolume`] / [`Command::SetMute`] | applies, silently | applies within one pump iteration, slewed | applies, silently |
//! | [`Command::SetReplayGain`] | applies, silently | applies within one pump iteration, slewed | applies, silently |
//!
//! # Volume
//!
//! ADR-0011 is the governing decision; [`crate::volume`] holds the unit, the
//! taper, and the fader. What belongs *here* is where the gain is applied and
//! what it survives.
//!
//! **Where.** In the session's pump, between the ring read and the sink write,
//! and nowhere else. That is the only place every sample passes through
//! exactly once regardless of how it got there — streamed anchor, prefetched
//! track, resampled fallback — so a volume applied there cannot be bypassed by
//! a code path someone adds later. Doing it in the producer instead would have
//! baked the gain into decoded audio that outlives the setting by a ring's
//! worth of buffering, which is precisely how a volume control comes to feel
//! late.
//!
//! **Realtime discipline.** Per pump *block* the volume costs **one acquire
//! load** of the effective gain (f32 bits in an `AtomicU32`) and **one
//! branch** — plus, on the scaling branch only, a second load of the session's
//! stream rate to size the slew step. Per *sample* it costs one multiply, into
//! a buffer preallocated when the engine thread starts. Nothing allocates,
//! nothing locks, nothing can panic. At unity the branch skips the multiply
//! *and* the copy: the
//! ring's slices go to the sink exactly as they did before volume control
//! existed, which is what makes bit-exactness at unity structural rather than
//! arithmetic (see [`crate::volume`]).
//!
//! **What it survives.** The volume is engine-thread state, not *session*
//! state, so pause, resume, seek, skip, queue replacement, a track boundary
//! and a sample-rate reopen all leave it exactly where it was — by
//! construction, not by remembering to copy it. The one thing the engine does
//! on its own initiative is **re-establish the volume after the output is
//! reopened** at a new rate, because a reopened device is a fresh device and
//! any attenuator it was carrying went with the old one.
//!
//! **Reporting.** [`Event::VolumeChanged`] carries the position, the mute
//! state, and the [`VolumePath`] — which of the two places the volume is
//! actually being applied. In **shared** mode it is
//! [`VolumePath::SoftwareGain`] below unity and [`VolumePath::Unity`] at it,
//! for the reasons ADR-0011 measured: there is no bit-exact per-application
//! volume to reach for. In **exclusive** mode baz owns the card, so
//! [`Sink::set_device_volume`] can drive its attenuator and
//! [`VolumePath::DeviceAttenuator`] becomes the ordinary reading — the stream
//! reaches the converter unscaled and the volume happens below everything baz
//! does (ADR-0012). Unity is still reported as [`VolumePath::Unity`] even
//! then: with nothing to attenuate, "no gain stage anywhere" is the more
//! precise of the two true statements.
//!
//! # ReplayGain
//!
//! ADR-0013 is the governing decision; [`crate::replaygain`] holds the units,
//! the tag parser and the selection rule. What belongs *here* is where the
//! number comes from, when it changes, and what it shares.
//!
//! **It shares the volume's gain stage entirely.** The resolved ReplayGain is
//! multiplied by the volume where the volume itself is settled, and the *product* is
//! published as the one gain the pump reads. There is no second multiply, no
//! second fader and no second slew: at unity-times-unity the pump takes the
//! transparent branch exactly as before, and at anything else it takes the same
//! single-multiply branch it already had. That is also why the fidelity readout
//! did not need a sibling — [`VolumePath`] describes the stage, and the stage is
//! where ReplayGain lands.
//!
//! **Where the tags come from.** The engine is given *paths*, never library
//! rows, so it reads ReplayGain from the file it is about to play:
//! [`AudioSource`] lifts it out of the metadata Symphonia already parsed during
//! the header probe, at no extra I/O, and it travels to the engine thread on
//! the same `TrackBound` that already carries the track's rate and depth. A
//! queue of paths the library has never seen therefore plays at the right level,
//! and the shelf and the engine cannot disagree about a file because they run
//! the same parser over the same keys ([`crate::replaygain::field_of_key`]).
//!
//! **When it changes: exactly at the boundary.** ReplayGain is per *track*,
//! and the engine can only change a gain between pump calls — so the pump
//! caps each read at the next known track boundary. The first sample of a new
//! track is therefore the first sample at its own gain, rather than up to a
//! block (46 ms at the app's chunk size) late. The change is then slewed like
//! any other, over [`RAMP_MS`](crate::volume::RAMP_MS), so a gapless splice
//! carries a 20 ms ramp rather than a step discontinuity. In **album** mode
//! across an album there is nothing to ramp: every track shares one album gain,
//! so the gain does not change at the boundary at all.
//!
//! **What it survives.** The settings are engine state, exactly as the volume
//! is: pause, resume, seek, skip, queue replacement and a rate reopen all leave
//! them untouched. The *resolved* figure is per track and is recomputed
//! whenever the delivering track changes, or the settings do.
//!
//! **Reporting.** [`Event::ReplayGainChanged`] carries the settings and the
//! resolved figure; it deliberately carries no fidelity flag, because
//! [`Event::VolumeChanged`]'s [`VolumePath`] already answers for the whole
//! stage. Engaging a non-unity ReplayGain therefore emits a `VolumeChanged`
//! whose `path` is [`VolumePath::SoftwareGain`] even though the volume did not
//! move — which is the truth, and the same neutral information ADR-0009 §5 asks
//! for rather than a warning.
//!
//! # Event semantics
//!
//! - [`Event::TrackStarted`] fires when a track's first samples are
//!   delivered to the sink (not when they are decoded — decode-ahead runs
//!   seconds early). A [`Command::Seek`] restarts the current track, so it
//!   fires again for that same track when the post-seek audio reaches the
//!   sink — the statement it makes ("this track's audio is now arriving") is
//!   true both times, and a front end that folds it idempotently sees
//!   nothing unusual.
//! - [`Event::Progress`] reports the position inside the current track at
//!   the cadence its protocol docs pin: one per quarter-second of delivered
//!   audio, plus one immediately after every `TrackStarted`, `Resumed`, and
//!   accepted `Seek`. See "Elapsed time" below for what "position" means
//!   precisely.
//! - [`Event::TrackFailed`] fires when a track cannot be opened or decoded;
//!   the queue continues with the next track. Because failures are found by
//!   decode-ahead, a `TrackFailed` for position *n+1* can arrive while
//!   position *n* is still audible. Per-track events are always emitted in
//!   queue order.
//! - [`Event::QueueEnded`] fires when every queued track has played, failed,
//!   or been skipped. Playback position resets to the queue top.
//! - Events are emitted only for state that changed: redundant commands
//!   (pausing while paused, stopping while stopped) emit nothing.
//!
//! # Pause, stop, and skip — implementation honesty
//!
//! **Pause** gates the pump: the session (ring, producer, decode-ahead)
//! stays intact and for device output the stream stays open, so resume is
//! gapless-instant and the delivered sample stream is bit-identical to an
//! unpaused run. (Device output has up to one device-ring's worth of already
//! -pumped audio that keeps draining after `Paused` — ordinary output
//! latency, ~0.2 s at the size the app uses.) Pause is therefore the one
//! transport command that does *not* call [`Sink::discard_buffered`]:
//! throwing that audio away is exactly what would cost resume its
//! bit-identical continuation, so the short trailing drain is the price and
//! it is knowingly paid.
//!
//! **Stop** and **Next** abort the session: an atomic stop flag releases the
//! producer, its threads are joined, and undelivered ring audio is
//! discarded. They also call [`Sink::discard_buffered`], which drops the
//! audio the sink itself had queued but not yet made audible — for device
//! output, the contents of the device ring. Abandoning the session without
//! that leaves up to a full device ring of the *abandoned* position playing
//! on afterwards, which is precisely how a transport command comes to feel
//! late. **Next is drain-and-restart**: a fresh session starts at the next
//! queue position, meaning a new decode of that track (first audio within
//! milliseconds for local files) rather than a sample-accurate splice out of
//! the running stream. That trade is deliberate for v0.1; the gapless path
//! stays reserved for its one guarantee — *adjacent* tracks playing to
//! completion.
//!
//! **Previous is the same drain-and-restart as Next**, aimed at whichever
//! queue position the conventional rule selects: at or past
//! [`PREVIOUS_RESTART_MS`] into the current track it aims at that track again,
//! before it at the one before, and at the head of the queue it restarts
//! because there is nothing before position 0. Restarting and stepping back
//! are the *same* operation with a different index — deliberately, so the two
//! halves of one button cannot drift apart in latency or in what they discard.
//! Its cost is `Next`'s cost, and its position reading is the one the module's
//! "Elapsed time" section describes, lead and all: the ~0.19 s a device buffer
//! can put between the counter and the speaker is two orders of magnitude
//! below the threshold it is compared against.
//!
//! **Seek is the same drain-and-restart**, aimed at the *current* queue
//! position instead of the next one, with the new session's first track
//! opened and [`AudioSource::seek`]ed to the target before its first block is
//! pushed. The cost is identical and identically documented: the running
//! session's undelivered ring audio *and* the sink's buffered audio are
//! discarded and the target track is decoded afresh, so first audio at the
//! new position arrives within tens of milliseconds rather than instantly.
//! What the listener does **not** hear in between is the old position: the
//! discard is what keeps the gap a short silence rather than a fifth of a
//! second of audio the user already asked to leave. Two further consequences
//! worth stating plainly:
//!
//! - **Seeking while paused** moves the position and stays paused: the new
//!   session is created in the paused state, so not one sample reaches the
//!   sink until the next [`Command::Play`]. An [`Event::Progress`] is emitted
//!   immediately so the position is never stale on screen.
//! - **Seeking at or past the end of the track** is [`Command::Next`]: the
//!   following queue position starts from its beginning, or the queue ends.
//!   The engine decides this from the track length it already knows; when a
//!   length was never declared, [`AudioSource::seek`] reports the overrun and
//!   the producer skips that track instead.
//!
//! # Elapsed time
//!
//! [`Event::Progress::elapsed_ms`] is `seek target + delivered audio since
//! the current track began`, where "delivered audio" is counted **in output
//! frames at the session's stream rate** — not in the source file's frames.
//!
//! That distinction is the whole correctness argument, and it matters
//! exactly when the two rates differ — which, since ADR-0009, means a fixed
//! output rate was chosen or the device would not follow the source. A 48 kHz
//! track played into a 44.1 kHz stream is resampled before it reaches the
//! ring, so one second of that track occupies 44 100 delivered frames, not
//! 48 000. Dividing delivered frames by the *file's* rate would report a
//! 60-second track as running 55.1 seconds — wrong by 8 %, and wrong in a way
//! that grows over the track. Dividing by the stream rate is wall-clock true
//! by construction, because the stream rate is the rate the audio is
//! actually being consumed at. The producer therefore publishes the
//! negotiated stream rate to the engine thread and the arithmetic uses only
//! that.
//!
//! [`Event::Progress::track_ms`] is the track's own length, computed from
//! its native frame count at its native rate, so it is unaffected by
//! resampling — as it must be: converting a track's sample rate does not
//! change how long it plays for.
//!
//! Two honest caveats:
//!
//! - Position is measured at the **sink**, so with device output it leads
//!   what is audible by up to one device ring (~0.19 s at the size the app
//!   uses), the same ordinary output latency the pause docs above describe.
//!   The lead is a steady-state property of continuous playback only: every
//!   command that abandons a session empties the sink's buffer as part of
//!   abandoning it, so a seek's own [`Event::Progress`] is never reporting
//!   across a bufferful of stale audio.
//! - `track_ms` is `None` for a stream that declares no length. Progress is
//!   still reported; there is simply no total to render against.
//!
//! # Shutdown
//!
//! Dropping the [`EngineHandle`] (or calling [`EngineHandle::shutdown`])
//! closes the command channel; the engine thread aborts any session, joins
//! its workers, drops the sink, and exits. The drop blocks until that
//! completes — bounded by at most one decode block per worker — so no
//! threads outlive the handle.
//!
//! # Event delivery
//!
//! Events arrive on a single `std::sync::mpsc` [`Receiver`] returned at
//! spawn: **one consumer** by design. A front end that needs fan-out (GUI +
//! remote transport at once) must forward from this receiver itself;
//! broadcast delivery is future protocol-layer work. If the receiver is
//! dropped, further events are discarded and the engine keeps running.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rtrb::RingBuffer;

#[cfg(feature = "device-output")]
use crate::playback::OutputMode;
#[cfg(feature = "device-output")]
use crate::playback::device::DeviceSink;
use crate::playback::engine::push_with_backpressure;
#[cfg(all(target_os = "linux", feature = "exclusive-output"))]
use crate::playback::exclusive::ExclusiveSink;
use crate::playback::resample::resample_interleaved;
use crate::playback::source::frames_to_ms;
use crate::playback::{
    AudioSource, BoundaryPolicy, CHANNELS, DecodedAudio, EngineConfig, OfflineSink, PlaybackError,
    Sink,
};
use crate::protocol::{Command, ConversionReason, Event, SignalChain, VolumePath};
use crate::replaygain::{
    ReplayGainDecision, ReplayGainSettings, ReplayGainState, ReplayGainTags, SharedReplayGain,
};
use crate::volume::{Fader, SharedVolume, Volume, VolumeState};

/// Sleep per engine-loop iteration while paused: long enough to idle
/// cheaply, short enough that resume feels instant.
const PAUSED_POLL: Duration = Duration::from_millis(2);
/// Sleep when the ring is empty but the producer is still working
/// (mirrors `playback::engine::consume`).
const STARVED_POLL: Duration = Duration::from_micros(50);
/// Producer-side poll while it waits for the engine thread to grant a stream
/// rate (see "Rate negotiation"). The engine answers on its very next loop
/// iteration, so this is a handful of wake-ups at worst.
const NEGOTIATE_POLL: Duration = Duration::from_micros(100);
/// [`SessionShared::rate_change_at`] sentinel for "the session ran to the end
/// of the queue"; any other value is the queue index where the sample rate
/// changed and a fresh session must take over.
const NO_RATE_CHANGE: usize = usize::MAX;
/// [`Event::Progress`] cadence divisor: one report per `1/PROGRESS_HZ` of
/// *delivered audio*. Deriving the cadence from the sample counter rather
/// than a clock keeps it exactly 4 Hz of playing time (never faster when the
/// pump runs ahead, never slower when it is starved) and keeps the check on
/// the engine loop down to one integer comparison.
const PROGRESS_HZ: u32 = 4;

/// How far into a track [`Command::Previous`] stops meaning "the track before
/// this one" and starts meaning "this one again", in milliseconds.
///
/// Three seconds is the convention every transport with this button uses, and
/// it is a convention rather than a measurement — but it is not arbitrary, and
/// the two failure modes it sits between are not symmetric:
///
/// - **Too short** and the button becomes a restart button. A listener two
///   seconds into a track they did not want has to press it twice, and the
///   second press is the one that does what they meant by the first.
/// - **Too long** and it becomes a skip-back button, taking away the ordinary
///   "start this again" gesture partway through a song.
///
/// Three seconds is comfortably past the moment a listener recognises the
/// track that just started (so a wrong-track correction is still one press)
/// and comfortably short of anywhere anyone deliberately listens from (so
/// restarting is still available where it is wanted). It is also an order of
/// magnitude larger than the position readout's own worst-case lead over what
/// is audible — up to one device buffer, ~0.19 s at the app's settings — so
/// that lead cannot move a press across the boundary.
///
/// It is public because a front end labelling or explaining the control should
/// quote the engine's number rather than a copy of it.
pub const PREVIOUS_RESTART_MS: u64 = 3_000;

/// The engine could not accept the command because its thread has shut
/// down (the handle was already consumed by shutdown, or the engine
/// thread terminated).
#[derive(Debug, thiserror::Error)]
#[error("the engine has shut down")]
pub struct EngineClosed;

/// A front end's connection to a running engine: send [`Command`]s, observe
/// progress, shut down.
///
/// Dropping the handle shuts the engine down cleanly (see the module docs).
#[derive(Debug)]
pub struct EngineHandle {
    commands: Option<Sender<Command>>,
    thread: Option<JoinHandle<()>>,
    delivered: Arc<AtomicUsize>,
    instruments: Arc<Instruments>,
    volume: Arc<SharedVolume>,
    replay_gain: Arc<SharedReplayGain>,
}

/// A running count of the conversions the engine has performed, readable from
/// an [`EngineHandle`] at any time.
///
/// Under the ADR-0009 default every field here stays at zero for as long as
/// the output device can run at the rates the music is stored at, which is the
/// ordinary case; the counters exist so that "no resampler was constructed" is
/// an assertable fact rather than an inference from a stopwatch. The
/// per-track, user-facing version of the same story is
/// [`Event::SignalPath`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Conversions {
    /// Tracks the engine has sample-rate converted since it spawned.
    pub resampled_tracks: usize,
    /// Wall time spent inside the resampler, in milliseconds. Exactly `0.0`
    /// when `resampled_tracks` is 0 — no resampler was ever constructed.
    pub resample_ms: f64,
    /// Times the output stream has been reconfigured to a different rate:
    /// once per rate change the queue asked for and the device granted.
    pub output_reconfigurations: usize,
}

/// The atomics behind [`Conversions`]. Written by producer threads (resample)
/// and the engine thread (reconfiguration), read by the handle from any
/// thread; relaxed ordering throughout because these are counters nothing
/// synchronizes on.
#[derive(Debug, Default)]
struct Instruments {
    resampled_tracks: AtomicUsize,
    resample_ns: AtomicU64,
    reconfigurations: AtomicUsize,
}

impl Instruments {
    fn record_resample(&self, elapsed: Duration) {
        self.resampled_tracks.fetch_add(1, Ordering::Relaxed);
        let ns = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.resample_ns.fetch_add(ns, Ordering::Relaxed);
    }

    fn snapshot(&self) -> Conversions {
        // Nanosecond totals are far below f64's exact-integer range for any
        // plausible session.
        #[allow(clippy::cast_precision_loss)]
        let resample_ms = self.resample_ns.load(Ordering::Relaxed) as f64 / 1.0e6;
        Conversions {
            resampled_tracks: self.resampled_tracks.load(Ordering::Relaxed),
            resample_ms,
            output_reconfigurations: self.reconfigurations.load(Ordering::Relaxed),
        }
    }
}

impl EngineHandle {
    /// Send a command to the engine.
    ///
    /// # Errors
    ///
    /// [`EngineClosed`] if the engine thread is no longer running.
    pub fn send(&self, command: Command) -> Result<(), EngineClosed> {
        self.commands
            .as_ref()
            .ok_or(EngineClosed)?
            .send(command)
            .map_err(|_| EngineClosed)
    }

    /// Total interleaved samples delivered to the sink since spawn,
    /// monotonically increasing across tracks and sessions. Divide by
    /// [`CHANNELS`] for frames. While paused this value does not advance —
    /// the tests use exactly that as the pause guarantee.
    #[must_use]
    pub fn samples_delivered(&self) -> usize {
        self.delivered.load(Ordering::Acquire)
    }

    /// What the engine has converted since it spawned — see [`Conversions`].
    ///
    /// All zeroes is the expected reading for a device that can run at the
    /// music's own rates.
    #[must_use]
    pub fn conversions(&self) -> Conversions {
        self.instruments.snapshot()
    }

    /// The volume, the mute state, and where the volume is being applied —
    /// the pull-side twin of [`Event::VolumeChanged`].
    ///
    /// A front end coming up mid-session reads this once to draw its control
    /// in the right place, then follows the event stream. `VolumeState`'s docs
    /// note the one caveat: the three fields are loaded independently, so a
    /// read racing a change may mix old and new. The events are the ordered
    /// account; this is the snapshot.
    #[must_use]
    pub fn volume(&self) -> VolumeState {
        self.volume.snapshot()
    }

    /// The ReplayGain settings and what they resolved to for the track now
    /// playing — the pull-side twin of [`Event::ReplayGainChanged`], on the
    /// same terms as [`Self::volume`] (a snapshot; the events are the ordered
    /// account).
    ///
    /// A freshly spawned engine reports
    /// [`ReplayGainMode::Off`](crate::protocol::ReplayGainMode::Off) and
    /// [`ReplayGainDecision::UNITY`], so a front end that never sends
    /// [`Command::SetReplayGain`] can read this once and know that nothing is
    /// being applied.
    #[must_use]
    pub fn replay_gain(&self) -> ReplayGainState {
        self.replay_gain.snapshot()
    }

    /// Shut the engine down and wait for its threads to finish. Equivalent
    /// to dropping the handle; provided so intent reads explicitly.
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        // Closing the command channel is the shutdown signal; the engine
        // thread observes the disconnect, aborts any session, and exits.
        self.commands = None;
        if let Some(handle) = self.thread.take() {
            // A panicked engine thread is a bug (docs/ENGINEERING.md); all
            // drop can do is not propagate it into the caller's unwind.
            let _ = handle.join();
        }
    }
}

/// The collected output of an engine spawned with [`spawn_offline`].
#[derive(Debug)]
pub struct OfflineOutput {
    output: Receiver<Vec<f32>>,
}

impl OfflineOutput {
    /// Wait for the engine to shut down and return every interleaved stereo
    /// sample it delivered to the sink, in order.
    ///
    /// This blocks until the engine thread exits, so shut the engine down
    /// first (drop the [`EngineHandle`]) or call this from another thread.
    /// Returns `None` only if the engine thread died without reporting —
    /// i.e. it panicked, which is a bug.
    #[must_use]
    pub fn wait(self) -> Option<Vec<f32>> {
        self.output.recv().ok()
    }
}

/// Spawn a headless engine delivering into an [`OfflineSink`] with room for
/// `capacity_samples` interleaved samples (the sink never grows; overflow is
/// dropped and counted, per its contract).
///
/// Returns the control handle, the event receiver (single consumer — see
/// the module docs), and the [`OfflineOutput`] that yields the delivered
/// samples after shutdown.
///
/// An offline sink has no rate of its own ([`Sink::negotiate_rate`]), so every
/// session runs at its own first track's native rate and nothing is ever
/// resampled — the headless configuration exercises the bit-perfect default,
/// not a fallback.
///
/// # Errors
///
/// [`PlaybackError::Io`] if the engine thread cannot be spawned.
pub fn spawn_offline(
    cfg: EngineConfig,
    capacity_samples: usize,
) -> Result<(EngineHandle, Receiver<Event>, OfflineOutput), PlaybackError> {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    let (out_tx, out_rx) = mpsc::channel();
    let delivered = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&delivered);
    let instruments = Arc::new(Instruments::default());
    let probes = Arc::clone(&instruments);
    let volume = Arc::new(SharedVolume::default());
    let gain = Arc::clone(&volume);
    let replay_gain = Arc::new(SharedReplayGain::default());
    let loudness = Arc::clone(&replay_gain);
    let thread = thread::Builder::new()
        .name("baz-engine".into())
        .spawn(move || {
            let control = Control::new(
                cmd_rx,
                event_tx,
                cfg,
                0,
                Observable {
                    delivered: counter,
                    instruments: probes,
                    volume: gain,
                    replay_gain: loudness,
                },
                OfflineSink::with_capacity(capacity_samples),
            );
            let sink = control.run();
            let _ = out_tx.send(sink.into_samples());
        })?;
    Ok((
        EngineHandle {
            commands: Some(cmd_tx),
            thread: Some(thread),
            delivered,
            instruments,
            volume,
            replay_gain,
        },
        event_rx,
        OfflineOutput { output: out_rx },
    ))
}

/// The output a device engine is playing through, so that one [`Control`] can
/// drive either backend.
///
/// A `Sink` implementation that forwards to whichever variant is present.
/// Every method is a one-line delegation, and the defaults the trait provides
/// are *not* re-implemented here — an added trait method with a sensible
/// default must not be silently swallowed by the wrapper.
#[cfg(feature = "device-output")]
enum Output {
    /// cpal, mixed with the rest of the system (ADR-0009).
    Shared(DeviceSink),
    /// An ALSA `hw:` device baz holds itself (ADR-0012).
    #[cfg(all(target_os = "linux", feature = "exclusive-output"))]
    Exclusive(ExclusiveSink),
}

#[cfg(feature = "device-output")]
impl Sink for Output {
    fn write(&mut self, samples: &[f32]) {
        match self {
            Self::Shared(sink) => sink.write(samples),
            #[cfg(all(target_os = "linux", feature = "exclusive-output"))]
            Self::Exclusive(sink) => sink.write(samples),
        }
    }

    fn discard_buffered(&mut self) {
        match self {
            Self::Shared(sink) => sink.discard_buffered(),
            #[cfg(all(target_os = "linux", feature = "exclusive-output"))]
            Self::Exclusive(sink) => sink.discard_buffered(),
        }
    }

    fn negotiate_rate(&mut self, desired: u32) -> Option<u32> {
        match self {
            Self::Shared(sink) => sink.negotiate_rate(desired),
            #[cfg(all(target_os = "linux", feature = "exclusive-output"))]
            Self::Exclusive(sink) => sink.negotiate_rate(desired),
        }
    }

    fn drain_buffered(&mut self) {
        match self {
            Self::Shared(sink) => sink.drain_buffered(),
            #[cfg(all(target_os = "linux", feature = "exclusive-output"))]
            Self::Exclusive(sink) => sink.drain_buffered(),
        }
    }

    fn set_device_volume(&mut self, gain: f32) -> Option<()> {
        match self {
            Self::Shared(sink) => sink.set_device_volume(gain),
            #[cfg(all(target_os = "linux", feature = "exclusive-output"))]
            Self::Exclusive(sink) => sink.set_device_volume(gain),
        }
    }

    fn is_exclusive(&self) -> bool {
        match self {
            Self::Shared(sink) => sink.is_exclusive(),
            #[cfg(all(target_os = "linux", feature = "exclusive-output"))]
            Self::Exclusive(sink) => sink.is_exclusive(),
        }
    }
}

/// Open the output `mode` asks for, on the engine thread.
///
/// Failure is failure: an exclusive open that cannot happen — the device is
/// busy, the name is not one of this machine's, the platform has no exclusive
/// backend — is reported, never quietly downgraded to shared mode. A listener
/// who asked baz to hold the card and got the sound server instead would have
/// been told the wrong thing about their signal path, which is the one outcome
/// ADR-0009 and ADR-0012 both exist to prevent.
#[cfg(feature = "device-output")]
fn open_output(
    mode: &OutputMode,
    sample_rate: u32,
    ring_frames: usize,
) -> Result<Output, PlaybackError> {
    match mode {
        OutputMode::Shared => DeviceSink::open(sample_rate, ring_frames).map(Output::Shared),
        #[cfg(all(target_os = "linux", feature = "exclusive-output"))]
        OutputMode::Exclusive { device } => {
            let chosen = crate::playback::exclusive::choose(device.as_deref())?;
            ExclusiveSink::open(&chosen, sample_rate, ring_frames).map(Output::Exclusive)
        }
        #[cfg(not(all(target_os = "linux", feature = "exclusive-output")))]
        OutputMode::Exclusive { .. } => Err(PlaybackError::Device(
            "exclusive output is not built into this baz: it needs the `exclusive-output` \
             feature, and today only Linux (ALSA hw:) has a backend — WASAPI exclusive and \
             CoreAudio hog mode are unwritten (ADR-0012)"
                .into(),
        )),
    }
}

/// Spawn an engine playing through the audio output the listener configured,
/// with a device ring of `device_ring_frames` frames.
///
/// # Which output
///
/// This is the resolution point for [`OutputMode::from_env`]: `BAZ_OUTPUT`
/// (and `BAZ_OUTPUT_DEVICE` with it) decides between shared mode — cpal, the
/// default, and what every earlier version of baz did — and exclusive mode, in
/// which baz holds an ALSA `hw:` device itself and nothing sits between the
/// decoder and the converter (ADR-0012). A front end with a settings surface
/// of its own should call [`spawn_device_with`] and pass the mode explicitly
/// rather than exporting variables into its own process; this function exists
/// so that opting in needs no front-end change at all.
///
/// # Errors
///
/// Whatever [`spawn_device_with`] reports, plus [`PlaybackError::Device`] if
/// `BAZ_OUTPUT` names something that is not an output mode.
///
/// `initial_sample_rate` is the rate the device is opened at *before any queue
/// exists* — the engine has to hold an open sink from the moment it spawns, and
/// nothing is known about the music yet. It is a starting point, not a policy:
/// under the ADR-0009 default every session renegotiates the stream to the rate
/// of the track that starts it, so a 48 kHz album ends up playing at 48 kHz
/// whatever this argument said. Pick a rate every device accepts (44 100 Hz)
/// and let negotiation do the rest.
///
/// The stream stays open across pause, seek, skip and stop; the only thing
/// that reopens it is a session starting at a rate the currently open stream
/// is not running at.
///
/// # Errors
///
/// [`PlaybackError::Device`] if no output device is usable at
/// `initial_sample_rate`; [`PlaybackError::Io`] if the engine thread cannot be
/// spawned.
#[cfg(feature = "device-output")]
pub fn spawn_device(
    cfg: EngineConfig,
    initial_sample_rate: u32,
    device_ring_frames: usize,
) -> Result<(EngineHandle, Receiver<Event>), PlaybackError> {
    spawn_device_with(
        cfg,
        &OutputMode::from_env()?,
        initial_sample_rate,
        device_ring_frames,
    )
}

/// Spawn an engine playing through an explicitly chosen output arrangement.
///
/// [`spawn_device`] is this function with the mode read from the environment;
/// this one is for a front end that has its own setting to honour, and it is
/// the whole of what wiring an output picker costs.
///
/// Everything else is identical, including `initial_sample_rate`'s meaning:
/// the rate the device is opened at *before any queue exists*, renegotiated to
/// the music's own rate by the first session either way (ADR-0009).
///
/// # Errors
///
/// - [`PlaybackError::DeviceBusy`] if exclusive mode was asked for and another
///   application — usually the sound server — holds the device. This is the
///   ordinary failure and it is reported rather than worked around: baz does
///   not fall back to shared mode behind the listener's back, because the
///   whole point of the setting is what it claims about the signal path.
/// - [`PlaybackError::Device`] if the output cannot be opened for any other
///   reason: no device, an unknown device name, several devices and none
///   chosen, or a platform with no exclusive backend.
/// - [`PlaybackError::Io`] if the engine thread cannot be spawned.
#[cfg(feature = "device-output")]
pub fn spawn_device_with(
    cfg: EngineConfig,
    output: &OutputMode,
    initial_sample_rate: u32,
    device_ring_frames: usize,
) -> Result<(EngineHandle, Receiver<Event>), PlaybackError> {
    let output = output.clone();
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    let (ack_tx, ack_rx) = mpsc::channel();
    let delivered = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&delivered);
    let instruments = Arc::new(Instruments::default());
    let probes = Arc::clone(&instruments);
    let volume = Arc::new(SharedVolume::default());
    let gain = Arc::clone(&volume);
    let replay_gain = Arc::new(SharedReplayGain::default());
    let loudness = Arc::clone(&replay_gain);
    let thread = thread::Builder::new()
        .name("baz-engine".into())
        .spawn(move || {
            // cpal streams are not Send, so the sink must be created (and
            // dropped) on the engine thread; the open result is reported
            // back through a one-shot channel. Reopening for a rate change
            // happens on this same thread for the same reason. An ALSA PCM
            // *is* Send, but it is opened here too: exclusive means the
            // handle must be released on the same thread's shutdown path,
            // and one arrangement for both backends is one thing to reason
            // about rather than two.
            match open_output(&output, initial_sample_rate, device_ring_frames) {
                Ok(sink) => {
                    let _ = ack_tx.send(Ok(()));
                    let control = Control::new(
                        cmd_rx,
                        event_tx,
                        cfg,
                        initial_sample_rate,
                        Observable {
                            delivered: counter,
                            instruments: probes,
                            volume: gain,
                            replay_gain: loudness,
                        },
                        sink,
                    );
                    drop(control.run()); // closes the device stream
                }
                Err(e) => {
                    let _ = ack_tx.send(Err(e));
                }
            }
        })?;
    let handle = EngineHandle {
        commands: Some(cmd_tx),
        thread: Some(thread),
        delivered,
        instruments,
        volume,
        replay_gain,
    };
    match ack_rx.recv() {
        Ok(Ok(())) => Ok((handle, event_rx)),
        Ok(Err(e)) => Err(e), // dropping `handle` joins the (finished) thread
        Err(_) => Err(PlaybackError::Device(
            "engine thread terminated while opening the device".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Engine (control + pump) thread
// ---------------------------------------------------------------------------

struct Control<S: Sink> {
    commands: Receiver<Command>,
    events: Sender<Event>,
    cfg: EngineConfig,
    /// The rate the sink is currently running at, as it last reported through
    /// [`Sink::negotiate_rate`]. Zero for a sink with no rate of its own
    /// (offline), which is also why a zero here never counts as a
    /// reconfiguration.
    open_rate: u32,
    /// Whether the sink holds its device exclusively ([`Sink::is_exclusive`]),
    /// read once when the engine thread starts because the arrangement is a
    /// property of the sink rather than of a moment. It is the ADR-0012 half of
    /// [`Event::SignalPath`].
    exclusive: bool,
    delivered: Arc<AtomicUsize>,
    instruments: Arc<Instruments>,
    queue: Vec<PathBuf>,
    /// Queue index where the next idle-state `Play` starts.
    position: usize,
    paused: bool,
    session: Option<Session>,
    /// Volume state shared with [`EngineHandle`] and read by the pump path
    /// (from [`Observable::volume`]). This thread is its only writer.
    volume: Arc<SharedVolume>,
    /// The gain applicator. Engine-thread state; see [`crate::volume`].
    ///
    /// One fader for both gains: the volume and ReplayGain are multiplied
    /// together before they reach it, so the pump does one multiply per sample
    /// whether none, one or both are engaged (ADR-0013).
    fader: Fader,
    /// Where the volume is currently being applied, as last reported.
    volume_path: VolumePath,
    /// ReplayGain state shared with [`EngineHandle`]. This thread is its only
    /// writer, and the pump path never reads it — the resolved gain is folded
    /// into [`SharedVolume`]'s one number.
    replay_gain: Arc<SharedReplayGain>,
    /// How ReplayGain is configured. Engine state, not session state: it
    /// survives every transport command exactly as the volume does.
    rg_settings: ReplayGainSettings,
    /// What those settings resolved to for the track currently being
    /// delivered, as last reported.
    rg_applied: ReplayGainDecision,
    /// Scratch for the scaled block: one pump chunk, allocated when the engine
    /// thread starts and never grown.
    ///
    /// A `Box<[f32]>` rather than a `Vec<f32>` on purpose — a boxed slice has
    /// no `push`, `extend` or `resize`, so "the pump path never allocates" is
    /// something the type forbids rather than something a comment asks for
    /// (`docs/ENGINEERING.md`: enforced by construction, with types that do not
    /// implement the tempting shortcuts).
    scratch: Box<[f32]>,
    sink: S,
}

/// The state an [`EngineHandle`] can read while the engine runs: the delivered
/// -sample counter, the conversion counters, and the volume.
///
/// Grouped because they travel together — one `Arc` each, created by the
/// spawner, cloned into the engine thread, and read from any thread — and
/// because passing three of them alongside five other arguments is how a
/// constructor stops being readable.
#[derive(Debug, Default)]
struct Observable {
    delivered: Arc<AtomicUsize>,
    instruments: Arc<Instruments>,
    volume: Arc<SharedVolume>,
    replay_gain: Arc<SharedReplayGain>,
}

impl<S: Sink> Control<S> {
    fn new(
        commands: Receiver<Command>,
        events: Sender<Event>,
        cfg: EngineConfig,
        open_rate: u32,
        observable: Observable,
        sink: S,
    ) -> Self {
        let Observable {
            delivered,
            instruments,
            volume,
            replay_gain,
        } = observable;
        let rg_settings = ReplayGainSettings::default();
        // `SharedReplayGain::default` already holds exactly this, so a handle
        // read before the engine thread's first iteration is correct. Doing it
        // again here is what keeps the engine's own copy and the shared one
        // from being two independent claims about the same defaults.
        replay_gain.publish(rg_settings, ReplayGainDecision::UNITY);
        Self {
            commands,
            events,
            cfg,
            open_rate,
            exclusive: sink.is_exclusive(),
            delivered,
            instruments,
            queue: Vec::new(),
            position: 0,
            paused: false,
            session: None,
            volume,
            fader: Fader::default(),
            volume_path: VolumePath::Unity,
            replay_gain,
            rg_settings,
            rg_applied: ReplayGainDecision::UNITY,
            // One pump chunk is the most `pump` can ever hand to the fader in
            // one call (the ring read is `min`'d against it), so this is
            // exactly enough and is allocated before any audio flows.
            scratch: vec![0.0; cfg.consumer_chunk_frames * CHANNELS].into_boxed_slice(),
            sink,
        }
    }

    /// The engine loop. Returns the sink at shutdown so spawners can hand
    /// its contents back (offline) or drop it in place (device).
    fn run(mut self) -> S {
        loop {
            if self.session.is_some() {
                // Active session: stay responsive to commands between pump
                // iterations without ever blocking on the channel.
                match self.commands.try_recv() {
                    Ok(cmd) => {
                        self.handle(cmd);
                        continue; // drain all pending commands first
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => break,
                }
                self.tick();
            } else {
                // Idle: block until a command arrives or the handle drops.
                match self.commands.recv() {
                    Ok(cmd) => self.handle(cmd),
                    Err(_) => break,
                }
            }
        }
        // Shutdown: dropping the session sets its stop flag and joins the
        // producer (and its prefetch) — bounded, no leaked threads.
        self.session = None;
        self.sink
    }

    /// One pump-and-report iteration of an active session.
    fn tick(&mut self) {
        // Ahead of the pause gate on purpose: a session created by a
        // seek-while-paused is still waiting to be told its stream rate, and
        // gating that would leave its producer parked until the user hit play.
        self.settle_rate();
        if self.paused {
            // The gate: no pulls, so the sink sees nothing until resume and
            // the ring (plus producer backpressure) preserves every sample.
            thread::sleep(PAUSED_POLL);
            return;
        }
        // Take delivery of anything the producer has published since the last
        // iteration — before the pump, not after, because a track boundary the
        // engine has not heard about yet is a boundary the pump would read
        // straight through at the previous track's ReplayGain.
        if let Some(session) = self.session.as_mut() {
            session.absorb();
            if session.advance_active() {
                // A new track is now the one being delivered. Resolve its
                // ReplayGain and fold it into the gain the pump reads, before
                // a single one of its samples is pulled.
                self.settle_replay_gain(false);
            }
        }
        // The pump path's one volume read: a single acquire load of the
        // effective gain — the volume and ReplayGain already multiplied
        // together — taken once per block and never per sample. This is where
        // a volume change becomes audible, which is what bounds "takes effect
        // promptly" at one pump iteration.
        self.fader.aim(self.volume.gain());
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let chunk_samples = self.cfg.consumer_chunk_frames * CHANNELS;
        let pumped = session.pump(
            &mut self.sink,
            chunk_samples,
            &self.delivered,
            &mut self.fader,
            &mut self.scratch,
        );
        session.report(&self.events, false);
        if session.complete() {
            session.report(&self.events, true);
            let resume_at = session.rate_change_at();
            self.session = None; // joins the (already finished) producer
            if let Some(next) = resume_at {
                self.continue_at_new_rate(next);
                return;
            }
            let _ = self.events.send(Event::QueueEnded);
            self.position = 0;
            // No track is being delivered any more, so the ReplayGain that was
            // being applied to one is no longer the state of anything.
            self.settle_replay_gain(false);
            return;
        }
        // Between pump iterations, never inside one: `pump` above is the
        // realtime-disciplined path and stays a ring read plus a sink write.
        if session.progress_due() {
            self.emit_progress();
        }
        if pumped {
            if !self.cfg.consumer_pace.is_zero() {
                thread::sleep(self.cfg.consumer_pace);
            }
        } else {
            thread::sleep(STARVED_POLL);
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::SetQueue { paths } => {
                self.stop_session();
                self.queue = paths;
                self.position = 0;
            }
            Command::Play => {
                if self.session.is_some() {
                    if self.paused {
                        self.paused = false;
                        let _ = self.events.send(Event::Resumed);
                        // Resumed is always followed by a fresh reading, so
                        // a front end that dropped the position while paused
                        // has it back before the first frame is drawn.
                        self.emit_progress();
                    }
                } else {
                    self.start_session(self.position, 0, None);
                }
            }
            Command::Pause => {
                if self.session.is_some() && !self.paused {
                    self.paused = true;
                    let _ = self.events.send(Event::Paused);
                }
            }
            Command::Stop => {
                self.stop_session();
                self.position = 0;
            }
            Command::Next => {
                if let Some(session) = self.session.take() {
                    let next = session.current + 1;
                    drop(session); // abort: stop flag + join
                    // The skipped track's audio is gone from the session ring;
                    // drop the copy already sitting in the sink too.
                    self.sink.discard_buffered();
                    self.paused = false;
                    self.start_session(next, 0, None);
                }
            }
            Command::Previous => self.previous(),
            Command::Seek { position_ms } => self.seek(position_ms),
            Command::SetVolume { position } => {
                let volume = Volume::new(position);
                if volume != self.volume.volume() {
                    self.volume.set_volume(volume);
                    self.apply_volume();
                }
            }
            Command::SetMute { muted } => {
                if muted != self.volume.muted() {
                    self.volume.set_muted(muted);
                    self.apply_volume();
                }
            }
            Command::SetReplayGain {
                mode,
                preamp_centidb,
                no_tag_preamp_centidb,
                prevent_clipping,
            } => {
                let settings = ReplayGainSettings::new(
                    mode,
                    preamp_centidb,
                    no_tag_preamp_centidb,
                    prevent_clipping,
                );
                if settings != self.rg_settings {
                    self.rg_settings = settings;
                    self.settle_replay_gain(true);
                }
            }
        }
    }

    /// Resolve ReplayGain for the track now being delivered, put the result
    /// into effect, and say so.
    ///
    /// Called on the occasions the answer can change: an accepted
    /// [`Command::SetReplayGain`] (`announce`, because the listener asked and
    /// deserves a confirmation even if the number happens to be the same), a
    /// **track boundary** the pump has reached, and the two moments a session
    /// ends and there is no longer a track to have a ReplayGain. The last two
    /// are announced only if the figure actually moved, so an album in album
    /// mode narrates its ReplayGain once.
    ///
    /// The resolved gain does not go anywhere of its own: it is handed to
    /// [`Self::settle_volume`], which multiplies it by the volume and publishes
    /// the single number the pump path reads. That is the whole of "sharing the
    /// volume's machinery" — there is no second gain stage to be out of step
    /// with the first, and [`VolumePath`] keeps answering the fidelity question
    /// for both.
    fn settle_replay_gain(&mut self, announce: bool) {
        let tags = self
            .session
            .as_ref()
            .map_or_else(ReplayGainTags::default, Session::active_replay_gain);
        let applied = self.rg_settings.resolve(tags);
        let news = applied != self.rg_applied;
        self.rg_applied = applied;
        self.replay_gain.publish(self.rg_settings, applied);
        if !(announce || news) {
            return;
        }
        // Republish the combined gain *first*. Announces `VolumeChanged` only
        // if the *path* changed — engaging ReplayGain moves the path off
        // `Unity`, which is exactly the news a fidelity indicator wants and the
        // only volume news there is here.
        //
        // Ordering is a contract, not a detail: every piece of shared state is
        // published before any event announcing it. A front end that observes
        // an event and then reads `volume()` or `replay_gain()` must never see
        // a value older than the news it just received. Emitting first left a
        // window in which `ReplayGainChanged` was visible while `volume()`
        // still said `Unity` — Linux almost always lost that race to the
        // reader, Windows did not, which is how CI found it.
        self.settle_volume(false);
        let _ = self.events.send(Event::ReplayGainChanged {
            mode: self.rg_settings.mode,
            preamp_centidb: self.rg_settings.preamp_centidb,
            no_tag_preamp_centidb: self.rg_settings.no_tag_preamp_centidb,
            prevent_clipping: self.rg_settings.prevent_clipping,
            source: applied.source,
            applied_centidb: applied.gain_centidb,
            clipping_prevented: applied.clipping_prevented,
        });
    }

    /// The gain the sample stream should end up carrying: the taper's
    /// amplitude, or silence while muted.
    ///
    /// Mute is folded in here rather than kept as a second thing the pump has
    /// to consult, so the realtime path reads one number and multiplies by it
    /// (see [`crate::volume`] for why mute is nevertheless separate *state*).
    fn effective_gain(&self) -> f32 {
        if self.volume.muted() {
            0.0
        } else {
            self.volume.volume().amplitude()
        }
    }

    /// Put the current volume into effect and say where it landed.
    ///
    /// The device is offered the gain first ([`Sink::set_device_volume`]):
    /// when a sink takes it, the fader stays at unity and the sample stream is
    /// passed through untouched, which is the whole reason the offer is made
    /// before the fallback rather than after. No backend baz ships takes it
    /// today (ADR-0011), so in practice this settles on software gain — and
    /// reports exactly that.
    ///
    /// The slew is skipped whenever nothing is audible (stopped, or paused):
    /// there is no discontinuity to hide in silence, and jumping is what lets
    /// a front end set a volume *before* pressing play and get it from the
    /// first sample.
    fn apply_volume(&mut self) {
        self.settle_volume(true);
    }

    /// Offer the gain to the device again after the output was rebuilt.
    ///
    /// [`Sink::negotiate_rate`] may replace the stream wholesale (the device
    /// backend does exactly that), and a fresh stream carries none of the old
    /// one's settings — so the arrangement has to be re-made rather than
    /// assumed to have survived. Announced only if the *path* changed, so an
    /// ordinary mixed-rate queue does not narrate its volume at every
    /// boundary.
    fn reestablish_volume(&mut self) {
        self.settle_volume(false);
    }

    /// Whether a gain change would be inaudible, so the fader may jump to it
    /// rather than slew.
    ///
    /// True while stopped, while paused, and for a session that has not
    /// delivered a sample yet — the last being what makes "set the volume (or
    /// the ReplayGain), then press play" exact from the very first sample
    /// instead of 20 ms later, and what keeps a seek's fresh session from
    /// opening at the gain the previous one ended on.
    fn nothing_audible(&self) -> bool {
        self.paused || self.session.as_ref().is_none_or(|s| s.pulled == 0)
    }

    /// The body of both: offer the volume to the sink, fold ReplayGain in,
    /// publish the one gain the pump reads, and report.
    ///
    /// # Two inputs, one stage
    ///
    /// The volume and ReplayGain multiply together into a single number
    /// ([`SharedVolume::set_gain`]), so the pump does one multiply per sample
    /// however many gains are engaged. The *device* is only ever offered the
    /// volume: an attenuator downstream of baz cannot carry a per-track
    /// ReplayGain, so when a sink takes the volume the ReplayGain still has to
    /// be applied here — and the path is then [`VolumePath::SoftwareGain`],
    /// because baz is scaling the samples whatever the device is also doing.
    /// Reporting `DeviceAttenuator` there would be claiming an untouched
    /// stream while multiplying it, which is the one outcome this whole design
    /// rules out.
    ///
    /// # The exact comparisons are the point
    ///
    /// `== 1.0` rather than a tolerance — see [`crate::volume`]'s note on
    /// `float_cmp`. An epsilon-wide band around unity would be a band in which
    /// baz scaled the samples while reporting [`VolumePath::Unity`]. Both
    /// inputs reach exactly `1.0` by construction rather than by luck: the
    /// volume taper is a cube of `1000/1000`, and
    /// [`ReplayGainDecision::amplitude`] returns `1.0` from an early return at
    /// zero centidecibels. Their product is therefore exactly `1.0` exactly
    /// when both are — `x * 1.0 == x` for every finite `x`, so neither input
    /// perturbs the other.
    #[allow(clippy::float_cmp)]
    fn settle_volume(&mut self, announce: bool) {
        let volume_gain = self.effective_gain();
        let replay_gain = self.rg_applied.amplitude();
        // What the *pump* must apply, which is not the volume when the device
        // took it: applying it in both places would apply it twice. The
        // atomic's contract is "the gain baz itself scales by".
        let device_took_it = self.sink.set_device_volume(volume_gain).is_some();
        let (volume_applied, volume_path) = if device_took_it {
            (1.0, VolumePath::DeviceAttenuator)
        } else if volume_gain == 1.0 {
            (volume_gain, VolumePath::Unity)
        } else {
            (volume_gain, VolumePath::SoftwareGain)
        };
        let applied = volume_applied * replay_gain;
        // ReplayGain is a software gain wherever the volume ended up: if it is
        // not unity, baz is multiplying, and the path says so.
        let path = if applied == 1.0 {
            volume_path
        } else {
            VolumePath::SoftwareGain
        };
        // Jump when nothing is audible (no discontinuity to hide in silence,
        // and the first sample is then exact), and when the device took the
        // volume — its attenuator changes at once, so a software side that
        // slewed would double-attenuate for the length of the ramp.
        if device_took_it || self.nothing_audible() {
            self.fader.jump(applied);
        }
        self.volume.set_gain(applied);
        let news = announce || path != self.volume_path;
        self.volume_path = path;
        self.volume.set_path(path);
        if news {
            let _ = self.events.send(Event::VolumeChanged {
                position: self.volume.volume().position(),
                muted: self.volume.muted(),
                path,
            });
        }
    }

    /// [`Command::Previous`]: go back — restart the current track, or step to
    /// the one before it.
    ///
    /// The same drain-and-restart machinery [`Command::Next`] and
    /// [`Command::Seek`] use, aimed at whichever queue position the
    /// [`PREVIOUS_RESTART_MS`] rule selects. Deliberately *one* mechanism
    /// rather than "seek to 0 when restarting, skip when stepping back":
    /// restarting a track and starting the one before it are the same
    /// operation with a different index, and the alternative would have made
    /// the two halves of one button take different code paths, differ in
    /// latency, and need separate tests for the same guarantee.
    ///
    /// Like `Next`, and unlike `Seek`, it **resumes**: the two halves of one
    /// transport control must not disagree about whether pressing them starts
    /// the music.
    fn previous(&mut self) {
        let Some(session) = self.session.take() else {
            return; // stopped: there is no current track to go back from
        };
        let current = session.current;
        let elapsed = session.elapsed_ms();
        drop(session); // abort: stop flag + join, exactly as Next does
        // The abandoned position's audio is still queued in the sink; drop it
        // for the reason `Sink::discard_buffered` exists.
        self.sink.discard_buffered();
        // Past the threshold, or already at the head of the queue with nothing
        // before it: this track again. Otherwise the one before it.
        let target = if elapsed >= PREVIOUS_RESTART_MS || current == 0 {
            current
        } else {
            current - 1
        };
        self.paused = false;
        self.start_session(target, 0, None);
    }

    /// [`Command::Seek`]: drain-and-restart the *current* track at
    /// `position_ms` (module docs). Same machinery as [`Command::Next`],
    /// aimed one queue position earlier and with a start offset.
    fn seek(&mut self, position_ms: u64) {
        let Some(session) = self.session.take() else {
            return; // stopped: there is no current track to seek within
        };
        let current = session.current;
        let track_ms = session.track_ms;
        let was_paused = self.paused;
        drop(session); // abort: stop flag + join, exactly as Next does
        // Aborting the session discards the audio the engine still held, but
        // the sink may hold a further bufferful of the position being left
        // behind. Dropping the session without dropping that is precisely the
        // "seek feels late" bug: see `Sink::discard_buffered`.
        self.sink.discard_buffered();
        if track_ms.is_some_and(|total| position_ms >= total) {
            // At or past the end is Next, per Command::Seek's contract.
            self.paused = false;
            self.start_session(current + 1, 0, None);
            return;
        }
        // The length carries over: it belongs to the track, not the session,
        // so the immediate Progress below can report a total straight away
        // instead of leaving the front end with a blank right-hand timestamp
        // until the new session's first bound arrives.
        self.start_session(current, position_ms, track_ms);
        if self.session.is_some() {
            // Seeking never changes whether audio is flowing.
            self.paused = was_paused;
            self.emit_progress();
        }
    }

    /// Answer a session's rate proposal: ask the sink to run at the rate the
    /// music is stored at, and publish whatever it grants.
    ///
    /// The engine thread owns the sink, so it is the only thread that may
    /// reopen a device — hence the handshake rather than the producer simply
    /// deciding. It runs once per session, before a single sample has been
    /// pushed, so a reopen here interrupts nothing.
    ///
    /// Nothing is negotiated for a sink that has no rate ([`OfflineSink`]):
    /// `None` grants the proposal outright, which is what makes an offline
    /// session play at its source's native rate.
    fn settle_rate(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        if session.rate_settled {
            return;
        }
        let proposed = session.shared.proposed_rate.load(Ordering::Acquire);
        if proposed == 0 {
            return; // the producer has not opened the first track yet
        }
        let shared = Arc::clone(&session.shared);
        let mut reopened = false;
        let granted = match self.sink.negotiate_rate(proposed) {
            Some(rate) => {
                if self.open_rate != 0 && rate != self.open_rate {
                    self.instruments
                        .reconfigurations
                        .fetch_add(1, Ordering::Relaxed);
                    reopened = true;
                }
                self.open_rate = rate;
                rate
            }
            None => proposed,
        };
        if reopened {
            // A reopened output is a new one; whatever volume arrangement the
            // old stream carried went with it.
            self.reestablish_volume();
        }
        // Release-store the granted rate: the producer's Acquire load of it is
        // what unparks it, so everything the negotiation did happens-before
        // the first sample is decoded against that rate.
        shared.stream_rate.store(granted, Ordering::Release);
        if let Some(session) = self.session.as_mut() {
            session.rate_settled = true;
        }
    }

    /// Hand the rest of the queue to a fresh session because the track at
    /// `next` is stored at a different sample rate (ADR-0009's reopen).
    ///
    /// The previous session has already played out — `complete()` means its
    /// producer finished *and* its ring drained — but the sink may still be
    /// holding a bufferful of the last track. That audio is owed to the
    /// listener, so this is the one place the engine **drains** instead of
    /// discarding: cutting it off would turn a rate change into a truncated
    /// ending. The new session then negotiates, which reopens the output at
    /// the new rate; the reconfiguration is what the listener hears as a short
    /// gap, and ADR-0009 measures and accepts it.
    fn continue_at_new_rate(&mut self, next: usize) {
        self.sink.drain_buffered();
        self.start_session(next, 0, None);
    }

    /// Send one [`Event::Progress`] now and re-arm the cadence, so an
    /// immediate report never doubles up with a scheduled one.
    fn emit_progress(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.arm_progress();
        if let Some(event) = session.progress() {
            let _ = self.events.send(event);
        }
    }

    /// Abort any active session and emit [`Event::Stopped`] if one existed.
    ///
    /// Stopping means stopping: the sink's buffered audio goes with the
    /// session, so silence follows the command rather than trailing it by a
    /// bufferful. (Pause takes a different path for exactly that reason —
    /// it keeps its buffer on purpose.)
    fn stop_session(&mut self) {
        if let Some(session) = self.session.take() {
            drop(session);
            self.sink.discard_buffered();
            let _ = self.events.send(Event::Stopped);
            // Nothing is being delivered, so nothing has a ReplayGain.
            self.settle_replay_gain(false);
        }
        self.paused = false;
    }

    /// Start a session at queue index `start`, beginning `seek_ms` into that
    /// track (0 for an ordinary start) and carrying `track_ms` as the known
    /// length of it (`None` until the producer reports one). Past the end of
    /// the queue (or on an empty queue) the run is already over:
    /// [`Event::QueueEnded`].
    fn start_session(&mut self, start: usize, seek_ms: u64, track_ms: Option<u64>) {
        self.paused = false;
        // Nothing has been delivered yet, so a slew would only mean the first
        // 20 ms of the new position playing at the *old* gain. Land on the
        // current setting instead — which is also what makes "set the volume,
        // then press play" exact from the first sample.
        self.fader.jump(self.volume.gain());
        if start >= self.queue.len() {
            self.position = 0;
            let _ = self.events.send(Event::QueueEnded);
            return;
        }
        self.session = Some(Session::start(
            self.queue.clone().into(),
            start,
            seek_ms,
            track_ms,
            Arc::clone(&self.instruments),
            self.cfg,
            self.exclusive,
        ));
    }
}

// ---------------------------------------------------------------------------
// A playback session: producer thread + rings + progress reporting
// ---------------------------------------------------------------------------

/// Flags shared between the engine thread and a session's producer side.
/// Atomics only — both sides stay lock-free.
struct SessionShared {
    /// Engine → producer: abandon the run (stop, skip, shutdown).
    stop: AtomicBool,
    /// Producer → engine: every track has been pushed or failed.
    producer_done: AtomicBool,
    /// Producer → engine: the rate the session's first playable track is
    /// stored at, published as soon as that track is open (0 until then).
    /// This is the *request* half of rate negotiation.
    proposed_rate: AtomicU32,
    /// Engine → producer: the rate this session's audio will be delivered at,
    /// published once the sink has answered the proposal above (0 until
    /// then). The producer parks on it, because nothing can be decoded to a
    /// rate that has not been settled.
    ///
    /// Every elapsed-time calculation divides by this and nothing else — see
    /// "Elapsed time" in the module docs for why the source file's own rate
    /// would be the wrong denominator.
    stream_rate: AtomicU32,
    /// Producer → engine: the queue index whose sample rate differs from
    /// [`Self::stream_rate`], i.e. where this session deliberately stopped
    /// short so the output can be reopened. [`NO_RATE_CHANGE`] when the
    /// session simply ran out of queue.
    rate_change_at: AtomicUsize,
}

impl Default for SessionShared {
    fn default() -> Self {
        Self {
            stop: AtomicBool::new(false),
            producer_done: AtomicBool::new(false),
            proposed_rate: AtomicU32::new(0),
            stream_rate: AtomicU32::new(0),
            // Zero is a real queue index, so "no rate change" needs its own
            // value rather than the numeric default.
            rate_change_at: AtomicUsize::new(NO_RATE_CHANGE),
        }
    }
}

/// A track's position and length within a session, as the producer reports
/// it to the engine thread over the bounds ring.
#[derive(Clone, Copy, Debug)]
struct TrackBound {
    /// Queue index of the track.
    index: usize,
    /// Session-relative interleaved sample offset where its audio begins.
    start_sample: usize,
    /// Its playing time, when known. The streamed anchor track reports the
    /// length its container declares (it has not finished decoding yet);
    /// decode-ahead tracks report the length they actually decoded to, which
    /// is the same number for a well-formed file and strictly more truthful
    /// for one whose header lies or is missing.
    duration_ms: Option<u64>,
    /// The rate the track is stored at, for the signal-path readout. Equal to
    /// the session's stream rate for every track the output can run at, which
    /// under the default is every track in the session.
    source_rate: u32,
    /// The depth the track's container declares, when it declares one.
    source_bits: Option<u32>,
    /// The ReplayGain the track's tags declare, read at open from metadata the
    /// probe had already parsed (ADR-0013). Travels with the boundary because
    /// the gain has to be in effect on the first sample *after* it.
    replay_gain: ReplayGainTags,
}

/// One run through the queue from a starting position. Owned by the engine
/// thread; the producer half runs on its own thread and communicates only
/// through SPSC rings and the shared atomics.
struct Session {
    audio: rtrb::Consumer<f32>,
    bounds: rtrb::Consumer<TrackBound>,
    fails: rtrb::Consumer<(usize, String)>,
    shared: Arc<SessionShared>,
    producer: Option<JoinHandle<()>>,
    queue: Arc<[PathBuf]>,
    /// Interleaved samples delivered to the sink so far this session.
    pulled: usize,
    /// Start sample of each queue index's audio, once known.
    boundaries: Vec<Option<usize>>,
    /// Declared length of each queue index's track, once known.
    durations: Vec<Option<u64>>,
    /// Stored rate and depth of each queue index's track, once known — the
    /// source half of [`Event::SignalPath`].
    formats: Vec<Option<(u32, Option<u32>)>>,
    /// Where each track's audio starts in the delivered stream and what
    /// ReplayGain it carries, **in delivery order** — the cut points the pump
    /// must not read across.
    ///
    /// Keyed by arrival rather than by queue index, unlike the four vectors
    /// above, because the question this answers is "which track's samples am I
    /// about to hand to the sink?" and the answer is a cursor
    /// ([`Self::active_slot`]) rather than a lookup. Bounds arrive in queue
    /// order over an SPSC ring and `start_sample` is a running total, so the
    /// starts here are non-decreasing by construction — which is what makes the
    /// cursor O(1) instead of a scan of the whole queue per pump block.
    replay_gains: Vec<(usize, ReplayGainTags)>,
    /// Cursor into [`Self::replay_gains`]: the track whose audio the next pump
    /// will deliver. `None` until the producer has published the first bound.
    active_slot: Option<usize>,
    /// The last [`Event::SignalPath`] this session emitted, so an unchanged
    /// chain is stated once rather than once per track.
    last_signal: Option<Event>,
    /// Which policy this session runs under, needed only to say *why* a
    /// conversion is happening when one is.
    boundary: BoundaryPolicy,
    /// Whether the output is held exclusively — the other half of the chain
    /// this session reports (ADR-0012). Copied from [`Control`] at start,
    /// because a sink does not change arrangement while it is open.
    exclusive: bool,
    /// Failure reason per queue index, once known (taken when reported).
    failures: Vec<Option<String>>,
    /// Reporting cursor: per-track events are emitted strictly in queue
    /// order, so a decode-ahead discovery never outruns the track before it.
    next_report: usize,
    /// Last queue index reported as started — what [`Command::Next`] skips
    /// from and what [`Command::Seek`] seeks within.
    current: usize,
    /// Where in the delivered stream the current track's audio begins; the
    /// origin every elapsed time is measured from.
    track_origin: usize,
    /// The current track's playing time, when known.
    track_ms: Option<u64>,
    /// Milliseconds into the *first* track of this session that its audio
    /// starts at — the [`Command::Seek`] target that created the session, 0
    /// otherwise. Applies to that track only; later tracks start at 0.
    seek_ms: u64,
    /// Queue index `seek_ms` applies to.
    seek_index: usize,
    /// `pulled` value at which the next cadence [`Event::Progress`] is due.
    next_progress: usize,
    /// Whether the engine thread has answered this session's rate proposal.
    /// Engine-thread state only — the producer learns the answer from
    /// [`SessionShared::stream_rate`].
    rate_settled: bool,
}

impl Session {
    fn start(
        queue: Arc<[PathBuf]>,
        start: usize,
        seek_ms: u64,
        track_ms: Option<u64>,
        instruments: Arc<Instruments>,
        cfg: EngineConfig,
        exclusive: bool,
    ) -> Self {
        let (ring_tx, ring_rx) = RingBuffer::new(cfg.ring_frames * CHANNELS);
        let remaining = (queue.len() - start).max(1);
        let (bounds_tx, bounds_rx) = RingBuffer::new(remaining);
        let (fails_tx, fails_rx) = RingBuffer::new(remaining);
        let shared = Arc::new(SessionShared::default());
        let task = ProducerTask {
            queue: Arc::clone(&queue),
            start,
            seek_ms,
            boundary: cfg.boundary,
            instruments,
            ring: ring_tx,
            bounds: bounds_tx,
            fails: fails_tx,
            shared: Arc::clone(&shared),
        };
        let producer = thread::spawn(move || task.run());
        let len = queue.len();
        Self {
            audio: ring_rx,
            bounds: bounds_rx,
            fails: fails_rx,
            shared,
            producer: Some(producer),
            queue,
            pulled: 0,
            boundaries: vec![None; len],
            durations: vec![None; len],
            formats: vec![None; len],
            // One entry per track this session can still reach; the producer
            // pushes at most that many bounds, so this never grows past its
            // reservation while audio is flowing.
            replay_gains: Vec::with_capacity(len.saturating_sub(start).max(1)),
            active_slot: None,
            last_signal: None,
            boundary: cfg.boundary,
            exclusive,
            failures: vec![None; len],
            next_report: start,
            current: start,
            // The session's first track always begins at delivered sample 0,
            // whichever queue index turns out to be playable.
            track_origin: 0,
            track_ms,
            seek_ms,
            seek_index: start,
            // Nothing to report until a track actually starts (or a seek or
            // resume asks for a reading), so the cadence starts disarmed.
            next_progress: usize::MAX,
            rate_settled: false,
        }
    }

    /// The queue index this session stopped short at because the sample rate
    /// changed there, if it did (see [`SessionShared::rate_change_at`]).
    fn rate_change_at(&self) -> Option<usize> {
        let at = self.shared.rate_change_at.load(Ordering::Acquire);
        (at != NO_RATE_CHANGE).then_some(at)
    }

    /// The signal-path statement for the track at `index`, or `None` when
    /// nothing about it is known yet.
    ///
    /// Deliberately says nothing about the volume: that travels on
    /// [`Event::VolumeChanged`], and the two events are separate because they
    /// change on incomparable cadences — this one per session, that one per
    /// pointer drag. `protocol`'s docs for both carry the full argument and
    /// the rule for combining them (`Direct` **and** `Unity` is what
    /// bit-exactness means since ADR-0011).
    fn signal_path(&self, index: usize) -> Option<Event> {
        let (source_rate_hz, source_bits) = (*self.formats.get(index)?)?;
        let output_rate_hz = self.shared.stream_rate.load(Ordering::Acquire);
        let why = match self.boundary {
            // Following the source is the policy, so a mismatch here means the
            // output could not be made to follow.
            BoundaryPolicy::BitPerfectReopen => ConversionReason::DeviceRateUnavailable,
            _ => ConversionReason::FixedOutputRate,
        };
        let conversion = (source_rate_hz != output_rate_hz).then_some(why);
        // Two independent facts, three states (see `SignalChain`): whether baz
        // converts, and whether baz owns the device. Owning the device does not
        // give it modes it does not have, which is why the exclusive variant
        // carries the reason rather than excluding it.
        let chain = match (self.exclusive, conversion) {
            (true, conversion) => SignalChain::Exclusive { conversion },
            (false, None) => SignalChain::Direct,
            (false, Some(reason)) => SignalChain::Converting { reason },
        };
        Some(Event::SignalPath {
            source_rate_hz,
            source_bits,
            output_rate_hz,
            chain,
        })
    }

    /// Position inside the current track, in milliseconds — the number
    /// [`Event::Progress`] reports and the one [`Command::Previous`] compares
    /// against [`PREVIOUS_RESTART_MS`].
    ///
    /// The module docs' "Elapsed time" contract, in integers: delivered frames
    /// since this track's origin, converted at the **stream** rate, offset by
    /// the seek target that started the session, and never past the track's
    /// declared end.
    fn elapsed_ms(&self) -> u64 {
        let frames = (self.pulled.saturating_sub(self.track_origin) / CHANNELS) as u64;
        let rate = self.shared.stream_rate.load(Ordering::Acquire);
        let delivered_ms = if frames == 0 || rate == 0 {
            0
        } else {
            frames_to_ms(frames, rate)
        };
        let offset = if self.current == self.seek_index {
            self.seek_ms
        } else {
            0
        };
        let elapsed = offset.saturating_add(delivered_ms);
        // Never report past the end: the last pump before a boundary can carry
        // a few frames of the next track's audio into this track's count, and
        // "3:01 of 3:00" is a bug on screen.
        self.track_ms.map_or(elapsed, |total| elapsed.min(total))
    }

    /// The current position reading, or `None` when audio has been delivered
    /// but the producer has not yet published the rate to interpret it at —
    /// a window of microseconds at the very start of a session, during which
    /// there is nothing truthful to say.
    ///
    /// The arithmetic is [`Self::elapsed_ms`]; what this adds is the one case
    /// where there is nothing truthful to *report*.
    fn progress(&self) -> Option<Event> {
        let frames = self.pulled.saturating_sub(self.track_origin) / CHANNELS;
        // Audio has been delivered but the producer has not published the rate
        // to interpret it at — a window of microseconds at the very start of a
        // session. (Nothing delivered yet is fine: the position is the seek
        // target, and no rate is needed to say so. That is the reading a
        // Seek's immediate Progress carries.)
        if frames > 0 && self.shared.stream_rate.load(Ordering::Acquire) == 0 {
            return None;
        }
        Some(Event::Progress {
            elapsed_ms: self.elapsed_ms(),
            track_ms: self.track_ms,
        })
    }

    /// Whether the cadence has come due, arming the next one. One integer
    /// comparison in the common case.
    fn progress_due(&mut self) -> bool {
        if self.pulled < self.next_progress {
            return false;
        }
        self.arm_progress();
        true
    }

    /// Schedule the next cadence report a quarter-second of delivered audio
    /// from now. Called by [`Self::progress_due`] and by every immediate
    /// report, so the two can never emit back-to-back.
    fn arm_progress(&mut self) {
        let rate = self.shared.stream_rate.load(Ordering::Acquire);
        let step = (rate / PROGRESS_HZ) as usize * CHANNELS;
        // Before the rate is known, retry on the next iteration rather than
        // arming a zero-length (and so always-due) interval.
        self.next_progress = self.pulled + step.max(1);
    }

    /// Pull up to `chunk_samples` from the ring into the sink, applying the
    /// volume on the way. This is the pump path: wait-free ring read,
    /// preallocated-sink write, atomic counter — no locks, no allocation (see
    /// module docs).
    ///
    /// # The two branches
    ///
    /// **Transparent** (the fader is at rest at exactly unity) is the branch
    /// that existed before volume control did, unchanged: the ring's slices go
    /// to the sink directly, with no copy and no arithmetic. That is what
    /// makes bit-exactness at unity structural — there is nothing here for a
    /// rounding argument to be about.
    ///
    /// **Scaling** rejoins the ring's two slices into `scratch` — sized to
    /// `chunk_samples` by [`Control::new`] and never grown — and scales the
    /// block there. Rejoining first is not incidental: the ring can wrap at
    /// any sample offset, and scaling the halves separately would step the
    /// slew half a frame out of phase across the join. The read length is
    /// `min`'d against the scratch, so every index below is in range by
    /// construction and nothing here can panic. One branch per block, one
    /// multiply per sample; [`Fader::apply`] states its own realtime contract.
    ///
    /// # A block never crosses a track boundary
    ///
    /// The read is also capped at the next known boundary
    /// ([`Self::next_boundary`]) — one comparison and one `min` per block. That
    /// is what makes a per-track ReplayGain change land on the *right sample*
    /// rather than up to a block late: the engine can only change the gain
    /// between pump calls, so a block spanning two tracks would play the front
    /// of the new one at the old one's gain. Capping costs one short block per
    /// boundary and nothing else; the samples delivered, and their order, are
    /// unchanged, which is why the bit-exactness fixtures are unaffected.
    fn pump(
        &mut self,
        sink: &mut dyn Sink,
        chunk_samples: usize,
        delivered: &AtomicUsize,
        fader: &mut Fader,
        scratch: &mut [f32],
    ) -> bool {
        let available = self.audio.slots();
        if available == 0 {
            return false;
        }
        let transparent = fader.is_transparent();
        // While scaling, never read more than the scratch can hold; the two
        // are equal by construction, and the `min` is what makes that a fact
        // rather than a comment.
        let mut n = if transparent {
            available.min(chunk_samples)
        } else {
            available.min(chunk_samples).min(scratch.len())
        };
        if let Some(edge) = self.next_boundary()
            && edge > self.pulled
        {
            n = n.min(edge - self.pulled);
        }
        let Ok(chunk) = self.audio.read_chunk(n) else {
            return false;
        };
        let (a, b) = chunk.as_slices();
        if transparent {
            sink.write(a);
            if !b.is_empty() {
                sink.write(b);
            }
        } else {
            // One load of the session's rate, used only to size the slew step.
            let rate = self.shared.stream_rate.load(Ordering::Acquire);
            let split = a.len();
            scratch[..split].copy_from_slice(a);
            scratch[split..n].copy_from_slice(b);
            let block = &mut scratch[..n];
            fader.apply(block, rate);
            sink.write(block);
        }
        chunk.commit_all();
        self.pulled += n;
        delivered.fetch_add(n, Ordering::Release);
        true
    }

    /// Take delivery of everything the producer has published: track bounds
    /// and failures, drained from their rings into this session's own vectors.
    ///
    /// Split out of [`Self::report`] so the engine can run it **before** the
    /// pump. Reporting must happen after the pump (a track is "started" once
    /// its audio has been delivered), but the boundary the pump must not read
    /// across has to be known before it. Draining is destructive, so calling
    /// this and then `report` in the same iteration processes each item once.
    ///
    /// Allocation: `replay_gains` was reserved for the whole reachable queue at
    /// session start, so the push here does not grow it. This runs on the
    /// engine thread between pumps, not inside one.
    fn absorb(&mut self) {
        while let Ok(bound) = self.bounds.pop() {
            if let Some(slot) = self.boundaries.get_mut(bound.index) {
                *slot = Some(bound.start_sample);
            }
            if let Some(slot) = self.durations.get_mut(bound.index) {
                *slot = bound.duration_ms;
            }
            if let Some(slot) = self.formats.get_mut(bound.index) {
                *slot = Some((bound.source_rate, bound.source_bits));
            }
            self.replay_gains
                .push((bound.start_sample, bound.replay_gain));
        }
        while let Ok((i, reason)) = self.fails.pop() {
            if let Some(slot) = self.failures.get_mut(i) {
                *slot = Some(reason);
            }
        }
    }

    /// Where the *next* track's audio begins in the delivered stream, when a
    /// bound for it has arrived — the cut point [`Self::pump`] will not read
    /// across.
    fn next_boundary(&self) -> Option<usize> {
        let next = self.active_slot.map_or(0, |slot| slot + 1);
        self.replay_gains.get(next).map(|(start, _)| *start)
    }

    /// Move the cursor onto whichever track's audio the next pump will
    /// deliver, reporting whether it moved.
    ///
    /// A loop rather than a single step because a zero-length track (a file
    /// that decoded to nothing) contributes a bound with no samples between it
    /// and the next, and skipping past it must not take two iterations of the
    /// engine loop.
    fn advance_active(&mut self) -> bool {
        let mut moved = false;
        while let Some(start) = self.next_boundary() {
            if self.pulled < start {
                break;
            }
            self.active_slot = Some(self.active_slot.map_or(0, |slot| slot + 1));
            moved = true;
        }
        moved
    }

    /// The ReplayGain tags of the track whose audio is being delivered — all
    /// `None` before the first bound arrives, which resolves to "the file said
    /// nothing" and so to the no-ReplayGain pre-amp.
    fn active_replay_gain(&self) -> ReplayGainTags {
        self.active_slot
            .and_then(|slot| self.replay_gains.get(slot))
            .map_or_else(ReplayGainTags::default, |(_, tags)| *tags)
    }

    /// Emit per-track events in strict queue order. A track is reported
    /// started once its first samples were delivered (`pulled` passed its
    /// boundary); failures are reported as soon as order allows. With
    /// `flush` (session complete) every remaining known track is reported.
    fn report(&mut self, events: &Sender<Event>, flush: bool) {
        self.absorb();
        while self.next_report < self.queue.len() {
            let i = self.next_report;
            if let Some(start_sample) = self.boundaries[i] {
                if self.pulled > start_sample || flush {
                    let _ = events.send(Event::TrackStarted {
                        path: self.queue[i].clone(),
                        position: i,
                    });
                    self.current = i;
                    self.track_origin = start_sample;
                    self.track_ms = self.durations[i];
                    // The chain follows the track it describes, and is stated
                    // only when it is news: identical for every track of an
                    // album, so an album says it once.
                    let signal = self.signal_path(i);
                    if signal.is_some() && signal != self.last_signal {
                        if let Some(event) = signal.clone() {
                            let _ = events.send(event);
                        }
                        self.last_signal = signal;
                    }
                    // A new track means a new position: make the cadence due
                    // now so `Progress` follows `TrackStarted` immediately
                    // (protocol docs) instead of up to 250 ms later.
                    self.next_progress = self.pulled;
                    if let Some(reason) = self.failures[i].take() {
                        // Opened, then failed mid-decode: started AND failed.
                        let _ = events.send(Event::TrackFailed {
                            path: self.queue[i].clone(),
                            reason,
                        });
                    }
                    self.next_report += 1;
                    continue;
                }
                break; // its audio hasn't reached the sink yet
            }
            if let Some(reason) = self.failures[i].take() {
                let _ = events.send(Event::TrackFailed {
                    path: self.queue[i].clone(),
                    reason,
                });
                self.next_report += 1;
                continue;
            }
            break; // the producer hasn't reached this track yet
        }
    }

    /// Whether the session has played out: the producer pushed everything
    /// and the ring is drained. (Checked only while unpaused, so a paused
    /// session never completes underneath the user.)
    fn complete(&self) -> bool {
        // Order matters: read `producer_done` before the ring so a push
        // between the two loads can only delay completion, never lose audio.
        let done = self.shared.producer_done.load(Ordering::Acquire);
        done && self.audio.slots() == 0
    }
}

impl Drop for Session {
    /// Abort: release the producer (it observes `stop` in every
    /// backpressure and decode loop) and join it, prefetch included. Ring
    /// audio not yet delivered is discarded. Natural completion takes the
    /// same path — the flag is simply set after the producer already
    /// finished.
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(handle) = self.producer.take() {
            let _ = handle.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Producer side (per session)
// ---------------------------------------------------------------------------

struct ProducerTask {
    queue: Arc<[PathBuf]>,
    start: usize,
    /// Milliseconds into the session's first playable track to begin at
    /// ([`Command::Seek`]'s target); 0 for an ordinary start.
    seek_ms: u64,
    boundary: BoundaryPolicy,
    instruments: Arc<Instruments>,
    ring: rtrb::Producer<f32>,
    bounds: rtrb::Producer<TrackBound>,
    fails: rtrb::Producer<(usize, String)>,
    shared: Arc<SessionShared>,
}

/// What decode-ahead found at a queue position.
enum Prefetched {
    /// The track, decoded at its native rate and ready to splice.
    Audio(DecodedAudio),
    /// The track is stored at a *different* sample rate from the session's.
    /// Under the bit-perfect default the session ends at this position so the
    /// output can be reopened; nothing was decoded, so discovering it cost a
    /// header probe rather than a track. The rate itself is not carried — the
    /// session that takes over opens the file and negotiates from it.
    RateChange,
}

type Prefetch = (usize, JoinHandle<Result<Prefetched, PlaybackError>>);

impl ProducerTask {
    fn run(mut self) {
        self.produce();
        self.shared.producer_done.store(true, Ordering::Release);
    }

    /// Stream the queue from `start` into the ring: the first playable
    /// track streams block-by-block (fast start), later tracks are decoded
    /// one ahead on a prefetch thread and spliced through the ring —
    /// gapless exactly as in [`run_playlist`](crate::playback::run_playlist). A track that fails to open
    /// or decode is recorded and skipped; the queue survives it.
    ///
    /// Under the ADR-0009 default the run also **ends** at the first track
    /// stored at a different sample rate, handing that index back to the
    /// engine so a new session can reopen the output at it.
    fn produce(&mut self) {
        let stop = Arc::clone(&self.shared);
        let stop = &stop.stop;

        let Some((idx, src)) = self.find_anchor(stop) else {
            return; // nothing playable (or stopping)
        };
        let source_rate = src.sample_rate();
        // Rate negotiation, producer half: propose the rate this music is
        // stored at and wait for the engine thread — the only thread that may
        // touch the sink — to say what the output will actually run at. The
        // answer arrives on its next loop iteration.
        self.shared
            .proposed_rate
            .store(source_rate, Ordering::Release);
        let Some(stream_rate) = self.await_stream_rate(stop) else {
            return; // stopping before the rate was ever settled
        };
        // Follow-the-source only cares about the *next* track's rate when it
        // is going to refuse to convert it; a fixed-rate session converts
        // everything and so needs no comparison.
        let follow = (self.boundary == BoundaryPolicy::BitPerfectReopen).then_some(stream_rate);
        let mut pending: Option<Prefetch> = self.spawn_prefetch(idx + 1, follow);
        let mut pushed = self.push_anchor(idx, src, stream_rate, stop);

        // Subsequent tracks, one decode ahead.
        let mut i = idx + 1;
        while i < self.queue.len() && !stop.load(Ordering::Acquire) {
            let found = match pending.take() {
                Some((_, handle)) => handle
                    .join()
                    .unwrap_or(Err(PlaybackError::WorkerPanicked("prefetch"))),
                None => prefetch(&self.queue[i], stop, follow),
            };
            let decoded = match found {
                Ok(Prefetched::RateChange) => {
                    // The session stops one track short of its queue on
                    // purpose. Publishing the index is the whole handover: the
                    // engine drains the output, reopens it at this track's
                    // rate, and starts a fresh session here.
                    self.shared.rate_change_at.store(i, Ordering::Release);
                    break;
                }
                Ok(Prefetched::Audio(decoded)) => Ok(decoded),
                Err(e) => Err(e),
            };
            // Only now start decode-ahead of the following track: past a rate
            // change there is nothing this session would do with it.
            pending = self.spawn_prefetch(i + 1, follow);
            if stop.load(Ordering::Acquire) {
                break;
            }
            match decoded.and_then(|d| {
                // The decoded length at the *native* rate is the track's
                // playing time; take it before the samples are converted to
                // the stream rate, which changes their count but not the
                // seconds they represent.
                let duration_ms = Some(frames_to_ms(d.frames() as u64, d.sample_rate));
                let format = (d.sample_rate, d.bits_per_sample, d.replay_gain);
                at_rate(d, stream_rate, &self.instruments)
                    .map(|samples| (samples, duration_ms, format))
            }) {
                Ok((samples, duration_ms, (source_rate, source_bits, replay_gain))) => {
                    let _ = self.bounds.push(TrackBound {
                        index: i,
                        start_sample: pushed,
                        duration_ms,
                        replay_gain,
                        source_rate,
                        source_bits,
                    });
                    if !push_with_backpressure(&mut self.ring, &samples, stop) {
                        break;
                    }
                    pushed += samples.len();
                }
                Err(e) => {
                    let _ = self.fails.push((i, e.to_string()));
                }
            }
            i += 1;
        }

        if let Some((_, handle)) = pending {
            // The prefetch loop observes `stop`, so this join is bounded.
            let _ = handle.join();
        }
    }

    /// Find the session's first playable track and open it, positioned at
    /// [`Self::seek_ms`]. Tracks that cannot be opened are reported as
    /// failures and skipped; a track the seek target lies past is skipped
    /// *silently*, and the search continues from the beginning of the next
    /// one — that is [`Command::Seek`]'s "past the end means Next" contract,
    /// reached here only when the engine could not apply it itself for want
    /// of a declared track length.
    ///
    /// Returns the queue index and the positioned source, or `None` if the
    /// queue ran out (or the session is stopping).
    fn find_anchor(&mut self, stop: &AtomicBool) -> Option<(usize, AudioSource)> {
        let mut idx = self.start;
        let mut seek_ms = self.seek_ms;
        while idx < self.queue.len() && !stop.load(Ordering::Acquire) {
            let opened = AudioSource::open(&self.queue[idx]).and_then(|mut src| {
                if seek_ms > 0 {
                    src.seek(seek_ms)?;
                }
                Ok(src)
            });
            match opened {
                Ok(src) => return Some((idx, src)),
                Err(PlaybackError::SeekPastEnd { .. }) => seek_ms = 0,
                Err(e) => {
                    let _ = self.fails.push((idx, e.to_string()));
                }
            }
            idx += 1;
        }
        None
    }

    /// Push the anchor — the session's first playable track — into the ring,
    /// returning how many interleaved samples reached it.
    ///
    /// At the stream rate, which under the default is every track the output
    /// device can run at, it **streams block-by-block** so the first sample is
    /// audible in under a millisecond. When the rates differ (the device
    /// offered no mode at the source rate, or a fixed output rate was chosen)
    /// it is decoded whole and resampled whole first, because the whole-buffer
    /// resampler needs the whole buffer — the wait ADR-0009 measures and
    /// accepts for a case that no longer happens on hardware that can play the
    /// file.
    ///
    /// A seek has already positioned the source either way; on the resampling
    /// path the remaining tail must still be longer than the resampler's
    /// alignment padding (a few milliseconds), or it is reported as a track
    /// failure like any other decode problem.
    fn push_anchor(
        &mut self,
        idx: usize,
        mut src: AudioSource,
        stream_rate: u32,
        stop: &AtomicBool,
    ) -> usize {
        let bound = TrackBound {
            index: idx,
            // The anchor's audio always begins at the session's sample 0.
            start_sample: 0,
            // The track's own length, unaffected by any resampling below.
            duration_ms: src.duration_ms(),
            source_rate: src.sample_rate(),
            source_bits: src.bits_per_sample(),
            replay_gain: src.replay_gain(),
        };
        let mut pushed = 0usize;
        if src.sample_rate() == stream_rate {
            let _ = self.bounds.push(bound);
            loop {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                match src.next_block() {
                    Ok(Some(block)) => {
                        if !push_with_backpressure(&mut self.ring, block, stop) {
                            break;
                        }
                        pushed += block.len();
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let _ = self.fails.push((idx, e.to_string()));
                        break;
                    }
                }
            }
            return pushed;
        }
        match decode_open(src, stop).and_then(|d| at_rate(d, stream_rate, &self.instruments)) {
            Ok(samples) => {
                if !stop.load(Ordering::Acquire) {
                    let _ = self.bounds.push(bound);
                    if push_with_backpressure(&mut self.ring, &samples, stop) {
                        pushed += samples.len();
                    }
                }
            }
            Err(e) => {
                let _ = self.fails.push((idx, e.to_string()));
            }
        }
        pushed
    }

    /// Park until the engine thread has answered this session's rate proposal
    /// (see [`Control::settle_rate`]). `None` means the session is being
    /// abandoned and the producer should simply leave.
    ///
    /// The wait is genuinely short — the engine answers on its next loop
    /// iteration, which for a starting session is microseconds away — but it
    /// is a wait, so it checks `stop` as diligently as every other loop here.
    fn await_stream_rate(&self, stop: &AtomicBool) -> Option<u32> {
        loop {
            let granted = self.shared.stream_rate.load(Ordering::Acquire);
            if granted != 0 {
                return Some(granted);
            }
            if stop.load(Ordering::Acquire) {
                return None;
            }
            thread::sleep(NEGOTIATE_POLL);
        }
    }

    fn spawn_prefetch(&self, index: usize, follow: Option<u32>) -> Option<Prefetch> {
        let path = self.queue.get(index)?.clone();
        let shared = Arc::clone(&self.shared);
        let handle = thread::spawn(move || prefetch(&path, &shared.stop, follow));
        Some((index, handle))
    }
}

/// Decode-ahead of one queue position.
///
/// `follow` carries the session's stream rate when the policy is to follow the
/// source: the file is opened, its declared rate compared, and a track at a
/// different rate reported as [`Prefetched::RateChange`] **without decoding
/// it** — the rate is in the header, so refusing costs a probe rather than a
/// whole track. `None` (fixed output rate) always decodes.
fn prefetch(
    path: &Path,
    stop: &AtomicBool,
    follow: Option<u32>,
) -> Result<Prefetched, PlaybackError> {
    let src = AudioSource::open(path)?;
    if let Some(stream_rate) = follow
        && src.sample_rate() != stream_rate
    {
        return Ok(Prefetched::RateChange);
    }
    decode_open(src, stop).map(Prefetched::Audio)
}

/// Decode a whole open source, checking `stop` between blocks so an aborting
/// session never waits for a full-track decode. On stop the partial result
/// is returned; callers observe the flag and discard it.
fn decode_open(mut src: AudioSource, stop: &AtomicBool) -> Result<DecodedAudio, PlaybackError> {
    let mut samples = Vec::new();
    while let Some(block) = src.next_block()? {
        samples.extend_from_slice(block);
        if stop.load(Ordering::Acquire) {
            break;
        }
    }
    Ok(DecodedAudio {
        samples,
        sample_rate: src.sample_rate(),
        bits_per_sample: src.bits_per_sample(),
        replay_gain: src.replay_gain(),
    })
}

/// Bring decoded audio to the session's stream rate.
///
/// The equal-rate branch is the one the bit-perfect default takes for every
/// track: it moves the samples and touches nothing. The other branch is
/// reached only when the output could not be made to run at the source rate,
/// or a fixed output rate was chosen — and it is counted, so "nothing was
/// resampled" is a fact the tests can read rather than infer.
fn at_rate(
    decoded: DecodedAudio,
    stream_rate: u32,
    instruments: &Instruments,
) -> Result<Vec<f32>, PlaybackError> {
    if decoded.sample_rate == stream_rate {
        return Ok(decoded.samples);
    }
    let t0 = Instant::now();
    let out = resample_interleaved(&decoded.samples, decoded.sample_rate, stream_rate)?;
    instruments.record_resample(t0.elapsed());
    Ok(out)
}

#[cfg(test)]
mod tests {
    //! What the engine asks of a sink that has a rate and a buffer — i.e. of
    //! real hardware.
    //!
    //! # Why these tests live here and not in `tests/engine.rs`
    //!
    //! The integration suite drives the engine through [`spawn_offline`],
    //! whose sink is an [`OfflineSink`] — and an offline sink has neither a
    //! downstream buffer to discard from nor a rate to negotiate. It is the
    //! record of delivered audio, not a queue standing in front of a clock, so
    //! two whole classes of behaviour are structurally unobservable through
    //! that path: *pre-seek audio still queued in the device ring keeps
    //! playing after the seek*, and *the output must be reopened at the rate
    //! of the music*. Saying so plainly and testing each half where it is real
    //! beats inventing an offline assertion that would pass either way:
    //!
    //! - **Does the engine ask for the discard, and for the rate, at exactly
    //!   the right moments?** That is a property of [`Control`], and it is
    //!   what these tests assert, by running the real control loop against a
    //!   sink that records the operations it receives. The test doubles do not
    //!   stand in for the behaviour under test — the behaviour under test is
    //!   the engine's *call*, observed directly. In particular
    //!   [`DeviceDouble`] answers rate requests exactly as `DeviceSink` does
    //!   (grant the rate if it has it, nearest one it has otherwise), which is
    //!   how the "device refuses the source rate" fallback becomes testable at
    //!   all: no real machine can be made to lack a 48 kHz mode on demand.
    //! - **Does the discard actually empty the device ring, and does a reopen
    //!   really produce a stream at the new rate?** Those are properties of
    //!   `DeviceSink`, asserted against a real audio device in
    //!   `tests/playback.rs` (`discard_buffered_empties_the_device_ring`,
    //!   `device_sink_reopens_at_the_requested_rate`, feature
    //!   `device-output`).

    use std::sync::Mutex;
    use std::time::Instant;

    use super::{
        Arc, BoundaryPolicy, Command, Control, Conversions, Duration, EngineConfig, Event,
        Instruments, Observable, Path, PathBuf, Sink, mpsc, thread,
    };
    use crate::protocol::{ConversionReason, SignalChain};

    const RATE: u32 = 44_100;
    /// The rate the owner's album is stored at, and the case ADR-0009 exists
    /// for.
    const HI_RATE: u32 = 48_000;
    /// Long enough that every command below lands mid-track.
    const TRACK_SECS: usize = 5;
    const TIMEOUT: Duration = Duration::from_secs(20);

    /// What a sink was asked to do, in order.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Op {
        Write,
        Discard,
        /// Wait for buffered audio to play out (a rate change).
        Drain,
        /// Reopen at a new rate.
        Negotiate,
    }

    /// A sink that records the operations the engine performs on it.
    struct RecordingSink {
        ops: Arc<Mutex<Vec<Op>>>,
    }

    impl Sink for RecordingSink {
        fn write(&mut self, samples: &[f32]) {
            if samples.is_empty() {
                return;
            }
            if let Ok(mut ops) = self.ops.lock() {
                // Collapse runs of writes: the pump writes constantly, and
                // only their ordering against `Discard` is under test.
                if ops.last() != Some(&Op::Write) {
                    ops.push(Op::Write);
                }
            }
        }

        fn discard_buffered(&mut self) {
            if let Ok(mut ops) = self.ops.lock() {
                ops.push(Op::Discard);
            }
        }
    }

    /// A 440 Hz stereo WAV of `frames` frames at `rate`.
    fn fixture_at(dir: &Path, name: &str, rate: u32, frames: usize) -> PathBuf {
        let path = dir.join(name);
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&path, spec).expect("create fixture wav");
        for n in 0..frames {
            #[allow(clippy::cast_precision_loss)] // frame indices are far below 2^52
            let t = n as f64 / f64::from(rate);
            #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
            let s = (0.5 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()) as f32;
            writer.write_sample(s).expect("write sample");
            writer.write_sample(s).expect("write sample");
        }
        writer.finalize().expect("finalize fixture wav");
        path
    }

    /// A five-second 440 Hz stereo WAV at [`RATE`].
    fn fixture(dir: &Path, name: &str) -> PathBuf {
        fixture_at(dir, name, RATE, TRACK_SECS * RATE as usize)
    }

    /// Paced so a 5 s track takes a few hundred ms to drain: every command
    /// below then lands mid-track, with audio genuinely in flight.
    fn config() -> EngineConfig {
        EngineConfig {
            ring_frames: 8192,
            consumer_chunk_frames: 2048,
            consumer_pace: Duration::from_millis(4),
            ..EngineConfig::default()
        }
    }

    /// A running [`Control`] on its own thread, plus what is needed to drive
    /// and observe it.
    struct Harness {
        commands: Option<mpsc::Sender<Command>>,
        events: mpsc::Receiver<Event>,
        ops: Arc<Mutex<Vec<Op>>>,
        thread: Option<thread::JoinHandle<()>>,
        _dir: tempfile::TempDir,
        track: PathBuf,
    }

    impl Harness {
        fn start() -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let track = fixture(dir.path(), "tone_5s.wav");
            let (cmd_tx, cmd_rx) = mpsc::channel();
            let (event_tx, event_rx) = mpsc::channel();
            let ops: Arc<Mutex<Vec<Op>>> = Arc::default();
            let sink = RecordingSink {
                ops: Arc::clone(&ops),
            };
            let thread = thread::spawn(move || {
                let control =
                    Control::new(cmd_rx, event_tx, config(), 0, Observable::default(), sink);
                drop(control.run());
            });
            Self {
                commands: Some(cmd_tx),
                events: event_rx,
                ops,
                thread: Some(thread),
                _dir: dir,
                track,
            }
        }

        fn send(&self, command: Command) {
            self.commands
                .as_ref()
                .expect("engine running")
                .send(command)
                .expect("engine accepts commands");
        }

        /// Start playback and block until audio is genuinely flowing into the
        /// sink, so a later command has something buffered to invalidate.
        fn play_until_audio_flows(&self) {
            self.send(Command::SetQueue {
                paths: vec![self.track.clone()],
            });
            self.send(Command::Play);
            loop {
                match self.events.recv_timeout(TIMEOUT) {
                    Ok(Event::TrackStarted { .. }) => break,
                    Ok(_) => {}
                    Err(e) => panic!("no TrackStarted: {e}"),
                }
            }
            let deadline = Instant::now() + TIMEOUT;
            while Instant::now() < deadline {
                if self.ops_since(0).contains(&Op::Write) {
                    return;
                }
                thread::sleep(Duration::from_millis(1));
            }
            panic!("the engine never wrote audio to the sink");
        }

        fn ops_since(&self, mark: usize) -> Vec<Op> {
            self.ops.lock().expect("ops lock")[mark..].to_vec()
        }

        /// Where the ops log stands now, so a command's effects are read
        /// separately from the previous one's.
        fn mark(&self) -> usize {
            self.ops.lock().expect("ops lock").len()
        }

        fn shutdown(mut self) {
            self.commands = None;
            if let Some(handle) = self.thread.take() {
                handle.join().expect("engine thread");
            }
        }
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            self.commands = None;
            if let Some(handle) = self.thread.take() {
                let _ = handle.join();
            }
        }
    }

    /// Poll the ops log until `done` accepts it (or time runs out) and return
    /// what it ended up as, so the assertion can be made on the caller's
    /// terms rather than on the timeout's.
    fn wait_until(harness: &Harness, mark: usize, done: impl Fn(&[Op]) -> bool) -> Vec<Op> {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let ops = harness.ops_since(mark);
            if done(&ops) || Instant::now() >= deadline {
                return ops;
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn discarded(ops: &[Op]) -> bool {
        ops.contains(&Op::Discard)
    }

    /// The bug: a seek abandoned the session but left the audio already
    /// handed to the sink queued in front of the new position. The engine
    /// must tell the sink to drop it, between the last pre-seek write and the
    /// first post-seek one.
    #[test]
    fn seek_discards_the_sinks_buffered_audio() {
        let harness = Harness::start();
        harness.play_until_audio_flows();
        let mark = harness.mark();
        harness.send(Command::Seek { position_ms: 3_000 });
        let ops = wait_until(&harness, mark, |ops| {
            ops.iter()
                .position(|op| *op == Op::Discard)
                .is_some_and(|i| ops[i + 1..].contains(&Op::Write))
        });
        let discard = ops
            .iter()
            .position(|op| *op == Op::Discard)
            .expect("Seek must discard the sink's buffered audio");
        assert!(
            ops[discard + 1..].contains(&Op::Write),
            "post-seek audio must reach the sink only after the discard: {ops:?}"
        );
        harness.shutdown();
    }

    /// Skipping a track abandons its audio the same way a seek does.
    #[test]
    fn next_discards_the_sinks_buffered_audio() {
        let harness = Harness::start();
        harness.play_until_audio_flows();
        let mark = harness.mark();
        harness.send(Command::Next);
        assert!(
            discarded(&wait_until(&harness, mark, discarded)),
            "Next must discard the sink's buffered audio"
        );
        harness.shutdown();
    }

    /// Stop means stop: silence follows the command instead of trailing it by
    /// a bufferful.
    #[test]
    fn stop_discards_the_sinks_buffered_audio() {
        let harness = Harness::start();
        harness.play_until_audio_flows();
        let mark = harness.mark();
        harness.send(Command::Stop);
        assert!(
            discarded(&wait_until(&harness, mark, discarded)),
            "Stop must discard the sink's buffered audio"
        );
        harness.shutdown();
    }

    /// Replacing the queue while playing stops playback (module docs), and so
    /// abandons the buffered audio with it.
    #[test]
    fn queue_replacement_discards_the_sinks_buffered_audio() {
        let harness = Harness::start();
        harness.play_until_audio_flows();
        let mark = harness.mark();
        harness.send(Command::SetQueue { paths: Vec::new() });
        assert!(
            discarded(&wait_until(&harness, mark, discarded)),
            "SetQueue while playing must discard the sink's buffered audio"
        );
        harness.shutdown();
    }

    /// Pause is the deliberate exception: it keeps its buffered audio, which
    /// is what makes resume gapless-instant and the delivered stream
    /// bit-identical to an unpaused run. Discarding here would break a
    /// documented guarantee, not improve one.
    #[test]
    fn pause_keeps_the_sinks_buffered_audio() {
        let harness = Harness::start();
        harness.play_until_audio_flows();
        let mark = harness.mark();
        harness.send(Command::Pause);
        loop {
            match harness.events.recv_timeout(TIMEOUT) {
                Ok(Event::Paused) => break,
                Ok(_) => {}
                Err(e) => panic!("no Paused event: {e}"),
            }
        }
        // Give the engine every chance to misbehave before concluding it did
        // not: many pump iterations' worth of idle time.
        thread::sleep(Duration::from_millis(50));
        assert!(
            !discarded(&harness.ops_since(mark)),
            "Pause must not discard buffered audio — resume would no longer be \
             sample-continuous"
        );
        harness.shutdown();
    }

    // -----------------------------------------------------------------------
    // Rate negotiation (ADR-0009)
    // -----------------------------------------------------------------------

    /// Rates a [`DeviceDouble`] was reopened at, in order.
    type Opened = Arc<Mutex<Vec<u32>>>;

    /// A sink standing in for real hardware: it has a rate, a fixed set of
    /// rates it can be opened at, and it answers a rate request the way
    /// `DeviceSink` does — grant the asked-for rate when it has it, otherwise
    /// the nearest one it does have.
    ///
    /// It exists because the interesting branch cannot be reached any other
    /// way: no real machine can be persuaded to lack a 48 kHz mode for the
    /// duration of one test, and the fallback it triggers is precisely the
    /// behaviour ADR-0009 has to get right.
    struct DeviceDouble {
        rate: u32,
        supported: Vec<u32>,
        opened: Opened,
    }

    impl DeviceDouble {
        fn new(rate: u32, supported: &[u32], opened: &Opened) -> Self {
            Self {
                rate,
                supported: supported.to_vec(),
                opened: Arc::clone(opened),
            }
        }
    }

    impl Sink for DeviceDouble {
        fn write(&mut self, _samples: &[f32]) {}

        fn negotiate_rate(&mut self, desired: u32) -> Option<u32> {
            let granted = self
                .supported
                .iter()
                .copied()
                .min_by_key(|r| r.abs_diff(desired))
                .unwrap_or(self.rate);
            if granted != self.rate {
                self.rate = granted;
                if let Ok(mut log) = self.opened.lock() {
                    log.push(granted);
                }
            }
            Some(granted)
        }
    }

    /// Play `queue` to its end through a real [`Control`] over `sink`, and
    /// return every event it emitted plus its conversion counters.
    fn run_queue<S: Sink + Send + 'static>(
        queue: Vec<PathBuf>,
        cfg: EngineConfig,
        sink: S,
        open_rate: u32,
    ) -> (Vec<Event>, Conversions) {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let instruments: Arc<Instruments> = Arc::default();
        let probes = Arc::clone(&instruments);
        let engine = thread::spawn(move || {
            let control = Control::new(
                cmd_rx,
                event_tx,
                cfg,
                open_rate,
                Observable {
                    instruments: probes,
                    ..Observable::default()
                },
                sink,
            );
            drop(control.run());
        });
        cmd_tx
            .send(Command::SetQueue { paths: queue })
            .expect("engine accepts commands");
        cmd_tx.send(Command::Play).expect("engine accepts commands");

        let mut events = Vec::new();
        let deadline = Instant::now() + TIMEOUT;
        while Instant::now() < deadline {
            match event_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Event::QueueEnded) => {
                    events.push(Event::QueueEnded);
                    break;
                }
                Ok(event) => events.push(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(cmd_tx);
        engine.join().expect("engine thread");
        (events, instruments.snapshot())
    }

    /// Every `SignalPath` in `events`, as the tuple a front end would render.
    fn chains(events: &[Event]) -> Vec<(u32, u32, SignalChain)> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::SignalPath {
                    source_rate_hz,
                    output_rate_hz,
                    chain,
                    ..
                } => Some((*source_rate_hz, *output_rate_hz, *chain)),
                _ => None,
            })
            .collect()
    }

    /// Half a second: real audio, but a queue of three still plays out inside
    /// a test.
    const SHORT_FRAMES: usize = 22_050;

    /// **A 48 kHz queue negotiates a 48 kHz output.** The device starts at
    /// 44.1 kHz — what the app opens before it knows what will be played — and
    /// the session moves it, because this device has a 48 kHz mode.
    ///
    /// Asserted three ways so it cannot pass by accident: the sink was
    /// reopened at 48 kHz, the readout says the chain is direct, and no
    /// resampler was constructed.
    #[test]
    fn a_48k_queue_negotiates_a_48k_output() {
        let dir = tempfile::tempdir().expect("temp dir");
        let track = fixture_at(dir.path(), "hi.wav", HI_RATE, SHORT_FRAMES);
        let opened: Opened = Arc::default();
        let sink = DeviceDouble::new(RATE, &[RATE, HI_RATE], &opened);
        let (events, conversions) = run_queue(vec![track], EngineConfig::default(), sink, RATE);

        assert_eq!(
            *opened.lock().expect("opened lock"),
            vec![HI_RATE],
            "the output must be reopened at the rate the music is stored at"
        );
        assert_eq!(
            chains(&events),
            vec![(HI_RATE, HI_RATE, SignalChain::Direct)]
        );
        assert_eq!(
            conversions,
            Conversions {
                resampled_tracks: 0,
                resample_ms: 0.0,
                output_reconfigurations: 1,
            },
            "one reconfiguration, no conversion"
        );
    }

    /// **A device with no 48 kHz mode plays it anyway, and says what it did.**
    ///
    /// Refusing to play a file because the DAC is fussy would be the wrong
    /// answer, so the engine converts to the nearest rate the device has. What
    /// makes that acceptable is that the chain reports itself as converting,
    /// attributed to the device rather than to a setting.
    #[test]
    fn a_device_without_the_rate_converts_and_reports_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        let track = fixture_at(dir.path(), "hi.wav", HI_RATE, SHORT_FRAMES);
        let opened: Opened = Arc::default();
        // 44.1 kHz only: the nearest thing this device has to 48 kHz.
        let sink = DeviceDouble::new(RATE, &[RATE], &opened);
        let (events, conversions) = run_queue(vec![track], EngineConfig::default(), sink, RATE);

        assert!(
            opened.lock().expect("opened lock").is_empty(),
            "a device with nowhere to move must not be reopened"
        );
        assert_eq!(
            chains(&events),
            vec![(
                HI_RATE,
                RATE,
                SignalChain::Converting {
                    reason: ConversionReason::DeviceRateUnavailable,
                },
            )],
            "the conversion must be visible, and attributed to the device"
        );
        assert_eq!(
            conversions.resampled_tracks, 1,
            "the track must actually have been converted — it did play"
        );
        assert!(
            conversions.resample_ms > 0.0,
            "a resampler ran, so time was spent in one: {conversions:?}"
        );
    }

    /// **A mixed-rate queue follows every rate and converts nothing.**
    ///
    /// 44.1 → 48 → 44.1 on a device that has both: three sessions, two
    /// reconfigurations, three direct chains, and exactly one `QueueEnded` —
    /// the splits are an internal handover and a front end must never see one
    /// as the end of the queue.
    #[test]
    fn a_mixed_rate_queue_follows_every_rate() {
        let dir = tempfile::tempdir().expect("temp dir");
        let queue = vec![
            fixture_at(dir.path(), "a_44k.wav", RATE, SHORT_FRAMES),
            fixture_at(dir.path(), "b_48k.wav", HI_RATE, SHORT_FRAMES),
            fixture_at(dir.path(), "c_44k.wav", RATE, SHORT_FRAMES),
        ];
        let opened: Opened = Arc::default();
        let sink = DeviceDouble::new(RATE, &[RATE, HI_RATE], &opened);
        let (events, conversions) = run_queue(queue, EngineConfig::default(), sink, RATE);

        assert_eq!(
            *opened.lock().expect("opened lock"),
            vec![HI_RATE, RATE],
            "the output follows the music: up to 48 kHz and back down"
        );
        assert_eq!(
            chains(&events),
            vec![
                (RATE, RATE, SignalChain::Direct),
                (HI_RATE, HI_RATE, SignalChain::Direct),
                (RATE, RATE, SignalChain::Direct),
            ],
        );
        assert_eq!(conversions.resampled_tracks, 0, "nothing may be converted");
        assert_eq!(conversions.output_reconfigurations, 2);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::TrackStarted { .. }))
                .count(),
            3,
            "every track must still play"
        );
        assert_eq!(
            events.iter().filter(|e| **e == Event::QueueEnded).count(),
            1,
            "a rate change is a handover, not the end of the queue"
        );
    }

    /// The same queue under the explicit fixed-rate opt-in: one stream, no
    /// reopens, and the one track that differs converted — attributed to the
    /// setting, not to the device.
    #[test]
    fn a_fixed_output_rate_never_reopens() {
        let dir = tempfile::tempdir().expect("temp dir");
        let queue = vec![
            fixture_at(dir.path(), "a_44k.wav", RATE, SHORT_FRAMES),
            fixture_at(dir.path(), "b_48k.wav", HI_RATE, SHORT_FRAMES),
        ];
        let opened: Opened = Arc::default();
        let sink = DeviceDouble::new(RATE, &[RATE, HI_RATE], &opened);
        let cfg = EngineConfig {
            boundary: BoundaryPolicy::ResampleToStreamRate,
            ..EngineConfig::default()
        };
        let (events, conversions) = run_queue(queue, cfg, sink, RATE);

        assert!(
            opened.lock().expect("opened lock").is_empty(),
            "a fixed output rate is fixed: nothing may reopen it"
        );
        assert_eq!(
            chains(&events),
            vec![
                (RATE, RATE, SignalChain::Direct),
                (
                    HI_RATE,
                    RATE,
                    SignalChain::Converting {
                        reason: ConversionReason::FixedOutputRate,
                    },
                ),
            ],
        );
        assert_eq!(conversions.resampled_tracks, 1);
    }

    /// A rate change is the one boundary where the engine **drains** the sink
    /// instead of discarding it: the previous track's tail is audio the
    /// listener is owed, and cutting it off would turn a rate change into a
    /// truncated ending.
    ///
    /// Asserted on the operation log, in order — drain before the reopen, and
    /// no discard anywhere near it.
    #[test]
    fn a_rate_change_drains_rather_than_discards() {
        let dir = tempfile::tempdir().expect("temp dir");
        let queue = vec![
            fixture_at(dir.path(), "a_44k.wav", RATE, SHORT_FRAMES),
            fixture_at(dir.path(), "b_48k.wav", HI_RATE, SHORT_FRAMES),
        ];
        let ops: Arc<Mutex<Vec<Op>>> = Arc::default();
        let sink = DrainingDouble {
            rate: RATE,
            ops: Arc::clone(&ops),
        };
        let (events, _) = run_queue(queue, EngineConfig::default(), sink, RATE);
        assert_eq!(
            events.iter().filter(|e| **e == Event::QueueEnded).count(),
            1
        );

        let log = ops.lock().expect("ops lock").clone();
        let drain = log
            .iter()
            .position(|op| *op == Op::Drain)
            .expect("a rate change must drain the sink before reopening it");
        assert!(
            log[drain..].contains(&Op::Negotiate),
            "the drain must come before the reopen, not after: {log:?}"
        );
        assert!(
            !log.contains(&Op::Discard),
            "nothing about a rate change abandons audio: {log:?}"
        );
    }

    /// A [`DeviceDouble`] that holds its device exclusively — the ADR-0012
    /// backend's answer to [`Sink::is_exclusive`], without needing the
    /// hardware.
    ///
    /// It exists for the reason every double in this module does: the
    /// behaviour under test is the *engine's* — that the chain it reports says
    /// which arrangement is in use, in both the converting and the
    /// non-converting case — and that behaviour is identical whether the sink
    /// underneath is a real ALSA PCM or a struct that says `true`. The real
    /// backend's half (that a `hw:` device was actually opened, and at the
    /// format and rate claimed) is asserted against real hardware in
    /// `tests/playback.rs`, feature `exclusive-output`.
    struct ExclusiveDouble(DeviceDouble);

    impl Sink for ExclusiveDouble {
        fn write(&mut self, samples: &[f32]) {
            self.0.write(samples);
        }

        fn negotiate_rate(&mut self, desired: u32) -> Option<u32> {
            self.0.negotiate_rate(desired)
        }

        fn is_exclusive(&self) -> bool {
            true
        }
    }

    /// **A sink that owns its device is reported as owning it**, and the
    /// ordinary state is the one with no conversion in it.
    ///
    /// The same queue and the same device as `a_48k_queue_negotiates_a_48k_output`,
    /// which asserts `SignalChain::Direct` for the shared-mode sink: the only
    /// difference between the two tests is the sink's answer to
    /// [`Sink::is_exclusive`], and the only difference between their
    /// assertions is the variant. That pairing is the point — nothing else
    /// about the engine changes.
    #[test]
    fn an_exclusive_sink_reports_an_exclusive_chain() {
        let dir = tempfile::tempdir().expect("temp dir");
        let track = fixture_at(dir.path(), "hi.wav", HI_RATE, SHORT_FRAMES);
        let opened: Opened = Arc::default();
        let sink = ExclusiveDouble(DeviceDouble::new(RATE, &[RATE, HI_RATE], &opened));
        let (events, conversions) = run_queue(vec![track], EngineConfig::default(), sink, RATE);

        assert_eq!(
            chains(&events),
            vec![(
                HI_RATE,
                HI_RATE,
                SignalChain::Exclusive { conversion: None },
            )],
        );
        assert_eq!(conversions.resampled_tracks, 0, "nothing may be converted");
        let chain = chains(&events)[0].2;
        assert!(chain.is_exclusive());
        assert!(!chain.is_converting());
    }

    /// **Owning the device does not give it modes it does not have.**
    ///
    /// A `hw:` DAC with no 48 kHz mode is still a DAC with no 48 kHz mode, so
    /// the engine converts and reports *both* facts on one chain. This is why
    /// exclusivity is not a fourth `SignalChain` variant that excludes
    /// conversion: a front end must be able to say "held exclusively, and
    /// converting because the hardware cannot follow", which is a true and
    /// perfectly ordinary sentence.
    #[test]
    fn an_exclusive_sink_that_cannot_follow_reports_both_facts() {
        let dir = tempfile::tempdir().expect("temp dir");
        let track = fixture_at(dir.path(), "hi.wav", HI_RATE, SHORT_FRAMES);
        let opened: Opened = Arc::default();
        // 44.1 kHz only: the nearest thing this device has to 48 kHz.
        let sink = ExclusiveDouble(DeviceDouble::new(RATE, &[RATE], &opened));
        let (events, conversions) = run_queue(vec![track], EngineConfig::default(), sink, RATE);

        assert_eq!(
            chains(&events),
            vec![(
                HI_RATE,
                RATE,
                SignalChain::Exclusive {
                    conversion: Some(ConversionReason::DeviceRateUnavailable),
                },
            )],
        );
        assert_eq!(conversions.resampled_tracks, 1, "the track did play");
        let chain = chains(&events)[0].2;
        assert!(chain.is_exclusive());
        assert_eq!(
            chain.conversion_reason(),
            Some(ConversionReason::DeviceRateUnavailable)
        );
    }

    /// **A sink that says nothing about exclusivity is shared**, and the whole
    /// shared-mode readout is byte-for-byte what it was before ADR-0012.
    ///
    /// The default on [`Sink::is_exclusive`] is what every existing backend
    /// and every existing test double relies on, so this pins the default
    /// rather than leaving "unchanged" to be inferred from the other tests
    /// still passing.
    #[test]
    fn a_sink_that_does_not_claim_exclusivity_reports_the_shared_chain() {
        let dir = tempfile::tempdir().expect("temp dir");
        let track = fixture_at(dir.path(), "hi.wav", HI_RATE, SHORT_FRAMES);
        let opened: Opened = Arc::default();
        let sink = DeviceDouble::new(RATE, &[RATE, HI_RATE], &opened);
        assert!(
            !sink.is_exclusive(),
            "the trait default is shared, and every shipped cpal backend keeps it"
        );
        let (events, _) = run_queue(vec![track], EngineConfig::default(), sink, RATE);
        assert_eq!(
            chains(&events),
            vec![(HI_RATE, HI_RATE, SignalChain::Direct)],
        );
    }

    /// A sink that records drain/negotiate/discard ordering. Always grants the
    /// requested rate, so the only thing under test is *when* the engine asks.
    struct DrainingDouble {
        rate: u32,
        ops: Arc<Mutex<Vec<Op>>>,
    }

    impl DrainingDouble {
        fn record(&self, op: Op) {
            if let Ok(mut ops) = self.ops.lock() {
                ops.push(op);
            }
        }
    }

    impl Sink for DrainingDouble {
        fn write(&mut self, _samples: &[f32]) {}

        fn discard_buffered(&mut self) {
            self.record(Op::Discard);
        }

        fn drain_buffered(&mut self) {
            self.record(Op::Drain);
        }

        fn negotiate_rate(&mut self, desired: u32) -> Option<u32> {
            if desired != self.rate {
                self.rate = desired;
                self.record(Op::Negotiate);
            }
            Some(desired)
        }
    }

    // -----------------------------------------------------------------------
    // Volume (ADR-0011)
    // -----------------------------------------------------------------------

    use crate::protocol::VolumePath;
    use crate::volume::{MAX_POSITION, SharedVolume, Volume};

    /// A sink with an attenuator of its own — the backend ADR-0011 says baz
    /// does not currently ship, standing in for the one it may.
    ///
    /// It exists for the same reason [`DeviceDouble`] does: the interesting
    /// branch is unreachable on real hardware (no shipped backend accepts a
    /// device volume) and it is precisely the branch that must be right when
    /// one does. What is under test is the *engine's* half of the arrangement —
    /// that it offers the gain to the sink before falling back, that it does
    /// not then also scale the samples, that it reports
    /// [`VolumePath::DeviceAttenuator`], and that it re-offers after a reopen.
    struct AttenuatingDouble {
        rate: u32,
        /// Every gain the engine handed to the device, in order.
        accepted: Arc<Mutex<Vec<f32>>>,
        /// Everything written, so the test can prove the stream was *not*
        /// scaled as well.
        written: Arc<Mutex<Vec<f32>>>,
    }

    impl Sink for AttenuatingDouble {
        fn write(&mut self, samples: &[f32]) {
            if let Ok(mut out) = self.written.lock() {
                out.extend_from_slice(samples);
            }
        }

        fn negotiate_rate(&mut self, desired: u32) -> Option<u32> {
            self.rate = desired;
            Some(desired)
        }

        fn set_device_volume(&mut self, gain: f32) -> Option<()> {
            if let Ok(mut log) = self.accepted.lock() {
                log.push(gain);
            }
            Some(())
        }
    }

    /// Run `queue` to its end, sending `before_play` first, and report the
    /// events plus the engine's final volume snapshot.
    fn run_with_volume<S: Sink + Send + 'static>(
        queue: Vec<PathBuf>,
        sink: S,
        before_play: &[Command],
    ) -> (Vec<Event>, crate::volume::VolumeState) {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let volume: Arc<SharedVolume> = Arc::default();
        let shared = Arc::clone(&volume);
        let engine = thread::spawn(move || {
            let control = Control::new(
                cmd_rx,
                event_tx,
                EngineConfig::default(),
                RATE,
                Observable {
                    volume: shared,
                    ..Observable::default()
                },
                sink,
            );
            drop(control.run());
        });
        cmd_tx
            .send(Command::SetQueue { paths: queue })
            .expect("engine accepts commands");
        for command in before_play {
            cmd_tx
                .send(command.clone())
                .expect("engine accepts commands");
        }
        cmd_tx.send(Command::Play).expect("engine accepts commands");

        let mut events = Vec::new();
        let deadline = Instant::now() + TIMEOUT;
        while Instant::now() < deadline {
            match event_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Event::QueueEnded) => {
                    events.push(Event::QueueEnded);
                    break;
                }
                Ok(event) => events.push(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(cmd_tx);
        engine.join().expect("engine thread");
        let state = volume.snapshot();
        (events, state)
    }

    /// **A sink with its own attenuator gets the volume, and the samples do
    /// not.**
    ///
    /// This is the whole point of the ADR-0011 abstraction: when the output can
    /// carry the volume, the stream stays bit-exact and the readout says
    /// `DeviceAttenuator` rather than `SoftwareGain`. Asserted three ways — the
    /// device was handed the gain, the delivered samples are unscaled, and the
    /// reported path is the device's.
    #[test]
    fn a_sink_with_an_attenuator_carries_the_volume_and_the_stream_is_untouched() {
        let dir = tempfile::tempdir().expect("temp dir");
        let track = fixture_at(dir.path(), "tone.wav", RATE, SHORT_FRAMES);
        let accepted: Arc<Mutex<Vec<f32>>> = Arc::default();
        let written: Arc<Mutex<Vec<f32>>> = Arc::default();
        let sink = AttenuatingDouble {
            rate: RATE,
            accepted: Arc::clone(&accepted),
            written: Arc::clone(&written),
        };
        let (events, state) = run_with_volume(
            vec![track.clone()],
            sink,
            &[Command::SetVolume { position: 500 }],
        );

        let handed = accepted.lock().expect("accepted lock").clone();
        assert_eq!(
            handed.first().copied(),
            Some(Volume::new(500).amplitude()),
            "the device must be offered the taper's gain, not the raw position"
        );
        assert_eq!(
            state.path,
            VolumePath::DeviceAttenuator,
            "a sink that took the volume must be reported as carrying it"
        );
        assert!(state.path.is_transparent());

        let reference = crate::playback::AudioSource::decode_all(&track)
            .expect("decode reference")
            .samples;
        let delivered = written.lock().expect("written lock").clone();
        assert_eq!(
            delivered, reference,
            "the device carried the volume, so baz must not have scaled anything"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                Event::VolumeChanged {
                    path: VolumePath::DeviceAttenuator,
                    ..
                }
            )),
            "the path must be reported, not merely taken: {events:?}"
        );
    }

    /// A reopened output is a new output: whatever attenuation the old stream
    /// was carrying went with it, so the engine must re-offer the gain rather
    /// than assume it survived. Without this, following the source rate would
    /// silently reset the volume to unity mid-album.
    #[test]
    #[allow(clippy::float_cmp)] // the gain must be re-offered exactly, not nearly
    fn a_rate_change_re_establishes_the_device_volume() {
        let dir = tempfile::tempdir().expect("temp dir");
        let queue = vec![
            fixture_at(dir.path(), "a_44k.wav", RATE, SHORT_FRAMES),
            fixture_at(dir.path(), "b_48k.wav", HI_RATE, SHORT_FRAMES),
        ];
        let accepted: Arc<Mutex<Vec<f32>>> = Arc::default();
        let sink = AttenuatingDouble {
            rate: RATE,
            accepted: Arc::clone(&accepted),
            written: Arc::default(),
        };
        let (_events, state) =
            run_with_volume(queue, sink, &[Command::SetVolume { position: 250 }]);

        let gain = Volume::new(250).amplitude();
        let handed = accepted.lock().expect("accepted lock").clone();
        assert!(
            handed.len() >= 2,
            "the gain must be re-offered after the output is reopened: {handed:?}"
        );
        assert!(
            handed.iter().all(|g| *g == gain),
            "every offer must be the same gain the listener set: {handed:?}"
        );
        assert_eq!(state.volume, Volume::new(250));
        assert_eq!(state.path, VolumePath::DeviceAttenuator);
    }

    /// A sink with no attenuator — every backend baz actually ships — falls
    /// back to software gain and says so.
    #[test]
    fn a_sink_without_an_attenuator_falls_back_to_software_gain() {
        let dir = tempfile::tempdir().expect("temp dir");
        let track = fixture_at(dir.path(), "tone.wav", RATE, SHORT_FRAMES);
        let opened: Opened = Arc::default();
        let sink = DeviceDouble::new(RATE, &[RATE], &opened);
        let (events, state) =
            run_with_volume(vec![track], sink, &[Command::SetVolume { position: 500 }]);

        assert_eq!(state.path, VolumePath::SoftwareGain);
        assert!(
            !state.path.is_transparent(),
            "software gain must not claim the stream is untouched"
        );
        assert_eq!(
            events
                .iter()
                .filter_map(|e| match e {
                    Event::VolumeChanged {
                        position,
                        muted,
                        path,
                    } => Some((*position, *muted, *path)),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![(500, false, VolumePath::SoftwareGain)],
            "exactly one change, reported once"
        );
    }

    /// Unity is not merely a gain of one — it is reported as its own state, so
    /// a front end can say "nothing is being applied" without inferring it from
    /// a float comparison of its own.
    #[test]
    fn returning_to_unity_is_reported_as_unity() {
        let dir = tempfile::tempdir().expect("temp dir");
        let track = fixture_at(dir.path(), "tone.wav", RATE, SHORT_FRAMES);
        let opened: Opened = Arc::default();
        let sink = DeviceDouble::new(RATE, &[RATE], &opened);
        let (events, state) = run_with_volume(
            vec![track],
            sink,
            &[
                Command::SetVolume { position: 500 },
                Command::SetVolume {
                    position: MAX_POSITION,
                },
            ],
        );

        assert_eq!(state.volume, Volume::UNITY);
        assert_eq!(state.path, VolumePath::Unity);
        let paths: Vec<VolumePath> = events
            .iter()
            .filter_map(|e| match e {
                Event::VolumeChanged { path, .. } => Some(*path),
                _ => None,
            })
            .collect();
        assert_eq!(
            paths,
            vec![VolumePath::SoftwareGain, VolumePath::Unity],
            "down from unity and back must be two statements, in that order"
        );
    }

    /// Redundant commands emit nothing — the rule the whole protocol follows
    /// (module docs), and the one that keeps a slider dragged across the same
    /// pixel from flooding the event channel.
    #[test]
    fn a_redundant_volume_command_says_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let track = fixture_at(dir.path(), "tone.wav", RATE, SHORT_FRAMES);
        let opened: Opened = Arc::default();
        let sink = DeviceDouble::new(RATE, &[RATE], &opened);
        let (events, _) = run_with_volume(
            vec![track],
            sink,
            &[
                // Already unity, already unmuted: neither is news.
                Command::SetVolume {
                    position: MAX_POSITION,
                },
                Command::SetMute { muted: false },
            ],
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::VolumeChanged { .. })),
            "nothing changed, so nothing may be announced: {events:?}"
        );
    }
}
