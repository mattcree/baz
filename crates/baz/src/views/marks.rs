//! **The strip that appears when more than one thing is selected**, and the
//! only place a bulk verb is spoken.
//!
//! `docs/WORK.md` item 62. baz's selection became a set
//! ([`crate::selection`]); this is what a set is *for*, and it exists as a
//! visible control rather than as a right-click menu because of the standing
//! rule `crate::menu` is built on:
//!
//! > **Every menu item sends only messages some visible on-screen control also
//! > sends, and no action's only route is a menu.**
//!
//! A bulk action reachable only by right-clicking would be an action whose
//! only route is a gesture, which this product does not ship. So the verbs
//! live here, in the open, and a menu may mirror them later.
//!
//! # Why it is a strip at the foot of the place
//!
//! It is **about the selection**, not about the page, and a selection is
//! something a listener assembled a moment ago and is about to spend. At the
//! foot it is next to the transport — where the other "and now do something"
//! controls are — and it is out of the way of the rows being ticked, which are
//! usually being ticked from the top down.
//!
//! # It is always in the tree
//!
//! Empty at rest, for the reason this codebase states at length elsewhere:
//! iced diffs the widget tree by position, so a strip that *appeared* would
//! push the place one level down and hand it a fresh state — scrolling the
//! list back to the top at the exact moment somebody has ticked their fifth
//! row in it. `crate::app`'s own `no_floating_layer_comes_and_goes_from_the_tree`
//! is the rule; this is the same rule one level in.
//!
//! # Nothing here rests on colour
//!
//! The count is a **number**, the verbs are **words**, and *Remove* stands
//! apart in the alert ink as the third tier of design note 20 — which is a
//! position in the row as much as a hue, because a reading nobody can make in
//! greyscale is not a reading.

use iced::widget::{Space, container, row, text};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::selection::{Content, Run};
use crate::theme;

/// The strip's height — [`theme::TRANSPORT_HIT`] with a [`theme::GAP_SM`]
/// above and below, so the words inside it own law L7's target and the strip
/// is not a 20 px line with two controls on it.
pub(crate) const STRIP_H: f32 = theme::TRANSPORT_HIT + 2.0 * theme::GAP_SM;

/// **What a set can be spent on**, given what is in it.
///
/// Its own function because the answer is a *product* decision rather than a
/// layout one, and because it is the thing worth testing: `Remove` belongs
/// only where a list is a thing a listener owns and can shorten.
#[must_use]
pub(crate) fn removable(marked: &[Content]) -> bool {
    matches!(
        marked.first().map(|first| first.run()),
        Some(Run::Queue | Run::PlaylistTracks(_))
    )
}

/// **The strip, or nothing at all.**
///
/// Nothing for a set of one: one selected row is the ordinary state of every
/// list in the product and has never needed a bar to explain it. The strip is
/// the answer to *you have built something*, and one thing is not built.
pub(crate) fn view(marked: &[Content], collecting: bool) -> Element<'static, Message> {
    if marked.len() < 2 {
        return Space::new().width(Length::Fill).height(0.0).into();
    }
    let room = theme::active();
    let mut words = row![
        text(count(marked.len()))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .font(theme::MEDIUM)
            .color(room.paper),
        Space::new().width(Length::Fill),
        crate::views::page::act("Play", true, Message::MarkedPlay),
        crate::views::page::act("Queue", true, Message::MarkedQueue),
    ]
    .spacing(theme::GAP_SM)
    .align_y(alignment::Vertical::Center);
    // The `+` slots' own condition, unchanged: with no playlists folder there
    // is nothing playlist-shaped to offer, and a control that cannot act must
    // not pretend it can.
    if collecting {
        words = words.push(crate::views::page::act(
            "Add to playlist…",
            true,
            Message::MarkedAddToPlaylist,
        ));
    }
    if removable(marked) {
        words = words.push(crate::views::page::destructive_act(
            "Remove",
            true,
            Message::MarkedRemove,
        ));
    }
    // **The way out is a word, not only a key.** Esc clears the set, and a
    // gesture that is the only route to a state is the thing the visible
    // control rule forbids.
    words = words.push(crate::views::page::act("Clear", true, Message::MarksClear));
    container(words)
        .width(Length::Fill)
        .height(Length::Fixed(STRIP_H))
        .padding(theme::pad(0.0, theme::HANG))
        .style(move |_theme| container::Style {
            // One plane up from the place, so it reads as laid *on* it rather
            // than cut into it — and no accent: the lamp is playback truth and
            // a selection is not (doc 07 L8.4).
            background: Some(iced::Background::Color(room.plinth)),
            ..container::Style::default()
        })
        .into()
}

/// **The same slot, saying where a drop will land.**
///
/// One slot with two tenants, and they cannot both be wanted: you are either
/// assembling a selection or dragging something in from outside. The hover
/// wins while it is happening, because it is about a gesture in flight and the
/// selection is not going anywhere.
///
/// Its own function rather than a branch inside [`view`] so the two readings
/// stay legible; `crate::app` chooses between them.
pub(crate) fn hint(words: String) -> Element<'static, Message> {
    let room = theme::active();
    container(
        text(words)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .font(theme::MEDIUM)
            .color(room.paper),
    )
    .width(Length::Fill)
    .height(Length::Fixed(STRIP_H))
    .padding(theme::pad(0.0, theme::HANG))
    .align_y(alignment::Vertical::Center)
    .style(move |_theme| container::Style {
        background: Some(iced::Background::Color(room.plinth)),
        ..container::Style::default()
    })
    .into()
}

/// `5 selected` — the count first, because the count is the fact the strip
/// exists to state and the verbs after it are what to do about it.
fn count(marked: usize) -> String {
    format!("{marked} selected")
}

#[cfg(test)]
mod tests {
    use super::{count, removable};
    use crate::selection::Content;

    /// **`Remove` belongs only where a listener owns the list.** A queue and a
    /// playlist page are both things they assembled and can shorten; an album
    /// page and a search result are not, and offering to remove a track from a
    /// record would be offering to delete a file under another name.
    #[test]
    fn only_a_list_a_listener_owns_can_be_shortened() {
        assert!(removable(&[Content::QueueTrack(0), Content::QueueTrack(1)]));
        assert!(removable(&[
            Content::PlaylistTrack {
                playlist: 1,
                row: 0
            },
            Content::PlaylistTrack {
                playlist: 1,
                row: 1
            },
        ]));
        for immovable in [
            vec![
                Content::AlbumTrack { album: 1, row: 0 },
                Content::AlbumTrack { album: 1, row: 1 },
            ],
            vec![
                Content::SearchTrack { album: 1, row: 0 },
                Content::SearchTrack { album: 1, row: 1 },
            ],
            vec![Content::Album(1), Content::Album(2)],
            vec![],
        ] {
            assert!(
                !removable(&immovable),
                "{immovable:?} was offered a Remove it cannot mean"
            );
        }
    }

    /// **The count is a number and reads as one**, because it is the fact the
    /// strip exists to state and no colour carries it.
    #[test]
    fn the_strip_states_how_many() {
        assert_eq!(count(2), "2 selected");
        assert_eq!(count(57), "57 selected");
    }
}
