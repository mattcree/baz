//! ReplayGain: reading the tags a file already carries, choosing a gain from
//! them, and refusing to clip while applying it.
//!
//! The governing decision is ADR-0013. This module is the whole of the
//! *arithmetic* — it opens no files, touches no audio and knows nothing about
//! sessions; [`crate::engine`] asks it what gain a track should be played at
//! and folds the answer into the one gain stage [`crate::volume`] owns. That
//! split is deliberate: everything here is a pure function of untrusted input,
//! which is what makes it unit-testable in a table and fuzzable without a
//! decoder.
//!
//! # Scope: the tags, and how a measurement is chosen against them
//!
//! baz honours `REPLAYGAIN_*` values that a scanner (foobar2000, `rsgain`,
//! `metaflac`, `loudgain`, …) already wrote into the files. Since ADR-0015 it
//! can also **compute** them for a file that carries none — the meter is
//! [`crate::loudness`] and the pass that drives it over a library is
//! [`crate::analysis`], neither of which is here. What *is* here is the rule
//! that chooses between the two ([`ReplayGainSettings::resolve_with`]) and the
//! type a measurement travels in ([`ComputedReplayGain`]).
//!
//! **Tags win, field by field.** A library that has never been measured
//! behaves exactly as it did before ADR-0015 existed, and a library that has
//! been measured still plays its tagged tracks at their tagged levels.
//!
//! # The units are integers, and that is a decision
//!
//! Gains are **centidecibels** (`i16`, hundredths of a dB, 0 = unity) and peaks
//! are **micro-units** (`u32`, millionths of full scale, 1 000 000 = 1.0).
//! Neither is a float, for the three reasons [`crate::protocol`]'s "Time on the
//! wire" section gives in full and [`crate::volume`] repeats for the volume
//! position:
//!
//! 1. **One canonical encoding**, so `Command::SetReplayGain`'s bytes can be
//!    pinned by `wire_format_is_stable` rather than by a float formatter.
//! 2. **[`Command`](crate::protocol::Command), [`Event`](crate::protocol::Event)
//!    and [`crate::library::TrackMeta`] keep their `Eq`.** `TrackMeta` is
//!    compared with `assert_eq!` all over the workspace and is embedded in
//!    `Album`/`Edition`; storing an `f32` in it would have deleted a working
//!    guarantee from the public API to gain nothing.
//! 3. **The resolution is free.** 0.01 dB is finer than the two decimal places
//!    the tag convention itself writes (`"-7.75 dB"`), and 1e-6 of full scale
//!    is −120 dB of peak resolution — six orders of magnitude below anything a
//!    clipping check could care about.
//!
//! # Parsing is defensive, because tags are untrusted input
//!
//! Every value here came out of a file somebody else wrote. [`parse_gain`],
//! [`parse_peak`] and [`parse_r128_gain`] are total functions: they return
//! `None` for anything they do not fully understand — empty strings, `NaN`,
//! `inf`, a gain of `1e30`, a negative peak — rather than saturating into a
//! number that would then be applied to somebody's speakers. `None` means "the
//! file did not say", which is a state the selection rules already handle.
//!
//! # What a gain is chosen from
//!
//! [`ReplayGainSettings::resolve`] is the single selection rule, stated once
//! and tested as a table. Its contract is on the method; the short version is
//! *off means off, album falls back to track, an untagged file gets the
//! no-ReplayGain preamp, and clipping prevention only ever reduces.*

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

use crate::protocol::{ReplayGainMode, ReplayGainSource};

/// Centidecibels per decibel — the gain unit's scale factor.
pub const CENTIDB_PER_DB: i16 = 100;

/// Micro-units per unit of linear amplitude — the peak unit's scale factor.
/// `1.0` (digital full scale) is [`PEAK_UNITY`].
pub const PEAK_UNITY: u32 = 1_000_000;

/// Widest gain accepted from a tag, in centidecibels: ±60 dB.
///
/// Real ReplayGain figures live inside ±30 dB (the loudest and quietest
/// masters ever pressed, against an −18 LUFS reference). Sixty is twice that
/// and still finite; a tag outside it is not a gain, it is a broken file, and
/// is read as "the file did not say".
pub const MAX_TAG_GAIN_CENTIDB: i16 = 6_000;

/// Widest peak accepted from a tag, in micro-units: 100.0 linear (+40 dB).
///
/// A peak above 1.0 is ordinary — lossy encoding overshoots, and a
/// ReplayGain 2.0 scanner records the true sample peak — but a peak of a
/// hundred times full scale is a parse error somebody else made.
pub const MAX_TAG_PEAK_MICRO: u32 = 100 * PEAK_UNITY;

/// Widest pre-amp accepted, in centidecibels: ±20 dB. Values outside clamp to
/// it (a settings dialogue is allowed to be sloppy; the engine is not).
pub const MAX_PREAMP_CENTIDB: i16 = 2_000;

/// The most gain the engine will ever apply for ReplayGain, in centidecibels:
/// +20 dB.
///
/// A total-function guard on hostile input rather than a policy: no legitimate
/// tag plus pre-amp reaches it. Genuine ReplayGain *does* amplify quiet
/// material — that is what it is for, and it is the one place baz applies gain
/// above unity (ADR-0013 amends ADR-0011's "no gain above unity", which
/// remains true of the volume control).
pub const MAX_APPLIED_CENTIDB: i16 = 2_000;

/// The most attenuation the engine will ever apply for ReplayGain, in
/// centidecibels: −90 dB, which is silence by any measure. The lower half of
/// the same guard.
pub const MIN_APPLIED_CENTIDB: i16 = -9_000;

/// What `R128_TRACK_GAIN`/`R128_ALBUM_GAIN` must be shifted by to mean the
/// same thing as `REPLAYGAIN_TRACK_GAIN`, in centidecibels: +5 dB.
///
/// The two tag families normalise to different targets. EBU R128 (and so the
/// Opus-style `R128_*` header gain) aims at **−23 LUFS**; ReplayGain 2.0 aims
/// at **−18 LUFS**. A gain that lands a track on −23 lands it on −18 with five
/// more decibels, so that is what is added when an `R128_*` value is the only
/// one a file carries. Stated as a constant rather than folded into the parser
/// so the assumption is visible and testable.
pub const R128_REFERENCE_OFFSET_CENTIDB: i16 = 5 * CENTIDB_PER_DB;

/// Which ReplayGain value a tag key names.
///
/// Public because [`field_of_key`] is how a metadata reader decides whether a
/// key is worth looking at before it pays for the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReplayGainField {
    /// `REPLAYGAIN_TRACK_GAIN` — a gain in decibels.
    TrackGain,
    /// `REPLAYGAIN_TRACK_PEAK` — a linear sample peak.
    TrackPeak,
    /// `REPLAYGAIN_ALBUM_GAIN` — a gain in decibels.
    AlbumGain,
    /// `REPLAYGAIN_ALBUM_PEAK` — a linear sample peak.
    AlbumPeak,
    /// `R128_TRACK_GAIN` — a Q7.8 fixed-point gain against the EBU R128
    /// reference (see [`R128_REFERENCE_OFFSET_CENTIDB`]).
    R128TrackGain,
    /// `R128_ALBUM_GAIN` — the album counterpart of
    /// [`Self::R128TrackGain`].
    R128AlbumGain,
}

/// The canonical spelling of every key [`field_of_key`] recognises. The list
/// is exhaustive on purpose: a key that is not on it is not ReplayGain.
const FIELD_NAMES: [(&str, ReplayGainField); 6] = [
    ("REPLAYGAIN_TRACK_GAIN", ReplayGainField::TrackGain),
    ("REPLAYGAIN_TRACK_PEAK", ReplayGainField::TrackPeak),
    ("REPLAYGAIN_ALBUM_GAIN", ReplayGainField::AlbumGain),
    ("REPLAYGAIN_ALBUM_PEAK", ReplayGainField::AlbumPeak),
    ("R128_TRACK_GAIN", ReplayGainField::R128TrackGain),
    ("R128_ALBUM_GAIN", ReplayGainField::R128AlbumGain),
];

/// Which ReplayGain value `key` names, or `None` for a key that names none.
///
/// One function covers every container baz reads, because every container
/// spells the keys the same way once two conventions are undone:
///
/// - **A namespace prefix is dropped.** MP4 carries these as freeform atoms,
///   whose full key is `----:com.apple.iTunes:replaygain_track_gain` (lofty's
///   spelling) or `com.apple.iTunes:replaygain_track_gain` (Symphonia's), and
///   Symphonia names an ID3v2 user-defined frame after its description as
///   `TXXX:REPLAYGAIN_TRACK_GAIN`. Everything up to and including the last `:`
///   is a namespace, and the name is what follows it — which covers all three
///   with one rule rather than three special cases.
/// - **Case and spacing are ignored.** Vorbis comments are conventionally
///   upper case and ID3v2 `TXXX` descriptions conventionally match, but both
///   are free text and taggers have written every casing there is; a space is
///   accepted where the convention has an underscore.
///
/// Nothing else is guessed at: a key that is not one of the six names is not a
/// ReplayGain key, and a near-miss such as `REPLAYGAIN_REFERENCE_LOUDNESS`
/// reads as `None` rather than as the nearest match.
#[must_use]
pub fn field_of_key(key: &str) -> Option<ReplayGainField> {
    // Everything before the last colon is a container's namespace, not part of
    // the name (MP4 freeform atoms). `rsplit` on a key without one yields the
    // whole key, which is the Vorbis/ID3v2 case.
    let name = key.rsplit(':').next()?;
    FIELD_NAMES
        .iter()
        .find(|(canonical, _)| name_matches(name, canonical))
        .map(|(_, field)| *field)
}

/// Whether `key` is `canonical` with ASCII case ignored and spaces read as
/// underscores. Byte-wise and allocation-free: both sides are ASCII by
/// construction (the canonical names are, and any byte that differs from one
/// fails the comparison).
fn name_matches(key: &str, canonical: &str) -> bool {
    key.len() == canonical.len()
        && key.bytes().zip(canonical.bytes()).all(|(a, b)| {
            let a = if a == b' ' {
                b'_'
            } else {
                a.to_ascii_uppercase()
            };
            a == b
        })
}

/// Parse a ReplayGain **gain** value into centidecibels.
///
/// The convention is a signed decimal followed by a `dB` unit —
/// `"-7.75 dB"` — but the unit is optional in the wild and the spacing is not
/// agreed on, so `"-7.75"`, `"-7.75dB"`, `"+2.34 DB"` and `" -7.75 dB "` all
/// parse to the same number. Anything else is `None`, including:
///
/// - a value that is not a number at all, or has trailing junk after the unit;
/// - `NaN` and `±inf`, which Rust's float parser accepts and this must not;
/// - a magnitude beyond [`MAX_TAG_GAIN_CENTIDB`].
///
/// Rounded to the nearest centidecibel, which is finer than the two decimal
/// places the convention writes.
#[must_use]
pub fn parse_gain(value: &str) -> Option<i16> {
    let text = value.trim();
    // A trailing `dB`, with or without a space before it. Stripped rather than
    // required: both spellings are common and neither is wrong.
    // `text.len()` is a count of *bytes*, so `cut` can land inside a character
    // — and a tag is arbitrary text out of a file nobody here wrote, so it
    // does. `is_char_boundary` first, or `"…ª"` panics on the slice rather
    // than declining to be a gain. Found by
    // `fuzz/fuzz_targets/replaygain_tags.rs`; pinned by
    // `a_multibyte_tail_is_not_a_unit_suffix`.
    let number = match text.len().checked_sub(2) {
        Some(cut) if text.is_char_boundary(cut) && text[cut..].eq_ignore_ascii_case("db") => {
            text[..cut].trim_end()
        }
        _ => text,
    };
    let db: f64 = number.parse().ok()?;
    if !db.is_finite() {
        return None;
    }
    let centidb = (db * f64::from(CENTIDB_PER_DB)).round();
    // The bound is checked in f64 before the cast, so the cast cannot wrap.
    if centidb.abs() > f64::from(MAX_TAG_GAIN_CENTIDB) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "range-checked against MAX_TAG_GAIN_CENTIDB immediately above"
    )]
    Some(centidb as i16)
}

/// Parse a ReplayGain **peak** value into micro-units of full scale.
///
/// Peaks are linear, not decibels: `"0.988525"` is 988 525 micro-units. A peak
/// above 1.0 is legitimate and is kept — lossy codecs overshoot, and a
/// ReplayGain 2.0 scanner records the true sample peak — up to
/// [`MAX_TAG_PEAK_MICRO`], beyond which the value is not a peak.
///
/// `None` for a negative peak, a non-finite one, a value with a unit suffix
/// (a peak has no unit, so `"1.0 dB"` is a file's mistake and not a number to
/// guess at), and anything unparseable. Rounded to the nearest micro-unit.
#[must_use]
pub fn parse_peak(value: &str) -> Option<u32> {
    let peak: f64 = value.trim().parse().ok()?;
    if !peak.is_finite() || peak < 0.0 {
        return None;
    }
    let micro = (peak * f64::from(PEAK_UNITY)).round();
    if micro > f64::from(MAX_TAG_PEAK_MICRO) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "non-negative and range-checked against MAX_TAG_PEAK_MICRO immediately above"
    )]
    Some(micro as u32)
}

/// Parse an `R128_TRACK_GAIN`/`R128_ALBUM_GAIN` value into centidecibels on
/// the **ReplayGain** reference.
///
/// The Opus-style form is a signed integer in Q7.8 fixed point — units of
/// 1/256 dB — and it normalises to −23 LUFS rather than ReplayGain's −18, so
/// [`R128_REFERENCE_OFFSET_CENTIDB`] is added on the way out. `"-2321"`
/// therefore reads as −9.07 + 5 = −4.07 dB, i.e. −407 centidecibels.
///
/// A fractional value is accepted (some taggers write one) and rounded; the
/// same finiteness and range rules as [`parse_gain`] apply, checked *after*
/// the reference shift so the number that is bounded is the number that would
/// be applied.
///
/// Opus files themselves are not in [`crate::library::AUDIO_EXTENSIONS`]
/// (Symphonia ships no Opus decoder), but the tag form turns up in Vorbis
/// comments on FLAC and Ogg Vorbis files written by R128-era tools, which is
/// why it is read here.
#[must_use]
pub fn parse_r128_gain(value: &str) -> Option<i16> {
    let q78: f64 = value.trim().parse().ok()?;
    if !q78.is_finite() {
        return None;
    }
    let centidb = (q78 * f64::from(CENTIDB_PER_DB) / 256.0).round()
        + f64::from(R128_REFERENCE_OFFSET_CENTIDB);
    if centidb.abs() > f64::from(MAX_TAG_GAIN_CENTIDB) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "range-checked against MAX_TAG_GAIN_CENTIDB immediately above"
    )]
    Some(centidb as i16)
}

/// The four ReplayGain figures a file can carry, as read from its tags.
///
/// `None` in every field is the ordinary state of an untagged library and is
/// what [`Self::is_empty`] reports; it is never a claim that a track needs no
/// gain, only that the file does not say what gain it needs.
///
/// The `R128_*` forms do not appear here: they are folded into the two gain
/// fields when they are the only values present ([`Self::from_pairs`]), so a
/// consumer never has to know which spelling a file used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ReplayGainTags {
    /// `REPLAYGAIN_TRACK_GAIN` in centidecibels — how much to change this
    /// track by so it matches the reference loudness.
    pub track_gain_centidb: Option<i16>,
    /// `REPLAYGAIN_TRACK_PEAK` in micro-units — the loudest sample in this
    /// track, linear, before any gain.
    pub track_peak_micro: Option<u32>,
    /// `REPLAYGAIN_ALBUM_GAIN` in centidecibels — the one gain that puts the
    /// whole album at the reference loudness while leaving the relative levels
    /// of its tracks exactly as the mastering engineer set them.
    pub album_gain_centidb: Option<i16>,
    /// `REPLAYGAIN_ALBUM_PEAK` in micro-units — the loudest sample anywhere in
    /// the album, which is what album mode must clip-check against so that
    /// every track of the album gets the *same* reduction.
    pub album_peak_micro: Option<u32>,
}

impl ReplayGainTags {
    /// Read ReplayGain out of a container's raw key/value pairs.
    ///
    /// The caller hands over whatever its tag reader produced, keys exactly as
    /// the file spells them; recognition, parsing and precedence all happen
    /// here so that lofty (the scanner) and Symphonia (the playback path)
    /// cannot disagree about what a file means. Pairs that are not ReplayGain,
    /// and ReplayGain pairs whose values do not parse, are ignored.
    ///
    /// # Precedence
    ///
    /// A `REPLAYGAIN_*` value wins over an `R128_*` one for the same field,
    /// whichever order they arrive in: the dB form is the convention the rest
    /// of this module is written against, and the R128 form has to be shifted
    /// onto its reference to be comparable at all. Within one family the
    /// **first** parseable value wins, so a file with two `REPLAYGAIN_TRACK_GAIN`
    /// comments is read the way every other player reads it.
    #[must_use]
    pub fn from_pairs<'a, I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str> + 'a,
        V: AsRef<str> + 'a,
    {
        let mut reader = ReplayGainReader::default();
        for (key, value) in pairs {
            reader.absorb(key.as_ref(), value.as_ref());
        }
        reader.finish()
    }

    /// Whether the file declared nothing at all — the state a library that has
    /// never been through a ReplayGain scanner is in.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
}

/// Accumulates ReplayGain key/value pairs as a tag reader walks a file, so the
/// two families can be resolved against each other once at the end rather than
/// depending on the order tags happen to appear in.
///
/// [`ReplayGainTags::from_pairs`] is this type with the loop written for you;
/// the reader itself is public for a caller that already has a filter in front
/// of it and wants to skip building a value string for keys that are not
/// ReplayGain ([`field_of_key`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct ReplayGainReader {
    track_gain: Option<i16>,
    track_peak: Option<u32>,
    album_gain: Option<i16>,
    album_peak: Option<u32>,
    r128_track: Option<i16>,
    r128_album: Option<i16>,
}

impl ReplayGainReader {
    /// Offer one raw key/value pair. Returns whether it was a ReplayGain key
    /// whose value parsed and whose field was still empty — i.e. whether it
    /// changed anything.
    pub fn absorb(&mut self, key: &str, value: &str) -> bool {
        let Some(field) = field_of_key(key) else {
            return false;
        };
        // "First parseable value wins": a slot already filled is left alone,
        // and an unparseable value never empties one.
        match field {
            ReplayGainField::TrackGain => fill(&mut self.track_gain, parse_gain(value)),
            ReplayGainField::TrackPeak => fill(&mut self.track_peak, parse_peak(value)),
            ReplayGainField::AlbumGain => fill(&mut self.album_gain, parse_gain(value)),
            ReplayGainField::AlbumPeak => fill(&mut self.album_peak, parse_peak(value)),
            ReplayGainField::R128TrackGain => fill(&mut self.r128_track, parse_r128_gain(value)),
            ReplayGainField::R128AlbumGain => fill(&mut self.r128_album, parse_r128_gain(value)),
        }
    }

    /// The resolved tags: the dB form where a file has one, the shifted R128
    /// form where it is all the file has (see [`ReplayGainTags::from_pairs`]).
    #[must_use]
    pub fn finish(self) -> ReplayGainTags {
        ReplayGainTags {
            track_gain_centidb: self.track_gain.or(self.r128_track),
            track_peak_micro: self.track_peak,
            album_gain_centidb: self.album_gain.or(self.r128_album),
            album_peak_micro: self.album_peak,
        }
    }
}

/// Write `value` into `slot` if `slot` is empty and `value` is present;
/// report whether anything moved.
fn fill<T>(slot: &mut Option<T>, value: Option<T>) -> bool {
    match (&slot, value) {
        (None, Some(value)) => {
            *slot = Some(value);
            true
        }
        _ => false,
    }
}

/// What a ReplayGain **analysis** measured for one file, and which version of
/// the file it measured (ADR-0015).
///
/// The four figures are the same four a tag carries, in the same units, which
/// is what lets [`ReplayGainSettings::resolve_with`] choose between them field
/// by field. What makes this type different from [`ReplayGainTags`] is the
/// second field: a computed figure is a claim about a file's *samples*, so it
/// stops being true the moment the file changes, and it therefore has to
/// remember which file it was.
///
/// Stored in the index in its own columns (schema v6), never mixed into the
/// tag columns — so a rescan, which knows nothing about measurements, cannot
/// overwrite one, and a listener asking where a figure came from gets the true
/// answer from the storage layer up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ComputedReplayGain {
    /// The measured figures, in the units [`ReplayGainTags`] uses.
    pub figures: ReplayGainTags,
    /// The file as it was when it was measured — the stamp the index held for
    /// it at that moment ([`FileStamp`](crate::library::FileStamp)).
    ///
    /// `None` means the measurement cannot be shown to still apply, which is
    /// the honest state for a file the filesystem would not timestamp, and it
    /// is treated as stale rather than as fresh ([`Self::figures_for`]).
    /// ADR-0010 made the same call for the scan stamp and for the same reason:
    /// `None` is never a claim of freshness.
    pub stamp: Option<crate::library::FileStamp>,
}

impl ComputedReplayGain {
    /// Whether this measurement still describes the file the index now knows,
    /// whose stamp is `current`.
    ///
    /// Both stamps must be present *and* equal. A missing stamp on either side
    /// is not a match: a measurement of a file that cannot be identified is a
    /// measurement of something, and "something" is not enough to play a gain
    /// from.
    #[must_use]
    pub fn is_fresh_for(self, current: Option<crate::library::FileStamp>) -> bool {
        matches!((self.stamp, current), (Some(a), Some(b)) if a == b)
    }

    /// The figures, if they still describe the file whose stamp is `current`,
    /// and nothing otherwise.
    ///
    /// The only accessor the selection path uses, so a stale measurement
    /// cannot reach a gain stage by anybody forgetting to check.
    #[must_use]
    pub fn figures_for(self, current: Option<crate::library::FileStamp>) -> ReplayGainTags {
        if self.is_fresh_for(current) {
            self.figures
        } else {
            ReplayGainTags::default()
        }
    }

    /// Whether nothing was measured at all.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.figures.is_empty()
    }
}

/// Where the engine looks for figures baz measured itself (ADR-0015).
///
/// The engine is given **paths and nothing else** — that is ADR-0013 §7, and it
/// is what makes a queue the library has never seen play at the right level. A
/// computed figure, unlike a tag, is not in the file, so there has to be
/// somewhere to ask; this trait is that seam, and
/// [`EngineHandle::set_computed_gains`](crate::engine::EngineHandle::set_computed_gains)
/// is how a front end plugs its library into it.
///
/// It is a trait rather than a concrete map for the reason
/// [`Sink::set_device_volume`](crate::playback::Sink::set_device_volume) is a
/// trait method rather than an unimplemented enum variant (ADR-0011 §7): the
/// branch is reachable by a test double *today*, so the engine's half of the
/// arrangement is tested before anything ships behind it.
///
/// # Contract
///
/// - **Answer without blocking.** It is consulted on the engine's control
///   thread at a track boundary, between pump iterations. It is never on the
///   realtime path — the resolved gain is folded into the single number the
///   pump reads — but a boundary is not a place to do I/O.
/// - **Answer with fresh figures only.** Staleness is the implementation's
///   business ([`ComputedReplayGain::figures_for`]); by the time a figure
///   reaches the engine it is a figure to be used.
/// - **All-`None` is a perfectly good answer** and is what an unmeasured
///   library returns for everything.
pub trait ComputedGains: std::fmt::Debug + Send + Sync {
    /// The figures baz measured for `path`, or all-`None` if it has measured
    /// none that still apply.
    fn computed(&self, path: &std::path::Path) -> ReplayGainTags;
}

/// How ReplayGain is configured: the mode, the two pre-amps, and whether to
/// stay below full scale.
///
/// This is engine state, not session state — like the volume it survives every
/// transport command — and it is the input half of
/// [`ReplayGainSettings::resolve`], the tags being the other half.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReplayGainSettings {
    /// Which of a track's figures to use, or [`ReplayGainMode::Off`].
    pub mode: ReplayGainMode,
    /// Added to whatever gain the tags asked for, in centidecibels — the
    /// listener's "and a bit louder than that" control. Clamped to
    /// ±[`MAX_PREAMP_CENTIDB`].
    pub preamp_centidb: i16,
    /// Applied instead, in centidecibels, when a file carries no usable gain
    /// at all. Clamped to ±[`MAX_PREAMP_CENTIDB`].
    ///
    /// **Zero by default, deliberately.** An untagged file is then played
    /// exactly as it is stored, so switching ReplayGain on cannot make a
    /// library baz has never scanned quieter, and an untagged track keeps
    /// ADR-0009's bit-perfect path intact (there is no gain to apply, so no
    /// arithmetic happens). foobar2000 makes the same value configurable and
    /// baz follows it there; what baz does not follow is defaulting it to
    /// anything but unity.
    pub no_tag_preamp_centidb: i16,
    /// Whether to reduce a gain that would push the file's declared peak above
    /// full scale. **On by default**; the exact rule is on
    /// [`ReplayGainSettings::resolve`].
    pub prevent_clipping: bool,
}

impl Default for ReplayGainSettings {
    /// Off, no pre-amp, clipping prevention armed.
    ///
    /// Off is the default for the reason ADR-0009 makes the bit-perfect path
    /// the default: a player that has not been told to change the samples must
    /// not change the samples. Switching ReplayGain on is a listener's
    /// decision, and it is the moment the honest readout starts saying
    /// "software gain".
    fn default() -> Self {
        Self {
            mode: ReplayGainMode::Off,
            preamp_centidb: 0,
            no_tag_preamp_centidb: 0,
            prevent_clipping: true,
        }
    }
}

impl ReplayGainSettings {
    /// Settings with both pre-amps clamped into ±[`MAX_PREAMP_CENTIDB`].
    ///
    /// Clamping rather than rejecting, for [`Volume::new`](crate::volume::Volume::new)'s
    /// reason: a front end computing a pre-amp from a dragged control will
    /// land past the end, and the honest answer to "more than the most" is the
    /// most.
    #[must_use]
    pub fn new(
        mode: ReplayGainMode,
        preamp_centidb: i16,
        no_tag_preamp_centidb: i16,
        prevent_clipping: bool,
    ) -> Self {
        Self {
            mode,
            preamp_centidb: preamp_centidb.clamp(-MAX_PREAMP_CENTIDB, MAX_PREAMP_CENTIDB),
            no_tag_preamp_centidb: no_tag_preamp_centidb
                .clamp(-MAX_PREAMP_CENTIDB, MAX_PREAMP_CENTIDB),
            prevent_clipping,
        }
    }

    /// The gain to play a track at, given what its file declares.
    ///
    /// The whole selection rule, in one place, tested as a table
    /// (`tests/replaygain.rs`). In order:
    ///
    /// 1. **[`ReplayGainMode::Off`] is off.** The answer is
    ///    [`ReplayGainDecision::UNITY`] — gain exactly `1.0`, no pre-amp, no
    ///    clip check, [`ReplayGainSource::Disabled`]. The engine then performs
    ///    no ReplayGain arithmetic whatsoever, which is what makes "off" and
    ///    "a baz without ReplayGain" the same stream bit for bit.
    /// 2. **[`ReplayGainMode::Track`] uses the track gain**, with the track
    ///    peak. It does *not* fall back to the album gain: album-relative
    ///    levels are the one thing track mode exists to remove, so supplying
    ///    them under the name "track" would be answering a different question.
    ///    A file with no track gain is handled by rule 4.
    /// 3. **[`ReplayGainMode::Album`] uses the album gain, and falls back to
    ///    the track gain** ([`ReplayGainSource::TrackFallback`]) when the file
    ///    declares no album value — a single downloaded track has no album to
    ///    be relative to, and playing it unnormalised would be worse than
    ///    playing it as its own album of one.
    ///
    ///    The **peak** follows the gain: album gain is clip-checked against the
    ///    album peak, falling back to the track peak. The album peak is the
    ///    loudest sample anywhere in the album, so it is *at least* this
    ///    track's own peak — checking against it means every track of the album
    ///    is reduced by the same amount, which is exactly the property album
    ///    mode exists to preserve. Clip-checking each track against its own
    ///    peak would reduce them by different amounts and reintroduce the
    ///    level differences the album gain was carrying.
    /// 4. **No usable gain → the no-ReplayGain pre-amp**
    ///    ([`ReplayGainSource::NoTag`], [`Self::no_tag_preamp_centidb`], zero
    ///    by default). No clip check: there is no peak to check against, and
    ///    the default of zero means an untagged file is not touched at all.
    /// 5. **Otherwise the pre-amp is added**, and the total is clamped into
    ///    [`MIN_APPLIED_CENTIDB`]`..=`[`MAX_APPLIED_CENTIDB`].
    /// 6. **Clipping prevention**, when [`Self::prevent_clipping`] is set and a
    ///    peak is known and non-zero: the applied gain is reduced to at most
    ///    `-20·log₁₀(peak)`, **rounded down** to a whole centidecibel so the
    ///    rounding itself cannot put the result back above full scale. It only
    ///    ever *reduces* — a peak below full scale never licenses extra gain,
    ///    because a peak is a bound on this file and not a target — and
    ///    [`ReplayGainDecision::clipping_prevented`] records when it bit.
    ///
    /// A peak of exactly zero (digital silence) is treated as no peak: there
    /// is nothing to clip, and `1/0` is not a gain.
    #[must_use]
    pub fn resolve(self, tags: ReplayGainTags) -> ReplayGainDecision {
        self.resolve_with(tags, ReplayGainTags::default())
    }

    /// [`Self::resolve`], with baz's **own measurements** as a second source
    /// of figures (ADR-0015).
    ///
    /// `computed` is what a ReplayGain analysis pass measured for this file —
    /// all-`None` for a file nothing has measured, which is what makes
    /// [`Self::resolve`] exactly this function with an empty second argument
    /// rather than a different rule that could drift from it.
    ///
    /// # Tags win, field by field
    ///
    /// For every figure the mode needs, a value the **file** carries is used in
    /// preference to one baz computed. Three reasons, in order of weight:
    ///
    /// 1. **The tag is what the listener's other software will use.** A library
    ///    is played by more than one program, and a track that is 0.3 dB
    ///    different in baz than in foobar2000 is a difference nobody asked for.
    /// 2. **The tag may encode a decision.** A scanner run with a different
    ///    reference, or a figure a user edited by hand, is a statement about
    ///    how the file should be played; baz's measurement is a statement about
    ///    what is in it. The first outranks the second.
    /// 3. **It makes the analysis pass safe to run.** Measuring a library can
    ///    never change how an already-tagged track sounds, so "analyse my
    ///    library" carries no risk of undoing work a scanner already did.
    ///
    /// Field by field rather than whole-set, because the two sets are not
    /// alternatives: a file may carry a track gain and no album gain, and an
    /// album figure baz measured is the right answer for the second without
    /// disturbing the first.
    ///
    /// # Which peak
    ///
    /// The peak follows the **gain's own origin** first, then the other origin,
    /// then the other field — so a tagged gain is clip-checked against the
    /// tagged peak where there is one, and against baz's measured peak where
    /// the file gives none. That second case is a strict improvement on
    /// ADR-0013's "no peak declared, so apply the gain in full": a peak baz
    /// measured is a fact about the same samples.
    ///
    /// The [`ReplayGainSource`] reported names the origin as well as the field
    /// — [`ReplayGainSource::ComputedTrack`] beside
    /// [`ReplayGainSource::Track`] — so "where did this figure come from" has a
    /// true answer rather than a plausible one.
    #[must_use]
    pub fn resolve_with(
        self,
        tags: ReplayGainTags,
        computed: ReplayGainTags,
    ) -> ReplayGainDecision {
        let (source, gain, peak) = match self.mode {
            ReplayGainMode::Off => return ReplayGainDecision::UNITY,
            ReplayGainMode::Track => match (tags.track_gain_centidb, computed.track_gain_centidb) {
                (Some(gain), _) => (
                    ReplayGainSource::Track,
                    Some(gain),
                    first([tags.track_peak_micro, computed.track_peak_micro]),
                ),
                (None, computed_gain) => (
                    ReplayGainSource::ComputedTrack,
                    computed_gain,
                    first([computed.track_peak_micro, tags.track_peak_micro]),
                ),
            },
            ReplayGainMode::Album => album_figure(tags, computed),
        };
        let Some(gain) = gain else {
            return ReplayGainDecision {
                source: ReplayGainSource::NoTag,
                gain_centidb: self
                    .no_tag_preamp_centidb
                    .clamp(-MAX_PREAMP_CENTIDB, MAX_PREAMP_CENTIDB),
                clipping_prevented: false,
            };
        };
        let requested = gain
            .saturating_add(self.preamp_centidb)
            .clamp(MIN_APPLIED_CENTIDB, MAX_APPLIED_CENTIDB);
        let ceiling = self
            .prevent_clipping
            .then(|| peak.and_then(headroom_centidb))
            .flatten();
        match ceiling {
            Some(ceiling) if ceiling < requested => ReplayGainDecision {
                source,
                gain_centidb: ceiling,
                clipping_prevented: true,
            },
            _ => ReplayGainDecision {
                source,
                gain_centidb: requested,
                clipping_prevented: false,
            },
        }
    }
}

/// The first present value, in the order given — the "prefer this, then that"
/// chain [`ReplayGainSettings::resolve_with`] is written in terms of.
fn first<T: Copy, const N: usize>(candidates: [Option<T>; N]) -> Option<T> {
    candidates.into_iter().flatten().next()
}

/// [`ReplayGainMode::Album`]'s half of [`ReplayGainSettings::resolve_with`]:
/// which album figure to use, from which origin, and which peak goes with it.
///
/// Split out because the four-way choice (album or track figure × tagged or
/// computed) is the one part of the selection rule that does not read as one
/// expression, and the rule is the thing this module exists to state clearly.
/// The order is: the file's album gain, then baz's measured album gain, then
/// the file's track gain, then baz's measured track gain — tags before
/// measurements at each level, and an *album* figure of either origin before a
/// track figure of either, because album mode exists to preserve the level
/// relationships inside an album and a track figure removes them.
fn album_figure(
    tags: ReplayGainTags,
    computed: ReplayGainTags,
) -> (ReplayGainSource, Option<i16>, Option<u32>) {
    // Album peaks before track peaks, own origin before the other: the album
    // peak is the loudest sample anywhere in the album, which is what makes
    // album mode reduce every track of it by the same amount (ADR-0013 §3).
    let album_peaks = |own_first: bool| {
        if own_first {
            first([
                tags.album_peak_micro,
                computed.album_peak_micro,
                tags.track_peak_micro,
                computed.track_peak_micro,
            ])
        } else {
            first([
                computed.album_peak_micro,
                tags.album_peak_micro,
                computed.track_peak_micro,
                tags.track_peak_micro,
            ])
        }
    };
    if let Some(gain) = tags.album_gain_centidb {
        return (ReplayGainSource::Album, Some(gain), album_peaks(true));
    }
    if let Some(gain) = computed.album_gain_centidb {
        return (
            ReplayGainSource::ComputedAlbum,
            Some(gain),
            album_peaks(false),
        );
    }
    // No album figure of either origin: a single downloaded track has no album
    // to be relative to, and playing it unnormalised would be worse than
    // playing it as its own album of one (ADR-0013 §3, rule 3).
    if let Some(gain) = tags.track_gain_centidb {
        return (
            ReplayGainSource::TrackFallback,
            Some(gain),
            first([
                tags.track_peak_micro,
                computed.track_peak_micro,
                tags.album_peak_micro,
                computed.album_peak_micro,
            ]),
        );
    }
    (
        ReplayGainSource::ComputedTrackFallback,
        computed.track_gain_centidb,
        first([
            computed.track_peak_micro,
            tags.track_peak_micro,
            computed.album_peak_micro,
            tags.album_peak_micro,
        ]),
    )
}

/// The largest gain, in whole centidecibels, that keeps `peak` at or below full
/// scale: `⌊-20·log₁₀(peak)⌋`.
///
/// Rounded **down** on purpose — rounding to nearest could round a limit up by
/// half a centidecibel and put the result 0.005 dB over full scale, which is
/// the one outcome the check exists to prevent. `None` for a peak of zero
/// (digital silence cannot clip) and for a limit that lands outside the applied
/// range, where the clamp in [`ReplayGainSettings::resolve`] already governs.
fn headroom_centidb(peak_micro: u32) -> Option<i16> {
    if peak_micro == 0 {
        return None;
    }
    let peak = f64::from(peak_micro) / f64::from(PEAK_UNITY);
    let limit = (-20.0 * peak.log10() * f64::from(CENTIDB_PER_DB)).floor();
    let limit = limit.clamp(
        f64::from(MIN_APPLIED_CENTIDB),
        f64::from(MAX_APPLIED_CENTIDB),
    );
    #[expect(
        clippy::cast_possible_truncation,
        reason = "clamped into the i16 applied range immediately above"
    )]
    Some(limit as i16)
}

/// What ReplayGain decided for one track: where the number came from, what it
/// is, and whether clipping prevention had to cut it.
///
/// Travels to a front end as [`Event::ReplayGainChanged`](crate::protocol::Event::ReplayGainChanged)
/// and is readable at any time through
/// [`EngineHandle::replay_gain`](crate::engine::EngineHandle::replay_gain).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReplayGainDecision {
    /// Which figure this gain came from, including the two cases where it came
    /// from no figure at all.
    pub source: ReplayGainSource,
    /// The gain actually applied, in centidecibels. Zero means unity, and
    /// unity means the engine applies no ReplayGain arithmetic.
    pub gain_centidb: i16,
    /// Whether the gain above is lower than the tags asked for because
    /// applying them in full would have exceeded full scale.
    pub clipping_prevented: bool,
}

impl ReplayGainDecision {
    /// No ReplayGain: the state [`ReplayGainMode::Off`] resolves to, and the
    /// one in which [`Self::amplitude`] is exactly `1.0`.
    pub const UNITY: Self = Self {
        source: ReplayGainSource::Disabled,
        gain_centidb: 0,
        clipping_prevented: false,
    };

    /// The linear gain this decision means.
    ///
    /// **Exactly `1.0` at zero centidecibels**, by an early return rather than
    /// by `10⁰` happening to be representable — the same structural exactness
    /// [`Volume::amplitude`](crate::volume::Volume::amplitude) provides at the
    /// top of the fader's travel, and for the same reason: the engine
    /// recognises unity with `==` and skips the multiply, so an answer that was
    /// only *nearly* one would silently scale a stream baz had promised not to
    /// touch.
    #[must_use]
    pub fn amplitude(self) -> f32 {
        if self.gain_centidb == 0 {
            return 1.0;
        }
        // centidb/100 dB, and amplitude = 10^(dB/20), so 10^(centidb/2000).
        // Computed in f64 and narrowed once, so the f32 carries one rounding
        // rather than two.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "deliberate narrowing of the final result to the sink's sample type"
        )]
        let amplitude = 10f64.powf(f64::from(self.gain_centidb) / 2000.0) as f32;
        amplitude
    }

    /// Whether this decision leaves the sample stream untouched — true exactly
    /// when the gain is zero centidecibels, whatever the reason.
    ///
    /// Named to match [`VolumePath::is_transparent`](crate::protocol::VolumePath::is_transparent),
    /// and answering the same question about the same single gain stage: baz
    /// has one, and ADR-0013 records that ReplayGain feeds it rather than
    /// standing beside it.
    #[must_use]
    pub fn is_transparent(self) -> bool {
        self.gain_centidb == 0
    }
}

impl Default for ReplayGainDecision {
    fn default() -> Self {
        Self::UNITY
    }
}

/// Everything a front end can observe about ReplayGain, as
/// [`EngineHandle::replay_gain`](crate::engine::EngineHandle::replay_gain)
/// reports it — the pull-side twin of
/// [`Event::ReplayGainChanged`](crate::protocol::Event::ReplayGainChanged).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReplayGainState {
    /// How ReplayGain is configured.
    pub settings: ReplayGainSettings,
    /// What that configuration resolved to for the track now playing (or, when
    /// nothing is playing, for a file with no tags).
    pub applied: ReplayGainDecision,
}

/// The ReplayGain state shared between the engine thread (sole writer) and any
/// [`EngineHandle`](crate::engine::EngineHandle) (reads it, from any thread).
///
/// Atomics only, exactly as [`SharedVolume`](crate::volume) is and for the same
/// reason: a status readout must never make a caller wait on the engine, and
/// the engine must never wait on a caller. The pump path does **not** read this
/// — the resolved gain is multiplied into the one number the pump already
/// loads — so nothing here is on the realtime path at all.
#[derive(Debug)]
pub(crate) struct SharedReplayGain {
    mode: AtomicU32,
    preamp: AtomicI32,
    no_tag_preamp: AtomicI32,
    prevent_clipping: AtomicBool,
    source: AtomicU32,
    applied: AtomicI32,
    clipping_prevented: AtomicBool,
}

impl Default for SharedReplayGain {
    /// [`ReplayGainSettings::default`] and [`ReplayGainDecision::UNITY`],
    /// written out rather than derived.
    ///
    /// Deriving would have zeroed every atomic, and a zeroed `prevent_clipping`
    /// is `false` — so a handle read between the engine's spawn and its first
    /// loop iteration would have reported clipping prevention off when it is
    /// on. The spawner constructs this *before* the engine thread starts, so
    /// "correct from construction" is the only version that is never briefly
    /// wrong.
    fn default() -> Self {
        let settings = ReplayGainSettings::default();
        Self {
            mode: AtomicU32::new(mode_code(settings.mode)),
            preamp: AtomicI32::new(i32::from(settings.preamp_centidb)),
            no_tag_preamp: AtomicI32::new(i32::from(settings.no_tag_preamp_centidb)),
            prevent_clipping: AtomicBool::new(settings.prevent_clipping),
            source: AtomicU32::new(source_code(ReplayGainDecision::UNITY.source)),
            applied: AtomicI32::new(i32::from(ReplayGainDecision::UNITY.gain_centidb)),
            clipping_prevented: AtomicBool::new(ReplayGainDecision::UNITY.clipping_prevented),
        }
    }
}

/// [`ReplayGainMode`] → discriminant. An explicit, exhaustive mapping rather
/// than `as` on the enum, for the reason `volume::path_code` gives: the codes
/// are private, so a new variant must fail to compile here rather than
/// silently pick a number.
const fn mode_code(mode: ReplayGainMode) -> u32 {
    match mode {
        ReplayGainMode::Off => 0,
        ReplayGainMode::Track => 1,
        ReplayGainMode::Album => 2,
    }
}

/// The inverse of [`mode_code`].
const fn mode_from_code(code: u32) -> ReplayGainMode {
    match code {
        1 => ReplayGainMode::Track,
        2 => ReplayGainMode::Album,
        _ => ReplayGainMode::Off,
    }
}

/// [`ReplayGainSource`] → discriminant; see [`mode_code`].
const fn source_code(source: ReplayGainSource) -> u32 {
    match source {
        ReplayGainSource::Disabled => 0,
        ReplayGainSource::Track => 1,
        ReplayGainSource::Album => 2,
        ReplayGainSource::TrackFallback => 3,
        ReplayGainSource::NoTag => 4,
        ReplayGainSource::ComputedTrack => 5,
        ReplayGainSource::ComputedAlbum => 6,
        ReplayGainSource::ComputedTrackFallback => 7,
    }
}

/// The inverse of [`source_code`].
const fn source_from_code(code: u32) -> ReplayGainSource {
    match code {
        1 => ReplayGainSource::Track,
        2 => ReplayGainSource::Album,
        3 => ReplayGainSource::TrackFallback,
        4 => ReplayGainSource::NoTag,
        5 => ReplayGainSource::ComputedTrack,
        6 => ReplayGainSource::ComputedAlbum,
        7 => ReplayGainSource::ComputedTrackFallback,
        _ => ReplayGainSource::Disabled,
    }
}

impl SharedReplayGain {
    /// Publish the whole state. Engine thread only.
    pub(crate) fn publish(&self, settings: ReplayGainSettings, applied: ReplayGainDecision) {
        self.mode.store(mode_code(settings.mode), Ordering::Release);
        self.preamp
            .store(i32::from(settings.preamp_centidb), Ordering::Release);
        self.no_tag_preamp
            .store(i32::from(settings.no_tag_preamp_centidb), Ordering::Release);
        self.prevent_clipping
            .store(settings.prevent_clipping, Ordering::Release);
        self.source
            .store(source_code(applied.source), Ordering::Release);
        self.applied
            .store(i32::from(applied.gain_centidb), Ordering::Release);
        self.clipping_prevented
            .store(applied.clipping_prevented, Ordering::Release);
    }

    /// The whole observable state in one read.
    ///
    /// The fields are loaded independently, so a caller racing a change can see
    /// a mode from before it and an applied gain from after. That is acceptable
    /// and is not papered over, for the reason `SharedVolume::snapshot` states:
    /// this is a status readout, [`Event::ReplayGainChanged`](crate::protocol::Event::ReplayGainChanged)
    /// is the ordered account of every change, and a torn read corrects itself
    /// on the next one.
    pub(crate) fn snapshot(&self) -> ReplayGainState {
        ReplayGainState {
            settings: ReplayGainSettings {
                mode: mode_from_code(self.mode.load(Ordering::Acquire)),
                preamp_centidb: clamp_centidb(self.preamp.load(Ordering::Acquire)),
                no_tag_preamp_centidb: clamp_centidb(self.no_tag_preamp.load(Ordering::Acquire)),
                prevent_clipping: self.prevent_clipping.load(Ordering::Acquire),
            },
            applied: ReplayGainDecision {
                source: source_from_code(self.source.load(Ordering::Acquire)),
                gain_centidb: clamp_centidb(self.applied.load(Ordering::Acquire)),
                clipping_prevented: self.clipping_prevented.load(Ordering::Acquire),
            },
        }
    }
}

/// Narrow a stored `i32` back to the `i16` the API speaks. Only ever holds
/// values this module wrote, which are `i16` by construction; the clamp makes
/// that a fact rather than an `unwrap`.
fn clamp_centidb(value: i32) -> i16 {
    i16::try_from(value).unwrap_or(if value < 0 { i16::MIN } else { i16::MAX })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unity must be *exactly* one, or the engine's `==` short circuit stops
    /// meaning what it says. This is the ReplayGain twin of
    /// `the_taper_hits_its_endpoints_exactly`.
    #[test]
    #[expect(clippy::float_cmp, reason = "exactness is the assertion")]
    fn zero_centidb_is_exactly_unity() {
        assert_eq!(ReplayGainDecision::UNITY.amplitude(), 1.0);
        assert!(ReplayGainDecision::UNITY.is_transparent());
        assert_eq!(ReplayGainDecision::default(), ReplayGainDecision::UNITY);
        let touched = ReplayGainDecision {
            source: ReplayGainSource::Track,
            gain_centidb: 0,
            clipping_prevented: false,
        };
        assert_eq!(touched.amplitude(), 1.0);
        assert!(
            touched.is_transparent(),
            "a tag that asks for no change asks for no arithmetic"
        );
    }

    /// −6.02 dB is a halving; the amplitude conversion must agree with the
    /// decibel definition to within f32's own resolution.
    #[test]
    fn the_amplitude_conversion_is_the_decibel_definition() {
        let half = ReplayGainDecision {
            source: ReplayGainSource::Track,
            gain_centidb: -602,
            clipping_prevented: false,
        };
        assert!(
            (half.amplitude() - 0.5).abs() < 1e-4,
            "{}",
            half.amplitude()
        );
        let double = ReplayGainDecision {
            source: ReplayGainSource::Track,
            gain_centidb: 602,
            clipping_prevented: false,
        };
        assert!((double.amplitude() - 2.0).abs() < 1e-3);
        assert!(!half.is_transparent());
    }

    #[test]
    fn the_shared_state_round_trips_every_field() {
        let shared = SharedReplayGain::default();
        assert_eq!(
            shared.snapshot(),
            ReplayGainState {
                // Correct from construction, not merely once the engine thread
                // has run — clipping prevention is on by default and a handle
                // read before the first loop iteration must say so.
                settings: ReplayGainSettings::default(),
                applied: ReplayGainDecision::UNITY,
            }
        );
        let settings = ReplayGainSettings::new(ReplayGainMode::Album, -150, 250, true);
        let applied = ReplayGainDecision {
            source: ReplayGainSource::TrackFallback,
            gain_centidb: -733,
            clipping_prevented: true,
        };
        shared.publish(settings, applied);
        assert_eq!(shared.snapshot(), ReplayGainState { settings, applied });
    }

    /// A tag whose last two *bytes* sit inside one character is not a gain —
    /// and finding that out must not be a panic.
    ///
    /// `parse_gain` looked for a `dB` suffix by slicing at `len() - 2`, which
    /// is a byte offset, on text that comes out of a media file and is under
    /// nobody's control. `fuzz/fuzz_targets/replaygain_tags.rs` found it in
    /// seconds and in three thousand shapes; the ones below are one per
    /// character width, plus the cases where the suffix is real and must still
    /// be honoured.
    ///
    /// It mattered beyond the parser: these tags are read during
    /// `AudioSource::open`, so a single mis-encoded `REPLAYGAIN_TRACK_GAIN` in
    /// one file's metadata was a panic on the decode thread.
    #[test]
    fn a_multibyte_tail_is_not_a_unit_suffix() {
        for value in [
            "ª",         // two bytes, and the cut lands inside it
            "ԋ",         // two bytes
            "岠",        // three bytes
            "\u{baeb5}", // four bytes
            "-7.75dԋ",
            "ԋԋԋ",
            "\u{a0}\u{a0}",
        ] {
            assert_eq!(parse_gain(value), None, "{value:?}");
            // The sibling parsers take the same text and must be as total.
            assert_eq!(parse_peak(value), None, "{value:?}");
            assert_eq!(parse_r128_gain(value), None, "{value:?}");
            let _ = field_of_key(value);
        }
        // And the suffix still works where it is really a suffix — the fix is
        // a boundary check, not a removal.
        assert_eq!(parse_gain("-7.75 dB"), Some(-775));
        assert_eq!(parse_gain("-7.75dB"), Some(-775));
        assert_eq!(parse_gain("-7.75"), Some(-775));
    }

    #[test]
    fn pre_amps_clamp_rather_than_reject() {
        let settings = ReplayGainSettings::new(ReplayGainMode::Track, i16::MAX, i16::MIN, true);
        assert_eq!(settings.preamp_centidb, MAX_PREAMP_CENTIDB);
        assert_eq!(settings.no_tag_preamp_centidb, -MAX_PREAMP_CENTIDB);
    }

    /// The floor in [`headroom_centidb`] is load-bearing: rounding to nearest
    /// would allow a limit half a centidecibel too generous, which is the one
    /// thing the check exists to stop.
    #[test]
    fn the_clipping_ceiling_never_rounds_upward() {
        for peak in [1, 500_000, 999_999, PEAK_UNITY, 1_500_000, 2_000_000] {
            let Some(limit) = headroom_centidb(peak) else {
                continue;
            };
            let gain = ReplayGainDecision {
                source: ReplayGainSource::Track,
                gain_centidb: limit,
                clipping_prevented: true,
            };
            let after = f64::from(gain.amplitude()) * f64::from(peak) / f64::from(PEAK_UNITY);
            assert!(
                after <= 1.0,
                "peak {peak} at {limit} centidB reaches {after}, which clips"
            );
        }
        // Silence has no peak to clip.
        assert_eq!(headroom_centidb(0), None);
        // Exactly full scale leaves exactly no headroom.
        assert_eq!(headroom_centidb(PEAK_UNITY), Some(0));
    }
}
