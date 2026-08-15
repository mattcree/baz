//! The **Settings place**: baz's standing decisions, and the shape every
//! setting after the first one takes.
//!
//! # What this surface is for
//!
//! Everything that is a standing decision rather than a transport action. It
//! holds three sections: **Playback** — ReplayGain (ADR-0013), whose content moved
//! here from the rail panel *verbatim* — and **Library**, the music folders baz
//! holds and the force sync that re-reads them (ADR-0022). It remains the
//! container for the ones the vision promises next: the output chain and
//! exclusive mode, a signal-path readout, the enrichment toggles that are off by
//! default.
//!
//! # Why it is a place, and no longer a panel
//!
//! It was a panel in the right-hand rail, sharing 340 px with the album
//! inspector and the queue, and the argument for that is answered at length in
//! [`crate::place`] and ADR-0016. In short: a preference is not a glance, the
//! rail was simultaneously too narrow and 60% empty for the settings that
//! exist, and none of the settings that are *coming* is a section in a 292 px
//! column. Leaving the shelf is the right cost — you are not browsing while you
//! set a pre-amp — and it is free to reverse, because the Library's scroll,
//! query and selection live in one struct that nothing here touches.
//!
//! # The shape a section takes
//!
//! One heading, one sentence of what the section is for, the controls, and —
//! where the engine has something to say about the here and now — a readout
//! underneath. A future section is another entry in the list on the left and
//! another block in the same scroll, in the same order, with the same three
//! type sizes. **Nothing about the layout has to be revisited to add one**,
//! which is the property a place buys that a panel could not.
//!
//! The section list was drawn with one entry in it, deliberately, against the
//! day there was a second — and that day arrived with Library. **It cost an
//! entry in [`SECTIONS`], a block below, and an `on_press`**: no new
//! arrangement, no new widths, no new heading treatment, and the alternative
//! (inventing the navigation at the same time as the second section) never
//! happened. A spine with one vertebra looked like an over-build for a week and
//! like the obvious place to put the next thing forever after.
//!
//! # The frame does not move
//!
//! Places replace each other, and the two frames a listener sees either side of
//! that replacement — the top strip and the now-playing bar — must be the same
//! height in both, or navigating would look like the window resizing. So this
//! place's header carries the same padding and the same hairline as the
//! Library's top bar, and the bar below is drawn by the shell for both.
//!
//! # Tone
//!
//! Every string about ReplayGain comes from [`crate::replaygain`] already
//! written, and this module chooses no words of its own about what the engine is
//! doing. That is deliberate and it is the same rule the bottom bar's signal
//! note follows: the vocabulary is unit-tested where it is decided, and the view
//! cannot soften or sharpen it. Nothing here is styled as a fault, and no
//! reading gets the lamp amber — the accent means playback truth (ADR-0013 §8,
//! ADR-0009 §5), and how a gain stage is configured is not a claim about the
//! music.
//!
//! The Library section's words *are* this module's, because its subject is: a
//! folder's track count and last scan are facts about the index, and the two
//! phrases that state them ([`scanned_phrase`], [`tracks_phrase`]) are pinned by
//! test here rather than described here. Two of them are load-bearing beyond
//! tone. A folder that is not reachable says so and says that **nothing was
//! removed from it** — because that is the guarantee, and a listener who sees a
//! NAS greyed out needs to know their library is intact. And the confirming
//! press of Remove names what goes: *the tracks*, not the files.

use std::path::{Path, PathBuf};

use iced::widget::{
    Column, Space, button, checkbox, column, container, image as iced_image, pick_list, row, rule,
    scrollable, text, text_input, tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::Message;
use crate::playback::OutputChoice;
use crate::player::PlayerState;
use crate::replaygain::{self, MODES};
use crate::views::place_header_with;
use crate::{icon, theme, theme_file};

/// One music folder, as the Library section draws it (ADR-0022).
///
/// A projection, built by the shell from the config (which folders) and the
/// index (what is in them). Nothing here is state.
#[derive(Debug, Clone)]
pub(crate) struct FolderRow {
    /// The folder itself.
    pub(crate) path: PathBuf,
    /// How many tracks the index holds **recorded under this folder** — not
    /// how many are under it by path. The two differ exactly where the old
    /// removal gate was wrong: a nested folder's tracks belong to whichever
    /// root read them.
    pub(crate) tracks: usize,
    /// When a scan of it last finished, in nanoseconds since the Unix epoch;
    /// `None` when none ever has.
    pub(crate) last_scan_ns: Option<i64>,
    /// Whether the most recent pass found it missing — an unmounted share, an
    /// unplugged drive, a folder somebody renamed.
    pub(crate) unavailable: bool,
}

/// Everything the Library section draws, gathered by the shell.
#[derive(Debug)]
pub(crate) struct LibraryView<'a> {
    /// The folders, in the listener's order.
    pub(crate) folders: Vec<FolderRow>,
    /// What has been typed into the add-a-folder field.
    pub(crate) input: &'a str,
    /// Why the last folder submitted was not added, if it was not.
    pub(crate) error: Option<&'a str>,
    /// Which folder's Remove has been pressed once and is waiting for the
    /// confirming press. See [`folder_block`].
    pub(crate) pending_removal: Option<usize>,
    /// Whether a scan is running right now — a force sync while one is in
    /// flight would be a second worker over the same library.
    pub(crate) scanning: bool,
    /// Rows belonging to no folder at all: pre-v8 rows nothing adopted, from a
    /// folder baz was pointed at once and is not pointed at now.
    pub(crate) unrooted: Vec<PathBuf>,
    /// Whether every rootless path is exposed for the confirming press.
    pub(crate) unrooted_pending: bool,
    /// Baz's listener-owned playlist directory, when available.
    pub(crate) playlists: Option<&'a Path>,
    /// Missing rows beneath successfully scanned roots whose parent directory
    /// is absent, making automatic removal unsafe.
    pub(crate) prunable: &'a [PathBuf],
    /// Whether the bulk removal's exact consequence is exposed for confirm.
    pub(crate) prune_pending: bool,
    /// Now, in nanoseconds since the Unix epoch — so "scanned four minutes ago"
    /// is arithmetic the view does rather than a clock it reads.
    pub(crate) now_ns: i64,
}

/// The launch-time snapshot used by the shared-output picker.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OutputView<'a> {
    pub(crate) choices: &'a [OutputChoice],
    pub(crate) selected: &'a OutputChoice,
    /// Endpoint opened by this process, which may differ from the persisted
    /// next-launch selection.
    pub(crate) active: &'a OutputChoice,
    pub(crate) error: Option<&'a str>,
}

/// Settings → Appearance state owned by the shell.
pub(crate) struct ThemeView<'a> {
    pub(crate) selected: String,
    pub(crate) json: &'a str,
    pub(crate) notice: Option<&'a str>,
}

/// Inner padding of the place's content area (logical px).
///
/// [`theme::HANG`], not `GAP_XL`: a place fills the window, so its content hangs
/// from the **one window gutter** every other window-edge surface hangs from
/// (law L1). `GAP_XL` is padding *inside* a panel and was never a window margin;
/// spending it as one is how baz ended up with three of them — 16 for the
/// chrome, 24 here, 40 on the wall — and nothing in either bar aligned with
/// anything in the collection.
const PLACE_PAD: f32 = theme::HANG;

/// The sections this place holds, in the order they are listed.
///
/// **Two, and the second one cost an entry here and a block below.** That was
/// the claim the place made when it had one section and drew the spine anyway,
/// and Library is the section that tests it: the arrangement, the widths, the
/// heading shape and the scroll are all unchanged, and the list became a real
/// control rather than a picture of one.
///
/// Playback stays first because it was first; a listener who opens Settings out
/// of habit finds what they left.
const SECTIONS: [&str; 5] = ["Playback", "Library", "Appearance", "Vibe", "Debug"];

/// The index of the Library section in [`SECTIONS`].
pub(crate) const LIBRARY_SECTION: usize = 1;
/// The index of the Vibe section in [`SECTIONS`].
pub(crate) const APPEARANCE_SECTION: usize = 2;
/// The index of the Vibe section in [`SECTIONS`].
pub(crate) const VIBE_SECTION: usize = 3;
/// The index of the session diagnostic stream in [`SECTIONS`].
pub(crate) const DEBUG_SECTION: usize = 4;

/// The Settings place: a header with the way back, a list of sections, and the
/// current section's content.
///
/// `window_width` decides the arrangement and nothing else: at
/// [`theme::SETTINGS_BREAKPOINT`] and above, the section list is a column on
/// the left and the content sits beside it; below it the two stack, because
/// under a thousand pixels the list and a 640 px form cannot both have their
/// width and the form is the one being used.
#[expect(
    clippy::needless_pass_by_value,
    reason = "ThemeView groups one settings form's borrowed and owned projection at the view boundary"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "the Settings place receives one explicit projection per independent section"
)]
pub(crate) fn view<'a>(
    player: &'a PlayerState,
    window_width: f32,
    section: usize,
    library: LibraryView<'a>,
    output: OutputView<'a>,
    vibe_workers: usize,
    theme_view: ThemeView<'a>,
    diagnostic_lines: Vec<String>,
    resources: Option<crate::resource::Reading>,
    sleep: Option<std::time::Duration>,
) -> Element<'a, Message> {
    let room = theme::active();
    let beside_the_list = window_width >= theme::SETTINGS_BREAKPOINT;
    let place_note = if section == DEBUG_SECTION {
        "Session diagnostics; nothing here is written to disk."
    } else {
        "Kept in config.toml, and remembered next time."
    };
    let blocks = match section {
        LIBRARY_SECTION => vec![library_section(library)],
        APPEARANCE_SECTION => vec![appearance_section(&theme_view)],
        VIBE_SECTION => vec![vibe_section(vibe_workers)],
        DEBUG_SECTION => vec![debug_section(diagnostic_lines, resources)],
        _ => vec![
            output_section(output, player),
            replay_gain_section(player),
            sleep_section(sleep),
        ],
    };
    let content = container(
        scrollable(
            Column::with_children(blocks)
                .spacing(theme::GAP_XL)
                .padding(theme::scroll_gutter()),
        )
        .direction(scrollable::Direction::Vertical(theme::list_scrollbar()))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .height(Length::Fill),
    )
    .width(Length::Fixed(content_width(window_width, beside_the_list)))
    .height(Length::Fill);

    let body: Element<'_, Message> = if beside_the_list {
        row![section_list(section), content]
            .spacing(theme::GAP_XL)
            .height(Length::Fill)
            .into()
    } else {
        // Below the breakpoint the spine lies down: the same entries, the same
        // one control height, in a row instead of a column. It is still the
        // navigation — a heading that named only the current section would
        // leave the other one unreachable at a small window.
        column![section_row(section), content]
            .spacing(theme::GAP_MD)
            .height(Length::Fill)
            .into()
    };

    column![
        // The frame is one function in five places (doc 10 §7 step 8): the
        // strip the record's page, the queue and the playlist already wear,
        // with this place's own name and note. Back sends
        // [`Message::LeavePlace`] — the same landing as the gear's toggle
        // and <kbd>Esc</kbd>'s peel: the Library.
        place_header_with("Settings", Some(place_note),),
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(PLACE_PAD),
    ]
    .into()
}

/// The existing tagged console stream, retained only for this process.
fn debug_section(
    lines: Vec<String>,
    resources: Option<crate::resource::Reading>,
) -> Element<'static, Message> {
    let room = theme::active();
    // **What baz itself is costing this machine**, above the log — the
    // owner's *"show how much RAM/CPU it is using in the debug menu"*.
    //
    // It stands in this section and nowhere else, and its heading says whose
    // numbers these are. Every other figure baz shows is a fact about the
    // listener's music; this is a fact about the program, and on a health
    // surface a resident-set figure would read as a verdict when the honest
    // reading of a large one is *this is what a decoded artwork cache costs*.
    // It is also where item 37's memory budget can finally be **measured**
    // inside the running app rather than only argued about.
    //
    // Before the first tick there is nothing to draw and the block is absent
    // rather than a placeholder — the clock is installed with the section, so
    // "nothing yet" lasts under a second and a reserved row would flicker.
    let mut section = column![].spacing(theme::GAP_XL);
    if let Some(reading) = resources {
        let mut block = column![section_heading(
            "This process",
            "Baz's own resource use · sampled once a second, only while you are \
             reading this section.",
        )]
        .spacing(theme::GAP_MD);
        for line in crate::resource::lines(reading) {
            block = block.push(
                text(line)
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .color(room.paper_dim),
            );
        }
        section = section.push(block);
    }
    let mut log = column![section_heading(
        "Diagnostic log",
        "This session only · newest first · the notification bell remains the event history.",
    )]
    .spacing(theme::GAP_MD);
    if lines.is_empty() {
        log = log.push(
            text("No diagnostics have been recorded in this session.")
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .color(room.paper_faint),
        );
    } else {
        for line in lines {
            log = log.push(
                text(line)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .font(theme::SANS)
                    .color(room.paper_dim),
            );
        }
    }
    section.push(log).into()
}

/// Four coordinated built-ins plus the bounded local JSON extension surface.
#[expect(
    clippy::too_many_lines,
    reason = "one small form is clearest as one composition beside its validation workflow"
)]
fn appearance_section<'a>(view: &ThemeView<'a>) -> Element<'a, Message> {
    let room = theme::active();
    let mut choices = column![].spacing(theme::GAP_XXS);
    for (code, name) in theme_file::BUILTINS {
        let selected = view.selected == code;
        choices = choices.push(
            button(
                container(
                    text(if selected {
                        format!("{name} · selected")
                    } else {
                        name.to_owned()
                    })
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY),
                )
                .height(Length::Fill)
                .align_y(alignment::Vertical::Center),
            )
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .width(Length::Fill)
            .padding(theme::pad(0.0, theme::GAP_MD))
            .style(move |_theme, status| theme::segment(room, status, selected))
            .on_press(Message::ThemeSelected(code)),
        );
    }

    let preview: Element<'_, Message> = match theme_file::preview(&view.selected) {
        Ok(preview) => {
            let mut swatches = row![].spacing(theme::GAP_XXS);
            for color in preview.colors {
                swatches = swatches.push(
                    container(Space::new())
                        .width(Length::Fill)
                        .height(Length::Fixed(theme::TRANSPORT_HIT))
                        .style(move |_theme| iced::widget::container::Style {
                            background: Some(color.into()),
                            ..Default::default()
                        }),
                );
            }
            column![
                text(format!("Preview · {}", preview.name))
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_dim),
                swatches,
            ]
            .spacing(theme::GAP_XS)
            .into()
        }
        Err(error) => text(format!(
            "Selected theme unavailable: {error}. Closing Time will be used."
        ))
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.alert)
        .into(),
    };

    let json = text_input("Paste a complete v1 theme JSON document", view.json)
        .on_input(Message::ThemeJsonChanged)
        .on_submit(Message::ThemeImportPasted)
        .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .style(move |_theme, status| theme::input(room, status));

    let actions = row![
        word_action("Import pasted", Message::ThemeImportPasted),
        word_action("Import file…", Message::ThemePickFile),
        word_action("Load template", Message::ThemeLoadTemplate),
        word_action("Export selected…", Message::ThemeExport),
    ]
    .spacing(theme::GAP_XS)
    .wrap();

    let mut section = column![
        section_heading(
            "Visual room",
            "Four safe built-ins, or a validated local JSON room. Whole-app changes apply on restart.",
        ),
        choices,
        preview,
        text("Custom JSON is data only: colours and focus opacity. No code, URLs, paths, fonts, layout or behaviour.")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint),
        json,
        actions,
    ]
    .spacing(theme::GAP_SM);
    if let Some(notice) = view.notice {
        section = section.push(
            text(notice)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(
                    if notice.contains("must")
                        || notice.contains("invalid")
                        || notice.contains("Could not")
                    {
                        room.alert
                    } else {
                        room.paper_dim
                    },
                ),
        );
    }
    section.into()
}

fn word_action(label: &'static str, message: Message) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press(message)
    .into()
}

/// Controls the amount of parallel local CLAP inference used by the first
/// library scan. More sessions trade additional CPU and RAM for a shorter
/// wait; the setting is persisted and applies to the next scan.
/// **The sleep timer**: how long until Baz stops on its own.
///
/// A parity gap, and the plainest kind — every phone player has one, and it
/// is the one thing a listener asks for at midnight and cannot improvise
/// (`docs/design/18-feature-parity.md` §2.3).
///
/// It **pauses**, and does not stop, close or quit: pausing is the one ending
/// that keeps the run, the position and the queue exactly where they were, so
/// the morning's first press carries on rather than starting over. Nothing is
/// faded — a fade would be baz changing the volume the listener set.
fn sleep_section(remaining: Option<std::time::Duration>) -> Element<'static, Message> {
    let room = theme::active();
    let mut choices = row![].spacing(theme::GAP_SM);
    for minutes in crate::app::SLEEP_CHOICES {
        choices = choices.push(word_control(
            minutes.label,
            true,
            Message::SleepTimerSet(minutes.minutes),
        ));
    }
    column![
        section_heading(
            "Sleep timer",
            "Pause the music after a while. The run, the position and the queue stay where they are.",
        ),
        choices,
        text(remaining.map_or_else(
            || "Off — nothing is scheduled.".to_owned(),
            |left| {
                let seconds = left.as_secs();
                format!(
                    "Pausing in {}:{:02}.",
                    seconds / 60,
                    seconds % 60
                )
            }
        ))
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(if remaining.is_some() {
            room.paper
        } else {
            room.paper_faint
        }),
    ]
    .spacing(theme::GAP_SM)
    .into()
}

fn vibe_section(workers: usize) -> Element<'static, Message> {
    let room = theme::active();
    column![
        section_heading(
            "Vibe model workers",
            "Concurrent local CLAP sessions used during the first library scan.",
        ),
        stepper_row(
            "Workers",
            workers.to_string(),
            workers > 1,
            workers < crate::config::MAX_VIBE_WORKERS,
            Message::VibeWorkers(workers.saturating_sub(1).max(1)),
            Message::VibeWorkers((workers + 1).min(crate::config::MAX_VIBE_WORKERS)),
        ),
        text(format!(
            "1–{} sessions. More workers finish sooner but use more CPU and RAM.",
            crate::config::MAX_VIBE_WORKERS
        ))
        .size(theme::SIZE_META)
        .line_height(theme::LEADING_META)
        .color(room.paper_faint),
    ]
    .spacing(theme::GAP_SM)
    .into()
}

/// Shared-mode endpoint selection. The engine owns a cpal stream for its
/// lifetime, so this standing decision is applied at the next launch rather
/// than tearing a playing run out from under the listener.
fn output_section<'a>(output: OutputView<'a>, player: &'a PlayerState) -> Element<'a, Message> {
    let room = theme::active();
    let picker = pick_list(
        output.choices,
        Some(output.selected),
        Message::OutputDeviceSelected,
    )
    .width(Length::Fill)
    .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
    .text_size(theme::SIZE_BODY)
    .text_line_height(theme::LEADING_BODY)
    .style(move |_theme, status| theme::output_picker(room, status))
    .menu_style(move |_theme| theme::output_menu(room));

    let mut section = column![
        section_heading(
            "Audio output",
            "Use the system default, or keep baz on one output device.",
        ),
        container(picker)
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .align_y(alignment::Vertical::Center),
        text(output_status(output.active, output.selected))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint),
    ]
    .spacing(theme::GAP_SM);
    if let Some(error) = output.error {
        section = section.push(
            text(error)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.alert),
        );
    }
    if let Some(reason) = player.availability_note() {
        section = section.push(
            text(format!(
                "Could not open {}: {reason}. Select another output and restart Baz.",
                output.active
            ))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.alert),
        );
    }
    if let Some(warning) = player.signal_warning() {
        section = section.push(
            container(
                column![
                    text(warning.title)
                        .size(theme::SIZE_BODY)
                        .line_height(theme::LEADING_BODY)
                        .font(theme::MEDIUM)
                        .color(room.alert),
                    text(warning.detail)
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .color(room.paper),
                ]
                .spacing(theme::GAP_XS),
            )
            .padding(theme::GAP_MD)
            .style(move |_theme| theme::panel(room)),
        );
    }
    section.into()
}

fn output_status(active: &OutputChoice, selected: &OutputChoice) -> String {
    if active == selected {
        format!("In use now: {active}.")
    } else {
        format!("In use now: {active}. Selected for next launch: {selected}.")
    }
}

/// How wide the form gets: what the window has left for it, capped at
/// [`theme::SETTINGS_CONTENT_W`].
///
/// Computed rather than expressed as a maximum on the container, and the
/// difference is not cosmetic: a `max_width` bounds the *limits* a child is
/// laid out in, and a `Fill` child inside a `Shrink` container resolves against
/// what the row actually handed it. Measuring the rendered pixels is what
/// caught that — the segmented control ran 998 px wide in a 640 px cap — so the
/// width is arithmetic the view does itself and `theme.rs` asserts.
///
/// The floor matters as much as the cap: at a small window the form gets
/// whatever there is, because a stepper row that will not fit is worse than a
/// long one.
///
/// # It answers the window now
///
/// The cap used to be the constant [`theme::SETTINGS_CONTENT_W`], so the form's
/// right edge landed on **878 at a 1280 px window and 878 at a 1920 px one** —
/// 0.686 W and then 0.457 W, with a thousand pixels of empty wall beside it and
/// one right-aligned line of type stranded in it (the audit's defect 9). A
/// measure has a comfortable range rather than a single right answer, so the
/// target is half the window, clamped into
/// `[SETTINGS_CONTENT_W, SETTINGS_CONTENT_MAX]` — 55 to 75 characters of body
/// text — and bounded by what the window actually has left.
fn content_width(window_width: f32, beside_the_list: bool) -> f32 {
    let taken = if beside_the_list {
        2.0f32.mul_add(PLACE_PAD, theme::SETTINGS_NAV_W) + theme::GAP_XL
    } else {
        2.0 * PLACE_PAD
    };
    let measure =
        (0.5 * window_width).clamp(theme::SETTINGS_CONTENT_W, theme::SETTINGS_CONTENT_MAX);
    (window_width - taken).clamp(theme::SETTINGS_CONTENT_MIN, measure)
}

// The place's own header function is gone (doc 10 §7 step 8): the strip is
// [`crate::views::place_header`], one function in five places, so the frame
// cannot drift between them. What this file used to argue for its private
// copy — the same padding, the same window gutter, the same hairline, Back
// centred in its box — is now argued once, where the strip is drawn.

/// The section list: the place's spine, as a column beside the content.
///
/// Styled with the same segmented control the edition selector and the
/// ReplayGain mode use, for the same reason the room answers *which one of
/// these few* the same way everywhere.
///
/// It became a live control the moment there was a second section to reach —
/// which is exactly the growth the one-vertebra spine was drawn for, and cost
/// an `on_press` and nothing else.
fn section_list(current: usize) -> Element<'static, Message> {
    let mut list = column![].spacing(theme::GAP_XXS);
    for (index, section) in SECTIONS.iter().enumerate() {
        list = list.push(section_entry(index, section, current, Length::Fill));
    }
    container(list)
        .width(Length::Fixed(theme::SETTINGS_NAV_W))
        .height(Length::Fill)
        .into()
}

/// The same spine, laid on its side for a window too narrow to hold it beside
/// the form. Same entries, same order, same one control height.
fn section_row(current: usize) -> Element<'static, Message> {
    let mut list = row![].spacing(theme::GAP_XXS);
    for (index, section) in SECTIONS.iter().enumerate() {
        list = list.push(section_entry(index, section, current, Length::Shrink));
    }
    list.into()
}

/// One entry of the spine, in either arrangement.
fn section_entry(
    index: usize,
    section: &'static str,
    current: usize,
    width: Length,
) -> Element<'static, Message> {
    let room = theme::active();
    let selected = index == current;
    button(
        // Centred in its own fixed box, which is what law L3 asks a fixed box
        // to state rather than leave to iced's top-left default.
        container(
            text(section)
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .font(theme::MEDIUM)
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .width(width)
    // One control height (law L7): the entry is a nav target and stands
    // `TRANSPORT_HIT`, not the 36 px its own padding used to make it.
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::segment(room, status, selected))
    .on_press(Message::SettingsSection(index))
    .into()
}

/// The ReplayGain section: the mode, what that mode does, the two pre-amps,
/// clipping prevention, and what it all came to for the track playing now.
fn replay_gain_section(player: &PlayerState) -> Element<'_, Message> {
    let room = theme::active();
    let state = player.replay_gain();
    // No engine, nothing to configure — the same rule the album panel's Play
    // button follows, and for the same reason: a control that cannot act must
    // not pretend it can.
    let live = player.engine_ready();

    let mut section = column![
        section_heading(
            "ReplayGain",
            "Play everything at the loudness its tags declare.",
        ),
        mode_selector(state, live),
        // The mode's own sentence, in the quiet ink: present in every mode, so
        // choosing one is never a guess — and in a slot of
        // [`theme::SETTING_NOTE_H`], reserved for the longest of them, so that
        // switching modes moves nothing below it. Without the reservation,
        // pressing *Album* (whose sentence wraps to two lines) would push the
        // pre-amps down by a line, taking the control out from under the
        // pointer that had just chosen it.
        container(
            text(state.mode_note())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        )
        .height(Length::Fixed(theme::SETTING_NOTE_H)),
    ]
    .spacing(theme::GAP_SM);

    section = section
        .push(stepper_row(
            "Pre-amp",
            state.preamp_label(),
            live && state.preamp_can_step(-1),
            live && state.preamp_can_step(1),
            Message::ReplayGainPreamp(-1),
            Message::ReplayGainPreamp(1),
        ))
        .push(stepper_row(
            "Untagged files",
            state.no_tag_preamp_label(),
            live && state.no_tag_preamp_can_step(-1),
            live && state.no_tag_preamp_can_step(1),
            Message::ReplayGainNoTagPreamp(-1),
            Message::ReplayGainNoTagPreamp(1),
        ))
        .push(
            // **A checkbox is a pointer target too** (law L7). It was
            // `SIZE_BODY` — a **13 px** box, the smallest control in the product
            // by a factor of two and the only one with no floor at all. It takes
            // [`theme::STEPPER_HIT`], the named secondary target, and its row
            // stands the full `TRANSPORT_HIT` so the tick sits on the same line
            // rhythm as the stepper rows above it.
            container(
                checkbox(state.prevent_clipping())
                    .label("Keep peaks below full scale")
                    .size(theme::STEPPER_HIT)
                    .text_size(theme::SIZE_META)
                    .text_line_height(theme::LEADING_META)
                    .spacing(theme::GAP_SM)
                    .style(move |_theme, status| theme::check(room, status))
                    .on_toggle_maybe(live.then_some(Message::ReplayGainPreventClipping)),
            )
            .height(Length::Fixed(theme::TRANSPORT_HIT))
            .align_y(alignment::Vertical::Center),
        );

    // What is in force right now — present only while a track is playing and
    // ReplayGain is on. Off states no figure at all: the engine performs no
    // ReplayGain arithmetic in that mode, and a `0.00 dB` here would describe
    // arithmetic that is not happening (ADR-0013 §2).
    if let Some(readout) = player.replay_gain_readout() {
        section = section.push(readout_block(vec![
            (readout.gain, room.paper),
            (readout.detail, room.paper_faint),
        ]));
    }

    if let Some(note) = player.availability_note() {
        section = section.push(readout_block(vec![(note.clone(), room.paper_faint)]));
    }

    section.into()
}

/// The **Library** section: which folders baz holds, what is in each of them,
/// when it last looked, and the two acts that change any of it — adding a
/// folder, and forcing a sync (ADR-0022).
///
/// The shape is the one this place already had: heading, sentence, controls,
/// readout. Nothing about the layout was revisited to add it, which is the
/// property the place was built to have.
fn library_section(library: LibraryView<'_>) -> Element<'_, Message> {
    let room = theme::active();
    let mut section = column![section_heading(
        "Music folders",
        "The folders baz holds. Your files are never moved or changed.",
    )]
    .spacing(theme::GAP_SM);

    if library.folders.is_empty() {
        section = section.push(
            text("No folders yet.")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        );
    }
    let folder_count = library.folders.len();
    for (index, folder) in library.folders.into_iter().enumerate() {
        let pending = library.pending_removal == Some(index);
        section = section.push(folder_block(
            index,
            folder_count,
            &folder,
            pending,
            library.scanning,
            library.now_ns,
        ));
    }

    section = section.push(add_folder_row(library.input));
    if let Some(error) = library.error {
        section = section.push(
            text(error)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.alert),
        );
    }

    section = section
        .push(Space::new().height(Length::Fixed(theme::GAP_MD)))
        .push(section_heading(
            "Force sync",
            "Re-reads every file, including the ones nothing has touched.",
        ))
        .push(force_sync_row(library.scanning));

    if let Some(playlists) = library.playlists {
        section = section
            .push(Space::new().height(Length::Fixed(theme::GAP_MD)))
            .push(section_heading(
                "Playlists folder",
                "Baz stores the playlist files you own here.",
            ))
            .push(
                row![
                    container(
                        text(playlists.display().to_string())
                            .size(theme::SIZE_META)
                            .line_height(theme::LEADING_META)
                            .color(room.paper_faint)
                            .wrapping(text::Wrapping::None),
                    )
                    .width(Length::Fill)
                    .clip(true),
                    word_control("Open folder", true, Message::OpenPlaylistsFolder),
                ]
                .spacing(theme::GAP_MD)
                .align_y(iced::Alignment::Center),
            );
    }

    if !library.prunable.is_empty() {
        section = section
            .push(Space::new().height(Length::Fixed(theme::GAP_MD)))
            .push(prune_block(
                library.prunable,
                library.prune_pending,
                library.scanning,
            ));
    }

    // What the index has to say about itself, in the slot this place reserves
    // for the machine's own report. Two lines at most, and each is present only
    // when it is true.
    let mut readings: Vec<(String, iced::Color)> = Vec::new();
    if library.scanning {
        readings.push(("Scanning now.".to_owned(), room.paper));
    }
    if !library.unrooted.is_empty() {
        section = section
            .push(Space::new().height(Length::Fixed(theme::GAP_MD)))
            .push(unrooted_block(
                &library.unrooted,
                library.unrooted_pending,
                library.scanning,
            ));
    }
    if !readings.is_empty() {
        section = section.push(readout_block(readings));
    }
    section.into()
}

fn unrooted_block(paths: &[PathBuf], pending: bool, scanning: bool) -> Element<'static, Message> {
    let room = theme::active();
    let mut block = column![section_heading(
        "Outside held folders",
        "Legacy index rows assigned to no folder cannot be refreshed or pruned by a scan.",
    )]
    .spacing(theme::GAP_XS);
    if pending {
        block = block.push(
            text(format!(
                "Remove {} from Baz's index? Files on disk, playlists, listening history and the current run are untouched.",
                tracks_phrase(paths.len())
            ))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper),
        );
        for path in paths {
            block = block.push(
                text(path.display().to_string())
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint),
            );
        }
        block = block.push(
            row![
                word_control("Remove from index", true, Message::PruneUnrooted),
                word_control("Keep", true, Message::CancelPruneUnrooted),
            ]
            .spacing(theme::GAP_XXS),
        );
    } else {
        block = block
            .push(
                text(format!(
                    "{}. Add their folder back to refresh them, or review the exact paths before removing only the stale index entries.",
                    tracks_phrase(paths.len())
                ))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.warning),
            )
            .push(word_control(
                "Review unheld paths",
                !scanning,
                Message::ConfirmPruneUnrooted,
            ));
    }
    block.into()
}

/// Preview and confirm rows whose album directory disappeared. The complete
/// path list is intentionally visible before the destructive press: its shape
/// is how a listener recognizes an unmounted nested share and chooses Keep.
fn prune_block(paths: &[PathBuf], pending: bool, scanning: bool) -> Element<'static, Message> {
    let room = theme::active();
    let mut block = column![section_heading(
        "Missing albums",
        "A completed scan could not find these paths, but their parent folders are absent too.",
    )]
    .spacing(theme::GAP_XS);

    if pending {
        block = block.push(
            text(format!(
                "Remove {} from Baz's index? Audio files, playlist files, listening history and the current run are untouched. Bringing the files back restores their original added dates.",
                tracks_phrase(paths.len())
            ))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper),
        );
        for path in paths {
            block = block.push(
                text(path.display().to_string())
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_faint),
            );
        }
        block = block.push(
            row![
                word_control("Prune index", true, Message::PruneMissing),
                word_control("Keep", true, Message::CancelPruneMissing),
            ]
            .spacing(theme::GAP_XXS),
        );
    } else {
        block = block
            .push(
                text(format!(
                    "{} retained for safety. Preview them before deciding.",
                    tracks_phrase(paths.len())
                ))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.warning),
            )
            .push(word_control(
                "Review missing paths",
                !scanning,
                Message::ConfirmPruneMissing,
            ));
    }
    block.into()
}

/// One folder: its path on a control-height line with its Remove, and one quiet
/// line underneath saying what is in it and when baz last looked.
///
/// **Removing is two presses, and the second one is labelled with what it
/// does.** The first press replaces the quiet line with the consequence in
/// words and the control with `Forget` / `Keep`; nothing has happened yet. A
/// single press would be a destructive act with no undo sitting one pixel from
/// a scroll — and what it destroys is worth naming, because the tracks go
/// (ADR-0022 §4) even though the files do not.
///
/// **And what it does not destroy is worth naming too** (ADR-0042). Since
/// schema v9 the one fact a rescan could never rediscover — when each of those
/// tracks first arrived — is kept, and adding the folder back restores it. That
/// changes what the confirming sentence can honestly promise, so it says it:
/// the act is now fully reversible, and a listener hesitating over a folder
/// they might want back should be told that before they press rather than
/// after.
fn folder_block(
    index: usize,
    count: usize,
    folder: &FolderRow,
    pending: bool,
    scanning: bool,
    now_ns: i64,
) -> Element<'static, Message> {
    let room = theme::active();
    let controls: Element<'static, Message> = if pending {
        row![
            word_control("Forget", true, Message::RemoveMusicFolder(index)),
            word_control("Keep", true, Message::CancelRemoveMusicFolder),
        ]
        .spacing(theme::GAP_XXS)
        .into()
    } else {
        row![
            word_control(
                "Up",
                !scanning && index > 0,
                Message::MoveMusicFolderUp(index),
            ),
            word_control(
                "Down",
                !scanning && index + 1 < count,
                Message::MoveMusicFolderDown(index),
            ),
            word_control(
                "Remove",
                !scanning,
                Message::ConfirmRemoveMusicFolder(index),
            ),
        ]
        .spacing(theme::GAP_XXS)
        .into()
    };
    let note = if pending {
        (forget_phrase(folder.tracks), room.paper)
    } else if folder.unavailable {
        (
            format!(
                "Not reachable right now — {} kept, nothing removed.",
                tracks_phrase(folder.tracks)
            ),
            room.paper_faint,
        )
    } else {
        (
            format!(
                "{} · {}",
                tracks_phrase(folder.tracks),
                scanned_phrase(folder.last_scan_ns, now_ns)
            ),
            room.paper_faint,
        )
    };
    column![
        container(
            row![
                text(folder.path.display().to_string())
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .color(room.paper)
                    .wrapping(text::Wrapping::None),
                Space::new().width(Length::Fill),
                controls,
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
        )
        // One row pitch, and it is the product's one control height (law L7),
        // exactly as the stepper rows above it.
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .align_y(alignment::Vertical::Center),
        text(note.0)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(note.1),
    ]
    .spacing(theme::GAP_XXS)
    .into()
}

/// The add-a-folder row: a well, its word, and the system picker beside them.
///
/// **Two doors, and neither replaces the other** (ADR-0025). `Browse…` opens
/// the desktop's own folder dialog — the portal on Linux, which is what makes
/// it GNOME's dialog on GNOME and KDE's on KDE. The well takes a typed path,
/// which the picker structurally cannot: a dialog offers only what the
/// filesystem shows it, and the share a listener knows by heart but has not
/// mounted today is exactly the folder worth configuring anyway. The refusals
/// ledger's rule — every act a visible pointer target — is met by each door on
/// its own, so losing the portal (a bare window manager, a broken service)
/// costs a convenience and no capability.
fn add_folder_row(input: &str) -> Element<'_, Message> {
    let room = theme::active();
    container(
        row![
            text_input("/path/to/another/folder", input)
                .on_input(Message::MusicFolderInput)
                .on_submit(Message::AddMusicFolder)
                // The product's one control height, like the search well and
                // the first-run field (law L7).
                .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
                .size(theme::SIZE_BODY)
                .line_height(theme::LEADING_BODY)
                .width(Length::Fill)
                .style(move |_theme, status| theme::input(room, status)),
            word_control("Add", !input.trim().is_empty(), Message::AddMusicFolder),
            // The ellipsis is the convention meaning "a dialog follows"; the
            // control itself decides nothing. Always enabled: there is no state
            // in which choosing a folder is not allowed to begin.
            word_control("Browse\u{2026}", true, Message::PickMusicFolder),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .align_y(alignment::Vertical::Center)
    .into()
}

/// The force-sync control, with the one thing it is worth knowing about it.
///
/// Disabled while a scan is running rather than queued: two workers over one
/// library would write the same rows twice and report two sets of counts.
fn force_sync_row(scanning: bool) -> Element<'static, Message> {
    let room = theme::active();
    container(
        row![
            text(if scanning {
                "A scan is running."
            } else {
                "Everything else is incremental; this is not."
            })
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_dim),
            Space::new().width(Length::Fill),
            word_control("Force sync", !scanning, Message::ForceSync),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .align_y(alignment::Vertical::Center)
    .into()
}

/// A word that acts: the transport's quiet card around a label, at the one
/// control height.
///
/// The same treatment `‹ Library` gets in the header, because it is the same
/// kind of thing — a control whose name is short and unambiguous, so it is a
/// word rather than a glyph baz would have to invent ([`crate::icon`]).
fn word_control(label: &'static str, enabled: bool, message: Message) -> Element<'static, Message> {
    let room = theme::active();
    button(
        container(
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM)
                .color(if enabled {
                    room.paper
                } else {
                    room.paper_muted
                })
                .wrapping(text::Wrapping::None),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_MD))
    .style(move |_theme, status| theme::transport(room, room.wall, status))
    .on_press_maybe(enabled.then_some(message))
    .into()
}

/// `n tracks`, singular where it should be. A count of zero is stated rather
/// than hidden: a folder baz has scanned and found nothing in is a fact worth
/// seeing, not an empty space.
fn tracks_phrase(tracks: usize) -> String {
    if tracks == 1 {
        "1 track".to_owned()
    } else {
        format!("{tracks} tracks")
    }
}

/// The confirming press's sentence: what goes, what stays, and what comes back.
///
/// Three clauses in the order a hesitating listener needs them.
///
/// 1. **What goes**, named and counted — *the tracks*, which is the index's
///    record of them and not the music.
/// 2. **What is not touched.** `The files stay on disk` is the oldest promise
///    in this place and it does not move: baz has never deleted a music file.
/// 3. **What survives the round trip** (ADR-0042). Before schema v9 this act
///    quietly destroyed each track's first-seen, so a folder removed and added
///    back filed every album under ADDED = *today*; now it is kept and
///    restored. A reversible act that reads as irreversible gets refused by
///    people who would have been fine, so the sentence has to carry it.
///
/// `when they arrived` rather than `their ADDED date`: the wall's word is a
/// column heading, and this is a sentence. It stays true for a pre-v7 row that
/// never had a first-seen — such a row read `Not recorded` before the act and
/// reads `Not recorded` after it, so nothing is lost either way.
fn forget_phrase(tracks: usize) -> String {
    format!(
        "Forget {}? The files stay on disk; baz stops holding them but remembers when they arrived.",
        tracks_phrase(tracks)
    )
}

/// When a scan of a folder last finished, in words.
///
/// Coarse on purpose, and coarser the further back it goes: the question a
/// listener is asking is "has baz looked since I copied that album in", and
/// `4 minutes ago` answers it where `2026-08-08 14:02:17` makes them do
/// arithmetic. A clock that has gone backwards reads as `just now` rather than
/// as a negative age.
fn scanned_phrase(last_scan_ns: Option<i64>, now_ns: i64) -> String {
    const MINUTE: i64 = 60 * 1_000_000_000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let Some(last) = last_scan_ns else {
        return "not scanned yet".to_owned();
    };
    let age = now_ns.saturating_sub(last).max(0);
    let plural = |n: i64, unit: &str| {
        if n == 1 {
            format!("scanned 1 {unit} ago")
        } else {
            format!("scanned {n} {unit}s ago")
        }
    };
    match age {
        age if age < MINUTE => "scanned just now".to_owned(),
        age if age < HOUR => plural(age / MINUTE, "minute"),
        age if age < DAY => plural(age / HOUR, "hour"),
        age => plural(age / DAY, "day"),
    }
}

/// A section's first two lines: **its name, then one sentence saying what it
/// is for.**
///
/// The shape every setting after the first one takes, and the reason it is a
/// function rather than two `text` calls copied into the next section: a place
/// whose sections each invented their own heading treatment is a junk drawer
/// with headings. Name in the emphasis size and the medium weight, sentence in
/// the meta size and the dim ink, one rung of the ladder between them.
///
/// One sentence, present tense, about what the setting *does* rather than what
/// it is — the vocabulary rule the whole product follows.
fn section_heading(name: &'static str, sentence: &'static str) -> Element<'static, Message> {
    let room = theme::active();
    column![
        text(name)
            .size(theme::SIZE_EMPHASIS)
            .line_height(theme::LEADING_EMPHASIS)
            .font(theme::MEDIUM)
            .color(room.paper),
        text(sentence)
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_dim),
    ]
    .spacing(theme::GAP_XXS)
    .into()
}

/// The last part of a section's shape: **what the engine has to say about the
/// here and now**, under a hairline.
///
/// Set apart from the controls above it because it is a different kind of
/// sentence: everything above is a decision the listener is making, and this is
/// the machine reporting what that decision came to for the track playing right
/// now. Without the rule the two read as one list and the readout looks like
/// another setting with its control missing.
///
/// A hairline is the whole of the separation — no surface step, no card. This
/// is a fourth structural rule beyond the three
/// `.interface-design/system.md` §2 names, and it earns the place the same way
/// they do: it divides two kinds of content inside one column, which is exactly
/// what the inspector's rule against the shelf does.
fn readout_block(lines: Vec<(String, iced::Color)>) -> Element<'static, Message> {
    let room = theme::active();
    let mut block =
        column![rule::horizontal(1).style(move |_theme| theme::hairline(room, room.wall))]
            .spacing(theme::GAP_SM);
    let mut readings = column![].spacing(theme::GAP_XXS);
    for (line, ink) in lines {
        readings = readings.push(
            text(line)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(ink),
        );
    }
    block = block.push(readings);
    block.into()
}

/// The mode control: the same quiet segmented control the album panel's
/// edition selector uses.
///
/// Reused rather than invented, because it is the same question — *which one
/// of these few* — and the room should answer it the same way twice. The
/// order is [`MODES`]', which is Off first: it is the default and the one that
/// changes nothing.
fn mode_selector(state: replaygain::ReplayGain, live: bool) -> Element<'static, Message> {
    let room = theme::active();
    let mut segments = row![].spacing(theme::GAP_XXS);
    for mode in MODES {
        let selected = state.mode() == mode;
        segments = segments.push(
            button(
                container(
                    text(replaygain::mode_label(mode))
                        .size(theme::SIZE_META)
                        .line_height(theme::LEADING_META)
                        .font(theme::MEDIUM)
                        .wrapping(text::Wrapping::None),
                )
                .width(Length::Fill)
                .align_x(alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(theme::pad(theme::GAP_XS, theme::GAP_SM))
            .style(move |_theme, status| theme::segment(room, status, selected))
            .on_press_maybe(live.then_some(Message::ReplayGainMode(mode))),
        );
    }
    container(segments)
        .width(Length::Fill)
        .padding(theme::SEGMENT_INSET)
        .style(move |_theme| theme::segmented(room))
        .into()
}

/// One numeric setting: its name, its value, and a `−`/`+` pair.
///
/// The value sits in a [`theme::SETTING_VALUE_W`] slot, so a repeated press
/// cannot move the button under the pointer holding it — the same fixed-slot
/// rule the bottom bar is built on, and it holds in a proportional face because
/// Plex Sans's figures are tabular. A stepper at the end of its travel renders
/// disabled rather than absorbing the press.
fn stepper_row(
    label: &'static str,
    value: String,
    can_decrease: bool,
    can_increase: bool,
    decrease: Message,
    increase: Message,
) -> Element<'static, Message> {
    let room = theme::active();
    container(
        row![
            text(label)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim)
                .wrapping(text::Wrapping::None),
            Space::new().width(Length::Fill),
            container(
                text(value)
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper)
                    .wrapping(text::Wrapping::None)
            )
            .width(Length::Fixed(theme::SETTING_VALUE_W))
            .align_x(alignment::Horizontal::Right),
            stepper(icon::Glyph::Minus, "Step down", can_decrease, decrease),
            stepper(icon::Glyph::Plus, "Step up", can_increase, increase),
        ]
        .spacing(theme::GAP_SM)
        .align_y(iced::Alignment::Center),
    )
    // One row pitch, and it is the product's one control height: the row is
    // `TRANSPORT_HIT` tall around a `STEPPER_HIT` pair, so two stepper rows are
    // 32 apart on the 4 px lattice rather than 24 apart on nothing.
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .align_y(alignment::Vertical::Center)
    .into()
}

/// One `−` or `+` button: the transport's quiet card in a smaller square,
/// wearing the drawn [`icon::Glyph::Minus`] / [`icon::Glyph::Plus`] pair
/// (doc 10 §3.6).
///
/// The pair used to be font characters — U+2212 chosen so the two matched
/// in width — and the same care is now structural: the minus *is* the
/// plus's own horizontal bar (asserted in `icon.rs`), so the pair cannot
/// drift apart. U+2212 stays legitimate in the **value** beside them,
/// where it is a figure; the slot is a control, and a control slot carries
/// a drawn glyph or a word, never a borrowed character. Icon-only, so each
/// carries its name as a tooltip (ADR-0017 §4c); the ink is the resting
/// ladder, because unlike the row slots these stand at rest.
fn stepper(
    glyph: icon::Glyph,
    name: &'static str,
    enabled: bool,
    message: Message,
) -> Element<'static, Message> {
    let room = theme::active();
    let mark = container(
        iced_image(icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::glyph_opacity(enabled, false)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    tooltip(
        button(mark)
            .width(Length::Fixed(theme::STEPPER_HIT))
            .height(Length::Fixed(theme::STEPPER_HIT))
            .padding(0)
            .style(move |_theme, status| theme::transport(room, room.wall, status))
            .on_press_maybe(enabled.then_some(message)),
        text(name)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Top,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60 * 1_000_000_000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;

    /// The Library section's one piece of arithmetic, pinned: a folder's age
    /// reads in the unit a listener is thinking in, and a folder nothing has
    /// scanned says so rather than pretending to a time.
    #[test]
    fn a_folders_last_scan_reads_in_the_unit_the_question_is_asked_in() {
        let now = 1_000 * DAY;
        assert_eq!(scanned_phrase(None, now), "not scanned yet");
        assert_eq!(scanned_phrase(Some(now), now), "scanned just now");
        assert_eq!(
            scanned_phrase(Some(now - MINUTE + 1), now),
            "scanned just now"
        );
        assert_eq!(
            scanned_phrase(Some(now - MINUTE), now),
            "scanned 1 minute ago"
        );
        assert_eq!(
            scanned_phrase(Some(now - 4 * MINUTE), now),
            "scanned 4 minutes ago"
        );
        assert_eq!(scanned_phrase(Some(now - HOUR), now), "scanned 1 hour ago");
        assert_eq!(
            scanned_phrase(Some(now - 23 * HOUR), now),
            "scanned 23 hours ago"
        );
        assert_eq!(scanned_phrase(Some(now - DAY), now), "scanned 1 day ago");
        assert_eq!(
            scanned_phrase(Some(now - 400 * DAY), now),
            "scanned 400 days ago"
        );
        // A clock that has gone backwards — a corrected system time, a stamp
        // written on another machine — reads as `just now`, never as a negative
        // age.
        assert_eq!(scanned_phrase(Some(now + DAY), now), "scanned just now");
    }

    #[test]
    fn a_track_count_is_stated_even_when_it_is_none() {
        assert_eq!(tracks_phrase(0), "0 tracks");
        assert_eq!(tracks_phrase(1), "1 track");
        assert_eq!(tracks_phrase(3_214), "3214 tracks");
    }

    #[test]
    fn output_status_distinguishes_this_process_from_the_next_launch() {
        let system = OutputChoice::SystemDefault;
        let dac = OutputChoice::Device("USB DAC".to_owned());
        assert_eq!(
            output_status(&system, &system),
            "In use now: System default."
        );
        assert_eq!(
            output_status(&system, &dac),
            "In use now: System default. Selected for next launch: USB DAC."
        );
    }

    /// The confirming press's sentence, pinned where it is decided. It has to
    /// name what goes, refuse to claim anything about the files, and state the
    /// guarantee that makes the act reversible (ADR-0042) — a listener reading
    /// only this line must not be able to conclude that removing a folder
    /// throws away when their records arrived, because since schema v9 it does
    /// not.
    #[test]
    fn the_confirming_press_names_what_goes_what_stays_and_what_comes_back() {
        let phrase = forget_phrase(412);
        assert_eq!(
            phrase,
            "Forget 412 tracks? The files stay on disk; baz stops holding them \
             but remembers when they arrived."
        );
        assert!(
            phrase.starts_with("Forget 412 tracks?"),
            "the count is named"
        );
        assert!(phrase.contains("files stay on disk"));
        assert!(phrase.contains("remembers when they arrived"));
        assert_eq!(
            forget_phrase(1),
            "Forget 1 track? The files stay on disk; baz stops holding them \
             but remembers when they arrived."
        );
    }
}
