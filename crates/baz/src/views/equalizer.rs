//! **The equaliser panel** — a curve you can see and grab, from anywhere.
//!
//! The owner, 2026-08-18: *"this is not something that should be buried in the
//! settings. It should be accessible potentially from anywhere maybe on the top
//! bar… sliders help show graphically how high the current of frequencies is
//! biased."*
//!
//! He is right on both counts and the first shipped version was wrong on both.
//! It lived in Settings → Playback, four scrolls down, and it drew ten stepper
//! rows — which give you each band's number and never give you the **shape**.
//! The shape is the only thing a graphic equaliser is for; a listener does not
//! think *+3 at 250 Hz*, they think *more warmth*, and that thought is a
//! picture.
//!
//! # A door in the app bar, and a panel under it
//!
//! The mark is [`crate::icon::Glyph::Equalizer`] — three faders that disagree,
//! which is what the panel behind it contains, so the door needs no word. It
//! stands beside the bell and the gear, resident in every place, because *from
//! anywhere* was the requirement and a control you have to navigate to is not
//! from anywhere.
//!
//! The panel floats at the pointer like the context menu and the status log,
//! and is stacked always-present for the reason every floating layer here is
//! (`app.rs`: iced diffs the tree by position, and a level that comes and goes
//! resets every widget beneath it).
//!
//! # What the picture is made of
//!
//! Eleven faders — ten bands and the pre-amp — over one zero line, with the
//! band's own frequency under it and its gain above. Three readings of the same
//! fact, deliberately: the **handle's height** is the curve at a glance, the
//! **fill** repeats it as a length so nothing rests on telling two colours
//! apart, and the **number** is there when a listener wants to match a band to
//! one they set yesterday.
//!
//! The pre-amp stands apart, after a rule, because it is not a band: it is the
//! headroom the bands are given back, and a listener reading the curve should
//! not read it as an eleventh frequency.

use iced::widget::{Space, button, checkbox, column, container, row, rule, text};
use iced::{Element, Length, alignment};

use crate::app::{EqualizerSettings, Message};
use crate::fader::Fader;
use crate::theme;

/// How tall a fader stands.
///
/// 168 is fourteen [`theme::GAP_MD`]s and gives 24 dB of travel seven pixels a
/// decibel — fine enough to place a band by eye, and short enough that eleven
/// of them plus their labels fit a panel that does not fill the window.
const FADER_H: f32 = 168.0;

/// The panel's own measure: eleven faders on their gaps, plus the padding.
const PANEL_W: f32 = 11.0 * crate::fader::HIT_W + 10.0 * theme::GAP_XS + 4.0 * theme::GAP_LG;

/// Draw the panel.
pub(crate) fn layer(eq: EqualizerSettings, window: iced::Size) -> Element<'static, Message> {
    let room = theme::active();
    let limit = baz_core::equalizer::LIMIT_DB;

    let mut bands = row![].spacing(theme::GAP_XS).align_y(iced::Alignment::End);
    for (index, centre) in baz_core::equalizer::CENTRES.into_iter().enumerate() {
        let db = f32::from(eq.bands_centidb[index]) / 100.0;
        bands = bands.push(strip(&label_of(centre), db, limit, room, move |db| {
            Message::EqualizerBandSet(index, centidb(db))
        }));
    }

    let preamp = f32::from(eq.preamp_centidb) / 100.0;
    let body = column![
        // **The switch first, and it says what off means.** Turning this on is
        // a decision about the signal path rather than a preference — see
        // `baz_core::equalizer` for why off is *not in the path* rather than
        // a flat filter — so the sentence is next to the switch and not in a
        // manual.
        row![
            checkbox(eq.enabled)
                .label("Shape the sound")
                .size(theme::STEPPER_HIT)
                .text_size(theme::SIZE_META)
                .text_line_height(theme::LEADING_META)
                .spacing(theme::GAP_SM)
                .style(move |_theme, status| theme::check(room, status))
                .on_toggle(Message::EqualizerEnabled),
            Space::new().width(Length::Fill),
            word("Flat", Message::EqualizerFlat),
            word("Suggest a pre-amp", Message::EqualizerSuggestPreamp),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
        // **The switch's own consequence, in the present tense.** Which state
        // you are in is the fact worth stating, and it is worth stating in
        // full: *off* here is not a flat filter, it is baz not being in the
        // path at all, and that is the promise the whole feature is measured
        // against.
        text(if eq.enabled {
            "Shaping the sound. Turn this off and baz plays the file untouched \
             — the same bytes, not a flat filter."
        } else {
            "Off. baz is playing the file untouched — the same bytes, not a \
             flat filter."
        })
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.paper_faint),
        row![
            // **The curve, and the faders standing on it.** The picture is
            // behind the controls rather than beside them because they are
            // the same fact: the handle is where you ask, the curve is what
            // you get, and a listener comparing two pictures across the panel
            // is doing arithmetic the panel should have done.
            iced::widget::stack![
                container(crate::response::Response::new(
                    baz_core::equalizer::Bands::from_centidb(eq.bands_centidb),
                    limit,
                    FADER_H,
                    room,
                ))
                // Down by exactly the gain label above it, so the curve's
                // own bounds are the faders' bounds and the two mappings are
                // fed the same rectangle.
                .padding(iced::Padding {
                    top: theme::HEADING_LINE_H + theme::GAP_XS,
                    ..iced::Padding::default()
                }),
                bands,
            ],
            // The pre-amp is not a band, and the rule says so.
            container(rule::vertical(1).style(move |_theme| theme::hairline(room, room.plinth)))
                .height(Length::Fixed(FADER_H))
                .padding(theme::pad(0.0, theme::GAP_SM)),
            strip("Pre-amp", preamp, limit, room, |db| {
                Message::EqualizerPreampSet(centidb(db))
            }),
        ]
        .spacing(theme::GAP_XS)
        .align_y(iced::Alignment::End),
    ]
    .spacing(theme::GAP_MD);

    let card = container(container(body).padding(theme::GAP_LG))
        .width(Length::Fixed(PANEL_W.min(window.width - 2.0 * theme::HANG)))
        .style(move |_theme| theme::menu(room));

    // Pressing outside puts it away, the manner every floating layer here has.
    iced::widget::stack![
        iced::widget::mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
        )
        .on_press(Message::ToggleEqualizer),
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Top)
            .padding(theme::pad(theme::APP_BAR_H + theme::GAP_SM, theme::GAP_LG)),
    ]
    .into()
}

/// One column: the gain, the fader, the name.
///
/// The gain sits **above** the fader rather than below it because the numbers
/// are read while the hand is on the handle, and a hand covers what is under
/// it.
fn strip(
    name: &str,
    db: f32,
    limit: f32,
    room: &'static theme::Palette,
    on_change: impl Fn(f32) -> Message + 'static,
) -> Element<'static, Message> {
    column![
        container(
            text(format!("{db:+.0}"))
                .size(theme::SIZE_HEADING)
                .line_height(theme::LEADING_HEADING)
                .font(theme::MEDIUM)
                .color(if db == 0.0 {
                    room.paper_muted
                } else {
                    room.paper
                })
        )
        .width(Length::Fixed(crate::fader::HIT_W))
        .align_x(alignment::Horizontal::Center),
        Fader::new(db, limit, FADER_H, room, on_change).on_release(Message::EqualizerCommitted),
        container(
            text(name.to_owned())
                .size(theme::SIZE_HEADING)
                .line_height(theme::LEADING_HEADING)
                .color(room.paper_faint)
                .wrapping(iced::widget::text::Wrapping::None)
        )
        .width(Length::Fixed(crate::fader::HIT_W))
        .align_x(alignment::Horizontal::Center),
    ]
    .spacing(theme::GAP_XS)
    .align_x(alignment::Horizontal::Center)
    .into()
}

/// A quiet word control, the same one Settings spends on its acts.
fn word(label: &'static str, message: Message) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::transport(room, room.plinth, status))
    .on_press(message)
    .into()
}

/// A band's own name: hertz under a thousand, kilohertz above, and short — the
/// column is one fader wide.
fn label_of(centre: f32) -> String {
    // 31.5 Hz rounds to 32 rather than reading `31.5` — the label is a name
    // for the band, not its coefficient, and half a hertz at the bottom of the
    // range is a distinction nobody is making by eye.
    if centre >= 1000.0 {
        format!("{:.0}k", centre / 1000.0)
    } else {
        format!("{centre:.0}")
    }
}

/// Decibels as the centidecibels the protocol and the config carry.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a fader is clamped to ±12 dB, so ±1200 fits i16 many times over"
)]
fn centidb(db: f32) -> i16 {
    (db * 100.0).round() as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The panel is as wide as what it holds**, and no wider — eleven faders
    /// on their gaps plus the card's own padding. Stated as arithmetic so a
    /// twelfth band, or a change to the hit band, moves the panel with it
    /// instead of clipping a fader.
    #[test]
    fn the_panel_is_the_width_of_its_faders() {
        let faders = 11.0 * crate::fader::HIT_W;
        let gaps = 10.0 * theme::GAP_XS;
        assert!(
            PANEL_W > faders + gaps,
            "the panel is narrower than the faders it holds"
        );
        assert!(
            PANEL_W < faders + gaps + 8.0 * theme::GAP_LG,
            "the panel has grown padding it does not draw"
        );
    }

    /// **Every band gets a label that fits a fader's width.** A frequency
    /// spelled `16000 Hz` under a 32 px column is a label that wraps or clips,
    /// and either one turns the row of names into noise.
    #[test]
    fn a_band_label_is_short_enough_to_stand_under_its_fader() {
        for centre in baz_core::equalizer::CENTRES {
            let label = label_of(centre);
            assert!(
                label.chars().count() <= 5,
                "{centre} Hz reads as {label:?}, which is too wide for the column"
            );
        }
        assert_eq!(label_of(31.5), "32");
        assert_eq!(label_of(1000.0), "1k");
        assert_eq!(label_of(16000.0), "16k");
    }

    /// Decibels round-trip through the protocol's own units.
    #[test]
    fn a_fader_position_becomes_the_units_the_engine_takes() {
        assert_eq!(centidb(0.0), 0);
        assert_eq!(centidb(-12.0), -1200);
        assert_eq!(centidb(12.0), 1200);
        assert_eq!(centidb(3.5), 350);
    }
}
