//! App-bar health bell and its bounded event-history float.

use iced::widget::{
    Space, button, column, container, image as iced_image, mouse_area, row, rule, scrollable,
    stack, text, tooltip,
};
use iced::{Color, Element, Length, Size, alignment};

use crate::app::Message;
use crate::health::{Event, Level, Log, Summary};
use crate::{icon, theme};

const DOT: f32 = 7.0;
const PANEL_W: f32 = 440.0;
const PANEL_H: f32 = 420.0;

pub(crate) fn bell(summary: Summary) -> Element<'static, Message> {
    let room = theme::active();
    let tone = tone(room, summary.level);
    let mark = container(iced::widget::stack![
        iced_image(icon::handle(icon::Glyph::Bell))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::glyph_ink(true, false, 0.0, false)),
        // **The dot is painted on a [`DOT`]-sized box, and *that* box is
        // aligned** — two containers, not one, and the difference is the whole
        // of the bell.
        //
        // It was one: a `Space` of `DOT` inside a container that carried both
        // `theme::status_dot` **and** `align_right(Length::Fill)` /
        // `align_bottom(Length::Fill)`. Those two calls set the container's
        // width and height to `Fill`, and a container paints its **own**
        // bounds — so the style landed on a 20 × 20 box with a 999 px corner
        // radius, which is a disc exactly the size of the glyph box, drawn
        // over the top of the glyph. The health indicator has been a plain
        // coloured circle in the app bar for as long as it has existed, and
        // the bell beneath it was never visible at any tone.
        //
        // That also hid a second defect: the glyph the disc was covering was
        // not the bell either (`Glyph::ALL` and `Glyph::index` disagreed), and
        // when the ordering was fixed on 2026-08-14 the real `BELL` outlines
        // turned out to draw a blob of their own. Three faults stacked in one
        // 20 px square, each of which made the next invisible.
        container(
            container(
                Space::new()
                    .width(Length::Fixed(DOT))
                    .height(Length::Fixed(DOT))
            )
            .style(move |_theme| theme::status_dot(tone))
        )
        .align_right(Length::Fill)
        .align_bottom(Length::Fill),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    tooltip(
        button(mark)
            .width(Length::Fixed(theme::TRANSPORT_HIT))
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.recess, status))
            .on_press(Message::ToggleStatus),
        text(format!("{} — open health", summary.label))
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

pub(crate) fn layer(log: &Log, summary: Summary, window: Size) -> Element<'static, Message> {
    let room = theme::active();
    let width = PANEL_W.min((window.width - 2.0 * theme::HANG).max(0.0));
    let height = PANEL_H.min(
        (window.height - theme::APP_BAR_H - theme::BAR_CONTENT_H - 3.0 * theme::GAP_MD)
            .max(theme::TRANSPORT_HIT),
    );
    let mut events = column![];
    for (index, event) in log.newest().enumerate() {
        if index > 0 {
            events = events
                .push(rule::horizontal(1).style(move |_theme| theme::hairline(room, room.plinth)));
        }
        events = events.push(event_row(event));
    }
    let tone = tone(room, summary.level);
    let retry = (matches!(summary.level, Level::Warning | Level::Error)).then(|| {
        button(
            text("Retry")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META),
        )
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(theme::pad(0.0, theme::GAP_MD))
        .style(move |_theme, status| theme::word_button(room, room.plinth, status))
        .on_press(Message::RetryHealth)
    });
    let mut head = row![
        column![
            text("STATUS")
                .size(theme::SIZE_CAPTION)
                .line_height(theme::LEADING_CAPTION)
                .font(theme::MEDIUM)
                .color(room.paper_faint),
            row![
                container(
                    Space::new()
                        .width(Length::Fixed(DOT))
                        .height(Length::Fixed(DOT))
                )
                .style(move |_theme| theme::status_dot(tone)),
                text(summary.label)
                    .size(theme::SIZE_HEADING)
                    .line_height(theme::LEADING_HEADING)
                    .font(theme::MEDIUM),
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(theme::GAP_XS),
        Space::new().width(Length::Fill).height(Length::Fixed(0.0)),
        button(
            text("Close")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META),
        )
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(theme::pad(0.0, theme::GAP_MD))
        .style(move |_theme, status| theme::word_button(room, room.plinth, status))
        .on_press(Message::CloseStatus),
    ]
    .align_y(iced::Alignment::Center);
    if let Some(retry) = retry {
        head = head.push(retry);
    }
    let history = scrollable(
        container(events)
            .width(Length::Fill)
            .padding(iced::Padding {
                top: theme::GAP_XS,
                right: theme::GAP_LG,
                bottom: theme::GAP_MD,
                left: theme::GAP_LG,
            }),
    )
    .height(Length::Fill)
    .direction(scrollable::Direction::Vertical(theme::wall_scrollbar()));
    let card = container(column![
        container(head).width(Length::Fill).padding(theme::GAP_LG),
        rule::horizontal(1).style(move |_theme| theme::hairline(room, room.plinth)),
        history,
    ])
    .width(Length::Fixed(width))
    .height(Length::Fixed(height))
    .style(move |_theme| theme::menu(room));

    stack![
        mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
            .on_press(Message::CloseStatus),
        container(iced::widget::opaque(card))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Top)
            .padding(iced::Padding {
                top: theme::APP_BAR_H + theme::GAP_MD,
                right: theme::HANG,
                bottom: 0.0,
                left: 0.0,
            }),
    ]
    .into()
}

fn event_row(event: &Event) -> Element<'static, Message> {
    let room = theme::active();
    let tone = tone(room, event.level);
    let detail: Element<'static, Message> = if event.detail.is_empty() {
        Space::new().height(Length::Fixed(0.0)).into()
    } else {
        text(event.detail.clone())
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_dim)
            .wrapping(text::Wrapping::Word)
            .into()
    };
    container(
        column![
            row![
                row![
                    container(
                        Space::new()
                            .width(Length::Fixed(DOT))
                            .height(Length::Fixed(DOT))
                    )
                    .style(move |_theme| theme::status_dot(tone)),
                    text(level_label(event.level))
                        .size(theme::SIZE_CAPTION)
                        .line_height(theme::LEADING_CAPTION)
                        .font(theme::MEDIUM)
                        .color(room.paper_faint),
                ]
                .spacing(theme::GAP_SM)
                .align_y(iced::Alignment::Center),
                Space::new().width(Length::Fill).height(Length::Fixed(0.0)),
                text(event.age())
                    .size(theme::SIZE_CAPTION)
                    .line_height(theme::LEADING_CAPTION)
                    .color(room.paper_faint),
            ],
            text(event.title.clone())
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM),
            detail,
        ]
        .spacing(theme::GAP_XS)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(theme::pad(theme::GAP_MD, 0.0))
    .into()
}

fn level_label(level: Level) -> &'static str {
    match level {
        Level::Ready => "READY",
        Level::Working => "WORKING",
        Level::Warning => "WARNING",
        Level::Error => "ERROR",
    }
}

fn tone(room: &theme::Palette, level: Level) -> Color {
    match level {
        Level::Ready => room.paper_muted,
        Level::Working => room.paper_faint,
        Level::Warning => room.warning,
        Level::Error => room.alert,
    }
}

#[cfg(test)]
mod tests {
    use super::DOT;
    use crate::theme;

    /// **The health dot is painted on a dot-sized box.**
    ///
    /// A `container` paints its **own** bounds, and `align_right(Length::Fill)`
    /// / `align_bottom(Length::Fill)` set those bounds to `Fill`. Carrying
    /// `theme::status_dot` and those two calls on one container therefore
    /// painted a 999 px-radius background across the whole glyph box — a disc
    /// exactly the size of the bell, drawn over the bell. The app bar's health
    /// indicator was that disc for as long as it existed.
    ///
    /// The fix is two containers, and this pins the split: the styled one must
    /// carry no `Fill`, and the aligning one must carry no style. It is a
    /// source scan because both halves build iced widgets and there is nothing
    /// else to interrogate — but *which container the style is on* is a fact
    /// about where the code is, which is the one kind of fact this form is
    /// good for.
    #[test]
    fn the_health_dot_is_painted_on_a_dot_sized_box() {
        let source = include_str!("status.rs").replace("\r\n", "\n");
        let bell = {
            let rest = source
                .split_once("pub(crate) fn bell(")
                .expect("the bell exists")
                .1;
            &rest[..rest.find("\n}\n").expect("a function ends")].to_owned()
        };
        let styled = bell
            .find(".style(move |_theme| theme::status_dot(tone))")
            .expect("the dot is styled");
        let aligned = bell
            .find(".align_right(Length::Fill)")
            .expect("the dot is aligned into the corner");
        assert!(
            styled < aligned,
            "the dot's paint and its Fill alignment are on one container again, \
             so the tone is drawn across the whole glyph box and the bell is \
             underneath it"
        );
        let between = &bell[styled..aligned];
        assert!(
            between.contains(')'),
            "the styled container is no longer closed before the aligning one \
             begins"
        );
        // And the box it paints is the dot's own size, not the glyph's.
        assert!(
            bell.contains("Length::Fixed(DOT)"),
            "the dot no longer declares its own size"
        );
        const { assert!(DOT < theme::ICON_PX) }
    }
}
