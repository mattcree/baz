//! **The context menu: a pointer mirror, governed exactly as the keyboard
//! is** (`docs/design/09-implicit-playlists.md` §5.2).
//!
//! baz had no menus of any kind before this module, and the visible-control
//! rule (`docs/REFUSALS.md`) would forbid one that *added* actions: a
//! right-click is a gesture, and no action's only route may be a gesture. So
//! the menu gets the keyboard's own governing rule before it gets a single
//! item — L8.7's clause, *the keyboard is the same decision, made twice*,
//! extended: made three times.
//!
//! > **Every menu item sends only messages some visible on-screen control
//! > also sends, and no action's only route is a menu.**
//!
//! That is what reconciles menus with L8.6 (*no two controls send the same
//! message*): a menu item is not a second control any more than a key binding
//! is — it is an accelerator layer over the controls that exist, and the
//! binding test, not the control table, is what pins it
//! (`tests::every_menu_item_is_a_press_some_control_also_makes`, the same
//! shape as `app.rs`'s keyboard sweep). An item whose gesture is two presses
//! in the interface — the track row's `+` and then the picker's Queue row —
//! carries both messages and makes both presses, which is exactly what the
//! hand it accelerates would have done.
//!
//! # What this module holds
//!
//! - [`Target`]: what was right-clicked — one variant per object class of
//!   §5.2's table (track rows wherever they appear, queue rows, album tiles,
//!   the bar's now-playing block; playlist panel rows deliberately have no
//!   menu at v1, their acts live on the page where the contents are visible).
//! - [`Facts`]: the readings an item list is decided against — engine
//!   readiness, the playlists folder, playing provenance (09 §6). Split from
//!   `App` so the builder is a pure function a test can sweep exhaustively.
//! - [`items`]: the four menus of §5.2's table, exactly. Verbs only, no
//!   state, nothing that is not a mirror; `Add to "{current}"` present only
//!   while a current playlist stands — absent, not disabled, otherwise (a
//!   control that cannot act must not pretend it can).
//! - [`area`]: the right-press capture. `mouse_area` carries
//!   `on_right_press` in iced 0.13 (`iced_widget-0.13.4/src/mouse_area.rs:53`)
//!   but its message carries no position, and the menu opens **at the
//!   pointer** — so this thin wrapper does what `mouse_area`'s own update
//!   does one line above its publish: it reads `cursor.position()` at the
//!   press and sends it along. Everything else passes through untouched.
//! - [`anchor`]/[`extent`]: the float's geometry — at the pointer, flipped
//!   to stay inside the window at the edges (§5.2), pure and unit-tested.
//!
//! # The widget itself
//!
//! The float is ADR-0016's verified mechanics, composed in `app.rs`: a
//! `stack` over the whole window (the bar included — the bar's own menu may
//! open over it), the card wrapped in `opaque` so a press inside it cannot
//! fall through, **no scrim** (refused), and a full-window `mouse_area`
//! under the card whose left press closes the menu — a press outside is
//! "put it down", never a spent click. A *right* press outside falls
//! through that backdrop (it has no right handler) to whatever row is under
//! it, whose own [`area`] replaces the menu — one menu at a time by
//! construction, because the state is a single `Option`. <kbd>Esc</kbd>
//! peels the menu first, before every panel layer (`App::escape`); an item
//! press closes it and fires. No submenus, no new key bindings.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Shell, Widget, overlay, renderer};
use iced::{Element, Event, Length, Point, Rectangle, Size, Theme, Vector, event, mouse};

use crate::app::Message;
use crate::theme;

/// What was right-clicked: one variant per object class of §5.2's table.
///
/// A target names *data* — an album id, a row index — never pixels, so the
/// menu survives the list scrolling under it and a stale press resolves the
/// way every stale press in baz does: against what the messages find when
/// they land, asking for nothing when the row is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// A track row with a record identity: the album page's rows and the
    /// wall's Songs section (both send [`Message::PlayTrack`] and spend the
    /// same `+`, so they are one menu). `row` is the track's position in the
    /// **selected edition**, exactly what the row's own press carries.
    Track { album: u64, row: usize },
    /// A row of the **Queue** place, by queue position.
    QueueRow { row: usize },
    /// An album tile on the wall (and the Songs section's record door is a
    /// door, not a tile — it opens no menu of its own).
    Album { album: u64 },
    /// A row of an open **playlist's page**. `missing` is the row's own
    /// reading: a broken entry plays nothing and transfers nothing, so its
    /// menu offers nothing.
    PlaylistTrack { row: usize, missing: bool },
    /// The bar's now-playing block — what makes S4 two gestures from
    /// anywhere: the sounding track is always in the bar.
    NowPlaying,
}

/// One item: a short verb, and the presses it makes. Every message here is
/// a press some visible control also makes — the mirror rule, pinned by
/// this module's tests.
#[derive(Debug, Clone)]
pub(crate) struct Item {
    /// The verb, kept short (§5.2): `Play`, `Queue`, `Add to "Road Trip"`…
    pub(crate) label: String,
    /// The messages the item sends, in press order. Usually one; two when
    /// the mirrored gesture is itself two presses (the `+` then a picker
    /// row).
    pub(crate) presses: Vec<Message>,
    /// The gesture that accelerates this exact act, printed quietly at the
    /// row's right edge — the era's menu convention (`⌘Q` beside Quit),
    /// applied to the mirror layer (doc 11 §5 P6.1), as a word — doc 10 §3.6
    /// bans borrowed characters, and the face has no `⇧`. `None` for the many
    /// items whose only accelerator is the control itself; the one carrier
    /// today is the tile menu's `Queue album`, whose shift-click twin was
    /// taught nowhere on screen.
    pub(crate) accelerator: Option<&'static str>,
}

/// An open menu: what was clicked, where, and the items as they were offered.
///
/// The items are captured at open — the menu shows what the listener saw,
/// and a press sends exactly what was on screen. The whole of the overlay
/// state is one `Option<Menu>` on `App`, which is what makes "one menu at a
/// time" structural rather than policed.
#[derive(Debug, Clone)]
pub(crate) struct Menu {
    /// Where the right press landed, in window coordinates — the anchor the
    /// card hangs from ([`anchor`]).
    pub(crate) at: Point,
    /// The items, in §5.2's table order.
    pub(crate) items: Vec<Item>,
}

/// The readings a menu is decided against — a snapshot of the facts, never
/// the holders of them, so [`items`] is a pure function.
#[derive(Debug, Clone, Default)]
pub(crate) struct Facts {
    /// Whether there is an engine to send a play gesture to (the row's own
    /// `on_press_maybe` condition).
    pub(crate) engine_ready: bool,
    /// Whether anything playlist-shaped can happen — the playlists folder
    /// exists ([`crate::playlists::Playlists::available`]), the `+` slots'
    /// own condition.
    pub(crate) collecting: bool,
    /// The current playlist (09 §6): playing provenance naming a file that
    /// still exists — `(panel row id, name)`. `None` withdraws the
    /// `Add to "{name}"` verb rather than letting it dangle.
    pub(crate) current: Option<(u64, String)>,
    /// The record under the lamp, if any — the now-playing block's own
    /// press condition.
    pub(crate) playing_album: Option<u64>,
    /// The sounding row's queue position, if the engine has confirmed one —
    /// what the bar's transfer items spend, because the bar's subject is
    /// the *sounding* track and the queue row holding it is the control
    /// that already knows how to transfer it.
    pub(crate) playing_queue_row: Option<usize>,
}

/// The four menus of §5.2's table, exactly — items in table order, each the
/// verbs the object's own controls already speak.
///
/// An empty answer means *no menu opens*: a target none of whose verbs can
/// act right now (no engine, no playlists folder, a missing entry) offers
/// nothing rather than a card of disabled words.
#[expect(
    clippy::too_many_lines,
    reason = "the five arms are §5.2's one table, transcribed; a function per \
              object class would let one menu's grammar drift from the others'"
)]
pub(crate) fn items(target: Target, facts: &Facts) -> Vec<Item> {
    let mut listed: Vec<Item> = Vec::new();
    let mut push = |label: String, presses: Vec<Message>, accelerator: Option<&'static str>| {
        listed.push(Item {
            label,
            presses,
            accelerator,
        });
    };
    match target {
        Target::Track { album, row } => {
            // `Play` · `Queue` · `Add to "{current}"` · `Add to playlist…`
            if facts.engine_ready {
                push(
                    "Play".to_owned(),
                    vec![Message::PlayTrack(album, row)],
                    None,
                );
            }
            if facts.collecting {
                push(
                    "Queue".to_owned(),
                    vec![Message::AddTrackToPlaylist(album, row), Message::PickQueue],
                    None,
                );
                if let Some((id, name)) = &facts.current {
                    push(
                        add_to(name),
                        vec![
                            Message::AddTrackToPlaylist(album, row),
                            Message::PickPlaylist(*id),
                        ],
                        None,
                    );
                }
                push(
                    "Add to playlist…".to_owned(),
                    vec![Message::AddTrackToPlaylist(album, row)],
                    None,
                );
            }
        }
        Target::QueueRow { row } => {
            // `Play` · `Add to "{current}"` · `Add to playlist…` · `Remove`
            if facts.engine_ready {
                push("Play".to_owned(), vec![Message::JumpToQueued(row)], None);
            }
            if facts.collecting {
                if let Some((id, name)) = &facts.current {
                    push(
                        add_to(name),
                        vec![
                            Message::AddQueuedToPlaylist(row),
                            Message::PickPlaylist(*id),
                        ],
                        None,
                    );
                }
                push(
                    "Add to playlist…".to_owned(),
                    vec![Message::AddQueuedToPlaylist(row)],
                    None,
                );
            }
            push("Remove".to_owned(), vec![Message::RemoveQueued(row)], None);
        }
        Target::Album { album } => {
            // `Open` · `Play album` · `Queue album` · `Add to playlist…`
            push("Open".to_owned(), vec![Message::AlbumClicked(album)], None);
            if facts.engine_ready {
                push(
                    "Play album".to_owned(),
                    vec![Message::PlayAlbum(album)],
                    None,
                );
            }
            if facts.collecting {
                // The one item with a printed accelerator (doc 11 §5 P6.1):
                // shift-click a sleeve queues the record, and until this
                // column the gesture was taught nowhere a user could
                // stumble on it. The menu row is exactly where the era
                // printed accelerators — beside the verb they accelerate.
                push(
                    "Queue album".to_owned(),
                    vec![Message::AddAlbumToPlaylist(album), Message::PickQueue],
                    // A word, not `⇧`: doc 10 §3.6 — a slot carries a drawn
                    // glyph or a word, never a character borrowed from a
                    // face that may not have it, and IBM Plex draws U+21E7
                    // as tofu (verified on a rendered frame).
                    Some("Shift-click"),
                );
                push(
                    "Add to playlist…".to_owned(),
                    vec![Message::AddAlbumToPlaylist(album)],
                    None,
                );
            }
        }
        Target::PlaylistTrack { row, missing } => {
            // The track-row menu, spending the page's own messages. A
            // missing entry's row is not a control (`views/playlist.rs`),
            // so its menu offers nothing at all.
            if missing {
                return listed;
            }
            if facts.engine_ready {
                push(
                    "Play".to_owned(),
                    vec![Message::PlaylistPlayTrack(row)],
                    None,
                );
            }
            if facts.collecting {
                push(
                    "Queue".to_owned(),
                    vec![Message::PlaylistAddEntry(row), Message::PickQueue],
                    None,
                );
                if let Some((id, name)) = &facts.current {
                    push(
                        add_to(name),
                        vec![Message::PlaylistAddEntry(row), Message::PickPlaylist(*id)],
                        None,
                    );
                }
                push(
                    "Add to playlist…".to_owned(),
                    vec![Message::PlaylistAddEntry(row)],
                    None,
                );
            }
        }
        Target::NowPlaying => {
            // `Go to record` · `Add to "{current}"` · `Add to playlist…` —
            // every item resolved against the *sounding* row, which is why
            // S4 is two gestures from anywhere: the bar is everywhere.
            if facts.playing_album.is_some() {
                push(
                    "Go to record".to_owned(),
                    vec![Message::ShowPlayingAlbum],
                    None,
                );
            }
            if let Some(row) = facts.playing_queue_row
                && facts.collecting
            {
                if let Some((id, name)) = &facts.current {
                    push(
                        add_to(name),
                        vec![
                            Message::AddQueuedToPlaylist(row),
                            Message::PickPlaylist(*id),
                        ],
                        None,
                    );
                }
                push(
                    "Add to playlist…".to_owned(),
                    vec![Message::AddQueuedToPlaylist(row)],
                    None,
                );
            }
        }
    }
    listed
}

/// `Add to "Road Trip"` — the S4 verb, naming the current playlist so the
/// press is legible before it is made (09 §6). Typographic quotes, the
/// pick labels' own.
fn add_to(name: &str) -> String {
    format!("Add to \u{201c}{name}\u{201d}")
}

/// The card's size for `count` items: [`theme::MENU_W`] wide, each item one
/// [`theme::TRANSPORT_HIT`] control height (law L7), the card's own
/// [`theme::GAP_XS`] air above and below.
pub(crate) fn extent(count: usize) -> Size {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a menu holds at most four items"
    )]
    let rows = count as f32;
    Size::new(
        theme::MENU_W,
        rows * theme::TRANSPORT_HIT + 2.0 * theme::GAP_XS,
    )
}

/// Where the card's top-left corner goes: at the pointer, **flipped** to the
/// pointer's other side at an edge the card would cross (§5.2), and clamped
/// to the window as the last resort so it is never clipped.
pub(crate) fn anchor(at: Point, size: Size, window: Size) -> Point {
    let flip = |position: f32, extent: f32, limit: f32| {
        if position + extent > limit {
            // The other side of the pointer, held on screen.
            (position - extent).max(0.0)
        } else {
            position
        }
    };
    Point::new(
        flip(at.x, size.width, window.width),
        flip(at.y, size.height, window.height),
    )
}

/// Wrap `content` so a right press inside it opens the menu for `target`,
/// **at the pointer**.
///
/// The wrapper forwards every event to its content first and captures only
/// the right press the content ignored — a row's own left press, hovers,
/// wheel travel all pass through exactly as before, which is what makes
/// this a layer over the controls rather than a change to them.
pub(crate) fn area<'a>(
    content: impl Into<Element<'a, Message>>,
    target: Target,
) -> Element<'a, Message> {
    Element::new(Area {
        content: content.into(),
        target,
    })
}

/// The right-press capture widget behind [`area`]. See the module docs for
/// why `mouse_area::on_right_press` alone was not enough: its message
/// carries no position, and the float opens at the pointer.
struct Area<'a> {
    content: Element<'a, Message>,
    target: Target,
}

impl Widget<Message, Theme, iced::Renderer> for Area<'_> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> event::Status {
        // The content first — a child that captures the event owns it
        // (`mouse_area` forwards in exactly this order). No child takes a
        // right press today, so the order is future-proofing, not routing.
        if self.content.as_widget_mut().on_event(
            &mut tree.children[0],
            event.clone(),
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        ) == event::Status::Captured
        {
            return event::Status::Captured;
        }
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) = event
            && let Some(at) = cursor.position()
            && layout.bounds().contains(at)
        {
            shell.publish(Message::OpenMenu(self.target, at));
            return event::Status::Captured;
        }
        event::Status::Ignored
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every fact state the sweep below visits: both engine states, both
    /// folder states, provenance standing and not, something sounding and
    /// not — the whole domain [`items`] is defined over.
    fn every_facts() -> Vec<Facts> {
        let mut swept = Vec::new();
        for engine_ready in [false, true] {
            for collecting in [false, true] {
                for current in [None, Some((7_u64, "Road Trip".to_owned()))] {
                    for playing in [None, Some((3_u64, 2_usize))] {
                        swept.push(Facts {
                            engine_ready,
                            collecting,
                            current: current.clone(),
                            playing_album: playing.map(|(album, _)| album),
                            playing_queue_row: playing.map(|(_, row)| row),
                        });
                    }
                }
            }
        }
        swept
    }

    /// Every target class, with fixture identities.
    fn every_target() -> Vec<Target> {
        vec![
            Target::Track { album: 11, row: 4 },
            Target::QueueRow { row: 6 },
            Target::Album { album: 11 },
            Target::PlaylistTrack {
                row: 2,
                missing: false,
            },
            Target::PlaylistTrack {
                row: 2,
                missing: true,
            },
            Target::NowPlaying,
        ]
    }

    /// **Every menu item is a press some visible on-screen control also
    /// makes** — §5.2's governing rule, pinned the way the keyboard's is
    /// (`app.rs`, `every_keyboard_binding_is_a_press_some_control_also_makes`):
    /// exhaustively, with the control that sends each message named. A new
    /// menu item therefore cannot be added without either pointing at a
    /// control or failing this test by name.
    ///
    /// The sweep produces every item every menu can offer, in every fact
    /// state, and checks every message in every item's press list. An item
    /// whose gesture is two presses (`Queue` = the `+` then the picker's
    /// Queue row) is checked press by press — each half must have its
    /// visible twin.
    #[test]
    fn every_menu_item_is_a_press_some_control_also_makes() {
        /// Message tag → the on-screen control that sends the same message.
        /// There is no "reason there is none" column here on purpose: the
        /// keyboard's table needed one for MPRIS, and a menu item with no
        /// visible twin is exactly the thing this test exists to refuse.
        const CONTROLS: [(&str, &str); 11] = [
            (
                "PlayTrack",
                "a track row's own press (album page, Songs section)",
            ),
            ("JumpToQueued", "a queue row's own press"),
            ("PlaylistPlayTrack", "a playlist page row's own press"),
            (
                "AlbumClicked",
                "an album tile's press (and the Songs section's record door)",
            ),
            ("PlayAlbum", "the record page's `Play album`"),
            ("ShowPlayingAlbum", "the bar's now-playing block"),
            ("AddTrackToPlaylist", "the track row's reserved `+` slot"),
            ("AddQueuedToPlaylist", "the queue row's reserved `+` slot"),
            (
                "PlaylistAddEntry",
                "the playlist page row's reserved `+` slot",
            ),
            ("AddAlbumToPlaylist", "the record page's `Add to playlist…`"),
            ("RemoveQueued", "the queue row's ✕"),
            // `PickQueue` / `PickPlaylist` are below as the picker's own rows.
        ];
        const PICKER_ROWS: [(&str, &str); 2] = [
            ("PickQueue", "the picker's Queue row (09 §8.1)"),
            (
                "PickPlaylist",
                "the picker's playlist rows, the hoisted playing one included",
            ),
        ];
        let mut produced: Vec<String> = Vec::new();
        for target in every_target() {
            for facts in every_facts() {
                for item in items(target, &facts) {
                    assert!(
                        !item.presses.is_empty(),
                        "`{}` presses nothing — an inert item is a lie",
                        item.label
                    );
                    for press in &item.presses {
                        let debug = format!("{press:?}");
                        let tag = debug
                            .split_once('(')
                            .map_or(debug.as_str(), |(head, _)| head)
                            .to_owned();
                        assert!(
                            CONTROLS
                                .iter()
                                .chain(PICKER_ROWS.iter())
                                .any(|(name, _)| *name == tag),
                            "menu item `{}` sends `{tag}`, which no visible control is \
                             on record as sending — name the control, or the item is a \
                             gesture-only route and the mirror rule refuses it",
                            item.label
                        );
                        produced.push(tag);
                    }
                }
            }
        }
        // The table cuts both ways: an entry no menu spends any more is
        // stale, and stale tables are how mirrors drift.
        for (tag, _) in CONTROLS.iter().chain(PICKER_ROWS.iter()) {
            assert!(
                produced.contains(&(*tag).to_owned()),
                "CONTROLS still names `{tag}`, which no menu item sends any more"
            );
        }
    }

    /// **The picker never offers `Add to "All songs"`** — the implicit list is
    /// playable and viewable, never a destination (`crate::all_songs`).
    ///
    /// There is no file behind it to append to, so an `Add` aimed at it would
    /// be a verb promising a write with nowhere to go. This is already
    /// structural — the picker's destinations are `PanelRow`s read out of the
    /// playlists folder, and `AllSongs` is not one and carries no id, no path
    /// and no `save` — but it is asserted anyway, over **every** target this
    /// menu knows and over the whole `Facts` space that could name a current
    /// list. "Structurally impossible" is what the last surface that quietly
    /// grew a destination looked like from the inside as well.
    ///
    /// The trap it closes is a specific one the earlier mapping named: giving
    /// the wall's run *provenance* would immediately make every one of these
    /// menus offer the named verb for a list with no file. So the sweep is over
    /// the labels, which is where that would surface.
    #[test]
    fn no_menu_anywhere_offers_to_add_to_the_implicit_list() {
        let targets = [
            Target::NowPlaying,
            Target::Album { album: 3 },
            Target::Track { album: 3, row: 1 },
            Target::QueueRow { row: 2 },
            Target::PlaylistTrack {
                row: 1,
                missing: false,
            },
            Target::PlaylistTrack {
                row: 1,
                missing: true,
            },
        ];
        // Every combination of the facts that could put a name in a verb.
        //
        // `current` is deliberately swept over its **reachable** values only —
        // no provenance, and a real playlist file. Handing it the implicit
        // list's name would prove nothing about the product, because the way
        // that value is built is the guard: it is `queue_provenance()` filtered
        // through `Playlists::row`, so it names a file that exists or it is
        // `None`. The half of the trap that lives upstream — the run carrying
        // provenance the implicit list has no business having — is asserted at
        // its own source, in `all_songs`, where a `Some` would have to be
        // written for this menu to ever see one.
        let currents = [None, Some((7, "Road Trip".to_owned()))];
        let named = format!(
            "Add to \u{201c}{}\u{201d}",
            crate::implicit::Origin::AllSongs.name()
        );
        for target in targets {
            for current in &currents {
                for collecting in [false, true] {
                    let facts = Facts {
                        engine_ready: true,
                        collecting,
                        current: current.clone(),
                        playing_album: Some(3),
                        playing_queue_row: Some(2),
                    };
                    for item in items(target, &facts) {
                        assert_ne!(
                            item.label, named,
                            "{target:?} offered to write to a list with no file"
                        );
                        assert!(
                            !item
                                .label
                                .contains(crate::implicit::Origin::AllSongs.name())
                                || !item.label.starts_with("Add to"),
                            "{target:?} named the implicit list in a transfer verb: {:?}",
                            item.label
                        );
                    }
                }
            }
        }
    }

    /// **S4's item, at the menu layer** (09 §4): provenance standing → the
    /// bar's menu carries `Add to "{name}"`, and its presses are the
    /// sounding row's `+` then the picker's hoisted row — the file append,
    /// never the run. No provenance, or nothing sounding → absent, not
    /// disabled.
    #[test]
    fn the_bars_menu_carries_the_current_playlist_exactly_while_provenance_stands() {
        let with = Facts {
            engine_ready: true,
            collecting: true,
            current: Some((7, "Road Trip".to_owned())),
            playing_album: Some(3),
            playing_queue_row: Some(2),
        };
        let listed = items(Target::NowPlaying, &with);
        let labels: Vec<&str> = listed.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "Go to record",
                "Add to \u{201c}Road Trip\u{201d}",
                "Add to playlist…"
            ],
            "§5.2's bar row, in table order, the current playlist named"
        );
        let add = &listed[1];
        assert!(
            matches!(
                add.presses.as_slice(),
                [Message::AddQueuedToPlaylist(2), Message::PickPlaylist(7)]
            ),
            "the S4 press is the sounding row's `+` then the hoisted pick — \
             the file gains the track, the run is untouched (09 §6): {:?}",
            add.presses
        );
        // Provenance withdrawn: the verb goes with it, absent not disabled.
        let without = Facts {
            current: None,
            ..with.clone()
        };
        assert!(
            items(Target::NowPlaying, &without)
                .iter()
                .all(|item| !item.label.starts_with("Add to \u{201c}")),
            "no provenance, no named verb"
        );
        // Nothing sounding: the bar's menu has nothing to say at all.
        let silent = Facts {
            playing_album: None,
            playing_queue_row: None,
            ..with
        };
        assert!(items(Target::NowPlaying, &silent).is_empty());
    }

    /// **The accelerator column carries exactly the gestures that exist**
    /// (doc 11 §5 P6.1): `Queue album` prints its shift-click twin — the
    /// one modifier gesture that was taught nowhere on screen — and no
    /// other item invents one. The era printed `⌘Q` beside Quit; a hint
    /// beside a verb with no gesture would be a lie in the same type.
    #[test]
    fn only_queue_album_prints_an_accelerator_and_it_is_shift_click() {
        for target in every_target() {
            for facts in every_facts() {
                for item in items(target, &facts) {
                    match item.label.as_str() {
                        "Queue album" => assert_eq!(
                            item.accelerator,
                            Some("Shift-click"),
                            "the tile menu's queueing verb teaches its gesture"
                        ),
                        _ => assert_eq!(
                            item.accelerator, None,
                            "`{}` has no gesture to print",
                            item.label
                        ),
                    }
                }
            }
        }
    }

    /// A missing playlist entry's menu offers nothing: its row is not a
    /// control (`views/playlist.rs`), and a menu that offered `Play` over a
    /// file that is gone would be the control pretending it can act.
    #[test]
    fn a_missing_entrys_menu_offers_nothing() {
        for facts in every_facts() {
            assert!(
                items(
                    Target::PlaylistTrack {
                        row: 0,
                        missing: true
                    },
                    &facts
                )
                .is_empty()
            );
        }
    }

    /// The float stays inside the window: anchored at the pointer in the
    /// open field, flipped to the pointer's other side at each edge it
    /// would cross, and clamped at the corner where even the flip runs out.
    #[test]
    fn the_card_flips_at_the_edges_and_never_leaves_the_window() {
        let window = Size::new(1280.0, 860.0);
        let size = extent(4);
        // Open field: the pointer is the corner.
        let at = Point::new(400.0, 300.0);
        assert_eq!(anchor(at, size, window), at);
        // The right edge: the card swings to the pointer's left.
        let at = Point::new(1250.0, 300.0);
        let placed = anchor(at, size, window);
        assert!((placed.x - (at.x - size.width)).abs() < f32::EPSILON);
        assert!((placed.y - at.y).abs() < f32::EPSILON);
        // The bottom edge: it swings above.
        let at = Point::new(400.0, 840.0);
        let placed = anchor(at, size, window);
        assert!((placed.y - (at.y - size.height)).abs() < f32::EPSILON);
        // Both at once, in a window smaller than two cards: clamped on
        // screen rather than clipped off it.
        let tiny = Size::new(size.width + 10.0, size.height + 10.0);
        let placed = anchor(Point::new(5.0, 5.0), size, tiny);
        assert!(placed.x >= 0.0 && placed.x + size.width <= tiny.width + size.width);
        assert!(placed.y >= 0.0);
    }

    /// The card's height is items × the product's one control height plus
    /// its own air — the arithmetic the anchor flips against, pinned so the
    /// two cannot drift.
    #[test]
    fn the_cards_extent_is_the_item_count_at_control_height() {
        let size = extent(3);
        assert!((size.width - theme::MENU_W).abs() < f32::EPSILON);
        assert!(
            (size.height - (3.0 * theme::TRANSPORT_HIT + 2.0 * theme::GAP_XS)).abs() < f32::EPSILON
        );
    }
}
