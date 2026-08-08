//! The MPRIS reading of playback state: pure, D-Bus-free, and testable
//! everywhere.
//!
//! Nothing in this module knows about zbus, sockets, or Linux. It is the
//! translation from [`crate::player`]'s event-derived state into the exact
//! values the MPRIS2 properties carry, plus the two pure decisions the
//! interface's methods have to make (which position a `Seek` offset lands on,
//! and whether a `SetPosition` call is stale). Keeping that split means the
//! part with the arithmetic — where microseconds and milliseconds meet, and
//! where a factor of a thousand would otherwise go unnoticed — is unit-tested
//! on every platform in the CI matrix, including the ones that never speak
//! MPRIS at all.
//!
//! # Units
//!
//! `baz-core`'s protocol is integer **milliseconds** end to end (see
//! `baz_core::protocol`); MPRIS is integer **microseconds**, signed. Every
//! crossing goes through [`ms_to_us`] / [`us_to_ms`] and nowhere else, and
//! both are tested against the boundary cases (zero, saturation, negative
//! offsets, sub-millisecond truncation).
//!
//! The volume crosses the same kind of boundary and is handled the same way.
//! MPRIS's `Volume` is a linear **amplitude** as an `f64`, where 1.0 is
//! normal; baz's protocol unit is an integer **control position** on a cubic
//! taper. [`volume_amplitude`] and [`position_for_amplitude`] are the two
//! directions, and both go through
//! [`Volume::amplitude`](baz_core::volume::Volume::amplitude) or its exact
//! inverse — the cube root — rather than inventing a second curve. That
//! matters more than it looks: a front end with its own idea of the taper
//! would make "half volume" mean one thing on the lock screen and another on
//! the fader six pixels from it, which is precisely what ADR-0011 put the
//! taper in `baz-core` to prevent. `volume_round_trips_through_the_core_taper`
//! pins every one of the 1001 positions.
//!
//! # The honesty rule, restated for D-Bus
//!
//! [`Snapshot::from_player`] reads only what the engine confirmed.
//! `PlaybackStatus` is [`Phase`], which moves solely in
//! [`PlayerState::apply`](crate::player::PlayerState::apply). `Position` is
//! the last [`Progress`](baz_core::protocol::Event::Progress) reading and is
//! **never** extrapolated between reports, so a desktop widget that polls it
//! sees baz's real knowledge (accurate to the engine's ~4 Hz cadence) rather
//! than a clock we ran ourselves. The `Can*` flags answer from the same state
//! the on-screen buttons answer from, so the media control and the bottom bar
//! can never disagree about what is possible.

use std::path::Path;

use baz_core::volume::{MAX_POSITION, Volume};

use crate::player::{Phase, PlayerState};

/// Prefix for our per-track object paths; the track sequence number
/// ([`PlayerState::track_seq`]) is appended.
const TRACK_ID_PREFIX: &str = "/org/mpris/MediaPlayer2/baz/track/";

/// MPRIS `PlaybackStatus`, which is a string on the wire and exactly three
/// values in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PlaybackStatus {
    /// A track is playing.
    Playing,
    /// A track is loaded and paused.
    Paused,
    /// No track is playing.
    #[default]
    Stopped,
}

impl PlaybackStatus {
    /// The spec's spelling. These three strings are the wire format; a client
    /// that receives anything else ignores us.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Playing => "Playing",
            Self::Paused => "Paused",
            Self::Stopped => "Stopped",
        }
    }

    /// The confirmed [`Phase`], one for one. There is no fourth MPRIS state
    /// and no baz state that maps to more than one of them.
    pub(crate) fn from_phase(phase: Phase) -> Self {
        match phase {
            Phase::Playing => Self::Playing,
            Phase::Paused => Self::Paused,
            Phase::Stopped => Self::Stopped,
        }
    }
}

/// Everything MPRIS `Metadata` carries about the current track.
///
/// Fields are `Option` where the tags may not have said, and the interface
/// omits absent keys rather than writing an empty string — an MPRIS client
/// that sees no `xesam:album` knows the album is unknown, where one that sees
/// `""` has been told the album is called nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackInfo {
    /// `mpris:trackid` — an object path unique to this track within the
    /// session, stable across a seek and changed by a track change.
    pub(crate) track_id: String,
    /// `xesam:title`.
    pub(crate) title: String,
    /// `xesam:artist` — the track's own artist when it has one, else the
    /// album artist, else empty (the key is then omitted).
    pub(crate) artists: Vec<String>,
    /// `xesam:albumArtist`.
    pub(crate) album_artists: Vec<String>,
    /// `xesam:album`.
    pub(crate) album: Option<String>,
    /// `xesam:trackNumber`.
    pub(crate) track_number: Option<i32>,
    /// `mpris:length`, in microseconds — the engine's confirmed track length,
    /// absent for a container that never declared one.
    pub(crate) length_us: Option<i64>,
    /// `mpris:artUrl` — a `file://` URL for a cover image that genuinely
    /// exists on disk, or nothing. See [`crate::mpris`] on why embedded art
    /// is not turned into a URL.
    pub(crate) art_url: Option<String>,
}

/// The whole MPRIS-visible state at one instant.
///
/// Volume is stored as the **control position and the mute flag**, not as the
/// `f64` the property carries, for two reasons: it keeps this type `Eq` — the
/// publish loop decides what to signal by comparing snapshots, and an `f64`
/// has no honest total equality — and it keeps the taper's application in one
/// place ([`Snapshot::volume`]) rather than at every construction site.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the six flags are MPRIS's six Can* properties, one for one; folding them into a \
              state machine would put a translation between the spec and the wire, which is \
              exactly what this struct exists to avoid"
)]
pub(crate) struct Snapshot {
    /// `PlaybackStatus`.
    pub(crate) status: PlaybackStatus,
    /// The current track, or `None` when nothing is playing — in which case
    /// `Metadata` is the empty map, as the spec requires.
    pub(crate) track: Option<TrackInfo>,
    /// `Position`, in microseconds.
    pub(crate) position_us: i64,
    /// The fader's control position, `0..=`[`MAX_POSITION`].
    pub(crate) volume_position: u16,
    /// Whether output is muted — separate engine state, folded into the
    /// reported `Volume` by [`Snapshot::volume`].
    pub(crate) muted: bool,
    /// `CanGoNext`.
    pub(crate) can_go_next: bool,
    /// `CanGoPrevious`.
    ///
    /// Reported rather than pinned to `false` since the transport gained a
    /// Previous button: the engine's `Command::Previous` was always there, and
    /// what was missing was anything to send it. It tracks
    /// [`PlayerState::previous_enabled`] — a running queue can always go back,
    /// because at the head of the queue, and past three seconds into any
    /// track, `Previous` restarts what is playing rather than doing nothing.
    pub(crate) can_go_previous: bool,
    /// `CanPlay`.
    pub(crate) can_play: bool,
    /// `CanPause`.
    pub(crate) can_pause: bool,
    /// `CanSeek`.
    pub(crate) can_seek: bool,
    /// `CanControl`.
    pub(crate) can_control: bool,
}

impl Default for Snapshot {
    /// Everything absent or refused, except the volume — which defaults to
    /// unity because that is what a freshly spawned engine is at (ADR-0011),
    /// and because the server serves this value until the first publish
    /// arrives. Defaulting it to zero would advertise a silent player.
    fn default() -> Self {
        Self {
            status: PlaybackStatus::default(),
            track: None,
            position_us: 0,
            volume_position: MAX_POSITION,
            muted: false,
            can_go_next: false,
            can_go_previous: false,
            can_play: false,
            can_pause: false,
            can_seek: false,
            can_control: false,
        }
    }
}

impl Snapshot {
    /// MPRIS `Volume`: a linear amplitude in `0.0..=1.0`.
    ///
    /// The **effective** level, so a muted player reports `0.0` — the fold
    /// the engine itself performs, and the only reading that answers the
    /// question a client is actually asking. Reporting the fader's position
    /// while the output is silent would put a media widget at full volume
    /// beside a player making no sound.
    pub(crate) fn volume(&self) -> f64 {
        if self.muted {
            0.0
        } else {
            volume_amplitude(self.volume_position)
        }
    }

    /// Read the MPRIS state off the player state machine.
    ///
    /// `art_url` is resolved by the caller (it needs the filesystem, which
    /// this module deliberately does not touch) and is simply carried.
    pub(crate) fn from_player(player: &PlayerState, art_url: Option<String>) -> Self {
        let track = player.now_playing().map(|now| {
            let album_artists = now.artist.clone().into_iter().collect::<Vec<_>>();
            let artists = now
                .track_artist
                .clone()
                .map_or_else(|| album_artists.clone(), |artist| vec![artist]);
            TrackInfo {
                track_id: track_id(player.track_seq()),
                title: now.title.clone(),
                artists,
                album_artists,
                album: now.album.clone(),
                track_number: now.track_number.and_then(|n| i32::try_from(n).ok()),
                length_us: player.track_ms().map(ms_to_us),
                art_url,
            }
        });
        Self {
            status: PlaybackStatus::from_phase(player.phase()),
            track,
            position_us: ms_to_us(player.elapsed_ms()),
            volume_position: player.volume().position(),
            muted: player.muted(),
            can_go_next: player.next_enabled(),
            can_go_previous: player.previous_enabled(),
            can_play: player.play_pause_enabled(),
            // Pause is a documented engine no-op while stopped, so offering
            // it then would be an offer we cannot honour.
            can_pause: player.engine_ready() && player.phase() != Phase::Stopped,
            can_seek: player.can_seek(),
            can_control: player.engine_ready(),
        }
    }
}

/// The object path naming the `seq`-th track of this session.
fn track_id(seq: u64) -> String {
    format!("{TRACK_ID_PREFIX}{seq}")
}

/// Milliseconds (baz's protocol unit) to microseconds (MPRIS's), saturating
/// rather than wrapping. `u64` milliseconds outrun `i64` microseconds only
/// past ~292 000 years, so the saturation is a formality — but it is a
/// formality that keeps this crate free of arithmetic that could panic.
pub(crate) fn ms_to_us(ms: u64) -> i64 {
    i64::try_from(ms).unwrap_or(i64::MAX).saturating_mul(1_000)
}

/// Microseconds (MPRIS) to milliseconds (baz's protocol), truncating toward
/// zero. Sub-millisecond precision is discarded deliberately: the protocol
/// has no way to express it and the engine's seek is drain-and-restart, so
/// pretending to microsecond accuracy would be the wrong kind of precise.
pub(crate) fn us_to_ms(us: i64) -> i64 {
    us / 1_000
}

/// A control position as the linear amplitude MPRIS `Volume` carries.
///
/// Straight through [`Volume::amplitude`] — the taper is `baz-core`'s and
/// this is a widening cast, not a second opinion.
pub(crate) fn volume_amplitude(position: u16) -> f64 {
    f64::from(Volume::new(position).amplitude())
}

/// The control position an MPRIS `Volume` write is asking for: the exact
/// inverse of the cubic taper, rounded to the nearest position.
///
/// Values outside `0.0..=1.0` are clamped rather than refused — the spec
/// names negatives specifically ("the volume should be set to 0.0") and baz
/// offers no gain above unity (ADR-0011), so the honest answer to "louder
/// than the loudest" is the loudest. A non-finite value is not a level at
/// all and reads as silence.
pub(crate) fn position_for_amplitude(amplitude: f64) -> u16 {
    // NaN first, because every comparison with it is false and it would
    // otherwise fall through to the arithmetic. An infinity is a level, of a
    // sort, and clamps like any other out-of-range one.
    if amplitude.is_nan() || amplitude <= 0.0 {
        return 0;
    }
    if amplitude >= 1.0 {
        return MAX_POSITION;
    }
    // The taper is `(position / MAX)³`, so its inverse is a cube root. f64
    // carries the f32 amplitude exactly and the cube root divides its
    // relative error by three, so the rounding below lands on the position
    // the amplitude came from — asserted for all 1001 of them.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "cbrt of a value in (0, 1) is in (0, 1), so the product is inside 0..MAX_POSITION"
    )]
    let position = (amplitude.cbrt() * f64::from(MAX_POSITION)).round() as u16;
    position.min(MAX_POSITION)
}

/// Where an MPRIS `SetPosition(track_id, position_us)` call should land, or
/// `None` when the call must be ignored.
///
/// The spec's two rules, both of which exist to stop a race from moving the
/// wrong track: a call naming a track that is no longer current is stale and
/// does nothing, and a negative position does nothing. `current` is the
/// track the player is on, `None` when it is on none.
pub(crate) fn set_position_target(
    current: Option<&TrackInfo>,
    track_id: &str,
    position_us: i64,
) -> Option<u64> {
    if position_us < 0 {
        return None;
    }
    let current = current?;
    if current.track_id != track_id {
        return None;
    }
    u64::try_from(us_to_ms(position_us)).ok()
}

/// A `file://` URL for `path`, percent-encoding everything outside RFC 3986's
/// unreserved set (path separators excepted).
///
/// `None` for a path that is not valid UTF-8: there is no way to spell such a
/// path in a URL without inventing bytes, and inventing bytes to name a cover
/// image is exactly the sort of small lie this project does not tell. Those
/// albums simply carry no `mpris:artUrl`.
pub(crate) fn file_url(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    let mut url = String::with_capacity(text.len() + "file://".len());
    url.push_str("file://");
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                url.push(char::from(byte));
            }
            other => {
                const HEX: [u8; 16] = *b"0123456789ABCDEF";
                url.push('%');
                url.push(char::from(HEX[usize::from(other >> 4)]));
                url.push(char::from(HEX[usize::from(other & 0x0f)]));
            }
        }
    }
    Some(url)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use baz_core::protocol::{Event, VolumePath};

    use crate::player::Availability;
    use crate::vm::{AlbumArtistVm, AlbumVm, EditionKey, EditionVm, TrackVm};

    use super::*;

    /// One album with one track, enough to resolve a now-playing readout.
    fn albums() -> Vec<AlbumVm> {
        vec![AlbumVm {
            id: 7,
            title: Some("Spirit of Eden".to_owned()),
            artist: AlbumArtistVm::Named("Talk Talk".to_owned()),
            track_artists_vary: false,
            year: Some(1988),
            genre: None,
            first_seen_ns: None,
            first_track: PathBuf::from("/music/eden/01.flac"),
            editions: vec![EditionVm {
                key: EditionKey(None),
                detail: None,
                bitrate: None,
                bit_depth: None,
                sample_rate: None,
                replay_gain: crate::vm::ReplayGainCoverage::default(),
                tracks: vec![TrackVm {
                    disc: None,
                    number: Some(3),
                    title: "Desire".to_owned(),
                    artist: None,
                    duration: Some(Duration::from_secs(415)),
                    path: PathBuf::from("/music/eden/01.flac"),
                    bytes: None,
                }],
            }],
        }]
    }

    /// A player state with a track playing at `elapsed_ms` of `track_ms`.
    fn playing(elapsed_ms: u64, track_ms: Option<u64>) -> PlayerState {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(crate::vm::album_queue(&albums[0], None));
        player.apply(
            &Event::TrackStarted {
                path: PathBuf::from("/music/eden/01.flac"),
                position: 0,
            },
            &albums,
        );
        player.apply(
            &Event::Progress {
                elapsed_ms,
                track_ms,
            },
            &albums,
        );
        player
    }

    /// The `mpris:trackid` the player is currently advertising.
    fn current_track_id(player: &PlayerState) -> String {
        Snapshot::from_player(player, None)
            .track
            .expect("a track is playing")
            .track_id
    }

    #[test]
    fn playback_status_strings_are_the_specs() {
        assert_eq!(PlaybackStatus::Playing.as_str(), "Playing");
        assert_eq!(PlaybackStatus::Paused.as_str(), "Paused");
        assert_eq!(PlaybackStatus::Stopped.as_str(), "Stopped");
    }

    #[test]
    fn playback_status_follows_the_confirmed_phase() {
        let albums = albums();
        let mut player = playing(0, Some(1_000));
        assert_eq!(
            Snapshot::from_player(&player, None).status.as_str(),
            "Playing"
        );
        player.apply(&Event::Paused, &albums);
        assert_eq!(
            Snapshot::from_player(&player, None).status.as_str(),
            "Paused"
        );
        player.apply(&Event::Resumed, &albums);
        assert_eq!(
            Snapshot::from_player(&player, None).status.as_str(),
            "Playing"
        );
        player.apply(&Event::QueueEnded, &albums);
        assert_eq!(
            Snapshot::from_player(&player, None).status.as_str(),
            "Stopped"
        );
    }

    /// A transport command in flight must not move `PlaybackStatus`: the
    /// honesty rule reaches D-Bus unchanged.
    #[test]
    fn a_pending_command_does_not_change_the_reported_status() {
        let mut player = playing(0, Some(1_000));
        let before = Snapshot::from_player(&player, None);
        player.note_transport_sent();
        assert_eq!(Snapshot::from_player(&player, None), before);
    }

    #[test]
    fn position_and_length_are_microseconds() {
        let player = playing(93_500, Some(214_000));
        let snapshot = Snapshot::from_player(&player, None);
        assert_eq!(snapshot.position_us, 93_500_000);
        assert_eq!(
            snapshot.track.expect("a track is playing").length_us,
            Some(214_000_000)
        );
    }

    #[test]
    fn an_undeclared_track_length_carries_no_mpris_length() {
        let player = playing(1_000, None);
        let snapshot = Snapshot::from_player(&player, None);
        assert_eq!(snapshot.track.expect("a track").length_us, None);
        assert!(!snapshot.can_seek, "a track with no length is not seekable");
    }

    #[test]
    fn metadata_carries_title_artist_album_and_track_number() {
        let player = playing(0, Some(415_000));
        let track = Snapshot::from_player(&player, None)
            .track
            .expect("a track is playing");
        assert_eq!(track.title, "Desire");
        assert_eq!(track.artists, vec!["Talk Talk".to_owned()]);
        assert_eq!(track.album_artists, vec!["Talk Talk".to_owned()]);
        assert_eq!(track.album.as_deref(), Some("Spirit of Eden"));
        assert_eq!(track.track_number, Some(3));
    }

    #[test]
    fn nothing_playing_means_no_metadata_and_the_placeholder_track_id() {
        let player = PlayerState::new(Availability::Ready);
        let snapshot = Snapshot::from_player(&player, None);
        assert!(
            snapshot.track.is_none(),
            "with no track, Metadata is the empty map the spec asks for"
        );
        assert_eq!(snapshot.position_us, 0);
        assert_eq!(snapshot.status, PlaybackStatus::Stopped);
    }

    #[test]
    fn the_track_id_survives_a_seek_and_changes_with_the_track() {
        let albums = albums();
        let mut player = playing(0, Some(415_000));
        let first = current_track_id(&player);
        assert!(first.starts_with("/org/mpris/MediaPlayer2/baz/track/"));

        // A seek restarts the same file: same track, same id.
        player.apply(
            &Event::TrackStarted {
                path: PathBuf::from("/music/eden/01.flac"),
                position: 0,
            },
            &albums,
        );
        assert_eq!(current_track_id(&player), first);

        // A genuinely different file is a different track.
        player.apply(
            &Event::TrackStarted {
                path: PathBuf::from("/music/eden/02.flac"),
                position: 1,
            },
            &albums,
        );
        assert_ne!(current_track_id(&player), first);
    }

    #[test]
    fn can_flags_answer_from_the_same_state_as_the_buttons() {
        let albums = albums();
        // No engine at all: nothing is possible.
        let dead = PlayerState::new(Availability::NoDevice("no device".to_owned()));
        let snapshot = Snapshot::from_player(&dead, None);
        assert!(!snapshot.can_control);
        assert!(!snapshot.can_play);
        assert!(!snapshot.can_pause);
        assert!(!snapshot.can_go_next);
        assert!(!snapshot.can_go_previous);
        assert!(!snapshot.can_seek);

        // Playing a track of known length: everything is possible.
        let mut player = playing(0, Some(415_000));
        let snapshot = Snapshot::from_player(&player, None);
        assert!(snapshot.can_control);
        assert!(snapshot.can_play);
        assert!(snapshot.can_pause);
        assert!(snapshot.can_go_next);
        assert!(snapshot.can_go_previous);
        assert!(snapshot.can_seek);

        // Stopped with a queue still loaded: Play can restart it, but there
        // is nothing to pause, skip past, or seek within.
        player.apply(&Event::QueueEnded, &albums);
        let snapshot = Snapshot::from_player(&player, None);
        assert!(snapshot.can_play);
        assert!(!snapshot.can_pause);
        assert!(!snapshot.can_go_next);
        assert!(
            !snapshot.can_go_previous,
            "a relative command has nothing to be relative to while stopped"
        );
        assert!(!snapshot.can_seek);
    }

    #[test]
    fn a_track_artist_overrides_the_album_artist_for_xesam_artist() {
        let mut albums = albums();
        albums[0].editions[0].tracks[0].artist = Some("Mark Hollis".to_owned());
        let mut player = PlayerState::new(Availability::Ready);
        player.apply(
            &Event::TrackStarted {
                path: PathBuf::from("/music/eden/01.flac"),
                position: 0,
            },
            &albums,
        );
        let track = Snapshot::from_player(&player, None).track.expect("a track");
        assert_eq!(track.artists, vec!["Mark Hollis".to_owned()]);
        assert_eq!(track.album_artists, vec!["Talk Talk".to_owned()]);
    }

    #[test]
    fn millisecond_microsecond_conversions_round_trip_and_saturate() {
        assert_eq!(ms_to_us(0), 0);
        assert_eq!(ms_to_us(1), 1_000);
        assert_eq!(ms_to_us(93_500), 93_500_000);
        assert_eq!(ms_to_us(u64::MAX), i64::MAX);
        assert_eq!(us_to_ms(0), 0);
        assert_eq!(us_to_ms(1_000), 1);
        // Sub-millisecond precision truncates toward zero, both ways.
        assert_eq!(us_to_ms(1_999), 1);
        assert_eq!(us_to_ms(-1_999), -1);
        assert_eq!(us_to_ms(-5_000_000), -5_000);
        for ms in [0_u64, 1, 250, 93_500, 3_600_000] {
            assert_eq!(us_to_ms(ms_to_us(ms)), i64::try_from(ms).expect("fits"));
        }
    }

    #[test]
    fn set_position_ignores_a_stale_track_id() {
        let player = playing(0, Some(415_000));
        let snapshot = Snapshot::from_player(&player, None);
        let current = snapshot.track.as_ref();
        let id = current_track_id(&player);

        assert_eq!(set_position_target(current, &id, 30_000_000), Some(30_000));
        assert_eq!(
            set_position_target(current, "/org/mpris/MediaPlayer2/baz/track/999", 30_000_000),
            None,
            "a call naming another track is stale"
        );
        assert_eq!(
            set_position_target(current, &id, -1),
            None,
            "a negative position does nothing"
        );
        assert_eq!(
            set_position_target(None, &id, 30_000_000),
            None,
            "there is nothing to position when nothing is playing"
        );
    }

    // -----------------------------------------------------------------
    // Volume: MPRIS's linear amplitude, baz's control position
    // -----------------------------------------------------------------

    /// The claim the module docs make, checked at every one of the taper's
    /// 1001 positions: the mapping to MPRIS and back is `baz-core`'s own
    /// curve and its exact inverse, so a client that reads the property and
    /// writes it back unchanged moves nothing.
    #[test]
    fn volume_round_trips_through_the_core_taper() {
        for position in 0..=MAX_POSITION {
            let amplitude = volume_amplitude(position);
            assert!(
                (0.0..=1.0).contains(&amplitude),
                "position {position} is not an amplitude: {amplitude}"
            );
            assert_eq!(
                position_for_amplitude(amplitude),
                position,
                "position {position} did not survive the round trip"
            );
        }
    }

    #[test]
    fn the_taper_endpoints_are_the_specs_endpoints() {
        // Unity is 1.0 exactly — the value MPRIS calls "normal volume", and
        // the position at which baz touches nothing.
        assert!((volume_amplitude(MAX_POSITION) - 1.0).abs() < f64::EPSILON);
        assert!(volume_amplitude(0).abs() < f64::EPSILON);
        assert_eq!(position_for_amplitude(1.0), MAX_POSITION);
        assert_eq!(position_for_amplitude(0.0), 0);
        // Half amplitude is a real position on the fader, not half travel:
        // the cube root of 0.5 is ~0.7937 of the way up.
        assert_eq!(position_for_amplitude(0.5), 794);
        // Half *travel* is a eighth of the amplitude — the 60 dB fader law,
        // read from the same curve the engine uses.
        assert!((volume_amplitude(500) - 0.125).abs() < 1e-9);
    }

    /// The spec names negatives specifically, and baz offers no gain above
    /// unity, so both ends clamp rather than erroring.
    #[test]
    fn out_of_range_volumes_clamp_rather_than_erroring() {
        assert_eq!(position_for_amplitude(-1.0), 0);
        assert_eq!(position_for_amplitude(-0.0), 0);
        assert_eq!(position_for_amplitude(4.2), MAX_POSITION);
        assert_eq!(position_for_amplitude(f64::INFINITY), MAX_POSITION);
        assert_eq!(position_for_amplitude(f64::NEG_INFINITY), 0);
        assert_eq!(position_for_amplitude(f64::NAN), 0, "not a level at all");
    }

    #[test]
    fn the_reported_volume_is_the_effective_level() {
        let albums = albums();
        let mut player = playing(0, Some(1_000));
        assert!((Snapshot::from_player(&player, None).volume() - 1.0).abs() < f64::EPSILON);

        player.apply(
            &Event::VolumeChanged {
                position: 500,
                muted: false,
                path: VolumePath::SoftwareGain,
            },
            &albums,
        );
        let snapshot = Snapshot::from_player(&player, None);
        assert_eq!(snapshot.volume_position, 500);
        assert!(!snapshot.muted);
        assert!((snapshot.volume() - 0.125).abs() < 1e-9);

        // Muted reports silence — what is coming out — while the fader's own
        // position is kept, so unmuting restores it.
        player.apply(
            &Event::VolumeChanged {
                position: 500,
                muted: true,
                path: VolumePath::SoftwareGain,
            },
            &albums,
        );
        let snapshot = Snapshot::from_player(&player, None);
        assert!(snapshot.volume().abs() < f64::EPSILON);
        assert_eq!(
            snapshot.volume_position, 500,
            "the position mute will restore is not destroyed by reporting 0.0"
        );
    }

    /// The server serves the default snapshot until the first publish, so
    /// its volume has to be the one a freshly spawned engine is at.
    #[test]
    fn the_default_snapshot_advertises_unity_not_silence() {
        let snapshot = Snapshot::default();
        assert_eq!(snapshot.volume_position, MAX_POSITION);
        assert!(!snapshot.muted);
        assert!((snapshot.volume() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn file_urls_percent_encode_what_a_url_cannot_carry() {
        assert_eq!(
            file_url(Path::new("/music/eden/cover.jpg")).as_deref(),
            Some("file:///music/eden/cover.jpg")
        );
        assert_eq!(
            file_url(Path::new("/music/Talk Talk/cover.jpg")).as_deref(),
            Some("file:///music/Talk%20Talk/cover.jpg")
        );
        assert_eq!(
            file_url(Path::new("/music/a#b?c/cover.jpg")).as_deref(),
            Some("file:///music/a%23b%3Fc/cover.jpg")
        );
        assert_eq!(
            file_url(Path::new("/music/Sigur Rós/cover.jpg")).as_deref(),
            Some("file:///music/Sigur%20R%C3%B3s/cover.jpg"),
            "non-ASCII is UTF-8 percent-encoded byte by byte"
        );
        assert_eq!(
            file_url(Path::new("/a-b_c.d~e/cover.png")).as_deref(),
            Some("file:///a-b_c.d~e/cover.png"),
            "the unreserved set is left alone"
        );
    }
}
