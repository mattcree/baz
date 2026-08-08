//! Keyboard control: one pure function from a key press to a [`Message`].
//!
//! Every binding here resolves to a message the interface already emits —
//! [`Message::PlayPause`] is the same message the bottom bar's toggle sends,
//! [`Message::NextTrack`] the same one its Next button sends. There is no
//! second transport path to keep in step with the first, and no keyboard
//! shortcut can do something no control on screen can do.
//!
//! # The focus rule
//!
//! A key press belongs to a focused text field or it belongs to the
//! application, never to both. baz does not *guess* which: iced reports an
//! [`event::Status`](iced::event::Status) alongside every runtime event
//! saying whether a widget already consumed it, and that report is the whole
//! of the rule. [`Focus::TextField`] (iced said `Captured`) means the search
//! well took the key — typed it, moved its cursor, or dismissed itself — and
//! [`binding_for`] answers `None` for **every** key in that state. Space
//! types a space. `/` types a slash. Left and Right move the caret. `n`
//! types an `n`.
//!
//! This is stronger than tracking focus ourselves would be. iced 0.13 exposes
//! no "is this widget focused" query a subscription could ask synchronously,
//! so any hand-kept flag would have to *infer* blur from clicks it cannot
//! locate — and the failure mode of a wrong guess is a space bar that pauses
//! the music mid-word. Deferring to the toolkit's own capture report cannot
//! be wrong, because it is the same decision the text field itself made a
//! moment earlier.
//!
//! One consequence is worth stating plainly: iced's `text_input` captures
//! *every* key press while focused except Tab and the vertical arrows, so
//! while the search well has focus no transport binding is live at all. The
//! way out is the way in — Escape, which the field consumes to blur itself,
//! after which the transport keys work. (The search field takes focus at
//! startup, so this is the first thing a keyboard user meets; it is
//! documented in the README's key table.)
//!
//! # Why these steps
//!
//! [`SEEK_STEP_MS`] is 5 s because that is what an arrow key means to people
//! already: mpv seeks ±5 s on Left/Right, and so does `YouTube`. It is long
//! enough to skip an intro figure and short enough that overshooting costs
//! one press back.
//!
//! [`SEEK_STEP_LARGE_MS`] is 30 s — the skip-forward step podcast players
//! standardised on (Apple Podcasts, Pocket Casts) and roughly one section of
//! a song. The point of a second step is that it is *obviously* different
//! from the first; 6× reads as a different gesture, where 2× would just feel
//! like an unreliable 5.
//!
//! Seeks are relative to the position the seek bar is *showing* (a scrub
//! under the pointer, else a seek awaiting confirmation, else the engine's
//! last report — [`crate::player`] pins that precedence), so holding Right
//! accumulates instead of fighting the engine's 4 Hz progress cadence.
//! Nothing here invents a position: the target is computed by
//! [`PlayerState::seek_by`](crate::player::PlayerState::seek_by) from
//! event-derived state and clamped to the track the engine confirmed.
//!
//! # Volume
//!
//! Up and Down move the fader by one
//! [`VOLUME_STEP`](crate::player::VOLUME_STEP) — 40 of the control's 1000
//! positions, which is 1.04 dB at the top of the cubic taper and therefore
//! the smallest press that does something a listener reliably hears. That
//! module carries the full derivation and the two properties that follow
//! from the number dividing 1000 exactly.
//!
//! `M` mutes and unmutes. It is the letter every player uses for it, it
//! needs no modifier, and — like the transport keys — it resolves against
//! the *confirmed* state rather than a flag we keep, so the command that
//! goes out is the idempotent `SetMute { muted }` the protocol asks for
//! rather than a toggle two front ends could disagree about.
//!
//! # Panels
//!
//! `Q` shows and hides the play queue, and <kbd>Ctrl</kbd>+<kbd>B</kbd>
//! (<kbd>Cmd</kbd>+<kbd>B</kbd>) hides the right-hand rail outright and brings
//! it back. Both resolve to the same messages the on-screen affordances send —
//! the top bar's Queue toggle and each panel's ✕ — so there is no keyboard-only
//! capability here, exactly as with the transport.
//!
//! `Q` is bare because it is a *view* key like `/`: it interrupts nothing, it
//! is reversible by pressing it again, and a modifier on a key you will press
//! dozens of times a session is a tax with no safety to buy. It is also free —
//! foobar2000 and `MusicBee` both put queue-adjacent commands on it, and nothing
//! in baz wanted `q`.
//!
//! <kbd>Ctrl</kbd>+<kbd>B</kbd> is the sidebar reflex from every editor
//! written this decade, and it earns its modifier for the opposite reason: it
//! is the *layout* key, the one that changes how much room the shelf gets, and
//! those are conventionally modified. What comes back is what was dismissed
//! (see [`crate::panels`]), so the pair is a true toggle rather than a
//! destructive close.
//!
//! <kbd>Ctrl</kbd>+<kbd>,</kbd> (<kbd>Cmd</kbd>+<kbd>,</kbd>) opens the
//! settings, and takes a modifier where `Q` does not. The reasoning is `Q`'s
//! in reverse: a preferences key is pressed a handful of times in a
//! *lifetime*, not dozens of times a session, so the tax the modifier charges
//! is never actually paid — and <kbd>Cmd</kbd>+<kbd>,</kbd> is a macOS system
//! convention that every cross-platform application has adopted, which makes
//! it the one binding here a listener is more likely to already know than to
//! learn. A bare `,` would also be the first bare punctuation key baz binds,
//! and punctuation is what people type when a field is not focused by
//! accident.
//!
//! **The `XF86AudioRaiseVolume` family is deliberately not bound.** The
//! transport media keys are bound (below) because `MediaPlayPause` means one
//! thing everywhere; the volume keys do not. On every desktop they mean *the
//! system's* volume, and on most they never reach an application at all — so
//! binding them would make one key change either baz's fader or the whole
//! machine's, depending on which daemon happened to grab it first. baz's
//! volume is baz's alone (ADR-0011: it is the per-application control), and a
//! key that means two different things is worse than a key that means one.

use iced::keyboard::{Key, Modifiers, key};

use crate::app::Message;

/// Step for an unmodified Left/Right, in milliseconds (module docs).
pub(crate) const SEEK_STEP_MS: i64 = 5_000;

/// Step for Shift+Left/Right, in milliseconds (module docs).
pub(crate) const SEEK_STEP_LARGE_MS: i64 = 30_000;

/// Who the key press belongs to — read off iced's capture report, not
/// inferred (module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Focus {
    /// A focused widget consumed the press. In baz that is always the search
    /// well: `text_input` is the only widget in the tree that handles key
    /// presses at all.
    TextField,
    /// No widget claimed the press, so it is the application's to interpret.
    Elsewhere,
}

impl From<iced::event::Status> for Focus {
    fn from(status: iced::event::Status) -> Self {
        match status {
            iced::event::Status::Captured => Self::TextField,
            iced::event::Status::Ignored => Self::Elsewhere,
        }
    }
}

/// The message a key press asks for, or `None` when it asks for nothing.
///
/// Pure: no state, no side effects, no iced runtime. The whole binding table
/// is this match, and it is exhaustively unit-tested below — including the
/// modifier variants that must *not* fire.
pub(crate) fn binding_for(key: &Key, modifiers: Modifiers, focus: Focus) -> Option<Message> {
    // The focused text field already had this key and made its decision.
    if focus == Focus::TextField {
        return None;
    }
    let bare = modifiers.is_empty();
    let shift = modifiers == Modifiers::SHIFT;
    let command = modifiers == Modifiers::COMMAND;

    match key.as_ref() {
        // Transport. Space is the universal play/pause and takes no
        // modifiers at all — Ctrl+Space and friends belong to whatever binds
        // them later.
        Key::Named(key::Named::Space) | Key::Character(" ") if bare => Some(Message::PlayPause),
        Key::Character("n" | "N") if bare || shift => Some(Message::NextTrack),
        // Ctrl+Right (Cmd+Right on macOS) is the second spelling of Next, for
        // hands that never leave the arrow cluster.
        Key::Named(key::Named::ArrowRight) if command => Some(Message::NextTrack),

        // Seeking. Shift widens the step; nothing else may ride along.
        Key::Named(key::Named::ArrowRight) if bare => Some(Message::SeekBy(SEEK_STEP_MS)),
        Key::Named(key::Named::ArrowRight) if shift => Some(Message::SeekBy(SEEK_STEP_LARGE_MS)),
        Key::Named(key::Named::ArrowLeft) if bare => Some(Message::SeekBy(-SEEK_STEP_MS)),
        Key::Named(key::Named::ArrowLeft) if shift => Some(Message::SeekBy(-SEEK_STEP_LARGE_MS)),

        // Volume. The vertical arrows are the axis a fader moves on, and
        // `M` is mute everywhere. Neither takes a modifier (module docs).
        Key::Named(key::Named::ArrowUp) if bare => Some(Message::VolumeStep(1)),
        Key::Named(key::Named::ArrowDown) if bare => Some(Message::VolumeStep(-1)),
        Key::Character("m" | "M") if bare || shift => Some(Message::ToggleMute),

        // Panels. `Q` shows what is playing next; Ctrl+B (Cmd+B) takes the
        // whole rail away and gives it back; Ctrl+`,` (Cmd+`,`) is the
        // settings (module docs).
        Key::Character("q" | "Q") if bare || shift => Some(Message::ToggleQueue),
        Key::Character("b" | "B") if command => Some(Message::TogglePanels),
        Key::Character(",") if command => Some(Message::ToggleSettings),

        // Search. `/` is the reflex from every pager and browser; Ctrl+F
        // (Cmd+F) is the reflex from every document. Shift is tolerated on
        // `/` because plenty of layouts need it to type the character.
        Key::Character("/") if bare || shift => Some(Message::FocusSearch),
        Key::Character("f" | "F") if command => Some(Message::FocusSearch),

        // Unchanged from the first keyboard pass: clear the search, else
        // close what the rail is showing.
        Key::Named(key::Named::Escape) if bare => Some(Message::EscapePressed),

        // Media keys, for the machines that deliver them to the focused
        // window rather than to a desktop shortcut daemon. On Linux the
        // desktop usually grabs these and routes them over MPRIS instead
        // (see [`crate::mpris`]); these arms are what make them work on
        // Windows and macOS, and on a bare Linux session with no shell
        // listening.
        Key::Named(key::Named::MediaPlayPause) => Some(Message::PlayPause),
        Key::Named(key::Named::MediaTrackNext) => Some(Message::NextTrack),
        Key::Named(key::Named::MediaStop) => Some(Message::Stop),
        Key::Named(key::Named::Play) => Some(Message::Play),
        Key::Named(key::Named::Pause) => Some(Message::Pause),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `Message` variants this module can produce, compared structurally
    /// (`Message` is deliberately not `PartialEq` — it carries iced payloads
    /// that have no useful equality).
    fn tag(message: &Message) -> String {
        format!("{message:?}")
    }

    fn named(name: key::Named) -> Key {
        Key::Named(name)
    }

    fn ch(c: &str) -> Key {
        Key::Character(c.into())
    }

    fn bind(key: &Key, modifiers: Modifiers) -> Option<String> {
        binding_for(key, modifiers, Focus::Elsewhere)
            .as_ref()
            .map(tag)
    }

    fn none() -> Modifiers {
        Modifiers::empty()
    }

    #[test]
    fn space_toggles_play_pause() {
        assert_eq!(
            bind(&named(key::Named::Space), none()).as_deref(),
            Some("PlayPause")
        );
        // The character spelling, for backends that report it that way.
        assert_eq!(bind(&ch(" "), none()).as_deref(), Some("PlayPause"));
    }

    /// The rule the whole module exists for: a focused search well types a
    /// space, it does not pause the music.
    #[test]
    fn space_in_the_search_field_is_not_a_transport_key() {
        assert!(binding_for(&named(key::Named::Space), none(), Focus::TextField).is_none());
        assert!(binding_for(&ch(" "), none(), Focus::TextField).is_none());
    }

    /// …and neither is anything else. A captured press is the field's.
    #[test]
    fn a_focused_text_field_swallows_every_binding() {
        let every_bound_key = [
            (named(key::Named::Space), none()),
            (named(key::Named::ArrowLeft), none()),
            (named(key::Named::ArrowRight), none()),
            (named(key::Named::ArrowLeft), Modifiers::SHIFT),
            (named(key::Named::ArrowRight), Modifiers::SHIFT),
            (named(key::Named::ArrowRight), Modifiers::COMMAND),
            (named(key::Named::ArrowUp), none()),
            (named(key::Named::ArrowDown), none()),
            (ch("n"), none()),
            (ch("m"), none()),
            (ch("q"), none()),
            (ch("b"), Modifiers::COMMAND),
            (ch(","), Modifiers::COMMAND),
            (ch("/"), none()),
            (ch("f"), Modifiers::COMMAND),
            (named(key::Named::Escape), none()),
            (named(key::Named::MediaPlayPause), none()),
            (named(key::Named::MediaTrackNext), none()),
        ];
        for (key, modifiers) in &every_bound_key {
            assert!(
                binding_for(key, *modifiers, Focus::TextField).is_none(),
                "{key:?} + {modifiers:?} must belong to the focused field"
            );
            // Each of them *is* a binding when the field does not have it —
            // otherwise this test would pass vacuously.
            assert!(
                binding_for(key, *modifiers, Focus::Elsewhere).is_some(),
                "{key:?} + {modifiers:?} should bind when nothing captured it"
            );
        }
    }

    #[test]
    fn arrows_seek_by_the_documented_steps() {
        assert_eq!(
            bind(&named(key::Named::ArrowRight), none()).as_deref(),
            Some("SeekBy(5000)")
        );
        assert_eq!(
            bind(&named(key::Named::ArrowLeft), none()).as_deref(),
            Some("SeekBy(-5000)")
        );
        assert_eq!(
            bind(&named(key::Named::ArrowRight), Modifiers::SHIFT).as_deref(),
            Some("SeekBy(30000)")
        );
        assert_eq!(
            bind(&named(key::Named::ArrowLeft), Modifiers::SHIFT).as_deref(),
            Some("SeekBy(-30000)")
        );
    }

    #[test]
    fn the_two_steps_are_the_documented_constants() {
        assert_eq!(SEEK_STEP_MS, 5_000);
        assert_eq!(SEEK_STEP_LARGE_MS, 30_000);
    }

    #[test]
    fn the_vertical_arrows_step_the_volume() {
        assert_eq!(
            bind(&named(key::Named::ArrowUp), none()).as_deref(),
            Some("VolumeStep(1)")
        );
        assert_eq!(
            bind(&named(key::Named::ArrowDown), none()).as_deref(),
            Some("VolumeStep(-1)")
        );
    }

    #[test]
    fn m_mutes_in_either_case() {
        assert_eq!(bind(&ch("m"), none()).as_deref(), Some("ToggleMute"));
        // Shift+M is still M, whatever the layout calls it — the same
        // tolerance `N` and `/` already get.
        assert_eq!(
            bind(&ch("M"), Modifiers::SHIFT).as_deref(),
            Some("ToggleMute")
        );
    }

    /// The volume keys are not media keys. `XF86AudioRaiseVolume` and friends
    /// mean *the system's* volume on every desktop, and baz's fader is baz's
    /// alone (module docs) — so they stay unbound, deliberately and testably.
    #[test]
    fn the_systems_volume_keys_are_left_to_the_system() {
        for key in [
            named(key::Named::AudioVolumeUp),
            named(key::Named::AudioVolumeDown),
            named(key::Named::AudioVolumeMute),
        ] {
            assert_eq!(bind(&key, none()), None, "{key:?} belongs to the desktop");
        }
    }

    #[test]
    fn next_track_has_two_spellings() {
        assert_eq!(bind(&ch("n"), none()).as_deref(), Some("NextTrack"));
        // Shift+N is still N, whatever the layout calls it.
        assert_eq!(
            bind(&ch("N"), Modifiers::SHIFT).as_deref(),
            Some("NextTrack")
        );
        assert_eq!(
            bind(&named(key::Named::ArrowRight), Modifiers::COMMAND).as_deref(),
            Some("NextTrack")
        );
    }

    /// Ctrl+Right is Next, not a 5-second seek: the modified arm must win.
    #[test]
    fn ctrl_right_is_next_not_a_seek() {
        assert_eq!(
            bind(&named(key::Named::ArrowRight), Modifiers::COMMAND).as_deref(),
            Some("NextTrack")
        );
        // Ctrl+Left is nothing — there is no Previous in the engine protocol.
        assert_eq!(
            bind(&named(key::Named::ArrowLeft), Modifiers::COMMAND),
            None
        );
    }

    /// `Q` shows and hides the queue, in either case, with no modifier — a
    /// view key, like `/`.
    #[test]
    fn q_toggles_the_queue_panel() {
        assert_eq!(bind(&ch("q"), none()).as_deref(), Some("ToggleQueue"));
        assert_eq!(
            bind(&ch("Q"), Modifiers::SHIFT).as_deref(),
            Some("ToggleQueue")
        );
    }

    /// Ctrl+B (Cmd+B) is the layout key: it hides the rail and brings it back.
    /// Bare `b` types a `b`.
    #[test]
    fn ctrl_b_hides_and_restores_the_rail() {
        assert_eq!(
            bind(&ch("b"), Modifiers::COMMAND).as_deref(),
            Some("TogglePanels")
        );
        assert_eq!(
            bind(&ch("B"), Modifiers::COMMAND).as_deref(),
            Some("TogglePanels")
        );
        assert_eq!(bind(&ch("b"), none()), None);
    }

    /// Ctrl+`,` (Cmd+`,`) is the preferences reflex from every platform.
    /// A bare comma types a comma — the modifier is the whole binding.
    #[test]
    fn ctrl_comma_opens_the_settings() {
        assert_eq!(
            bind(&ch(","), Modifiers::COMMAND).as_deref(),
            Some("ToggleSettings")
        );
        assert_eq!(bind(&ch(","), none()), None);
        assert_eq!(bind(&ch(","), Modifiers::SHIFT), None);
    }

    #[test]
    fn search_focus_has_two_spellings() {
        assert_eq!(bind(&ch("/"), none()).as_deref(), Some("FocusSearch"));
        assert_eq!(
            bind(&ch("/"), Modifiers::SHIFT).as_deref(),
            Some("FocusSearch")
        );
        assert_eq!(
            bind(&ch("f"), Modifiers::COMMAND).as_deref(),
            Some("FocusSearch")
        );
        assert_eq!(
            bind(&ch("F"), Modifiers::COMMAND).as_deref(),
            Some("FocusSearch")
        );
        // Bare `f` types an `f`; it is not a shortcut.
        assert_eq!(bind(&ch("f"), none()), None);
    }

    #[test]
    fn escape_keeps_its_existing_meaning() {
        assert_eq!(
            bind(&named(key::Named::Escape), none()).as_deref(),
            Some("EscapePressed")
        );
    }

    #[test]
    fn media_keys_reach_the_transport() {
        assert_eq!(
            bind(&named(key::Named::MediaPlayPause), none()).as_deref(),
            Some("PlayPause")
        );
        assert_eq!(
            bind(&named(key::Named::MediaTrackNext), none()).as_deref(),
            Some("NextTrack")
        );
        assert_eq!(
            bind(&named(key::Named::MediaStop), none()).as_deref(),
            Some("Stop")
        );
        assert_eq!(
            bind(&named(key::Named::Play), none()).as_deref(),
            Some("Play")
        );
        assert_eq!(
            bind(&named(key::Named::Pause), none()).as_deref(),
            Some("Pause")
        );
    }

    /// A binding must not fire because an unrelated modifier happened to be
    /// down. Each case here is a key that *does* bind bare.
    #[test]
    fn unrelated_modifiers_suppress_a_binding() {
        let suppressed = [
            (named(key::Named::Space), Modifiers::COMMAND),
            (named(key::Named::Space), Modifiers::ALT),
            (named(key::Named::Space), Modifiers::SHIFT),
            (named(key::Named::Space), Modifiers::LOGO),
            (named(key::Named::ArrowRight), Modifiers::ALT),
            (named(key::Named::ArrowLeft), Modifiers::ALT),
            (
                named(key::Named::ArrowRight),
                Modifiers::COMMAND | Modifiers::SHIFT,
            ),
            (named(key::Named::ArrowUp), Modifiers::SHIFT),
            (named(key::Named::ArrowUp), Modifiers::COMMAND),
            (named(key::Named::ArrowDown), Modifiers::ALT),
            (ch("n"), Modifiers::COMMAND),
            (ch("n"), Modifiers::ALT),
            (ch("m"), Modifiers::COMMAND),
            (ch("m"), Modifiers::ALT),
            (ch("q"), Modifiers::COMMAND),
            (ch("q"), Modifiers::ALT),
            (ch("b"), Modifiers::ALT),
            (ch("b"), Modifiers::COMMAND | Modifiers::SHIFT),
            (ch("/"), Modifiers::COMMAND),
            (ch("f"), Modifiers::ALT),
            (named(key::Named::Escape), Modifiers::COMMAND),
            (named(key::Named::Escape), Modifiers::SHIFT),
        ];
        for (key, modifiers) in &suppressed {
            assert_eq!(
                bind(key, *modifiers),
                None,
                "{key:?} must not bind with {modifiers:?}"
            );
        }
    }

    #[test]
    fn unknown_keys_map_to_nothing() {
        let unbound = [
            named(key::Named::Enter),
            named(key::Named::Tab),
            named(key::Named::Backspace),
            named(key::Named::Delete),
            named(key::Named::Home),
            named(key::Named::End),
            named(key::Named::PageUp),
            named(key::Named::F1),
            named(key::Named::MediaTrackPrevious),
            ch("a"),
            ch("z"),
            ch("1"),
            ch("."),
            ch("?"),
        ];
        for key in &unbound {
            assert_eq!(bind(key, none()), None, "{key:?} must be unbound");
            assert!(
                binding_for(key, none(), Focus::TextField).is_none(),
                "{key:?} must be unbound in a text field too"
            );
        }
    }

    #[test]
    fn focus_reads_iceds_capture_report() {
        assert_eq!(Focus::from(iced::event::Status::Captured), Focus::TextField);
        assert_eq!(Focus::from(iced::event::Status::Ignored), Focus::Elsewhere);
    }
}
