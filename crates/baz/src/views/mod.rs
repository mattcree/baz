//! View composition — ADR-0006's layer 3, and the only disposable one.
//!
//! One module per surface of the interface:
//!
//! - [`setup`] — the first-run "Where's your music?" screen.
//! - [`top_bar`] — the search well, the group-key row, and the quiet counts.
//! - [`shelf`] — the wall: the shelved, virtualized album grid, its pinned
//!   group headers, the index rail, its tiles and its empty states.
//! - [`album`] — the record's page: art, identity, `Play album`, the track
//!   list and the condition report.
//! - [`queue`] — the queue place: what baz handed the engine, and where it is
//!   in it.
//! - [`playlist`] — a playlist's page: the durable list, its acts, and its
//!   rows in the queue place's anatomy (ADR-0024 §4).
//! - [`playlist_panel`] — the one summoned panel: the directory of every
//!   list baz holds, the unnamed sounding one at its head, and the picker a
//!   transfer gesture summons (ADR-0024 §5, as amended by design doc 09).
//!   Not a place — it floats over one, which is why the "one kind of
//!   surface" sentence below now carries its named exception.
//! - [`settings`] — the Settings place: the standing decisions, today
//!   ReplayGain.
//! - [`bottom_bar`] — now-playing, transport, the two timestamps, and the
//!   needle flush on the window's bottom edge.
//! - [`context_menu`] — the mirror layer's float (doc 09 §5.2): the card of
//!   verbs at the pointer, over whichever place and the bar alike. Not a
//!   surface of its own — every item is a press some visible control also
//!   makes ([`crate::menu`]).
//!
//! # There is one kind of surface now, and a bar
//!
//! ADR-0016 had four kinds — place, inspector, popover, bar. ADR-0022 deleted
//! two of them: **every surface here except [`bottom_bar`] is a place, or part
//! of one**. [`top_bar`] and [`shelf`] compose the Library; [`album`],
//! [`queue`] and [`settings`] are the other three. Places fill the window and
//! replace each other ([`crate::place`]), and [`bottom_bar`] is in every one of
//! them and never moves.
//!
//! That is why [`place_header`] is shared rather than copied: three places draw
//! the same strip, in the same geometry as the Library's [`top_bar`], because
//! **the frame is the frame in every place** — navigating may not slide the
//! content area by a pixel.
//!
//! Everything here is iced-specific and holds no state: each module exposes a
//! `view` function that reads [`crate::app`]'s state (and [`crate::player`]'s
//! render-ready readings) and returns an [`Element`]. Composition — which
//! surface is on screen — stays in `app.rs` with the state and the update loop;
//! these modules only know how to draw one surface each. A layout or visual
//! redesign rewrites these files and nothing else, which is the whole point of
//! the split.
//!
//! Values, not layout, live in [`crate::theme`]: no view function here may
//! carry a hardcoded color, size, or padding (ADR-0006 calls that a
//! review-blocking defect). The few constants that *are* here are geometry a
//! single surface owns, and each sits in the module that draws it.
//!
//! # `views::shelf` and `shelf`
//!
//! There are two shelves and they are different layers: [`crate::shelf`] is
//! the pure virtualization *math* (layer 1, unit-tested without a window),
//! [`views::shelf`](shelf) is the *composition* that spends it. The geometry
//! module keeps its place and its name; where a view file needs it, it is
//! imported as `geometry` so the two never read as the same thing.

pub(crate) mod album;
pub(crate) mod bottom_bar;
pub(crate) mod context_menu;
pub(crate) mod drag_ghost;
pub(crate) mod home;
pub(crate) mod lane;
pub(crate) mod now_playing;
pub(crate) mod playlist;
pub(crate) mod playlist_panel;
pub(crate) mod queue;
pub(crate) mod settings;
pub(crate) mod setup;
pub(crate) mod shelf;
pub(crate) mod top_bar;

use iced::widget::{Space, column, container, horizontal_rule, image as iced_image, row, text};
use iced::{Color, Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::{theme, vm};

/// A `size`×`size` block filled with the album's deterministic two-color
/// gradient (hash → HSL, see [`vm::gradient_colors`]) — a stand-in sleeve,
/// square-cornered like the artwork it substitutes.
///
/// Shared rather than owned by one surface: the same placeholder stands in
/// for a missing sleeve on a tile and on the record's page, and a redesign that
/// changed one and not the other would be a bug.
///
/// # It is quieter than a real cover, on purpose
///
/// The stops are pulled back toward the sleeve's recess backing by
/// [`theme::Palette::placeholder_ink`], and that is the fix for something
/// plainly wrong in every wide screenshot: at full strength these gradients were
/// the *brightest* objects on a wall of mostly-dark real artwork, so the eye
/// went first to the records baz knows least about. An album with no cover
/// should be the quietest tile in its row.
///
/// The hues survive the mix, which is the whole reason the gradient exists:
/// two albums with no art must still look like two different albums.
///
/// # `shown`
///
/// How strongly the placeholder is drawn, 0…1 — the gradient's own answer to
/// the opacity a real thumbnail is composited at when its record is **outside a
/// running shuffle's pool** ([`theme::POOL_DIM`]). A gradient background is
/// painted rather than sampled, so there is nothing to set an opacity on; it is
/// mixed toward the wall instead, which is what compositing it at that opacity
/// against the wall would have produced. Ordinary tiles pass 1.0 and the mix is
/// the identity.
pub(crate) fn gradient_block(album_id: u64, size: f32, shown: f32) -> Element<'static, Message> {
    let room = theme::active();
    let (c1, c2) = vm::gradient_colors(album_id);
    let to_color = |c: [u8; 3]| {
        let ink = room.placeholder_ink(Color::from_rgb8(c[0], c[1], c[2]));
        theme::Palette::mix(room.wall, ink, shown.clamp(0.0, 1.0))
    };
    let gradient = iced::gradient::Linear::new(iced::Radians(2.4))
        .add_stop(0.0, to_color(c1))
        .add_stop(1.0, to_color(c2));
    container(Space::new(Length::Fixed(size), Length::Fixed(size)))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Gradient(gradient.into())),
            ..container::Style::default()
        })
        .into()
}

/// **A playlist's sleeve** (ADR-0024 §A1): a collage of quotations from the
/// records it holds, at `edge` px — the panel's rows draw it at
/// [`theme::PANEL_SLEEVE`], the playlist's page at [`theme::ART_MAX`].
///
/// The rule, at every size: four or more distinct records → a 2 × 2 collage
/// of the first four, in playlist order; one to three → the first record's
/// sleeve, full-bleed; none the library resolves → the rest tile (the
/// surface step, the name in ink — an empty made thing is quiet, not
/// decorated). Cells come from the wall's own thumbnail cache and degrade to
/// the wall's own deterministic gradient while a decode is in flight, so a
/// playlist's sleeve can never disagree with the tiles of the records it
/// quotes.
///
/// This *constructs* a playlist's sleeve out of whole, unmarked artwork at
/// thumbnail scale; nothing is drawn on top of any record's sleeve, and no
/// cell exceeds the decoded source (§A1 argues both against the refusals by
/// name). Shared by the panel and the page for [`gradient_block`]'s reason:
/// two renderings of one identity that could drift apart would be a bug.
pub(crate) fn playlist_sleeve(
    shelf: &Shelf,
    art: &[u64],
    name: &str,
    edge: f32,
) -> Element<'static, Message> {
    let room = theme::active();
    match art {
        [] => {
            // The rest tile: the name whole at page scale, its initial at
            // panel scale — a 40 px tile has no room for words and needs
            // only to be tellable apart.
            let large = edge >= theme::ART_MIN;
            let label: String = if large {
                name.to_owned()
            } else {
                name.chars()
                    .next()
                    .map(|initial| initial.to_uppercase().to_string())
                    .unwrap_or_default()
            };
            let word = if large {
                text(label)
                    .size(theme::SIZE_TITLE)
                    .line_height(theme::LEADING_TITLE)
                    .font(theme::SEMIBOLD)
            } else {
                text(label)
                    .size(theme::SIZE_EMPHASIS)
                    .line_height(theme::LEADING_EMPHASIS)
                    .font(theme::MEDIUM)
            };
            container(word)
                .width(Length::Fixed(edge))
                .height(Length::Fixed(edge))
                .padding(if large { theme::GAP_MD } else { 0.0 })
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center)
                .clip(true)
                .style(move |_theme| theme::playlist_rest_tile(room))
                .into()
        }
        [a, b, c, d, ..] => {
            let half = edge / 2.0;
            column![
                row![sleeve_cell(shelf, *a, half), sleeve_cell(shelf, *b, half)],
                row![sleeve_cell(shelf, *c, half), sleeve_cell(shelf, *d, half)],
            ]
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .into()
        }
        // Below four distinct records the first one's face is the sleeve —
        // one rule at every size, and the tiling question never opens.
        [first, ..] => sleeve_cell(shelf, *first, edge),
    }
}

/// One quotation in a playlist's sleeve: the record's thumbnail from the
/// wall's cache, or — while its decode is in flight, or where no art can be
/// decoded — the same deterministic gradient the record's own tile shows.
fn sleeve_cell(shelf: &Shelf, album: u64, size: f32) -> Element<'static, Message> {
    match shelf.thumbs.peek(&album) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .into(),
        None => gradient_block(album, size, 1.0),
    }
}

/// **The strip every place that is not the Library wears**: the way back, the
/// place's name, and one quiet line saying what the place is or how to leave
/// it.
///
/// It occupies the Library's top-bar geometry exactly — the same vertical
/// padding, the same [`theme::HANG`] window gutter (law L1), the same hairline
/// underneath — so that moving between places does not slide the content area
/// by a pixel. **The frame is the frame in every place**, and with four places
/// wearing it — Album, Queue, Playlist, and since doc 10 §7 step 8 Settings —
/// it is one function in five places (the Library's own strip being
/// [`top_bar`]) rather than copies that can drift.
///
/// **The header carries no way back, and that is not a missing affordance.**
/// It held a `‹ Library` door and an `Esc returns to Library` hint until the
/// returns lane shipped; the lane is resident in every place and both of its
/// states, so `Library` is permanently one press away, up and to the left, and
/// a second door in every header was the same statement made twice. The
/// keyboard is untouched — <kbd>Esc</kbd> still peels and still lands on the
/// Library — and the visible-control rule holds through the lane's own row.
/// **Do not restore a back door here**: its absence is the lane's presence.
pub(crate) fn place_header(name: &'static str) -> Element<'static, Message> {
    place_header_with(name, None)
}

/// [`place_header`], with one quiet statement at the strip's right edge.
///
/// `note` is for a statement about the *place*, never a keyboard hint — the
/// Settings place's *"Kept in config.toml…"* is the only one today. The strip
/// stays one function so the geometry cannot drift between the place that
/// carries a note and the ones that do not.
///
/// It used to carry a third parameter, an extra tenant after the place's name,
/// and the Album place's `‹ Prev` / `Next ›` pair was its only customer. The
/// owner removed the pair — *"previous and next on albums doesn't make sense
/// on the album view"* — and the slot went with it rather than being left open
/// for the next thing that fancies the strip.
pub(crate) fn place_header_with(
    name: &'static str,
    note: Option<&'static str>,
) -> Element<'static, Message> {
    let room = theme::active();
    // The place's name leads the strip. It stands where the way-back used to,
    // so the frame's left edge is unchanged (law L1) and moving between places
    // still slides nothing.
    let mut strip = row![
        text(name)
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .font(theme::MEDIUM),
    ]
    .spacing(theme::GAP_LG)
    .align_y(iced::Alignment::Center);
    strip = strip.push(Space::with_width(Length::Fill));
    if let Some(note) = note {
        strip = strip.push(
            text(note)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        );
    }
    column![
        container(strip).padding(theme::pad(theme::TOP_BAR_PAD_V, theme::HANG)),
        horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
    ]
    .into()
}

/// **The one gutter a place's body hangs from** (law L1): [`theme::HANG`] on
/// every edge, with the scrollbar's declared lane added to the right.
///
/// A place fills the window, so its content hangs from the same two lines the
/// wall and both bars do — `x = HANG` and `x = W − HANG` — and from `y = HANG`,
/// which is the free top a place has and a panel never did. `GAP_XL` is padding
/// *inside* a panel and was never a window margin; spending it as one is how
/// baz ended up with three of them.
///
/// The right edge carries [`theme::SCROLLBAR_LANE`] as well, and that is the
/// one inset the law permits there: it is *declared* rather than absorbed, so a
/// page long enough to scroll does not put its bar over the last character of
/// every duration.
pub(crate) fn place_pad() -> iced::Padding {
    iced::Padding {
        top: theme::HANG,
        right: theme::HANG + theme::SCROLLBAR_LANE,
        bottom: theme::HANG,
        left: theme::HANG,
    }
}

/// A block's name inside a place: a hairline, then the word in the room's
/// quietest voice.
///
/// The one structural rule beyond the three `.interface-design/system.md` §2
/// names, and it earns its place the way the Settings readout's does: it
/// divides two kinds of content inside one column. Shared by the record page's
/// `Tracks` and `Details` because a page whose two blocks named themselves
/// differently would read as two surfaces.
pub(crate) fn section_rule(name: &'static str) -> Element<'static, Message> {
    let room = theme::active();
    column![
        horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
        text(theme::tracked(&name.to_uppercase()))
            .size(theme::SIZE_HEADING)
            .line_height(theme::LEADING_HEADING)
            .font(theme::MEDIUM)
            .color(room.paper_faint),
    ]
    .spacing(theme::GAP_SM)
    .into()
}

/// [`section_rule`], with one quiet fact at the rule's right edge — the
/// Songs section teaching its own accelerator (*"Enter plays the first
/// match."*, doc 11 §5 P6.4): the era printed the shortcut beside the verb
/// it accelerates, and without menus that duty falls to the surface the
/// verb lives on. The note takes the readout ink, never a control's — it is
/// a fact about the rows below, not a thing to press.
pub(crate) fn section_rule_noted(
    name: &'static str,
    note: &'static str,
) -> Element<'static, Message> {
    let room = theme::active();
    column![
        horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
        row![
            text(theme::tracked(&name.to_uppercase()))
                .size(theme::SIZE_HEADING)
                .line_height(theme::LEADING_HEADING)
                .font(theme::MEDIUM)
                .color(room.paper_faint),
            Space::with_width(Length::Fill),
            text(note)
                .size(theme::SIZE_CAPTION)
                .line_height(theme::LEADING_CAPTION)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        ]
        .align_y(iced::Alignment::Center),
    ]
    .spacing(theme::GAP_SM)
    .into()
}

#[cfg(test)]
mod tests {
    /// Every string literal in the view sources' *code* lines — comments
    /// stripped — which is a conservative superset of what can ship on
    /// screen.
    fn shipped_strings() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("views"))
            .expect("the views directory")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        // The context menu's labels ship too, and they are built in `menu.rs`.
        files.push(root.join("menu.rs"));
        let mut found = Vec::new();
        for path in files {
            let name = path
                .file_name()
                .expect("a file name")
                .to_string_lossy()
                .into_owned();
            let source = std::fs::read_to_string(&path)
                .expect("a view source")
                .replace("\r\n", "\n");
            // Only what ships: test modules (this one included) may name the
            // room's words in order to ban them.
            let source = source
                .split("#[cfg(test)]")
                .next()
                .expect("a source has a head");
            for line in source.lines() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                // Walk the line's string literals, escapes respected.
                let bytes = line.as_bytes();
                let mut i = 0;
                while i < bytes.len() {
                    if bytes[i] == b'"' {
                        let mut j = i + 1;
                        let mut literal = String::new();
                        while j < bytes.len() && bytes[j] != b'"' {
                            if bytes[j] == b'\\' {
                                j += 1;
                            }
                            if j < bytes.len() {
                                literal.push(bytes[j] as char);
                            }
                            j += 1;
                        }
                        found.push((name.clone(), literal));
                        i = j + 1;
                    } else {
                        i += 1;
                    }
                }
            }
        }
        assert!(
            found.iter().any(|(_, s)| s.contains("Play album")),
            "the sweep must actually be seeing the shipped copy"
        );
        found
    }

    /// **One vocabulary** (doc 11 §5 P4): no word from the room-vocabulary
    /// list ships in user-facing copy. "The wall", "the hang", "the stack",
    /// "marquee" and the pull's internals are the corpus's own names for
    /// its ideas — correctly internal, like a stage crew's slang — and the
    /// one leak the critique found (*"Esc returns to the wall"* beside
    /// `‹ Library`, two names for one destination in one strip) is exactly
    /// what this pin keeps closed. Licensed uses stay licensed: `Pull` the
    /// control and its offer line "The pull" (P9, the owner's call), and
    /// `Save as playlist` / `Add to playlist…` are ordinary words.
    #[test]
    fn no_room_vocabulary_ships_in_user_facing_copy() {
        let licensed = ["The pull", "Pull"];
        for (file, literal) in shipped_strings() {
            if licensed.contains(&literal.as_str()) {
                continue;
            }
            let lowered = literal.to_lowercase();
            for banned in ["wall", "hang", "marquee", "pull's", "the stack"] {
                // Word boundaries: "wall" must not hide in "wallpaper" and
                // fail the sweep for the wrong reason — every hit is read
                // as its own word.
                let hit = lowered
                    .split(|c: char| !c.is_alphanumeric() && c != '\u{2019}' && c != '\'')
                    .any(|word| word == banned)
                    || (banned.contains(' ') && lowered.contains(banned));
                assert!(
                    !hit,
                    "{file}: the literal {literal:?} ships the room's own \
                     word {banned:?} — plain words wherever the software \
                     speaks (doc 11 §5 P4; `02` §2.7)"
                );
            }
        }
    }

    /// **Every row-shaped control names the ground it stands on.**
    ///
    /// [`theme::track_row`]'s hover used to be the constant
    /// `Palette::plinth`, which is right for a row on the wall and mute for a
    /// row on the panel — whose own ground *is* `plinth`, so its rows painted
    /// the colour already under them. The owner named it (2026-08-09, *"a bit…
    /// unresponsive"*); the fix was to make the hover a *relation*
    /// (`Palette::step_up`), which only works if every call site says what it
    /// stands on.
    ///
    /// Asserted over the source because the failure is invisible in a
    /// rendering and silent in a type: a ground of the wrong plane compiles,
    /// draws, and answers the pointer with nothing. A future surface composed
    /// on a new plane fails the build rather than the review.
    #[test]
    fn every_row_shaped_control_names_the_ground_it_stands_on() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut sites = 0_u32;
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("views"))
            .expect("the views directory")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        files.sort();
        for path in files {
            let source = std::fs::read_to_string(&path)
                .expect("a view source")
                .replace("\r\n", "\n");
            let source = source
                .split("#[cfg(test)]")
                .next()
                .expect("a source has a head");
            let name = path.file_name().expect("a file name").to_string_lossy();
            for (at, _) in source.match_indices("theme::track_row(") {
                // Comments name the function too; only calls are call sites.
                let line_start = source[..at].rfind('\n').map_or(0, |index| index + 1);
                if source[line_start..at].trim_start().starts_with("//") {
                    continue;
                }
                let tail: String = source[at..].chars().take(80).collect();
                let arguments = tail
                    .split_once('(')
                    .map(|(_, rest)| rest.replace(['\n', ' '], ""))
                    .unwrap_or_default();
                assert!(
                    arguments.starts_with("room,room."),
                    "{name}: a row must name the surface it stands on — \
                     `theme::track_row(room, <ground>, …)` — and this one \
                     reads `{}`",
                    arguments.chars().take(40).collect::<String>()
                );
                sites += 1;
            }
        }
        // Not vacuous: the wall's rows, the panel's, the menu card's and the
        // returns lane's are all in the walk.
        assert!(sites >= 6, "only {sites} row call sites found");
    }

    /// The Shuffle tooltip's figure is [`crate::shuffle::SLEEVES`], not a
    /// number that can drift from it: the tooltip teaches the bounded draw
    /// (doc 11 §5 P6.2), and a bound taught wrong would be worse than one
    /// untaught.
    #[test]
    fn the_shuffle_tooltip_states_the_real_bound() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/top_bar.rs"),
        )
        .expect("the top bar's source")
        .replace("\r\n", "\n");
        let taught = format!(
            "Play {} records drawn from what the Library shows",
            crate::shuffle::SLEEVES
        );
        assert!(
            source.contains(&taught),
            "the Shuffle tooltip must state the draw's real bound: {taught:?}"
        );
    }
}
