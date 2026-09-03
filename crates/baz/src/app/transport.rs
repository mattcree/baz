//! **The transport as one small machine** — play, pause, stop, previous and
//! next; the needle; the fader and mute; the settings panel's ReplayGain
//! controls. Every message here resolves to a single [`Command`] and goes to
//! the engine, and none of them touches a thing the interface displays: the
//! state machine follows the engine's confirming event and nothing else
//! (`player.rs`'s honesty rule).
//!
//! Moved out of `app.rs` on 2026-09-03 as step 2 of the split `BACKLOG.md`
//! proposes — a pure move, the `pub(super)` on each method and the module
//! path on one doc link being the whole diff.

use std::time::{Duration, Instant};

use baz_core::protocol::Command;
use baz_core::replaygain::ReplayGainSettings;

use super::{App, Message, persist};
use crate::player;

/// Quiet time after the last fader wheel step before its confirmed position is
/// persisted. Audio answers every step immediately; disk sees one settled act.
pub(super) const VOLUME_WHEEL_SETTLE: Duration = Duration::from_millis(240);

impl App {
    /// Write the fader position the engine has just confirmed, if it moved.
    ///
    /// `VolumeChanged` also reports mute and output-path changes. Comparing
    /// only the control position means those independent facts never turn
    /// into config writes, while drag, keyboard and MPRIS volume gestures all
    /// share this one confirmation-driven persistence path.
    pub(super) fn persist_volume(&mut self) {
        if self.player.volume_gesture_active() || self.volume_wheel_settles.is_some() {
            return;
        }
        let volume = self.player.volume();
        if volume == self.saved_volume {
            return;
        }
        self.saved_volume = volume;
        persist(|config| config.volume = volume);
    }

    /// Send an accepted seek target to the engine. `None` means there was
    /// nothing honest to seek to and nothing was asked for; the state machine
    /// has already recorded an accepted request as pending, and the bar keeps
    /// showing it until an event confirms (see `player.rs`).
    pub(super) fn send_seek(&mut self, target: Option<u64>) {
        if let Some(position_ms) = target
            && !self.playback.send(Command::Seek { position_ms })
        {
            self.player.engine_closed();
        }
    }

    /// Answer a transport message, reporting whether it was one.
    ///
    /// The six of them are one small machine, exactly as the volume's nine and
    /// ReplayGain's four are: each resolves to a single
    /// [`Command`] and goes out through
    /// [`Self::send_transport`], and none of them touches a single thing the
    /// interface displays — the state machine follows the engine's confirming
    /// event and nothing else (`player.rs`'s honesty rule).
    ///
    /// Play, Pause and Stop have no button of their own: a desktop media
    /// widget (and a media key) asks for a *direction* rather than a toggle,
    /// where the bar's control covers both. Previous, Next and the toggle do,
    /// and they arrive here from the button, the keyboard and MPRIS as the
    /// same message.
    pub(super) fn update_transport(&mut self, message: &Message) -> bool {
        let command = match *message {
            // The same reading the glyph is drawn from, so a press asks for
            // exactly what the button was showing (Play also resumes a paused
            // engine, so a stale read is still safe).
            Message::PlayPause => match self.player.play_pause() {
                player::PlayPause::Pause => Command::Pause,
                player::PlayPause::Play => Command::Play,
            },
            Message::NextTrack => Command::Next,
            Message::PreviousTrack => Command::Previous,
            Message::Play => Command::Play,
            Message::Pause => Command::Pause,
            Message::Stop => Command::Stop,
            _ => return false,
        };
        self.send_transport(command);
        true
    }

    /// Answer a volume message, reporting whether it was one.
    ///
    /// Every arm follows the same shape as the seek bar's: the state machine
    /// decides what — if anything — to ask for from event-derived state, and
    /// the answer goes to the engine. Nothing here writes the volume the
    /// interface displays; only `Event::VolumeChanged` does (see `player.rs`).
    /// The needle's five pointer messages, answered together for the same
    /// reason the volume's nine are: every one of them resolves to "tell the
    /// state machine, maybe tell the engine".
    ///
    /// The player resolves the gesture to a current-song timestamp; this
    /// method only dispatches the resulting `Seek` command.
    pub(super) fn update_needle(&mut self, message: &Message) -> bool {
        match *message {
            Message::NeedlePressed(pointer) => {
                self.player.press(pointer);
            }
            Message::NeedleDragged(pointer) => self.player.drag_to(pointer),
            Message::NeedleHovered(pointer) => {
                self.player.hover_to(pointer);
            }
            Message::NeedleLeft => self.player.hover_left(),
            Message::NeedleReleased => {
                let position_ms = self.player.release_drag();
                self.send_seek(position_ms);
            }
            _ => return false,
        }
        true
    }

    pub(super) fn update_volume(&mut self, message: &Message) -> bool {
        match *message {
            Message::VolumePressed(pointer) => {
                let target = self.player.press_volume(pointer);
                self.send_volume(target);
            }
            Message::VolumeDragged(pointer) => {
                let target = self.player.drag_volume(pointer);
                self.send_volume(target);
            }
            Message::VolumeHovered(pointer) => self.player.hover_volume(pointer),
            Message::VolumeLeft => self.player.volume_left(),
            Message::VolumeReleased => {
                self.player.release_volume();
                // Confirmed positions heard during the drag were deliberately
                // not written one pixel at a time. Commit the latest one when
                // the hand lets go; a final in-flight confirmation will update
                // it once more when it arrives.
                self.persist_volume();
            }
            Message::VolumeWheel(steps) => {
                let target = self.player.step_volume(steps);
                self.send_volume(target);
                if target.is_some() && self.player.engine_ready() {
                    self.volume_wheel_settles = Some(Instant::now() + VOLUME_WHEEL_SETTLE);
                }
            }
            Message::VolumeWheelSettled(now) => {
                if self.volume_wheel_settles.is_some_and(|at| now >= at) {
                    self.volume_wheel_settles = None;
                    self.persist_volume();
                }
            }
            Message::SetVolume(position) => {
                let target = self.player.set_volume(position);
                self.send_volume(target);
            }
            Message::ToggleMute => {
                let muted = self.player.toggle_mute();
                self.send_mute(muted);
            }
            Message::SetMute(muted) => {
                let requested = self.player.set_muted(muted);
                self.send_mute(requested);
            }
            _ => return false,
        }
        true
    }

    /// Send an accepted volume position to the engine. `None` means there was
    /// no engine to ask and nothing was requested. Nothing about the fader's
    /// reading moves here: the state machine recorded the request as pending
    /// for the view's benefit, and the position itself changes only when
    /// `Event::VolumeChanged` says so (see `player.rs`).
    pub(super) fn send_volume(&mut self, target: Option<u16>) {
        if let Some(position) = target
            && !self.playback.send(Command::SetVolume { position })
        {
            self.player.engine_closed();
        }
    }

    /// Send an accepted mute state to the engine. Idempotent by protocol —
    /// `SetMute { muted }`, never a toggle (ADR-0011 §3).
    pub(super) fn send_mute(&mut self, target: Option<bool>) {
        if let Some(muted) = target
            && !self.playback.send(Command::SetMute { muted })
        {
            self.player.engine_closed();
        }
    }

    /// The settings panel's ReplayGain controls, answered here for
    /// [`Self::update_volume`]'s reason: every one of them resolves to "ask
    /// the engine for a complete setting", so they are four arms of one small
    /// machine rather than four more in the shelf's update loop.
    ///
    /// Returns whether the message was one of them.
    ///
    /// Nothing on screen moves in any of these arms. The state machine keeps
    /// following [`baz_core::protocol::Event::ReplayGainChanged`] and nothing else, so a press
    /// that the engine clamps, refuses, or answers differently from is
    /// rendered as the engine's answer (ADR-0013, and `crate::replaygain`).
    pub(super) fn update_replay_gain(&mut self, message: &Message) -> bool {
        let state = self.player.replay_gain();
        let asked = match *message {
            Message::ReplayGainMode(mode) => state.with_mode(mode),
            Message::ReplayGainPreamp(steps) => state.stepped_preamp(steps),
            Message::ReplayGainNoTagPreamp(steps) => state.stepped_no_tag_preamp(steps),
            Message::ReplayGainPreventClipping(prevent) => state.with_prevent_clipping(prevent),
            _ => return false,
        };
        // A redundant command emits nothing, so sending one is harmless; a
        // failed send means the engine is gone and the state machine must
        // stop claiming otherwise.
        if !self.playback.send(command_for(asked)) {
            self.player.engine_closed();
        }
        true
    }

    /// Send a transport command, marking it pending on acceptance and
    /// downgrading to engine-closed state when the channel is gone.
    pub(super) fn send_transport(&mut self, command: Command) {
        if self.playback.send(command) {
            self.player.note_transport_sent();
        } else {
            self.player.engine_closed();
        }
    }
}

/// The absolute, idempotent command that asks the engine for `settings`.
///
/// One place, because ADR-0013's command carries the *whole* setting: every
/// control in the settings panel resolves to a complete
/// [`ReplayGainSettings`] and then comes through here, so no control can send
/// a partial one.
pub(super) fn command_for(settings: ReplayGainSettings) -> Command {
    Command::SetReplayGain {
        mode: settings.mode,
        preamp_centidb: settings.preamp_centidb,
        no_tag_preamp_centidb: settings.no_tag_preamp_centidb,
        prevent_clipping: settings.prevent_clipping,
    }
}
