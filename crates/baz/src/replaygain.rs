//! ReplayGain as the interface reads and writes it: the engine's own account
//! folded from [`Event::ReplayGainChanged`], the settings a control press asks
//! for, and the words the readout says (ADR-0013).
//!
//! # The honesty rule, again
//!
//! ADR-0013 says it in as many words: *observe `Event::ReplayGainChanged` and
//! follow it rather than your own optimistic copy, so two front ends on one
//! engine agree.* So [`ReplayGain::apply`] is the only thing in this module
//! that changes anything. Every control here is a **question**, not a
//! mutation: [`ReplayGain::with_mode`] and its siblings return the
//! [`ReplayGainSettings`] a press is asking the engine for and leave this
//! state exactly as it was. Nothing on screen moves until the engine says it
//! moved — including the case where it says something we never asked for,
//! because another front end asked or because the engine clamped a pre-amp we
//! sent past its limit.
//!
//! That is also why the command is absolute rather than a delta. The protocol
//! makes `SetReplayGain` carry the whole setting for exactly this reason, so
//! [`ReplayGain::stepped_preamp`] answers with a complete settings value —
//! *the mode and both pre-amps and the clipping flag* — rather than with a
//! nudge that could be applied twice or not at all.
//!
//! # The fidelity indicator is not here
//!
//! Deliberately, and this is the second thing ADR-0013 is emphatic about.
//! ReplayGain is a software gain; baz has **one** gain stage and one readout
//! for it, which is [`VolumePath::is_transparent`](baz_core::protocol::VolumePath::is_transparent)
//! from `Event::VolumeChanged` combined with `SignalChain::Direct` from
//! `Event::SignalPath` — the same pair
//! [`PlayerState::bit_exact`](crate::player::PlayerState::bit_exact) already
//! asks about. Switching ReplayGain on will move that readout, which is
//! correct and is reported where it always was. Nothing in this module
//! answers "is this bit-exact", because two answers to one question is how two
//! answers come to disagree.
//!
//! # Tone
//!
//! Information, never a warning — ADR-0013 §8, on the terms ADR-0009 §5 and
//! ADR-0011 §8 set. Three consequences are load-bearing enough to state:
//!
//! - **`no_tag` is a fact about a file, not a failure.** It is the *expected*
//!   reading for a library that has never been through a scanner, so
//!   [`ReplayGainReadout`] says the file carries no ReplayGain and that it is
//!   playing as stored — never "missing", never "not found".
//! - **`disabled` is a different fact and reads differently.** Off is not a
//!   gain of zero that happens to be inaudible; the engine performs no
//!   ReplayGain arithmetic at all (ADR-0013 §2), so the readout states *no
//!   number* rather than `0.00 dB`. A figure there would describe an
//!   arithmetic that is not happening.
//! - **`clipping_prevented` is a sentence, not a badge.** "reduced to keep its
//!   peak below full scale" is the difference ADR-0013 asks a front end to
//!   surface between *your +6 dB was applied* and *it was cut*. It is the same
//!   ink as the rest of the line.
//!
//! Every string this module produces is flat: no severity, no icon choice, no
//! colour, and above all no amber — the lamp is reserved for playback truth,
//! and what a gain stage is doing is not a claim about the music. The view
//! layer has no decision left to make, which is what keeps the tone out of its
//! hands.
//!
//! Everything here is pure and iced-free (ADR-0006 layer 1).

use baz_core::protocol::{Event, ReplayGainMode, ReplayGainSource};
use baz_core::replaygain::{MAX_PREAMP_CENTIDB, ReplayGainSettings};

/// One press of a pre-amp stepper, in centidecibels: half a decibel.
///
/// Chosen against hearing and against the range, the way
/// [`VOLUME_STEP`](crate::player::VOLUME_STEP) is. Around **1 dB** is the
/// smallest level change a listener reliably notices, so half of it means two
/// presses are audible and one press is a refinement rather than a jump — and
/// unlike the volume fader, a pre-amp is a number people set once against a
/// figure they have in mind, so landing *on* it matters more than sweeping.
///
/// It also divides [`MAX_PREAMP_CENTIDB`] exactly (40 presses reach ±20 dB),
/// so the ends of the travel are reachable by pressing rather than by
/// arriving near them — the same property ADR-0011 §step chose 40 positions
/// for, for the same reason.
pub const PREAMP_STEP_CENTIDB: i16 = 50;

/// The three modes, in the order the control offers them.
///
/// Off first because it is the default and the one that changes nothing
/// (ADR-0013 §2), then the two that do, narrowest scope first. The array is
/// the single source of that order: the segmented control iterates it, and so
/// does the test that pins it.
pub const MODES: [ReplayGainMode; 3] = [
    ReplayGainMode::Off,
    ReplayGainMode::Track,
    ReplayGainMode::Album,
];

/// What the engine last said about ReplayGain: the confirmed settings, and
/// what they resolved to for the track playing now.
///
/// Kept whole rather than reduced to a number on arrival, for
/// [`SignalPath`](crate::player::SignalPath)'s reason: *which figure* the gain
/// came from is what the readout can actually explain, and "the file has no
/// ReplayGain", "ReplayGain is off" and "album mode found only a track gain"
/// are three different facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayGain {
    settings: ReplayGainSettings,
    source: ReplayGainSource,
    applied_centidb: i16,
    clipping_prevented: bool,
}

impl Default for ReplayGain {
    /// A freshly spawned engine is off, at no pre-amp, with clipping
    /// prevention armed — so these are the right values before anybody has
    /// been asked (ADR-0013 §2 and §5). [`ReplayGain::seed`] replaces them
    /// with the engine's own reading at start-up all the same.
    fn default() -> Self {
        Self {
            settings: ReplayGainSettings::default(),
            source: ReplayGainSource::Disabled,
            applied_centidb: 0,
            clipping_prevented: false,
        }
    }
}

impl ReplayGain {
    /// Seed from [`EngineHandle::replay_gain`](baz_core::engine::EngineHandle::replay_gain)
    /// at start-up — the pull-side snapshot ADR-0013 provides for exactly this
    /// moment, so the controls are right on the first frame instead of on the
    /// first change.
    ///
    /// Takes the four facts rather than the
    /// [`ReplayGainState`](baz_core::replaygain::ReplayGainState) carrying
    /// them, for the reason
    /// [`PlayerState::seed_volume`](crate::player::PlayerState::seed_volume)
    /// does: that type is `#[non_exhaustive]`, so nothing outside `baz-core` —
    /// a unit test here included — can build one, and a seeding path no test
    /// can exercise is a seeding path nobody has checked.
    pub fn seed(
        &mut self,
        settings: ReplayGainSettings,
        source: ReplayGainSource,
        applied_centidb: i16,
        clipping_prevented: bool,
    ) {
        self.settings = settings;
        self.source = source;
        self.applied_centidb = applied_centidb;
        self.clipping_prevented = clipping_prevented;
    }

    /// Fold one engine event. Only [`Event::ReplayGainChanged`] moves
    /// anything, and it moves all of it: the event carries the whole state, so
    /// each arrival simply replaces what was known.
    ///
    /// The settings half is echoed back by the engine *as it clamped it*,
    /// which is why it is taken from the event rather than kept from the
    /// command — a pre-amp we sent past ±20 dB comes back as ±20 dB, and the
    /// control should show what is in force.
    pub fn apply(&mut self, event: &Event) {
        let Event::ReplayGainChanged {
            mode,
            preamp_centidb,
            no_tag_preamp_centidb,
            prevent_clipping,
            source,
            applied_centidb,
            clipping_prevented,
        } = event
        else {
            return;
        };
        self.settings = ReplayGainSettings::new(
            *mode,
            *preamp_centidb,
            *no_tag_preamp_centidb,
            *prevent_clipping,
        );
        self.source = *source;
        self.applied_centidb = *applied_centidb;
        self.clipping_prevented = *clipping_prevented;
    }

    /// The settings the engine has confirmed — what to persist, and what a
    /// control renders itself from.
    #[must_use]
    pub fn settings(self) -> ReplayGainSettings {
        self.settings
    }

    /// Which mode is in force.
    #[must_use]
    pub fn mode(self) -> ReplayGainMode {
        self.settings.mode
    }

    /// Whether clipping prevention is armed.
    #[must_use]
    pub fn prevent_clipping(self) -> bool {
        self.settings.prevent_clipping
    }

    /// The settings a press on `mode` is asking for.
    #[must_use]
    pub fn with_mode(self, mode: ReplayGainMode) -> ReplayGainSettings {
        ReplayGainSettings {
            mode,
            ..self.settings
        }
    }

    /// The settings a press on the clipping-prevention control is asking for.
    #[must_use]
    pub fn with_prevent_clipping(self, prevent_clipping: bool) -> ReplayGainSettings {
        ReplayGainSettings {
            prevent_clipping,
            ..self.settings
        }
    }

    /// The settings `steps` presses of the tagged-file pre-amp stepper are
    /// asking for; negative goes down.
    #[must_use]
    pub fn stepped_preamp(self, steps: i32) -> ReplayGainSettings {
        ReplayGainSettings {
            preamp_centidb: step(self.settings.preamp_centidb, steps),
            ..self.settings
        }
    }

    /// The settings `steps` presses of the untagged-file pre-amp stepper are
    /// asking for; negative goes down.
    #[must_use]
    pub fn stepped_no_tag_preamp(self, steps: i32) -> ReplayGainSettings {
        ReplayGainSettings {
            no_tag_preamp_centidb: step(self.settings.no_tag_preamp_centidb, steps),
            ..self.settings
        }
    }

    /// The tagged-file pre-amp, ready to render.
    #[must_use]
    pub fn preamp_label(self) -> String {
        format_centidb(self.settings.preamp_centidb)
    }

    /// The untagged-file pre-amp, ready to render.
    #[must_use]
    pub fn no_tag_preamp_label(self) -> String {
        format_centidb(self.settings.no_tag_preamp_centidb)
    }

    /// Whether a pre-amp stepper can move further in `steps`' direction —
    /// both pre-amps stop at ±[`MAX_PREAMP_CENTIDB`], and a control that
    /// cannot act should say so rather than absorb the press.
    #[must_use]
    pub fn preamp_can_step(self, steps: i32) -> bool {
        step(self.settings.preamp_centidb, steps) != self.settings.preamp_centidb
    }

    /// Whether the untagged-file pre-amp stepper can move further.
    #[must_use]
    pub fn no_tag_preamp_can_step(self, steps: i32) -> bool {
        step(self.settings.no_tag_preamp_centidb, steps) != self.settings.no_tag_preamp_centidb
    }

    /// What the current mode does, in one plain sentence — the line under the
    /// mode control, present in every mode so that choosing one is never a
    /// guess and switching one never changes how much room the panel needs.
    #[must_use]
    pub fn mode_note(self) -> &'static str {
        mode_note(self.settings.mode)
    }

    /// The gain in force for the track playing now, and where it came from.
    ///
    /// `playing` is whether a track is actually sounding, and it is a
    /// parameter rather than something inferred here because it is the
    /// [`PlayerState`](crate::player::PlayerState)'s fact, not this module's.
    /// With nothing playing there is nothing to report: the engine's resolved
    /// figure is then about no file at all, and rendering it would say "this
    /// file carries no ReplayGain" about a file nobody chose.
    #[must_use]
    pub fn readout(self, playing: bool) -> Option<ReplayGainReadout> {
        if self.settings.mode == ReplayGainMode::Off || !playing {
            return None;
        }
        Some(ReplayGainReadout {
            gain: format_centidb(self.applied_centidb),
            detail: self.detail(),
        })
    }

    /// The explaining half of the readout: the source, plus the clipping note
    /// when clipping prevention had to cut.
    fn detail(self) -> String {
        let source = source_phrase(self.source, self.applied_centidb);
        if self.clipping_prevented {
            return format!("{source}, reduced to keep this track's peak below full scale");
        }
        source.to_owned()
    }
}

/// The gain in force and one sentence explaining it — the settings panel's
/// "what is happening right now" line.
///
/// Two strings and nothing else: no severity, no icon choice, no colour. The
/// view has no decision left to make, which is what keeps the tone out of the
/// view layer's hands — the same shape, for the same reason, as
/// [`SignalNote`](crate::player::SignalNote).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayGainReadout {
    /// The applied gain, `-7.75 dB` — [`Event::ReplayGainChanged`]'s
    /// `applied_centidb` rendered as ADR-0013 asks, hundredths of a decibel
    /// divided by a hundred.
    pub gain: String,
    /// Where that figure came from, as a phrase that finishes the sentence
    /// the panel starts.
    pub detail: String,
}

/// What a mode does, for the line under the mode control.
///
/// Written for a listener rather than for the specification: "off" says what
/// baz will *not* do, and the two live modes say which unit of music they keep
/// intact. The fallback arm exists because
/// [`ReplayGainMode`] is `#[non_exhaustive]` — a mode this build has not heard
/// of still gets an honest sentence rather than an empty one.
#[must_use]
pub fn mode_note(mode: ReplayGainMode) -> &'static str {
    match mode {
        ReplayGainMode::Off => "baz plays every file at the level it was mastered at.",
        ReplayGainMode::Track => {
            "Every track arrives at the same loudness — what a shuffled queue wants."
        }
        ReplayGainMode::Album => {
            "The level differences within an album are kept — what an album wants."
        }
        _ => "This build does not have a description for that mode.",
    }
}

/// The label a mode's segment carries.
#[must_use]
pub fn mode_label(mode: ReplayGainMode) -> &'static str {
    match mode {
        ReplayGainMode::Off => "Off",
        ReplayGainMode::Track => "Track",
        ReplayGainMode::Album => "Album",
        _ => "Other",
    }
}

/// Where the applied gain came from, as the phrase that finishes the readout.
///
/// The whole [`ReplayGainSource`] vocabulary, mapped once (ADR-0013's "render
/// `source` as the explanation"):
///
/// - [`Track`](ReplayGainSource::Track) / [`Album`](ReplayGainSource::Album) —
///   the file's own figure, named by which one.
/// - [`TrackFallback`](ReplayGainSource::TrackFallback) — album mode over a
///   file with no album value. Said as the fact it is rather than as a
///   shortfall: a single downloaded track has no album to be relative to.
/// - [`NoTag`](ReplayGainSource::NoTag) — **the expected reading for a library
///   that has never been scanned**, and not a failure. `applied` distinguishes
///   the two shapes it takes: at the default pre-amp of zero the file is
///   playing exactly as stored, which is worth saying outright, and at a
///   non-zero one the listener's own untagged pre-amp is what is being heard.
/// - [`Disabled`](ReplayGainSource::Disabled) — off. Only reachable here if an
///   engine reports it while a mode is set, which it does not; the phrase is
///   present so the match is total rather than because it renders.
#[must_use]
pub fn source_phrase(source: ReplayGainSource, applied_centidb: i16) -> &'static str {
    match source {
        ReplayGainSource::Track => "from this track's ReplayGain tag",
        ReplayGainSource::Album => "from this album's ReplayGain tag",
        ReplayGainSource::TrackFallback => {
            "from this track's ReplayGain tag — the file declares no album figure"
        }
        ReplayGainSource::NoTag if applied_centidb == 0 => {
            "this file carries no ReplayGain, so it plays exactly as stored"
        }
        ReplayGainSource::NoTag => "from the pre-amp for files that carry no ReplayGain",
        ReplayGainSource::Disabled => "ReplayGain is off for this track",
        // `ReplayGainSource` is #[non_exhaustive]: a figure this build cannot
        // name is still a figure, and saying so beats saying nothing.
        _ => "from a ReplayGain figure this build cannot name",
    }
}

/// A gain in centidecibels as decibels: `-775` is `-7.75 dB`.
///
/// ADR-0013's "render `applied_centidb` as `applied_centidb / 100.0` dB",
/// done in integers. The division is exact in decimal — a centidecibel *is* a
/// hundredth — so there is no rounding to get wrong and no float formatter in
/// the path; the two decimals are the two the tag convention writes.
///
/// The sign is always shown for a non-zero figure, because the difference
/// between turning a track up and turning it down is the whole content of the
/// number. **Zero carries no sign**: `0.00 dB` is baz not changing the level,
/// and `+0.00 dB` would dress that up as a direction.
#[must_use]
pub fn format_centidb(centidb: i16) -> String {
    let sign = match centidb.signum() {
        1 => "+",
        -1 => "-",
        _ => "",
    };
    // `unsigned_abs` rather than `abs`, so `i16::MIN` cannot panic on a value
    // that only a broken engine could send.
    let magnitude = centidb.unsigned_abs();
    format!("{sign}{}.{:02} dB", magnitude / 100, magnitude % 100)
}

/// `centidb` moved by `steps` presses, clamped into ±[`MAX_PREAMP_CENTIDB`].
///
/// Saturating throughout: the arithmetic is in `i32` so a large `steps` cannot
/// wrap before the clamp sees it, and the clamp is the engine's own limit, so
/// a control that has run out of travel simply stops rather than sending
/// something the engine will silently clamp anyway.
fn step(centidb: i16, steps: i32) -> i16 {
    let moved =
        i32::from(centidb).saturating_add(steps.saturating_mul(i32::from(PREAMP_STEP_CENTIDB)));
    let clamped = moved.clamp(
        -i32::from(MAX_PREAMP_CENTIDB),
        i32::from(MAX_PREAMP_CENTIDB),
    );
    #[expect(
        clippy::cast_possible_truncation,
        reason = "clamped into ±MAX_PREAMP_CENTIDB, which is an i16, immediately above"
    )]
    let stepped = clamped as i16;
    stepped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changed(
        mode: ReplayGainMode,
        preamp: i16,
        no_tag: i16,
        prevent_clipping: bool,
        source: ReplayGainSource,
        applied: i16,
        clipped: bool,
    ) -> Event {
        Event::ReplayGainChanged {
            mode,
            preamp_centidb: preamp,
            no_tag_preamp_centidb: no_tag,
            prevent_clipping,
            source,
            applied_centidb: applied,
            clipping_prevented: clipped,
        }
    }

    /// A fresh front end matches a freshly spawned engine, so the controls are
    /// right before anything has been asked (ADR-0013 §2, §5).
    #[test]
    fn a_fresh_state_is_off_with_clipping_prevention_armed() {
        let state = ReplayGain::default();
        assert_eq!(state.mode(), ReplayGainMode::Off);
        assert_eq!(state.settings().preamp_centidb, 0);
        assert_eq!(state.settings().no_tag_preamp_centidb, 0);
        assert!(state.prevent_clipping());
        assert_eq!(state.settings(), ReplayGainSettings::default());
        assert_eq!(state.readout(true), None, "off states no figure");
    }

    /// The honesty rule: the state follows the event and nothing else. Asking
    /// a control what it wants must leave the state exactly where it was.
    #[test]
    fn controls_ask_and_do_not_move_anything() {
        let mut state = ReplayGain::default();
        state.apply(&changed(
            ReplayGainMode::Track,
            0,
            0,
            true,
            ReplayGainSource::Track,
            -775,
            false,
        ));
        let before = state;

        // Every control, exercised for its answer.
        assert_eq!(
            state.with_mode(ReplayGainMode::Album).mode,
            ReplayGainMode::Album
        );
        assert_eq!(state.stepped_preamp(1).preamp_centidb, PREAMP_STEP_CENTIDB);
        assert_eq!(
            state.stepped_no_tag_preamp(-2).no_tag_preamp_centidb,
            -2 * PREAMP_STEP_CENTIDB
        );
        assert!(!state.with_prevent_clipping(false).prevent_clipping);

        assert_eq!(state, before, "a control press moved the state on its own");
    }

    /// …and the whole setting travels, not the field that changed. The command
    /// is absolute, so a mode press must carry the pre-amps with it or sending
    /// it would silently reset them.
    #[test]
    fn every_control_answers_with_the_whole_setting() {
        let mut state = ReplayGain::default();
        state.apply(&changed(
            ReplayGainMode::Album,
            -350,
            250,
            false,
            ReplayGainSource::Album,
            -350,
            false,
        ));
        for asked in [
            state.with_mode(ReplayGainMode::Track),
            state.stepped_preamp(1),
            state.stepped_no_tag_preamp(1),
            state.with_prevent_clipping(true),
        ] {
            // Exactly one field differs from the confirmed settings.
            let confirmed = state.settings();
            let differences = usize::from(asked.mode != confirmed.mode)
                + usize::from(asked.preamp_centidb != confirmed.preamp_centidb)
                + usize::from(asked.no_tag_preamp_centidb != confirmed.no_tag_preamp_centidb)
                + usize::from(asked.prevent_clipping != confirmed.prevent_clipping);
            assert_eq!(
                differences, 1,
                "{asked:?} changed more than it was asked to"
            );
        }
    }

    /// The engine's echo is the truth, including a value we never asked for
    /// and a clamp applied to one we did.
    #[test]
    fn the_event_is_the_truth_even_when_it_is_not_what_was_sent() {
        let mut state = ReplayGain::default();
        // Another front end put it in album mode with a pre-amp.
        state.apply(&changed(
            ReplayGainMode::Album,
            -300,
            -500,
            false,
            ReplayGainSource::Album,
            -800,
            false,
        ));
        assert_eq!(state.mode(), ReplayGainMode::Album);
        assert_eq!(state.settings().preamp_centidb, -300);
        assert_eq!(state.settings().no_tag_preamp_centidb, -500);
        assert!(!state.prevent_clipping());

        // A pre-amp beyond the engine's limit comes back clamped, and the
        // control follows the clamp rather than the request.
        state.apply(&changed(
            ReplayGainMode::Album,
            MAX_PREAMP_CENTIDB,
            0,
            true,
            ReplayGainSource::Album,
            0,
            false,
        ));
        assert_eq!(state.settings().preamp_centidb, MAX_PREAMP_CENTIDB);
    }

    /// Any other event is not this module's business and must move nothing —
    /// `Event` is `#[non_exhaustive]`, so the fold has to be a filter.
    #[test]
    fn other_events_change_nothing() {
        let mut state = ReplayGain::default();
        state.apply(&changed(
            ReplayGainMode::Track,
            100,
            0,
            true,
            ReplayGainSource::Track,
            -500,
            false,
        ));
        let before = state;
        for event in [Event::Paused, Event::Resumed, Event::QueueEnded] {
            state.apply(&event);
            assert_eq!(state, before, "{event:?} moved ReplayGain state");
        }
    }

    #[test]
    fn seeding_takes_the_engines_reading_whole() {
        let mut state = ReplayGain::default();
        state.seed(
            ReplayGainSettings::new(ReplayGainMode::Album, -350, 100, false),
            ReplayGainSource::TrackFallback,
            -233,
            true,
        );
        assert_eq!(state.mode(), ReplayGainMode::Album);
        assert!(!state.prevent_clipping());
        let readout = state
            .readout(true)
            .expect("a mode is set and a track plays");
        assert_eq!(readout.gain, "-2.33 dB");
        assert!(readout.detail.contains("no album figure"), "{readout:?}");
        assert!(readout.detail.contains("below full scale"), "{readout:?}");
    }

    // -----------------------------------------------------------------------
    // The dB rendering
    // -----------------------------------------------------------------------

    /// Sign, magnitude and the two decimals, pinned as bytes.
    #[test]
    fn a_gain_renders_as_hundredths_of_a_decibel_with_its_sign() {
        for (centidb, rendered) in [
            (0_i16, "0.00 dB"),
            (-775, "-7.75 dB"),
            (600, "+6.00 dB"),
            (104, "+1.04 dB"),
            (233, "+2.33 dB"),
            (-500, "-5.00 dB"),
            (50, "+0.50 dB"),
            (-50, "-0.50 dB"),
            (2000, "+20.00 dB"),
            (-2000, "-20.00 dB"),
            (-9000, "-90.00 dB"),
        ] {
            assert_eq!(format_centidb(centidb), rendered, "for {centidb}");
        }
    }

    /// The two places nearest zero, where a naive `centidb / 100` loses the
    /// sign entirely and a naive remainder prints a bare minus.
    #[test]
    fn a_gain_below_a_tenth_of_a_decibel_keeps_its_sign_and_its_leading_zero() {
        assert_eq!(format_centidb(-5), "-0.05 dB");
        assert_eq!(format_centidb(5), "+0.05 dB");
        assert_eq!(format_centidb(-1), "-0.01 dB");
        assert_eq!(format_centidb(-99), "-0.99 dB");
        assert_eq!(format_centidb(-100), "-1.00 dB");
    }

    /// Zero is *not* signed: it is baz not changing the level, and a `+`
    /// would dress that up as a direction.
    #[test]
    fn zero_carries_no_sign() {
        assert_eq!(format_centidb(0), "0.00 dB");
        assert!(!format_centidb(0).contains('+'));
        assert!(!format_centidb(0).contains('-'));
    }

    /// A figure only a broken engine could send must render, not panic —
    /// `i16::MIN` has no positive counterpart, which is what `abs` trips over.
    #[test]
    fn the_extremes_of_the_type_render_rather_than_panic() {
        assert_eq!(format_centidb(i16::MIN), "-327.68 dB");
        assert_eq!(format_centidb(i16::MAX), "+327.67 dB");
    }

    /// Rendering and parsing are the same decision: every value the engine can
    /// send comes back through the string as itself.
    #[test]
    fn the_rendering_is_reversible_over_the_whole_applied_range() {
        for centidb in -9_000_i16..=2_000 {
            let rendered = format_centidb(centidb);
            let number = rendered
                .strip_suffix(" dB")
                .expect("every rendering carries the unit");
            let parsed: f64 = number.parse().expect("a bare decimal");
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the value came from an i16 a hundredth at a time"
            )]
            let back = (parsed * 100.0).round() as i16;
            assert_eq!(back, centidb, "{rendered} did not read back as {centidb}");
        }
    }

    // -----------------------------------------------------------------------
    // The source vocabulary
    // -----------------------------------------------------------------------

    /// Every source says something different, and the two that are *not*
    /// failures say so in their own words. ADR-0013: `no_tag` means "this file
    /// has no ReplayGain", which is a fact about the file, and `disabled` is a
    /// different fact that must look different.
    #[test]
    fn every_source_gets_its_own_words() {
        let phrases = [
            source_phrase(ReplayGainSource::Track, -775),
            source_phrase(ReplayGainSource::Album, -775),
            source_phrase(ReplayGainSource::TrackFallback, -775),
            source_phrase(ReplayGainSource::NoTag, 0),
            source_phrase(ReplayGainSource::NoTag, -500),
            source_phrase(ReplayGainSource::Disabled, 0),
        ];
        for (i, a) in phrases.iter().enumerate() {
            for b in &phrases[i + 1..] {
                assert_ne!(a, b, "two sources read the same");
            }
            assert!(!a.is_empty());
        }
        // Named facts, checked rather than merely distinct.
        assert!(source_phrase(ReplayGainSource::Track, 0).contains("track's"));
        assert!(source_phrase(ReplayGainSource::Album, 0).contains("album's"));
        assert!(source_phrase(ReplayGainSource::TrackFallback, 0).contains("no album figure"));
        assert!(source_phrase(ReplayGainSource::NoTag, 0).contains("carries no ReplayGain"));
        assert!(source_phrase(ReplayGainSource::NoTag, 0).contains("exactly as stored"));
        assert!(source_phrase(ReplayGainSource::Disabled, 0).contains("off"));
    }

    /// The tone rule, as a test rather than as a comment: nothing in this
    /// module's vocabulary blames a file, a device, or the listener.
    #[test]
    fn nothing_in_the_vocabulary_reads_as_a_warning() {
        let forbidden = [
            "error",
            "fail",
            "warning",
            "missing",
            "invalid",
            "unsupported",
            "degrad",
            "problem",
            "cannot be",
            "bad ",
            "!",
        ];
        let mut strings: Vec<String> = MODES
            .iter()
            .flat_map(|&mode| [mode_note(mode).to_owned(), mode_label(mode).to_owned()])
            .collect();
        for source in [
            ReplayGainSource::Track,
            ReplayGainSource::Album,
            ReplayGainSource::TrackFallback,
            ReplayGainSource::NoTag,
            ReplayGainSource::Disabled,
        ] {
            strings.push(source_phrase(source, 0).to_owned());
            strings.push(source_phrase(source, -500).to_owned());
        }
        let mut clipped = ReplayGain::default();
        clipped.apply(&changed(
            ReplayGainMode::Track,
            600,
            0,
            true,
            ReplayGainSource::Track,
            210,
            true,
        ));
        strings.push(clipped.readout(true).expect("a mode is set").detail);
        for string in strings {
            let lowered = string.to_lowercase();
            for word in forbidden {
                assert!(
                    !lowered.contains(word),
                    "{string:?} reads as a warning (contains {word:?})"
                );
            }
        }
    }

    /// The three modes, in the order the control offers them, and each with a
    /// label and a sentence of its own.
    #[test]
    fn the_mode_vocabulary_is_complete_and_ordered() {
        assert_eq!(
            MODES,
            [
                ReplayGainMode::Off,
                ReplayGainMode::Track,
                ReplayGainMode::Album
            ]
        );
        assert_eq!(MODES[0], ReplayGainMode::default(), "Off is the default");
        for (i, &mode) in MODES.iter().enumerate() {
            for &other in &MODES[i + 1..] {
                assert_ne!(mode_label(mode), mode_label(other));
                assert_ne!(mode_note(mode), mode_note(other));
            }
        }
    }

    // -----------------------------------------------------------------------
    // The readout
    // -----------------------------------------------------------------------

    /// Off states no figure at all — the engine is doing no arithmetic, and a
    /// `0.00 dB` there would describe arithmetic that is not happening
    /// (ADR-0013 §2).
    #[test]
    fn off_reads_differently_from_a_gain_of_zero() {
        let mut off = ReplayGain::default();
        off.apply(&changed(
            ReplayGainMode::Off,
            0,
            0,
            true,
            ReplayGainSource::Disabled,
            0,
            false,
        ));
        assert_eq!(off.readout(true), None);

        // An untagged file in track mode *is* a gain of zero, and says so.
        let mut untagged = ReplayGain::default();
        untagged.apply(&changed(
            ReplayGainMode::Track,
            0,
            0,
            true,
            ReplayGainSource::NoTag,
            0,
            false,
        ));
        let readout = untagged.readout(true).expect("a mode is set");
        assert_eq!(readout.gain, "0.00 dB");
        assert!(
            readout.detail.contains("carries no ReplayGain"),
            "{readout:?}"
        );
    }

    /// With nothing playing the engine's resolved figure is about no file, so
    /// the panel says nothing rather than describing a file nobody chose.
    #[test]
    fn nothing_playing_states_no_figure() {
        let mut state = ReplayGain::default();
        state.apply(&changed(
            ReplayGainMode::Track,
            0,
            0,
            true,
            ReplayGainSource::NoTag,
            0,
            false,
        ));
        assert_eq!(state.readout(false), None);
        assert!(state.readout(true).is_some());
    }

    /// Clipping prevention is surfaced as ADR-0013 asks: the number that was
    /// applied, and the reason it is not the number the tags asked for.
    #[test]
    fn a_cut_gain_says_it_was_cut_and_what_it_was_cut_to() {
        let mut state = ReplayGain::default();
        state.apply(&changed(
            ReplayGainMode::Album,
            600,
            0,
            true,
            ReplayGainSource::Album,
            104,
            true,
        ));
        let readout = state.readout(true).expect("a mode is set");
        assert_eq!(readout.gain, "+1.04 dB");
        assert!(readout.detail.contains("album's"), "{readout:?}");
        assert!(
            readout
                .detail
                .contains("reduced to keep this track's peak below full scale"),
            "{readout:?}"
        );

        // And an uncut gain says nothing about clipping at all.
        state.apply(&changed(
            ReplayGainMode::Album,
            600,
            0,
            true,
            ReplayGainSource::Album,
            600,
            false,
        ));
        let readout = state.readout(true).expect("a mode is set");
        assert!(!readout.detail.contains("reduced"), "{readout:?}");
    }

    // -----------------------------------------------------------------------
    // The steppers
    // -----------------------------------------------------------------------

    #[test]
    fn a_preamp_steps_by_half_a_decibel_and_stops_at_the_engines_limit() {
        let mut state = ReplayGain::default();
        state.apply(&changed(
            ReplayGainMode::Track,
            0,
            0,
            true,
            ReplayGainSource::Track,
            0,
            false,
        ));
        assert_eq!(state.stepped_preamp(1).preamp_centidb, 50);
        assert_eq!(state.stepped_preamp(-1).preamp_centidb, -50);
        assert_eq!(state.preamp_label(), "0.00 dB");

        // The step divides the limit exactly, so pressing lands *on* it.
        assert_eq!(
            step(0, i32::from(MAX_PREAMP_CENTIDB / PREAMP_STEP_CENTIDB)),
            MAX_PREAMP_CENTIDB
        );
        // And past it, the answer to "more than the most" is the most.
        assert_eq!(
            state.stepped_preamp(1_000).preamp_centidb,
            MAX_PREAMP_CENTIDB
        );
        assert_eq!(
            state.stepped_no_tag_preamp(-1_000).no_tag_preamp_centidb,
            -MAX_PREAMP_CENTIDB
        );
        // Including a step count that would overflow the arithmetic.
        assert_eq!(
            state.stepped_preamp(i32::MAX).preamp_centidb,
            MAX_PREAMP_CENTIDB
        );
        assert_eq!(
            state.stepped_preamp(i32::MIN).preamp_centidb,
            -MAX_PREAMP_CENTIDB
        );
    }

    /// A stepper at the end of its travel says so, rather than absorbing the
    /// press — the same rule the transport buttons follow.
    #[test]
    fn a_stepper_at_the_end_of_its_travel_reports_that_it_cannot_move() {
        let mut state = ReplayGain::default();
        state.apply(&changed(
            ReplayGainMode::Track,
            MAX_PREAMP_CENTIDB,
            -MAX_PREAMP_CENTIDB,
            true,
            ReplayGainSource::Track,
            0,
            false,
        ));
        assert!(!state.preamp_can_step(1), "already at the top");
        assert!(state.preamp_can_step(-1));
        assert!(!state.no_tag_preamp_can_step(-1), "already at the bottom");
        assert!(state.no_tag_preamp_can_step(1));
        assert_eq!(state.preamp_label(), "+20.00 dB");
        assert_eq!(state.no_tag_preamp_label(), "-20.00 dB");
    }

    /// Stepping up and back down returns to where it started, at every point
    /// of the travel — the property that makes a stepper trustworthy.
    #[test]
    fn stepping_up_and_back_down_is_the_identity_inside_the_travel() {
        let mut state = ReplayGain::default();
        let mut centidb = -MAX_PREAMP_CENTIDB;
        while centidb <= MAX_PREAMP_CENTIDB - PREAMP_STEP_CENTIDB {
            state.apply(&changed(
                ReplayGainMode::Track,
                centidb,
                0,
                true,
                ReplayGainSource::Track,
                0,
                false,
            ));
            let up = state.stepped_preamp(1).preamp_centidb;
            state.apply(&changed(
                ReplayGainMode::Track,
                up,
                0,
                true,
                ReplayGainSource::Track,
                0,
                false,
            ));
            assert_eq!(
                state.stepped_preamp(-1).preamp_centidb,
                centidb,
                "a round trip from {centidb} did not come back"
            );
            centidb += PREAMP_STEP_CENTIDB;
        }
    }
}
