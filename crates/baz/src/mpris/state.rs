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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the five flags are MPRIS's five Can* properties, one for one; folding them into a \
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
    /// `CanGoNext`.
    pub(crate) can_go_next: bool,
    /// `CanPlay`.
    pub(crate) can_play: bool,
    /// `CanPause`.
    pub(crate) can_pause: bool,
    /// `CanSeek`.
    pub(crate) can_seek: bool,
    /// `CanControl`.
    pub(crate) can_control: bool,
}

impl Snapshot {
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
            can_go_next: player.next_enabled(),
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

    use baz_core::protocol::Event;

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
            first_track: PathBuf::from("/music/eden/01.flac"),
            editions: vec![EditionVm {
                key: EditionKey(None),
                detail: None,
                tracks: vec![TrackVm {
                    number: Some(3),
                    title: "Desire".to_owned(),
                    artist: None,
                    duration: Some(Duration::from_secs(415)),
                    path: PathBuf::from("/music/eden/01.flac"),
                }],
            }],
        }]
    }

    /// A player state with a track playing at `elapsed_ms` of `track_ms`.
    fn playing(elapsed_ms: u64, track_ms: Option<u64>) -> PlayerState {
        let albums = albums();
        let mut player = PlayerState::new(Availability::Ready);
        player.note_queue_sent(1);
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
        assert!(!snapshot.can_seek);

        // Playing a track of known length: everything is possible.
        let mut player = playing(0, Some(415_000));
        let snapshot = Snapshot::from_player(&player, None);
        assert!(snapshot.can_control);
        assert!(snapshot.can_play);
        assert!(snapshot.can_pause);
        assert!(snapshot.can_go_next);
        assert!(snapshot.can_seek);

        // Stopped with a queue still loaded: Play can restart it, but there
        // is nothing to pause, skip past, or seek within.
        player.apply(&Event::QueueEnded, &albums);
        let snapshot = Snapshot::from_player(&player, None);
        assert!(snapshot.can_play);
        assert!(!snapshot.can_pause);
        assert!(!snapshot.can_go_next);
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
