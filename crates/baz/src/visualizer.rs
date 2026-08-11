//! Now Playing's switchable, audio-truthful visual field.

use std::f32::consts::TAU;

use baz_core::engine::{VISUAL_SAMPLE_COUNT, VisualizationFrame};
use iced::widget::{
    Space, button, column, container, image as iced_image, row, stack, text, tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::theme;

const BANDS: usize = 24;
const VISUAL_PAD: f32 = 24.0;
const BAR_GAP: f32 = 4.0;

/// The subject shown above the current track's identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Cover,
    JewelCase,
    Spectrum,
    Levels,
}

impl Mode {
    pub(crate) const ALL: [Self; 4] = [Self::Cover, Self::JewelCase, Self::Spectrum, Self::Levels];

    #[must_use]
    pub(crate) fn needs_audio(self) -> bool {
        matches!(self, Self::Spectrum | Self::Levels)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Cover => "Cover art",
            Self::JewelCase => "Jewel case",
            Self::Spectrum => "Spectrum",
            Self::Levels => "VU meters",
        }
    }

    fn glyph(self) -> crate::icon::Glyph {
        match self {
            Self::Cover => crate::icon::Glyph::VisualCover,
            Self::JewelCase => crate::icon::Glyph::VisualCase,
            Self::Spectrum => crate::icon::Glyph::VisualSpectrum,
            Self::Levels => crate::icon::Glyph::VisualLevels,
        }
    }
}

/// Draw only the selected subject; its controls live in the app bar.
pub(crate) fn view(
    mode: Mode,
    width: f32,
    audio: &VisualizationFrame,
    cover: Element<'static, Message>,
    jewel_case: Element<'static, Message>,
) -> Element<'static, Message> {
    let subject = match mode {
        Mode::Cover => cover,
        Mode::JewelCase => jewel_case,
        Mode::Spectrum => spectrum(audio, width, width),
        Mode::Levels => levels(audio, width, width),
    };
    // Every mode occupies the same square stage. The physical jewel case is
    // wider than it is tall, so it is centred inside that stage; switching
    // modes must not move the placard or redefine the artwork area.
    container(subject)
        .width(Length::Fixed(width))
        .height(Length::Fixed(width))
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
        .into()
}

/// Four icon detents for the app bar's view-options slot.
pub(crate) fn marks(current: Mode) -> Element<'static, Message> {
    row(Mode::ALL.map(|mode| mode_button(mode, current))).into()
}

fn mode_button(choice: Mode, selected: Mode) -> Element<'static, Message> {
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
            .on_press(Message::VisualizationMode(choice))
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

fn spectrum(audio: &VisualizationFrame, width: f32, height: f32) -> Element<'static, Message> {
    let room = theme::active();
    let bands = frequency_bands(audio);
    let usable_h = (height - 2.0 * VISUAL_PAD).max(1.0);
    let mut bars = row![].spacing(BAR_GAP).align_y(iced::Alignment::End);
    for level in bands {
        let bar_h = (usable_h * level).max(2.0);
        let bar = container(Space::new(Length::Fill, Length::Fixed(bar_h)))
            .width(Length::FillPortion(1))
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(room.lamp)),
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
        .padding(VISUAL_PAD)
        .style(move |_theme| visual_ground(room))
        .into()
}

fn levels(audio: &VisualizationFrame, width: f32, height: f32) -> Element<'static, Message> {
    let room = theme::active();
    let meter_h = (height - 2.0 * VISUAL_PAD - theme::LINE_CAPTION).max(1.0);
    let left = level_meter("L", audio.left_rms, audio.left_peak, meter_h);
    let right = level_meter("R", audio.right_rms, audio.right_peak, meter_h);
    container(row![left, right].width(Length::Fill).spacing(theme::GAP_XL))
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .padding(VISUAL_PAD)
        .style(move |_theme| visual_ground(room))
        .into()
}

fn level_meter(label: &'static str, rms: f32, peak: f32, height: f32) -> Element<'static, Message> {
    let room = theme::active();
    let fill_h = height * amplitude_height(rms);
    let peak_h = height * amplitude_height(peak);
    let meter = stack![
        container(Space::new(Length::Fill, Length::Fixed(height))).style(move |_theme| {
            container::Style {
                background: Some(iced::Background::Color(room.plinth)),
                border: iced::Border {
                    color: room.hairline(room.plinth),
                    width: 1.0,
                    radius: 2.0.into(),
                },
                ..container::Style::default()
            }
        }),
        container(
            column![
                Space::with_height(Length::Fixed((height - peak_h - 1.0).max(0.0))),
                container(Space::new(Length::Fill, Length::Fixed(1.0))).style(move |_theme| {
                    container::Style {
                        background: Some(iced::Background::Color(room.paper)),
                        ..container::Style::default()
                    }
                }),
                Space::with_height(Length::Fill),
            ]
            .height(Length::Fixed(height)),
        )
        .width(Length::Fill),
        container(
            column![
                Space::with_height(Length::Fill),
                container(Space::new(Length::Fill, Length::Fixed(fill_h.max(2.0)))).style(
                    move |_theme| container::Style {
                        background: Some(iced::Background::Color(room.lamp)),
                        border: iced::Border {
                            radius: 2.0.into(),
                            ..iced::Border::default()
                        },
                        ..container::Style::default()
                    }
                ),
            ]
            .height(Length::Fixed(height)),
        )
        .width(Length::Fill),
    ];
    column![
        meter,
        text(label)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION)
            .font(theme::MEDIUM)
            .color(room.paper_faint),
    ]
    .spacing(theme::GAP_XS)
    .align_x(iced::Alignment::Center)
    .width(Length::FillPortion(1))
    .into()
}

fn visual_ground(room: &theme::Palette) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(room.recess)),
        border: iced::Border {
            color: room.hairline(room.recess),
            width: 1.0,
            radius: theme::RADIUS_CTRL.into(),
        },
        ..container::Style::default()
    }
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
    fn silence_has_no_spectrum_or_level() {
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
    fn a_tone_lifts_a_band_without_leaving_the_meter() {
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
