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

use iced::widget::{
    Space, button, checkbox, column, container, pick_list, row, rule, text, text_input,
};
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

/// The panel's own measure: **the wider of the two things it holds.**
///
/// It used to be the fader row alone, which was right while the row above it
/// held a switch and two words. Adding the preset picker and `Keep` made that
/// row the wider of the two, and the panel did not know — so `Make room` wrapped
/// onto a second line and the controls row grew a storey while the faders sat
/// in a panel with space to spare.
///
/// A panel measured by one of its two rows will do that again the next time
/// the other one gains a control, so it is measured by both.
const PANEL_W: f32 = if FADER_ROW_W > CONTROLS_ROW_W {
    FADER_ROW_W
} else {
    CONTROLS_ROW_W
};

/// Eleven faders on their gaps, plus the card's padding.
const FADER_ROW_W: f32 = 11.0 * crate::fader::HIT_W + 10.0 * theme::GAP_XS + 4.0 * theme::GAP_LG;

/// What the row of controls above them needs.
///
/// The switch and its label, the picker, and two words on their gaps. Stated
/// as a number because text has no width until it is laid out — and held to
/// the truth by `the_controls_row_fits_on_one_line`, which measures the
/// strings this panel actually ships rather than trusting the arithmetic.
const CONTROLS_ROW_W: f32 = 560.0;

/// Draw the panel.
pub(crate) fn layer(
    eq: EqualizerSettings,
    saved: &[crate::config::SavedCurve],
    naming: Option<&str>,
    window: iced::Size,
) -> Element<'static, Message> {
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
            presets(eq.bands_centidb, saved, room),
            keeping(eq.bands_centidb, saved),
            word("Make room", Message::EqualizerSuggestPreamp),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
        // **The line under the controls says what is true right now**, and
        // which sentence that is depends on what the panel is doing. Three
        // states, in the order a listener meets them.
        text(sentence(eq))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint),
        naming_row(naming, room),
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

/// **The offered curves**, and the listener's own if it is not one of them.
///
/// The owner: *"maybe we should add a few presets in a dropdown — that seems
/// common."* It is, and this panel had exactly one — a `Flat` button — because
/// an earlier note here argued that named curves are somebody else's taste.
/// See `baz_core::equalizer::PRESETS` for why that objection was right about
/// genres and wrong as a rule, and why the offered set is situations instead.
///
/// A curve dragged by hand shows as **Custom** rather than as whichever preset
/// it is nearest: the picker reports what the faders say, and there is no
/// hidden state for it to disagree with. Drag back onto a preset's exact
/// numbers and it is that preset again.
fn presets(
    bands_centidb: [i16; 10],
    saved: &[crate::config::SavedCurve],
    room: &'static theme::Palette,
) -> Element<'static, Message> {
    let mut choices: Vec<Choice> = (0..baz_core::equalizer::PRESETS.len())
        .map(Choice::Builtin)
        .collect();
    choices.extend(
        saved
            .iter()
            .enumerate()
            .map(|(at, curve)| Choice::Saved(at, curve.name.clone())),
    );
    // The listener's own curves are looked for **first**: a curve saved on top
    // of an offered one is the one they named, and naming it is the stronger
    // statement about what it is.
    let selected = saved
        .iter()
        .position(|curve| curve.bands_centidb == bands_centidb)
        .map(|at| Choice::Saved(at, saved[at].name.clone()))
        .or_else(|| {
            baz_core::equalizer::PRESETS
                .iter()
                .position(|preset| preset.bands_centidb == bands_centidb)
                .map(Choice::Builtin)
        });
    pick_list(choices, selected, Message::EqualizerPresetChosen)
        .placeholder("Custom")
        .width(Length::Fixed(PRESET_W))
        .padding(theme::pad(0.0, theme::GAP_MD))
        .text_size(theme::SIZE_META)
        .text_line_height(theme::LEADING_META)
        .style(move |_theme, status| theme::picker(room, status))
        .menu_style(move |_theme| theme::picker_menu(room))
        .into()
}

/// How wide the preset picker stands.
///
/// Wide enough for `Quiet listening` — the longest offered name — plus its
/// handle, and no wider: the picker is one of three things in the row and the
/// faders below are what the panel is for.
const PRESET_W: f32 = 150.0;

/// **One row of the picker** — an offered curve or one the listener saved.
///
/// Two variants rather than one flat index because the two lists change
/// independently: saving a curve must not renumber the offered ones, and
/// forgetting one must not silently select its neighbour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Choice {
    /// An index into `baz_core::equalizer::PRESETS`.
    Builtin(usize),
    /// An index into the listener's own saved curves.
    Saved(usize, String),
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin(at) => f.write_str(
                baz_core::equalizer::PRESETS
                    .get(*at)
                    .map_or("Custom", |preset| preset.name),
            ),
            Self::Saved(_, name) => f.write_str(name),
        }
    }
}

/// **One word slot whose meaning follows the curve.**
///
/// *Keep* when what is on the faders is not saved, *Forget* when it is one of
/// the listener's own, and **nothing at all** when it is one baz offers —
/// there is no sense in saving a copy of `More bass` under another name, and
/// less in offering to delete something that is not the listener's to delete.
///
/// Absent rather than disabled, which is this product's rule everywhere: an
/// inert control is a lie about what is available.
fn keeping(
    bands_centidb: [i16; 10],
    saved: &[crate::config::SavedCurve],
) -> Element<'static, Message> {
    if saved
        .iter()
        .any(|curve| curve.bands_centidb == bands_centidb)
    {
        return word("Forget", Message::EqualizerForget);
    }
    if baz_core::equalizer::Preset::matching(bands_centidb).is_some() {
        return Space::new().into();
    }
    word("Keep", Message::EqualizerSaveStart)
}

/// The name field's identity, so opening it can put the caret in it.
pub(crate) fn name_id() -> iced::widget::Id {
    iced::widget::Id::new("baz-equalizer-name")
}

/// **The name field**, when a curve is being kept.
///
/// It replaces nothing and pushes nothing aside: it is a row of its own under
/// the sentence, present only while a name is being typed. <kbd>Enter</kbd>
/// keeps it and <kbd>Esc</kbd> — the panel's own peel — puts the whole panel
/// away, so the field's cancel is a word rather than a key a listener has to
/// know.
fn naming_row(naming: Option<&str>, room: &'static theme::Palette) -> Element<'static, Message> {
    let Some(text_so_far) = naming else {
        return Space::new().into();
    };
    row![
        text_input("Name this curve", text_so_far)
            .id(name_id())
            .on_input(Message::EqualizerSaveName)
            .on_submit(Message::EqualizerSaveCommit)
            .width(Length::Fill)
            .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .style(move |_theme, status| theme::input(room, status)),
        word("Keep", Message::EqualizerSaveCommit),
        word("Cancel", Message::EqualizerSaveCancel),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .into()
}

/// **The headroom control is named for what it does** — see [`sentence`].
///
/// The owner: *"what does the suggest a pre-amp mean — a bit obscure?"* It
/// was, and the label was the whole problem. *Suggest a pre-amp* names the
/// **mechanism** (there is a pre-amp; here is a suggested value for it) and
/// says nothing about the **situation** (you are boosting; a boost has to come
/// from somewhere; this takes it from the whole signal so nothing clips).
///
/// `Make room` names the situation, and the line under the row says which room
/// and how much, with the numbers in it.
///
/// **What is true right now**, in one line under the controls.
///
/// Three states, and each one is a fact rather than an instruction:
///
/// 1. **Off** — and off is not a flat filter, it is baz not being in the path
///    at all. That is the promise the whole feature is measured against, so it
///    is stated in full every time.
/// 2. **On, boosting more than there is room for** — this is where the
///    headroom control stops being obscure, because the sentence names the
///    loudest boost and what pressing it would do.
/// 3. **On, with room** — the plain present tense.
fn sentence(eq: EqualizerSettings) -> String {
    if !eq.enabled {
        return "Off. baz is playing the file untouched — the same bytes, not a \
                flat filter."
            .to_owned();
    }
    let bands = baz_core::equalizer::Bands::from_centidb(eq.bands_centidb);
    let wanted = bands.suggested_preamp();
    let preamp = f32::from(eq.preamp_centidb) / 100.0;
    // `suggested_preamp` answers zero or a negative number of decibels: the
    // amount the whole signal has to come down by so the largest boost fits.
    if wanted < 0.0 && preamp > wanted + 0.05 {
        let boost = -wanted;
        return format!(
            "The biggest lift here is +{boost:.0} dB, and there is nowhere for it \
             to go — loud parts can distort. Make room turns everything down by \
             {boost:.0} so it fits."
        );
    }
    "Shaping the sound. Turn this off and baz plays the file untouched — the \
     same bytes, not a flat filter."
        .to_owned()
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
            PANEL_W < faders + gaps + 24.0 * theme::GAP_LG,
            "the panel has grown padding it does not draw"
        );
    }

    /// **The controls row fits on one line.**
    ///
    /// It did not, and nothing failed: the picker and `Keep` joined the switch
    /// and `Make room`, the row overflowed a panel measured by its *faders*,
    /// and `Make room` quietly wrapped onto two lines. A wrap is what this
    /// toolkit does instead of complaining, so it has to be measured.
    ///
    /// Measured in the crudest honest way — a per-character estimate for the
    /// panel's own type size, against the strings the panel actually ships.
    /// It cannot be exact without laying the text out, and it does not need to
    /// be: the fault it guards against is a control being *added* to a full
    /// row, which is tens of pixels, not ones.
    #[test]
    fn the_controls_row_fits_on_one_line() {
        // A little over half the point size, which is a fair mean advance for
        // this face at small sizes; generous rather than tight, because the
        // cost of being wrong is a panel a few pixels wider than it needs.
        let per_char = theme::SIZE_META * 0.58;
        #[expect(clippy::cast_precision_loss, reason = "labels are a few dozen chars")]
        let measure = |label: &str| label.chars().count() as f32 * per_char;

        let switch = theme::STEPPER_HIT + theme::GAP_SM + measure("Shape the sound");
        // Both words that can stand in the keeping slot, so neither the wider
        // one nor a future third can overflow unnoticed.
        let keeping = measure("Forget").max(measure("Keep")) + 2.0 * theme::GAP_MD;
        let headroom = measure("Make room") + 2.0 * theme::GAP_MD;
        let needed = switch
            + theme::GAP_SM
            + PRESET_W
            + theme::GAP_SM
            + keeping
            + theme::GAP_SM
            + headroom
            + 2.0 * theme::GAP_LG;

        assert!(
            PANEL_W >= needed,
            "the controls row needs about {needed:.0} px and the panel is \
             {PANEL_W:.0} — the last control on the row will wrap onto a \
             second line rather than saying so"
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
