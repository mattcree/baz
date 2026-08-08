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
//! # Previous
//!
//! <kbd>Ctrl</kbd>+<kbd>←</kbd> (<kbd>Cmd</kbd>+<kbd>←</kbd>) is Previous, the
//! exact mirror of the <kbd>Ctrl</kbd>+<kbd>→</kbd> that was already Next. The
//! symmetry is the argument: the arrow cluster's two horizontal keys seek, and
//! the same two under the transport modifier step tracks, so a hand that knows
//! one knows the other. It is also the only spelling this command gets a bare
//! key for — `p` is not the reflex `n` is, and it is one slip away from a
//! *play* the transport already binds to Space.
//!
//! `MediaTrackPrevious` joins the media-key family below for the reason the
//! rest of them are there: on the machines that deliver it to the window
//! rather than to a shortcut daemon it means one thing, and it is the same
//! thing everywhere. Leaving it unbound while `MediaTrackNext` worked was an
//! artefact of the engine having no `Previous` command to send, which it now
//! does (ADR-0014's siblings; the command itself predates it).
//!
//! Both resolve to [`Message::PreviousTrack`] — the same message the bottom
//! bar's new `|◀` sends and the same one MPRIS's `Previous` maps to, so the
//! rule that no shortcut can do what no control can do still holds.
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
//! # Layers
//!
//! Three keys move between what is on screen, and each one now names exactly
//! one layer — which is the change the information-architecture move bought
//! (ADR-0016, `docs/design/01-ux-audit-and-ia.md` §4.8).
//!
//! `Q` shows and hides **Queue**. Same key, same meaning, better place: it
//! used to raise a queue *panel* in the right-hand rail, costing the shelf two
//! columns of covers for a glance; it now raises the popover anchored to the
//! bar that describes it, which costs the shelf nothing. It resolves to
//! [`Message::ToggleQueue`] — the same message the bar's labelled `Queue`
//! control sends — so there is no keyboard-only capability here, exactly as
//! with the transport.
//!
//! `Q` is bare because it is a *view* key like `/`: it interrupts nothing, it
//! is reversible by pressing it again, and a modifier on a key you will press
//! dozens of times a session is a tax with no safety to buy. It is also free —
//! foobar2000 and `MusicBee` both put queue-adjacent commands on it, and nothing
//! in baz wanted `q`.
//!
//! <kbd>Ctrl</kbd>+<kbd>B</kbd> (<kbd>Cmd</kbd>+<kbd>B</kbd>) hides the album
//! inspector and brings it back. It is the sidebar reflex from every editor
//! written this decade, and it earns its modifier for the opposite reason to
//! `Q`'s: it is the *layout* key, the one that changes how much room the shelf
//! gets, and those are conventionally modified. What comes back is what was
//! dismissed (see [`crate::selection`]), so the pair is a true toggle rather
//! than a destructive close — and it is an *honest* sidebar toggle now that
//! there is exactly one sidebar. It no longer conjures a queue panel out of an
//! empty rail, which was the audit's evidence that the rail had no model behind
//! it.
//!
//! <kbd>Ctrl</kbd>+<kbd>,</kbd> (<kbd>Cmd</kbd>+<kbd>,</kbd>) goes to the
//! **Settings place** and comes back ([`crate::place`]). Same key as before; it
//! is now *navigation* rather than a request to raise a panel, which is what
//! the macOS convention it borrows has always meant — and it is the same
//! message the top bar's `Settings` control and the place's own Back both send.
//!
//! It takes a modifier where `Q` does not, and the reasoning is `Q`'s
//! in reverse: a preferences key is pressed a handful of times in a
//! *lifetime*, not dozens of times a session, so the tax the modifier charges
//! is never actually paid — and <kbd>Cmd</kbd>+<kbd>,</kbd> is a macOS system
//! convention that every cross-platform application has adopted, which makes
//! it the one binding here a listener is more likely to already know than to
//! learn. A bare `,` would also be the first bare punctuation key baz binds,
//! and punctuation is what people type when a field is not focused by
//! accident.
//!
//! <kbd>Esc</kbd> peels **one layer, top down**: the popover first, then a
//! place that is not home, then — in the search well — the query, then the
//! inspector. It never has to choose between unrelated things, because at each
//! press exactly one layer is the top one. The layering itself lives in
//! `app.rs`, where the layers do; this module only says that the key means
//! "peel".
//!
//! # The pull — <kbd>Ctrl</kbd>+<kbd>R</kbd>
//!
//! The one binding ADR-0017 §1.2's key table spends on a *draw*, and it earns
//! its modifier twice over. It is the letter every application spells *refresh*
//! or *random* with, so the reflex is already there; and unlike `Q` or `1`–`5`
//! it is not a view key — it moves the wall, opens the column and replaces
//! whatever suggestion was standing. A bare `r` would also be the first letter
//! a listener types into the well when they mean *Radiohead*.
//!
//! It resolves to [`Message::Pull`], which is the message the top bar's `Pull`
//! word sends, so the visible-control rule holds. What it does **not** do is
//! start anything: the pull selects a record and prints when it was last heard,
//! and the control that accepts it is the inspector's own `Play album`. Pressing
//! this key again pulls a different record; <kbd>Esc</kbd> puts the offer back.
//!
//! Shuffle gets **no** key at all, deliberately. The rule is one-directional —
//! every action needs a visible control, not every control needs a key — and
//! the shortcuts baz spends are for the things a hand does dozens of times a
//! session. Starting a shuffle is a decision made once an evening, from a word
//! that is already on screen.
//!
//! # The arrangement — `1` … `5`
//!
//! The five group keys (ADR-0019) select from the number row: `1` ARTIST,
//! `2` YEAR, `3` GENRE, `4` ADDED, `5` PLAYED, in the order the top bar's row
//! of words states them and the order
//! [`GroupKey::ALL`](baz_core::index::GroupKey::ALL) publishes them. The
//! mapping is [`group_key`], which reads that array rather than repeating it,
//! so the digits, the words and the library's own order cannot drift apart.
//!
//! **Digits, deliberately.** ADR-0017 §1.2's keyboard table already spends
//! `1` and `2` on the Wall / Marquee lenses and states the trade out loud —
//! *digits are not letters, and no album title begins with one often enough to
//! matter* — which is the same argument, made for the same reason, one step
//! earlier: when type-anywhere lands (step 11) every bare *letter* becomes
//! query and the number row is the one place a bare binding can survive.
//! Whichever of the two ends up owning the digits, this is where the row is
//! spent, and the resolution is a decision for step 18 rather than a thing to
//! guess at now.
//!
//! They are bare for `Q`'s reason: selecting an arrangement is a *view* act —
//! it interrupts nothing, plays nothing, and is undone by pressing another
//! one — and a modifier on a key pressed dozens of times a session is a tax
//! with no safety to buy. Each resolves to
//! [`Message::GroupKeySelected`], which is the same message the word in the
//! top bar sends, so the visible-control rule holds: there is no arrangement
//! reachable only from the keyboard.
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
        // hands that never leave the arrow cluster — and Ctrl+Left is its
        // mirror, Previous (module docs).
        Key::Named(key::Named::ArrowRight) if command => Some(Message::NextTrack),
        Key::Named(key::Named::ArrowLeft) if command => Some(Message::PreviousTrack),

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

        // Layers. `Q` shows what is playing next; Ctrl+B (Cmd+B) takes the
        // right-hand column away and gives it back; Ctrl+`,` (Cmd+`,`) is the
        // settings (module docs).
        Key::Character("q" | "Q") if bare || shift => Some(Message::ToggleQueue),
        Key::Character("b" | "B") if command => Some(Message::TogglePanels),
        Key::Character(",") if command => Some(Message::ToggleSettings),

        // The pull: one record, weighted toward the long unplayed, offered
        // rather than started (module docs).
        Key::Character("r" | "R") if command => Some(Message::Pull),

        // Arrangement. `1`–`5` are the five group keys, in the order the top
        // bar's row of words states them (module docs).
        Key::Character(digit @ ("1" | "2" | "3" | "4" | "5")) if bare => {
            Some(Message::GroupKeySelected(group_key(digit)?))
        }

        // Search. `/` is the reflex from every pager and browser; Ctrl+F
        // (Cmd+F) is the reflex from every document. Shift is tolerated on
        // `/` because plenty of layouts need it to type the character.
        Key::Character("/") if bare || shift => Some(Message::FocusSearch),
        Key::Character("f" | "F") if command => Some(Message::FocusSearch),

        // Peel one layer, top down (module docs; `app.rs` holds the order).
        Key::Named(key::Named::Escape) if bare => Some(Message::EscapePressed),

        // Media keys, for the machines that deliver them to the focused
        // window rather than to a desktop shortcut daemon. On Linux the
        // desktop usually grabs these and routes them over MPRIS instead
        // (see [`crate::mpris`]); these arms are what make them work on
        // Windows and macOS, and on a bare Linux session with no shell
        // listening.
        Key::Named(key::Named::MediaPlayPause) => Some(Message::PlayPause),
        Key::Named(key::Named::MediaTrackNext) => Some(Message::NextTrack),
        Key::Named(key::Named::MediaTrackPrevious) => Some(Message::PreviousTrack),
        Key::Named(key::Named::MediaStop) => Some(Message::Stop),
        Key::Named(key::Named::Play) => Some(Message::Play),
        Key::Named(key::Named::Pause) => Some(Message::Pause),

        _ => None,
    }
}

/// The group key a digit names: `1` is the first word in
/// [`GroupKey::ALL`](baz_core::index::GroupKey::ALL) and `5` is the last.
///
/// Derived from the enum's own order rather than written out, so the row of
/// words in the top bar, the digits that select them and the order
/// `baz-core` publishes are one list. A sixth key (CRATES, MOOD) becomes
/// `6` here with no edit at all — and until then `6` is unbound, because
/// `ALL` has five entries and this returns `None` past its end.
fn group_key(digit: &str) -> Option<baz_core::index::GroupKey> {
    let index = digit.parse::<usize>().ok()?.checked_sub(1)?;
    baz_core::index::GroupKey::ALL.get(index).copied()
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
            (named(key::Named::ArrowLeft), Modifiers::COMMAND),
            (named(key::Named::ArrowUp), none()),
            (named(key::Named::ArrowDown), none()),
            (ch("n"), none()),
            (ch("m"), none()),
            (ch("q"), none()),
            (ch("b"), Modifiers::COMMAND),
            (ch(","), Modifiers::COMMAND),
            (ch("/"), none()),
            (ch("f"), Modifiers::COMMAND),
            (ch("1"), none()),
            (ch("5"), none()),
            (named(key::Named::Escape), none()),
            (named(key::Named::MediaPlayPause), none()),
            (named(key::Named::MediaTrackNext), none()),
            (named(key::Named::MediaTrackPrevious), none()),
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

    /// Ctrl+Right is Next and Ctrl+Left is Previous, not 5-second seeks: the
    /// modified arms must win over the bare ones for both arrows.
    #[test]
    fn the_modified_arrows_step_tracks_rather_than_seeking() {
        assert_eq!(
            bind(&named(key::Named::ArrowRight), Modifiers::COMMAND).as_deref(),
            Some("NextTrack")
        );
        assert_eq!(
            bind(&named(key::Named::ArrowLeft), Modifiers::COMMAND).as_deref(),
            Some("PreviousTrack")
        );
        // …and the bare and Shift arms still seek, so the modifier is the
        // whole of the difference.
        assert_eq!(
            bind(&named(key::Named::ArrowLeft), none()).as_deref(),
            Some("SeekBy(-5000)")
        );
        assert_eq!(
            bind(&named(key::Named::ArrowLeft), Modifiers::SHIFT).as_deref(),
            Some("SeekBy(-30000)")
        );
    }

    /// Previous has two spellings and both suppress under a focused field —
    /// the focus rule is not relaxed for the newest binding.
    #[test]
    fn previous_has_two_spellings_and_neither_survives_a_focused_field() {
        for (key, modifiers) in [
            (named(key::Named::ArrowLeft), Modifiers::COMMAND),
            (named(key::Named::MediaTrackPrevious), none()),
        ] {
            assert_eq!(
                bind(&key, modifiers).as_deref(),
                Some("PreviousTrack"),
                "{key:?} + {modifiers:?} should be Previous"
            );
            assert!(
                binding_for(&key, modifiers, Focus::TextField).is_none(),
                "{key:?} + {modifiers:?} must belong to the focused field"
            );
        }
        // Ctrl+Left is the whole binding: an extra modifier is not it, and a
        // bare Left is a seek (asserted above), not a track step.
        assert_eq!(
            bind(
                &named(key::Named::ArrowLeft),
                Modifiers::COMMAND | Modifiers::SHIFT
            ),
            None
        );
        assert_eq!(
            bind(&named(key::Named::ArrowLeft), Modifiers::ALT),
            None,
            "Alt+Left is the browser's Back, not baz's Previous"
        );
    }

    /// `Q` shows and hides **Queue**, in either case, with no modifier — a
    /// view key, like `/`. The key did not move; what it raises did.
    #[test]
    fn q_toggles_the_queue_panel_popover() {
        assert_eq!(bind(&ch("q"), none()).as_deref(), Some("ToggleQueue"));
        assert_eq!(
            bind(&ch("Q"), Modifiers::SHIFT).as_deref(),
            Some("ToggleQueue")
        );
    }

    /// Ctrl+B (Cmd+B) is the layout key: it hides the inspector and brings it back.
    /// Bare `b` types a `b`.
    #[test]
    fn ctrl_b_hides_and_restores_the_inspector() {
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

    /// Ctrl+`,` (Cmd+`,`) is the preferences reflex from every platform, and it
    /// is navigation between places now rather than a panel toggle.
    /// A bare comma types a comma — the modifier is the whole binding.
    #[test]
    fn ctrl_comma_navigates_to_the_settings() {
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

    /// **`1`–`5` are the five group keys, in `baz-core`'s own order**, and the
    /// mapping is that order rather than a copy of it.
    #[test]
    fn the_number_row_selects_the_five_arrangements() {
        use baz_core::index::GroupKey;

        for (index, key) in GroupKey::ALL.iter().enumerate() {
            let digit = (index + 1).to_string();
            assert_eq!(
                bind(&ch(&digit), none()).as_deref(),
                Some(format!("GroupKeySelected({key:?})").as_str()),
                "{digit} should select {key:?}"
            );
        }
        // Named for what they are, so a reordering of `ALL` is a visible test
        // failure rather than a silently different wall.
        assert_eq!(
            bind(&ch("1"), none()).as_deref(),
            Some("GroupKeySelected(Artist)")
        );
        assert_eq!(
            bind(&ch("5"), none()).as_deref(),
            Some("GroupKeySelected(Played)")
        );
        // There is no sixth key and no zeroth one.
        assert_eq!(bind(&ch("6"), none()), None);
        assert_eq!(bind(&ch("0"), none()), None);
        // A modifier is not the binding, and a focused well types the digit.
        for modifiers in [Modifiers::COMMAND, Modifiers::ALT, Modifiers::SHIFT] {
            assert_eq!(bind(&ch("2"), modifiers), None, "{modifiers:?}");
        }
        assert!(binding_for(&ch("2"), none(), Focus::TextField).is_none());
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
            bind(&named(key::Named::MediaTrackPrevious), none()).as_deref(),
            Some("PreviousTrack")
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
            ch("a"),
            ch("z"),
            // `1`–`5` are the five group keys; `6` is not, because there is no
            // sixth key to select (see `group_key`).
            ch("6"),
            ch("0"),
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
