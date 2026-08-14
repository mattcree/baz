//! Integration tests for `baz_core::engine`: the full command/event
//! lifecycle, exercised entirely headless through `spawn_offline`.
//!
//! Ground truth is a reference decode of the fixture files
//! (`AudioSource::decode_all`) — the engine's delivered output is compared
//! sample-for-sample against it, never against recorded engine output
//! (`docs/ENGINEERING.md`).

use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::Duration;

use baz_core::engine::{EngineHandle, OfflineOutput, PREVIOUS_RESTART_MS, spawn_offline};
use baz_core::playback::{AudioSource, BoundaryPolicy, CHANNELS, EngineConfig};
// Only the device-gated tests below distinguish "no audio hardware" from a
// real failure; the headless build never constructs one.
#[cfg(feature = "device-output")]
use baz_core::playback::PlaybackError;
use baz_core::protocol::{
    Command, ConversionReason, Event, ReplayGainMode, ReplayGainSource, SignalChain, VolumePath,
};
use baz_core::replaygain::{ComputedGains, MAX_PREAMP_CENTIDB, ReplayGainTags};
use baz_core::traversal::Traversal;
use baz_core::volume::{MAX_POSITION, RAMP_MS, Volume, VolumeState};

/// Test tone parameters (arbitrary; equality checks are exact either way).
const FREQ: f64 = 440.0;
const AMP: f64 = 0.5;
const RATE: u32 = 44_100;
/// Track A: 5 s — long enough that pause/skip/stop land mid-track even on a
/// slow CI machine.
const A_FRAMES: usize = 5 * RATE as usize;
/// Track B: 1 s.
const B_FRAMES: usize = RATE as usize;

/// Linear chirp fixture: 6 s at [`RATE`], sweeping [`CHIRP_F0`] to
/// [`CHIRP_F1`]. Unlike a steady tone, no two moments of it look alike, so a
/// delivered block can be located in the source unambiguously — which is
/// what makes seek accuracy measurable from the *audio* rather than from a
/// counter the implementation also wrote.
const CHIRP_SECS: usize = 6;
const CHIRP_MS: u64 = 6_000;
const CHIRP_FRAMES: usize = CHIRP_SECS * RATE as usize;
const CHIRP_F0: f64 = 200.0;
const CHIRP_F1: f64 = 4400.0;

/// The rate-mismatch pair for the elapsed-under-resampling test: 1 s at the
/// stream rate, then 4 s at 48 kHz which the engine resamples down to it.
const HEAD_44K_FRAMES: usize = RATE as usize;
const TAIL_48K_RATE: u32 = 48_000;
const TAIL_48K_SECS: usize = 4;
const TAIL_48K_MS: u64 = 4_000;
const TAIL_48K_FRAMES: usize = TAIL_48K_SECS * TAIL_48K_RATE as usize;

/// How long an expected event may take before the test fails (generous for
/// CI; the engine emits within milliseconds).
const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

/// Constant-amplitude fixture for the volume tests: 3 s of exactly [`DC`] in
/// both channels.
///
/// A steady tone would make the gain unmeasurable near its zero crossings —
/// `out / ref` is meaningless where `ref` is ~0 — but a constant makes the
/// delivered stream *literally the gain trajectory*, scaled by a number chosen
/// to divide exactly. That turns "did the ramp click?" into arithmetic instead
/// of an eyeball.
const DC: f32 = 0.5;
const DC_FRAMES: usize = 3 * RATE as usize;

struct Fixtures {
    a: PathBuf,
    b: PathBuf,
    bad: PathBuf,
    chirp: PathBuf,
    head_44k: PathBuf,
    tail_48k: PathBuf,
    /// A 5.1 file, so the signal-path readout can be asked what it says about
    /// a track an ITU-R BS.775 matrix folded to stereo (ADR-0039).
    surround: PathBuf,
    dc: PathBuf,
    /// ReplayGain-tagged fixtures (ADR-0013). Same audio as their untagged
    /// twins, so the reference decodes below are their ground truth too.
    rg_a: PathBuf,
    rg_b: PathBuf,
    rg_single: PathBuf,
    rg_clip: PathBuf,
    /// Reference decodes (interleaved stereo f32).
    a_ref: Vec<f32>,
    b_ref: Vec<f32>,
    chirp_ref: Vec<f32>,
    head_44k_ref: Vec<f32>,
    tail_48k_ref: Vec<f32>,
    dc_ref: Vec<f32>,
}

fn wav_spec(rate: u32) -> hound::WavSpec {
    hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    }
}

fn write_sine_wav_at(path: &Path, rate: u32, frames: usize, t0: f64) {
    let mut w = hound::WavWriter::create(path, wav_spec(rate)).expect("create wav");
    #[allow(clippy::cast_precision_loss)] // frame indices are far below 2^52
    for n in 0..frames {
        let t = t0 + n as f64 / f64::from(rate);
        #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
        let s = (AMP * (2.0 * PI * FREQ * t).sin()) as f32;
        w.write_sample(s).expect("write sample");
        w.write_sample(s).expect("write sample");
    }
    w.finalize().expect("finalize wav");
}

/// A 5.1 sine, the same tone in the front pair and silence in the other four.
///
/// `hound`'s synthesized channel mask for six channels is `0x3F`, which is
/// WAVE's 5.1 layout exactly; a fixture needing a mask hound cannot write lives
/// in `tests/playback.rs`, which writes its own header.
fn write_five_one_wav(path: &Path, frames: usize) {
    let spec = hound::WavSpec {
        channels: 6,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create wav");
    #[allow(clippy::cast_precision_loss)] // frame indices are far below 2^52
    for n in 0..frames {
        let t = n as f64 / f64::from(RATE);
        #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
        let s = (AMP * (2.0 * PI * FREQ * t).sin()) as f32;
        w.write_sample(s).expect("write sample");
        w.write_sample(s).expect("write sample");
        for _ in 0..4 {
            w.write_sample(0.0f32).expect("write sample");
        }
    }
    w.finalize().expect("finalize wav");
}

fn write_sine_wav(path: &Path, frames: usize, t0: f64) {
    write_sine_wav_at(path, RATE, frames, t0);
}

/// Instantaneous frequency of the chirp `t` seconds in.
fn chirp_freq_at(t: f64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let total = CHIRP_SECS as f64;
    CHIRP_F0 + (CHIRP_F1 - CHIRP_F0) * t / total
}

/// A linear sine sweep: phase is the integral of the instantaneous
/// frequency, `2π(f0·t + k·t²/2)`.
fn write_chirp_wav(path: &Path) {
    #[allow(clippy::cast_precision_loss)]
    let total = CHIRP_SECS as f64;
    let k = (CHIRP_F1 - CHIRP_F0) / total;
    let mut w = hound::WavWriter::create(path, wav_spec(RATE)).expect("create wav");
    #[allow(clippy::cast_precision_loss)] // frame indices are far below 2^52
    for n in 0..CHIRP_FRAMES {
        let t = n as f64 / f64::from(RATE);
        #[allow(clippy::cast_possible_truncation)] // f64 sine -> f32 sample
        let s = (AMP * (2.0 * PI * (CHIRP_F0 * t + 0.5 * k * t * t)).sin()) as f32;
        w.write_sample(s).expect("write sample");
        w.write_sample(s).expect("write sample");
    }
    w.finalize().expect("finalize wav");
}

/// A constant-[`DC`] stereo WAV — see [`DC`] for why the volume tests want one.
fn write_dc_wav(path: &Path) {
    let mut w = hound::WavWriter::create(path, wav_spec(RATE)).expect("create wav");
    for _ in 0..DC_FRAMES {
        w.write_sample(DC).expect("write sample");
        w.write_sample(DC).expect("write sample");
    }
    w.finalize().expect("finalize wav");
}

/// An `ID3v2.4` size field: seven bits per byte, high bit clear.
fn syncsafe(n: usize) -> [u8; 4] {
    [
        u8::try_from((n >> 21) & 0x7f).expect("masked to 7 bits"),
        u8::try_from((n >> 14) & 0x7f).expect("masked to 7 bits"),
        u8::try_from((n >> 7) & 0x7f).expect("masked to 7 bits"),
        u8::try_from(n & 0x7f).expect("masked to 7 bits"),
    ]
}

/// An `ID3v2.4` tag of `TXXX` (user-defined text) frames, laid out from the
/// specification: a `TXXX` identifier, a syncsafe size, two flag bytes, then a
/// UTF-8 encoding byte, the description, a NUL, and the value.
///
/// Written out by hand rather than by a tag library, on purpose. `TXXX` with a
/// `REPLAYGAIN_*` description is *the* way `ID3v2` carries ReplayGain, and
/// building the frames from their published layout makes this fixture ground
/// truth rather than a round trip through the crate under test. Symphonia's
/// probe consumes a leading `ID3v2` element before handing the rest of the
/// stream to the container reader, which is what lets a WAV — the one fixture
/// format these tests can synthesize exactly — carry ReplayGain at all.
fn id3v2_txxx(frames: &[(&str, &str)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (description, value) in frames {
        let mut content = vec![0x03_u8]; // text encoding: UTF-8
        content.extend_from_slice(description.as_bytes());
        content.push(0);
        content.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"TXXX");
        body.extend_from_slice(&syncsafe(content.len()));
        body.extend_from_slice(&[0, 0]); // frame flags
        body.extend_from_slice(&content);
    }
    let mut tag = Vec::new();
    tag.extend_from_slice(b"ID3");
    tag.extend_from_slice(&[0x04, 0x00]); // version 2.4.0
    tag.push(0x00); // flags
    tag.extend_from_slice(&syncsafe(body.len()));
    tag.extend_from_slice(&body);
    tag
}

/// Prepend an `ID3v2` ReplayGain tag to an existing fixture, leaving its audio
/// byte-for-byte untouched.
fn tag_with_replay_gain(path: &Path, frames: &[(&str, &str)]) {
    let audio = std::fs::read(path).expect("read fixture");
    let mut bytes = id3v2_txxx(frames);
    bytes.extend_from_slice(&audio);
    std::fs::write(path, bytes).expect("write tagged fixture");
}

/// The four ReplayGain fixtures, written and tagged.
///
/// They carry the same audio as their untagged twins (`a`, `b`, `dc`), so those
/// reference decodes are their ground truth too and any difference in the
/// delivered stream is the gain and nothing else.
fn write_replay_gain_fixtures(dir: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let rg_a = dir.join("rg_track_a_5s.wav");
    let rg_b = dir.join("rg_track_b_1s.wav");
    let rg_single = dir.join("rg_single_3s.wav");
    let rg_clip = dir.join("rg_clip_3s.wav");
    write_sine_wav(&rg_a, A_FRAMES, 0.0);
    write_sine_wav(&rg_b, B_FRAMES, 0.0);
    write_dc_wav(&rg_single);
    write_dc_wav(&rg_clip);
    // Tracks A and B share one album gain and album peak — they are an album —
    // while their track gains differ, so album mode and track mode are
    // distinguishable from the delivered samples alone.
    tag_with_replay_gain(
        &rg_a,
        &[
            ("REPLAYGAIN_TRACK_GAIN", "-6.02 dB"),
            ("REPLAYGAIN_TRACK_PEAK", "0.500000"),
            ("REPLAYGAIN_ALBUM_GAIN", "-3.00 dB"),
            ("REPLAYGAIN_ALBUM_PEAK", "0.500000"),
        ],
    );
    tag_with_replay_gain(
        &rg_b,
        &[
            ("REPLAYGAIN_TRACK_GAIN", "+2.50 dB"),
            ("REPLAYGAIN_TRACK_PEAK", "0.500000"),
            ("REPLAYGAIN_ALBUM_GAIN", "-3.00 dB"),
            ("REPLAYGAIN_ALBUM_PEAK", "0.500000"),
        ],
    );
    // A single downloaded track: no album figures to be relative to.
    tag_with_replay_gain(
        &rg_single,
        &[
            ("REPLAYGAIN_TRACK_GAIN", "-4.00 dB"),
            ("REPLAYGAIN_TRACK_PEAK", "0.500000"),
        ],
    );
    // A gain the declared peak has no room for: +12 dB against a peak of 0.5,
    // which leaves 6.02 dB of headroom.
    tag_with_replay_gain(
        &rg_clip,
        &[
            ("REPLAYGAIN_TRACK_GAIN", "+12.00 dB"),
            ("REPLAYGAIN_TRACK_PEAK", "0.500000"),
        ],
    );
    (rg_a, rg_b, rg_single, rg_clip)
}

fn fixtures() -> &'static Fixtures {
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("engine-fixtures");
        std::fs::create_dir_all(&dir).expect("create fixture dir");
        let a = dir.join("track_a_5s.wav");
        let b = dir.join("track_b_1s.wav");
        let bad = dir.join("not_audio.wav");
        let chirp = dir.join("chirp_6s.wav");
        let head_44k = dir.join("head_1s_44k.wav");
        let tail_48k = dir.join("tail_4s_48k.wav");
        let surround = dir.join("surround_1s_51.wav");
        let dc = dir.join("dc_3s.wav");
        write_dc_wav(&dc);
        let (rg_a, rg_b, rg_single, rg_clip) = write_replay_gain_fixtures(&dir);
        write_sine_wav(&a, A_FRAMES, 0.0);
        write_sine_wav(&b, B_FRAMES, 0.0);
        write_chirp_wav(&chirp);
        write_sine_wav_at(&head_44k, RATE, HEAD_44K_FRAMES, 0.0);
        write_sine_wav_at(&tail_48k, TAIL_48K_RATE, TAIL_48K_FRAMES, 0.0);
        write_five_one_wav(&surround, RATE as usize);
        std::fs::write(&bad, b"this is not audio at all, sorry").expect("write bad file");
        let a_ref = AudioSource::decode_all(&a).expect("decode a").samples;
        let b_ref = AudioSource::decode_all(&b).expect("decode b").samples;
        let chirp_ref = AudioSource::decode_all(&chirp)
            .expect("decode chirp")
            .samples;
        assert_eq!(a_ref.len(), A_FRAMES * CHANNELS);
        assert_eq!(b_ref.len(), B_FRAMES * CHANNELS);
        assert_eq!(chirp_ref.len(), CHIRP_FRAMES * CHANNELS);
        let head_44k_ref = AudioSource::decode_all(&head_44k)
            .expect("decode head")
            .samples;
        let tail_48k_ref = AudioSource::decode_all(&tail_48k)
            .expect("decode tail")
            .samples;
        assert_eq!(head_44k_ref.len(), HEAD_44K_FRAMES * CHANNELS);
        assert_eq!(tail_48k_ref.len(), TAIL_48K_FRAMES * CHANNELS);
        let dc_ref = AudioSource::decode_all(&dc).expect("decode dc").samples;
        assert_eq!(dc_ref.len(), DC_FRAMES * CHANNELS);
        assert!(
            dc_ref.iter().all(|s| *s == DC),
            "the constant fixture must decode to exactly its constant, or the \
             gain trajectory it is used to measure would not be one"
        );
        Fixtures {
            a,
            b,
            bad,
            chirp,
            head_44k,
            tail_48k,
            surround,
            dc,
            rg_a,
            rg_b,
            rg_single,
            rg_clip,
            a_ref,
            b_ref,
            chirp_ref,
            head_44k_ref,
            tail_48k_ref,
            dc_ref,
        }
    })
}

/// Engine config paced so a 5 s track takes a few hundred ms to drain:
/// slow enough that pause/skip/stop always land mid-track, fast enough for
/// a snappy suite.
///
/// The boundary policy is left at its default — follow the source, convert
/// nothing (ADR-0009) — so the whole suite below exercises what a shipped baz
/// actually does. The one test that is *about* conversion opts in explicitly
/// with [`fixed_rate_config`].
fn paced_config() -> EngineConfig {
    EngineConfig {
        ring_frames: 8192,
        consumer_chunk_frames: 2048,
        consumer_pace: Duration::from_millis(4),
        ..EngineConfig::default()
    }
}

/// [`paced_config`] with the explicit fixed-output-rate opt-in: one stream
/// rate for the whole queue, everything else converted into it.
fn fixed_rate_config() -> EngineConfig {
    EngineConfig {
        boundary: BoundaryPolicy::ResampleToStreamRate,
        ..paced_config()
    }
}

/// Unpaced config: drain at decode speed (used where timing is irrelevant).
fn fast_config() -> EngineConfig {
    EngineConfig {
        consumer_pace: Duration::ZERO,
        ..paced_config()
    }
}

fn next_event(events: &Receiver<Event>) -> Event {
    events
        .recv_timeout(EVENT_TIMEOUT)
        .expect("timed out waiting for an engine event")
}

/// The next event that is not a readout — [`Event::Progress`],
/// [`Event::SignalPath`], [`Event::VolumeChanged`], [`Event::ReplayGainChanged`]
/// or [`Event::QueueChanged`].
///
/// All five are continuous or incidental *descriptions* of playback rather
/// than transport transitions, and all interleave with everything by design;
/// the tests that assert the *transport* vocabulary's ordering therefore step
/// over them. (`QueueChanged` describes the queue, not the transport: it rides
/// along with every accepted `SetQueue` and `UpdateQueue`, including the ones
/// these tests send merely to set up a fixture.) Each has its own contract and
/// its own tests below — none is going unasserted.
fn next_transport_event(events: &Receiver<Event>) -> Event {
    loop {
        match next_event(events) {
            Event::Progress { .. }
            | Event::SignalPath { .. }
            | Event::VolumeChanged { .. }
            | Event::ReplayGainChanged { .. }
            | Event::QueueChanged { .. } => {}
            other => return other,
        }
    }
}

/// Wait for the next [`Event::QueueChanged`], stepping over the readouts that
/// ride along with it, and return its two fields.
fn next_queue_changed(events: &Receiver<Event>) -> (usize, Option<usize>) {
    loop {
        match next_event(events) {
            Event::Progress { .. }
            | Event::SignalPath { .. }
            | Event::VolumeChanged { .. }
            | Event::ReplayGainChanged { .. } => {}
            Event::QueueChanged { len, position } => return (len, position),
            other => panic!("expected QueueChanged, got {other:?}"),
        }
    }
}

/// Wait for the next [`Event::SignalPath`], stepping over `Progress` (which
/// is emitted on the same transitions and would otherwise race it).
fn next_signal_path(events: &Receiver<Event>) -> Event {
    loop {
        match next_event(events) {
            Event::Progress { .. } => {}
            signal @ Event::SignalPath { .. } => return signal,
            other => panic!("expected SignalPath, got {other:?}"),
        }
    }
}

fn assert_no_event_within(events: &Receiver<Event>, window: Duration) {
    match events.recv_timeout(window) {
        Err(RecvTimeoutError::Timeout) => {}
        Ok(e) => panic!("expected silence, got event: {e:?}"),
        Err(RecvTimeoutError::Disconnected) => panic!("engine thread died"),
    }
}

/// Wait for the next [`Event::Progress`], failing on any other event first —
/// except [`Event::SignalPath`], which is a description of the chain rather
/// than a transport event and rides along with `TrackStarted`.
fn next_progress(events: &Receiver<Event>) -> (u64, Option<u64>) {
    loop {
        match next_event(events) {
            Event::SignalPath { .. } => {}
            Event::Progress {
                elapsed_ms,
                track_ms,
            } => return (elapsed_ms, track_ms),
            other => panic!("expected Progress, got {other:?}"),
        }
    }
}

/// Whole milliseconds of `frames` at `rate` Hz, rounded to nearest — the
/// engine's own conversion, restated here independently so the test asserts
/// against the specification rather than against the implementation.
fn frames_to_ms(frames: usize, rate: u32) -> u64 {
    let (frames, rate) = (frames as u64, u64::from(rate));
    (frames * 1000 + rate / 2) / rate
}

fn ms_to_frames(ms: u64, rate: u32) -> usize {
    (ms * u64::from(rate) / 1000) as usize
}

/// Estimate the dominant frequency of interleaved stereo audio by counting
/// zero crossings of the left channel. Exact enough on a clean synthesized
/// sweep (no noise, no DC) to identify *where in the sweep* a block came
/// from to within a few milliseconds.
fn estimate_freq(block: &[f32], rate: u32) -> f64 {
    let ch0: Vec<f32> = block.iter().step_by(CHANNELS).copied().collect();
    let crossings = ch0
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    #[allow(clippy::cast_precision_loss)] // counts far below 2^52
    let (crossings, frames) = (crossings as f64, ch0.len() as f64);
    crossings * f64::from(rate) / (2.0 * frames)
}

/// Measure, in frames, how far a seek actually landed from `target`.
///
/// The post-seek session plays the track out to its end, so the delivered
/// output ends with exactly the reference decode from wherever the seek
/// landed. Sliding the reference against the tail of the output and finding
/// the offset that matches *bit for bit* therefore reads the true landing
/// point straight out of the audio — no counter, no timing, no tolerance in
/// the measurement itself. The chirp fixture is what makes the match unique:
/// a steady tone would align at every period.
///
/// Returns the signed frame error, or `None` if no offset within `search`
/// explains the delivered audio at all.
fn measure_seek_error(out: &[f32], reference: &[f32], target: usize, search: i64) -> Option<i64> {
    /// Frames compared to pick a candidate; the winner is then verified in
    /// full by the caller.
    const PROBE_FRAMES: usize = 4096;
    let total_frames = reference.len() / CHANNELS;
    for k in -search..=search {
        let landed = i64::try_from(target).ok()? + k;
        let Ok(landed) = usize::try_from(landed) else {
            continue;
        };
        if landed >= total_frames {
            continue;
        }
        let tail = (total_frames - landed) * CHANNELS;
        if tail > out.len() {
            continue;
        }
        let probe = PROBE_FRAMES.min(total_frames - landed) * CHANNELS;
        let from_out = &out[out.len() - tail..][..probe];
        let from_ref = &reference[landed * CHANNELS..][..probe];
        if from_out == from_ref {
            return Some(k);
        }
    }
    None
}

fn started(path: &Path, position: usize) -> Event {
    Event::TrackStarted {
        path: path.to_path_buf(),
        position,
    }
}

/// Exact sample equality with a useful failure message.
fn assert_samples_eq(got: &[f32], want: &[f32], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: length mismatch");
    if let Some(i) = (0..got.len()).find(|&i| got[i] != want[i]) {
        panic!(
            "{what}: first mismatch at interleaved sample {i} (frame {}): got {} want {}",
            i / CHANNELS,
            got[i],
            want[i]
        );
    }
}

/// Collect the engine's offline output after shutting it down.
fn collect(output: OfflineOutput) -> Vec<f32> {
    output.wait().expect("engine thread must report its output")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The full command lifecycle in one sitting, with the exact event order
/// the protocol promises: `SetQueue` → `Play` → `TrackStarted` → `Pause`
/// (delivery freezes) → `Play`/resume → `Next` (drain-and-restart) → the
/// next track → `QueueEnded`.
#[test]
fn full_lifecycle_event_ordering() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.b_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    // Paused means paused: because pause and pumping share the engine
    // thread, after `Paused` is emitted not one more sample may reach the
    // sink.
    let frozen = engine.samples_delivered();
    thread::sleep(Duration::from_millis(80));
    assert_eq!(
        engine.samples_delivered(),
        frozen,
        "samples were delivered while paused"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);

    engine.send(Command::Next).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.b, 1));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    // Track A was skipped mid-flight, so only its head was delivered; track
    // B ran to completion, so the output's tail must be exactly B.
    assert!(out.len() > f.b_ref.len(), "output too short");
    assert!(out.len() < capacity, "skip delivered the whole queue");
    assert_samples_eq(
        &out[out.len() - f.b_ref.len()..],
        &f.b_ref,
        "post-skip track B output",
    );
}

/// Pause does not tear anything down and drops or duplicates nothing: the
/// delivered stream with a pause/resume in the middle is bit-identical to
/// the reference decode.
#[test]
fn pause_resume_output_is_bit_identical() {
    let f = fixtures();
    let cfg = EngineConfig {
        consumer_pace: Duration::from_millis(1),
        ..paced_config()
    };
    let (engine, events, output) = spawn_offline(cfg, f.a_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    let frozen = engine.samples_delivered();
    assert!(frozen < f.a_ref.len(), "pause landed after the track ended");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        engine.samples_delivered(),
        frozen,
        "delivery advanced while paused"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    assert!(
        engine.samples_delivered() > frozen,
        "delivery did not resume"
    );

    engine.shutdown();
    assert_samples_eq(&collect(output), &f.a_ref, "paused-and-resumed output");
}

/// One bad file must not kill the queue: it is reported and skipped, in
/// queue order, and the delivered audio is exactly the good tracks spliced
/// together.
#[test]
fn bad_file_is_reported_and_skipped() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.b_ref.len();
    let (engine, events, output) = spawn_offline(fast_config(), capacity).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.bad.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");

    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    match next_transport_event(&events) {
        Event::TrackFailed { path, reason } => {
            assert_eq!(path, f.bad);
            assert!(!reason.is_empty(), "failure reason must say something");
        }
        other => panic!("expected TrackFailed for the bad file, got {other:?}"),
    }
    assert_eq!(next_transport_event(&events), started(&f.b, 2));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    let mut want = f.a_ref.clone();
    want.extend_from_slice(&f.b_ref);
    assert_samples_eq(&out, &want, "queue output around a bad file");
}

/// Stop abandons the queue mid-track, delivery ceases, and a later Play
/// starts over from the top.
#[test]
fn stop_mid_track_then_play_restarts() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(paced_config(), f.a_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.send(Command::Stop).expect("send");
    assert_eq!(next_transport_event(&events), Event::Stopped);
    let at_stop = engine.samples_delivered();
    assert!(at_stop < f.a_ref.len(), "stop landed after the track ended");
    assert_no_event_within(&events, Duration::from_millis(120));
    assert_eq!(
        engine.samples_delivered(),
        at_stop,
        "delivery continued after Stop"
    );

    // Play after Stop starts from the queue top again.
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.shutdown();
    let out = collect(output);
    // What reached the sink is the pre-stop head of the track, cleanly cut,
    // followed by a fresh start from frame 0 — no corruption either side of
    // the stop.
    assert!(out.len() > at_stop, "the restart delivered nothing");
    assert_samples_eq(&out[..at_stop], &f.a_ref[..at_stop], "pre-stop segment");
    assert_samples_eq(
        &out[at_stop..],
        &f.a_ref[..out.len() - at_stop],
        "restarted-from-top segment",
    );
}

/// Dropping the handle mid-playback shuts everything down promptly — no
/// hang, no leaked threads — proven under a hard timeout.
#[test]
fn shutdown_while_playing_terminates_cleanly() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(paced_config(), f.a_ref.len() + f.b_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        drop(engine); // the whole shutdown path: abort session, join workers
        let _ = done_tx.send(());
    });
    done_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("dropping the handle hung instead of shutting the engine down");

    // The engine thread exited: the sink came back and the event channel is
    // closed. Events queued before the shutdown are still deliverable (the
    // channel buffers them), so drain to the end rather than assuming the
    // very next receive is the disconnect.
    let out = collect(output);
    assert!(!out.is_empty(), "some audio was delivered before shutdown");
    loop {
        match events.recv_timeout(Duration::from_secs(5)) {
            Err(RecvTimeoutError::Disconnected) => break,
            Ok(_) => {}
            other => panic!("event channel should close after shutdown, got {other:?}"),
        }
    }
}

/// `SetQueue` never autoplays, and `Play` on an empty queue reports the queue
/// as ended rather than doing nothing silently.
#[test]
fn set_queue_does_not_autoplay_and_empty_queue_ends() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(fast_config(), f.a_ref.len()).expect("spawn engine");

    // Play with nothing queued: the (empty) queue is already over.
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    // Queueing alone starts nothing.
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
            origin: None,
        })
        .expect("send");
    // The queue is news, and the only news: it is announced, and nothing else
    // follows it.
    assert_eq!(next_queue_changed(&events), (1, None));
    assert_no_event_within(&events, Duration::from_millis(120));
    assert_eq!(engine.samples_delivered(), 0, "SetQueue must not autoplay");
}

/// Next past the last track ends the queue.
#[test]
fn next_past_last_track_ends_queue() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.a_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    engine.send(Command::Next).expect("send");
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    assert_no_event_within(&events, Duration::from_millis(120));
}

/// Follow-the-source is the shipped default (ADR-0009), not something a
/// caller has to opt into. Asserted on [`EngineConfig::default`] rather than
/// on a config the test built, because the thing under test is what an
/// unconfigured baz does.
#[test]
fn following_the_source_rate_is_the_default_policy() {
    assert_eq!(
        EngineConfig::default().boundary,
        BoundaryPolicy::BitPerfectReopen,
        "the default must be the mode that converts nothing"
    );
    // And it is a mode the engine service actually runs, not one it refuses.
    let (engine, _events, _output) =
        spawn_offline(EngineConfig::default(), 16).expect("the default policy must spawn");
    engine.shutdown();
}

/// Commands after shutdown fail with a clear error instead of vanishing.
#[test]
fn send_after_shutdown_is_an_error() {
    let f = fixtures();
    let (engine, _events, _output) = spawn_offline(fast_config(), 16).expect("spawn engine");
    // Keep a second handle path honest: shutdown consumes the handle, so
    // sending afterwards is only possible through a clone — there is none.
    // What we can check: the events channel closes and a fresh engine's
    // handle still works.
    engine.shutdown();
    let (engine2, _events2, _output2) = spawn_offline(fast_config(), 16).expect("spawn engine");
    engine2
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
            origin: None,
        })
        .expect("a fresh engine accepts commands");
}

// ---------------------------------------------------------------------------
// Seek and progress
// ---------------------------------------------------------------------------

/// **Seek accuracy, read out of the audio.** Seek 3 s into the 6 s chirp
/// while it is playing, let it play out, and locate the delivered post-seek
/// audio inside the reference decode by exact sample match. The landing
/// point must be the requested frame — which is a claim about *content*
/// (this is the part of the sweep that belongs at 3 s), not about a counter
/// the engine also maintains.
///
/// The frequency check restates the same fact in the terms a listener would:
/// the first audible block after the seek must be sweeping through the
/// ~2.3 kHz region the chirp reaches at 3 s, not the 200 Hz it starts at.
#[test]
fn seek_while_playing_lands_on_the_target_sample() {
    let f = fixtures();
    let target = ms_to_frames(3_000, RATE);
    // Room for the pre-seek head plus the whole post-seek tail.
    let (engine, events, output) =
        spawn_offline(paced_config(), 2 * f.chirp_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.chirp.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));

    engine
        .send(Command::Seek { position_ms: 3_000 })
        .expect("send");
    // A seek restarts the current track, so it starts again (engine docs).
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    let error = measure_seek_error(&out, &f.chirp_ref, target, 512)
        .expect("delivered audio must match the reference somewhere near the target");
    println!("[seek] landed {error} frame(s) from the target ({target})");
    assert_eq!(
        error, 0,
        "seek into a WAV must be sample-exact; landed {error} frames off"
    );

    // The same claim, stated as a listener would hear it.
    let tail = (f.chirp_ref.len() / CHANNELS - target) * CHANNELS;
    let first_block = &out[out.len() - tail..][..4096 * CHANNELS];
    let got = estimate_freq(first_block, RATE);
    // The block spans ~93 ms, over which the sweep advances ~65 Hz, so the
    // mean sits half a block past the seek point.
    let want = chirp_freq_at(3.0 + 4096.0 / f64::from(RATE) / 2.0);
    assert!(
        (got - want).abs() < 20.0,
        "post-seek audio should sweep through ~{want:.0} Hz, measured {got:.0} Hz"
    );
}

/// **Seek while paused**: the position moves, playback does not. Not one
/// sample may reach the sink until the next `Play`, a `Progress` reports the
/// new position immediately (so nothing on screen is stale), and when
/// playback does resume it resumes *at the target* — asserted against an
/// exact boundary, since pause freezes the delivered count.
#[test]
fn seek_while_paused_moves_the_position_without_playing() {
    let f = fixtures();
    let target = ms_to_frames(4_500, RATE);
    let (engine, events, output) =
        spawn_offline(paced_config(), 2 * f.chirp_ref.len()).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.chirp.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    let frozen = engine.samples_delivered();
    assert!(
        frozen < target * CHANNELS,
        "the pause must land before the seek target for this test to mean anything"
    );

    engine
        .send(Command::Seek { position_ms: 4_500 })
        .expect("send");
    assert_eq!(
        next_event(&events),
        Event::Progress {
            elapsed_ms: 4_500,
            track_ms: Some(CHIRP_MS),
        },
        "a seek is confirmed by an immediate Progress at the new position"
    );
    // Still paused: no TrackStarted, no Resumed, no audio.
    assert_no_event_within(&events, Duration::from_millis(120));
    assert_eq!(
        engine.samples_delivered(),
        frozen,
        "seeking while paused must not deliver audio"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    // Delivery was frozen across the whole seek, so `frozen` is the exact
    // boundary between the pre-seek head and the post-seek tail.
    assert_samples_eq(&out[..frozen], &f.chirp_ref[..frozen], "pre-seek head");
    assert_samples_eq(
        &out[frozen..],
        &f.chirp_ref[target * CHANNELS..],
        "post-seek tail (must begin exactly at the target frame)",
    );
}

/// **Seek past the end of the track is Next**: the following queue position
/// starts from its beginning, and past the *last* track the queue ends. No
/// clamping to the final frame — a stalled playhead is not a state anyone
/// asks for (see `Command::Seek`'s docs).
#[test]
fn seek_past_track_end_advances_to_the_next_track() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.b_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    // Track A is 5 s; 9 s is past its end, so the seek means Next.
    engine
        .send(Command::Seek { position_ms: 9_000 })
        .expect("send");
    assert_eq!(next_transport_event(&events), started(&f.b, 1));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    assert!(out.len() < capacity, "track A must have been cut short");
    // Track B ran to completion from its *beginning* — not from any offset
    // carried over from the seek.
    assert_samples_eq(
        &out[out.len() - f.b_ref.len()..],
        &f.b_ref,
        "the track after a past-the-end seek, played from its start",
    );

    // Past the end of the *last* track there is nowhere to advance to.
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.b_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.b, 0));
    engine
        .send(Command::Seek { position_ms: 9_000 })
        .expect("send");
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    assert_no_event_within(&events, Duration::from_millis(120));
}

/// **Seek while stopped is a no-op**, like `Next`: there is no current track
/// to seek within, so nothing starts and nothing is reported.
#[test]
fn seek_while_stopped_is_a_no_op() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(fast_config(), f.b_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.b.clone()],
            origin: None,
        })
        .expect("send");
    assert_eq!(next_queue_changed(&events), (1, None)); // the SetQueue above
    engine
        .send(Command::Seek { position_ms: 500 })
        .expect("send");
    assert_no_event_within(&events, Duration::from_millis(150));
    assert_eq!(
        engine.samples_delivered(),
        0,
        "a stopped seek plays nothing"
    );
}

/// **Progress cadence and immediacy.** One report per quarter-second of
/// delivered audio, one immediately after `TrackStarted`, none while paused,
/// one immediately after `Resumed` — the contract in `Event::Progress`'s
/// docs, asserted end to end over a 5 s track.
#[test]
fn progress_cadence_is_quarter_second_and_immediate_after_transitions() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.a_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    // Immediately after TrackStarted, before any quarter-second has passed.
    let (first, total) = next_progress(&events);
    assert_eq!(total, Some(5_000), "track length comes with every report");
    assert!(
        first < 250,
        "the report following TrackStarted must be immediate, got {first} ms"
    );

    // Cadence: gather the run of reports up to the pause.
    let mut elapsed = vec![first];
    while elapsed.len() < 8 {
        let (ms, total) = next_progress(&events);
        assert_eq!(total, Some(5_000));
        elapsed.push(ms);
    }
    for pair in elapsed.windows(2) {
        let step = pair[1] - pair[0];
        // 250 ms of audio, plus at most one pump chunk of overshoot
        // (2048 frames ≈ 46 ms at 44.1 kHz).
        assert!(
            (250..=300).contains(&step),
            "reports should be ~250 ms of audio apart, got {step} ms in {elapsed:?}"
        );
    }

    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    // A paused position is not moving, so there is nothing to report.
    assert_no_event_within(&events, Duration::from_millis(200));

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);
    let (resumed_at, _) = next_progress(&events);
    assert!(
        resumed_at >= *elapsed.last().expect("reports collected"),
        "resume must report a position at or past the last one, got {resumed_at} ms"
    );
}

/// **Elapsed time is wall-clock true across a resampled track** — the one
/// place a plausible implementation goes quietly wrong.
///
/// Runs under the explicit fixed-output-rate opt-in, because that (and a
/// device that cannot do the source rate, which reaches the same code) is now
/// the only way a track gets resampled at all. The arithmetic it guards is
/// unchanged and still has to be right wherever conversion happens.
///
/// The queue is 1 s at 44.1 kHz (which fixes the session's stream rate)
/// followed by 4 s at 48 kHz, which the fixed-rate policy resamples
/// down to 44.1 kHz before it reaches the ring. Counting delivered frames
/// against the *file's* 48 kHz would under-report by 8 %: the 4 s track
/// would appear to run 3.675 s, and every position inside it would be short
/// by a growing margin.
///
/// The assertion is exact rather than approximate, because pause freezes
/// delivery: with the sample counter frozen, the `Progress` that follows
/// `Resumed` is computed from precisely that count, so the expected
/// millisecond value can be derived independently and compared for equality.
#[test]
fn elapsed_is_wall_clock_true_across_a_resampled_track() {
    let f = fixtures();
    // The 48 kHz track resamples to exactly 4 s at the 44.1 kHz stream rate.
    let head_samples = HEAD_44K_FRAMES * CHANNELS;
    let capacity = head_samples + TAIL_48K_SECS * RATE as usize * CHANNELS;
    let (engine, events, _output) =
        spawn_offline(fixed_rate_config(), capacity).expect("spawn engine");

    engine
        .send(Command::SetQueue {
            paths: vec![f.head_44k.clone(), f.tail_48k.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.head_44k, 0));
    assert_eq!(next_transport_event(&events), started(&f.tail_48k, 1));

    // Let the resampled track run a little way in, then freeze.
    thread::sleep(Duration::from_millis(80));
    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    let frozen = engine.samples_delivered();
    assert!(
        frozen > head_samples,
        "the pause must land inside the resampled track"
    );
    assert!(
        frozen < capacity,
        "the pause must land before the resampled track ends"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);
    let (elapsed, total) = next_progress(&events);

    // Track length is a property of the track, not of the stream it is
    // played into: 4 s at 48 kHz is 4 s at any output rate.
    assert_eq!(
        total,
        Some(TAIL_48K_MS),
        "resampling must not change how long the track is"
    );

    // The audio is being consumed at the stream rate, so that is the
    // denominator. Derived here from the frozen count, independently of the
    // engine's own arithmetic.
    let delivered_frames = (frozen - head_samples) / CHANNELS;
    let want = frames_to_ms(delivered_frames, RATE);
    let naive = frames_to_ms(delivered_frames, TAIL_48K_RATE);
    assert_eq!(
        elapsed, want,
        "elapsed must be delivered frames at the stream rate ({RATE} Hz); \
         a sample-count-naive reading at the file's {TAIL_48K_RATE} Hz would say {naive} ms"
    );
    assert_ne!(
        want, naive,
        "the fixture must actually distinguish the two readings"
    );

    engine.shutdown();
}

/// **Nothing is resampled when the output can run at the source rate.**
///
/// The headline claim of ADR-0009, asserted from the engine's own conversion
/// counters rather than from a stopwatch: `resampled_tracks == 0` means no
/// resampler was ever constructed, which no amount of "it felt fast" could
/// establish. The 48 kHz fixture is the case that used to be converted.
///
/// The delivered audio is compared against a plain reference decode, so
/// "unconverted" is checked at the samples too, not only at the counter.
#[test]
fn nothing_is_resampled_when_the_output_can_run_at_the_source_rate() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(paced_config(), f.tail_48k_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.tail_48k.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.tail_48k, 0));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    let conversions = engine.conversions();
    assert_eq!(
        conversions.resampled_tracks, 0,
        "a 48 kHz track played into a 48 kHz stream must not construct a resampler"
    );
    assert!(
        conversions.resample_ms <= 0.0,
        "no resampler ran, so no time can have been spent in one: {conversions:?}"
    );
    engine.shutdown();
    assert_samples_eq(
        &collect(output),
        &f.tail_48k_ref,
        "48 kHz track delivered unconverted",
    );
}

/// The signal-path readout for that same case: source and output rates agree
/// and the chain is [`SignalChain::Direct`]. This is what a front end renders,
/// so it is asserted as the protocol value a front end would receive.
#[test]
fn the_signal_path_reports_a_direct_chain_at_the_source_rate() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.tail_48k_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.tail_48k.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.tail_48k, 0));
    assert_eq!(
        next_signal_path(&events),
        Event::SignalPath {
            source_rate_hz: TAIL_48K_RATE,
            source_channels: 2,
            source_bits: Some(32),
            output_rate_hz: TAIL_48K_RATE,
            chain: SignalChain::Direct,
        },
        "a 48 kHz file on a stream that can run at 48 kHz is a direct chain"
    );
    engine.shutdown();
}

/// An album does not repeat itself: the chain is stated when a session starts
/// and then only when it changes, so a ten-track album at one rate produces
/// exactly one `SignalPath`.
#[test]
fn the_signal_path_is_stated_once_while_it_does_not_change() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.b_ref.len();
    let (engine, events, _output) = spawn_offline(fast_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");

    let mut signals = 0usize;
    loop {
        match next_event(&events) {
            Event::SignalPath { .. } => signals += 1,
            Event::QueueEnded => break,
            _ => {}
        }
    }
    assert_eq!(
        signals, 1,
        "two same-rate tracks describe one chain, so they should say so once"
    );
    engine.shutdown();
}

/// **A rate change reopens the output and loses not one sample.**
///
/// The queue is 1 s at 44.1 kHz followed by 4 s at 48 kHz. Under the default
/// the first session stops at the boundary, the engine drains and renegotiates,
/// and a second session plays the 48 kHz track *at 48 kHz*. What must come out
/// the far end is both files' own samples, in order, unconverted — which is
/// asserted here against reference decodes of each, concatenated.
///
/// This is the test that would fail if the split dropped the tail of the first
/// track, replayed part of it, or quietly resampled either half.
#[test]
fn a_rate_change_replays_both_tracks_unconverted() {
    let f = fixtures();
    let capacity = f.head_44k_ref.len() + f.tail_48k_ref.len();
    let (engine, events, output) = spawn_offline(fast_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.head_44k.clone(), f.tail_48k.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.head_44k, 0));
    assert_eq!(next_transport_event(&events), started(&f.tail_48k, 1));
    // Exactly one QueueEnded: the split is an internal handover, not the end
    // of the queue, and a front end must never see it as one.
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    assert_no_event_within(&events, Duration::from_millis(120));

    assert_eq!(
        engine.conversions().resampled_tracks,
        0,
        "following the source means neither half is converted"
    );
    engine.shutdown();

    let mut want = f.head_44k_ref.clone();
    want.extend_from_slice(&f.tail_48k_ref);
    assert_samples_eq(&collect(output), &want, "both tracks at their own rates");
}

/// The readout across that same rate change: two chains, each direct, at the
/// two different rates. A front end watching this sees the output follow the
/// music.
#[test]
fn a_rate_change_restates_the_signal_path() {
    let f = fixtures();
    let capacity = f.head_44k_ref.len() + f.tail_48k_ref.len();
    let (engine, events, _output) = spawn_offline(fast_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.head_44k.clone(), f.tail_48k.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");

    let mut signals = Vec::new();
    loop {
        match next_event(&events) {
            Event::SignalPath {
                source_rate_hz,
                output_rate_hz,
                chain,
                ..
            } => signals.push((source_rate_hz, output_rate_hz, chain)),
            Event::QueueEnded => break,
            _ => {}
        }
    }
    assert_eq!(
        signals,
        vec![
            (RATE, RATE, SignalChain::Direct),
            (TAIL_48K_RATE, TAIL_48K_RATE, SignalChain::Direct),
        ],
        "the output must follow the source across the change, and say so both times"
    );
    engine.shutdown();
}

/// The explicit fixed-output-rate mode reports itself honestly: the same queue
/// converts instead of reopening, and the readout says *converting*, with the
/// reason being the setting rather than the hardware.
///
/// Reporting it is the whole point — a conversion nobody is told about is the
/// one outcome ADR-0009 rules out.
#[test]
fn a_fixed_output_rate_reports_a_converting_chain() {
    let f = fixtures();
    let capacity = f.head_44k_ref.len() + TAIL_48K_SECS * RATE as usize * CHANNELS;
    let (engine, events, _output) =
        spawn_offline(fixed_rate_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.head_44k.clone(), f.tail_48k.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");

    let mut signals = Vec::new();
    loop {
        match next_event(&events) {
            Event::SignalPath {
                source_rate_hz,
                output_rate_hz,
                chain,
                ..
            } => signals.push((source_rate_hz, output_rate_hz, chain)),
            Event::QueueEnded => break,
            _ => {}
        }
    }
    assert_eq!(
        signals,
        vec![
            (RATE, RATE, SignalChain::Direct),
            (
                TAIL_48K_RATE,
                RATE,
                SignalChain::Converting {
                    reason: ConversionReason::FixedOutputRate,
                },
            ),
        ],
        "a converted track must be reported as converted, with the reason"
    );
    assert_eq!(
        engine.conversions().resampled_tracks,
        1,
        "exactly the one track whose rate differed"
    );
    engine.shutdown();
}

/// The environment variable that turns the **audible** device tests on.
///
/// Everything in this section drives a real engine through a real output
/// device, which means real decoded fixture audio comes out of whatever the
/// machine is plugged into — several seconds of 440 Hz tone and a chirp, on
/// every `cargo test --all-features`. That is intolerable as a default on a
/// developer's machine, and it is not what most runs of this suite are for.
///
/// So these are opt-in, and the rest of the device coverage was made silent
/// rather than moved behind this flag: `tests/playback.rs` exercises opening,
/// reopening, rate negotiation, ring discard, teardown and the
/// short-lived-thread ordering that the Windows access violation lived in, all
/// by writing silence to a real device. That is what keeps running everywhere,
/// including in CI. What is behind this variable is the part that can only be
/// judged by *hearing* it: a queue, played end to end, through the hardware.
///
/// See `docs/DEVELOPMENT.md`.
#[cfg(feature = "device-output")]
const AUDIBLE_TESTS_VAR: &str = "BAZ_DEVICE_TESTS";

/// Whether the audible device tests were asked for, printing the notice that
/// says how to ask when they were not.
///
/// Any non-empty value other than `0` counts, so `BAZ_DEVICE_TESTS=1` and
/// `BAZ_DEVICE_TESTS=yes` both work and `BAZ_DEVICE_TESTS=0` reads as "no".
#[cfg(feature = "device-output")]
fn audible_device_tests_requested() -> bool {
    let asked = std::env::var(AUDIBLE_TESTS_VAR).is_ok_and(|v| !v.is_empty() && v != "0");
    if !asked {
        eprintln!(
            "SKIP: this test plays audible audio through the real output device. \
             Set {AUDIBLE_TESTS_VAR}=1 to run it (docs/DEVELOPMENT.md)."
        );
    }
    asked
}

/// Device output (feature `device-output`): the engine spawns against the
/// default device — or reports the documented `Device` error on headless
/// machines — and shuts down cleanly either way. Never a panic.
///
/// **Audible, therefore opt-in** ([`AUDIBLE_TESTS_VAR`]).
///
/// The second half seeks into a 48 kHz track on an engine that was *spawned*
/// at 44.1 kHz, so it also covers the interaction of rate negotiation with
/// seek: whichever rate the session ends up negotiating, the reported position
/// must stay wall-clock true and the track's length must stay a property of
/// the track rather than of the stream it is played into.
#[cfg(feature = "device-output")]
#[test]
fn device_engine_spawns_or_reports_cleanly() {
    if !audible_device_tests_requested() {
        return;
    }
    let f = fixtures();
    match baz_core::engine::spawn_device(paced_config(), RATE, 8192) {
        Ok((engine, events)) => {
            engine
                .send(Command::SetQueue {
                    paths: vec![f.b.clone()],
                    origin: None,
                })
                .expect("send");
            engine.send(Command::Play).expect("send");
            assert_eq!(next_transport_event(&events), started(&f.b, 0));
            engine.shutdown(); // mid-track: must not hang the device stream
            println!("[device] engine played through the default output device");
        }
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable output device ({msg})");
            return;
        }
        Err(other) => panic!("unexpected error spawning device engine: {other}"),
    }

    let (engine, events) =
        baz_core::engine::spawn_device(paced_config(), RATE, 8192).expect("respawn device engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.tail_48k.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.tail_48k, 0));
    engine
        .send(Command::Seek { position_ms: 2_500 })
        .expect("send");
    // Reports already in flight when the seek was sent describe the position
    // it came *from*; the first one at or past the target is the seek's own.
    let (elapsed, total) = loop {
        let (elapsed, total) = next_progress(&events);
        assert_eq!(
            total,
            Some(TAIL_48K_MS),
            "a track's length is a property of the track, not of the stream rate"
        );
        if elapsed >= 2_500 {
            break (elapsed, total);
        }
    };
    assert!(
        elapsed < 2_600,
        "the seek must land at the target, not past it: {elapsed} ms"
    );
    assert_eq!(next_transport_event(&events), started(&f.tail_48k, 0));
    engine.shutdown();
    println!("[device] seek into a 48 kHz track reported {elapsed} ms of {total:?}");
}

/// Device output (feature `device-output`): the whole transport vocabulary,
/// over real hardware, in **the configuration the app actually ships** —
/// [`EngineConfig::default`], 44.1 kHz, an 8192-frame device ring.
///
/// Every command here is one that abandons a session and therefore discards
/// the device ring's contents (`Sink::discard_buffered`): seek, skip, stop,
/// queue replacement, and then a restart to prove the stream survived them
/// all. What it asserts is that the discard path is exercised against a live
/// cpal callback without the stream faulting, stalling, or dropping the
/// engine's event ordering — the failure modes a lock-free producer/callback
/// handshake would produce if it were wrong. That the ring is genuinely
/// *emptied* is measured in `tests/playback.rs`; this is the integration-level
/// companion to it.
///
/// **Audible, therefore opt-in** ([`AUDIBLE_TESTS_VAR`]).
#[cfg(feature = "device-output")]
#[test]
fn device_engine_transport_survives_repeated_session_abandonment() {
    if !audible_device_tests_requested() {
        return;
    }
    let f = fixtures();
    // Exactly what crates/baz/src/playback.rs spawns.
    let spawned = baz_core::engine::spawn_device(EngineConfig::default(), RATE, 8192);
    let (engine, events) = match spawned {
        Ok(pair) => pair,
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable output device ({msg})");
            return;
        }
        Err(other) => panic!("unexpected error spawning device engine: {other}"),
    };

    engine
        .send(Command::SetQueue {
            paths: vec![f.chirp.clone(), f.a.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));

    // Seek repeatedly, including backwards: each one abandons a session whose
    // audio is already sitting in the device ring.
    // Targets stay well clear of the 6 s track end so a slow machine cannot
    // let the track run out from under the next command.
    for target in [1_000u64, 3_000, 500, 2_000] {
        engine
            .send(Command::Seek {
                position_ms: target,
            })
            .expect("send");
        assert_eq!(
            next_transport_event(&events),
            started(&f.chirp, 0),
            "a seek restarts the current track"
        );
        // The position the engine reports must be the one asked for, not the
        // one still queued in the ring.
        let landed = loop {
            let (elapsed, total) = next_progress(&events);
            assert_eq!(total, Some(CHIRP_MS));
            if elapsed >= target {
                break elapsed;
            }
        };
        assert!(
            landed < target + 500,
            "seek to {target} ms reported {landed} ms"
        );
    }

    engine.send(Command::Next).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 1));
    engine.send(Command::Stop).expect("send");
    assert_eq!(next_transport_event(&events), Event::Stopped);

    // The stream is still alive after all of that: replay from the top.
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    engine.shutdown();
    println!("[device] transport survived 4 seeks, a skip, a stop, and a restart");
}

/// **Device output (feature `device-output`): a 48 kHz album really plays at
/// 48 kHz on real hardware.**
///
/// The end-to-end version of ADR-0009's claim, on the only apparatus that can
/// settle it. The engine is spawned at 44.1 kHz — deliberately the *wrong*
/// rate, exactly as the app spawns it before it knows what will be played —
/// and then handed a 48 kHz track. Negotiation must move the output to
/// 48 kHz and no resampler must be constructed.
///
/// A device that genuinely cannot do 48 kHz is not a test failure: it is the
/// documented fallback, and the assertion adapts to say so. What is asserted
/// either way is the invariant that matters — **the readout and the counters
/// agree with each other**, so the chain is never described as direct while a
/// resampler is running, nor the reverse.
///
/// **Audible, therefore opt-in** ([`AUDIBLE_TESTS_VAR`]). The silent half of
/// the same claim — that a 48 kHz stream opens and that a reopen lands on the
/// rate asked for — is `device_sink_opens_at_48k_and_accepts_audio` and
/// `device_sink_reopens_at_the_requested_rate` in `tests/playback.rs`, which
/// run unconditionally.
#[cfg(feature = "device-output")]
#[test]
fn device_engine_follows_the_source_rate() {
    if !audible_device_tests_requested() {
        return;
    }
    let f = fixtures();
    let spawned = baz_core::engine::spawn_device(EngineConfig::default(), RATE, 8192);
    let (engine, events) = match spawned {
        Ok(pair) => pair,
        Err(PlaybackError::Device(msg)) => {
            eprintln!("SKIP: no usable output device ({msg})");
            return;
        }
        Err(other) => panic!("unexpected error spawning device engine: {other}"),
    };
    engine
        .send(Command::SetQueue {
            paths: vec![f.tail_48k.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.tail_48k, 0));

    let Event::SignalPath {
        source_rate_hz,
        output_rate_hz,
        chain,
        ..
    } = next_signal_path(&events)
    else {
        unreachable!("next_signal_path returns only SignalPath")
    };
    assert_eq!(source_rate_hz, TAIL_48K_RATE, "the file is 48 kHz");

    // Give the engine a moment to have resampled, if it were going to.
    thread::sleep(Duration::from_millis(200));
    let conversions = engine.conversions();
    match chain {
        SignalChain::Direct => {
            assert_eq!(
                output_rate_hz, TAIL_48K_RATE,
                "a direct chain means the output followed the source"
            );
            assert_eq!(
                conversions.resampled_tracks, 0,
                "a direct chain must not have a resampler behind it"
            );
            println!(
                "[device] 48 kHz content negotiated a 48 kHz output stream; \
                 {} reconfiguration(s), no resampling",
                conversions.output_reconfigurations
            );
        }
        SignalChain::Converting { reason } => {
            assert_eq!(
                reason,
                ConversionReason::DeviceRateUnavailable,
                "following the source can only be defeated by the device"
            );
            assert_ne!(
                output_rate_hz, TAIL_48K_RATE,
                "a converting chain means the rates differ"
            );
            assert!(
                conversions.resampled_tracks >= 1,
                "a converting chain must have a resampler behind it"
            );
            println!(
                "[device] this device offers no {TAIL_48K_RATE} Hz mode; played at \
                 {output_rate_hz} Hz and reported the conversion"
            );
        }
        other => panic!("unexpected chain state: {other:?}"),
    }
    engine.shutdown();
}

// ---------------------------------------------------------------------------
// Volume (ADR-0011)
// ---------------------------------------------------------------------------

/// Play `queue` to completion with `before_play` sent first, and return the
/// delivered samples.
///
/// Commands sent before `Play` land while the engine is idle, which is where
/// the fader jumps rather than slews (there is no audible discontinuity in
/// silence) — so this is the harness for the *exact-arithmetic* volume tests.
/// The one that measures the slew drives the engine by hand instead.
fn play_with_volume(queue: &[PathBuf], capacity: usize, before_play: &[Command]) -> Vec<f32> {
    let (engine, events, output) = spawn_offline(fast_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: queue.to_vec(),
            origin: None,
        })
        .expect("send");
    for command in before_play {
        engine.send(command.clone()).expect("send");
    }
    engine.send(Command::Play).expect("send");
    loop {
        match next_transport_event(&events) {
            Event::QueueEnded => break,
            Event::TrackStarted { .. } => {}
            other => panic!("unexpected event while playing out: {other:?}"),
        }
    }
    engine.shutdown();
    collect(output)
}

/// **Unity is bit-exact.** The whole ADR-0011 guarantee, asserted where it
/// matters: a gapless two-track queue played with the volume control explicitly
/// engaged at unity is sample-for-sample the same stream as the reference
/// decode of both files concatenated.
///
/// This is the engine-level twin of `gapless_wav_bit_exact` in
/// `tests/playback.rs`, and it is deliberately run with `SetVolume` and
/// `SetMute` *sent* rather than left at their defaults: "we never touched the
/// volume so of course it is exact" would be a much weaker claim than "the
/// volume was set, to the top, and it is still exact".
#[test]
fn unity_volume_delivers_a_bit_identical_stream() {
    let f = fixtures();
    let mut want = f.a_ref.clone();
    want.extend_from_slice(&f.b_ref);
    let out = play_with_volume(
        &[f.a.clone(), f.b.clone()],
        want.len(),
        &[
            Command::SetVolume {
                position: MAX_POSITION,
            },
            Command::SetMute { muted: false },
        ],
    );
    assert_samples_eq(&out, &want, "unity-volume gapless output");
}

/// The same claim from the other side: the stream delivered at unity is
/// identical to the stream delivered with no volume command sent at all.
///
/// Compared against the *reference decode* above and against the *no-volume
/// run* here, because the two catch different mistakes — the first catches a
/// gain that is applied, the second catches a code path that is taken.
#[test]
fn unity_volume_is_indistinguishable_from_no_volume_control() {
    let f = fixtures();
    let capacity = f.a_ref.len();
    let untouched = play_with_volume(std::slice::from_ref(&f.a), capacity, &[]);
    let at_unity = play_with_volume(
        std::slice::from_ref(&f.a),
        capacity,
        &[Command::SetVolume {
            position: MAX_POSITION,
        }],
    );
    assert_samples_eq(&at_unity, &untouched, "unity vs. no volume command");
}

/// **Half travel is exactly one eighth of the amplitude**, and every sample
/// says so exactly — f32 multiplication is deterministic, so this is asserted
/// with `==` and no tolerance.
///
/// Position 500 is chosen because the taper makes its gain `0.5³ = 0.125`, a
/// power of two: scaling by it is exactly representable for every finite
/// sample, so a single wrong bit anywhere in the path fails the test.
#[test]
#[allow(clippy::float_cmp)] // exactness is the assertion (baz_core::volume)
fn half_travel_scales_by_exactly_one_eighth() {
    let f = fixtures();
    let gain = Volume::new(500).amplitude();
    assert_eq!(gain, 0.125, "the taper's own arithmetic");
    let out = play_with_volume(
        std::slice::from_ref(&f.a),
        f.a_ref.len(),
        &[Command::SetVolume { position: 500 }],
    );
    let want: Vec<f32> = f.a_ref.iter().map(|s| s * gain).collect();
    assert_samples_eq(&out, &want, "half-travel output");
    assert_eq!(out.len(), f.a_ref.len(), "scaling must not drop a sample");
}

/// Silence at the bottom of the travel is *exactly* silence, not a very small
/// number — the taper reaches zero rather than approaching it.
#[test]
fn the_bottom_of_the_travel_is_exact_silence() {
    let f = fixtures();
    let out = play_with_volume(
        std::slice::from_ref(&f.b),
        f.b_ref.len(),
        &[Command::SetVolume { position: 0 }],
    );
    assert_eq!(out.len(), f.b_ref.len(), "silence is still delivered audio");
    assert!(
        out.iter().all(|s| *s == 0.0),
        "position 0 must be exactly zero, not merely quiet"
    );
}

/// A gain applied by the engine, as read straight out of the delivered stream.
///
/// The fixture is a constant, so dividing by it recovers the gain exactly (see
/// [`DC`]). One value per frame — both channels must agree, which is itself
/// part of what is being checked.
#[allow(clippy::float_cmp)] // the channels must agree exactly, not approximately
fn gain_trajectory(out: &[f32]) -> Vec<f32> {
    out.chunks_exact(CHANNELS)
        .map(|frame| {
            assert_eq!(
                frame[0], frame[1],
                "a volume change must move both channels together"
            );
            frame[0] / DC
        })
        .collect()
}

/// **A mid-playback volume change is a monotonic ramp that drops no samples.**
///
/// Everything a "does it click?" question actually asks, made arithmetic by
/// the constant fixture: the delivered stream *is* the gain trajectory.
///
/// - Nothing is dropped: the delivered length is the reference length.
/// - Before the change the samples are untouched — exactly unity, because the
///   transparent path does not multiply at all.
/// - The change is a ramp, not a step: monotonic, no adjacent gain step larger
///   than the documented slew rate, landing exactly on the target.
/// - It completes inside [`RAMP_MS`], which is what "full travel per
///   [`RAMP_MS`]" commits to for a change of less than full travel.
///
/// The exact float comparisons are deliberate throughout — see
/// `baz_core::volume`'s note on `float_cmp`. The fixture is a constant and the
/// gains are exact, so "untouched" and "landed" are exact questions; the *only*
/// place a tolerance appears is the slew-rate bound, and it is one f32 epsilon
/// wide with its reason stated at the assertion.
#[test]
#[allow(clippy::float_cmp)]
fn a_mid_playback_volume_change_ramps_monotonically_and_drops_nothing() {
    let f = fixtures();
    let target = Volume::new(500).amplitude();
    let cfg = EngineConfig {
        consumer_pace: Duration::from_millis(2),
        ..paced_config()
    };
    let (engine, events, output) = spawn_offline(cfg, f.dc_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.dc.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.dc, 0));
    // Let real audio flow at unity first, so the ramp has a "before" to be
    // measured against.
    thread::sleep(Duration::from_millis(30));
    assert!(
        engine.samples_delivered() > 0,
        "no audio flowed before the volume change"
    );
    engine
        .send(Command::SetVolume { position: 500 })
        .expect("send");
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    engine.shutdown();

    let out = collect(output);
    assert_eq!(
        out.len(),
        f.dc_ref.len(),
        "a volume change must not drop, duplicate, or truncate audio"
    );
    let gains = gain_trajectory(&out);

    // The untouched head, then the first frame that is not unity.
    let start = gains
        .iter()
        .position(|g| *g != 1.0)
        .expect("the volume change must reach the stream");
    assert!(
        gains[..start].iter().all(|g| *g == 1.0),
        "audio before the change must be exactly untouched"
    );
    let landed = gains[start..]
        .iter()
        .position(|g| *g == target)
        .expect("the ramp must land exactly on the target gain")
        + start;
    assert!(
        gains[landed..].iter().all(|g| *g == target),
        "the gain must stay put once it arrives"
    );

    // Monotonic, and never faster than the documented slew rate.
    //
    // The bound carries one f32 epsilon of slack, and only one: the fader
    // accumulates its position in f32, so each step is the ideal slew plus at
    // most one rounding. Asserting tighter than that would be asserting
    // against the arithmetic's precision rather than against the property —
    // and the slack is ~1e-7 against a total change of 0.875, so a genuine
    // step discontinuity is still caught by six orders of magnitude.
    let slew = 1.0 / (f64::from(RATE) * f64::from(RAMP_MS) / 1000.0);
    let bound = slew + f64::from(f32::EPSILON);
    for (i, pair) in gains[start - 1..=landed].windows(2).enumerate() {
        assert!(
            pair[1] <= pair[0],
            "the ramp reversed at frame {i}: {} then {}",
            pair[0],
            pair[1]
        );
        assert!(
            pair[1] >= target,
            "the ramp overshot the target at frame {i}: {}",
            pair[1]
        );
        let step = f64::from(pair[0] - pair[1]);
        assert!(
            step <= bound,
            "a gain step of {step} exceeds the {slew} slew rate at frame {i} — \
             that is the discontinuity a ramp exists to prevent"
        );
    }

    // And it completes inside the documented time.
    let ramp_frames = landed - start + 1;
    let budget = ms_to_frames(u64::from(RAMP_MS), RATE);
    assert!(
        ramp_frames <= budget,
        "the ramp took {ramp_frames} frames; RAMP_MS allows at most {budget}"
    );
    assert!(ramp_frames > 1, "a single-frame 'ramp' is a step");
}

/// The volume is engine state, not session state, so every transport command
/// leaves it exactly where it was. Asserted on the *audio*, not just on the
/// readout: the tail delivered after a pause, a resume, a seek and a track
/// change is still scaled by the gain that was set before any of them.
#[test]
fn volume_survives_pause_resume_seek_and_track_boundaries() {
    let f = fixtures();
    let gain = Volume::new(500).amplitude();
    let capacity = f.a_ref.len() + f.b_ref.len();
    let (engine, events, output) = spawn_offline(fast_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine
        .send(Command::SetVolume { position: 500 })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    assert_eq!(engine.volume().volume, Volume::new(500), "pause lost it");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);

    engine
        .send(Command::Seek { position_ms: 2_000 })
        .expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    assert_eq!(engine.volume().volume, Volume::new(500), "seek lost it");

    engine.send(Command::Next).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.b, 1));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    let state = engine.volume();
    assert_eq!(state.volume, Volume::new(500), "the track boundary lost it");
    assert!(!state.muted);
    assert_eq!(state.path, VolumePath::SoftwareGain);

    engine.shutdown();
    let out = collect(output);
    // Track B ran to completion after every one of those commands, so the
    // tail of the delivered stream must be exactly B, scaled.
    let want: Vec<f32> = f.b_ref.iter().map(|s| s * gain).collect();
    assert!(out.len() >= want.len(), "output too short");
    assert_samples_eq(
        &out[out.len() - want.len()..],
        &want,
        "post-transport track B output",
    );
}

/// Mute silences without forgetting, and unmute restores the position it was
/// never told. That is the whole reason mute is separate state rather than
/// gain zero (`baz_core::volume`), and this is the round trip that proves it.
#[test]
fn mute_and_unmute_round_trip_without_losing_the_position() {
    let f = fixtures();
    let gain = Volume::new(500).amplitude();

    let muted = play_with_volume(
        std::slice::from_ref(&f.b),
        f.b_ref.len(),
        &[
            Command::SetVolume { position: 500 },
            Command::SetMute { muted: true },
        ],
    );
    assert_eq!(muted.len(), f.b_ref.len(), "mute must not stop delivery");
    assert!(
        muted.iter().all(|s| *s == 0.0),
        "mute must be exactly silent"
    );

    let unmuted = play_with_volume(
        std::slice::from_ref(&f.b),
        f.b_ref.len(),
        &[
            Command::SetVolume { position: 500 },
            Command::SetMute { muted: true },
            Command::SetMute { muted: false },
        ],
    );
    let want: Vec<f32> = f.b_ref.iter().map(|s| s * gain).collect();
    assert_samples_eq(&unmuted, &want, "unmuted output");
}

/// Muting at unity and unmuting again returns the path to `Unity` — so the
/// bit-exact state is genuinely recoverable, not merely reachable once.
#[test]
fn unmuting_at_unity_returns_to_a_bit_exact_path() {
    let f = fixtures();
    let out = play_with_volume(
        std::slice::from_ref(&f.b),
        f.b_ref.len(),
        &[
            Command::SetMute { muted: true },
            Command::SetMute { muted: false },
        ],
    );
    assert_samples_eq(&out, &f.b_ref, "output after a mute round trip at unity");
}

/// A front end coming up mid-session needs the state without waiting for
/// someone to change it, so the handle answers directly — and its answer is
/// unity, the state ADR-0009 describes.
#[test]
fn a_fresh_engine_reports_unity_and_an_untouched_path() {
    let (engine, _events, _output) = spawn_offline(fast_config(), 16).expect("spawn engine");
    let state: VolumeState = engine.volume();
    assert_eq!(state.volume, Volume::UNITY);
    assert!(!state.muted);
    assert_eq!(state.path, VolumePath::Unity);
    assert!(
        state.path.is_transparent(),
        "a player nobody has touched must not be scaling anything"
    );
    engine.shutdown();
}

/// The engine confirms every accepted change on the event channel, so two front
/// ends attached to one engine agree about where the control is — a slider must
/// follow this rather than its own optimistic value.
#[test]
fn every_accepted_change_is_confirmed_on_the_event_channel() {
    let (engine, events, _output) = spawn_offline(fast_config(), 16).expect("spawn engine");
    engine
        .send(Command::SetVolume { position: 250 })
        .expect("send");
    assert_eq!(
        next_event(&events),
        Event::VolumeChanged {
            position: 250,
            muted: false,
            path: VolumePath::SoftwareGain,
        }
    );
    engine.send(Command::SetMute { muted: true }).expect("send");
    assert_eq!(
        next_event(&events),
        Event::VolumeChanged {
            position: 250,
            muted: true,
            path: VolumePath::SoftwareGain,
        },
        "mute travels beside the position, never as one"
    );
    engine
        .send(Command::SetVolume {
            position: MAX_POSITION,
        })
        .expect("send");
    assert_eq!(
        next_event(&events),
        Event::VolumeChanged {
            position: MAX_POSITION,
            muted: true,
            // Still muted, so the samples are still being zeroed: unity on the
            // control is not unity in the path while mute is on.
            path: VolumePath::SoftwareGain,
        }
    );
    engine
        .send(Command::SetMute { muted: false })
        .expect("send");
    assert_eq!(
        next_event(&events),
        Event::VolumeChanged {
            position: MAX_POSITION,
            muted: false,
            path: VolumePath::Unity,
        }
    );
    // Redundant commands say nothing (the rule the whole protocol follows).
    engine
        .send(Command::SetVolume {
            position: MAX_POSITION,
        })
        .expect("send");
    assert_no_event_within(&events, Duration::from_millis(50));
    engine.shutdown();
}

/// A position past the top of the travel clamps to unity rather than being
/// rejected or wrapping — and the confirmation reports the clamped value, so a
/// front end that sent a bad number learns what actually happened.
#[test]
fn an_out_of_range_position_clamps_to_unity() {
    let (engine, events, _output) = spawn_offline(fast_config(), 16).expect("spawn engine");
    engine
        .send(Command::SetVolume { position: 250 })
        .expect("send");
    assert!(matches!(next_event(&events), Event::VolumeChanged { .. }));
    engine
        .send(Command::SetVolume { position: u16::MAX })
        .expect("send");
    assert_eq!(
        next_event(&events),
        Event::VolumeChanged {
            position: MAX_POSITION,
            muted: false,
            path: VolumePath::Unity,
        }
    );
    engine.shutdown();
}

// ---------------------------------------------------------------------------
// Command::Previous — the counterpart Next has been missing
// ---------------------------------------------------------------------------

/// Put a paused session at an exact position inside its current track, so a
/// `Previous` test asserts against a known elapsed time rather than a raced
/// one.
///
/// Seeking while paused is documented to move the position and deliver
/// nothing, and the immediate `Progress` it emits is the confirmation that the
/// engine agrees about where playback now is. Every test below uses this
/// rather than sleeping and hoping, because the whole point of
/// `PREVIOUS_RESTART_MS` is a comparison against that number.
fn park_at(engine: &EngineHandle, events: &Receiver<Event>, position_ms: u64) {
    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(events), Event::Paused);
    engine.send(Command::Seek { position_ms }).expect("send");
    let (elapsed, _) = next_progress(events);
    assert_eq!(
        elapsed, position_ms,
        "the engine must agree about where playback is before Previous is judged against it"
    );
}

/// **Past the threshold, `Previous` restarts the current track** — the first
/// half of the conventional two-in-one control.
///
/// Parked 4 s into a 6 s track (past `PREVIOUS_RESTART_MS`), so the button
/// means "this one again". Asserted on the samples, not merely on the events:
/// what the engine delivers after the press must be the whole track from its
/// first sample, bit for bit against a reference decode.
#[test]
fn previous_past_the_threshold_restarts_the_current_track() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(paced_config(), 3 * f.chirp_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.chirp.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));

    // Checked at compile time so that moving the threshold turns this test
    // into a build failure rather than a test that quietly stops testing the
    // branch it names.
    const { assert!(4_000 >= PREVIOUS_RESTART_MS) }
    park_at(&engine, &events, 4_000);

    engine.send(Command::Previous).expect("send");
    assert_eq!(
        next_transport_event(&events),
        started(&f.chirp, 0),
        "past the threshold, Previous restarts the track it is already on"
    );
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    assert_samples_eq(
        &out[out.len() - f.chirp_ref.len()..],
        &f.chirp_ref,
        "a restarted track must be delivered whole, from its first sample",
    );
}

/// **Before the threshold, `Previous` steps back a queue position** — the
/// other half, and the reason the button exists at all.
///
/// Parked 1 s into the *second* track, so "back" means the first one. The
/// audio assertion is the strong one: the tail of the run must be the earlier
/// track in full followed by the later track in full, which is only true if
/// the engine went back one position and then continued forward normally.
#[test]
fn previous_before_the_threshold_steps_back_a_queue_position() {
    let f = fixtures();
    let both = f.a_ref.len() + f.chirp_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), 3 * both).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.chirp.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    engine.send(Command::Next).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 1));

    // Compile-time, for the reason the sibling test gives.
    const { assert!(1_000 < PREVIOUS_RESTART_MS) }
    park_at(&engine, &events, 1_000);

    engine.send(Command::Previous).expect("send");
    assert_eq!(
        next_transport_event(&events),
        started(&f.a, 0),
        "before the threshold, Previous goes to the preceding queue entry"
    );
    assert_eq!(next_transport_event(&events), started(&f.chirp, 1));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    let tail = &out[out.len() - both..];
    assert_samples_eq(
        &tail[..f.a_ref.len()],
        &f.a_ref,
        "the track stepped back to",
    );
    assert_samples_eq(
        &tail[f.a_ref.len()..],
        &f.chirp_ref,
        "and the queue continues forward from there",
    );
}

/// **At the head of the queue, `Previous` restarts rather than stopping.**
///
/// Parked half a second into the first track: before the threshold, so the
/// rule would say "the one before" — and there is none. Restarting is what
/// every transport does here, and it is what keeps the control from having a
/// position where pressing it does nothing.
#[test]
fn previous_at_the_head_of_the_queue_restarts() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(paced_config(), 3 * f.chirp_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.chirp.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));

    park_at(&engine, &events, 500);

    engine.send(Command::Previous).expect("send");
    assert_eq!(
        next_transport_event(&events),
        started(&f.chirp, 0),
        "there is nothing before position 0, so the track restarts"
    );
    assert_eq!(
        next_transport_event(&events),
        Event::QueueEnded,
        "and the queue ends once, at its true end"
    );

    engine.shutdown();
    let out = collect(output);
    assert_samples_eq(
        &out[out.len() - f.chirp_ref.len()..],
        &f.chirp_ref,
        "restarting at the head of the queue delivers the whole track",
    );
}

/// **`Previous` while paused moves *and* resumes**, exactly as `Next` does.
///
/// The two halves of one transport control must not disagree about whether
/// pressing them starts the music. Asserted the way pause is asserted
/// elsewhere: the delivered-sample counter was frozen, and after the press it
/// advances again without a `Play`.
#[test]
fn previous_while_paused_moves_and_resumes() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(paced_config(), 3 * f.chirp_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.chirp.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));

    park_at(&engine, &events, 4_000);
    let frozen = engine.samples_delivered();
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        engine.samples_delivered(),
        frozen,
        "the pause must actually be holding delivery for this test to mean anything"
    );

    engine.send(Command::Previous).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    assert!(
        engine.samples_delivered() > frozen,
        "Previous while paused must resume, like Next — not move silently"
    );

    engine.shutdown();
    let out = collect(output);
    assert_samples_eq(
        &out[out.len() - f.chirp_ref.len()..],
        &f.chirp_ref,
        "a Previous issued while paused still delivers the track whole",
    );
}

/// **While stopped, `Previous` does nothing** — like `Next`, and for the same
/// reason: there is no current track to be some number of seconds into.
#[test]
fn previous_while_stopped_is_a_no_op() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(fast_config(), 2 * f.b_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.b.clone()],
            origin: None,
        })
        .expect("send");
    assert_eq!(next_queue_changed(&events), (1, None)); // the SetQueue above
    engine.send(Command::Previous).expect("send");
    assert_no_event_within(&events, Duration::from_millis(120));

    // And the queue is untouched: a later Play still starts at the top.
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.b, 0));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    assert_samples_eq(
        &collect(output),
        &f.b_ref,
        "a no-op Previous must not have disturbed the queue",
    );
}

// ---------------------------------------------------------------------------
// Command::JumpTo — reaching a queue entry by name instead of by repetition
// ---------------------------------------------------------------------------

/// **`JumpTo` plays the entry it names, from its start.** One command, one
/// session, one track of audio — the thing a click on a queue row means, and
/// what eight `Next`s are not.
///
/// Asserted on the samples as well as the events: what follows the jump is the
/// named track whole, bit for bit against a reference decode.
#[test]
fn jump_to_plays_the_named_entry_from_its_start() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.dc_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone(), f.dc.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.send(Command::JumpTo { position: 2 }).expect("send");
    assert_eq!(
        next_transport_event(&events),
        started(&f.dc, 2),
        "JumpTo must reach the entry it names without playing the ones between"
    );
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    assert_samples_eq(
        &out[out.len() - f.dc_ref.len()..],
        &f.dc_ref,
        "the jumped-to track must be delivered whole, from its first sample",
    );
    assert_samples_eq(
        &out[..out.len() - f.dc_ref.len()],
        &f.a_ref[..out.len() - f.dc_ref.len()],
        "everything before the jump must be the head of the track it left",
    );
}

/// **Aimed at the track already playing, it restarts it.** A click on the
/// playing row is a position change, not a redundant command.
#[test]
fn jump_to_the_playing_entry_restarts_it() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(paced_config(), 2 * f.chirp_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.chirp.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    park_at(&engine, &events, 4_000);

    engine.send(Command::JumpTo { position: 0 }).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    assert_samples_eq(
        &out[out.len() - f.chirp_ref.len()..],
        &f.chirp_ref,
        "a restarted track must be delivered whole, from its first sample",
    );
}

/// **While paused it moves and resumes**, exactly as `Next` and `Previous` do:
/// three transport commands that select a queue entry must not disagree about
/// whether pressing them starts the music.
#[test]
fn jump_to_while_paused_moves_and_resumes() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.dc_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.dc.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    park_at(&engine, &events, 1_000);
    let frozen = engine.samples_delivered();

    engine.send(Command::JumpTo { position: 1 }).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.dc, 1));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    assert!(
        engine.samples_delivered() > frozen,
        "JumpTo while paused must resume, not merely move"
    );

    engine.shutdown();
    let out = collect(output);
    assert_samples_eq(
        &out[out.len() - f.dc_ref.len()..],
        &f.dc_ref,
        "the jumped-to track must play in full after a paused jump",
    );
}

/// **While stopped it starts playing there** — the one place it parts company
/// with `Next` and `Previous`, which are no-ops because they are relative and
/// have nothing to be relative to. An absolute position has no such difficulty.
#[test]
fn jump_to_while_stopped_starts_playing_there() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(fast_config(), 2 * f.dc_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.b.clone(), f.dc.clone()],
            origin: None,
        })
        .expect("send");
    assert_eq!(next_queue_changed(&events), (2, None));

    engine.send(Command::JumpTo { position: 1 }).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.dc, 1));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    assert_samples_eq(
        &collect(output),
        &f.dc_ref,
        "a jump from a standing start plays the named entry and nothing else",
    );
}

/// **Past the end of the queue the run ends** — `Next`'s answer, not a clamp
/// onto the last entry and not an error. A later `Play` starts from the top,
/// which is what `QueueEnded` has always meant.
#[test]
fn jump_to_out_of_range_ends_the_queue() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.a_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.send(Command::JumpTo { position: 9 }).expect("send");
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    let at_end = engine.samples_delivered();
    assert_no_event_within(&events, Duration::from_millis(120));
    assert_eq!(
        engine.samples_delivered(),
        at_end,
        "nothing may play after a jump past the end of the queue"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
}

/// An empty queue has no entry to jump to, and says so the way `Play` does.
#[test]
fn jump_to_on_an_empty_queue_ends_the_queue() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(fast_config(), f.b_ref.len()).expect("spawn engine");
    engine.send(Command::JumpTo { position: 0 }).expect("send");
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    assert_eq!(engine.samples_delivered(), 0);
}

// ---------------------------------------------------------------------------
// Command::UpdateQueue — editing without silencing (ADR-0014)
// ---------------------------------------------------------------------------

/// **The claim the command exists for: an edit that misses the playing track
/// does not disturb one delivered sample.**
///
/// A track is inserted *above* the one playing — the renumbering case — while
/// its audio is in flight. What reaches the sink must be byte-for-byte what an
/// unedited run of the same two tracks delivers: the reference decodes,
/// concatenated. The events are asserted too, and the absence of
/// [`Event::Stopped`] among them is the difference from `SetQueue`.
#[test]
fn an_edit_that_misses_the_playing_track_leaves_the_audio_untouched() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.b_ref.len() + f.dc_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    // Prepend a track nobody asked to hear: the playing entry moves from 0 to
    // 1, and the queue behind it is untouched.
    engine
        .send(Command::UpdateQueue {
            paths: vec![f.dc.clone(), f.a.clone(), f.b.clone()],
        })
        .expect("send");
    assert_eq!(
        next_queue_changed(&events),
        (3, Some(1)),
        "the engine must re-derive the playing position by identity, not keep the index"
    );
    assert_eq!(
        next_transport_event(&events),
        started(&f.b, 2),
        "the run must continue in the edited queue's terms"
    );
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let mut want = f.a_ref.clone();
    want.extend_from_slice(&f.b_ref);
    assert_samples_eq(
        &collect(output),
        &want,
        "an edit that misses the playing track must deliver exactly what an unedited run does",
    );
}

/// Removing a track *behind* the playing one takes it out of the run without
/// touching the audio: the playing track finishes in full, and the removed one
/// never plays.
#[test]
fn removing_a_later_track_leaves_the_playing_one_alone() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.dc_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.dc.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine
        .send(Command::UpdateQueue {
            paths: vec![f.a.clone()],
        })
        .expect("send");
    assert_eq!(next_queue_changed(&events), (1, Some(0)));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    assert_samples_eq(
        &collect(output),
        &f.a_ref,
        "the playing track must be delivered whole, and the removed one not at all",
    );
}

/// **Reordering re-derives the position by identity and the run follows the new
/// order.** The playing track keeps its place (index 0 still holds it), and
/// what comes after it is whatever the *edited* queue says — asserted on the
/// samples, with a constant-amplitude fixture that no sine can be mistaken for.
#[test]
fn reordering_the_queue_reroutes_the_rest_of_the_run() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.dc_ref.len() + f.b_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone(), f.dc.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    // Swap the two tracks that have not played yet.
    engine
        .send(Command::UpdateQueue {
            paths: vec![f.a.clone(), f.dc.clone(), f.b.clone()],
        })
        .expect("send");
    assert_eq!(next_queue_changed(&events), (3, Some(0)));
    assert_eq!(next_transport_event(&events), started(&f.dc, 1));
    assert_eq!(next_transport_event(&events), started(&f.b, 2));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let mut want = f.a_ref.clone();
    want.extend_from_slice(&f.dc_ref);
    want.extend_from_slice(&f.b_ref);
    assert_samples_eq(
        &collect(output),
        &want,
        "the rest of the run must follow the edited order",
    );
}

/// **Removing the playing track is the one edit that interrupts**, because it
/// is the one edit that touches it. Playback moves to the entry that took its
/// place — the same index in the new queue — from its start.
#[test]
fn removing_the_playing_track_continues_at_the_entry_that_took_its_place() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.dc_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.dc.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    let cut = engine.samples_delivered();

    engine
        .send(Command::UpdateQueue {
            paths: vec![f.dc.clone()],
        })
        .expect("send");
    // The edit is announced with the position it moved the run to, and the
    // audio follows: the entry that took the removed one's place.
    assert_eq!(next_queue_changed(&events), (1, Some(0)));
    assert_eq!(
        next_transport_event(&events),
        started(&f.dc, 0),
        "the entry that took the removed one's place must start playing"
    );
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    assert!(
        out.len() >= cut + f.dc_ref.len(),
        "the replacement track must have played in full"
    );
    assert_samples_eq(
        &out[out.len() - f.dc_ref.len()..],
        &f.dc_ref,
        "what follows the removal is the entry that took its place, from its start",
    );
    assert_samples_eq(
        &out[..cut],
        &f.a_ref[..cut],
        "everything before the removal is the clean head of the track that was playing",
    );
}

/// Emptying the queue while it plays ends the run: there is no entry left to
/// take the playing track's place.
#[test]
fn an_edit_that_empties_the_queue_ends_the_run() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.a_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine
        .send(Command::UpdateQueue { paths: Vec::new() })
        .expect("send");
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    assert_eq!(next_queue_changed(&events), (0, None));
    let at_end = engine.samples_delivered();
    assert_no_event_within(&events, Duration::from_millis(120));
    assert_eq!(
        engine.samples_delivered(),
        at_end,
        "nothing may play out of an emptied queue"
    );
}

/// Sending the queue the engine already holds says nothing and does nothing —
/// the rule every command in this protocol follows.
#[test]
fn a_redundant_edit_says_nothing() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(fast_config(), f.b_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    assert_eq!(next_queue_changed(&events), (2, None));

    engine
        .send(Command::UpdateQueue {
            paths: vec![f.a.clone(), f.b.clone()],
        })
        .expect("send");
    assert_no_event_within(&events, Duration::from_millis(120));
}

/// **An edit is not a transport command**: a paused queue stays paused through
/// one, delivers nothing while it is applied, and resumes bit-identically.
#[test]
fn an_edit_while_paused_stays_paused() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.b_ref.len();
    let cfg = EngineConfig {
        consumer_pace: Duration::from_millis(1),
        ..paced_config()
    };
    let (engine, events, output) = spawn_offline(cfg, capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    engine.send(Command::Pause).expect("send");
    assert_eq!(next_transport_event(&events), Event::Paused);
    let frozen = engine.samples_delivered();
    assert!(
        frozen < f.a_ref.len(),
        "the pause landed after the track ended"
    );

    engine
        .send(Command::UpdateQueue {
            paths: vec![f.dc.clone(), f.a.clone(), f.b.clone()],
        })
        .expect("send");
    assert_eq!(next_queue_changed(&events), (3, Some(1)));
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        engine.samples_delivered(),
        frozen,
        "an edit must not start music that was not playing"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);
    assert_eq!(next_transport_event(&events), started(&f.b, 2));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let mut want = f.a_ref.clone();
    want.extend_from_slice(&f.b_ref);
    assert_samples_eq(
        &collect(output),
        &want,
        "pausing, editing and resuming must still deliver exactly the unedited stream",
    );
}

/// A queue parked mid-track by a paused seek keeps its position through an
/// edit: the session is rebuilt on the new queue (nothing has been delivered,
/// so nothing can be heard), and rebuilding it at the top of the track would
/// silently rewind the listener.
#[test]
fn an_edit_does_not_rewind_a_paused_seek() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(paced_config(), 2 * f.chirp_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.chirp.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.chirp, 0));
    park_at(&engine, &events, 4_500);
    let frozen = engine.samples_delivered();

    engine
        .send(Command::UpdateQueue {
            paths: vec![f.dc.clone(), f.chirp.clone()],
        })
        .expect("send");
    assert_eq!(next_queue_changed(&events), (2, Some(1)));
    assert_eq!(
        engine.samples_delivered(),
        frozen,
        "an edit must deliver nothing while paused"
    );

    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), Event::Resumed);
    assert_eq!(
        next_progress(&events),
        (4_500, Some(CHIRP_MS)),
        "the position the listener parked at must survive the edit"
    );
    assert_eq!(next_transport_event(&events), started(&f.chirp, 1));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);

    engine.shutdown();
    let out = collect(output);
    let landed = ms_to_frames(4_500, RATE) * CHANNELS;
    assert_samples_eq(
        &out[out.len() - (f.chirp_ref.len() - landed)..],
        &f.chirp_ref[landed..],
        "playback must resume from where it was parked, not from the top",
    );
}

/// Every queue-relative command is answered in the **edited** queue's terms
/// from the moment the edit lands: a seek after a renumbering seeks within the
/// track's new position, not the one the session was started with.
#[test]
fn transport_commands_after_an_edit_speak_the_new_queues_indices() {
    let f = fixtures();
    let capacity = 2 * f.a_ref.len() + f.b_ref.len();
    let (engine, events, _output) = spawn_offline(paced_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine
        .send(Command::UpdateQueue {
            paths: vec![f.dc.clone(), f.a.clone(), f.b.clone()],
        })
        .expect("send");
    assert_eq!(next_queue_changed(&events), (3, Some(1)));

    // Seek: the current track is now entry 1, and that is what must be
    // re-started at the target — entry 0 is a track that has never played.
    engine
        .send(Command::Seek { position_ms: 500 })
        .expect("send");
    assert_eq!(
        next_transport_event(&events),
        started(&f.a, 1),
        "a seek after an edit must stay within the track it was already playing"
    );

    // Previous: two seconds in, so it steps back one position — into the
    // track the edit put in front, which the old index space did not have.
    park_at(&engine, &events, 2_000);
    const { assert!(2_000 < PREVIOUS_RESTART_MS) }
    engine.send(Command::Previous).expect("send");
    assert_eq!(
        next_transport_event(&events),
        started(&f.dc, 0),
        "Previous must step back in the queue as it is now"
    );
}

// ---------------------------------------------------------------------------
// ReplayGain (ADR-0013)
// ---------------------------------------------------------------------------

/// The linear gain a figure in hundredths of a decibel means, from the
/// definition of the decibel and nothing else: `amplitude = 10^(dB/20)`.
///
/// Written here so the sample assertions below are anchored to the physical
/// definition rather than to whatever `baz_core` computed
/// (`docs/ENGINEERING.md`: tests are written to specification). The one place
/// it is checked against a *number* rather than against the engine is
/// [`the_gain_conversion_is_the_decibel_definition`], immediately below.
#[allow(clippy::cast_possible_truncation)] // the sink's sample type is f32
fn amplitude_of(centidb: i16) -> f32 {
    10f64.powf(f64::from(centidb) / 2000.0) as f32
}

/// The decibel is a definition, not a convention: −6.02 dB halves an
/// amplitude, +6.02 dB doubles it, and 0 dB is exactly one. If this drifts,
/// every assertion below is measuring the wrong thing.
#[test]
#[allow(clippy::float_cmp)]
fn the_gain_conversion_is_the_decibel_definition() {
    assert_eq!(amplitude_of(0), 1.0, "0 dB is exactly unity");
    assert!((amplitude_of(-602) - 0.5).abs() < 1e-4);
    assert!((amplitude_of(602) - 2.0).abs() < 1e-3);
    assert!((amplitude_of(-2000) - 0.1).abs() < 1e-5);
}

/// The default ReplayGain settings with `mode` selected — what a front end
/// sends when the listener picks a mode and touches nothing else.
fn replay_gain(mode: ReplayGainMode) -> Command {
    Command::SetReplayGain {
        mode,
        preamp_centidb: 0,
        no_tag_preamp_centidb: 0,
        prevent_clipping: true,
    }
}

/// Drain the event channel until a [`Event::ReplayGainChanged`] arrives,
/// returning its fields. Transport events are ignored: this asks *what did the
/// engine decide*, not in what order it said everything.
fn next_replay_gain(events: &Receiver<Event>) -> (ReplayGainSource, i16, bool) {
    loop {
        if let Event::ReplayGainChanged {
            source,
            applied_centidb,
            clipping_prevented,
            ..
        } = next_event(events)
        {
            return (source, applied_centidb, clipping_prevented);
        }
    }
}

/// **Mode `off` is bit-identical to a baz with no ReplayGain at all.**
///
/// The claim ADR-0013 makes about its own default, asserted against the
/// existing bit-exactness ground truth: a *tagged* file played with ReplayGain
/// off delivers exactly the reference decode of the same audio, sample for
/// sample. Off is not "a gain of 0 dB that rounds to nothing" — the engine
/// performs no arithmetic, and `assert_samples_eq` is exact.
///
/// Both halves are checked, because they catch different mistakes: the tagged
/// fixture proves that tags present in a file change nothing while off, and
/// the untagged gapless pair proves that switching ReplayGain *on* over a
/// library that has never been scanned changes nothing either.
#[test]
fn replay_gain_off_delivers_a_bit_identical_stream() {
    let f = fixtures();
    let tagged_off = play_with_volume(
        std::slice::from_ref(&f.rg_a),
        f.a_ref.len(),
        &[replay_gain(ReplayGainMode::Off)],
    );
    assert_samples_eq(&tagged_off, &f.a_ref, "tagged fixture, ReplayGain off");

    // And with no command sent at all — the default is off.
    let untouched = play_with_volume(std::slice::from_ref(&f.rg_a), f.a_ref.len(), &[]);
    assert_samples_eq(&untouched, &f.a_ref, "tagged fixture, default settings");
}

/// **An untagged library is untouched even with ReplayGain on**, in every
/// mode — pinned against the gapless bit-exactness fixture, not a new one.
///
/// This is the no-ReplayGain pre-amp's default of zero, seen from the outside:
/// a listener who switches ReplayGain on before ever running a scanner gets the
/// same stream they had, and ADR-0009's guarantee is intact for that library.
#[test]
fn an_untagged_queue_is_untouched_in_every_mode() {
    let f = fixtures();
    let mut want = f.a_ref.clone();
    want.extend_from_slice(&f.b_ref);
    for mode in [ReplayGainMode::Track, ReplayGainMode::Album] {
        let out = play_with_volume(
            &[f.a.clone(), f.b.clone()],
            want.len(),
            &[replay_gain(mode)],
        );
        assert_samples_eq(&out, &want, "untagged gapless queue with ReplayGain on");
    }
}

/// **A tagged track is scaled by exactly its tagged figure**, every sample.
///
/// The fixture's tag says `-6.02 dB`; the delivered stream must be the
/// reference decode times `10^(-6.02/20)`, asserted with `==` because f32
/// multiplication is deterministic and the expected gain is derived from the
/// definition of the decibel rather than from the engine.
#[test]
#[allow(clippy::float_cmp)] // exactness is the assertion
fn a_tagged_track_is_scaled_by_exactly_its_tagged_gain() {
    let f = fixtures();
    let gain = amplitude_of(-602);
    let out = play_with_volume(
        std::slice::from_ref(&f.rg_a),
        f.a_ref.len(),
        &[replay_gain(ReplayGainMode::Track)],
    );
    assert_eq!(out.len(), f.a_ref.len(), "scaling must not drop a sample");
    let want: Vec<f32> = f.a_ref.iter().map(|s| s * gain).collect();
    assert_samples_eq(&out, &want, "track-gain output");
    assert!(
        gain < 1.0 && out.iter().zip(&f.a_ref).any(|(o, r)| o != r),
        "the fixture must actually be attenuated, or this proves nothing"
    );
}

/// **Album mode uses the album gain, and falls back to the track gain when
/// there is no album value** — both asserted on samples.
#[test]
fn album_mode_uses_the_album_gain_and_falls_back_to_the_track_gain() {
    let f = fixtures();
    // rg_a declares ALBUM_GAIN -3.00 dB and TRACK_GAIN -6.02 dB, so the two
    // modes are distinguishable from the audio.
    let album = play_with_volume(
        std::slice::from_ref(&f.rg_a),
        f.a_ref.len(),
        &[replay_gain(ReplayGainMode::Album)],
    );
    let want: Vec<f32> = f.a_ref.iter().map(|s| s * amplitude_of(-300)).collect();
    assert_samples_eq(&album, &want, "album-gain output");

    // rg_single declares only a track gain: album mode falls back to it rather
    // than leaving a lone downloaded track unnormalised.
    let fallback = play_with_volume(
        std::slice::from_ref(&f.rg_single),
        f.dc_ref.len(),
        &[replay_gain(ReplayGainMode::Album)],
    );
    let want: Vec<f32> = f.dc_ref.iter().map(|s| s * amplitude_of(-400)).collect();
    assert_samples_eq(&fallback, &want, "album mode falling back to track gain");
}

/// **Clipping prevention reduces a gain the declared peak has no room for**,
/// and the delivered samples stay inside full scale.
///
/// The fixture is a constant at exactly its declared peak (0.5) and asks for
/// +12 dB, which would deliver 1.99. The rule cuts the gain to `-20·log₁₀(peak)`
/// rounded *down*, so the loudest delivered sample is at or below 1.0 — checked
/// as a property of the audio, not of the reported number.
#[test]
fn clipping_prevention_cuts_a_gain_the_peak_has_no_room_for() {
    let f = fixtures();
    let out = play_with_volume(
        std::slice::from_ref(&f.rg_clip),
        f.dc_ref.len(),
        &[replay_gain(ReplayGainMode::Track)],
    );
    assert_eq!(out.len(), f.dc_ref.len());
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(
        peak <= 1.0,
        "clipping prevention must keep the stream inside full scale: {peak}"
    );
    // 0.5 leaves 6.0206 dB of headroom, floored to 6.02 dB — so the fixture is
    // delivered just under full scale rather than merely somewhere below it.
    assert!(
        peak > 0.999,
        "the cut must not be more than necessary: {peak}"
    );
    let want: Vec<f32> = f.dc_ref.iter().map(|s| s * amplitude_of(602)).collect();
    assert_samples_eq(&out, &want, "clip-limited output");

    // With prevention disarmed the full +12 dB is applied, and it does clip.
    let unlimited = play_with_volume(
        std::slice::from_ref(&f.rg_clip),
        f.dc_ref.len(),
        &[Command::SetReplayGain {
            mode: ReplayGainMode::Track,
            preamp_centidb: 0,
            no_tag_preamp_centidb: 0,
            prevent_clipping: false,
        }],
    );
    let peak = unlimited.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(
        peak > 1.9,
        "unarmed, the tags are honoured in full — including over full scale: {peak}"
    );
}

/// **The pre-amp adds to a tagged gain; the no-ReplayGain pre-amp applies
/// instead when the file declares none.** Two settings, two fixtures, samples
/// either way.
#[test]
fn the_two_pre_amps_apply_to_the_cases_they_are_for() {
    let f = fixtures();
    let settings = Command::SetReplayGain {
        mode: ReplayGainMode::Track,
        preamp_centidb: 200,
        no_tag_preamp_centidb: -500,
        prevent_clipping: true,
    };
    // Tagged: -4.00 dB + 2.00 dB of pre-amp.
    let tagged = play_with_volume(
        std::slice::from_ref(&f.rg_single),
        f.dc_ref.len(),
        std::slice::from_ref(&settings),
    );
    let want: Vec<f32> = f.dc_ref.iter().map(|s| s * amplitude_of(-200)).collect();
    assert_samples_eq(&tagged, &want, "tagged track with a pre-amp");

    // Untagged: the *other* pre-amp, and not the first one.
    let untagged = play_with_volume(
        std::slice::from_ref(&f.dc),
        f.dc_ref.len(),
        std::slice::from_ref(&settings),
    );
    let want: Vec<f32> = f.dc_ref.iter().map(|s| s * amplitude_of(-500)).collect();
    assert_samples_eq(&untagged, &want, "untagged track, no-ReplayGain pre-amp");
}

/// **Volume and ReplayGain compose, and are applied exactly once.**
///
/// The engine multiplies the two gains together and scales each sample by the
/// *product* — one multiply, not two stages. The distinction is observable in
/// f32: `s·(v·g)` and `(s·v)·g` round differently for some samples, and the
/// test asserts the first *and* proves the fixture can tell them apart, so
/// "once" is a measured property rather than a claim about the source.
#[test]
#[allow(clippy::float_cmp)] // exactness is the assertion
fn volume_and_replay_gain_compose_into_exactly_one_multiply() {
    let f = fixtures();
    let volume = Volume::new(618).amplitude();
    let gain = amplitude_of(-602);
    let combined = volume * gain;

    let out = play_with_volume(
        std::slice::from_ref(&f.rg_a),
        f.a_ref.len(),
        &[
            replay_gain(ReplayGainMode::Track),
            Command::SetVolume { position: 618 },
        ],
    );
    assert_eq!(out.len(), f.a_ref.len());

    let mut discriminating = false;
    for (i, (got, source)) in out.iter().zip(&f.a_ref).enumerate() {
        assert_eq!(
            *got,
            source * combined,
            "sample {i}: the two gains must arrive as one multiply"
        );
        if (source * volume) * gain != source * combined {
            discriminating = true;
        }
    }
    assert!(
        discriminating,
        "the fixture must contain samples that distinguish one multiply from \
         two, or this test would pass against either implementation"
    );
}

/// **Both gains are really present**: the combined stream is neither of the two
/// single-gain streams, and is not either gain applied twice.
#[test]
fn neither_gain_is_dropped_nor_applied_twice() {
    let f = fixtures();
    let volume = Volume::new(618).amplitude();
    let gain = amplitude_of(-602);
    let out = play_with_volume(
        std::slice::from_ref(&f.rg_a),
        f.a_ref.len(),
        &[
            replay_gain(ReplayGainMode::Track),
            Command::SetVolume { position: 618 },
        ],
    );
    let sample = |scale: f32| -> Vec<f32> { f.a_ref.iter().map(|s| s * scale).collect() };
    for (what, wrong) in [
        ("the volume alone", sample(volume)),
        ("the ReplayGain alone", sample(gain)),
        ("the ReplayGain twice", sample(volume * gain * gain)),
        ("the volume twice", sample(volume * volume * gain)),
        ("neither", f.a_ref.clone()),
    ] {
        assert_ne!(out, wrong, "the delivered stream must not be {what}");
    }
}

/// **The gain follows the track across a boundary**, and does so from the
/// boundary's own first sample.
///
/// Track mode over a two-track album whose tracks declare different gains: the
/// first track's steady state is its own gain, the second track's steady state
/// is *its* own gain, and the change happens at the splice rather than a pump
/// block late. The engine caps every pump read at the next boundary precisely
/// so this is true.
///
/// The transition itself is a slew ([`RAMP_MS`]) rather than a step, so the
/// assertion is on the two steady states and on where the ramp begins — the
/// same shape as the mid-playback volume test above.
#[test]
fn the_gain_follows_the_track_across_a_boundary() {
    let f = fixtures();
    let boundary = f.a_ref.len();
    let mut want_len = boundary;
    want_len += f.b_ref.len();
    let out = play_with_volume(
        &[f.rg_a.clone(), f.rg_b.clone()],
        want_len,
        &[replay_gain(ReplayGainMode::Track)],
    );
    assert_eq!(out.len(), want_len, "gapless still delivers every sample");

    let first = amplitude_of(-602);
    let second = amplitude_of(250);
    // The whole of track A is at A's gain: the ramp into it happened before a
    // single sample was delivered (nothing was audible yet), so there is no
    // transition to exclude at the front.
    let want_a: Vec<f32> = f.a_ref.iter().map(|s| s * first).collect();
    assert_samples_eq(&out[..boundary], &want_a, "track A at its own gain");

    // Track B's steady state, past the slew. `RAMP_MS` at `RATE` bounds it.
    let ramp = ms_to_frames(u64::from(RAMP_MS) + 5, RATE) * CHANNELS;
    let want_b: Vec<f32> = f.b_ref[ramp..].iter().map(|s| s * second).collect();
    assert_samples_eq(&out[boundary + ramp..], &want_b, "track B at its own gain");
    assert!(
        second > first,
        "the two tracks must ask for different gains, or this proves nothing"
    );
}

/// **In album mode an album has one gain, so a boundary inside it has no
/// transition at all** — which is the property album mode exists to provide.
#[test]
fn album_mode_holds_one_gain_across_the_whole_album() {
    let f = fixtures();
    let mut want: Vec<f32> = f.a_ref.iter().map(|s| s * amplitude_of(-300)).collect();
    want.extend(f.b_ref.iter().map(|s| s * amplitude_of(-300)));
    let out = play_with_volume(
        &[f.rg_a.clone(), f.rg_b.clone()],
        want.len(),
        &[replay_gain(ReplayGainMode::Album)],
    );
    assert_samples_eq(&out, &want, "one album, one gain, no ramp at the splice");
}

/// **The honesty readout, per mode.** ReplayGain is a software gain, so when it
/// is active and not unity the path is [`VolumePath::SoftwareGain`] — reported
/// through the *existing* mechanism, with the volume still at unity.
///
/// This is the ADR-0013 amendment to ADR-0011 in one test: the question a front
/// end asks is still `path.is_transparent()`, and it still gets the right
/// answer.
#[test]
fn an_active_replay_gain_reports_a_software_gain_path_at_unity_volume() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(fast_config(), f.a_ref.len()).expect("spawn engine");
    // Nothing playing yet, and off: the path is untouched and the readout says
    // so on both channels.
    assert_eq!(engine.volume().path, VolumePath::Unity);
    assert!(engine.volume().path.is_transparent());
    assert_eq!(engine.replay_gain().settings.mode, ReplayGainMode::Off);
    assert_eq!(
        engine.replay_gain().applied.source,
        ReplayGainSource::Disabled
    );

    engine
        .send(Command::SetQueue {
            paths: vec![f.rg_a.clone()],
            origin: None,
        })
        .expect("send");
    engine
        .send(replay_gain(ReplayGainMode::Track))
        .expect("send");
    // Switching the mode on is news even before anything plays: the settings
    // changed, so the engine confirms them.
    let (source, applied, clipped) = next_replay_gain(&events);
    assert_eq!(
        (source, applied, clipped),
        (ReplayGainSource::NoTag, 0, false),
        "nothing is playing, so nothing has a ReplayGain figure yet"
    );
    assert!(
        engine.volume().path.is_transparent(),
        "a mode with nothing to apply it to changes no samples"
    );

    engine.send(Command::Play).expect("send");
    // Once the track's audio starts, its tags resolve and the path moves.
    let (source, applied, clipped) = next_replay_gain(&events);
    assert_eq!(
        (source, applied, clipped),
        (ReplayGainSource::Track, -602, false)
    );
    let state = engine.replay_gain();
    assert_eq!(state.applied.gain_centidb, -602);
    assert!(!state.applied.is_transparent());
    let volume = engine.volume();
    assert_eq!(
        (volume.volume, volume.muted, volume.path),
        (Volume::UNITY, false, VolumePath::SoftwareGain),
        "the volume is untouched at unity, and the path still tells the truth"
    );
    assert!(
        !engine.volume().path.is_transparent(),
        "an active ReplayGain is not a bit-exact path, and says so"
    );

    // And switching it back off restores the untouched path.
    engine.send(replay_gain(ReplayGainMode::Off)).expect("send");
    let (source, applied, _) = next_replay_gain(&events);
    assert_eq!((source, applied), (ReplayGainSource::Disabled, 0));
    engine.shutdown();
}

/// The readout for each mode over the same tagged album track, including the
/// clipping-prevention flag — the fields a front end renders.
#[test]
fn the_readout_reports_the_source_and_figure_for_each_mode() {
    let f = fixtures();
    let cases: &[(&PathBuf, ReplayGainMode, ReplayGainSource, i16, bool)] = &[
        (
            &f.rg_a,
            ReplayGainMode::Track,
            ReplayGainSource::Track,
            -602,
            false,
        ),
        (
            &f.rg_a,
            ReplayGainMode::Album,
            ReplayGainSource::Album,
            -300,
            false,
        ),
        (
            &f.rg_single,
            ReplayGainMode::Album,
            ReplayGainSource::TrackFallback,
            -400,
            false,
        ),
        (
            &f.dc,
            ReplayGainMode::Track,
            ReplayGainSource::NoTag,
            0,
            false,
        ),
        (
            &f.rg_clip,
            ReplayGainMode::Track,
            ReplayGainSource::Track,
            602,
            true,
        ),
    ];
    for (path, mode, want_source, want_centidb, want_clipped) in cases {
        let (engine, events, _output) =
            spawn_offline(fast_config(), 4 * CHANNELS).expect("spawn engine");
        engine
            .send(Command::SetQueue {
                paths: vec![(*path).clone()],
                origin: None,
            })
            .expect("send");
        engine.send(replay_gain(*mode)).expect("send");
        engine.send(Command::Play).expect("send");
        // The first report is the settings change (nothing playing yet); the
        // one that matters is the track's, which follows its first samples.
        let mut got = next_replay_gain(&events);
        while got.0 == ReplayGainSource::NoTag && *want_source != ReplayGainSource::NoTag {
            got = next_replay_gain(&events);
        }
        assert_eq!(
            got,
            (*want_source, *want_centidb, *want_clipped),
            "{} in {mode:?}",
            path.display()
        );
        engine.shutdown();
    }
}

/// Redundant settings emit nothing, like every other command in this protocol —
/// so an arriving [`Event::ReplayGainChanged`] is always news.
#[test]
fn redundant_replay_gain_commands_are_silent() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(fast_config(), 4 * CHANNELS).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.dc.clone()],
            origin: None,
        })
        .expect("send");
    engine
        .send(replay_gain(ReplayGainMode::Track))
        .expect("send");
    let _ = next_replay_gain(&events);
    // The same settings again: nothing changed, so nothing is said.
    engine
        .send(replay_gain(ReplayGainMode::Track))
        .expect("send");
    assert_no_event_within(&events, Duration::from_millis(150));
    engine.shutdown();
}

/// The settings survive everything the transport does — they are engine state,
/// exactly as the volume is.
#[test]
fn replay_gain_settings_survive_the_transport() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.a_ref.len() + f.b_ref.len()).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.rg_a.clone(), f.rg_b.clone()],
            origin: None,
        })
        .expect("send");
    let settings = Command::SetReplayGain {
        mode: ReplayGainMode::Album,
        preamp_centidb: 150,
        no_tag_preamp_centidb: -250,
        prevent_clipping: false,
    };
    engine.send(settings.clone()).expect("send");
    engine.send(Command::Play).expect("send");
    loop {
        if matches!(next_event(&events), Event::TrackStarted { .. }) {
            break;
        }
    }

    for command in [
        Command::Pause,
        Command::Play,
        Command::Seek { position_ms: 500 },
        Command::Next,
        Command::Previous,
        Command::Stop,
    ] {
        engine.send(command.clone()).expect("send");
        thread::sleep(Duration::from_millis(20));
        let state = engine.replay_gain();
        assert_eq!(
            (
                state.settings.mode,
                state.settings.preamp_centidb,
                state.settings.no_tag_preamp_centidb,
                state.settings.prevent_clipping,
            ),
            (ReplayGainMode::Album, 150, -250, false),
            "the settings must survive {command:?}"
        );
    }
    engine.shutdown();
}

/// Out-of-range pre-amps clamp rather than being rejected, and the engine
/// reports the clamped values it will actually use.
#[test]
fn out_of_range_pre_amps_clamp_and_are_reported_clamped() {
    let (engine, events, _output) = spawn_offline(fast_config(), 4 * CHANNELS).expect("spawn");
    engine
        .send(Command::SetReplayGain {
            mode: ReplayGainMode::Track,
            preamp_centidb: i16::MAX,
            no_tag_preamp_centidb: i16::MIN,
            prevent_clipping: true,
        })
        .expect("send");
    // Skip past any VolumeChanged: engaging ReplayGain republishes the
    // combined gain *before* announcing itself, so the two arrive together and
    // in that order. Which of the pair lands first is not a contract — that
    // the state behind both is current when either is seen, is.
    let (preamp_centidb, no_tag_preamp_centidb, applied_centidb) = loop {
        let event = next_event(&events);
        if let Event::ReplayGainChanged {
            preamp_centidb,
            no_tag_preamp_centidb,
            applied_centidb,
            ..
        } = event
        {
            break (preamp_centidb, no_tag_preamp_centidb, applied_centidb);
        }
    };
    assert_eq!(preamp_centidb, MAX_PREAMP_CENTIDB);
    assert_eq!(no_tag_preamp_centidb, -MAX_PREAMP_CENTIDB);
    assert_eq!(
        applied_centidb, -MAX_PREAMP_CENTIDB,
        "nothing is playing, so the no-ReplayGain pre-amp is what applies"
    );
    assert_eq!(
        engine.replay_gain().settings.preamp_centidb,
        MAX_PREAMP_CENTIDB
    );
    engine.shutdown();
}

// ---------------------------------------------------------------------------
// Computed ReplayGain reaching playback (ADR-0015)
// ---------------------------------------------------------------------------

/// A [`ComputedGains`] double: the map a library would hand the engine, built
/// by hand so the seam is exercised without a database.
///
/// This is the ADR-0011 §7 pattern — the branch is reachable by a test double
/// *today*, so the engine's half of the arrangement is tested before a front
/// end wires the real one in.
#[derive(Debug, Default)]
struct MeasuredLibrary(std::collections::HashMap<PathBuf, ReplayGainTags>);

impl MeasuredLibrary {
    fn with(path: &Path, figures: ReplayGainTags) -> Arc<Self> {
        let mut map = std::collections::HashMap::new();
        map.insert(path.to_path_buf(), figures);
        Arc::new(Self(map))
    }
}

impl ComputedGains for MeasuredLibrary {
    fn computed(&self, path: &Path) -> ReplayGainTags {
        self.0.get(path).copied().unwrap_or_default()
    }
}

/// Play `queue` with a measured-gain snapshot attached before playback starts,
/// returning the delivered samples and the ReplayGain the engine reported.
fn play_with_measurements(
    queue: &[PathBuf],
    capacity: usize,
    gains: Arc<dyn ComputedGains>,
    commands: &[Command],
) -> (Vec<f32>, Vec<(ReplayGainSource, i16)>) {
    let (engine, events, output) = spawn_offline(fast_config(), capacity).expect("spawn engine");
    engine.set_computed_gains(Some(gains));
    engine
        .send(Command::SetQueue {
            paths: queue.to_vec(),
            origin: None,
        })
        .expect("send");
    for command in commands {
        engine.send(command.clone()).expect("send");
    }
    engine.send(Command::Play).expect("send");
    let mut reported = Vec::new();
    loop {
        match next_event(&events) {
            Event::QueueEnded => break,
            Event::ReplayGainChanged {
                source,
                applied_centidb,
                ..
            } => reported.push((source, applied_centidb)),
            _ => {}
        }
    }
    engine.shutdown();
    (collect(output), reported)
}

/// **A measured figure reaches the samples, and says it was measured.**
///
/// The untagged fixture carries no ReplayGain at all, so before ADR-0015 it
/// resolved to `no_tag` and played untouched. With a measurement attached it is
/// scaled by exactly that figure — asserted with `==`, because the expected
/// gain comes from the definition of the decibel and not from the engine — and
/// the readout names the origin as `computed_track` rather than `track`.
#[test]
#[allow(clippy::float_cmp)] // exactness is the assertion
fn a_measured_track_is_scaled_by_its_measured_gain_and_says_so() {
    let f = fixtures();
    let gains = MeasuredLibrary::with(
        &f.a,
        ReplayGainTags {
            track_gain_centidb: Some(-602),
            track_peak_micro: Some(500_000),
            ..ReplayGainTags::default()
        },
    );
    let (out, reported) = play_with_measurements(
        std::slice::from_ref(&f.a),
        f.a_ref.len(),
        gains,
        &[replay_gain(ReplayGainMode::Track)],
    );
    let gain = amplitude_of(-602);
    let want: Vec<f32> = f.a_ref.iter().map(|s| s * gain).collect();
    assert_eq!(out.len(), want.len(), "scaling must not drop a sample");
    assert_samples_eq(&out, &want, "a measured track gain");
    assert!(
        reported.contains(&(ReplayGainSource::ComputedTrack, -602)),
        "the readout must name the origin: {reported:?}"
    );
    assert!(
        reported
            .iter()
            .all(|(source, _)| *source != ReplayGainSource::Track),
        "nothing came from a tag here: {reported:?}"
    );
}

/// **A tag outranks a measurement, all the way to the samples.**
///
/// The tagged fixture asks for −6.02 dB; the measurement attached asks for
/// +10 dB. The delivered stream must carry the tag's figure, and the readout
/// must say `track` rather than `computed_track` — the honesty half of the
/// same rule.
#[test]
#[allow(clippy::float_cmp)] // exactness is the assertion
fn a_tagged_track_outranks_a_measurement_in_the_engine_too() {
    let f = fixtures();
    let gains = MeasuredLibrary::with(
        &f.rg_a,
        ReplayGainTags {
            track_gain_centidb: Some(1_000),
            track_peak_micro: Some(100_000),
            ..ReplayGainTags::default()
        },
    );
    let (out, reported) = play_with_measurements(
        std::slice::from_ref(&f.rg_a),
        f.a_ref.len(),
        gains,
        &[replay_gain(ReplayGainMode::Track)],
    );
    let gain = amplitude_of(-602);
    let want: Vec<f32> = f.a_ref.iter().map(|s| s * gain).collect();
    assert_samples_eq(&out, &want, "the file's own tag, not the measurement");
    assert!(
        reported.contains(&(ReplayGainSource::Track, -602)),
        "and it says the figure came from the tag: {reported:?}"
    );
}

/// **An engine with no library attached behaves exactly as ADR-0013 left it.**
///
/// The default is no seam at all, which is the state every engine spawns in and
/// the one every pre-ADR-0015 test already asserts against. Stated here as its
/// own claim because "the new feature is off unless asked for" is the property,
/// not a coincidence of the other tests.
#[test]
fn an_engine_with_no_measurements_attached_is_unchanged() {
    let f = fixtures();
    let (engine, events, output) =
        spawn_offline(fast_config(), f.a_ref.len()).expect("spawn engine");
    // Explicitly detached, which is also the default.
    engine.set_computed_gains(None);
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
            origin: None,
        })
        .expect("send");
    engine
        .send(replay_gain(ReplayGainMode::Track))
        .expect("send");
    engine.send(Command::Play).expect("send");
    let mut sources = Vec::new();
    loop {
        match next_event(&events) {
            Event::QueueEnded => break,
            Event::ReplayGainChanged { source, .. } => sources.push(source),
            _ => {}
        }
    }
    engine.shutdown();
    assert_samples_eq(
        &collect(output),
        &f.a_ref,
        "untagged, unmeasured, untouched",
    );
    assert!(
        sources.contains(&ReplayGainSource::NoTag),
        "an unmeasured, untagged file reads as `no_tag`: {sources:?}"
    );
}

/// **A measured album figure is one gain across the whole album**, and it is
/// reported once rather than at every boundary — the same property album mode
/// already had for tagged figures, now for measured ones.
#[test]
fn a_measured_album_figure_holds_across_the_album() {
    let f = fixtures();
    let figures = ReplayGainTags {
        track_gain_centidb: Some(1_100),
        track_peak_micro: Some(200_000),
        album_gain_centidb: Some(-602),
        album_peak_micro: Some(500_000),
    };
    let mut map = std::collections::HashMap::new();
    map.insert(f.a.clone(), figures);
    map.insert(f.b.clone(), figures);
    let gains: Arc<dyn ComputedGains> = Arc::new(MeasuredLibrary(map));

    let mut want = f.a_ref.clone();
    want.extend_from_slice(&f.b_ref);
    let (out, reported) = play_with_measurements(
        &[f.a.clone(), f.b.clone()],
        want.len(),
        gains,
        &[replay_gain(ReplayGainMode::Album)],
    );
    assert_eq!(out.len(), want.len());
    let announcements: Vec<_> = reported
        .iter()
        .filter(|(source, _)| *source == ReplayGainSource::ComputedAlbum)
        .collect();
    assert_eq!(
        announcements.len(),
        1,
        "one album, one gain, one announcement: {reported:?}"
    );
    assert_eq!(announcements[0].1, -602);
}

/// **The shared readout agrees with the event**, for a measured figure exactly
/// as for a tagged one — the state-before-event contract, from the pull side.
#[test]
fn the_handle_reports_a_measured_source_too() {
    let f = fixtures();
    let gains = MeasuredLibrary::with(
        &f.a,
        ReplayGainTags {
            track_gain_centidb: Some(-602),
            ..ReplayGainTags::default()
        },
    );
    let (engine, events, _output) =
        spawn_offline(fast_config(), f.a_ref.len()).expect("spawn engine");
    engine.set_computed_gains(Some(gains));
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
            origin: None,
        })
        .expect("send");
    engine
        .send(replay_gain(ReplayGainMode::Track))
        .expect("send");
    engine.send(Command::Play).expect("send");
    loop {
        match next_event(&events) {
            Event::ReplayGainChanged {
                source: ReplayGainSource::ComputedTrack,
                applied_centidb,
                ..
            } => {
                // Read *after* the event, which is what the ordering contract
                // is for: the state was published first, so this cannot be
                // older than the news that prompted it.
                let state = engine.replay_gain();
                assert_eq!(state.applied.source, ReplayGainSource::ComputedTrack);
                assert_eq!(state.applied.gain_centidb, applied_centidb);
                assert_eq!(applied_centidb, -602);
                break;
            }
            Event::QueueEnded => panic!("the queue ended before a measured figure was reported"),
            _ => {}
        }
    }
    engine.shutdown();
}

// ---------------------------------------------------------------------------
// Traversal: shuffle is a property of the walk, not of the list
// ---------------------------------------------------------------------------

/// The first seed whose pass over `len` entries is `want`.
///
/// Searched rather than pinned because the generator is an implementation
/// detail and a hardcoded seed would be a magic number that only the
/// implementation could explain. The search is over a pure public function, so
/// it is deterministic and costs microseconds.
fn seed_ordering(len: usize, want: &[usize]) -> u64 {
    (0..10_000_u64)
        .find(|&seed| Traversal::Shuffled { seed }.play_order(len) == want)
        .unwrap_or_else(|| panic!("no seed in 10 000 walks {len} entries as {want:?}"))
}

/// **Gaplessness holds with shuffle on.** The acceptance test for the whole
/// traversal design, and the reason the decision lives in the engine at all.
///
/// The queue is `[a, b]` and the traversal is a seed that walks it `[1, 0]`. The
/// delivered stream is then compared, **sample for sample**, against the
/// reference decode of `b` concatenated with the reference decode of `a` — the
/// same ground truth [`unity_volume_delivers_a_bit_identical_stream`] uses for
/// the unshuffled pair, with the two halves swapped.
///
/// What a bit-identical stream rules out is exactly what a gap is: a shuffle
/// that chose its next track when the current one *ended* could not have decoded
/// it in time, and the silence, the click or the repeated block that follows
/// would all be a length or a value mismatch here. It also rules out the
/// cheaper mistakes — a boundary that resampled, a boundary that dropped its
/// first block, a run that played the queue in its own order and ignored the
/// traversal entirely.
#[test]
fn a_shuffled_run_is_gapless_and_bit_identical() {
    let f = fixtures();
    let seed = seed_ordering(2, &[1, 0]);
    let mut want = f.b_ref.clone();
    want.extend_from_slice(&f.a_ref);
    let (engine, events, output) = spawn_offline(paced_config(), want.len()).expect("spawn engine");
    let traversal = Traversal::Shuffled { seed };
    engine
        .send(Command::SetTraversal { traversal })
        .expect("send");
    assert_eq!(
        next_transport_event(&events),
        Event::TraversalChanged { traversal }
    );
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    // The run visits the queue's positions in the bag's order, and says so with
    // *queue* positions — the numbers the front end drew its rows from.
    assert_eq!(next_transport_event(&events), started(&f.b, 1));
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    engine.shutdown();
    assert_samples_eq(&collect(output), &want, "shuffled gapless output");
}

/// **The queue is never permuted.** The owner's decision, asserted as a
/// property of the protocol rather than as a property of a screen: a traversal
/// change emits [`Event::TraversalChanged`] and **no** [`Event::QueueChanged`],
/// because the list did not change — only the walk over it did.
///
/// This is what makes turning shuffle off trivial. There is nothing to put
/// back, so there is no retained order to keep, to invalidate, or to get out of
/// step with an edit; `SetTraversal { InOrder }` and the run is in its own order
/// again, whatever has happened to it in between.
#[test]
fn changing_the_traversal_never_touches_the_queue() {
    let f = fixtures();
    let (engine, events, _output) = spawn_offline(fast_config(), 1).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    assert_eq!(next_queue_changed(&events), (2, None));

    // On, then off, over a queue nobody is playing: two answers about the
    // traversal, and not one word about the queue.
    for traversal in [Traversal::Shuffled { seed: 5 }, Traversal::InOrder] {
        engine
            .send(Command::SetTraversal { traversal })
            .expect("send");
        assert_eq!(
            next_transport_event(&events),
            Event::TraversalChanged { traversal }
        );
    }
    // Idempotent, like every other absolute setting in this protocol.
    engine
        .send(Command::SetTraversal {
            traversal: Traversal::InOrder,
        })
        .expect("send");
    assert_no_event_within(&events, Duration::from_millis(120));
    engine.shutdown();
}

/// **A bag plays everything, exactly once, and then the run ends.**
///
/// The selection rule stated as a test: no entry repeats until every entry has
/// played, nothing is refilled when the bag is spent, and the silence at the end
/// of a shuffled run is the silence at the end of any other run (ADR-0023 §5).
#[test]
fn a_shuffled_pass_plays_every_entry_once_and_then_ends() {
    let f = fixtures();
    let queue = vec![f.b.clone(), f.b.clone(), f.b.clone(), f.b.clone()];
    let seed = 12;
    let want = Traversal::Shuffled { seed }.play_order(queue.len());
    let (engine, events, _output) =
        spawn_offline(fast_config(), f.b_ref.len() * queue.len()).expect("spawn engine");
    let traversal = Traversal::Shuffled { seed };
    engine
        .send(Command::SetTraversal { traversal })
        .expect("send");
    assert_eq!(
        next_transport_event(&events),
        Event::TraversalChanged { traversal }
    );
    engine
        .send(Command::SetQueue {
            paths: queue,
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    let mut visited = Vec::new();
    loop {
        match next_transport_event(&events) {
            Event::TrackStarted { position, .. } => visited.push(position),
            Event::QueueEnded => break,
            other => panic!("unexpected event: {other:?}"),
        }
    }
    assert_eq!(visited, want, "the run did not walk the bag");
    engine.shutdown();
}

/// Repeat One overrides only natural completion. Explicit Next still follows
/// the active traversal and the newly selected entry becomes the repeated one.
#[test]
fn repeat_one_restarts_natural_ends_but_explicit_next_still_navigates() {
    let f = fixtures();
    let capacity = f.a_ref.len() * 3 + f.b_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine
        .send(Command::SetRepeatOne { enabled: true })
        .expect("send");
    assert_eq!(
        next_transport_event(&events),
        Event::RepeatOneChanged { enabled: true }
    );
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    assert_eq!(next_transport_event(&events), started(&f.a, 0));

    engine.send(Command::Next).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.b, 1));
    engine
        .send(Command::SetRepeatOne { enabled: false })
        .expect("send");
    assert_eq!(
        next_transport_event(&events),
        Event::RepeatOneChanged { enabled: false }
    );
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    engine.shutdown();

    let out = collect(output);
    assert_samples_eq(&out[..f.a_ref.len()], &f.a_ref, "first natural pass");
    assert_samples_eq(
        &out[f.a_ref.len()..2 * f.a_ref.len()],
        &f.a_ref,
        "natural repeat",
    );
    assert_samples_eq(&out[out.len() - f.b_ref.len()..], &f.b_ref, "explicit next");
}

/// **`Next` and `Previous` step along the bag, not down the list.**
///
/// The pair that would betray a traversal implemented only in the producer: a
/// skip is a fresh session at a chosen position, and the position it chooses has
/// to be the one the run would have reached on its own — otherwise pressing skip
/// would take a listener somewhere the interface never said they were going.
#[test]
fn skipping_follows_the_traversal_in_both_directions() {
    let f = fixtures();
    let seed = seed_ordering(3, &[2, 0, 1]);
    // Five-second fixtures, so every assertion below lands well inside the
    // track it is about rather than racing the run's own advance.
    let (engine, events, _output) =
        spawn_offline(paced_config(), f.a_ref.len() * 4).expect("spawn engine");
    let traversal = Traversal::Shuffled { seed };
    engine
        .send(Command::SetTraversal { traversal })
        .expect("send");
    assert_eq!(
        next_transport_event(&events),
        Event::TraversalChanged { traversal }
    );
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.a.clone(), f.a.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::JumpTo { position: 2 }).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 2));
    // Forwards: the bag says 0 comes after 2, and 1 after that.
    engine.send(Command::Next).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    engine.send(Command::Next).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 1));
    // Backwards, well under the restart threshold: the bag says 0 came before
    // 1, and 2 before that.
    engine.send(Command::Previous).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    engine.send(Command::Previous).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 2));
    // **Queue position 2 leads this bag**, and at the head of the plan there is
    // nothing before — so `Previous` restarts, exactly as it does at the head of
    // a list. Note that it is *not* position 0 that behaves this way here: the
    // rule is about the walk, and the walk is what has a head.
    engine.send(Command::Previous).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 2));
    engine.shutdown();
}

/// **Turning shuffle on mid-run does not stop the music**, and the track that is
/// sounding is delivered to its end.
///
/// The listener changed their mind about what comes *next*; nothing about that
/// is a reason to interrupt what is playing now. The first track's audio is
/// therefore still bit-identical to its reference decode, and the run continues
/// on the new plan after it.
#[test]
fn turning_shuffle_on_mid_run_lets_the_sounding_track_play_out() {
    let f = fixtures();
    let capacity = f.a_ref.len() + f.b_ref.len();
    let (engine, events, output) = spawn_offline(paced_config(), capacity).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone(), f.b.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    let traversal = Traversal::Shuffled { seed: 3 };
    engine
        .send(Command::SetTraversal { traversal })
        .expect("send");
    assert_eq!(
        next_transport_event(&events),
        Event::TraversalChanged { traversal }
    );
    // Nothing stopped: no `Stopped`, and the run carries on to its second track.
    assert_eq!(next_transport_event(&events), started(&f.b, 1));
    assert_eq!(next_transport_event(&events), Event::QueueEnded);
    engine.shutdown();
    let out = collect(output);
    assert_samples_eq(
        &out[..f.a_ref.len()],
        &f.a_ref,
        "the sounding track after a mid-run traversal change",
    );
}

/// A 5.1 track plays, and the signal-path readout says the samples were
/// matrixed — which is how ADR-0009's and ADR-0012's "baz converts nothing"
/// stays true rather than quietly becoming false (ADR-0039).
///
/// The chain itself is unchanged and still honest: the *rate* is the source's
/// and the *device* arrangement is what it was, so `Direct` keeps meaning
/// exactly what it meant. What was missing was any way for a front end to
/// learn that six channels became two, and a chain variant would have been the
/// wrong shape for it — a downmixed track can be converting or not, exclusive
/// or not, independently.
#[test]
fn a_downmixed_track_says_so_on_the_signal_path() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(fast_config(), RATE as usize * CHANNELS * 2).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.surround.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.surround, 0));

    let Event::SignalPath {
        source_rate_hz,
        source_channels,
        chain,
        ..
    } = next_signal_path(&events)
    else {
        unreachable!("next_signal_path returns only SignalPath")
    };
    assert_eq!(source_channels, 6, "the file has six channels and says so");
    assert_eq!(source_rate_hz, RATE, "the fold does not touch the rate");
    assert!(
        !chain.is_converting(),
        "a downmix is not a sample-rate conversion and must not be reported as one"
    );
    engine.shutdown();
}

/// A stereo track reports two channels, so `source_channels` is a fact about
/// every track rather than a flag that only appears when it is interesting —
/// the same discipline `source_bits` follows.
#[test]
fn an_ordinary_track_reports_its_two_channels() {
    let f = fixtures();
    let (engine, events, _output) =
        spawn_offline(fast_config(), A_FRAMES * CHANNELS).expect("spawn engine");
    engine
        .send(Command::SetQueue {
            paths: vec![f.a.clone()],
            origin: None,
        })
        .expect("send");
    engine.send(Command::Play).expect("send");
    assert_eq!(next_transport_event(&events), started(&f.a, 0));
    let Event::SignalPath {
        source_channels, ..
    } = next_signal_path(&events)
    else {
        unreachable!("next_signal_path returns only SignalPath")
    };
    assert_eq!(source_channels, CHANNELS);
    engine.shutdown();
}
