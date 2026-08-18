//! The Linux MPRIS2 server: two zbus interfaces on a dedicated thread.
//!
//! See [`crate::mpris`] for the design, the dependency choice, and the
//! graceful-degradation contract. This module is only the wiring: the
//! interface definitions, the connection, and the publish loop that turns a
//! [`Snapshot`] into `PropertiesChanged` signals.
//!
//! Everything with a decision in it — what `PlaybackStatus` says, how
//! milliseconds become microseconds, whether a `SetPosition` is stale — lives
//! in [`super::state`], which is pure and unit-tested on every platform. What
//! is left here needs a session bus to exercise, and is verified against a
//! private one with `busctl` rather than pretended about in a unit test.
#![expect(
    clippy::unused_self,
    reason = "a D-Bus property getter has to be a method on the served type, and the constant \
              ones (Identity, CanQuit, MinimumRate, …) genuinely read no state; #[interface] \
              offers no other shape for them"
)]
#![expect(
    clippy::needless_pass_by_value,
    reason = "method arguments arrive deserialized from the wire, so they are owned by \
              construction; a borrowed parameter is not a signature #[interface] can generate"
)]

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use iced::Subscription;
use iced::futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use iced::futures::stream::{self, StreamExt as _};
use zbus::blocking::Connection;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};
use zbus::{fdo, interface};

use super::state::{self, Snapshot};
use super::{DESKTOP_ENTRY, Request};

/// The object the two MPRIS interfaces are served at (fixed by the spec).
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";

/// Well-known bus name for the only instance of baz on a session.
const BUS_NAME: &str = "org.mpris.MediaPlayer2.baz";

/// `Identity`: the name a desktop shows for the player.
const IDENTITY: &str = "baz";

/// One publish: the new state, plus whether the engine has just confirmed a
/// seek (which the spec wants announced with `Seeked`, since a position that
/// jumps is exactly what a polling client cannot infer).
struct Update {
    snapshot: Snapshot,
    seeked: bool,
}

/// The GUI's connection to the MPRIS server thread.
pub(crate) struct Mpris {
    updates: Sender<Update>,
    /// Take-once slot the subscription stream drains, exactly as
    /// [`crate::playback`] does for engine events.
    requests: Arc<Mutex<Option<UnboundedReceiver<Request>>>>,
}

impl Mpris {
    /// Spawn the MPRIS thread. Returns immediately and never fails: if the
    /// thread cannot start, or cannot reach a bus once it has, the handle
    /// simply publishes into a channel nobody reads.
    pub(crate) fn start() -> Self {
        let (updates_tx, updates_rx) = channel();
        let (requests_tx, requests_rx) = unbounded();
        let spawned = thread::Builder::new()
            .name("baz-mpris".into())
            .spawn(move || serve(&updates_rx, requests_tx));
        if let Err(error) = spawned {
            crate::baz_log!("[mpris] could not start the D-Bus thread: {error}");
        }
        Self {
            updates: updates_tx,
            requests: Arc::new(Mutex::new(Some(requests_rx))),
        }
    }

    /// Hand the server thread the current state. A closed channel (no bus,
    /// or the thread gave up) is not an error worth reporting per event —
    /// the reason was printed once when it happened.
    pub(crate) fn publish(&self, snapshot: Snapshot, seeked: bool) {
        let _ = self.updates.send(Update { snapshot, seeked });
    }

    /// D-Bus method calls, as an iced subscription.
    pub(crate) fn subscription(&self) -> Subscription<Request> {
        #[derive(Clone)]
        struct StreamSlot(Arc<Mutex<Option<UnboundedReceiver<Request>>>>);

        impl std::hash::Hash for StreamSlot {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                "baz-mpris-requests".hash(state);
            }
        }

        fn requests(slot: &StreamSlot) -> iced::futures::stream::BoxStream<'static, Request> {
            let slot = Arc::clone(&slot.0);
            stream::once(async move {
                let taken = slot.lock().ok().and_then(|mut slot| slot.take());
                match taken {
                    Some(rx) => rx.boxed(),
                    None => stream::empty().boxed(),
                }
            })
            .flatten()
            .boxed()
        }

        Subscription::run_with(StreamSlot(Arc::clone(&self.requests)), requests)
    }
}

/// The `baz-mpris` thread: connect, serve, then publish until the app drops
/// its sender. Every exit path is a printed line and a return.
fn serve(updates: &Receiver<Update>, requests: UnboundedSender<Request>) {
    let root = Root {
        requests: requests.clone(),
    };
    let player = Player {
        snapshot: Snapshot::default(),
        requests,
    };
    let connection = match connect(root, player) {
        Ok(connection) => connection,
        Err(error) => {
            crate::baz_log!("[mpris] no session bus; desktop media controls unavailable ({error})");
            return;
        }
    };
    let Some(name) = claim_name(&connection) else {
        crate::baz_log!("[mpris] could not claim a bus name; desktop media controls unavailable");
        return;
    };
    crate::baz_log!("[mpris] serving {name} at {OBJECT_PATH}");

    let object_server = connection.object_server();
    let interface = match object_server.interface::<_, Player>(OBJECT_PATH) {
        Ok(interface) => interface,
        Err(error) => {
            crate::baz_log!("[mpris] player interface unavailable: {error}");
            return;
        }
    };
    let context = interface.signal_emitter().clone();

    // Ends when the app drops its sender — i.e. when baz exits, which drops
    // the connection and releases the name.
    while let Ok(update) = updates.recv() {
        let seeked = update.seeked;
        let mut player = interface.get_mut();
        let previous = std::mem::replace(&mut player.snapshot, update.snapshot);
        let current = &player.snapshot;
        // Position is deliberately absent: the spec says it is polled, and a
        // signal every 250 ms would be noise on the bus for a number every
        // client already interpolates.
        let changed = Changed {
            status: previous.status != current.status,
            metadata: previous.track != current.track,
            // Compared on the position and the mute flag rather than on the
            // reported f64: the same two facts the property is computed from,
            // and comparable exactly.
            volume: (previous.volume_position, previous.muted)
                != (current.volume_position, current.muted),
            can_go_next: previous.can_go_next != current.can_go_next,
            can_go_previous: previous.can_go_previous != current.can_go_previous,
            can_play: previous.can_play != current.can_play,
            can_pause: previous.can_pause != current.can_pause,
            can_seek: previous.can_seek != current.can_seek,
            can_control: previous.can_control != current.can_control,
        };
        let position_us = current.position_us;
        emit(&player, &context, &changed);
        drop(player);
        if seeked {
            let _ = zbus::block_on(Player::seeked(&context, position_us));
        }
    }
}

/// Which properties differ from the last published snapshot.
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag per signalled MPRIS property; see Snapshot in the state module"
)]
struct Changed {
    status: bool,
    metadata: bool,
    volume: bool,
    can_go_next: bool,
    can_go_previous: bool,
    can_play: bool,
    can_pause: bool,
    can_seek: bool,
    can_control: bool,
}

/// Emit `PropertiesChanged` for each property that moved. A failed emission
/// means the bus went away; the next `recv` will end the loop.
fn emit(player: &Player, context: &SignalEmitter<'static>, changed: &Changed) {
    if changed.status {
        let _ = zbus::block_on(player.playback_status_changed(context));
    }
    if changed.metadata {
        let _ = zbus::block_on(player.metadata_changed(context));
    }
    if changed.volume {
        let _ = zbus::block_on(player.volume_changed(context));
    }
    if changed.can_go_next {
        let _ = zbus::block_on(player.can_go_next_changed(context));
    }
    if changed.can_go_previous {
        let _ = zbus::block_on(player.can_go_previous_changed(context));
    }
    if changed.can_play {
        let _ = zbus::block_on(player.can_play_changed(context));
    }
    if changed.can_pause {
        let _ = zbus::block_on(player.can_pause_changed(context));
    }
    if changed.can_seek {
        let _ = zbus::block_on(player.can_seek_changed(context));
    }
    if changed.can_control {
        let _ = zbus::block_on(player.can_control_changed(context));
    }
}

/// Open the session bus and serve both interfaces at the MPRIS object path.
fn connect(root: Root, player: Player) -> zbus::Result<Connection> {
    zbus::blocking::connection::Builder::session()?
        .serve_at(OBJECT_PATH, root)?
        .serve_at(OBJECT_PATH, player)?
        .build()
}

/// Claim `org.mpris.MediaPlayer2.baz`, falling back to the spec's
/// per-instance name when another baz already holds it.
fn claim_name(connection: &Connection) -> Option<String> {
    if connection.request_name(BUS_NAME).is_ok() {
        return Some(BUS_NAME.to_owned());
    }
    let instance = format!("{}.instance{}", BUS_NAME, std::process::id());
    connection
        .request_name(instance.as_str())
        .ok()
        .map(|()| instance)
}

/// `org.mpris.MediaPlayer2` — what the player *is*.
struct Root {
    requests: UnboundedSender<Request>,
}

impl Root {
    fn ask(&self, request: Request) {
        let _ = self.requests.unbounded_send(request);
    }
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    /// Bring the window forward. Best effort — see [`crate::mpris`].
    fn raise(&self) {
        self.ask(Request::Raise);
    }

    /// Close baz.
    fn quit(&self) {
        self.ask(Request::Quit);
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_quit(&self) -> bool {
        true
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn can_raise(&self) -> bool {
        true
    }

    /// baz exposes no `org.mpris.MediaPlayer2.TrackList`.
    #[zbus(property(emits_changed_signal = "const"))]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn identity(&self) -> String {
        IDENTITY.to_owned()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn desktop_entry(&self) -> String {
        DESKTOP_ENTRY.to_owned()
    }

    /// Empty: `OpenUri` is not supported, so there is no scheme we would
    /// accept (see [`crate::mpris`]).
    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_uri_schemes(&self) -> Vec<String> {
        Vec::new()
    }

    /// Empty, for the same reason as [`Root::supported_uri_schemes`].
    #[zbus(property(emits_changed_signal = "const"))]
    fn supported_mime_types(&self) -> Vec<String> {
        Vec::new()
    }
}

/// `org.mpris.MediaPlayer2.Player` — what the player is *doing*.
///
/// The snapshot is owned by the object server and replaced wholesale by the
/// publish loop; every property getter is a read of it. Nothing here derives
/// state of its own.
struct Player {
    snapshot: Snapshot,
    requests: UnboundedSender<Request>,
}

impl Player {
    fn ask(&self, request: Request) {
        let _ = self.requests.unbounded_send(request);
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn next(&self) {
        self.ask(Request::Next);
    }

    fn previous(&self) {
        self.ask(Request::Previous);
    }

    fn pause(&self) {
        self.ask(Request::Pause);
    }

    fn play_pause(&self) {
        self.ask(Request::PlayPause);
    }

    fn stop(&self) {
        self.ask(Request::Stop);
    }

    fn play(&self) {
        self.ask(Request::Play);
    }

    /// Relative seek. The offset is microseconds and may be negative;
    /// clamping into the track happens where the position lives
    /// ([`crate::player::PlayerState::seek_by`]).
    fn seek(&self, offset: i64) {
        self.ask(Request::SeekBy(state::us_to_ms(offset)));
    }

    /// Absolute seek within a named track. Ignored when the name is stale or
    /// the position is negative — [`state::set_position_target`] holds both
    /// rules and the tests for them.
    fn set_position(&self, track_id: OwnedObjectPath, position: i64) {
        let track_id = track_id.into_inner();
        if let Some(position_ms) =
            state::set_position_target(self.snapshot.track.as_ref(), track_id.as_str(), position)
        {
            self.ask(Request::SeekTo(position_ms));
        }
    }

    /// Not supported, and `SupportedUriSchemes` is empty so that a client can
    /// know that without calling (see [`crate::mpris`]).
    fn open_uri(&self, uri: String) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported(format!(
            "baz plays its scanned library; it does not open {uri}"
        )))
    }

    /// Emitted when the engine confirms a seek — the one position change a
    /// polling client could not have predicted.
    #[zbus(signal)]
    async fn seeked(context: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.snapshot.status.as_str().to_owned()
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        metadata(&self.snapshot)
    }

    /// The last position the engine reported, in microseconds. Never
    /// extrapolated (see [`crate::mpris`]); `emits_changed_signal = "false"`
    /// is the spec's own annotation for this property.
    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        self.snapshot.position_us
    }

    /// baz plays at the source's rate and offers no rate control (ADR-0009),
    /// so the rate is `1.0` and the bounds pin it there.
    #[zbus(property(emits_changed_signal = "const"))]
    fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    /// **Whether the run is walked in a drawn order.**
    ///
    /// Present since 2026-08-18. It was absent for a stated reason — *"baz has
    /// neither loop nor shuffle yet"* — and that reason expired when both
    /// shipped, leaving a desktop's shuffle switch and `playerctl shuffle on`
    /// doing nothing to a player that had the feature.
    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.snapshot.shuffle
    }

    /// Set it. A **property**, so this states a value; the crossed-arrows
    /// control states the other one. Both land on `App::set_shuffle`, which is
    /// what stops a client writing `true` to something already true from
    /// turning it off.
    ///
    /// Nothing about the reported state moves here — the request goes to the
    /// engine and the property changes when the front end publishes again,
    /// which is the honesty rule the volume setter follows.
    #[zbus(property)]
    fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
        if !self.snapshot.can_control {
            return Err(fdo::Error::NotSupported("baz has no engine to shuffle".to_owned()).into());
        }
        self.ask(Request::SetShuffle(shuffle));
        Ok(())
    }

    /// **What happens when the run reaches its end.**
    ///
    /// The spec's three strings map onto baz's three states exactly, so
    /// nothing is approximated: `None` ends the run, `Track` repeats the
    /// completed track, `Playlist` re-walks the traversal — which for a
    /// shuffled run is the order it drew rather than a fresh draw, because
    /// that order *is* the run.
    #[zbus(property)]
    fn loop_status(&self) -> String {
        state::loop_status_of(self.snapshot.repeat).to_owned()
    }

    /// Set it, by the same mapping.
    ///
    /// An unknown string is **refused rather than rounded** to the nearest
    /// state: the spec enumerates exactly three, and quietly treating a fourth
    /// as `None` would turn a client's bug into a silent change of the
    /// listener's playback.
    #[zbus(property)]
    fn set_loop_status(&self, status: &str) -> zbus::Result<()> {
        if !self.snapshot.can_control {
            return Err(fdo::Error::NotSupported(
                "baz has no engine to set a loop mode on".to_owned(),
            )
            .into());
        }
        let Some(repeat) = state::repeat_of(status) else {
            return Err(fdo::Error::InvalidArgs(format!(
                "LoopStatus is None, Track or Playlist; not {status:?}"
            ))
            .into());
        };
        self.ask(Request::SetRepeat(repeat));
        Ok(())
    }

    /// The fader's level as a linear amplitude, `0.0..=1.0` — mapped through
    /// `baz-core`'s taper, and `0.0` while muted (see [`crate::mpris`]).
    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.snapshot.volume()
    }

    /// Set the level. Refused when `CanControl` is false, as the spec asks;
    /// otherwise mapped back through the same taper and, when it asks for
    /// sound while muted, accompanied by an unmute.
    ///
    /// Nothing about the reported state moves here. The requests go to the
    /// engine and the property changes when `Event::VolumeChanged` comes
    /// back — the honesty rule, unchanged by the direction of travel.
    #[zbus(property)]
    fn set_volume(&self, volume: f64) -> zbus::Result<()> {
        if !self.snapshot.can_control {
            // `#[interface]` property setters return `zbus::Result`, so the
            // fdo error is wrapped rather than returned directly; the wire
            // name a client sees is still `org.freedesktop.DBus.Error.NotSupported`.
            return Err(fdo::Error::NotSupported(
                "baz has no engine to set a volume on".to_owned(),
            )
            .into());
        }
        let position = state::position_for_amplitude(volume);
        if position > 0 && self.snapshot.muted {
            self.ask(Request::SetMute(false));
        }
        self.ask(Request::SetVolume(position));
        Ok(())
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.snapshot.can_go_next
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.snapshot.can_go_previous
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.snapshot.can_play
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.snapshot.can_pause
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.snapshot.can_seek
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        self.snapshot.can_control
    }
}

/// The `a{sv}` metadata map for a snapshot — empty when nothing is playing,
/// as the spec requires, and with every absent tag simply missing rather than
/// present-and-blank.
fn metadata(snapshot: &Snapshot) -> HashMap<String, OwnedValue> {
    let mut map = HashMap::new();
    let Some(track) = snapshot.track.as_ref() else {
        return map;
    };
    if let Ok(path) = ObjectPath::try_from(track.track_id.clone()) {
        insert(&mut map, "mpris:trackid", Value::ObjectPath(path));
    }
    if let Some(length_us) = track.length_us {
        insert(&mut map, "mpris:length", Value::I64(length_us));
    }
    if let Some(art_url) = track.art_url.as_ref() {
        insert(&mut map, "mpris:artUrl", Value::from(art_url.clone()));
    }
    insert(&mut map, "xesam:title", Value::from(track.title.clone()));
    if !track.artists.is_empty() {
        insert(&mut map, "xesam:artist", Value::from(track.artists.clone()));
    }
    if !track.album_artists.is_empty() {
        insert(
            &mut map,
            "xesam:albumArtist",
            Value::from(track.album_artists.clone()),
        );
    }
    if let Some(album) = track.album.as_ref() {
        insert(&mut map, "xesam:album", Value::from(album.clone()));
    }
    if let Some(number) = track.track_number {
        insert(&mut map, "xesam:trackNumber", Value::I32(number));
    }
    map
}

/// Insert a metadata value, skipping anything that cannot be owned (nothing
/// we construct can fail this — only file descriptors can — but the map is
/// better one key short than the build one `unwrap` heavier).
fn insert(map: &mut HashMap<String, OwnedValue>, key: &str, value: Value<'_>) {
    if let Ok(owned) = OwnedValue::try_from(value) {
        map.insert(key.to_owned(), owned);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The metadata map is the one piece of D-Bus shaping that does not need
    /// a bus: it is a pure function of the snapshot.
    fn snapshot_with_track() -> Snapshot {
        Snapshot {
            track: Some(state::TrackInfo {
                track_id: "/org/mpris/MediaPlayer2/baz/track/1".to_owned(),
                title: "Desire".to_owned(),
                artists: vec!["Talk Talk".to_owned()],
                album_artists: vec!["Talk Talk".to_owned()],
                album: Some("Spirit of Eden".to_owned()),
                track_number: Some(3),
                length_us: Some(415_000_000),
                art_url: Some("file:///music/eden/cover.jpg".to_owned()),
            }),
            ..Snapshot::default()
        }
    }

    /// The D-Bus signature of a metadata key's value, as a string.
    fn signature(map: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
        map.get(key)
            .map(|value| value.value_signature().to_string())
    }

    /// A metadata key's value, read back as the type it should carry.
    fn text<'a>(map: &'a HashMap<String, OwnedValue>, key: &str) -> Option<&'a str> {
        <&str>::try_from(&**map.get(key)?).ok()
    }

    fn number(map: &HashMap<String, OwnedValue>, key: &str) -> Option<i64> {
        i64::try_from(&**map.get(key)?).ok()
    }

    #[test]
    fn metadata_carries_the_spec_keys_with_the_spec_types() {
        let map = metadata(&snapshot_with_track());
        assert_eq!(
            signature(&map, "mpris:trackid").as_deref(),
            Some("o"),
            "the track id is an object path, not a string"
        );
        assert_eq!(
            signature(&map, "mpris:length").as_deref(),
            Some("x"),
            "length is a 64-bit microsecond count"
        );
        assert_eq!(
            signature(&map, "xesam:artist").as_deref(),
            Some("as"),
            "artist is a list, even with one artist"
        );
        assert_eq!(signature(&map, "xesam:trackNumber").as_deref(), Some("i"));
        assert_eq!(signature(&map, "xesam:title").as_deref(), Some("s"));
        assert_eq!(number(&map, "mpris:length"), Some(415_000_000));
        assert_eq!(
            map.get("xesam:trackNumber")
                .and_then(|value| i32::try_from(&**value).ok()),
            Some(3)
        );
        assert_eq!(text(&map, "xesam:title"), Some("Desire"));
        assert_eq!(text(&map, "xesam:album"), Some("Spirit of Eden"));
        assert_eq!(
            text(&map, "mpris:artUrl"),
            Some("file:///music/eden/cover.jpg")
        );
        assert_eq!(
            map.get("mpris:trackid")
                .and_then(|value| ObjectPath::try_from(&**value).ok())
                .as_deref(),
            Some("/org/mpris/MediaPlayer2/baz/track/1")
        );
    }

    #[test]
    fn absent_tags_are_absent_keys_not_empty_ones() {
        let mut snapshot = snapshot_with_track();
        if let Some(track) = snapshot.track.as_mut() {
            track.album = None;
            track.track_number = None;
            track.length_us = None;
            track.art_url = None;
            track.artists.clear();
            track.album_artists.clear();
        }
        let map = metadata(&snapshot);
        for absent in [
            "xesam:album",
            "xesam:trackNumber",
            "mpris:length",
            "mpris:artUrl",
            "xesam:artist",
            "xesam:albumArtist",
        ] {
            assert!(!map.contains_key(absent), "{absent} should be omitted");
        }
        // The two that are always known stay.
        assert!(map.contains_key("mpris:trackid"));
        assert!(map.contains_key("xesam:title"));
    }

    #[test]
    fn nothing_playing_is_an_empty_metadata_map() {
        assert!(metadata(&Snapshot::default()).is_empty());
    }
}
