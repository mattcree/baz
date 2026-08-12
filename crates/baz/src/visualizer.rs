//! Now Playing's foreground choice and optional audio-truthful background.

use std::f32::consts::TAU;

use baz_core::engine::{VISUAL_SAMPLE_COUNT, VisualizationFrame};
use iced::widget::{Space, button, column, container, image as iced_image, row, text, tooltip};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::theme;

const BANDS: usize = 24;
const BAR_GAP: f32 = 4.0;

/// The record object shown above the current track's identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Foreground {
    Cover,
    JewelCase,
    None,
}

impl Foreground {
    const ALL: [Self; 3] = [Self::Cover, Self::JewelCase, Self::None];

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::JewelCase => "jewel-case",
            Self::None => "none",
        }
    }

    pub(crate) fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.code() == code)
    }

    pub(crate) const fn draws_art(self) -> bool {
        !matches!(self, Self::None)
    }

    pub(crate) const fn draws_case(self) -> bool {
        matches!(self, Self::JewelCase)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Cover => "Cover art",
            Self::JewelCase => "Jewel case",
            Self::None => "No album object",
        }
    }

    fn glyph(self) -> crate::icon::Glyph {
        match self {
            Self::Cover => crate::icon::Glyph::VisualCover,
            Self::JewelCase => crate::icon::Glyph::VisualCase,
            Self::None => crate::icon::Glyph::VisualNone,
        }
    }
}

/// The two independent decisions represented by the app-bar controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) foreground: Foreground,
    pub(crate) spectrum: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            foreground: Foreground::JewelCase,
            spectrum: false,
        }
    }
}

/// Draw the selected record object in the square foreground stage.
///
/// `None` is handled by the parent composition instead of this function: its
/// defining property is that the stage itself does not exist.
pub(crate) fn foreground(
    choice: Foreground,
    width: f32,
    cover: Element<'static, Message>,
    jewel_case: Element<'static, Message>,
) -> Element<'static, Message> {
    let subject = match choice {
        Foreground::Cover => cover,
        Foreground::JewelCase => jewel_case,
        Foreground::None => unreachable!("None has no foreground stage"),
    };
    // Both choices occupy the same square stage. The physical jewel case is
    // wider than it is tall, so it is centred inside that stage; switching the
    // foreground must not move the placard or redefine the artwork area.
    container(subject)
        .width(Length::Fixed(width))
        .height(Length::Fixed(width))
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
        .into()
}

/// Three radio-like foreground marks followed by one independent spectrum
/// toggle, all in the app bar's existing display-options slot.
pub(crate) fn marks(state: State) -> Element<'static, Message> {
    row(Foreground::ALL.map(|choice| foreground_button(choice, state.foreground)))
        .push(spectrum_button(state.spectrum))
        .into()
}

fn foreground_button(choice: Foreground, selected: Foreground) -> Element<'static, Message> {
    let room = theme::active();
    let active = choice == selected;
    let mark = container(
        iced_image(crate::icon::handle(choice.glyph()))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(if active {
                theme::GLYPH_OPACITY_HOVER
            } else {
                theme::GLYPH_OPACITY
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let boxed: Element<'static, Message> = if active {
        container(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .into()
    } else {
        button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.recess, status))
            .on_press(Message::VisualizationForeground(choice))
            .into()
    };
    tooltip(
        boxed,
        text(choice.label())
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

fn spectrum_button(on: bool) -> Element<'static, Message> {
    let room = theme::active();
    let mark = container(
        iced_image(crate::icon::inked(
            crate::icon::Glyph::VisualSpectrum,
            if on { room.lamp } else { room.glyph() },
        ))
        .width(Length::Fixed(theme::ICON_PX))
        .height(Length::Fixed(theme::ICON_PX))
        .opacity(if on { 1.0 } else { theme::GLYPH_OPACITY }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let control = button(mark)
        .width(Length::Fixed(theme::STEPPER_HIT))
        .height(Length::Fixed(theme::STEPPER_HIT))
        .padding(0)
        .style(move |_theme, status| theme::transport(room, room.recess, status))
        .on_press(Message::ToggleSpectrum);
    tooltip(
        control,
        text(if on {
            "Spectrum is on — hide the audio background"
        } else {
            "Spectrum is off — show the audio background"
        })
        .size(theme::SIZE_CAPTION)
        .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// Draw the spectrum across the whole Now Playing body. It has no ground of
/// its own: the artwork-derived field remains visible through and around the
/// bars, while the cover or jewel case is composed independently above it.
pub(crate) fn background(
    audio: &VisualizationFrame,
    width: f32,
    height: f32,
) -> Element<'static, Message> {
    let room = theme::active();
    let bands = frequency_bands(audio);
    let usable_h = height.max(1.0);
    let mut bars = row![].spacing(BAR_GAP).align_y(iced::Alignment::End);
    for level in bands {
        let bar_h = (usable_h * level).max(2.0);
        let ink = iced::Color {
            a: 0.18,
            ..room.lamp
        };
        let bar = container(Space::new(Length::Fill, Length::Fixed(bar_h)))
            .width(Length::FillPortion(1))
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(ink)),
                border: iced::Border {
                    radius: 1.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });
        bars = bars.push(
            column![Space::with_height(Length::Fill), bar]
                .width(Length::FillPortion(1))
                .height(Length::Fill),
        );
    }
    container(bars)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .into()
}

fn amplitude_height(amplitude: f32) -> f32 {
    if !amplitude.is_finite() || amplitude <= 0.0 {
        return 0.0;
    }
    ((20.0 * amplitude.log10() + 60.0) / 60.0).clamp(0.0, 1.0)
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "sample rates are audio-sized integers and both loop bounds are fixed below 256"
)]
fn frequency_bands(audio: &VisualizationFrame) -> [f32; BANDS] {
    if audio.sample_rate == 0 {
        return [0.0; BANDS];
    }
    let rate = audio.sample_rate as f32;
    let highest = (rate * 0.45).min(16_000.0);
    let ratio = (highest / 55.0).powf(1.0 / (BANDS - 1) as f32);
    std::array::from_fn(|band| {
        let frequency = 55.0 * ratio.powi(band as i32);
        goertzel(&audio.samples, frequency, rate)
    })
}

#[expect(
    clippy::cast_precision_loss,
    reason = "the fixed 256-sample window's indices are exactly representable in f32"
)]
fn goertzel(samples: &[f32; VISUAL_SAMPLE_COUNT], frequency: f32, rate: f32) -> f32 {
    let omega = TAU * frequency / rate;
    let coefficient = 2.0 * omega.cos();
    let mut previous = 0.0_f32;
    let mut before_previous = 0.0_f32;
    for (index, sample) in samples.iter().enumerate() {
        let phase = index as f32 / (VISUAL_SAMPLE_COUNT - 1) as f32;
        let window = 4.0 * phase * (1.0 - phase);
        let current = sample * window + coefficient * previous - before_previous;
        before_previous = previous;
        previous = current;
    }
    let power = previous * previous + before_previous * before_previous
        - coefficient * previous * before_previous;
    let amplitude = power.max(0.0).sqrt() * 2.0 / VISUAL_SAMPLE_COUNT as f32;
    amplitude_height(amplitude * 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_foreground_choice_and_spectrum_toggle_are_independent() {
        let mut state = State::default();
        assert_eq!(state.foreground, Foreground::JewelCase);
        assert!(!state.spectrum);

        state.spectrum = true;
        assert_eq!(state.foreground, Foreground::JewelCase);
        state.foreground = Foreground::Cover;
        assert!(state.spectrum);
        state.foreground = Foreground::None;
        assert!(state.spectrum);
        state.spectrum = false;
        assert_eq!(state.foreground, Foreground::None);
    }

    #[test]
    fn every_foreground_has_a_stable_config_word() {
        for foreground in Foreground::ALL {
            assert_eq!(Foreground::from_code(foreground.code()), Some(foreground));
        }
        assert_eq!(Foreground::from_code("future-object"), None);
    }

    #[test]
    fn none_is_the_only_choice_without_art_or_a_stage() {
        assert!(Foreground::Cover.draws_art());
        assert!(Foreground::JewelCase.draws_art());
        assert!(!Foreground::None.draws_art());
        assert!(Foreground::JewelCase.draws_case());
        assert!(!Foreground::Cover.draws_case());
        assert!(!Foreground::None.draws_case());
    }

    #[test]
    fn silence_has_no_spectrum() {
        assert!(
            frequency_bands(&VisualizationFrame::default())
                .into_iter()
                .all(|level| level.abs() < f32::EPSILON)
        );
        assert!(amplitude_height(0.0).abs() < f32::EPSILON);
    }

    #[test]
    #[expect(
        clippy::cast_precision_loss,
        reason = "the fixed 256-sample test window's indices are exactly representable in f32"
    )]
    fn a_tone_lifts_a_band_without_leaving_the_unit_range() {
        let mut frame = VisualizationFrame {
            sample_rate: 44_100,
            ..VisualizationFrame::default()
        };
        for (index, sample) in frame.samples.iter_mut().enumerate() {
            *sample = (TAU * 440.0 * index as f32 / 44_100.0).sin();
        }
        let bands = frequency_bands(&frame);
        assert!(bands.iter().copied().fold(0.0, f32::max) > 0.5);
        assert!(bands.into_iter().all(|level| (0.0..=1.0).contains(&level)));
    }
}
