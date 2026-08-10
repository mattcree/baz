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
//! about. The search well is the lane's (ADR-0036 gave it one meaning and one
//! home). A place's identity, breadcrumb and note are the place's. The
//! transport is the bottom bar's, whose ratchet says nothing leaves it either.
//! The wall's arrangement keys are the wall's. All of it is pinned in this
//! file's tests.
//!
//! # The zones, and the pattern that decides them
//!
//! The owner, approving the right-hand placement: *"I don't mind if we have
//! the controls on the right hand side as long as we have **a sensible
//! consistent pattern**"*. So the bar is four named zones, in this order at
//! every width and in every place:
//!
//! ```text
//!   baz  ·······························  ▤ ▤ ▤ ▤    ⚙    ─  □  ✕
//!    1              2                        3       4       5
//!   name          handle                    view    app    window
//! ```
//!
//! 1. **The window's name.** What this window *is* — the one thing a title bar
//!    says that nothing else in baz says, and the only zone that is a
//!    statement rather than a control (L8.5).
//! 2. **The handle.** Never a tenant. It is the gesture surface: press and
//!    travel moves the window, press twice maximises it. A control admitted
//!    here would be a control that eats the window's own gesture.
//! 3. **The view** — controls that change *how the place you are in is shown*.
//!    The display options today.
//! 4. **The application** — doors to what the application holds rather than
//!    what a place does. The gear today.
//! 5. **The window** — controls that act on the window as an object of the
//!    desktop. Minimise, maximise, close, in that order, always.
//!
//! **The pattern is that scope widens rightward**: the view you are in, then
//! the application around it, then the window around that. It is a rule and
//! not a description, which is the test the owner set — *given a new control,
//! the rule says where it goes without an argument*. Ask what it acts on. One
//! place: it does not enter the bar. The view, in every place: zone 3. The
//! application: zone 4. The window: zone 5. Two people reading that cannot
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
//! # Both reserved slots are reserved in every place
//!
//! [`theme::APP_BAR_MARKS_W`] and [`theme::APP_BAR_BUTTONS_W`] are fixed
//! widths, held whether or not anything is drawn in them. That is what lets
//! this bar be *the same bar* on all seven places while still obeying
//! ADR-0028's *absent, not disabled*: on a record's page, a playlist's, Now
//! playing and Settings there are no works to hang, so there are no marks —
//! and the gear and the buttons do not move a pixel to notice.
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
    Space, button, column, container, horizontal_rule, image as iced_image, mouse_area, row, text,
    tooltip,
};
use iced::{Element, Length, alignment};

use crate::app::Message;
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
/// nothing to the left of them moves when they appear, because they sit at the
/// trailing edge.
pub(crate) fn view(
    window_w: f32,
    density: Option<crate::shelf::Density>,
    maximized: bool,
    owns_chrome: bool,
    ink: Ink,
) -> Element<'static, Message> {
    let room = theme::active();
    // The window's name — what the platform title bar said before baz took the
    // band over. Quiet: the metadata size in the faintest readout ink, which
    // is one step below the group keys and two below anything you press. It is
    // a **statement**, not a control (L8.5), and it is the one thing in this
    // bar that is neither.
    let name: Element<'static, Message> = container(
        text("baz")
            .size(theme::SIZE_META)
            .line_height(theme::LEADING_META)
            .font(theme::MEDIUM)
            .color(room.paper_faint)
            .wrapping(text::Wrapping::None),
    )
    .width(Length::Fixed(theme::APP_BAR_NAME_W))
    .height(Length::Fixed(theme::TRANSPORT_HIT))
    .align_y(alignment::Vertical::Center)
    .into();
    let furniture = row![marks(density), gear(ink),]
        .spacing(theme::GAP_LG)
        .align_y(iced::Alignment::Center);
    // Absent rather than disabled, and absent rather than a held slot: see the
    // note on `owns_chrome` above.
    let buttons: Element<'static, Message> = if owns_chrome {
        window_controls(maximized, ink)
    } else {
        Space::with_width(Length::Shrink).into()
    };
    // The bar's one flexible region. It is a plain `Space` and **not** a
    // handle of its own: the whole bar is the handle (see below), so a second
    // one here would be two answers to one gesture.
    let gap = Space::with_width(Length::Fill);
    // **Zones 1…5, in that order, at every width and in every place.** One
    // arrangement rather than a mirrored pair: the owner's *"as long as we
    // have a sensible consistent pattern"* is better served by one layout
    // everywhere than by two that are each correct on one platform.
    let line = row![name, gap, furniture, buttons]
        .spacing(theme::GAP_LG)
        .align_y(iced::Alignment::Center);
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
            // One window gutter, law L1 — the same `HANG` the wall, both
            // strips, the now-playing bar and the index rail hang from. This
            // bar spans the **window**, not the body, so its right edge is the
            // window's `W − HANG`: the window buttons belong to the window and
            // may not be inset by a lane.
            .padding(theme::pad(theme::APP_BAR_PAD_V, theme::HANG))
            .width(Length::Fixed(window_w))
            // The now-playing bar's own ground. The window's two chrome bands
            // are one surface interrupted by the places between them, and
            // giving the top band a plane of its own would have made them two
            // ideas.
            .style(move |_theme| theme::bar(room)),
    )
    // Press and drag moves the window; press twice quickly maximises or
    // restores it. Both are one message, because iced 0.13's `mouse_area` has
    // no `on_double_click` — that arrived in 0.14 — so the second press is
    // recognised by the shell against the first's clock (`app.rs`'s
    // `Message::WindowDragged`).
    .on_press(Message::WindowDragged)
    // The platform's own window menu, where the platform has one.
    .on_right_press(Message::WindowMenuRequested);
    column![
        band,
        horizontal_rule(1).style(move |_theme| theme::hairline(room, room.wall)),
    ]
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
fn marks(density: Option<crate::shelf::Density>) -> Element<'static, Message> {
    let inner: Element<'static, Message> = match density {
        Some(current) => crate::views::density_marks(current),
        None => Space::new(Length::Fixed(theme::APP_BAR_MARKS_W), Length::Shrink).into(),
    };
    container(inner)
        .width(Length::Fixed(theme::APP_BAR_MARKS_W))
        .align_x(alignment::Horizontal::Right)
        .into()
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
    .spacing(theme::GAP_XS)
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
        let at = src
            .find("let buttons: Element<'static, Message> = if owns_chrome {")
            .expect("the buttons are drawn behind the chrome question");
        let arm = &src[at..src[at..].find("};").expect("the arm ends") + at];
        assert!(
            arm.contains("window_controls(maximized, ink)"),
            "the buttons are no longer drawn in the owns-chrome arm"
        );
        assert_eq!(
            src.matches("window_controls(maximized, ink)").count(),
            1,
            "the buttons are drawn from more than one place, so one of them \
             can escape the condition"
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
        // - `SearchChanged` / `ClearSearch` — the well is the lane's, and
        //   ADR-0036 gave it one meaning and one home;
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
            "SearchChanged",
            "ClearSearch",
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
            body.contains("None => Space::new(Length::Fixed(theme::APP_BAR_MARKS_W)"),
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
    /// in the order the module documents: name, handle, view, application,
    /// window.
    #[test]
    fn the_bar_has_one_arrangement_and_it_is_the_stated_order() {
        let source = source();
        let code = source.split("#[cfg(test)]").next().expect("a head");
        assert!(
            code.contains("let line = row![name, gap, furniture, buttons]"),
            "the bar's zones are no longer in the order the pattern states"
        );
        assert!(
            !code.contains("Side::") && !code.contains("controls.side"),
            "a second arrangement has come back: one layout everywhere is the \
             whole of what the owner asked for by *consistent*"
        );
        // Zone 3 before zone 4 inside the right cluster — the view, then the
        // application. Scope widens rightward, and that is the rule a future
        // tenant is placed by.
        let furniture = code
            .split_once("let furniture = row![")
            .expect("the view/application cluster")
            .1;
        assert!(
            furniture.starts_with("marks(density), gear(ink)"),
            "the display options no longer stand inside the gear: scope \
             widens rightward, and this pair is the rule's smallest instance"
        );
    }
}
