//! Now Playing's foreground choice and optional audio-truthful background.

use std::f32::consts::TAU;

use baz_core::engine::{VISUAL_SAMPLE_COUNT, VisualizationFrame};
use iced::widget::{Space, button, column, container, image as iced_image, row, text, tooltip};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::theme;

const BANDS: usize = 24;
const BAR_GAP: f32 = 4.0;
const HISTORY_FRAMES: usize = 32;

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

    /// The next object in the cycle — the owner, 2026-08-17: *"can we make
    /// the three album cover views into a toggle cycle similar to the
    /// background visualisation button"*.
    ///
    /// The order is the order the three marks stood in, so a listener who knew
    /// where to reach finds the same sequence under one press instead of
    /// three targets. It returns to `Cover` from `None`, exactly as
    /// [`Mode::next`] returns to `Off` — a cycle with no end is the only kind
    /// a single control can offer.
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Cover => Self::JewelCase,
            Self::JewelCase => Self::None,
            Self::None => Self::Cover,
        }
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

/// The optional audio-truthful field behind the independent foreground.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Off,
    Spectrum,
    Waveform,
    Spectrogram,
}

impl Mode {
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Off => Self::Spectrum,
            Self::Spectrum => Self::Waveform,
            Self::Waveform => Self::Spectrogram,
            Self::Spectrogram => Self::Off,
        }
    }

    pub(crate) const fn active(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub(crate) const fn records_history(self) -> bool {
        matches!(self, Self::Waveform | Self::Spectrogram)
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Off => "Audio visualization off",
            Self::Spectrum => "Spectrum",
            Self::Waveform => "Rolling waveform",
            Self::Spectrogram => "Spectrogram",
        }
    }
}

/// Fixed storage shared by the two history-based modes. Capturing and drawing
/// are both bounded by constants, and the owner drops this whole clock away
/// from visible Now Playing.
#[derive(Debug, Clone)]
pub(crate) struct History {
    amplitudes: [f32; HISTORY_FRAMES],
    spectra: [[f32; BANDS]; HISTORY_FRAMES],
    cursor: usize,
    len: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            amplitudes: [0.0; HISTORY_FRAMES],
            spectra: [[0.0; BANDS]; HISTORY_FRAMES],
            cursor: 0,
            len: 0,
        }
    }
}

impl History {
    #[expect(
        clippy::cast_precision_loss,
        reason = "the fixed 256-sample divisor is exactly representable in f32"
    )]
    pub(crate) fn capture(&mut self, mode: Mode, audio: &VisualizationFrame) {
        match mode {
            Mode::Waveform => {
                let mean_square = audio
                    .samples
                    .iter()
                    .map(|sample| sample * sample)
                    .sum::<f32>()
                    / VISUAL_SAMPLE_COUNT as f32;
                self.amplitudes[self.cursor] = amplitude_height(mean_square.sqrt());
            }
            Mode::Spectrogram => self.spectra[self.cursor] = frequency_bands(audio),
            Mode::Off | Mode::Spectrum => return,
        }
        self.cursor = (self.cursor + 1) % HISTORY_FRAMES;
        self.len = (self.len + 1).min(HISTORY_FRAMES);
    }

    fn ordered_index(&self, position: usize) -> usize {
        (self.cursor + HISTORY_FRAMES - self.len + position) % HISTORY_FRAMES
    }

    fn amplitude(&self, position: usize) -> f32 {
        self.amplitudes[self.ordered_index(position)]
    }

    fn spectrum(&self, position: usize) -> &[f32; BANDS] {
        &self.spectra[self.ordered_index(position)]
    }
}

/// The independent decisions represented by the app-bar controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) foreground: Foreground,
    pub(crate) mode: Mode,
    pub(crate) facts: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            foreground: Foreground::JewelCase,
            mode: Mode::Off,
            facts: true,
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

/// **Three controls, not five**: the album object, the audio field and the
/// fact feed, each one press that moves to the next state.
///
/// The album object was three radio-like marks standing side by side until
/// 2026-08-17 — the owner: *"can we make the three album cover views into a
/// toggle cycle similar to the background visualisation button"*. The
/// visualizer beside it had been a cycle all along, so the bar was spending
/// five slots on two questions and answering them in two different
/// grammars. It is one grammar now, and the slot it gives back is two
/// [`theme::STEPPER_HIT`] boxes of the app bar's scarcest lane.
///
/// **What a cycle costs, stated rather than discovered**: the states are no
/// longer all visible at once, so a listener cannot see that a jewel case is
/// available without pressing. The tooltip carries it — *"Cover art — choose
/// Jewel case"* — which is the same promise [`mode_button`] has always made
/// and is why he named that control as the one to match.
pub(crate) fn marks(state: State) -> Element<'static, Message> {
    row![
        foreground_button(state.foreground),
        mode_button(state.mode),
        facts_button(state.facts),
    ]
    .into()
}

fn facts_button(on: bool) -> Element<'static, Message> {
    let room = theme::active();
    let mark = container(
        iced_image(crate::icon::inked(
            crate::icon::Glyph::VisualFacts,
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
        .on_press(Message::ToggleFacts);
    tooltip(
        control,
        text(if on {
            "Fact feed is on — hide it"
        } else {
            "Fact feed is off — show it"
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

/// The album object, as one cycling control — [`mode_button`]'s twin.
///
/// It shows **the state it is in** and names the state the press leads to, and
/// it is lit on the same rule its neighbour uses: lit while something is
/// drawn, quiet at `None`, so the two controls read as one pair answering
/// *what is on the screen*.
///
/// No new message. `VisualizationForeground` already means *be this object*,
/// and the button computes which one — so the shell's arm, the config it
/// persists and every test over them are untouched by the change of grammar.
fn foreground_button(selected: Foreground) -> Element<'static, Message> {
    let room = theme::active();
    let showing = selected.draws_art();
    let mark = container(
        iced_image(crate::icon::inked(
            selected.glyph(),
            if showing { room.lamp } else { room.glyph() },
        ))
        .width(Length::Fixed(theme::ICON_PX))
        .height(Length::Fixed(theme::ICON_PX))
        .opacity(if showing { 1.0 } else { theme::GLYPH_OPACITY }),
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
        .on_press(Message::VisualizationForeground(selected.next()));
    tooltip(
        control,
        text(format!(
            "{} — choose {}",
            selected.label(),
            selected.next().label()
        ))
        .size(theme::SIZE_CAPTION)
        .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

fn mode_button(mode: Mode) -> Element<'static, Message> {
    let room = theme::active();
    let mark = container(
        iced_image(crate::icon::inked(
            crate::icon::Glyph::VisualSpectrum,
            if mode.active() {
                room.lamp
            } else {
                room.glyph()
            },
        ))
        .width(Length::Fixed(theme::ICON_PX))
        .height(Length::Fixed(theme::ICON_PX))
        .opacity(if mode.active() {
            1.0
        } else {
            theme::GLYPH_OPACITY
        }),
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
        .on_press(Message::NextVisualization);
    let next = mode.next();
    tooltip(
        control,
        text(format!("{} — choose {}", mode.label(), next.label()))
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

/// Draw the chosen audio field across the whole Now Playing body. It has no ground of
/// its own: the artwork-derived field remains visible through and around the
/// bars, while the cover or jewel case is composed independently above it.
pub(crate) fn background(
    mode: Mode,
    audio: &VisualizationFrame,
    history: &History,
    width: f32,
    height: f32,
) -> Element<'static, Message> {
    match mode {
        Mode::Off => Space::new().width(Length::Fill).height(Length::Fill).into(),
        Mode::Spectrum => spectrum(audio, width, height),
        Mode::Waveform => waveform(history, width, height),
        Mode::Spectrogram => spectrogram(history, width, height),
    }
}

fn spectrum(audio: &VisualizationFrame, width: f32, height: f32) -> Element<'static, Message> {
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
        let bar = container(
            Space::new()
                .width(Length::Fill)
                .height(Length::Fixed(bar_h)),
        )
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
            column![Space::new().height(Length::Fill), bar]
                .width(Length::FillPortion(1))
                .height(Length::Fill),
        );
    }
    container(bars)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .into()
}

fn waveform(history: &History, width: f32, height: f32) -> Element<'static, Message> {
    let room = theme::active();
    let mut trace = row![].spacing(2.0).align_y(iced::Alignment::Center);
    for position in 0..history.len {
        let line_h = (height * history.amplitude(position) * 0.72).max(2.0);
        let ink = iced::Color {
            a: 0.28,
            ..room.lamp
        };
        trace = trace.push(
            container(Space::new())
                .width(Length::FillPortion(1))
                .height(Length::Fixed(line_h))
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(ink)),
                    border: iced::Border {
                        radius: 1.0.into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                }),
        );
    }
    container(trace)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .center_y(Length::Fill)
        .into()
}

fn spectrogram(history: &History, width: f32, height: f32) -> Element<'static, Message> {
    let room = theme::active();
    let mut texture = row![].spacing(1.0);
    for position in 0..history.len {
        let mut slice = column![].spacing(1.0);
        for level in history.spectrum(position).iter().rev() {
            let ink = iced::Color {
                a: 0.04 + 0.34 * level,
                ..room.lamp
            };
            slice = slice.push(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::FillPortion(1))
                    .style(move |_theme| container::Style {
                        background: Some(iced::Background::Color(ink)),
                        ..container::Style::default()
                    }),
            );
        }
        texture = texture.push(slice.width(Length::FillPortion(1)).height(Length::Fill));
    }
    container(texture)
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
    fn the_foreground_choice_and_visualizer_are_independent() {
        let mut state = State::default();
        assert_eq!(state.foreground, Foreground::JewelCase);
        assert_eq!(state.mode, Mode::Off);

        state.mode = Mode::Spectrum;
        assert_eq!(state.foreground, Foreground::JewelCase);
        state.foreground = Foreground::Cover;
        assert_eq!(state.mode, Mode::Spectrum);
        state.foreground = Foreground::None;
        assert_eq!(state.mode, Mode::Spectrum);
        state.mode = Mode::Off;
        assert_eq!(state.foreground, Foreground::None);
    }

    /// **The album object cycles too, and comes back to where it started** —
    /// one control, three states, no dead end.
    #[test]
    fn the_album_object_cycles_through_all_three_and_returns() {
        let mut seen = vec![Foreground::Cover];
        let mut at = Foreground::Cover;
        for _ in 0..Foreground::ALL.len() {
            at = at.next();
            seen.push(at);
        }
        assert_eq!(
            seen,
            vec![
                Foreground::Cover,
                Foreground::JewelCase,
                Foreground::None,
                Foreground::Cover,
            ],
            "the cycle skips a state or does not return"
        );
        // Every state is reachable from every other, which is the property a
        // single control has to have to replace three targets.
        for start in Foreground::ALL {
            let mut reached = vec![start];
            let mut at = start;
            for _ in 1..Foreground::ALL.len() {
                at = at.next();
                reached.push(at);
            }
            for choice in Foreground::ALL {
                assert!(
                    reached.contains(&choice),
                    "{choice:?} is unreachable from {start:?}"
                );
            }
        }
    }

    #[test]
    fn modes_cycle_from_off_and_back_to_off() {
        let mut mode = Mode::Off;
        for expected in [Mode::Spectrum, Mode::Waveform, Mode::Spectrogram, Mode::Off] {
            mode = mode.next();
            assert_eq!(mode, expected);
        }
    }

    #[test]
    #[expect(
        clippy::cast_precision_loss,
        reason = "the fixed test range is far below f32's exact integer limit"
    )]
    fn history_is_a_fixed_ring_with_oldest_to_newest_reads() {
        let mut history = History::default();
        for value in 1..=HISTORY_FRAMES + 3 {
            let mut frame = VisualizationFrame::default();
            frame.samples.fill(value as f32 / 100.0);
            history.capture(Mode::Waveform, &frame);
        }
        assert_eq!(history.len, HISTORY_FRAMES);
        assert!(history.amplitude(0) < history.amplitude(HISTORY_FRAMES - 1));
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
