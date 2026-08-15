//! **Composing a playlist**: the place design 21 draws, built on plan 22's
//! engine.
//!
//! # It was a form, and it should be a place
//!
//! The owner, on the page this replaces: *"the ui layout for the vibe playlist
//! isn't great… we have to scroll to see the playlist. it should work on both
//! wide and narrow layouts."* Everything below follows from taking that
//! literally.
//!
//! **Two panes** (§8). The ask on the left at [`theme::COMPOSE_ASK_W`], the
//! result on the right, so the list is on screen the whole time you are tuning
//! and the curve sits over it — a line and its result as one picture rather
//! than two screens apart. Below [`theme::COMPOSE_BREAKPOINT`] the same three
//! blocks stack, ask–curve–list, with the commitment pinned to the foot of the
//! ask so it is always in reach. Nothing is hidden behind a tab at any width:
//! somebody who learns this page on a laptop should not have to learn it again
//! on a desktop.
//!
//! **Two questions, in the listener's own words** (§2). *What do you want to
//! hear?* and *how should it move?* — and nothing on the surface says vibe,
//! contour, dimension or recipe. There is **one request**, and it is a
//! sentence about sound; a mood is not a second input beside it but a shortcut
//! that writes into the only input there is. That is the model defect the
//! owner found on the drawings, and the reason [`ask`] is one band with the
//! field at its head.
//!
//! **Four readouts** (§6), each a rendering of an engine fact rather than a
//! second opinion computed here: the live match count and its closest three
//! ([`ask`]), the eligible cloud behind the line ([`shape`]), the match ticks
//! per row and the new/kept diff ([`result`]). If a readout ever needs
//! arithmetic the engine did not supply, that is an engine gap and not a view
//! workaround.
//!
//! **One deliberate refusal.** The list does not update while the line is
//! dragged. It is affordable — retrieval over an analysed library is
//! sub-second — and it is still wrong: a result that changes under your hand
//! cannot be read, and you would be tuning against a moving target. Everything
//! *about* the answer updates live; the answer waits to be asked for.

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Element, Length, Size};

use crate::app::{Message, Shelf};
use crate::playlists::Playlists;
use crate::{theme, views};

pub(crate) mod ask;
pub(crate) mod result;
pub(crate) mod shape;

/// **Which of design 21 §7's nine states the page is in.**
///
/// Named rather than inferred at each call site, because the first two are
/// where a new listener spends their entire first session and the shipping
/// build designed neither. A page you cannot touch for two hours reads as
/// broken, so [`Stage::Cold`] and [`Stage::Listening`] both keep the ask pane
/// fully live and put the honest reading in the result pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    /// 1 · Never listened. Nothing analysed, nothing running.
    Cold,
    /// 2 · Listening. The scan is running; a partial list is already possible.
    Listening,
    /// 3 · Ready — or 4/6/7, which differ only by what is in the result pane.
    Ready,
}

impl Stage {
    fn of(state: &crate::vibe::State) -> Self {
        if state.preparing || state.analyzing {
            Self::Listening
        } else if state.has_features() {
            Self::Ready
        } else {
            Self::Cold
        }
    }
}

/// **How the page is laid out at this size** — one decision, made once, read
/// by every block.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout {
    /// Whether the two panes sit side by side.
    pub(crate) side_by_side: bool,
    /// Whether the curve is drawn, or has collapsed to its sentence and its
    /// presets.
    pub(crate) draw_curve: bool,
    /// The measure the result's rows take.
    pub(crate) measure: f32,
}

impl Layout {
    pub(crate) fn of(size: Size) -> Self {
        let side_by_side = size.width >= theme::COMPOSE_BREAKPOINT;
        Self {
            side_by_side,
            draw_curve: size.height >= theme::COMPOSE_SHORT_H,
            measure: if side_by_side {
                (size.width - theme::COMPOSE_ASK_W - theme::GAP_XL - 2.0 * theme::HANG)
                    .clamp(theme::COMPOSE_RESULT_MIN, theme::LIST_MEASURE)
            } else {
                // The row lane takes a maximum measure rather than the window
                // — design note 20 §1's product-wide rule, and this page is
                // one of its five customers.
                (size.width - 2.0 * theme::HANG).min(theme::LIST_MEASURE)
            },
        }
    }
}

/// The whole page.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    playlists: &'a Playlists,
    size: Size,
) -> Element<'a, Message> {
    let room = theme::active();
    let vibe = &shelf.vibe;
    let layout = Layout::of(size);
    let stage = Stage::of(vibe);

    if !playlists.available() {
        return views::hint("Playlist storage is unavailable on this system.");
    }
    if !cfg!(feature = "vibe-analysis") {
        return views::hint(
            "This is the light build. Install the full build to compose from your music.",
        );
    }

    let ask = ask::view(shelf, playlists, stage, layout);
    let answer = column![
        shape::view(vibe, layout),
        result::view(shelf, playlists, stage, layout),
    ]
    .spacing(theme::GAP_LG)
    .width(Length::Fill);

    let body: Element<'a, Message> = if layout.side_by_side {
        row![
            container(ask).width(Length::Fixed(theme::COMPOSE_ASK_W)),
            container(answer).width(Length::Fill),
        ]
        .spacing(theme::GAP_XL)
        .into()
    } else {
        // Narrow: the same three blocks, in the same order, with the
        // commitment pinned at the foot of the ask so it is always in reach
        // (the quorum's R11). Nothing moves to a tab and nothing is dropped.
        //
        // Bounded to the row lane's own measure rather than to the window: a
        // stacked page whose chips ran the full width of a wide-but-short
        // window would be a worse reading than the split it just lost.
        container(column![ask, answer].spacing(theme::GAP_LG))
            .max_width(theme::LIST_MEASURE)
            .into()
    };

    scrollable(container(body).padding(views::place_pad()))
        .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// A block's heading, in the voice the rest of the product uses for one.
pub(crate) fn heading(words: &str) -> Element<'static, Message> {
    let room = theme::active();
    iced::widget::text(words.to_owned())
        .size(theme::SIZE_EMPHASIS)
        .line_height(theme::LEADING_EMPHASIS)
        .font(theme::MEDIUM)
        .color(room.paper)
        .into()
}

/// A pressable word in the chip anatomy the whole page uses: the starting
/// points, the vocabulary and the shape presets are the same kind of thing —
/// *press this instead of doing it by hand* — so they are the same control.
pub(crate) fn chip(label: &str, lit: bool, message: Message) -> Element<'_, Message> {
    let room = theme::active();
    iced::widget::button(
        iced::widget::text(label)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            // **Weight and value, never hue.** A chip that is lit is the one
            // the request currently matches, and the difference has to be
            // readable by somebody who cannot separate two inks: it is set in
            // the medium face at full paper against the regular face dimmed,
            // which is a step in two non-colour dimensions at once.
            //
            // `theme::tile` used to draw this and cannot: it ignores its own
            // `selected` argument and returns the same transparent style
            // either way, so the state was carried entirely by the text
            // colour and the call read as though it were not.
            .font(if lit { theme::MEDIUM } else { theme::SANS })
            .color(if lit { room.paper } else { room.paper_dim }),
    )
    .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(message)
    .into()
}

/// Chips laid out in rows of at most `per_row`, because twelve words at
/// [`theme::COMPOSE_ASK_W`] do not fit on one line and a horizontal scroll for
/// six words would be absurd.
pub(crate) fn wrap_chips(
    mut chips: Vec<Element<'_, Message>>,
    per_row: usize,
) -> Element<'_, Message> {
    let per_row = per_row.max(1);
    let mut rows = column![].spacing(theme::GAP_XS);
    while !chips.is_empty() {
        let take = per_row.min(chips.len());
        let mut line = row![].spacing(theme::GAP_XS);
        for chip in chips.drain(..take) {
            line = line.push(chip);
        }
        rows = rows.push(line.push(Space::new().width(Length::Fill)));
    }
    rows.into()
}
