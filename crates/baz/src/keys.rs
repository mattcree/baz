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
//! saying whether a widget already consumed it, and that report is almost the
//! whole of the rule. [`Focus::TextField`] (iced said `Captured`) means the search
//! well took the key — typed it, moved its cursor, or dismissed itself — and
//! [`binding_for`] answers `None` for **every** key in that state. Space
//! types a space. `/` types a slash. `n` types an `n`.
//!
//! This is stronger than tracking focus ourselves would be. iced 0.13 exposes
//! no "is this widget focused" query a subscription could ask synchronously,
//! so any hand-kept flag would have to *infer* blur from clicks it cannot
//! locate — and the failure mode of a wrong guess is a space bar that pauses
//! the music mid-word. Deferring to the toolkit's own capture report cannot
//! be wrong, because it is the same decision the text field itself made a
//! moment earlier.
//!
//! There is one visible, state-bounded exception: while search results stand,
//! the chooser claims the four **bare arrows before capture status**. Its own
//! guide advertises `↑↓ select · ←→ action`; without that seam iced's focused
//! `text_input` would consume Left/Right as caret movement and the advertised
//! action axis would be unreachable. The first arrow blurs the well, so every
//! other key continues to obey the ordinary focus rule. While the search well
//! has focus no transport binding is live at all. Escape leaves it, after
//! which the transport keys work. (**Nothing has focus at startup**, so
//! this is something a listener walks into rather than the first thing they
//! meet: <kbd>Space</kbd> means play/pause on the first frame. This
//! parenthetical said the opposite until 2026-08-10 — the well *did* take
//! focus at startup, and `Shelf::open` stopped focusing it when type-anywhere
//! landed, which is the section immediately below. The README's key table says
//! the same thing.)
//!
//! # Type anywhere — and the focus rule is untouched
//!
//! ADR-0017 §1.2, build-plan step 11: **a bare printable character filters the
//! wall from wherever you are**, with no field to click first. The audit had
//! rejected this — *"type-ahead search cannot coexist with bare-letter
//! transport bindings; the transport wins"* — and that resolution is
//! superseded, because the frequency argument runs the other way: on a 40 000
//! album wall filtering is the primary act of navigation and muting is not,
//! and `n` / `m` / `q` were baz's own inventions rather than muscle memory
//! inherited from anything we surveyed. The budget the critique set is the
//! right one: *keystroke → filtered wall = next frame*, and a door you must
//! open first is a click before sound.
//!
//! **How a bare letter reaches the query without the focus rule bending.**
//! It does not need to bend, and this is the whole mechanism:
//!
//! 1. Nothing is focused, so iced reports the press `Ignored` —
//!    [`Focus::Elsewhere`] — and it is the application's to interpret.
//! 2. [`binding_for`] answers [`Message::QueryTyped`] carrying the character
//!    the key produced. It is the *last* `Character` arm in the table, so
//!    every binding above it wins; a key that means something is never text.
//! 3. `app.rs` appends the character to the query, re-filters, and focuses the
//!    well — one message, both halves, so the first keystroke both filters and
//!    lands somewhere visible.
//! 4. **Every keystroke after it is the field's**, by the ordinary rule: the
//!    well now has focus, iced reports `Captured`, and this function answers
//!    `None` for all of them. baz never types into a field it is also
//!    shortcutting.
//!
//! So exactly one press per query goes through this module, and the *field*
//! remains what holds the caret, the selection, the paste and the focus ring.
//! ADR-0017 refused the critique's removal of the well for that reason and for
//! §4's: `text_input` is the only focusable widget in baz and the only thing
//! an accessibility tree would have to attach to.
//!
//! **The query is drawn in the well**, at `SIZE_BODY`, and not as the
//! critique's ~48 px display type bottom-left: `02` §3.2 reserves poster sizes
//! for the *work*, and the critique put the query and the 11 px wall label in
//! the same bottom-left corner, where they would collide every time somebody
//! filtered while music was playing.
//!
//! # The modifier layer
//!
//! Bare letters being query is not free: **every bare-letter shortcut had to
//! move**, and three did. `q` → <kbd>Ctrl</kbd>+<kbd>U</kbd>, `m` →
//! <kbd>Ctrl</kbd>+<kbd>M</kbd>, and `n` gives up its letter to the second
//! spelling it already had, <kbd>Ctrl</kbd>+<kbd>→</kbd>. ADR-0017 §1.2 names
//! `M` as a defect in the critique's own scheme — it claimed all letters were
//! query and then bound bare `M` to mute four lines later — and resolves it
//! the consistent way, which is the way taken here: **no letter binds bare, at
//! all.** The mute glyph in the bar remains the pointer route, as the
//! visible-control rule requires.
//!
//! The number row is the one exception and it is argued below.
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
//! one knows the other. It never had a bare letter — `p` is not the reflex `n`
//! was, and it is one slip away from a *play* the transport already binds to
//! Space — and under type-anywhere no letter has one, so Previous and Next are
//! now spelled the same way as each other.
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
//! <kbd>Ctrl</kbd>+<kbd>M</kbd> mutes and unmutes. `M` is the letter every
//! player uses for it and it used to be bare; type-anywhere took the letter
//! and the modifier is where it went (ADR-0017 §1.2's keyboard table). Like
//! the transport keys it resolves against the *confirmed* state rather than a
//! flag we keep, so the command that goes out is the idempotent
//! `SetMute { muted }` the protocol asks for rather than a toggle two front
//! ends could disagree about.
//!
//! # Enter — what confirms
//!
//! <kbd>Enter</kbd> confirms the explicitly selected result and action while
//! the search chooser stands. Typing selects nothing: Up/Down or a pointer
//! first states the result, and Left/Right states a track's action. Outside
//! search, Enter activates the ordinary selected content.
//!
//! It is also the one binding that arrives by two roads and must mean the same
//! thing on both. With the well focused iced's `text_input` consumes
//! <kbd>Enter</kbd> and publishes its `on_submit`; with the well unfocused
//! this module binds it. Both are [`Message::PlayFirstMatch`], so which road a
//! press took is invisible.
//!
//! # Density — a zoom, on the modifier layer
//!
//! <kbd>Ctrl</kbd>+<kbd>-</kbd> and <kbd>Ctrl</kbd>+<kbd>=</kbd> step the
//! wall's density (ADR-0017 step 6, [`crate::shelf::Density`]), and
//! <kbd>Ctrl</kbd>+scroll is the same gesture with a pointer
//! ([`wheel_binding`]). Both are **accelerators of a visible control** — the
//! four detent marks (ADR-0028 as amended), which send the identical message
//! with the mirror delta — exactly as the digits accelerate the group keys'
//! row. The marks stand on every place that hangs works, so the keys have a
//! visible twin wherever they change anything. The two keys are the zoom pair every
//! browser, editor and image viewer has trained: the physical keys are
//! adjacent, they are *not* letters, and neither is anything baz could have
//! spent on the query.
//!
//! Shift is tolerated on both, because `+` is Shift+`=` on most layouts and a
//! listener pressing <kbd>Ctrl</kbd>+<kbd>+</kbd> means the same thing as one
//! pressing <kbd>Ctrl</kbd>+<kbd>=</kbd>. `_` joins `-` for the same reason.
//! There is no <kbd>Ctrl</kbd>+<kbd>0</kbd> reset: the marks name every step
//! and the default is one press from either neighbour, so a reset would be a
//! second way to reach a mark that is already on screen.
//!
//! **One toolkit limit, stated where it bites.** While the search well has
//! focus, iced's `text_input` swallows a chord whose key produces a printable
//! character — `Ctrl+-`, `Ctrl+=`, `Ctrl+,` — because it inserts what the
//! press *produced* and only checks the command modifier for its own four
//! clipboard chords. The focus rule says a captured press is the field's and
//! that rule does not bend, so those chords do nothing there rather than
//! reaching this table. What they must not do is type themselves into the
//! query, and `app.rs`'s `update_modified_input` is where that is stopped —
//! with its measurement. <kbd>Esc</kbd> leaves the field, and
//! <kbd>Ctrl</kbd>+scroll never had the problem.
//!
//! # Layers
//!
//! Two keys move between what is on screen, and each one names exactly one
//! *place* — there being nothing else left to name (ADR-0022). There were
//! three; the third is dealt with below.
//!
//! <kbd>Ctrl</kbd>+<kbd>U</kbd> goes to **Now playing** — *up next*, which is
//! exactly the half of that surface the queue place used to
//! be. It was bare `Q` and ADR-0017 §1.2's table moves it, for the reason every
//! letter moved: bare letters are the query now. The old argument for `Q` being
//! bare — a view key you press dozens of times a session should not be taxed —
//! is real and is simply outbid, and the tax it now pays is one modifier.
//!
//! **It stops toggling**, and that is the mirror rule rather than a loss: it is
//! now the accelerator of the lane's own `Now playing` row, and
//! [`crate::place::Place::go`] settles what a destination does — *pressing the
//! row you are already on must leave you there*. A key that closed what its
//! visible twin does not close would be a second behaviour with no control.
//! <kbd>Esc</kbd> is the way out, and always was.
//!
//! **It used to turn the run on as well, and no longer has to.** That second
//! half was legal by ADR-0023's amendment — an accelerator may send the two
//! messages its visible controls send — and it is now moot: the owner removed
//! the place's `Run` word (2026-08-10), the run column stands whenever there is
//! a run, and the chord resolves to the one destination message the lane's row
//! and the bar's now-playing block already send. **One message, three visible
//! twins**, which is a stronger legality than the construction it replaces.
//!
//! What it reaches is its fourth home: a queue *panel* in the right-hand rail,
//! then a popover anchored to the bar, then a **place** of its own (ADR-0022),
//! and now the other half of the place that was already showing the cursor
//! (`docs/design/12-now-playing-and-kiosk.md` §3.4).
//!
//! `U` rather than `Q` because the two are not interchangeable under Ctrl:
//! <kbd>Ctrl</kbd>+<kbd>Q</kbd> is *quit* on every desktop baz runs on, and a
//! key that closes the application on a listener expecting a popover is the
//! worst possible mis-key. `U` is `Up next`, it is unclaimed, and it is what
//! the ADR's table names.
//!
//! **<kbd>Ctrl</kbd>+<kbd>B</kbd> is gone.** It hid the album inspector and
//! brought it back — the sidebar reflex from every editor written this decade —
//! and it earned its modifier by being the *layout* key, the one that changed
//! how much room the shelf got. ADR-0022 removed every side surface baz had, so
//! there is no layout to change and no sidebar to toggle. The key is left
//! **unbound** rather than given something else to do: a reflex that survives a
//! redesign pointing at a new meaning is worse than one that simply stops.
//!
//! <kbd>Ctrl</kbd>+<kbd>,</kbd> (<kbd>Cmd</kbd>+<kbd>,</kbd>) goes to the
//! **Settings place** and comes back ([`crate::place`]). Same key as before; it
//! is now *navigation* rather than a request to raise a panel, which is what
//! the macOS convention it borrows has always meant — and it is the same
//! message the top bar's `Settings` control and the place's own Back both send.
//!
//! <kbd>Cmd</kbd>+<kbd>,</kbd> is a macOS system convention that every
//! cross-platform application has adopted, which makes it the one binding here
//! a listener is more likely to already know than to learn. A bare `,` is not
//! available in any case: it is a printable character, and printable
//! characters are the query.
//!
//! <kbd>Esc</kbd> peels **one layer, top down**: the popover first, then a
//! place that is not home, then the query, then the inspector. It never has to
//! choose between unrelated things, because at each press exactly one layer is
//! the top one. The layering itself lives in `app.rs`, where the layers do;
//! this module only says that the key means "peel".
//!
//! Under type-anywhere the query layer is the one that matters, and the order
//! is why <kbd>Esc</kbd> *clears* before it blurs: a listener who has typed
//! three letters into a wall wants the wall back, not the caret moved. iced's
//! `text_input` consumes <kbd>Esc</kbd> to blur itself first, so what a
//! listener actually presses is **Esc, Esc** — blur, then clear — and the
//! second press reaches [`Message::EscapePressed`] through this module. That
//! is a toolkit limit rather than a design choice and it is recorded as one in
//! `app.rs`'s `escape`.
//!
//! # <kbd>Ctrl</kbd>+<kbd>R</kbd> is free again
//!
//! It was **the pull** — the one binding ADR-0017 §1.2's key table spent on a
//! *draw*. The owner removed the control on 2026-08-10 and the key went with
//! it: baz does not keep an accelerator for an act it no longer has, and a
//! chord left bound to nothing is a chord that does something surprising the
//! next time somebody reaches for it.
//!
//! Shuffle gets **no** key either, still deliberately. The rule is
//! one-directional — every action needs a visible control, not every control
//! needs a key — and the shortcuts baz spends are for the things a hand does
//! dozens of times a session. Turning shuffle on is a standing decision made
//! once, from a control that is on screen in every place (`crate::views::bottom_bar`).
//!
//! # The arrangement — `1` … `6`
//!
//! The six group keys (ADR-0019) select from the number row: `1` A–Z,
//! `2` ARTIST, `3` YEAR, `4` GENRE, `5` ADDED, `6` PLAYED, in the order the
//! top bar's row of words states them and the order
//! [`GroupKey::ALL`](baz_core::index::GroupKey::ALL) publishes them. The
//! mapping is [`group_key`], which reads that array rather than repeating it,
//! so the digits, the words and the library's own order cannot drift apart —
//! and it is why restoring `A–Z` at the head of the row moved every other
//! key's digit without an edit here.
//!
//! **Digits, deliberately — and they are the one place bare characters are not
//! query.** ADR-0017 §1.2 states the trade out loud: *digits are not letters,
//! and no album title begins with one often enough to matter*. Type-anywhere
//! (step 11) took every letter and every punctuation mark for the query; the
//! number row is what survived, and this is where it is spent. The ADR's table
//! pencils `1` and `2` in for the Wall / Marquee lenses, which are step 18 and
//! not built; the six group keys are built, they are six and not two, and a
//! row of words in the top bar already names them.
//!
//! `6` has been bound before, to ADR-0035's `ARTISTS` — a word that was not a
//! key. It is a key now, `PLAYED`, and it binds for the ordinary reason every
//! other digit does.
//!
//! **The whole row is out of the query, not just the six keys that bind.**
//! `0` and `7`–`9` do nothing rather than typing themselves, because a row in
//! which `1` arranges the wall and `7` types a `7` is two rules wearing one
//! shape. The cost is stated rather than discovered: from a cold wall you
//! cannot type `1999`. You press `/` first — which is what `/` is for — and
//! from that moment the well has focus and every digit types, including the
//! first one.
//!
//! Each resolves to [`Message::GroupKeySelected`], the same message the word
//! in the top bar sends, so the visible-control rule holds: there is no
//! arrangement reachable only from the keyboard.
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
    // The zoom pair alone tolerates Shift, because `+` *is* Shift+`=` on most
    // layouts and `_` is Shift+`-` (module docs). No other binding does.
    let zoom = command || modifiers == Modifiers::COMMAND | Modifiers::SHIFT;

    match key.as_ref() {
        // Transport. Space is the universal play/pause and takes no
        // modifiers at all — Ctrl+Space and friends belong to whatever binds
        // them later. It is a printable character and is deliberately *not*
        // query: a space cannot start a search, and the one key every player
        // on earth pauses with does not become a text key because the letters
        // around it did.
        Key::Named(key::Named::Space) | Key::Character(" ") if bare => Some(Message::PlayPause),
        // Ctrl+Right (Cmd+Right on macOS) is Next — the only spelling it has
        // now that bare `n` is query — and Ctrl+Left is its mirror, Previous
        // (module docs).
        Key::Named(key::Named::ArrowRight) if command => Some(Message::NextTrack),
        Key::Named(key::Named::ArrowLeft) if command => Some(Message::PreviousTrack),

        // Seeking. Shift widens the step; nothing else may ride along.
        Key::Named(key::Named::ArrowRight) if bare => {
            Some(Message::Direction(crate::search::Direction::Right))
        }
        Key::Named(key::Named::ArrowRight) if shift => Some(Message::SeekBy(SEEK_STEP_LARGE_MS)),
        Key::Named(key::Named::ArrowLeft) if bare => {
            Some(Message::Direction(crate::search::Direction::Left))
        }
        Key::Named(key::Named::ArrowLeft) if shift => Some(Message::SeekBy(-SEEK_STEP_LARGE_MS)),

        // Volume. The vertical arrows are the axis a fader moves on — they are
        // not printable and keep their bare bindings — and mute moved to the
        // modifier layer with every other letter (module docs).
        Key::Named(key::Named::ArrowUp) if bare => {
            Some(Message::Direction(crate::search::Direction::Up))
        }
        Key::Named(key::Named::ArrowDown) if bare => {
            Some(Message::Direction(crate::search::Direction::Down))
        }
        Key::Character("m" | "M") if command => Some(Message::ToggleMute),

        // Places. Ctrl+U goes to Now playing — what is playing and what is
        // **up next**, which since the `Run` word went is one surface with no
        // density to ask for — and Ctrl+`,` (Cmd+`,`) to the settings; both are
        // navigation rather than requests to raise a layer (module docs). It
        // does not toggle: it is the accelerator of a destination, and a
        // destination never closes itself.
        Key::Character("u" | "U") if command => Some(Message::ShowNowPlaying),
        Key::Character(",") if command => Some(Message::ToggleSettings),

        // The playlist panel's door (ADR-0024 §5): the same press as the
        // Library strip's labelled `Playlists` word. `P` is the word's own
        // letter and it is unclaimed under the command modifier; bare `p` is
        // the query, like every letter (ADR-0017 §1.2), and a door is
        // modified — L8.7's layer table, the same argument that moved `Q`.
        Key::Character("p" | "P") if command => Some(Message::TogglePlaylists),
        // **Ctrl+B returns.** Doc 07 §5.3 deleted it because *"its subject was
        // a sidebar that no longer exists"*; ADR-0030 built the sidebar again,
        // its meaning is unchanged, and so the key comes back with it. It is
        // the accelerator of the two marks at the lane's foot, never the
        // control itself (the mirror rule).
        Key::Character("b" | "B") if command => Some(Message::ToggleLane),

        // The zoom. Ctrl+`-` tightens the hang, Ctrl+`=` loosens it, and the
        // shifted spellings of both keys mean the same thing (module docs).
        Key::Character("-" | "_") if zoom => Some(Message::DensityStep(-1)),
        Key::Character("=" | "+") if zoom => Some(Message::DensityStep(1)),

        // Undo (doc 11 §5 P2): the universal chord, over the transient
        // `Undo` word the edited place carries — the visible twin that
        // makes the accelerator legal (doc 09 §5.2's construction). Which
        // list surface answers is the shell's business; anywhere without an
        // edit history, the chord falls dead.
        Key::Character("z" | "Z") if command => Some(Message::Undo),

        // Arrangement. `1`–`6` are the six group keys, in the order the top
        // bar's row of words states them (module docs).
        Key::Character(digit @ ("1" | "2" | "3" | "4" | "5" | "6")) if bare => {
            Some(Message::GroupKeySelected(group_key(digit)?))
        }
        // Search. `/` is the reflex from every pager and browser; Ctrl+F
        // (Cmd+F) is the reflex from every document. Shift is tolerated on
        // `/` because plenty of layouts need it to type the character. Both
        // survive type-anywhere as the *explicit* door: a listener who wants
        // the caret before they want a filter, and the only way to start a
        // query with a digit.
        Key::Character("/") if bare || shift => Some(Message::FocusSearch),
        Key::Character("f" | "F") if command => Some(Message::FocusSearch),

        // Confirm the open chooser, else activate selected content (module docs).
        Key::Named(key::Named::Enter) if bare => Some(Message::PlayFirstMatch),

        // Peel one layer, top down (module docs; `app.rs` holds the order).
        Key::Named(key::Named::Escape) if bare => Some(Message::EscapePressed),

        // **Type anywhere.** The last `Character` arm in the table, so every
        // binding above wins and a key that means something is never text
        // (module docs). Shift rides along because a capital letter is a
        // letter.
        Key::Character(text) if (bare || shift) && is_query_text(text) => {
            Some(Message::QueryTyped(text.to_owned()))
        }

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
/// [`GroupKey::ALL`](baz_core::index::GroupKey::ALL) and `6` is the last.
///
/// Derived from the enum's own order rather than written out, so the row of
/// words in the top bar, the digits that select them and the order
/// `baz-core` publishes are one list. A seventh key (CRATES, MOOD) becomes
/// `7` here with one character's edit in the table above and none at all in
/// this function.
fn group_key(digit: &str) -> Option<baz_core::index::GroupKey> {
    let index = digit.parse::<usize>().ok()?.checked_sub(1)?;
    baz_core::index::GroupKey::ALL.get(index).copied()
}

/// **Whether the text a key produced is query text** — the whole of what
/// "any bare printable character" means (module docs).
///
/// Three exclusions and each one is a binding that outranks the query:
///
/// - **whitespace and controls**, because a query cannot begin with a space
///   and `Space` is play/pause;
/// - **the ASCII digits**, because the number row selects the arrangement and
///   the row is spent as a row rather than key by key;
/// - **`/`**, which is the explicit door to the well and the one way to start
///   a query with a digit.
///
/// Everything else is text, including punctuation an album title actually
/// contains — `!!!`, `Sgt. Pepper`, `AC/DC`'s slash notwithstanding — and
/// every letter of every script, because this asks what the *key produced*
/// rather than which key it was.
///
/// A multi-character string (a dead key resolving, an input method
/// committing) is text when every one of its characters is, which is the same
/// rule applied the same way rather than a special case.
fn is_query_text(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| !c.is_control() && !c.is_whitespace() && !c.is_ascii_digit() && c != '/')
}

/// **Whether an edit the search field published is really query text.**
///
/// The other side of [`is_query_text`], for the path where the *field* saw the
/// key rather than this module. iced 0.13's `text_input` inserts whatever
/// character a press produced and consults the command modifier for its own
/// four clipboard chords only, so `Ctrl+-` typed a hyphen into the query
/// (measured; see `app.rs`'s `update_modified_input`). One rule, both paths:
/// **a keystroke made with the command modifier is never query text.**
///
/// Only the command modifier. Shift is a capital letter, and `Alt` is
/// deliberately absent: winit reports `AltGr` as its own level shift rather
/// than as `Alt`, so a press that composes `€` or `é` on a European layout
/// arrives here unmodified and must go on working — discarding `Alt` would
/// have bought nothing and cost those keyboards their letters.
pub(crate) fn field_edit_is_query(modifiers: Modifiers) -> bool {
    !modifiers.command()
}

/// **<kbd>Ctrl</kbd>+scroll is the zoom's pointer accelerator** — the same
/// gesture as <kbd>Ctrl</kbd>+<kbd>-</kbd> / <kbd>Ctrl</kbd>+<kbd>=</kbd>,
/// over the same message the density marks at the foot of the index rail's
/// lane send (ADR-0028). The visible-control rule is met by the marks; the
/// gesture is the fast route, the way a key is, and its old claim to *be*
/// the control is retired — a gesture-only action was the contradiction
/// doc 11 §5 P8 named.
///
/// Pure, like [`binding_for`], and for the same reason: it is one decision
/// about one event, and the state it would otherwise need — which modifiers
/// are down — is passed in. iced 0.13's `WheelScrolled` carries no modifiers
/// of its own, so `app.rs` tracks them from `ModifiersChanged` and hands them
/// here.
///
/// A scroll **up** loosens the hang, which is the direction every zoom in
/// every application agrees on. Anything without the command modifier, and any
/// notch with no vertical travel, is the wall scrolling and is not this.
pub(crate) fn wheel_binding(delta_y: f32, modifiers: Modifiers) -> Option<Message> {
    if !modifiers.command() || delta_y == 0.0 || !delta_y.is_finite() {
        return None;
    }
    Some(Message::DensityStep(if delta_y > 0.0 { 1 } else { -1 }))
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
            (ch("m"), Modifiers::COMMAND),
            (ch("u"), Modifiers::COMMAND),
            (ch(","), Modifiers::COMMAND),
            (ch("-"), Modifiers::COMMAND),
            (ch("="), Modifiers::COMMAND),
            (ch("/"), none()),
            (ch("f"), Modifiers::COMMAND),
            (ch("1"), none()),
            (ch("5"), none()),
            // The type-anywhere arm itself: a bare letter is query when
            // nothing is focused and is *typed* when the well is, which is the
            // one place the focus rule is doing the most work.
            (ch("k"), none()),
            (ch("K"), Modifiers::SHIFT),
            (ch("&"), none()),
            (named(key::Named::Enter), none()),
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
            Some("Direction(Right)")
        );
        assert_eq!(
            bind(&named(key::Named::ArrowLeft), none()).as_deref(),
            Some("Direction(Left)")
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
    fn the_vertical_arrows_route_by_open_surface() {
        assert_eq!(
            bind(&named(key::Named::ArrowUp), none()).as_deref(),
            Some("Direction(Up)")
        );
        assert_eq!(
            bind(&named(key::Named::ArrowDown), none()).as_deref(),
            Some("Direction(Down)")
        );
    }

    /// **Mute moved to the modifier layer**, in either case — and bare `m` is
    /// now a letter of the query, which is the whole of ADR-0017 §1.2's
    /// resolution of the critique's own `M`-while-all-letters-are-query
    /// defect.
    #[test]
    fn ctrl_m_mutes_and_bare_m_is_query() {
        assert_eq!(
            bind(&ch("m"), Modifiers::COMMAND).as_deref(),
            Some("ToggleMute")
        );
        assert_eq!(
            bind(&ch("M"), Modifiers::COMMAND).as_deref(),
            Some("ToggleMute")
        );
        assert_eq!(bind(&ch("m"), none()).as_deref(), Some("QueryTyped(\"m\")"));
        assert_eq!(
            bind(&ch("M"), Modifiers::SHIFT).as_deref(),
            Some("QueryTyped(\"M\")")
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

    /// **Next has one spelling now.** `n` was the bare letter and the arrow
    /// cluster was its second spelling; type-anywhere took the letter and
    /// ADR-0017 §1.2's table leaves the arrow, which is also the mirror of
    /// Previous.
    #[test]
    fn next_track_is_the_modified_arrow_and_n_is_query() {
        assert_eq!(
            bind(&named(key::Named::ArrowRight), Modifiers::COMMAND).as_deref(),
            Some("NextTrack")
        );
        assert_eq!(bind(&ch("n"), none()).as_deref(), Some("QueryTyped(\"n\")"));
        assert_eq!(
            bind(&ch("N"), Modifiers::SHIFT).as_deref(),
            Some("QueryTyped(\"N\")")
        );
        // And Ctrl+N is not a second spelling either: it is unbound, so a hand
        // reaching for the old key finds nothing rather than something else.
        assert_eq!(bind(&ch("n"), Modifiers::COMMAND), None);
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
        // …and bare Left routes through the open surface while Shift still
        // asks the transport for its larger seek.
        assert_eq!(
            bind(&named(key::Named::ArrowLeft), none()).as_deref(),
            Some("Direction(Left)")
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

    /// **Ctrl+U goes to Now playing**, in either case — and bare `q` is query,
    /// along with Ctrl+Q, which is *quit* everywhere else and must not be a
    /// place here.
    ///
    /// `Q` has not opened the queue since ADR-0017 §1.2 spent every bare letter
    /// on the query, whatever two stale doc comments used to say; the assertion
    /// below is where that has been true all along.
    #[test]
    fn ctrl_u_is_the_run_and_bare_q_is_the_query() {
        assert_eq!(
            bind(&ch("u"), Modifiers::COMMAND).as_deref(),
            Some("ShowNowPlaying")
        );
        assert_eq!(
            bind(&ch("U"), Modifiers::COMMAND).as_deref(),
            Some("ShowNowPlaying")
        );
        assert_eq!(bind(&ch("q"), none()).as_deref(), Some("QueryTyped(\"q\")"));
        assert_eq!(
            bind(&ch("q"), Modifiers::COMMAND),
            None,
            "Ctrl+Q closes applications; it must not open a popover"
        );
    }

    /// **`Ctrl+B` is the returns lane's, again.** Bare `b` is a letter of the
    /// query, as every bare letter is.
    ///
    /// It hid the album inspector and brought it back, and it earned its
    /// modifier by being *the layout key* — the one that changed how much room
    /// the shelf got. Doc 07 §5.3 retired it when ADR-0022 left no sidebar to
    /// hide: *"a reflex that survives a redesign pointing at a new meaning is
    /// worse than one that stops."*
    ///
    /// ADR-0030 built the subject again. The key returns because **its meaning
    /// is unchanged** — it is still the one key that changes how much room the
    /// collection gets — which is the only condition on which a retired reflex
    /// may be revived. It is the accelerator of the two marks at the lane's
    /// foot, never the control itself.
    #[test]
    fn ctrl_b_collapses_the_returns_lane_again() {
        for key in [ch("b"), ch("B")] {
            assert_eq!(
                bind(&key, Modifiers::COMMAND).as_deref(),
                Some("ToggleLane"),
                "{key:?}"
            );
        }
        // The shifted chord stays unbound: one key, one meaning.
        assert_eq!(bind(&ch("b"), Modifiers::COMMAND | Modifiers::SHIFT), None);
        assert_eq!(bind(&ch("b"), none()).as_deref(), Some("QueryTyped(\"b\")"));
    }

    /// Ctrl+`,` (Cmd+`,`) is the preferences reflex from every platform, and it
    /// is navigation between places now rather than a panel toggle.
    /// A bare comma is query — the modifier is the whole binding.
    #[test]
    fn ctrl_comma_navigates_to_the_settings() {
        assert_eq!(
            bind(&ch(","), Modifiers::COMMAND).as_deref(),
            Some("ToggleSettings")
        );
        assert_eq!(bind(&ch(","), none()).as_deref(), Some("QueryTyped(\",\")"));
        assert_eq!(
            bind(&ch(","), Modifiers::SHIFT).as_deref(),
            Some("QueryTyped(\",\")")
        );
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
        // Bare `f` is a letter of the query; it is not a shortcut.
        assert_eq!(bind(&ch("f"), none()).as_deref(), Some("QueryTyped(\"f\")"));
    }

    /// **`1`–`6` are the six group keys, in `baz-core`'s own order**, and the
    /// mapping is that order rather than a copy of it — which is why `A–Z`
    /// taking the head of the row moved every other key's digit by one with no
    /// list to keep in step.
    #[test]
    fn the_number_row_selects_the_six_arrangements() {
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
            Some("GroupKeySelected(Alphabet)")
        );
        assert_eq!(
            bind(&ch("2"), none()).as_deref(),
            Some("GroupKeySelected(Artist)")
        );
        assert_eq!(
            bind(&ch("6"), none()).as_deref(),
            Some("GroupKeySelected(Played)")
        );
        // **And there is no seventh word and no zeroth one.**
        assert_eq!(bind(&ch("7"), none()), None);
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

    /// **Enter confirms the explicit search choice**, and it is the same
    /// message the well's own `on_submit` sends, so the two roads a press can
    /// take are one intention (module docs, ADR-0036's interaction correction).
    #[test]
    fn enter_confirms_the_search_choice() {
        assert_eq!(
            bind(&named(key::Named::Enter), none()).as_deref(),
            Some("PlayFirstMatch")
        );
        // Bare only: a modified Enter is somebody else's binding, not a
        // quieter way to start the music.
        for modifiers in [Modifiers::COMMAND, Modifiers::SHIFT, Modifiers::ALT] {
            assert_eq!(
                bind(&named(key::Named::Enter), modifiers),
                None,
                "{modifiers:?}"
            );
        }
    }

    /// **Ctrl+Z is undo** (doc 11 §5 P2), and only Ctrl+Z: bare `z` is the
    /// query, and a further modifier is somebody else's chord (there is no
    /// redo, so Ctrl+Shift+Z means nothing rather than something surprising).
    #[test]
    fn ctrl_z_is_undo_and_bare_z_is_still_the_query() {
        assert_eq!(bind(&ch("z"), Modifiers::COMMAND).as_deref(), Some("Undo"));
        assert_eq!(bind(&ch("Z"), Modifiers::COMMAND).as_deref(), Some("Undo"));
        assert_eq!(bind(&ch("z"), none()).as_deref(), Some("QueryTyped(\"z\")"));
        assert_eq!(bind(&ch("z"), Modifiers::COMMAND | Modifiers::SHIFT), None);
        assert_eq!(bind(&ch("z"), Modifiers::ALT), None);
        assert!(binding_for(&ch("z"), Modifiers::COMMAND, Focus::TextField).is_none());
    }

    /// **Type anywhere**: a bare printable character is the query, in every
    /// script, with punctuation, and with Shift for its capitals — and a
    /// *modified* one never is.
    #[test]
    fn a_bare_printable_character_is_the_query_and_a_modified_one_is_not() {
        for text in ["k", "z", "é", "曲", "ß", "&", "!", "'", "-", ".", "?", ","] {
            assert_eq!(
                bind(&ch(text), none()).as_deref(),
                Some(format!("QueryTyped({text:?})").as_str()),
                "bare {text:?} should reach the query"
            );
            // Shift is a capital, not a modifier that suppresses.
            assert_eq!(
                bind(&ch(text), Modifiers::SHIFT).as_deref(),
                Some(format!("QueryTyped({text:?})").as_str()),
                "Shift+{text:?} should reach the query"
            );
        }
        // A capital arrives as its own text, and it arrives verbatim: the
        // query is what the key produced, not what we guessed it meant.
        assert_eq!(
            bind(&ch("K"), Modifiers::SHIFT).as_deref(),
            Some("QueryTyped(\"K\")")
        );
        // A multi-character commit (a dead key resolving, an input method)
        // is text by the same rule.
        assert_eq!(bind(&ch("ǽ"), none()).as_deref(), Some("QueryTyped(\"ǽ\")"));

        // **A modified character is never query.** Ctrl and Alt belong to the
        // modifier layer, whether or not the key they are on binds anything.
        for text in ["k", "z", "j", "&", "é"] {
            for modifiers in [
                Modifiers::COMMAND,
                Modifiers::ALT,
                Modifiers::LOGO,
                Modifiers::COMMAND | Modifiers::SHIFT,
            ] {
                assert!(
                    !format!("{:?}", bind(&ch(text), modifiers)).contains("QueryTyped"),
                    "{text:?} + {modifiers:?} reached the query"
                );
            }
        }
        // And a focused well takes the letter itself — the focus rule, on the
        // arm that most needs it.
        assert!(binding_for(&ch("k"), none(), Focus::TextField).is_none());
    }

    /// The three characters a bare press does **not** send to the query, each
    /// because something above it in the table has the key (module docs on
    /// [`is_query_text`]).
    #[test]
    fn space_the_digits_and_the_slash_are_not_query() {
        assert_eq!(bind(&ch(" "), none()).as_deref(), Some("PlayPause"));
        assert_eq!(bind(&ch("/"), none()).as_deref(), Some("FocusSearch"));
        // The number row is spent as a row: the six that bind state the
        // wall's arrangement, and the four that do not are silent, so `1` and
        // `7` are one rule rather than two.
        for digit in ["1", "2", "3", "4", "5", "6"] {
            assert!(
                bind(&ch(digit), none())
                    .as_deref()
                    .is_some_and(|tag| tag.starts_with("GroupKeySelected")),
                "{digit} should arrange the wall"
            );
        }
        for digit in ["0", "7", "8", "9"] {
            assert_eq!(bind(&ch(digit), none()), None, "{digit} must not be query");
        }
        // The predicate itself, stated once rather than inferred from the
        // table above it.
        assert!(is_query_text("k") && is_query_text("&") && is_query_text("曲"));
        assert!(!is_query_text("") && !is_query_text(" ") && !is_query_text("\t"));
        assert!(!is_query_text("7") && !is_query_text("/") && !is_query_text("a/b"));
    }

    /// **The zoom, both keys and both shifted spellings** — and it is the only
    /// pair in the table that tolerates Shift alongside the command modifier.
    #[test]
    fn ctrl_minus_and_ctrl_equals_step_the_density() {
        for (text, tag) in [("-", "DensityStep(-1)"), ("=", "DensityStep(1)")] {
            assert_eq!(bind(&ch(text), Modifiers::COMMAND).as_deref(), Some(tag));
            assert_eq!(
                bind(&ch(text), Modifiers::COMMAND | Modifiers::SHIFT).as_deref(),
                Some(tag)
            );
        }
        // `_` and `+` are the same physical keys, shifted.
        assert_eq!(
            bind(&ch("_"), Modifiers::COMMAND | Modifiers::SHIFT).as_deref(),
            Some("DensityStep(-1)")
        );
        assert_eq!(
            bind(&ch("+"), Modifiers::COMMAND | Modifiers::SHIFT).as_deref(),
            Some("DensityStep(1)")
        );
        // Bare, they are query — `-` is in album titles and `=` is in a few.
        assert_eq!(bind(&ch("-"), none()).as_deref(), Some("QueryTyped(\"-\")"));
        assert_eq!(bind(&ch("="), none()).as_deref(), Some("QueryTyped(\"=\")"));
        // Alt is not the zoom.
        assert_eq!(bind(&ch("-"), Modifiers::ALT), None);
    }

    /// **One rule about modifiers, on both paths into the query.** A key this
    /// module sees and a key the *field* sees must agree about what is text,
    /// or `Ctrl+-` types a hyphen into the query — which it did, on a real
    /// frame, before [`field_edit_is_query`] existed.
    #[test]
    fn a_command_modified_keystroke_is_never_query_on_either_path() {
        for modifiers in [
            Modifiers::COMMAND,
            Modifiers::COMMAND | Modifiers::SHIFT,
            Modifiers::COMMAND | Modifiers::ALT,
        ] {
            assert!(!field_edit_is_query(modifiers), "{modifiers:?}");
            // …and the binding path says the same thing about the same press.
            assert!(
                !format!("{:?}", bind(&ch("-"), modifiers)).contains("QueryTyped"),
                "{modifiers:?}"
            );
        }
        // Everything else is text. Shift is a capital; Alt is AltGr's
        // neighbour and a European layout's letters must survive it.
        for modifiers in [Modifiers::empty(), Modifiers::SHIFT, Modifiers::ALT] {
            assert!(field_edit_is_query(modifiers), "{modifiers:?}");
        }
    }

    /// **Ctrl+scroll is the same gesture as Ctrl+`=`**, with the same sign
    /// convention, and a plain scroll is the wall scrolling rather than a
    /// zoom.
    #[test]
    fn ctrl_scroll_steps_the_density_and_a_plain_scroll_does_not() {
        let tag = |delta: f32, modifiers| {
            wheel_binding(delta, modifiers)
                .as_ref()
                .map(|message| format!("{message:?}"))
        };
        assert_eq!(
            tag(1.0, Modifiers::COMMAND).as_deref(),
            Some("DensityStep(1)"),
            "scrolling up loosens the hang, as every zoom does"
        );
        assert_eq!(
            tag(-1.0, Modifiers::COMMAND).as_deref(),
            Some("DensityStep(-1)")
        );
        // The magnitude is not spent: one notch is one step, so a trackpad's
        // 40-pixel flick does not fall through the whole ladder.
        assert_eq!(
            tag(120.0, Modifiers::COMMAND).as_deref(),
            Some("DensityStep(1)")
        );
        assert_eq!(
            tag(0.2, Modifiers::COMMAND | Modifiers::SHIFT).as_deref(),
            Some("DensityStep(1)")
        );
        // Without the modifier it is the wall scrolling, which is not this
        // module's business at all.
        for modifiers in [Modifiers::empty(), Modifiers::SHIFT, Modifiers::ALT] {
            assert_eq!(tag(3.0, modifiers), None, "{modifiers:?}");
        }
        // A notch with no travel, and the values a broken backend can hand us.
        assert_eq!(tag(0.0, Modifiers::COMMAND), None);
        assert_eq!(tag(f32::NAN, Modifiers::COMMAND), None);
        assert_eq!(tag(f32::INFINITY, Modifiers::COMMAND), None);
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
    /// down. Each case here is a key that *does* bind under some other
    /// modifier state — including the letters, whose bare press is now the
    /// query.
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
            (ch("m"), Modifiers::ALT),
            (ch("m"), Modifiers::COMMAND | Modifiers::SHIFT),
            (ch("u"), Modifiers::ALT),
            (ch("u"), Modifiers::COMMAND | Modifiers::SHIFT),
            (ch("b"), Modifiers::ALT),
            (ch("b"), Modifiers::COMMAND | Modifiers::SHIFT),
            (ch("/"), Modifiers::COMMAND),
            (ch("f"), Modifiers::ALT),
            (ch("1"), Modifiers::COMMAND),
            (ch("1"), Modifiers::ALT),
            (named(key::Named::Enter), Modifiers::COMMAND),
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
            named(key::Named::Tab),
            named(key::Named::Backspace),
            named(key::Named::Delete),
            named(key::Named::Home),
            named(key::Named::End),
            named(key::Named::PageUp),
            named(key::Named::F1),
            // `1`–`6` are the six group keys; the rest of the row is silent,
            // because the row is spent as a row (module docs).
            ch("7"),
            ch("0"),
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
