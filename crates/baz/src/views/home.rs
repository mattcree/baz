//! **The Home place** — the interrupted run, and what is new.
//!
//! ADR-0030 §3.2 recommended a home *band* at the head of the Library's body
//! and drew this page in §9.4 as the alternative it was being recommended
//! against. **The owner chose the page**, and the product's preamble
//! says his decision is sufficient on its own; the ADR carries the amendment.
//!
//! # Three sections, and an honest inventory behind them
//!
//! ADR-0030 §6 inventoried what a home surface could truthfully hold and
//! found exactly two facts worth the room. That survives the change from band
//! to place unchanged, because it was an argument about *facts*, not about
//! geometry — and the owner has since added two more:
//!
//! - **`CONTINUE`** — the run to carry on with (ADR-0023 §6's snapshot, built
//!   for this; see [`crate::session`]).
//! - **`All songs`** — one tile, the whole collection, in the wall's own tile
//!   anatomy with a list's own sleeve. The owner: *"again I wanted the Play
//!   all, to be more like a tile on the home screen, a special 'playlist'"*.
//!   See [`all_songs_tile`] for why it wears the collage, why it sits second,
//!   and why the strip's `Play all` stays where it is.
//! - **`RECENTLY ADDED`** — a row of records by first-seen, in the wall's own
//!   tile, carrying the wall's own hover options.
//! - **`COLLECTION`** — four figures about the library, at the **foot** of the
//!   page. The owner's, verbatim: *"the album and track count below the search
//!   bar doesn't look good… maybe this should go into the home as some basic
//!   stats?"*. See [`collection`] for what the four are, why they are four, and
//!   why they are the last thing on the page rather than the first.
//!
//! # The band asks one question, and it is only asked in the silence
//!
//! > **`CONTINUE` stands whenever there is a run to carry on with and nothing
//! > is sounding.** Start anything, anywhere in the product, and it is gone;
//! > stop, and it is back, describing where you now are.
//!
//! The owner's rule, in his words: *"keep it simple with the continue part…
//! once you select resume, it just disappears"*, *"or takes you to now
//! playing"*, *"it just reappears when you stop the player"*. It replaces a
//! design in which the band gained a second reading and turned into a
//! `NOW PLAYING` placard once the music started, and it is better than that
//! design rather than merely smaller than it:
//!
//! - **It is one predicate, not a lifecycle.** [`standing`] is the whole of
//!   it, and there is no bookkeeping about a question having been *spent* that
//!   could get out of step with the engine.
//! - **There is no path where the band is wrongly absent.** Every state that
//!   is not *sounding* either has a run to offer or has nothing to offer, and
//!   both are drawn correctly by the same three lines.
//! - **It costs nothing at rest.** A live needle on this page would have
//!   wanted the position while the music ran; a band that is *absent* while
//!   the music runs wants nothing, so Home adds no subscription and no clock
//!   at all. That is the idle-cost problem deleted rather than budgeted for.
//! - **It is useful after every stop**, not only after a launch. Pause an
//!   album halfway, come to Home, and the way back in is right there.
//!
//! What is sounding is the bottom bar's job, in every place, and
//! [`Place::NowPlaying`](crate::place::Place::NowPlaying) is a place of its own
//! one row up in the returns lane. A Home band that described the sounding
//! track would be the same fact in three places at once.
//!
//! **`Resume` is the one play gesture in the product that navigates**, and it
//! goes to `Now playing` — see [`resume_line`].
//!
//! Refused from the page and still refused: **recently played** and
//! **playlists**, which are the returns lane's content one column to the left
//! — one fact drawn twice is doc 07 L8.6's test; **any unbidden suggestion**,
//! because generation without a request is what the home page of every
//! streaming product is made of (the pull was baz's one *requested* draw, it
//! never reached this page, and the owner removed it on 2026-08-10); and every
//! **engagement** statistic — what you played, how often, for how long — which
//! is not close. `COLLECTION` is not one of those, and the line is worth
//! stating plainly: it describes **what you own**, not what you do with it, and
//! every figure in it would be identical if the application had never been
//! opened.
//!
//! **A section is absent, not empty.** `CONTINUE` is absent while something is
//! sounding, with no run to carry on with, and when the library no longer holds
//! the file the run is on; `All songs` is absent when there is nothing to play;
//! `RECENTLY ADDED` is absent when the library holds fewer than a row of
//! records; `COLLECTION` is absent when there is no collection. A page with
//! none of the four says so in one line rather than drawing four empty
//! headings.
//!
//! # The signature of the whole design
//!
//! The placard carries the needle, and **nothing is drawn on the artwork**.
//! Every product this one is measured against puts a progress bar across the
//! bottom of the cover; baz puts it under the wall label, at exactly the
//! sleeve's width, where a gallery puts the caption. That is the one drawing
//! in the mockup the owner approved that is not a rearrangement of something
//! already shipped, and it is what [`needle`] is.

use iced::widget::{
    Space, button, column, container, image as iced_image, pick_list, row, scrollable, text,
    text_input,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::player::{Phase, PlayerState};
use crate::selection::Content;
use crate::shelf::Grid;
use crate::views::{gradient_block, section_rule};
use crate::{icon, theme, vm};

/// **The needle's arithmetic**, alone: how much of the line is amber and how
/// much is muted, given a position, a length and the sleeve's width.
///
/// Split out from the drawing so the numbers are testable without a window —
/// the one thing on this page that is arithmetic rather than composition, and
/// the one thing that would be wrong in a way a screenshot could not show.
///
/// Three properties hold at every input, and the tests state them: the two
/// runs and the tick fill the sleeve's width exactly; a track with no declared
/// length reads as unstarted rather than as finished; and a position past the
/// end clamps rather than overrunning.
#[must_use]
fn needle_runs(elapsed_ms: u64, total_ms: u64, width: f32) -> (f32, f32) {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a track's length in milliseconds is far below f32's \
                  exact-integer range; the quotient is a fraction of one"
    )]
    let fraction = if total_ms > 0 {
        (elapsed_ms as f32 / total_ms as f32).clamp(0.0, 1.0)
    } else {
        // No declared length is *unstarted*, never finished: an undeclared
        // duration is the scan not having read one, and a full amber line
        // would be the interface inventing a fact about a track it has not
        // measured.
        0.0
    };
    // The tick takes 1 px out of the line, so the two runs and it add to the
    // width exactly — the rule that keeps the needle the sleeve's own measure
    // at every position, which is what makes the band read as one object.
    let usable = (width - theme::NEEDLE_TICK_W).max(0.0);
    let filled = (usable * fraction).round();
    (filled, usable - filled)
}

/// The Home place's body.
pub(crate) fn view<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    resume: &'a crate::session::Snapshot,
    width: f32,
    hang: Grid,
    collecting: crate::playlists::Collecting,
) -> Element<'a, Message> {
    let room = theme::active();
    let mut body = column![].spacing(theme::HANG);

    let continuing = continue_band(shelf, player, resume, width);
    let everything = all_songs_tile(shelf, player, hang);
    let request = cfg!(feature = "vibe-analysis").then(|| vibe_shortcut(collecting.available));
    let added = recently_added(shelf, player, hang, collecting);
    let counted = collection(shelf);
    let nothing = continuing.is_none()
        && everything.is_none()
        && request.is_none()
        && added.is_none()
        && counted.is_none();
    if let Some(band) = continuing {
        body = body.push(band);
    }
    // **Second, under `CONTINUE` and above `RECENTLY ADDED`** — see
    // [`all_songs_tile`] for the argument, which is about what each of the
    // three offers is *for* rather than about how big they are.
    if let Some(band) = everything {
        body = body.push(band);
    }
    if let Some(section) = request {
        body = body.push(section);
    }
    if let Some(band) = added {
        body = body.push(band);
    }
    // **Last, always.** See [`collection`]: you come to Home to get back into
    // music, not to read numbers, so the numbers close the page.
    if let Some(band) = counted {
        body = body.push(band);
    }
    if nothing {
        // A page with neither fact says so once, plainly, and offers the one
        // thing there is to do: the collection.
        return container(
            text("Nothing to pick up yet. The Library is where everything is.")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint),
        )
        .center(Length::Fill)
        .into();
    }
    scrollable(container(body).padding(crate::views::place_pad()))
        .direction(iced::widget::scrollable::Direction::Vertical(
            theme::wall_scrollbar(),
        ))
        .style(move |_theme, status| theme::scrollbar(room, room.wall, status))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// The Home entry for opt-in local sonic analysis and its ordinary playlist.
#[expect(
    clippy::too_many_lines,
    reason = "the Home request composer keeps every visible request/result state together"
)]
pub(crate) fn vibe_creator<'a>(
    shelf: &'a Shelf,
    _player: &'a PlayerState,
    available: bool,
    width: f32,
    save_enabled: bool,
) -> Element<'a, Message> {
    let room = theme::active();
    let state = &shelf.vibe;
    let lead = column![
        section_rule("Make a mix"),
        text("Build a journey through qualities Baz can hear in your own music.")
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .color(room.paper),
        text("On this device · your audio never leaves Baz")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint),
    ]
    .spacing(theme::GAP_SM);
    if !available {
        return lead
            .push(
                text("Playlist storage is unavailable on this system.")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_dim),
            )
            .into();
    }
    let mut composer = lead;
    if !cfg!(feature = "vibe-analysis") {
        return composer
            .push(
                text(
                    "This is the light build. Install the full build to add local sonic analysis.",
                )
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
            )
            .spacing(theme::GAP_SM)
            .into();
    }

    let prompt = text_input("Try “dreamy shoegaze for a rainy evening”", &state.prompt)
        .on_input(Message::VibePrompt)
        .on_submit(Message::VibeCreate)
        .width(Length::Fill)
        .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
        .size(theme::SIZE_BODY)
        .line_height(theme::LEADING_BODY)
        .style(move |_theme, status| theme::input(room, status));
    let length = pick_list(
        crate::vibe::MixLength::ALL,
        Some(state.length),
        Message::VibeLength,
    )
    .width(Length::Fixed(170.0))
    .padding(theme::pad(theme::WELL_PAD_V, theme::GAP_MD))
    .text_size(theme::SIZE_BODY)
    .text_line_height(theme::LEADING_BODY)
    .style(move |_theme, status| theme::output_picker(room, status))
    .menu_style(move |_theme| theme::output_menu(room));
    let busy = state.preparing || state.analyzing;
    let can_create = !state.preparing
        && !state.prompt.trim().is_empty()
        && (!state.analyzing || state.has_features());
    composer = composer
        .push(prompt)
        .push(
            row![
                word_button(
                    "Late-night focus",
                    Message::PlaylistCreationExample("Late-night focus"),
                ),
                word_button(
                    "Warm Sunday morning",
                    Message::PlaylistCreationExample("Warm Sunday morning"),
                ),
                word_button(
                    "Restless then calm",
                    Message::PlaylistCreationExample("Restless then calm"),
                ),
            ]
            .spacing(theme::GAP_SM),
        )
        .push(
            row![
                column![
                    text("LENGTH")
                        .size(theme::SIZE_CAPTION)
                        .line_height(theme::LEADING_CAPTION)
                        .font(theme::MEDIUM)
                        .color(room.paper_faint),
                    length
                ]
                .spacing(theme::GAP_XS),
                Space::new().width(Length::Fill),
                word_button_maybe("Create mix", can_create.then_some(Message::VibeCreate))
            ]
            .spacing(theme::GAP_MD)
            .align_y(iced::Alignment::End),
        );

    if state.open && !state.has_features() && !state.preparing && !state.analyzing {
        composer = composer
            .push(
                text(format!(
                    "To make this mix, Baz will read {} selected-edition tracks once, keep a disposable local index, and never upload audio. You can keep listening or cancel.",
                    crate::vibe::library_paths(&shelf.albums, &shelf.edition_choice).len()
                ))
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_dim),
            )
            .push(
                row![
                    word_button("Analyse locally & create", Message::VibeAnalyze),
                    word_button("Not now", Message::VibeCancel)
                ]
                .spacing(theme::GAP_SM),
            );
    }
    if state.preparing {
        composer = composer.push(
            text("Checking the local analysis index…")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
        );
    }
    if state.analyzing {
        let current = state
            .current
            .as_deref()
            .map_or("next track", crate::vibe::seed_name);
        composer = composer
            .push(
                text(format!(
                    "Analysing {} of {} · {} · {} skipped",
                    state.done.saturating_add(state.failed).saturating_add(1),
                    state.total,
                    current,
                    state.failed
                ))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
            )
            .push(if state.has_features() {
                text(format!(
                    "A mix can use the {} tracks analysed so far; the scan will continue in the background.",
                    state.done.saturating_sub(state.failed)
                ))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim)
            } else {
                text("Create mix will become available after the first track is analysed.")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_dim)
            })
            .push(word_button("Cancel analysis", Message::VibeAnalysisCancel));
    }
    if let Some(error) = state.failure_note() {
        composer = composer.push(
            text(error)
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.lamp)
                .width(Length::Fill)
                .wrapping(text::Wrapping::Word),
        );
    }
    if let Some(preview) = &state.preview {
        let result = format!(
            "“{}” · {} · {} tracks",
            preview.request,
            preview.duration_note(),
            preview.items.len()
        );
        composer = composer
            .push(
                text(result)
                    .size(theme::SIZE_BODY)
                    .line_height(theme::LEADING_BODY)
                    .font(theme::MEDIUM)
                    .color(room.paper),
            )
            .push(
                text(preview.pool_note())
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_dim),
            );
        if state.request_changed() {
            composer = composer.push(
                text("Request changed · Create mix to update this preview")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .color(room.paper_dim),
            );
        }
        for (position, item) in preview.items.iter().enumerate() {
            let marker: Element<'_, Message> = text(format!("{:02}", position + 1))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .into();
            let under = item
                .artist
                .as_deref()
                .or(item.album_artist.as_deref())
                .map(|artist| (artist.into(), room.paper_dim, None));
            let context = item
                .album
                .as_deref()
                .map(|album| (album.into(), None, width >= theme::PLAYLIST_BREAKPOINT));
            let track = crate::views::page::track_row(crate::views::page::TrackRow {
                marker,
                artwork: None,
                title: item.title.as_str().into(),
                ink: room.paper,
                under,
                context,
                duration: item
                    .duration
                    .map(crate::vm::format_duration)
                    .unwrap_or_default()
                    .into(),
                playing: false,
                selected: false,
                press: None,
            });
            composer = composer.push(
                row![
                    track,
                    crate::views::page::favourite_slot(
                        &item.path,
                        crate::app::is_favourite(shelf, &item.path),
                    ),
                    crate::views::page::icon_slot(
                        crate::icon::Glyph::ArrowUp,
                        "Move up",
                        position > 0,
                        true,
                        Message::VibePreviewShift(position, -1),
                    ),
                    crate::views::page::icon_slot(
                        crate::icon::Glyph::ArrowDown,
                        "Move down",
                        position + 1 < preview.items.len(),
                        true,
                        Message::VibePreviewShift(position, 1),
                    ),
                    crate::views::page::icon_slot(
                        crate::icon::Glyph::Close,
                        "Remove from mix",
                        true,
                        true,
                        Message::VibePreviewRemove(position),
                    ),
                ]
                .spacing(theme::GAP_XS)
                .align_y(iced::Alignment::Center),
            );
        }
        let can_act = !preview.items.is_empty();
        composer = composer.push(
            row![
                word_button_maybe("Play", can_act.then_some(Message::VibePlay)),
                word_button_maybe(
                    "Save playlist",
                    (can_act && save_enabled).then_some(Message::VibeSubmit),
                ),
                word_button_maybe(
                    "Another version",
                    (!state.request_changed()).then_some(Message::VibeAnother),
                ),
            ]
            .spacing(theme::GAP_SM),
        );
    } else if state.has_features() && !busy && state.open {
        composer = composer.push(
            text("No tracks could satisfy this request without breaking the diversity rules. Try a shorter mix or a broader description.")
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_dim),
        );
    }
    if state.has_features() && !busy {
        composer = composer.push(word_button("Refresh local analysis", Message::VibeAnalyze));
    }
    composer.spacing(theme::GAP_SM).width(Length::Fill).into()
}

/// Home keeps discovery, while the composer itself belongs to New playlist.
fn vibe_shortcut<'a>(available: bool) -> Element<'a, Message> {
    let room = theme::active();
    column![
        section_rule("Make a playlist"),
        text("Describe a journey and Baz will shape it from music on this device.")
            .size(theme::SIZE_BODY)
            .line_height(theme::LEADING_BODY)
            .color(room.paper),
        word_button_maybe(
            "Make a vibe playlist",
            available.then_some(Message::NewPlaylistOpenVibe),
        )
    ]
    .spacing(theme::GAP_SM)
    .into()
}

fn word_button<'a>(label: &str, message: Message) -> iced::widget::Button<'a, Message> {
    word_button_maybe(label, Some(message))
}

fn word_button_maybe<'a>(
    label: &str,
    message: Option<Message>,
) -> iced::widget::Button<'a, Message> {
    let room = theme::active();
    button(
        container(
            text(label.to_owned())
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .font(theme::MEDIUM),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .padding(theme::pad(0.0, theme::GAP_SM))
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press_maybe(message)
}

/// **What the `CONTINUE` band is a placard for**: the track to carry on with
/// and how far into it to carry on from — or `None` when there is nothing to
/// carry on with, or something is already sounding.
///
/// **One function with one answer**, deliberately. The band draws from the
/// live run when there is one and from the persisted snapshot only before
/// anything has played, and those are not two code paths that could disagree
/// about which record the listener is looking at — they are two arms of the
/// same `match`, in priority order:
///
/// 1. **Something is sounding** ([`Phase::Playing`]) — no band. This is the
///    whole of the owner's rule and it is read off the engine, so it is true
///    of every route into playback: the wall's hover `Play`, a playlist,
///    `Play all`, the bar, a media key, MPRIS.
/// 2. **The engine holds a track and is not playing it** — *paused*, and the
///    band describes **what you paused**, at the engine's own confirmed
///    position. Never the launch snapshot, which by then is describing the
///    start of this same track rather than where you actually are.
/// 3. **Something has sounded and the engine holds nothing** — the run
///    *ended*. **No band.** This is the one case the word "stopped" does not
///    settle on its own, and it goes the other way from a pause: a run you
///    played to the end has no "where you stopped", the needle would sit at a
///    finish, and the product's standing rules is emphatic that the queue empties and
///    the silence at the end of a run is a feature. An offer to carry on with
///    something you completed is the interface remembering something that is
///    over.
/// 4. **Nothing has sounded** — the run baz launched with, at the position it
///    was interrupted at ([`crate::session`]). The only moment the snapshot is
///    read, and `crate::app`'s `next_snapshot` guarantees it is still the file
///    baz opened: nothing this process writes can move it while nothing has
///    sounded, so what the band shows cannot drift under it.
///
/// Pure, and it takes the two values it reads rather than the shell, so the
/// rule is unit-testable without a window — which is what the tests below walk.
#[must_use]
pub(crate) fn standing<'a>(
    player: &'a PlayerState,
    resume: &'a crate::session::Snapshot,
) -> Option<(&'a std::path::Path, u64)> {
    if player.phase() == Phase::Playing {
        return None;
    }
    if let Some(path) = player.now_playing_path() {
        return Some((path, player.elapsed_ms()));
    }
    if player.has_sounded() {
        return None;
    }
    resume.current().map(|path| (path, resume.position_ms))
}

/// **`CONTINUE`** — the run to carry on with, drawn only in the silence.
///
/// The sleeve at [`theme::CONTINUE_SLEEVE`] beside a placard: the artist in
/// letterspaced caps, the work's title in [`theme::WORK_TITLE`], the condition
/// line, then the needle and what it is a needle into.
///
/// **Absent, not empty** (ADR-0030 §6): nothing to carry on with ([`standing`]),
/// or a track whose file the library no longer holds, and there is no band at
/// all. Nothing here draws a placeholder, because a placard about no work is
/// worse than no placard.
///
/// **The needle is static by construction.** [`standing`] answers `None` while
/// anything is playing, so the position this draws is one the engine has
/// stopped moving — which is why Home needs no clock, no timer and no
/// subscription of its own to keep it honest. It is never extrapolated, and
/// there is nothing here that could extrapolate it.
fn continue_band<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    resume: &'a crate::session::Snapshot,
    width: f32,
) -> Option<Element<'a, Message>> {
    let room = theme::active();
    let (path, elapsed) = standing(player, resume)?;
    // The record the interrupted track belongs to, found by path — the same
    // reconciliation every other reading of a queue position uses, and what
    // keeps the band true across a rescan that renumbered the run.
    let (album, track) = shelf.albums.iter().find_map(|album| {
        album
            .editions
            .iter()
            .flat_map(|edition| edition.tracks.iter())
            .find(|track| track.path == path)
            .map(|track| (album, track))
    })?;
    let edge = theme::CONTINUE_SLEEVE;
    let sleeve: Element<'a, Message> = match shelf.thumb(album.id) {
        Some(handle) => iced_image(handle.clone())
            .width(Length::Fixed(edge))
            .height(Length::Fixed(edge))
            .into(),
        None => gradient_block(album.id, edge, 1.0),
    };

    // The condition line: what the record is, in the album page's own
    // vocabulary and from the same view model — `1988 · FLAC · 16-bit ·
    // 44.1 kHz`, and each part absent when the scan did not read it.
    let edition = album.editions.first();
    let mut condition: Vec<String> = Vec::new();
    if let Some(year) = album.year {
        condition.push(year.to_string());
    }
    if let Some(edition) = edition {
        if let Some(format) = edition.key.0 {
            condition.push(format.name().to_owned());
        }
        if let Some(depth) = edition.bit_depth {
            condition.push(format!("{depth}-bit"));
        }
        if let Some(rate) = edition.sample_rate {
            condition.push(vm::format_sample_rate(rate));
        }
    }

    // **The length is the scan's reading, never the engine's**, even when the
    // band is describing a paused session that the engine could report one
    // for. Every other fact on this placard — the artist, the work, the
    // condition line — comes from the library's view model, and one figure
    // from the other side would let the length and the condition line
    // disagree about which file they are describing.
    let total = track
        .duration
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0);

    let mut placard = column![
        // The artist in letterspaced caps — the section rule's own voice, in
        // the placard's top line, which is where a wall label puts it.
        text(theme::tracked(&album.artist.label().to_uppercase()))
            .size(theme::SIZE_HEADING)
            .line_height(theme::LEADING_HEADING)
            .font(theme::MEDIUM)
            .color(room.paper_faint),
        // **The work's own title, in serif italic.** The one string in the
        // product that takes it; see [`theme::WORK_TITLE`].
        text(
            album
                .title
                .clone()
                .unwrap_or_else(|| "Unknown Album".into())
        )
        .size(theme::SIZE_TITLE)
        .line_height(theme::LEADING_TITLE)
        .font(theme::WORK_TITLE)
        .color(room.paper)
        .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_XS);
    if !condition.is_empty() {
        placard = placard.push(
            text(condition.join(" · "))
                .size(theme::SIZE_META)
                .line_height(theme::LEADING_META)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        );
    }
    placard = placard
        .push(Space::new().height(Length::Fixed(theme::GAP_SM)))
        .push(needle(elapsed, total, edge))
        .push(resume_line(player, track, elapsed, total));

    Some(
        column![
            section_rule("Continue"),
            container(
                row![sleeve, placard]
                    .spacing(theme::GAP_XL)
                    .align_y(iced::Alignment::Start)
            )
            .width(Length::Fixed(width.min(theme::ALBUM_BREAKPOINT))),
        ]
        .spacing(theme::GAP_LG)
        .into(),
    )
}

/// **The needle**: a 2 px hairline exactly the sleeve's width, amber up to the
/// elapsed fraction with a 1 px tick at the position, muted after.
///
/// It is drawn on the *placard*, at the sleeve's measure — not on the artwork.
/// That is the design's signature and it is a rule rather than a preference:
/// the product's standing rules forbids drawing on a work, and every product baz is
/// measured against puts this line across the bottom of the cover.
///
/// The amber is licensed: this is playback truth, which is the accent's one
/// meaning. The tick is what turns a proportion into a *position* — a bar
/// alone reads as "how much", and a mark on it reads as "where".
pub(crate) fn needle(elapsed_ms: u64, total_ms: u64, width: f32) -> Element<'static, Message> {
    let room = theme::active();
    let (filled, rest) = needle_runs(elapsed_ms, total_ms, width);
    let lane = |w: f32, colour: iced::Color, h: f32| {
        container(
            Space::new()
                .width(Length::Fixed(w))
                .height(Length::Fixed(h)),
        )
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(colour)),
            ..container::Style::default()
        })
        .into()
    };
    let mut parts: Vec<Element<'static, Message>> = Vec::new();
    if filled > 0.0 {
        parts.push(lane(filled, room.lamp, theme::NEEDLE_H));
    }
    // The tick: 1 px at the position, at the full accent, drawn taller than
    // the line so it reads as a mark *on* it rather than as a longer run of it.
    parts.push(lane(
        theme::NEEDLE_TICK_W,
        room.lamp_bright,
        theme::NEEDLE_H + theme::GAP_XS,
    ));
    if rest > 0.0 {
        parts.push(lane(rest, room.hairline_strong(room.wall), theme::NEEDLE_H));
    }
    container(
        iced::widget::Row::with_children(parts)
            .align_y(iced::Alignment::Center)
            .height(Length::Fixed(theme::NEEDLE_H + theme::GAP_XS)),
    )
    .width(Length::Fixed(width))
    .clip(true)
    .into()
}

/// `Resume · Anhydrous 2 · 3:12 of 6:27` — the verb, the track, and the
/// position in figures.
///
/// **`Resume` is the ordinary `Play`** (ADR-0030 §6), aimed at where the band
/// says you are: it is the one press that spends the position on the placard,
/// and it is the only thing on this page that starts audio.
///
/// **It is also the one play gesture in the product that navigates** — it
/// starts the run *and* goes to
/// [`Place::NowPlaying`](crate::place::Place::NowPlaying). Pressing `Play` on
/// the wall's hover options, on a record's page or in a playlist deliberately
/// moves you nowhere, and this one is not an inconsistency with them but the
/// difference between two verbs: those say *play this*, and answering them by
/// leaving the surface you are choosing from would be the interface taking the
/// wheel; `Resume` says *pick up where I left off*, and the place that
/// describes where you are is the answer to it rather than a side effect of
/// it. It is also what makes the band's disappearance coherent — you are not
/// left standing on Home watching a placard go, you are on the surface that
/// describes what is now sounding. See `crate::app`'s `App::resume_the_run`.
fn resume_line<'a>(
    player: &'a PlayerState,
    track: &'a vm::TrackVm,
    elapsed: u64,
    total: u64,
) -> Element<'a, Message> {
    let room = theme::active();
    let figures = format!(
        "{} of {}",
        vm::format_duration(std::time::Duration::from_millis(elapsed)),
        vm::format_duration(std::time::Duration::from_millis(total)),
    );
    let verb = button(
        container(
            row![
                iced_image(icon::handle(icon::Glyph::Play))
                    .width(Length::Fixed(theme::ICON_PX))
                    .height(Length::Fixed(theme::ICON_PX)),
                text("Resume")
                    .size(theme::SIZE_META)
                    .line_height(theme::LEADING_META)
                    .font(theme::MEDIUM)
                    .wrapping(text::Wrapping::None),
            ]
            .spacing(theme::GAP_SM)
            .align_y(iced::Alignment::Center),
        )
        .height(Length::Fill)
        .align_y(alignment::Vertical::Center),
    )
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .padding(theme::pad(0.0, theme::GAP_SM))
    .style(move |_theme, status| theme::word_button(room, room.wall, status))
    .on_press_maybe(player.engine_ready().then_some(Message::ResumeRun));
    row![
        verb,
        text(format!("{} · {figures}", track.title))
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    ]
    .spacing(theme::GAP_SM)
    .align_y(iced::Alignment::Center)
    .into()
}

/// **`ALL SONGS`, as a tile** — the owner, 2026-08-10: *"again I wanted the
/// Play all, to be more like a tile on the home screen, a special
/// 'playlist'"*, and the *again* is the point: it had been asked for before and
/// not built.
///
/// # It wears a list's sleeve, not a designed one
///
/// The collage ([`crate::views::playlist_sleeve`]) — four quotations from the
/// records the list holds, in the list's own order. The alternative was a
/// designed face of its own, on the argument that four arbitrary covers claim to
/// characterise a list whose whole definition is *no selection at all*, and that
/// they re-shuffle whenever the wall is re-arranged.
///
/// **It is refused because this list already has a face.** The playlist panel's
/// `All songs` row draws exactly this collage (shipped 2026-08-10), and a second
/// face for one list is a worse fault than a restless one: a listener would have
/// to learn twice that these are the same thing. `crate::implicit`'s own words
/// settle it — *an implicit list is a list and gets a list's sleeve*. What the
/// collage is arbitrary about, the caption states exactly.
///
/// A typographic face was the other candidate and it fails on a second count:
/// the figures it would carry — records, songs, running time — are the
/// `COLLECTION` footer's own, three sections down the same page, which is doc 07
/// L8.6's one fact drawn twice.
///
/// # Where it sits, and why
///
/// **Second: under `CONTINUE`, above `RECENTLY ADDED`.** The page is ordered by
/// how *particular* its offers are, not by how large:
///
/// - `CONTINUE` is your own interrupted run — the most specific answer to *what
///   now*, and absent most of the time, which is why it leads when it is there
///   rather than taking the room when it is not.
/// - **`All songs` is the broadest way in and it is always there.** With
///   `CONTINUE` absent — the ordinary state of this page — it is the first thing
///   on it, which is right for a door. With `CONTINUE` standing, your own run
///   leads and the whole collection follows it.
/// - `RECENTLY ADDED` is a narrower, more particular offer than *all of it*, and
///   it is the section that changes.
/// - `COLLECTION` stays last: it is the one section you read rather than press.
///
/// **No section rule over it.** A rule names a *set* of things; this is one
/// thing and it names itself in its own caption, where `All songs` under a rule
/// reading `ALL SONGS` would be the word twice. It stands on the same grid as
/// the row below — one column of the wall's own tile — so it rhymes with
/// `RECENTLY ADDED` without pretending to be a section.
///
/// # And the strip keeps its `Play all`
///
/// Both stay, and they are not the same control at a different size. `Play all`
/// lives in the Library strip **beside the query and the arrangement that decide
/// the wall**, and its contract is *exactly what you can see, in the order you
/// can see it* — it is the only way to play seven search results. This tile is
/// on a page that shows no wall and no query, so it plays the collection whole
/// (`crate::implicit::ImplicitList::everything`) rather than applying a filter
/// the listener cannot see or clear from where they are standing.
///
/// So: one list, one origin, one sleeve, one `Play`, two scopes — and each
/// states its own scope where it stands. `ACTS_W` is untouched, because nothing
/// left the strip.
///
/// **Absent, not empty**, like every other band here: no records, no tile. A
/// door into nothing is worse than no door.
fn all_songs_tile<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    hang: Grid,
) -> Option<Element<'a, Message>> {
    let list = shelf.everything();
    crate::views::list_tile::view(
        shelf,
        player,
        hang,
        &list,
        shelf.hovered_all_songs,
        crate::views::list_tile::Actions {
            content: Content::AllSongs,
            play: Message::PlayEverything,
            open: Some(Message::ShowAllSongs),
            enter: Message::AllSongsHovered(true),
            exit: Message::AllSongsHovered(false),
        },
    )
}
/// **`RECENTLY ADDED`** — one row of records by `first_seen_ns`, newest first,
/// in **the wall's own tile**.
///
/// Not a second tile design: `views::shelf::tile` is called with the wall's
/// own [`Grid`], so the sleeve, the caption, the playing mark and the hover
/// options are the wall's, to the pixel. A record behaves the same wherever it
/// is drawn, which is what makes a second surface showing records affordable
/// at all.
///
/// **Absent, not empty**: a library with fewer records than a row has columns
/// has nothing to say that the wall one press away does not say better.
/// **The row `RECENTLY ADDED` draws**, resolved: newest `first_seen_ns` first,
/// ties by title, one row's worth — or empty when the library has fewer
/// records than a row has columns.
///
/// Shared with the shell, which asks for the same ids to decode their art:
/// the wall's prefetch is a range over the wall, and a record drawn *beside*
/// the wall is not in it (`Shelf::request_thumbs_for`). Two answers to "which
/// records does Home show" could disagree, and the one that disagreed would be
/// the one whose covers never arrived.
///
/// The tie-break is the returns lane's own total-order rule, for the reason it
/// has there: two launches over the same library must draw the same row.
pub(crate) fn newest(shelf: &Shelf, hang: Grid) -> Vec<&vm::AlbumVm> {
    let columns = hang.columns;
    let mut newest: Vec<&vm::AlbumVm> = shelf
        .albums
        .iter()
        .filter(|album| album.first_seen_ns.is_some())
        .collect();
    if newest.len() < columns {
        return Vec::new();
    }
    newest.sort_by(|a, b| {
        b.first_seen_ns
            .cmp(&a.first_seen_ns)
            .then_with(|| a.title.cmp(&b.title))
    });
    newest.truncate(columns);
    newest
}

fn recently_added<'a>(
    shelf: &'a Shelf,
    player: &'a PlayerState,
    hang: Grid,
    collecting: crate::playlists::Collecting,
) -> Option<Element<'a, Message>> {
    let newest = newest(shelf, hang);
    if newest.is_empty() {
        return None;
    }
    let mut tiles = row![].spacing(hang.gutter);
    for album in newest {
        tiles = tiles.push(crate::views::shelf::tile(
            shelf, player, hang, album, 0.0, collecting,
        ));
    }
    Some(
        column![crate::views::section_rule("Recently added"), tiles]
            .spacing(theme::GAP_LG)
            .into(),
    )
}

/// **`COLLECTION`** — four figures about the library, at the foot of the page.
///
/// The owner's ask: *"the album and track count below the search bar doesn't
/// look good… maybe this should go into the home as some basic stats?"*. The
/// counts came off the search well ([`crate::views::lane`]) and arrived here,
/// and this is what they became.
///
/// # Where it sits, and why that is the foot
///
/// **Last.** Home's job is to put you back into music: `CONTINUE` is the one
/// thing on this page you press, and `RECENTLY ADDED` is a row of records you
/// can start from. Neither may be pushed down by an inventory. The figures are
/// something you read *once in a while* — how big has this got, how much of it
/// have I never got to the end of — and a fact you consult occasionally goes
/// where the page ends, not where the eye lands. It is also the only section
/// here that is pure statement: nothing in it is pressable, so putting it above
/// two sections that are would be leading with the part you cannot use.
///
/// # The four, and the ones that were cut
///
/// Kept — records, the people who made them, the files, and the time
/// ([`vm::Collection`]):
///
/// ```text
///   25          9          206         14 hours
///   ALBUMS      ARTISTS    TRACKS      OF MUSIC
/// ```
///
/// It reads as a sentence about a collection rather than as a table: *25
/// albums, 9 artists, 206 tracks, 14 hours of music*. Cut, and each for a
/// reason rather than for room:
///
/// - **When the collection was last added to.** `RECENTLY ADDED` is drawn one
///   section above this one, and it says the same thing with covers. One fact
///   drawn twice is doc 07 L8.6's test, and the section that already passes it
///   keeps the fact.
/// - **How many records have never been played.** A figure about the
///   *listener*, not about the collection — it is read out of the play ledger
///   and it would change while you sat looking at it. ADR-0030 §6 refuses
///   every engagement statistic, and this is the one on the list that is easy
///   to mistake for an inventory fact.
/// - **The library's size on disk.** True, cheap and dull: it is a fact about
///   a filesystem, and nothing you would do differently having read it. The
///   `Details` block on a record's page is where bytes belong.
///
/// # Absent, not empty
///
/// No records, no section — the same rule the two bands above it keep. A
/// `COLLECTION` heading over four zeroes would be the page reporting on a
/// library that does not exist yet, and the first-run page already has one
/// line that says the right thing.
fn collection(shelf: &Shelf) -> Option<Element<'static, Message>> {
    let counted = shelf.collection;
    if counted.albums == 0 {
        return None;
    }
    let mut cells = row![].spacing(theme::GAP_XL);
    for (figure, label) in [
        (counted.albums.to_string(), "ALBUMS"),
        (counted.artists.to_string(), "ARTISTS"),
        (counted.tracks.to_string(), "TRACKS"),
        (playing_time(counted.playing_ms), "OF MUSIC"),
    ] {
        cells = cells.push(stat(figure, label));
    }
    Some(
        column![section_rule("Collection"), cells]
            .spacing(theme::GAP_LG)
            .into(),
    )
}

/// One cell of the `COLLECTION` footer: the figure over its word.
///
/// The figure takes the **emphasis** size — one step above the body, which is
/// the room's *quiet prominence* and not a headline; the word takes the section
/// heading's own voice, tracked caps at the smallest size in the scale, in
/// [`theme::Palette::paper_faint`]. So a cell is the page's own two smallest
/// voices stacked, and the block reads as a footnote with structure rather
/// than as a dashboard tile: no card, no rule, no colour, nothing pressable.
///
/// [`theme::STAT_W`] is a **pitch**: every cell is that wide whatever is in
/// it, so the four figures stand on one lattice and the row reads left to
/// right as a sentence. `font.rs` measures both lines of every cell against
/// it.
fn stat(figure: String, label: &'static str) -> Element<'static, Message> {
    let room = theme::active();
    container(
        column![
            text(figure)
                .size(theme::SIZE_EMPHASIS)
                .line_height(theme::LEADING_EMPHASIS)
                .font(theme::MEDIUM)
                .color(room.paper)
                .wrapping(text::Wrapping::None),
            text(theme::tracked(label))
                .size(theme::SIZE_HEADING)
                .line_height(theme::LEADING_HEADING)
                .font(theme::MEDIUM)
                .color(room.paper_faint)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(theme::GAP_XXS),
    )
    .width(Length::Fixed(theme::STAT_W))
    .clip(true)
    .into()
}

/// How long the collection is, in **one** unit: `14 hours`, `38 days`,
/// `42 minutes`.
///
/// One unit rather than two, because the figure is a sense of scale and not a
/// duration you are going to act on — `38 days 4 hours` is a stopwatch reading
/// for a fact whose interesting digit is the first one. The unit is the
/// largest that gives a whole number, and the singular is spelled rather than
/// left as `1 days`.
///
/// It is **not** [`vm::format_duration`], which sets a position inside a track
/// as `3:12` — a clock face for a collection would be six-digit nonsense.
fn playing_time(ms: u64) -> String {
    let plural = |n: u64, unit: &str| {
        if n == 1 {
            format!("{n} {unit}")
        } else {
            format!("{n} {unit}s")
        }
    };
    let seconds = ms / 1_000;
    let days = seconds / 86_400;
    if days > 0 {
        return plural(days, "day");
    }
    let hours = seconds / 3_600;
    if hours > 0 {
        return plural(hours, "hour");
    }
    plural(seconds / 60, "minute")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use baz_core::protocol::Event;

    use super::*;
    use crate::player::Availability;

    const FIRST: &str = "/m/Anhydrous 1.flac";
    const SECOND: &str = "/m/Anhydrous 2.flac";
    const ELSEWHERE: &str = "/m/Some Other Record.flac";

    /// The interrupted run baz launched with: three tracks, stopped 3:12 into
    /// the second.
    fn interrupted() -> crate::session::Snapshot {
        crate::session::Snapshot {
            paths: vec![
                PathBuf::from(FIRST),
                PathBuf::from(SECOND),
                PathBuf::from("/m/Anhydrous 3.flac"),
            ],
            cursor: 1,
            position_ms: 192_000,
            provenance: None,
            assembled: false,
        }
    }

    fn started(path: &str) -> Event {
        Event::TrackStarted {
            path: PathBuf::from(path),
            position: 0,
        }
    }

    /// A shell at launch: an engine, and nothing has sounded through it.
    fn at_launch() -> PlayerState {
        PlayerState::new(Availability::Ready)
    }

    #[test]
    fn new_playlist_owns_the_composer_and_home_keeps_only_its_shortcut() {
        let source = include_str!("home.rs");
        let body = source
            .split("fn vibe_creator")
            .nth(1)
            .expect("composer")
            .split("fn word_button")
            .next()
            .expect("composer body");
        for required in [
            "Make a mix",
            "Try “dreamy shoegaze for a rainy evening”",
            "VibePrompt",
            "Create mix",
            "Analyse locally & create",
            "Save playlist",
            "Another version",
        ] {
            assert!(body.contains(required), "composer lost {required:?}");
        }
        assert!(!body.contains("Make a sonic playlist"));
        assert!(!body.contains("Use sounding track as anchor"));
        let home = source
            .split("fn vibe_shortcut")
            .nth(1)
            .expect("Home shortcut")
            .split("fn word_button")
            .next()
            .expect("shortcut body");
        assert!(home.contains("Make a vibe playlist"));
        assert!(!home.contains("VibePrompt"));
    }

    /// **The band stands on the interrupted run until something sounds**, and
    /// the moment anything does, it is gone.
    ///
    /// The owner's rule from the near side: *"once you select resume, it just
    /// disappears"*. What the band shows before the press is the snapshot's
    /// own record at the snapshot's own position — the launch state, and the
    /// only state in which the file is read at all.
    #[test]
    fn the_band_stands_on_the_interrupted_run_until_something_sounds() {
        let resume = interrupted();
        let mut player = at_launch();
        assert_eq!(
            standing(&player, &resume),
            Some((Path::new(SECOND), 192_000)),
            "at launch the band is the interrupted run, at its stored position"
        );

        player.apply(&started(SECOND), &[]);
        assert_eq!(
            standing(&player, &resume),
            None,
            "something is sounding, so the question the band asks is not one \
             to ask"
        );
    }

    /// **Every route into playback takes the band away**, because the rule is
    /// read off the engine rather than off the gesture.
    ///
    /// The wall's hover `Play`, a playlist, `Play all`, the bar, a media key,
    /// MPRIS — none of them is named here and none of them has to be: they all
    /// end in one [`Event::TrackStarted`], and a track that has nothing to do
    /// with the snapshot takes the band away exactly as the snapshot's own
    /// does. Nothing in [`standing`] compares the two paths.
    #[test]
    fn playback_from_anywhere_at_all_takes_the_band_away() {
        let resume = interrupted();
        let mut player = at_launch();
        player.apply(&started(ELSEWHERE), &[]);
        assert_eq!(standing(&player, &resume), None);
    }

    /// **A pause brings the band back describing what you paused** — not the
    /// launch snapshot, which by then names the start of the track you are
    /// halfway through.
    ///
    /// The owner's rule from the far side: *"it just reappears when you stop
    /// the player"*. This is the case that makes the band useful after every
    /// stop rather than only after a launch, and it is why the content comes
    /// from the live run whenever there is one.
    #[test]
    fn a_pause_brings_the_band_back_describing_what_was_paused() {
        let resume = interrupted();
        let mut player = at_launch();
        player.apply(&started(ELSEWHERE), &[]);
        player.apply(
            &Event::Progress {
                elapsed_ms: 45_000,
                track_ms: Some(300_000),
            },
            &[],
        );
        player.apply(&Event::Paused, &[]);
        assert_eq!(
            standing(&player, &resume),
            Some((Path::new(ELSEWHERE), 45_000)),
            "the band describes the paused run at the engine's own position, \
             and the launch snapshot is not consulted once anything has sounded"
        );

        player.apply(&Event::Resumed, &[]);
        assert_eq!(standing(&player, &resume), None, "sounding again");

        // …and it comes back on the next pause, at the next position.
        player.apply(
            &Event::Progress {
                elapsed_ms: 61_000,
                track_ms: Some(300_000),
            },
            &[],
        );
        player.apply(&Event::Paused, &[]);
        assert_eq!(
            standing(&player, &resume),
            Some((Path::new(ELSEWHERE), 61_000))
        );
    }

    /// **A run that finished is not a run to carry on with.**
    ///
    /// The one case the word *stopped* does not settle on its own, and it goes
    /// the other way from a pause. A run played to its end has no "where you
    /// stopped"; the product's standing rules calls the silence at the end of a run a
    /// feature, and an offer to carry on with something you completed is the
    /// interface remembering something that is over. The snapshot is *not*
    /// fallen back on here — that is what [`PlayerState::has_sounded`] is for,
    /// since the phase, the queue and the playing row look the same in this
    /// state as they do at launch.
    #[test]
    fn a_run_that_finished_is_not_a_run_to_carry_on_with() {
        for ending in [Event::QueueEnded, Event::Stopped] {
            let resume = interrupted();
            let mut player = at_launch();
            player.apply(&started(SECOND), &[]);
            player.apply(&ending, &[]);
            assert_eq!(
                standing(&player, &resume),
                None,
                "{ending:?} left an offer to replay a run that is over"
            );
            assert!(
                !resume.is_empty() && resume.current().is_some(),
                "…and the snapshot is still perfectly readable, which is the \
                 point: only `has_sounded` tells this state from a launch"
            );
        }
    }

    /// **Nothing to carry on with is a state**, and the band is absent rather
    /// than empty (ADR-0030 §6). A fresh install, a snapshot whose cursor
    /// fell outside its run, and an engine that never started are all it.
    #[test]
    fn nothing_to_carry_on_with_is_a_state() {
        assert_eq!(
            standing(&at_launch(), &crate::session::Snapshot::default()),
            None,
            "a fresh install"
        );
        let mut past_the_end = interrupted();
        past_the_end.cursor = 9;
        assert_eq!(
            standing(&at_launch(), &past_the_end),
            None,
            "a cursor outside the run names no track"
        );
    }

    /// **Home has nothing to animate.** [`standing`] answers `None` for every
    /// state in which the engine is moving a position, so the needle this page
    /// draws is always one that has stopped — which is why Home adds no
    /// timer, no clock and no subscription of its own, and why the position it
    /// draws can never be an extrapolation.
    ///
    /// The claim is checked rather than asserted in prose: the only phase in
    /// which a position advances is [`Phase::Playing`], and there is no
    /// snapshot and no engine state that produces a band in it.
    #[test]
    fn the_band_is_never_on_screen_while_a_position_is_moving() {
        let resume = interrupted();
        let mut player = at_launch();
        for event in [
            started(SECOND),
            Event::Progress {
                elapsed_ms: 1_000,
                track_ms: Some(300_000),
            },
            Event::Paused,
            Event::Resumed,
            started(ELSEWHERE),
            Event::Paused,
            Event::Resumed,
            Event::QueueEnded,
        ] {
            player.apply(&event, &[]);
            if player.phase() == Phase::Playing {
                assert_eq!(
                    standing(&player, &resume),
                    None,
                    "{event:?} left a static needle on screen while the engine \
                     was moving the position behind it"
                );
            }
        }
    }

    /// **The needle is exactly the sleeve's width, at every position.**
    ///
    /// That is the rule the whole band's composition rests on: the placard's
    /// line and the artwork beside it are one measure, so a needle that
    /// rounded its way to 131 or 133 px would break the alignment the eye
    /// actually reads. Swept at 1 ms over a real track's length.
    #[test]
    fn the_needle_is_the_sleeves_measure_at_every_position() {
        let width = theme::CONTINUE_SLEEVE;
        let total = 387_000_u64;
        for elapsed in (0..=total).step_by(97) {
            let (filled, rest) = needle_runs(elapsed, total, width);
            assert!(filled >= 0.0 && rest >= 0.0, "{elapsed} ms");
            assert!(
                (filled + rest + theme::NEEDLE_TICK_W - width).abs() < 0.001,
                "{elapsed} ms: {filled} + {rest} + tick != {width}"
            );
        }
    }

    /// The two ends, and they are the ends: nothing amber at the start, and
    /// nothing muted at the finish.
    #[test]
    fn the_needle_starts_empty_and_finishes_full() {
        let width = theme::CONTINUE_SLEEVE;
        let (filled, rest) = needle_runs(0, 100_000, width);
        assert!((filled - 0.0).abs() < f32::EPSILON);
        assert!((rest - (width - theme::NEEDLE_TICK_W)).abs() < 0.001);

        let (filled, rest) = needle_runs(100_000, 100_000, width);
        assert!((filled - (width - theme::NEEDLE_TICK_W)).abs() < 0.001);
        assert!((rest - 0.0).abs() < 0.001);
    }

    /// **`All songs` is a tile on Home, and it is the wall's own tile** — the
    /// owner, 2026-08-10: *"again I wanted the Play all, to be more like a tile
    /// on the home screen, a special 'playlist'"*.
    ///
    /// Pinned over this file's source, the way the footer below is and for the
    /// same reason: there is no `Shelf` to construct without a database and a
    /// scan thread, and every claim here is about what this function builds.
    /// Each is named by the literal a reviewer would have to move.
    #[test]
    fn all_songs_is_a_tile_in_the_walls_own_anatomy() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/home.rs"),
        )
        .expect("this file")
        .replace("\r\n", "\n");
        let shipped = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        let tile = shipped
            .split_once("fn all_songs_tile<'a>(")
            .expect("the tile")
            .1;
        let tile = &tile[..tile.find("\n}\n").expect("a function ends")];
        let anatomy = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/list_tile.rs"),
        )
        .expect("the shared list tile")
        .replace("\r\n", "\n");

        // **It plays everything you own**, not whatever the wall is filtered
        // to: Home shows no wall and no query, so a filter set on another page
        // has nothing on screen to be read from.
        assert!(
            tile.contains("shelf.everything()"),
            "the tile reads a scope it has nowhere to show"
        );
        assert!(
            tile.contains("Message::PlayEverything"),
            "the tile's press no longer plays the list"
        );
        // **A list's sleeve, because an implicit list is a list.** The panel's
        // `All songs` row draws this same collage, and one list has one face.
        assert!(
            anatomy.contains("crate::views::playlist_sleeve("),
            "the tile grew a second face for a list that already has one"
        );
        // **The wall's own tile anatomy**, to the token: the grid's art edge,
        // the sleeve inside its mat, the two-lane caption box, the state rule.
        for token in [
            "hang.art",
            "theme::SLEEVE_MAT",
            "theme::sleeve_mat(room)",
            "theme::CAPTION_LINE_H",
            "theme::CAPTION_H",
            "crate::views::shelf::state_rule(",
        ] {
            assert!(
                anatomy.contains(token),
                "the tile stopped standing in the wall's anatomy: `{token}`"
            );
        }
        // **The wall's own hover layer**, built by the wall's own function —
        // and two options rather than four, because the two it does not have
        // are the two an implicit list cannot answer.
        assert!(
            anatomy.contains("crate::views::shelf::veil(") && anatomy.contains("stack!["),
            "the tile draws a hover layer of its own instead of the wall's"
        );
        assert!(
            anatomy.contains("\"Play\"") && anatomy.contains("\"Open\""),
            "the tile lost one of its two options"
        );
        assert!(
            !anatomy.contains("\"Add to…\""),
            "the veil offers to add to a list with no file behind it"
        );
        // **Absent, not empty** — the rule every band on this page keeps.
        assert!(
            anatomy.contains("if list.is_empty() {\n        return None;"),
            "an empty library still draws a door into nothing"
        );

        // **Second on the page**: under CONTINUE, above RECENTLY ADDED. Ordered
        // by how particular each offer is, not by how large.
        let view = shipped
            .split_once("pub(crate) fn view<'a>(")
            .expect("the page")
            .1;
        let drawn = |guard: &str| view.find(guard).unwrap_or_else(|| panic!("{guard}"));
        assert!(
            drawn("if let Some(band) = continuing {") < drawn("if let Some(band) = everything {")
                && drawn("if let Some(band) = everything {") < drawn("if let Some(band) = added {"),
            "the tile is not between CONTINUE and RECENTLY ADDED"
        );
        // …and it counts towards the page having something to say.
        assert!(
            view.contains("everything.is_none()"),
            "a library with only this tile could draw the empty-page line"
        );
    }

    /// **The collection's counts are the Home place's footer now** — the far
    /// half of the move the owner asked for, whose near half is
    /// [`crate::views::lane`]'s well losing its second line.
    ///
    /// *"the album and track count below the search bar doesn't look good…
    /// maybe this should go into the home as some basic stats?"* The retired
    /// readout pinned two format strings; this pins what replaced the resting
    /// one — four figures, their four words, and the fact that they are drawn
    /// **after** the two sections you can act on.
    #[test]
    fn the_collection_is_the_pages_footer_and_its_four_figures() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views/home.rs"),
        )
        .expect("this file")
        .replace("\r\n", "\n");
        let shipped = source
            .split("#[cfg(test)]")
            .next()
            .expect("a source has a head");
        let band = shipped
            .split_once("fn collection(shelf: &Shelf)")
            .expect("the collection band")
            .1;
        let band = &band[..band.find("\n}\n").expect("a function ends")];
        for figure in ["ALBUMS", "ARTISTS", "TRACKS", "OF MUSIC"] {
            assert!(
                band.contains(figure),
                "the collection footer lost its {figure:?} cell"
            );
        }
        assert!(
            band.contains("counted.albums") && band.contains("counted.tracks"),
            "the two figures that came off the search well are not the two \
             the footer states"
        );
        assert!(
            band.contains("if counted.albums == 0 {\n        return None;"),
            "a library with no records still draws a COLLECTION heading — a \
             section is absent, not empty"
        );
        // **Last on the page.** Home is a door back into music; an inventory
        // must not push the one thing you press down the page.
        let view = shipped
            .split_once("pub(crate) fn view<'a>(")
            .expect("the page")
            .1;
        let drawn = |guard: &str| view.find(guard).unwrap_or_else(|| panic!("{guard}"));
        assert!(
            drawn("if let Some(band) = continuing {") < drawn("if let Some(band) = counted {")
                && drawn("if let Some(band) = added {") < drawn("if let Some(band) = counted {"),
            "the COLLECTION footer is drawn above CONTINUE or RECENTLY ADDED — \
             it is the page's last section, not its first"
        );
        // …and the page still says one plain line when it has nothing at all,
        // which the footer must be part of deciding.
        assert!(
            view.contains("counted.is_none()"),
            "an empty library could now draw an empty page rather than its one \
             line, because the footer is not in the `nothing` predicate"
        );
    }

    /// **The playing time is one unit, and the singular is spelled.**
    ///
    /// A collection's length is a sense of scale, not a stopwatch reading, so
    /// the unit is the largest that gives a whole number and there is no
    /// second term after it. `1 days` would be the sort of thing that makes a
    /// page feel machine-written.
    #[test]
    fn the_playing_time_is_one_unit_and_reads_as_english() {
        const MINUTE: u64 = 60_000;
        const HOUR: u64 = 60 * MINUTE;
        const DAY: u64 = 24 * HOUR;
        assert_eq!(playing_time(0), "0 minutes");
        assert_eq!(playing_time(MINUTE), "1 minute");
        assert_eq!(playing_time(42 * MINUTE), "42 minutes");
        assert_eq!(playing_time(HOUR), "1 hour");
        assert_eq!(playing_time(HOUR - 1), "59 minutes", "no rounding up");
        assert_eq!(playing_time(14 * HOUR), "14 hours");
        assert_eq!(playing_time(DAY), "1 day");
        assert_eq!(playing_time(38 * DAY + 7 * HOUR), "38 days", "one unit");
        // The figure the cell reserves for: nothing here can produce two.
        assert_eq!(playing_time(9_999 * DAY), "9999 days");
    }

    /// **The four figures are counted once, off the albums, and they are the
    /// four the footer states.**
    ///
    /// [`vm::Collection`] is what ADR-0030 §4's responsiveness contract makes
    /// necessary — the count is a walk of every track, so it happens where the
    /// albums are built and never in a frame. This is the arithmetic on its
    /// own: named artists only, case-folded, and an album owned in two formats
    /// counted once as a record and twice as files, because that is what it is.
    #[test]
    fn the_collection_counts_records_people_files_and_time() {
        use std::time::Duration;

        let track = |title: &str, secs: Option<u64>| vm::TrackVm {
            disc: None,
            number: None,
            title: title.to_owned(),
            artist: None,
            duration: secs.map(Duration::from_secs),
            path: std::path::PathBuf::from(format!("/m/{title}.flac")),
            bytes: None,
        };
        let edition = |tracks: Vec<vm::TrackVm>| vm::EditionVm {
            key: vm::EditionKey(None),
            detail: None,
            bitrate: None,
            bit_depth: None,
            sample_rate: None,
            replay_gain: vm::ReplayGainCoverage::default(),
            tracks,
        };
        let album = |artist: vm::AlbumArtistVm, editions: Vec<vm::EditionVm>| vm::AlbumVm {
            id: 0,
            title: Some("A record".to_owned()),
            track_artists_vary: false,
            artist,
            year: None,
            genre: None,
            first_seen_ns: None,
            first_track: std::path::PathBuf::from("/m/a.flac"),
            editions,
        };

        let albums = vec![
            // One record, owned twice: one album, two editions, six files.
            album(
                vm::AlbumArtistVm::Named("Boards of Canada".to_owned()),
                vec![
                    edition(vec![track("a", Some(600)), track("b", Some(300))]),
                    edition(vec![
                        track("c", Some(600)),
                        track("d", Some(300)),
                        track("e", None),
                        track("f", Some(0)),
                    ]),
                ],
            ),
            // The same artist, spelled differently: still one artist.
            album(
                vm::AlbumArtistVm::Named("boards of canada".to_owned()),
                vec![edition(vec![track("g", Some(1_800))])],
            ),
            // Neither of these names a person, so neither is counted as one.
            album(vm::AlbumArtistVm::Various, vec![edition(vec![])]),
            album(vm::AlbumArtistVm::Unknown, vec![edition(vec![])]),
        ];
        // `tracks` comes from the library rather than from this walk, so the
        // figure on Home and the figure the library reports are one number.
        let counted = vm::Collection::count(&albums, 7);
        assert_eq!(counted.albums, 4, "records, not editions and not files");
        assert_eq!(counted.artists, 1, "named, folded, and never a placeholder");
        assert_eq!(counted.tracks, 7, "the library's own figure, untouched");
        assert_eq!(
            counted.playing_ms, 3_600_000,
            "an unreadable duration contributes nothing rather than a guess"
        );
        assert_eq!(playing_time(counted.playing_ms), "1 hour");

        assert_eq!(
            vm::Collection::count(&[], 0),
            vm::Collection::default(),
            "no collection counts as nothing, which is what makes the footer \
             absent rather than four zeroes"
        );
    }

    /// **A track with no declared length reads as unstarted**, never as
    /// finished — an undeclared duration is the scan not having read one, and
    /// a full amber line would be the interface inventing a fact about a track
    /// it has not measured. A position past the end clamps rather than
    /// overrunning, which is the same rule from the other side.
    #[test]
    fn the_needle_invents_nothing_and_overruns_nothing() {
        let width = theme::CONTINUE_SLEEVE;
        for elapsed in [0, 1, 10_000, u64::MAX] {
            let (filled, rest) = needle_runs(elapsed, 0, width);
            assert!(
                (filled - 0.0).abs() < f32::EPSILON,
                "{elapsed} ms of nothing"
            );
            assert!((rest - (width - theme::NEEDLE_TICK_W)).abs() < 0.001);
        }
        let (filled, rest) = needle_runs(999_999, 1_000, width);
        assert!((filled - (width - theme::NEEDLE_TICK_W)).abs() < 0.001);
        assert!(rest >= 0.0, "the muted run never goes negative");
    }
}
