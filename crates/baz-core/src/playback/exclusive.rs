//! Exclusive-mode output on Linux: baz holds an ALSA `hw:` PCM itself, behind
//! the non-default `exclusive-output` feature (ADR-0012).
//!
//! ADR-0009 made baz follow the source rate and convert nothing, and stated
//! the boundary of that claim honestly: *shared mode*. The samples leave baz
//! at the file's own rate in a lossless carrier, and what `PipeWire`, `PulseAudio`
//! or `CoreAudio` then does with them — resample to a graph rate it is holding
//! for another client, mix them with a browser tab — is outside a shared-mode
//! backend's control. [`ExclusiveSink`] removes that last hop on Linux: it
//! opens a hardware PCM directly, so there is no server in the path at all.
//!
//! ```text
//! shared:     decoder -> engine -> cpal -> sound server -> kernel -> DAC
//! exclusive:  decoder -> engine ----------> ALSA hw: ------> kernel -> DAC
//! ```
//!
//! # `hw:`, never `plughw:`
//!
//! The whole point is that nothing converts, and `plughw:` is precisely
//! libasound's "convert whatever you give me" wrapper: it will happily accept
//! a rate the hardware cannot do and resample it inside the process, silently.
//! This backend therefore opens **only** names beginning with `hw:`
//! ([`ExclusiveSink::open`] rejects anything else with a typed error) and
//! additionally calls `snd_pcm_hw_params_set_rate_resample(0)`, so even a
//! future `hw:`-shaped alias cannot pull the rate plugin into the path. When
//! the hardware has no mode for the material, the *engine* converts and
//! [`Event::SignalPath`](crate::protocol::Event::SignalPath) says so — the
//! same arrangement as shared mode, and the same refusal to do it quietly.
//!
//! # Choosing a device
//!
//! ADR-0011 established the reason this backend cannot simply open `"default"`:
//! on a `PipeWire` or `PulseAudio` desktop `"default"` *is* the sound server's
//! bridge, so opening it exclusively would open the server, not the card.
//! [`devices`] therefore enumerates real hardware — every card's playback PCMs,
//! by control interface, which works whether or not the PCM is currently in use
//! — and produces `hw:CARD,DEV` names.
//!
//! **baz never guesses which of them the listener meant.** With one enumerated
//! device it is used; with several, opening without naming one is a typed
//! error that lists them. Exclusive mode is an explicit choice, so the device
//! is an explicit choice too: a machine here offers analog, S/PDIF, four HDMI
//! sinks and a USB DAC, and picking one of those for someone is not a default,
//! it is a coin toss.
//!
//! # When the device is busy
//!
//! An exclusive open of a PCM another application holds — including the sound
//! server, which is the ordinary case on a desktop that is currently playing
//! something — fails immediately with `EBUSY`. That becomes
//! [`PlaybackError::DeviceBusy`], which names the device and travels out
//! through the same spawn result every other device failure uses. It never
//! blocks, never retries in a loop, and never panics: the whole failure mode
//! is one `snd_pcm_open` returning 16.
//!
//! # Sample format: the exact carrier the hardware offers
//!
//! Shared mode hands cpal f32 and is done (ADR-0009 §6). A `hw:` PCM takes
//! what the converter takes, which in practice is never float, so this backend
//! converts — and the conversion is chosen so that it costs nothing:
//!
//! | Device format | Scale | Exact for |
//! |---|---|---|
//! | `FLOAT_LE` | none — the samples are written through untouched | everything |
//! | `S32_LE` | ×2³¹ | every integer source of 24 bits or fewer |
//! | `S24_LE` | ×2²³ | every integer source of 24 bits or fewer |
//! | `S16_LE` | ×2¹⁵ | 16-bit and 8-bit sources |
//!
//! The exactness is arithmetic, not approximate. Symphonia normalises integer
//! PCM to f32 by dividing by a power of two (2¹⁵ for 16-bit, 2²³ for 24-bit),
//! so a decoded sample is *k*/2ⁿ for an integer *k*; multiplying by 2³¹ gives
//! *k*·2³¹⁻ⁿ, which is an integer, is within `i32`, and is produced exactly by
//! the f64 multiply this module uses. `s32_is_exact_for_every_16_bit_code` and
//! `s32_is_exact_for_every_24_bit_code` assert it over the entire code space
//! rather than on a sample of it.
//!
//! `S16_LE` is the one row where a 24-bit master loses resolution, and it is
//! reached only on hardware that offers nothing wider. That is where dither
//! would become a live question (ADR-0011 flagged it); the format ladder
//! prefers every wider option first, so on the hardware measured for ADR-0012
//! the row is never taken.
//!
//! # Threading, and why there is no callback
//!
//! There is no second thread and no second ring. `DeviceSink` needs both
//! because cpal is a pull model: a realtime callback owned by the host asks
//! for audio, so a wait-free ring has to stand between it and the engine.
//! ALSA is a push model — `snd_pcm_writei` hands frames to the kernel's DMA
//! buffer — so the engine's own pump thread feeds the device directly, and
//! **no code in this module runs on a realtime thread.** The kernel's ring
//! *is* the buffer; [`Sink::write`] never blocks longer than it takes for one
//! period of space to appear, because it asks `snd_pcm_avail_update` how much
//! room there is before writing rather than handing over a block and waiting.
//!
//! That also makes three sink operations exact instead of approximate:
//!
//! - [`Sink::discard_buffered`] is `snd_pcm_drop` + `snd_pcm_prepare`: the
//!   kernel buffer is emptied synchronously and completely. Shared mode's
//!   watermark handshake exists because only cpal's callback may advance the
//!   ring's read index; here there is no such constraint.
//! - [`Sink::drain_buffered`] polls `snd_pcm_delay` — the true count of frames
//!   between the last write and the converter — under a bounded budget, so a
//!   rate change waits for exactly the audio still owed and no longer.
//! - [`Sink::negotiate_rate`] closes the PCM and reopens it at the new rate.
//!   Exclusive means exclusive: the two streams cannot briefly coexist the way
//!   `DeviceSink`'s build-then-swap allows, so this one releases first and, if
//!   the new rate cannot be opened, reopens the old one rather than leaving
//!   the engine with a silent sink.
//!
//! # Hardware volume
//!
//! ADR-0011 built [`Sink::set_device_volume`] and found nothing correct to put
//! behind it in shared mode: the per-application controls are a float multiply
//! in someone else's process, and the hardware controls are card-wide, on a
//! card baz could not even identify. Exclusive mode answers both objections at
//! once — the card is named in [`ExclusiveDevice::pcm_name`], and baz is
//! holding its playback PCM — so this backend drives the card's mixer, and the
//! sample stream reaches the converter unscaled. See
//! [`ExclusiveSink::set_device_volume`] for what is picked and what is
//! declined.
//!
//! # Other platforms
//!
//! WASAPI exclusive mode (`AUDCLNT_SHAREMODE_EXCLUSIVE`) and `CoreAudio` hog
//! mode (`kAudioDevicePropertyHogMode`) are the equivalents, and neither is
//! built here. Both need a per-platform system dependency baz does not
//! currently take; ALSA is the one platform where the cost is a dependency
//! *line* (`alsa` 0.9.1 is already in the tree through cpal). ADR-0012 records
//! what each would involve.

use std::time::{Duration, Instant};

use alsa::card;
use alsa::ctl::{Ctl, DeviceIter};
use alsa::mixer::{MilliBel, Mixer, Selem, SelemId};
use alsa::pcm::{Access, Format, HwParams, PCM};
use alsa::{Direction, Round, ValueOr};

use super::sink::Sink;
use super::{CHANNELS, PlaybackError};

/// Frames converted per `snd_pcm_writei`. Preallocated once at open and never
/// grown — a boxed slice rather than a `Vec` so "the write path does not
/// allocate" is forbidden by the type rather than asked for by a comment
/// (`docs/ENGINEERING.md`).
///
/// 4096 frames is ~93 ms at 44.1 kHz: comfortably more than any single pump
/// chunk the engine produces (2048 frames at the app's settings), so the loop
/// below runs once per write in the ordinary case, and small enough that the
/// buffer is 32 KiB.
const SCRATCH_FRAMES: usize = 4096;

/// How long [`Sink::write`] will keep offering frames to a device that is
/// accepting none before giving up on the stream.
///
/// The module's standing rule, inherited from `device.rs`: nothing waits
/// forever on hardware that may never come back. Five seconds is far past any
/// legitimate backpressure (the whole kernel buffer is ~186 ms) and short
/// enough that a wedged device fails the stream instead of the engine.
const WRITE_STALL_BUDGET: Duration = Duration::from_secs(5);

/// How long one `snd_pcm_wait` blocks for space before the loop rechecks its
/// own budget and the failure flag.
const WAIT_MS: u32 = 200;

/// Bound on [`Sink::drain_buffered`], matching `device.rs`: ten times the
/// buffer this backend asks for, so a healthy device always finishes and a
/// stalled one cannot wedge a rate change.
const DRAIN_BUDGET: Duration = Duration::from_millis(2_000);

/// Poll interval while draining.
const DRAIN_POLL: Duration = Duration::from_millis(1);

/// Mixer elements preferred for the hardware volume, most specific first.
///
/// `PCM` is the element that attenuates the stream itself and is usually the
/// finest-grained one a card has (256 steps over 51 dB on the machine ADR-0012
/// measured, against 88 steps over 65 dB for `Master`). A USB DAC generally
/// has neither name — its single feature unit is named after the device — so
/// the search falls through to "the first playback element with a decibel
/// scale", which is what picks it up.
const PREFERRED_VOLUME_ELEMENTS: [&str; 4] = ["PCM", "Master", "Speaker", "Headphone"];

/// A hardware playback PCM baz can open exclusively.
///
/// Produced by [`devices`]. The `pcm_name` is an ALSA device string of the
/// form `hw:CARD,DEV` and is what [`ExclusiveSink::open`] takes; the rest is
/// there so a listener choosing between six outputs is choosing between names
/// they recognise rather than between numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExclusiveDevice {
    /// ALSA PCM name, always `hw:CARD,DEV` — never `plughw:` (module docs).
    pub pcm_name: String,
    /// Index of the card this PCM belongs to, and therefore the mixer
    /// (`hw:CARD`) whose attenuator the hardware volume drives.
    pub card_index: i32,
    /// The card's human name, e.g. `iFi (by AMR) HD USB Audio`.
    pub card_name: String,
    /// The PCM's own name, e.g. `ALC897 Analog` or the monitor a HDMI sink is
    /// attached to.
    pub pcm_description: String,
}

impl std::fmt::Display for ExclusiveDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({} — {})",
            self.pcm_name, self.card_name, self.pcm_description
        )
    }
}

/// Every hardware playback PCM on this machine, in card and device order.
///
/// Enumerated through each card's **control** interface rather than by opening
/// PCMs, so a device another application is currently holding still appears —
/// which is what makes "that one is busy" a thing baz can report about a device
/// the listener picked, rather than a device that mysteriously vanished from
/// the list.
///
/// The list deliberately contains no `"default"`, no `plughw:`, and no
/// `dmix`/`pulse`/`pipewire` plugin names: those are the sound server, and
/// opening the sound server exclusively is the one thing this backend exists
/// not to do (ADR-0011's measurement of what `"default"` actually is).
/// Capture-only cards contribute nothing.
///
/// Cards that cannot be opened for control are skipped rather than failing the
/// enumeration: one unreadable card must not hide the other five.
#[must_use]
pub fn devices() -> Vec<ExclusiveDevice> {
    let mut found = Vec::new();
    for card in card::Iter::new().flatten() {
        let Ok(ctl) = Ctl::from_card(&card, false) else {
            continue;
        };
        let card_name = ctl
            .card_info()
            .ok()
            .and_then(|info| info.get_name().map(ToOwned::to_owned).ok())
            .unwrap_or_else(|| format!("card {}", card.get_index()));
        for device in DeviceIter::new(&ctl) {
            let Ok(index) = u32::try_from(device) else {
                continue;
            };
            // No playback stream on this device number: capture-only, or a
            // control-only node. `pcm_info` says so without opening anything.
            let Ok(info) = ctl.pcm_info(index, 0, Direction::Playback) else {
                continue;
            };
            let pcm_description = info
                .get_name()
                .map_or_else(|_| format!("device {index}"), ToOwned::to_owned);
            found.push(ExclusiveDevice {
                pcm_name: format!("hw:{},{index}", card.get_index()),
                card_index: card.get_index(),
                card_name: card_name.clone(),
                pcm_description,
            });
        }
    }
    found
}

/// The device to open, given what the caller asked for.
///
/// `None` means "you choose", which baz will only do when there is nothing to
/// choose: exactly one enumerated device. Anything else is a typed error that
/// lists the candidates, because a player that silently picks HDMI over the
/// listener's DAC is worse than one that asks.
///
/// # Errors
///
/// [`PlaybackError::Device`] when there is no hardware playback device at all,
/// when several exist and none was named, or when the named device is not one
/// of them.
pub fn choose(requested: Option<&str>) -> Result<ExclusiveDevice, PlaybackError> {
    let available = devices();
    if let Some(name) = requested {
        return available
            .iter()
            .find(|d| d.pcm_name == name)
            .cloned()
            .ok_or_else(|| {
                PlaybackError::Device(format!(
                    "no exclusive output device named {name}; this machine has {}",
                    describe(&available)
                ))
            });
    }
    if available.len() > 1 {
        return Err(PlaybackError::Device(format!(
            "exclusive output needs a device name: this machine has {}",
            describe(&available)
        )));
    }
    available.into_iter().next().ok_or_else(|| {
        PlaybackError::Device("no hardware playback device found for exclusive output".into())
    })
}

/// Render a device list for an error message, one `hw:` name and description
/// per entry.
fn describe(devices: &[ExclusiveDevice]) -> String {
    if devices.is_empty() {
        return "none".to_string();
    }
    devices
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The sample format an open PCM was configured with, and therefore the
/// conversion [`Sink::write`] performs on the way to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeviceFormat {
    /// 32-bit float — the engine's own format, written through with no
    /// arithmetic at all. Rare on `hw:` devices.
    F32,
    /// 32-bit signed integer. Carries every integer source of 24 bits or fewer
    /// exactly (module docs).
    S32,
    /// 24-bit signed integer in a 32-bit container. Exact for the same
    /// sources.
    S24,
    /// 16-bit signed integer. The one rung of the ladder that costs a 24-bit
    /// master resolution, and it is taken only when the device offers nothing
    /// else.
    S16,
}

impl DeviceFormat {
    /// The ALSA format this maps to.
    fn alsa(self) -> Format {
        match self {
            Self::F32 => Format::float(),
            Self::S32 => Format::s32(),
            Self::S24 => Format::S24LE,
            Self::S16 => Format::s16(),
        }
    }

    /// Whether every integer PCM source baz decodes (8-, 16- and 24-bit)
    /// reaches the converter with no value changed.
    ///
    /// f32's mantissa already bounds baz's fidelity at 24 bits (ADR-0009 §6),
    /// so this is the whole of the question below that.
    #[must_use]
    pub fn is_exact_for_24_bit(self) -> bool {
        matches!(self, Self::F32 | Self::S32 | Self::S24)
    }
}

/// Formats to try, widest and most exact first (module docs).
const FORMAT_LADDER: [DeviceFormat; 4] = [
    DeviceFormat::F32,
    DeviceFormat::S32,
    DeviceFormat::S24,
    DeviceFormat::S16,
];

/// Preallocated conversion storage, sized once at open.
///
/// [`DeviceFormat::F32`] needs none: the engine's own slice is what gets
/// written, which is the one case with no arithmetic whatsoever.
enum Scratch {
    None,
    I32(Box<[i32]>),
    I16(Box<[i16]>),
}

/// The card mixer element carrying the hardware volume, once one has been
/// found (module docs, and [`ExclusiveSink::set_device_volume`]).
#[derive(Clone, Debug)]
struct HardwareVolume {
    name: String,
    index: u32,
    min_db: MilliBel,
    max_db: MilliBel,
}

/// A [`Sink`] that owns an ALSA hardware PCM outright.
///
/// Dropping it closes the device, which is also what releases it back to the
/// sound server.
pub struct ExclusiveSink {
    device: ExclusiveDevice,
    /// The open PCM. `None` only after a reopen failed outright, which is the
    /// state [`ExclusiveSink::failed`] reports.
    pcm: Option<PCM>,
    format: DeviceFormat,
    scratch: Scratch,
    rate: u32,
    buffer_frames: usize,
    period_frames: usize,
    /// The card's mixer, kept open so a volume change is a write rather than
    /// an open. `None` when the card has no usable playback attenuator.
    mixer: Option<Mixer>,
    volume: Option<HardwareVolume>,
    xruns: u64,
    failed: bool,
}

/// Hand-written because `alsa`'s `PCM` and `Mixer` are opaque FFI handles with
/// no `Debug` of their own. Every field is still represented — the three that
/// cannot print themselves appear as the fact worth knowing about them (open,
/// present, how big) rather than being quietly dropped.
impl std::fmt::Debug for ExclusiveSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExclusiveSink")
            .field("device", &self.device)
            .field("pcm", &if self.pcm.is_some() { "open" } else { "closed" })
            .field("format", &self.format)
            .field("scratch", &self.scratch.capacity())
            .field("rate", &self.rate)
            .field("buffer_frames", &self.buffer_frames)
            .field("period_frames", &self.period_frames)
            .field("mixer", &self.mixer.is_some())
            .field("volume", &self.volume)
            .field("xruns", &self.xruns)
            .field("failed", &self.failed)
            .finish()
    }
}

/// An open, configured PCM and what it was configured to.
struct Opened {
    pcm: PCM,
    format: DeviceFormat,
    rate: u32,
    buffer_frames: usize,
    period_frames: usize,
}

impl ExclusiveSink {
    /// Open `device` exclusively at `sample_rate`, with a kernel buffer of
    /// about `buffer_frames` frames.
    ///
    /// The rate is a request, exactly as in shared mode: ALSA answers with the
    /// nearest rate the hardware actually has, and
    /// [`ExclusiveSink::sample_rate`] reports what that was. Nothing is
    /// resampled here — when the answer differs, the engine converts and
    /// reports the chain as converting (`ADR-0009`), which is the same
    /// arrangement `DeviceSink` has.
    ///
    /// # Errors
    ///
    /// - [`PlaybackError::DeviceBusy`] — another application (usually the
    ///   sound server) holds this PCM. Reported, never waited on.
    /// - [`PlaybackError::Device`] — the name is not a `hw:` device, the card
    ///   offers no stereo configuration or no usable sample format, or the
    ///   configuration could not be applied.
    pub fn open(
        device: &ExclusiveDevice,
        sample_rate: u32,
        buffer_frames: usize,
    ) -> Result<Self, PlaybackError> {
        let opened = open_pcm(device, sample_rate, buffer_frames)?;
        let scratch = Scratch::for_format(opened.format);
        let (mixer, volume) = open_hardware_volume(device.card_index);
        Ok(Self {
            device: device.clone(),
            pcm: Some(opened.pcm),
            format: opened.format,
            scratch,
            rate: opened.rate,
            buffer_frames: opened.buffer_frames,
            period_frames: opened.period_frames,
            mixer,
            volume,
            xruns: 0,
            failed: false,
        })
    }

    /// The device this sink holds.
    #[must_use]
    pub fn device(&self) -> &ExclusiveDevice {
        &self.device
    }

    /// The rate the hardware is actually running at.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }

    /// The sample format the hardware is actually running in — the exactness
    /// question, answered by the device rather than assumed (module docs).
    #[must_use]
    pub fn format(&self) -> DeviceFormat {
        self.format
    }

    /// Kernel buffer size in frames, as ALSA granted it.
    #[must_use]
    pub fn buffer_frames(&self) -> usize {
        self.buffer_frames
    }

    /// Period size in frames, as ALSA granted it.
    #[must_use]
    pub fn period_frames(&self) -> usize {
        self.period_frames
    }

    /// Whether a hardware attenuator was found on this card, and which mixer
    /// element it is.
    ///
    /// `None` means the card has no playback volume with a decibel scale — the
    /// S/PDIF and HDMI outputs on the machine ADR-0012 measured are exactly
    /// that — and the engine will apply software gain and report it.
    #[must_use]
    pub fn hardware_volume_element(&self) -> Option<&str> {
        self.volume.as_ref().map(|v| v.name.as_str())
    }

    /// The attenuator's range in decibels, when there is one.
    #[must_use]
    pub fn hardware_volume_db_range(&self) -> Option<(f32, f32)> {
        self.volume
            .as_ref()
            .map(|v| (v.min_db.to_db(), v.max_db.to_db()))
    }

    /// Times the device under-ran and the stream had to be recovered.
    ///
    /// Like `DeviceSink::underrun_samples` this counts every occurrence,
    /// including the legitimate one at the end of a drained stream, so it is
    /// meaningful as a delta across a window of continuous playback.
    #[must_use]
    pub fn xruns(&self) -> u64 {
        self.xruns
    }

    /// Whether the stream has been abandoned — a reopen that could not be
    /// undone, or a device that stopped accepting frames.
    ///
    /// A failed sink writes nothing rather than blocking, so the engine keeps
    /// running and a front end sees silence rather than a hang.
    #[must_use]
    pub fn failed(&self) -> bool {
        self.failed
    }

    /// Frames handed to the device that the converter has not played yet —
    /// the audio standing between the last [`Sink::write`] and the speaker.
    ///
    /// Read straight from `snd_pcm_delay`, so a [`Sink::discard_buffered`] is
    /// visible here as an immediate drop to zero. (Immediate, not eventual:
    /// there is no callback to wait for, which is the structural difference
    /// from `DeviceSink`'s watermark.)
    #[must_use]
    pub fn queued_frames(&self) -> u64 {
        u64::try_from(self.delay_frames()).unwrap_or(0)
    }

    /// The attenuation the hardware is currently applying, in decibels, when
    /// there is a hardware volume at all.
    ///
    /// The read-back half of [`Sink::set_device_volume`]: what the element
    /// actually landed on after its own quantisation, which is what makes
    /// "the hardware took the volume" an assertion rather than a hope.
    #[must_use]
    pub fn hardware_volume_db(&self) -> Option<f32> {
        let mixer = self.mixer.as_ref()?;
        let volume = self.volume.as_ref()?;
        let selem = mixer.find_selem(&SelemId::new(&volume.name, volume.index))?;
        selem
            .get_playback_vol_db(alsa::mixer::SelemChannelId::FrontLeft)
            .ok()
            .map(MilliBel::to_db)
    }

    /// Frames still to be played, or 0 when the device cannot say.
    fn delay_frames(&self) -> i64 {
        self.pcm.as_ref().and_then(|p| p.delay().ok()).unwrap_or(0)
    }
}

/// Open and fully configure one `hw:` PCM.
fn open_pcm(
    device: &ExclusiveDevice,
    sample_rate: u32,
    buffer_frames: usize,
) -> Result<Opened, PlaybackError> {
    if !device.pcm_name.starts_with("hw:") {
        return Err(PlaybackError::Device(format!(
            "exclusive output refuses {}: only hw: devices are opened directly, because \
             plughw: and the plugin names convert without saying so (ADR-0012)",
            device.pcm_name
        )));
    }
    let pcm = PCM::new(&device.pcm_name, Direction::Playback, false).map_err(|e| {
        if e.errno() == EBUSY {
            PlaybackError::DeviceBusy {
                device: device.to_string(),
            }
        } else {
            PlaybackError::Device(format!("cannot open {} exclusively: {e}", device.pcm_name))
        }
    })?;
    let (format, rate, buffer, period) = configure(&pcm, device, sample_rate, buffer_frames)?;
    pcm.prepare()
        .map_err(|e| PlaybackError::Device(format!("cannot prepare {}: {e}", device.pcm_name)))?;
    Ok(Opened {
        pcm,
        format,
        rate,
        buffer_frames: buffer,
        period_frames: period,
    })
}

/// `EBUSY` — what `snd_pcm_open` returns when another application already
/// holds the PCM, which on a desktop is the ordinary answer.
///
/// Spelled out rather than pulled from `libc`: the value is fixed by the Linux
/// kernel ABI (`asm-generic/errno-base.h`) on every architecture, this module
/// only builds on Linux, and a whole dependency for one integer is not a
/// trade `docs/ENGINEERING.md` would make.
const EBUSY: i32 = 16;

/// Apply hardware parameters: stereo, the best format the device offers, the
/// rate nearest the one asked for, and no rate plugin anywhere.
fn configure(
    pcm: &PCM,
    device: &ExclusiveDevice,
    sample_rate: u32,
    buffer_frames: usize,
) -> Result<(DeviceFormat, u32, usize, usize), PlaybackError> {
    let name = &device.pcm_name;
    let fail =
        |what: &str, e: alsa::Error| PlaybackError::Device(format!("{name}: cannot {what}: {e}"));
    let hwp = HwParams::any(pcm).map_err(|e| fail("read hardware parameters", e))?;
    // The single most important line in this file: never let libasound put a
    // rate converter in the path. On a hw: device it would not, but saying so
    // is what makes the claim structural rather than incidental.
    hwp.set_rate_resample(false)
        .map_err(|e| fail("disable the ALSA rate plugin", e))?;
    hwp.set_access(Access::RWInterleaved)
        .map_err(|e| fail("select interleaved access", e))?;
    let format = FORMAT_LADDER
        .into_iter()
        .find(|f| hwp.test_format(f.alsa()).is_ok())
        .ok_or_else(|| {
            PlaybackError::Device(format!(
                "{name} offers none of the sample formats baz can write (float, S32, S24, S16)"
            ))
        })?;
    hwp.set_format(format.alsa())
        .map_err(|e| fail("select a sample format", e))?;
    let channels = u32::try_from(CHANNELS)
        .map_err(|_| PlaybackError::Device("channel count exceeds u32".into()))?;
    hwp.set_channels(channels).map_err(|e| {
        PlaybackError::Device(format!(
            "{name} cannot run in stereo ({e}); baz emits {CHANNELS} channels"
        ))
    })?;
    let rate = hwp
        .set_rate_near(sample_rate, ValueOr::Nearest)
        .map_err(|e| fail("set a sample rate", e))?;
    let requested = i64::try_from(buffer_frames).unwrap_or(i64::MAX);
    let buffer = hwp
        .set_buffer_size_near(requested)
        .map_err(|e| fail("size the buffer", e))?;
    // Four periods to a buffer: enough wake-ups that `write` returns promptly
    // with space available, few enough that the device is not interrupted more
    // than it needs to be.
    let period = hwp
        .set_period_size_near(buffer / 4, ValueOr::Nearest)
        .map_err(|e| fail("size a period", e))?;
    pcm.hw_params(&hwp)
        .map_err(|e| fail("apply hardware parameters", e))?;

    let swp = pcm
        .sw_params_current()
        .map_err(|e| fail("read software parameters", e))?;
    // Start as soon as one period is queued rather than waiting for the whole
    // buffer, so first audio after a start or a discard is one period away.
    swp.set_start_threshold(period)
        .map_err(|e| fail("set the start threshold", e))?;
    swp.set_avail_min(period)
        .map_err(|e| fail("set the wake-up threshold", e))?;
    pcm.sw_params(&swp)
        .map_err(|e| fail("apply software parameters", e))?;

    Ok((
        format,
        rate,
        usize::try_from(buffer).unwrap_or(0),
        usize::try_from(period).unwrap_or(0),
    ))
}

impl Scratch {
    fn for_format(format: DeviceFormat) -> Self {
        let samples = SCRATCH_FRAMES * CHANNELS;
        match format {
            DeviceFormat::F32 => Self::None,
            DeviceFormat::S32 | DeviceFormat::S24 => Self::I32(vec![0; samples].into_boxed_slice()),
            DeviceFormat::S16 => Self::I16(vec![0; samples].into_boxed_slice()),
        }
    }

    /// The largest number of interleaved samples one `writei` can carry.
    fn capacity(&self) -> usize {
        match self {
            Self::None => SCRATCH_FRAMES * CHANNELS,
            Self::I32(b) => b.len(),
            Self::I16(b) => b.len(),
        }
    }
}

/// Scale one f32 sample to an integer of `bits` bits, exactly where the input
/// is a *k*/2ⁿ produced by decoding integer PCM (module docs).
///
/// The multiply is done in f64 so that the product of a 24-bit mantissa and a
/// power of two up to 2³¹ is represented exactly; f32 would round it. Clamping
/// is symmetric about the integer range, so a full-scale negative sample maps
/// to the most negative code and a full-scale positive one to the most
/// positive, with no wrap.
#[inline]
#[allow(clippy::cast_possible_truncation)] // clamped into range immediately above
fn to_integer(sample: f32, bits: u32) -> i64 {
    let scale = f64::from(1u32 << (bits - 1));
    let max = scale - 1.0;
    let min = -scale;
    (f64::from(sample) * scale).round().clamp(min, max) as i64
}

/// Fill `dst` with `src` scaled to `bits`-bit integers in an `i32` container.
#[allow(clippy::cast_possible_truncation)] // `to_integer` clamps into i32 range
fn fill_i32(dst: &mut [i32], src: &[f32], bits: u32) {
    for (out, sample) in dst.iter_mut().zip(src) {
        *out = to_integer(*sample, bits) as i32;
    }
}

/// Fill `dst` with `src` scaled to 16-bit integers.
#[allow(clippy::cast_possible_truncation)] // `to_integer` clamps into i16 range
fn fill_i16(dst: &mut [i16], src: &[f32]) {
    for (out, sample) in dst.iter_mut().zip(src) {
        *out = to_integer(*sample, 16) as i16;
    }
}

/// Find the card's best playback attenuator, if it has one.
///
/// Returns the open mixer alongside the element, because a `Selem` borrows its
/// `Mixer` and the sink has to outlive both — so what is stored is the mixer
/// plus enough to look the element up again on each change (a volume change is
/// a pointer drag, not a realtime event).
fn open_hardware_volume(card_index: i32) -> (Option<Mixer>, Option<HardwareVolume>) {
    let Ok(mixer) = Mixer::new(&format!("hw:{card_index}"), false) else {
        return (None, None);
    };
    let mut best: Option<HardwareVolume> = None;
    let mut best_rank = usize::MAX;
    for elem in mixer.iter() {
        let Some(selem) = Selem::new(elem) else {
            continue;
        };
        if !selem.has_playback_volume() {
            continue;
        }
        let (min_db, max_db) = selem.get_playback_db_range();
        // No decibel scale means no way to ask for a known attenuation, and
        // guessing one from a raw 0..87 range is exactly the kind of assumption
        // ADR-0011 refused to make.
        if min_db.0 >= max_db.0 {
            continue;
        }
        let id = selem.get_id();
        let Ok(name) = id.get_name() else {
            continue;
        };
        let rank = PREFERRED_VOLUME_ELEMENTS
            .iter()
            .position(|p| *p == name.trim())
            .unwrap_or(PREFERRED_VOLUME_ELEMENTS.len());
        if rank < best_rank {
            best_rank = rank;
            best = Some(HardwareVolume {
                name: name.to_string(),
                index: id.get_index(),
                min_db,
                max_db,
            });
        }
    }
    (Some(mixer), best)
}

impl Sink for ExclusiveSink {
    /// Convert and hand `samples` to the device, waiting for space rather than
    /// for a callback.
    ///
    /// Runs on the engine's pump thread — not a realtime thread; this backend
    /// has none (module docs) — and never allocates: the conversion target is
    /// the boxed slice sized at open, and the loop is bounded by
    /// `WRITE_STALL_BUDGET` so a device that stops accepting frames fails the
    /// stream instead of the engine.
    fn write(&mut self, samples: &[f32]) {
        if self.failed || samples.is_empty() {
            return;
        }
        let capacity = self.scratch.capacity();
        let mut offset = 0;
        let mut deadline = Instant::now() + WRITE_STALL_BUDGET;
        while offset < samples.len() {
            let Some(pcm) = self.pcm.as_ref() else {
                return;
            };
            if Instant::now() >= deadline {
                self.failed = true;
                return;
            }
            let available = match pcm.avail_update() {
                Ok(frames) => frames,
                Err(e) => {
                    // An under-run shows up here as well as on write; recover
                    // and try again rather than treating it as fatal.
                    if pcm.try_recover(e, true).is_err() {
                        self.failed = true;
                        return;
                    }
                    self.xruns += 1;
                    continue;
                }
            };
            let room = usize::try_from(available).unwrap_or(0) * CHANNELS;
            if room == 0 {
                // Block on the device's own poll descriptors, bounded, so the
                // budget above and the failure flag are rechecked.
                if let Err(e) = pcm.wait(Some(WAIT_MS))
                    && pcm.try_recover(e, true).is_err()
                {
                    self.failed = true;
                    return;
                }
                continue;
            }
            let take = room.min(capacity).min(samples.len() - offset);
            // Whole frames only: half a frame would swap the channels of
            // everything after it. All three inputs to the `min` are multiples
            // of `CHANNELS` (the engine writes interleaved stereo, `room` is a
            // frame count scaled up, the scratch is sized in frames), so this
            // rounds nothing away in practice — and if a caller ever broke
            // that, dropping the odd tail beats desynchronising the channels
            // for the rest of the track.
            let take = take - (take % CHANNELS);
            if take == 0 {
                return;
            }
            let block = &samples[offset..offset + take];
            let written = match &mut self.scratch {
                Scratch::None => pcm.io_f32().and_then(|io| io.writei(block)),
                Scratch::I32(buf) => {
                    let bits = if self.format == DeviceFormat::S24 {
                        24
                    } else {
                        32
                    };
                    fill_i32(&mut buf[..take], block, bits);
                    pcm.io_i32().and_then(|io| io.writei(&buf[..take]))
                }
                Scratch::I16(buf) => {
                    fill_i16(&mut buf[..take], block);
                    pcm.io_i16().and_then(|io| io.writei(&buf[..take]))
                }
            };
            match written {
                Ok(0) => {} // no progress: the budget above is what bounds this
                Ok(frames) => {
                    offset += frames * CHANNELS;
                    deadline = Instant::now() + WRITE_STALL_BUDGET;
                }
                Err(e) => {
                    if pcm.try_recover(e, true).is_err() {
                        self.failed = true;
                        return;
                    }
                    self.xruns += 1;
                }
            }
        }
    }

    /// Empty the kernel buffer outright, so the next [`Sink::write`] is the
    /// next thing heard.
    ///
    /// `snd_pcm_drop` discards everything queued and `snd_pcm_prepare` puts the
    /// stream back in a startable state — synchronous, complete, and with no
    /// handshake, because unlike cpal there is no separate callback that owns
    /// the read side. This is the seek-latency mechanism `device.rs` has to
    /// build a watermark for.
    fn discard_buffered(&mut self) {
        let Some(pcm) = self.pcm.as_ref() else {
            return;
        };
        if pcm.drop().is_err() || pcm.prepare().is_err() {
            self.failed = true;
        }
    }

    /// Wait for the frames already handed over to reach the converter, so a
    /// following reopen cannot truncate them.
    ///
    /// Polls `snd_pcm_delay`, which is the actual count of frames still to be
    /// played, under `DRAIN_BUDGET`. Bounded for the reason the whole
    /// module is: a device that has stopped consuming must not wedge a rate
    /// change.
    fn drain_buffered(&mut self) {
        let end = Instant::now() + DRAIN_BUDGET;
        while self.delay_frames() > 0 {
            if self.failed || Instant::now() >= end {
                return;
            }
            std::thread::sleep(DRAIN_POLL);
        }
    }

    /// Reopen the device at `desired` Hz, returning the rate it ended up at.
    ///
    /// Same rate as now: nothing happens, and the stream is untouched — the
    /// common case, since an album is one rate.
    ///
    /// Otherwise the PCM is **released first**. `DeviceSink` can build the new
    /// stream before dropping the old one because shared mode permits the
    /// moment both exist; exclusive mode is exactly the arrangement in which it
    /// does not. If the new rate cannot be opened the old one is reopened, and
    /// only if *that* fails as well does the sink go to
    /// [`ExclusiveSink::failed`] — silence the engine keeps running through,
    /// rather than a wedge.
    fn negotiate_rate(&mut self, desired: u32) -> Option<u32> {
        if desired == self.rate {
            return Some(self.rate);
        }
        let previous = self.rate;
        self.pcm = None; // release the device before asking for it again
        for rate in [desired, previous] {
            if let Ok(opened) = open_pcm(&self.device, rate, self.buffer_frames) {
                self.scratch = Scratch::for_format(opened.format);
                self.format = opened.format;
                self.rate = opened.rate;
                self.buffer_frames = opened.buffer_frames;
                self.period_frames = opened.period_frames;
                self.pcm = Some(opened.pcm);
                return Some(self.rate);
            }
        }
        self.failed = true;
        Some(previous)
    }

    /// Put `gain` in the card's own attenuator, leaving the sample stream
    /// untouched — the ADR-0011 device-volume path, reachable at last because
    /// baz owns the card (ADR-0012).
    ///
    /// `Some(())` means the hardware took it and the engine will pass samples
    /// through unscaled. `None` is returned, deliberately, in three cases:
    ///
    /// - **The card has no playback attenuator with a decibel scale.** S/PDIF
    ///   and HDMI outputs generally do not. Software gain, reported as such.
    /// - **Unity.** There is nothing to attenuate, so the element is set to
    ///   0 dB and the answer is `None` — which makes the engine report
    ///   [`VolumePath::Unity`](crate::protocol::VolumePath::Unity), the more
    ///   precise statement of the two. Claiming `DeviceAttenuator` for "no
    ///   attenuation" would be true and useless.
    /// - **Below the attenuator's reach**, including mute. A hardware minimum
    ///   of −51 dB is not silence, and software gain reaches exactly zero, so
    ///   the honest floor is where the hardware stops.
    ///
    /// # What is and is not claimed
    ///
    /// The claim is that **the samples are not scaled** — the fidelity
    /// question. It is *not* a claim that the attenuation equals the fader
    /// position to the decibel, and the difference is worth stating rather
    /// than glossing: a mixer element's travel is quantised far more coarsely
    /// than the control's 1000 positions are (0.2 dB per step on the `PCM`
    /// element of the machine ADR-0012 measured, 0.0039 dB on that machine's
    /// USB DAC, against the control's own ~0.06 dB), and the element lands on
    /// the nearest value it has. Measured: a request for −6.0206 dB landed on
    /// −6.00 dB. Two hundredths of a decibel is a hundred times below
    /// audibility, but it is not zero, and the trade it buys — the sample
    /// stream untouched — is the one worth having.
    fn set_device_volume(&mut self, gain: f32) -> Option<()> {
        let mixer = self.mixer.as_ref()?;
        let volume = self.volume.as_ref()?;
        let selem = mixer.find_selem(&SelemId::new(&volume.name, volume.index))?;
        if gain >= 1.0 {
            // Unity: park the hardware at 0 dB and decline, so the engine
            // reports Unity rather than DeviceAttenuator (docs above).
            if selem
                .set_playback_db_all(MilliBel(0), Round::Floor)
                .is_err()
            {
                self.volume = None;
            }
            return None;
        }
        if gain <= 0.0 {
            return None; // only software gain reaches exactly zero
        }
        #[allow(clippy::cast_possible_truncation)] // millibels are small integers
        let millibels = (2_000.0 * f64::from(gain).log10()).round() as i64;
        if millibels < volume.min_db.0 {
            return None; // past the bottom of the hardware's travel
        }
        if selem
            .set_playback_db_all(MilliBel(millibels), Round::Floor)
            .is_err()
        {
            // A mixer that has started refusing writes is not one to keep
            // claiming the volume through.
            self.volume = None;
            return None;
        }
        Some(())
    }

    /// Always true: this sink exists only when a `hw:` PCM was opened
    /// exclusively, and it holds it until it is dropped.
    fn is_exclusive(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exactness claim in the module docs, asserted over the **entire**
    /// 16-bit code space rather than a sample of it: every code a 16-bit file
    /// can contain, decoded to f32 the way Symphonia decodes it (÷2¹⁵), and
    /// scaled to S32 by this module, must come out as that code shifted left
    /// by 16 — not near it.
    #[test]
    fn s32_is_exact_for_every_16_bit_code() {
        for code in i16::MIN..=i16::MAX {
            let decoded = f32::from(code) / 32_768.0;
            assert_eq!(
                to_integer(decoded, 32),
                i64::from(code) * 65_536,
                "16-bit code {code} did not survive the S32 conversion"
            );
        }
    }

    /// The same claim for 24-bit material — the depth ADR-0009 exists for.
    /// Every one of the 16 777 216 codes, decoded by ÷2²³ and scaled by 2³¹,
    /// must be exactly the code shifted left by 8.
    #[test]
    fn s32_is_exact_for_every_24_bit_code() {
        for code in -(1i32 << 23)..(1i32 << 23) {
            #[allow(clippy::cast_precision_loss)] // 24-bit codes are exact in f32
            let decoded = code as f32 / 8_388_608.0;
            assert_eq!(
                to_integer(decoded, 32),
                i64::from(code) * 256,
                "24-bit code {code} did not survive the S32 conversion"
            );
        }
    }

    /// S24 in a 32-bit container is the identity for 24-bit material, which is
    /// what makes it the second rung of the ladder rather than a compromise.
    #[test]
    fn s24_is_the_identity_for_24_bit_codes() {
        for code in [-(1i32 << 23), -1, 0, 1, (1 << 23) - 1, 12_345] {
            #[allow(clippy::cast_precision_loss)] // 24-bit codes are exact in f32
            let decoded = code as f32 / 8_388_608.0;
            assert_eq!(to_integer(decoded, 24), i64::from(code));
        }
    }

    /// Full scale must land on the end codes, not wrap past them. A decoder
    /// producing exactly ±1.0 (float PCM does) is the case that would.
    #[test]
    fn full_scale_clamps_instead_of_wrapping() {
        assert_eq!(to_integer(1.0, 32), i64::from(i32::MAX));
        assert_eq!(to_integer(-1.0, 32), i64::from(i32::MIN));
        assert_eq!(to_integer(2.0, 16), i64::from(i16::MAX));
        assert_eq!(to_integer(-2.0, 16), i64::from(i16::MIN));
    }

    /// The ladder is ordered by exactness, and the honest answer about which
    /// rungs cost a 24-bit master nothing is on the type.
    #[test]
    fn the_format_ladder_prefers_exact_carriers() {
        assert_eq!(FORMAT_LADDER[0], DeviceFormat::F32);
        assert!(FORMAT_LADDER[..3].iter().all(|f| f.is_exact_for_24_bit()));
        assert!(!DeviceFormat::S16.is_exact_for_24_bit());
    }

    /// Enumeration must never offer the sound server. `"default"`, `plughw:`
    /// and the plugin names are exactly what ADR-0011 measured to be the
    /// bridge rather than the card, and opening one of them "exclusively"
    /// would be opening `PipeWire`.
    #[test]
    fn enumeration_offers_only_hardware_pcms() {
        for device in devices() {
            assert!(
                device.pcm_name.starts_with("hw:"),
                "enumeration produced a non-hardware PCM: {device}"
            );
            assert!(
                !device.pcm_name.starts_with("plughw:"),
                "plughw: converts without saying so: {device}"
            );
        }
    }

    /// A device baz did not enumerate is refused by name, with the real list in
    /// the message — never opened on the off chance.
    #[test]
    fn an_unknown_device_name_is_refused_with_the_list() {
        let error = choose(Some("hw:99,99")).expect_err("hw:99,99 does not exist");
        let message = error.to_string();
        assert!(
            message.contains("hw:99,99"),
            "the message must name what was asked for: {message}"
        );
    }

    /// `plughw:` is the one name that would silently convert, so it is refused
    /// at the open even if someone hands it in directly.
    #[test]
    fn plughw_is_refused_even_when_handed_in_directly() {
        let device = ExclusiveDevice {
            pcm_name: "plughw:0,0".into(),
            card_index: 0,
            card_name: "test".into(),
            pcm_description: "test".into(),
        };
        let error =
            ExclusiveSink::open(&device, 44_100, 8192).expect_err("plughw: must be refused");
        assert!(
            error.to_string().contains("plughw"),
            "the refusal must say what it refused: {error}"
        );
    }
}
