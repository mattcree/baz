//! Bounded motion: one animated scalar, told the time rather than asking for
//! it (ADR-0020).
//!
//! ADR-0006 layer 1 — pure, iced-free, unit-tested — and shaped deliberately
//! like `shelf::GridHold` was — this codebase's first answer to "a rule that
//! needs a clock", and one ADR-0022 deleted along with the reflow it deferred.
//! The pattern outlived it. The whole rule is arithmetic over an [`Instant`]
//! the caller supplies, so every one of it can be exercised without a window and
//! without waiting.
//!
//! # Why this exists at all, and why it costs nothing
//!
//! Every design document baz wrote specified hard cuts everywhere, on the
//! grounds that a transition
//!
//! > "means driving state from a `window::frames()` subscription, **which
//! > redraws whether or not anything is moving**"
//!
//! That sentence is true of an *unconditional* subscription and false of a
//! **bounded** one, and the difference inverts the conclusion. baz already
//! shipped the bounded pattern twice in `app.rs` — the grid hold's
//! `time::every` guard, and a `window::frames()` subscription dropped once
//! startup is logged — so the mechanism was never missing; the specification
//! mis-stated what it would cost and three documents inherited the error.
//! ADR-0020 reverses it, and `docs/design/04-fluidity.md` carries the
//! measurements: a bounded driver idles at **0.0 % CPU with the clock stopped**,
//! where an unconditional `frames()` idles at 60 fps forever.
//!
//! [`Tween::live`] is what makes that a **test** rather than a promise. It is a
//! boolean the subscription reads, so "no redraw while idle" is asserted in
//! `app.rs` (`the_motion_clock_is_off_until_something_moves`) instead of being
//! remembered.
//!
//! # The degrade-to-instant path
//!
//! [`Tween::go`] with [`Duration::ZERO`] — or with a target the tween has
//! already reached — settles immediately and asks for no clock. That is the same
//! code path as "motion disabled", which is why a future *reduce motion* setting
//! is one constant rather than a branch at every call site.
//!
//! # What may move
//!
//! ADR-0020 §2's five, and nothing else. Still refused, and they are refusals
//! rather than omissions (`docs/REFUSALS.md`): shelf-grid stagger or pop-in, any
//! fade as a thumbnail decodes, album-art crossfades, **the bar's geometry**,
//! and anything at all that needs a redraw while the window is idle. Motion
//! states what changed; it never decorates, and it never moves the transport.

use std::time::{Duration, Instant};

/// How often a live tween is advanced (ADR-0020, `docs/design/04-fluidity.md`
/// §1.7).
///
/// **A timer, not `window::frames()`**, even though both stop correctly when
/// the last tween settles: a gated `frames()` subscription free-runs *while it
/// is live* — 281 fps against a timer's 124 under llvmpipe — because it is the
/// redraw-produces-message loop switched off only at the ends. A timer is
/// rate-limited by construction.
///
/// 8 ms is half a frame at 60 Hz, so a tween never misses a vsync it could have
/// been drawn on; the compositor, not this number, decides how many frames
/// actually land.
pub const TICK: Duration = Duration::from_millis(8);

/// An icon button's ink fade: **90 ms** (ADR-0020 §2.1).
pub const INK: Duration = Duration::from_millis(90);
/// A shelf tile's hover rule: **90 ms** (ADR-0020 §2.3).
///
/// # Two of ADR-0020's five have no subject any more
///
/// **§2.2, the queue popover's 140 ms fade and 8 px rise**, and **§2.4, the
/// album inspector's 150 ms width**, are both deleted by ADR-0022 along with
/// the surfaces they moved: there is no popover to arrive and no column to
/// widen. Neither is *forbidden* — the ADR that permitted them is not
/// reversed — they simply have nothing left to animate, and a `Duration`
/// constant nothing reads is worse than a paragraph saying why.
///
/// A **place change is a hard cut**, and that is a decision rather than an
/// omission: the surfaces either side of a navigation share no element to move,
/// so any transition between them would be decoration, and ADR-0020 §3 forbids
/// decoration. Three of the five ship: this one, the icon button's ink, and the
/// lamp.
pub const TILE: Duration = Duration::from_millis(90);
/// The lamp warming when the light moves to another record: **200 ms**, and
/// **linear** — the one transition of the five that is not eased (ADR-0020 §2.5,
/// `docs/design/02-visual-language.md` §7 named it first).
pub const LAMP: Duration = Duration::from_millis(200);

/// How a tween gets from where it is to where it is going.
///
/// Two, and there will not be a third without an ADR: **no spring, no bounce,
/// no overshoot** (`docs/design/02-visual-language.md` §7, upheld by ADR-0020).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Curve {
    /// Constant rate. The lamp warms on this one: a filament coming up has no
    /// reason to decelerate.
    Linear,
    /// `1 − (1 − t)³` — fast away from the old value, settling into the new
    /// one. Everything a pointer causes uses this, because the hand has already
    /// arrived and the interface should look like it is catching up rather than
    /// winding up.
    #[default]
    EaseOut,
}

impl Curve {
    /// Shape a linear `t` in `[0, 1]`.
    fn shape(self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::EaseOut => {
                let inverse = 1.0 - t;
                inverse.mul_add(-(inverse * inverse), 1.0)
            }
        }
    }
}

/// A flight in progress: where it started, where it is going, and when.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Flight {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
}

/// **One animated scalar.** Settled until something asks it to move, and it
/// settles again on its own.
///
/// Told the time rather than asking for it: [`Self::go`] and [`Self::tick`] take
/// the [`Instant`], and [`Self::value`] — the one the view reads, every frame —
/// touches no clock at all. That is what makes the whole rule testable, and it
/// was the same division the grid hold made, for the same reason.
///
/// Small on purpose. `size_of::<Tween>()` is **48 bytes**
/// (`a_tween_is_forty_eight_bytes`), so twenty animated scalars are under a
/// kilobyte against a 150 MiB thumbnail budget — the memory promise ADR-0020
/// had to keep is kept by construction rather than by restraint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    /// Where it is now — what [`Self::value`] answers and what a fresh flight
    /// departs from.
    value: f32,
    /// The flight in progress, if it is moving at all. `None` is the ordinary
    /// state and the one that asks for no clock.
    flight: Option<Flight>,
    /// How it moves. Fixed for the tween's life: a scalar that eased one way
    /// and not the other would be two transitions wearing one name.
    curve: Curve,
}

impl Tween {
    /// A tween at rest on `value`, asking for nothing.
    #[must_use]
    pub const fn settled(value: f32) -> Self {
        Self {
            value,
            flight: None,
            curve: Curve::EaseOut,
        }
    }

    /// The same tween on `curve`.
    #[must_use]
    pub const fn with_curve(mut self, curve: Curve) -> Self {
        self.curve = curve;
        self
    }

    /// **Send it to `target` over `duration`, from wherever it is now.**
    ///
    /// Retargeting mid-flight departs from the value on screen rather than from
    /// the value the last flight started at, so a pointer moving in, out and in
    /// again produces two short correct movements instead of one stale one.
    ///
    /// A zero duration, or a target already reached, settles immediately and
    /// **asks for no clock** — that is the degrade-to-instant path, and it is
    /// the same code path as "motion disabled".
    pub fn go(&mut self, target: f32, duration: Duration, now: Instant) {
        if duration.is_zero() || (target - self.value).abs() < f32::EPSILON {
            self.set(target);
            return;
        }
        self.flight = Some(Flight {
            from: self.value,
            to: target,
            start: now,
            duration,
        });
    }

    /// Jump to `target` with no motion at all, cancelling any flight.
    ///
    /// What a window resize spends: dragging an edge is not a transition, and a
    /// layout tweening toward a width the pointer is still changing would chase
    /// the hand.
    pub fn set(&mut self, target: f32) {
        self.value = target;
        self.flight = None;
    }

    /// What `view` draws. **No clock contact** — the value is whatever the last
    /// [`Self::tick`] left, so a frame drawn between ticks draws the frame
    /// before it rather than a fresh reading of the wall clock.
    #[must_use]
    pub const fn value(self) -> f32 {
        self.value
    }

    /// **"Must I keep a clock?"** — the boolean the subscription reads.
    ///
    /// False whenever nothing is moving, which is the whole of ADR-0020's
    /// idle-cost claim and the reason it is a test rather than a promise.
    #[must_use]
    pub const fn live(self) -> bool {
        self.flight.is_some()
    }

    /// Advance to `now`, reporting whether the tween is *still* live.
    ///
    /// A flight that has run its duration lands exactly on its target and drops
    /// the flight, so a tick that arrives 400 ms late — a stalled scan, a
    /// suspended laptop — settles on the target rather than past it. A
    /// transition may arrive late; it may never arrive wrong.
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some(flight) = self.flight else {
            return false;
        };
        let elapsed = now.saturating_duration_since(flight.start);
        if elapsed >= flight.duration {
            self.set(flight.to);
            return false;
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a transition's elapsed fraction is a small finite ratio"
        )]
        let t = (elapsed.as_secs_f64() / flight.duration.as_secs_f64()) as f32;
        let shaped = self.curve.shape(t.clamp(0.0, 1.0));
        self.value = shaped.mul_add(flight.to - flight.from, flight.from);
        true
    }
}

/// **One tween, keyed by which thing the pointer is on** — never one per thing.
///
/// The shelf draws up to a few hundred tiles and at most one of them is hovered,
/// so a tween per tile would be a tween per *album*, allocated for a state only
/// one of them can be in (ADR-0020 §2.3 says so in as many words). This holds
/// the key and the single scalar instead.
///
/// Crossing from one key straight to another **hands the mark over** rather than
/// restarting it: the new key inherits the strength the old one had reached and
/// carries on toward 1, which is what a rule appearing to travel with the
/// pointer looks like. The alternative — snapping to 0 and easing back up — is a
/// flicker in the exact gesture the transition exists to smooth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyed<K> {
    /// Which thing the strength belongs to. `None` once a fade-out has settled.
    key: Option<K>,
    /// How strongly that thing is marked, in `[0, 1]`.
    tween: Tween,
}

impl<K: Copy + PartialEq> Keyed<K> {
    /// Nothing marked, which is where every session starts and where every
    /// fade-out returns.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            key: None,
            tween: Tween::settled(0.0),
        }
    }

    /// The pointer arrived on `key`: mark it, over `duration`.
    pub fn enter(&mut self, key: K, duration: Duration, now: Instant) {
        if self.key != Some(key) {
            self.key = Some(key);
            // Deliberately *not* a reset to 0: see the type's note on handing
            // the mark over.
        }
        self.tween.go(1.0, duration, now);
    }

    /// The pointer left `key`: unmark it, over `duration`.
    ///
    /// **Only if `key` is still the keyed one.** Both crossings are published
    /// from one `CursorMoved` in widget order, so moving between two neighbours
    /// delivers the new one's entry *before* the old one's exit — and an exit
    /// that meant "nothing is marked" would undo the entry that had just
    /// arrived. It is the same rule `app.rs` follows for the hovered row and the
    /// hovered tile, for the same toolkit reason.
    pub fn leave(&mut self, key: K, duration: Duration, now: Instant) {
        if self.key == Some(key) {
            self.tween.go(0.0, duration, now);
        }
    }

    /// How strongly `key` is marked, in `[0, 1]`. Zero for everything that is
    /// not the keyed thing, which is every other tile on the wall.
    #[must_use]
    pub fn strength(self, key: K) -> f32 {
        if self.key == Some(key) {
            self.tween.value()
        } else {
            0.0
        }
    }

    /// Which thing is marked, if any.
    #[must_use]
    pub const fn key(self) -> Option<K> {
        self.key
    }

    /// Whether a clock is still needed.
    #[must_use]
    pub const fn live(self) -> bool {
        self.tween.live()
    }

    /// Advance to `now`, reporting whether this is still live.
    ///
    /// A fade-out that has settled drops the key as well as the flight, so a
    /// keyed tween at rest holds no reference to a tile that may not be on the
    /// wall any more.
    pub fn tick(&mut self, now: Instant) -> bool {
        let live = self.tween.tick(now);
        if !live && self.tween.value() <= 0.0 {
            self.key = None;
        }
        live
    }
}

impl<K: Copy + PartialEq> Default for Keyed<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// Which icon-only control the pointer is on.
///
/// The seam ADR-0020 §2.1 exists to close. A `button` style's `text_color`
/// never reaches a rasterised glyph sprite — the mark is an `image`, inked at
/// raster time — so `theme::transport`'s hover and press arms were **dead code**
/// for every icon button in the product, and the glyph was byte-identical at
/// rest and under the pointer (`docs/design/04-fluidity.md` §3.1 finding 2).
/// The lever that does reach it is the image's own opacity, and an opacity is a
/// number the *shell* can hold.
///
/// So each icon button reports its own crossings with a `mouse_area` and the
/// shell holds the one answer — exactly the mechanism the queue rows' ✕ and the
/// shelf's tiles already use, and for exactly the same toolkit reason. With the
/// answer in hand the ink ladder completes: 0.57 rest, 1.00 hover, 0.75 press,
/// 0.28 disabled ([`crate::theme::glyph_ink`]).
///
/// Only the controls whose whole mark is a glyph are here. The queue rows' ✕ is
/// deliberately absent: it is drawn at all only while the pointer is on its row,
/// so its resting reading *is* its hovered reading and there is nothing to fade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    /// The bottom bar's Previous.
    Previous,
    /// The bottom bar's play/pause toggle.
    PlayPause,
    /// The bottom bar's Next.
    Next,
    /// The bottom bar's speaker.
    Mute,
}

/// What every icon button needs to know to ink itself: which one the pointer is
/// on, how far that one's fade has travelled, and whether it is held down.
///
/// One `Copy` value threaded through the view layer rather than three
/// parameters, because every surface that draws an icon button draws more than
/// one of them and each has to ask the same two questions. It is deliberately
/// *only* the pointer's half of the ladder: whether a control is live or waiting
/// is [`crate::player::PlayerState`]'s to say, and the two are combined at the
/// call site by [`crate::theme::glyph_ink`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ink {
    hover: Keyed<Control>,
    pressed: Option<Control>,
}

impl Ink {
    /// The shell's current reading.
    #[must_use]
    pub const fn new(hover: Keyed<Control>, pressed: Option<Control>) -> Self {
        Self { hover, pressed }
    }

    /// How far `control`'s hover fade has travelled, in `[0, 1]`.
    #[must_use]
    pub fn hover(self, control: Control) -> f32 {
        self.hover.strength(control)
    }

    /// Whether `control` is the one being held down.
    #[must_use]
    pub fn pressed(self, control: Control) -> bool {
        self.pressed == Some(control)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One frame at 60 Hz, which is what the transitions are actually ticked
    /// against on a vsynced display.
    const FRAME: Duration = Duration::from_micros(16_667);

    /// **It stops, and no later instant revives it.**
    ///
    /// The assertion ADR-0020's idle-cost claim rests on: `live()` is the
    /// boolean the subscription reads, so a tween that stayed live after its
    /// duration would be a `frames()` loop with extra steps.
    #[test]
    fn a_flight_is_live_until_its_duration_elapses_and_then_never_again() {
        let start = Instant::now();
        let mut tween = Tween::settled(0.0);
        assert!(!tween.live(), "a settled tween asks for no clock");
        assert!(!tween.tick(start), "and ticking it changes nothing");

        tween.go(1.0, LAMP, start);
        assert!(tween.live());
        assert!(tween.tick(start + LAMP / 2));
        assert!(tween.value() > 0.0 && tween.value() < 1.0);

        assert!(!tween.tick(start + LAMP), "it settles exactly at its end");
        assert!(!tween.live());
        assert!((tween.value() - 1.0).abs() < f32::EPSILON);
        // No later instant brings it back, which is what makes the subscription
        // go away and stay away.
        for later in [LAMP, LAMP * 2, Duration::from_secs(60)] {
            assert!(!tween.tick(start + later));
            assert!(!tween.live());
        }
    }

    /// **A transition may arrive late; it may never arrive wrong.**
    ///
    /// A 400 ms stall between ticks — a scan batch, a suspended laptop — lands
    /// exactly on the target rather than extrapolating past it.
    #[test]
    fn overshooting_the_end_settles_on_the_target_not_past_it() {
        let start = Instant::now();
        let mut tween = Tween::settled(0.0);
        tween.go(1.0, INK, start);
        assert!(!tween.tick(start + Duration::from_millis(400)));
        assert!(
            (tween.value() - 1.0).abs() < f32::EPSILON,
            "landed on {}",
            tween.value()
        );
    }

    /// Pointer in, out, and in again: two short correct movements, not one
    /// stale one.
    #[test]
    fn retargeting_mid_flight_starts_afresh_from_where_it_is() {
        let start = Instant::now();
        let mut tween = Tween::settled(0.0);
        tween.go(1.0, INK, start);
        tween.tick(start + INK / 2);
        let midway = tween.value();
        assert!(midway > 0.0 && midway < 1.0);

        // Out again, from where it actually is — never from 1.0.
        tween.go(0.0, INK, start + INK / 2);
        tween.tick(start + INK / 2 + Duration::from_micros(1));
        assert!(
            tween.value() <= midway + f32::EPSILON,
            "the fade-out began above where the fade-in had reached: {} > {midway}",
            tween.value()
        );
        assert!(tween.value() > 0.0, "and it did not snap to the target");

        // …and in again, still continuous.
        tween.go(1.0, INK, start + INK);
        assert!(tween.live());
        assert!(!tween.tick(start + INK * 2));
        assert!((tween.value() - 1.0).abs() < f32::EPSILON);
    }

    /// **The degrade-to-instant path**, and how a future "reduce motion"
    /// setting is implemented: pass [`Duration::ZERO`] and every call site
    /// becomes a hard cut with no branching anywhere else.
    #[test]
    fn a_zero_duration_is_the_degrade_to_instant_path() {
        let start = Instant::now();
        let mut tween = Tween::settled(0.0);
        tween.go(1.0, Duration::ZERO, start);
        assert!(!tween.live(), "an instant transition asks for no clock");
        assert!((tween.value() - 1.0).abs() < f32::EPSILON);

        // And so does a target that is already where the tween is: hovering a
        // control the pointer never left must not start a flight.
        let mut settled = Tween::settled(1.0);
        settled.go(1.0, INK, start);
        assert!(!settled.live());
    }

    /// `set` jumps and asks for no clock — what a window resize spends.
    #[test]
    fn set_jumps_without_asking_for_a_clock() {
        let start = Instant::now();
        let mut tween = Tween::settled(0.0);
        tween.go(1.0, LAMP, start);
        assert!(tween.live());
        tween.set(0.0);
        assert!(!tween.live(), "a jump cancels the flight it interrupts");
        assert!(tween.value().abs() < f32::EPSILON);
    }

    /// **The memory promise, as an equation.** ADR-0020 measured 48 bytes and
    /// the shipped type has to be the type that was measured.
    #[test]
    fn a_tween_is_forty_eight_bytes() {
        assert_eq!(std::mem::size_of::<Tween>(), 48);
        // Twenty animated scalars against a 150 MiB thumbnail budget.
        assert!(20 * std::mem::size_of::<Tween>() < 1024);
    }

    /// **A 150 ms transition is nine frames at 60 Hz** — asserted rather than
    /// guessed, because "nine frames" is the whole of what motion costs in
    /// drawing terms.
    #[test]
    fn a_200ms_transition_is_about_twelve_frames_at_60hz() {
        let start = Instant::now();
        let mut tween = Tween::settled(0.0);
        tween.go(1.0, LAMP, start);
        let mut ticks = 0;
        let mut moved = 0;
        let mut at = start;
        while tween.live() {
            at += FRAME;
            ticks += 1;
            if tween.tick(at) {
                moved += 1;
            }
        }
        // Twelve frames: eleven that moved it and the twelfth that landed it.
        assert_eq!(ticks, 12);
        assert_eq!(moved, 11);
        assert!(!tween.live());
        assert!((tween.value() - 1.0).abs() < f32::EPSILON);
    }

    /// Ease-out is fast away and slow in, and it **never overshoots** — no
    /// spring, no bounce, at any point of the flight.
    #[test]
    fn the_ease_out_curve_starts_fast_and_never_overshoots() {
        let start = Instant::now();
        let mut tween = Tween::settled(0.0);
        tween.go(1.0, LAMP, start);
        let mut previous = 0.0;
        for step in 1..=150 {
            let now = start + Duration::from_millis(step);
            tween.tick(now);
            let value = tween.value();
            assert!(
                (0.0..=1.0).contains(&value),
                "{step} ms: {value} is outside the flight"
            );
            assert!(value >= previous, "{step} ms: the value went backwards");
            previous = value;
        }
        // Half way through the *time*, an ease-out is well past half way
        // through the *distance*.
        let mut half = Tween::settled(0.0);
        half.go(1.0, LAMP, start);
        half.tick(start + LAMP / 2);
        assert!(half.value() > 0.5, "ease-out is front-loaded");
    }

    /// Linear is linear — the lamp's curve, checked at the midpoint because
    /// that is where every other curve differs from it.
    #[test]
    fn linear_is_linear() {
        let start = Instant::now();
        let mut tween = Tween::settled(0.0).with_curve(Curve::Linear);
        tween.go(1.0, LAMP, start);
        tween.tick(start + LAMP / 2);
        assert!(
            (tween.value() - 0.5).abs() < 0.01,
            "half the time is half the way: {}",
            tween.value()
        );
        tween.tick(start + LAMP / 4);
        assert!((tween.value() - 0.25).abs() < 0.01);
    }

    /// **One tween serves the whole wall**, and crossing from one tile to the
    /// next hands the mark over rather than restarting it.
    #[test]
    fn one_tween_serves_every_tile_and_hands_off_between_them() {
        let start = Instant::now();
        let mut hover: Keyed<u64> = Keyed::new();
        assert_eq!(hover.key(), None);
        assert!(hover.strength(1).abs() < f32::EPSILON);

        hover.enter(1, TILE, start);
        hover.tick(start + TILE / 2);
        let carried = hover.strength(1);
        assert!(carried > 0.0 && carried < 1.0);
        assert!(
            hover.strength(2).abs() < f32::EPSILON,
            "every other tile on the wall is unmarked"
        );

        // Across the gutter to the neighbour: entry first, then the old tile's
        // exit, exactly as the toolkit publishes them.
        hover.enter(2, TILE, start + TILE / 2);
        hover.leave(1, TILE, start + TILE / 2);
        assert_eq!(hover.key(), Some(2));
        assert!(
            (hover.strength(2) - carried).abs() < f32::EPSILON,
            "the mark travels with the pointer rather than starting over"
        );
        assert!(hover.strength(1).abs() < f32::EPSILON);
        assert!(!hover.tick(start + TILE * 2));
        assert!((hover.strength(2) - 1.0).abs() < f32::EPSILON);
    }

    /// An exit naming a tile that is no longer the marked one changes nothing —
    /// the ordering rule, as an assertion.
    #[test]
    fn leaving_a_tile_that_is_not_the_keyed_one_changes_nothing() {
        let start = Instant::now();
        let mut hover: Keyed<u64> = Keyed::new();
        hover.enter(1, TILE, start);
        hover.tick(start + TILE);
        hover.enter(2, TILE, start + TILE);
        hover.leave(1, TILE, start + TILE);
        assert_eq!(hover.key(), Some(2), "the stale exit did not steal the key");
        assert!(!hover.live(), "and it started no flight of its own");
        assert!((hover.strength(2) - 1.0).abs() < f32::EPSILON);
    }

    /// A settled fade-out drops the key, so a tween at rest holds no reference
    /// to a tile that may have left the wall.
    #[test]
    fn the_key_is_dropped_once_the_fade_out_settles() {
        let start = Instant::now();
        let mut hover: Keyed<u64> = Keyed::new();
        hover.enter(7, TILE, start);
        hover.tick(start + TILE);
        assert_eq!(hover.key(), Some(7));

        hover.leave(7, TILE, start + TILE);
        assert!(hover.live());
        assert_eq!(hover.key(), Some(7), "still keyed while it fades");
        assert!(!hover.tick(start + TILE * 2));
        assert_eq!(hover.key(), None);
        assert!(!hover.live());
    }

    /// The durations ADR-0020 §2 names, pinned so an edit has to argue with the
    /// decision rather than with a number.
    ///
    /// **Three of the five, and the other two are named as gone.** ADR-0022
    /// deleted the queue popover and the album inspector, so §2.2's 140 ms
    /// arrival and §2.4's 150 ms width have no surface to move; neither is
    /// forbidden, and if either surface ever returns its number returns with
    /// it. A place change is a hard cut.
    #[test]
    fn the_transitions_run_for_the_times_the_decision_names() {
        assert_eq!(INK, Duration::from_millis(90));
        assert_eq!(TILE, Duration::from_millis(90));
        assert_eq!(LAMP, Duration::from_millis(200));
        // The tick is finer than half a frame at 60 Hz, so it never becomes the
        // thing that decides how smooth a transition looks.
        assert!(TICK * 2 <= FRAME);
    }

    /// Every icon button is in [`Control::ALL`], and each is its own identity —
    /// two controls sharing one would share a hover.
    #[test]
    fn every_icon_button_has_an_identity_of_its_own() {
        let all = [
            Control::Previous,
            Control::PlayPause,
            Control::Next,
            Control::Mute,
        ];
        for (index, control) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(control, other);
            }
        }
        // And an `Ink` answers about exactly one of them at a time.
        let mut hover = Keyed::new();
        hover.enter(Control::Next, INK, Instant::now());
        hover.tick(Instant::now() + INK);
        let ink = Ink::new(hover, Some(Control::Next));
        for control in all {
            let expected = f32::from(u8::from(control == Control::Next));
            assert!((ink.hover(control) - expected).abs() < f32::EPSILON);
            assert_eq!(ink.pressed(control), control == Control::Next);
        }
    }
}
