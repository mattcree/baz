//! **One playlist page, at either persistence state.**
//!
//! A saved `.m3u8` and the unsaved run are one product concept. The file and
//! the run have different capabilities, but they do not earn different
//! geometry: this module alone owns their sleeve, responsive composition,
//! identity hierarchy, `TRACKS` block, empty state and row-space mapping.
//!
//! Callers supply facts and controls in named slots. A saved list spends them
//! on Play, Rename, Delete and file counts; the unsaved list spends them on
//! Save, live cursor/remaining time and run provenance. Neither caller can
//! choose another sleeve size, breakpoint, empty-state anatomy or scroller.

use iced::widget::{Space, image as iced_image, scrollable};
use iced::{Element, Length};

use crate::app::{Message, Shelf};
use crate::theme;
use crate::views::page;

/// The one empty statement under a playlist page's `TRACKS` rule.
pub(crate) const EMPTY: &str = "Nothing here yet.";

/// The responsive reading both persistence states use while building rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Layout {
    side_by_side: bool,
}

impl Layout {
    /// Whether the row has room for its dedicated Album column.
    #[must_use]
    pub(crate) fn side_by_side(self) -> bool {
        self.side_by_side
    }

    /// Translate the page scroller's offset into row space.
    ///
    /// The desktop scroller begins at row zero. The stacked document begins
    /// with the sleeve and identity, so the same generous margin used by the
    /// virtual window absorbs that fixed prefix without duplicating the page's
    /// vertical arithmetic in either caller.
    #[must_use]
    pub(crate) fn rows_scroll(self, scroll: f32) -> f32 {
        if self.side_by_side {
            scroll
        } else {
            (scroll - super::playlist::WINDOW_MARGIN).max(0.0)
        }
    }
}

/// Resolve the playlist page's one responsive form.
#[must_use]
pub(crate) fn layout(window_width: f32) -> Layout {
    Layout {
        side_by_side: page::is_playlist_two_column(window_width),
    }
}

/// Preserve the same row-space offset when either playlist state crosses
/// between the desktop table and stacked document.
#[must_use]
pub(crate) fn reflow_scroll_offset(scroll: f32, was_table: bool, is_table: bool) -> f32 {
    match (was_table, is_table) {
        (true, false) if scroll > 0.0 => scroll + super::playlist::WINDOW_MARGIN,
        (false, true) => (scroll - super::playlist::WINDOW_MARGIN).max(0.0),
        _ => scroll,
    }
}

/// The state/capability slots around the shared playlist anatomy.
pub(crate) struct PlaylistPage<'a> {
    /// The subject in the place header: a breadcrumb for a file, a plain name
    /// for the transient run.
    pub(crate) lead: Element<'static, Message>,
    /// The name used by the identity and by an artwork-free rest tile.
    pub(crate) name: String,
    /// Up to four record identities quoted by the collage.
    pub(crate) art: Vec<u64>,
    /// The **authored** sleeve, where the listener set one — drawn instead of
    /// the collage. `None` on the transient run and on the built-in, neither
    /// of which is a file a picture can sit beside.
    pub(crate) image: Option<iced_image::Handle>,
    /// The durable Play commitment. `None` for the already-current run; the
    /// compositor reserves the same control-height slot in that state.
    pub(crate) commitment: Option<Element<'a, Message>>,
    /// Persistence-specific quiet acts: Rename/Delete or Save/readout.
    pub(crate) acts: Vec<Element<'a, Message>>,
    /// The shared three-line identity with state-specific words and controls.
    pub(crate) identity: page::Identity<'a>,
    /// The virtualized slice, expressed in the shared row anatomy.
    pub(crate) rows: Vec<Element<'a, Message>>,
    /// The one viewport reading for this persistence state.
    pub(crate) on_scroll: fn(scrollable::Viewport) -> Message,
}

/// Draw either persistence state through one playlist composition.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    subject: PlaylistPage<'a>,
    window_width: f32,
) -> Element<'a, Message> {
    let responsive = layout(window_width);
    let PlaylistPage {
        lead,
        name,
        art,
        commitment,
        image,
        acts,
        identity,
        rows,
        on_scroll,
    } = subject;
    page::view(
        page::Page {
            lead,
            sleeve: super::playlist_sleeve_authored(
                shelf,
                image.as_ref(),
                &art,
                &name,
                theme::ALBUM_SLEEVE,
            ),
            // A run has no command that creates the playback truth it already
            // carries. Keep the slot rather than collapsing the aside.
            commitment: Some(commitment.unwrap_or_else(|| {
                Space::new()
                    .height(Length::Fixed(theme::TRANSPORT_HIT))
                    .into()
            })),
            acts,
            aside_tail: Vec::new(),
            identity,
            rows,
            side_by_side: responsive.side_by_side,
            row_spacing: 0.0,
            on_scroll: Some(on_scroll),
            empty: EMPTY,
        },
        window_width,
    )
}

/// One playlist-row record sleeve. Saved entries and run entries resolve the
/// same library identity through this exact drawing path.
pub(crate) fn row_art(shelf: &Shelf, album_id: Option<u64>) -> Element<'static, Message> {
    let edge = theme::PANEL_SLEEVE;
    match album_id {
        Some(id) => shelf.thumb(id).map_or_else(
            || crate::views::gradient_block(id, edge, 1.0),
            |handle| {
                iced_image(handle.clone())
                    .width(Length::Fixed(edge))
                    .height(Length::Fixed(edge))
                    .into()
            },
        ),
        None => Space::new()
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .into(),
    }
}

#[cfg(test)]
mod tests {
    /// Saved and unsaved lists may populate capability slots; neither may
    /// grow a private playlist composition again.
    #[test]
    fn both_persistence_states_reach_one_playlist_page() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for file in ["src/views/playlist.rs", "src/views/queue.rs"] {
            let source = std::fs::read_to_string(root.join(file))
                .expect("a playlist state's source")
                .split("#[cfg(test)]")
                .next()
                .expect("a source has a head")
                .to_owned();
            assert!(
                source.contains("playlist_page::view("),
                "{file} bypasses the shared playlist page"
            );
            let private_source = source.replace("playlist_page::view(", "");
            for private in [
                "page::view(",
                "playlist_sleeve(",
                "is_playlist_two_column(",
                "place_pad()",
                "scrollable(",
            ] {
                assert!(
                    !private_source.contains(private),
                    "{file} owns `{private}` again instead of supplying a slot"
                );
            }
        }
    }

    #[test]
    fn both_states_share_the_breakpoint_and_row_space_mapping() {
        let narrow = super::layout(crate::theme::PLAYLIST_BREAKPOINT - 1.0);
        let wide = super::layout(crate::theme::PLAYLIST_BREAKPOINT);
        assert!(!narrow.side_by_side());
        assert!(wide.side_by_side());
        assert!((wide.rows_scroll(731.0) - 731.0).abs() < f32::EPSILON);
        assert!(narrow.rows_scroll(100.0).abs() < f32::EPSILON);
        assert!(
            (narrow.rows_scroll(super::super::playlist::WINDOW_MARGIN + 131.0) - 131.0).abs()
                < f32::EPSILON
        );
    }
}
