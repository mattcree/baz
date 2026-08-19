//! **The app bar** — the band across the top of the window, in every place,
//! identical (ADR-0040).
//!
//! The owner, 2026-08-10, in three messages: *"please remove the 'Play all'
//! button at the top of the library"*, *"and please put the display options at
//! the top bar"*, and *"we should have replaced the top window chrome with an
//! app bar which has this + settings + the window controls, the same on all
//! screens"*. This file is the third of those; the first two are removals
//! elsewhere that this bar is the destination for.
//!
//! # What it is, and what it is not
//!
//! It is **the window's own chrome**: the band a platform title bar occupies,
//! drawn by baz. It is not a toolbar and it is not a place's strip.
//!
//! # The admission rule is the owner's, in his words
//!
//! *"adding controls that apply to all windows makes sense in the top bar"*
//! (2026-08-10). On a single-window product that reads as **all places**, and
//! it is a test rather than a sentiment:
//!
//! > **A control enters this bar only if it applies in every place. If it
//! > applies to one place, the bar is not where it goes.**
//!
//! That decides the four tenants without appeal to taste — the window exists
//! in every place, Settings is the application's, the display options apply to
//! every surface that hangs works — and it decides `Play all` **out**, which
//! is what the owner asked for and now has a reason as well as an instruction:
//! it is the Library's and only the Library's.
//!
//! It also fixes what may **not** migrate in, which matters more, because the
//! failure mode of a resident bar is accretion one locally argued admission at
//! a time — exactly how the Library strip got crowded enough to complain
//! about. Search is the app-wide exception recorded in ADR-0040's 2026-08-12
//! amendment: it is resident in every place and overlays answers without
//! navigating away. A place's identity, breadcrumb and note are the place's. The
//! transport is the bottom bar's, whose ratchet says nothing leaves it either.
//! The wall's arrangement keys are the wall's. All of it is pinned in this
//! file's tests.
//!
//! # The zones, and the pattern that decides them
//!
//! The owner, approving the right-hand placement: *"I don't mind if we have
//! the controls on the right hand side as long as we have **a sensible
//! consistent pattern**"*. So the bar is seven named zones, in this order at
//! every width and in every place:
//!
//! ```text
//!   ▧ ‹›  [ Search library ]  ··········  ▤▤▤▤   🔔⚙   ─□✕
//!   1  2           3                4        5      6     7
//!  mark hist     search            handle   view   app   window
//! ```
//!
//! 1. **The application's mark.** What this window *is* — the one thing a title
//!    bar says that nothing else in baz says, and the only zone that is a
//!    statement rather than a control (L8.5). It was the word `baz` until the
//!    owner asked for the icon on 2026-08-10; [`mark`] records why it replaced
//!    the word rather than joining it.
//! 2. **History.** Browser-style Back/Forward over visited places, never
//!    track transport.
//! 3. **Search.** The application-wide chooser, resident and place-preserving.
//! 4. **The handle.** Never a tenant. It is the gesture surface: press and
//!    travel moves the window, press twice maximises it. A control admitted
//!    here would be a control that eats the window's own gesture.
//! 5. **The view** — controls that change *how the place you are in is shown*.
//!    The display options today.
//! 6. **The application** — doors to what the application holds rather than
//!    what a place does: the health bell and the gear, one cluster on
//!    [`theme::CONTROL_CLUSTER_GAP`]. They stood a `GAP_LG` apart — the seam
//!    *between* zones, spent inside one — until the owner read the bar:
//!    *"the top bar has weird spacing as well for icons/controls."*
//! 7. **The window** — controls that act on the window as an object of the
//!    desktop. Minimise, maximise, close, in that order, always.
//!
//! **The pattern is that scope widens rightward**: the view you are in, then
//! the application around it, then the window around that. It is a rule and
//! not a description, which is the test the owner set — *given a new control,
//! the rule says where it goes without an argument*. Ask what it acts on. One
//! place: it does not enter the bar. The view, in every place: zone 5. The
//! application: zone 6. The window: zone 7. Two people reading that cannot
//! disagree, which is what *"consistent pattern"* has to mean if it is to be
//! worth anything.
//!
//! This is also the answer to the standing complaint — *"just adding stuff
//! into that top bar isn't good… we need to have a proper think about how we
//! lay out controls and what is intuitive"* (2026-08-09). Nothing was added to
//! the crowded strip: the strip **lost two tenants**, and what arrived arrived
//! on a new surface with a stated admission rule and a stated order.
//!
//! # The buttons are on the right, always
//!
//! Read the platform's `button-layout` and mirror the bar was built and then
//! removed, on the owner's *"I don't mind if we have the controls on the right
//! hand side"*. Right-always is one fewer conditional in the layout, one fewer
//! subprocess at startup, and — the real argument — one arrangement rather
//! than two, which is what a *consistent* pattern means.
//!
//! **The known cost is macOS**, where the window buttons belong at the left
//! and this bar will look foreign. baz builds three platforms and lives on
//! Flathub; the owner has weighed that and declined the per-platform path.
//! ADR-0040 §4 records it, with the one-line reversal.
//!
//! # The display options keep the treatment they had
//!
//! The owner, looking at the shipped Library: *"the way they appear for the
//! library is nice"*. So the marks move **as they are** — the same four
//! sprites of the wall at its four hangs, the same [`theme::STEPPER_HIT`]
//! boxes, the same resting ink with the current step lifted, the same
//! tooltips. [`crate::views::density_marks`] is literally the same function
//! it was; what changed is where it is called from and which way it is laid.
//! If this bar could not have carried them as they are, that would have been a
//! fact about the bar to solve rather than a licence to redraw them.
//!
//! # One slot is reserved in every place, and the other is not
//!
//! [`theme::APP_BAR_MARKS_W`] is a fixed width, held whether or not anything is
//! drawn in it. That is what lets this bar be *the same bar* on all seven
//! places while still obeying ADR-0028's *absent, not disabled*: on a record's
//! page, a playlist's, Now playing and Settings there are no works to hang, so
//! there are no marks — and the gear does not move a pixel to notice.
//!
//! [`theme::APP_BAR_BUTTONS_W`] is a declaration of what the buttons *take*
//! when they are drawn, and it is **not** held open when they are not. The two
//! cases look alike and are not: the marks come and go *within a run*, as you
//! walk between places, so a collapsing slot would be the frame sliding under
//! the pointer; the buttons are settled once per process by `app::owns_chrome`,
//! so there is no frame in which they arrive and nothing can be seen to move.
//! Holding their 120 px would be dead gutter in every build that ships.
//!
//! What that does mean is that **the gear is the trailing control in one state
//! and not in the other**, so the bar's alignment rule is written over *the
//! trailing control* rather than over the gear — see the gutter note on the
//! band below, and `theme::the_bars_trailing_ink_lands_on_the_windows_gutter`.
//!
//! # The whole band is the handle
//!
//! `docs/BACKLOG.md` put it as *"dragging is what the gaps do, not what the
//! bar does"*, and the mechanism that delivers it is **capture**, not a hole
//! cut in the band: iced 0.13's `mouse_area` runs its content's handler first
//! and returns if the content took the press, so every button keeps its own
//! press and everything else — the gaps, the name, the empty slots — moves the
//! window.

use iced::widget::{
    Space, button, column, container, image as iced_image, mouse_area, row, rule, text, tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::{Message, Shelf};
use crate::motion::{Control, Ink};
use crate::{icon, theme};

/// The app bar, at `window_w`, over `place`'s answer to the display options.
///
/// `density` is `Some` exactly where the place hangs works — the Library, Home
/// and an artist's page — and `None` on the four that do not. It is the *place*
/// that answers, in `app.rs`, rather than this file deciding: which places hang
/// works is a fact about the composition, and a view file that guessed would be
/// a second answer to a question the shell already answers.
///
/// `maximized` swaps one glyph and one word ([`window_button`]); it is read
/// from the window rather than assumed, because a maximise button that still
/// says `Maximise` on a maximised window is the "stable in every state" clause
/// of the icon-only law (doc 10 §3.1) failing in the one state anybody checks.
///
/// `owns_chrome` is the shell's answer to *does baz draw this window's title
/// bar*. When it does not — which is what ships today — **the three window
/// buttons are absent**, because the platform is already drawing a set four
/// pixels above them. The owner, looking at exactly that: *"until we have no
/// window chrome, remove the window controls..."*. Their slot is not held
/// open: an empty reservation for a control that cannot exist in this state is
/// the "present and inert" failure this bar's own admission rule refuses, and
/// `owns_chrome` is settled once per process, so there is no frame in which
/// they arrive and nothing can be seen to move.
///
/// **This sentence used to end differently** — *"nothing to the left of them
/// moves when they appear"* — and that was simply false: with the slot not
/// held, everything to the left of them moves by 120 px when they appear, which
/// is the price of not holding it. Worse, it was false by a further 16 px in
/// the state that ships, because the row spent a seam on the `Space` that stood
/// in for them. Both are fixed above; the claim is now the one that is true.
#[expect(
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    reason = "the shell owns the app-bar facts and keeping them explicit makes its composition auditable"
)]
pub(crate) fn view(
    shelf: &Shelf,
    window_w: f32,
    density: Option<crate::shelf::Density>,
    visualization: Option<crate::visualizer::State>,
    can_back: bool,
    can_forward: bool,
    maximized: bool,
    owns_chrome: bool,
    over_field: bool,
    health: crate::health::Summary,
    ink: Ink,
) -> Element<'_, Message> {
    let room = theme::active();
    let name = mark();
    let history = history(can_back, can_forward, ink);
    let search = crate::views::search::well(shelf);
    // **Two zones, two seams.** The display options are the *view's*
    // (ADR-0040 §2 zone 3) and the bell and the gear are the *application's*
    // (zone 4), so the pair is one cluster on
    // [`theme::CONTROL_CLUSTER_GAP`] and the seam to the marks is the
    // between-zones [`theme::GAP_LG`]. Until 2026-08-15 all three sat at 16 —
    // the between-clusters number spent inside a cluster, which is why the
    // bell and the gear read as two marks adrift beside three tight window
    // buttons: the owner's *"the top bar has weird spacing as well for
    // icons/controls"*.
    let application = row![
        equalizer(ink),
        crate::views::status::bell(health),
        gear(ink)
    ]
    .spacing(theme::CONTROL_CLUSTER_GAP)
    .align_y(iced::Alignment::Center);
    let furniture = row![marks(density, visualization), application]
        .spacing(theme::GAP_LG)
        .align_y(iced::Alignment::Center);
    // Absent rather than disabled, and absent rather than a held slot: see the
    // note on `owns_chrome` above.
    //
    // **`None`, not a zero-width `Space`**, and the difference is 16 px of the
    // owner's complaint. `Row::spacing` puts a seam between every *pair of
    // children*, and a shrink-width `Space` is a child like any other: the bar
    // spent a `GAP_LG` on a placeholder for a control that was not there, so
    // the gear — the last thing actually drawn — stood at `W − HANG − GAP_LG`
    // while this file claimed it stood at `W − HANG`. `Row::push_maybe` pushes
    // nothing for a `None` (`iced_widget-0.13.4/src/row.rs:148`), so there is
    // no child and therefore no seam.
    let buttons: Option<Element<'static, Message>> =
        owns_chrome.then(|| window_controls(maximized, ink));
    // The bar's one flexible region. It is a plain `Space` and **not** a
    // handle of its own: the whole bar is the handle (see below), so a second
    // one here would be two answers to one gesture.
    let gap = Space::new().width(Length::Fill);
    // **Zones 1…7, in that order, at every width and in every place.** One
    // arrangement rather than a mirrored pair: the owner's *"as long as we
    // have a sensible consistent pattern"* is better served by one layout
    // everywhere than by two that are each correct on one platform.
    let mut line = row![name, history, search, gap, furniture];
    if let Some(buttons) = buttons {
        line = line.push(buttons);
    }
    let line = line.spacing(theme::GAP_LG).align_y(iced::Alignment::Center);
    // **The whole bar is the title bar's own gesture surface.** One
    // `mouse_area` around the band, not around the gap between its clusters:
    // iced 0.13's `mouse_area` runs its *content's* handler first and returns
    // if the content captured the press (`iced_widget-0.13.4/src/mouse_area.rs:211`),
    // so every button in the bar takes its own press and everything that is
    // not a button — the gaps, the window's name, the empty slots — drags the
    // window. That is the correct reading of `docs/BACKLOG.md`'s *"dragging is
    // what the gaps do, not what the bar does"*: the *rule* is about controls
    // keeping their presses, and the mechanism that delivers it is capture,
    // not a hole cut in the middle of the band.
    //
    // Getting this wrong is the one failure that would make the window
    // unusable rather than merely worse. A borderless window that cannot be
    // moved is broken; one that cannot be edge-resized is annoying.
    let band = mouse_area(
        container(line)
            // Resident chrome has a compact edge of its own; the collection's
            // 40 px `HANG` remains the wall/rail law. This bar spans the
            // **window**, not the body, so its edges are the window's.
            //
            // **The two horizontal gutters differ, and both put ink on the
            // compact app-bar edge.** Zone 1 fills its own box. Zones 3–5 hold
            // sprites centred in boxes twice their size, so trailing padding is
            // `APP_BAR_EDGE` *less* that
            // inset ([`theme::APP_BAR_HANG_R`]) — otherwise the box lands on the
            // line and the drawing lands 8 px inside it, which is the other
            // half of the 2026-08-10 defect. The arithmetic and the measurement
            // are in `theme::app_bar_pad`.
            .padding(theme::app_bar_pad())
            .width(Length::Fixed(window_w))
            // The now-playing bar's own ground. The window's two chrome bands
            // are one surface interrupted by the places between them, and
            // giving the top band a plane of its own would have made them two
            // ideas.
            //
            // **Except where a field runs behind it.** The owner: *"for the
            // top bar can you make sure it's transparent as well, or rather
            // not black."* On Now playing the page is a wash of the record's
            // own colour, and an opaque band across the top of it is a black
            // stripe laid over a picture — the bar reading as a lid rather
            // than as the top of the same room. Transparent there, and the
            // wash carries the whole height of the window.
            //
            // Everywhere else it keeps its ground: over a scrolling wall of
            // sleeves, a transparent bar is marks on top of artwork, and the
            // legibility that band is *for* goes with it.
            .style(move |_theme| {
                if over_field {
                    container::Style::default()
                } else {
                    theme::bar(room)
                }
            }),
    )
    // Press and drag moves the window; press twice quickly maximises or
    // restores it. Both are one message, because iced 0.13's `mouse_area` has
    // no `on_double_click` — that arrived in 0.14 — so the second press is
    // recognised by the shell against the first's clock (`app.rs`'s
    // `Message::WindowDragged`).
    .on_press(Message::WindowDragged)
    // The platform's own window menu, where the platform has one.
    .on_right_press(Message::WindowMenuRequested);
    // The seam belongs to the ground: with none, there is nothing for a
    // hairline to divide, and drawing one anyway would put the stripe back a
    // pixel at a time.
    if over_field {
        return band.into();
    }
    column![
        band,
        rule::horizontal(1).style(move |_theme| theme::hairline(room, room.wall)),
    ]
    .into()
}

/// Browser-style place history sits at the top-left beside the application
/// mark. It is intentionally separate from the bottom bar's track transport:
/// these arrows revisit pages, never audio.
fn history(can_back: bool, can_forward: bool, ink: Ink) -> Element<'static, Message> {
    row![
        history_button(
            icon::Glyph::HistoryBack,
            "Back",
            can_back.then_some(Message::HistoryBack),
            Control::HistoryBack,
            ink,
        ),
        history_button(
            icon::Glyph::HistoryForward,
            "Forward",
            can_forward.then_some(Message::HistoryForward),
            Control::HistoryForward,
            ink,
        ),
    ]
    .spacing(theme::CONTROL_CLUSTER_GAP)
    .align_y(iced::Alignment::Center)
    .into()
}

fn history_button(
    glyph: icon::Glyph,
    word: &'static str,
    message: Option<Message>,
    named: Control,
    ink: Ink,
) -> Element<'static, Message> {
    let room = theme::active();
    let enabled = message.is_some();
    let mark = container(
        iced_image(icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::glyph_ink(
                enabled,
                false,
                ink.hover(named),
                ink.pressed(named),
            )),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let control = button(mark)
        .width(Length::Fixed(theme::TRANSPORT_HIT))
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(0)
        .style(move |_theme, status| theme::transport(room, room.recess, status))
        .on_press_maybe(message);
    let named_control = tooltip(
        control,
        text(word)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room));
    mouse_area(named_control)
        .on_enter(Message::ControlEntered(named))
        .on_exit(Message::ControlLeft(named))
        .into()
}

/// **Zone 1 — the application's mark**, hanging from the window's leading
/// gutter in its [`theme::APP_BAR_NAME_W`] slot.
///
/// The owner, 2026-08-10: *"we probably want an icon for our app to show in the
/// bar"*. It replaces the word `baz`, which stood here at the metadata size in
/// the faintest readout ink — and *replaces* rather than joins, which is the
/// choice worth stating because the other one was available:
///
/// - **The slot declares the larger mark.** It is 32 for a 24 px mark and one
///   `GAP_SM`; the minimum-width budget still retains 96 px of drag slack.
/// - **They would say the same thing twice.** On a single-window product this
///   zone's content never varies — it is `baz` in every place, in every state,
///   forever — so it carries identity and nothing else, and a mark carries
///   identity better than a three-letter lowercase word at the faintest ink in
///   the room. It is the same reading that put a gear where the word `Settings`
///   used to be (doc 10 §3.4), arrived at from the other direction: there the
///   symbol had to earn a *door's* label, here there is no door and no label,
///   only a statement.
/// - **It is what the reference does.** The owner named it — *"similar to stuff
///   like spotify"* — and that window's chrome carries the mark, not the word.
///
/// **It is still a statement and not a control** (L8.5): no button, no tooltip,
/// no press of its own, which means it is 16 px more of the band that drags the
/// window rather than 16 px less. That is also why the icon-only law (doc 10
/// §3.1) does not reach it — the law is about a *control* naming itself with a
/// symbol, and this names nothing because it does nothing. The same reasoning
/// ADR-0040 §3 used to admit the three window buttons without a new licence.
///
/// What it costs is one accent that is not playback truth; [`icon::app_mark`]
/// states the exception and its boundary, and ADR-0040's amendment states the
/// reversal.
fn mark() -> Element<'static, Message> {
    container(
        iced_image(icon::app_mark())
            .width(Length::Fixed(theme::APP_MARK_PX))
            .height(Length::Fixed(theme::APP_MARK_PX)),
    )
    .width(Length::Fixed(theme::APP_BAR_NAME_W))
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    // **No lead of its own.** It carried a `GAP_MD`, which put the mark's ink
    // 12 px inside law L1's gutter — the one thing the doc comment above
    // claimed it did not do — and put its optical centre 10 px off the four
    // destination glyphs in the returns lane. The owner asked for that
    // alignment back (*"can we make the icon for the app align with the icons
    // in the sidebar"*), and with the slot now the mark's own size
    // ([`theme::APP_BAR_NAME_W`]) the container's edge *is* the ink's edge, so
    // the centre is `APP_BAR_EDGE + APP_MARK_PX / 2` = 32 — the lane's
    // [`theme::SIDEBAR_HEAD_GLYPH_X`] exactly, and asserted as such.
    .align_x(alignment::Horizontal::Left)
    .align_y(alignment::Vertical::Center)
    .into()
}

/// **The display options**, in their reserved slot: the density detents where
/// the place hangs works, and the slot's air where it does not.
///
/// The marks themselves are [`crate::views::density_marks`] — the same one
/// function, the same one [`Message::DensityStep`], so moving them here did not
/// make a second control (doc 07 L8.6). What moved is the *placement*: they
/// stood at the trailing edge of the block of works they hang — the index
/// rail's lane on the Library, a section rule on Home and an artist's page —
/// until the owner put them in the bar by instruction on 2026-08-10.
///
/// **Right-aligned in the slot**, so that the run's own right edge is fixed and
/// the gear beside it never moves; on the four places with nothing to draw, the
/// slot is [`Space`] of exactly the same width.
fn marks(
    density: Option<crate::shelf::Density>,
    visualization: Option<crate::visualizer::State>,
) -> Element<'static, Message> {
    let inner: Element<'static, Message> = match (density, visualization) {
        (Some(current), None) => crate::views::density_marks(current),
        (None, Some(current)) => crate::visualizer::marks(current),
        (None, None) => Space::new()
            .width(Length::Fixed(theme::APP_BAR_MARKS_W))
            .height(Length::Shrink)
            .into(),
        (Some(_), Some(_)) => {
            unreachable!("a place cannot have density and visualization controls")
        }
    };
    container(inner)
        .width(Length::Fixed(theme::APP_BAR_MARKS_W))
        .align_x(alignment::Horizontal::Right)
        .into()
}

/// **The equaliser's door** — the owner, 2026-08-18: *"this is not something
/// that should be buried in the settings. It should be accessible potentially
/// from anywhere maybe on the top bar."*
///
/// It stands in the application cluster with the bell and the gear, because
/// what it opens is a property of the *player* rather than of the place you
/// are in — the same argument that moved the gear here, applied to a control
/// that had been three scrolls into a settings section.
///
/// **No word beside it, and no lit state.** The mark is three faders that
/// disagree, which is what the panel behind it contains; and lighting it while
/// the equaliser is on would spend the lamp on something that is not playback
/// truth, which is the one thing the accent is reserved for. The panel's own
/// switch says whether it is on, in words, where the decision is made.
fn equalizer(ink: Ink) -> Element<'static, Message> {
    glyph_button(
        icon::Glyph::Equalizer,
        "Equaliser",
        Message::ToggleEqualizer,
        Control::Equalizer,
        ink,
    )
}

/// **The gear** — the route to the Settings place, moved off the Library strip
/// and into the bar, in the same corner it was already in.
///
/// It is the same control, the same message and the same anatomy as the strip's
/// was; what changed is that it is now in **every** place rather than only the
/// Library, which is what "the application's affair" always implied and what the
/// strip could not deliver. Doc 10 §3.4's licence for a bare symbol survives
/// intact and unamended — *"universal, **and top-right is its universal
/// position**"* — because this move keeps the corner. That is worth stating,
/// because the owner's earlier sketch (*"the settings on the left"*,
/// 2026-08-09) would have spent half of it, and `docs/BACKLOG.md` had already
/// priced what that would cost.
fn gear(ink: Ink) -> Element<'static, Message> {
    glyph_button(
        icon::Glyph::Gear,
        "Settings",
        Message::ToggleSettings,
        Control::Settings,
        ink,
    )
}

/// **The bar with the frame off**: the window's own controls, and nothing else.
///
/// The owner, on the first cut of chromeless mode: *"we should still keep the
/// window controls when we go into the 'full screen' mode."* He is right, and
/// the first version's answer to it — a lone mark drawn on the Now playing
/// page — was solving the wrong half. The problem was never *how do I get
/// back*; it was that on the platforms where baz draws its own title bar,
/// hiding that bar takes minimise, maximise and close with it, and a window
/// with no close button is not a mode, it is a trap.
///
/// So the strip stays and empties instead. What is left is the window's own
/// furniture plus the toggle that brought you here, which keeps the way out
/// where the way in was rather than somewhere new to learn.
///
/// It is **not** the ordinary bar with its tenants hidden: this is a separate,
/// shorter strip with no ground of its own, so the field and the sleeve run
/// under it and the only ink is the marks themselves.
pub(crate) fn chromeless(
    maximized: bool,
    owns_chrome: bool,
    near: bool,
    ink: Ink,
) -> Element<'static, Message> {
    let mut controls = row![crate::visualizer::chromeless_mark(true)]
        .spacing(theme::CONTROL_CLUSTER_GAP)
        .align_y(iced::Alignment::Center);
    if owns_chrome {
        controls = controls.push(window_controls(maximized, ink));
    }
    // `theme::app_bar_pad` rather than a spelling of its own, so the marks sit
    // on exactly the verticals the framed bar puts them on and taking the
    // frame away moves nothing sideways — the owner's *"make sure the padding
    // is the same for the window controls"*.
    //
    // **The fade-in on approach is not built yet.** `near` is threaded and
    // ignored: the same ask says these should be invisible until the pointer
    // comes near, and doing that honestly means an opacity factor down through
    // every mark's own `iced_image`, because iced has no opacity wrapper for a
    // container. Half of it — a conditional that removed the row — would move
    // everything under it on every approach, which is the one thing this file
    // spends its longest comment forbidding.
    let _ = near;
    container(
        row![Space::new().width(Length::Fill), controls]
            .align_y(iced::Alignment::Center)
            .padding(theme::app_bar_pad()),
    )
    .height(Length::Fixed(theme::APP_BAR_H))
    .width(Length::Fill)
    .into()
}

/// **The three window controls** — minimise, maximise, close, in that order,
/// at the bar's right end.
///
/// Fixed rather than read from the desktop, on the owner's decision of
/// 2026-08-10: *"I don't mind if we have the controls on the right hand side
/// as long as we have a sensible consistent pattern"*. A `chrome` module that
/// read GNOME's `button-layout` and KDE's `kwinrc` and mirrored the bar was
/// built and then deleted against that sentence — it was a conditional, a
/// startup subprocess and a second arrangement, bought to serve a preference
/// the owner does not hold.
///
/// The order is the Windows/GNOME-default one, which is also the order every
/// GTK and Qt application on the owner's desktop draws. **macOS is the known
/// cost** and is recorded in ADR-0040 §4 rather than papered over.
///
/// The slot is [`theme::APP_BAR_BUTTONS_W`] and the run is right-aligned in
/// it, so the reservation is the geometry the budget adds up.
fn window_controls(maximized: bool, ink: Ink) -> Element<'static, Message> {
    let line = row![
        window_button(WindowControl::Minimise, maximized, ink),
        window_button(WindowControl::Maximise, maximized, ink),
        window_button(WindowControl::Close, maximized, ink),
    ]
    .spacing(theme::CONTROL_CLUSTER_GAP)
    .align_y(iced::Alignment::Center);
    container(line)
        .width(Length::Fixed(theme::APP_BAR_BUTTONS_W))
        .align_x(alignment::Horizontal::Right)
        .into()
}

/// One of the three buttons zone 5 holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowControl {
    /// Put the window down.
    Minimise,
    /// Fill the screen, or come back off it — one control, two states.
    Maximise,
    /// Close the window, which for baz means quit (`app.rs`'s one exit path).
    Close,
}

/// One window control.
///
/// # Why these are icons, and why that needs no new licence
///
/// Doc 10 §3.4's enumerated list of *two* symbols that may stand as a **door's**
/// whole label is untouched by these three, because they are not doors: they
/// navigate nowhere, and L8.4 does not reach them. What they must clear is the
/// icon-only law itself (doc 10 §3.1), and they clear all three of its tests
/// more comfortably than anything already on the sheet: minimise, maximise and
/// close are standardized across every desktop this audience arrives from **in
/// symbol and in position**, baz's semantics are the convention's exactly
/// (`window::minimize`, `window::toggle_maximize`, `window::close`), and the
/// one control with two states says which state it is in by changing its
/// drawing and its word rather than by keeping one and hoping.
///
/// Each carries its tooltip — the accessible name in a toolkit with no
/// accessibility tree (ADR-0017 §4c) — and the [`theme::TRANSPORT_HIT`] 32 box
/// law L7 sets as the floor.
fn window_button(control: WindowControl, maximized: bool, ink: Ink) -> Element<'static, Message> {
    let (glyph, word, message, named) = match control {
        WindowControl::Minimise => (
            icon::Glyph::WindowMinimise,
            "Minimise",
            Message::WindowMinimised,
            Control::WindowMinimise,
        ),
        WindowControl::Maximise if maximized => (
            icon::Glyph::WindowRestore,
            "Restore",
            Message::WindowMaximiseToggled,
            Control::WindowMaximise,
        ),
        WindowControl::Maximise => (
            icon::Glyph::WindowMaximise,
            "Maximise",
            Message::WindowMaximiseToggled,
            Control::WindowMaximise,
        ),
        WindowControl::Close => (
            icon::Glyph::Close,
            "Close",
            Message::Quit,
            Control::WindowClose,
        ),
    };
    glyph_button(glyph, word, message, named, ink)
}

/// The bar's one control anatomy: a sprite in a [`theme::TRANSPORT_HIT`] box,
/// inked by the pointer through the same crossings and the same 90 ms tween as
/// the transport's (ADR-0020 §2.1), named by a tooltip below it.
///
/// One function because this bar holds five buttons of one kind, and five
/// copies of an anatomy is five things that can drift.
fn glyph_button(
    glyph: icon::Glyph,
    word: &'static str,
    message: Message,
    named: Control,
    ink: Ink,
) -> Element<'static, Message> {
    let room = theme::active();
    let mark = container(
        iced_image(icon::handle(glyph))
            .width(Length::Fixed(theme::ICON_PX))
            .height(Length::Fixed(theme::ICON_PX))
            .opacity(theme::glyph_ink(
                true,
                false,
                ink.hover(named),
                ink.pressed(named),
            )),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center);
    let control = button(mark)
        .width(Length::Fixed(theme::TRANSPORT_HIT))
        .height(Length::Fixed(theme::TRANSPORT_HIT))
        .padding(0)
        // The bar's own ground, so the hover wash is a step up from `recess`
        // rather than from the wall — every row-shaped control names the
        // surface it stands on, and so does every box-shaped one.
        .style(move |_theme, status| theme::transport(room, room.recess, status))
        .on_press(message);
    let named_control = tooltip(
        control,
        text(word)
            .size(theme::SIZE_CAPTION)
            .line_height(theme::LEADING_CAPTION),
        // Below, always: this bar stands on the window's own top edge and a
        // tip above any of its controls would clip off the screen.
        tooltip::Position::Bottom,
    )
    .gap(theme::GAP_XS)
    .padding(theme::GAP_XS)
    .style(move |_theme| theme::tooltip(room));
    mouse_area(named_control)
        .on_enter(Message::ControlEntered(named))
        .on_exit(Message::ControlLeft(named))
        .into()
}

#[cfg(test)]
mod tests {
    /// This file's own source — the idiom every view module in this crate uses
    /// to assert a claim about what is *drawn*, in a toolkit whose widget tree
    /// cannot be walked.
    fn source() -> String {
        include_str!("app_bar.rs").replace("\r\n", "\n")
    }

    /// **The window buttons exist exactly when baz owns the window's chrome.**
    ///
    /// The owner, 2026-08-10, looking at the shipped state: *"until we have no
    /// window chrome, remove the window controls..."*. With the platform's
    /// title bar above baz's own band, minimise/maximise/close were drawn
    /// twice, and one pair did nothing the other did not.
    ///
    /// **Not removed — conditional**, which is why this is worth a test rather
    /// than a deletion: the same rule that hides them today draws them the day
    /// `decorations` goes off, with no second edit and no chance of a build
    /// that owns its chrome and has no way to close.
    ///
    /// Asserted on the source because iced's widget tree cannot be walked (see
    /// [`source`]): the call must be *inside* the condition, so a refactor that
    /// hoists it out fails here rather than in somebody's window.
    #[test]
    fn the_window_buttons_are_conditional_on_owning_the_chrome() {
        // Only the code: this module's own prose names the call too, and a
        // test that counted itself would be counting the wrong thing.
        let src = source();
        let src = src
            .split_once("\nmod tests {")
            .expect("the test module")
            .0
            .to_owned();
        assert!(
            src.contains("owns_chrome.then(|| window_controls(maximized, ink))"),
            "the buttons are no longer drawn behind the chrome question"
        );
        // **Every call site is behind the question**, rather than there being
        // only one call site. There are two now — the ordinary bar and the
        // chromeless strip, which keeps the buttons because hiding baz's
        // title bar would otherwise take the only close button with it — so
        // counting sites stopped being the same claim as *none of them
        // escapes*. This checks the claim instead.
        for (at, _) in src.match_indices("window_controls(maximized, ink)") {
            let before = &src[..at];
            let guarded = before.ends_with("owns_chrome.then(|| ")
                || before
                    .rsplit_once("if owns_chrome {")
                    .is_some_and(|(_, after)| !after.contains('}'));
            assert!(
                guarded,
                "a call to `window_controls` at byte {at} is not behind the \
                 chrome question, so a build that owns its chrome could draw \
                 the buttons twice — or one that does not could draw a second \
                 set over the system's own"
            );
        }
        // **And absent means no child, not a child of no width.** `Row`'s
        // spacing falls between every pair of *children*, so a placeholder
        // `Space` still collects a `GAP_LG` and pushes everything to its left
        // 16 px off the window's gutter. That is exactly what shipped, and it
        // is 16 of the 25 px the owner saw between the gear and the index rail
        // on 2026-08-10. `push_maybe(None)` pushes nothing
        // (`iced_widget-0.13.4/src/row.rs:148`).
        assert!(
            src.contains("if let Some(buttons) = buttons")
                && src.contains("line = line.push(buttons)"),
            "the buttons are no longer pushed conditionally into the row"
        );
        assert!(
            !src.contains("Space::new().width(Length::Shrink)"),
            "a zero-width placeholder is back in the bar's row, and it takes a \
             GAP_LG seam with it — the trailing control no longer stands on \
             the window's gutter"
        );
    }

    /// **The trailing gutter is the ink gutter, and the view spends it from
    /// one place.**
    ///
    /// The owner, 2026-08-10: *"the settings cog is padded in quite a bit and
    /// does not align with the rail"*. The arithmetic, the measurement and the
    /// rule live in `theme::app_bar_pad` and
    /// `theme::the_bars_trailing_ink_lands_on_the_windows_gutter`; what this
    /// file has to hold is that it **asks for them** rather than rebuilding a
    /// padding of its own, because a view that spelled `HANG` on both edges
    /// would silently put the bar's trailing glyph 8 px inside the line the
    /// index rail draws its letters on and nothing would look broken.
    #[test]
    fn the_band_hangs_from_the_gutter_the_theme_derives() {
        let source = source();
        let code = source.split("#[cfg(test)]").next().expect("a head");
        assert!(
            code.contains(".padding(theme::app_bar_pad())"),
            "the band no longer takes its padding from the theme"
        );
        assert!(
            !code.contains("theme::pad(theme::APP_BAR_PAD_V"),
            "the band has gone back to a symmetric gutter"
        );
    }

    /// **Zone 1 is the application's mark, and it is not a control.**
    ///
    /// The owner, 2026-08-10: *"we probably want an icon for our app to show in
    /// the bar"*. Two things are worth pinning and they are both about what
    /// zone 1 *is not*:
    ///
    /// 1. **It is not on the glyph sheet.** [`icon::app_mark`] is the
    ///    launcher's own full-colour PNG, decoded once; `icon::handle` is a
    ///    coverage sprite the room inks. Drawing zone 1 through `handle` would
    ///    mean somebody had flattened the mark to a monochrome outline and made
    ///    a second master of it.
    /// 2. **It is not a button.** L8.5's statement/control split is the whole
    ///    reason this zone can be a bare symbol with no tooltip, and it is also
    ///    what keeps those 16 px part of the band that drags the window. A
    ///    `button` here would take the press the title bar needs.
    #[test]
    fn the_windows_name_is_the_applications_mark_and_not_a_control() {
        let source = source();
        let code = source.split("#[cfg(test)]").next().expect("a head");
        let rest = code.split_once("fn mark()").expect("zone 1").1;
        let body = &rest[..rest.find("\n}\n").expect("a function ends")];
        assert!(
            body.contains("icon::app_mark()"),
            "zone 1 no longer draws the application's own mark"
        );
        assert!(
            !body.contains("icon::handle(") && !body.contains("Glyph::"),
            "zone 1 draws a sheet glyph: the application's icon is a \
             full-colour asset with one master (packaging/icons), and a \
             monochrome outline of it would be a second"
        );
        assert!(
            !body.contains("button(") && !body.contains("tooltip("),
            "zone 1 has become a control, which takes a press the band needs \
             to drag the window and spends a licence L8.5 does not require"
        );
    }

    /// **The bar's admission rule, asserted rather than merely written.**
    ///
    /// The owner's own words, 2026-08-10: *"adding controls that apply to all
    /// windows makes sense in the top bar"* — read on a single-window product
    /// as **all places**. A control enters this bar only if it applies
    /// everywhere; if it applies to one place, the bar is not where it goes.
    ///
    /// **The failure mode of a resident bar is accretion**, one locally
    /// argued admission at a time — which is precisely how the Library strip
    /// got crowded enough for the owner to complain about it. So the closed
    /// tenancy is pinned here in both directions: the four tenants that are
    /// in, and the named surfaces whose controls may not migrate.
    #[test]
    fn the_bar_holds_the_window_and_the_application_and_nothing_else() {
        let source = source();
        let code = source.split("#[cfg(test)]").next().expect("a head");
        for tenant in [
            "views::search::well",
            "Message::ToggleSettings",
            "Message::DensityStep",
            "Message::WindowMinimised",
            "Message::WindowMaximiseToggled",
            "Message::Quit",
            "Message::WindowDragged",
            "Message::WindowMenuRequested",
        ] {
            // `DensityStep` is spent through `views::density_marks`, so the
            // literal is in this file's prose rather than its code; the rest
            // are pressed here.
            assert!(
                source.contains(tenant),
                "the app bar no longer carries {tenant}"
            );
        }
        // **What may not migrate in**, each named by the surface it belongs
        // to, because a list of forbidden *messages* would be a list somebody
        // could satisfy by inventing a new one:
        //
        // - `GroupKeySelected` — the arrangement is the wall's;
        // - `PlayPause` / `PlayEverything` / `ToggleShuffle` — playback is the
        //   bottom bar's, and its ratchet says nothing leaves it either;
        // - `PlayAll` — the Library's alone, which is why it is out rather
        //   than moved;
        // - `place_header` — a place's identity, breadcrumb and note belong to
        //   the place.
        for stranger in [
            "PlayAll",
            "GroupKeySelected",
            "PlayPause",
            "PlayEverything",
            "ToggleShuffle",
            "place_header",
        ] {
            assert!(
                !code.contains(stranger),
                "{stranger} applies to one place or one subject, and the bar \
                 holds what applies to all of them (ADR-0040 §1, the owner's \
                 rule)"
            );
        }
    }

    /// **The whole band is the window's handle, and the controls still take
    /// their own presses.**
    ///
    /// This is the one property whose absence would make a borderless window
    /// *broken* rather than worse: a title bar you cannot drag is a window you
    /// cannot move. It rests on iced 0.13's `mouse_area` running its content's
    /// handler first and returning on capture
    /// (`iced_widget-0.13.4/src/mouse_area.rs:211`), so the assertion is that
    /// the band is wrapped rather than that a gap is.
    #[test]
    fn the_whole_band_moves_the_window_and_the_controls_keep_their_presses() {
        let source = source();
        let code = source.split("#[cfg(test)]").next().expect("a head");
        let rest = code
            .split_once("let band = mouse_area(")
            .expect("the band is the handle")
            .1;
        for gesture in [
            ".on_press(Message::WindowDragged)",
            ".on_right_press(Message::WindowMenuRequested)",
        ] {
            assert!(
                rest.contains(gesture),
                "the band no longer answers {gesture} — a title bar that \
                 cannot be dragged is a window that cannot be moved"
            );
        }
        // …and every control in it is a `button`, which is what captures the
        // press before the band sees it. Five of them: four window-ish
        // controls through one anatomy, plus the marks' own.
        assert!(
            code.contains("let control = button(mark)"),
            "the bar's controls are no longer buttons, so the band would \
             swallow their presses"
        );
    }

    /// **Both slots are reserved, and the reservation is what makes one bar
    /// possible.**
    ///
    /// If the marks' slot collapsed where a place hangs no works, the gear and
    /// the window buttons would sit 96 px further out on four of the seven
    /// places and the frame would move as you navigated. The `None` arm must
    /// therefore draw the slot's full width, and the container must declare it.
    #[test]
    fn the_display_options_slot_is_held_where_no_works_hang() {
        let source = source();
        let rest = source
            .split_once("fn marks(density: Option<crate::shelf::Density>)")
            .expect("the slot")
            .1;
        let body = &rest[..rest.find("\n}\n").expect("a function ends")];
        assert!(
            body.contains("None => Space::new()")
                && body.contains(".width(Length::Fixed(theme::APP_BAR_MARKS_W))"),
            "the empty slot no longer holds its width, so the bar's right \
             cluster moves between places"
        );
        assert!(
            body.contains(".width(Length::Fixed(theme::APP_BAR_MARKS_W))"),
            "the slot does not declare its reserved width"
        );
    }

    /// **One arrangement, and the zones are in their stated order.**
    ///
    /// The owner's acceptance criterion for this bar is *"a sensible
    /// consistent pattern"*, and the thing that would quietly break it is a
    /// second arrangement — a mirror for one platform, a variant for one
    /// place. So what is pinned is that the line is built **unconditionally**,
    /// in the order the module documents: mark, history, search, handle, view,
    /// application, window.
    #[test]
    fn the_bar_has_one_arrangement_and_it_is_the_stated_order() {
        let source = source();
        let code = source.split("#[cfg(test)]").next().expect("a head");
        assert!(
            code.contains("let mut line = row![name, history, search, gap, furniture]")
                && code.contains("line = line.push(buttons)"),
            "the bar's zones are no longer in the order the pattern states"
        );
        assert!(
            !code.contains("Side::") && !code.contains("controls.side"),
            "a second arrangement has come back: one layout everywhere is the \
             whole of what the owner asked for by *consistent*"
        );
        // Zone 5 before zone 6 inside the right cluster — the view, then the
        // application. Scope widens rightward, and that is the rule a future
        // tenant is placed by.
        let application = code
            .split_once("let application = row![")
            .expect("the application cluster")
            .1;
        let application = &application[..application.find(";\n").expect("a binding ends")];
        assert!(
            application.contains("crate::views::status::bell(health)")
                && application.contains("gear(ink)")
                && application.contains("theme::CONTROL_CLUSTER_GAP"),
            "the application's two doors are no longer one cluster on the \
             cluster seam"
        );
        let furniture = code
            .split_once("let furniture = row![")
            .expect("the view/application cluster")
            .1;
        let furniture = &furniture[..furniture.find(";\n").expect("a binding ends")];
        assert!(
            furniture.contains("marks(density, visualization)")
                && furniture.contains("application")
                && furniture.contains("theme::GAP_LG"),
            "the display options and the application's doors no longer stand \
             in the scope-widening order the bar promises, one zone seam apart"
        );

        // **Two seams in this bar and no third.** The owner's *"the top bar
        // has weird spacing as well for icons/controls"* was three rhythms for
        // one kind of object: `GAP_XS` inside the history pair and the window
        // buttons, `GAP_LG` between the bell and the gear. Every seam between
        // controls is now the cluster's 8 or the zone's 16 — a detent run
        // (`density_marks`, `visualizer::marks`) touches, because it is one
        // control with several states rather than a cluster of controls.
        assert!(
            !code.contains("spacing(theme::GAP_XS)"),
            "a third control rhythm is back in the app bar"
        );
        assert_eq!(
            code.matches("spacing(theme::CONTROL_CLUSTER_GAP)").count(),
            4,
            "the bar's clusters — history, the application's doors, the window \
             buttons, and the chromeless strip that keeps those buttons when \
             the rest of the bar goes — no longer all stand on the cluster seam"
        );
    }
}
